"""Reproduce numerical S7 gates only; CF acceptance question remains OPEN.
Usage: python3 .../verify.py /fresh/output [/path/to/typescript/bin/tsc]
"""
import hashlib,json,os,subprocess,sys,time
from pathlib import Path
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[3];CRATE=ROOT/'prototypes/lp1-node';MODEL=ROOT/'prototypes/lp3-model';OUT=Path(sys.argv[1]).resolve();OUT.mkdir(parents=True,exist_ok=True)
assert not (OUT/'commands.json').exists(),'use fresh output'
env=dict(os.environ,CARGO_MANIFEST_DIR=str(CRATE),CARGO_TARGET_DIR=str(OUT/'target'),POCOL_REVIEW_TARGET=str(OUT/'target'),CARGO_BUILD_JOBS='2')
if Path('/workspace/pocol-tools/cargo').exists():env.update(CARGO_HOME='/workspace/pocol-tools/cargo',RUSTUP_HOME='/workspace/pocol-tools/rustup');env['PATH']='/workspace/pocol-tools/cargo/bin:'+env['PATH']
records=[]
def run(name,args,cwd=ROOT):
 start=time.monotonic()
 with (OUT/(name+'.log')).open('w') as f:p=subprocess.run(args,cwd=cwd,env=env,stdout=f,stderr=subprocess.STDOUT)
 records.append(dict(name=name,args=args,cwd=str(cwd),exitCode=p.returncode,seconds=round(time.monotonic()-start,3)));(OUT/'commands.json').write_text(json.dumps(records,indent=2)+'\n');print(name,p.returncode,flush=True);assert p.returncode==0,name
run('rust-version',['rustc','+1.58.1','-vV']);run('cargo-version',['cargo','+1.58.1','-V']);run('node-version',['node','--version']);run('python-version',['python3','--version'])
run('release-build',['cargo','+1.58.1','build','--release','--offline','--locked'],CRATE)
run('regression',['cargo','+1.58.1','test','--release','--offline','--locked','--lib','--test','gate','--test','param_gate','--test','assignment','--test','adversarial','--test','fixtures_integration','--test','genesis_spec','--test','profile','--test','timing','--test','asert_named','--test','rewards','--test','asert_alloc'],CRATE)
run('format',['cargo','+1.58.1','fmt','--','--check'],CRATE)
tsc=Path(sys.argv[2]) if len(sys.argv)>2 else Path('/workspace/work/lp3-tscompiler/node_modules/typescript/bin/tsc')
if not tsc.exists():tsc=MODEL/'node_modules/typescript/bin/tsc'
assert tsc.exists(),'proper pinned compiler required; stripping is not type checking'
run('typecheck',['node',str(tsc),'--noEmit','--project',str(MODEL/'tsconfig.json'),'--typeRoots',str(tsc.parents[2]/'@types')])
run('nine-ts-tests',['node','--experimental-strip-types',str(MODEL/'tests.ts')])
deps=OUT/'target/release/deps';serde=list(deps.glob('libserde_json-*.rlib'));assert len(serde)==1
for source,binary,needs_json in [('asert_probe.rs','asert-probe',True),('rewards_probe.rs','rewards-probe',True),('bench.rs','bench-rust',False)]:
 args=['rustc','+1.58.1','--edition=2021','-O',str(HERE/source),'--extern','lp1_node='+str(OUT/'target/release/liblp1_node.rlib'),'-L','dependency='+str(deps)]
 if needs_json:args+=['--extern','serde_json='+str(serde[0])]
 run(binary+'-compile',args+['-o',str(OUT/binary)])
run('million',['python3',str(HERE/'million.py'),str(OUT)])
assert json.loads((OUT/'million.json').read_text())['cases']==1000000
run('asq-python',['python3',str(HERE/'quality.py'),str(OUT)])
run('asq-ts',['node','--experimental-strip-types',str(HERE/'quality.mjs')])
assert json.loads((OUT/'asq-python.json').read_text())['Fsha256']==json.loads((OUT/'asq-ts.log').read_text())['Fsha256']
run('rewards-oracle',['python3',str(HERE/'rewards_oracle.py'),str(OUT)])
run('mutations',['python3',str(HERE/'mutations.py'),str(OUT/'mutations')])
# Benchmarks are intentionally last and sequential, after other authored CPU workloads.
run('bench-rust-final',[str(OUT/'bench-rust')]);run('bench-ts-final',['node','--experimental-strip-types',str(HERE/'bench.mjs')])
print('PASS numerical S7 gates only; CF1-CF6 decision/T0 audit/S8-S9 pending; LP3 NOT ACCEPTED')
