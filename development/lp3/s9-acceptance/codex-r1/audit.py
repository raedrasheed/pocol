"""Final read-only LP3 evidence gate. Requires separately captured live GitHub receipts."""
from pathlib import Path
import hashlib,json,subprocess,sys
ROOT=Path.cwd();LIVE=Path(sys.argv[1]);OUT=Path(sys.argv[2]);OUT.mkdir(parents=True,exist_ok=True)
def read(p):return json.loads((ROOT/p).read_text())
def need(value,why):
 if not value:raise AssertionError(why)
def github_checks(v):
 need(v['head']=='56cda6f4d848526150607509c7273ba9f2f9f440','unexpected tested head')
 for name in ['windows-regression','typecheck-and-test']:
  matching=[c for c in v['checks'] if c['name']==name];need(len(matching)==1,'missing/duplicate '+name)
  need(matching[0]['status']=='completed' and matching[0]['conclusion']=='success','unrun/blocked/failed '+name)
 need(v['pr18']['merged'] and v['pr18']['base']=='work/lp3-consensus-foundation','unmerged or wrong base')
v=json.loads(LIVE.read_text());github_checks(v)
# Negative acceptance control: an in-progress or missing check may never pass this gate.
import copy
for kind in ['pending','missing','failed']:
 bad=copy.deepcopy(v)
 if kind=='missing':bad['checks']=bad['checks'][1:]
 elif kind=='pending':bad['checks'][0]['status']='in_progress';bad['checks'][0]['conclusion']=None
 else:bad['checks'][0]['conclusion']='failure'
 try:github_checks(bad)
 except AssertionError:pass
 else:raise AssertionError('acceptance bypass '+kind)
windows=json.loads((LIVE.parent/'windows/results.json').read_text())
need(windows['commit']==v['head'],'Windows source mismatch')
for name in ['store-journal','store-process-crash-recovery','cargo-test-full','genesis-batch-alloc-debug','genesis-batch-alloc-release-max','verify-fixtures','e2e-http','e2e-store','lp3-genesis-diff','lp3-genesis-cli']:
 rows=[r for r in windows['results'] if r['check']==name];need(len(rows)==1 and rows[0]['status']=='PASS',name+' not actually PASS')
need(windows['counts']['FAIL']==0,'Windows failed steps')
source=json.loads((LIVE.parent/'windows/source-hashes.json').read_text());need(source['commit']==v['head'],'source hash receipt mismatch')
for row in source['files']:
 b=(ROOT/row['path']).read_bytes();need(row['sha256'] in [hashlib.sha256(b).hexdigest(),hashlib.sha256(b.replace(b'\r\n',b'\n').replace(b'\n',b'\r\n')).hexdigest()],'Windows checkout source '+row['path'])
diff=json.loads((LIVE.parent/'windows/lp3-genesis-diff.json').read_text());need(diff['inputs']==30075 and diff['agree']==30075 and diff['mismatches']==0,'Windows differential')
linux=json.loads((LIVE.parent/'linux-cli.json').read_text());need(linux['ok'] and len(linux['results'])==9 and all(r['status']=='PASS' for r in linux['results']),'Linux actual OS-limit checks')

manifests=[]
for p in (ROOT/'development/lp3').rglob('SHA256SUMS'):
 if 's9-acceptance' in p.parts:continue
 n=0
 for line in p.read_text().splitlines():
  if not line.strip():continue
  sha,rel=line.split('  ',1);f=p.parent/rel;need(f.exists() and hashlib.sha256(f.read_bytes()).hexdigest()==sha,str(f));n+=1
 manifests.append(dict(manifest=str(p.relative_to(ROOT)),files=n))
paths=['development/m1','development/lp1','development/lp2','development/decisions','development/lp3/s1-genesisspec','development/lp3/s2-profile','development/lp3/s3-timing','development/lp3/s4-assignment','prototypes/lp1-node/fixtures','prototypes/lp1-node/Cargo.lock']
need(subprocess.check_output(['git','diff','c662bff','HEAD','--',*paths])==b'','baseline preservation')
s5=read('development/lp3/s5-paramgate/codex-r2/RESULTS.json');need(s5['oracle']['cases']==1450 and s5['oracle']['mismatches']==0 and s5['mutationsCaught']==32 and not s5['bootable'],'S5')
s7=read('development/lp3/s7-evidence/codex-r1/RESULTS.json');need(s7['millionCases']==1000000 and s7['rustMismatches']==s7['tsMismatches']==0 and s7['all601ExponentsCovered'] and s7['maxIntermediateBits']<=512,'S7 equivalence')
need(s7['asbAllocations']==0 and s7['asbPositiveCounterControl'] and s7['latencyPassedRust'] and s7['latencyPassedTS'],'S7 resource/latency')
from decimal import Decimal
need(Decimal(s7['asqMaxRelativeError'])<=Decimal('.0002'),'ASQ')
cf=read('development/lp3/s7-evidence/codex-r2/RESULTS.json');need(cf['cfOracleCases']==16000 and cf['cfRustMismatches']==cf['cfTsMismatches']==0 and cf['rustCoreMutationsCaught']==11 and cf['tsCfMutationsCaught']==6,'approved CF')
s8=read('development/lp3/s8-audit/codex-r1/RESULTS.json');need(s8['portableRustTests']==131 and s8['tsGroups']==11 and s8['oracleMismatches']==0 and s8['allControlsDetected'] and s8['httpFailures']==0,'S8')
for rel,count in [('mutations',10),('signature-mutation',1),('reproduction/ts-mutations',12)]:
 p=ROOT/'development/lp3/s8-audit/codex-r1'/rel/'RESULTS.json'
 # Main Rust mutation receipts are in reproduction, not duplicate scratch outputs.
 if not p.exists():p=ROOT/'development/lp3/s8-audit/codex-r1/reproduction'/rel/'RESULTS.json'
 rows=json.loads(p.read_text());need(len(rows)==count and all(r['caught'] for r in rows),rel)
result=dict(status='PASS final amended LP3 evidence gate',scope='LP3 with resource amendment 01 and separately approved CF core mapping',testedCoreHead=v['head'],foundation=v['foundation'],criteria={f'S{i}':'PASS amended/core scope' for i in range(9)},manifestEntries=sum(m['files'] for m in manifests),manifests=manifests,acceptanceNegativeControls=3,windows=windows['counts'],blockedOrUnrunCountedPassing=0,bootable=False,originalGeneralR18='DEFERRED LP4 NOT PASSED',originalGeneralR21='DEFERRED LP4 NOT PASSED',caps64Minimum='DEFERRED LP4 NOT PASSED',realFinalizationCursorPendingAndFinalSubsidy='LP4 NOT RUN',SV6='LP4 NOT RUN',physicalScheduler='LP6 NOT RUN',S9='Final report/PR reviewed; foundation docs merge remains to record')
(OUT/'audit.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items() if k!='manifests'}))
