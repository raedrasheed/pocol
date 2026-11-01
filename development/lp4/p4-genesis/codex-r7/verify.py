"""Scoped R7 source, independent and compiled evidence verification."""
from pathlib import Path
import hashlib,json,re,sys
repo=Path(sys.argv[1]).resolve();p=Path(__file__).resolve().parent
for line in (p/'SHA256SUMS').read_text().splitlines():
 digest,name=line.split('  ',1);f=(p/name).resolve();assert f.is_relative_to(p)
 assert hashlib.sha256(f.read_bytes()).hexdigest()==digest,name
for name,digest in json.loads((p/'source-bindings.json').read_text())['files'].items():
 assert hashlib.sha256((repo/name).read_bytes()).hexdigest()==digest,name
s=(p/'rust-tests.log').read_text();assert 'test result: FAILED' not in s and sum(map(int,re.findall(r'test result: ok\. (\d+) passed',s)))==96
for file,n in [('issuance-mutations.json',13),('integration-mutations.json',2)]:
 rows=json.loads((p/file).read_text());assert len(rows)==n and all(x['buildExit']==0 and x['testExit']==101 and x['assertionKilled'] for x in rows)
i=json.loads((p/'independent.json').read_text());r=json.loads((p/'VECTORS-RECEIPT.json').read_text());facts=json.loads((p/'RESOURCE-FACTS.json').read_text());assert i['inputSHA256']==r['sha256']==facts['inputSHA256']
assert len(i['cases'])==8 and all(x['comparison']=='MATCH' for x in i['cases']) and [x['members'] for x in i['genesis']]==[4,1024]
assert all(x['completeCPWords']==52 and x['logicalCodeBytes']==8939 and x['classification'].startswith('SYNTHETIC_MEASUREMENT') for x in facts['profiles'])
full=json.loads((repo/'development/LP4-ACCEPTANCE.json').read_text());assert len(full['obligations'])==27 and all(x['status']=='NOT PASSED' for x in full['obligations']) and not full['bootable']
if len(sys.argv)>2:
 f=Path(sys.argv[2]);assert f.stat().st_size==r['bytes'] and hashlib.sha256(f.read_bytes()).hexdigest()==r['sha256']
 v=json.loads(f.read_text())
 for g,m in zip(v['genesis'],facts['profiles']):
  assert g['genesisHash']==m['genesisHash'] and hashlib.sha256(bytes.fromhex(g['spec'].removeprefix('0x'))).hexdigest()==m['genesisSpecSHA256']
  assert [hashlib.sha256(bytes.fromhex(x.removeprefix('0x'))).hexdigest() for x in g['programs']]==m['programSHA256']
print('VERIFIED scoped R7:96 Rust,2 genesis/8 independent signed cases,13+2 compiled controls; general/full LP4 NOT PASSED; boot=false')
