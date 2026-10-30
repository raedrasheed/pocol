"""Minimal independent counterexample for historical observation aliasing."""
import importlib.util,json
from types import SimpleNamespace
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
p=ROOT/'m1-draft-0.16/tools/run_checks_016.py'
s=importlib.util.spec_from_file_location('c28_original',p);m=importlib.util.module_from_spec(s);s.loader.exec_module(m)
class Engine:
 def __init__(self):self.violations=[];self.sets=[]
 def run(self,t):
  if t>=6000 and not self.violations:self.violations.append({'t':6000,'kind':'later'})
e=Engine();spec={'rows':[{'t':5000,'violations':[]},{'t':6000,'violations':[{'t':6000,'kind':'later'}]}]}
A=SimpleNamespace(R13=SimpleNamespace(SR12=None),C=None)
cells=m.admin_cells(A,e,spec,6000)
r={'earlierExpected':cells[0][1],'earlierActual':cells[0][2],'laterActual':cells[1][2],'sameObject':cells[0][2] is cells[1][2],'counterexampleConfirmed':cells[0][2]!=cells[0][1] and cells[0][2] is cells[1][2]}
Path(__file__).with_suffix('.json').write_text(json.dumps(r,indent=2)+'\n',encoding='utf-8');print(json.dumps(r));assert r['counterexampleConfirmed']
