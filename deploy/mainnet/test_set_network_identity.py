#!/usr/bin/env python3
"""set_network_identity.py switches every listed pin (chain ID, genesis hash, GenesisSpec) from the --from network's
identity to the --to network's, with the content versions and the Studio installer derived from them; --check fails on
any other identity; a redo switches from the current one; line endings are kept, and hashed sources with converted
(CRLF) line endings are refused; nothing is written when anything fails, an I/O error included; bad, non-canonical or
contradictory invitations are refused, as is a --to genesis the tree's release node does not decode; a switch records
the --from network in the retired list and a retired --to needs --allow-revert; every tracked file that pins the
tree's current identity is listed in FILES or kept on purpose, and none (FILES included) pins a retired identity the
tree no longer pins; and the shipped release texts that are true only on the retired network are gone once the tree
pins a network that is not retired.

The tests name no network: the --from fixture is the tree's current identity (the GenesisSpec the node file pins)
and the --to fixtures are derived from it, so they pass whatever network the tree pins."""
import base64, hashlib, json, os, re, shutil, subprocess, tempfile, unittest
from pathlib import Path
from unittest import mock
import set_network_identity as ni
import build_installer

CUR_CHAIN, CUR_GENESIS, CUR_SPEC = ni.current_identity(ni.ROOT)
CUR_VERSION = ni.spec_identity(CUR_SPEC)[0]
RELEASE = ni.node_spec_version(ni.ROOT)
def _free_chain():
    """The chain ID of the --to fixtures: a valid chain ID that no listed file mentions in any spelling."""
    texts = [(ni.ROOT / rel).read_text(encoding='utf-8').lower() for rel in ni.FILES]
    for chain in range(990_001, 999_999, 7):
        if chain != CUR_CHAIN and not any(t in text for text in texts for t in (str(chain), f'{chain:_}', f'{chain:x}')):
            return chain
    raise AssertionError('no free fixture chain ID')
NEW_CHAIN = _free_chain()
def anywhere(identities):
    """An independent detector of identities: each chain ID with or without digit groups, its hex digits, and the
    start of each genesis hash, in any case; None for no identity."""
    alts = [a for chain, genesis in identities for a in (re.escape(f'{chain}'), re.escape(f'{chain:_}'), f'{chain:x}', genesis[:8])]
    return re.compile('(?i)' + '|'.join(alts)) if alts else None
CUR_ANYWHERE = anywhere([(CUR_CHAIN, CUR_GENESIS)])
# The retired identities (read independently of the tool), and those the tree no longer pins: no tracked file outside
# the kept ones may hold one of those, FILES included, so no copy of an earlier network outlives its switch.
RETIRED = [(e['chainId'], e['genesisHash'][2:]) for e in json.loads((ni.ROOT / ni.RETIRED).read_text(encoding='utf-8'))['retired']]
GONE_ANYWHERE = anywhere([r for r in RETIRED if r != (CUR_CHAIN, CUR_GENESIS)])
# The tree pins a network that is not retired: the identity switch has happened.
SWITCHED = (CUR_CHAIN, CUR_GENESIS) not in RETIRED

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

def int_item(v):
    """A canonical RLP integer item."""
    raw = v.to_bytes((v.bit_length() + 7) // 8, 'big')
    return raw if len(raw) == 1 and raw[0] < 0x80 else bytes([0x80 + len(raw)]) + raw

def parts(spec):
    """(version item, chainId payload, the items after chainId) of a GenesisSpec."""
    _, start, end = ni.rlp_item(spec, 0)
    _, _, version_end = ni.rlp_item(spec, start)
    _, s, e = ni.rlp_item(spec, version_end)
    return spec[start:version_end], spec[s:e], spec[e:end]

_version, _cur_chain, _rest = parts(CUR_SPEC)
_cp_is_list, _cp_start, _cp_end = ni.rlp_item(_rest, 0)   # CP, the list after chainId
assert _cp_is_list
# The CP items of version 1: the current CP's first 52 (the v2 schema is the v1 schema plus fifteen fields).
_cp_v1 = b''
_j = _cp_start
for _ in range(ni.CP_ITEMS[1]):
    _e = ni.rlp_item(_rest, _j)[2]; _cp_v1 += _rest[_j:_e]; _j = _e
_tail = _rest[_cp_end:]
# The fifteen CP v2 fields (the lp4 devnet defaults, as pocol-extension/tests/decoder_corpus.mjs uses).
CP_V2_EXTRA = [31557600, 62500000000000000, 16, 648000, 10**15, 0, 0, 5000, 10000000, 4, 40, 21600, 1250, 3, 2500]

def make_spec(chain=NEW_CHAIN, version=1, flip=0, extra=None):
    """A GenesisSpec of `version` for `chain` built from the current one's items, its last byte changed by `flip`."""
    cp = _cp_v1 + (b''.join(int_item(v) for v in (CP_V2_EXTRA if extra is None else extra)) if version == 2 else b'')
    spec = bytearray(rlp_list(int_item(version) + int_item(chain) + rlp_list(cp) + _tail))
    spec[-1] ^= flip
    return bytes(spec)

SPEC_A, SPEC_B = make_spec(flip=1), make_spec(flip=2)
HASH_A, HASH_B = ('0x' + ni.keccak256(s).hex() for s in (SPEC_A, SPEC_B))
SPEC_V3 = make_spec(version=2, flip=1)
HASH_V3 = '0x' + ni.keccak256(SPEC_V3).hex()
_payload = SPEC_A[ni.rlp_item(SPEC_A, 0)[1]:]
_v, _chain, _r = parts(SPEC_A)
_c_is_list, _c_start, _c_end = ni.rlp_item(_r, 0)
NON_CANONICAL = {
    'leading zero in chainId': rlp_list(_v + bytes([0x80 + len(_chain) + 1, 0]) + _chain + _r),
    'long form chainId': rlp_list(_v + bytes([0xb8, len(_chain)]) + _chain + _r),
    'version as a list': rlp_list(b'\xc0' + bytes([0x80 + len(_chain)]) + _chain + _r),
    'trailing bytes': SPEC_A + b'\x00',
    'version 2 with the version-1 CP': rlp_list(b'\x02' + bytes([0x80 + len(_chain)]) + _chain + _r),
    'version wrapped as 81 01': rlp_list(b'\x81\x01' + bytes([0x80 + len(_chain)]) + _chain + _r),
    'outer length with a leading zero': bytes([0xfa]) + len(_payload).to_bytes(3, 'big') + _payload,
    'a wrapped single byte after the last item': rlp_list(_payload + b'\x81\x05'),
    'a wrapped single byte inside CP': rlp_list(_v + bytes([0x80 + len(_chain)]) + _chain
                                                + rlp_list(_r[_c_start:_c_end] + b'\x81\x05') + _r[_c_end:]),
    'a 55-byte item in long form': rlp_list(_payload + b'\xb8\x37' + b'\x01' * 55),
    'an item that overruns its list': rlp_list(_payload + b'\xc2\x01\x82\x03\x04'),
}

def invitation(spec, info='derived'):
    """A pocol-network-v2 invitation of `spec`; devnet.json carries the identity the spec proves unless given."""
    if info == 'derived':
        _, chain = ni.spec_identity(spec)
        info = {'chainId': chain, 'genesisHash': '0x' + ni.keccak256(spec).hex(), 'forks': [[1, 0]]}
    files = {'genesis.spec.hex': '0x' + spec.hex(), 'alloc.json': '{}'}
    if info is not None:
        files['devnet.json'] = json.dumps(info)
    return json.dumps({'schema': 'pocol-network-v2', 'consensusRulesVersion': 1, 'seeds': ['127.0.0.1:30300'], 'files': files})

def held(text, chain_id, genesis, spec_hex):
    """(chain ID tokens, hash copies, GenesisSpec copies), counted independently of the tool."""
    spellings = '|'.join(re.escape(s) for s in (str(chain_id), f'{chain_id:_}', hex(chain_id)))
    chain = len(re.findall(rf'(?ai)(?<!\w)(?:{spellings})n?(?!\w)', text))
    run = lambda h: len(re.findall(rf'(?i)(?<![0-9a-f]){h}(?![0-9a-f])', text))
    return chain, run(genesis), run(spec_hex)

# Files that pin the tree's current identity on purpose and are not switched. Every other tracked file that pins it
# must be in ni.FILES, so no copy can be missed at a switch.
KEPT_PREFIXES = {
    'development/': "the project's history",
    'deploy/websites/': 'website snapshots: build output that is updated or rebuilt with the new network',
}
KEPT = {
    ni.RETIRED: 'the retired identities the switch tool and this guard look for',
    'prototypes/pocold/src/net.rs': 'a pinned canonical Hello frame (wire golden vectors), not a network setting',
    'prototypes/pocold/tests/data/wire-golden.json': 'the pinned canonical Hello frame (wire golden vectors)',
}
# Shipped with the packages, so the identity-switch release replaces or bumps them (Phase 7 bumps the versions):
# while the tree pins a retired network each holds its pattern (the old release's version, or text that is true only
# on the retired network); once it pins a network that is not retired none may. A version bump before the switch
# removes its entry here.
STEP3 = {
    'deploy/standalone/package.py': re.compile(r"version\s*=\s*'0\.4\.0'"),
    'deploy/standalone/README.md': re.compile(r'Wallet 0\.4\.0|retain their genesis'),
    'deploy/standalone/linux-desktop/pocol-desktop': re.compile(r'0\.4\.0'),
    'deploy/standalone/linux-desktop/install-desktop-icon.sh': re.compile(r'0\.4\.0'),
    'prototypes/pocol-windows/Wallet.cs': re.compile(r'Assembly(?:File)?Version\("0\.4\.0\.0"\)'),
    'prototypes/pocol-windows/Wallet.xaml': re.compile(r'PoCol Wallet · 0\.4\.0'),
    'prototypes/pocol-extension/package.json': re.compile(r'"version":\s*"0\.4\.1"'),
    'prototypes/pocol-extension/README-IDE.md': re.compile(
        r'(?m)^# PoCol Studio, IDE & Viewer 0\.4\.1|chain ID and genesis remain unchanged'),
    'prototypes/pocol-extension/README-STUDIO.md': re.compile(
        r'(?m)^# PoCol Studio extension — 0\.4\.1|its chain ID and genesis are unchanged'),
    'prototypes/pocol-extension/README-AR.md': re.compile(r'الإصدار 0\.4\.[01]|ومعرّف الشبكة وكتلة التكوين كما هي'),
    'prototypes/pocol-studio/package.json': re.compile(r'"version":\s*"0\.1\.1"'),
    'prototypes/pocol-studio/dist/app.js': re.compile(r'PoCol-Studio-IDE-Viewer-0\.4\.[01]\.zip'),
    'prototypes/pocol-studio/QUICKSTART-AR.txt': re.compile(r'PoCol-Studio-IDE-Viewer-0\.4\.[01]\.zip'),
    'prototypes/pocol-android/app/build.gradle': re.compile(r"versionName '0\.4\.0'"),
    '.github/workflows/five-device-build.yml': re.compile(r'0\.4\.0'),
}
TRACKED = list(ni.FILES) + [page for page, _ in ni.VERSIONED] + [ni.INSTALLER, ni.NODE_SPEC, ni.RETIRED]

def unaccounted(files, current=CUR_ANYWHERE, gone=GONE_ANYWHERE):
    """Paths among (path, text) pairs outside the kept ones that pin the current identity without being listed, or
    pin a retired identity the tree no longer pins (listed or not)."""
    return [rel for rel, text in files
            if not rel.startswith(tuple(KEPT_PREFIXES)) and rel not in KEPT
            and ((rel not in ni.FILES and current.search(text)) or (gone is not None and gone.search(text)))]

def digests(files):
    """{name: SHA-256}, so a failing comparison reports quickly instead of diffing whole files."""
    return {name: hashlib.sha256(data).hexdigest() for name, data in files.items()}

def installer_payloads(text):
    """{name: bytes} embedded in the Studio gateway installer."""
    return {name: base64.b64decode(payload) for name, payload in
            re.findall(r"'([a-z_]+\.py)': \{'sha256': '[0-9a-f]{64}', 'base64': '([A-Za-z0-9+/=]+)'\}", text)}

class SetNetworkIdentity(unittest.TestCase):
    def copy(self):
        tmp = Path(tempfile.mkdtemp(prefix='pocol-identity-'))
        self.addCleanup(shutil.rmtree, tmp)
        for rel in TRACKED:
            (tmp / rel).parent.mkdir(parents=True, exist_ok=True)
            shutil.copy(ni.ROOT / rel, tmp / rel)
        for name, spec in (('cur.json', CUR_SPEC), ('a.json', SPEC_A), ('b.json', SPEC_B), ('v3.json', SPEC_V3)):
            (tmp / name).write_text(invitation(spec), encoding='utf-8')
        for i, spec in enumerate(NON_CANONICAL.values()):
            (tmp / f'bad{i}.json').write_text(invitation(spec, None), encoding='utf-8')
        return tmp

    def run_tool(self, root, *extra, source='cur.json', target='a.json'):
        return ni.main(['--from', str(root / source), '--to', str(root / target), '--root', str(root), *extra])

    def snapshot(self, root):
        return {rel: hashlib.sha256((root / rel).read_bytes()).hexdigest() for rel in TRACKED}

    def assert_carries(self, root, genesis, spec, chain=NEW_CHAIN):
        for rel, expected in ni.FILES.items():
            self.assertEqual(held((root / rel).read_text(encoding='utf-8'), chain, genesis[2:], spec.hex()), expected, rel)
        for page, rel in ni.VERSIONED:
            self.assertEqual(versions((root / page).read_text(encoding='utf-8'), Path(rel).name), {sha12((root / rel).read_bytes())}, page)
        payloads = installer_payloads((root / ni.INSTALLER).read_text(encoding='utf-8'))
        self.assertEqual(digests(payloads), digests({name: (root / rel).read_bytes() for name, rel in ni.INSTALLER_SOURCES.items()}))

    def test_the_tool_names_no_network(self):
        source = (Path(ni.__file__)).read_text(encoding='utf-8')
        self.assertNotRegex(source, r'(?m)^(OLD|NEW)_[A-Z_]+\s*=', 'no built-in old or new identity')
        self.assertIsNone(CUR_ANYWHERE.search(source), 'the tool does not pin the current identity')
        self.assertIsNone(re.search(r'deploy/mainnet/[A-Za-z0-9_-]+/|\b[0-9]{9,}\b', source), 'no network directory or chain ID')

    def test_the_current_identity_is_proven_and_the_fixtures_are_valid(self):
        self.assertEqual(ni.keccak256(CUR_SPEC).hex(), CUR_GENESIS)
        self.assertEqual(ni.spec_identity(CUR_SPEC)[1], CUR_CHAIN)
        self.assertIn(RELEASE, ni.CP_ITEMS)
        for spec in (SPEC_A, SPEC_B):
            self.assertEqual(ni.spec_identity(spec), (1, NEW_CHAIN))
            self.assertNotEqual(spec, CUR_SPEC)
        self.assertEqual(ni.spec_identity(SPEC_V3, (2,)), (2, NEW_CHAIN))
        root = self.copy()
        self.assertEqual(ni.invitation_identity(root / 'cur.json')[:3], (CUR_CHAIN, CUR_GENESIS, CUR_SPEC))

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
            self.assertEqual(held(text, CUR_CHAIN, CUR_GENESIS, CUR_SPEC.hex()), (0, 0, 0), rel)
        rust = (root / ni.ANCHOR).read_text()
        self.assertIn(f'const MAINNET_CHAIN_ID: u64 = {NEW_CHAIN:_};', rust)
        self.assertIn(f'const MAINNET_GENESIS: &str = "{HASH_A[2:]}";', rust)
        self.assertIn(f'let genesis = "0x{SPEC_A.hex()}";', rust)
        network = (root / 'prototypes/pocol-extension/src/lib/studio-network.js').read_text()
        for line in (f'"chainId": {NEW_CHAIN}', f'"genesisHash": "{HASH_A}"', f'"genesisPre": "0x{SPEC_A.hex()}"'):
            self.assertIn(line, network)
        self.assertIn(f'<span class="network-id">{NEW_CHAIN}</span>', (root / 'prototypes/pocol-extension/src/contract.html').read_text())
        self.assertIn(f'>{NEW_CHAIN}</span>', (root / 'prototypes/pocol-extension/src/publisher.html').read_text())
        self.assertIn(f"chain:'{hex(NEW_CHAIN)}'", (root / 'prototypes/pocol-extension/tests/contracts_test.mjs').read_text())
        self.assertIn(f'assert.equal(spec.chainId,{NEW_CHAIN}n)', (root / 'prototypes/pocol-extension/tests/studio_test.mjs').read_text())
        self.assertEqual(self.run_tool(root, '--check'), 0)
        self.assertEqual(list(root.rglob('*.network-identity.tmp')), [], 'every new file replaced its original')
        before = self.snapshot(root)
        self.assertEqual(self.run_tool(root), 0, 'the tree already pins --to: the run finishes the derived files')
        self.assertEqual(before, self.snapshot(root), 'a second run changes nothing')

    def test_check_refuses_another_identity_and_a_redo_switches_to_it(self):
        root = self.copy()
        self.assertEqual(self.run_tool(root), 0)
        self.assertEqual(self.run_tool(root, '--check', source='a.json', target='b.json'), 2, 'pinned to A, not B')
        with self.assertRaises(SystemExit):
            self.run_tool(root, target='b.json')
        self.assertEqual(self.run_tool(root, source='a.json', target='b.json'), 0, 'a redo for the same chain')
        self.assert_carries(root, HASH_B, SPEC_B)
        self.assertEqual(self.run_tool(root, '--check', source='a.json', target='b.json'), 0)
        with self.assertRaises(SystemExit, msg='pinned to B; A, which B replaced, is retired'):
            self.run_tool(root, '--check', source='b.json', target='a.json')
        # And back, on purpose only: the original network is retired (listed before, or by the first switch).
        before = self.snapshot(root)
        with self.assertRaises(SystemExit):
            self.run_tool(root, source='b.json', target='cur.json')
        self.assertEqual(before, self.snapshot(root), 'a refused revert writes nothing')
        self.assertEqual(self.run_tool(root, '--allow-revert', source='b.json', target='cur.json'), 0)
        after, original = self.snapshot(root), self.snapshot(ni.ROOT)
        self.assertEqual({k: v for k, v in after.items() if k != ni.RETIRED}, {k: v for k, v in original.items() if k != ni.RETIRED},
                         'switching back restores the original bytes of every pin')
        expected = RETIRED + [r for r in [(CUR_CHAIN, CUR_GENESIS)] if r not in RETIRED] + [(NEW_CHAIN, HASH_A[2:]), (NEW_CHAIN, HASH_B[2:])]
        self.assertEqual(ni.read_retired((root / ni.RETIRED).read_bytes()), expected, 'the list is append-only')

    def test_a_switch_retires_the_from_network_and_a_retired_target_is_refused(self):
        root = self.copy()
        original = (ni.ROOT / ni.RETIRED).read_bytes()
        self.assertEqual(self.run_tool(root), 0)
        listed = ni.read_retired((root / ni.RETIRED).read_bytes())
        self.assertEqual(listed[:len(RETIRED)], RETIRED, 'earlier entries are kept')
        self.assertIn((CUR_CHAIN, CUR_GENESIS), listed)
        self.assertEqual(len(listed), len(set(listed)), 'an identity is listed once')
        self.assertEqual(json.loads((root / ni.RETIRED).read_text())['about'], json.loads(original)['about'])
        before = self.snapshot(root)
        self.assertEqual(self.run_tool(root), 0, 'finishing a switch retires nothing more')
        self.assertEqual(before, self.snapshot(root))
        # A network listed as retired is never a target by accident, a redo (A to B to A) included.
        self.assertEqual(self.run_tool(root, source='a.json', target='b.json'), 0)
        before = self.snapshot(root)
        with self.assertRaises(SystemExit):
            self.run_tool(root, source='b.json', target='a.json')
        with self.assertRaises(SystemExit):
            self.run_tool(root, '--check', source='b.json', target='a.json')
        self.assertEqual(before, self.snapshot(root))

    def test_a_missing_or_malformed_retired_list_is_refused(self):
        for damage in (None, b'{"retired": [{"chainId": "1", "genesisHash": "0x00"}]}', b'[]', b'not json'):
            root = self.copy()
            if damage is None:
                (root / ni.RETIRED).unlink()
            else:
                (root / ni.RETIRED).write_bytes(damage)
            before = self.snapshot(root) if damage is not None else None
            with self.assertRaises(SystemExit):
                self.run_tool(root)
            if before is not None:
                self.assertEqual(before, self.snapshot(root))

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
        for extra in (f'# {HASH_A}\n', f'# {SPEC_A.hex()}\n'):
            with self.subTest(extra=extra[:20]):
                root = self.copy()
                rel = 'deploy/rewards/gateway.py'
                # It still holds its table's copies of the current identity, but after the switch it would hold one
                # copy of the given genesis hash (or GenesisSpec) more than its table allows.
                (root / rel).write_text((root / rel).read_text(encoding='utf-8') + extra, encoding='utf-8')
                before = self.snapshot(root)
                self.assertEqual(self.run_tool(root), 2)
                self.assertEqual(before, self.snapshot(root))

    def test_a_node_file_whose_spec_does_not_prove_its_identity_is_refused(self):
        root = self.copy()
        anchor = root / ni.ANCHOR
        text = anchor.read_text(encoding='utf-8')
        for old, new in ((f'const MAINNET_GENESIS: &str = "{CUR_GENESIS}";', f'const MAINNET_GENESIS: &str = "{HASH_B[2:]}";'),
                         (f'const MAINNET_CHAIN_ID: u64 = {CUR_CHAIN:_};', f'const MAINNET_CHAIN_ID: u64 = {CUR_CHAIN + 1:_};')):
            with self.subTest(old=old):
                self.assertIn(old, text)
                anchor.write_text(text.replace(old, new), encoding='utf-8')
                before = self.snapshot(root)
                with self.assertRaises(SystemExit):
                    self.run_tool(root)
                self.assertEqual(before, self.snapshot(root))
        anchor.write_text(text, encoding='utf-8')

    def test_a_tree_pinned_to_neither_network_is_refused(self):
        root = self.copy()
        before = self.snapshot(root)
        for source in ('a.json', 'b.json'):
            with self.subTest(source=source), self.assertRaises(SystemExit):
                self.run_tool(root, source=source, target='b.json' if source == 'a.json' else 'a.json')
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

    def test_a_from_pin_left_after_a_switch_fails_check_and_redo(self):
        for legacy in (f'LEGACY = {CUR_CHAIN}\n', f'LEGACY = "0x{CUR_GENESIS}"\n'):
            with self.subTest(legacy=legacy):
                root = self.copy()
                self.assertEqual(self.run_tool(root), 0)
                rel = 'deploy/studio/pocol_studio_gateway.py'
                (root / rel).write_text((root / rel).read_text(encoding='utf-8') + legacy, encoding='utf-8')
                before = self.snapshot(root)
                self.assertEqual(self.run_tool(root, '--check'), 2, 'the --from identity is left behind')
                self.assertEqual(before, self.snapshot(root))

    def test_upper_case_copies_and_inputs_are_switched(self):
        root = self.copy()
        # A copy of the hash and of the GenesisSpec in upper case still counts, and is switched.
        for rel, old in (('deploy/rewards/gateway.py', CUR_GENESIS),
                         ('prototypes/pocol-extension/src/lib/studio-network.js', CUR_SPEC.hex())):
            text = (root / rel).read_text(encoding='utf-8')
            self.assertIn(old, text)
            (root / rel).write_text(text.replace(old, old.upper()), encoding='utf-8')
        # The invitation's GenesisSpec and devnet.json hash may be in upper case too; the files get lower case.
        doc = json.loads(invitation(SPEC_A))
        doc['files']['genesis.spec.hex'] = '0X' + SPEC_A.hex().upper()
        info = json.loads(doc['files']['devnet.json']); info['genesisHash'] = '0x' + HASH_A[2:].upper()
        doc['files']['devnet.json'] = json.dumps(info)
        (root / 'upper.json').write_text(json.dumps(doc), encoding='utf-8')
        self.assertEqual(self.run_tool(root, target='upper.json'), 0)
        self.assert_carries(root, HASH_A, SPEC_A)
        rust = (root / ni.ANCHOR).read_text(encoding='utf-8')
        self.assertIn(f'const MAINNET_GENESIS: &str = "{HASH_A[2:]}";', rust)
        for rel in ni.FILES:
            text = (root / rel).read_text(encoding='utf-8').lower()
            self.assertNotIn(CUR_GENESIS, text, rel)
            self.assertNotIn(CUR_SPEC.hex(), text, rel)

    def test_a_contradictory_or_tampered_invitation_is_refused(self):
        root = self.copy()
        before = self.snapshot(root)
        tampered = bytearray(CUR_SPEC); tampered[-1] ^= 1
        bad = {
            # devnet.json names the --from network, but its GenesisSpec is another one.
            'tampered.json': invitation(bytes(tampered), {'chainId': CUR_CHAIN, 'genesisHash': '0x' + CUR_GENESIS}),
            'wrong-chain.json': invitation(SPEC_A, {'chainId': NEW_CHAIN + 1, 'genesisHash': HASH_A}),
            'wrong-hash.json': invitation(SPEC_A, {'chainId': NEW_CHAIN, 'genesisHash': HASH_B}),
            'devnet-not-json.json': json.dumps({'files': {'genesis.spec.hex': SPEC_A.hex(), 'devnet.json': '{'}}),
            'no-spec.json': json.dumps({'schema': 'pocol-network-v2', 'files': {}}),
            'not-json.json': '{"files":',
            'not-hex.json': json.dumps({'files': {'genesis.spec.hex': '0xzz'}}),
            'truncated.json': invitation(SPEC_A[:-1], None),
            'too-large.json': json.dumps({'files': {'genesis.spec.hex': SPEC_A.hex()}, 'pad': 'x' * ni.MAX_INVITATION}),
        }
        for name, text in bad.items():
            (root / name).write_text(text, encoding='utf-8')
        for name in bad:
            for source, target in ((name, 'a.json'), ('cur.json', name)):
                with self.subTest(source=source, target=target), self.assertRaises(SystemExit):
                    self.run_tool(root, source=source, target=target)
        for name in [f'bad{i}.json' for i in range(len(NON_CANONICAL))] + ['missing.json']:
            with self.subTest(target=name), self.assertRaises(SystemExit):
                self.run_tool(root, target=name)
        # The same network on both sides, and target chain IDs that are never a target.
        with self.assertRaises(SystemExit):
            self.run_tool(root, target='cur.json')
        for chain in (777001, 2**53):
            spec = make_spec(chain=chain, flip=4)
            (root / f'chain-{chain}.json').write_text(invitation(spec), encoding='utf-8')
            with self.subTest(chain=chain), self.assertRaises(SystemExit):
                self.run_tool(root, target=f'chain-{chain}.json')
        self.assertEqual(before, self.snapshot(root))

    def test_a_target_genesis_the_release_node_does_not_decode_is_refused(self):
        self.assertEqual(ni.spec_identity(SPEC_A), (1, NEW_CHAIN))
        self.assertEqual(ni.spec_identity(SPEC_V3), (2, NEW_CHAIN))
        with self.assertRaisesRegex(ValueError, 'version is not 1'):
            ni.spec_identity(SPEC_V3, (1,))
        # Each version with the other's CP length, a version without a schema, and a CP v2 one item short or long,
        # are refused whichever versions are allowed.
        for bad in (NON_CANONICAL['version 2 with the version-1 CP'], make_spec(version=3), make_spec(version=2, extra=CP_V2_EXTRA[:-1]),
                    make_spec(version=2, extra=CP_V2_EXTRA + [1]), rlp_list(SPEC_V3[ni.rlp_item(SPEC_V3, 0)[1]:] + b'\x01')):
            for allowed in ((1,), (2,), (1, 2)):
                with self.subTest(spec=bad.hex()[:16], allowed=allowed), self.assertRaises(ValueError):
                    ni.spec_identity(bad, allowed)
        root = self.copy()
        node = root / ni.NODE_SPEC
        text = node.read_text(encoding='utf-8')
        node.write_text(re.sub(r'pub const SPEC_VERSION: u8 = [0-9]+;', 'pub const SPEC_VERSION: u8 = 1;', text), encoding='utf-8')
        before = self.snapshot(root)
        for args in ((), ('--check',)):
            with self.subTest(args=args), self.assertRaises(SystemExit):
                self.run_tool(root, *args, target='v3.json')
        self.assertEqual(before, self.snapshot(root), 'a refused switch writes nothing')
        # A tree whose release node decodes spec 2 switches to it, and a spec-2 tree is switched onward.
        node.write_text(re.sub(r'pub const SPEC_VERSION: u8 = [0-9]+;', 'pub const SPEC_VERSION: u8 = 2;', text), encoding='utf-8')
        self.assertEqual(self.run_tool(root, target='v3.json'), 0)
        self.assert_carries(root, HASH_V3, SPEC_V3)
        self.assertEqual(self.run_tool(root, '--check', target='v3.json'), 0)
        (root / 'v3b.json').write_text(invitation(make_spec(version=2, flip=2)), encoding='utf-8')
        self.assertEqual(self.run_tool(root, source='v3.json', target='v3b.json'), 0)
        self.assert_carries(root, '0x' + ni.keccak256(make_spec(version=2, flip=2)).hex(), make_spec(version=2, flip=2))

    def test_the_guard_flags_planted_copies(self):
        planted = [('a.py', f'X = {CUR_CHAIN}n'), ('b.rs', f'const X: u64 = {CUR_CHAIN:_};'), ('c.js', f"c='0x{CUR_CHAIN:X}'"),
                   ('d.md', f'genesis {CUR_GENESIS[:8]}...'), ('e.json', CUR_SPEC.hex())]
        self.assertEqual(unaccounted(planted), [rel for rel, _ in planted])
        exempt = [('development/x.md', str(CUR_CHAIN)), ('deploy/websites/x.js', str(CUR_CHAIN)), (list(ni.FILES)[0], str(CUR_CHAIN)),
                  (ni.RETIRED, str(CUR_CHAIN))]
        self.assertEqual(unaccounted(exempt), [])
        # A retired identity the tree no longer pins: flagged everywhere outside the kept files, FILES included.
        gone_chain, gone_genesis = NEW_CHAIN, HASH_A[2:]
        gone = anywhere([(gone_chain, gone_genesis)])
        planted = [('a.py', f'X = {gone_chain}n'), ('b.rs', f'const X: u64 = {gone_chain:_};'), ('c.js', f"c='0x{gone_chain:X}'"),
                   ('d.md', f'genesis {gone_genesis[:8].upper()}...'), (list(ni.FILES)[0], f'chain {gone_chain}')]
        self.assertEqual(unaccounted(planted, gone=gone), [rel for rel, _ in planted])
        exempt = [('development/x.md', str(gone_chain)), ('deploy/websites/x.js', str(gone_chain)), (ni.RETIRED, str(gone_chain)),
                  ('prototypes/pocold/tests/data/wire-golden.json', str(gone_chain)), ('e.md', 'no identity')]
        self.assertEqual(unaccounted(exempt, gone=gone), [])

    def test_every_tracked_file_that_pins_the_current_identity_is_listed_or_kept(self):
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
        self.assertEqual(unaccounted(texts()), [], 'pins of the current identity: add the file to FILES, or to KEPT with a '
                         'reason; pins of a retired identity: remove them')
        for rel in KEPT:
            self.assertTrue((ni.ROOT / rel).is_file(), rel)

    def test_the_retired_release_texts_go_with_the_switch(self):
        held = [rel for rel, old in STEP3.items() if old.search((ni.ROOT / rel).read_text(encoding='utf-8'))]
        if SWITCHED:
            self.assertEqual(held, [], 'the identity-switch release replaces or bumps these: ' + ', '.join(held))
        else:
            self.assertEqual(held, list(STEP3), 'update STEP3 to what the files hold today')
        # The detector works: each pattern finds the text it names on the retired release's tree.
        self.assertIsNotNone(STEP3['prototypes/pocol-extension/README-STUDIO.md'].search('its chain ID and genesis are unchanged'))

if __name__ == '__main__':
    unittest.main()
