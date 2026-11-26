#!/usr/bin/env python3
"""The explorer gateway host on a local v3 devnet: the observer node (install_observer_node.py's command line, no
miner key), the explorer gateway on it with the transaction index (POCOL_TX_DB), the holders index (POCOL_HOLDERS_DB)
and the miner map (pocol_geoip.py: a GeoIP database built from the synthetic test CSV, POCOL_GEOIP_DB, and a
throwaway geo-reporters credential made for this run), transactions of every kind, the Transactions page newest first
against the canonical chain, search, a pinned snapshot, a gateway restart, the Holders pages and statistics against
the node's own state, an operator node report (pocol_geo_reporter.py's node read and report on node a) placing node
a's miner, and a reorganization of the observer's chain (a partitioned devnet whose isolated branch loses): the
transaction and holder pages pinned to the lost branch answer 409 reset and both indexes converge to the healed chain.

    POCOLD=/path/to/pocold python3 -B deploy/explorer/e2e_explorer_devnet.py [WORK]

devnet.py's fixed ports (30300-30302, 8540-8542) must be free; the observer's RPC and the gateway take free loopback
ports. Local only, throwaway devnet accounts (pocol-sign --account, never a funded key), no request leaves loopback.

Two devnet differences from Mainnet, both handled here and not in the products:
- Genesis supply. The holders index's rule is the profile's (pocol_holders: |M_0List| x 10^24 wei, every founder's
  allocation plus its bond); a devnet (`pocold init-devnet`) also funds its --accounts and gives miners only their bond.
  The gateway is therefore started through GATEWAY_LAUNCH, which runs gateway.py's own main() with HolderIndex's
  documented `genesis_wei` argument set to this devnet's genesis supply (its alloc.json plus the system accounts'
  balances at block 0, read from the node). Without it every snapshot would rightly publish complete false with that
  difference.
- Source addresses. Every devnet address is loopback, which the GeoIP code never looks up. Behind Caddy the gateway
  takes the client address from the last X-Forwarded-For hop of a loopback peer (gateway.client_address); this test,
  a loopback client, sends the report with an X-Forwarded-For naming an address the synthetic CSV places in FR, the way
  Caddy would forward a reporting server's public address. No product code has a test-only path for this.

Prints PASS/FAIL lines and a JSON summary; exit status 1 on any failure."""
import contextlib
import hashlib
import hmac
import io
import json
import os
import secrets
import signal
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
REWARDS = ROOT / 'deploy' / 'rewards'
sys.path.insert(0, str(ROOT / 'prototypes' / 'pocold' / 'scripts'))
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(REWARDS))
import devnet as dn  # noqa: E402
import install_observer_node as observer  # noqa: E402
import pocol_geoip  # noqa: E402
import pocol_holders  # noqa: E402
import pocol_geo_reporter as reporter  # noqa: E402

BIN = Path(dn.BIN).parent
SIGN = os.environ.get('POCOL_SIGN', str(BIN / 'pocol-sign'))
REGISTRY, ESCROW = '0x0000000000000000000000000000000000c0c001', '0x0000000000000000000000000000000000c0c002'
SYSTEM_ACCOUNTS = [REGISTRY, ESCROW, '0x0000000000000000000000000000000000c0c003', '0x0000000000000000000000000000000000c0c004',
                   '0x0000000000000000000000000000000000c0c005', '0x0000000000000000000000000000000000c0c0ff', '0x' + 'ff' * 19 + 'fe']
# The synthetic CSV (deploy/rewards/testdata/geoip-sample.csv) places 5.0.2.0/24 in FR. It is the reporting server's
# source address as Caddy would forward it; nothing contacts it.
REPORT_SOURCE, REPORT_COUNTRY = '5.0.2.7', 'FR'
TEST_RELEASE = '2000-01'          # a release month no DB-IP file has: the database is the synthetic CSV
HOLDERS_RESET = {'error': 'The holder snapshot changed. Refreshing.', 'reset': True}
# gateway.py's main() with HolderIndex's genesis_wei (the devnet's genesis supply) as its default; argv: the rewards
# folder, the genesis supply in wei, gateway.py and its arguments.
GATEWAY_LAUNCH = '\n'.join([
    'import runpy, sys',
    'sys.path.insert(0, sys.argv[1])',
    'import pocol_holders',
    'genesis_wei, _init = int(sys.argv[2]), pocol_holders.HolderIndex.__init__',
    'def init(self, *args, **options):',
    '    options.setdefault("genesis_wei", genesis_wei)',
    '    _init(self, *args, **options)',
    'pocol_holders.HolderIndex.__init__ = init',
    'sys.argv = sys.argv[3:]',
    'runpy.run_path(sys.argv[0], run_name="__main__")'])
results, procs = [], []


class Transient(RuntimeError):
    """A read that met a moving chain (a new snapshot, a tip reorganization, the state window): read again."""


def free_port():
    with socket.socket() as s:
        s.bind(('127.0.0.1', 0))
        return s.getsockname()[1]


def port_free(port):
    with socket.socket() as s:
        return s.connect_ex(('127.0.0.1', port)) != 0


OBS_RPC, GW = free_port(), free_port()


def ok(name, cond, detail=''):
    results.append({'name': name, 'pass': bool(cond), 'detail': str(detail)[:400]})
    print(('PASS ' if cond else 'FAIL ') + name + (f' — {str(detail)[:400]}' if detail != '' else ''), flush=True)


def wait(pred, seconds, what):
    end, last = time.time() + seconds, None
    while time.time() < end:
        try:
            value = pred()
            if value:
                return value
        except Exception as error:  # noqa: BLE001 (the condition is retried until the deadline)
            last = error
        time.sleep(1)
    raise RuntimeError(f'timeout: {what}' + (f' ({last})' if last else ''))


def rpc(port, method, params=None, path='/'):
    body = json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': method, 'params': params or []}).encode()
    req = urllib.request.Request(f'http://127.0.0.1:{port}{path}', data=body, headers={'content-type': 'application/json'})
    with urllib.request.urlopen(req, timeout=15) as r:
        value = json.load(r)
    if 'error' in value:
        raise RuntimeError(value['error'])
    return value['result']


def get(path):
    """GET on the gateway. This test is one client of the gateway's budget (10 tokens a second, a history page costs
    5, a holders page 2, the miner map 5): a busy answer (429, or 503 while the budget is spent) is retried after a
    second, up to 20 times."""
    for attempt in range(20):
        try:
            with urllib.request.urlopen(f'http://127.0.0.1:{GW}{path}', timeout=15) as r:
                return r.status, json.load(r)
        except urllib.error.HTTPError as e:
            status, value = e.code, json.loads(e.read() or b'null')
            if status in (429, 503) and attempt < 19 and path.startswith('/api/transactions.php') or status == 429 and attempt < 19:
                time.sleep(1)
                continue
            return status, value


def post_report(body, headers):
    """POST /api/geo-report.php from this loopback client: the HTTP status (a 429 is retried)."""
    for attempt in range(20):
        req = urllib.request.Request(f'http://127.0.0.1:{GW}/api/geo-report.php', data=body, method='POST',
                                     headers={'Content-Type': 'application/json', **headers})
        try:
            with urllib.request.urlopen(req, timeout=15) as r:
                return r.status
        except urllib.error.HTTPError as e:
            e.read()
            if e.code == 429 and attempt < 19:
                time.sleep(1)
                continue
            return e.code


def spawn(name, args, work, env=None):
    log = open(os.path.join(work, name + '.log'), 'ab')
    p = subprocess.Popen(args, stdout=log, stderr=log, start_new_session=True, env=env)
    procs.append(p)
    return p


def stop(p):
    if p.poll() is None:
        os.killpg(p.pid, signal.SIGTERM)
        try:
            p.wait(60)
        except subprocess.TimeoutExpired:
            os.killpg(p.pid, signal.SIGKILL); p.wait(10)


def canonical(port):
    """(height, position, hash) of every transaction of the node's canonical chain, newest first."""
    head = int(rpc(port, 'eth_blockNumber'), 16)
    rows = []
    for n in range(head + 1):
        b = rpc(port, 'eth_getBlockByNumber', [hex(n), False])
        rows += [(n, i, h) for i, h in enumerate(b['transactions'])]
    return sorted(rows, reverse=True), head


def indexed(size=100):
    """Every row of the index's newest-first pages (and the first page's answer)."""
    rows, page, first = [], 1, None
    while True:
        status, value = get(f'/api/transactions.php?page={page}&size={size}&order=newest&q=')
        if status != 200:
            raise RuntimeError(f'/api/transactions.php {status} {value}')
        data = value['data']
        first = first or data
        rows += [(int(r['blockNumber'], 16), int(r['transactionIndex'], 16), r['hash']) for r in data['rows']]
        if len(data['rows']) < size:
            return rows, first
        page += 1


# ---------------- the holders index ----------------
def holder_rows(pin=None, order='balance_desc', kind='all', size=100):
    """Every row of one holders snapshot (the newest, or `pin`) and the first page's answer; Transient when the pages
    met another snapshot."""
    rows, page, first = [], 1, None
    while True:
        query = f'/api/holders.php?page={page}&size={size}&order={order}&type={kind}'
        if pin:
            query += f'&height={pin["height"]}&hash={pin["hash"]}'
        status, value = get(query)
        if status != 200:
            raise Transient(f'{query}: {status} {value}')
        data = value['data']
        if first is None:
            first = data
            pin = pin or {'height': data['snapshot']['height'], 'hash': data['snapshot']['hash']}
        if data['snapshot']['hash'] != pin['hash']:
            raise Transient('the pages met another snapshot')
        rows += data['rows']
        if page * size >= data['total']:
            return rows, first
        page += 1


def chain_to(port, height):
    """The light blocks 0..height of the node's canonical chain, linked by parent hash; Transient if it moved."""
    blocks = [rpc(port, 'eth_getBlockByNumber', [hex(n), False]) for n in range(height + 1)]
    if any(b is None for b in blocks) or any(blocks[n]['parentHash'] != blocks[n - 1]['hash'] for n in range(1, height + 1)):
        raise Transient('the chain moved while it was read')
    return blocks


def audit(port, genesis_wei, miners, min_height=0, expect=None):
    """One holders snapshot (the newest at or above min_height) read whole and checked against the node at its height
    h: every row's available balance is eth_getBalance(address, h); base fees, blocks mined, heldTotal and
    burnedTotal are recomputed from the node; the statistics of the same snapshot agree with the rows; `expect` maps
    an address to (transactions, available wei or None), or is a function of the canonical blocks 0..h giving it. Returns
    {h, snapshot, rows, first, stats, problems}; Transient while the snapshot cannot be checked (it is not the newest
    at min_height yet, the statistics show another snapshot, h left the node's 32-block state window, or the block at
    h changed during the reads)."""
    rows, first = holder_rows()
    snap = first['snapshot']
    h = snap['height']
    if h < min_height:
        raise Transient(f'snapshot {h} below {min_height}')
    status, value = get('/api/holders-stats.php')
    if status != 200 or value['data']['snapshot'] != snap:
        raise Transient('the statistics show another snapshot')
    stats = value['data']
    blocks = chain_to(port, h)
    if blocks[h]['hash'].lower() != snap['hash']:
        raise Transient('the snapshot block is no longer canonical')
    at = hex(h)
    try:
        node = {r['address']: int(rpc(port, 'eth_getBalance', [r['address'], at]), 16) for r in rows}
        held_total = int(rpc(port, 'eth_getStorageAt', [REGISTRY, '0x4', at]), 16)
        burned = int(rpc(port, 'eth_getStorageAt', [ESCROW, '0x0', at]), 16)
        escrow_balance = int(rpc(port, 'eth_getBalance', [ESCROW, at]), 16)
    except RuntimeError as error:
        raise Transient(f'state at {h}: {error}') from None
    if rpc(port, 'eth_getBlockByNumber', [at, False])['hash'].lower() != snap['hash']:
        raise Transient('the block at h changed during the reads')
    problems = []
    def need(cond, what):
        if not cond:
            problems.append(what)
    supply = int(first['supply']['currentSupplyWei'])
    total = lambda r: int(r['totalWei'])
    need(first['index']['state'] == 'ready', f'index state {first["index"]["state"]}')
    need([r['rank'] for r in rows] == list(range(1, len(rows) + 1)) and len(rows) == first['total'], 'ranks are not 1..total')
    need(all((total(a), b['address']) >= (total(b), a['address']) and (total(a) > total(b) or a['address'] < b['address'])
             for a, b in zip(rows, rows[1:])), 'not ordered by Total descending, ties by address ascending')
    need(sum(map(total, rows)) == supply, f'sum of Total {sum(map(total, rows))} != currentSupplyWei {supply}')
    need(all(int(r['sharePercentE10']) == total(r) * 10 ** 12 // supply for r in rows), 'a share is not floor(Total x 10^12 / S)')
    need(all(int(r['availableWei']) == node[r['address']] for r in rows),
         'available != eth_getBalance at h: ' + ', '.join(r['address'] for r in rows if int(r['availableWei']) != node[r['address']]))
    need(all(total(r) == int(r['availableWei']) + int(r['bondedWei']) for r in rows if r['type'] != 'system'), 'Total != available + bonded')
    fees = sum(int(b['baseFeePerGas'], 16) * int(b['gasUsed'], 16) for b in blocks[1:])
    s = stats['supply']
    issued = int(s['issuedWei'])
    need(int(s['genesisWei']) == genesis_wei, f'genesisWei {s["genesisWei"]} != the devnet genesis supply {genesis_wei}')
    need(int(s['mintedWei']) == genesis_wei + issued, 'minted != genesis + issued')
    need(int(s['baseFeeBurnedWei']) == fees, f'base fees {s["baseFeeBurnedWei"]} != the sum over blocks 1..h {fees}')
    need(int(s['protocolBurnedWei']) == burned and int(s['bondedWei']) == held_total, 'protocol burned or bonded != the storage at h')
    need(int(s['totalBurnedWei']) == fees + burned and int(s['currentSupplyWei']) == genesis_wei + issued - fees - burned == supply,
         'current supply != genesis + issued - base fees - protocol burned')
    # The supply identity on the node's own balances: every balance the index found, at h, sums to G(h) - F(h).
    need(sum(node.values()) == genesis_wei + issued - fees == int(stats['verification']['balancesSumWei']) == int(stats['verification']['expectedSumWei']),
         f'sum of eth_getBalance at h {sum(node.values())} != genesis + issued - base fees {genesis_wei + issued - fees}')
    need(first['verification']['complete'] is True and stats['verification']['complete'] is True
         and stats['verification']['unlocatedWei'] == '0' and stats['verification']['accountsMissing'] in (0, None),
         f'verification {stats["verification"]}')
    need(int(s['pendingRewardsWei']) == escrow_balance - held_total - burned, 'pending rewards != Escrow balance - heldTotal - burnedTotal')
    by = {r['address']: r for r in rows}
    if escrow_balance - held_total - burned > 0:
        need(ESCROW in by and by[ESCROW]['type'] == 'system' and by[ESCROW]['label'] == 'RewardEscrow'
             and total(by[ESCROW]) == escrow_balance - held_total - burned, 'the RewardEscrow row')
    # The founders (the devnet's miners): the bond on the miner ID, never also on its distinct reward address.
    for m in miners:
        row, rew = by.get(m['id']), by.get(m['reward'])
        mined = sum(1 for b in blocks[1:] if b['miner'].lower() == m['id'])
        need(row is not None and row['type'] == 'miner' and int(row['bondedWei']) == m['bond'] and row['rewardAddress'] == m['reward']
             and row['minerIds'] == [] and row['blocksMined'] == mined, f'miner row {m["label"]}: {row} (bond {m["bond"]}, mined {mined})')
        need(rew is None or (rew['type'] == 'reward' and rew['minerIds'] == [m['id']] and rew['bondedWei'] == '0'
                             and rew['rewardAddress'] is None), f'reward row {m["label"]}: {rew}')
    need(len({m['reward'] for m in miners} | {m['id'] for m in miners}) == 2 * len(miners), 'the founders\' reward addresses are not distinct')
    # The statistics of the same snapshot.
    people = [r for r in rows if r['type'] != 'system']
    hs = stats['holders']
    need(hs['count'] == len(people) and hs['withBonded'] == sum(1 for r in people if int(r['bondedWei']) > 0)
         and hs['withAvailable'] == sum(1 for r in people if int(r['availableWei']) > 0), f'holder counts {hs}')
    for n in (1, 10, 50, 100):
        top = stats['concentration'][f'top{n}']
        need(int(top['totalWei']) == sum(map(total, people[:n])) and int(top['sharePercentE10']) == sum(map(total, people[:n])) * 10 ** 12 // supply,
             f'concentration top{n}')
    for d in stats['distribution']:
        inside = [total(r) for r in people if int(d['minWei']) <= total(r) and (d['maxWei'] is None or total(r) < int(d['maxWei']))]
        need(d['accounts'] == len(inside) and int(d['totalWei']) == sum(inside), f'distribution bucket {d}')
    need(sum(d['accounts'] for d in stats['distribution']) == len(people), 'the distribution buckets do not cover every holder')
    need(int(s['systemWei']) == sum(map(total, (r for r in rows if r['type'] == 'system')))
         and int(s['circulatingWei']) == sum(int(r['availableWei']) for r in people) and int(s['availableWei']) == sum(node.values()),
         'system, circulating or available sums')
    rewards = stats.get('rewards') or {}
    need((rewards.get('from') is None) == all(r['rewardsWei'] is None for r in rows)
         and (rewards.get('from') is None or all(isinstance(r['rewardsWei'], str) for r in rows)), f'rewards {rewards}')
    # Mining rewards since the index start, checked on the node: a devnet founder's reward address sends nothing and
    # receives only reward payments, so its rewards from block `from` through h are its balance gain since block
    # from - 1, while that block is inside the node's state window.
    rewards_checked = 0
    if rewards.get('from') is not None and rewards.get('to') == h and rewards['from'] >= 1:
        for r in rows:
            if r['type'] != 'reward' or r['transactions'] != 0:
                continue
            try:
                start = int(rpc(port, 'eth_getBalance', [r['address'], hex(rewards['from'] - 1)]), 16)
            except RuntimeError:
                break  # block from - 1 left the state window
            need(int(r['rewardsWei']) == node[r['address']] - start,
                 f'{r["address"]}: rewards {r["rewardsWei"]} != its balance gain {node[r["address"]] - start} since block {rewards["from"] - 1}')
            rewards_checked += 1
    for address, (txs, balance) in ((expect(blocks) if callable(expect) else expect) or {}).items():
        row = by.get(address)
        need(row is not None and row['transactions'] == txs and (balance is None or int(row['availableWei']) == balance),
             f'{address}: {row} (expected {txs} transactions' + (f', {balance} wei)' if balance is not None else ')'))
    return {'h': h, 'snapshot': snap, 'rows': rows, 'first': first, 'stats': stats, 'problems': problems,
            'rewardsChecked': rewards_checked}


def checked(port, genesis_wei, miners, seconds, what, **options):
    """audit() retried while Transient, then one PASS/FAIL line."""
    value = wait(lambda: audit(port, genesis_wei, miners, **options), seconds, what)
    ok(what, not value['problems'], '; '.join(value['problems']) or
       f'snapshot {value["h"]}: {len(value["rows"])} holders, supply {value["first"]["supply"]["currentSupplyWei"]} wei')
    return value


def main():
    work = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix='pocol-explorer-e2e-')
    os.makedirs(work, exist_ok=True)
    print('work', work, flush=True)
    busy = [p for p in (30300, 30301, 30302, 8540, 8541, 8542) if not port_free(p)]
    if busy:
        raise RuntimeError(f'devnet ports busy: {busy}')
    with contextlib.redirect_stdout(io.StringIO()):  # init prints the devnet's whole devnet.json
        dn.init(os.path.join(work, 'dn'), 'a,b,c', ['--accounts', 'e,f,g', '--t-blk', '2', '--tau', '20', '--c', '8', '--d-att', '41',
                                                   '--fb-wait', '3', '--work-bits', '15'])
    net = os.path.join(work, 'dn', 'net')
    info = json.load(open(os.path.join(net, 'devnet.json')))
    chain, genesis = info['chainId'], info['genesisHash'].lower()
    dn.start(os.path.join(work, 'dn'), log=True)
    gw = None
    try:
        wait(lambda: int(rpc(8540, 'eth_blockNumber'), 16) >= 4, 180, 'devnet height 4')
        # The devnet's genesis supply: alloc.json and the system accounts' balances at block 0 (the founders' bonds in
        # RewardEscrow), read while block 0 is inside the node's 32-block state window.
        alloc = json.load(open(os.path.join(net, 'alloc.json')))
        genesis_wei = sum(int(v['balance']) for v in alloc.values())
        genesis_wei += sum(int(rpc(8540, 'eth_getBalance', [a, '0x0']), 16) for a in SYSTEM_ACCOUNTS if a not in alloc)
        records = {r['id'].lower(): r for r in rpc(8540, 'pocol_getMiners')}
        miners = [{'label': m['label'], 'id': m['id'].lower(), 'reward': m['reward'].lower(), 'bond': int(records[m['id'].lower()]['bond'], 16)}
                  for m in sorted(info['miners'], key=lambda m: m['label'])]
        ok('the devnet\'s three miners are registry records with distinct reward addresses and a held bond',
           len(miners) == 3 and all(m['bond'] > 0 and m['reward'] != m['id'] for m in miners), json.dumps(miners))
        # ---- the observer node: the installer's network files and command line, devnet paths and ports ----
        files = {name: open(os.path.join(net, name)).read() for name in observer.FILES}
        raw = json.dumps({'schema': 'pocol-network-v2', 'consensusRulesVersion': 2,
                          'seeds': ['127.0.0.1:30300', '127.0.0.1:30301', '127.0.0.1:30302'], 'files': files}).encode()
        c, g, seeds, written = observer.parse_invitation(raw)
        obs_net = Path(work, 'observer', 'network'); obs_net.mkdir(parents=True)
        for name in observer.FILES:
            (obs_net / name).write_text(written[name])
        args = observer.exec_args(seeds, pocold=dn.BIN)
        observer.check_args(args)
        args = [a.replace(str(observer.NETWORK_DIR), str(obs_net)).replace(str(observer.STATE / 'data'), os.path.join(work, 'observer', 'data'))
                .replace(observer.RPC, f'127.0.0.1:{OBS_RPC}') for a in args]
        ok('the observer command line has no miner and loopback RPC, and the invitation names the devnet', c == chain and g == genesis
           and '--miner' not in ' '.join(args) and args[args.index('--rpc') + 1] == f'127.0.0.1:{OBS_RPC}', ' '.join(args[1:]))
        obs = spawn('observer', args + ['--log'], work)
        head = wait(lambda: (lambda s, h: s['head']['height'] >= h - 1 and s)(rpc(OBS_RPC, 'pocol_status'), int(rpc(8540, 'eth_blockNumber'), 16)),
                    180, 'the observer follows the devnet')
        ok('the observer syncs from the seeds and mines nothing', head['stats']['mined'] == 0 and not head.get('miner'),
           json.dumps({'height': head['head']['height'], 'stats': head['stats'], 'miner': head.get('miner')}))
        # ---- the gateway: the transaction index, the holders index and the miner map ----
        Path(work, 'network.json').write_text(json.dumps({'chainId': chain, 'genesisHash': genesis, 'specVersion': 2, 'v3': True}))
        database = os.path.join(work, 'geoip-test.bin')
        pocol_geoip.build(REWARDS / 'testdata' / 'geoip-sample.csv', TEST_RELEASE, database)
        # A throwaway credential for this run only (the gateway's geo-reporters and the reporter's geo-reporter).
        salt, key = secrets.token_hex(32), secrets.token_hex(32)
        credentials = Path(work, 'credentials'); credentials.mkdir(mode=0o700)
        (credentials / 'geo-reporters').write_text(json.dumps({'salt': salt, 'reporters': {'founder-1': key}}))
        Path(work, 'reporter-credential.json').write_text(json.dumps({'reporter': 'founder-1', 'key': key, 'salt': salt}))
        for f in (credentials / 'geo-reporters', Path(work, 'reporter-credential.json')):
            f.chmod(0o600)
        env = {**os.environ, 'POCOL_TX_DB': os.path.join(work, 'transactions.sqlite3'), 'POCOL_HOLDERS_DB': os.path.join(work, 'holders.sqlite3'),
               'POCOL_GEOIP_DB': database, 'CREDENTIALS_DIRECTORY': str(credentials)}
        gw_args = [sys.executable, '-B', '-c', GATEWAY_LAUNCH, str(REWARDS), str(genesis_wei), str(REWARDS / 'gateway.py'),
                   '--port', str(GW), '--rpc-port', str(OBS_RPC), '--v3-network', os.path.join(work, 'network.json'),
                   '--site-origin', 'http://127.0.0.1:8661']
        gw = spawn('gateway', gw_args, work, env)
        wait(lambda: get('/health')[0] == 200, 30, 'the gateway')
        # ---- transactions of every kind, sent to node a ----
        addr = lambda label: subprocess.run([SIGN, '--chain-id', str(chain), '--account', label, '--address'], capture_output=True, text=True, check=True).stdout.strip().lower()
        e, f = addr('e'), addr('f')
        def send(label, nonce, to=None, value=0, data=None, gas=21000, port=8540):
            cmd = [SIGN, '--chain-id', str(chain), '--account', label, '--nonce', str(nonce), '--value', str(value), '--gas', str(gas)]
            if to: cmd += ['--to', to]
            if data: cmd += ['--data', data]
            raw_tx = subprocess.run(cmd, capture_output=True, text=True, check=True).stdout.strip()
            return rpc(port, 'eth_sendRawTransaction', [raw_tx])
        sent = [send('e', i, f, 10 ** 15 * (i + 1)) for i in range(5)]
        sent.append(send('f', 0, '0x0000000000000000000000000000000000c0c001', 0, '0xdeadbeef', 100000))   # a contract call (reverts)
        sent.append(send('g', 0, None, 0, '0x600160005260206000f3', 200000))                              # a contract creation
        receipts = wait(lambda: all(rpc(8540, 'eth_getTransactionReceipt', [h]) for h in sent) and [rpc(8540, 'eth_getTransactionReceipt', [h]) for h in sent], 120, 'the transactions are included')
        target = max(int(r['blockNumber'], 16) for r in receipts)
        wait(lambda: (lambda st, v: st == 200 and v['data']['index']['height'] >= target)(*get('/api/transactions.php')), 90, 'the index passes the last transaction')
        rows, first = indexed()
        expected, _ = canonical(OBS_RPC)
        expected = [r for r in expected if r[0] <= first['snapshot']['height']]
        ok('the index lists every canonical transaction newest first (block, position)', rows == expected and len(rows) >= 7, f'{len(rows)} rows, snapshot {first["snapshot"]["height"]}')
        kinds = {r['hash']: r['kind'] for r in get('/api/transactions.php?size=100')[1]['data']['rows']}
        ok('kinds: transfers, a contract call and a contract creation', [kinds.get(h) for h in sent] == ['Transfer'] * 5 + ['Contract call', 'Contract creation'], [kinds.get(h) for h in sent])
        status, value = get(f'/api/transactions.php?q={e[:12]}&size=100')
        ok('search by a sender prefix finds its five transfers', status == 200 and sorted(r['hash'] for r in value['data']['rows']) == sorted(sent[:5]), value['data']['total'] if status == 200 else value)
        status, value = get(f'/api/transactions.php?q={sent[5]}')
        ok('search by a full transaction hash finds it', status == 200 and [r['hash'] for r in value['data']['rows']] == [sent[5]])
        values = [int(r['value'], 16) for r in get('/api/transactions.php?order=value_desc&size=100')[1]['data']['rows'] if r['value']]
        ok('value order: highest first', values == sorted(values, reverse=True) and values[0] == 5 * 10 ** 15, values[:6])
        # A pinned snapshot: the block of the last transaction once it is 6 blocks deep (a devnet reorganizes its tip
        # often; a snapshot at the tip itself may rightly answer 409), with the rows the canonical chain has up to it.
        wait(lambda: int(rpc(OBS_RPC, 'eth_blockNumber'), 16) >= target + 6, 120, 'six blocks over the last transaction')
        snap = {'height': target, 'hash': rpc(OBS_RPC, 'eth_getBlockByNumber', [hex(target), False])['hash'].lower()}
        pinned_rows = [r for r in canonical(OBS_RPC)[0] if r[0] <= target]
        more = [send('e', 5 + i, f, 1) for i in range(2)]
        wait(lambda: all(rpc(8540, 'eth_getTransactionReceipt', [h]) for h in more), 120, 'two more transactions')
        wait(lambda: get('/api/transactions.php')[1]['data']['total'] >= len(pinned_rows) + 2, 60, 'the index takes them')
        status, value = get(f'/api/transactions.php?size=100&height={snap["height"]}&hash={snap["hash"]}')
        newest = get('/api/transactions.php')[1]['data']['rows']
        ok('a page pinned to an earlier block keeps that block\'s list; the newest page has the two new transactions first',
           status == 200 and [r['hash'] for r in value['data']['rows']] == [r[2] for r in pinned_rows]
           and sorted(r['hash'] for r in newest[:2]) == sorted(more), f'{status} reorgs {rpc(OBS_RPC, "pocol_status")["stats"]["reorgs"]}')
        # ---- the holders index: ready, its first snapshots against the node ----
        ready = wait(lambda: (lambda st, v: st == 200 and v['data']['index']['state'] == 'ready' and v['data'])(*get('/api/holders.php')), 180, 'the holders index is ready')
        ok('/api/holders.php answers pocol-holders-v1 for the devnet with the index "ready"', ready['schema'] == 'pocol-holders-v1'
           and ready['chainId'] == chain and ready['genesisHash'] == genesis, json.dumps(ready['index']))
        # The devnet's --accounts (e, f, g) are funded at genesis and can be discovered only once they transact (on
        # Mainnet the founders are the only genesis accounts, found through the registry): a snapshot below the first
        # transactions rightly publishes their balances as unlocatedWei with complete false.
        checked(OBS_RPC, genesis_wei, miners, 120, 'holders: ranks, Total = available + bonded, every available balance = eth_getBalance at the snapshot, '
                'sum of Total = currentSupplyWei, the founders\' bonds on their IDs, the supply identity, complete, statistics of the same snapshot',
                min_height=target)
        # ---- a gateway restart resumes the same files ----
        stop(gw)
        gw = spawn('gateway', gw_args, work, env)
        wait(lambda: get('/health')[0] == 200, 30, 'the gateway again')
        wait(lambda: get('/api/transactions.php')[1]['data']['index']['state'] == 'ready', 60, 'the index after the restart')
        rows2, _ = indexed()
        expected2, _ = canonical(OBS_RPC)
        expected2 = [r for r in expected2 if r[0] <= get('/api/transactions.php')[1]['data']['snapshot']['height']]
        ok('after a gateway restart the index resumes from its file: the same rows, no duplicate', rows2[:len(expected2)] == expected2 and len(set(r[2] for r in rows2)) == len(rows2), f'{len(rows2)} rows')
        # ---- fresh accounts: g funds x and 24 others; then x pays y ----
        x, y = addr('holder-x'), addr('holder-y')
        others = [addr(f'holder-{i:02d}') for i in range(24)]
        funding = [send('g', 1, x, 2 * 10 ** 18)] + [send('g', 2 + i, a, 10 ** 15 * (i + 1)) for i, a in enumerate(others)]
        funded = wait(lambda: all(rpc(8540, 'eth_getTransactionReceipt', [h]) for h in funding) and [rpc(8540, 'eth_getTransactionReceipt', [h]) for h in funding], 180, 'the funding transfers')
        funded_at = max(int(r['blockNumber'], 16) for r in funded)
        before = checked(OBS_RPC, genesis_wei, miners, 180, 'holders after the funding: the fresh accounts are listed with one transaction each and their exact balances',
                         min_height=funded_at, expect={x: (1, 2 * 10 ** 18), **{a: (1, 10 ** 15 * (i + 1)) for i, a in enumerate(others)}})
        ok('holders: y does not exist yet', y not in {r['address'] for r in before['rows']})
        pay = send('holder-x', 0, y, 5 * 10 ** 17)
        paid = wait(lambda: rpc(8540, 'eth_getTransactionReceipt', [pay]), 120, 'x pays y')
        cost = int(paid['gasUsed'], 16) * int(paid['effectiveGasPrice'], 16)
        after = checked(OBS_RPC, genesis_wei, miners, 180, 'holders at a later snapshot: x (2 transactions, 2 PCOL - 0.5 PCOL - its fee) and y (1 transaction, 0.5 PCOL)',
                        min_height=int(paid['blockNumber'], 16), expect={x: (2, 2 * 10 ** 18 - 5 * 10 ** 17 - cost), y: (1, 5 * 10 ** 17)})
        reward_rows = [r for r in after['rows'] if r['type'] == 'reward']
        ok('holders: mining rewards since the index start are exact (each founder reward address\'s rewards = its balance gain on the node since the block before the start) and non-zero',
           max(before['rewardsChecked'], after['rewardsChecked']) >= 1 and any(int(r['rewardsWei'] or 0) > 0 for r in reward_rows),
           f'rewards {after["stats"].get("rewards")}; checked {before["rewardsChecked"]} and {after["rewardsChecked"]} addresses; '
           + ', '.join(f'{r["address"][:10]} {r["rewardsWei"]}' for r in reward_rows))
        # ---- a chunk stored through a top-level ChunkFactory call (PoCol Mainnet 2026-11-26: a site publication's two
        # chunks were existing accounts no receipt or log names, published as accountsMissing 2) ----
        chunk_data = b'pocol explorer e2e chunk ' + secrets.token_bytes(16)
        store_input = ('0x' + pocol_holders.CHUNK_STORE + (32).to_bytes(32, 'big').hex() + len(chunk_data).to_bytes(32, 'big').hex()
                       + (chunk_data + b'\0' * (-len(chunk_data) % 32)).hex())
        chunk_tx = send('holder-x', 1, pocol_holders.CHUNK_FACTORY, 0, store_input, 300000)
        stored = wait(lambda: rpc(8540, 'eth_getTransactionReceipt', [chunk_tx]), 120, 'the ChunkFactory store call')
        chunk = pocol_holders.chunk_address(chunk_data)
        code = '0x00' + chunk_data.hex()
        # The observer gets node a's blocks by gossip, and a devnet often reorganizes its tip: wait for the chunk there.
        wait(lambda: rpc(OBS_RPC, 'eth_getCode', [chunk, 'latest']).lower() == code, 120, 'the observer has the chunk')
        ok('the store call created its chunk inside the call: the node has 0x00 || data at the index\'s CREATE2 address, and no receipt field or log names it',
           stored['status'] == '0x1' and stored.get('contractAddress') is None and stored['logs'] == [],
           json.dumps({'status': stored['status'], 'chunk': chunk}))
        def counted():
            st, value = get('/api/holders-stats.php')
            if st != 200:
                return None
            snap = value['data']['snapshot']
            # The snapshot's own block must be canonical on the observer and hold the chunk (a receipt height from
            # node a can be stale after a tip reorganization), and only a snapshot whose pocol_getCaps answer was
            # for its own block compares the count (else null).
            if rpc(OBS_RPC, 'eth_getBlockByNumber', [hex(snap['height']), False])['hash'].lower() != snap['hash'].lower():
                return None
            if rpc(OBS_RPC, 'eth_getCode', [chunk, hex(snap['height'])]).lower() != code:
                return None
            v = value['data']['verification']
            return v['accountsOnChain'] is not None and v
        v = wait(counted, 120, 'a snapshot whose own state holds the chunk, with the account count compared')
        ok('holders: the chunk is a discovered account, so the found accounts equal pocol_getCaps\' count and the verification is complete',
           v['complete'] is True and v['accountsMissing'] == 0 and v['unlocatedWei'] == '0', json.dumps(v))
        # Paging: two pages of 25, the second pinned to the first page's snapshot while newer snapshots are published.
        # The pinned snapshot is one 6 blocks deep that the chain kept (a devnet often reorganizes its tip, and a page
        # pinned to a lost tip block is rightly 409); it is still kept by the index (the newest 3 and every one
        # younger than 180 s).
        def deep_snapshot():
            value = audit(OBS_RPC, genesis_wei, miners, min_height=int(paid['blockNumber'], 16))
            wait(lambda: int(rpc(OBS_RPC, 'eth_blockNumber'), 16) >= value['h'] + 6, 60, 'six blocks over the snapshot')
            kept = rpc(OBS_RPC, 'eth_getBlockByNumber', [hex(value['h']), False])['hash'].lower() == value['snapshot']['hash']
            return kept and not value['problems'] and value
        after = wait(deep_snapshot, 180, 'a holders snapshot six blocks deep')
        total = after['first']['total']
        pin = {'height': after['h'], 'hash': after['snapshot']['hash']}
        status, page2 = get(f'/api/holders.php?page=2&size=25&height={pin["height"]}&hash={pin["hash"]}')
        status_asc, asc = get(f'/api/holders.php?page=1&size=25&order=balance_asc&height={pin["height"]}&hash={pin["hash"]}')
        ok('a holders page pinned to an earlier snapshot keeps it while newer ones are published: page 2 is ranks 26..50 of it, and the smallest-first page is the same ranks reversed',
           total > 25 and status == 200 and page2['data']['snapshot']['hash'] == pin['hash'] and page2['data']['rows'] == after['rows'][25:50]
           and status_asc == 200 and asc['data']['rows'] == after['rows'][::-1][:25], f'{total} holders, {status} {status_asc}')
        status, miners_page = get(f'/api/holders.php?type=miner&height={pin["height"]}&hash={pin["hash"]}')
        status_q, found = get(f'/api/holders.php?q={y[:10]}&height={pin["height"]}&hash={pin["hash"]}')
        ok('holders: the miner filter lists the three founders\' IDs (global ranks kept) and an address-prefix search finds y',
           status == 200 and sorted(r['address'] for r in miners_page['data']['rows']) == sorted(m['id'] for m in miners)
           and all(r == next(a for a in after['rows'] if a['address'] == r['address']) for r in miners_page['data']['rows'])
           and status_q == 200 and y in [r['address'] for r in found['data']['rows']], f'{status} {status_q}')
        status, bad = get('/api/holders.php?size=7')
        status2, bad2 = get('/api/holders.php?page=1&page=2')
        ok('holders: a size outside 25/50/100 and a repeated key are 400', status == 400 and status2 == 400, f'{status} {bad} {status2} {bad2}')
        # ---- an operator node report on node a: pocol_geo_reporter's node read and report, sent from FR ----
        reporter.RPC_PORT = 8540  # the reporter reads its own node on 127.0.0.1:8545; node a's RPC is 8540 here
        config = Path(work, 'reporter-config.json')
        config.write_text(json.dumps({'chainId': chain, 'genesisHash': genesis, 'explorerUrl': 'https://explorer.invalid/api/geo-report.php',
                                      'database': database}))
        printed = []
        dry = reporter.run(str(config), str(Path(work, 'reporter-credential.json')), dry_run=True, out=printed.append)
        a_id = next(m['id'] for m in miners if m['label'] == 'a')
        dry_report = json.loads(printed[0]) if printed else {}
        ok('the reporter\'s dry run on node a names node a\'s miner (its registry record ID), only keyed ids, no IP',
           dry == 0 and dry_report.get('miner') == a_id and '127.0.0.1' not in ''.join(printed) and dry_report.get('peers') == []
           and dry_report.get('unlocated', 0) >= 1, ' | '.join(printed)[:300])
        status_a, params_a = reporter.read_node()
        db = pocol_geoip.GeoDatabase(database)
        try:
            body, signature, report = reporter.build_report(status_a, params_a, reporter.load_config(str(config)),
                                                            reporter.load_credential(str(Path(work, 'reporter-credential.json'))), db, time.time())
        finally:
            db.close()
        sent_from = {'X-PoCol-Reporter': 'founder-1', 'X-PoCol-Signature': signature, 'X-Forwarded-For': REPORT_SOURCE}
        forged = {**sent_from, 'X-PoCol-Signature': hmac.new(b'\1' * 32, body, hashlib.sha256).hexdigest()}
        answers = {'bad signature': post_report(body, forged), 'with Origin': post_report(body, {**sent_from, 'Origin': 'http://127.0.0.1:8661'}),
                   'accepted': post_report(body, sent_from), 'replayed': post_report(body, sent_from)}
        ok('/api/geo-report.php: a bad signature is 401, an Origin header 400, the signed report 204, the same report again 409',
           answers == {'bad signature': 401, 'with Origin': 400, 'accepted': 204, 'replayed': 409}, json.dumps(answers))
        report_head = json.loads(body)['head']['height']

        def located():
            # One answer and the observer's chain up to its window's last block, read again when the block at the
            # answer's height is no longer canonical (a tip reorganization between the two reads). "Connected" is
            # measured against the index tip, which can trail node a's head under load: the answer is read again
            # until the index tip is within pocol_geoip.CONNECTED_BLOCKS of the report's head (the report is sent
            # once here, while an operator's reporter sends a new one every interval).
            with urllib.request.urlopen(f'http://127.0.0.1:{GW}/api/miner-geography.php', timeout=15) as r:
                raw = r.read().decode()
            value = json.loads(raw)['data']
            chain_blocks = chain_to(OBS_RPC, value['blocksWindow']['to'])
            return (value['miners']['located'] == 1 and value['height'] == value['blocksWindow']['to']
                    and abs(report_head - value['height']) <= pocol_geoip.CONNECTED_BLOCKS
                    and chain_blocks[value['height']]['hash'].lower() == value['blockHash'] and (value, raw, chain_blocks))
        geo, raw_map, blocks = wait(located, 60, 'the miner map places node a\'s miner, its index tip within 6 blocks of the report\'s head')
        window = blocks[max(1, geo['blocksWindow']['from']):]
        a_blocks = sum(1 for b in window if b['miner'].lower() == a_id)
        fr = geo['countries'][0] if geo['countries'] else {}
        ok('/api/miner-geography.php: 3 registered, node a\'s miner located in FR from its report\'s source address, 2 Unknown, connected, its blocks of the window',
           geo['schema'] == 'pocol-miners-geography-v2' and geo['chainId'] == chain and geo['genesisHash'] == genesis
           and geo['miners']['registered'] == 3 and geo['miners']['located'] == 1 and geo['miners']['unknown'] == 2
           and geo['miners']['connected'] == 1 and fr.get('code') == REPORT_COUNTRY and fr['miners']['located'] == 1
           and fr['miners']['active'] == 1 and fr['blocks'] == a_blocks and geo['blocksWindow']['blocks'] == len(window)
           and geo['unknown'] == {'miners': 2, 'blocks': len(window) - a_blocks}
           and int(fr['blocksSharePercentE10']) == a_blocks * 10 ** 12 // len(window), json.dumps(geo)[:400])
        ok('the miner map names its sources (the observer, founder-1 fresh) and the test database; its node counts include the reporter\'s own node in FR',
           geo['database'] == {'provider': 'DB-IP Lite', 'release': TEST_RELEASE, 'license': 'CC BY 4.0'}
           and {(s['kind'], s['name'], s['fresh']) for s in geo['sources']} == {('observer', 'explorer', True), ('operator', 'founder-1', True)}
           and fr.get('nodes') == 1 and geo['nodes']['located'] == 1 and geo['nodes']['observed'] >= 2, json.dumps({'sources': geo['sources'], 'nodes': geo['nodes']}))
        source_id = pocol_geoip.keyed_id(bytes.fromhex(salt), REPORT_SOURCE)
        leaks = [v for v in [REPORT_SOURCE, '127.0.0.1', source_id, key, salt] + [m['id'][2:] for m in miners] + [m['reward'][2:] for m in miners]
                 + [p['id'] for p in report['peers']] if v in raw_map]
        ok('the miner map answer carries no IP, keyed id, miner address or secret', leaks == [], leaks)
        # ---- a reorganization of the observer's chain ----
        # a and b keep mining together; c mines alone, and the observer follows only c. c's branch (with a transaction
        # of its own) loses: c rejoins a and b without mining and adopts their heavier chain; so does the observer.
        dn.stop(os.path.join(work, 'dn')); stop(obs)
        time.sleep(1)
        def node(i, peers, miner=True, listen=True):
            l = 'abc'[i]
            cmd = [dn.BIN, 'run', '--network', net, '--data', os.path.join(work, 'dn', 'data', l), '--name', l, '--rpc', f'127.0.0.1:{8540 + i}', '--log', '--no-discovery']
            cmd += (['--listen', f'127.0.0.1:{30300 + i}'] if listen else []) + sum([['--peer', f'127.0.0.1:{30300 + j}'] for j in peers], []) + (['--miner', l] if miner else [])
            return spawn('node-' + l, cmd, work)
        pa, pb, pc = node(0, [1]), node(1, [0]), node(2, [])
        obs_args = [a for a in args]
        seed_free = [x for k, x in enumerate(obs_args) if x != '--seed' and (k == 0 or obs_args[k - 1] != '--seed')]
        obs = spawn('observer', seed_free + ['--no-discovery', '--peer', '127.0.0.1:30302', '--log'], work)
        wait(lambda: rpc(8542, 'pocol_status')['peers'] and rpc(8540, 'pocol_status')['peers'] and rpc(OBS_RPC, 'pocol_status')['peers'], 120, 'the partition')
        lone = send('e', 7, f, 777, port=8542)
        wait(lambda: rpc(8542, 'eth_getTransactionReceipt', [lone]), 120, 'the isolated branch includes its transaction')
        lone_at = int(wait(lambda: rpc(OBS_RPC, 'eth_getTransactionReceipt', [lone]), 120, 'the observer has it')['blockNumber'], 16)
        wait(lambda: any(r['hash'] == lone for r in get('/api/transactions.php')[1]['data']['rows']), 60, 'the index lists the isolated branch transaction')
        lone_snapshot = get('/api/transactions.php')[1]['data']['snapshot']
        def lone_holders():
            status, value = get('/api/holders.php')
            return status == 200 and value['data']['snapshot']['height'] >= lone_at and value['data']['snapshot']
        lone_holder_snapshot = wait(lone_holders, 120, 'a holders snapshot on the isolated branch')
        ab = send('f', 1, e, 888)  # nonce 1 of f on a and b's branch
        wait(lambda: rpc(8540, 'eth_getTransactionReceipt', [ab]), 120, 'a and b include theirs')
        c_work = int(rpc(8542, 'pocol_status')['head']['totalWork'], 16) if isinstance(rpc(8542, 'pocol_status')['head']['totalWork'], str) else rpc(8542, 'pocol_status')['head']['totalWork']
        def heavier():
            w = rpc(8540, 'pocol_status')['head']['totalWork']
            return (int(w, 16) if isinstance(w, str) else w) > c_work
        stop(pc)
        wait(heavier, 300, 'a and b heavier than c')
        reorgs_before = rpc(OBS_RPC, 'pocol_status')['stats']['reorgs']
        pc = node(2, [0, 1], miner=False)
        def adopted():
            ah = rpc(8540, 'pocol_status')['head']
            oh = rpc(OBS_RPC, 'pocol_status')['head']
            return oh['height'] >= ah['height'] - 1 and rpc(OBS_RPC, 'eth_getBlockByNumber', [hex(oh['height'] - 1), False])['hash'] == rpc(8540, 'eth_getBlockByNumber', [hex(oh['height'] - 1), False])['hash']
        wait(adopted, 300, 'the observer adopts a and b\'s chain')
        reorgs_after = rpc(OBS_RPC, 'pocol_status')['stats']['reorgs']
        ok('the observer reorganized onto a and b\'s chain', reorgs_after > reorgs_before, f'reorgs {reorgs_before} -> {reorgs_after}')
        def settled():
            # The index equals the canonical chain up to its snapshot, and the snapshot is past the blocks that hold
            # a and b's transaction and, when the new chain included it again, the isolated one.
            rows3, data = indexed()
            exp, _ = canonical(OBS_RPC)
            exp = [r for r in exp if r[0] <= data['snapshot']['height']]
            receipts = [rpc(OBS_RPC, 'eth_getTransactionReceipt', [h]) for h in (ab, lone)]
            need = max(int(r['blockNumber'], 16) for r in receipts if r) if any(receipts) else 0
            return rows3 == exp and data['snapshot']['height'] >= need and (rows3, data, receipts[1])
        rows3, data, lone_receipt = wait(settled, 120, 'the index equals the new canonical chain')
        hashes = {r[2] for r in rows3}
        row_of = {r['hash']: r for r in get('/api/transactions.php?size=100')[1]['data']['rows']}
        ok('after the reorganization the index equals the canonical chain: a and b\'s transaction is listed; the isolated one only where the new chain included it',
           ab in hashes and ((lone in hashes) == bool(lone_receipt))
           and (not lone_receipt or row_of[lone]['blockHash'] == lone_receipt['blockHash'].lower()),
           f'lone included again: {bool(lone_receipt)}; {len(rows3)} rows')
        stale = lone_snapshot
        status, value = get(f'/api/transactions.php?height={stale["height"]}&hash={stale["hash"]}')
        ok('a page pinned to the abandoned branch is 409 with reset', (status, value) == (409, {'error': 'The chain changed. Refreshing transaction history.', 'reset': True}), (status, value))
        # The holders index converges to the healed chain: a snapshot past a and b's transaction, checked whole
        # against the observer's state at its height (e and f at their new balances), and the page pinned to the
        # isolated branch's snapshot is 409 reset.
        ab_at = int(rpc(OBS_RPC, 'eth_getTransactionReceipt', [ab])['blockNumber'], 16)
        # e sent 5 + 2 transfers and received a and b's; f received those 7, sent the contract call and a and b's
        # transfer; the isolated transfer (e to f) counts for both where the healed chain included it again by h.
        def healed_counts(blocks):
            again = any(lone in b['transactions'] for b in blocks)
            return {e: (len(sent[:5]) + len(more) + 1 + again, None), f: (len(sent[:5]) + len(more) + 2 + again, None)}
        healed = checked(OBS_RPC, genesis_wei, miners, 240, 'holders converge to the healed chain: every balance, the supply identity and complete at a snapshot past a and b\'s transaction',
                         min_height=ab_at, expect=healed_counts)
        status, value = get(f'/api/holders.php?page=1&size=25&height={lone_holder_snapshot["height"]}&hash={lone_holder_snapshot["hash"]}')
        ok('a holders page pinned to the isolated branch\'s snapshot is 409 reset', (status, value) == (409, HOLDERS_RESET),
           f'{(status, value)}; the healed snapshot {healed["h"]} {healed["snapshot"]["hash"][:12]}')
        for p in (pa, pb, pc):
            stop(p)
    finally:
        for p in procs:
            try:
                stop(p)
            except Exception:  # noqa: BLE001
                pass
        dn.stop(os.path.join(work, 'dn'))
    failed = [r for r in results if not r['pass']]
    summary = {'result': 'FAIL' if failed else 'PASS', 'checks': len(results), 'failed': len(failed), 'work': work}
    json.dump({**summary, 'results': results}, open(os.path.join(work, 'e2e_explorer_devnet.json'), 'w'), indent=1)
    print(json.dumps(summary), flush=True)
    return 1 if failed else 0


if __name__ == '__main__':
    try:
        code = main()
    except Exception as error:  # noqa: BLE001
        ok('the explorer devnet e2e completed', False, repr(error))
        code = 1
    sys.exit(code)
