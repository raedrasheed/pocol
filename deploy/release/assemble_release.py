#!/usr/bin/env python3
"""Assemble the downloadable files of one PoCol release into one directory with one SHA256SUMS, ready for the owner
to sign (deploy/release/README.md). Standard library only.

    python3 assemble_release.py --out DIR FILE...

Only these names are accepted (V is deploy/standalone/package.py's version, E the extension's package.json version):

    PoCol-Mainnet-V-linux-x86_64.tar.gz     the Linux package (package.py --platform Linux)
    PoCol-Mainnet-V-windows-x86_64.zip      the Windows package (package.py --platform Windows)
    PoCol-Mainnet-V-arm64.apk               the Android app, already signed by the owner with apksigner
    PoCol-Studio-IDE-Viewer-E.zip           the Chrome extension (scripts/package-release.py)
    network.pocol.json                      the network invitation, when it is published with the release

It refuses an `-unsigned` APK name, an APK without an APK Signing Block (`APK Sig Block 42`), an extension zip whose
manifest is not the release build's (a [REHEARSAL] or fault build), duplicates, unknown names and an existing
non-empty DIR. It copies the files, writes SHA256SUMS (sorted, LF) and prints the next step. It signs nothing: the
signatures are the owner's (sign-release.sh or Sign-PoColRelease.ps1), and verify_release.py says NOT VERIFIED until
they are there.
"""
import argparse, hashlib, json, re, shutil, sys, zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
SUMS = 'SHA256SUMS'
APK_SIG_BLOCK = b'APK Sig Block 42'
RELEASE_NAME = 'PoCol Studio, IDE & Viewer'


class Refused(Exception):
    pass


def native_version(root=ROOT):
    text = (root / 'deploy/standalone/package.py').read_text(encoding='utf-8')
    return re.search(r"(?m)^version='([0-9]+\.[0-9]+\.[0-9]+)'$", text)[1]


def extension_version(root=ROOT):
    return json.loads((root / 'prototypes/pocol-extension/package.json').read_text(encoding='utf-8'))['version']


def release_name(root=ROOT):
    """The packager's release manifest name (prototypes/pocol-extension/scripts/package-release.py RELEASE_NAME)."""
    text = (root / 'prototypes/pocol-extension/scripts/package-release.py').read_text(encoding='utf-8')
    m = re.search(r"(?m)^RELEASE_NAME = '([^']+)'$", text)
    return m[1] if m else RELEASE_NAME


def allowed_names(root=ROOT):
    v, e = native_version(root), extension_version(root)
    return {f'PoCol-Mainnet-{v}-linux-x86_64.tar.gz': 'linux', f'PoCol-Mainnet-{v}-windows-x86_64.zip': 'windows',
            f'PoCol-Mainnet-{v}-arm64.apk': 'apk', f'PoCol-Studio-IDE-Viewer-{e}.zip': 'extension',
            'network.pocol.json': 'invitation'}


def check_apk(path):
    if '-unsigned' in path.name:
        raise Refused(f'{path.name}: an unsigned APK is never a release file; sign it with apksigner first')
    data = path.read_bytes()
    # The APK Signing Block (v2+) sits just before the ZIP central directory and ends with this magic.
    if APK_SIG_BLOCK not in data or not data.startswith(b'PK'):
        raise Refused(f'{path.name}: no APK Signing Block; sign it with apksigner (release key) first')


def check_extension(path, root=ROOT):
    try:
        with zipfile.ZipFile(path) as z:
            manifest = json.loads(z.read('manifest.json'))
    except (OSError, KeyError, ValueError, zipfile.BadZipFile) as e:
        raise Refused(f'{path.name}: not an extension package ({e})') from None
    if manifest.get('name') != release_name(root):
        raise Refused(f'{path.name}: manifest name {manifest.get("name")!r} is not the release build ({release_name(root)!r})')
    if manifest.get('version') != extension_version(root):
        raise Refused(f'{path.name}: manifest version {manifest.get("version")!r} is not package.json\'s')


def assemble(out, files, root=ROOT):
    out = Path(out)
    names = allowed_names(root)
    seen = {}
    for f in map(Path, files):
        if not f.is_file() or f.is_symlink():
            raise Refused(f'{f}: not a regular file')
        if f.name.endswith('.apk') and '-unsigned' in f.name:
            check_apk(f)
        if f.name not in names:
            raise Refused(f'{f.name}: not a release file name (allowed: {", ".join(sorted(names))})')
        if f.name in seen:
            raise Refused(f'{f.name}: given twice')
        kind = names[f.name]
        if kind == 'apk':
            check_apk(f)
        elif kind == 'extension':
            check_extension(f, root)
        elif kind == 'invitation':
            json.loads(f.read_text(encoding='utf-8'))
        seen[f.name] = f
    if not seen:
        raise Refused('no file given')
    if out.exists() and (not out.is_dir() or any(out.iterdir())):
        raise Refused(f'{out}: exists and is not an empty directory')
    out.mkdir(parents=True, exist_ok=True)
    for name, f in sorted(seen.items()):
        shutil.copyfile(f, out / name)
    sums = ''.join(f'{hashlib.sha256((out / n).read_bytes()).hexdigest()}  {n}\n' for n in sorted(seen))
    (out / SUMS).write_bytes(sums.encode('ascii'))
    return sorted(seen)


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('--out', type=Path, required=True)
    p.add_argument('files', nargs='+', type=Path)
    a = p.parse_args(argv)
    try:
        names = assemble(a.out, a.files)
    except (Refused, OSError, ValueError) as e:
        print(f'REFUSED: {e}', file=sys.stderr)
        return 1
    print(f'{a.out}/{SUMS}: {len(names)} files ({", ".join(names)})')
    print('UNSIGNED: the owner signs it next (deploy/release/README.md, sign-release.sh or Sign-PoColRelease.ps1)')
    return 0


if __name__ == '__main__':
    sys.exit(main())
