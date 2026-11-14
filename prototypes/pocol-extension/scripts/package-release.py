#!/usr/bin/env python3
"""Package only the built extension with reproducible archive metadata."""
from pathlib import Path
import hashlib
import json
import sys
import zipfile

root = Path(__file__).resolve().parents[1]
dist = root / 'dist'
version = json.loads((root / 'package.json').read_text())['version']
output = Path(sys.argv[1]) if len(sys.argv) > 1 else root / 'releases' / f'PoCol-Studio-IDE-Viewer-{version}.zip'
required = ['lib/forks.js', 'lib/rp.js', 'lib/refdec.js', 'icons/pocol-16.png', 'icons/pocol-32.png', 'icons/pocol-48.png', 'icons/pocol-128.png', 'manifest.json', 'contract.html', 'contract-ui.js', 'lib/contracts.js',
            'tools/Export-Studio-Wallet.ps1', 'tools/export-studio-wallet.py', 'README-EN.md', 'README-AR.md',
            'LICENSE', 'NOTICE', 'vendor/hashes/LICENSE', 'vendor/secp256k1/LICENSE',
            'vendor/ethers-LICENSE.md']
for name in required:
    if not (dist / name).is_file():
        raise SystemExit(f'Missing built release file: {name}. Run npm run build first.')
# A v3-preview build (build.mjs --v3-preview, dist-v3-preview/) decodes a spec-2 genesis at its entry points; the release
# keeps the SPEC_VERSION ceiling until activation, so a preview dist (or one of its files) copied over dist/ is refused.
if 'export const DECODE_CEILING = SPEC_VERSION;' not in (dist / 'lib/refdec.js').read_text(encoding='utf-8') \
        or 'PREVIEW' in json.loads((dist / 'manifest.json').read_text(encoding='utf-8'))['name']:
    raise SystemExit('Refusing a v3-preview build: rebuild the release with npm run build')
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
