import copy
import json
import unittest
from prepare_nonce_upgrade import prepare


class PrepareUpgrade(unittest.TestCase):
    def setUp(self):
        self.old = {'schema': 'pocol-network-v1', 'compatibilitySha256': 'a' * 64,
                    'seeds': ['127.0.0.1:30300'], 'files': {
                        'alloc.json': '{"unchanged":true}\n', 'genesis.spec.hex': '00\n',
                        'devnet.json': json.dumps({'keyMode': 'independent-key-files', 'forks': [[1, 0]], 'chainId': 7})}}
        self.build = {'schema': 'pocol-testnet-build-v1', 'compatibilitySha256': 'b' * 64, 'features': []}

    def test_preserves_everything_except_release_and_fork_schedule(self):
        before = copy.deepcopy(self.old)
        result = prepare(self.old, self.build, 200, 100)
        self.assertEqual(self.old, before)
        self.assertEqual(result['files']['genesis.spec.hex'], self.old['files']['genesis.spec.hex'])
        self.assertEqual(result['files']['alloc.json'], self.old['files']['alloc.json'])
        self.assertEqual(result['seeds'], self.old['seeds'])
        self.assertEqual(json.loads(result['files']['devnet.json'])['forks'], [[1, 0], [2, 200]])

    def test_rejects_retroactive_or_unrepresentable_activation(self):
        for height in [0, 100, 99, -1, 2**64, True]:
            with self.assertRaises(ValueError): prepare(self.old, self.build, height, 100)

    def test_rejects_existing_schedule_and_test_build(self):
        changed = copy.deepcopy(self.old)
        changed['files']['devnet.json'] = json.dumps({'keyMode': 'independent-key-files', 'forks': [[1, 0], [2, 150]]})
        with self.assertRaises(ValueError): prepare(changed, self.build, 200, 100)
        with self.assertRaises(ValueError): prepare(self.old, {**self.build, 'features': ['caps-test']}, 200, 100)


if __name__ == '__main__': unittest.main()
