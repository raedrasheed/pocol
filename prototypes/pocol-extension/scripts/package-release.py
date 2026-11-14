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
# The release decodes the v3 GenesisSpec (spec 2) by default; a stale dist/ built before that (SPEC_VERSION 1) is refused.
if 'export const SPEC_VERSION = 2,' not in (dist / 'lib/refdec.js').read_text(encoding='utf-8'):
    raise SystemExit('Refusing a stale build that does not decode spec 2: rebuild the release with npm run build')
# D1: until the identity switch the built-in profile is a placeholder of no network (NETWORK_PLACEHOLDER in
# lib/studio-network.js, written by deploy/mainnet/set_network_identity.py --placeholder); no release carries it.
if 'NETWORK_PLACEHOLDER' in (dist / 'lib/studio-network.js').read_text(encoding='utf-8'):
    raise SystemExit('Refusing a build whose built-in profile is the network placeholder (NETWORK_PLACEHOLDER): '
                     'a release is packaged only after the identity switch pins the real network')
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
