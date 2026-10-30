"""Reproduce the partial S5 review; never constitutes full S5 acceptance.
Run: python3 development/lp3/s5-paramgate/codex-r1/verify.py OUTSIDE_REPO_OUTPUT
Uses the existing Rust 1.58.1 installation and cached locked dependencies.
"""
import hashlib, json, os, shutil, subprocess, sys, tempfile, time
from pathlib import Path
HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[3]
CRATE = ROOT/'prototypes/lp1-node'
OUT = Path(sys.argv[1]).resolve()
OUT.mkdir(parents=True, exist_ok=True)
assert not (OUT/'commands.json').exists(), 'Use a fresh evidence directory'
ENV = dict(os.environ)
if Path('/workspace/pocol-tools/cargo').exists():
 ENV.update(CARGO_HOME='/workspace/pocol-tools/cargo',RUSTUP_HOME='/workspace/pocol-tools/rustup')
 ENV['PATH']='/workspace/pocol-tools/cargo/bin:'+ENV['PATH']
ENV['CARGO_BUILD_JOBS']='2'
ENV['CARGO_TARGET_DIR']=str(OUT/'target')
reuse = Path(sys.argv[2]).resolve() if len(sys.argv)>2 else None
if reuse:
 ENV['CARGO_TARGET_DIR']=str(reuse)
commands=[]
def run(name, args, cwd=CRATE, success=0):
 start=time.monotonic()
 with (OUT/(name+'.log')).open('w') as log:
  p=subprocess.run(args,cwd=cwd,env=ENV,stdout=log,stderr=subprocess.STDOUT)
 commands.append(dict(name=name,args=list(map(str,args)),cwd=str(cwd),exitCode=p.returncode,seconds=round(time.monotonic()-start,3)))
 (OUT/'commands.json').write_text(json.dumps(commands,indent=2)+'\n')
 print(name,p.returncode,flush=True)
 assert p.returncode==success,(name,p.returncode)
 return (OUT/(name+'.log')).read_text()
def git(*args):return subprocess.check_output(['git',*args],cwd=ROOT,text=True).strip()
sources={str(p.relative_to(ROOT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in [CRATE/'src/param_gate.rs',CRATE/'src/lib.rs',CRATE/'tests/param_gate.rs',HERE/'oracle.py',HERE/'probe.rs',HERE/'verify.py']}
(OUT/'sources.json').write_text(json.dumps(dict(testedCommit=git('rev-parse','HEAD'),sha256=sources),indent=2)+'\n')
run('rust-version',['rustc','+1.58.1','-vV'])
run('cargo-version',['cargo','+1.58.1','-V'])
run('python-version',['python3','--version'])
if reuse:
 run('corrected-review-tests',['cargo','+1.58.1','test','--offline','--locked','--test','param_gate'])
else:
 run('build',['cargo','+1.58.1','build','--offline','--locked'])
 run('regression',['cargo','+1.58.1','test','--offline','--locked','--lib','--test','param_gate','--test','assignment','--test','adversarial','--test','fixtures_integration','--test','genesis_spec','--test','profile','--test','timing'])
run('format',['cargo','+1.58.1','fmt','--','--check'])
target=Path(ENV['CARGO_TARGET_DIR'])
deps=target/'debug/deps'
serde=list(deps.glob('libserde_json-*.rlib'));assert len(serde)==1
probe=OUT/'probe'
run('probe-compile',['rustc','+1.58.1','--edition=2021',str(HERE/'probe.rs'),'--extern','lp1_node='+str(target/'debug/liblp1_node.rlib'),'--extern','serde_json='+str(serde[0]),'-L','dependency='+str(deps),'-o',str(probe)])
text=run('oracle',['python3',str(HERE/'oracle.py'),str(probe)])
(OUT/'oracle.json').write_text(json.dumps(json.loads(text),indent=2)+'\n')
# Standalone module compilation avoids dependency rebuilds while isolating every mutant.
# Each executable runs the unchanged integration tests, with its own fresh source/output.
source=(CRATE/'src/param_gate.rs').read_text()
predicates=[line.strip()[3:] for line in source.splitlines() if line.strip().startswith('&& ')]
mutants=[('remove-first-r26-predicate','self.heavy_resp_mem >= self.resp_max','true')]
mutants += [('remove-r26-predicate-'+str(i), '&& '+p, '&& true') for i,p in enumerate(predicates)]
mutants += [('accept-derive-error','Err(_) => return false','Err(_) => return true'),
 ('wrong-lag','(l_max + 2048)','(l_max + 2047)'),
 ('wrong-ceil','records as u128 + ret as u128 - 1','records as u128'),
 ('per-item-grace','self.grace_ms as u128, rate','self.grace_ms as u128 * self.batch_max as u128, rate')]
results=[]
for name,old,new in mutants:
 assert source.count(old)==1,(name,old,source.count(old))
 with tempfile.TemporaryDirectory(prefix=name+'-',dir=OUT) as td:
  td=Path(td)
  (td/'param_gate.rs').write_text(source.replace(old,new))
  (td/'lib.rs').write_text('pub mod param_gate;\n')
  run(name+'-compile',['rustc','+1.58.1','--edition=2021','--crate-name','lp1_node','--crate-type','rlib',str(td/'lib.rs'),'-o',str(td/'liblp1_node.rlib')])
  run(name+'-test-compile',['rustc','+1.58.1','--edition=2021','--test',str(CRATE/'tests/param_gate.rs'),'--extern','lp1_node='+str(td/'liblp1_node.rlib'),'-o',str(td/'test')])
  log=run(name,[str(td/'test')],success=101)
  assert 'test result: FAILED' in log
  results.append(dict(name=name,caught=True))
(OUT/'mutations.json').write_text(json.dumps(results,indent=2)+'\n')
base='f933ef8a1e43a22ee6c7ca08c0b14236e086a95f'
protected=['development/m1','development/lp1','development/lp2','development/lp3/s1-genesisspec','development/lp3/s2-profile','development/lp3/s3-timing','development/lp3/s4-assignment','.github','prototypes/lp1-node/Cargo.toml','prototypes/lp1-node/Cargo.lock']
assert not git('diff','--name-only',base,'--',*protected)
(OUT/'preservation.json').write_text(json.dumps(dict(base=base,protectedPaths=protected,changes=[]),indent=2)+'\n')
print('Partial arithmetic review passed; full S5 remains BLOCKED BY DESIGN',flush=True)
