"""Inventory installed test/compiler tools without reading credentials or user data.
RECORD integrity is not upstream archive authentication or a complete license audit.
Run in the comparison venv: SCRIPT SOLC_NODE_MODULES RUST_TOOLCHAIN OUTPUT.
"""
import base64,csv,hashlib,json,pathlib,re,subprocess,sys,tomllib
from importlib import metadata

def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def notices(base):
    return [dict(path=str(p.relative_to(base)),sha256=sha(p)) for p in sorted(base.rglob('*')) if p.is_file() and any(w in p.name.upper() for w in ['LICENSE','COPYING','NOTICE','COPYRIGHT'])]

node_modules=pathlib.Path(sys.argv[1]);rust=pathlib.Path(sys.argv[2]);output=pathlib.Path(sys.argv[3]);packages=[]
for dist in sorted(metadata.distributions(),key=lambda d:d.metadata['Name'].lower()):
    record=dist.read_text('RECORD');assert record is not None,(dist.metadata['Name'],'missing RECORD')
    count=0
    for name,digest,size in csv.reader(record.splitlines()):
        if not digest:continue  # RECORD itself and generated pyc are not authenticated.
        alg,expected=digest.split('=',1);path=dist.locate_file(name);assert path.resolve().is_relative_to(pathlib.Path(sys.prefix).resolve()),'RECORD outside tool venv';assert path.is_file(),(dist.metadata['Name'],name,'missing')
        data=path.read_bytes();actual=base64.urlsafe_b64encode(hashlib.new(alg,data).digest()).rstrip(b'=').decode()
        assert actual==expected,(dist.metadata['Name'],name,'RECORD mismatch');count+=1
    license_files=[]
    for name in dist.files or []:
        if '.dist-info/' in str(name) and any(w in str(name).upper() for w in ['LICENSE','COPYING','NOTICE','AUTHORS']):
            license_files.append(dict(path=str(name),sha256=sha(dist.locate_file(name))))
    metadata_path=next(p for p in dist.files if str(p).endswith('.dist-info/METADATA'))
    packages.append(dict(name=dist.metadata['Name'],version=dist.version,recordFilesVerified=count,metadataSHA256=sha(dist.locate_file(metadata_path)),declaredLicense=dist.metadata.get('License-Expression') or dist.metadata.get('License'),notices=license_files))

root=pathlib.Path(__file__).resolve().parents[1];expected={}
for name in ['python-requirements.lock','python-comparison-extra.lock']:
    for line in (root/'tests'/name).read_text().splitlines():
        m=re.match(r'^([\w-]+)==([^\s]+)',line)
        if m:expected[m[1].lower().replace('_','-')]=m[2]
installed={p['name'].lower().replace('_','-'):p['version'] for p in packages}
assert all(installed.get(n)==v for n,v in expected.items()),'Python lock/install mismatch'

npm=[];lock=json.loads((root/'tools/package-lock.json').read_text())
for location,entry in lock['packages'].items():
    if not location:continue
    base=node_modules/location.removeprefix('node_modules/');meta=json.loads((base/'package.json').read_text())
    assert meta['version']==entry['version'],location
    npm.append(dict(name=meta['name'],version=meta['version'],declaredLicense=meta.get('license'),lockedArchiveIntegrity=entry['integrity'],packageJSONSHA256=sha(base/'package.json'),notices=notices(base)))
manifest_path=rust/'lib/rustlib/multirust-channel-manifest.toml';manifest=tomllib.loads(manifest_path.read_text());components=[]
for name in ['cargo','rustc','rust-std']:
    p=manifest['pkg'][name];target=p['target']['x86_64-unknown-linux-gnu']
    components.append(dict(name=name,version=p['version'],upstreamArchive=target['xz_url'],declaredArchiveSHA256=target['xz_hash'],archiveReverifiedHere=False))
result=dict(scope='Installed test/compiler inventory; full source/license/distribution audit NOT PASSED',python=dict(version=sys.version,packages=packages,unlockedBootstrapPackages=sorted(set(installed)-set(expected)),missingPackagedNotices=[p['name'] for p in packages if not p['notices']],blake2MissingGrant='Resolved separately by source-bound R14/blake2b-py-0.3.2-LICENSE.txt'),npm=dict(packages=npm,archiveIntegrityReverifiedHere=False,compilerVersion=subprocess.check_output(['node','-e',f"console.log(require({json.dumps(str(node_modules/'solc'))}).version())"],text=True).strip(),embeddedCompilerSHA256=sha(node_modules/'solc/soljson.js')),rust=dict(version=subprocess.check_output([str(rust/'bin/rustc'),'-vV'],text=True),rustcSHA256=sha(rust/'bin/rustc'),cargoSHA256=sha(rust/'bin/cargo'),channelManifestSHA256=sha(manifest_path),components=components,notices=notices(rust/'share/doc/rust')),bootable=False)
output.write_text(json.dumps(result,indent=2)+'\n')
print(f'Verified RECORD bytes for{len(packages)} Python distributions and locked tool versions; inventoried{len(npm)} npm packages/3 Rust components; full audit NOT PASSED')
