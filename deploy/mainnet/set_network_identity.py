#!/usr/bin/env python3
"""Switches every client in the repository from one network's identity to another's (chain ID, genesis hash,
GenesisSpec). Both networks are given as network invitations (network.pocol.json, as `pocol network` writes it):

    python3 deploy/mainnet/set_network_identity.py --from OLD.json --to NEW.json           # switch
    python3 deploy/mainnet/set_network_identity.py --from OLD.json --to NEW.json --check   # verify only

Every switch records the --from identity in RETIRED (deploy/mainnet/retired-networks.json, append-only), and a --to
identity that is listed there is refused: a rollback reverts the switch commit, it is not a reversed run. Only
--allow-revert switches back to a retired network on purpose (it stays listed).

--from names the network the tree pins today. An invitation that is no longer in the tree is taken from git
history, e.g. `git show <commit>:<path>/network.pocol.json > OLD.json`. --to is the network the clients are switched
to. Nothing in this tool names any particular network: each identity is read from its invitation, whose
genesis.spec.hex proves it (the chain ID is the GenesisSpec's own, the genesis hash its keccak256, and a devnet.json
chainId or genesisHash must agree).

Each GenesisSpec must be canonical RLP throughout, with six top items and the CP item count of its version's CP schema
(52 items for version 1, 64 for version 2). --to must be a version the tree's node decodes (SPEC_VERSION in
prototypes/lp1-node/src/genesis_spec.rs), so the clients are never switched to a genesis their release refuses.

The tree's current identity is the one the node file ANCHOR names, proven by its own GenesisSpec; it must be --from
(a switch) or --to (a run that finishes the derived files). Before and after the switch every listed file must carry
exactly its FILES counts of the identity and, after it, no copy of the --from identity, so --check fails on a tree
pinned to any other genesis. A redo with another genesis for the same chain (a postponed launch) is a switch from the
current invitation to the new one.

Derived files move with their sources: a page that loads a listed file by a content version (`?v=` and the first 12
hex digits of the file's SHA-256) gets the new version, and the Studio gateway installer is rebuilt from the switched
gateway sources. Those hashed sources must have the LF line endings git stores, so a checkout that converts line
endings (core.autocrlf) is refused. Files are read and written as bytes (line endings are kept), and nothing is
written unless every file passes: every new file is written beside its original first, then each original is replaced
whole, the node file last. Documents about the project's history (development/) and the website snapshots in
deploy/websites (build output) are not changed. Nothing is deployed.
"""
import argparse, hashlib, json, os, re, shutil, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'deploy' / 'rewards'))
sys.path.insert(0, str(ROOT / 'deploy' / 'studio'))
from pocol_rewards import keccak256  # noqa: E402  (the keccak256 the reward accounting uses)
import build_installer  # noqa: E402

# The CP item count of each GenesisSpec version's CP schema (genesis_spec.rs CP_LEN, CP_V2_LEN).
CP_ITEMS = {1: 52, 2: 64}
# The node's GenesisSpec codec: its SPEC_VERSION is the highest version a release node decodes.
NODE_SPEC = 'prototypes/lp1-node/src/genesis_spec.rs'
# The identities the clients were switched away from (append-only): {"retired": [{"chainId", "genesisHash"}]}.
RETIRED = 'deploy/mainnet/retired-networks.json'
# The largest invitation the launcher reads (launcher.rs MAX_INVITATION).
MAX_INVITATION = 1 << 20
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
    """A whole token, also as a JavaScript BigInt literal (`1234n`)."""
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
    if cur[:3] != new[:3]:
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

def spec_identity(b, versions=tuple(CP_ITEMS)):
    """(specVersion, chainId) of a GenesisSpec [version, chainId, CP, allocRoot, M_0List, sysCodeHash], after
    checking that the whole spec is one canonical RLP list of six items, that the version is one of `versions`, that
    CP is a list of that version's CP item count, and that the chainId integer has no leading zero."""
    try:
        is_list, start, end = rlp_item(b, 0)
        if not is_list or end != len(b):
            raise ValueError('not one RLP list')
        rlp_walk(b, start, end)
        is_list, vs, ve = rlp_item(b, start)
        version = b[vs] if not is_list and ve - vs == 1 and b[vs] in CP_ITEMS else None
        if version not in versions:
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

def node_spec_version(root):
    """The GenesisSpec version the tree's release node decodes (genesis_spec.rs SPEC_VERSION)."""
    m = re.search(r'pub const SPEC_VERSION: u8 = ([0-9]+);', (root / NODE_SPEC).read_text(encoding='utf-8'))
    if not m or int(m.group(1)) not in CP_ITEMS:
        raise SystemExit(f'{NODE_SPEC}: SPEC_VERSION is not readable')
    return int(m.group(1))

def invitation_identity(path, versions=tuple(CP_ITEMS)):
    """(chain ID, genesis hash hex, GenesisSpec bytes, spec version) of a network invitation, proven by its
    genesis.spec.hex; a devnet.json chainId or genesisHash must agree. ValueError otherwise."""
    try:
        data = Path(path).read_bytes()
    except OSError as e:
        raise ValueError(f'cannot read it: {e.strerror or e}') from None
    if len(data) > MAX_INVITATION:
        raise ValueError('a network invitation is at most 1 MiB')
    try:
        doc = json.loads(data.decode('utf-8'))
    except (UnicodeDecodeError, ValueError):
        raise ValueError('not a JSON network invitation') from None
    files = doc.get('files') if isinstance(doc, dict) else None
    if not isinstance(files, dict) or not isinstance(files.get('genesis.spec.hex'), str):
        raise ValueError('no files.genesis.spec.hex: not a network invitation')
    spec = read_spec(files['genesis.spec.hex'])
    version, chain_id = spec_identity(spec, versions)
    genesis = keccak256(spec).hex()
    info = files.get('devnet.json')
    if info is not None:
        try:
            info = json.loads(info) if isinstance(info, str) else None
        except ValueError:
            info = None
        if not isinstance(info, dict):
            raise ValueError('files.devnet.json is not a JSON object')
        if 'chainId' in info and info['chainId'] != chain_id:
            raise ValueError(f"devnet.json chainId {info['chainId']!r} is not the GenesisSpec's {chain_id}")
        if 'genesisHash' in info and str(info['genesisHash']).lower() != '0x' + genesis:
            raise ValueError(f"devnet.json genesisHash is not keccak256 of the GenesisSpec (0x{genesis})")
    return chain_id, genesis, spec, version

def current_identity(root):
    """The tree's identity as ANCHOR names it, proven by the GenesisSpec it pins."""
    text = (root / ANCHOR).read_text(encoding='utf-8')
    chain = re.search(r'const MAINNET_CHAIN_ID: u64 = ([0-9_]+);', text)
    genesis = re.search(r'const MAINNET_GENESIS: &str = "([0-9a-fA-F]{64})";', text)
    spec = re.search(r'let genesis = "0x([0-9a-fA-F]+)";', text)
    if not (chain and genesis and spec):
        raise SystemExit(f'{ANCHOR}: the current chain ID, genesis hash or GenesisSpec is not readable')
    chain_id, genesis, spec = int(chain.group(1).replace('_', '')), genesis.group(1).lower(), bytes.fromhex(spec.group(1))
    try:
        proven = keccak256(spec).hex() == genesis and spec_identity(spec)[1] == chain_id
    except ValueError:
        proven = False
    if not proven:
        raise SystemExit(f'{ANCHOR}: its GenesisSpec does not prove its chain ID and genesis hash')
    return chain_id, genesis, spec

def read_retired(data):
    """[(chain ID, genesis hash hex)] of RETIRED's bytes; ValueError if it is not that list."""
    try:
        doc = json.loads(data.decode('utf-8'))
        out = [(e['chainId'], e['genesisHash']) for e in doc['retired']]
    except (UnicodeDecodeError, ValueError, KeyError, TypeError):
        raise ValueError('not a {"retired": [{"chainId", "genesisHash"}]} list') from None
    for chain, genesis in out:
        if type(chain) is not int or not isinstance(genesis, str) or not re.fullmatch(r'0x[0-9a-f]{64}', genesis):
            raise ValueError(f'an entry is not a chain ID and a 0x-prefixed lower-case genesis hash: {chain!r}, {genesis!r}')
    return [(chain, genesis[2:]) for chain, genesis in out]

def with_retired(data, chain_id, genesis):
    """RETIRED's bytes with the identity appended, unchanged if it is listed already."""
    doc = json.loads(data.decode('utf-8'))
    if (chain_id, genesis) not in read_retired(data):
        doc['retired'].append({'chainId': chain_id, 'genesisHash': '0x' + genesis})
    return (json.dumps(doc, indent=2, ensure_ascii=False) + '\n').encode('utf-8')

def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('--from', dest='source', required=True, type=Path,
                   help='the invitation (network.pocol.json) of the network the tree pins today')
    p.add_argument('--to', dest='target', required=True, type=Path,
                   help='the invitation (network.pocol.json) of the network the clients are switched to')
    p.add_argument('--check', action='store_true', help='verify that the tree carries exactly the --to identity')
    p.add_argument('--allow-revert', action='store_true',
                   help=f'allow a --to identity listed in {RETIRED} (switch back to a retired network on purpose)')
    p.add_argument('--root', type=Path, default=ROOT)
    a = p.parse_args(argv)
    try:
        old_chain, old_genesis, old_spec, _ = invitation_identity(a.source)
    except ValueError as e:
        p.error(f'--from: {e}')
    release = node_spec_version(a.root)
    try:
        chain_id, genesis, spec, _ = invitation_identity(a.target, tuple(v for v in CP_ITEMS if v <= release))
    except ValueError as e:
        hint = f' (the release node decodes GenesisSpec versions up to {release})' if 'version is not' in str(e) else ''
        p.error(f'--to: {e}{hint}')
    if (old_chain, old_genesis) == (chain_id, genesis):
        p.error('--from and --to name the same network')
    if chain_id in (0, 777001) or not 0 < chain_id < 2**53:
        p.error(f'--to: chain ID {chain_id} is reserved or outside the JavaScript safe-integer range')
    try:
        retired_data = (a.root / RETIRED).read_bytes()
        retired = read_retired(retired_data)
    except OSError as e:
        raise SystemExit(f'{RETIRED}: cannot read it: {e.strerror or e}')
    except ValueError as e:
        raise SystemExit(f'{RETIRED}: {e}')
    if (chain_id, genesis) in retired and not a.allow_revert:
        p.error(f'--to: chain {chain_id} genesis 0x{genesis} is a retired network ({RETIRED}); a rollback reverts the '
                'switch commit; pass --allow-revert only to switch back to it on purpose')
    cur_chain, cur_genesis, cur_spec = current_identity(a.root)
    if (cur_chain, cur_genesis) not in ((old_chain, old_genesis), (chain_id, genesis)):
        raise SystemExit(f'{ANCHOR}: the tree pins chain {cur_chain} genesis 0x{cur_genesis}, which is neither --from '
                         'nor --to; pass the invitation of the network the tree pins today as --from')
    cur, new = tokens(cur_chain, cur_genesis), tokens(chain_id, genesis)
    old = tokens(old_chain, old_genesis)
    same = (cur_chain, cur_genesis) == (chain_id, genesis)
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
        left = counts(after, old, old_spec.hex())
        # A redo for the same chain keeps its chain ID, so only the old genesis hash and GenesisSpec count then.
        if old_chain == chain_id:
            left = (0,) + left[1:]
        if any(left):
            problems.append(f'{rel}: would still pin the --from identity {left}')
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
    # A switch retires the --from network (a run that finishes the derived files changes no identity).
    data[RETIRED] = retired_data
    out[RETIRED] = retired_data if same else with_retired(retired_data, old_chain, old_genesis)
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
        problems += [f'{rel}: does not carry the --to identity (or its content version, installer payload or retired list)' for rel in changed]
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
    if not a.check and not same:
        print(f'switching from chain {cur_chain} genesis 0x{cur_genesis} to chain {chain_id} genesis 0x{genesis}')
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
          f'chainId={chain_id} genesisHash=0x{genesis}')
    return 0

def remove(paths):
    for path in paths:
        try:
            path.unlink()
        except OSError:
            pass

if __name__ == '__main__':
    sys.exit(main())
