#!/usr/bin/env python3
"""set_network_identity.py switches every listed pin (chain ID, genesis hash, GenesisSpec) from the --from network's
identity to the --to network's, with the content versions and the Studio installer derived from them; --check fails on
any other identity; a redo switches from the current one; line endings are kept, and hashed sources with converted
(CRLF) line endings are refused; nothing is written when anything fails, an I/O error included; bad, non-canonical or
contradictory invitations are refused, as is a --to genesis the tree's release node does not decode; a switch records
the --from network in the retired list and a retired --to needs --allow-revert; every tracked file that pins the
tree's current identity is listed in FILES or kept on purpose, and none (FILES included) pins a retired identity the
tree no longer pins; and the shipped release texts that are true only on the retired network are gone once the tree
pins a network that is not retired; the extension's built-in profile placeholder (D1) is proven, placed only into the
placeholder files, and replaced (with its mark) by the switch; the release's NETWORK_PROFILES (readiness.rs) holds
exactly the entry of the tree's identity unless that is a retired or the rehearsal network (then none, as today), each
entry a NetworkProfile in exactly the forms readiness.rs's WRITTEN test compiles: on a public network chain
(PUBLIC_NETWORK_CHAINS, equal to the mainnet profile's chain) ExpectedAdmission::PUBLIC_MAINNET with the approval
reference of OWNER-DECISIONS-2026-11-20 D1, on any other chain ExpectedAdmission::FRESH_NETWORK without one (never
ExpectedAdmission::LAUNCH, and never the other chain's form): a switch writes the --to entry and removes the --from one,
--check refuses a missing or extra entry, and a round trip restores the file byte for byte.

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
# The built-in profile placeholder (decision D1): a spec-2 GenesisSpec of no network, from its committed invitation.
PLACEHOLDER = ni.ROOT / 'deploy' / 'mainnet' / 'placeholder-network.pocol.json'
PH_CHAIN, PH_GENESIS, PH_SPEC, PH_VERSION = ni.invitation_identity(PLACEHOLDER, (2,))
def _free_chain():
    """The chain ID of the --to fixtures: a valid chain ID that no listed file mentions in any spelling."""
    texts = [(ni.ROOT / rel).read_text(encoding='utf-8').lower() for rel in ni.FILES]
    for chain in range(990_001, 999_999, 7):
        if chain != CUR_CHAIN and not any(t in text for text in texts for t in (str(chain), f'{chain:_}', f'{chain:x}')):
            return chain
    raise AssertionError('no free fixture chain ID')
NEW_CHAIN = _free_chain()
def anywhere(identities, keep_chain=None):
    """An independent detector of identities: each chain ID with or without digit groups, its hex digits, and the
    start of each genesis hash, in any case; None for no identity. keep_chain is a chain ID that still belongs to the
    current network: an identity on it (a redo with another genesis for the same chain, a postponed launch) is detected
    by its genesis hash only, as the tool switches it, so the current network's chain ID pins are not taken for it."""
    alts = [a for chain, genesis in identities
            for a in ((() if chain == keep_chain else (re.escape(f'{chain}'), re.escape(f'{chain:_}'), f'{chain:x}')) + (genesis[:8],))]
    return re.compile('(?i)' + '|'.join(alts)) if alts else None
CUR_ANYWHERE = anywhere([(CUR_CHAIN, CUR_GENESIS)])
# The current genesis hash alone (the check of the files that name the public network chain on purpose).
CUR_GENESIS_ANYWHERE = anywhere([(CUR_CHAIN, CUR_GENESIS)], keep_chain=CUR_CHAIN)
# The retired identities (read independently of the tool), and those the tree no longer pins: no tracked file outside
# the kept ones may hold one of those, FILES included, so no copy of an earlier network outlives its switch.
RETIRED = [(e['chainId'], e['genesisHash'][2:]) for e in json.loads((ni.ROOT / ni.RETIRED).read_text(encoding='utf-8'))['retired']]
GONE_ANYWHERE = anywhere([r for r in RETIRED if r != (CUR_CHAIN, CUR_GENESIS)], keep_chain=CUR_CHAIN)
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
# The CP items of version 1: the current CP's first 52 (the v2 schema is the v1 schema plus seven fields).
_cp_v1_items = []
_j = _cp_start
for _ in range(ni.CP_ITEMS[1]):
    _e = ni.rlp_item(_rest, _j)[2]; _cp_v1_items.append(_rest[_j:_e]); _j = _e
_cp_v1 = b''.join(_cp_v1_items)
_tail = _rest[_cp_end:]
# The seven CP v2 fields (the lp4 devnet defaults, as pocol-extension/tests/decoder_corpus.mjs uses; FN-2a deleted F0_*,
# FN-2b P_MAX, T_PROB, E_epoch, theta_bp and k_sigma).
CP_V2_EXTRA = [31557600, 62500000000000000, 5000, 10000000, 4, 40, 2500]

# allocRoot, M_0List and sysCodeHash of the current GenesisSpec; a spec-2 one names the v3 program set's sysCodeHash
# (the only set the deploy tools accept on a spec-2 genesis).
_alloc, _members, _sys = _tail[:33], _tail[33:-33], _tail[-33:]
assert _alloc[0] == _sys[0] == 0xa0
V3_SYS = bytes.fromhex(ni.pocol_identity.V3_SYS_CODE_HASH)

def make_spec(chain=NEW_CHAIN, version=1, flip=0, extra=None, sys=None, items=None, nonce_mode=1):
    """A GenesisSpec of `version` for `chain` built from the current one's items, with one byte changed by `flip`: the
    last byte (sysCodeHash) of a version-1 spec, the last byte of allocRoot of a version-2 one, which names the v3
    program set (or `sys`) and has CP nonceMode `nonce_mode` (1, the only one a release node and the extension admit on
    spec 2). `items` replaces the encoded CP v2 fields."""
    if version != 2:
        cp = _cp_v1
        spec = bytearray(rlp_list(int_item(version) + int_item(chain) + rlp_list(cp) + _tail))
        spec[-1] ^= flip
        return bytes(spec)
    extras = items if items is not None else b''.join(int_item(v) for v in (CP_V2_EXTRA if extra is None else extra))
    alloc = _alloc[:-1] + bytes([_alloc[-1] ^ flip])
    cp = b''.join(int_item(nonce_mode) if i == ni.NONCE_MODE else item for i, item in enumerate(_cp_v1_items))
    return rlp_list(int_item(version) + int_item(chain) + rlp_list(cp + extras) + alloc + _members
                    + b'\xa0' + (V3_SYS if sys is None else sys))

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
    return json.dumps({'schema': 'pocol-network-v2', 'consensusRulesVersion': 2, 'seeds': ['127.0.0.1:30300'], 'files': files})

def expects(path):
    """The --expect-* arguments of an invitation, computed here (not by the tool): its file SHA-256 and the identity its
    GenesisSpec proves."""
    doc = json.loads(Path(path).read_text(encoding='utf-8'))
    spec = bytes.fromhex(doc['files']['genesis.spec.hex'].removeprefix('0x'))
    chain = json.loads(doc['files']['devnet.json'])['chainId']
    return ['--expect-sha256', hashlib.sha256(Path(path).read_bytes()).hexdigest(), '--expect-chain-id', str(chain),
            '--expect-genesis', '0x' + ni.keccak256(spec).hex()]

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
}
# Files outside FILES that name the public network's chain ID (PoCol Mainnet's, the chain of the mainnet profile and of
# PUBLIC_NETWORK_CHAINS) on purpose: they declare, build, test or check the public network itself, or name it to refuse
# it, and are not client pins the identity switch rewrites. Before the switch this guard requires that exactly these
# files name that chain (test_every_tracked_file_that_names_the_public_network_chain_is_listed), so a new mention is
# reviewed here; once the tree pins a network on that chain their mentions are not taken for unlisted client pins
# (ID-04: the switch rehearsal found these 27 files, and the runbook, flagged). A retired identity is still refused in
# them like anywhere else. Each entry is (the number of times the file names the chain ID, the reason): the count is
# pinned too, so a new mention inside a file that is already listed is reviewed here as well; in readiness.rs only its
# #[cfg(test)] module may name it (outside_tests).
MAINNET_NAMED = {
    '.github/workflows/public-mainnet-e2e.yml': (1, 'CI of the ceremony-filled mainnet profile on its own chain (throwaway keys)'),
    'deploy/faucet/README.md': (1, 'documents the faucet configuration of the mainnet chain'),
    'deploy/faucet/config.mainnet.example.json': (1, "the faucet's mainnet configuration example (its chain ID)"),
    'deploy/faucet/pocol_faucet.py': (1, "the faucet's mainnet chain ceiling (MAINNET_CHAIN_IDS)"),
    'deploy/mainnet/README.md': (2, 'the step-7 runbook names the published identity the switch checks'),
    'deploy/mainnet/test_v3_profiles.py': (1, "checks the committed profiles' chain IDs"),
    'deploy/mainnet/v3-profile.json': (1, 'the mainnet profile: its chain ID is the public network chain'),
    'deploy/public-testnet/node_operator.py': (1, "the release tools' public network chains (PUBLIC_NETWORK_CHAINS)"),
    'deploy/rehearsal/registration_fill.py': (2, 'refuses the mainnet chain unless asked'),
    'deploy/rehearsal/test_collect_measures.py': (2, 'tests the refusal of an unexpected chain ID'),
    'deploy/rehearsal/test_registration_fill.py': (3, 'tests the refusal of the mainnet chain'),
    'deploy/release/README.md': (1, "documents the cross-platform build check's throwaway genesis on the mainnet chain"),
    'deploy/release/compare_build_identity.py': (2, 'builds a throwaway genesis on the mainnet chain to compare builds'),
    'deploy/standalone/Initialize-PoColNetwork.ps1': (3, "the ceremony's profile table and public network chains"),
    'deploy/standalone/README.md': (1, "documents the ceremony's mainnet profile"),
    'deploy/standalone/test_initialize_network.ps1': (2, "tests the ceremony's profile table"),
    'deploy/standalone/test_initialize_network.py': (1, "tests the ceremony's profile table"),
    'prototypes/lp1-node/src/profile.rs': (2, 'PUBLIC_NETWORK_CHAINS of the profile code (R22, OWNER-DECISIONS-2026-11-20 D5)'),
    'prototypes/lp1-node/tests/gate.rs': (3, 'tests the public-network scope gate on the mainnet chain'),
    'prototypes/lp1-node/tests/profile.rs': (4, 'tests PUBLIC_NETWORK_CHAINS'),
    'prototypes/lp4-execution/src/admission/tests.rs': (1, 'tests the admission of the mainnet chain'),
    'prototypes/lp4-execution/src/p1_profile.rs': (2, 'PUBLIC_NETWORK_CHAINS of the profile code (a copy of profile.rs)'),
    'prototypes/pocold/src/mobile.rs': (1, "the mobile engine's throwaway genesis on the mainnet chain (temp dir only)"),
    'prototypes/pocold/src/readiness.rs': (3, "readiness tests of the public chain's entry form (WRITTEN, PUBLIC)"),
    'prototypes/pocold/tests/e2e_admission_interop.py': (2, 'single-host interop test on the mainnet chain (throwaway keys)'),
    'prototypes/pocold/tests/e2e_public_genesis.py': (4, 'single-host public-network genesis test (throwaway keys)'),
    'prototypes/pocold/tests/fn4_profiles.rs': (3, 'builds and admits the committed mainnet profile'),
    'prototypes/pocold/tests/launch_delay_asert.rs': (1, 'the late-start check of the fresh-network profiles'),
}
# Shipped with the packages, so the identity-switch release replaces or bumps them: while the tree pins a retired
# network each holds its pattern (the old release's version, or text that is true only on the retired network); once
# it pins a network that is not retired none may. A version bump before the switch removes its entry here, as does a
# file that stops holding the version (five-device-build.yml reads it from package.py since Phase 7). The native 0.5.0
# bump (owner decision 2026-11-16) landed before the switch, so package.py, the standalone README, the Linux desktop
# texts, the Windows wallet and build.gradle are gone from this list; what remains moves with the signed extension
# release and its download links. The Studio quick start's upgrade section names no version since Sites W14.
STEP3 = {
    # 0.5.0 is decided, but the version moves with the signed release and its download links (the websites' check,
    # deploy/websites/tools/site_identity.py, requires the links to name the package.json version).
    'prototypes/pocol-extension/package.json': re.compile(r'"version":\s*"0\.4\.1"'),
    'prototypes/pocol-studio/package.json': re.compile(r'"version":\s*"0\.1\.1"'),
}
# Current documents whose text described the tree before the switch: once the tree pins a network that is not retired,
# none may still say that the pin is the retired network's, that the extension holds the D1 placeholder, that no launch
# network is pinned, or that the client texts carry the retired identity. Matched on whitespace-joined text, so a
# rewrapped line is still found.
SUPERSEDED_AFTER_SWITCH = {
    'README.md': re.compile(r"(?i)that pin is the retired network's|Until the new genesis exists|is launching a brand-new public network"),
    'deploy/mainnet/README.md': re.compile(r"(?i)Until the new genesis exists, the extension's built-in profile|it holds a placeholder|"
                                           r"keep the tree's identity until the switch|\(CI checks the refusal instead of packaging\)"),
    'development/public-network/FINDINGS.md': re.compile(r"(?i)No launch network is pinned in the clients yet|"
                                                         r"until the identity switch the source holds the D1 placeholder|"
                                                         r"carry the retired network identity until the identity switch\.\*\* Until"),
}
# The Studio's install dialog linked the retired network's extension package; since 2026-11-22 (SITE-01) it links
# pocol.net's release verification page instead, before and after the switch.
RETIRED_EXTENSION_LINK = ('prototypes/pocol-studio/dist/app.js', re.compile(r'PoCol-Studio-IDE-Viewer-0\.4\.[01]\.zip'))
TRACKED = list(ni.FILES) + [page for page, _ in ni.VERSIONED] + [ni.INSTALLER, ni.NODE_SPEC, ni.RETIRED, ni.PROFILES,
                                                                  ni.REHEARSAL_PROFILE, ni.PUBLIC_CHAINS, ni.MAINNET_PROFILE]
# The rehearsal network's chain ID (read independently of the tool): never a launch network.
REHEARSAL_CHAIN = json.loads((ni.ROOT / ni.REHEARSAL_PROFILE).read_text(encoding='utf-8'))['chainId']

# The public network chains (read independently of the tool): PUBLIC_NETWORK_CHAINS of the profile code, and the
# mainnet profile's chain, which it must equal.
PUBLIC = frozenset(int(c) for c in re.search(r'(?m)^pub const PUBLIC_NETWORK_CHAINS: &\[u64\] = &\[([0-9, ]*)\];',
                   (ni.ROOT / 'prototypes/lp4-execution/src/p1_profile.rs').read_text(encoding='utf-8')).group(1).split(', ') if c)
MAINNET_CHAIN = json.loads((ni.ROOT / 'deploy/mainnet/v3-profile.json').read_text(encoding='utf-8'))['chainId']
def chain_anywhere(chain):
    """The chain ID alone, in the spellings anywhere() detects (decimal, digit groups, hex digits), in any case."""
    return re.compile('(?i)' + '|'.join((re.escape(f'{chain}'), re.escape(f'{chain:_}'), f'{chain:x}')))
MAINNET_ANYWHERE = chain_anywhere(MAINNET_CHAIN)
# The files whose mentions of the current network are not unlisted pins: MAINNET_NAMED, once the tree pins a network on
# the public network chain.
NAMED = set(MAINNET_NAMED) if CUR_CHAIN == MAINNET_CHAIN else set()
# One NETWORK_PROFILES entry as readiness.rs compiles it (read independently of the tool): a NetworkProfile with the
# chain ID, the decoded genesis hash and, on a public network chain, the public network's admission with the owner's
# approval reference, on any other chain the experimental admission without one.
ENTRY = re.compile(r'NetworkProfile\{chain_id:([0-9]+),genesis_hash:genesis_hash\("([0-9a-f]{64})"\),'
                   r'(admission:ExpectedAdmission::PUBLIC_MAINNET,approval:Some\("OWNER-DECISIONS-2026-11-20 D1"\)|admission:ExpectedAdmission::FRESH_NETWORK,approval:None)\}')

def entries_of(body):
    """[(chain ID, genesis hash hex)] of a list body of ENTRY entries; every NetworkProfile in it must be one, in the
    form of its chain."""
    found = [(int(c), g, t) for c, g, t in ENTRY.findall(body)]
    assert body.count('NetworkProfile{') == len(found), f'an entry not in the written form: {body}'
    for c, _, t in found:
        assert ('PUBLIC_MAINNET' in t) == (c in PUBLIC), f'chain {c}: an entry in the other chain form: {body}'
    return [(c, g) for c, g, _ in found]

def profiles(root):
    """[(chain ID, genesis hash hex)] of NETWORK_PROFILES, read independently of the tool."""
    text = (root / ni.PROFILES).read_text(encoding='utf-8')
    line = next(l for l in text.splitlines() if l.startswith('pub const NETWORK_PROFILES:'))
    return entries_of(line)

def unaccounted(files, current=CUR_ANYWHERE, gone=GONE_ANYWHERE, named=None, current_genesis=CUR_GENESIS_ANYWHERE):
    """Paths among (path, text) pairs outside the kept ones that pin the current identity without being listed, or
    pin a retired identity the tree no longer pins (listed, named or not). A file in `named` (MAINNET_NAMED once the tree
    pins the public network chain) names the chain ID on purpose, so only the current genesis hash (`current_genesis`)
    is flagged in it: a named file never carries a second copy of the network's genesis."""
    named = NAMED if named is None else named
    def pins_current(rel, text):
        if rel in ni.FILES:
            return False
        text = without_profiles(rel, text)
        if rel not in named:
            return current is not None and current.search(text)
        # readiness.rs names the public chain in its tests only: outside its #[cfg(test)] module the chain ID alone is
        # still a pin.
        if rel == ni.PROFILES and current is not None and current.search(outside_tests(text)):
            return True
        return current_genesis is not None and current_genesis.search(text)
    return [rel for rel, text in files
            if not rel.startswith(tuple(KEPT_PREFIXES)) and rel not in KEPT
            and (pins_current(rel, text) or (gone is not None and gone.search(text)))]

# The start of a Rust file's test module.
TEST_MODULE = re.compile(r'(?m)^#\[cfg\(test\)\]\s*\nmod tests\s*\{')

def outside_tests(text):
    """A Rust file's text before its #[cfg(test)] module (the whole text when it has none)."""
    m = TEST_MODULE.search(text)
    return text if m is None else text[:m.start()]

def without_profiles(rel, text):
    """The text the current-identity guard searches: in ni.PROFILES only the NETWORK_PROFILES declaration's body is
    left out (it pins the tree's identity once it is a launch network, and the tool's --check verifies it); every other
    line of that file stays under the guard."""
    if rel != ni.PROFILES: return text
    try:
        return ni.with_profiles(text, [], PUBLIC)
    except ValueError:
        return text

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
        shutil.copy(PLACEHOLDER, tmp / 'ph.json')
        for name, spec in (('cur.json', CUR_SPEC), ('a.json', SPEC_A), ('b.json', SPEC_B), ('v3.json', SPEC_V3)):
            (tmp / name).write_text(invitation(spec), encoding='utf-8')
        for i, spec in enumerate(NON_CANONICAL.values()):
            (tmp / f'bad{i}.json').write_text(invitation(spec, None), encoding='utf-8')
        return tmp

    def run_tool(self, root, *extra, source='cur.json', target='a.json'):
        return ni.main(['--from', str(root / source), '--to', str(root / target), '--root', str(root), *extra])

    def published(self, root, target):
        """The --expect-* arguments a switch to `target` needs when its chain is a public network chain (the tree's own
        network once the identity switch has pinned PoCol Mainnet), else none: a test that switches to such a network
        for another purpose gives the published values, so it is refused only for the reason it tests."""
        return expects(root / target) if ni.invitation_identity(root / target)[0] in PUBLIC else []

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
            text = (ni.ROOT / rel).read_text(encoding='utf-8')
            if rel in ni.PLACEHOLDER_FILES and ni.PLACEHOLDER_MARK in text:
                # The built-in profile placeholder (D1): its counts are of the placeholder, and it holds no current pin.
                self.assertEqual(held(text, PH_CHAIN, PH_GENESIS, PH_SPEC.hex()), expected, rel)
                self.assertEqual(held(text, CUR_CHAIN, CUR_GENESIS, CUR_SPEC.hex()), (0, 0, 0), rel)
            else:
                self.assertEqual(held(text, CUR_CHAIN, CUR_GENESIS, CUR_SPEC.hex()), expected, rel)

    def test_the_placeholder_is_proven_and_names_no_network(self):
        self.assertEqual((PH_VERSION, ni.spec_identity(PH_SPEC, (2,))), (2, (2, PH_CHAIN)))
        self.assertNotIn(PH_CHAIN, [chain for chain, _ in RETIRED] + [CUR_CHAIN, 0, 777001])
        self.assertNotIn(PH_GENESIS, [genesis for _, genesis in RETIRED] + [CUR_GENESIS])
        doc = json.loads(PLACEHOLDER.read_text(encoding='utf-8'))
        self.assertEqual(doc['seeds'], [], 'a placeholder has no seeds')
        self.assertIn('NOT A NETWORK', doc['about'])
        for rel in ni.PLACEHOLDER_FILES:
            self.assertIn(rel, ni.FILES)
            text = (ni.ROOT / rel).read_text(encoding='utf-8')
            if ni.PLACEHOLDER_MARK in text:
                self.assertEqual(ni.placeholder_identity(rel, text), (PH_CHAIN, PH_GENESIS, PH_SPEC))
                # The mark sits right after the NETWORK object.
                self.assertIn(ni.PLACEHOLDER_AFTER + ni.PLACEHOLDER_MARK, text)
            else:
                self.assertIsNone(ni.placeholder_identity(rel, text))
        self.assertIsNone(ni.placeholder_identity('x', 'no mark'))

    def test_placing_the_placeholder_and_switching_it_away(self):
        root = self.copy()
        rel = next(iter(ni.PLACEHOLDER_FILES))
        def place(*extra, placeholder='ph.json'):
            return ni.main(['--placeholder', str(root / placeholder), '--root', str(root), *extra])
        before = self.snapshot(root)
        if ni.PLACEHOLDER_MARK in (ni.ROOT / rel).read_text(encoding='utf-8'):
            self.assertEqual(place('--check'), 0, 'the tree carries the placeholder')
            self.assertEqual(place(), 0)
            self.assertEqual(before, self.snapshot(root), 'placing it again changes nothing')
        # Another placeholder replaces it in the placeholder files only.
        ph2 = make_spec(chain=NEW_CHAIN + 7, version=2, flip=3)
        (root / 'ph2.json').write_text(invitation(ph2), encoding='utf-8')
        self.assertEqual(place(placeholder='ph2.json'), 0)
        after = self.snapshot(root)
        self.assertEqual({k for k in after if after[k] != before[k]}, set(ni.PLACEHOLDER_FILES), 'only the placeholder files change')
        text = (root / rel).read_text(encoding='utf-8')
        self.assertEqual(ni.placeholder_identity(rel, text), (NEW_CHAIN + 7, ni.keccak256(ph2).hex(), ph2))
        self.assertEqual(text.count(ni.PLACEHOLDER_MARK), 1)
        self.assertEqual(held(text, NEW_CHAIN + 7, ni.keccak256(ph2).hex(), ph2.hex()), ni.FILES[rel])
        # Refused, writing nothing: a spec-1 placeholder, a retired or the current network, with --from or --to, and a
        # --to that is the placeholder.
        before = self.snapshot(root)
        for args in (['--placeholder', str(root / 'a.json')], ['--placeholder', str(root / 'cur.json')],
                     ['--placeholder', str(root / 'ph.json'), '--from', str(root / 'cur.json')],
                     ['--placeholder', str(root / 'ph.json'), '--to', str(root / 'a.json')]):
            with self.assertRaises(SystemExit, msg=args):
                ni.main([*args, '--root', str(root)])
        self.assertEqual(self.run_tool(root, target='ph2.json'), 2, '--to is the placeholder')
        self.assertEqual(before, self.snapshot(root))
        # The switch to a real network replaces the placeholder and removes its mark.
        self.assertEqual(self.run_tool(root), 0)
        self.assert_carries(root, HASH_A, SPEC_A)
        text = (root / rel).read_text(encoding='utf-8')
        self.assertNotIn('NETWORK_PLACEHOLDER', text)
        self.assertEqual(held(text, NEW_CHAIN + 7, ni.keccak256(ph2).hex(), ph2.hex()), (0, 0, 0))
        # A placeholder placed over the tree's identity (A now), then switched to B with the rest of the tree.
        self.assertEqual(place(), 0)
        text = (root / rel).read_text(encoding='utf-8')
        self.assertEqual(ni.placeholder_identity(rel, text), (PH_CHAIN, PH_GENESIS, PH_SPEC))
        self.assertEqual(held(text, NEW_CHAIN, HASH_A[2:], SPEC_A.hex()), (0, 0, 0))
        self.assertEqual(self.run_tool(root, source='a.json', target='b.json'), 0)
        self.assert_carries(root, HASH_B, SPEC_B)
        self.assertNotIn('NETWORK_PLACEHOLDER', (root / rel).read_text(encoding='utf-8'))

    def test_a_placeholder_that_does_not_prove_itself_is_refused(self):
        root = self.copy()
        rel = next(iter(ni.PLACEHOLDER_FILES))
        self.assertEqual(ni.main(['--placeholder', str(root / 'ph.json'), '--root', str(root)]), 0)
        text = (root / rel).read_text(encoding='utf-8')
        for bad in (text.replace(f'"chainId": {PH_CHAIN},', f'"chainId": {PH_CHAIN + 1},'),
                    text.replace(PH_GENESIS, PH_GENESIS[:-1] + ('0' if PH_GENESIS[-1] != '0' else '1')),
                    text.replace(ni.PLACEHOLDER_MARK, ni.PLACEHOLDER_MARK * 2)):
            self.assertNotEqual(bad, text)
            (root / rel).write_text(bad, encoding='utf-8')
            before = self.snapshot(root)
            self.assertEqual(self.run_tool(root), 2)
            self.assertEqual(ni.main(['--placeholder', str(root / 'ph.json'), '--root', str(root)]), 2)
            self.assertEqual(before, self.snapshot(root))
        # An upper-case placeholder hash and GenesisSpec still count, and are switched.
        (root / rel).write_text(text.replace(PH_GENESIS, PH_GENESIS.upper()).replace(PH_SPEC.hex(), PH_SPEC.hex().upper()), encoding='utf-8')
        self.assertEqual(self.run_tool(root), 0)
        self.assert_carries(root, HASH_A, SPEC_A)

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
        # The switch replaces the built-in profile placeholder (D1) too, and its mark goes.
        self.assertNotIn('NETWORK_PLACEHOLDER', network)
        self.assertEqual(held(network, PH_CHAIN, PH_GENESIS, PH_SPEC.hex()), (0, 0, 0))
        # The extension pages name no network: their chain ID chip is filled from the built-in profile at run time.
        for page in ('contract.html', 'publisher.html'):
            text = (ni.ROOT / 'prototypes/pocol-extension/src' / page).read_text(encoding='utf-8')
            self.assertNotIn(page, [Path(rel).name for rel in ni.FILES])
            self.assertIn(' id="networkChain" title="Chain ID"></span>', text, page)
            self.assertEqual(held(text, CUR_CHAIN, CUR_GENESIS, CUR_SPEC.hex()), (0, 0, 0), page)
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
            self.run_tool(root, *self.published(root, 'cur.json'), source='b.json', target='cur.json')
        self.assertEqual(before, self.snapshot(root), 'a refused revert writes nothing')
        self.assertEqual(self.run_tool(root, '--allow-revert', *self.published(root, 'cur.json'), source='b.json', target='cur.json'), 0)
        after, original = self.snapshot(root), self.snapshot(ni.ROOT)
        # The built-in profile placeholder (D1) is replaced for good: switching back pins the original network there.
        kept = {ni.RETIRED} | {rel for rel in ni.PLACEHOLDER_FILES if ni.PLACEHOLDER_MARK in (ni.ROOT / rel).read_text(encoding='utf-8')}
        # Switching back pins a network the switches retired, so NETWORK_PROFILES holds no entry then; while the tree
        # pins a retired network it held none before either, and the file is restored byte for byte.
        self.assertEqual(profiles(root), [])
        if SWITCHED:
            kept.add(ni.PROFILES)
        self.assertEqual({k: v for k, v in after.items() if k not in kept}, {k: v for k, v in original.items() if k not in kept},
                         'switching back restores the original bytes of every pin')
        for rel in kept - {ni.RETIRED, ni.PROFILES}:
            text = (root / rel).read_text(encoding='utf-8')
            self.assertEqual(held(text, CUR_CHAIN, CUR_GENESIS, CUR_SPEC.hex()), ni.FILES[rel], rel)
            self.assertNotIn(ni.PLACEHOLDER_MARK, text)
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
        rel = 'README.md'
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
                rel = 'README.md'
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
        rel = 'README.md'
        text = (root / rel).read_text(encoding='utf-8')
        (root / rel).write_text(re.sub(CUR_GENESIS, HASH_A[2:], text, flags=re.IGNORECASE), encoding='utf-8')
        before = self.snapshot(root)
        self.assertEqual(self.run_tool(root), 2, 'it no longer holds its copies of the current identity')
        self.assertEqual(before, self.snapshot(root))

    def test_any_case_of_the_hex_chain_id_is_switched(self):
        root = self.copy()
        # The extension test's chain ID, spelled as upper-case hex (one chain ID token either way).
        rel = 'prototypes/pocol-extension/tests/publisher_test.mjs'
        text = (root / rel).read_text(encoding='utf-8')
        upper = '0x' + hex(CUR_CHAIN)[2:].upper()
        self.assertIn(f'chainId={CUR_CHAIN};', text)
        (root / rel).write_text(text.replace(f'chainId={CUR_CHAIN};', f'chainId={upper};'), encoding='utf-8')
        self.assertEqual(self.run_tool(root), 0)
        self.assert_carries(root, HASH_A, SPEC_A)

    def test_a_hash_inside_longer_hex_is_neither_counted_nor_changed(self):
        root = self.copy()
        rel = 'README.md'
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
        for rel, old in (('README.md', CUR_GENESIS), (ni.ANCHOR, CUR_SPEC.hex())):
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
            self.run_tool(root, *self.published(root, 'cur.json'), target='cur.json')
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
        # A spec-2 genesis must pass the whole release decoder on the final CP schema v2 and name the v3 program
        # set, with or without the release version allowed beside it.
        self.assertEqual(ni.CP_ITEMS, {1: len(ni.pocol_identity.CP_SCHEMA), 2: len(ni.pocol_identity.CP_SCHEMA_V2)})
        k = {f[0]: i - 52 for i, f in enumerate(ni.pocol_identity.CP_SCHEMA_V2) if i >= 52}
        def with_extra(name, value):
            extra = list(CP_V2_EXTRA)
            extra[k[name]] = value
            return make_spec(version=2, extra=extra)
        # rho_bp as 82 00 05, a canonical RLP string holding a leading-zero integer, so it reaches gsInt. (A wrapped
        # single byte, 81 05, is refused earlier as non-canonical RLP: see 'a wrapped single byte inside CP'.)
        leading_zero = b''.join(int_item(v) for v in CP_V2_EXTRA[:-1]) + b'\x82\x00\x05'
        for bad, reason in ((make_spec(version=2, sys=bytes(b ^ 0xff for b in V3_SYS)), 'v3 program set'), (make_spec(version=2, sys=b'\x00' * 32), 'v3 program set'),
                            (with_extra('rho_bp', 10001), 'gsRange: rho_bp'), (with_extra('H_half', 0), 'gsRange: H_half'),
                            (with_extra('gamma_tip_bp', 10001), 'gsRange: gamma_tip_bp'), (with_extra('R_react', 256), 'gsRange: R_react'),
                            (with_extra('C_react', 1 << 32), 'gsRange: C_react'), (with_extra('S_tail', 1 << 256), 'gsRange: S_tail'),
                            (make_spec(version=2, items=leading_zero), 'gsInt: rho_bp')):
            for allowed in ((2,), (1, 2)):
                with self.subTest(reason=reason, allowed=allowed), self.assertRaisesRegex(ValueError, reason):
                    ni.spec_identity(bad, allowed)
        # The reference decoder accepts SPEC_V3 at its default (release) ceiling, as the Rust and JS decoders do; only a
        # spec-1-only ceiling refuses it. The tree's release node decodes spec 2.
        decoded = ni.pocol_identity.decode_genesis_spec(SPEC_V3)
        self.assertEqual((decoded['specVersion'], len(decoded['cp']), decoded['sysCodeHash']), (2, 59, V3_SYS))
        self.assertEqual(decoded, ni.pocol_identity.decode_genesis_spec(SPEC_V3, 2))
        with self.assertRaisesRegex(ValueError, 'gsVersion:2'):
            ni.pocol_identity.decode_genesis_spec(SPEC_V3, 1)
        self.assertEqual(RELEASE, 2)
        root = self.copy()
        node = root / ni.NODE_SPEC
        text = node.read_text(encoding='utf-8')
        node.write_text(re.sub(r'pub const SPEC_VERSION: u8 = [0-9]+;', 'pub const SPEC_VERSION: u8 = 1;', text), encoding='utf-8')
        before = self.snapshot(root)
        for args in ((), ('--check',)):
            with self.subTest(args=args), self.assertRaises(SystemExit):
                self.run_tool(root, *args, target='v3.json')
        self.assertEqual(before, self.snapshot(root), 'a refused switch writes nothing')
        # A tree whose release node decodes spec 2 (the tree's own) switches to it, and a spec-2 tree is switched onward.
        node.write_text(re.sub(r'pub const SPEC_VERSION: u8 = [0-9]+;', 'pub const SPEC_VERSION: u8 = 2;', text), encoding='utf-8')
        # Even then, a spec-2 genesis that names another program set or fails the release decoder is refused.
        for name, bad in (('v3-other-set.json', make_spec(version=2, sys=b'\x00' * 32)),
                          ('v3-bad-rho.json', make_spec(version=2, extra=CP_V2_EXTRA[:-1] + [10001]))):
            (root / name).write_text(invitation(bad, {'chainId': NEW_CHAIN, 'genesisHash': '0x' + ni.keccak256(bad).hex()}), encoding='utf-8')
            before = self.snapshot(root)
            with self.subTest(name=name), self.assertRaises(SystemExit):
                self.run_tool(root, target=name)
            self.assertEqual(before, self.snapshot(root), 'a malformed v3 genesis writes nothing')
            (root / name).unlink()
        self.assertEqual(self.run_tool(root, target='v3.json'), 0)
        self.assert_carries(root, HASH_V3, SPEC_V3)
        self.assertEqual(self.run_tool(root, '--check', target='v3.json'), 0)
        (root / 'v3b.json').write_text(invitation(make_spec(version=2, flip=2)), encoding='utf-8')
        self.assertEqual(self.run_tool(root, source='v3.json', target='v3b.json'), 0)
        self.assert_carries(root, '0x' + ni.keccak256(make_spec(version=2, flip=2)).hex(), make_spec(version=2, flip=2))

    def test_the_tree_pins_the_network_profiles_of_its_identity(self):
        # Today the tree pins a retired network, so NETWORK_PROFILES is empty; once it pins a launch network it holds
        # exactly that network's entry.
        expected = [] if (CUR_CHAIN, CUR_GENESIS) in RETIRED or CUR_CHAIN == REHEARSAL_CHAIN else [(CUR_CHAIN, CUR_GENESIS)]
        self.assertEqual(profiles(ni.ROOT), expected)
        self.assertEqual(ni.read_profiles((ni.ROOT / ni.PROFILES).read_text(encoding='utf-8'), PUBLIC), expected)
        if not SWITCHED:
            self.assertEqual(expected, [], 'the retired identity is the current one: no launch network is pinned')
        self.assertEqual(ni.launch_profiles((CUR_CHAIN, CUR_GENESIS), RETIRED, ni.rehearsal_chain(ni.ROOT)), expected)
        self.assertEqual(ni.rehearsal_chain(ni.ROOT), REHEARSAL_CHAIN)

    def test_the_entry_form_the_tool_writes_is_the_one_readiness_compiles(self):
        # readiness.rs's unit test WRITTEN compiles entries in this form and checks their decoded genesis hashes.
        text = (ni.ROOT / ni.PROFILES).read_text(encoding='utf-8')
        self.assertEqual(ni.PROFILES_DECL, 'pub const NETWORK_PROFILES:&[NetworkProfile]=&[')
        self.assertEqual(ni.PROFILES_ADMISSION, 'ExpectedAdmission::FRESH_NETWORK')
        self.assertEqual((ni.PUBLIC_ADMISSION, ni.PUBLIC_APPROVAL), ('ExpectedAdmission::PUBLIC_MAINNET', 'OWNER-DECISIONS-2026-11-20 D1'))
        decl = '    const WRITTEN:&[NetworkProfile]=&['
        line = next(l for l in text.splitlines() if l.startswith(decl))
        self.assertTrue(line.endswith('];'))
        body = line[len(decl):-2]
        entries = entries_of(body)
        self.assertEqual(len(entries), 2)
        self.assertEqual([c in PUBLIC for c, _ in entries], [True, False], 'WRITTEN holds one entry of each form')
        self.assertEqual(body.count('},NetworkProfile{'), 1, 'entries separated by ","')
        self.assertEqual(ni.profiles_text(entries, PUBLIC), body)
        # The samples start from the file with an empty list, whatever the tree pins (after the switch it holds an entry).
        empty = ni.with_profiles(text, [], PUBLIC)
        self.assertEqual(empty.count(ni.PROFILES_DECL + ']'), 1)
        self.assertEqual(ni.with_profiles(empty, ni.read_profiles(text, PUBLIC), PUBLIC), text)
        sample = empty.replace(ni.PROFILES_DECL + ']', ni.PROFILES_DECL + body + ']')
        self.assertEqual(ni.read_profiles(sample, PUBLIC), entries)
        self.assertEqual(ni.read_profiles(ni.with_profiles(text, entries, PUBLIC), PUBLIC), entries)
        self.assertEqual(ni.with_profiles(ni.with_profiles(text, entries, PUBLIC), [], PUBLIC), empty)
        # Malformed entries, a duplicate, an entry that expects any admission or approval other than its chain's form
        # (the specification admission LAUNCH included, the other chain's form, another or a missing approval
        # reference), or names its fields in another order: each is refused.
        for bad in (body.replace('genesis_hash("0', 'genesis_hash("0x', 1), body.upper(), body + ',', body.replace('(', '( ', 1),
                    body.replace(str(entries[0][0]), f'{entries[0][0]:_}', 1), ni.profiles_text([entries[0], entries[0]], PUBLIC),
                    body.replace('ExpectedAdmission::FRESH_NETWORK', 'ExpectedAdmission::LAUNCH', 1),
                    body.replace('ExpectedAdmission::FRESH_NETWORK', 'ExpectedAdmission{waivers:&[],'
                                 '..ExpectedAdmission::FRESH_NETWORK}', 1),
                    body.replace('ExpectedAdmission::FRESH_NETWORK', 'FRESH_NETWORK', 1), body.replace('},', '}, ', 1),
                    body.replace('admission:ExpectedAdmission::FRESH_NETWORK', '', 1),
                    body.replace('admission:ExpectedAdmission::PUBLIC_MAINNET,approval:Some("OWNER-DECISIONS-2026-11-20 D1")',
                                 'admission:ExpectedAdmission::FRESH_NETWORK,approval:None', 1),
                    body.replace('admission:ExpectedAdmission::FRESH_NETWORK,approval:None',
                                 'admission:ExpectedAdmission::PUBLIC_MAINNET,approval:Some("OWNER-DECISIONS-2026-11-20 D1")', 1),
                    body.replace('approval:Some("OWNER-DECISIONS-2026-11-20 D1")', 'approval:None', 1),
                    body.replace('approval:None', 'approval:Some("OWNER-DECISIONS-2026-11-20 D1")', 1),
                    body.replace('OWNER-DECISIONS-2026-11-20 D1', 'OWNER-DECISIONS-2026-11-18 D1', 1),
                    body.replace(',approval:None', '', 1),
                    body.replace(f'chain_id:{entries[0][0]},genesis_hash:genesis_hash("{entries[0][1]}")',
                                 f'genesis_hash:genesis_hash("{entries[0][1]}"),chain_id:{entries[0][0]}', 1)):
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                ni.read_profiles(empty.replace(ni.PROFILES_DECL + ']', ni.PROFILES_DECL + bad + ']'), PUBLIC)
        with self.assertRaises(ValueError):
            ni.read_profiles(text.replace(ni.PROFILES_DECL, ni.PROFILES_DECL.replace('=&[', '= &[')), PUBLIC)

    def test_the_public_network_chains_are_the_mainnet_profiles_chain(self):
        # The tool reads PUBLIC_NETWORK_CHAINS (the R22 list the node's boot admission uses); it is exactly the chain of
        # the mainnet profile, never the rehearsal chain or a retired network.
        self.assertEqual(ni.public_chains(ni.ROOT), PUBLIC)
        self.assertEqual(PUBLIC, frozenset([MAINNET_CHAIN]))
        self.assertNotIn(REHEARSAL_CHAIN, PUBLIC)
        self.assertFalse(PUBLIC & {c for c, _ in RETIRED})
        root = self.copy()
        rel = root / ni.PUBLIC_CHAINS
        text = rel.read_text(encoding='utf-8')
        for bad in (text.replace(ni.PUBLIC_CHAINS_DECL, ni.PUBLIC_CHAINS_DECL + '0x'), text.replace(ni.PUBLIC_CHAINS_DECL, ''),
                    text + '\n' + ni.PUBLIC_CHAINS_DECL + '];\n'):
            rel.write_text(bad, encoding='utf-8')
            with self.subTest(bad=bad[:0]), self.assertRaises(SystemExit):
                ni.public_chains(root)

    def test_a_switch_to_a_public_network_chain_pins_the_public_admission_with_the_approval(self):
        # OWNER-DECISIONS-2026-11-20 D1, D4, D5: --to on the mainnet profile's chain writes the PUBLIC_MAINNET entry with
        # the approval reference, --check requires exactly it, and the experimental form for that chain is refused.
        root = self.copy()
        spec = make_spec(chain=MAINNET_CHAIN, version=2, flip=5)
        (root / 'mainnet.json').write_text(invitation(spec), encoding='utf-8')
        genesis = ni.keccak256(spec).hex()
        original = (root / ni.PROFILES).read_bytes()
        published = expects(root / 'mainnet.json')
        with self.assertRaises(SystemExit, msg='a public network chain needs the published values'):
            self.run_tool(root, target='mainnet.json')
        self.assertEqual(self.run_tool(root, *published, target='mainnet.json'), 0)
        self.assertEqual(profiles(root), [(MAINNET_CHAIN, genesis)])
        line = next(l for l in (root / ni.PROFILES).read_text(encoding='utf-8').splitlines() if l.startswith(ni.PROFILES_DECL))
        self.assertEqual(line, ni.PROFILES_DECL + f'NetworkProfile{{chain_id:{MAINNET_CHAIN},genesis_hash:genesis_hash("{genesis}"),'
                         'admission:ExpectedAdmission::PUBLIC_MAINNET,approval:Some("OWNER-DECISIONS-2026-11-20 D1")}];')
        self.assertEqual(self.run_tool(root, '--check', *published, target='mainnet.json'), 0)
        with self.assertRaises(SystemExit, msg='its check needs them too'):
            self.run_tool(root, '--check', target='mainnet.json')
        text = (root / ni.PROFILES).read_text(encoding='utf-8')
        fresh = line.replace('admission:ExpectedAdmission::PUBLIC_MAINNET,approval:Some("OWNER-DECISIONS-2026-11-20 D1")',
                             'admission:ExpectedAdmission::FRESH_NETWORK,approval:None')
        for bad in (fresh, line.replace('approval:Some("OWNER-DECISIONS-2026-11-20 D1")', 'approval:None')):
            with self.subTest(bad=bad):
                (root / ni.PROFILES).write_text(text.replace(line, bad), encoding='utf-8')
                self.assertEqual(self.run_tool(root, '--check', *published, target='mainnet.json'), 2)
        (root / ni.PROFILES).write_text(text, encoding='utf-8')
        # Switching on to another network replaces the entry with that network's own form.
        self.assertEqual(self.run_tool(root, source='mainnet.json', target='a.json'), 0)
        self.assertEqual(profiles(root), [(NEW_CHAIN, HASH_A[2:])])
        self.assertIn('admission:ExpectedAdmission::FRESH_NETWORK,approval:None', (root / ni.PROFILES).read_text(encoding='utf-8'))
        self.assertNotEqual((root / ni.PROFILES).read_bytes(), original)

    def test_a_switch_writes_the_network_profile_and_check_requires_exactly_it(self):
        root = self.copy()
        original = (root / ni.PROFILES).read_bytes()
        self.assertEqual(self.run_tool(root), 0)
        self.assertEqual(profiles(root), [(NEW_CHAIN, HASH_A[2:])], 'the --to entry')
        self.assertEqual(self.run_tool(root, '--check'), 0)
        switched = (root / ni.PROFILES).read_bytes()
        rest = lambda data: re.sub(rb'(?m)^pub const NETWORK_PROFILES:.*$', b'', data)
        self.assertEqual(rest(switched), rest(original), 'only the NETWORK_PROFILES line changes')
        text = switched.decode('utf-8')
        # A missing entry, an extra one, another network's in its place, or a duplicate: --check refuses each.
        other = (NEW_CHAIN + 1, 'cd' * 32)
        for entries in ([], [(NEW_CHAIN, HASH_A[2:]), other], [other], [(NEW_CHAIN, HASH_B[2:])],
                        [(NEW_CHAIN, HASH_A[2:]), (NEW_CHAIN, HASH_A[2:])], [(CUR_CHAIN, CUR_GENESIS)]):
            with self.subTest(entries=entries):
                (root / ni.PROFILES).write_text(text.replace(ni.PROFILES_DECL + ni.profiles_text([(NEW_CHAIN, HASH_A[2:])], PUBLIC),
                                                            ni.PROFILES_DECL + ni.profiles_text(entries, PUBLIC)), encoding='utf-8')
                self.assertNotEqual((root / ni.PROFILES).read_bytes(), switched)
                self.assertEqual(self.run_tool(root, '--check'), 2)
        # An entry of another network is refused by a switch too, which then writes nothing.
        (root / ni.PROFILES).write_text(text.replace(ni.PROFILES_DECL + ni.profiles_text([(NEW_CHAIN, HASH_A[2:])], PUBLIC),
                                                    ni.PROFILES_DECL + ni.profiles_text([(NEW_CHAIN, HASH_A[2:]), other], PUBLIC)), encoding='utf-8')
        before = self.snapshot(root)
        self.assertEqual(self.run_tool(root, source='a.json', target='b.json'), 2)
        self.assertEqual(before, self.snapshot(root))
        # A missing entry is restored by finishing the switch (the tree pins --to already).
        (root / ni.PROFILES).write_text(text.replace(ni.PROFILES_DECL + ni.profiles_text([(NEW_CHAIN, HASH_A[2:])], PUBLIC),
                                                    ni.PROFILES_DECL), encoding='utf-8')
        self.assertEqual(self.run_tool(root), 0)
        self.assertEqual((root / ni.PROFILES).read_bytes(), switched)
        # A redo replaces the --from entry with the --to one.
        self.assertEqual(self.run_tool(root, source='a.json', target='b.json'), 0)
        self.assertEqual(profiles(root), [(NEW_CHAIN, HASH_B[2:])])
        self.assertEqual(self.run_tool(root, '--check', source='a.json', target='b.json'), 0)

    def test_the_rehearsal_and_retired_networks_get_no_network_profile(self):
        root = self.copy()
        (root / 'rehearsal.json').write_text(invitation(make_spec(chain=REHEARSAL_CHAIN, version=2, flip=4)), encoding='utf-8')
        self.assertEqual(self.run_tool(root, target='rehearsal.json'), 0)
        self.assertEqual(ni.current_identity(root)[0], REHEARSAL_CHAIN, 'the clients pin the rehearsal network')
        self.assertEqual(profiles(root), [], 'the rehearsal network is never a launch network')
        self.assertEqual(self.run_tool(root, '--check', target='rehearsal.json'), 0)
        rehearsal_genesis = ni.current_identity(root)[1]
        text = (root / ni.PROFILES).read_text(encoding='utf-8')
        (root / ni.PROFILES).write_text(text.replace(ni.PROFILES_DECL, ni.PROFILES_DECL + ni.profiles_text([(REHEARSAL_CHAIN, rehearsal_genesis)], PUBLIC)), encoding='utf-8')
        self.assertEqual(self.run_tool(root, '--check', target='rehearsal.json'), 2, 'an entry for it is refused')
        (root / ni.PROFILES).write_text(text, encoding='utf-8')
        # From the rehearsal network to a launch network, and back on purpose: the retired network gets no entry.
        self.assertEqual(self.run_tool(root, source='rehearsal.json', target='v3.json'), 0)
        self.assertEqual(profiles(root), [(NEW_CHAIN, HASH_V3[2:])])
        self.assertEqual(self.run_tool(root, '--allow-revert', source='v3.json', target='rehearsal.json'), 0)
        self.assertEqual(profiles(root), [])
        self.assertEqual((root / ni.PROFILES).read_text(encoding='utf-8'), text)

    def test_a_round_trip_restores_network_profiles_byte_for_byte(self):
        root = self.copy()
        original = (root / ni.PROFILES).read_bytes()
        self.assertEqual(self.run_tool(root), 0)
        self.assertEqual(self.run_tool(root, source='a.json', target='b.json'), 0)
        self.assertNotEqual((root / ni.PROFILES).read_bytes(), original)
        self.assertEqual(self.run_tool(root, '--allow-revert', *self.published(root, 'cur.json'), source='b.json', target='cur.json'), 0)
        if SWITCHED:
            # The reverted-to network was retired by the first switch: its entry is not written back.
            self.assertEqual(profiles(root), [])
        else:
            self.assertEqual((root / ni.PROFILES).read_bytes(), original)
        self.assertEqual(self.run_tool(root, '--check', '--allow-revert', *self.published(root, 'cur.json'), source='b.json', target='cur.json'), 0)

    def test_the_guard_flags_planted_copies(self):
        planted = [('a.py', f'X = {CUR_CHAIN}n'), ('b.rs', f'const X: u64 = {CUR_CHAIN:_};'), ('c.js', f"c='0x{CUR_CHAIN:X}'"),
                   ('d.md', f'genesis {CUR_GENESIS[:8]}...'), ('e.json', CUR_SPEC.hex())]
        self.assertEqual(unaccounted(planted), [rel for rel, _ in planted])
        exempt = [('development/x.md', str(CUR_CHAIN)), ('deploy/websites/x.js', str(CUR_CHAIN)), (list(ni.FILES)[0], str(CUR_CHAIN)),
                  (ni.RETIRED, str(CUR_CHAIN))]
        self.assertEqual(unaccounted(exempt), [])
        # In readiness.rs only the NETWORK_PROFILES entry is exempt; a copy anywhere else in that file is flagged.
        entry = ni.PROFILES_DECL + ni.profiles_text([(CUR_CHAIN, CUR_GENESIS)], PUBLIC) + '];\n'
        self.assertEqual(unaccounted([(ni.PROFILES, entry)]), [])
        # A stray chain ID or genesis hash outside readiness.rs's test module is flagged whatever the tree pins.
        for stray in (f'// chain {CUR_CHAIN}\n', f'const G:&str="{CUR_GENESIS}";\n', f'const G:&str="0x{CUR_GENESIS.upper()}";\n'):
            with self.subTest(stray=stray):
                self.assertEqual(unaccounted([(ni.PROFILES, entry + stray)]), [ni.PROFILES])
                self.assertEqual(unaccounted([(ni.PROFILES, ni.PROFILES_DECL + '];\n' + stray)]), [ni.PROFILES])
                tests = '#[cfg(test)]\nmod tests{\n    const PUBLIC:u64=1;\n}\n'
                self.assertEqual(unaccounted([(ni.PROFILES, entry + stray + tests)]), [ni.PROFILES])
        # readiness.rs names the public network chain in its own tests (MAINNET_NAMED), so once the tree pins that chain
        # its chain ID inside the #[cfg(test)] module is not taken for a pin; a genesis hash there is flagged.
        inside = entry + f'#[cfg(test)]\nmod tests{{\n    const PUBLIC:u64={CUR_CHAIN};\n}}\n'
        named = set(MAINNET_NAMED)
        self.assertEqual(unaccounted([(ni.PROFILES, inside)], named=named), [])
        self.assertEqual(unaccounted([(ni.PROFILES, inside)], named=set()), [ni.PROFILES])
        self.assertEqual(unaccounted([(ni.PROFILES, inside.replace('const PUBLIC:u64=', f'const G:&str="{CUR_GENESIS}";const PUBLIC:u64='))],
                                     named=named), [ni.PROFILES])
        self.assertEqual(unaccounted([(ni.PROFILES, entry + f'// chain {CUR_CHAIN}\n' + inside[len(entry):])], named=named), [ni.PROFILES])
        self.assertEqual(outside_tests('a\n#[cfg(test)]\nmod tests {\nb'), 'a\n')
        self.assertEqual(outside_tests('a\n'), 'a\n')
        # In a file outside FILES and outside MAINNET_NAMED the current chain ID alone is a pin, whatever the tree pins.
        self.assertEqual(unaccounted([('z.rs', f'// chain {CUR_CHAIN}\n')]), ['z.rs'])
        # A retired identity the tree no longer pins: flagged everywhere outside the kept files, FILES included.
        gone_chain, gone_genesis = NEW_CHAIN, HASH_A[2:]
        gone = anywhere([(gone_chain, gone_genesis)])
        planted = [('a.py', f'X = {gone_chain}n'), ('b.rs', f'const X: u64 = {gone_chain:_};'), ('c.js', f"c='0x{gone_chain:X}'"),
                   ('d.md', f'genesis {gone_genesis[:8].upper()}...'), (list(ni.FILES)[0], f'chain {gone_chain}')]
        self.assertEqual(unaccounted(planted, gone=gone), [rel for rel, _ in planted])
        exempt = [('development/x.md', str(gone_chain)), ('deploy/websites/x.js', str(gone_chain)), (ni.RETIRED, str(gone_chain)),
                  ('e.md', 'no identity')]
        self.assertEqual(unaccounted(exempt, gone=gone), [])
        # A retired genesis of the current chain (a redo with another genesis for the same chain): its hash is flagged,
        # the current network's chain ID is not.
        gone = anywhere([(CUR_CHAIN, gone_genesis)], keep_chain=CUR_CHAIN)
        planted = [('d.md', f'genesis {gone_genesis[:8]}...'), (list(ni.FILES)[0], f'genesis 0x{gone_genesis.upper()}')]
        self.assertEqual(unaccounted(planted, gone=gone), [rel for rel, _ in planted])
        exempt = [(list(ni.FILES)[0], f'chain {CUR_CHAIN} = 0x{CUR_CHAIN:x} = {CUR_CHAIN:_}')]
        self.assertEqual(unaccounted(exempt, gone=gone), [])

    def test_a_same_chain_redo_leaves_no_pin_the_guard_flags(self):
        """A postponed launch keeps the chain ID and replaces the genesis: the switch retires the old genesis, and the
        switched tree passes the guard (its chain ID pins are the current network's, not the retired one's)."""
        root = self.copy()
        (root / 'same.json').write_text(invitation(make_spec(chain=CUR_CHAIN, flip=1)), encoding='utf-8')
        self.assertEqual(self.run_tool(root, *self.published(root, 'same.json'), target='same.json'), 0)
        chain, genesis, _ = ni.current_identity(root)
        self.assertEqual(chain, CUR_CHAIN)
        retired = ni.read_retired((root / ni.RETIRED).read_bytes())   # (chain ID, genesis hex without 0x)
        self.assertIn((CUR_CHAIN, CUR_GENESIS), retired)
        gone = anywhere([r for r in retired if r != (chain, genesis)], keep_chain=chain)
        files = [(rel, (root / rel).read_text(encoding='utf-8')) for rel in TRACKED]
        self.assertEqual(unaccounted(files, current=anywhere([(chain, genesis)]), gone=gone), [])
        self.assertEqual(self.run_tool(root, '--check', *self.published(root, 'same.json'), target='same.json'), 0)

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

    def test_every_tracked_file_that_names_the_public_network_chain_is_listed(self):
        # ID-04: the files outside FILES that name the public network's chain ID are exactly MAINNET_NAMED, whatever the
        # tree pins, so the guard above passes after a switch to that chain without a hand edit, and a new mention (a
        # client pin the switch would miss included) fails here first.
        try:
            listed = subprocess.run(['git', '-C', str(ni.ROOT), 'ls-files', '-z'], capture_output=True, check=True).stdout
        except (OSError, subprocess.CalledProcessError):
            self.skipTest('not a git checkout')
        self.assertIn(MAINNET_CHAIN, PUBLIC)
        found = set()
        for rel in filter(None, listed.decode('utf-8').split('\0')):
            if rel.startswith(tuple(KEPT_PREFIXES)) or rel in KEPT or rel in ni.FILES:
                continue
            try:
                text = (ni.ROOT / rel).read_text(encoding='utf-8')
            except (UnicodeDecodeError, OSError):
                continue
            if MAINNET_ANYWHERE.search(without_profiles(rel, text)):
                found.add(rel)
        self.assertEqual(sorted(found - set(MAINNET_NAMED)), [], 'names the public network chain: add it to FILES (a client pin) '
                         'or to MAINNET_NAMED with a reason')
        # The number of mentions in each listed file is pinned too, so a new one inside a listed file is reviewed here.
        counted = {rel: len(MAINNET_ANYWHERE.findall(without_profiles(rel, (ni.ROOT / rel).read_text(encoding='utf-8'))))
                   for rel in sorted(found & set(MAINNET_NAMED))}
        self.assertEqual({rel: n for rel, n in counted.items() if n != MAINNET_NAMED[rel][0]}, {},
                         'names the public network chain more or fewer times than MAINNET_NAMED pins: review the mentions '
                         'and update the count')
        self.assertTrue(all(isinstance(n, int) and n > 0 and reason for n, reason in MAINNET_NAMED.values()))
        self.assertEqual(sorted(set(MAINNET_NAMED) - found), [], 'no longer names the public network chain: remove it from MAINNET_NAMED')
        self.assertFalse(set(MAINNET_NAMED) & (set(ni.FILES) | set(KEPT)))
        # The exemption covers only the current network on the public chain: a retired identity in a named file, and a
        # copy of the current identity in a named file while the tree pins another chain, are still flagged.
        rel = sorted(MAINNET_NAMED)[0]
        self.assertEqual(unaccounted([(rel, f'chain {MAINNET_CHAIN}')], current=chain_anywhere(MAINNET_CHAIN), named=set(MAINNET_NAMED)), [])
        self.assertEqual(unaccounted([(rel, f'chain {MAINNET_CHAIN}')], current=chain_anywhere(MAINNET_CHAIN), named=set()), [rel])
        self.assertEqual(unaccounted([(rel, f'chain {NEW_CHAIN}')], gone=anywhere([(NEW_CHAIN, HASH_A[2:])]), named=set(MAINNET_NAMED)), [rel])
        # A named file holding the genesis hash of the network on that chain is flagged: it names the chain, never pins it.
        genesis = HASH_A[2:]
        self.assertEqual(unaccounted([(rel, f'genesis 0x{genesis}')], current=anywhere([(MAINNET_CHAIN, genesis)]), named=set(MAINNET_NAMED),
                                     current_genesis=anywhere([(MAINNET_CHAIN, genesis)], keep_chain=MAINNET_CHAIN)), [rel])

    def test_the_retired_release_texts_go_with_the_switch(self):
        held = [rel for rel, old in STEP3.items() if old.search((ni.ROOT / rel).read_text(encoding='utf-8'))]
        if SWITCHED:
            self.assertEqual(held, [], 'the identity-switch release replaces or bumps these: ' + ', '.join(held))
        else:
            self.assertEqual(held, list(STEP3), 'update STEP3 to what the files hold today')
        # The retired extension link is gone already, switched or not; the detector works on the retired release's text.
        rel, old = RETIRED_EXTENSION_LINK
        self.assertIsNone(old.search((ni.ROOT / rel).read_text(encoding='utf-8')), rel)
        self.assertIsNotNone(old.search('PoCol-Studio-IDE-Viewer-0.4.1.zip'))

    def test_the_documents_no_longer_describe_the_tree_before_the_switch(self):
        held = {rel: rx.search(' '.join((ni.ROOT / rel).read_text(encoding='utf-8').split()))
                for rel, rx in SUPERSEDED_AFTER_SWITCH.items()}
        if SWITCHED:
            self.assertEqual({rel: m.group(0) for rel, m in held.items() if m}, {},
                             'the tree pins a network that is not retired: these texts still describe the tree before the switch')
        # The detector works: each pattern finds the pre-switch text it was written for.
        for rel, sample in (
                ('README.md', "Until the new genesis exists, that pin is the retired network's; the clients are switched"),
                ('deploy/mainnet/README.md', "Until the new genesis exists, the extension's built-in profile pins no retired network"),
                ('deploy/mainnet/README.md', 'so no release ships it (CI checks the refusal instead of packaging).'),
                ('deploy/mainnet/README.md', "The extension READMEs keep the tree's identity until the switch."),
                ('development/public-network/FINDINGS.md', '**No launch network is pinned in the clients yet.** NETWORK_PROFILES'),
                ('development/public-network/FINDINGS.md', 'preimage; until the identity switch the source holds the D1 placeholder, F-57)'),
                ('development/public-network/FINDINGS.md', '**Client texts, the Studio gateway and the websites carry the retired '
                 'network identity until the identity switch.** Until the identity switch (set_network_identity.py')):
            self.assertRegex(sample, SUPERSEDED_AFTER_SWITCH[rel], rel)
        # The history that replaced them is not taken for a pre-switch claim.
        for rel, sample in (('deploy/mainnet/README.md', 'Before step 7, while the new genesis did not exist, the extension'),
                            ('development/public-network/FINDINGS.md', "**The websites still link the retired network's native "
                             'packages and invitation; the client pins moved to PoCol Mainnet at step 7.**')):
            self.assertNotRegex(sample, SUPERSEDED_AFTER_SWITCH[rel], rel)

    def test_a_spec2_genesis_without_the_full_nonce_domain_is_refused(self):
        # ID-03: the release node admits a spec-2 genesis only with nonceMode 1 and the extension's profile check refuses
        # any other, so the tool refuses it before writing anything (it accepted one before this check).
        bounded = make_spec(version=2, flip=6, nonce_mode=0)
        self.assertEqual(ni.pocol_identity.decode_genesis_spec(bounded)['cp'][ni.NONCE_MODE], 0, 'the reference decoder alone accepts it')
        for allowed in ((2,), (1, 2)):
            with self.subTest(allowed=allowed), self.assertRaisesRegex(ValueError, 'nonceMode 0'):
                ni.spec_identity(bounded, allowed)
        self.assertEqual(ni.spec_identity(make_spec(version=2, flip=6)), (2, NEW_CHAIN), 'nonceMode 1 is accepted')
        root = self.copy()
        for name, chain, spec in (('bounded.json', NEW_CHAIN, bounded),
                                  ('bounded-mainnet.json', MAINNET_CHAIN, make_spec(chain=MAINNET_CHAIN, version=2, flip=6, nonce_mode=0))):
            (root / name).write_text(invitation(spec, {'chainId': chain, 'genesisHash': '0x' + ni.keccak256(spec).hex()}), encoding='utf-8')
            before = self.snapshot(root)
            for extra in ((), ('--check',)):
                with self.subTest(name=name, extra=extra), self.assertRaises(SystemExit):
                    self.run_tool(root, *extra, *expects(root / name), target=name)
            self.assertEqual(before, self.snapshot(root))

    def test_a_switch_is_checked_against_the_published_values(self):
        # ID-03: --expect-sha256, --expect-chain-id and --expect-genesis are required on a public network chain and must
        # match whenever they are given, so a wrong or tampered invitation never re-pins the clients.
        root = self.copy()
        spec = make_spec(chain=MAINNET_CHAIN, version=2, flip=7)
        (root / 'mainnet.json').write_text(invitation(spec), encoding='utf-8')
        good = expects(root / 'mainnet.json')
        other = make_spec(chain=MAINNET_CHAIN, version=2, flip=8)
        # The same network re-serialised (another file SHA-256, the same proven identity) and another genesis of the chain.
        doc = json.loads((root / 'mainnet.json').read_text(encoding='utf-8'))
        (root / 'reformatted.json').write_text(json.dumps(doc, indent=2), encoding='utf-8')
        (root / 'other.json').write_text(invitation(other), encoding='utf-8')
        def without(flag):
            i = good.index(flag)
            return good[:i] + good[i + 2:]
        def replaced(flag, value):
            i = good.index(flag)
            return good[:i + 1] + [value] + good[i + 2:]
        before = self.snapshot(root)
        bad = [(target, args) for target, args in (
            ('mainnet.json', []), ('mainnet.json', without('--expect-sha256')), ('mainnet.json', without('--expect-chain-id')),
            ('mainnet.json', without('--expect-genesis')), ('mainnet.json', replaced('--expect-sha256', 'ab' * 32)),
            ('mainnet.json', replaced('--expect-sha256', good[1].upper()[:-1] + 'G')),
            ('mainnet.json', replaced('--expect-chain-id', str(MAINNET_CHAIN + 1))),
            ('mainnet.json', replaced('--expect-chain-id', f'{MAINNET_CHAIN:_}')),
            ('mainnet.json', replaced('--expect-genesis', '0x' + ni.keccak256(other).hex())),
            ('mainnet.json', replaced('--expect-genesis', good[5][2:])),
            ('reformatted.json', good), ('other.json', good))]
        for target, args in bad:
            for extra in ((), ('--check',)):
                with self.subTest(target=target, args=args, extra=extra), self.assertRaises(SystemExit):
                    self.run_tool(root, *extra, *args, target=target)
        self.assertEqual(before, self.snapshot(root), 'a refused switch writes nothing')
        # Upper-case values are the same values; the switch runs and its check passes.
        upper = replaced('--expect-sha256', good[1].upper())
        upper = upper[:5] + ['0x' + good[5][2:].upper()]
        self.assertEqual(self.run_tool(root, *upper, target='mainnet.json'), 0)
        self.assertEqual(self.run_tool(root, '--check', *good, target='mainnet.json'), 0)
        # Off a public chain the values are optional, and checked when given.
        root = self.copy()
        with self.assertRaises(SystemExit):
            self.run_tool(root, '--expect-chain-id', str(NEW_CHAIN + 1))
        with self.assertRaises(SystemExit):
            self.run_tool(root, '--expect-genesis', HASH_B)
        self.assertEqual(self.run_tool(root, '--expect-chain-id', str(NEW_CHAIN), '--expect-genesis', HASH_A), 0)
        # --placeholder takes no expectation.
        with self.assertRaises(SystemExit):
            ni.main(['--placeholder', str(root / 'ph.json'), '--root', str(root), *good])

if __name__ == '__main__':
    unittest.main()
