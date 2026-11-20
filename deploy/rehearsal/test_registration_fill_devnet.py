#!/usr/bin/env python3
"""The registration fill harness on a real local devnet (opt-in: POCOL_REHEARSAL_E2E=1; a few minutes).

The devnet is built with the repository's own tooling (`pocol identity`, `pocol-key generate`, `pocold init-testnet`,
`pocol network`, `pocol start`, as deploy/standalone's tests build theirs): one founder node, the rehearsal profile's CP
with a small set (M_max 8) and short clocks (Theta_inact 3, E_win 8, W_act 4, U 12, C_react 12, which ParamGate R8
admits), and an operator account that funds the newcomers. The harness then runs as the rehearsal would run it:
- `fill` registers to the full set (bonded, idle and one mining ACCRUING identity, whose own `pocol start` node mines),
  never more than R_max (4) of its registrations in a block;
- `m3` takes the capacity path: two registerReplacing in one block on distinct victims, register alone reverting with
  Capacity, removeInactive reverting below a full set, the free seat taken again, and a victim registering again after U;
- `stop-miners` and `delete-keys` leave no key file, and no key ever appears in the harness's output or files.
Binaries: POCOLD_BIN_DIR (default prototypes/pocold/target/release), built with `cargo build --release --bins`.
"""
import json
import os
import socket
import subprocess
import sys
import tempfile
import time
import unittest
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
BIN = Path(os.environ.get('POCOLD_BIN_DIR', ROOT / 'prototypes/pocold/target/release'))
EXE = '.exe' if os.name == 'nt' else ''
HARNESS = HERE / 'registration_fill.py'
PROFILE = json.loads((ROOT / 'deploy/mainnet/v3-rehearsal-profile.json').read_text(encoding='utf-8'))
ENABLED = os.environ.get('POCOL_REHEARSAL_E2E') == '1'


def port():
    with socket.socket() as s:
        s.bind(('127.0.0.1', 0))
        return s.getsockname()[1]


def tool(name, *args):
    p = subprocess.run([str(BIN / (name + EXE)), *map(str, args)], capture_output=True, text=True, timeout=120)
    if p.returncode:
        raise AssertionError(f'{name}: {p.stderr}')
    return p.stdout


def rpc(url, method, params=()):
    body = json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': method, 'params': list(params)}).encode()
    with urllib.request.urlopen(urllib.request.Request(url, body, {'Content-Type': 'application/json'}), timeout=10) as r:
        v = json.load(r)
    if 'error' in v:
        raise AssertionError(v['error'])
    return v['result']


@unittest.skipUnless(ENABLED, 'opt-in: POCOL_REHEARSAL_E2E=1 (a real devnet, a few minutes)')
@unittest.skipUnless(os.name != 'nt', 'Linux/macOS: the harness keys need POSIX modes')
class Devnet(unittest.TestCase):
    def setUp(self):
        for name in ('pocol', 'pocold', 'pocol-key', 'pocol-sign'):
            self.assertTrue((BIN / (name + EXE)).is_file(), f'{BIN / name} (build with cargo build --release --bins)')
        self.tmp = tempfile.TemporaryDirectory(prefix='pocol-fill-')
        self.addCleanup(self.tmp.cleanup)
        self.root = Path(self.tmp.name)
        self.procs, self.logs = [], []
        self.addCleanup(self.stop)

    def stop(self):
        for p in self.procs:
            if p.poll() is None:
                p.terminate()
        for p in self.procs:
            try:
                p.wait(timeout=20)
            except subprocess.TimeoutExpired:
                p.kill()
        for log in self.logs:
            log.close()

    def harness(self, *args, expect=0):
        p = subprocess.run([sys.executable, '-B', str(HARNESS), *map(str, args)], capture_output=True, text=True, timeout=1500)
        self.outputs.append(p.stdout + p.stderr)
        self.assertEqual(p.returncode, expect, p.stdout[-4000:] + p.stderr[-4000:])
        return p.stdout

    def test_fill_to_a_full_set_and_the_m3_capacity_path(self):
        self.outputs = []
        home = self.root / 'founder'
        founder = json.loads(tool('pocol', 'identity', '--home', home))['address']
        op_key = self.root / 'operator.pocol-key'
        operator = tool('pocol-key', 'generate', '--out', op_key).strip()
        cp = dict(PROFILE['cp'])
        cp.update({'M_max': 8, 'Theta_inact': 3, 'E_win': 8, 'W_act': 4, 'U': 12, 'C_react': 12})
        config = {'chainId': 777962, 'genesisTime': int(time.time()) + 5, 'registrationPerBlock': PROFILE['registrationPerBlock'],
                  'localTiming': PROFILE['localTiming'], 'cp': cp, 'miners': [{'id': founder, 'reward': founder}],
                  'accounts': [{'address': operator, 'wei': str(10**24)}]}
        (self.root / 'config.json').write_text(json.dumps(config), encoding='utf-8')
        tool('pocold', 'init-testnet', '--config', self.root / 'config.json', '--out', self.root / 'genesis')
        p2p, api = port(), port()
        tool('pocol', 'network', '--from', self.root / 'genesis', '--seed', f'127.0.0.1:{p2p}', '--out', self.root / 'network.pocol.json')
        log = open(self.root / 'founder.log', 'w', encoding='utf-8')
        self.logs.append(log)
        self.procs.append(subprocess.Popen([str(BIN / ('pocol' + EXE)), 'start', '--home', str(home), '--network', str(self.root / 'network.pocol.json'),
                                            '--listen', f'127.0.0.1:{p2p}', '--rpc', f'127.0.0.1:{api}'], stdout=log, stderr=log))
        url = f'http://127.0.0.1:{api}'
        deadline = time.time() + 180
        while True:
            try:
                if int(rpc(url, 'eth_blockNumber'), 16) >= 2:
                    break
            except (OSError, AssertionError):
                pass
            self.assertLess(time.time(), deadline, 'the founder node did not start')
            time.sleep(0.5)
        state = self.root / 'harness'
        common = ['--rpc', url, '--dir', state, '--bin-dir', BIN, '--operator-key', op_key]
        # A refusal: the harness is pointed at the rehearsal chain, but this node is the local devnet.
        self.harness('fill', *common, '--count', '1', '--expect-chain-id', PROFILE['chainId'], expect=3)
        miner_base = port()
        # The mining identity's node runs in its own session, outside self.procs: stopped before the temporary directory
        # (its home and key) is removed, whether the test passes or fails. Runs before self.stop and tmp.cleanup (LIFO).
        self.addCleanup(subprocess.run, [sys.executable, '-B', str(HARNESS), 'stop-miners', '--dir', str(state)],
                        capture_output=True, timeout=120)
        out = self.harness('fill', *common, '--count', '20', '--miners', '1', '--bonded', '1', '--network', self.root / 'network.pocol.json',
                           '--peer', f'127.0.0.1:{p2p}', '--miner-port-base', miner_base, '--progress-every', '1')
        report = json.loads((state / 'report.json').read_text())
        self.assertEqual((report['registered'], report['plan']['planned'], report['plan']['clamped']), (7, 7, True), out[-3000:])
        self.assertEqual(report['byRole'], {'miner': 1, 'bonded': 1, 'idle': 5})
        self.assertLessEqual(report['maxRegistrationsInABlock'], PROFILE['registrationPerBlock'])
        self.assertEqual(report['failuresByReason'], {})
        # The founder and the bonded identity are HELD, the others ACCRUING (the mining one's rewards may already have filled
        # its bond to B_reg, which makes it HELD).
        census = report['census']['byBondState']
        self.assertEqual((report['census']['active'], sum(census.values())), (8, 8))
        self.assertIn(census.get('HELD'), (2, 3))
        self.assertEqual(report['registeredAndActiveAtEnd'], 7)
        for key in ('blockIntervalS', 'rpcLatency', 'memberReconstructUsP99Max', 'notMeasuredHere'):
            self.assertIn(key, report['measures'])
        # Each block the harness used: at most R_max of its registrations, as the chain itself shows.
        for per_block in report['registrationsPerBlock']:
            self.assertLessEqual(int(per_block), PROFILE['registrationPerBlock'])
        # The mining identity's own node mines: its record's last credited share moves past its registration.
        miner = next(m for m in report['miners'])
        deadline = time.time() + 240
        while True:
            rec = next(r for r in rpc(url, 'pocol_getMiners') if r['id'] == miner['address'])
            if rec['lastShare'] > rec['registeredAt']:
                break
            self.assertLess(time.time(), deadline, f'the mining identity did not mine: {rec}')
            time.sleep(2)
        self.assertIn(rec['bondState'], (4, 1), 'ACCRUING, or HELD once its rewards filled the bond')
        self.assertEqual(next(i for i in json.loads((state / 'state.json').read_text())['identities'] if i['address'] == miner['address'])['bondWei'], 0)
        # M3 at the full set, and a victim registering again after U.
        self.harness('m3', *common, '--reregister-after-u', '--wait-blocks', '120')
        m3 = json.loads((state / 'state.json').read_text())['m3']
        self.assertTrue(m3['pass'], json.dumps(m3, indent=1))
        c = m3['checks']
        self.assertEqual(len(c['twoReplacingSameBlock']['blocks']), 1)
        self.assertNotEqual(*c['twoReplacingSameBlock']['victims'])
        self.assertTrue(c['registerAloneCapacity']['reason'].startswith('Capacity'), c['registerAloneCapacity'])
        self.assertEqual(c['removeInactiveBelowFullSet']['removalCodeBefore'], 2)
        self.assertTrue(c['victimReregistersAfterU']['pass'])
        status = json.loads(self.harness('status', '--rpc', url, '--dir', state))
        self.assertEqual(status['census']['active'], 8)
        # No key in any output or public file; after stop-miners and delete-keys, no key file at all.
        keys = list(state.rglob('*.pocol-key'))
        self.assertGreater(len(keys), 7)
        shown = ''.join(self.outputs).lower() + (state / 'state.json').read_text().lower() + (state / 'report.json').read_text().lower()
        for k in keys:
            self.assertEqual(k.stat().st_mode & 0o777, 0o600)
            self.assertNotIn(k.read_bytes().hex(), shown)
        self.assertEqual(state.stat().st_mode & 0o777, 0o700)
        # Another spelling of --dir finds the same mining node (its home is recorded as an absolute path).
        self.harness('delete-keys', '--dir', f'{state}/../{state.name}/', expect=3)   # the mining node still runs
        self.assertEqual(len(json.loads(self.harness('stop-miners', '--dir', f'{state}/../{state.name}'))['stopped']), 1)
        self.harness('delete-keys', '--dir', state, expect=3)   # the identities' funds are lost with their keys
        self.assertTrue(list(state.rglob('*.pocol-key')))
        self.assertIn('deletedKeyFiles', self.harness('delete-keys', '--dir', state, '--discard-funds'))
        self.assertEqual(list(state.rglob('*.pocol-key')), [])


if __name__ == '__main__':
    unittest.main()
