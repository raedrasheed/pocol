#!/usr/bin/env python3
"""Package only the built extension with reproducible archive metadata."""
from pathlib import Path
import hashlib
import json
import re
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
# lib/studio-network.js, written by deploy/mainnet/set_network_identity.py --placeholder); no release carries it,
# neither by its mark nor by its identity (a build whose mark was dropped by hand or by a merge still pins it).
network = (dist / 'lib/studio-network.js').read_text(encoding='utf-8')
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
