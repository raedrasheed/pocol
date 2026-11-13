#!/usr/bin/env python3
"""Switches every client in the repository to the new network's identity (chain ID, genesis hash, GenesisSpec).

The earn-in network's genesis exists only after `pocold cutover` has run at the agreed final height S
(development/decisions/EARN-IN-SPEC.md section 7), so the clients are switched in one reviewed step,
with the public genesis.spec.hex that the cutover wrote:

    python deploy/mainnet/network_identity.py --genesis-hash 0x<64 hex from cutover-report.json> \\
        --genesis-spec /var/tmp/pocol-earnin-genesis/genesis.spec.hex
    python deploy/mainnet/network_identity.py --genesis-hash 0x... --genesis-spec ... --check   # verify only

The given GenesisSpec must be canonical RLP throughout, of version 1 with six top items and the 52 CP items of
CP schema v1, hash (keccak256) to --genesis-hash and carry --chain-id; the extension and the node pin the whole
spec, which proves the other two. A spec-2 (v3) genesis (CP schema v2, 67 CP items) is switched to only with an
explicit --v3: the release node and clients refuse spec 2 until activation (V3-PR-I), so the switch is for a tree
whose node and clients are built for it. The tree's
current identity is the one the node file ANCHOR names, proven by its own GenesisSpec. Before and after the
switch every listed file must carry exactly its FILES counts of the identity and no copy of the old Mainnet
identity, so --check fails on a tree pinned to any other genesis, and a redo with a new genesis for the
same chain (a postponed launch) switches from the current one. The Mainnet identity is never a target: a
rollback reverts the switch commit.

Derived files move with their sources: a page that loads a listed file by a content version (`?v=` and
the first 12 hex digits of the file's SHA-256) gets the new version, and the Studio gateway installer is
rebuilt from the switched gateway sources. Those hashed sources must have the LF line endings git stores,
so a checkout that converts line endings (core.autocrlf) is refused. Files are read and written as bytes
(line endings are kept), and nothing is written unless every file passes: every new file is written beside
its original first, then each original is replaced whole, the node file last. Documents about the old
network's history and the website snapshots in deploy/websites (build output) are not
changed. Nothing is deployed.
"""
import argparse, hashlib, json, os, re, shutil, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'deploy' / 'rewards'))
sys.path.insert(0, str(ROOT / 'deploy' / 'studio'))
from pocol_rewards import keccak256  # noqa: E402  (the keccak256 the reward accounting uses)
import build_installer  # noqa: E402

OLD_CHAIN_ID = 1599846715   # PoCol Mainnet before the cutover; never a target
OLD_GENESIS = 'be137bfdb4fdf332f2e1500ee1a67e0b22ce6bdf57efa46fb4ea7b0debe509ca'
# The published Mainnet GenesisSpec (keccak256 = OLD_GENESIS), from the H15000 invitation.
OLD_INVITATION = ROOT / 'deploy' / 'mainnet' / 'h15000' / 'network.pocol.json'
# sha256(b'PoCol earn-in network 2026')[:4] mod (2^31 - 2^20) + 2^20; not listed on chainid.network.
NEW_CHAIN_ID = 261597237
# The GenesisSpec version the release node and clients decode (genesis_spec.rs and refdec.js SPEC_VERSION), the v3
# genesis version (accepted only with --v3), and the CP item count of each version's schema (CP_LEN, CP_V2_LEN).
SPEC_VERSION = 1
V3_SPEC_VERSION = 2
CP_ITEMS = {1: 52, 2: 67}
# The node's network designation names the tree's current identity: MAINNET_CHAIN_ID, MAINNET_GENESIS,
# and the published GenesisSpec in its test.
ANCHOR = 'prototypes/pocold/src/network_display.rs'
# Each listed file and the copies of the identity it pins: (chain ID in any spelling, genesis hash,
# GenesisSpec). Code, tests, and the documents that describe the current network and ship with it.
FILES = {
    'prototypes/pocold/src/network_display.rs': (1, 1, 1),
    'prototypes/pocol-extension/src/lib/studio-network.js': (1, 1, 1),
    'prototypes/pocol-extension/src/contract.html': (1, 0, 0),
    'prototypes/pocol-extension/src/publisher.html': (1, 0, 0),
    'prototypes/pocol-extension/tests/studio_test.mjs': (3, 0, 0),
    'prototypes/pocol-extension/tests/publisher_test.mjs': (1, 0, 0),
    'prototypes/pocol-extension/tests/contract_ui_test.mjs': (3, 0, 0),
    'prototypes/pocol-extension/tests/contracts_test.mjs': (1, 0, 0),
    'prototypes/pocol-extension/README-AR.md': (1, 0, 0),
    'prototypes/pocol-extension/README-IDE.md': (1, 1, 0),
    'prototypes/pocol-extension/README-STUDIO.md': (1, 1, 0),
    'prototypes/pocol-studio/dist/app.js': (3, 1, 0),
    'prototypes/pocol-studio/README.md': (1, 1, 0),
    'prototypes/pocol-windows/WalletUiTests.cs': (1, 0, 0),
    'deploy/rewards/pocol_rewards.py': (1, 1, 0),
    'deploy/rewards/gateway.py': (1, 1, 0),
    'deploy/rewards/test_pocol_rewards.py': (1, 1, 0),
    'deploy/standalone/package.py': (1, 1, 0),
    'deploy/studio/install_studio_gateway.py': (3, 2, 0),
    'deploy/studio/pocol_studio_gateway.py': (1, 1, 0),
    'deploy/studio/README.md': (1, 0, 0),
    'deploy/studio/README-AR.txt': (1, 0, 0),
    'README.md': (1, 1, 0),
}
# (page, listed file): the page loads the file as `<name>?v=<first 12 hex of its SHA-256>`, as the
# deployed Studio does, so a cache never serves the old content under the new version.
VERSIONED = [
    ('prototypes/pocol-studio/dist/index.html', 'prototypes/pocol-studio/dist/app.js'),
]
# Built by deploy/studio/build_installer.py from the Studio gateway sources, which it embeds.
INSTALLER = 'deploy/studio/Install-PoCol-Studio-Gateway.sh'
INSTALLER_SOURCES = {name: f'deploy/studio/{name}' for name in build_installer.files}

def tokens(chain_id, genesis):
    """Every spelling the files use: decimal, Rust digit groups, 0x hex, and the hash with or without 0x."""
    return [str(chain_id), f'{chain_id:_}', hex(chain_id), genesis]

def token(t):
    """A whole token, also as a JavaScript BigInt literal (`1599846715n`)."""
    return rf'(?<![0-9A-Za-z_]){re.escape(t)}(?=n?(?![0-9A-Za-z_]))'

def hexrun(h):
    """A hash or GenesisSpec as a whole hex run (it may follow 0x, never sits inside longer hex)."""
    return rf'(?<![0-9A-Fa-f]){h}(?![0-9A-Fa-f])'

def counts(text, ident, spec_hex):
    """(chain ID tokens, genesis hash copies, GenesisSpec copies) of one identity in text."""
    chain = sum(len(re.findall(token(t), text, re.IGNORECASE)) for t in ident[:3])
    return chain, len(re.findall(hexrun(ident[3]), text, re.IGNORECASE)), len(re.findall(hexrun(spec_hex), text, re.IGNORECASE))

def rewrite(text, cur, new, cur_spec, new_spec):
    # The GenesisSpec first (its bytes carry the chain ID), then the genesis hash, then the chain ID spellings.
    out = re.sub(hexrun(cur_spec), new_spec, text, flags=re.IGNORECASE)
    out = re.sub(hexrun(cur[3]), new[3], out, flags=re.IGNORECASE)
    for c, n in zip(cur[:3], new[:3]):
        out = re.sub(token(c), n, out, flags=re.IGNORECASE)
    return out

def version(data):
    return hashlib.sha256(data).hexdigest()[:12]

def read_spec(text):
    """GenesisSpec bytes from genesis.spec.hex text (optional 0x, surrounding whitespace)."""
    h = text.strip()
    h = h[2:] if h[:2].lower() == '0x' else h
    if not h or len(h) % 2 or not re.fullmatch(r'[0-9a-fA-F]+', h):
        raise ValueError('the genesis spec must be hex bytes')
    return bytes.fromhex(h)

def rlp_item(b, i):
    """(is list, payload start, payload end) of the canonical RLP item at i; ValueError otherwise."""
    p = b[i]
    if p < 0x80:
        return False, i, i + 1
    is_list = p >= 0xc0
    short = p - (0xc0 if is_list else 0x80)
    if short <= 55:
        s, n = i + 1, short
        if not is_list and n == 1 and b[s] < 0x80:
            raise ValueError('a single byte below 0x80 must not be wrapped')
    else:
        k = short - 55
        if b[i + 1] == 0:
            raise ValueError('leading zero in an RLP length')
        s, n = i + 1 + k, int.from_bytes(b[i + 1:i + 1 + k], 'big')
        if n <= 55:
            raise ValueError('long form for a short RLP item')
    if s + n > len(b):
        raise ValueError('truncated RLP')
    return is_list, s, s + n

def rlp_walk(b, start, end):
    """Checks that b[start:end] is a sequence of canonical RLP items, nested lists included."""
    i = start
    while i < end:
        is_list, s, e = rlp_item(b, i)
        if e > end:
            raise ValueError('an RLP item overruns its list')
        if is_list:
            rlp_walk(b, s, e)
        i = e

def spec_identity(b, versions=(SPEC_VERSION,)):
    """(specVersion, chainId) of a GenesisSpec [version, chainId, CP, allocRoot, M_0List, sysCodeHash], after
    checking that the whole spec is one canonical RLP list of six items, that the version is one of `versions` (the
    release version 1 by default; 2 is the v3 genesis), that CP is a list of that version's CP item count, and that
    the chainId integer has no leading zero."""
    try:
        is_list, start, end = rlp_item(b, 0)
        if not is_list or end != len(b):
            raise ValueError('not one RLP list')
        rlp_walk(b, start, end)
        is_list, vs, ve = rlp_item(b, start)
        version = b[vs] if not is_list and ve - vs == 1 and b[vs] in CP_ITEMS else None
        if version not in versions:
            if version == V3_SPEC_VERSION:
                raise ValueError('version 2 (a v3 genesis) is refused by the release node and clients until activation; '
                                 'pass --v3 only for a tree built for it')
            raise ValueError(f"version is not {' or '.join(map(str, versions))}")
        is_list, s, e = rlp_item(b, ve)
        if is_list or s == e or b[s] == 0:
            raise ValueError('chainId is not a canonical integer')
        items, i = [], start
        while i < end:
            is_list, cs, ce = rlp_item(b, i)
            items.append((is_list, cs, ce))
            i = ce
        if len(items) != 6:
            raise ValueError('a GenesisSpec has six top items')
        cp_list, cs, ce = items[2]
        count, j = 0, cs
        while cp_list and j < ce:
            j = rlp_item(b, j)[2]
            count += 1
        if not cp_list or count != CP_ITEMS[version]:
            raise ValueError(f'CP of version {version} has {CP_ITEMS[version]} items')
    except IndexError:
        raise ValueError('truncated RLP') from None
    return version, int.from_bytes(b[s:e], 'big')

def spec_chain_id(b, versions=(SPEC_VERSION,)):
    """chainId of a GenesisSpec of one of `versions` (spec_identity)."""
    return spec_identity(b, versions)[1]

def old_spec():
    """The published Mainnet GenesisSpec, checked against the old identity."""
    spec = read_spec(json.loads(OLD_INVITATION.read_text(encoding='utf-8'))['files']['genesis.spec.hex'])
    if keccak256(spec).hex() != OLD_GENESIS or spec_chain_id(spec) != OLD_CHAIN_ID:
        raise SystemExit(f'{OLD_INVITATION}: not the published Mainnet genesis')
    return spec

def current_identity(root, versions=(SPEC_VERSION,)):
    """The tree's identity as ANCHOR names it, proven by the GenesisSpec it pins (of one of `versions`)."""
    text = (root / ANCHOR).read_text(encoding='utf-8')
    chain = re.search(r'const MAINNET_CHAIN_ID: u64 = ([0-9_]+);', text)
    genesis = re.search(r'const MAINNET_GENESIS: &str = "([0-9a-fA-F]{64})";', text)
    spec = re.search(r'let genesis = "0x([0-9a-fA-F]+)";', text)
    if not (chain and genesis and spec):
        raise SystemExit(f'{ANCHOR}: the current chain ID, genesis hash or GenesisSpec is not readable')
    chain_id, genesis, spec = int(chain.group(1).replace('_', '')), genesis.group(1).lower(), bytes.fromhex(spec.group(1))
    try:
        proven = keccak256(spec).hex() == genesis and spec_chain_id(spec, versions) == chain_id
    except ValueError:
        proven = False
    if not proven:
        raise SystemExit(f'{ANCHOR}: its GenesisSpec does not prove its chain ID and genesis hash'
                         + ('' if V3_SPEC_VERSION in versions else ' (a tree on a v3 genesis is switched only with --v3)'))
    return chain_id, genesis, spec

def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('--genesis-hash', required=True, help='new genesis hash, 0x + 64 hex digits')
    p.add_argument('--genesis-spec', required=True, type=Path,
                   help="the new network's genesis.spec.hex, written by `pocold cutover` (public)")
    p.add_argument('--chain-id', type=int, default=NEW_CHAIN_ID)
    p.add_argument('--check', action='store_true', help='verify that the tree carries exactly this identity')
    p.add_argument('--v3', action='store_true',
                   help='the given GenesisSpec is a spec-2 (v3) genesis; the release node and clients refuse spec 2 until '
                        'activation, so use this only for a tree built for it (the current one may be of either version)')
    p.add_argument('--root', type=Path, default=ROOT)
    a = p.parse_args(argv)
    if not re.fullmatch(r'0x[0-9a-fA-F]{64}', a.genesis_hash):
        p.error('--genesis-hash must be 0x followed by 64 hex digits')
    genesis = a.genesis_hash[2:].lower()
    if a.chain_id in (0, OLD_CHAIN_ID, 777001) or not 0 < a.chain_id < 2**53 or genesis == OLD_GENESIS:
        p.error('the new identity must differ from the old network and be a valid chain ID')
    try:
        spec = read_spec(a.genesis_spec.read_text(encoding='utf-8'))
        spec_chain = spec_chain_id(spec, (V3_SPEC_VERSION,) if a.v3 else (SPEC_VERSION,))
    except (OSError, UnicodeDecodeError, ValueError) as e:
        p.error(f'--genesis-spec: {e}' + (' (--v3 takes a spec-2 genesis only)' if a.v3 and 'version is not' in str(e) else ''))
    if keccak256(spec).hex() != genesis:
        p.error('--genesis-spec does not hash (keccak256) to --genesis-hash')
    if spec_chain != a.chain_id:
        p.error(f'--genesis-spec carries chain ID {spec_chain}, not {a.chain_id}')
    cur_chain, cur_genesis, cur_spec = current_identity(a.root, (SPEC_VERSION, V3_SPEC_VERSION) if a.v3 else (SPEC_VERSION,))
    cur, new = tokens(cur_chain, cur_genesis), tokens(a.chain_id, genesis)
    mainnet, mainnet_spec = tokens(OLD_CHAIN_ID, OLD_GENESIS), old_spec().hex()
    same = (cur_chain, cur_genesis) == (a.chain_id, genesis)
    problems, data, out, stale = [], {}, {}, False
    for rel, expected in FILES.items():
        data[rel] = (a.root / rel).read_bytes()
        text = data[rel].decode('utf-8')
        held = counts(text, cur, cur_spec.hex())
        if held != expected:
            stale = True
            problems.append(f'{rel}: holds {held} copies of the current identity (chain ID, genesis hash, '
                            f'GenesisSpec), expected {expected}')
        after = text if same else rewrite(text, cur, new, cur_spec.hex(), spec.hex())
        got = counts(after, new, spec.hex())
        if got != expected:
            problems.append(f'{rel}: would hold {got} copies of the given identity, expected {expected}')
        legacy = counts(after, mainnet, mainnet_spec)
        if any(legacy):
            problems.append(f'{rel}: would still pin the old Mainnet identity {legacy}')
        out[rel] = after.encode('utf-8')
    for page, rel in VERSIONED:
        data.setdefault(page, (a.root / page).read_bytes())
        text = out.get(page, data[page]).decode('utf-8')
        # Any reference to the file's name with a version, whatever path precedes it.
        ref = re.compile(rf'(?<![0-9A-Za-z_.-]){re.escape(Path(rel).name)}\?v=[0-9a-fA-F]+(?![0-9A-Za-z_])')
        if not ref.search(text):
            problems.append(f'{page}: no versioned reference to {Path(rel).name}')
            continue
        out[page] = ref.sub(f'{Path(rel).name}?v={version(out[rel])}', text).encode('utf-8')
    data[INSTALLER] = (a.root / INSTALLER).read_bytes()
    out[INSTALLER] = build_installer.script({name: out[rel] for name, rel in INSTALLER_SOURCES.items()}).encode('utf-8')
    # The content versions and the installer payloads hash these files' bytes, which must be the bytes git
    # stores (LF). In a checkout that converts line endings (core.autocrlf) they would differ, and CI would
    # refuse the versions the tool wrote.
    for rel in [rel for _, rel in VERSIONED] + list(INSTALLER_SOURCES.values()):
        if b'\r\n' in (data[rel] if rel in data else (a.root / rel).read_bytes()):
            problems.append(f'{rel}: has CRLF line endings, so its content version or installer payload would not '
                            'match the committed file; use a checkout without line-ending conversion: clone again with '
                            '`git clone -c core.autocrlf=false`, or in this checkout, with nothing uncommitted, run '
                            '`git config core.autocrlf false`, `git rm -r --cached -q .` and `git reset --hard`')
    changed = [rel for rel in out if out[rel] != data[rel]]
    if a.check and changed:
        problems += [f'{rel}: does not carry this identity (or its content version / installer payload)' for rel in changed]
    for line in problems:
        print('FAIL', line, file=sys.stderr)
    if stale and same:
        print('FAIL hint: a listed file holds more or fewer copies of the identity than its FILES counts; name the '
              'network in words instead of adding copies, or update its counts in FILES', file=sys.stderr)
    elif stale:
        print('FAIL hint: a file was edited by hand or an earlier run was interrupted; restore the listed files '
              '(git checkout) and run again', file=sys.stderr)
    if problems:
        return 2
    if not a.check:
        # Every new file is written beside its original first, so an I/O error changes nothing. Then each
        # original is replaced whole, the node file (which names the tree's identity) last, so the next run
        # sees a run that stopped part way.
        staged = []
        try:
            for rel in sorted(changed, key=lambda r: r == ANCHOR):
                path = a.root / rel
                tmp = path.with_name(path.name + '.network-identity.tmp')
                staged.append((tmp, path))
                tmp.write_bytes(out[rel])
                shutil.copymode(path, tmp)
        except OSError as e:
            remove(tmp for tmp, _ in staged)
            print(f'FAIL {e}; nothing was changed', file=sys.stderr)
            return 2
        for i, (tmp, path) in enumerate(staged):
            try:
                os.replace(tmp, path)
            except OSError as e:
                remove(tmp for tmp, _ in staged[i:])
                replaced = [str(path.relative_to(a.root)) for _, path in staged[:i]]
                if same or not replaced:
                    # Only derived files changed (or none), so the next run finishes the job.
                    print(f'FAIL {e}; {i} of {len(staged)} files were replaced; run the same command again', file=sys.stderr)
                else:
                    print(f'FAIL {e}; {i} of {len(staged)} files were replaced: restore them '
                          f'(git checkout -- {" ".join(replaced)}) and run the same command again', file=sys.stderr)
                return 2
    print(('verified' if a.check else 'updated'), len(FILES), 'files;', 'changed:', len(changed),
          f'chainId={a.chain_id} genesisHash=0x{genesis}' + (f' specVersion={V3_SPEC_VERSION}' if a.v3 else ''))
    return 0

def remove(paths):
    for path in paths:
        try:
            path.unlink()
        except OSError:
            pass

if __name__ == '__main__':
    sys.exit(main())
