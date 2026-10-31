"""Bounded public-only preflight checks; no real node or service is contacted."""
import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import preflight


class PreflightTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(); self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.bundle = self.root / 'bundle'; self.bundle.mkdir()
        self.data = self.root / 'state'; self.data.mkdir()
        self.source = 'a' * 64
        self.binding = {'chainId': preflight.CHAIN_ID, 'genesisHash': preflight.GENESIS,
                        'compatibilitySha256': self.source, 'protocolVersion': 2,
                        'stateStore': True, 'node': 'seed-1', 'miner': '0x' + '11' * 20}
        self.node = {'name': 'seed-1', 'minerAddress': self.binding['miner'], 'peers': ['192.0.2.1:30300']}
        self.network = {'chainId': preflight.CHAIN_ID, 'genesisHash': preflight.GENESIS,
                        'forks': [[1, 0]], 'keyMode': 'independent-key-files'}
        self.build = {'compatibilitySha256': self.source, 'protocolVersion': 2, 'features': []}
        for filename in preflight.REQUIRED_FILES - {'manifest.json'}:
            path = self.bundle / filename; path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text('public fixture')
        self.write('node.json', self.node); self.write('net/devnet.json', self.network)
        self.write('build-info.json', self.build)
        self.manifest()
        (self.data / 'network-binding.json').write_text(json.dumps(self.binding))
        self.secret = 'NEVER_RETURN_OR_READ_THIS_SECRET'
        (self.data / 'identity.pocol-key').write_text(self.secret)
        self.params = {**self.binding, 'nonceMode': 0, 'activeVersion': 1, 'forkSchedule': [[1, 0]],
                       'networkName': 'PoCol Mainnet', 'networkEnvironment': 'mainnet',
                       'genesisPre': self.secret, 'environment': {'credential': self.secret}}
        self.status = {**{key: value for key, value in self.binding.items() if key != 'node'},
                       'name': self.binding['node'], 'head': {'height': 100, 'hash': '0x' + '22' * 32},
                       'compatiblePeers': 2, 'stats': {'historyDamaged': False}, 'peers': [self.secret]}
        self.block = {'number': '0x64', 'hash': self.status['head']['hash'],
                      'stateRoot': '0x' + '33' * 32, 'receiptsRoot': '0x' + '44' * 32}
        self.calls = []
        self.services = [{'unit': 'pocol-testnet.service', 'available': True, 'activeState': 'active'}]

    def write(self, name, value):
        (self.bundle / name).write_text(json.dumps(value))

    def manifest(self):
        hashes = {p.relative_to(self.bundle).as_posix(): hashlib.sha256(p.read_bytes()).hexdigest()
                  for p in self.bundle.rglob('*') if p.is_file() and p.name != 'manifest.json'}
        self.write('manifest.json', {'schema': 'pocol-public-testnet-bundle-v1',
                                    'identity': self.binding, 'sha256': hashes})

    def rpc(self, method, params):
        self.calls.append((method, params))
        return copy.deepcopy({'pocol_getParams': self.params, 'pocol_status': self.status,
                              'eth_getBlockByNumber': self.block}[method])

    def collect(self):
        return preflight.collect(self.rpc, self.bundle, self.data, services=self.services)

    def test_success_reports_public_identity_and_never_selects_activation(self):
        original = {p: p.read_bytes() for p in self.data.iterdir()}
        result = self.collect()
        self.assertEqual(result['result'], 'PASS')
        self.assertEqual(result['node']['head']['height'], '100')
        self.assertEqual(result['node']['forkSchedule'], [[1, '0']])
        self.assertEqual(result['node']['networkName'], 'PoCol Mainnet')
        self.assertFalse(result['activationHeightSelected']); self.assertFalse(result['deploymentPerformed'])
        self.assertFalse(result['publicLaunchReady'])
        self.assertNotIn(self.secret, json.dumps(result))
        self.assertEqual({p: p.read_bytes() for p in self.data.iterdir()}, original)
        self.assertEqual({method for method, _ in self.calls}, preflight.READ_METHODS)
        self.assertEqual(result['stateBinding'], self.binding)
        self.assertEqual(result['node']['binding'], self.binding)

    def test_full_state_binding_must_exactly_match_operator_launch_identity(self):
        for field, value in (('node', 'different-seed'), ('miner', '0x' + '77' * 20),
                             ('protocolVersion', 3), ('stateStore', False), ('extra', 'private')):
            with self.subTest(field=field):
                (self.data / 'network-binding.json').write_text(json.dumps({**self.binding, field: value}))
                result = self.collect()
                self.assertEqual(result['result'], 'FAIL')
                self.assertFalse(result['checks']['stateBinding'])
                self.assertNotIn('private', json.dumps(result))

    def test_live_status_name_and_miner_must_match_installed_node(self):
        original = copy.deepcopy(self.status)
        for field, value in (('name', 'different-running-seed'), ('miner', '0x' + '77' * 20),
                             ('protocolVersion', 3)):
            with self.subTest(field=field):
                self.status = {**original, field: value}
                self.assertEqual(self.collect()['result'], 'FAIL')

    def test_observer_null_miner_matches_exactly_and_is_not_zero_address(self):
        self.binding['miner'] = self.node['minerAddress'] = self.status['miner'] = None
        self.write('node.json', self.node); self.manifest()
        (self.data / 'network-binding.json').write_text(json.dumps(self.binding))
        result = self.collect()
        self.assertEqual(result['result'], 'PASS')
        self.assertIsNone(result['stateBinding']['miner'])
        self.assertIsNone(result['node']['binding']['miner'])
        self.status['miner'] = '0x' + '00' * 20
        self.assertEqual(self.collect()['result'], 'FAIL')
        self.status['miner'] = None
        (self.data / 'network-binding.json').write_text(json.dumps({**self.binding, 'miner': '0x' + '00' * 20}))
        self.assertFalse(self.collect()['checks']['stateBinding'])

    def test_active_version_must_match_schedule_at_observed_head(self):
        self.params['activeVersion'] = 2
        self.assertFalse(self.collect()['checks']['node'])
        self.params['forkSchedule'] = self.network['forks'] = [[1, 0], [2, 100]]
        self.write('net/devnet.json', self.network); self.manifest()
        for height, active in ((99, 1), (100, 2), (101, 2)):
            with self.subTest(height=height):
                self.status['head']['height'] = height
                self.block['number'] = hex(height)
                self.params['activeVersion'] = active
                self.assertEqual(self.collect()['result'], 'PASS')
                self.params['activeVersion'] = 3 - active
                self.assertFalse(self.collect()['checks']['node'])

    def test_wrong_chain_running_source_or_state_binding_is_refused(self):
        for mutation in ('chain', 'source', 'binding'):
            with self.subTest(mutation=mutation):
                params, status = copy.deepcopy(self.params), copy.deepcopy(self.status)
                if mutation == 'chain': self.params['chainId'] = self.status['chainId'] = 7
                elif mutation == 'source': self.params['compatibilitySha256'] = self.status['compatibilitySha256'] = 'b' * 64
                else: (self.data / 'network-binding.json').write_text(json.dumps({**self.binding, 'compatibilitySha256': 'b' * 64}))
                self.assertEqual(self.collect()['result'], 'FAIL')
                self.params, self.status = params, status
                (self.data / 'network-binding.json').write_text(json.dumps(self.binding))

    def test_hash_change_during_capture_is_not_reported_as_stable(self):
        self.block['hash'] = '0x' + '33' * 32
        result = self.collect()
        self.assertEqual(result['result'], 'FAIL')
        self.assertNotIn('node', result)
        self.assertIn('changed', result['issues'][0]['reason'])

    def test_shared_comparison_height_preserves_full_hashes_and_roots(self):
        def query(method, params):
            if method == 'eth_getBlockByNumber' and params[0] == '0x5a':
                return {**self.block, 'number': '0x5a', 'hash': '0x' + '55' * 32}
            return self.rpc(method, params)
        result = preflight.collect(query, self.bundle, self.data, services=self.services, check_height=90)
        self.assertEqual(result['result'], 'PASS')
        self.assertEqual(result['comparisonBlock']['height'], '90')
        self.assertEqual(result['comparisonBlock']['hash'], '0x' + '55' * 32)
        self.assertEqual(result['comparisonBlock']['stateRoot'], self.block['stateRoot'])
        self.assertEqual(result['node']['head']['height'], '100')
        result = preflight.collect(query, self.bundle, self.data, services=self.services, check_height=101)
        self.assertEqual(result['result'], 'FAIL')

    def test_reorg_during_comparison_read_does_not_publish_comparison(self):
        head_reads = 0
        def query(method, params):
            nonlocal head_reads
            if method == 'eth_getBlockByNumber':
                if params[0] == '0x5a':
                    return {**self.block, 'number': '0x5a', 'hash': '0x' + '55' * 32}
                head_reads += 1
                if head_reads == 2: return {**self.block, 'hash': '0x' + '66' * 32}
            return self.rpc(method, params)
        result = preflight.collect(query, self.bundle, self.data, services=self.services, check_height=90)
        self.assertEqual(result['result'], 'FAIL')
        self.assertNotIn('comparisonBlock', result)
        self.assertNotIn('node', result)

    def test_corrupt_bundle_and_unexpected_key_file_fail_without_reading_key(self):
        (self.bundle / 'bin/pocold').write_text('tampered')
        self.assertEqual(self.collect()['result'], 'FAIL')
        self.manifest()
        key = self.bundle / 'identity.pocol-key'; key.write_text(self.secret)
        original_digest = preflight.digest
        def guarded(path):
            if path == key: raise AssertionError('preflight read a key')
            return original_digest(path)
        with patch.object(preflight, 'digest', side_effect=guarded):
            result = self.collect()
        self.assertEqual(result['result'], 'FAIL')
        self.assertNotIn(self.secret, json.dumps(result))

    def test_activity_and_history_failures_remain_explicit(self):
        self.status['compatiblePeers'] = 0
        self.assertFalse(self.collect()['checks']['peers'])
        self.status['stats']['historyDamaged'] = True
        self.assertFalse(self.collect()['checks']['node'])
        self.status['stats'] = []
        self.assertFalse(self.collect()['checks']['node'])
        self.services[0]['activeState'] = 'inactive'
        self.assertFalse(self.collect()['checks']['service'])

    def test_rpc_endpoint_accepts_only_explicit_loopback_http(self):
        for endpoint in ('http://127.0.0.1:8545', 'http://[::1]:8545/'):
            self.assertEqual(preflight.LocalRpc(endpoint).port, 8545)
        for endpoint in ('https://gateway.pocol.net/rpc', 'http://localhost:8545',
                         'http://169.254.169.254', 'file:///etc/passwd',
                         'http://user:password@127.0.0.1:8545', 'http://127.0.0.1:8545/?secret=x'):
            with self.subTest(endpoint=endpoint), self.assertRaises(ValueError): preflight.LocalRpc(endpoint)
        with self.assertRaises(preflight.PreflightError):
            preflight.LocalRpc('http://127.0.0.1:8545')('eth_sendRawTransaction', ['0x00'])

    def test_service_probe_requests_no_environment_commands_or_credentials(self):
        class Result:
            returncode = 0
            stdout = ('Id=pocol-testnet.service\nLoadState=loaded\nActiveState=active\nSubState=running\n'
                      'WorkingDirectory=/opt/pocol-testnet\nFragmentPath=/etc/systemd/system/pocol-testnet.service\n'
                      'User=pocol-testnet\nEnvironment=NEVER_RETURN_OR_READ_THIS_SECRET\n')
        with patch.object(preflight.subprocess, 'run', return_value=Result()) as process:
            result = preflight.service_report('pocol-testnet.service')
        command = process.call_args.args[0]
        self.assertNotIn('Environment', command[-1]); self.assertNotIn('ExecStart', command[-1])
        self.assertNotIn(self.secret, json.dumps(result))
        self.assertEqual(result['workingDirectory'], '/opt/pocol-testnet')

    def test_schedule_is_precise_and_malformed_shapes_are_refused(self):
        height = 2**63 + 1
        self.params['forkSchedule'] = self.network['forks'] = [[1, 0], [2, height]]
        self.write('net/devnet.json', self.network); self.manifest()
        self.assertEqual(self.collect()['node']['forkSchedule'][1][1], str(height))
        self.params['forkSchedule'] = [[1, 0], [2, True]]
        self.assertFalse(self.collect()['checks']['node'])


if __name__ == '__main__':
    unittest.main()
