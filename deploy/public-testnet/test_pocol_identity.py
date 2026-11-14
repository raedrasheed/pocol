"""The consensus identity reference (VA-SPEC §2.1) against a pinned golden value and the schedule rule."""
import hashlib
import json
import struct
import unittest

import pocol_identity as ident

# A synthetic network with one scheduled activation; the new network's golden identity is pinned once its genesis
# exists. Pinned also by pocold identity::tests::a_scheduled_upgrade_identity_is_pinned.
CHAIN_ID = 777915
GENESIS = '5a' * 32
FORKS = [(1, 0), (2, 1000)]
GOLDEN = '633afe4beab508ba5ab174300e3961d6ebb72235b68b994d597c508c5abd3021'


def published_devnet_json():
    """devnet.json as `pocold init-testnet` writes it for a spec-1 continuous network (no specVersion field), with
    the activation scheduled."""
    return json.loads(json.dumps({
        'chainId': CHAIN_ID, 'genesisHash': '0x' + GENESIS, 'genesisTime': 1800000000, 'profileKind': 'simnet',
        'keyMode': 'independent-key-files', 'forks': [list(f) for f in FORKS], 'registrationPerBlock': 1,
        'continuous': True, 'miners': [], 'calibration': {'hashesPerSecond': 26214, 'class': 'synthetic-fixture'}}))


class ConsensusIdentity(unittest.TestCase):
    def test_the_scheduled_upgrade_identity_equals_the_pinned_value(self):
        self.assertEqual(ident.consensus_id(CHAIN_ID, GENESIS, 1, 0, FORKS), GOLDEN)

    def test_the_identity_is_the_documented_byte_string(self):
        # Built here with struct, independently of consensus_id's own encoding.
        data = b'pocol-consensus-identity-v1\0' + struct.pack('>Q', CHAIN_ID) + bytes.fromhex(GENESIS)
        data += struct.pack('>IBB', 1, 0, 2) + struct.pack('>HQHQ', 1, 0, 2, 1000)
        self.assertEqual(hashlib.sha256(data).hexdigest(), GOLDEN)

    def test_published_network_files_give_the_pinned_identity(self):
        info = published_devnet_json()
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

    def test_a_pinned_preview_still_commits_its_source_aggregate(self):
        # Mirrors pocold identity::tests::pinned_preview_still_commits_its_source_aggregate and identity.rs flags()/
        # sources(): bit2 (v3-preview) appends the aggregate even on a pinned tree. The Rust test pins no hex value, so
        # the expected bytes are built here with struct, independently of consensus_id's own encoding.
        self.assertEqual(ident.FLAG_V3_PREVIEW, 4)
        genesis = '01' * 32
        def documented(flags, sources=None):
            data = b'pocol-consensus-identity-v1\0' + struct.pack('>Q', 7) + bytes.fromhex(genesis)
            data += struct.pack('>IBB', 1, flags, 1) + struct.pack('>HQ', 1, 0) + (bytes.fromhex(sources) if sources else b'')
            return hashlib.sha256(data).hexdigest()
        ordinary = ident.consensus_id(7, genesis, 1, 0, [(1, 0)])
        a = ident.consensus_id(7, genesis, 1, ident.FLAG_V3_PREVIEW, [(1, 0)], '08' * 32)
        b = ident.consensus_id(7, genesis, 1, ident.FLAG_V3_PREVIEW, [(1, 0)], '09' * 32)
        unpinned = ident.consensus_id(7, genesis, 1, ident.FLAG_V3_PREVIEW | ident.FLAG_UNPINNED, [(1, 0)], '08' * 32)
        self.assertEqual(a, documented(4, '08' * 32))
        self.assertEqual(unpinned, documented(6, '08' * 32))
        self.assertEqual(len({ordinary, a, b, unpinned}), 4)
        # The aggregate goes exactly with bit1 or bit2 (identity.rs asserts the same).
        with self.assertRaises(ValueError):
            ident.consensus_id(7, genesis, 1, ident.FLAG_V3_PREVIEW, [(1, 0)])
        with self.assertRaises(ValueError):
            ident.consensus_id(7, genesis, 1, 0, [(1, 0)], '08' * 32)
        # build-info: a pinned preview build reports v3Preview true and 'v3-preview' in features (pocold build.rs);
        # either one sets bit2, and its consensusSourcesSha256 (the actual aggregate) is appended.
        info = {'chainId': 7, 'genesisHash': '0x' + genesis, 'forks': [[1, 0]]}
        pinned = {'consensusRulesVersion': 1, 'consensusPinned': True, 'features': [], 'consensusSourcesSha256': '08' * 32}
        self.assertEqual(ident.for_network(info, pinned), ordinary)
        self.assertEqual(ident.for_network(info, dict(pinned, v3Preview=False)), ordinary)
        for preview in (dict(pinned, v3Preview=True, features=['v3-preview']), dict(pinned, v3Preview=True),
                        dict(pinned, features=['v3-preview'])):
            self.assertEqual(ident.build_flags(preview), ident.FLAG_V3_PREVIEW, preview)
            self.assertEqual(ident.for_network(info, preview), a, preview)
        self.assertEqual(ident.for_network(info, dict(pinned, v3Preview=True, consensusSourcesSha256='09' * 32)), b)
        self.assertEqual(ident.for_network(info, dict(pinned, v3Preview=True, consensusPinned=False)), unpinned)
        # The ordinary pinned identity is unchanged.
        scheduled = {'chainId': CHAIN_ID, 'genesisHash': '0x' + GENESIS, 'forks': [list(f) for f in FORKS]}
        self.assertEqual(ident.for_network(scheduled, dict(pinned, v3Preview=False)), GOLDEN)

    def test_a_spec_2_genesis_needs_a_preview_build_and_keeps_the_identity_formula(self):
        # A v3 genesis (pocold testnet --v3 writes specVersion 2 and v3 true): the spec version is no identity input,
        # so a preview build's identity is the documented byte string with bit2 and the aggregate, built here with
        # struct. A release build refuses it (genesis_spec.rs SPEC_VERSION is 1 until V3-PR-I).
        self.assertEqual((ident.SPEC_VERSION, ident.PREVIEW_SPEC_VERSION), (1, 2))
        genesis = '02' * 32
        v3 = {'chainId': 7, 'genesisHash': '0x' + genesis, 'forks': [[1, 0]], 'specVersion': 2, 'v3': True}
        pinned = {'consensusRulesVersion': 1, 'consensusPinned': True, 'features': [], 'consensusSourcesSha256': '08' * 32}
        preview = dict(pinned, v3Preview=True, features=['v3-preview'])
        data = b'pocol-consensus-identity-v1\0' + struct.pack('>Q', 7) + bytes.fromhex(genesis)
        data += struct.pack('>IBB', 1, 4, 1) + struct.pack('>HQ', 1, 0) + bytes.fromhex('08' * 32)
        self.assertEqual(ident.for_network(v3, preview), hashlib.sha256(data).hexdigest())
        self.assertEqual(ident.for_network(v3, preview), ident.for_network({k: v3[k] for k in ('chainId', 'genesisHash', 'forks')}, preview))
        self.assertEqual(ident.for_network(dict(v3, forks=[[1, 0], [2, 1]]), preview),
                         ident.consensus_id(7, genesis, 1, 4, [(1, 0), (2, 1)], '08' * 32))
        for release in (pinned, dict(pinned, v3Preview=False), dict(pinned, consensusPinned=False), dict(pinned, features=['caps-test'])):
            with self.assertRaisesRegex(ValueError, 'release build refuses a spec-2', msg=release):
                ident.for_network(v3, release)
        # A preview build still opens a spec-1 network (with its own isolated identity).
        self.assertEqual(ident.for_network(dict(v3, specVersion=1, v3=False), preview), ident.for_network({k: v3[k] for k in ('chainId', 'genesisHash', 'forks')}, preview))
        for bad in ({'specVersion': 3, 'v3': True}, {'specVersion': 0}, {'specVersion': True}, {'specVersion': '2', 'v3': True},
                    {'specVersion': 2}, {'specVersion': 2, 'v3': 1}, {'v3': True}, {'specVersion': 1, 'v3': True}, {'v3': False, 'specVersion': 2}):
            with self.assertRaisesRegex(ValueError, 'GenesisSpec version', msg=bad):
                ident.for_network({**{k: v3[k] for k in ('chainId', 'genesisHash', 'forks')}, **bad}, preview)
        # Published spec-1 files give the pinned identity, with or without an explicit specVersion 1.
        info = published_devnet_json()
        build = {'consensusRulesVersion': 1, 'consensusPinned': True, 'features': [], 'consensusSourcesSha256': '00' * 32}
        self.assertNotIn('specVersion', info)
        self.assertEqual(ident.spec_version(info), 1)
        self.assertEqual(ident.for_network(dict(info, specVersion=1), build), GOLDEN)

    def test_the_schedule_rule_is_the_runtime_shape(self):
        self.assertEqual(ident.validated_schedule([[1, 0], [2, 9]]), ((1, 0), (2, 9)))
        self.assertEqual(ident.validated_schedule([[1, 0], [2, 9], [3, 20]], 3)[-1], (3, 20))
        for bad in ([], [[2, 0]], [[1, 1]], [[1, 0], [1, 5]], [[1, 0], [3, 5]], [[1, 0], [2, 0]], [[1, 0], [2, 9], [3, 20]],
                    [[1, 0]] * 17, [[1, 0], [2, 1 << 64]]):
            with self.assertRaises(ValueError, msg=bad):
                ident.validated_schedule(bad)


if __name__ == '__main__':
    unittest.main()
