"""Root minimal C28 probe against the actual observation-time repair."""
import importlib.util,json
from pathlib import Path
from types import SimpleNamespace
ROOT=Path(__file__).resolve().parents[2]
def load(name,p):
 s=importlib.util.spec_from_file_location(name,p);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);return m
m=load('c28_probe_original',ROOT/'m1-draft-0.16/tools/run_checks_016.py')
c=load('c28_probe_repair',ROOT/'m1-draft-0.18/tools/observe_capture.py');c.install(m)
class Engine:
 def __init__(self):self.violations=[];self.sets=[]
 def run(self,t):
  if t>=6000 and not self.violations:self.violations.append({'t':6000,'kind':'later'})
e=Engine();spec={'rows':[{'t':5000,'violations':[]},{'t':6000,'violations':[{'t':6000,'kind':'later'}]}]}
A=SimpleNamespace(R13=SimpleNamespace(SR12=None),C=None)
cells=m.admin_cells(A,e,spec,6000);before=json.dumps(cells,sort_keys=True)
e.violations[0]['kind']='mutated';spec['rows'][1]['violations'][0]['kind']='expected-mutated'
checks={'pastEmpty':cells[0][2]==[],'laterContainsOnly6000':cells[1][2]==[{'t':6000,'kind':'later'}],'noSharedActual':cells[0][2] is not cells[1][2],'futureEngineAndExpectedMutationCannotChangeCapture':json.dumps(cells,sort_keys=True)==before}
r={'checks':len(checks),'passed':sum(checks.values()),'failed':sum(not v for v in checks.values()),'results':checks};Path(__file__).with_suffix('.json').write_text(json.dumps(r,indent=2)+'\n',encoding='utf-8');print(json.dumps(r));assert all(checks.values())
