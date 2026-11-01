"""Prepared independent root boundary probes; execute only after author terminal receipt."""
import importlib.util,json,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];rev=sys.argv[1]
p=ROOT/('m1-draft-'+rev)/'tools/logclient_ref.py'
s=importlib.util.spec_from_file_location('lc_independent',p);m=importlib.util.module_from_spec(s);s.loader.exec_module(m)
rows=[]
def ck(n,a,b):rows.append({'check':n,'actual':a,'expected':b,'pass':a==b})
def rng(a=1,b=10,mark=False,retries=0):return {'a':a,'b':b,'withinLimit':mark,'retries':retries}
def limit(reason,lc,a=1):return {'error':{'code':-32020,'data':{'reason':reason,'lastCompleteBlock':lc,'fromBlock':a,'nextFromBlock':lc+1}}}
for reason in ['resultBytes','resultCount','scanBytes']:
 ck(reason+'_prefix',m.step(rng(),limit(reason,5)),{'act':'replace','first':[1,5,True],'second':[6,10,False]})
 ck(reason+'_lc_b',m.step(rng(),limit(reason,10))['act'],'violation')
 ck(reason+'_lc_a_minus1',m.step(rng(),limit(reason,0))['act'],'violation')
 ck(reason+'_marked',m.step(rng(mark=True),limit(reason,5))['act'],'violation')
for reason in ['deadline','scratch']:
 ck(reason+'_split',m.step(rng(mark=True),limit(reason,0)),{'act':'split','first':[1,5,True],'second':[6,10,True]})
 ck(reason+'_singleton_last_retry',m.step(rng(a=1,b=1,retries=2),limit(reason,0)),{'act':'retry','delayMs':500,'retries':3})
 ck(reason+'_singleton_exhaustion',m.step(rng(a=1,b=1,retries=3),limit(reason,0)),{'act':'error','reason':reason,'block':1})
 ck(reason+'_lc_b',m.step(rng(),limit(reason,10))['act'],'violation')
 ck(reason+'_lc_a_minus2',m.step(rng(),limit(reason,-1))['act'],'violation')
ck('lc_bool_rejected',m.step(rng(),limit('deadline',True))['act'],'violation')
ck('busy_does_not_consume_singleton_retries',m.step(rng(retries=3),{'error':{'code':-32021,'data':{'reason':'busy','retryAfterMs':500}}}),{'act':'retry','delayMs':500,'retries':3})
ck('unsupported',m.step(rng(),{'error':{'code':-32601}}),{'act':'error','reason':'unsupported'})
base={'address':'0x'+'11'*20,'blockNumber':'0x1','logIndex':'0x0','blockHash':'0x'+'22'*32,'data':'0x','topics':[]}
ck('complete_duplicate_tuple',m.step(rng(),{'result':[base,dict(base)]})['act'],'violation')
changed=dict(base,logIndex='0x1',blockHash='0x'+'33'*32)
ck('complete_changed_hash',m.step(rng(),{'result':[base,changed]})['act'],'violation')
ck('append_continuity_across_pieces',m.step(rng(),{'result':[base]},last=(1,0))['act'],'violation')
ck('complete_past_range',m.step(rng(a=2),{'result':[base]})['act'],'violation')
r={'revision':rev,'checks':len(rows),'passed':sum(x['pass'] for x in rows),'failed':sum(not x['pass'] for x in rows),'results':rows}
Path(__file__).with_name('logclient-independent-probes-'+rev+'.json').write_text(json.dumps(r,indent=2)+'\n',encoding='utf-8');print(json.dumps({k:v for k,v in r.items() if k!='results'}))
for x in rows:
 if not x['pass']:print(json.dumps(x))
sys.exit(1 if r['failed'] else 0)
