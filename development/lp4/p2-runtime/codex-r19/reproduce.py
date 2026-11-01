"""Reproduce scoped C6 forced-surplus controls; no boot or acceptance upgrade.
Usage: python reproduce.py OUTPUT CARGO NODE_MODULES PYTHON
Set CARGO_HOME/RUSTUP_HOME/PATH to the existing pinned offline toolchain. The
provided Python must have the pinned PAY comparison dependencies installed.
"""
import pathlib,sys,shutil,subprocess,os,json,hashlib
here=pathlib.Path(__file__).resolve().parent;root=here.parents[3];out=pathlib.Path(sys.argv[1]).resolve();cargo,modules,python=sys.argv[2:];out.mkdir(parents=True,exist_ok=True);crate=out/'crate'
expected=json.loads((here/'verified.json').read_text())['sourceHashes']
for name,digest in expected.items():assert hashlib.sha256((root/'prototypes/lp4-execution'/name).read_bytes()).hexdigest()==digest,'requires frozen C6 source, not another PR head'
shutil.copytree(root/'prototypes/lp4-execution',crate,ignore=shutil.ignore_patterns('target','__pycache__'),dirs_exist_ok=True)
for name in ['tests/forced_surplus.rs','tests/support/surplus_registry.rs','examples/forced_surplus_vectors.rs']:
 p=crate/name;p.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(here/name,p)
env=dict(os.environ,NODE_PATH=str(pathlib.Path(modules).resolve()),CARGO_INCREMENTAL='0')
subprocess.run(['node',str(here/'compile.cjs'),str(here/'ForceSurplus.sol'),str(out/'force-surplus.json')],env=env,check=True);assert (out/'force-surplus.json').read_bytes()==(here/'force-surplus.json').read_bytes();shutil.copyfile(out/'force-surplus.json',crate/'tests/data/force-surplus.json')
base=[cargo,'+1.99.1'];manifest=['--offline','--locked','--manifest-path',str(crate/'Cargo.toml')]
with (out/'rust-tests.log').open('w') as log:subprocess.run(base+['test']+manifest+['--test','forced_surplus'],env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
with (out/'vectors.json').open('w') as vectors:subprocess.run(base+['run']+manifest+['--example','forced_surplus_vectors'],env=env,stdout=vectors,check=True)
env['PYTHONPATH']=str(crate/'tests');subprocess.run([python,str(here/'oracle.py'),str(out/'vectors.json'),str(out/'independent.json')],env=env,check=True)
p=crate/'contracts/RegistryPayment.sol';s=p.read_text();old='SystemAddress.pay(reward,bond)';assert s.count(old)==1;p.write_text(s.replace(old,'SystemAddress.pay(reward,address(this).balance)'))
subprocess.run(['node',str(crate/'tests/compile_registry.cjs'),str(crate/'tests/data/registry-payment.json')],env=env,check=True)
with (out/'mutant-build.log').open('w') as log:subprocess.run(base+['test']+manifest+['--test','forced_surplus','--no-run'],env=env,stdout=log,stderr=subprocess.STDOUT,check=True)
r=subprocess.run(base+['test']+manifest+['--test','forced_surplus','--','--nocapture'],env=env,capture_output=True,text=True);(out/'mutant-assertion.log').write_text(r.stdout+r.stderr);assert r.returncode!=0 and 'assertion' in r.stdout+r.stderr
print('PASS22 independent steps and1compiled assertion control; full P2/LP4 NOT PASSED')
