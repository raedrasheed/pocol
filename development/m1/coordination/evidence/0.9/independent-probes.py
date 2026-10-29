"""Independent probes over the preserved 0.9 review copy, not author goldens."""
import json,sys,hashlib
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(Path(__file__).parent/'m1-draft-0.9/tools'))
import storequeue_ref as S
rows=[]
def check(name,got,want):
    rows.append({'check':name,'passed':got==want,'actual':got,'expected':want})
oracle=json.loads((Path(__file__).parent/'storequeue-utf8-oracle.json').read_text(encoding='utf-8'))
for c in oracle['cases']:check('Node.TextEncoder '+c['id'],S.qbytes({'op':'set','k':c['key'],'v':c['value']}),c['expected'])
q=S.StoreQueue();q.open_session('A','N','a');q.attach('F','A')
q.submit(0,'F','m0',{'op':'set','k':'k','v':'v'})
q.submit(1,'F','m1',{'op':'set','k':'x','v':'v'})
q.tick(5000);check('timeout retains ownership/seat/counters',[q.msgs['m0'].own,q.seats,q.glob],['active',1,[2,132]])
check('timeout response exactly once',[(r['code'],r['reason']) for r in q.replies],[(-32603,'storeTimeout')])
q.tick(10000);check('wait not expired before acceptance+10000',q.glob,[2,132])
q.tick(10001);check('wait expired exactly acceptance+10000',q.glob,[1,66])
q.settle(12000,1);check('late success releases once',q.glob,[0,0]);check('late success actual dictionary',q.backend['site:N:a'],{'k':'v'})
check('no second late response',[(r['id'],r['code']) for r in q.replies],[('m0',-32603),('m1',-32005)])
check('once-only releases',[q.stats['releaseCount'],q.stats['doubleReleaseCount']],[2,0])
q=S.StoreQueue();q.open_session('A','N','a');q.attach('F','A');q.submit(0,'F','m0',{'op':'set','k':'k','v':'v'})
q.tick(5000);q.close_session(5100,'A');check('close after timeout retains resources',q.glob,[1,66])
q.settle(7000,1);check('close drains session',[q.glob,'A' in q.sessions,len(q.replies)],[[0,0],False,1])
q=S.StoreQueue();q.open_session('A','N','a');q.attach('F','A');q.submit(0,'F','m0',{'op':'set','k':'k','v':'v'})
snapshots=[];original=q._snapshot
def capture(frame,key,badge):
    snapshots.append({'frame':frame,'dict':dict(q._dict(key)),'badge':badge});original(frame,key,badge)
q._snapshot=capture
q.navigate(100,'F','F2');q.settle(300,1)
check('full dictionary at snapshot (not keys/length only)',snapshots,[{'frame':'F2','dict':{'k':'v'},'badge':False}])
check('old frame receives no success',q.replies,[])
q=S.StoreQueue();q.open_session('A','N1','a');q.open_session('B','N2','a');q.attach('F','A');q.attach('G','B')
q.submit(0,'F','m0',{'op':'set','k':'k','v':'a','site':'forged'})
q.submit(1,'G','m1',{'op':'set','k':'k','v':'b','site':'forged'})
check('frame-derived keys and separate networks',sorted(q.active),['site:N1:a','site:N2:a'])
q.settle(2,1);q.settle(3,2)
check('actual byte-value separation',q.backend,{'site:N1:a':{'k':'a'},'site:N2:a':{'k':'b'}})
try:q.settle(4,1);duplicate='accepted'
except ValueError:duplicate='refused'
check('duplicate backend settlement refused',duplicate,'refused')
check('duplicate settlement leaves counters intact',q.glob,[0,0])
for name in ['reference','vectors']+[f'm1-draft-0.{i}' for i in range(2,9)]:
    pass
cp=json.loads((ROOT/'coordination/checkpoints/issue3-start.json').read_text(encoding='utf-8-sig'))
changed=[c['path'] for c in cp['files'] if hashlib.sha256(Path(c['path']).read_bytes()).hexdigest().upper()!=c['sha256']]
check('all prior baseline/spec/fixture hashes preserved',len(changed),0)
out={'oracle':'Independent Node TextEncoder and hand-derived event expectations, not author-generated goldens','summary':{'checks':len(rows),'passed':sum(r['passed'] for r in rows),'failed':sum(not r['passed'] for r in rows)},'results':rows}
(Path(__file__).parent/'storequeue-independent-probes-0.9.json').write_text(json.dumps(out,indent=2,ensure_ascii=True))
print(json.dumps(out['summary']))
for r in rows:
    if not r['passed']:print('FAIL',json.dumps(r))
sys.exit(out['summary']['failed']!=0)
