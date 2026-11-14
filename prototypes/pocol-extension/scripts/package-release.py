#!/usr/bin/env python3
"""Package only the built extension with reproducible archive metadata.

    python3 scripts/package-release.py [OUTPUT.zip] [--retired FILE] [--unreleased]

The packager refuses anything that is not the release build (Sites W3), independently of the tests on dist/:
  - the manifest name is exactly RELEASE_NAME (so "[TEST FAULT ...]" and "[REHEARSAL]" builds are refused), MV3, and
    its version is package.json's;
  - host_permissions, content_scripts and the extension-page CSP are exactly the release values;
  - the sandbox CSP is the release (closed) policy for the sha256 of the loader actually inlined in sandbox.html;
  - the built-in profile (lib/studio-network.js NETWORK) uses the pinned RN endpoint;
  - no file mentions the retired chatgpt.site preview origins;
  - the decoder decodes spec 2;
  - the built-in profile is a release identity: not the placeholder (a NETWORK name containing "placeholder", or an
    @placeholder marker in studio-network.js) and its chain ID and genesis hash are not in
    deploy/mainnet/retired-networks.json (or --retired FILE).
--unreleased packages a CI artifact of a tree whose built-in identity is not a release identity yet (until the Phase 10
pin): every other refusal applies, the archive name must contain UNRELEASED, and it must never be published.
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


def built_network(dist):
    text = (dist / 'lib/studio-network.js').read_text(encoding='utf-8')
    m = re.search(r'export const NETWORK = Object\.freeze\((\{.*?\})\);', text, re.S)
    if not m:
        refuse('lib/studio-network.js has no NETWORK = Object.freeze({...}) object')
    try:
        return json.loads(m.group(1)), text
    except ValueError:
        refuse('lib/studio-network.js NETWORK is not plain JSON')


def check_release(root, dist, version, retired_file, unreleased):
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
    network, network_text = built_network(dist)
    if network.get('endpoint') != RELEASE_ENDPOINT or network.get('trustLevel') != 'RN':
        refuse(f'the built-in profile must use {RELEASE_ENDPOINT} at trust level RN')
    for path in sorted(dist.rglob('*')):
        if path.is_file() and b'chatgpt.site' in path.read_bytes():
            refuse(f'{path.relative_to(dist).as_posix()} names a retired chatgpt.site preview origin')
    # The release decodes the v3 GenesisSpec (spec 2) by default; a stale dist/ built before that (SPEC_VERSION 1) is refused.
    if 'export const SPEC_VERSION = 2,' not in (dist / 'lib/refdec.js').read_text(encoding='utf-8'):
        refuse('a stale build that does not decode spec 2: rebuild the release with npm run build')
    identity = []
    if 'placeholder' in str(network.get('name', '')).lower() or '@placeholder' in network_text:
        identity.append('the built-in profile is the placeholder identity')
    try:
        retired = json.loads(retired_file.read_text(encoding='utf-8'))['retired']
    except (OSError, ValueError, KeyError, TypeError):
        if not unreleased:
            refuse(f'cannot read the retired network identities from {retired_file}')
        retired = []
    genesis = str(network.get('genesisHash', '')).lower()
    for r in retired:
        if r.get('chainId') == network.get('chainId') or str(r.get('genesisHash', '')).lower() == genesis:
            identity.append(f'the built-in profile pins the retired network {r.get("chainId")} ({r.get("genesisHash")})')
    if identity and not unreleased:
        refuse('; '.join(identity) + '. A release needs the mainnet identity (Phase 10); use --unreleased for a CI artifact')
    return identity


def main(argv):
    root = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('output', nargs='?', type=Path)
    parser.add_argument('--retired', type=Path, default=root.parents[1] / 'deploy' / 'mainnet' / 'retired-networks.json')
    parser.add_argument('--unreleased', action='store_true')
    args = parser.parse_args(argv)
    dist = root / 'dist'
    version = json.loads((root / 'package.json').read_text())['version']
    suffix = '-UNRELEASED' if args.unreleased else ''
    output = args.output or root / 'releases' / f'PoCol-Studio-IDE-Viewer-{version}{suffix}.zip'
    if args.unreleased and 'UNRELEASED' not in output.name:
        refuse('an --unreleased archive must have UNRELEASED in its name')
    required = ['lib/forks.js', 'lib/rp.js', 'lib/refdec.js', 'lib/studio-network.js', 'sandbox.html', 'icons/pocol-16.png', 'icons/pocol-32.png', 'icons/pocol-48.png', 'icons/pocol-128.png', 'manifest.json', 'contract.html', 'contract-ui.js', 'lib/contracts.js',
                'tools/Export-Studio-Wallet.ps1', 'tools/export-studio-wallet.py', 'README-EN.md', 'README-AR.md',
                'LICENSE', 'NOTICE', 'vendor/hashes/LICENSE', 'vendor/secp256k1/LICENSE',
                'vendor/ethers-LICENSE.md']
    for name in required:
        if not (dist / name).is_file():
            raise SystemExit(f'Missing built release file: {name}. Run npm run build first.')
    identity = check_release(root, dist, version, args.retired, args.unreleased)
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
    if identity:
        print('UNRELEASED (never publish): ' + '; '.join(identity), file=sys.stderr)
    print(f'{output}\nSHA-256 {digest}\n{output.stat().st_size} bytes')


if __name__ == '__main__':
    main(sys.argv[1:])
