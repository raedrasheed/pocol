"""S8 local reproducible gates; Windows full-suite receipts are additionally required.
Run with loopback network access for HTTP tests. No blocked step counts as PASS.
"""
import hashlib,json,os,subprocess,sys
from pathlib import Path
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[3];CRATE=ROOT/'prototypes/lp1-node';OUT=Path(sys.argv[1]).resolve();OUT.mkdir(parents=True,exist_ok=True);assert not(OUT/'commands.json').exists()
env=dict(os.environ,CARGO_MANIFEST_DIR=str(CRATE),CARGO_HOME='/workspace/pocol-tools/cargo',RUSTUP_HOME='/workspace/pocol-tools/rustup');env['PATH']='/workspace/pocol-tools/cargo/bin:'+env['PATH'];records=[]
def run(name,args):
 with (OUT/(name+'.log')).open('w') as f:p=subprocess.run(args,cwd=ROOT,env=env,stdout=f,stderr=subprocess.STDOUT)
 records.append(dict(name=name,args=args,exitCode=p.returncode));(OUT/'commands.json').write_text(json.dumps(records,indent=2)+'\n');print(name,p.returncode,flush=True);assert p.returncode==0,name
base=['cargo','+1.58.1'];manifest=['--manifest-path',str(CRATE/'Cargo.toml')]
run('debug-build',base+['build','--offline','--locked']+manifest)
run('release-build',base+['build','--release','--offline','--locked']+manifest)
run('portable-regression',base+['test','--release','--offline','--locked']+manifest+['--lib',*[x for n in ['gate','param_gate','assignment','adversarial','adversarial_lp3','fixtures_integration','genesis_spec','profile','timing','asert_named','rewards','asert_alloc','envelope','finality_core','http_loopback'] for x in ['--test',n]],'--','--nocapture'])
run('format',base+['fmt']+manifest+['--','--check'])
run('http-process',['python3',str(CRATE/'tests/e2e_http.py'),'--bin',str(CRATE/'target/release/lp1-node'),'--fixtures',str(CRATE/'fixtures')])
tsc=Path(sys.argv[2]).resolve();run('typecheck',['node',str(tsc),'--noEmit','--project',str(ROOT/'prototypes/lp3-model/tsconfig.json'),'--typeRoots',str(tsc.parents[2]/'@types')])
run('eleven-ts-tests',['node','--experimental-strip-types',str(ROOT/'prototypes/lp3-model/tests.ts')])
old=ROOT/'development/lp3/s6-model/codex-r1'
for name,args in [('assignment-oracle',[str(OUT),str(ROOT/'prototypes/lp3-model')]),('arithmetic-oracle',[str(OUT),'12000']),('genesis-oracle',[str(OUT)]),('timing-oracle',[str(OUT)])]:run(name,['python3',str(old/(name+'.py')),*args])
run('rust-mutations',['python3',str(HERE/'mutations.py'),str(OUT/'mutations')])
run('signature-mutation',['python3',str(HERE/'signature_mutation.py'),str(OUT/'signature-mutation')])
run('ts-mutations',['python3',str(HERE/'ts_mutations.py'),str(OUT/'ts-mutations'),str(tsc)])
files=[*sorted((CRATE/'src').glob('*.rs')),*sorted((ROOT/'prototypes/lp3-model').glob('*.ts'))]
(OUT/'source-hashes.json').write_text(json.dumps({str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in files},indent=2)+'\n')
print('PASS local portable S8 gates; supported Windows journal/crash/process CI receipt still REQUIRED, not implied')
