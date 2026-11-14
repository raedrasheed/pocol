#!/usr/bin/env python3
"""Explicit allowlists only: no network keys, private state or Android signing keys."""
import argparse, hashlib, json, runpy, shutil, subprocess, tarfile
from pathlib import Path
p=argparse.ArgumentParser(); p.add_argument('--platform',choices=['Linux','Windows','Android'],required=True); p.add_argument('--out',type=Path,required=True)
p.add_argument('--network',type=Path,help='the v2 invitation this release is for; release-info then names its consensusIdentity')
# The installed app is org.pocol.node; an APK for a new signing key is another app (V3-33), built with
# gradle -PpocolApplicationId=org.pocol.wallet. Only these two ids are packaged.
p.add_argument('--android-application-id',choices=['org.pocol.node','org.pocol.wallet'],default='org.pocol.node'); args=p.parse_args()
root=Path(__file__).resolve().parents[2]
# The consensus identity reference (standard library only; never imported from a package).
identity=runpy.run_path(str(root/'deploy/public-testnet/pocol_identity.py'))
args.out.mkdir()
version='0.4.0'
if args.platform=='Android':
    # CI has no retained signer for the installed app. An unsigned release is a
    # signing input, never an installable update or a freshly debug-signed APK.
    apk='arm64-unsigned.apk' if args.android_application_id=='org.pocol.node' else f'arm64-{args.android_application_id}-unsigned.apk'
    files={f'PoCol-Mainnet-{version}-{apk}':root/'prototypes/pocol-android/app/build/outputs/apk/release/app-release-unsigned.apk'}
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
    for name in (['Start-PoCol.cmd','Initialize-PoColNetwork.ps1'] if suffix else ['install-linux.sh']): files[name]=root/'deploy/standalone'/name
    info=subprocess.check_output([str(files['pocol'+suffix]),'build-info'])
    (args.out/'build-info.json').write_bytes(info)
build=json.loads((args.out/'build-info.json').read_text(encoding='utf-8'))
# Release builds are compiled with POCOL_RELEASE=1, so build.rs refuses an unpinned tree; refuse its build-info too
# (VA-SPEC §2.4).
if build.get('consensusPinned') is not True: raise SystemExit('refusing to package an unpinned consensus build (consensusPinned is not true)')
chain,genesis=1599846715,'0xbe137bfdb4fdf332f2e1500ee1a67e0b22ce6bdf57efa46fb4ea7b0debe509ca'
network_identity=None
if args.network:
    invitation=json.loads(args.network.read_bytes()); info=json.loads(invitation['files']['devnet.json'])
    if invitation['schema']!='pocol-network-v2' or invitation['consensusRulesVersion']!=build['consensusRulesVersion']: raise SystemExit('the invitation binds other consensus rules')
    if (info['chainId'],info['genesisHash'].lower())!=(chain,genesis): raise SystemExit('the invitation is for another network')
    network_identity=identity['for_network'](info,build)
if args.platform=='Linux':
    for asset in (root/'deploy/standalone/linux-desktop').iterdir():
        if asset.is_file(): files[asset.name]=asset
for name,source in files.items(): shutil.copy2(source,args.out/name)
shutil.copy2(root/'deploy/standalone/README.md',args.out/'README.md')
shutil.copy2(root/'prototypes/third-party/evm-crypto/NOTICE.txt',args.out/'THIRD-PARTY-CRYPTO-NOTICES.txt')
commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()
(args.out/'source-commit.txt').write_text(commit+'\n',encoding='utf-8',newline='\n')
release={'version':version,'platform':args.platform,'sourceCommit':commit,
         'compatibilitySha256':build['compatibilitySha256'],'consensusRulesVersion':build['consensusRulesVersion'],
         'protocolVersionMin':build['protocolVersionMin'],'consensusIdentity':network_identity,
         'networkName':'PoCol Mainnet','networkEnvironment':'mainnet',
         'operatorDesignation':'PoCol Mainnet','requiresCoordinatedUpgrade':True,'activationHeight':None,
         'chainId':chain,'genesisHash':genesis,
         'publicLaunchReady':False,'networkInvitationIncluded':False,
         'distribution':'unsigned-apk' if args.platform=='Android' else 'portable-native-tools',
         'installable':args.platform!='Android'}
if args.platform=='Android':
    if args.android_application_id=='org.pocol.node':
        release.update({'applicationId':'org.pocol.node','versionCode':5,
                        'signingStatus':'blocked-requires-existing-signer',
                        'lastDistributedSignerSha256':'bbb399007be5217a5a939eaa9b0ed1335170f7397a7487ef6da9acddb21b8772'})
    else:
        # A separate app for the owner's new release key: it installs beside org.pocol.node and never updates it.
        release.update({'applicationId':args.android_application_id,'versionCode':5,
                        'signingStatus':'requires-new-release-signer','installsBeside':'org.pocol.node'})
(args.out/'release-info.json').write_text(json.dumps(release,indent=2)+'\n',encoding='utf-8',newline='\n')
sums=''.join(f'{hashlib.sha256(f.read_bytes()).hexdigest()}  {f.name}\n' for f in sorted(args.out.iterdir()) if f.is_file())
(args.out/'SHA256SUMS').write_text(sums,encoding='utf-8',newline='\n')

if args.platform=='Linux':
    # Artifact ZIPs discard executable bits. Keep the runnable distribution inside a tarball.
    packaged=sorted(args.out.iterdir())
    archive=args.out/f'PoCol-Mainnet-{version}-linux-x86_64.tar.gz'
    with tarfile.open(archive, 'w:gz') as tar:
        for file in packaged:
            info=tar.gettarinfo(str(file), arcname=file.name)
            info.uid=info.gid=0; info.uname=info.gname=''
            with file.open('rb') as stream: tar.addfile(info, stream)
    for file in packaged: file.unlink()
    (args.out/'SHA256SUMS').write_text(f'{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n',encoding='utf-8',newline='\n')
