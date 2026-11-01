import pathlib,tomllib,urllib.request,hashlib,tarfile,json
root=pathlib.Path('/workspace/pocol-tools/rustup/toolchains/1.99.1-x86_64-unknown-linux-gnu');out=pathlib.Path('/workspace/work/lp4/p2-registry-full/r16-rust-sources');out.mkdir(exist_ok=True);manifest=tomllib.loads((root/'lib/rustlib/multirust-channel-manifest.toml').read_text());rows=[]
for name in ['cargo','rustc','rust-std']:
 p=manifest['pkg'][name]['target']['x86_64-unknown-linux-gnu'];url=p['xz_url'];assert url.startswith('https://static.rust-lang.org/dist/2026-10-15/')
 archive=out/(name+'.tar.xz')
 if not archive.is_file():
  with urllib.request.urlopen(url,timeout=60) as stream,archive.open('wb') as f:
   while True:
    chunk=stream.read(1024*1024)
    if not chunk:break
    f.write(chunk)
 digest=hashlib.sha256(archive.read_bytes()).hexdigest();assert digest==p['xz_hash'],name
 verified=[];packaging=[]
 with tarfile.open(archive,mode='r:xz') as tar:
  for m in tar:
   path=pathlib.PurePosixPath(m.name);assert not path.is_absolute() and '..' not in path.parts
   if not m.isfile():continue
   parts=path.parts
   # Installer packaging and manifests are not installed byte-for-byte payloads.
   rel=pathlib.PurePosixPath(*parts[2:]) if len(parts)>2 else None
   if len(parts)<3 or str(rel) in ['manifest.in']:
    packaging.append(m.name);continue
   data=tar.extractfile(m).read();target=root/str(rel)
   assert target.is_file() and target.read_bytes()==data,(name,str(rel),'archive/install mismatch')
   verified.append(dict(path=str(rel),sha256=hashlib.sha256(data).hexdigest(),bytes=len(data)))
 rows.append(dict(component=name,archiveSHA256=digest,source=url,version=manifest['pkg'][name]['version'],regularPayloadFilesVerified=verified,installerPackagingNotCounted=packaging));print(name,len(verified),'installed regular payload files match pinned official archive',flush=True)
 (out/'source-proof.json').write_text(json.dumps(dict(scope='Pinned official Rust toolchain archive SHA256 and installed regular payload bytes; not a compiler source rebuild or full release distribution audit',components=rows,bootable=False),indent=2)+'\n')
