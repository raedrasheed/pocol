"""Portable R4 source/receipt verifier; never grants execution acceptance."""
from pathlib import Path
import hashlib,json,re,sys
repo=Path(sys.argv[1]).resolve();packet=Path(__file__).resolve().parent
for line in (packet/'SHA256SUMS').read_text().splitlines():
 digest,name=line.split('  ',1);p=(packet/name).resolve();assert p.is_relative_to(packet)
 assert hashlib.sha256(p.read_bytes()).hexdigest()==digest,name
for path,digest in json.loads((packet/'source-bindings.json').read_text())['files'].items():
 assert hashlib.sha256((repo/path).read_bytes()).hexdigest()==digest,path
log=(packet/'rust-tests.log').read_text()
assert 'test result: FAILED' not in log and sum(map(int,re.findall(r'test result: ok\. (\d+) passed',log)))==92
for name,count in [('issuance-mutations',13),('base-mutations',15),('nested-mutations',2)]:
 rows=json.loads((packet/(name+'.json')).read_text());assert len(rows)==count
 assert all(r['buildExit']==0 and r['testExit']==101 and r['assertionKilled'] for r in rows)
i=json.loads((packet/'issuance-independent.json').read_text());v=json.loads((packet/'VECTORS-RECEIPT.json').read_text())
assert i['inputSHA256']==v['sha256'] and len(i['cases'])==6 and [r['members'] for r in i['genesis']]==[4,1024]
assert all(r['comparison']=='MATCH' for r in i['cases']) and i['bootable'] is False
full=json.loads((repo/'development/LP4-ACCEPTANCE.json').read_text())
assert len(full['obligations'])==27 and all(r['status']=='NOT PASSED' for r in full['obligations']) and full['bootable'] is False
assert full['issuanceFundingDecision']['status']==full['systemExecutionDecision']['status']=='APPROVED_ADDITIVE_LP4'
old=json.loads((packet/'hosted-r2-final/verified-hosted.json').read_text())
assert old['run']==39451150747 and old['allZipHashesVerified'] is True and len(old['artifacts'])==5
if len(sys.argv)>2:
 p=Path(sys.argv[2]);assert p.stat().st_size==v['bytes'] and hashlib.sha256(p.read_bytes()).hexdigest()==v['sha256']
print('VERIFIED partial R4 source/92 Rust/30 compiled controls/2 genesis+6 independent cases; all full LP4 gates NOT PASSED; boot=false')
