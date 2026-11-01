from pathlib import Path
import json,hashlib,re,collections,sys
p=Path(sys.argv[1]);corpus=Path(sys.argv[2]);report={'run':39463120365,'sourceHead':'3bde461b0c921db5fd5bd95179e7f3ad52fd3e7e','tree':'28101df34e73feb6357bca2bd9cea945f0aee958'}
artifacts=json.loads((p/'artifacts-verified.json').read_text())
for a in artifacts:
 b=(p/(a['name']+'.zip')).read_bytes();assert len(b)==a['bytes'] and hashlib.sha256(b).hexdigest()==a['sha256']
jobs=json.loads((p/'jobs.json').read_text());assert len(jobs)==5 and all(j['status']=='completed' and j['conclusion']=='success' for j in jobs)
base=p/'lp4-independent-cancun/independent-results';summary=json.loads((base/'summary.json').read_text());assert summary['counts']=={'total':20661,'MATCH':19488,'EXCEPTION_PRESENCE_ONLY':1173};assert not summary['limited'] and summary['selectedCases'] is None;assert (base/'incomplete.jsonl').read_bytes()==b''
sources=json.loads((base/'sources.json').read_text());assert len(sources)==len({r['path'] for r in sources})==2807
identities={};exceptions={}
for source in sources:
 b=(corpus/source['path']).read_bytes();assert hashlib.sha256(b).hexdigest()==source['sha256']
 for name,test in json.loads(b).items():
  for i,post in enumerate(test.get('post',{}).get('Cancun',[])):
   key=(source['path'],name,i);assert key not in identities;identities[key]=post
assert len(identities)==20661
primary=p/'lp4-p2-controls/lp4-controls/gst/cases.jsonl';assert hashlib.sha256(primary.read_bytes()).hexdigest()=='1bd455f96ded27e7f7f65253a3a3912f8044089ec60394cc036e994cb3c5a28b'
seen=set();roots=0;rejects=0
for line in primary.open():
 row=json.loads(line);key=(row['file'],row['name'],row['variant']);assert key not in seen;seen.add(key);post=identities[key];assert row['indexes']==post['indexes'] and row.get('expectedException')==post.get('expectException')
 if post.get('expectException'):rejects+=1
 else:
  # Primary row structure is inspected explicitly below; bytes equal already
  # published R4 receipts, verified above, preserving full per-case values.
  assert row["actualRoot"]==row["expectedRoot"]==post["hash"] and row["actualLogs"]==row["expectedLogs"]==post["logs"] and row["controlPass"]
  roots+=1
assert seen==set(identities) and (roots,rejects)==(19488,1173)
linux=p/'lp4-p2-controls/lp4-controls/rust-tests.log';windows=p/'lp4-native-windows/rust-tests.log'
for f in [linux,windows]:
 text=f.read_text();assert sum(map(int,re.findall(r'test result: ok\. (\d+) passed',text)))==92;assert not re.search(r'test result: FAILED',text)
controls={}
for f in (p/'lp4-p2-controls').glob('*/results.json'):
 rows=json.loads(f.read_text());assert isinstance(rows,list) and all(r['assertionKilled'] for r in rows);controls[f.parent.name]=len(rows)
legacy=json.loads((p/'lp4-legacy-windows/results.json').read_text());assert legacy['counts']=={'PASS':16,'FAIL':0,'BLOCKED BY PLATFORM':8,'NOT RUN':2}
report.update(artifacts=artifacts,allZipHashesVerified=True,jobs=jobs,sourceFiles=2807,allSourceHashesVerified=True,total=20661,uniqueIdentities=20661,exactPrimaryRootLogPairs=roots,expectedRejectionPresence=rejects,excluded=0,blocked=0,incomplete=0,mismatch=0,engineError=0,seconds=summary['seconds'],primaryBytesEqualPublishedR4=True,rustLinux=92,rustWindows=92,compiledControls=controls,legacy=legacy['counts'],scope='Upstream control comparison and SYS/ISSUANCE-R4 native CODE-01 and SYS-01 controls (not wire authentication) only;1173 second-engine rejection presence is not signed-wire/error-precedence proof; no P2/P3/LP4 acceptance',bootable=False)
(p/'verified-hosted.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({k:report[k] for k in ['total','exactPrimaryRootLogPairs','expectedRejectionPresence','excluded','blocked','allZipHashesVerified','rustLinux','rustWindows']}))
