"""Reproduce S6 comparisons; Node >=22.18, Python, Rust 1.58.1 and pinned tsc required.
Usage: python3 .../verify.py /fresh/output [/path/to/typescript/bin/tsc]
The committed package-lock supports npm ci when registry access is available.
"""
import hashlib,json,os,subprocess,sys,time
from pathlib import Path
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[3];MODEL=ROOT/'prototypes/lp3-model';OUT=Path(sys.argv[1]).resolve();OUT.mkdir(parents=True,exist_ok=True)
assert not (OUT/'commands.json').exists(),'use fresh evidence output'
tsc=Path(sys.argv[2]) if len(sys.argv)>2 else MODEL/'node_modules/typescript/bin/tsc'
if not tsc.exists() and Path('/workspace/work/lp3-tscompiler/node_modules/typescript/bin/tsc').exists():tsc=Path('/workspace/work/lp3-tscompiler/node_modules/typescript/bin/tsc')
assert tsc.exists(),'install exact lockfile compiler or supply recorded compiler path; type stripping is not type checking'
types=tsc.parents[2]/'@types';records=[]
def run(name,args,cwd=ROOT):
 t=time.monotonic()
 with (OUT/(name+'.log')).open('w') as f:p=subprocess.run(args,cwd=cwd,stdout=f,stderr=subprocess.STDOUT)
 records.append(dict(name=name,args=list(map(str,args)),cwd=str(cwd),exitCode=p.returncode,seconds=round(time.monotonic()-t,3)));(OUT/'commands.json').write_text(json.dumps(records,indent=2)+'\n');print(name,p.returncode,flush=True);assert p.returncode==0,name
run('node-version',['node','--version']);run('compiler-version',['node',str(tsc),'--version']);run('python-version',['python3','--version'])
run('typecheck',['node',str(tsc),'--noEmit','--project',str(MODEL/'tsconfig.json'),'--typeRoots',str(types)])
run('eight-tests',['node','--experimental-strip-types',str(MODEL/'tests.ts')])
run('assignment-oracle',['python3',str(HERE/'assignment-oracle.py'),str(OUT),str(MODEL)])
run('arithmetic-oracle',['python3',str(HERE/'arithmetic-oracle.py'),str(OUT),'12000'])
run('genesis-oracle',['python3',str(HERE/'genesis-oracle.py'),str(OUT)])
run('timing-oracle',['python3',str(HERE/'timing-oracle.py'),str(OUT)])
run('mutations',['python3',str(HERE/'mutations.py'),str(OUT/'mutations')])
(OUT/'source-binding.json').write_text(json.dumps({str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in MODEL.iterdir() if p.is_file()},indent=2)+'\n')
print('PASS S6 reference scope; no boot or full LP3 acceptance')
