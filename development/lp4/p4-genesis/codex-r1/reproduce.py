"""Reproduce diagnostics in an owned scratch copy of the exact R20 tree.
No selected SystemCall rule is installed into the repository or live node.
"""
import os,sys,json,hashlib,subprocess,shutil
from pathlib import Path
source,output,cargo,node_modules,python=map(Path,sys.argv[1:]);here=Path(__file__).resolve().parent
tree=subprocess.check_output(['git','rev-parse','HEAD^{tree}'],cwd=source,text=True).strip();assert tree=='31c9bb503449d0aebf0cb02883f7bb7c52f699c7'
assert not subprocess.check_output(['git','status','--porcelain','-uall','--','prototypes/lp4-execution'],cwd=source)
output.mkdir(parents=True,exist_ok=False);crate=output/'crate';shutil.copytree(source/'prototypes/lp4-execution',crate,ignore=shutil.ignore_patterns('target','node_modules','__pycache__','work'))
for f in (here/'diagnostic').rglob('*'):
 if f.is_file():
  target=crate/f.relative_to(here/'diagnostic');target.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(f,target)
env=dict(os.environ,NODE_PATH=str(node_modules),CARGO_INCREMENTAL='0',CARGO_TARGET_DIR=os.environ.get('CARGO_TARGET_DIR',str(output/'target')))
def run(command,stdout,stderr):
 with (output/stdout).open('wb') as out,(output/stderr).open('wb') as err:r=subprocess.run([str(x) for x in command],env=env,stdout=out,stderr=err)
 assert r.returncode==0,(command[0],r.returncode,stderr)
artifact=crate/'tests/data/system-epoch-witness.json';run(['node',crate/'tests/compile_system_epoch.cjs',artifact],'compiler-stdout.log','compiler-stderr.log');assert artifact.read_bytes()==(here/'diagnostic/tests/data/system-epoch-witness.json').read_bytes()
run([cargo,'+1.99.1','run','--offline','--locked','--manifest-path',crate/'Cargo.toml','--example','system_epoch_decision'],'diagnostic.json','rust-build.log');assert (output/'diagnostic.json').read_bytes()==(here/'diagnostic.json').read_bytes()
run([python,crate/'tests/system_epoch_oracle.py',output/'diagnostic.json',artifact,output/'independent.json'],'oracle-stdout.log','oracle-stderr.log');assert (output/'independent.json').read_bytes()==(here/'independent.json').read_bytes()
result=dict(sourceTree=tree,compilerByteIdentical=True,actualExecutionsByteIdentical=True,independentResultsByteIdentical=True,diagnosticSHA256=hashlib.sha256((output/'diagnostic.json').read_bytes()).hexdigest(),independentSHA256=hashlib.sha256((output/'independent.json').read_bytes()).hexdigest(),scope=__doc__,proposedRuleApproved=False,bootable=False);(output/'REPRODUCTION.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
