import copy
import json
import unittest
from unittest.mock import patch
from prepare_nonce_upgrade import pocol_identity, prepare


class PrepareUpgrade(unittest.TestCase):
    def setUp(self):
        self.old = {'schema': 'pocol-network-v2', 'consensusRulesVersion': 1,
                    'seeds': ['127.0.0.1:30300'], 'files': {
                        'alloc.json': '{"unchanged":true}\n', 'genesis.spec.hex': '00\n',
                        'devnet.json': json.dumps({'keyMode': 'independent-key-files', 'forks': [[1, 0]], 'chainId': 7,
                                                   'genesisHash': '0x' + '01' * 32})}}
        self.build = {'schema': 'pocol-testnet-build-v1', 'compatibilitySha256': 'b' * 64, 'features': [],
                      'consensusRulesVersion': 1, 'consensusPinned': True, 'maxForkVersion': 2}

    def test_preserves_everything_except_the_fork_schedule_and_writes_v2(self):
        before = copy.deepcopy(self.old)
        result = prepare(self.old, self.build, [[2, 200]], 100)
        self.assertEqual(self.old, before)
        self.assertEqual(result['files']['genesis.spec.hex'], self.old['files']['genesis.spec.hex'])
        self.assertEqual(result['files']['alloc.json'], self.old['files']['alloc.json'])
        self.assertEqual(result['seeds'], self.old['seeds'])
        self.assertEqual(json.loads(result['files']['devnet.json'])['forks'], [[1, 0], [2, 200]])
        self.assertEqual(list(result), ['schema', 'consensusRulesVersion', 'seeds', 'files'])
        self.assertEqual((result['schema'], result['consensusRulesVersion']), ('pocol-network-v2', 1))

    def test_no_appended_entry_is_a_rebind_with_identical_files(self):
        result = prepare(self.old, self.build, [], 100)
        self.assertEqual(result['files'], self.old['files'])
        self.assertEqual(result['schema'], 'pocol-network-v2')

    def test_repeated_appends_add_entries_in_order_up_to_the_build_maximum(self):
        three = {**self.build, 'maxForkVersion': 3}
        result = prepare(self.old, three, [[2, 200], [3, 300]], 100)
        self.assertEqual(json.loads(result['files']['devnet.json'])['forks'], [[1, 0], [2, 200], [3, 300]])
        with_two = prepare(self.old, self.build, [[2, 200]], 100)
        self.assertEqual(json.loads(prepare(with_two, three, [[3, 300]], 250)['files']['devnet.json'])['forks'],
                         [[1, 0], [2, 200], [3, 300]])
        for appended in ([[3, 300]], [[2, 200], [3, 300]], [[2, 300], [2, 400]]):
            with self.subTest(appended=appended), self.assertRaises(ValueError):
                prepare(self.old, self.build, appended, 100)

    def test_rejects_retroactive_or_unrepresentable_activation(self):
        for height in [0, 100, 99, -1, 2**64, True]:
            with self.assertRaises(ValueError): prepare(self.old, self.build, [[2, height]], 100)
        with_two = prepare(self.old, self.build, [[2, 200]], 100)
        with self.assertRaises(ValueError):
            prepare(with_two, {**self.build, 'maxForkVersion': 3}, [[3, 150]], 100)

    def test_rejects_test_unpinned_and_other_rules_builds(self):
        for change in ({'features': ['caps-test']}, {'consensusPinned': False}, {'consensusRulesVersion': 2},
                       {'consensusRulesVersion': None}):
            with self.subTest(change=change), self.assertRaises(ValueError):
                prepare(self.old, {**self.build, **change}, [[2, 200]], 100)

    def test_a_v1_file_is_converted_only_when_its_build_is_certified(self):
        v1 = {'schema': 'pocol-network-v1', 'compatibilitySha256': 'a' * 64,
              'seeds': self.old['seeds'], 'files': self.old['files']}
        with self.assertRaisesRegex(ValueError, 'other consensus rules'): prepare(v1, self.build, [], 100)
        entry = {'digest': 'a' * 64, 'rules': 1, 'chainId': 7, 'genesisHash': '0x' + '01' * 32,
                 'sources': None, 'certified': 'evidence'}
        with patch.object(pocol_identity, 'LEGACY_BUILDS', (entry,)):
            result = prepare(v1, self.build, [], 100)
        self.assertEqual((result['schema'], result['files']), ('pocol-network-v2', v1['files']))
        self.assertNotIn('compatibilitySha256', result)


if __name__ == '__main__': unittest.main()
