"""Separate review: compile each faulty complete library in an isolated directory."""
import hashlib,json,os,shutil,subprocess,tempfile,time
from pathlib import Path
ROOT=Path('/workspace/Proof-of-Collaboration');CRATE=ROOT/'prototypes/lp1-node';OUT=Path('/workspace/work/lp3-s5-amended/mutations');OUT.mkdir(exist_ok=True)
env=dict(os.environ,CARGO_HOME='/workspace/pocol-tools/cargo',RUSTUP_HOME='/workspace/pocol-tools/rustup',CARGO_MANIFEST_DIR=str(CRATE))
env['PATH']='/workspace/pocol-tools/cargo/bin:'+env['PATH']
deps=CRATE/'target/debug/deps';extern=[]
for name in ['sha2','sha3','libsecp256k1','serde_json']:
 files=list(deps.glob('lib'+name+'-*.rlib'));assert len(files)==1,(name,files)
 extern+=['--extern',name+'='+str(files[0])]
source=(CRATE/'src/gate.rs').read_text();tests=CRATE/'tests/gate.rs';rows=[]
mutants=[('disable-R'+str(i+1),source.replace('    GateReport {\n','    bad['+str(i)+'] = false;\n    GateReport {\n',1)) for i in range(26)]
for name,old in [('ignore-resource-binding','r.profile_binding == binding'),('ignore-model','r.model == RESOURCE_MODEL'),('ignore-artifact','r.artifact != [0; 32]')]:
 assert source.count(old)>=1
 mutants.append((name,source.replace(old,'true',1)))
for name,text in mutants:
 assert text!=source
 with tempfile.TemporaryDirectory(prefix=name+'-',dir=OUT) as td:
  td=Path(td);shutil.copytree(CRATE/'src',td/'src');(td/'src/gate.rs').write_text(text)
  commands=[['rustc','+1.58.1','--edition=2021','--crate-name','lp1_node','--crate-type','rlib',str(td/'src/lib.rs'),'-L','dependency='+str(deps),*extern,'-o',str(td/'liblp1_node.rlib')],['rustc','+1.58.1','--edition=2021','--test',str(tests),'--extern','lp1_node='+str(td/'liblp1_node.rlib'),'-L','dependency='+str(deps),*extern,'-o',str(td/'test')],[str(td/'test')]]
  rec=[]
  for stage,args in enumerate(commands):
   t=time.monotonic()
   with (OUT/(name+'-'+str(stage)+'.log')).open('w') as f:p=subprocess.run(args,env=env,stdout=f,stderr=subprocess.STDOUT)
   rec.append(dict(args=args,exitCode=p.returncode,seconds=round(time.monotonic()-t,3)))
   if stage<2:assert p.returncode==0,(name,stage)
  caught=rec[-1]['exitCode']==101 and 'test result: FAILED' in (OUT/(name+'-2.log')).read_text()
  rows.append(dict(name=name,caught=caught,commands=rec));(OUT/'RESULTS.json').write_text(json.dumps(rows,indent=2)+'\n')
  print(name,caught,flush=True)
assert all(r['caught'] for r in rows),[r['name'] for r in rows if not r['caught']]
