"""Compiled PAY mutation controls. Distinct package identity, shared dependency cache.
Test names select actual compiled Solidity invocation and state assertions.
"""
import os,pathlib,shutil,subprocess,json,sys
root=pathlib.Path(__file__).resolve().parents[1];out=pathlib.Path(sys.argv[1]).resolve();out.mkdir(parents=True,exist_ok=True)
cases=[
 ('disable','collision.rs','if storage_nonempty(context,target)','if false && storage_nonempty(context,target)','create_and_create2_block_unloaded_nonzero_and_allow_zero'),
 ('partial-scan','collision.rs','unwrap_or(*value)!=U256::ZERO','unwrap_or(U256::ZERO)!=U256::ZERO','create_and_create2_block_unloaded_nonzero_and_allow_zero'),
 ('ignore-clears','collision.rs','journal.and_then(|a|a.storage.get(key)).map(|s|s.present_value()).unwrap_or(*value)','*value','complete_storage_overlay_and_rollback_facts'),
 ('ignore-new-slots','collision.rs','if journal.map(|a|a.storage.values()','if false && journal.map(|a|a.storage.values()','complete_storage_overlay_and_rollback_facts'),
 ('undo-nonce','collision.rs','if storage_nonempty(context,target){return fail(InstructionResult::CreateCollision);}', 'if storage_nonempty(context,target){context.evm.journaled_state.state.get_mut(&inputs.caller).unwrap().info.nonce-=1;return fail(InstructionResult::CreateCollision);}', 'create_and_create2_block_unloaded_nonzero_and_allow_zero'),
 ('cold-target','collision.rs','if storage_nonempty(context,target){return fail(InstructionResult::CreateCollision);}', 'if storage_nonempty(context,target){context.evm.journaled_state.state.get_mut(&target).unwrap().mark_cold();return fail(InstructionResult::CreateCollision);}', 'collision_preserves_target_warmth'),
 ('gas-refund','collision.rs','if storage_nonempty(context,target){return fail(InstructionResult::CreateCollision);}', 'if storage_nonempty(context,target){return fail(InstructionResult::Return);}', 'top_level_creation_collides_and_preserves_snapshot'),
]

results=[]
for name,file,old,new,test in cases:
 folder=out/name
 if folder.exists():shutil.rmtree(folder)
 shutil.copytree(root,folder,ignore=shutil.ignore_patterns('target','work','__pycache__'))
 manifest=folder/'Cargo.toml';text=manifest.read_text();assert text.count('name = "pocol-lp4-execution"')==1
 manifest.write_text(text.replace('name = "pocol-lp4-execution"',f'name = "pocol-lp4-execution-create-{name}"')+'\n[lib]\nname = "pocol_lp4_execution"\n')
 lock=folder/'Cargo.lock';lock.write_text(lock.read_text().replace('name = "pocol-lp4-execution"',f'name = "pocol-lp4-execution-create-{name}"'))
 path=folder/'src'/file;text=path.read_text();assert text.count(old)==1,(name,old);path.write_text(text.replace(old,new))
 env=dict(os.environ,CARGO_TARGET_DIR=str(root/'target'));prefix=['cargo','test','--offline','--locked','--manifest-path',str(manifest),'--test','create_collision']
 build=subprocess.run(prefix+['--no-run'],env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT);(out/(name+'-build.log')).write_bytes(build.stdout)
 run=subprocess.run(prefix+[test,'--','--exact','--nocapture'],env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT) if build.returncode==0 else None
 if run is not None:(out/(name+'-test.log')).write_bytes(run.stdout)
 killed=build.returncode==0 and run is not None and run.returncode!=0 and b'assertion' in run.stdout and b'1 test' in run.stdout
 results.append(dict(name=name,file=file,old=old,new=new,test=test,buildExit=build.returncode,testExit=None if run is None else run.returncode,assertionKilled=killed))
 (out/'results.json').write_text(json.dumps(results,indent=2)+'\n')
assert all(r['assertionKilled'] for r in results),results
print(f'PASS: {len(results)} compiled CREATE assertion controls; full P2 acceptance remains separate')
