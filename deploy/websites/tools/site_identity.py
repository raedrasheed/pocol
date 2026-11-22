#!/usr/bin/env python3
"""The network identity of the website snapshots (explorer.pocol.net, ide.pocol.net, pocol.net and the Studio's
dist/), switched and checked through exact anchors. The snapshots are built bundles without sources (owner answer Q1),
so every change is an anchored edit: each anchor is a literal text that must occur exactly its count of times, and no
copy of an identity value may sit outside the anchors.

    python3 deploy/websites/tools/site_identity.py --check                     # CI: the tree matches DECLARED
    python3 deploy/websites/tools/site_identity.py                             # finish the derived files only
    python3 deploy/websites/tools/site_identity.py --to NEW.json [--gateway-host HOST] [--commit SHA] [--rpc-url URL]
    python3 deploy/websites/tools/site_identity.py --edits EDITS.json           # anchored bundle edits (W7, W14)
    python3 deploy/websites/tools/site_identity.py --download SITE/downloads/NAME=SHA256 ...   # kept-out archive lines

DECLARED (deploy/websites/site-identity.json) names the identity the snapshots pin: chain ID, genesis hash and the
GenesisSpec that proves both, the display name, the IDE's RPC URL, the explorer's seed list and the source links. --to
takes a network invitation (network.pocol.json, as `pocol network` writes it), proven by its GenesisSpec through
deploy/mainnet/set_network_identity.py's decoder; its `seeds` become the explorer's seed list. The explorer also names
its gateway host (gatewayHost): --gateway-host sets it, and a --to run without it is refused unless the current host is
one of the new seeds, so a switch never leaves an old server named. --commit points every source link at that commit;
--rpc-url changes the IDE's RPC endpoint (default: unchanged), so a test can point a copy at a local gateway. The
repository's own snapshots (--root is the repository) always name an https RPC endpoint; a loopback one is refused
there, --check included, and is only for a test copy. The display name is not switched (owner question Q6 keeps
"PoCol Mainnet").

Derived files move with their sources:
- a hashed bundle (BUNDLES) is named `<prefix>-<first 12 hex of its SHA-256>.js`; a changed bundle is renamed and the
  site's index.html reference and its line in an include list (INCLUDE_LISTS: the explorer's S1 file list) follow;
- a `<path>?v=<12 hex>` reference to a file of the same site that is not a page gets that file's content version;
- each site's SHA256SUMS.txt lists every file of the site, and keeps the lines of the download files that DECLARED's
  keptOut names as kept out of Git. Only those lines are carried: the line of any other file that is not in the
  snapshot (a deleted tracked .sha256) is dropped, so --check fails until a run removes it. A declared file that is
  present on disk (an archive restored under downloads/) is listed by its bytes and stays declared.

--download PATH=SHA256 (repeatable; PATH is `<site>/downloads/<name>`, a .zip, .tar.gz or .apk archive or its .sig or
.sha256) writes the SHA256SUMS.txt line of a release file that is kept out of Git and adds its name to keptOut. It
drops the lines it supersedes (and their keptOut names): the
kept-out lines of another version of the same archive family (the name before `-X.Y.Z`, for example
`PoCol-Studio-IDE-Viewer` or `PoCol-Mainnet`) and any other kept-out line with the bytes of one of those (an unversioned
copy such as `PoCol-Studio-Chrome.zip`). A file of the snapshot that it supersedes (a tracked `.sha256`) is refused
until it is removed, and a PATH that is in the snapshot is refused (its line is its own digest).

--check also refuses: any retired identity (deploy/mainnet/retired-networks.json) other than the declared one, so the
check turns on by itself once the snapshots pin a network that is not retired (a retired identity on the declared chain
ID, a postponed launch with a new genesis, is refused by its genesis hash only); any `chatgpt.site` origin; a download
link whose version is not the extension's package.json version; an extension version label (`PoCol extension X.Y.Z`,
`Studio & Viewer X.Y.Z`, `Version X.Y.Z`, `الإصدار X.Y.Z`, `إضافة PoCol X.Y.Z`) that is not that version, except a
capability message that names the oldest version with a feature (`X.Y.Z or later`, `X.Y.Z أو أحدث`); a native package
name `PoCol-Mainnet-X.Y.Z-*` that is not one of deploy/release/assemble_release.py's names at
deploy/standalone/package.py's version (or its .sig or .sha256), apart from the retired names in LEGACY_NATIVE that the
pages still link until the signed release moves them (W16), each of which must still be named; an index.html script or
stylesheet that does not exist; a hashed bundle whose name is not its content version; any config.php in a snapshot (never read), and server
code or a hidden file in the explorer snapshot (an api/ entry, a .php file, a dotfile such as .htaccess): the gateway
host serves the explorer as static files, and its /api reads are deploy/rewards/explorer_api.py (Sites W8).

Files are read and written as bytes decoded as UTF-8 (line endings are kept). Nothing is written unless every check
passes, and the tree a run would write is checked as --check checks it (every anchor at its count, no identity value
outside the anchors, after the anchored edits too): each new file is written beside its original first, then each
original is replaced whole, DECLARED last.
`config.php` is never opened. Nothing is deployed.
"""
import argparse, hashlib, json, os, posixpath, re, shutil, sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'deploy' / 'mainnet'))
sys.path.insert(0, str(ROOT / 'deploy' / 'rewards'))
sys.path.insert(0, str(ROOT / 'deploy' / 'studio'))
sys.path.insert(0, str(ROOT / 'deploy' / 'public-testnet'))
sys.path.insert(0, str(ROOT / 'deploy' / 'release'))
import set_network_identity as ni  # noqa: E402  (the invitation decoder, the token rules and the retired list)
import assemble_release as release  # noqa: E402  (the native release file names)

DECLARED = 'deploy/websites/site-identity.json'
RETIRED = ni.RETIRED
EXTENSION_PACKAGE = 'prototypes/pocol-extension/package.json'
NATIVE_PACKAGE = 'deploy/standalone/package.py'
EXPLORER, IDE, MAIN = 'deploy/websites/explorer.pocol.net', 'deploy/websites/ide.pocol.net', 'deploy/websites/pocol.net'
STUDIO = 'prototypes/pocol-studio/dist'
# Document roots; True: the site publishes a SHA256SUMS.txt of its files.
SITES = {EXPLORER: True, IDE: True, MAIN: True, STUDIO: False}
# DECLARED's keptOut key of each site that publishes a SHA256SUMS.txt.
SUMS_SITES = {posixpath.basename(site): site for site, has_sums in SITES.items() if has_sums}
# Files outside the document roots that pin the identity (a test of the deployed PHP).
EXTRA = ['deploy/websites/tests/api_security.php', 'deploy/websites/tests/browser_security.mjs']
SUMS = 'SHA256SUMS.txt'
# A download file that may be listed in SHA256SUMS.txt but kept out of Git (deploy/websites/README.md): an archive, its
# signature or its .sha256. Which ones are kept out is DECLARED's keptOut: a tracked .sha256 has this form too.
KEPT_OUT = re.compile(r'downloads/[^/]+\.(?:zip|tar\.gz|apk)(?:\.sig|\.sha256)?')
# A --download name: a kept-out file name of plain characters.
DOWNLOAD_NAME = re.compile(r'downloads/[A-Za-z0-9][A-Za-z0-9._-]*\.(?:zip|tar\.gz|apk)(?:\.sig|\.sha256)?')
# An archive family and its version: `downloads/<family>-X.Y.Z<rest>`.
ARCHIVE_VERSION = re.compile(r'downloads/(.+?)-([0-9]+\.[0-9]+\.[0-9]+)(?=[-.])(.*)')
# Hashed bundles: key -> (site, prefix). The file is `<site>/assets/<prefix>-<12 hex>.js`, exactly one per key.
BUNDLES = {
    'explorer-bundle': (EXPLORER, 'index'),
    'ide-bundle': (IDE, 'index-extension041'),
}
# Files outside the document roots that name a hashed bundle on a line of its own (`assets/<prefix>-<12 hex>.js`): the
# explorer's S1 include list (Sites W5). A rename rewrites that line and --check refuses a list that names any other.
INCLUDE_LISTS = {'explorer-bundle': 'deploy/websites/tools/explorer-s1-include.txt'}
LINK_PREFIX = 'https://github.com/raedrasheed/pocol/'
# The source links that --commit derives.
COMMIT_LINKS = {
    'source': 'tree/{sha}',
    'rpc': 'blob/{sha}/prototypes/pocold/src/rpc.rs',
    'network': 'tree/{sha}/prototypes/pocold',
    'studioSite': 'blob/{sha}/prototypes/pocold/src/site.rs',
    'studioExtension': 'tree/{sha}/prototypes/pocol-extension',
}
# (file, template, count): a file is a BUNDLES key or a repo-relative path. A template names fields in braces:
# chainId, chainIdHex (minimal lower-case 0x hex), genesisHash (0x, lower case), name, rpcUrl, seedHosts (the seed hosts
# as a JavaScript array body of template literals), seedPort, link.<name>.
ANCHORS = [
    # Explorer bundle: the identity constants and every comparison (gateway, transactions, geography, parameters).
    ('explorer-bundle', 'Ae={chainId},je=`{genesisHash}`;', 1),
    ('explorer-bundle', 'if(Number(t.chainId)!=={chainId}||t.genesisHash?.toLowerCase()!==`{genesisHash}`)', 1),
    ('explorer-bundle', 'if(Number(e.chainId)!=={chainId}||e.genesisHash?.toLowerCase()!==`{genesisHash}`)', 1),
    ('explorer-bundle', 'e.schema!==`pocol-transactions-v1`||e.chainId!=={chainId}||e.genesisHash!==`{genesisHash}`', 1),
    ('explorer-bundle', 'e.schema!==`pocol-network-geography-v1`||e.chainId!=={chainId}||e.genesisHash!==`{genesisHash}`', 1),
    ('explorer-bundle', 'if(Number(e?.chainId)!=={chainId}||e?.genesisHash?.toLowerCase()!==`{genesisHash}`)', 1),
    ('explorer-bundle', 'children:[l?l.params.chainId:{chainId},(0,N.jsx)', 1),
    ('explorer-bundle', 'value:l?.params.chainId??{chainId}}),(0,N.jsx)(L,{label:`Genesis hash`,'
                        'value:l?.params.genesisHash??`{genesisHash}`}', 1),
    # Explorer bundle: the seed list and the source links.
    ('explorer-bundle', 'className:`reference-grid`,children:[{seedHosts}].map((e,t)=>', 1),
    ('explorer-bundle', '(0,N.jsxs)(`span`,{children:[e,`:{seedPort}`]})', 1),
    ('explorer-bundle', '` installer on {gatewayHost}. It reads the existing node', 1),
    ('explorer-bundle', 'href:`{link.source}`,target:`_blank`,rel:`noreferrer`,children:[`PoCol source`', 1),
    ('explorer-bundle', 'href:`{link.rpc}`,target:`_blank`,rel:`noreferrer`,children:[`PoCol RPC implementation`', 1),
    ('explorer-bundle', 'href:`{link.network}`,target:`_blank`,rel:`noreferrer`,children:[`Network and wallet implementation`', 1),
    (f'{EXPLORER}/index.html', '<meta name="description" content="Explore {name}. Browse blocks', 1),
    # IDE bundle: the frozen network object (L9/R9 compare it).
    ('ide-bundle', 'var w9=Object.freeze({{name:`{name}`,chainId:{chainId},chainIdHex:`{chainIdHex}`,'
                   'genesisHash:`{genesisHash}`,rpcUrl:`{rpcUrl}`,explorerUrl:', 1),
    (f'{IDE}/DEPLOY-README-AR.txt', '\nChain ID: {chainId}\n', 1),
    (f'{IDE}/DEPLOY-README-AR.txt', '\nChain ID (hex): {chainIdHex}\n', 1),
    (f'{IDE}/DEPLOY-README-AR.txt', '\nRPC: {rpcUrl}\n', 1),
    (f'{IDE}/DEPLOY-README-AR.txt', '\nGenesis: {genesisHash}\n', 1),
    # pocol.net: the network summary and the block reward, in the page scripts and the PHP behind them.
    (f'{MAIN}/network.js', "\n  const CHAIN_ID = {chainId};\n  const GENESIS = '{genesisHash}';\n", 1),
    (f'{MAIN}/reward.js', "\n  const CHAIN_ID = {chainId};\n  const GENESIS = '{genesisHash}';\n", 1),
    (f'{MAIN}/api/network.php', "\nconst POCOL_SUMMARY_CHAIN = {chainId};\nconst POCOL_SUMMARY_GENESIS = '{genesisHash}';\n", 1),
    (f'{MAIN}/api/reward.php', "\nconst POCOL_REWARD_GENESIS = '{genesisHash}';\n", 1),
    (f'{MAIN}/api/reward.php', '($v->chainId ?? null) !== {chainId} ||', 1),
    ('deploy/websites/tests/api_security.php', "'chainId'=>{chainId},'genesisHash'=>POCOL_REWARD_GENESIS", 1),
    # Studio: the source links of the install dialog.
    (f'{STUDIO}/app.js', 'href="{link.studioSite}">${{t(\'source\')}}', 1),
    (f'{STUDIO}/app.js', 'href="{link.studioExtension}">${{t(\'extensionSource\')}}', 1),
]
FIELD = re.compile(r'\{(chainId|chainIdHex|genesisHash|name|rpcUrl|gatewayHost|seedHosts|seedPort|link\.[A-Za-z]+)\}')
BINARY = {'.png', '.ico', '.ttf', '.woff', '.woff2', '.zip', '.jpg', '.jpeg', '.webp', '.wasm', '.gz'}
DOWNLOAD = re.compile(r'PoCol-Studio-IDE-Viewer-([0-9]+\.[0-9]+\.[0-9]+)\.zip')
# Extension version labels (Sites W14): the version a page names for the extension must be package.json's. The forms
# with `to` or `version` are there so that a capability message is seen and exempted, never skipped by the pattern.
_SEMVER = r'([0-9]+\.[0-9]+\.[0-9]+)(?![0-9A-Za-z])'
LABELS = [re.compile(prefix + _SEMVER) for prefix in (
    r'PoCol extension (?:to )?(?:version )?',
    r'Studio (?:&|&amp;) Viewer (?:to )?(?:version )?',
    r'(?<![A-Za-z])Version ',
    r'إضافة PoCol (?:لل|ال|بال)?(?:إصدار )?',
    r'(?<![\u0600-\u06ff])(?:ال|لل|بال)إصدار ',
)]
# A capability message names the oldest version that has a feature ("Update the PoCol extension to version 0.4.0 or
# later"): it is not a label of the offered package.
OR_LATER = re.compile(r'\s+(?:or later|or newer|أو أحدث|أو ما بعده)')
# Native package names (assemble_release.py's, at package.py's version).
NATIVE = re.compile(r'PoCol-Mainnet-[0-9]+\.[0-9]+\.[0-9]+-[A-Za-z0-9_-]+(?:\.[A-Za-z0-9]+)+')
# The retired network's native package that pocol.net still links, labelled as the retired network's, beside "Coming
# from the retired network?" (index.html, pocol-tools.html, guide-content.js) until the signed release moves those links
# (W16, PR F). The Linux package and the unlabelled "MAINNET EDITION" and tools-page links were removed on 2026-11-22:
# they offered the retired network's 0.4.0 packages as PoCol Mainnet applications. --check fails
# once no page links a name listed here, so the exemption leaves with the last link.
LEGACY_NATIVE = frozenset({'PoCol-Mainnet-0.4.0-Windows.zip'})
VERSIONED = re.compile(r'(?<![0-9A-Za-z_.:/-])(/?[A-Za-z0-9_][A-Za-z0-9_./-]*)\?v=([0-9a-f]{12})(?![0-9A-Za-z])')
# The explorer snapshot is static (Sites W8): its /api paths are the gateway's (deploy/rewards/explorer_api.py) and its
# headers are the Caddy site's (deploy/explorer/explorer_site.py), so no server code or hidden file may be in it.
SERVER_CODE = re.compile(r'(^|/)\.|\.(php[0-9]?|phtml|phar)$|^api(/|$)', re.IGNORECASE)

class Refused(Exception):
    pass

def sha12(data):
    return hashlib.sha256(data).hexdigest()[:12]

def is_config_php(rel):
    return Path(rel).name.lower() == 'config.php'

# ---------------- the declared identity ----------------
def check_seed(seed):
    m = re.fullmatch(r'((?:[0-9]{1,3}\.){3}[0-9]{1,3}|[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?(?:\.[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?)+):([0-9]{1,5})', seed)
    if not m or not 0 < int(m.group(2)) < 65536 or (re.fullmatch(r'[0-9.]+', m.group(1)) and any(int(p) > 255 for p in m.group(1).split('.'))):
        raise Refused(f'seed {seed!r} is not host:port')
    return m.group(1), int(m.group(2))

def check_host(host):
    if not isinstance(host, str):
        raise Refused(f'host {host!r}')
    return check_seed(host + ':1')[0]

def check_rpc_url(url):
    if not isinstance(url, str) or not re.fullmatch(r'https://[a-z0-9.-]+(?::[0-9]{1,5})?/[A-Za-z0-9/._-]*|http://127\.0\.0\.1:[0-9]{1,5}/[A-Za-z0-9/._-]*', url):
        raise Refused(f'rpcUrl {url!r} is not an https URL or a loopback http URL with a path')
    return url

def check_link(name, url):
    if not isinstance(url, str) or not url.startswith(LINK_PREFIX) or not re.fullmatch(r'[A-Za-z0-9:/._#-]+', url):
        raise Refused(f'link {name} {url!r} is not a {LINK_PREFIX} URL')
    return url

def validate(doc):
    """The declared identity, checked: the GenesisSpec proves the chain ID and genesis hash."""
    try:
        spec = ni.read_spec(doc['genesisSpec'])
        version, chain = ni.spec_identity(spec)
    except (KeyError, TypeError, ValueError) as e:
        raise Refused(f'{DECLARED}: no valid genesisSpec ({e})') from None
    genesis = '0x' + ni.keccak256(spec).hex()
    if doc.get('chainId') != chain or doc.get('genesisHash') != genesis:
        raise Refused(f'{DECLARED}: chainId and genesisHash must be the GenesisSpec\'s ({chain}, {genesis})')
    if not isinstance(doc.get('name'), str) or not re.fullmatch(r'[A-Za-z0-9 ._-]{1,40}', doc['name']):
        raise Refused(f'{DECLARED}: name')
    check_rpc_url(doc.get('rpcUrl'))
    check_host(doc.get('gatewayHost'))
    seeds = doc.get('seeds')
    if not isinstance(seeds, list) or not 1 <= len(seeds) <= 16 or len(set(seeds)) != len(seeds):
        raise Refused(f'{DECLARED}: seeds must be 1 to 16 distinct host:port entries')
    if len({check_seed(s)[1] for s in seeds}) != 1:
        raise Refused(f'{DECLARED}: the explorer shows one port, so every seed must use the same port')
    links = doc.get('links')
    if not isinstance(links, dict) or set(links) != set(COMMIT_LINKS):
        raise Refused(f'{DECLARED}: links must name exactly {sorted(COMMIT_LINKS)}')
    for k, v in links.items():
        check_link(k, v)
    kept = doc.get('keptOut')
    if not isinstance(kept, dict) or set(kept) != set(SUMS_SITES):
        raise Refused(f'{DECLARED}: keptOut must name exactly {sorted(SUMS_SITES)}')
    for site, names in kept.items():
        if not isinstance(names, list) or not all(isinstance(n, str) and DOWNLOAD_NAME.fullmatch(n) for n in names) \
                or names != sorted(set(names)):
            raise Refused(f'{DECLARED}: keptOut {site}: a sorted list of distinct names downloads/<archive> (a .zip, '
                          '.tar.gz or .apk, or its .sig or .sha256)')
    return doc

def fields(doc):
    hosts = [check_seed(s)[0] for s in doc['seeds']]
    out = {'chainId': str(doc['chainId']), 'chainIdHex': hex(doc['chainId']), 'genesisHash': doc['genesisHash'],
           'name': doc['name'], 'rpcUrl': doc['rpcUrl'], 'gatewayHost': doc['gatewayHost'],
           'seedHosts': ','.join(f'`{h}`' for h in hosts), 'seedPort': str(check_seed(doc['seeds'][0])[1])}
    out.update({f'link.{k}': v for k, v in doc['links'].items()})
    return out

def render(template, values):
    # Literal braces are doubled in a template (`{{`), as in str.format.
    out, i = [], 0
    for m in re.finditer(r'\{\{|\}\}|\{[^{}]*\}', template):
        out.append(template[i:m.start()])
        tok = m.group(0)
        if tok in ('{{', '}}'):
            out.append(tok[0])
        else:
            if not FIELD.fullmatch(tok):
                raise ValueError(f'unknown field {tok} in an anchor')
            out.append(values[tok[1:-1]])
        i = m.end()
    out.append(template[i:])
    return ''.join(out)

def tokens(values):
    """The identity values that may only occur inside anchors: (description, regex)."""
    out = [('chain ID', ni.token(values['chainId'])), ('chain ID (hex)', ni.token(values['chainIdHex'])),
           ('genesis hash', ni.hexrun(values['genesisHash'][2:]))]
    for host in re.findall(r'`([^`]+)`', values['seedHosts']):
        out.append((f'seed {host}', rf'(?<![0-9A-Za-z.-]){re.escape(host)}(?![0-9A-Za-z-]|\.[0-9A-Za-z])'))
    return out

def identity_regex(chain, genesis, keep_chain=None):
    """Any spelling of one identity (chain ID decimal, digit groups, hex; genesis hash with or without 0x). keep_chain
    is the declared chain ID: a retired identity on it (a postponed launch that replaces the genesis and keeps the
    chain ID) is matched by its genesis hash only, so the declared chain ID's own pins are not taken for it."""
    parts = ([] if chain == keep_chain else [ni.token(t) for t in (str(chain), f'{chain:_}', hex(chain))]) + \
        [ni.hexrun(genesis.removeprefix('0x'))]
    return re.compile('|'.join(f'(?:{p})' for p in parts), re.IGNORECASE)

# ---------------- the tree ----------------
class Tree:
    """The website files as they are on disk with the pending changes `out` ({path: bytes, or None for a removed
    file}) over them. Files are read lazily as bytes; config.php is never opened."""
    def __init__(self, root):
        self.root, self.cache, self.out = root, {}, {}

    def files(self, site):
        base, found = self.root / site, set()
        if base.is_dir():
            for path in base.rglob('*'):
                if path.is_symlink():
                    raise Refused(f'{path.relative_to(self.root).as_posix()}: a symbolic link in a website snapshot')
                if path.is_file():
                    found.add(path.relative_to(self.root).as_posix())
        found |= {f for f in self.out if f.startswith(site + '/')}
        return sorted(f for f in found if self.out.get(f, b'') is not None)

    def exists(self, rel):
        return self.out[rel] is not None if rel in self.out else (self.root / rel).is_file()

    def read(self, rel):
        if is_config_php(rel):
            raise Refused(f'{rel}: config.php is never read')
        if rel in self.out:
            if self.out[rel] is None:
                raise Refused(f'{rel}: removed')
            return self.out[rel]
        if rel not in self.cache:
            self.cache[rel] = (self.root / rel).read_bytes()
        return self.cache[rel]

    def text(self, rel):
        """The file as text, or None for a binary file or config.php (which is never read)."""
        if Path(rel).suffix.lower() in BINARY or is_config_php(rel):
            return None
        try:
            return self.read(rel).decode('utf-8')
        except UnicodeDecodeError:
            return None

    def changed(self):
        return {rel: data for rel, data in self.out.items()
                if data is None or not (self.root / rel).is_file() or (self.root / rel).read_bytes() != data}

def bundle_path(tree, key):
    site, prefix = BUNDLES[key]
    found = [f for f in tree.files(site)
             if re.fullmatch(rf'{re.escape(site)}/assets/{re.escape(prefix)}-[A-Za-z0-9_]+\.js', f)]
    if len(found) != 1:
        raise Refused(f'{site}/assets: expected exactly one {prefix}-<hash>.js bundle, found {found}')
    return found[0]

def anchor_file(tree, f):
    return bundle_path(tree, f) if f in BUNDLES else f

# ---------------- switch ----------------
def apply_anchors(tree, cur, new, problems):
    """Moves every anchor from cur to new values (into tree.out); problems lists every mismatch."""
    by_file = {}
    for f, template, count in ANCHORS:
        by_file.setdefault(anchor_file(tree, f), []).append((template, count))
    for rel, anchors in by_file.items():
        # The identity in deploy/mainnet/set_network_identity.py's FILES (the Studio's app.js) is that tool's; this
        # tool only moves the anchors it lists there.
        cur_tokens = [] if rel in ni.FILES else tokens(cur)
        try:
            text = tree.text(rel)
        except OSError:
            text = None
        if text is None:
            problems.append(f'{rel}: cannot read it as UTF-8 text')
            continue
        expected = {d: 0 for d, _ in cur_tokens}
        for template, count in anchors:
            old = render(template, cur)
            held = text.count(old)
            if held != count:
                problems.append(f'{rel}: holds {held} copies of the anchor {old[:120]!r}, expected {count}')
            for d, rx in cur_tokens:
                expected[d] += count * len(re.findall(rx, old, re.IGNORECASE))
        for d, rx in cur_tokens:
            held = len(re.findall(rx, text, re.IGNORECASE))
            if held != expected[d]:
                problems.append(f'{rel}: holds {held} copies of the {d}, {expected[d]} of them inside its anchors: '
                                'a copy outside the anchors (add an anchor, or name the value in words)')
        after = text
        for template, _ in anchors:
            after = after.replace(render(template, cur), render(template, new))
        for template, count in anchors:
            got = after.count(render(template, new))
            if got != count:
                problems.append(f'{rel}: would hold {got} copies of the anchor {render(template, new)[:120]!r}, expected {count}')
        if after != text:
            tree.out[rel] = after.encode('utf-8')

def apply_edits(tree, edits, problems):
    """Anchored edits (W7): each `old` must occur exactly `count` times in its file and is replaced by `new`."""
    for i, e in enumerate(edits):
        if not isinstance(e, dict) or set(e) != {'file', 'old', 'new', 'count'} \
                or not all(isinstance(e[k], str) for k in ('file', 'old', 'new')) \
                or type(e['count']) is not int or e['count'] < 1 or not e['old'] or e['old'] == e['new']:
            raise Refused(f'edit {i}: must be {{"file", "old", "new", "count" >= 1}} with a non-empty old text that differs from new')
        rel = anchor_file(tree, e['file'])
        if not any(rel.startswith(site + '/') for site in SITES) or '..' in Path(rel).parts or is_config_php(rel) \
                or rel.endswith('/' + SUMS):
            raise Refused(f'edit {i}: {rel} is not an editable website file')
        text = tree.text(rel) if tree.exists(rel) else None
        if text is None:
            raise Refused(f'edit {i}: {rel} is not a text file of the snapshots')
        held = text.count(e['old'])
        if held != e['count']:
            problems.append(f'edit {i}: {rel} holds {held} copies of {e["old"][:120]!r}, expected {e["count"]}')
            continue
        tree.out[rel] = text.replace(e['old'], e['new']).encode('utf-8')

def resolve(site, rel, path):
    """The site file a reference names, relative to the referring file, else to the site root."""
    if path.startswith('/'):
        return posixpath.normpath(site + path)
    near = posixpath.normpath(posixpath.join(posixpath.dirname(rel), path))
    return near if near.startswith(site + '/') else posixpath.normpath(f'{site}/{path}')

def archive_version(name):
    """(family, version) of a kept-out archive name, or None."""
    m = ARCHIVE_VERSION.fullmatch(name)
    return (m.group(1), m.group(2)) if m else None

def finish(tree, problems, downloads=None, kept_out=None):
    """The derived files (into tree.out): bundle names, content versions and SHA256SUMS.txt, and the new keptOut
    (returned). kept_out (DECLARED's keptOut) names the kept-out lines that are carried; downloads ({site path: digest},
    --download) adds kept-out lines and drops the ones they supersede."""
    # A hashed bundle is named by its content version; the site's index.html reference follows.
    for key, (site, prefix) in BUNDLES.items():
        rel = bundle_path(tree, key)
        data = tree.read(rel)
        name = f'{site}/assets/{prefix}-{sha12(data)}.js'
        if name == rel:
            continue
        index = f'{site}/index.html'
        page = tree.text(index)
        old_ref, new_ref = '"/' + rel[len(site) + 1:] + '"', '"/' + name[len(site) + 1:] + '"'
        if page.count(old_ref) != 1:
            problems.append(f'{index}: references {old_ref} {page.count(old_ref)} times, expected once')
            continue
        base = Path(rel).name
        others = [f for f in tree.files(site) if f not in (index, f'{site}/{SUMS}', rel) and base in (tree.text(f) or '')]
        if others:
            problems.append(f'{rel}: also named by {others}; only index.html may name a hashed bundle')
        tree.out[index] = page.replace(old_ref, new_ref).encode('utf-8')
        tree.out[name], tree.out[rel] = data, None
        if key in INCLUDE_LISTS:
            listed = INCLUDE_LISTS[key]
            text = tree.text(listed) if tree.exists(listed) else None
            old_line, new_line = rel[len(site) + 1:], name[len(site) + 1:]
            lines = None if text is None else text.split('\n')
            if lines is None or lines.count(old_line) != 1:
                problems.append(f'{listed}: must name {old_line} on one line of its own, to follow the rename to {new_line}')
                continue
            lines[lines.index(old_line)] = new_line
            tree.out[listed] = '\n'.join(lines).encode('utf-8')
    # Content versions of the files that are not pages, to a fixed point (a script may version a stylesheet).
    for _ in range(8):
        moved = False
        for site in SITES:
            files = tree.files(site)
            present = set(files)
            for rel in files:
                if rel.endswith('/' + SUMS):
                    continue
                text = tree.text(rel)
                if text is None or '?v=' not in text:
                    continue
                def version(m, rel=rel, site=site):
                    target = resolve(site, rel, m.group(1))
                    if target not in present or target == rel or target.endswith(('.html', '.htm')) or is_config_php(target):
                        return m.group(0)
                    return f'{m.group(1)}?v={sha12(tree.read(target))}'
                after = VERSIONED.sub(version, text)
                if after != text:
                    tree.out[rel] = after.encode('utf-8')
                    moved = True
        if not moved:
            break
    else:
        problems.append('content versions do not settle (a cycle of versioned references)')
    # SHA256SUMS.txt of each site: every file but itself, and the declared kept-out lines.
    new_kept = {}
    for site, has_sums in SITES.items():
        if not has_sums:
            continue
        sums = f'{site}/{SUMS}'
        lines = {}
        for f in tree.files(site):
            if f == sums:
                continue
            if is_config_php(f):
                problems.append(f'{f}: a config.php must never be in a website snapshot (it was not read)')
                continue
            lines[f[len(site) + 1:]] = hashlib.sha256(tree.read(f)).hexdigest()
        # Only the declared kept-out lines are read back; the line of any other file that is not in the snapshot goes.
        declared = set((kept_out or {}).get(posixpath.basename(site), []))
        kept = {}
        for line in tree.text(sums).splitlines():
            digest, _, name = line.partition('  ')
            if name in declared and name not in lines and re.fullmatch(r'[0-9a-f]{64}', digest):
                kept[name] = digest
        given = {path[len(site) + 1:]: digest for path, digest in (downloads or {}).items() if path.startswith(site + '/')}
        for name in sorted(declared - set(kept) - set(lines) - set(given)):
            problems.append(f'{sums}: has no line for {name}, which {DECLARED} keptOut names; give it with --download')
        superseded = set()
        for name, digest in given.items():
            if name in lines:
                problems.append(f'{site}/{name}: is in the snapshot, so its {SUMS} line is its own digest, not a --download')
                continue
            family = archive_version(name)
            if family is None:
                continue
            for old in list(kept):
                if old not in given and (archive_version(old) or (None,))[0] == family[0] and archive_version(old) != family:
                    superseded.add(kept.pop(old))
            for old in lines:
                if KEPT_OUT.fullmatch(old) and (archive_version(old) or (None,))[0] == family[0] \
                        and archive_version(old)[1] != family[1]:
                    problems.append(f'{site}/{old}: superseded by --download {site}/{name}; remove it from the snapshot')
        # A copy of a superseded archive under another name (the unversioned PoCol-Studio-Chrome.zip) goes with it.
        for old in [n for n, d in kept.items() if d in superseded and n not in given]:
            del kept[old]
        kept.update(given)
        # A declared kept-out archive restored under downloads/ (a local test or a deployment) is listed by the bytes on
        # disk and stays declared, so a clean checkout without it still carries its line.
        new_kept[posixpath.basename(site)] = sorted({n for n in kept if n not in lines} | {n for n in declared if n in lines and KEPT_OUT.fullmatch(n)})
        lines.update({n: d for n, d in kept.items() if n not in lines})
        text = ''.join(f'{lines[n]}  {n}\n' for n in sorted(lines))
        if text != tree.text(sums):
            tree.out[sums] = text.encode('utf-8')
    return new_kept

# ---------------- checks ----------------
def tree_checks(tree, doc, retired, problems):
    """The checks that hold for any declared identity, on the tree as it would be after the run."""
    values = fields(doc)
    anchored = {anchor_file(tree, f) for f, _, _ in ANCHORS}
    own = re.compile('|'.join(f'(?:{rx})' for _, rx in tokens(values)), re.IGNORECASE)
    gone = [identity_regex(c, g, keep_chain=doc['chainId']) for c, g in retired if (c, g) != (doc['chainId'], doc['genesisHash'][2:])]
    version = json.loads((tree.root / EXTENSION_PACKAGE).read_text(encoding='utf-8'))['version']
    try:
        native = set(release.native_names(tree.root))
        native_version = release.native_version(tree.root)
    except (OSError, TypeError) as e:
        raise Refused(f'{NATIVE_PACKAGE}: no native release version ({e})') from None
    native |= {n + suffix for n in native for suffix in ('.sig', '.sha256')}
    legacy_named = set()
    for site in list(SITES) + ['deploy/websites/tests']:
        for rel in tree.files(site):
            if not any(rel.startswith(s + '/') for s in SITES) and rel not in EXTRA:
                continue
            if is_config_php(rel):
                problems.append(f'{rel}: a config.php must never be in a website snapshot (it was not read)')
                continue
            text = tree.text(rel)
            if text is None:
                continue
            # The identity in set_network_identity.py's FILES (the Studio's app.js) is that tool's, and its guard's.
            if rel in ni.FILES:
                gone_here = []
            else:
                gone_here = gone
                if rel not in anchored and own.search(text):
                    problems.append(f'{rel}: pins the declared identity outside the anchors')
            for rx in gone_here:
                if rx.search(text):
                    problems.append(f'{rel}: still pins a retired network ({RETIRED})')
            if 'chatgpt.site' in text.lower():
                problems.append(f'{rel}: names a chatgpt.site origin')
            bad = sorted({v for v in DOWNLOAD.findall(text) if v != version})
            if bad:
                problems.append(f'{rel}: links the extension package {", ".join(bad)}, not {version} ({EXTENSION_PACKAGE})')
            if rel in EXTRA:
                continue
            labels = sorted({m.group(0) for rx in LABELS for m in rx.finditer(text)
                             if m.group(1) != version and not OR_LATER.match(text, m.end())})
            if labels:
                problems.append(f'{rel}: labels the extension {", ".join(map(repr, labels))}, not {version} '
                                f'({EXTENSION_PACKAGE}); only a capability message ("X or later") may name another version')
            names = set(NATIVE.findall(text))
            legacy_named |= names & LEGACY_NATIVE
            wrong = sorted(names - native - LEGACY_NATIVE)
            if wrong:
                problems.append(f'{rel}: names the native package {", ".join(wrong)}, not one of deploy/release/'
                                f'assemble_release.py\'s names at {NATIVE_PACKAGE} version {native_version}: {sorted(native)}')
    for name in sorted(LEGACY_NATIVE - legacy_named):
        problems.append(f'LEGACY_NATIVE: no website file names {name} any more; remove it from the exemption')
    for site in SITES:
        index = f'{site}/index.html'
        page = tree.text(index)
        refs = re.findall(r'<script\b[^>]*\bsrc="([^"]+)"', page) + re.findall(r'<link\b[^>]*\bhref="([^"]+)"', page)
        for ref in refs:
            if re.match(r'(?:[a-z]+:)?//', ref):
                continue
            if not tree.exists(resolve(site, index, ref.split('?')[0].split('#')[0])):
                problems.append(f'{index}: references {ref}, which does not exist')
    for key, (site, prefix) in BUNDLES.items():
        rel = bundle_path(tree, key)
        if rel != f'{site}/assets/{prefix}-{sha12(tree.read(rel))}.js':
            problems.append(f'{rel}: a hashed bundle is named by its content version')
        if key in INCLUDE_LISTS:
            listed = INCLUDE_LISTS[key]
            text = tree.text(listed) if tree.exists(listed) else None
            named = [] if text is None else [line for line in text.split('\n')
                                             if re.fullmatch(rf'assets/{re.escape(prefix)}-[A-Za-z0-9_-]+\.js', line)]
            if named != [rel[len(site) + 1:]]:
                problems.append(f'{listed}: names {named or "no bundle"}, not the bundle {rel[len(site) + 1:]}')
    for rel in tree.files(EXPLORER):
        if SERVER_CODE.search(rel[len(EXPLORER) + 1:]):
            problems.append(f'{rel}: server code or a hidden file in the explorer snapshot (the gateway host serves it as '
                            'static files; its /api reads are deploy/rewards/explorer_api.py)')

def load_retired(root):
    try:
        return ni.read_retired((root / RETIRED).read_bytes())
    except (OSError, ValueError) as e:
        raise Refused(f'{RETIRED}: {e}') from None

def parse_downloads(values):
    """{site path: digest} of the --download PATH=SHA256 values."""
    out = {}
    for value in values or []:
        path, sep, digest = value.rpartition('=')
        site = next((s for s, has_sums in SITES.items() if has_sums and path.startswith(s + '/')), None)
        if not sep or site is None or not DOWNLOAD_NAME.fullmatch(path[len(site) + 1:]):
            raise Refused(f'--download {value!r}: PATH=SHA256 with PATH <site>/downloads/<archive> (a .zip, .tar.gz or '
                          f'.apk, or its .sig or .sha256) in a site that publishes {SUMS}: '
                          f'{", ".join(s for s, has_sums in SITES.items() if has_sums)}')
        if not re.fullmatch(r'[0-9a-f]{64}', digest):
            raise Refused(f'--download {path}: the SHA-256 is 64 lower-case hex digits')
        if path in out:
            raise Refused(f'--download {path}: given twice')
        out[path] = digest
    return out

def plan(root, target=None, commit=None, rpc_url=None, edits=None, allow_revert=False, gateway_host=None, downloads=None):
    """(problems, {path: new bytes or None}, new declared doc) of a run."""
    tree = Tree(root)
    try:
        cur = validate(json.loads((root / DECLARED).read_text(encoding='utf-8')))
    except (OSError, ValueError) as e:
        raise Refused(f'{DECLARED}: {e}') from None
    retired = load_retired(root)
    new = dict(cur, links=dict(cur['links']), seeds=list(cur['seeds']))
    if target is not None:
        release = ni.node_spec_version(ROOT)
        try:
            chain, genesis, spec, _ = ni.invitation_identity(target, tuple(v for v in ni.CP_ITEMS if v <= release))
            seeds = json.loads(Path(target).read_text(encoding='utf-8')).get('seeds')
        except ValueError as e:
            raise Refused(f'--to: {e}') from None
        if chain in (0, 777001) or not 0 < chain < 2**53:
            raise Refused(f'--to: chain ID {chain} is reserved or outside the JavaScript safe-integer range')
        if (chain, genesis) in retired and (chain, genesis) != (cur['chainId'], cur['genesisHash'][2:]) and not allow_revert:
            raise Refused(f'--to: chain {chain} genesis 0x{genesis} is a retired network ({RETIRED}); pass --allow-revert '
                          'only to switch back to it on purpose')
        new.update(chainId=chain, genesisHash='0x' + genesis, genesisSpec='0x' + spec.hex(), seeds=seeds)
    if commit is not None:
        if not re.fullmatch(r'[0-9a-f]{40}', commit):
            raise Refused('--commit: a full 40-digit lower-case commit hash')
        new['links'] = {k: LINK_PREFIX + v.format(sha=commit) for k, v in COMMIT_LINKS.items()}
    if rpc_url is not None:
        new['rpcUrl'] = rpc_url
    if gateway_host is not None:
        new['gatewayHost'] = check_host(gateway_host)
    new = validate(new)
    if Path(root).resolve() == ROOT and not new['rpcUrl'].startswith('https://'):
        raise Refused(f'{DECLARED}: rpcUrl {new["rpcUrl"]!r}: the repository\'s snapshots name an https RPC endpoint; '
                      'a loopback URL is only for a test copy (--root)')
    if target is not None and gateway_host is None and new['gatewayHost'] not in {check_seed(s)[0] for s in new['seeds']}:
        raise Refused(f'--to: the explorer names {new["gatewayHost"]} as its gateway host, which is not a seed of the new '
                      'network; pass --gateway-host')
    problems = []
    apply_anchors(tree, fields(cur), fields(new), problems)
    if edits is not None and not problems:
        apply_edits(tree, edits, problems)
    if not problems:
        # The tree as it would be written, checked as --check checks it: an edit that moves an anchor or adds a copy of
        # an identity value inside an anchored file is refused here, not by the next check.
        after = []
        apply_anchors(tree, fields(new), fields(new), after)
        problems += [f'after the {"edits" if edits is not None else "switch"}: {line}' for line in after]
    if problems:
        return problems, {}, new
    new['keptOut'] = finish(tree, problems, downloads, new['keptOut'])
    tree_checks(tree, new, retired, problems)
    declared = (json.dumps(new, indent=2, ensure_ascii=False) + '\n').encode('utf-8')
    if declared != tree.read(DECLARED):
        tree.out[DECLARED] = declared
    return problems, tree.changed(), new

def write(root, changed):
    """Each new file is written beside its original first; then each original is replaced whole, the files a rename
    leaves behind are removed, and DECLARED is replaced last, so an interrupted run never declares an identity the
    files do not carry."""
    staged = []
    try:
        for rel in sorted((r for r, d in changed.items() if d is not None), key=lambda r: r == DECLARED):
            path = root / rel
            tmp = path.with_name(path.name + '.site-identity.tmp')
            staged.append((tmp, path))
            tmp.write_bytes(changed[rel])
            if path.exists():
                shutil.copymode(path, tmp)
    except OSError as e:
        for tmp, _ in staged:
            tmp.unlink(missing_ok=True)
        raise Refused(f'{e}; nothing was changed') from None
    last = staged.pop() if staged and staged[-1][1] == root / DECLARED else None
    for tmp, path in staged:
        os.replace(tmp, path)
    for rel, data in changed.items():
        if data is None:
            (root / rel).unlink()
    if last:
        os.replace(*last)

def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('--to', type=Path, help='the invitation (network.pocol.json) of the network the websites are switched to')
    p.add_argument('--commit', help='point every source link at this commit (40 hex digits)')
    p.add_argument('--rpc-url', help="the IDE's RPC endpoint (https, or http://127.0.0.1:<port>/<path> for a test copy)")
    p.add_argument('--gateway-host', help='the host the explorer names as its gateway host (default: unchanged)')
    p.add_argument('--edits', type=Path, help='a JSON list of anchored edits [{"file", "old", "new", "count"}]')
    p.add_argument('--download', action='append', metavar='PATH=SHA256',
                   help=f'the {SUMS} line of a release file kept out of Git, PATH <site>/downloads/<name> (repeatable)')
    p.add_argument('--allow-revert', action='store_true', help=f'allow a --to identity listed in {RETIRED}')
    p.add_argument('--check', action='store_true', help='verify only; fails if the run would change any file')
    p.add_argument('--root', type=Path, default=ROOT)
    a = p.parse_args(argv)
    try:
        if a.rpc_url is not None:
            check_rpc_url(a.rpc_url)
        edits = None
        if a.edits is not None:
            try:
                edits = json.loads(a.edits.read_text(encoding='utf-8'))
            except (OSError, ValueError) as e:
                raise Refused(f'--edits: {e}') from None
            if not isinstance(edits, list) or not edits:
                raise Refused('--edits: a non-empty JSON list')
        downloads = parse_downloads(a.download)
        problems, changed, new = plan(a.root, a.to, a.commit, a.rpc_url, edits, a.allow_revert, a.gateway_host, downloads)
    except Refused as e:
        print('FAIL', e, file=sys.stderr)
        return 2
    if a.check and changed:
        problems += [f'{rel}: {"would be removed" if d is None else "does not carry the declared identity or its derived content"}'
                     for rel, d in sorted(changed.items())]
    for line in problems:
        print('FAIL', line, file=sys.stderr)
    if problems:
        return 2
    if not a.check:
        try:
            write(a.root, changed)
        except Refused as e:
            print('FAIL', e, file=sys.stderr)
            return 2
    print('verified' if a.check else 'updated', f'{len(ANCHORS)} anchors;', 'changed:', len(changed),
          f'chainId={new["chainId"]} genesisHash={new["genesisHash"]}')
    return 0

if __name__ == '__main__':
    sys.exit(main())
