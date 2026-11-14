"""The consensus identity reference (VA-SPEC §2.1) against a pinned golden value and the schedule rule, and the
GenesisSpec reference decoder against the Rust verdicts of the decoder corpus."""
import hashlib
import json
from pathlib import Path
import re
import struct
import sys
import unittest

import pocol_identity as ident

REPO = Path(__file__).resolve().parents[2]
CORPUS = REPO / 'prototypes' / 'pocol-extension' / 'tests' / 'data'

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

    def test_the_retired_preview_bit_is_no_flag(self):
        # Mirrors pocold identity::tests::the_retired_preview_bit_is_not_a_flag_and_never_takes_an_aggregate: bit 2 was
        # the v3-preview build's flag. The v3 rules are the release rules (consensus rules 2), so a pinned identity
        # never carries the aggregate, and only the unpinned bit takes it.
        self.assertFalse(hasattr(ident, 'FLAG_V3_PREVIEW'))
        self.assertEqual(ident.FLAGS_WITH_SOURCES, ident.FLAG_UNPINNED)
        genesis = '01' * 32
        for flags in (4, 0, ident.FLAG_CAPS_TEST):
            with self.assertRaises(ValueError, msg=flags):
                ident.consensus_id(7, genesis, 2, flags, [(1, 0)], '08' * 32)
        # build-info carries no v3Preview field, and the flags are bit0 and bit1 only.
        pinned = {'consensusRulesVersion': 2, 'consensusPinned': True, 'features': [], 'consensusSourcesSha256': '08' * 32}
        self.assertEqual(ident.build_flags(pinned), 0)
        self.assertEqual(ident.build_flags(dict(pinned, consensusPinned=False, features=['caps-test'])), ident.FLAG_CAPS_TEST | ident.FLAG_UNPINNED)

    def test_a_spec_2_genesis_opens_on_a_release_build_and_keeps_the_identity_formula(self):
        # A v3 genesis (pocold testnet --v3 writes specVersion 2 and v3 true): the spec version is no identity input,
        # so a release build's identity on it is the documented byte string with rules 2, no flag and no aggregate,
        # built here with struct (genesis_spec.rs SPEC_VERSION is 2).
        self.assertEqual(ident.SPEC_VERSION, 2)
        genesis = '02' * 32
        v3 = {'chainId': 7, 'genesisHash': '0x' + genesis, 'forks': [[1, 0]], 'specVersion': 2, 'v3': True}
        release = {'consensusRulesVersion': 2, 'consensusPinned': True, 'features': [], 'consensusSourcesSha256': '08' * 32}
        data = b'pocol-consensus-identity-v1\0' + struct.pack('>Q', 7) + bytes.fromhex(genesis)
        data += struct.pack('>IBB', 2, 0, 1) + struct.pack('>HQ', 1, 0)
        self.assertEqual(ident.for_network(v3, release), hashlib.sha256(data).hexdigest())
        self.assertEqual(ident.for_network(v3, release), ident.for_network({k: v3[k] for k in ('chainId', 'genesisHash', 'forks')}, release))
        self.assertEqual(ident.for_network(dict(v3, forks=[[1, 0], [2, 1]]), release),
                         ident.consensus_id(7, genesis, 2, 0, [(1, 0), (2, 1)]))
        # An unpinned or caps-test build opens it too, with its own flags (and the aggregate when unpinned).
        self.assertEqual(ident.for_network(v3, dict(release, consensusPinned=False)), ident.consensus_id(7, genesis, 2, ident.FLAG_UNPINNED, [(1, 0)], '08' * 32))
        self.assertEqual(ident.for_network(v3, dict(release, features=['caps-test'])), ident.consensus_id(7, genesis, 2, ident.FLAG_CAPS_TEST, [(1, 0)]))
        # A spec-1 network still opens, with the same formula.
        self.assertEqual(ident.for_network(dict(v3, specVersion=1, v3=False), release), ident.for_network({k: v3[k] for k in ('chainId', 'genesisHash', 'forks')}, release))
        for bad in ({'specVersion': 3, 'v3': True}, {'specVersion': 0}, {'specVersion': True}, {'specVersion': '2', 'v3': True},
                    {'specVersion': 2}, {'specVersion': 2, 'v3': 1}, {'v3': True}, {'specVersion': 1, 'v3': True}, {'v3': False, 'specVersion': 2}):
            with self.assertRaisesRegex(ValueError, 'GenesisSpec version', msg=bad):
                ident.for_network({**{k: v3[k] for k in ('chainId', 'genesisHash', 'forks')}, **bad}, release)
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
    writes them) on every GenesisSpec item of the decoder corpus and of its spec-2 supplement at the release ceiling
    (SPEC_VERSION 2), as refdec.js does (pocol-extension/tests/decoder_diff.mjs): one verdict file per corpus."""

    def test_the_corpus_verdicts_equal_the_rust_reference(self):
        checked, accepted, spec2 = 0, 0, 0
        for items, want in ((corpus('decoder-corpus.json'), json.loads((CORPUS / 'decoder-verdicts.json').read_text())),
                            (corpus('decoder-corpus-v3.json'), json.loads((CORPUS / 'decoder-verdicts-v3.json').read_text()))):
            self.assertEqual(set(want), {item['label'] for item in items})
            for item in items:
                if item['kind'] != 'spec':
                    continue
                b = bytes.fromhex(item['hex'])
                got = verdict(b, ident.SPEC_VERSION)
                self.assertEqual(got, want[item['label']], item['label'])
                # The default ceiling is the release decoder.
                self.assertEqual(ident.decode_genesis_spec.__defaults__, (ident.SPEC_VERSION,))
                accepted += got == 'ok'
                # A spec-1-only ceiling agrees off specVersion 2, which it refuses with gsVersion.
                one = verdict(b, 1)
                if one != 'gsVersion:2':
                    self.assertEqual(one, got, item['label'])
                if got == 'ok' and ident.decode_genesis_spec(b)['specVersion'] == 2:
                    spec2 += 1
                    self.assertEqual(one, 'gsVersion:2', item['label'])
                checked += 1
        self.assertGreater(checked, 1100)
        self.assertGreater(accepted, spec2)
        self.assertGreaterEqual(spec2, 10)

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
        lens = {m[0]: int(m[1]) for m in re.findall(r'pub const (CP_LEN|CP_V2_LEN): usize = (\d+);', source)}
        self.assertEqual((len(ident.CP_SCHEMA), len(ident.CP_SCHEMA_V2)), (lens['CP_LEN'], lens['CP_V2_LEN']))
        self.assertEqual((len(ident.CP_SCHEMA), len(ident.CP_SCHEMA_V2)), (52, 59))
        self.assertEqual(int(re.search(r'const ALPHA_BP: usize = (\d+);', source)[1]), ident._ALPHA_BP)
        self.assertEqual(int(re.search(r'pub const SPEC_VERSION: u8 = (\d+);', source)[1]), ident.SPEC_VERSION)
        # The fields FN-2a (no creation fee) and FN-2b (no probation pool, deadline or seat contest) deleted are gone.
        names = {f[0] for f in ident.CP_SCHEMA_V2}
        self.assertFalse(names & {'P_MAX', 'T_PROB', 'F0_acct', 'F0_slot', 'F0_code', 'E_epoch', 'theta_bp', 'k_sigma'})
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
        # on every spec-2 item the release reference accepts it gives the same version, chain, CP and sysCodeHash.
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
        spec = ident.public_spec(info, v3)
        self.assertEqual(spec, ident.public_spec(info, v3, ident.SPEC_VERSION))
        rho = [f[0] for f in ident.CP_SCHEMA_V2].index('rho_bp')
        self.assertEqual((spec['specVersion'], len(spec['cp']), spec['sysCodeHash'].hex(), rho, spec['cp'][rho]),
                         (2, 59, ident.V3_SYS_CODE_HASH, 58, 2500))
        self.assertEqual(ident.public_spec(info, items['v3/m0-256'], 2)['m0'][255][0], bytes([0x31] * 18 + [0, 255]))
        # A spec-1-only ceiling refuses it with the reference's gsVersion; a malformed spec-2 genesis with its reference
        # verdict; a declaration, chain or program set that differs, with what differs.
        cases = [((info, v3, 1), 'gsVersion:2'), ((info, items['v3/order/len-before-range'], 2), 'gsLen:root'),
                 ((info, items['v3/m0-256-last-system'], 2), 'gsSys:0xff'), ((info, items['v3/gamma-past-bound'], 2), 'gsRange:gamma_bp'),
                 ((info, items['v3/cp-64-before-fn2b'], 2), 'gsCount:CP 64'), ((info, items['v3/cp-67-before-fn2a'], 2), 'gsCount:CP 67'),
                 ((info, items['v3/order/extra-width-unbounded'], 2), 'gsRange:C_react'),
                 ((dict(info, chainId=chain + 1), v3, 2), 'chain ID differs'),
                 ((info, items['devnet-v2/base'], 2), 'v3 program set'),
                 (({'chainId': chain}, v3, 2), 'version differs'), (({'chainId': chain, 'specVersion': 2}, v3, 2), 'inconsistent')]
        for args, message in cases:
            with self.subTest(message=message), self.assertRaisesRegex(ValueError, message):
                ident.public_spec(*args)
        # A spec-1 genesis (the pocold devnet spec of the corpus) under a spec-1 declaration, at either ceiling.
        spec1 = bytes.fromhex((CORPUS / 'devnet-spec.hex').read_text().strip().removeprefix('0x'))
        info1 = {'chainId': ident.decode_genesis_spec(spec1)['chainId']}
        for ceiling in (1, 2):
            decoded = ident.public_spec(info1, spec1, ceiling)
            self.assertEqual((decoded['specVersion'], decoded['chainId'], len(decoded['cp'])), (1, info1['chainId'], 52))
        with self.assertRaisesRegex(ValueError, 'version differs'):
            ident.public_spec({**info1, 'specVersion': 2, 'v3': True}, spec1, 2)


if __name__ == '__main__':
    unittest.main()
