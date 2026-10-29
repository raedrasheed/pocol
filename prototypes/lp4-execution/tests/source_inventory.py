"""Verify every registry archive in Cargo.lock and inventory license files.
Usage: python source_inventory.py ISOLATED_CARGO_HOME OUTPUT.json
This inventories sources/expressions, not completion of a license/notice audit.
"""
import hashlib,json,pathlib,sys,tomllib
root=pathlib.Path(__file__).resolve().parents[1];home=pathlib.Path(sys.argv[1]);packages=[]
lock=tomllib.loads((root/'Cargo.lock').read_text())
for p in lock['package']:
 if 'source' not in p:continue
 name=p['name']+'-'+p['version'];bases=list((home/'registry/src').glob('*/'+name));archives=list((home/'registry/cache').glob('*/'+name+'.crate'))
 assert len(bases)==len(archives)==1,name
 base=bases[0];checksum=hashlib.sha256(archives[0].read_bytes()).hexdigest();assert checksum==p['checksum'],name
 info=tomllib.loads((base/'Cargo.toml').read_text())['package'];assert info.get('license'),name
 files=[dict(path=f.name,sha256=hashlib.sha256(f.read_bytes()).hexdigest()) for f in sorted(base.iterdir()) if f.is_file() and any(x in f.name.upper() for x in ['LICENSE','COPYING','NOTICE'])]
 packages.append(dict(package=name,licenseExpression=info['license'],crateSHA256=checksum,licenseFiles=files))
pathlib.Path(sys.argv[2]).write_text(json.dumps(dict(scope=f'All {len(packages)} external locked registry packages; includes optional/target packages, not all compiled',status='SOURCE_CHECKSUMS_VERIFIED; license/notice audit NOT PASSED',packages=packages),indent=2)+'\n')
print(f'PASS {len(packages)} locked archive SHA256 checks; license audit NOT PASSED')
