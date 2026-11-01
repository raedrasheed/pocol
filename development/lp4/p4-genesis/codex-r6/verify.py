"""Verify scoped D27 evidence; never full LP4 acceptance."""
from pathlib import Path
import hashlib,json,re,sys
repo=Path(sys.argv[1]).resolve();p=Path(__file__).resolve().parent
for line in (p/'SHA256SUMS').read_text().splitlines():
 digest,name=line.split('  ',1);file=(p/name).resolve();assert file.is_relative_to(p)
 assert hashlib.sha256(file.read_bytes()).hexdigest()==digest,name
for name,digest in json.loads((p/'source-bindings.json').read_text())['files'].items():
 assert hashlib.sha256((repo/name).read_bytes()).hexdigest()==digest,name
s=(p/'rust-tests.log').read_text();assert 'test result: FAILED' not in s and sum(map(int,re.findall(r'test result: ok\. (\d+) passed',s)))==95
rows=json.loads((p/'mutations.json').read_text());assert len(rows)==8
assert all(x['solcExit']==x['buildExit']==0 and x['testExit']==101 and x['assertionKilled'] for x in rows)
i=json.loads((p/'independent.json').read_text());r=json.loads((p/'VECTORS-RECEIPT.json').read_text());assert i['inputSHA256']==r['sha256']
assert len(i['cases'])==3 and sum(len(x['executions']) for x in i['cases'])==6 and all(x['comparison']=='MATCH' for x in i['cases'])
assert len(i['negative'])==4 and all(x['rejected'] for x in i['negative']) and not i['bootable']
full=json.loads((repo/'development/LP4-ACCEPTANCE.json').read_text());assert len(full['obligations'])==27 and all(x['status']=='NOT PASSED' for x in full['obligations']) and not full['bootable']
if len(sys.argv)>2:
 file=Path(sys.argv[2]);assert file.stat().st_size==r['bytes'] and hashlib.sha256(file.read_bytes()).hexdigest()==r['sha256']
print('VERIFIED D27 component95 Rust/8 compiled controls/6 exact independent executions/4 rejection outcomes; full LP4 NOT PASSED; boot=false')
