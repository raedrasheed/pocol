#!/usr/bin/env python3
"""The explorer gateway host on a local v3 devnet: the observer node (install_observer_node.py's command line, no
miner key), the explorer gateway on it with the transaction index (POCOL_TX_DB), transactions of every kind, the
Transactions page newest first against the canonical chain, search, a pinned snapshot, a gateway restart, and a
reorganization of the observer's chain (a partitioned devnet whose isolated branch loses).

    POCOLD=/path/to/pocold python3 deploy/explorer/e2e_explorer_devnet.py [WORK]

Uses devnet.py's fixed ports (30300-30302, 8540-8542) plus 30309/8549 (observer) and 8662 (gateway); local only,
throwaway devnet accounts (pocol-sign --account, never a funded key). Prints PASS/FAIL lines and a JSON summary;
exit status 1 on any failure."""
import contextlib
import io
import json
import os
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(ROOT / 'prototypes' / 'pocold' / 'scripts'))
sys.path.insert(0, str(HERE))
import devnet as dn  # noqa: E402
import install_observer_node as observer  # noqa: E402

BIN = Path(dn.BIN).parent
SIGN = os.environ.get('POCOL_SIGN', str(BIN / 'pocol-sign'))
OBS_RPC, OBS_P2P, GW = 8549, 30309, 8662
results, procs = [], []


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
    5): a busy answer (429, or 503 while the budget is spent) is retried after a second, up to 20 times."""
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


def main():
    work = sys.argv[1] if len(sys.argv) > 1 else tempfile.mkdtemp(prefix='pocol-explorer-e2e-')
    os.makedirs(work, exist_ok=True)
    print('work', work, flush=True)
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
        # ---- the gateway with the transaction index ----
        Path(work, 'network.json').write_text(json.dumps({'chainId': chain, 'genesisHash': genesis, 'specVersion': 2, 'v3': True}))
        env = {**os.environ, 'POCOL_TX_DB': os.path.join(work, 'transactions.sqlite3')}
        gw_args = [sys.executable, '-B', str(ROOT / 'deploy/rewards/gateway.py'), '--port', str(GW), '--rpc-port', str(OBS_RPC),
                   '--v3-network', os.path.join(work, 'network.json'), '--site-origin', 'http://127.0.0.1:8661']
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
        # ---- a gateway restart resumes the same file ----
        stop(gw)
        gw = spawn('gateway', gw_args, work, env)
        wait(lambda: get('/health')[0] == 200, 30, 'the gateway again')
        wait(lambda: get('/api/transactions.php')[1]['data']['index']['state'] == 'ready', 60, 'the index after the restart')
        rows2, _ = indexed()
        expected2, _ = canonical(OBS_RPC)
        expected2 = [r for r in expected2 if r[0] <= get('/api/transactions.php')[1]['data']['snapshot']['height']]
        ok('after a gateway restart the index resumes from its file: the same rows, no duplicate', rows2[:len(expected2)] == expected2 and len(set(r[2] for r in rows2)) == len(rows2), f'{len(rows2)} rows')
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
        wait(lambda: rpc(OBS_RPC, 'eth_getTransactionReceipt', [lone]), 120, 'the observer has it')
        wait(lambda: any(r['hash'] == lone for r in get('/api/transactions.php')[1]['data']['rows']), 60, 'the index lists the isolated branch transaction')
        lone_snapshot = get('/api/transactions.php')[1]['data']['snapshot']
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
