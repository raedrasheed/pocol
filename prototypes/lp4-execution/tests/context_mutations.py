"""Separate compiled field/ancestor controls; compile errors never count as kills."""
import os,pathlib,shutil,subprocess,json,sys
from mutant_copy import anchor_outside, outputs_before, release_outputs # paths outside the crate stay valid in the copy
root=pathlib.Path(__file__).resolve().parents[1];out=pathlib.Path(sys.argv[1]).resolve();out.mkdir(parents=True,exist_ok=True)
fields=[('number','b.number=block.height','b.number=U256::ZERO'),('timestamp','b.timestamp=block.timestamp','b.timestamp=U256::ZERO'),('coinbase','b.coinbase=pay::escrow_address()','b.coinbase=Address::ZERO'),('prevrandao','b.prevrandao=Some(block.prevrandao)','b.prevrandao=Some(B256::ZERO)'),('gas-limit','b.gas_limit=block.gas_limit','b.gas_limit=U256::from(30_000_000u64)'),('base-fee','b.basefee=block.base_fee','b.basefee=U256::ZERO'),('history','fn block_hash_ref(&self, number: u64) -> Result<B256, Infallible> { Ok(self.db.block_hash(number)) }','fn block_hash_ref(&self, number: u64) -> Result<B256, Infallible> { Ok(revm::primitives::keccak256(number.to_string().as_bytes())) }')]
# The engine reads the snapshot through state_view::StateRef (C-2 item 3), not a clone: the ancestor-cache control
# makes StateRef ignore the snapshot's block hashes (EmptyDB's fallback), as clearing them in the old clone did.
sources={'history':'src/state_view.rs'}
results=[]
for name,old,new in fields:
    folder=out/name
    if folder.exists():shutil.rmtree(folder)
    shutil.copytree(root,folder,ignore=shutil.ignore_patterns('target','work','__pycache__'))
    anchor_outside(root,folder)
    identity='pocol-lp4-execution-context-'+name
    manifest=folder/'Cargo.toml';text=manifest.read_text();assert text.count('name = "pocol-lp4-execution"')==1
    manifest.write_text(text.replace('name = "pocol-lp4-execution"',f'name = "{identity}"')+'\n[lib]\nname = "pocol_lp4_execution"\n')
    lock=folder/'Cargo.lock';lock.write_text(lock.read_text().replace('name = "pocol-lp4-execution"',f'name = "{identity}"'))
    path=folder/sources.get(name,'src/native.rs');text=path.read_text();assert text.count(old)==1,(name,old);path.write_text(text.replace(old,new))
    test='blockhash_exact_256_window_genesis_and_current_boundaries' if name=='history' else 'explicit_fields_execute_and_snapshot_is_immutable'
    env=dict(os.environ,CARGO_TARGET_DIR=str(root/'target'));prefix=['cargo','test','--offline','--locked','--manifest-path',str(manifest),'--test','native_context']
    before=outputs_before(env,folder) # this copy's build outputs are removed once its result is written
    build=subprocess.run(prefix+['--no-run'],env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT);(out/(name+'-build.log')).write_bytes(build.stdout)
    run=subprocess.run(prefix+[test,'--','--exact','--nocapture'],env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT) if build.returncode==0 else None
    if run is not None:(out/(name+'-test.log')).write_bytes(run.stdout)
    killed=build.returncode==0 and run is not None and run.returncode!=0 and b'assertion' in run.stdout and b'1 test' in run.stdout
    results.append(dict(name=name,old=old,new=new,test=test,buildExit=build.returncode,testExit=None if run is None else run.returncode,assertionKilled=killed))
    (out/'results.json').write_text(json.dumps(results,indent=2)+'\n')
    release_outputs(env,folder,before)
assert all(r['assertionKilled'] for r in results),results
print(f'PASS {len(results)} compiled context assertion controls; header/apply provenance NOT PASSED')
