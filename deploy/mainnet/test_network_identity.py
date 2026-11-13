#!/usr/bin/env python3
"""network_identity.py switches every listed pin (chain ID, genesis hash, GenesisSpec) with the content
versions and the Studio installer derived from them; --check fails on any other identity; a redo switches
from the current one; line endings are kept, and hashed sources with converted (CRLF) line endings are
refused; nothing is written when anything fails, an I/O error included; bad or non-canonical input is
refused; no tracked file pins the old Mainnet identity unless it is switched or kept on purpose; and on the
switched tree the shipped notes, versions and release texts of the old network are replaced (runbook step 3).

The tests derive their fixtures from the tree's current identity, so they pass before the cutover and on
the reviewed step-3 commit that switches the tree (the runbook merges it through CI)."""
import base64, hashlib, json, os, re, shutil, subprocess, tempfile, unittest
from pathlib import Path
from unittest import mock
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
    'a 55-byte item in long form': rlp_list(_payload + b'\xb8\x37' + b'\x01' * 55),
    'an item that overruns its list': rlp_list(_payload + b'\xc2\x01\x82\x03\x04'),
}

def int_item(v):
    """A canonical RLP integer item."""
    raw = v.to_bytes((v.bit_length() + 7) // 8, 'big')
    return raw if len(raw) == 1 and raw[0] < 0x80 else bytes([0x80 + len(raw)]) + raw

# A spec-2 (v3) genesis for the new chain: SPEC_A's items with version 2, the fifteen CP v2 fields appended (the
# lp4 devnet defaults, as pocol-extension/tests/decoder_corpus.mjs uses for its devnet-v2 items) and the v3 program
# set's sysCodeHash (the only set the deploy tools accept on a spec-2 genesis); `flip` changes the last byte of allocRoot.
CP_V2_EXTRA = [31557600, 62500000000000000, 16, 648000, 10**15, 0, 0, 5000, 10000000, 4, 40, 21600, 1250, 3, 2500]
_alloc, _members, _sys = _rest[_cp_end:_cp_end + 33], _rest[_cp_end + 33:-33], _rest[-33:]
assert _alloc[0] == _sys[0] == 0xa0
V3_SYS = bytes.fromhex(ni.pocol_identity.V3_SYS_CODE_HASH)
def spec_v2(version=b'\x02', extra=CP_V2_EXTRA, flip=0, sys=V3_SYS, items=None):
    cp = rlp_list(_rest[_cp_start:_cp_end] + (items if items is not None else b''.join(int_item(v) for v in extra)))
    alloc = _alloc[:-1] + bytes([_alloc[-1] ^ flip])
    return rlp_list(version + bytes([0x80 + len(_chain)]) + _chain + cp + alloc + _members + b'\xa0' + sys)
SPEC_V3 = spec_v2()
HASH_V3 = '0x' + ni.keccak256(SPEC_V3).hex()

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
    'deploy/mainnet/collect_seeds.py': 'read-only evidence for the old network before cutover; its source pin refuses a switched preflight',
    'deploy/mainnet/test_new_network_config.py': 'an old devnet.json fixture',
    'deploy/faucet/test_pocol_faucet.py': 'the faucet refuses the old chain',
    'deploy/mainnet/h15000/network.pocol.json': "the old network's invitation (and its published GenesisSpec)",
    'deploy/mainnet/h15000/README.md': "the old network's H15000 upgrade",
    'deploy/mainnet/README.md': "the old network's operations",
    'deploy/mainnet/EARN-IN-CUTOVER-RUNBOOK.md': 'the move from the old network',
    'deploy/mainnet/EARN-IN-CUTOVER-RUNBOOK-AR.md': 'the move from the old network',
    # PR5: the old network as the legacy 0.4.0 build and the pinned golden values of the consensus identity.
    'deploy/public-testnet/pocol_identity.py': "the LEGACY_BUILDS mirror: the 0.4.0 build bound to the old chain",
    'deploy/public-testnet/test_pocol_identity.py': "the old network's golden consensus identity",
    'prototypes/pocold/src/identity.rs': "the old network's golden consensus identity (T11)",
    'prototypes/pocold/src/launcher.rs': 'LEGACY_BUILDS: the 0.4.0 build bound to the old chain, and its tests',
    'prototypes/pocold/tests/consensus_guard.rs': 'checks that LEGACY_BUILDS stays bound to the old chain',
    'prototypes/pocold/src/net.rs': 'a pinned canonical Hello frame (wire golden vectors), not a network setting',
    'prototypes/pocold/tests/data/wire-golden.json': 'the pinned canonical Hello frame (wire golden vectors)',
}
# The release notes every package ships. The step-3 PR replaces the old network's 0.4.0 notes with the new
# network's, which may name the old network as history but must name the tree's identity: the chain ID and
# genesis hash users check (after a redo, the new genesis).
NOTES = 'deploy/standalone/RELEASE-UPGRADE.md'
# Shipped with the packages, so the step-3 PR replaces or bumps them (runbook step 3): before the switch each
# holds its pattern (the old release's version, or text that is true only on the old network), on the
# switched tree none may.
STEP3 = {
    NOTES: re.compile(r'(?m)^# PoCol Mainnet · 0\.4\.0 update$|^## Windows: keep the same account and wallet home$'),
    'deploy/standalone/package.py': re.compile(r"version\s*=\s*'0\.4\.0'"),
    'deploy/standalone/README.md': re.compile(r'Wallet 0\.4\.0|retain their genesis'),
    'deploy/standalone/linux-desktop/pocol-desktop': re.compile(r'0\.4\.0'),
    'deploy/standalone/linux-desktop/install-desktop-icon.sh': re.compile(r'0\.4\.0'),
    'deploy/standalone/linux-desktop/README-LINUX-ICONS.md': re.compile(r'0\.4\.0'),
    'prototypes/pocol-windows/Wallet.cs': re.compile(r'Assembly(?:File)?Version\("0\.4\.0\.0"\)'),
    'prototypes/pocol-windows/Wallet.xaml': re.compile(r'PoCol Wallet · 0\.4\.0'),
    'prototypes/pocol-extension/package.json': re.compile(r'"version":\s*"0\.4\.1"'),
    'prototypes/pocol-extension/README-IDE.md': re.compile(
        r'(?m)^# PoCol Studio, IDE & Viewer 0\.4\.1|chain ID and genesis remain unchanged'),
    'prototypes/pocol-extension/README-STUDIO.md': re.compile(
        r'(?m)^# PoCol Studio extension — 0\.4\.1|its chain ID, genesis and wallet addresses are unchanged'),
    'prototypes/pocol-extension/README-AR.md': re.compile(r'الإصدار 0\.4\.[01]|ومعرّف الشبكة وكتلة التكوين كما هي'),
    'prototypes/pocol-studio/package.json': re.compile(r'"version":\s*"0\.1\.1"'),
    'prototypes/pocol-studio/dist/app.js': re.compile(r'PoCol-Studio-IDE-Viewer-0\.4\.[01]\.zip'),
    'prototypes/pocol-studio/QUICKSTART-AR.txt': re.compile(r'PoCol-Studio-IDE-Viewer-0\.4\.[01]\.zip'),
    'prototypes/pocol-android/app/build.gradle': re.compile(r"versionName '0\.4\.0'"),
    '.github/workflows/five-device-build.yml': re.compile(r'0\.4\.0'),
}
TRACKED = list(ni.FILES) + [page for page, _ in ni.VERSIONED] + [ni.INSTALLER]

def unaccounted(files):
    """Paths among (path, text) pairs that pin the old identity but are neither switched nor kept."""
    return [rel for rel, text in files
            if not rel.startswith(tuple(KEPT_PREFIXES)) and rel not in KEPT and rel != NOTES
            and (rel not in ni.FILES or SWITCHED) and OLD_ANYWHERE.search(text)]

def digests(files):
    """{name: SHA-256}, so a failing comparison reports quickly instead of diffing whole files."""
    return {name: hashlib.sha256(data).hexdigest() for name, data in files.items()}

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
        self.assertEqual(digests(payloads), digests({name: (root / rel).read_bytes() for name, rel in ni.INSTALLER_SOURCES.items()}))

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
        self.assertEqual(hashlib.sha256((ni.ROOT / ni.INSTALLER).read_bytes()).hexdigest(),
                         hashlib.sha256(build_installer.script(sources).encode('utf-8')).hexdigest(),
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
        self.assertEqual(list(root.rglob('*.network-identity.tmp')), [], 'every new file replaced its original')
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

    def test_an_io_error_while_writing_changes_nothing(self):
        root = self.copy()
        before = self.snapshot(root)
        # A file part way through the list cannot be written (a read-only directory, a full disk). It pins the
        # genesis hash, so every switch changes it, a redo for the same chain included.
        hashed = [rel for rel, (_, h, _) in ni.FILES.items() if h and rel != ni.ANCHOR]
        victim = Path(hashed[len(hashed) // 2]).name + '.network-identity.tmp'
        real = shutil.copymode
        def copymode(src, dst, *args, **kwargs):
            if Path(dst).name == victim:
                raise PermissionError(13, 'Permission denied', str(dst))
            return real(src, dst, *args, **kwargs)
        with mock.patch.object(ni.shutil, 'copymode', copymode):
            self.assertEqual(self.run_tool(root), 2)
        self.assertEqual(before, self.snapshot(root))
        self.assertEqual(list(root.rglob('*.network-identity.tmp')), [])

    def test_a_failed_replacement_is_reported_and_the_next_run_sees_it(self):
        root = self.copy()
        real, calls = os.replace, []
        def replace(src, dst):
            calls.append(dst)
            if len(calls) == 3:
                raise PermissionError(13, 'Permission denied', str(dst))
            return real(src, dst)
        with mock.patch.object(ni.os, 'replace', replace):
            self.assertEqual(self.run_tool(root), 2)
        self.assertEqual(list(root.rglob('*.network-identity.tmp')), [])
        self.assertEqual(digests({'anchor': (root / ni.ANCHOR).read_bytes()}), digests({'anchor': (ni.ROOT / ni.ANCHOR).read_bytes()}),
                         'the node file goes last')
        self.assertEqual(self.run_tool(root), 2, 'two files already carry the given identity')
        for rel in TRACKED:
            shutil.copy(ni.ROOT / rel, root / rel)
        self.assertEqual(self.run_tool(root), 0, 'restored, the switch runs')

    def test_a_hashed_source_with_crlf_line_endings_is_refused(self):
        # A checkout with core.autocrlf: the versions and payloads would be hashes of bytes git does not store.
        for rel in [rel for _, rel in ni.VERSIONED] + list(ni.INSTALLER_SOURCES.values()):
            with self.subTest(rel=rel):
                root = self.copy()
                crlf = (root / rel).read_bytes().replace(b'\n', b'\r\n')
                (root / rel).write_bytes(crlf)
                before = self.snapshot(root)
                self.assertEqual(self.run_tool(root), 2)
                self.assertEqual(before, self.snapshot(root))
                root = self.copy()
                self.assertEqual(self.run_tool(root), 0)
                (root / rel).write_bytes((root / rel).read_bytes().replace(b'\n', b'\r\n'))
                self.assertEqual(self.run_tool(root, '--check'), 2)

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
        # The hash with hex digits after or before it, and the chain ID inside longer words and numbers.
        longer = f'# 0x{CUR_GENESIS}ab ab{CUR_GENESIS} {CUR_CHAIN}0 x{CUR_CHAIN} {CUR_CHAIN}_1\n'
        (root / rel).write_text((root / rel).read_text(encoding='utf-8') + longer, encoding='utf-8')
        self.assertEqual(self.run_tool(root), 0)
        self.assertIn(longer, (root / rel).read_text(encoding='utf-8'))

    def test_an_old_mainnet_pin_left_after_a_switch_fails_check_and_redo(self):
        for legacy in ('LEGACY = 1599846715\n', f'LEGACY = "0x{ni.OLD_GENESIS}"\n'):
            with self.subTest(legacy=legacy):
                root = self.copy()
                self.assertEqual(self.run_tool(root), 0)
                rel = 'deploy/studio/pocol_studio_gateway.py'
                (root / rel).write_text((root / rel).read_text(encoding='utf-8') + legacy, encoding='utf-8')
                before = self.snapshot(root)
                self.assertEqual(self.run_tool(root, '--check'), 2)
                self.assertEqual(self.run_tool(root, genesis=HASH_B, spec='b.hex'), 2)
                self.assertEqual(before, self.snapshot(root))

    def test_upper_case_copies_and_inputs_are_switched(self):
        root = self.copy()
        # A copy of the hash and of the GenesisSpec in upper case still counts, and is switched.
        for rel, old in (('deploy/rewards/gateway.py', CUR_GENESIS),
                         ('prototypes/pocol-extension/src/lib/studio-network.js', CUR_SPEC.hex())):
            text = (root / rel).read_text(encoding='utf-8')
            self.assertIn(old, text)
            (root / rel).write_text(text.replace(old, old.upper()), encoding='utf-8')
        # The given hash and the spec file may be in upper case too; the files get lower case.
        (root / 'upper.hex').write_text('0X' + SPEC_A.hex().upper() + '\n', encoding='utf-8')
        self.assertEqual(self.run_tool(root, genesis='0x' + HASH_A[2:].upper(), spec='upper.hex'), 0)
        self.assert_carries(root, HASH_A, SPEC_A)
        rust = (root / ni.ANCHOR).read_text(encoding='utf-8')
        self.assertIn(f'const MAINNET_GENESIS: &str = "{HASH_A[2:]}";', rust)
        for rel in ni.FILES:
            text = (root / rel).read_text(encoding='utf-8').lower()
            self.assertNotIn(CUR_GENESIS, text, rel)
            self.assertNotIn(CUR_SPEC.hex(), text, rel)

    def test_a_copy_of_the_given_spec_outside_the_table_fails(self):
        root = self.copy()
        rel = 'deploy/rewards/gateway.py'
        (root / rel).write_text((root / rel).read_text(encoding='utf-8') + f'# {SPEC_A.hex()}\n', encoding='utf-8')
        before = self.snapshot(root)
        self.assertEqual(self.run_tool(root), 2, 'one GenesisSpec copy more than its table allows')
        self.assertEqual(before, self.snapshot(root))

    def test_a_tampered_old_invitation_is_refused(self):
        root = self.copy()
        tampered = root / 'old-invitation.json'
        invitation = json.loads(ni.OLD_INVITATION.read_text(encoding='utf-8'))
        spec = invitation['files']['genesis.spec.hex'].strip()
        invitation['files']['genesis.spec.hex'] = spec[:-2] + ('00' if spec[-2:] != '00' else '01')
        tampered.write_text(json.dumps(invitation), encoding='utf-8')
        before = self.snapshot(root)
        with mock.patch.object(ni, 'OLD_INVITATION', tampered), self.assertRaises(SystemExit):
            self.run_tool(root)
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
        # Specs that are valid RLP but carry chain IDs that are never a target: the old network's (with
        # another genesis) and 2**53, above the JavaScript safe-integer range.
        for name, chain in (('old-chain.hex', ni.OLD_CHAIN_ID), ('too-large.hex', 2**53)):
            raw = chain.to_bytes((chain.bit_length() + 7) // 8, 'big')
            spec = bytearray(rlp_list(_version + bytes([0x80 + len(raw)]) + raw + _rest))
            spec[-1] ^= 4
            (root / name).write_text(bytes(spec).hex(), encoding='utf-8')
            with self.subTest(chain=chain), self.assertRaises(SystemExit):
                self.run_tool(root, '--chain-id', str(chain), genesis='0x' + ni.keccak256(bytes(spec)).hex(), spec=name)
        self.assertEqual(before, self.snapshot(root))

    def test_a_spec_2_genesis_is_switched_to_only_with_v3(self):
        self.assertEqual(ni.spec_identity(SPEC_A), (1, ni.NEW_CHAIN_ID))
        self.assertEqual(ni.spec_identity(CUR_SPEC), (1, CUR_CHAIN))
        self.assertEqual(ni.spec_identity(SPEC_V3, (2,)), (2, ni.NEW_CHAIN_ID))
        with self.assertRaisesRegex(ValueError, 'v3 genesis'):
            ni.spec_chain_id(SPEC_V3)
        # The release version and the v3 version each with the other's CP length, a version without a schema, and
        # a CP v2 one item short or long, are refused whichever versions are allowed.
        for bad in (spec_v2(version=b'\x01'), NON_CANONICAL['version 2'], spec_v2(version=b'\x03'),
                    spec_v2(extra=CP_V2_EXTRA[:-1]), spec_v2(extra=CP_V2_EXTRA + [1]), rlp_list(SPEC_V3[ni.rlp_item(SPEC_V3, 0)[1]:] + b'\x01')):
            for versions in ((1,), (2,), (1, 2)):
                with self.subTest(spec=bad.hex()[:16], versions=versions), self.assertRaises(ValueError):
                    ni.spec_identity(bad, versions)
        # A spec-2 genesis must pass the whole v3-preview decoder and name the v3 program set, with or without the
        # release version allowed beside it.
        extra = b''.join(int_item(v) for v in CP_V2_EXTRA[:-1])
        for bad, reason in ((spec_v2(sys=_sys[1:]), 'v3 program set'), (spec_v2(sys=b'\x00' * 32), 'v3 program set'),
                            (spec_v2(extra=CP_V2_EXTRA[:-1] + [10001]), 'gsRange: rho_bp'),
                            (spec_v2(extra=[0] + CP_V2_EXTRA[1:]), 'gsRange: H_half'),
                            (spec_v2(items=extra + b'\x82\x00\x05'), 'gsInt: rho_bp'),
                            (spec_v2(extra=CP_V2_EXTRA[:7] + [10001] + CP_V2_EXTRA[8:]), 'gsRange: gamma_tip_bp')):
            for versions in ((2,), (1, 2)):
                with self.subTest(reason=reason, versions=versions), self.assertRaisesRegex(ValueError, reason):
                    ni.spec_identity(bad, versions)
        # The reference decoder accepts SPEC_V3 at the preview ceiling only, as the Rust and JS decoders do.
        self.assertEqual(ni.pocol_identity.decode_genesis_spec(SPEC_V3, 2)['specVersion'], 2)
        with self.assertRaisesRegex(ValueError, 'gsVersion:2'):
            ni.pocol_identity.decode_genesis_spec(SPEC_V3)
        root = self.copy()
        (root / 'v3bad.hex').write_text('0x' + spec_v2(sys=b'\x00' * 32).hex() + '\n', encoding='utf-8')
        before = self.snapshot(root)
        with self.assertRaises(SystemExit):
            self.run_tool(root, '--v3', genesis='0x' + ni.keccak256(spec_v2(sys=b'\x00' * 32)).hex(), spec='v3bad.hex')
        self.assertEqual(before, self.snapshot(root), 'a malformed v3 genesis writes nothing')
        (root / 'v3bad.hex').unlink()
        (root / 'v3.hex').write_text('0x' + SPEC_V3.hex() + '\n', encoding='utf-8')
        (root / 'v3b.hex').write_text('0x' + spec_v2(flip=1).hex() + '\n', encoding='utf-8')
        before = self.snapshot(root)
        for args, case in (((), dict(genesis=HASH_V3, spec='v3.hex')), (('--check',), dict(genesis=HASH_V3, spec='v3.hex')),
                           (('--v3',), dict()), (('--v3', '--check'), dict())):
            with self.subTest(args=args), self.assertRaises(SystemExit):
                self.run_tool(root, *args, **case)
        self.assertEqual(before, self.snapshot(root), 'a refused v3 switch writes nothing')
        self.assertEqual(self.run_tool(root, '--v3', genesis=HASH_V3, spec='v3.hex'), 0)
        self.assert_carries(root, HASH_V3, SPEC_V3)
        self.assertEqual(self.run_tool(root, '--v3', '--check', genesis=HASH_V3, spec='v3.hex'), 0)
        # The tree now pins a spec-2 genesis: without --v3 its identity is not proven, so nothing runs.
        switched = self.snapshot(root)
        with self.assertRaises(SystemExit):
            self.run_tool(root, '--check', genesis=HASH_V3, spec='v3.hex')
        with self.assertRaises(SystemExit):
            self.run_tool(root)
        self.assertEqual(switched, self.snapshot(root))
        # A redo to another v3 genesis of the same chain switches from the current one.
        hash_v3b = '0x' + ni.keccak256(spec_v2(flip=1)).hex()
        self.assertEqual(self.run_tool(root, '--v3', genesis=hash_v3b, spec='v3b.hex'), 0)
        self.assert_carries(root, hash_v3b, spec_v2(flip=1))

    def test_the_release_notes_name_the_current_identity(self):
        # True of the old notes today; on the switched tree the new notes must name the new network, and after a
        # redo with a later genesis, that genesis.
        notes = (ni.ROOT / NOTES).read_text(encoding='utf-8')
        self.assertTrue(str(CUR_CHAIN) in notes, f'{NOTES}: the notes name the chain ID {CUR_CHAIN} (runbook step 3)')
        self.assertTrue('0x' + CUR_GENESIS in notes.lower(),
                        f'{NOTES}: the notes name the genesis hash 0x{CUR_GENESIS}; after a redo, the new one (runbook step 3)')
        if SWITCHED:
            # The way across for an existing wallet: the key file stays, the old network's files move aside.
            for marker in ('identity.pocol-key', 'network.pocol.json'):
                self.assertTrue(marker in notes, f'{NOTES}: the notes give existing wallets their way across (runbook step 3)')

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
        held = [rel for rel, old in STEP3.items() if old.search((ni.ROOT / rel).read_text(encoding='utf-8'))]
        if SWITCHED:
            self.assertEqual(held, [], 'the step-3 PR replaces or bumps these (runbook step 3): ' + ', '.join(held))
        else:
            self.assertEqual(held, list(STEP3), 'update STEP3 to what the files hold today')

if __name__ == '__main__':
    unittest.main()
