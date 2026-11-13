"""Bounded public-only preflight checks; no real node or service is contacted."""
import copy
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

import preflight

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'public-testnet'))
import pocol_identity  # noqa: E402


H15000 = Path(__file__).parent / 'h15000/network.pocol.json'
H15000_SPEC = json.loads(H15000.read_text())['files']['genesis.spec.hex']
# A spec-2 (v3) GenesisSpec from the decoder corpus: the Rust reference gives it gsVersion in the release build and
# accepts it in the v3-preview build (pocold/tests/decoder_reference.rs, the two verdict files).
EXTENSION_DATA = Path(__file__).resolve().parents[2] / 'prototypes/pocol-extension/tests/data'
SPEC_V2 = next(item['hex'] for item in json.loads((EXTENSION_DATA / 'decoder-corpus-v3.json').read_text())['items']
               if item['label'] == 'v3/base')
# The same spec naming another program set (the decoder accepts it; pocold would not run it), and malformed spec-2 items.
SPEC_V2_OTHER_PROGRAMS = next(item['hex'] for item in json.loads((EXTENSION_DATA / 'decoder-corpus.json').read_text())['items']
                              if item['label'] == 'devnet-v2/base')
V3_CORPUS = {item['label']: item['hex'] for item in json.loads((EXTENSION_DATA / 'decoder-corpus-v3.json').read_text())['items']}


class PreflightTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(); self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.bundle = self.root / 'bundle'; self.bundle.mkdir()
        self.data = self.root / 'state'; self.data.mkdir()
        # A 0.4.0 seed: the published legacy source and no consensus identity fields (plan T29).
        self.source = preflight.LEGACY_SOURCE
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
        # The published GenesisSpec of the network this fixture names (spec version 1).
        (self.bundle / 'net/genesis.spec.hex').write_text(H15000_SPEC + '\n')
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


    def r1(self, marker_source=None):
        """Turn the fixture into an R1 seed: a pinned build that reports its rules and consensus identity."""
        self.source = 'c' * 64
        identity = pocol_identity.consensus_id(preflight.CHAIN_ID, preflight.GENESIS, 1, 0, [(1, 0)])
        fields = {'compatibilitySha256': self.source, 'consensusRulesVersion': 1, 'consensusIdentity': identity}
        self.binding.update(fields)
        self.build.update(compatibilitySha256=self.source, consensusRulesVersion=1, consensusPinned=True)
        self.write('build-info.json', self.build); self.manifest()
        (self.data / 'network-binding.json').write_text(json.dumps({**self.binding, 'compatibilitySha256': marker_source or self.source}))
        for value in (self.params, self.status): value.update(fields, consensusPinned=True)
        return identity

    def test_an_r1_seed_passes_and_its_identity_is_recomputed_from_public_fields(self):
        identity = self.r1()
        result = self.collect()
        self.assertEqual(result['result'], 'PASS', result['issues'])
        self.assertEqual(result['node']['consensusIdentity'], identity)
        self.assertNotIn('legacyRules', result['node'])
        self.assertEqual(result['warnings'], [])

    def test_a_marker_of_an_earlier_build_of_the_same_rules_passes_with_a_warning(self):
        self.r1(marker_source='d' * 64)
        result = self.collect()
        self.assertEqual(result['result'], 'PASS', result['issues'])
        self.assertEqual(len(result['warnings']), 1)
        self.assertIn('another build of the same consensus rules', result['warnings'][0])

    def test_a_consensus_identity_that_does_not_match_its_fields_fails(self):
        self.r1()
        for value in (self.params, self.status): value['consensusIdentity'] = 'e' * 64
        result = self.collect()
        self.assertFalse(result['checks']['node'])
        self.assertIn('does not match', result['issues'][0]['reason'])
        self.r1()
        self.params['forkSchedule'] = [[1, 0], [2, 50]]
        self.assertFalse(self.collect()['checks']['node'], 'another schedule is another identity')

    def test_an_unpinned_r1_node_or_bundle_fails(self):
        self.r1(); self.status['consensusPinned'] = False
        self.assertFalse(self.collect()['checks']['node'])
        self.r1(); self.build['consensusPinned'] = False; self.write('build-info.json', self.build); self.manifest()
        self.assertFalse(self.collect()['checks']['bundle'])

    def test_a_running_consensus_identity_other_than_the_bundle_fails(self):
        self.r1()
        self.build['consensusRulesVersion'] = 2; self.write('build-info.json', self.build)
        self.binding['consensusRulesVersion'] = 2
        self.binding['consensusIdentity'] = pocol_identity.consensus_id(preflight.CHAIN_ID, preflight.GENESIS, 2, 0, [(1, 0)])
        self.manifest()
        result = self.collect()
        self.assertFalse(result['checks']['bundle'])
        self.assertIn('running consensus identity differs', json.dumps(result['issues']))

    def test_only_the_published_legacy_source_may_omit_the_consensus_identity(self):
        for value in (self.params, self.status): value['compatibilitySha256'] = 'b' * 64
        result = self.collect()
        self.assertFalse(result['checks']['node'])
        self.assertIn('no consensus identity', result['issues'][0]['reason'])

    def test_a_legacy_marker_under_an_r1_bundle_is_refused(self):
        self.r1()
        legacy = {k: v for k, v in self.binding.items() if k not in ('consensusRulesVersion', 'consensusIdentity')}
        (self.data / 'network-binding.json').write_text(json.dumps({**legacy, 'compatibilitySha256': preflight.LEGACY_SOURCE}))
        self.assertFalse(self.collect()['checks']['stateBinding'])

    def test_a_0_4_0_seed_reports_legacy_rules(self):
        result = self.collect()
        self.assertEqual(result['result'], 'PASS', result['issues'])
        self.assertEqual(result['node']['legacyRules'], 1)
        self.assertNotIn('consensusIdentity', result['node'])

    def test_the_schedule_rule_is_generic_with_the_runtime_shape(self):
        self.assertEqual(preflight.forks([[1, 0], [2, 9]]), [[1, 0], [2, 9]])
        with patch.object(preflight, 'MAX_FORK_VERSION', 3):
            self.assertEqual(preflight.forks([[1, 0], [2, 9], [3, 20]])[-1], [3, 20])
        for bad in ([], [[2, 0]], [[1, 1]], [[1, 0], [1, 5]], [[1, 0], [3, 5]], [[1, 0], [2, 0]],
                    [[1, 0], [2, 9], [3, 20]], [[1, 0]] * 17, [[1, 0], [2, 9], [2, 20]]):
            with self.subTest(bad=bad), self.assertRaises(preflight.PreflightError): preflight.forks(bad)

    def test_a_release_seed_refuses_a_spec_2_genesis(self):
        verdicts = {name: json.loads((EXTENSION_DATA / name).read_text())['devnet-v2/base']
                    for name in ('decoder-verdicts.json', 'decoder-verdicts-preview.json')}
        self.assertEqual(verdicts, {'decoder-verdicts.json': 'gsVersion:2', 'decoder-verdicts-preview.json': 'ok'})
        v3_verdicts = json.loads((EXTENSION_DATA / 'decoder-verdicts-v3.json').read_text())
        self.assertEqual((v3_verdicts['release']['v3/base'], v3_verdicts['preview']['v3/base']), ('gsVersion:2', 'ok'))
        # The inline v3 program set hash and CP count are the reference's.
        self.assertEqual((preflight.V3_SYS_CODE_HASH, preflight.V3_CP_ITEMS), (pocol_identity.V3_SYS_CODE_HASH, len(pocol_identity.CP_SCHEMA_V2)))
        self.assertEqual(preflight.SPEC_VERSION, 1)
        self.assertEqual(preflight.spec_version(self.network, H15000_SPEC), 1)
        self.assertEqual(preflight.spec_version({'specVersion': 1}, '0x' + H15000_SPEC[2:].upper() + '\n'), 1)
        v3 = {**self.network, 'specVersion': 2, 'v3': True}
        cases = [(v3, SPEC_V2, 'refuses a spec-2'), (self.network, SPEC_V2, 'differs from the network metadata'),
                 (v3, H15000_SPEC, 'differs from the network metadata'),
                 ({**self.network, 'specVersion': 2}, SPEC_V2, 'inconsistent'), ({**self.network, 'v3': True}, SPEC_V2, 'inconsistent'),
                 ({**self.network, 'specVersion': 3, 'v3': True}, SPEC_V2, 'inconsistent'), ({**self.network, 'specVersion': True}, H15000_SPEC, 'inconsistent'),
                 (self.network, 'public fixture', 'invalid public GenesisSpec'), (self.network, '0x', 'invalid public GenesisSpec'),
                 (self.network, '01', 'invalid public GenesisSpec version'), (self.network, 'c0', 'invalid public GenesisSpec version'),
                 (self.network, 'c3810102', 'invalid public GenesisSpec version'), (self.network, 'c180', 'invalid public GenesisSpec version'),
                 # A spec-2 genesis is identified as a v3 genesis (and then refused) only when well formed with the v3
                 # program set; otherwise the report says what it is.
                 (v3, SPEC_V2_OTHER_PROGRAMS, 'does not name the v3 program set'),
                 (v3, V3_CORPUS['v3/order/count-before-int'], 'invalid spec-2 \\(v3\\) GenesisSpec'),
                 (v3, V3_CORPUS['v3/cp-v2-extras-nested'], 'invalid spec-2 \\(v3\\) GenesisSpec'),
                 (v3, V3_CORPUS['v3/order/len-before-range'], 'invalid spec-2 \\(v3\\) GenesisSpec'),
                 (v3, SPEC_V2[:-2], 'invalid spec-2 \\(v3\\) GenesisSpec'), (v3, SPEC_V2 + '00', 'invalid spec-2 \\(v3\\) GenesisSpec'),
                 (v3, 'c202c0', 'invalid spec-2 \\(v3\\) GenesisSpec'), (v3, 'c102', 'invalid spec-2 \\(v3\\) GenesisSpec')]
        for network, spec, message in cases:
            with self.subTest(network=network, spec=spec[:12]):
                with self.assertRaisesRegex(preflight.PreflightError, message): preflight.spec_version(network, spec)
        # The declaration rule mirrors pocol_identity.spec_version.
        for network in ({}, {'specVersion': 1}, {'specVersion': 2, 'v3': True}, {'specVersion': 2}, {'v3': True}, {'specVersion': 0}):
            try: reference = pocol_identity.spec_version(network)
            except ValueError: reference = None
            try: inline = preflight.spec_version(network, SPEC_V2 if network.get('specVersion') == 2 else H15000_SPEC)
            except preflight.PreflightError as error: inline = 2 if 'refuses a spec-2' in str(error) else None
            self.assertEqual(inline, reference, network)
        # A bundle on a spec-2 genesis fails its stage, before any identity is reported; the H15000 bundle passes.
        self.assertEqual(self.collect()['result'], 'PASS')
        (self.bundle / 'net/genesis.spec.hex').write_text(SPEC_V2 + '\n'); self.write('net/devnet.json', v3); self.manifest()
        result = self.collect()
        self.assertEqual(result['result'], 'FAIL')
        self.assertFalse(result['checks']['bundle'])
        self.assertIn({'stage': 'bundle', 'reason': 'a release seed refuses a spec-2 (v3) genesis until activation'}, result['issues'])
        self.write('net/devnet.json', self.network); self.manifest()
        self.assertIn({'stage': 'bundle', 'reason': 'GenesisSpec version differs from the network metadata'}, self.collect()['issues'])

    def test_the_inline_consensus_id_equals_the_reference_implementation(self):
        cases = [(preflight.CHAIN_ID, preflight.GENESIS, 1, [[1, 0], [2, 150]]), (7, '0x' + '01' * 32, 2, [[1, 0]]),
                 (2**64 - 1, '0x' + 'ff' * 32, 2**32 - 1, [[1, 0], [2, 2**64 - 1]])]
        for chain, genesis, rules, schedule in cases:
            with self.subTest(chain=chain):
                self.assertEqual(preflight.consensus_id(chain, genesis, rules, schedule),
                                 pocol_identity.consensus_id(chain, genesis, rules, 0, schedule))
        # The published H15000 network, read from its invitation so the old identity is not pinned here again.
        info = json.loads(json.loads((Path(__file__).parent / 'h15000/network.pocol.json').read_text())['files']['devnet.json'])
        self.assertEqual(preflight.consensus_id(info['chainId'], info['genesisHash'], 1, info['forks']),
                         'bcc4b71570fed4788d84baff137e26a88527c1fd17a11f595d92ce1e5e50b110', 'the T11 golden identity')


if __name__ == '__main__':
    unittest.main()
