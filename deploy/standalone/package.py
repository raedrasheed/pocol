#!/usr/bin/env python3
"""Explicit allowlists only: no network keys, private state or Android signing keys."""
import argparse, hashlib, json, shutil, subprocess, tarfile
from pathlib import Path
p=argparse.ArgumentParser(); p.add_argument('--platform',choices=['Linux','Windows','Android'],required=True); p.add_argument('--out',type=Path,required=True); args=p.parse_args()
root=Path(__file__).resolve().parents[2]; args.out.mkdir()
if args.platform=='Android':
    files={'PoCol-testnet-arm64-debug.apk':root/'prototypes/pocol-android/app/build/outputs/apk/debug/app-debug.apk',
           'PoCol-testnet-arm64-unsigned.apk':root/'prototypes/pocol-android/app/build/outputs/apk/release/app-release-unsigned.apk'}
    infos=list((root/'prototypes/pocold/target/aarch64-linux-android/release/build').glob('pocold-*/out/build-info.json'))
    assert infos and len({json.loads(f.read_text())['compatibilitySha256'] for f in infos})==1, 'ambiguous Android build identity'
    (args.out/'build-info.json').write_bytes(infos[0].read_bytes())
else:
    suffix='.exe' if args.platform=='Windows' else ''
    files={n+suffix:root/'prototypes/pocold/target/release'/(n+suffix) for n in ['pocol','pocold','pocol-key','pocol-sign','pocol-site']}
    if suffix:
        files['PoCol-Wallet.exe']=root/'prototypes/pocold/target/release/PoCol-Wallet.exe'
        files['PoCol-Wallet-preview.png']=root/'wallet-preview.png'
    for name in (['Start-PoCol.cmd','Initialize-PoColNetwork.ps1'] if suffix else ['install-linux.sh']): files[name]=root/'deploy/standalone'/name
    info=subprocess.check_output([str(files['pocol'+suffix]),'build-info'])
    (args.out/'build-info.json').write_bytes(info)
if args.platform=='Linux':
    for asset in (root/'deploy/standalone/linux-desktop').iterdir():
        if asset.is_file(): files[asset.name]=asset
for name,source in files.items(): shutil.copy2(source,args.out/name)
shutil.copy2(root/'deploy/standalone/README.md',args.out/'README.md')
(args.out/'source-commit.txt').write_text(subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip()+'\n')
sums=''.join(f'{hashlib.sha256(f.read_bytes()).hexdigest()}  {f.name}\n' for f in sorted(args.out.iterdir()) if f.is_file())
(args.out/'SHA256SUMS').write_text(sums)

if args.platform=='Linux':
    # Artifact ZIPs discard executable bits. Keep the runnable distribution inside a tarball.
    packaged=sorted(args.out.iterdir())
    archive=args.out/'PoCol-linux-x86_64.tar.gz'
    with tarfile.open(archive, 'w:gz') as tar:
        for file in packaged:
            info=tar.gettarinfo(str(file), arcname=file.name)
            info.uid=info.gid=0; info.uname=info.gname=''
            with file.open('rb') as stream: tar.addfile(info, stream)
    for file in packaged: file.unlink()
    (args.out/'SHA256SUMS').write_text(f'{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}\n')
