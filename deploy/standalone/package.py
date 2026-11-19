#!/usr/bin/env python3
"""Explicit allowlists only: no network keys, private state or Android signing keys."""
import argparse, gzip, hashlib, json, os, re, runpy, shutil, subprocess, tarfile, zipfile
from pathlib import Path
# The native release version (Linux, Windows and Android; owner decision 2026-11-16: 0.5.0). Its other copies
# (Wallet.cs, Wallet.xaml, build.gradle versionName, the pocold crate version that build-info reports, the shipped README
# and the Linux desktop texts) must equal it: test_release_package.py checks them, this script refuses a build-info of
# another version, and five-device-build.yml reads it from here (`--version`).
version='0.5.0'
p=argparse.ArgumentParser(); p.add_argument('--version',action='version',version=version)
p.add_argument('--platform',choices=['Linux','Windows','Android'],required=True); p.add_argument('--out',type=Path,required=True)
p.add_argument('--network',type=Path,help='the v3 (spec-2) invitation of the network this release is for; release-info then names its chain ID, genesis hash and consensusIdentity')
args=p.parse_args()
root=Path(__file__).resolve().parents[2]
# The consensus identity reference (standard library only; never imported from a package).
identity=runpy.run_path(str(root/'deploy/public-testnet/pocol_identity.py'))

def android_app():
    """The application id, versionCode and versionName the APK is built with (app/build.gradle, the only source)."""
    gradle=(root/'prototypes/pocol-android/app/build.gradle').read_text(encoding='utf-8')
    found={k:re.findall(rf"\b{k} {pattern}",gradle) for k,pattern in
           (('applicationId',r"'([a-z][a-z0-9_]*(?:\.[a-z][a-z0-9_]*)+)'"),('versionCode',r'([1-9][0-9]*)\b'),('versionName',r"'([^']+)'"))}
    if any(len(v)!=1 for v in found.values()): raise SystemExit('build.gradle must state one applicationId, versionCode and versionName')
    if found['versionName'][0]!=version: raise SystemExit(f"build.gradle versionName {found['versionName'][0]} is not the release version {version}")
    return found['applicationId'][0],int(found['versionCode'][0])

args.out.mkdir()
if args.platform=='Android':
    application_id,version_code=android_app()
    # The release stays unsigned until the owner signs it with the release key. An unsigned release is a
    # signing input, never an installable update or a freshly debug-signed APK.
    files={f'PoCol-Mainnet-{version}-arm64-unsigned.apk':root/'prototypes/pocol-android/app/build/outputs/apk/release/app-release-unsigned.apk'}
    infos=list((root/'prototypes/pocold/target/aarch64-linux-android/release/build').glob('pocold-*/out/build-info.json'))
    assert infos and len({json.loads(f.read_text(encoding='utf-8'))['compatibilitySha256'] for f in infos})==1, 'ambiguous Android build identity'
    # build.rs writes the raw build identity; readiness is computed at run time (V3-28 H1), which a host cannot do for
    # the Android binary. Say so in the same keys the desktop build-info carries, never as a computed value.
    android_info=json.loads(infos[0].read_text(encoding='utf-8'))
    android_info['publicLaunchReady']=False
    android_info['launchBlockers']=[{'id':'readiness-not-computed','detail':'this file is the Android build\'s raw identity; the computed readiness is in pocol_status on the device (pocold::readiness)'}]
    (args.out/'build-info.json').write_text(json.dumps(android_info,separators=(',',':')),encoding='utf-8',newline='\n')
else:
    suffix='.exe' if args.platform=='Windows' else ''
    files={n+suffix:root/'prototypes/pocold/target/release'/(n+suffix) for n in ['pocol','pocold','pocol-key','pocol-sign','pocol-site']}
    if suffix:
        files['PoCol-Wallet.exe']=root/'prototypes/pocold/target/release/PoCol-Wallet.exe'
        files['PoCol-Wallet-preview.png']=root/'wallet-preview.png'
    # encrypt-server-key.sh: the opt-in move of a server's key into an encrypted systemd credential (K1).
    for name in (['Start-PoCol.cmd','Initialize-PoColNetwork.ps1'] if suffix else ['install-linux.sh','encrypt-server-key.sh']): files[name]=root/'deploy/standalone'/name
    # FN-7: the genesis ceremony (Initialize-PoColNetwork.ps1) fills the committed profiles, covered by SHA256SUMS.
    if suffix:
        for name in ['v3-rehearsal-profile.json','v3-profile.json']: files[name]=root/'deploy/mainnet'/name
    info=subprocess.check_output([str(files['pocol'+suffix]),'build-info'])
    (args.out/'build-info.json').write_bytes(info)
build=json.loads((args.out/'build-info.json').read_text(encoding='utf-8'))
# Release builds are compiled with POCOL_RELEASE=1, so build.rs refuses an unpinned tree; refuse its build-info too
# (VA-SPEC §2.4).
if build.get('consensusPinned') is not True: raise SystemExit('refusing to package an unpinned consensus build (consensusPinned is not true)')
if build.get('version')!=version: raise SystemExit(f"refusing to package: build-info reports version {build.get('version')!r}, not the release version {version}")

def known_identities(path,list_key=None):
    """(chainId, lowercase genesisHash) pairs read from a committed file: retired-networks.json's `retired` list, or the
    devnet.json of an invitation (the placeholder). Fails closed: a missing or malformed file refuses the release."""
    try:
        doc=json.loads(path.read_text(encoding='utf-8'))
        entries=doc[list_key] if list_key else [json.loads(doc['files']['devnet.json'])]
        pairs={(e['chainId'],str(e['genesisHash']).lower()) for e in entries}
        if not pairs or any(type(c) is not int or not re.fullmatch(r'0x[0-9a-f]{64}',g) for c,g in pairs): raise ValueError('no identity')
        return pairs
    except (OSError,KeyError,TypeError,ValueError) as e:
        raise SystemExit(f'refusing to package: {path} names no network identity ({e})')

chain=genesis=network_identity=None
if args.network:
    invitation=json.loads(args.network.read_bytes()); info=json.loads(invitation['files']['devnet.json'])
    if invitation['schema']!='pocol-network-v2' or invitation['consensusRulesVersion']!=build['consensusRulesVersion']: raise SystemExit('the invitation binds other consensus rules')
    chain,genesis=info.get('chainId'),str(info.get('genesisHash','')).lower()
    if type(chain) is not int or not re.fullmatch(r'0x[0-9a-f]{64}',genesis): raise SystemExit('the invitation names no chain ID and genesis hash')
    # Neither the chain ID nor the genesis hash of the placeholder (deploy/mainnet, decision D1) or of a retired network.
    mainnet=root/'deploy/mainnet'
    def names(pairs): return any(chain==c or genesis==g for c,g in pairs)
    if names(known_identities(mainnet/'placeholder-network.pocol.json')): raise SystemExit('the invitation is the network placeholder, not a network')
    if names(known_identities(mainnet/'retired-networks.json','retired')): raise SystemExit('the invitation is for a retired network')
    try: spec=identity['spec_version'](info)
    except ValueError as e: raise SystemExit(f'the invitation declares no valid GenesisSpec version ({e})')
    if spec!=2: raise SystemExit('the invitation is for a spec-1 network; a release is for a v3 (spec-2) network only')
    # release-info names the network PoCol Mainnet, so only the mainnet profile's chain ID may be stamped into it (the
    # rehearsal is packaged without --network).
    try: mainnet_chain=json.loads((mainnet/'v3-profile.json').read_text(encoding='utf-8'))['chainId']
    except (OSError,KeyError,TypeError,ValueError) as e: raise SystemExit(f'refusing to package: no mainnet profile chain ID ({e})')
    if chain!=mainnet_chain: raise SystemExit('the invitation is not for the network the mainnet profile names (deploy/mainnet/v3-profile.json chainId)')
    network_identity=identity['for_network'](info,build)
if args.platform=='Linux':
    for asset in (root/'deploy/standalone/linux-desktop').iterdir():
        if asset.is_file(): files[asset.name]=asset
for name,source in files.items(): shutil.copy2(source,args.out/name)
shutil.copy2(root/'deploy/standalone/README.md',args.out/'README.md')
shutil.copy2(root/'prototypes/third-party/evm-crypto/NOTICE.txt',args.out/'THIRD-PARTY-CRYPTO-NOTICES.txt')
commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
(args.out/'source-commit.txt').write_text(commit+'\n',encoding='utf-8',newline='\n')
# The network identity is the invitation's (--network), or none: no release compiles a chain ID or genesis in.
release={'version':version,'platform':args.platform,'sourceCommit':commit,
         'compatibilitySha256':build['compatibilitySha256'],'consensusRulesVersion':build['consensusRulesVersion'],
         'protocolVersionMin':build['protocolVersionMin'],'consensusIdentity':network_identity,
         'networkName':'PoCol Mainnet','networkEnvironment':'mainnet','operatorDesignation':'PoCol Mainnet',
         'chainId':chain,'genesisHash':genesis,
         'publicLaunchReady':False,'networkInvitationIncluded':False,
         'distribution':'unsigned-apk' if args.platform=='Android' else 'portable-native-tools',
         'installable':args.platform!='Android'}
if args.platform=='Android':
    release.update({'applicationId':application_id,'versionCode':version_code,'signingStatus':'requires-release-signer'})
(args.out/'release-info.json').write_text(json.dumps(release,indent=2)+'\n',encoding='utf-8',newline='\n')
# The package's own SHA256SUMS: integrity of its files (Initialize-PoColNetwork.ps1 checks it). The published release
# SHA256SUMS and the release signatures come from deploy/release (assemble_release.py, then the owner's signature).
sums=''.join(f'{hashlib.sha256(f.read_bytes()).hexdigest()}  {f.name}\n' for f in sorted(args.out.iterdir(),key=lambda f:f.name) if f.is_file())
(args.out/'SHA256SUMS').write_text(sums,encoding='utf-8',newline='\n')

def source_date_epoch():
    """The one timestamp of the Linux archive (reproducible builds): SOURCE_DATE_EPOCH when it is set, else the source
    commit's committer time, so two builds of one commit stamp the same date whatever the clock, TZ or file mtimes."""
    given=os.environ.get('SOURCE_DATE_EPOCH')
    if given is not None:
        if not re.fullmatch(r'[0-9]{1,12}',given): raise SystemExit(f'SOURCE_DATE_EPOCH {given!r} is not a whole number of seconds')
        return int(given)
    return int(subprocess.check_output(['git','log','-1','--format=%ct','HEAD'],cwd=root,text=True).strip())

if args.platform=='Linux':
    # Artifact ZIPs discard executable bits. Keep the runnable distribution inside a tarball. Byte-stable
    # (deploy/release/double_build.py compares two builds): entries in ordinal name order, one fixed mtime, owner 0:0 with
    # no names, mode 0755 for an executable and 0644 otherwise (never the builder's umask), ustar headers without pax
    # records, and a gzip header with no file name and no timestamp.
    packaged=sorted(args.out.iterdir(),key=lambda f:f.name)
    archive=args.out/f'PoCol-Mainnet-{version}-linux-x86_64.tar.gz'
    epoch=source_date_epoch()
    with archive.open('wb') as raw, gzip.GzipFile(filename='',mode='wb',fileobj=raw,mtime=0,compresslevel=9) as gz, \
            tarfile.open(fileobj=gz,mode='w',format=tarfile.USTAR_FORMAT) as tar:
        for file in packaged:
            info=tarfile.TarInfo(file.name)
            info.size=file.stat().st_size; info.mtime=epoch; info.type=tarfile.REGTYPE
            info.mode=0o755 if file.stat().st_mode & 0o111 else 0o644
            info.uid=info.gid=0; info.uname=info.gname=''
            with file.open('rb') as stream: tar.addfile(info, stream)
    for file in packaged: file.unlink()
    # Integrity only; the published tarball's signature comes from deploy/release.
    (args.out/'SHA256SUMS').write_text(f'{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n',encoding='utf-8',newline='\n')
if args.platform=='Windows':
    # The signed Windows download is this byte-stable archive (fixed entry dates and order), not the CI artifact ZIP:
    # it keeps the package's own SHA256SUMS inside, which Initialize-PoColNetwork.ps1 checks after extraction.
    packaged=sorted(args.out.iterdir(),key=lambda f:f.name)
    archive=args.out/f'PoCol-Mainnet-{version}-windows-x86_64.zip'
    with zipfile.ZipFile(archive,'w') as zf:
        for file in packaged:
            entry=zipfile.ZipInfo(file.name,date_time=(1980,1,1,0,0,0))
            entry.compress_type=zipfile.ZIP_DEFLATED; entry.external_attr=0o100644<<16
            zf.writestr(entry,file.read_bytes())
    for file in packaged: file.unlink()
    (args.out/'SHA256SUMS').write_text(f'{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n',encoding='utf-8',newline='\n')
