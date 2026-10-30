import sys
"""Separate sequential review: isolated complete Rust library and assertion controls."""
import json,os,shutil,subprocess,tempfile,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[4];CRATE=ROOT/'prototypes/lp1-node';OUT=Path(sys.argv[1]).resolve();OUT.mkdir(exist_ok=True)
env=dict(os.environ,CARGO_HOME='/workspace/pocol-tools/cargo',RUSTUP_HOME='/workspace/pocol-tools/rustup',CARGO_MANIFEST_DIR=str(CRATE));env['PATH']='/workspace/pocol-tools/cargo/bin:'+env['PATH'];deps=Path(os.environ.get('POCOL_REVIEW_TARGET',str(CRATE/'target')))/'release/deps';extern=[]
for name in ['sha2','sha3','libsecp256k1','serde_json']:
 fs=list(deps.glob('lib'+name+'-*.rlib'));assert len(fs)==1;extern+=['--extern',name+'='+str(fs[0])]
mutants=[('cf1-ignore-time', 'finality_core.rs', 'h >= h_close || ts >= t_close', 'h >= h_close', 'finality_core.rs'), ('cf2-lose-first-close', 'finality_core.rs', 'first_close.or_else', 'None.or_else', 'finality_core.rs'), ('cf3-floor-cursor', 'finality_core.rs', 'records as u128 + ret as u128 - 1', 'records as u128', 'finality_core.rs'), ('cf4-exclusive-gate', 'finality_core.rs', 'h >= height || ts >= time', 'h > height || ts > time', 'finality_core.rs'), ('cf5-ignore-cursor', 'finality_core.rs', 'p.closing && p.cursor_done', 'p.closing && true', 'finality_core.rs'), ('cf6-allow-final-child', 'finality_core.rs', '!parent_final && h <= h_end', 'h <= h_end', 'finality_core.rs'), ('t0-ignore-low-s', 'envelope.rs', '!h.sig.is_empty() && hashes::check_sig_shape(&h.sig).is_err()', 'false', 'envelope.rs'), ('t0-ignore-winner-shape', 'envelope.rs', 'hashes::check_sig_shape(&h.winner_sig).is_err()', 'false', 'envelope.rs'), ('t0-ignore-share-order', 'envelope.rs', 'h.shares.windows(2).any(|v| v[0] >= v[1]) || h.shares.contains(&h.nonce)', 'false', 'envelope.rs'), ('t0-ignore-chain', 'envelope.rs', 'h.chain_id != cfg.chain_id', 'false', 'envelope.rs'), ('t0-ignore-template-auth', 'envelope.rs', 'if !valid {', 'if false {', 'envelope.rs')]
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
