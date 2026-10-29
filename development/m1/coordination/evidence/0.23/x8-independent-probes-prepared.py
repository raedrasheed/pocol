"""Root X8 evaluator probes; never opens a page or any network connection."""
import importlib.util,json,copy,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2];rev=sys.argv[1]
p=ROOT/('m1-draft-'+rev)/'tools/x8_eval.py'
s=importlib.util.spec_from_file_location('x8_root_independent',p);m=importlib.util.module_from_spec(s);s.loader.exec_module(m)
c=json.loads((ROOT/('m1-draft-'+rev)/'vectors/x8-config.json').read_text(encoding='utf-8'))
ports=[{'proto':x['proto'],'port':x['port']} for x in c['canary']['ports']]
channels={x['id']:x['expected'] for x in c['channels']}
r={'selftests':{'K'+str(k):copy.deepcopy(ports) for k in range(6)},'windows':[]}
for k in range(6):
 for ch in channels:
  pr,po=channels[ch];r['windows'].append({'K':'K'+str(k),'C':ch,'events':[{'proto':pr,'port':po}] if k==4 else [],'secondary':[],'dns':[]})
rows=[]
def ck(name,run,expected):
 got=m.evaluate(c,run);rows.append({'check':name,'expected':expected,'actual':got['verdict'],'pass':got['verdict'] in expected})
ck('all_required_positive_controls_and_zero_cells',r,['PASS'])
for k in [0,1,2,5]:
 for ch in channels:
  if k==5 and ch not in ['C5','C6','C7']:continue
  q=copy.deepcopy(r);w=next(x for x in q['windows'] if x['K']=='K'+str(k) and x['C']==ch);pr,po=channels[ch];w['events']=[{'proto':pr,'port':po}];ck('gating_reach_K'+str(k)+'_'+ch,q,['FAIL'])
for ch in channels:
 q=copy.deepcopy(r);next(x for x in q['windows'] if x['K']=='K4' and x['C']==ch)['events']=[];ck('missing_positive_'+ch,q,['PENDING'])
q=copy.deepcopy(r);w=next(x for x in q['windows'] if x['K']=='K0' and x['C']=='C1');w['events']=[{'proto':'tcp','port':18080}];q['windows'].append({'K':'K0','C':'C1','events':[],'secondary':[],'dns':[]})
ck('duplicate_window_cannot_erase_recorded_K0_reach',q,['FAIL','INVALID'])
report={'revision':rev,'checks':len(rows),'passed':sum(x['pass'] for x in rows),'failed':sum(not x['pass'] for x in rows),'results':rows}
Path(__file__).with_name('x8-independent-probes-'+rev+'.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
print(json.dumps({k:v for k,v in report.items() if k!='results'}))
for x in rows:
 if not x['pass']:print(json.dumps(x))
sys.exit(1 if report['failed'] else 0)
