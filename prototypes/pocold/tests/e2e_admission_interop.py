#!/usr/bin/env python3
"""The move of the mainnet chain to scope public-network changes no consensus validity rule and no node compatibility
(OWNER-DECISIONS-2026-11-20 D6 E), shown with real processes: a node of the old admission and a node of the new one on
the same genesis agree on one chain.

The old build is the last commit before the change (8b465b9 by default in CI), which boots chain 2656089850 as a Simnet
profile on a Simnet node in scope experimental-devnet; this tree boots it as the network profile on a network node in
scope public-network. Both build the same genesis bytes from the same filled deploy/mainnet/v3-profile.json (chain id
kept, throwaway keys made here, a temp dir only: never a launch genesis). Checks, each a bounded wait:
  * both builders write the same genesis hash; each admits it in its own scope;
  * both builds report the same consensus rules version, and the two nodes send the same consensus identity (the
    identity commits to chain, genesis, rules, flags and fork schedule, never to the admission) and peer;
  * each node mines blocks and imports the other's: each validates the blocks of the other admission;
  * a transfer is included with one receipt on both nodes;
  * one canonical hash, stateRoot and receiptsRoot at a confirmed height on both nodes.
POCOL_OLD_BIN_DIR names the old build's binaries (required); POCOLD_BIN_DIR this tree's (default
prototypes/pocold/target/release); POCOL_E2E_HOST the listen address (default: the host's non-loopback address).
"""
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

sys.path.insert(0, str(Path(__file__).resolve().parent))
import e2e_public_genesis as pg  # noqa: E402

NEW = pg.BIN
OLD = Path(os.environ['POCOL_OLD_BIN_DIR']) if os.environ.get('POCOL_OLD_BIN_DIR') else None


def call(bin_dir, name, *args):
    p = subprocess.run([str(bin_dir / (name + pg.EXE)), *map(str, args)], capture_output=True, text=True, encoding='utf-8', timeout=120)
    if p.returncode:
        raise AssertionError(f'{bin_dir.name}/{name} {args[:1]} failed: {p.stderr[-2000:]}')
    return p.stdout


def main():
    if OLD is None:
        raise SystemExit('POCOL_OLD_BIN_DIR must name the old admission build (for example a release build of 8b465b9)')
    host = pg.host_address()
    work = Path(tempfile.mkdtemp(prefix='pocol-e2e-interop-'))
    os.chmod(work, 0o700)
    processes, logs, checks = {}, [], []

    def ok(what):
        checks.append(what)
        print('PASS ' + what, flush=True)
    try:
        builds = {'old': json.loads(call(OLD, 'pocol', 'build-info')), 'new': json.loads(call(NEW, 'pocol', 'build-info'))}
        for k in ('consensusRulesVersion', 'protocolVersion', 'protocolVersionMin', 'maxForkVersion', 'consensusPinned', 'features'):
            assert builds['old'].get(k) == builds['new'].get(k), (k, builds['old'].get(k), builds['new'].get(k))
        assert builds['new']['consensusPinned'] is True and builds['old']['consensusPinned'] is True
        homes = {'old': work / 'old', 'new': work / 'new'}
        founder = {k: json.loads(call(NEW, 'pocol', 'identity', '--home', h))['address'] for k, h in homes.items()}
        keys = work / 'keys'
        keys.mkdir(mode=0o700)
        others = [json.loads(call(NEW, 'pocol', 'identity', '--home', work / f'offline{i}'))['address'] for i in (3, 4)]
        reward = [call(NEW, 'pocol-key', 'generate', '--out', keys / f'r{i}.pocol-key').strip() for i in range(1, 5)]
        profile = json.loads(pg.PROFILE.read_text(encoding='utf-8'))
        profile['genesisTime'] = int(time.time()) + 15
        profile['hashRate']['hashesPerSecond'] = pg.TEST_HASHES_PER_SECOND
        for f, i, r in zip(profile['founders'], [founder['old'], founder['new']] + others, reward):
            f['id'], f['reward'] = i, r
        assert profile['chainId'] == pg.MAINNET_CHAIN_ID and 'PLACEHOLDER' not in json.dumps(profile)
        (work / 'v3-profile.json').write_text(json.dumps(profile), encoding='utf-8')
        summary = {}
        for k, b in (('old', OLD), ('new', NEW)):
            summary[k] = json.loads(call(b, 'pocold', 'init-testnet', '--config', work / 'v3-profile.json', '--out', work / f'net-{k}'))
        assert summary['old']['genesisHash'] == summary['new']['genesisHash'], (summary['old']['genesisHash'], summary['new']['genesisHash'])
        assert (summary['old']['scope'], summary['new']['scope']) == ('experimental-devnet', 'public-network'), (summary['old']['scope'], summary['new']['scope'])
        for name in ('genesis.spec.hex', 'alloc.json'):
            assert (work / f'net-old/{name}').read_bytes() == (work / f'net-new/{name}').read_bytes(), name
        ok(f'both builders write genesis {summary["new"]["genesisHash"][:18]} on chain {pg.MAINNET_CHAIN_ID}; old scope experimental-devnet, new scope public-network')

        p2p = {k: pg.free_port(host) for k in homes}
        api = {k: pg.free_port('127.0.0.1') for k in homes}
        invitation = {}
        for k, b in (('old', OLD), ('new', NEW)):
            invitation[k] = work / f'network-{k}.pocol.json'
            call(b, 'pocol', 'network', '--from', work / f'net-{k}', '--seed', f'{host}:{p2p["old"]}', '--seed', f'{host}:{p2p["new"]}', '--out', invitation[k])
        for k, b in (('old', OLD), ('new', NEW)):
            log = (work / f'{k}.log').open('ab')
            logs.append(log)
            processes[k] = subprocess.Popen([str(b / ('pocol' + pg.EXE)), 'start', '--home', str(homes[k]), '--network', str(invitation[k]),
                                             '--listen', f'{host}:{p2p[k]}', '--rpc', f'127.0.0.1:{api[k]}'], stdout=log, stderr=subprocess.STDOUT)

        def wait(predicate, what, timeout):
            t = time.monotonic()
            while time.monotonic() - t < timeout:
                for k, p in processes.items():
                    if p.poll() is not None:
                        raise AssertionError(f'{k} exited while waiting for {what}')
                try:
                    if predicate():
                        print(f'WAIT {what}: met after {time.monotonic() - t:.1f}s (bound {timeout}s)', flush=True)
                        return
                except (OSError, ValueError, KeyError, TypeError, RuntimeError):
                    pass
                time.sleep(1)
            raise AssertionError(f'timeout ({timeout}s) waiting for {what}')
        status = lambda k: pg.rpc(api[k], 'pocol_status')
        height = lambda k: status(k)['head']['height']

        def canonical(k, n):
            b = pg.rpc(api[k], 'eth_getBlockByNumber', [hex(n), False])
            return b and (b['hash'], b['stateRoot'], b['receiptsRoot'])
        wait(lambda: all(status(k)['compatiblePeers'] >= 1 for k in homes), 'the old and the new node peer', 240)
        s = {k: status(k) for k in homes}
        assert s['old']['consensusIdentity'] == s['new']['consensusIdentity'], 'one consensus identity across the two admissions'
        assert (s['old']['admission']['scope'], s['new']['admission']['scope']) == ('experimental-devnet', 'public-network')
        assert s['new']['publicLaunchApproved'] is False and s['old']['publicLaunchReady'] is False
        ok('the two nodes send one consensus identity and peer, each booted in its own admission')

        wait(lambda: all(status(k)['stats']['mined'] >= 3 and status(k)['stats']['imported'] >= 3 for k in homes), 'each node mines blocks and imports the other\'s', 600)
        nonce = int(pg.rpc(api['new'], 'eth_getTransactionCount', [reward[0], 'latest']), 16)
        raw = call(NEW, 'pocol-sign', '--chain-id', pg.MAINNET_CHAIN_ID, '--key-file', keys / 'r1.pocol-key', '--to', reward[1], '--value', pg.PCOL,
                   '--nonce', nonce, '--gas', 21000).strip()
        tx = pg.rpc(api['old'], 'eth_sendRawTransaction', [raw])
        seen = {}

        def included():
            receipts = [pg.rpc(api[k], 'eth_getTransactionReceipt', [tx]) for k in homes]
            if not all(receipts) or len({(r['blockHash'], r['status']) for r in receipts}) != 1:
                return False
            n = int(receipts[0]['blockNumber'], 16)
            seen['receipt'] = receipts[0]
            return all(height(k) >= n + pg.CONFIRMATIONS and canonical(k, n)[0] == receipts[0]['blockHash'] for k in homes)
        wait(included, 'the transfer\'s receipt on both nodes', 300)
        assert seen['receipt']['status'] == '0x1', seen
        ok('a transfer sent to the old node is included with one receipt on both nodes')

        h = min(height(k) for k in homes) - pg.CONFIRMATIONS
        roots = {canonical(k, h) for k in homes}
        assert len(roots) == 1, roots
        mined = {k: status(k)['stats']['mined'] for k in homes}
        ok(f'both nodes agree on hash, stateRoot and receiptsRoot at height {h} (mined: old {mined["old"]}, new {mined["new"]})')
        print(json.dumps({'result': 'PASS', 'scope': 'single-host interop of the old and the new admission on chain 2656089850 (throwaway keys, temp dir)',
                          'checks': checks, 'genesisHash': summary['new']['genesisHash'], 'consensusIdentity': s['new']['consensusIdentity'],
                          'oldCompatibilitySha256': builds['old']['compatibilitySha256'], 'newCompatibilitySha256': builds['new']['compatibilitySha256'],
                          'commonHeight': h, 'commonHash': roots.pop()[0], 'mined': mined, 'host': host}))
    except BaseException:
        for log in logs:
            log.flush()
        for path in sorted(work.glob('*.log')):
            print(f'--- {path.name}\n' + path.read_text(encoding='utf-8', errors='replace')[-3000:], file=sys.stderr)
        raise
    finally:
        for p in processes.values():
            p.terminate()
        for p in processes.values():
            try:
                p.wait(timeout=30)
            except subprocess.TimeoutExpired:
                p.kill()
                p.wait(timeout=10)
        for log in logs:
            log.close()
        shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    main()
