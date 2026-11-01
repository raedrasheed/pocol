"""Independent root counterexamples for the repaired S-a/S-e boundary."""
import importlib.util,json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];p=ROOT/'m1-draft-0.20/tools/sink_checker_strict.py'
s=importlib.util.spec_from_file_location('sink_repair_probe',p);m=importlib.util.module_from_spec(s);s.loader.exec_module(m)
anchor={'number':1,'hash':'0x'+'11'*32}
def entry(n,ev):return {'seq':n,'kind':'sink','ev':ev}
base=[entry(0,['begin',1,anchor])]
cases=[
 ('unknown_not_terminal',base+[entry(1,['unknownTerminal',1,'fake'])],{'S-a'}),
 ('valid_abort',base+[entry(1,['abort',1,'reorg'])],set()),
 ('unknown_then_abort',base+[entry(1,['unknownTerminal',1,'fake']),entry(2,['abort',1,'reorg'])],{'S-a'}),
 ('short_event',base+[entry(1,['piece'])],{'S-a'}),
 ('boolean_attempt_id',[entry(0,['begin',True,anchor]),entry(1,['abort',True,'reorg'])],{'S-a'}),
 ('float_attempt_id',[entry(0,['begin',1.0,anchor]),entry(1,['abort',1.0,'reorg'])],{'S-a'}),
 ('bad_entry',base+[{'seq':True,'kind':'sink','ev':['abort',1,'reorg']}],{'S-a','S-e'})]
summary={'pieces':1,'logs':0,'logRequests':0,'headRequests':0,'totalRequests':0,'anchor':anchor}
valid=base+[entry(1,['piece',1,0,[1,1],[]]),entry(2,['commit',1,summary])]
cases.append(('valid_empty_piece_commit',valid,set()))
for field in ['pieces','logs','logRequests','headRequests','totalRequests']:
 for wrong in [True,0.0,-1]:
  altered=dict(summary);altered[field]=wrong
  cases.append(('bad_counter_'+field+'_'+type(wrong).__name__,valid[:-1]+[entry(2,['commit',1,altered])],{'S-e'}))
rows=[]
for name,events,want in cases:
 try:
  got=m.StrictSinkChecker().check(events,1,1);rules={r['rule'] for r in got};ok=want<=rules if want else not rules
 except Exception as e:got={'exception':type(e).__name__};ok=False
 rows.append({'check':name,'expectedMinimumRules':sorted(want),'actual':got,'pass':ok})
r={'checks':len(rows),'passed':sum(x['pass'] for x in rows),'failed':sum(not x['pass'] for x in rows),'results':rows}
Path(__file__).with_suffix('.json').write_text(json.dumps(r,indent=2)+'\n',encoding='utf-8');print(json.dumps({k:v for k,v in r.items() if k!='results'}));assert not r['failed']
