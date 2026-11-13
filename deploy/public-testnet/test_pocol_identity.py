"""The consensus identity reference (VA-SPEC §2.1) against the pinned H15000 value and the schedule rule."""
import hashlib
import json
from pathlib import Path
import re
import struct
import sys
import unittest

import pocol_identity as ident

H15000 = Path(__file__).resolve().parents[1] / 'mainnet' / 'h15000' / 'network.pocol.json'
REPO = Path(__file__).resolve().parents[2]
CORPUS = REPO / 'prototypes' / 'pocol-extension' / 'tests' / 'data'
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
        # The published spec-1 H15000 files give the pinned identity, with or without an explicit specVersion 1.
        info = json.loads(json.loads(H15000.read_text())['files']['devnet.json'])
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



def corpus(name):
    return json.loads((CORPUS / name).read_text())['items']


def verdict(b, max_version):
    try:
        ident.decode_genesis_spec(b, max_version)
        return 'ok'
    except ident.GenesisSpecError as error:
        return f'{error.code}:{error.detail}'


class GenesisSpecReference(unittest.TestCase):
    """The Python port of genesis_spec.rs decode_bounded gives the Rust verdicts (pocold/tests/decoder_reference.rs
    writes them) on every GenesisSpec item of the decoder corpus and of its V3-PR-G3a supplement, at the release ceiling
    and at the v3-preview ceiling, as refdec.js does (pocol-extension/tests/decoder_diff.mjs)."""

    def test_the_corpus_verdicts_equal_the_rust_reference(self):
        release = json.loads((CORPUS / 'decoder-verdicts.json').read_text())
        preview = json.loads((CORPUS / 'decoder-verdicts-preview.json').read_text())
        v3 = json.loads((CORPUS / 'decoder-verdicts-v3.json').read_text())
        checked, accepted = 0, {1: 0, 2: 0}
        for items, want in ((corpus('decoder-corpus.json'), {1: release, 2: preview}),
                            (corpus('decoder-corpus-v3.json'), {1: v3['release'], 2: v3['preview']})):
            for item in items:
                if item['kind'] != 'spec':
                    continue
                b = bytes.fromhex(item['hex'])
                for ceiling in (1, 2):
                    got = verdict(b, ceiling)
                    self.assertEqual(got, want[ceiling][item['label']], (item['label'], ceiling))
                    accepted[ceiling] += got == 'ok'
                # The default ceiling is the release decoder.
                self.assertEqual(verdict(b, ident.SPEC_VERSION), verdict(b, 1))
                checked += 1
        self.assertGreater(checked, 1270)
        self.assertGreater(accepted[2], accepted[1])

    def test_the_version_detail_is_the_exact_decimal_at_any_length(self):
        # genesis_spec.rs reports gsVersion with decimal.rs decimal(raw) whatever the item's length; Python 3.11+ refuses
        # str() of an int past 4300 digits, so the port converts long items itself. Compared with an unlimited str().
        limit = sys.get_int_max_str_digits() if hasattr(sys, 'get_int_max_str_digits') else None
        cases = [b'', bytes(5), b'\x07', b'\0\0\x01\0', bytes(range(256)) * 9, b'\0' * 3 + b'\xff' * 2000, b'\x01' * 1024,
                 b'\x01' * 1025, b'\x80' + bytes(4096), bytes(range(1, 256)) * 40]
        expected = []
        try:
            if limit is not None:
                sys.set_int_max_str_digits(0)
            expected = [str(int.from_bytes(raw, 'big')) for raw in cases]
        finally:
            if limit is not None:
                sys.set_int_max_str_digits(limit)
        self.assertEqual([ident._decimal(raw) for raw in cases], expected)
        self.assertTrue(any(len(x) > 4300 for x in expected))
        # A version item of 10,000 bytes is refused with its full decimal, not with Python's conversion error.
        version = bytes(range(1, 251)) * 40
        spec = bytes([0xf9, 0x27, 0x13, 0xb9, 0x27, 0x10]) + version
        with self.assertRaises(ident.GenesisSpecError) as caught:
            ident.decode_genesis_spec(spec, 2)
        self.assertEqual((caught.exception.code, len(caught.exception.detail)), ('gsVersion', len(ident._decimal(version))))

    def test_the_schemas_are_the_rust_schemas(self):
        source = (REPO / 'prototypes' / 'lp1-node' / 'src' / 'genesis_spec.rs').read_text(encoding='utf-8')
        def schema(name):
            body = re.search(rf'{name}: \[CpField; [A-Z0-9_ -]+\] = \[(.*?)\];', source, re.S)
            self.assertIsNotNone(body, name)
            fields = []
            for field, width, lo, hi in re.findall(r'f\("(\w+)", (\d+), ([\d_]+), Upper::(None|Gamma|Value\([\d_]+\))\)', body[1]):
                bound = None if hi == 'None' else ident.GAMMA if hi == 'Gamma' else int(hi[6:-1].replace('_', ''))
                fields.append((field, int(width), int(lo.replace('_', '')), bound))
            return tuple(fields)
        self.assertEqual(schema('CP_SCHEMA'), ident.CP_SCHEMA)
        self.assertEqual(schema('CP_V2_EXTRA'), ident.CP_V2_EXTRA)
        self.assertEqual((len(ident.CP_SCHEMA), len(ident.CP_SCHEMA_V2)), (52, 67))
        self.assertEqual((ident.cp_schema(1), ident.cp_schema(2), ident.cp_schema(3), ident.cp_schema(0)),
                         (ident.CP_SCHEMA, ident.CP_SCHEMA_V2, None, None))

    def test_the_v3_program_set_hash_is_recomputed_from_the_bundled_programs(self):
        sys.path.insert(0, str(REPO / 'deploy' / 'rewards'))
        import pocol_rewards
        programs = json.loads((REPO / 'prototypes' / 'lp4-execution' / 'tests' / 'data' / 'system-v3.json').read_text())['programs']
        codes = [bytes.fromhex(p['runtime'].removeprefix('0x')) for p in programs]
        self.assertEqual(len(codes), 5)
        self.assertEqual(pocol_rewards.keccak256(pocol_rewards.rlp_encode(codes + [b''])).hex(), ident.V3_SYS_CODE_HASH)
        self.assertEqual('0x' + ident.V3_SYS_CODE_HASH, pocol_rewards.V3_SYS_CODE_HASH)

    def test_the_reward_gateway_reads_every_accepted_v3_genesis_as_the_reference_does(self):
        # deploy/rewards/pocol_rewards keeps its own minimal v3 reader (a trusted genesis hash binds the spec there);
        # on every spec-2 item the v3-preview reference accepts it gives the same version, chain, CP and sysCodeHash.
        sys.path.insert(0, str(REPO / 'deploy' / 'rewards'))
        import pocol_rewards
        accepted = 0
        for item in corpus('decoder-corpus.json') + corpus('decoder-corpus-v3.json'):
            b = bytes.fromhex(item['hex'])
            if item['kind'] != 'spec' or verdict(b, 2) != 'ok' or ident.decode_genesis_spec(b, 2)['specVersion'] != 2:
                continue
            spec = ident.decode_genesis_spec(b, 2)
            self.assertEqual(pocol_rewards.decode_genesis_spec(b), (2, spec['chainId'], spec['cp'], '0x' + spec['sysCodeHash'].hex()), item['label'])
            accepted += 1
        self.assertGreaterEqual(accepted, 100)

    def test_a_public_spec_2_genesis_is_decoded_and_identified(self):
        items = {item['label']: bytes.fromhex(item['hex']) for item in corpus('decoder-corpus-v3.json') + corpus('decoder-corpus.json')}
        v3 = items['v3/base']
        chain = ident.decode_genesis_spec(v3, 2)['chainId']
        info = {'chainId': chain, 'specVersion': 2, 'v3': True}
        spec = ident.public_spec(info, v3, ident.PREVIEW_SPEC_VERSION)
        self.assertEqual((spec['specVersion'], len(spec['cp']), spec['sysCodeHash'].hex(), spec['cp'][66]), (2, 67, ident.V3_SYS_CODE_HASH, 2500))
        self.assertEqual(ident.public_spec(info, items['v3/m0-256'], 2)['m0'][255][0], bytes([0x31] * 18 + [0, 255]))
        # A release ceiling refuses it with the reference's gsVersion; a malformed spec-2 genesis with its reference
        # verdict; a declaration, chain or program set that differs, with what differs.
        cases = [((info, v3, 1), 'gsVersion:2'), ((info, items['v3/order/len-before-range'], 2), 'gsLen:root'),
                 ((info, items['v3/m0-256-last-system'], 2), 'gsSys:0xff'), ((info, items['v3/gamma-past-bound'], 2), 'gsRange:gamma_bp'),
                 ((dict(info, chainId=chain + 1), v3, 2), 'chain ID differs'),
                 ((info, items['devnet-v2/base'], 2), 'v3 program set'),
                 (({'chainId': chain}, v3, 2), 'version differs'), (({'chainId': chain, 'specVersion': 2}, v3, 2), 'inconsistent')]
        for args, message in cases:
            with self.subTest(message=message), self.assertRaisesRegex(ValueError, message):
                ident.public_spec(*args)
        # A spec-1 genesis: the published H15000 spec under its own devnet.json, at either ceiling.
        published = json.loads(H15000.read_text())['files']
        spec1 = bytes.fromhex(published['genesis.spec.hex'].strip().removeprefix('0x'))
        info1 = json.loads(published['devnet.json'])
        for ceiling in (1, 2):
            decoded = ident.public_spec(info1, spec1, ceiling)
            self.assertEqual((decoded['specVersion'], decoded['chainId'], len(decoded['cp'])), (1, 1599846715, 52))
        with self.assertRaisesRegex(ValueError, 'version differs'):
            ident.public_spec({**info1, 'specVersion': 2, 'v3': True}, spec1, 2)


if __name__ == '__main__':
    unittest.main()
