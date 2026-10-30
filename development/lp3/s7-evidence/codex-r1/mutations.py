import sys
"""Separate sequential review: isolated complete Rust library and assertion controls."""
import json,os,shutil,subprocess,tempfile,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[4];CRATE=ROOT/'prototypes/lp1-node';OUT=Path(sys.argv[1]).resolve();OUT.mkdir(exist_ok=True)
env=dict(os.environ,CARGO_HOME='/workspace/pocol-tools/cargo',RUSTUP_HOME='/workspace/pocol-tools/rustup',CARGO_MANIFEST_DIR=str(CRATE));env['PATH']='/workspace/pocol-tools/cargo/bin:'+env['PATH'];deps=Path(os.environ.get('POCOL_REVIEW_TARGET',str(CRATE/'target')))/'release/deps';extern=[]
for name in ['sha2','sha3','libsecp256k1','serde_json']:
 fs=list(deps.glob('lib'+name+'-*.rlib'));assert len(fs)==1;extern+=['--extern',name+'='+str(fs[0])]
mutants=[('lose-actual-trace','asert.rs','delta_t: dt','delta_t: 0','asert_named.rs'),('truncate-asert-negative','asert.rs','num.div_euclid(tau)','num / tau','asert_named.rs'),('wrong-asert-factor','asert.rs','195_766_423_245_049u128','195_766_423_245_04u128','asert_named.rs'),('ignore-kappa-denominator','rewards.rs','total.max(kappa as u64 * m as u64)','total.max(1)','rewards.rs'),('ignore-tomb','rewards.rs','if row.tomb {','if false {','rewards.rs'),('ignore-missing-account','rewards.rs','else if !row.account_exists {','else if false {','rewards.rs'),('drop-round-burn','rewards.rs','result.round_burn = amount.checked_sub(&sum).ok_or(RewardError::Arithmetic)?;','result.round_burn = U256::ZERO;','rewards.rs'),('wrong-winner-split','rewards.rs','part(pot, alpha as u64, 10000)','part(pot, alpha as u64, 9999)','rewards.rs')]
results=[]
for name,file,old,new,test in mutants:
 source=(CRATE/'src'/file).read_text();assert source.count(old)==1,(name,source.count(old))
 with tempfile.TemporaryDirectory(prefix=name+'-',dir=OUT) as td:
  td=Path(td);shutil.copytree(CRATE/'src',td/'src');(td/'src'/file).write_text(source.replace(old,new));steps=[]
  argslist=[['rustc','+1.58.1','--edition=2021','--crate-name','lp1_node','--crate-type','rlib',str(td/'src/lib.rs'),'-L','dependency='+str(deps),*extern,'-o',str(td/'liblp1_node.rlib')],['rustc','+1.58.1','--edition=2021','--test',str(CRATE/'tests'/test),'--extern','lp1_node='+str(td/'liblp1_node.rlib'),'-L','dependency='+str(deps),*extern,'-o',str(td/'tests')],[str(td/'tests')]]
  for stage,args in enumerate(argslist):
   t=time.monotonic()
   with (OUT/(name+'-'+str(stage)+'.log')).open('w') as f:p=subprocess.run(args,env=env,stdout=f,stderr=subprocess.STDOUT)
   steps.append(dict(args=args,exitCode=p.returncode,seconds=round(time.monotonic()-t,3)))
   if stage<2:assert p.returncode==0,(name,stage)
  detected=steps[-1]['exitCode']==101 and 'test result: FAILED' in (OUT/(name+'-2.log')).read_text();results.append(dict(name=name,caught=detected,commands=steps));(OUT/'RESULTS.json').write_text(json.dumps(results,indent=2)+'\n');print(name,detected,flush=True)
assert all(r['caught'] for r in results),[r['name'] for r in results if not r['caught']]
