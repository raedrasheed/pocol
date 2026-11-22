#!/usr/bin/env python3
"""Package the release build for the Chrome Web Store.

    python3 scripts/package-store.py [OUTPUT.zip] [--retired FILE]          (every store upload; the recommended way)
    python3 scripts/package-store.py --first-upload-key KEY.pem --unofficial-key-pem [OUTPUT.zip]
                                                     (only if the owner chooses the unofficial key.pem method, once)

The release manifest has a "key" field: build.mjs writes the owner's public key of release/manifest-key.pub into it,
which gives the side-loaded release its fixed extension id. Developer reports say the Chrome Web Store refuses an
upload whose manifest has one (no current official page says so), and a store item gets its own id at its first
upload in any case. This packager makes the store upload from the same dist/:
  - it runs every check of scripts/package-release.py first (check_release: the release name, MV3 at package.json's
    version, the exact permissions, host permissions, content scripts, background, sandbox and CSPs, no network
    placeholder, no retired network, the committed owner key), so it refuses whatever that packager refuses;
  - the zip holds exactly the release zip's members with the same bytes, except manifest.json, which is the release
    manifest without "key" (the other fields and their order unchanged, written as build.mjs writes it);
  - the archive metadata is the release zip's (scripts/package-release.py write_release_zip), so the store zip is
    reproducible too, and its `<zip>.sha256` (integrity only) is written beside it.

The extension id is the owner's decision (store/README.md, "The store item's id"). The documented and recommended
route is to upload this zip without any key, accept the id the store assigns, and, if side-loaded builds should share
it, adopt the item's public key (dashboard, Package tab, View public key) in release/manifest-key.pub for later
releases; users move their wallets with the encrypted backup in every case. The alternative is the historical key.pem
method, which only an archived Chromium page describes: --first-upload-key KEY.pem writes `<name>-first-upload.zip`
with the matching private key added as key.pem, after checking with `openssl pkey` that KEY.pem is the private key of
release/manifest-key.pub. It is refused unless --unofficial-key-pem acknowledges that the method is unofficial, that
the zip holds the release private key, and that a failed attempt may leave the id reserved. That zip is never
published, signed or committed (no .sha256 is written for it); upload it once from the owner's machine and delete it.

Nothing here signs or uploads anything.
"""
from pathlib import Path
import argparse
import base64
import hashlib
import importlib.util
import json
import subprocess
import sys
import zipfile

HERE = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location('package_release', HERE / 'package-release.py')
release = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(release)

REQUIRED = ['lib/forks.js', 'lib/rp.js', 'lib/refdec.js', 'lib/studio-network.js', 'sandbox.html', 'icons/pocol-16.png',
            'icons/pocol-32.png', 'icons/pocol-48.png', 'icons/pocol-128.png', 'manifest.json', 'contract.html',
            'contract-ui.js', 'lib/contracts.js', 'tools/Export-Studio-Wallet.ps1', 'tools/export-studio-wallet.py',
            'README-EN.md', 'README-AR.md', 'LICENSE', 'NOTICE', 'vendor/hashes/LICENSE', 'vendor/secp256k1/LICENSE',
            'vendor/ethers-LICENSE.md']


def store_manifest(data):
    """The release manifest.json bytes without its "key" field, in build.mjs's form (JSON.stringify(m, null, 2) + LF)."""
    manifest = json.loads(data.decode('utf-8'))
    if not isinstance(manifest, dict) or 'key' not in manifest:
        raise SystemExit('Refusing to package for the store: the release manifest has no "key" (rebuild with npm run build)')
    del manifest['key']
    return (json.dumps(manifest, indent=2, ensure_ascii=False) + '\n').encode('utf-8')


def public_key_of(pem):
    """base64 DER SubjectPublicKeyInfo of a private key file, from `openssl pkey` (the key is never printed)."""
    try:
        out = subprocess.run(['openssl', 'pkey', '-in', str(pem), '-pubout', '-outform', 'DER'], capture_output=True, check=True)
    except FileNotFoundError:
        raise SystemExit('Refusing to package for the store: openssl is needed to check --first-upload-key')
    except subprocess.CalledProcessError:
        raise SystemExit(f'Refusing to package for the store: {pem} is not a readable private key')
    return base64.b64encode(out.stdout).decode('ascii')


def write_store_zip(dist, output, key_pem=None):
    """Write the store zip of dist/ (release members, manifest.json without "key", plus key.pem when given) with the
    release zip's fixed metadata; return its SHA-256. A zip without key.pem also gets its `<zip>.sha256`."""
    dist, output = Path(dist), Path(output)
    if dist.resolve() in output.resolve().parents:
        raise SystemExit('Refusing to package for the store: the zip must be written outside dist/')
    members = [(name, store_manifest(path.read_bytes()) if name == 'manifest.json' else path.read_bytes())
               for name, path in release.release_members(dist)]
    if key_pem is not None:
        if any(name == 'key.pem' for name, _ in members):
            raise SystemExit('Refusing to package for the store: dist/ already holds a key.pem')
        members = sorted(members + [('key.pem', Path(key_pem).read_bytes())])
    output.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(output, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=release.ZIP_LEVEL) as archive:
        for name, data in members:
            entry = zipfile.ZipInfo(name, date_time=release.ZIP_DATE)
            entry.create_system = 3
            entry.compress_type = zipfile.ZIP_DEFLATED
            entry.external_attr = (0o100600 if name == 'key.pem' else release.ZIP_MODE) << 16
            archive.writestr(entry, data, compresslevel=release.ZIP_LEVEL)
    digest = hashlib.sha256(output.read_bytes()).hexdigest()
    if key_pem is None:
        output.with_suffix(output.suffix + '.sha256').write_bytes(f'{digest}  {output.name}\n'.encode('utf-8'))
    return digest


def main(argv):
    root = HERE.parent
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument('output', nargs='?', type=Path)
    parser.add_argument('--retired', type=Path, default=root.parents[1] / 'deploy' / 'mainnet' / 'retired-networks.json')
    parser.add_argument('--first-upload-key', type=Path, metavar='KEY.pem',
                        help='the unofficial key.pem method only: the private key of release/manifest-key.pub, added as key.pem')
    parser.add_argument('--unofficial-key-pem', action='store_true',
                        help='acknowledge that the key.pem method is unofficial and puts the release private key into the zip')
    args = parser.parse_args(argv)
    if args.first_upload_key is not None and not args.unofficial_key_pem:
        raise SystemExit('Refusing --first-upload-key without --unofficial-key-pem: the key.pem upload is not an official Chrome Web '
                         'Store method and puts the release private key into the zip. The recommended way is the zip without a key '
                         '(store/README.md, "The store item\'s id").')
    dist = root / 'dist'
    version = json.loads((root / 'package.json').read_text(encoding='utf-8'))['version']
    for name in REQUIRED:
        if not (dist / name).is_file():
            raise SystemExit(f'Missing built release file: {name}. Run npm run build first.')
    release_id = release.check_release(root, dist, version, args.retired)
    if args.first_upload_key is not None:
        if public_key_of(args.first_upload_key) != release.manifest_key(root):
            raise SystemExit('Refusing to package for the store: --first-upload-key is not the private key of '
                             'release/manifest-key.pub, so the store item would not get the published extension id')
    stem = f'PoCol-Studio-IDE-Viewer-{version}-chrome-web-store'
    output = args.output or root / 'releases' / (stem + ('-first-upload.zip' if args.first_upload_key else '.zip'))
    digest = write_store_zip(dist, output, args.first_upload_key)
    print(f'{output}\nSHA-256 {digest}\n{output.stat().st_size} bytes\nextension id {release_id} (the side-loaded release\'s; the store item has it '
          f'only with the key.pem method, otherwise note the id the dashboard assigns)')
    if args.first_upload_key:
        print('CONTAINS THE PRIVATE KEY (key.pem): upload it once as the first upload, then delete it; never publish, sign or commit it')
    else:
        print('Chrome Web Store upload: manifest.json without "key"; every other file is the release build. The store assigns the '
              'item id at the first upload unless the owner chose the unofficial key.pem method (store/README.md)')


if __name__ == '__main__':
    main(sys.argv[1:])
