"""Prepare source-bound runtime notices from locked archives and pinned missing grants.
Build/test tools are not linked runtime dependencies. No execution/resource PASS.
"""
import argparse,gzip,hashlib,io,json,pathlib,re,tarfile,tomllib
p=argparse.ArgumentParser();p.add_argument('cargo_home');p.add_argument('active_tree');p.add_argument('upstream');p.add_argument('output');a=p.parse_args()
root=pathlib.Path(__file__).resolve().parents[1];home=pathlib.Path(a.cargo_home);out=pathlib.Path(a.output);out.mkdir(parents=True,exist_ok=True)
active=set()
for line in pathlib.Path(a.active_tree).read_text().splitlines():
 m=re.match(r'([\w-]+) v([\w.+-]+)',line)
 if m:active.add(m[1]+'-'+m[2])
upstream=json.loads(pathlib.Path(a.upstream).read_text());byref={}
for item in upstream:
 raw=item['content'].encode();blob=hashlib.sha1(b'blob '+str(len(raw)).encode()+b'\0'+raw).hexdigest();assert blob==item['expectedBlob']==item['actualBlob']
 byref[(item['repo'],item['sha'],item['path'])]=item
fallback={
 'alloy-eip2930-0.1.0':('alloy-rs/eips','3136c2883e8d59cd1ac5b004dffb8bc826f0fc03','LICENSE-MIT'),
 'alloy-eip7702-0.5.1':('alloy-rs/eips','ace19c509587b16e528ec441ffcd29cd9d4f2acd','LICENSE-MIT'),
 'alloy-primitives-0.8.26':('alloy-rs/core','79d0ef6e6a4331f5512957bb62a3018ad782b15d','LICENSE-MIT'),
 'alloy-rlp-0.3.16':('alloy-rs/rlp','87cd1b72eddd24410b0cf782436ad3ea94262d05','LICENSE-MIT'),
 'alloy-rlp-derive-0.3.16':('alloy-rs/rlp','87cd1b72eddd24410b0cf782436ad3ea94262d05','LICENSE-MIT'),
 'blst-0.3.17':('supranational/blst','54e6e55674722fc2797ebb4bbb71b26d881eb4b8','LICENSE'),
 'c-kzg-1.0.3':('ethereum/c-kzg-4844','75d569b16cb2e17a33484af0f03ec633cba3b582','LICENSE'),
 'ruint-1.20.1':('alloy-rs/ruint','8a242035914783682b9e6a4bc87e0d65221bff77','LICENSE'),
 'ruint-macro-1.2.1':('alloy-rs/ruint','89d58efbd4c6de4994e5e404ed6450b8bb927202','LICENSE'),
 'aurora-engine-modexp-1.3.0':('aurora-is-near/aurora-engine','4173da04948a1b2f0f36760fec9d5cd525d6f245','Cargo.toml')}
choices={'MIT':'MIT','MIT OR Apache-2.0':'MIT','Apache-2.0 OR MIT':'MIT','MIT/Apache-2.0':'MIT','Apache-2.0':'Apache-2.0','CC0-1.0':'CC0-1.0','Unlicense OR MIT':'MIT','Zlib':'Zlib','Apache-2.0 OR BSL-1.0':'Apache-2.0','BSD-3-Clause':'BSD-3-Clause','(MIT OR Apache-2.0) AND Unicode-3.0':'MIT AND Unicode-3.0','Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT':'MIT','MIT OR Apache-2.0 OR LGPL-2.1-or-later':'MIT'}
# A package whose license texts and copyright lines are in a file with another name (r-efi's README: "See AUTHORS
# file for details"; the crate ships no LICENSE file). That file is a notice like any LICENSE file.
grant_files={'r-efi-6.0.0':'AUTHORS'}
def notice(name,rel):return any(x in pathlib.PurePosixPath(rel).name.upper()for x in ['LICENSE','COPYING','NOTICE','COPYRIGHT'])or rel==grant_files.get(name)
files={};packages=[];locked=0
for package in tomllib.loads((root/'Cargo.lock').read_text())['package']:
 if 'source'not in package:continue
 locked+=1;name=package['name']+'-'+package['version'];archives=list((home/'registry/cache').glob('*/'+name+'.crate'));assert len(archives)==1
 raw=archives[0].read_bytes();assert hashlib.sha256(raw).hexdigest()==package['checksum']
 if name not in active:continue
 bases=list((home/'registry/src').glob('*/'+name));assert len(bases)==1;base=bases[0]
 # Bind extracted metadata/notices to the checksum-verified archive itself.
 archive_files={}
 with tarfile.open(fileobj=io.BytesIO(raw),mode='r:gz') as archive:
  for member in archive.getmembers():
   parts=pathlib.PurePosixPath(member.name).parts
   assert parts and parts[0]==name and '..'not in parts
   rel='/'.join(parts[1:])
   if member.isfile()and(rel in ['Cargo.toml','.cargo_vcs_info.json']or notice(name,rel)):
    assert member.size<4*1024*1024
    archive_files[rel]=archive.extractfile(member).read()
 assert (base/'Cargo.toml').read_bytes()==archive_files['Cargo.toml']
 meta=tomllib.loads((base/'Cargo.toml').read_text())['package'];license=meta['license'];assert license in choices,(name,license)
 notices=[]
 for f in sorted(base.rglob('*')):
  if not f.is_file()or not notice(name,f.relative_to(base).as_posix()):continue
  assert f.resolve().is_relative_to(base.resolve())and f.stat().st_size<4*1024*1024
  b=f.read_bytes();rel=f.relative_to(base).as_posix();assert b==archive_files[rel],(name,rel,'source/archive mismatch');path=name+'/'+rel;files[path]=b;notices.append({'path':path,'sha256':hashlib.sha256(b).hexdigest(),'origin':'checksum-verified crate'})
 if name in grant_files:
  grant=name+'/'+grant_files[name];assert grant in[n['path']for n in notices],(name,grant);assert choices[license]=='MIT'and b'Permission is hereby granted, free of charge'in files[grant],(name,'MIT grant text')
 if not notices:
  ref=fallback[name];item=byref[ref];assert (base/'.cargo_vcs_info.json').read_bytes()==archive_files['.cargo_vcs_info.json'];vcs=json.loads(archive_files['.cargo_vcs_info.json']);assert vcs['git']['sha1']==ref[1]
  b=item['content'].encode();path=name+'/UPSTREAM-'+ref[2];files[path]=b;notices.append({'path':path,'sha256':hashlib.sha256(b).hexdigest(),'origin':'https://github.com/'+ref[0]+'/blob/'+ref[1]+'/'+ref[2],'gitBlob':item['actualBlob']})
  if name=='aurora-engine-modexp-1.3.0':assert tomllib.loads(item['content'])['workspace']['package']['license']=='CC0-1.0'
 # Preserve the actual package grant and normalized provenance, not just a label.
 files[name+'/Cargo.toml']= (base/'Cargo.toml').read_bytes()
 packages.append(dict(package=name,crateSHA256=package['checksum'],declaredSPDX=license,selectedBranch=choices[license],notices=notices,nestedNoticesIncluded=any('/depend/'in n['path']for n in notices)))
# Active path packages other than this crate are the repository's patched forks in prototypes/third-party/evm-crypto
# (no registry archive). Their files are bound to that directory's MANIFEST.json hashes; NOTICE.txt goes with each.
vendor=root.parent/'third-party/evm-crypto';vmanifest=json.loads((vendor/'MANIFEST.json').read_text())
own=tomllib.loads((root/'Cargo.toml').read_text())['package'];own=own['name']+'-'+own['version']
vnotice=(vendor/'NOTICE.txt').read_bytes();assert hashlib.sha256(vnotice).hexdigest()==vmanifest['additional_file_sha256']['NOTICE.txt']
for package in tomllib.loads((root/'Cargo.lock').read_text())['package']:
 name=package['name']+'-'+package['version']
 if 'source'in package or name==own or name not in active:continue
 entries=[x for x in vmanifest['packages']if x['directory']==package['name']+'-'+x['upstream_version']and x['local_version']==package['version']];assert len(entries)==1,(name,'not a recorded evm-crypto fork')
 entry=entries[0];base=vendor/entry['directory'];hashes=entry['current_file_sha256']
 assert hashlib.sha256((base/'Cargo.toml').read_bytes()).hexdigest()==hashes['Cargo.toml']
 meta=tomllib.loads((base/'Cargo.toml').read_text())['package'];assert (meta['name'],meta['version'])==(package['name'],package['version']);license=meta['license'];assert license in choices,(name,license)
 notices=[];local='prototypes/third-party/evm-crypto/'+entry['directory']
 for f in sorted(base.rglob('*')):
  rel=f.relative_to(base).as_posix()
  if not f.is_file()or not notice(name,rel):continue
  b=f.read_bytes();assert hashlib.sha256(b).hexdigest()==hashes[rel],(name,rel,'file/MANIFEST.json mismatch');path=name+'/'+rel;files[path]=b;notices.append({'path':path,'sha256':hashlib.sha256(b).hexdigest(),'origin':'repository patched fork '+local})
 assert notices,(name,'no license file')
 path=name+'/NOTICE.txt';files[path]=vnotice;notices.append({'path':path,'sha256':hashlib.sha256(vnotice).hexdigest(),'origin':'prototypes/third-party/evm-crypto/NOTICE.txt'})
 files[name+'/Cargo.toml']=(base/'Cargo.toml').read_bytes()
 packages.append(dict(package=name,crateSHA256=None,localFork=local,upstreamCrateSHA256=entry['registry_archive_sha256'],declaredSPDX=license,selectedBranch=choices[license],notices=notices,nestedNoticesIncluded=False))
assert len(packages)==len(active)-1,(len(packages),len(active))
# CC0's unchanged standard legal text is retained as a reference for the workspace grant.
files['_standard/CC0-1.0.txt']=files['tiny-keccak-2.0.2/LICENSE']
manifest=dict(scope='Default-feature external runtime/build dependency closure, --target all; no optional-feature or compiler/test-tool distribution claim',status='SOURCE CHECKSUMS AND NOTICE PACK PREPARED; whole P2/LP4 NOT PASSED',lockedArchivesVerified=locked,activePackages=len(packages),selectedBranches=sorted(set(x['selectedBranch']for x in packages)),packages=packages,additionalNotes=['secp256k1-sys bindings declare CC0; embedded secp256k1 MIT COPYING and other bundled notices are included','unicode-ident requires Unicode-3.0 in addition to MIT; LICENSE-UNICODE included','ruint-macro repository alias moved recmo/uint -> alloy-rs/ruint; exact original VCS revision preserved','aurora CC0 grant is the checksum-linked upstream workspace declaration; standard CC0 legal text retained separately','r-efi ships its MIT, Apache-2.0 and LGPL-2.1+ grants and copyright lines in AUTHORS (no LICENSE file); that file is included and the MIT branch is selected','alloy-primitives, alloy-eip7702 and revm-precompile are the repository-maintained patched forks in prototypes/third-party/evm-crypto (crateSHA256 null; upstreamCrateSHA256 is the original registry archive); their license files and NOTICE.txt are checked against that directory\'s MANIFEST.json','Inactive locked optional packages are not silently counted as linked default runtime packages'])
files['manifest.json']=(json.dumps(manifest,indent=2)+'\n').encode()
buffer=io.BytesIO()
with tarfile.open(fileobj=buffer,mode='w')as tar:
 for path,b in sorted(files.items()):
  info=tarfile.TarInfo(path);info.size=len(b);info.mtime=0;info.mode=0o644;info.uid=info.gid=0;info.uname=info.gname='';tar.addfile(info,io.BytesIO(b))
archive=gzip.compress(buffer.getvalue(),mtime=0);(out/'runtime-notices.tar.gz').write_bytes(archive)
(out/'manifest.json').write_text(json.dumps(manifest,indent=2)+'\n');(out/'archive-sha256.json').write_text(json.dumps({'runtime-notices.tar.gz':hashlib.sha256(archive).hexdigest()},indent=2)+'\n')
print(f'Prepared notices for {len(packages)} active packages; {locked} locked archive hashes verified; P2 NOT PASSED')
