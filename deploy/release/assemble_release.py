#!/usr/bin/env python3
"""Assemble the downloadable files of one PoCol release into one directory with one SHA256SUMS, ready for the owner
to sign (deploy/release/README.md). Standard library only.

    python3 assemble_release.py --out DIR FILE...

Only these names are accepted (V is deploy/standalone/package.py's version; the extension's package.json version
must be the same V, owner decision 2026-11-16, or nothing is assembled):

    PoCol-Mainnet-V-linux-x86_64.tar.gz     the Linux package (package.py --platform Linux)
    PoCol-Mainnet-V-windows-x86_64.zip      the Windows package (package.py --platform Windows)
    PoCol-Mainnet-V-arm64.apk               the Android app, already signed by the owner with apksigner
    PoCol-Studio-IDE-Viewer-V.zip           the Chrome extension (scripts/package-release.py)
    network.pocol.json                      the mainnet invitation, when it is published with the release

It refuses an extension version that is not V (before the identity switch package.json still says 0.4.1, so no
release is assembled), an `-unsigned` APK name, an APK without an APK Signing Block at its place (the 16-byte magic
`APK Sig Block 42` just before the ZIP central directory, with matching size fields and a v2 or v3 signature), an
extension zip whose manifest is not the release build's (a [REHEARSAL] or fault build), an invitation that is not
the mainnet's (package.py --network's checks: schema pocol-network-v2, the node's consensus rules, a spec-2 genesis,
deploy/mainnet/v3-profile.json's chain ID, not the placeholder and not a retired network), duplicates, unknown names
and an existing non-empty DIR. It copies the files, writes SHA256SUMS (sorted, LF) and prints the next step. It signs
nothing: the signatures are the owner's (sign-release.sh or Sign-PoColRelease.ps1), and verify_release.py says NOT
VERIFIED until they are there.
"""
import argparse, hashlib, json, re, runpy, shutil, struct, sys, zipfile
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
SUMS = 'SHA256SUMS'
APK_SIG_BLOCK = b'APK Sig Block 42'
# The APK Signature Scheme v2, v3 and v3.1 block IDs (apksigner); one of them must be in the signing block.
APK_SIGNATURE_IDS = {0x7109871a, 0xf05368c0, 0x1b93ad61}
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
    # Owner decision 2026-11-16: the native release version is the extension's. Before the identity switch the
    # extension stays at 0.4.1 (the retired network's published build), so no release can be assembled until then.
    if e != v:
        raise Refused(f'the extension version {e} (prototypes/pocol-extension/package.json) is not the native release '
                      f'version {v} (deploy/standalone/package.py); a release is assembled only after the identity switch '
                      f'sets the extension to {v}')
    return {f'PoCol-Mainnet-{v}-linux-x86_64.tar.gz': 'linux', f'PoCol-Mainnet-{v}-windows-x86_64.zip': 'windows',
            f'PoCol-Mainnet-{v}-arm64.apk': 'apk', f'PoCol-Studio-IDE-Viewer-{v}.zip': 'extension',
            'network.pocol.json': 'invitation'}


def apk_signing_block(data):
    """The ID-value pairs of the APK Signing Block, or None. The block sits just before the central directory that the
    end-of-central-directory record names: u64 size, the pairs (u64 length, u32 ID, value), u64 size again, then the
    magic. Only its real position counts; the magic anywhere else (an entry's bytes) is not a signature."""
    if not data.startswith(b'PK\x03\x04') or len(data) < 22:
        return None
    eocd = data.rfind(b'PK\x05\x06', max(0, len(data) - 22 - 0xffff))
    if eocd < 0 or eocd + 22 > len(data):
        return None
    cd_size, cd_offset, comment = struct.unpack_from('<IIH', data, eocd + 12)
    if eocd + 22 + comment != len(data) or cd_offset + cd_size != eocd or cd_offset < 32:
        return None
    if data[cd_offset - 16:cd_offset] != APK_SIG_BLOCK:
        return None
    (size,) = struct.unpack_from('<Q', data, cd_offset - 24)
    start = cd_offset - 8 - size
    if size < 32 or start < 0 or struct.unpack_from('<Q', data, start)[0] != size:
        return None
    pairs, i, end = {}, start + 8, cd_offset - 24
    while i < end:
        if i + 12 > end:
            return None
        length, ident = struct.unpack_from('<QI', data, i)
        if length < 4 or i + 8 + length > end:
            return None
        pairs[ident] = data[i + 12:i + 8 + length]
        i += 8 + length
    return pairs


def check_apk(path):
    if '-unsigned' in path.name:
        raise Refused(f'{path.name}: an unsigned APK is never a release file; sign it with apksigner first')
    pairs = apk_signing_block(path.read_bytes())
    if pairs is None or not APK_SIGNATURE_IDS & set(pairs):
        raise Refused(f'{path.name}: no APK Signing Block (v2 or v3) before the central directory; sign it with '
                      'apksigner (release key) first')


def known_identities(path, list_key=None):
    """(chainId, lowercase genesisHash) pairs of a committed file, as package.py reads them: retired-networks.json's
    `retired` list, or the devnet.json of an invitation (the placeholder). A missing or malformed file refuses."""
    try:
        doc = json.loads(path.read_text(encoding='utf-8'))
        entries = doc[list_key] if list_key else [json.loads(doc['files']['devnet.json'])]
        pairs = {(e['chainId'], str(e['genesisHash']).lower()) for e in entries}
        if not pairs or any(type(c) is not int or not re.fullmatch(r'0x[0-9a-f]{64}', g) for c, g in pairs):
            raise ValueError('no identity')
        return pairs
    except (OSError, KeyError, TypeError, ValueError) as e:
        raise Refused(f'{path} names no network identity ({e})') from None


def consensus_rules_version(root=ROOT):
    """The node's consensus rules version (prototypes/pocold/src/identity.rs CONSENSUS_RULES_VERSION)."""
    text = (root / 'prototypes/pocold/src/identity.rs').read_text(encoding='utf-8')
    m = re.search(r'(?m)^pub const CONSENSUS_RULES_VERSION: u32 = ([0-9]+);$', text)
    if not m:
        raise Refused('prototypes/pocold/src/identity.rs states no CONSENSUS_RULES_VERSION')
    return int(m[1])


def check_invitation(path, root=ROOT):
    """package.py --network's identity checks: the mainnet invitation only, never the rehearsal's, the placeholder, a
    retired network or a spec-1 genesis."""
    try:
        invitation = json.loads(path.read_bytes().decode('utf-8'))
        info = json.loads(invitation['files']['devnet.json'])
        schema, rules = invitation.get('schema'), invitation.get('consensusRulesVersion')
        chain, genesis = info.get('chainId'), str(info.get('genesisHash', '')).lower()
    except (OSError, KeyError, TypeError, AttributeError, ValueError) as e:
        raise Refused(f'{path.name}: not a network invitation ({e})') from None
    if schema != 'pocol-network-v2' or type(rules) is not int or rules != consensus_rules_version(root):
        raise Refused(f'{path.name}: the invitation binds other consensus rules (schema {schema!r}, rules {rules!r})')
    if type(chain) is not int or not re.fullmatch(r'0x[0-9a-f]{64}', genesis):
        raise Refused(f'{path.name}: the invitation names no chain ID and genesis hash')
    mainnet = root / 'deploy/mainnet'
    def names(pairs):
        return any(chain == c or genesis == g for c, g in pairs)
    if names(known_identities(mainnet / 'placeholder-network.pocol.json')):
        raise Refused(f'{path.name}: the invitation is the network placeholder, not a network')
    if names(known_identities(mainnet / 'retired-networks.json', 'retired')):
        raise Refused(f'{path.name}: the invitation is for a retired network')
    identity = runpy.run_path(str(root / 'deploy/public-testnet/pocol_identity.py'))
    try:
        spec = identity['spec_version'](info)
    except ValueError as e:
        raise Refused(f'{path.name}: the invitation declares no valid GenesisSpec version ({e})') from None
    if spec != 2:
        raise Refused(f'{path.name}: the invitation is for a spec-1 network; a release is for a v3 (spec-2) network only')
    try:
        mainnet_chain = json.loads((mainnet / 'v3-profile.json').read_text(encoding='utf-8'))['chainId']
    except (OSError, KeyError, TypeError, ValueError) as e:
        raise Refused(f'no mainnet profile chain ID ({e})') from None
    if chain != mainnet_chain:
        raise Refused(f'{path.name}: the invitation is for chain ID {chain}, not the mainnet (deploy/mainnet/v3-profile.json '
                      f'chainId {mainnet_chain}); the rehearsal invitation is never a release file')


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
            check_invitation(f, root)
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
