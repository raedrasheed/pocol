"""Reproduce amended S5 evidence in a fresh external directory, Rust 1.58.1.
python3 development/lp3/s5-paramgate/codex-r2/verify.py /absolute/fresh/output
"""
import hashlib,json,os,subprocess,sys,time
from pathlib import Path
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[3];CRATE=ROOT/'prototypes/lp1-node';OUT=Path(sys.argv[1]).resolve();OUT.mkdir(parents=True,exist_ok=True)
assert not (OUT/'commands.json').exists(),'Use a fresh output directory'
env=dict(os.environ,CARGO_MANIFEST_DIR=str(CRATE),CARGO_BUILD_JOBS='2',CARGO_TARGET_DIR=str(OUT/'target'),POCOL_REVIEW_TARGET=str(OUT/'target'))
if Path('/workspace/pocol-tools/cargo').exists():
 env.update(CARGO_HOME='/workspace/pocol-tools/cargo',RUSTUP_HOME='/workspace/pocol-tools/rustup');env['PATH']='/workspace/pocol-tools/cargo/bin:'+env['PATH']
records=[]
def run(name,args):
 start=time.monotonic()
 with (OUT/(name+'.log')).open('w') as f:p=subprocess.run(args,cwd=CRATE,env=env,stdout=f,stderr=subprocess.STDOUT)
 records.append(dict(name=name,args=args,cwd=str(CRATE),exitCode=p.returncode,seconds=round(time.monotonic()-start,3)))
 (OUT/'commands.json').write_text(json.dumps(records,indent=2)+'\n')
 print(name,p.returncode,flush=True);assert p.returncode==0,name
for name,args in [('rust-version',['rustc','+1.58.1','-vV']),('cargo-version',['cargo','+1.58.1','-V']),('python-version',['python3','--version']),('build',['cargo','+1.58.1','build','--offline','--locked']),('regression',['cargo','+1.58.1','test','--offline','--locked','--lib','--test','gate','--test','param_gate','--test','assignment','--test','adversarial','--test','fixtures_integration','--test','genesis_spec','--test','profile','--test','timing']),('format',['cargo','+1.58.1','fmt','--','--check'])]:run(name,args)
deps=OUT/'target/debug/deps';serde=list(deps.glob('libserde_json-*.rlib'));assert len(serde)==1
run('probe-compile',['rustc','+1.58.1','--edition=2021',str(HERE/'probe.rs'),'--extern','lp1_node='+str(OUT/'target/debug/liblp1_node.rlib'),'--extern','serde_json='+str(serde[0]),'-L','dependency='+str(deps),'-o',str(OUT/'probe')])
run('oracle',['python3',str(HERE/'oracle.py'),str(OUT/'probe')])
run('mutations',['python3',str(HERE/'mutations.py'),str(OUT/'mutations')])
files=['prototypes/lp1-node/src/gate.rs','prototypes/lp1-node/src/param_gate.rs','prototypes/lp1-node/tests/gate.rs','prototypes/lp1-node/tests/param_gate.rs']
(OUT/'source-binding.json').write_text(json.dumps({p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in files},indent=2)+'\n')
print('PASS amended LP3 S5 evaluator; original general R18/R21/caps64 obligations DEFERRED LP4; boot DISABLED')
