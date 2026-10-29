"""Compiled CODE-01 controls. Compile failure is never a detected mutation."""
import os,pathlib,shutil,subprocess,json,sys
root=pathlib.Path(__file__).resolve().parents[1];out=pathlib.Path(sys.argv[1]).resolve();out.mkdir(parents=True,exist_ok=True)
full='code_caps_engine_tests::independent_engine_limits_against_complete_state_roots'
fields=[
 ('runtime-off','code_caps.rs','Some(limits.runtime as usize)','Some(usize::MAX/2)',full),
 ('runtime-old-cancun','code_caps.rs','Some(limits.runtime as usize)','Some(24576)',full),
 ('initcode-coupled','native.rs','.modify_cfg_env(|cfg|cfg.chain_id=chain_id)','.modify_cfg_env(|cfg|{cfg.chain_id=chain_id;if let Some(l)=limits{cfg.limit_contract_code_size=Some(l.runtime as usize);}})',full),
 ('logical-off','code_caps.rs','if proposed>limits.logical','if false && proposed>limits.logical',full),
 ('logical-equality','code_caps.rs','if proposed>limits.logical','if proposed>=limits.logical',full),
 ('journal-overlay-ignored','code_caps.rs','let total=current_total(context)','let total=inventory(&context.evm.db)',full),
 ('physical-dedup','code_caps.rs','db.accounts.values().filter_map(|a|a.info()).try_fold','{let mut hashes=std::collections::HashSet::new(); db.accounts.values().filter_map(|a|a.info()).filter(|info|hashes.insert(info.code_hash)).collect::<Vec<_>>()}.into_iter().try_fold','code_caps_engine_tests::shared_code_charged_per_address'),
 ('early-selfdestruct-credit','code_caps.rs','base.chain(state.values().map(|a|a.info.clone()))','base.chain(state.values().filter(|a|!a.is_selfdestructed()).map(|a|a.info.clone()))',full),
 ('enclosing-rollback-disabled','code_caps.rs','let previous=handler.execution.create_return.clone();','let call=handler.execution.call_return.clone(); handler.execution.call_return=Arc::new(move |context,frame,mut result|{if result.result==InstructionResult::Revert{result.result=InstructionResult::Return;}call(context,frame,result)}); let previous=handler.execution.create_return.clone();',full),
 ('code-hash-unchecked','code_caps.rs','Some(code) if code.hash_slow()==info.code_hash','Some(code) if true','code_caps_engine_tests::missing_hash_or_r9_reject'),
 ('r9-disabled','code_caps.rs','if limits.runtime<24577','if false && limits.runtime<24577','code_caps_engine_tests::missing_hash_or_r9_reject'),
]
results=[]
for name,source,old,new,test in fields:
    folder=out/name
    if folder.exists():raise RuntimeError(f'Preserve existing attempt; choose new output: {folder}')
    shutil.copytree(root,folder,ignore=shutil.ignore_patterns('target','work','__pycache__'))
    identity='pocol-lp4-execution-code-'+name
    manifest=folder/'Cargo.toml';text=manifest.read_text();assert text.count('name = "pocol-lp4-execution"')==1
    manifest.write_text(text.replace('name = "pocol-lp4-execution"',f'name = "{identity}"')+'\n[lib]\nname = "pocol_lp4_execution"\n')
    lock=folder/'Cargo.lock';lock.write_text(lock.read_text().replace('name = "pocol-lp4-execution"',f'name = "{identity}"'))
    path=folder/'src'/source;text=path.read_text();assert text.count(old)==1,(name,old);path.write_text(text.replace(old,new))
    env=dict(os.environ,CARGO_TARGET_DIR=os.environ.get('CARGO_TARGET_DIR',str(root/'target')),CARGO_INCREMENTAL='0');prefix=['cargo','test','--offline','--locked','--manifest-path',str(manifest),'--lib']
    build=subprocess.run(prefix+['--no-run'],env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT);(out/(name+'-build.log')).write_bytes(build.stdout)
    run=subprocess.run(prefix+[test,'--','--exact','--nocapture'],env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT) if build.returncode==0 else None
    if run is not None:(out/(name+'-test.log')).write_bytes(run.stdout)
    killed=build.returncode==0 and run is not None and run.returncode!=0 and b'assertion' in run.stdout and b'1 test' in run.stdout
    results.append(dict(name=name,old=old,new=new,test=test,buildExit=build.returncode,testExit=None if run is None else run.returncode,assertionKilled=killed))
    (out/'results.json').write_text(json.dumps(results,indent=2)+'\n')
assert all(r['assertionKilled'] for r in results),results
print(f'PASS {len(results)} compiled CODE-01 assertion controls; complete integration NOT PASSED')
