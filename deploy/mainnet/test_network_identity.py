#!/usr/bin/env python3
"""network_identity.py switches every listed pin (chain ID, genesis hash, GenesisSpec) with the content
versions and the Studio installer derived from them; --check fails on any other identity; a redo switches
from the current one; line endings are kept; nothing is written when anything fails; bad or non-canonical
input is refused; and no tracked file pins the old Mainnet identity unless it is switched or kept on purpose.

The tests derive their fixtures from the tree's current identity, so they pass before the cutover and on
the reviewed step-3 commit that switches the tree (the runbook merges it through CI)."""
import base64, hashlib, re, shutil, subprocess, tempfile, unittest
from pathlib import Path
import network_identity as ni
import build_installer

CUR_CHAIN, CUR_GENESIS, CUR_SPEC = ni.current_identity(ni.ROOT)
SWITCHED = CUR_CHAIN != ni.OLD_CHAIN_ID
# An independent detector of the old Mainnet identity: the chain ID with or without digit groups, its hex
# digits, and the start of the genesis hash, in any case.
OLD_ANYWHERE = re.compile(r'(?i)1_?599_?846_?715|5f5bb93b|be137bfd')

def sha12(data):
    """The content version the pages use: the first 12 hex digits of SHA-256 (computed here, not by the tool)."""
    return hashlib.sha256(data).hexdigest()[:12]

def versions(page_text, name):
    return set(re.findall(rf'(?<![0-9A-Za-z_.-]){re.escape(name)}\?v=([0-9a-f]+)(?![0-9a-f])', page_text))

def rlp_list(payload):
    n = len(payload)
    if n <= 55:
        return bytes([0xc0 + n]) + payload
    size = n.to_bytes((n.bit_length() + 7) // 8, 'big')
    return bytes([0xf7 + len(size)]) + size + payload

def parts(spec):
    """(version item, chainId payload, the items after chainId) of a GenesisSpec."""
    _, start, end = ni.rlp_item(spec, 0)
    _, _, version_end = ni.rlp_item(spec, start)
    _, s, e = ni.rlp_item(spec, version_end)
    return spec[start:version_end], spec[s:e], spec[e:end]

def fixture(flip):
    """The current GenesisSpec with chainId set to the new chain ID and its last byte changed, so it
    differs from the current identity before and after the cutover."""
    version_item, _, rest = parts(CUR_SPEC)
    chain = ni.NEW_CHAIN_ID.to_bytes(4, 'big')
    spec = bytearray(rlp_list(version_item + bytes([0x80 + len(chain)]) + chain + rest))
    spec[-1] ^= flip
    return bytes(spec)

SPEC_A, SPEC_B = fixture(1), fixture(2)
HASH_A, HASH_B = ('0x' + ni.keccak256(s).hex() for s in (SPEC_A, SPEC_B))
_version, _chain, _rest = parts(SPEC_A)
_payload = SPEC_A[ni.rlp_item(SPEC_A, 0)[1]:]
_cp_is_list, _cp_start, _cp_end = ni.rlp_item(_rest, 0)   # CP, the list after chainId
assert _cp_is_list
NON_CANONICAL = {
    'leading zero in chainId': rlp_list(_version + bytes([0x80 + len(_chain) + 1, 0]) + _chain + _rest),
    'long form chainId': rlp_list(_version + bytes([0xb8, len(_chain)]) + _chain + _rest),
    'version as a list': rlp_list(b'\xc0' + bytes([0x80 + len(_chain)]) + _chain + _rest),
    'trailing bytes': SPEC_A + b'\x00',
    'version 2': rlp_list(b'\x02' + bytes([0x80 + len(_chain)]) + _chain + _rest),
    'version wrapped as 81 01': rlp_list(b'\x81\x01' + bytes([0x80 + len(_chain)]) + _chain + _rest),
    'outer length with a leading zero': bytes([0xfa]) + len(_payload).to_bytes(3, 'big') + _payload,
    'a wrapped single byte after the last item': rlp_list(_payload + b'\x81\x05'),
    'a wrapped single byte inside CP': rlp_list(_version + bytes([0x80 + len(_chain)]) + _chain
                                                + rlp_list(_rest[_cp_start:_cp_end] + b'\x81\x05') + _rest[_cp_end:]),
}

def held(text, chain_id, genesis, spec_hex):
    """(chain ID tokens, hash copies, GenesisSpec copies), counted independently of the tool."""
    spellings = '|'.join(re.escape(s) for s in (str(chain_id), f'{chain_id:_}', hex(chain_id)))
    chain = len(re.findall(rf'(?ai)(?<!\w)(?:{spellings})n?(?!\w)', text))
    run = lambda h: len(re.findall(rf'(?i)(?<![0-9a-f]){h}(?![0-9a-f])', text))
    return chain, run(genesis), run(spec_hex)

# Files that pin the old Mainnet identity on purpose and are not switched. Every other tracked file that
# pins it must be in ni.FILES, so no copy can be missed at the cutover.
KEPT_PREFIXES = {
    'development/': "the project's history",
    'deploy/websites/': 'website snapshots: build output that runbook step 5 updates or rebuilds',
}
KEPT = {
    'deploy/mainnet/network_identity.py': 'the identity it replaces',
    'deploy/mainnet/test_network_identity.py': "this guard's detector",
    'deploy/mainnet/new_network_config.py': 'refuses the old chain ID for the new network',
    'deploy/mainnet/test_new_network_config.py': 'an old devnet.json fixture',
    'deploy/faucet/test_pocol_faucet.py': 'the faucet refuses the old chain',
    'deploy/mainnet/h15000/network.pocol.json': "the old network's invitation (and its published GenesisSpec)",
    'deploy/mainnet/h15000/README.md': "the old network's H15000 upgrade",
    'deploy/mainnet/README.md': "the old network's operations",
    'deploy/mainnet/EARN-IN-CUTOVER-RUNBOOK.md': 'the move from the old network',
    'deploy/mainnet/EARN-IN-CUTOVER-RUNBOOK-AR.md': 'the move from the old network',
}
# Shipped with every package, so the step-3 PR replaces or bumps them (runbook step 3): before the switch
# they may name the old network or its versions, on the switched tree they must not.
STEP3 = {
    'deploy/standalone/RELEASE-UPGRADE.md': OLD_ANYWHERE,
    'deploy/standalone/package.py': re.compile(r"version\s*=\s*'0\.4\.0'"),
    'prototypes/pocol-extension/package.json': re.compile(r'"version":\s*"0\.4\.1"'),
}
TRACKED = list(ni.FILES) + [page for page, _ in ni.VERSIONED] + [ni.INSTALLER]

def unaccounted(files):
    """Paths among (path, text) pairs that pin the old identity but are neither switched nor kept."""
    return [rel for rel, text in files
            if not rel.startswith(tuple(KEPT_PREFIXES)) and rel not in KEPT and rel not in STEP3
            and (rel not in ni.FILES or SWITCHED) and OLD_ANYWHERE.search(text)]

def installer_payloads(text):
    """{name: bytes} embedded in the Studio gateway installer."""
    return {name: base64.b64decode(payload) for name, payload in
            re.findall(r"'([a-z_]+\.py)': \{'sha256': '[0-9a-f]{64}', 'base64': '([A-Za-z0-9+/=]+)'\}", text)}

class NetworkIdentity(unittest.TestCase):
    def copy(self):
        tmp = Path(tempfile.mkdtemp(prefix='pocol-identity-'))
        self.addCleanup(shutil.rmtree, tmp)
        for rel in TRACKED:
            (tmp / rel).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy(ni.ROOT / rel, tmp / rel)
        for name, spec in (('a.hex', SPEC_A), ('b.hex', SPEC_B), *((f'bad{i}.hex', s) for i, s in enumerate(NON_CANONICAL.values()))):
            (tmp / name).write_text('0x' + spec.hex() + '\n', encoding='utf-8')
        return tmp

    def run_tool(self, root, *extra, genesis=HASH_A, spec='a.hex'):
        return ni.main(['--genesis-hash', genesis, '--genesis-spec', str(root / spec), '--root', str(root), *extra])

    def snapshot(self, root):
        # Digests, so a failing comparison reports quickly instead of diffing whole files.
        return {rel: hashlib.sha256((root / rel).read_bytes()).hexdigest() for rel in TRACKED}

    def assert_carries(self, root, genesis, spec):
        for rel, expected in ni.FILES.items():
            self.assertEqual(held((root / rel).read_text(encoding='utf-8'), ni.NEW_CHAIN_ID, genesis[2:], spec.hex()), expected, rel)
        for page, rel in ni.VERSIONED:
            self.assertEqual(versions((root / page).read_text(encoding='utf-8'), Path(rel).name), {sha12((root / rel).read_bytes())}, page)
        payloads = installer_payloads((root / ni.INSTALLER).read_text(encoding='utf-8'))
        self.assertEqual(payloads, {name: (root / rel).read_bytes() for name, rel in ni.INSTALLER_SOURCES.items()})

    def test_the_current_identity_is_proven_and_the_fixtures_are_valid(self):
        self.assertEqual(ni.keccak256(CUR_SPEC).hex(), CUR_GENESIS)
        self.assertEqual(ni.spec_chain_id(CUR_SPEC), CUR_CHAIN)
        self.assertEqual(CUR_CHAIN, ni.NEW_CHAIN_ID if SWITCHED else ni.OLD_CHAIN_ID)
        if not SWITCHED:
            self.assertEqual(CUR_GENESIS, ni.OLD_GENESIS)
        for spec in (SPEC_A, SPEC_B):
            self.assertEqual(ni.spec_chain_id(spec), ni.NEW_CHAIN_ID)
            self.assertNotEqual(spec, CUR_SPEC)

    def test_every_listed_file_holds_its_counts_of_the_current_identity(self):
        for rel, expected in ni.FILES.items():
            self.assertEqual(held((ni.ROOT / rel).read_text(encoding='utf-8'), CUR_CHAIN, CUR_GENESIS, CUR_SPEC.hex()), expected, rel)

    def test_derived_files_follow_their_sources_today(self):
        sources = {name: (ni.ROOT / rel).read_bytes() for name, rel in ni.INSTALLER_SOURCES.items()}
        self.assertEqual((ni.ROOT / ni.INSTALLER).read_text(encoding='utf-8'), build_installer.script(sources),
                         'rebuild it: python3 deploy/studio/build_installer.py')
        for page, rel in ni.VERSIONED:
            self.assertEqual(versions((ni.ROOT / page).read_text(encoding='utf-8'), Path(rel).name), {sha12((ni.ROOT / rel).read_bytes())}, page)

    def test_update_check_and_idempotence(self):
        root = self.copy()
        self.assertEqual(self.run_tool(root, '--check'), 2, 'check fails before the update')
        self.assertEqual(self.run_tool(root), 0)
        self.assert_carries(root, HASH_A, SPEC_A)
        for rel in ni.FILES:
            text = (root / rel).read_text(encoding='utf-8')
            self.assertIsNone(OLD_ANYWHERE.search(text), rel)
            self.assertEqual(held(text, CUR_CHAIN, CUR_GENESIS, CUR_SPEC.hex())[1:], (0, 0), rel)
        rust = (root / 'prototypes/pocold/src/network_display.rs').read_text()
        self.assertIn('const MAINNET_CHAIN_ID: u64 = 261_597_237;', rust)
        self.assertIn(f'const MAINNET_GENESIS: &str = "{HASH_A[2:]}";', rust)
        self.assertIn(f'let genesis = "0x{SPEC_A.hex()}";', rust)
        network = (root / 'prototypes/pocol-extension/src/lib/studio-network.js').read_text()
        for line in ('"chainId": 261597237', f'"genesisHash": "{HASH_A}"', f'"genesisPre": "0x{SPEC_A.hex()}"'):
            self.assertIn(line, network)
        self.assertIn('<span class="network-id">261597237</span>', (root / 'prototypes/pocol-extension/src/contract.html').read_text())
        self.assertIn('>261597237</span>', (root / 'prototypes/pocol-extension/src/publisher.html').read_text())
        self.assertIn("chain:'0xf97a835'", (root / 'prototypes/pocol-extension/tests/contracts_test.mjs').read_text())
        self.assertIn('assert.equal(spec.chainId,261597237n)', (root / 'prototypes/pocol-extension/tests/studio_test.mjs').read_text())
        self.assertEqual(self.run_tool(root, '--check'), 0)
        before = self.snapshot(root)
        self.assertEqual(self.run_tool(root), 0)
        self.assertEqual(before, self.snapshot(root), 'a second run changes nothing')

    def test_check_refuses_another_identity_and_a_redo_switches_to_it(self):
        root = self.copy()
        self.assertEqual(self.run_tool(root), 0)
        self.assertEqual(self.run_tool(root, '--check', genesis=HASH_B, spec='b.hex'), 2, 'pinned to A, not B')
        self.assertEqual(self.run_tool(root, genesis=HASH_B, spec='b.hex'), 0, 'a redo for the same chain')
        self.assert_carries(root, HASH_B, SPEC_B)
        self.assertEqual(self.run_tool(root, '--check', genesis=HASH_B, spec='b.hex'), 0)
        self.assertEqual(self.run_tool(root, '--check'), 2)

    def test_check_refuses_a_stale_content_version_or_installer(self):
        root = self.copy()
        self.assertEqual(self.run_tool(root), 0)
        page, rel = ni.VERSIONED[0]
        switched = (root / page).read_bytes()
        stale = sha12((ni.ROOT / rel).read_bytes())
        (root / page).write_text(switched.decode().replace(f'?v={sha12((root / rel).read_bytes())}', f'?v={stale}'), encoding='utf-8')
        self.assertEqual(self.run_tool(root, '--check'), 2)
        (root / page).write_bytes(switched)
        shutil.copy(ni.ROOT / ni.INSTALLER, root / ni.INSTALLER)
        self.assertEqual(self.run_tool(root, '--check'), 2, 'an installer that still embeds the old sources')

    def test_every_versioned_reference_moves_and_a_missing_one_fails(self):
        root = self.copy()
        page, rel = ni.VERSIONED[0]
        name, text = Path(rel).name, (root / page).read_text(encoding='utf-8')
        old = re.search(rf'{re.escape(name)}\?v=([0-9a-f]+)', text).group(1)
        (root / page).write_text(text + f'<link rel="modulepreload" href="./{name}?v={old}">\n', encoding='utf-8')
        self.assertEqual(self.run_tool(root), 0)
        self.assert_carries(root, HASH_A, SPEC_A)
        root = self.copy()
        (root / page).write_text(re.sub(rf'{re.escape(name)}\?v=[0-9a-f]+', name, text), encoding='utf-8')
        before = self.snapshot(root)
        self.assertEqual(self.run_tool(root), 2)
        self.assertEqual(before, self.snapshot(root))

    def test_line_endings_are_kept(self):
        root = self.copy()
        rel = 'deploy/rewards/gateway.py'
        crlf = (root / rel).read_bytes().replace(b'\n', b'\r\n')
        (root / rel).write_bytes(crlf)
        self.assertEqual(self.run_tool(root), 0)
        data = (root / rel).read_bytes()
        self.assertEqual(data.count(b'\r\n'), crlf.count(b'\r\n'))
        self.assertNotIn(b'\n', data.replace(b'\r\n', b''), 'no bare LF')

    def test_nothing_is_written_when_a_file_fails(self):
        for damage in ('no identity here\n', None):
            root = self.copy()
            rel = list(ni.FILES)[-1]
            text = (root / rel).read_text(encoding='utf-8')
            # Either a file without its pins, or one that holds an extra copy of the current hash.
            (root / rel).write_text(damage if damage else text + f'\n0x{CUR_GENESIS}\n', encoding='utf-8')
            before = self.snapshot(root)
            self.assertEqual(self.run_tool(root), 2)
            self.assertEqual(before, self.snapshot(root))

    def test_a_copy_of_the_new_identity_outside_the_table_fails(self):
        root = self.copy()
        rel = 'deploy/rewards/gateway.py'
        # It still holds its table's copies of the current identity, but after the switch it would hold one
        # copy of the given genesis hash more than its table allows.
        (root / rel).write_text((root / rel).read_text(encoding='utf-8') + f'# {HASH_A}\n', encoding='utf-8')
        before = self.snapshot(root)
        self.assertEqual(self.run_tool(root), 2)
        self.assertEqual(before, self.snapshot(root))

    def test_a_node_file_whose_spec_does_not_prove_its_identity_is_refused(self):
        root = self.copy()
        anchor = root / ni.ANCHOR
        text = anchor.read_text(encoding='utf-8')
        (anchor).write_text(text.replace(f'const MAINNET_GENESIS: &str = "{CUR_GENESIS}";',
                                         f'const MAINNET_GENESIS: &str = "{HASH_B[2:]}";'), encoding='utf-8')
        before = self.snapshot(root)
        with self.assertRaises(SystemExit):
            self.run_tool(root)
        self.assertEqual(before, self.snapshot(root))

    def test_a_file_that_already_holds_the_given_hash_is_refused(self):
        root = self.copy()
        rel = 'deploy/rewards/gateway.py'
        text = (root / rel).read_text(encoding='utf-8')
        (root / rel).write_text(re.sub(CUR_GENESIS, HASH_A[2:], text, flags=re.IGNORECASE), encoding='utf-8')
        before = self.snapshot(root)
        self.assertEqual(self.run_tool(root), 2, 'it no longer holds its copies of the current identity')
        self.assertEqual(before, self.snapshot(root))

    def test_any_case_of_the_hex_chain_id_is_switched(self):
        root = self.copy()
        rel = 'prototypes/pocol-extension/tests/contracts_test.mjs'
        text = (root / rel).read_text(encoding='utf-8')
        upper = '0x' + hex(CUR_CHAIN)[2:].upper()
        self.assertIn(hex(CUR_CHAIN), text)
        (root / rel).write_text(text.replace(hex(CUR_CHAIN), upper), encoding='utf-8')
        self.assertEqual(self.run_tool(root), 0)
        self.assert_carries(root, HASH_A, SPEC_A)

    def test_a_hash_inside_longer_hex_is_neither_counted_nor_changed(self):
        root = self.copy()
        rel = 'deploy/rewards/gateway.py'
        longer = f'# 0x{CUR_GENESIS}ab\n'
        (root / rel).write_text((root / rel).read_text(encoding='utf-8') + longer, encoding='utf-8')
        self.assertEqual(self.run_tool(root), 0)
        self.assertIn(longer, (root / rel).read_text(encoding='utf-8'))

    def test_an_old_mainnet_pin_left_after_a_switch_fails_check_and_redo(self):
        root = self.copy()
        self.assertEqual(self.run_tool(root), 0)
        rel = 'deploy/studio/pocol_studio_gateway.py'
        (root / rel).write_text((root / rel).read_text(encoding='utf-8') + 'LEGACY = 1599846715\n', encoding='utf-8')
        before = self.snapshot(root)
        self.assertEqual(self.run_tool(root, '--check'), 2)
        self.assertEqual(self.run_tool(root, genesis=HASH_B, spec='b.hex'), 2)
        self.assertEqual(before, self.snapshot(root))

    def test_a_node_file_whose_chain_id_its_spec_does_not_prove_is_refused(self):
        root = self.copy()
        anchor = root / ni.ANCHOR
        text = anchor.read_text(encoding='utf-8')
        (anchor).write_text(text.replace(f'const MAINNET_CHAIN_ID: u64 = {CUR_CHAIN:_};',
                                         f'const MAINNET_CHAIN_ID: u64 = {CUR_CHAIN + 1:_};'), encoding='utf-8')
        self.assertNotEqual(text, anchor.read_text(encoding='utf-8'))
        with self.assertRaises(SystemExit):
            self.run_tool(root)

    def test_bad_input_is_refused_and_nothing_is_written(self):
        root = self.copy()
        other = rlp_list(_version + bytes([0x84]) + (ni.NEW_CHAIN_ID + 1).to_bytes(4, 'big') + _rest)
        (root / 'other-chain.hex').write_text(other.hex(), encoding='utf-8')
        (root / 'not-hex.hex').write_text('0xzz', encoding='utf-8')
        (root / 'truncated.hex').write_text(SPEC_A[:-1].hex(), encoding='utf-8')
        before = self.snapshot(root)
        cases = [dict(genesis='0x1234'), dict(genesis='0x' + ni.OLD_GENESIS), dict(genesis='0x' + 'ab' * 32),
                 dict(genesis='0x' + ni.keccak256(other).hex(), spec='other-chain.hex'),
                 dict(spec='not-hex.hex'), dict(spec='truncated.hex'), dict(spec='missing.hex')]
        cases += [dict(genesis='0x' + ni.keccak256(s).hex(), spec=f'bad{i}.hex') for i, s in enumerate(NON_CANONICAL.values())]
        for case in cases:
            with self.subTest(case=case), self.assertRaises(SystemExit):
                self.run_tool(root, **case)
        with self.assertRaises(SystemExit):
            self.run_tool(root, '--chain-id', str(ni.OLD_CHAIN_ID))
        self.assertEqual(before, self.snapshot(root))

    def test_the_guard_flags_planted_copies(self):
        planted = [('a.py', 'X = 1599846715n'), ('b.rs', 'const X: u64 = 1_599_846_715;'), ('c.js', "c='0x5F5BB93B'"),
                   ('d.md', 'genesis be137bfd...'), ('e.json', CUR_SPEC.hex() if not SWITCHED else 'f9016b01845f5bb93b')]
        self.assertEqual(unaccounted(planted), [rel for rel, _ in planted])
        exempt = [('development/x.md', '1599846715'), ('deploy/websites/x.js', '1599846715'), ('deploy/mainnet/README.md', '1599846715'),
                  ('deploy/standalone/RELEASE-UPGRADE.md', '1599846715')]
        self.assertEqual(unaccounted(exempt), [])
        self.assertEqual(unaccounted([(list(ni.FILES)[0], '1599846715')]), [list(ni.FILES)[0]] if SWITCHED else [])

    def test_every_tracked_file_that_pins_the_old_identity_is_switched_or_kept(self):
        try:
            listed = subprocess.run(['git', '-C', str(ni.ROOT), 'ls-files', '-z'], capture_output=True, check=True).stdout
        except (OSError, subprocess.CalledProcessError):
            self.skipTest('not a git checkout')
        def texts():
            for rel in filter(None, listed.decode('utf-8').split('\0')):
                try:
                    yield rel, (ni.ROOT / rel).read_text(encoding='utf-8')
                except (UnicodeDecodeError, OSError):
                    continue
        self.assertEqual(unaccounted(texts()), [], 'pins of the old identity: add the file to FILES, or to KEPT with a reason')
        for rel in KEPT:
            self.assertTrue((ni.ROOT / rel).is_file(), rel)
        for rel, old in STEP3.items():
            text = (ni.ROOT / rel).read_text(encoding='utf-8')
            if SWITCHED:
                self.assertIsNone(old.search(text), f'{rel}: the step-3 PR replaces or bumps it (runbook step 3)')
            else:
                self.assertIsNotNone(old.search(text), f'{rel}: update STEP3 to what it holds today')

if __name__ == '__main__':
    unittest.main()
