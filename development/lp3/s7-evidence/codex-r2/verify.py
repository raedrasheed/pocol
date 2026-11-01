"""Reproduce approved S7 CF/T0 core tests. Does not rerun immutable numerical r1."""
import json,os,subprocess,sys
from pathlib import Path
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[3];CRATE=ROOT/'prototypes/lp1-node';OUT=Path(sys.argv[1]).resolve();OUT.mkdir(parents=True,exist_ok=True)
env=dict(os.environ,CARGO_MANIFEST_DIR=str(CRATE),CARGO_HOME='/workspace/pocol-tools/cargo',RUSTUP_HOME='/workspace/pocol-tools/rustup');env['PATH']='/workspace/pocol-tools/cargo/bin:'+env['PATH'];records=[]
def run(name,args):
 with (OUT/(name+'.log')).open('w') as f:p=subprocess.run(args,cwd=ROOT,env=env,stdout=f,stderr=subprocess.STDOUT)
 records.append(dict(name=name,args=args,exitCode=p.returncode));(OUT/'commands.json').write_text(json.dumps(records,indent=2)+'\n');print(name,p.returncode,flush=True);assert p.returncode==0,name
run('release-build',['cargo','+1.58.1','build','--release','--offline','--locked','--manifest-path',str(CRATE/'Cargo.toml')])
run('regression',['cargo','+1.58.1','test','--release','--offline','--locked','--manifest-path',str(CRATE/'Cargo.toml'),'--lib',*[x for n in ['gate','param_gate','assignment','adversarial','fixtures_integration','genesis_spec','profile','timing','asert_named','rewards','asert_alloc','finality_core','envelope'] for x in ['--test',n]]])
run('format',['cargo','+1.58.1','fmt','--manifest-path',str(CRATE/'Cargo.toml'),'--','--check'])
tsc=Path(sys.argv[2]).resolve();run('typecheck',['node',str(tsc),'--noEmit','--project',str(ROOT/'prototypes/lp3-model/tsconfig.json'),'--typeRoots',str(tsc.parents[2]/'@types')])
run('ts-tests',['node','--experimental-strip-types',str(ROOT/'prototypes/lp3-model/tests.ts')])
deps=CRATE/'target/release/deps';serde=list(deps.glob('libserde_json-*.rlib'));assert len(serde)==1
run('probe-compile',['rustc','+1.58.1','--edition=2021','-O',str(HERE/'cf_probe.rs'),'--extern','lp1_node='+str(CRATE/'target/release/liblp1_node.rlib'),'-L','dependency='+str(deps),'--extern','serde_json='+str(serde[0]),'-o',str(OUT/'cf-probe')])
run('oracle',['python3',str(HERE/'cf_oracle.py'),str(OUT)])
run('rust-mutations',['python3',str(HERE/'mutations.py'),str(OUT/'mutations')])
run('ts-mutations',['python3',str(HERE/'ts_mutations.py'),str(OUT/'ts-mutations'),str(tsc)])
print('PASS newly approved CF and T0 semantic core only; execution and boot NOT accepted')
