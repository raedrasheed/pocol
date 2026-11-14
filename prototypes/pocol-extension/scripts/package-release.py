#!/usr/bin/env python3
"""Package only the built extension with reproducible archive metadata.

    python3 scripts/package-release.py [OUTPUT.zip] [--retired FILE]

The packager refuses anything that is not the release build (Sites W3), independently of the tests on dist/:
  - the manifest name is exactly RELEASE_NAME (so "[TEST FAULT ...]" and "[REHEARSAL]" builds are refused), MV3, and
    its version is package.json's;
  - the manifest has only the release's top-level keys (no optional_permissions, optional_host_permissions,
    web_accessible_resources, externally_connectable or anything new), and permissions, host_permissions,
    content_scripts, background, sandbox.pages and the extension-page CSP are exactly the release values;
  - the sandbox CSP is the release (closed) policy for the sha256 of the loader actually inlined in sandbox.html;
  - the built-in profile (lib/studio-network.js NETWORK) uses the pinned RN endpoint;
  - the built-in profile is not the network placeholder (D1, Sites W9): neither by its NETWORK_PLACEHOLDER mark nor by
    the placeholder identity (deploy/mainnet/placeholder-network.pocol.json, and the one written here);
  - no file mentions the retired chatgpt.site preview origins;
  - no file shows a retired network: a chain ID of deploy/mainnet/retired-networks.json as a decimal word, or its
    genesis hash (so a tree whose NETWORK was repinned but whose pages, e.g. publisher.html and contract.html, were
    not is refused too);
  - the decoder decodes spec 2;
  - the built-in profile's chain ID and genesis hash are not in deploy/mainnet/retired-networks.json (or --retired FILE).
Until the identity switch the tree carries the placeholder, so no release is packaged: CI checks that this packager
refuses the build with "network placeholder" instead (.github/workflows).
"""
from pathlib import Path
import argparse
import base64
import hashlib
import json
import re
import sys
import zipfile

RELEASE_NAME = 'PoCol Studio, IDE & Viewer'
HOST_PERMISSIONS = ['http://127.0.0.1/*', 'https://gateway.pocol.net/*']
PERMISSIONS = ['storage', 'unlimitedStorage', 'declarativeNetRequest']
BACKGROUND = {'service_worker': 'sw.js', 'type': 'module'}
SANDBOX = {'pages': ['sandbox.html']}
# Every top-level key the release manifest may have (build.mjs). Anything else, e.g. optional_permissions,
# web_accessible_resources or externally_connectable, is refused; W14 adds the fixed `key` (Q7) here with its value.
MANIFEST_KEYS = {'manifest_version', 'name', 'version', 'description', 'minimum_chrome_version', 'background', 'permissions',
                 'host_permissions', 'content_scripts', 'omnibox', 'options_page', 'icons', 'action', 'sandbox',
                 'content_security_policy'}
CONTENT_SCRIPTS = [
    {'matches': ['https://studio.pocol.net/*'], 'js': ['studio-content.js'], 'run_at': 'document_start', 'all_frames': False},
    {'matches': ['https://ide.pocol.net/*'], 'js': ['ide-content.js'], 'run_at': 'document_start', 'all_frames': False},
]
EXTENSION_PAGES_CSP = "script-src 'self'; object-src 'self'"
RELEASE_ENDPOINT = 'https://gateway.pocol.net/studio/rpc'
SANDBOX_PREFIX = '<!doctype html>\n<html><head><meta charset="utf-8"><script>'
SANDBOX_SUFFIX = '</script></head><body></body></html>\n'


def sandbox_csp(loader_hash):
    """build.mjs SANDBOX_CSP(hash, false): the closed release policy (kept in step by tests/package_release_test.mjs)."""
    return (f"sandbox allow-scripts allow-forms; default-src 'none'; script-src 'sha256-{loader_hash}' blob:; "
            "style-src blob: 'unsafe-inline'; img-src blob: data:; font-src blob:; media-src blob:; connect-src 'none'; "
            "frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none'")


def refuse(message):
    raise SystemExit('Refusing to package: ' + message)


def built_network(text):
    m = re.search(r'export const NETWORK = Object\.freeze\((\{.*?\})\);', text, re.S)
    if not m:
        refuse('lib/studio-network.js has no NETWORK = Object.freeze({...}) object')
    try:
        return json.loads(m.group(1))
    except ValueError:
        refuse('lib/studio-network.js NETWORK is not plain JSON')


def retired_patterns(retired):
    """(label, bytes regex) for each retired chain ID (a decimal word: no letter or digit on either side) and genesis
    hash (hex, any case, with or without 0x)."""
    out = []
    for r in retired:
        cid, genesis = r['chainId'], str(r['genesisHash']).lower()
        if not isinstance(cid, int) or isinstance(cid, bool) or cid < 0 or not re.fullmatch(r'0x[0-9a-f]{64}', genesis):
            raise ValueError('retired entry')
        out.append((f'chain ID {cid}', re.compile(rb'(?<![0-9A-Za-z])' + str(cid).encode() + rb'(?![0-9A-Za-z])')))
        out.append((f'genesis {genesis}', re.compile(re.escape(genesis[2:].encode()), re.I)))
    return out


def refuse_placeholder(root, network):
    """D1 (Sites W9): until the identity switch the built-in profile is a placeholder of no network (NETWORK_PLACEHOLDER
    in lib/studio-network.js, written by deploy/mainnet/set_network_identity.py --placeholder); no release carries it,
    neither by its mark nor by its identity (a build whose mark was dropped by hand or by a merge still pins it)."""
    if 'NETWORK_PLACEHOLDER' in network:
        raise SystemExit('Refusing a build whose built-in profile is the network placeholder (NETWORK_PLACEHOLDER): '
                         'a release is packaged only after the identity switch pins the real network')
    # The placeholder identity (deploy/mainnet/placeholder-network.pocol.json: the decoder corpus's v3 devnet GenesisSpec
    # under chain ID 1234567891, which no network uses), written here so the refusal holds outside the repository too; the
    # repository's placeholder file, when present, is refused as well (a re-placed placeholder).
    placeholders = {(1234567891, '3130bfed4d9f4b4f6df657220d94035c21bfe7cc445df9d5572979dc4767f74f')}
    placeholder_file = root.parents[1] / 'deploy' / 'mainnet' / 'placeholder-network.pocol.json'
    if placeholder_file.is_file():
        try:
            info = json.loads(json.loads(placeholder_file.read_text(encoding='utf-8'))['files']['devnet.json'])
            placeholders.add((int(info['chainId']), str(info['genesisHash']).lower().removeprefix('0x')))
        except (KeyError, TypeError, ValueError) as e:
            raise SystemExit(f'Refusing to package: {placeholder_file} names no placeholder identity ({e})')
    for chain_id, genesis in sorted(placeholders):
        if re.search(rf'(?<![0-9A-Za-z_]){chain_id}(?![0-9])|(?<![0-9A-Za-z_])0x{chain_id:x}(?![0-9A-Fa-f])', network, re.IGNORECASE) \
                or re.search(rf'(?<![0-9A-Fa-f]){genesis}(?![0-9A-Fa-f])', network, re.IGNORECASE):
            raise SystemExit(f'Refusing a build whose built-in profile pins the network placeholder identity (chain {chain_id}, '
                             f'genesis 0x{genesis}) without its mark: run the identity switch before packaging a release')


def check_release(root, dist, version, retired_file):
    manifest = json.loads((dist / 'manifest.json').read_text(encoding='utf-8'))
    if manifest.get('name') != RELEASE_NAME:
        refuse(f'manifest name {manifest.get("name")!r} is not the release name {RELEASE_NAME!r} (fault or rehearsal build)')
    if manifest.get('manifest_version') != 3 or manifest.get('version') != version:
        refuse('the manifest is not MV3 at package.json version ' + version + ': rebuild with npm run build')
    if manifest.get('host_permissions') != HOST_PERMISSIONS or 'optional_host_permissions' in manifest:
        refuse(f'host_permissions are {manifest.get("host_permissions")}, not exactly {HOST_PERMISSIONS}')
    if manifest.get('content_scripts') != CONTENT_SCRIPTS:
        refuse('content_scripts do not match exactly https://studio.pocol.net/* and https://ide.pocol.net/*')
    if 'externally_connectable' in manifest:
        refuse('the release manifest must not be externally_connectable')
    if manifest.get('permissions') != PERMISSIONS or 'optional_permissions' in manifest:
        refuse(f'permissions are {manifest.get("permissions")}, not exactly {PERMISSIONS} (and no optional_permissions)')
    if 'web_accessible_resources' in manifest:
        refuse('the release manifest must not expose web_accessible_resources')
    if manifest.get('sandbox') != SANDBOX:
        refuse(f'the sandbox pages are {manifest.get("sandbox")}, not exactly {SANDBOX}')
    if manifest.get('background') != BACKGROUND:
        refuse(f'the background is {manifest.get("background")}, not exactly {BACKGROUND}')
    extra = sorted(set(manifest) - MANIFEST_KEYS)
    if extra:
        refuse('the manifest has keys the release does not: ' + ', '.join(extra))
    page = (dist / 'sandbox.html').read_text(encoding='utf-8')
    if not (page.startswith(SANDBOX_PREFIX) and page.endswith(SANDBOX_SUFFIX)):
        refuse('sandbox.html is not the built sandbox page')
    loader = page[len(SANDBOX_PREFIX):-len(SANDBOX_SUFFIX)]
    loader_hash = base64.b64encode(hashlib.sha256(loader.encode('utf-8')).digest()).decode('ascii')
    csp = manifest.get('content_security_policy') or {}
    if csp.get('sandbox') != sandbox_csp(loader_hash):
        refuse('the sandbox CSP is not the release policy for the loader in sandbox.html')
    if csp.get('extension_pages') != EXTENSION_PAGES_CSP:
        refuse('the extension-page CSP is not the release policy')
    network_text = (dist / 'lib/studio-network.js').read_text(encoding='utf-8')
    # D1 is refused before NETWORK is parsed and before the retired identities, which the placeholder tree's pages still
    # show: a placeholder build fails as the placeholder (the refusal CI expects), whatever else it holds.
    refuse_placeholder(root, network_text)
    network = built_network(network_text)
    if network.get('endpoint') != RELEASE_ENDPOINT or network.get('trustLevel') != 'RN':
        refuse(f'the built-in profile must use {RELEASE_ENDPOINT} at trust level RN')
    try:
        retired = json.loads(retired_file.read_text(encoding='utf-8'))['retired']
        shown = retired_patterns(retired)
    except (OSError, ValueError, KeyError, TypeError, AttributeError):
        refuse(f'cannot read the retired network identities from {retired_file}')
    shown_in = []
    for path in sorted(dist.rglob('*')):
        if not path.is_file():
            continue
        data, name = path.read_bytes(), path.relative_to(dist).as_posix()
        if b'chatgpt.site' in data:
            refuse(f'{name} names a retired chatgpt.site preview origin')
        hits = sorted({label for label, pattern in shown if pattern.search(data)})
        if hits:
            shown_in.append(f'{name} shows the retired network ' + ', '.join(hits))
    # The release decodes the v3 GenesisSpec (spec 2) by default; a stale dist/ built before that (SPEC_VERSION 1) is refused.
    if 'export const SPEC_VERSION = 2,' not in (dist / 'lib/refdec.js').read_text(encoding='utf-8'):
        refuse('a stale build that does not decode spec 2: rebuild the release with npm run build')
    identity = []
    genesis = str(network.get('genesisHash', '')).lower()
    for r in retired:
        if r.get('chainId') == network.get('chainId') or str(r.get('genesisHash', '')).lower() == genesis:
            identity.append(f'the built-in profile pins the retired network {r.get("chainId")} ({r.get("genesisHash")})')
    identity += shown_in
    if identity:
        refuse('; '.join(identity) + '. A release needs the mainnet identity (Phase 10)')


def main(argv):
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('output', nargs='?', type=Path)
    parser.add_argument('--retired', type=Path, default=root.parents[1] / 'deploy' / 'mainnet' / 'retired-networks.json')
    args = parser.parse_args(argv)
    dist = root / 'dist'
    version = json.loads((root / 'package.json').read_text())['version']
    output = args.output or root / 'releases' / f'PoCol-Studio-IDE-Viewer-{version}.zip'
    required = ['lib/forks.js', 'lib/rp.js', 'lib/refdec.js', 'lib/studio-network.js', 'sandbox.html', 'icons/pocol-16.png', 'icons/pocol-32.png', 'icons/pocol-48.png', 'icons/pocol-128.png', 'manifest.json', 'contract.html', 'contract-ui.js', 'lib/contracts.js',
                'tools/Export-Studio-Wallet.ps1', 'tools/export-studio-wallet.py', 'README-EN.md', 'README-AR.md',
                'LICENSE', 'NOTICE', 'vendor/hashes/LICENSE', 'vendor/secp256k1/LICENSE',
                'vendor/ethers-LICENSE.md']
    for name in required:
        if not (dist / name).is_file():
            raise SystemExit(f'Missing built release file: {name}. Run npm run build first.')
    check_release(root, dist, version, args.retired)
    output.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(output, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
        for path in sorted(dist.rglob('*')):
            if path.is_symlink():
                raise SystemExit(f'Release must not contain symbolic links: {path.name}')
            if not path.is_file():
                continue
            relative = path.relative_to(dist).as_posix()
            if 'node_modules' in path.parts or 'dist-fault' in relative or relative.endswith('.pocol-key'):
                raise SystemExit(f'Unexpected release member: {relative}')
            entry = zipfile.ZipInfo(relative, date_time=(1980, 1, 1, 0, 0, 0))
            entry.compress_type = zipfile.ZIP_DEFLATED
            entry.external_attr = 0o100644 << 16
            archive.writestr(entry, path.read_bytes())
    digest = hashlib.sha256(output.read_bytes()).hexdigest()
    output.with_suffix(output.suffix + '.sha256').write_text(f'{digest}  {output.name}\n')
    print(f'{output}\nSHA-256 {digest}\n{output.stat().st_size} bytes')


if __name__ == '__main__':
    main(sys.argv[1:])
