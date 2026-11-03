#!/usr/bin/env python3
"""Cutover rehearsal for the earn-in network (EARN-IN-SPEC.md section 7, decisions N1-N4, A2).

Two modes, both on copies; neither touches a live service, a key or the original data:

--local   Builds an old-style continuous network (100 PCOL bonds, the current programs) from three
          `pocol start` miners, adds the kinds of state Mainnet holds (a fourth miner registered
          with a full bond, a transfer, a contract with storage), records the expected balances at a
          height S through RPC, and stops every node. It then copies one home and runs
          `pocold cutover` on the copy twice (the genesis bytes must match), checks the new
          allocation against the RPC record independently of the cutover report, starts the three
          miners on the new genesis, and checks that it mines, keeps the contract, refunds the old
          bond, holds 20 PCOL miner bonds, and lets the migrated fourth account register again with
          a 0 PCOL bond. About 10 minutes.

--copy    The operator rehearsal on a COPY of a real server's network bundle and data directory:
          runs the cutover twice at the given height with the server's own NODE and LOCAL flags,
          checks determinism and the report's totals, and prints a summary. It starts nothing.

    python3 deploy/mainnet/rehearse_cutover.py --local --work /tmp/pocol-rehearsal
    python3 deploy/mainnet/rehearse_cutover.py --copy --network COPY/net --data COPY/data \\
        --height S --config new-network.json --work /tmp/pocol-rehearsal -- --state-store \\
        --checkpoint-every 128 --delta 1 --phi 1 --t-exec 1 --delta-rel 8 --gamma 5
"""
import argparse, filecmp, http.client, json, os, shutil, subprocess, sys, time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'standalone'))
from test_launcher import BIN, EXE, call, identity, port, rpc  # noqa: E402

PCOL = 10**18
OLD_BOND, NEW_BOND = 100 * PCOL, 20 * PCOL
# A contract whose constructor stores 42 in slot 0 and whose runtime code is one STOP byte.
CONTRACT_INIT = '0x602a6000556001601160003960016000f300'

def cutover(network, data, height, config, out, node_args):
    p = subprocess.run([str(BIN / ('pocold' + EXE)), 'cutover', '--network', str(network), '--data', str(data), '--height', str(height),
                        '--config', str(config), '--out', str(out), *node_args], capture_output=True, text=True, timeout=1800)
    if p.returncode: raise AssertionError(f'cutover failed: {p.stderr.strip()}')
    report = out / 'cutover-report.json'
    # A pocold without the cutover command (the deployed one, for example) prints its usage and exits 0.
    if not report.is_file(): raise AssertionError(f'{BIN / ("pocold" + EXE)} exited 0 without writing {report}: it is a build '
                                                  'without the cutover command; set POCOLD_BIN_DIR to the merged release')
    return json.loads(report.read_text(encoding='utf-8'))

def same_genesis(a, b):
    for name in ('genesis.spec.hex', 'alloc.json', 'cutover-report.json'):
        if not filecmp.cmp(a / name, b / name, shallow=False): raise AssertionError(f'{name} differs between two runs: not deterministic')

def check_report(report):
    total = int(report['allocTotal']) + int(report['minerBondsDeducted'])
    assert int(report['totalSupply']) == total, report
    assert int(report['allocTotal']) == int(report['copiedBalances']) + int(report['bondsCreditedTotal']) - int(report['minerBondsDeducted']), report
    assert report['newGenesis']['genesisHash'].startswith('0x') and len(report['newGenesis']['genesisHash']) == 66, report

def summary(report):
    return {'old': report['old'], 'new': report['newGenesis'], 'accountsCopied': report['accountsCopied'],
            'copiedBalances': report['copiedBalances'], 'bondsCreditedTotal': report['bondsCreditedTotal'], 'bondsCredited': len(report['bondsCredited']),
            'minerBondsDeducted': report['minerBondsDeducted'], 'potsDroppedTotal': report['potsDroppedTotal'], 'potsDropped': len(report['potsDropped']),
            'excluded': report['excluded'], 'totalSupply': report['totalSupply']}

def copy_mode(a):
    work = Path(a.work); work.mkdir(parents=True)
    r1 = cutover(a.network, a.data, a.height, a.config, work / 'run1', a.node_args)
    r2 = cutover(a.network, a.data, a.height, a.config, work / 'run2', a.node_args)
    same_genesis(work / 'run1', work / 'run2'); check_report(r1)
    print(json.dumps({'result': 'PASS', 'mode': 'copy', 'deterministic': True, **summary(r1)}, indent=1))

class Nodes:
    def __init__(self, root): self.root = root; self.procs = []; self.logs = []
    def start(self, home, inv, p2p, api, name):
        log = open(self.root / f'{name}.log', 'w', encoding='utf-8'); self.logs.append(log)
        self.procs.append(subprocess.Popen([str(BIN / ('pocol' + EXE)), 'start', '--home', str(home), '--network', str(inv),
                                            '--listen', f'127.0.0.1:{p2p}', '--rpc', f'127.0.0.1:{api}'], stdout=log, stderr=log))
    def stop(self):
        for p in self.procs:
            if p.poll() is None: p.terminate()
        for p in self.procs:
            try: p.wait(timeout=20)
            except subprocess.TimeoutExpired: p.kill(); p.wait()
        for log in self.logs: log.close()
        self.procs, self.logs = [], []

def wait(predicate, seconds, what):
    end = time.time() + seconds
    while time.time() < end:
        try:
            value = predicate()
            if value: return value
        except (OSError, RuntimeError, http.client.HTTPException, KeyError, ValueError): pass
        time.sleep(1)
    raise AssertionError(f'timed out: {what}')

def send(api, home, chain_id, to, value=0, data=None, gas=21000):
    sender = identity(home)
    nonce = int(rpc(api, 'eth_getTransactionCount', [sender, 'pending']), 16)
    args = [str(BIN / ('pocol-sign' + EXE)), '--key-file', str(home / 'identity.pocol-key'), '--chain-id', str(chain_id),
            '--nonce', str(nonce), '--value', str(value), '--gas', str(gas)]
    if to: args += ['--to', to]
    if data: args += ['--data', data]
    raw = subprocess.run(args, capture_output=True, text=True, check=True).stdout.strip()
    tx = rpc(api, 'eth_sendRawTransaction', [raw])
    return wait(lambda: (lambda r: r if r and r.get('blockNumber') else None)(rpc(api, 'eth_getTransactionReceipt', [tx])), 300, f'receipt {tx}')

def head(api): return int(rpc(api, 'eth_blockNumber'), 16)

def local_mode(a):
    root = Path(a.work); root.mkdir(parents=True)
    homes = [root / f'old{i}' for i in range(4)]; ids = [identity(h) for h in homes]
    alice, bob = root / 'alice', root / 'bob'; ida, idb = identity(alice), identity(bob)
    old_chain, new_chain = 777950, 777951
    cfg = {'continuous': True, 'chainId': old_chain, 'genesisTime': int(time.time()) + 5, 'registrationPerBlock': 1,
           'miners': [{'id': i, 'reward': i} for i in ids[:3]],
           'accounts': [{'address': i, 'wei': str(10**24)} for i in ids] + [{'address': ida, 'wei': str(500 * PCOL)}]}
    (root / 'old.json').write_text(json.dumps(cfg)); call('pocold', 'init-testnet', '--config', root / 'old.json', '--out', root / 'old-genesis')
    p2p = [port() for _ in range(3)]; api = [port() for _ in range(3)]
    args = ['network', '--from', root / 'old-genesis', '--out', root / 'old.pocol.json']
    for p in p2p: args += ['--seed', f'127.0.0.1:{p}']
    call('pocol', *args)
    nodes = Nodes(root)
    try:
        for i in range(3): nodes.start(homes[i], root / 'old.pocol.json', p2p[i], api[i], f'old-node{i}')
        wait(lambda: head(api[0]) >= 3, 240, 'old network mining')
        # State Mainnet holds: a full-bond registration (PAY-02 needs a prior transaction), a transfer, a contract.
        send(api[0], homes[3], old_chain, ids[3], 1)
        reg = send(api[0], homes[3], old_chain, '0x' + '00' * 17 + 'c0c001', OLD_BOND, '0x4420e486' + '00' * 12 + ids[3][2:].lower(), 1_000_000)
        assert reg['status'] == '0x1', reg
        assert send(api[0], alice, old_chain, idb, 7 * PCOL)['status'] == '0x1'
        made = send(api[0], alice, old_chain, None, 0, CONTRACT_INIT, 200_000); contract = made['contractAddress']
        assert made['status'] == '0x1' and contract, made
        wait(lambda: head(api[0]) >= int(made['blockNumber'], 16) + 3, 120, 'blocks after the last transaction')
        s = head(api[0]) - 1; tag = hex(s)
        accounts = ids + [ida, idb, contract]
        expect = {x.lower(): {'balance': int(rpc(api[0], 'eth_getBalance', [x, tag]), 16), 'nonce': int(rpc(api[0], 'eth_getTransactionCount', [x, tag]), 16)} for x in accounts}
        held = {m['reward'].lower(): int(m['bond'], 16) for m in rpc(api[0], 'pocol_getMiners') if m['bondState'] == 1 and not m['tomb']}
        assert set(held) == {i.lower() for i in ids} and all(v == OLD_BOND for v in held.values()), held
        old_hash = rpc(api[0], 'eth_getBlockByNumber', [tag, False])['hash']
    finally:
        nodes.stop()
    # The rehearsal runs on a copy; the stopped original stays as it was.
    copy = root / 'copy'; shutil.copytree(homes[0], copy, ignore=shutil.ignore_patterns('identity.pocol-key'))
    new_cfg = {'continuous': True, 'earnin': True, 'chainId': new_chain, 'genesisTime': int(time.time()) + 60, 'registrationPerBlock': 1,
               'miners': [{'id': i, 'reward': i} for i in ids[:3]], 'accounts': []}
    (root / 'new.json').write_text(json.dumps(new_cfg))
    flags = ['--state-store']
    r1 = cutover(copy / 'network', copy / 'state', s, root / 'new.json', root / 'new-genesis', flags)
    r2 = cutover(copy / 'network', copy / 'state', s, root / 'new.json', root / 'new-genesis-2', flags)
    same_genesis(root / 'new-genesis', root / 'new-genesis-2'); check_report(r1)
    assert r1['old']['height'] == s and r1['old']['blockHash'] == old_hash and r1['old']['chainId'] == old_chain, r1['old']
    # Independent check of the allocation against the old chain's RPC answers at S (C2-C4).
    alloc = {k.lower(): v for k, v in json.loads((root / 'new-genesis/alloc.json').read_text()).items()}
    def bal(x): return int(alloc[x]['balance'])
    for x, e in expect.items():
        want = e['balance'] + held.get(x, 0) - (NEW_BOND if x in {i.lower() for i in ids[:3]} else 0)
        assert x in alloc, f'{x} missing from the new allocation'
        assert bal(x) == want, (x, bal(x), want)
        assert int(alloc[x].get('nonce', 0)) == max(e['nonce'], 1 if x in {i.lower() for i in ids[:3]} else 0), (x, alloc[x])
    assert alloc[contract.lower()].get('code') and alloc[contract.lower()].get('storage'), alloc[contract.lower()]
    # Boot the new network from the cutover genesis with the same three miner keys.
    p2p = [port() for _ in range(3)]; api = [port() for _ in range(3)]
    args = ['network', '--from', root / 'new-genesis', '--out', root / 'new.pocol.json']
    for p in p2p: args += ['--seed', f'127.0.0.1:{p}']
    call('pocol', *args)
    fresh = [root / f'new{i}' for i in range(4)]
    for old, new in zip(homes, fresh):
        new.mkdir(mode=0o700); os.chmod(new, 0o700)
        for name in ('identity.pocol-key', 'identity.address'):
            if (old / name).exists(): shutil.copy2(old / name, new / name)
        assert identity(new) == identity(old)
    try:
        for i in range(3): nodes.start(fresh[i], root / 'new.pocol.json', p2p[i], api[i], f'new-node{i}')
        wait(lambda: head(api[0]) >= 5, 600, 'new network mining')
        assert int(rpc(api[0], 'eth_chainId'), 16) == new_chain
        assert rpc(api[0], 'eth_getStorageAt', [contract, '0x0', 'latest']) in ('0x' + '0' * 62 + '2a', '0x2a'), 'contract storage kept'
        assert rpc(api[0], 'eth_getCode', [contract, 'latest']) == '0x00', 'contract code kept'
        assert int(rpc(api[0], 'eth_getBalance', [idb, 'latest']), 16) == expect[idb.lower()]['balance'] == 7 * PCOL
        assert int(rpc(api[0], 'eth_getBalance', [ids[3], 'latest']), 16) == expect[ids[3].lower()]['balance'] + OLD_BOND, 'old bond refunded'
        miners = rpc(api[0], 'pocol_getMiners')
        assert sorted(m['id'].lower() for m in miners) == sorted(i.lower() for i in ids[:3]) and all(int(m['bond'], 16) == NEW_BOND and m['bondState'] == 1 for m in miners), miners
        # The migrated fourth account registers again on the new network with a 0 PCOL bond.
        reg = send(api[0], fresh[3], new_chain, '0x' + '00' * 17 + 'c0c001', 0, '0x4420e486' + '00' * 12 + ids[3][2:].lower(), 400_000)
        assert reg['status'] == '0x1', reg
        mine = [m for m in rpc(api[0], 'pocol_getMiners') if m['id'].lower() == ids[3].lower()]
        assert mine and mine[0]['bondState'] == 4 and int(mine[0]['bond'], 16) == 0, mine
        height = head(api[0])
    finally:
        nodes.stop()
    print(json.dumps({'result': 'PASS', 'mode': 'local', 'deterministic': True, 'S': s, 'newHeight': height,
                      'checkedAccounts': len(expect), **summary(r1)}, indent=1))

def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    mode = p.add_mutually_exclusive_group(required=True)
    mode.add_argument('--local', action='store_true'); mode.add_argument('--copy', action='store_true')
    p.add_argument('--work', required=True, help='a new directory for the rehearsal output')
    p.add_argument('--network', type=Path); p.add_argument('--data', type=Path); p.add_argument('--height', type=int); p.add_argument('--config', type=Path)
    p.add_argument('node_args', nargs='*', help='after --: the NODE and LOCAL flags the server runs pocold with')
    a = p.parse_args(argv)
    if Path(a.work).exists(): p.error('--work must be a new directory')
    if a.copy:
        if not (a.network and a.data and a.height is not None and a.config): p.error('--copy needs --network, --data, --height and --config')
        copy_mode(a)
    else:
        local_mode(a)
    return 0

if __name__ == '__main__':
    sys.exit(main())
