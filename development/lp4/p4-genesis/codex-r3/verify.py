"""Portable R3 evidence/source verification; never complete acceptance."""
from pathlib import Path
import json,hashlib,sys,re
repo=Path(sys.argv[1]).resolve();packet=Path(__file__).resolve().parent
for directory in [packet,packet.parent/'codex-r2']:
 for line in (directory/'SHA256SUMS').read_text().splitlines():
  digest,path=line.split('  ',1);p=(directory/path).resolve();assert p.is_relative_to(directory)
  assert hashlib.sha256(p.read_bytes()).hexdigest()==digest,path
b=json.loads((packet/'source-bindings.json').read_text())
for path,digest in b['files'].items():assert hashlib.sha256((repo/path).read_bytes()).hexdigest()==digest,path
old=json.loads((packet.parent/'codex-r2/source-bindings.json').read_text())['sourceSHA256']
for path in ['prototypes/lp4-execution/src/native.rs','prototypes/lp4-execution/src/system.rs']:assert b['files'][path]==old[path]
log=(packet/'rust-tests.log').read_text();assert 'test result: FAILED' not in log and sum(map(int,re.findall(r'test result: ok\. (\d+) passed',log)))==83
log=(packet/'p3-overlay-tests.log').read_text();assert 'test result: FAILED' not in log and sum(map(int,re.findall(r'test result: ok\. (\d+) passed',log)))==102
for file,count in [('base-mutations.json',15),('nested-mutations.json',2)]:
 rows=json.loads((packet/file).read_text());assert len(rows)==count and all(x['buildExit']==0 and x['testExit']==101 and x['assertionKilled'] for x in rows)
a=json.loads((packet/'issuance-diagnostic.json').read_text());i=json.loads((packet/'issuance-independent.json').read_text());assert len(a['rows'])==len(i['comparisons'])==2 and all(x['approved'] is False for x in a['rows']+i['comparisons'])
assert i['inputSHA256']==hashlib.sha256((packet/'issuance-diagnostic.json').read_bytes()).hexdigest() and i['rootsDiffer']
v=json.loads((repo/'prototypes/lp4-execution/tests/data/system-boundary-independent.json').read_text());assert v['minimumGas']==102758 and len(v['cases'])==5
assert v['compiledArtifactSHA256']==hashlib.sha256((repo/'prototypes/lp4-execution/tests/data/system-boundary-witness.json').read_bytes()).hexdigest()
full=json.loads((repo/'development/LP4-ACCEPTANCE.json').read_text());assert len(full['obligations'])==27 and all(x['status']=='NOT PASSED' for x in full['obligations']) and full['bootable'] is False
assert full['systemExecutionDecision']['status']=='APPROVED_ADDITIVE_LP4' and full['issuanceFundingDecision']['status']=='NOT APPROVED'
if len(sys.argv)>2:
 p=Path(sys.argv[2]);r=json.loads((packet/'LOCAL-PRESERVATION.json').read_text());assert p.stat().st_size==r['bytes'] and hashlib.file_digest(p.open('rb'),'sha256').hexdigest()==r['sha256']
print('VERIFIED R3 controls/diagnostics/source hashes; issuance NOT APPROVED; full SYS/P2/P3/LP4 NOT PASSED; boot=false')
