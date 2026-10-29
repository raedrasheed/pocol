"""Verify four R2 artifacts; never claim pending independent/full acceptance.
Args: artifact_root frozen_corpus source_repo [source_revision]
Public source defaults d741; local alias5636 has the identical verified tree.
"""
from pathlib import Path
import sys,json,hashlib,subprocess,re
root,corpus,repo=map(lambda x:Path(x).resolve(),sys.argv[1:4]);revision=sys.argv[4] if len(sys.argv)>4 else 'd741363e872936764eb27e918332d0f6878b6195'
packet=Path(__file__).resolve().parent
assert subprocess.check_output(['git','-C',str(repo),'rev-parse',revision+'^{tree}']).decode().strip()=='7fd53dc67c2deb313b590248271307e71dd94b6b'
def source(path):return subprocess.check_output(['git','-C',str(repo),'show',revision+':'+path])
manifest=json.loads((packet/'r2-four-artifacts.json').read_text());assert len(manifest['artifacts'])==4
for a in manifest['artifacts']:
 p=root/(a['name']+'.zip');assert p.stat().st_size==a['bytes'] and hashlib.file_digest(p.open('rb'),'sha256').hexdigest()==a['sha256']
m=root/'lp4-p2-controls/lp4-controls'
for file,path in [('system-vectors.json','development/lp4/p4-genesis/codex-r2/vectors.json'),('system-independent.json','development/lp4/p4-genesis/codex-r2/independent.json'),('system-epoch-witness.json','prototypes/lp4-execution/tests/data/system-epoch-witness.json')]:assert (m/file).read_bytes()==source(path)
for p in [m/'rust-tests.log',root/'lp4-native-windows/rust-tests.log']:
 s=p.read_text();assert 'test result: FAILED' not in s and sum(map(int,re.findall(r'test result: ok\. (\d+) passed',s)))==80
mut=json.loads((root/'lp4-p2-controls/lp4-system-mutations/results.json').read_text());assert len(mut)==15 and all(x['buildExit']==0 and x['testExit']==101 and x['assertionKilled'] for x in mut)
sources=json.loads((m/'gst/sources.json').read_text());assert len(sources)==2807 and len({s['path'] for s in sources})==2807
expected={}
for entry in sources:
 data=(corpus/entry['path']).read_bytes();assert hashlib.sha256(data).hexdigest()==entry['sha256']
 for name,test in json.loads(data).items():
  for i,post in enumerate(test.get('post',{}).get('Cancun',[])):expected[(entry['path'],name,i)]=post
assert len(expected)==20661
seen=set();full=rejected=0
for line in (m/'gst/cases.jsonl').read_text().splitlines():
 a=json.loads(line);key=(a['file'],a['name'],a['variant']);assert key not in seen;seen.add(key);p=expected[key]
 assert a['indexes']==p['indexes'] and a['expectedException']==p.get('expectException') and a['controlPass'] is True and 'runnerFailure' not in a
 if p.get('expectException'):assert 'error' in a;rejected+=1
 else:assert a['expectedRoot']==a['actualRoot']==p['hash'] and a['expectedLogs']==a['actualLogs']==p['logs'];full+=1
assert seen==set(expected) and (full,rejected)==(19488,1173)
assert hashlib.sha256((m/'gst/cases.jsonl').read_bytes()).hexdigest()=='1bd455f96ded27e7f7f65253a3a3912f8044089ec60394cc036e994cb3c5a28b'
delta=json.loads((m/'code-deltas/summary.json').read_text());ind=json.loads((m/'code-deltas-independent/summary.json').read_text());assert delta['total']==20661 and delta['changed']==14 and delta['unchanged']==20647 and all(delta[k]==0 for k in ['excluded','blocked','runnerFailure'])
assert ind['total']==14 and ind['matchedExecution']==8 and ind['matchedSnapshotAdmissionOnly']==6 and ind['excluded']==ind['blocked']==0
legacy=json.loads((root/'lp4-legacy-windows/results.json').read_text());assert legacy['counts']=={'PASS':16,'FAIL':0,'BLOCKED BY PLATFORM':8,'NOT RUN':2}
g=json.loads((root/'lp4-native-cancun-reference/geth-results/summary.json').read_text());assert g['complete'] is True and g['limited'] is False and g['exit']==0 and g['counts']['total']==20661 and g['counts']['STATE_AND_NATIVE_LOG_ASSERTIONS']==19488 and g['counts']['EXPECTED_REJECTION_PRESENCE_ONLY']==1173
for a in json.loads((packet/'r2-job-log-digests.json').read_text())['decodedJobLogs']:
 p=root/a['file'];assert p.stat().st_size==a['bytes'] and hashlib.file_digest(p.open('rb'),'sha256').hexdigest()==a['sha256']
print('VERIFIED four R2 ZIP/source hashes, complete primary corpus and scoped controls; independent pending; no complete-run or LP4 acceptance')
