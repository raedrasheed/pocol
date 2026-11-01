"""Offline receipt integrity gate; does not execute or accept deferred LP4 work."""
import copy,hashlib,json,subprocess,sys
from pathlib import Path
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[3]
def need(x,msg):
 if not x:raise AssertionError(msg)
def checks(v):
 need(v['head']=='3423bbf6ba995ba1c5d77ca69084905272135211','head')
 need(v['merge']['merged'] and v['merge']['sha']=='93ca35b97d7eeb9a11af539559e89d8e170b9a50','normal merge')
 a=[c for c in v['checks']if c['name']=='windows-regression'];need(len(a)==1 and a[0]['status']=='completed' and a[0]['conclusion']=='success','missing/pending/failed Windows check')
v=json.loads((HERE/'github-receipts.json').read_text());checks(v)
for kind in ['missing','pending','failed']:
 bad=copy.deepcopy(v)
 if kind=='missing':bad['checks']=[]
 elif kind=='pending':bad['checks'][0]['status']='in_progress';bad['checks'][0]['conclusion']=None
 else:bad['checks'][0]['conclusion']='failure'
 try:checks(bad)
 except AssertionError:pass
 else:raise AssertionError('acceptance bypass '+kind)
r=json.loads((HERE/'windows/results.json').read_text());need(r['commit']==v['head'] and r['counts']['FAIL']==0,'Windows source/result')
for name in ['cargo-test-full','store-journal','store-process-crash-recovery','genesis-batch-alloc-debug','genesis-batch-alloc-release-max','e2e-http','e2e-store','lp3-genesis-diff']:
 need(any(x['check']==name and x['status']=='PASS'for x in r['results']),name)
h=json.loads((HERE/'windows/source-hashes.json').read_text())
for row in h['files']:
 b=(ROOT/row['path']).read_bytes();need(row['sha256']in[hashlib.sha256(b).hexdigest(),hashlib.sha256(b.replace(b'\r\n',b'\n').replace(b'\n',b'\r\n')).hexdigest()],row['path'])
a=json.loads((ROOT/'development/LP4-ACCEPTANCE.json').read_text());need(a['status']=='NOT PASSED' and not a['bootable'],'LP4 status');need(len(a['obligations'])==23 and all(r['status']=='NOT PASSED' and not r['evidence'] for r in a['obligations']),'carried obligation promotion')
need(a['pendingOwnerDefinition']['status']=='PENDING' and not a['pendingOwnerDefinition']['answerAssumed'],'assumed owner answer')
need(subprocess.check_output(['git','diff','041061a','HEAD','--','development/m1','development/lp3','prototypes/lp1-node/fixtures','prototypes/lp1-node/Cargo.lock'])==b'','immutable baseline')
print(json.dumps(dict(status='PASS P1 receipts only',checksNegativeControls=3,sourceHashes=len(h['files']),LP4='NOT PASSED',ownerDefinition='PENDING',bootable=False)))
