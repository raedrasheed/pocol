#!/usr/bin/env python3
"""Package the tested Studio, extension and gateway using Python's standard library.

Run the extension build and component tests first. This script does not install
dependencies, sign transactions, deploy services, or upload anything.

Before the signed release (owner policy 2026-11-22: no extension package link or listed digest until then), the
Chrome ZIP is a release artifact for the owner to sign, not a Studio download: it is not copied into dist/downloads
nor into the cPanel ZIP, and Studio's install dialog (dist/app.js) must link pocol.net's release verification
(RELEASE_PAGE) and name no extension package.
"""
import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import zipfile


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


# An extension package name (the versioned release ZIP or the old unversioned one), like the one
# deploy/websites/tests/test_downloads.py refuses on every site.
EXTENSION_PACKAGE = re.compile(r'PoCol-Studio-(?:IDE-Viewer-[^"\'<>\s/]+?|Chrome)\.zip')
RELEASE_PAGE = 'https://pocol.net/chrome-extension.html#verify-release'


def check_install_dialog(app_js):
    """Studio's install dialog links pocol.net's release verification, and no extension package, before the signed
    release; raises ValueError otherwise."""
    packages = sorted(set(EXTENSION_PACKAGE.findall(app_js)))
    if packages:
        raise ValueError(f'dist/app.js names the extension package(s) {packages}: no extension package is linked '
                         'before the signed release; link ' + RELEASE_PAGE + ' instead.')
    if RELEASE_PAGE not in app_js:
        raise ValueError(f'dist/app.js does not link {RELEASE_PAGE} (the extension release and verification).')


def files_under(root):
    result = []
    for path in sorted(root.rglob('*')):
        if path.is_symlink():
            raise ValueError(f'Symbolic links are not allowed in release inputs: {path}')
        if path.is_file():
            result.append(path)
    return result


def write_zip(destination, entries):
    """Fixed ZIP metadata makes byte-identical inputs produce the same archive."""
    names = set()
    with zipfile.ZipFile(destination, 'w', compression=zipfile.ZIP_DEFLATED,
                         compresslevel=9) as archive:
        for name, path in sorted(entries):
            if name in names or name.startswith('/') or '..' in Path(name).parts:
                raise ValueError(f'Unsafe or duplicate archive path: {name}')
            names.add(name)
            info = zipfile.ZipInfo(name, date_time=(1980, 1, 1, 0, 0, 0))
            info.create_system = 3
            info.external_attr = 0o100644 << 16
            info.compress_type = zipfile.ZIP_DEFLATED
            archive.writestr(info, path.read_bytes(), compresslevel=9)
    with zipfile.ZipFile(destination) as archive:
        failed = archive.testzip()
        if failed:
            raise ValueError(f'Corrupt release entry: {failed}')


def main():
    studio_default = Path(__file__).resolve().parents[1]
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--studio-dir', type=Path, default=studio_default)
    parser.add_argument('--extension-dir', type=Path)
    parser.add_argument('--gateway-dir', type=Path)
    parser.add_argument('--output-dir', type=Path, required=True)
    parser.add_argument('--guide', type=Path, help='Optional Arabic quick-start to copy into the release')
    args = parser.parse_args()
    studio = args.studio_dir.resolve()
    extension = (args.extension_dir or studio.parent / 'pocol-extension').resolve()
    gateway = (args.gateway_dir or studio.parents[1] / 'deploy/studio').resolve()
    output = args.output_dir.resolve()
    if any(output == root or root in output.parents for root in (studio, extension, gateway)):
        raise ValueError('Use an output directory outside the source inputs.')
    dist = studio / 'dist'
    extension_dist = extension / 'dist'
    required = [dist / 'index.html', dist / '.htaccess',
                extension_dist / 'manifest.json', extension / 'scripts/package-release.py',
                gateway / 'build_installer.py', gateway / 'README-AR.txt']
    for path in required:
        if not path.is_file() or path.is_symlink():
            raise ValueError(f'Missing regular release input: {path}')
    manifest = json.loads((extension_dist / 'manifest.json').read_text())
    if manifest.get('manifest_version') != 3 or '[TEST FAULT' in manifest.get('name', ''):
        raise ValueError('The extension must be a release MV3 build, never a fault build.')
    version = json.loads((extension / 'package.json').read_text())['version']
    if manifest.get('version') != version:
        raise ValueError('Extension build is stale: package and manifest versions differ.')
    # Studio's install dialog links the release verification page, not a package (owner policy 2026-11-22).
    check_install_dialog((dist / 'app.js').read_text(encoding='utf-8'))
    # dist/ is uploaded as it is (README.md), so a package an older builder copied into dist/downloads would be served.
    stale = sorted(p.relative_to(dist).as_posix() for p in files_under(dist)
                   if p.relative_to(dist).parts[0] == 'downloads' and EXTENSION_PACKAGE.search(p.name))
    if stale:
        raise ValueError(f'dist/ holds the extension package(s) {stale}: delete them; no extension package is served '
                         'before the signed release.')
    subprocess.run([sys.executable, str(gateway / 'build_installer.py')], check=True)
    output.mkdir(parents=True, exist_ok=True)

    chrome_zip = output / f'PoCol-Studio-IDE-Viewer-{version}.zip'
    extension_inputs = files_under(extension_dist)
    # The Chrome ZIP is a release artifact for signing only; the Studio web package carries no downloads/.
    subprocess.run([sys.executable, str(extension / 'scripts/package-release.py'), str(chrome_zip)], check=True)

    web_inputs = []
    for path in files_under(dist):
        rel = path.relative_to(dist)
        if '.openai' in rel.parts or rel.parts[0] == 'downloads':
            continue
        web_inputs.append((rel.as_posix(), path))
    write_zip(output / 'PoCol-Studio-cPanel.zip', web_inputs)

    installer = gateway / 'Install-PoCol-Studio-Gateway.sh'
    shutil.copyfile(installer, output / installer.name)
    gateway_names = ['README-AR.txt', 'pocol_studio_gateway.py', 'install_studio_gateway.py',
                     'build_installer.py', 'test_studio_gateway.py', installer.name]
    if (gateway / 'README.md').is_file():
        gateway_names.append('README.md')
    write_zip(output / 'PoCol-Studio-Gateway-Source.zip',
              [(name, gateway / name) for name in gateway_names])

    artifact_names = [chrome_zip.name, 'PoCol-Studio-cPanel.zip', installer.name,
                      'PoCol-Studio-Gateway-Source.zip']
    for source, name in [(gateway / 'README-AR.txt', 'PoCol-Studio-Server-AR.txt'),
                         (studio / 'RELEASE-NOTES.md', 'RELEASE-NOTES.md')]:
        shutil.copyfile(source, output / name)
        artifact_names.append(name)
    if args.guide:
        guide = args.guide.resolve()
        if not guide.is_file() or guide.is_symlink():
            raise ValueError('The guide must be a regular file.')
        guide_name = 'PoCol-Studio-Quick-Start-AR.txt'
        shutil.copyfile(guide, output / guide_name)
        artifact_names.append(guide_name)
    sources = {'studio/' + name: sha256(path) for name, path in web_inputs}
    sources.update({'extension/' + p.relative_to(extension_dist).as_posix(): sha256(p)
                    for p in extension_inputs})
    sources.update({'gateway/' + name: sha256(gateway / name) for name in gateway_names})
    summary = {
        'schema': 'pocol-studio-release-v1',
        'studioVersion': json.loads((studio / 'package.json').read_text())['version'],
        'extensionVersion': version,
        'artifacts': {name: {'sha256': sha256(output / name), 'bytes': (output / name).stat().st_size}
                      for name in artifact_names},
        'sourceSha256': sources,
        'verification': 'Packaging integrity only; see RELEASE-NOTES.md for test results and limits.',
    }
    release_manifest = output / 'release-manifest.json'
    release_manifest.write_text(json.dumps(summary, ensure_ascii=False, indent=2) + '\n', encoding='utf-8')
    (output / 'SHA256SUMS.txt').write_text(
        ''.join(f'{sha256(output / name)}  {name}\n'
                for name in artifact_names + [release_manifest.name]), encoding='utf-8')
    print(json.dumps({'output': str(output), 'artifacts': summary['artifacts']}, indent=2))


if __name__ == '__main__':
    main()
