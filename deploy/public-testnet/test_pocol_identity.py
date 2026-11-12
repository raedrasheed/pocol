"""The consensus identity reference (VA-SPEC §2.1) against the pinned H15000 value and the schedule rule."""
import hashlib
import json
from pathlib import Path
import re
import struct
import unittest

import pocol_identity as ident

H15000 = Path(__file__).resolve().parents[1] / 'mainnet' / 'h15000' / 'network.pocol.json'
LAUNCHER = Path(__file__).resolve().parents[2] / 'prototypes' / 'pocold' / 'src' / 'launcher.rs'
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
        # The ordinary pinned H15000 identity is unchanged.
        h15000 = {'chainId': 1599846715, 'genesisHash': '0x' + GENESIS, 'forks': [[1, 0], [2, 15000]]}
        self.assertEqual(ident.for_network(h15000, dict(pinned, v3Preview=False)), GOLDEN)

    def test_the_schedule_rule_is_the_runtime_shape(self):
        self.assertEqual(ident.validated_schedule([[1, 0], [2, 9]]), ((1, 0), (2, 9)))
        self.assertEqual(ident.validated_schedule([[1, 0], [2, 9], [3, 20]], 3)[-1], (3, 20))
        for bad in ([], [[2, 0]], [[1, 1]], [[1, 0], [1, 5]], [[1, 0], [3, 5]], [[1, 0], [2, 0]], [[1, 0], [2, 9], [3, 20]],
                    [[1, 0]] * 17, [[1, 0], [2, 1 << 64]]):
            with self.assertRaises(ValueError, msg=bad):
                ident.validated_schedule(bad)


    def test_the_legacy_mirror_equals_the_launcher_table(self):
        # Lexical: every Legacy { .. } literal in LEGACY_BUILDS, so the Python tools certify exactly what pocol does.
        source = LAUNCHER.read_text(encoding='utf-8')
        table = re.search(r'pub const LEGACY_BUILDS: &\[Legacy\] = &\[(.*?)\];', source, re.S)
        self.assertIsNotNone(table, 'LEGACY_BUILDS not found in launcher.rs')
        def value(text):
            text = text.strip()
            if text == 'None': return None
            some = re.fullmatch(r'Some\("([^"]*)"\)', text)
            if some: return some[1]
            return text[1:-1] if text.startswith('"') else int(text)
        names = {'digest': 'digest', 'rules': 'rules', 'chain_id': 'chainId', 'genesis_hash': 'genesisHash',
                 'sources': 'sources', 'certified': 'certified'}
        entries = []
        for body in re.findall(r'Legacy \{(.*?)\}', table[1], re.S):
            fields = dict(re.findall(r'(\w+):\s*((?:Some\("[^"]*"\))|None|"[^"]*"|[0-9]+)', body))
            self.assertEqual(set(fields), set(names), body)
            entries.append({names[k]: value(v) for k, v in fields.items()})
        self.assertEqual(tuple(entries), ident.LEGACY_BUILDS)

    def test_a_legacy_build_has_rules_only_when_certified_and_on_its_own_chain(self):
        entry = dict(ident.LEGACY_BUILDS[0])
        args = (entry['digest'], entry['chainId'], entry['genesisHash'])
        self.assertIsNone(ident.legacy_rules(*args), 'shipped uncertified (owner decision D1)')
        certified = (dict(entry, certified='evidence'),)
        self.assertEqual(ident.legacy_rules(*args, table=certified), 1)
        self.assertEqual(ident.legacy_rules(entry['digest'].upper(), entry['chainId'], entry['genesisHash'].upper(), table=certified), 1)
        for other in ((('f' * 64),) + args[1:], (args[0], 7, args[2]), (args[0], args[1], '0x' + '00' * 32),
                      (None, args[1], args[2]), (args[0], True, args[2])):
            self.assertIsNone(ident.legacy_rules(*other, table=certified), other)


if __name__ == '__main__':
    unittest.main()
