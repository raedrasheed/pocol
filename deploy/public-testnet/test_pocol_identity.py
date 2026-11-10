"""The consensus identity reference (VA-SPEC §2.1) against the pinned H15000 value and the schedule rule."""
import hashlib
import json
from pathlib import Path
import struct
import unittest

import pocol_identity as ident

H15000 = Path(__file__).resolve().parents[1] / 'mainnet' / 'h15000' / 'network.pocol.json'
GENESIS = 'be137bfdb4fdf332f2e1500ee1a67e0b22ce6bdf57efa46fb4ea7b0debe509ca'
# Pinned also by pocold identity::tests::the_h15000_identity_is_pinned.
GOLDEN = 'bcc4b71570fed4788d84baff137e26a88527c1fd17a11f595d92ce1e5e50b110'


class ConsensusIdentity(unittest.TestCase):
    def test_the_h15000_identity_equals_the_pinned_value(self):
        self.assertEqual(ident.consensus_id(1599846715, GENESIS, 1, 0, [(1, 0), (2, 15000)]), GOLDEN)

    def test_the_identity_is_the_documented_byte_string(self):
        # Built here with struct, independently of consensus_id's own encoding.
        data = b'pocol-consensus-identity-v1\0' + struct.pack('>Q', 1599846715) + bytes.fromhex(GENESIS)
        data += struct.pack('>IBB', 1, 0, 2) + struct.pack('>HQHQ', 1, 0, 2, 15000)
        self.assertEqual(hashlib.sha256(data).hexdigest(), GOLDEN)

    def test_the_published_h15000_network_files_give_the_pinned_identity(self):
        info = json.loads(json.loads(H15000.read_text())['files']['devnet.json'])
        build = {'consensusRulesVersion': 1, 'consensusPinned': True, 'features': [], 'consensusSourcesSha256': '00' * 32}
        self.assertEqual(ident.for_network(info, build), GOLDEN)

    def test_flags_and_the_unpinned_aggregate_change_the_identity(self):
        info = {'chainId': 7, 'genesisHash': '0x' + '01' * 32, 'forks': [[1, 0]]}
        pinned = {'consensusRulesVersion': 1, 'consensusPinned': True, 'features': [], 'consensusSourcesSha256': 'aa' * 32}
        ids = {ident.for_network(info, pinned), ident.for_network(info, dict(pinned, features=['caps-test'])),
               ident.for_network(info, dict(pinned, consensusPinned=False)),
               ident.for_network(info, dict(pinned, consensusPinned=False, consensusSourcesSha256='bb' * 32)),
               ident.for_network(info, dict(pinned, consensusRulesVersion=2)),
               ident.for_network(dict(info, forks=[[1, 0], [2, 5]]), pinned)}
        self.assertEqual(len(ids), 6)
        with self.assertRaises(ValueError):
            ident.consensus_id(7, info['genesisHash'], 1, ident.FLAG_UNPINNED, [(1, 0)])

    def test_the_schedule_rule_is_the_runtime_shape(self):
        self.assertEqual(ident.validated_schedule([[1, 0], [2, 9]]), ((1, 0), (2, 9)))
        self.assertEqual(ident.validated_schedule([[1, 0], [2, 9], [3, 20]], 3)[-1], (3, 20))
        for bad in ([], [[2, 0]], [[1, 1]], [[1, 0], [1, 5]], [[1, 0], [3, 5]], [[1, 0], [2, 0]], [[1, 0], [2, 9], [3, 20]],
                    [[1, 0]] * 17, [[1, 0], [2, 1 << 64]]):
            with self.assertRaises(ValueError, msg=bad):
                ident.validated_schedule(bad)


if __name__ == '__main__':
    unittest.main()
