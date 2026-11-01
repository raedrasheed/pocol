"""Verify portable SYS-01 R2 source/evidence bindings, never full acceptance."""
import pathlib,hashlib,json,re,sys
repo=pathlib.Path(sys.argv[1]).resolve();packet=pathlib.Path(__file__).resolve().parent
for line in (packet/'SHA256SUMS').read_text().splitlines():
 digest,path=line.split('  ',1);p=(packet/path).resolve();assert p.is_relative_to(packet)
 assert hashlib.sha256(p.read_bytes()).hexdigest()==digest,path
bindings=json.loads((packet/'source-bindings.json').read_text())
for path,digest in bindings['sourceSHA256'].items():assert hashlib.sha256((repo/path).read_bytes()).hexdigest()==digest,path
vectors=json.loads((packet/'vectors.json').read_text());independent=json.loads((packet/'independent.json').read_text())
assert independent['inputSHA256']==hashlib.sha256((packet/'vectors.json').read_bytes()).hexdigest()
assert len(independent['comparisons'])==13 and all(r['comparison']=='MATCH' for r in independent['comparisons'])
mutations=json.loads((packet/'mutation-results.json').read_text());assert len(mutations)==15
assert all(r['buildExit']==0 and r['testExit']==101 and r['assertionKilled'] for r in mutations)
for file,count in [('sys01-full-tests-bound-final.log',80),('sys01-p3-overlay-bound-final.log',99)]:
 log=(packet/file).read_text();assert 'test result: FAILED' not in log
 assert sum(map(int,re.findall(r'test result: ok\. (\d+) passed',log)))==count
accept=json.loads((packet/'ACCEPTANCE.json').read_text());assert accept['status']=='NOT PASSED' and accept['bootable'] is False and all(r['status']=='NOT PASSED' for r in accept['requiredAfterDecision'])
full=json.loads((repo/'development/LP4-ACCEPTANCE.json').read_text());assert full['bootable'] is False and len(full['obligations'])==27 and all(x['status']=='NOT PASSED' for x in full['obligations'])
assert full['systemExecutionDecision']['id']=='LP4-SYS-01'
if len(sys.argv)>2:
 archive=pathlib.Path(sys.argv[2]);receipt=json.loads((packet/'LOCAL-PRESERVATION.json').read_text())
 assert archive.stat().st_size==receipt['bytes'] and hashlib.file_digest(archive.open('rb'),'sha256').hexdigest()==receipt['sha256']
print('VERIFIED portable primitive evidence/source hashes; full SYS/P2/P3/LP4 NOT PASSED; boot=false')
