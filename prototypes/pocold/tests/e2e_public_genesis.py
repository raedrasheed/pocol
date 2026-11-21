#!/usr/bin/env python3
"""The public network's genesis end to end, with real processes (final design §6 tests 1 and 5, OWNER-DECISIONS-2026-11-20
D6 E): deploy/mainnet/v3-profile.json filled the way the genesis ceremony fills it, KEEPING its chain id 2656089850, in a
temporary directory only, with throwaway keys made here (never a launch genesis; nothing here is committed or published).

Checks, each a bounded wait that names what it waited for:
  * init-testnet admits it in scope public-network as the network profile on a network node, not specification
    compliant; its summary names environment mainnet and scope public-network, and approval and readiness are false for
    exactly the two reasons an unpinned network has (network-not-release-pinned, launch-not-approved);
  * build-info (pocold and pocol) reports the build alone: no-network, no identity fields;
  * the launcher's invitation description agrees with the booted nodes on the approval reference, the technical
    readiness, the name and the environment, adds admission-not-booted and reports no scope;
  * three founder nodes (`pocol start`, the release launcher) peer over the host's non-loopback address when it has one
    (the seeds and listen addresses of the invitation; never a public server), and every node reports exactly the
    init-testnet summary's approval and readiness fields, the invitation's consensus identity and this build's digest;
    the fourth founder stays offline;
  * the third founder joins late and syncs to the same canonical chain;
  * a transfer signed with a founder's reward key is included and its receipt is identical on every node;
  * a newcomer with only that transfer registers with a 0 PCOL bond (earn-in: probation), starts a node, mines on
    probation and its bond grows from its own rewards; a founder's reward account grows from block rewards; every node
    agrees on these balances at one block;
  * a founder node restarts and replays to the same chain;
  * convergence: one canonical hash, stateRoot and receiptsRoot at a confirmed height on every node.
The readiness after the release pin (approved and ready, PoCol Mainnet) needs a build that pins the genesis: it is
proved in tests/fn4_profiles.rs (`a_booted_mainnet_genesis_pinned_with_its_approval_...`, a test-only pin of a
throwaway mainnet-chain genesis); no flag or variable of these binaries can pin one, so it is not repeated here.

This is a single-host test: it is not a multi-host soak, an Internet ingress test or a device run. POCOLD_BIN_DIR selects
the binaries (default prototypes/pocold/target/release); POCOL_E2E_HOST overrides the address the nodes listen on.
Exit status 0 only if every check passes; the result is printed as one JSON line.
"""
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[3]
BIN = Path(os.environ.get('POCOLD_BIN_DIR', str(ROOT / 'prototypes/pocold/target/release')))
EXE = '.exe' if os.name == 'nt' else ''
PROFILE = ROOT / 'deploy/mainnet/v3-profile.json'
MAINNET_CHAIN_ID = 2656089850
PCOL = 10**18
# The fields every network producer reports (pocold::readiness::annotate).
FIELDS = ('publicLaunchApproved', 'launchApprovalReference', 'publicLaunchReady', 'launchBlockers', 'technicalReadiness',
          'networkName', 'networkEnvironment', 'networkScope')
UNPINNED = ['network-not-release-pinned', 'launch-not-approved']
# keccak256("register(address)")[:4] (tests/e2e_independent_keys.py); the registry system program.
REGISTER = '0x4420e486'
REGISTRY = '0x0000000000000000000000000000000000c0c001'
CONFIRMATIONS = 3
# A throwaway calibration for this test genesis: a block is found in a few hundred hashes, as the test flag's synthetic
# value (testnet::TestPlaceholders). It is not a measurement; the ceremony measures the founders' hash rate (R25d).
TEST_HASHES_PER_SECOND = 16


def run(name, *args, timeout=120):
    p = subprocess.run([str(BIN / (name + EXE)), *map(str, args)], capture_output=True, text=True, encoding='utf-8', timeout=timeout)
    if p.returncode:
        raise AssertionError(f'{name} {args[:2]} failed: {p.stderr[-2000:]}')
    return p.stdout


def free_port(host):
    with socket.socket() as s:
        s.bind((host, 0))
        return s.getsockname()[1]


def host_address():
    """The host's own non-loopback IPv4 address (the route a UDP socket would take; no packet is sent), or loopback."""
    if os.environ.get('POCOL_E2E_HOST'):
        return os.environ['POCOL_E2E_HOST']
    try:
        with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as s:
            s.connect(('192.0.2.1', 9))
            address = s.getsockname()[0]
        with socket.socket() as s:
            s.bind((address, 0))
        return address
    except OSError:
        return '127.0.0.1'


def rpc(port, method, params=None):
    for _ in range(20):
        req = urllib.request.Request(f'http://127.0.0.1:{port}', data=json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': method, 'params': params or []}).encode(),
                                     headers={'Content-Type': 'application/json'})
        with urllib.request.urlopen(req, timeout=5) as r:
            value = json.load(r)
        error = value.get('error')
        if error and error.get('code') == -32021:  # load shedding: ask again after retryAfterMs
            time.sleep(min(2.0, (error.get('data') or {}).get('retryAfterMs', 200) / 1000))
            continue
        if error:
            raise RuntimeError(error)
        return value['result']
    raise RuntimeError(f'{method}: node busy')


def ids(v):
    return [b['id'] for b in v['launchBlockers']]


def main():
    host = host_address()
    checks = []

    def ok(what):
        checks.append(what)
        print('PASS ' + what, flush=True)

    work = Path(tempfile.mkdtemp(prefix='pocol-e2e-public-'))
    os.chmod(work, 0o700)
    processes, logs = {}, []
    try:
        # Throwaway keys: four founders (a node identity each) with their own reward keys, and one newcomer.
        homes = {f'f{i}': work / f'f{i}' for i in range(1, 5)}
        homes['n1'] = work / 'n1'
        address = {label: json.loads(run('pocol', 'identity', '--home', h))['address'] for label, h in homes.items()}
        keys = work / 'reward-keys'
        keys.mkdir(mode=0o700)
        reward = {f'f{i}': run('pocol-key', 'generate', '--out', keys / f'r{i}.pocol-key').strip() for i in range(1, 5)}
        profile = json.loads(PROFILE.read_text(encoding='utf-8'))
        assert profile['chainId'] == MAINNET_CHAIN_ID, profile['chainId']
        profile['genesisTime'] = int(time.time()) + 10
        profile['hashRate']['hashesPerSecond'] = TEST_HASHES_PER_SECOND
        for i, f in enumerate(profile['founders'], 1):
            f['id'], f['reward'] = address[f'f{i}'], reward[f'f{i}']
        assert 'PLACEHOLDER' not in json.dumps(profile), 'every ceremony input is filled'
        config = work / 'v3-profile.json'
        config.write_text(json.dumps(profile), encoding='utf-8')
        summary = json.loads(run('pocold', 'init-testnet', '--config', config, '--out', work / 'net'))
        assert (summary['chainId'], summary['scope'], summary['profileKind'], summary['nodeKind']) == (MAINNET_CHAIN_ID, 'public-network', 'network', 'node'), summary
        assert summary['specificationCompliant'] is False, summary
        assert (summary['networkEnvironment'], summary['networkScope'], summary.get('networkName')) == ('mainnet', 'public-network', None), summary
        assert (summary['publicLaunchApproved'], summary['publicLaunchReady'], ids(summary)) == (False, False, UNPINNED), summary
        pending = [b['id'] for b in summary['technicalReadiness']['pending']]
        assert summary['technicalReadiness']['complete'] is False and 'admission-scope' in pending, pending
        assert [p for p in pending if p.startswith('admission-waiver-')] == ['admission-waiver-R13', 'admission-waiver-R21', 'admission-waiver-R26'], pending
        info = json.loads((work / 'net/devnet.json').read_text(encoding='utf-8'))
        assert info['genesisHash'] == summary['genesisHash'] and info['admission']['scope'] == 'public-network', info['admission']
        ok('init-testnet admits the filled mainnet profile on chain 2656089850 in scope public-network (network profile, network node)')

        build = {}
        for name in ('pocold', 'pocol'):
            build[name] = b = json.loads(run(name, 'build-info'))
            assert b['publicLaunchApproved'] is False and 'no-network' in ids(b), b
            assert all(b.get(k) is None for k in ('launchApprovalReference', 'networkName', 'networkEnvironment', 'networkScope')), b
        assert build['pocold']['compatibilitySha256'] == build['pocol']['compatibilitySha256']
        ok('build-info reports the build alone: no-network, no identity fields')

        p2p = {label: free_port(host) for label in ('f1', 'f2', 'f3', 'n1')}
        api = {label: free_port('127.0.0.1') for label in p2p}
        seeds = []
        for label in ('f1', 'f2', 'f3'):
            seeds += ['--seed', f'{host}:{p2p[label]}']
        invitation = work / 'network.pocol.json'
        described = json.loads(run('pocol', 'network', '--from', work / 'net', *seeds, '--out', invitation))
        inspected = json.loads(run('pocol', 'inspect', '--network', invitation))
        for d in (described, inspected):
            assert ids(d) == UNPINNED + ['admission-not-booted'] and d['networkScope'] is None and d['publicLaunchApproved'] is False, d
            assert (d['chainId'], d['genesisHash']) == (MAINNET_CHAIN_ID, summary['genesisHash']), d
        ok('the launcher describes the invitation: not approved, admission-not-booted, no scope')

        def start(label):
            log = (work / f'{label}.log').open('ab')
            logs.append(log)
            processes[label] = subprocess.Popen([str(BIN / ('pocol' + EXE)), 'start', '--home', str(homes[label]), '--network', str(invitation),
                                                 '--listen', f'{host}:{p2p[label]}', '--rpc', f'127.0.0.1:{api[label]}'], stdout=log, stderr=subprocess.STDOUT)

        def stop(label):
            p = processes.pop(label)
            p.terminate()
            try:
                p.wait(timeout=30)
            except subprocess.TimeoutExpired:
                p.kill()
                p.wait(timeout=10)

        def wait(predicate, what, timeout):
            t = time.monotonic()
            while time.monotonic() - t < timeout:
                for label, p in processes.items():
                    if p.poll() is not None:
                        raise AssertionError(f'{label} exited while waiting for {what}')
                try:
                    if predicate():
                        print(f'WAIT {what}: met after {time.monotonic() - t:.1f}s (bound {timeout}s)', flush=True)
                        return
                except (OSError, ValueError, KeyError, TypeError, RuntimeError):
                    pass
                time.sleep(1)
            raise AssertionError(f'timeout ({timeout}s) waiting for {what}')

        def status(label):
            return rpc(api[label], 'pocol_status')

        def height(label):
            return status(label)['head']['height']

        def canonical(label, n):
            b = rpc(api[label], 'eth_getBlockByNumber', [hex(n), False])
            return b and (b['hash'], b['stateRoot'], b['receiptsRoot'])

        def balance(label, who, n):
            return int(rpc(api[label], 'eth_getBalance', [who, hex(n)]), 16)

        def send(key, to, value, data=None, gas=21000):
            nonce = int(rpc(api['f1'], 'eth_getTransactionCount', [address_of_key[key], 'latest']), 16)
            command = ['--chain-id', MAINNET_CHAIN_ID, '--key-file', key_file[key], '--to', to, '--value', value, '--nonce', nonce, '--gas', gas]
            if data:
                command += ['--data', data]
            return rpc(api['f1'], 'eth_sendRawTransaction', [run('pocol-sign', *command).strip()])

        def confirmed(tx, nodes):
            seen = {}

            def same():
                receipts = [rpc(api[n], 'eth_getTransactionReceipt', [tx]) for n in nodes]
                if not all(receipts) or len({(r['blockHash'], r['status']) for r in receipts}) != 1:
                    return False
                h = int(receipts[0]['blockNumber'], 16)
                seen['receipt'] = receipts[0]
                return all(height(n) >= h + CONFIRMATIONS and canonical(n, h)[0] == receipts[0]['blockHash'] for n in nodes)
            wait(same, f'receipt of {tx[:12]} on {",".join(nodes)}', 300)
            return seen['receipt']

        key_file = {'r1': keys / 'r1.pocol-key', 'n1': homes['n1'] / 'identity.pocol-key'}
        address_of_key = {'r1': reward['f1'], 'n1': address['n1']}

        start('f1')
        start('f2')
        wait(lambda: all(height(n) >= 3 and status(n)['compatiblePeers'] >= 1 for n in ('f1', 'f2')), 'f1 and f2 mine and peer', 240)
        for n in ('f1', 'f2'):
            s = status(n)
            for k in FIELDS:
                assert s.get(k) == summary.get(k), (n, k, s.get(k), summary.get(k))
            assert (s['chainId'], s['genesisHash'], s['admission']['scope']) == (MAINNET_CHAIN_ID, summary['genesisHash'], 'public-network'), n
            assert s['consensusIdentity'] == described['consensusIdentity'] and s['compatibilitySha256'] == build['pocold']['compatibilitySha256'], n
            assert s['peers'] and all(p.rsplit(':', 1)[0] == host for p in s['peers']), (n, s['peers'])
            for k in ('launchApprovalReference', 'technicalReadiness', 'networkName', 'networkEnvironment'):
                assert described.get(k) == s.get(k), (n, k)
        ok(f'two founder nodes peer over {host} and report exactly the init-testnet summary\'s approval and readiness fields')

        wait(lambda: min(height(n) for n in ('f1', 'f2')) >= 12, 'height 12 before the late join', 300)
        start('f3')
        target = min(height(n) for n in ('f1', 'f2'))
        wait(lambda: height('f3') >= target and canonical('f3', target) == canonical('f1', target) == canonical('f2', target), f'f3 syncs to height {target}', 300)
        ok(f'the third founder joins late and syncs to the same chain (height {target})')

        live = ('f1', 'f2', 'f3')
        gift = send('r1', address['n1'], PCOL)
        r = confirmed(gift, live)
        assert r['status'] == '0x1', r
        h = int(r['blockNumber'], 16)
        assert len({balance(n, address['n1'], h) for n in live}) == 1 and balance('f1', address['n1'], h) == PCOL
        ok('a transfer signed with a founder reward key is included with one receipt on every node')

        registration = send('n1', REGISTRY, 0, REGISTER + address['n1'][2:].rjust(64, '0'), 1_000_000)
        r = confirmed(registration, live)
        assert r['status'] == '0x1', r

        def record(n):
            return [m for m in rpc(api[n], 'pocol_getMiners') if m['id'].lower() == address['n1'].lower()]
        assert all(len(record(n)) == 1 and int(record(n)[0]['bond'], 16) == 0 and record(n)[0]['status'] == 1 for n in live), [record(n) for n in live]
        ok('the newcomer registers with a 0 PCOL bond as the first transaction it sends (earn-in probation)')

        start('n1')
        wait(lambda: status('n1')['stats']['mined'] > 0, 'the newcomer mines a block on probation', 600)
        wait(lambda: all(int(record(n)[0]['bond'], 16) > 0 for n in live), 'the newcomer\'s bond grows from its rewards', 600)
        allocation = 999_980 * PCOL
        h = min(height(n) for n in live + ('n1',)) - CONFIRMATIONS
        founder2 = {balance(n, reward['f2'], h) for n in live + ('n1',)}
        assert len(founder2) == 1 and founder2.pop() > allocation, 'founder 2\'s reward account grows from block rewards, equal on every node'
        ok('block rewards: a founder reward account grows and the newcomer\'s bond accrues, equal on every node')

        stop('f2')
        start('f2')
        h = min(height(n) for n in ('f1', 'f3')) - CONFIRMATIONS
        wait(lambda: canonical('f2', h) == canonical('f1', h), f'f2 restarts and replays to height {h}', 240)
        ok('a founder node restarts and replays to the same chain')

        nodes = live + ('n1',)
        wait(lambda: min(height(n) for n in nodes) >= h + 2 * CONFIRMATIONS, 'every node advances after the restart', 300)
        h = min(height(n) for n in nodes) - CONFIRMATIONS
        roots = {canonical(n, h) for n in nodes}
        assert len(roots) == 1, roots
        final = {n: status(n) for n in nodes}
        for n, s in final.items():
            assert (s['publicLaunchApproved'], s['publicLaunchReady'], ids(s), s['networkEnvironment'], s['networkScope']) == (False, False, UNPINNED, 'mainnet', 'public-network'), n
        ok(f'four nodes agree on hash, stateRoot and receiptsRoot at height {h}; every node still reports the unpinned network\'s two blockers')
        print(json.dumps({'result': 'PASS', 'scope': 'single-host public-network genesis e2e (chain 2656089850, throwaway keys, temp dir)',
                          'checks': checks, 'chainId': MAINNET_CHAIN_ID, 'genesisHash': summary['genesisHash'],
                          'consensusIdentity': described['consensusIdentity'], 'compatibilitySha256': build['pocold']['compatibilitySha256'],
                          'host': host, 'nonLoopbackPeering': not host.startswith('127.'), 'commonHeight': h, 'commonHash': roots.pop()[0],
                          'transactions': {'transfer': gift, 'registration': registration},
                          'multiHostProven': False, 'internetIngressProven': False, 'releasePinProven': 'tests/fn4_profiles.rs (test-only pin)'}))
    except BaseException:
        for log in logs:
            log.flush()
        for path in sorted(work.glob('*.log')):
            print(f'--- {path.name}\n' + path.read_text(encoding='utf-8', errors='replace')[-3000:], file=sys.stderr)
        raise
    finally:
        for label in list(processes):
            try:
                stop(label)
            except Exception:
                pass
        for log in logs:
            log.close()
        shutil.rmtree(work, ignore_errors=True)


if __name__ == '__main__':
    main()
