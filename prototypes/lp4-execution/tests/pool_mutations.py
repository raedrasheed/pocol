"""Separate compiled signed-wire/fee/receipt controls; compile errors never count as kills."""
import os,pathlib,shutil,subprocess,json,sys
root=pathlib.Path(__file__).resolve().parents[1];out=pathlib.Path(sys.argv[1]).resolve();out.mkdir(parents=True,exist_ok=True)
fields=[
 ('rpc-chain-code','wire.rs','=>Some(-32015)','=>Some(-32016)','rpc_pool','actual_rpc_handler_rejects_chain_type_unprotected_and_keeps_pool_and_snapshot'),
 ('pool-size','pool.rs','if raw.len()>self.profile.cp().TX_MAX as usize','if false && raw.len()>self.profile.cp().TX_MAX as usize','pool','actual_admission_identity_size_capacity_and_reorg_recheck'),
 ('pool-capacity','pool.rs','if !duplicate && raw.len()>self.budget.saturating_sub(self.bytes)','if false && !duplicate && raw.len()>self.budget.saturating_sub(self.bytes)','pool','actual_admission_identity_size_capacity_and_reorg_recheck'),
 ('reorg-clear','pool.rs','let old=std::mem::take(&mut self.entries);','let old=self.entries.clone();','pool','actual_admission_identity_size_capacity_and_reorg_recheck'),
 ('stale-duplicate','pool.rs','let duplicate=self.entries.iter().any(|s|s.hash()==hash);','let duplicate=self.entries.iter().any(|s|s.hash()==hash);if duplicate{return Ok(hash)}','pool','actual_admission_identity_size_capacity_and_reorg_recheck'),
 ('gas-context','pool.rs','if block.gas_limit!=revm::primitives::U256::from(self.profile.cp().GAS_LIMIT)','if false && block.gas_limit!=revm::primitives::U256::from(self.profile.cp().GAS_LIMIT)','pool','source_identity_hash_and_changed_profile_cannot_reuse_admission'),
 ('native-profile-chain','native.rs','cfg.chain_id=chain_id','cfg.chain_id=CHAIN_ID','pool','network_identity_drives_both_signature_domain_and_actual_chainid_opcode'),
 ('wire-profile-chain','wire.rs','decode_on_chain(raw,profile.cp().TX_MAX as usize,profile.spec().chain_id)','decode_on_chain(raw,profile.cp().TX_MAX as usize,CHAIN_ID)','pool','network_identity_drives_both_signature_domain_and_actual_chainid_opcode'),
 ('http-host','rpc_http.rs','if value!=format!("127.0.0.1:{port}")','if false && value!=format!("127.0.0.1:{port}")','rpc_pool','real_loopback_http_submission_delivers_ci2_error_without_state_or_pool_write'),
 ('http-origin','rpc_http.rs','"origin"=>return Err(ReqErr::Status(403,"originUnconfigured")),','"origin"=>{},','rpc_pool','real_loopback_http_submission_delivers_ci2_error_without_state_or_pool_write'),
]
results=[]
for name,source,old,new,suite,test in fields:
    folder=out/name
    if folder.exists():shutil.rmtree(folder)
    shutil.copytree(root,folder,ignore=shutil.ignore_patterns('target','work','__pycache__'))
    identity='pocol-lp4-execution-pool-'+name
    manifest=folder/'Cargo.toml';text=manifest.read_text();assert text.count('name = "pocol-lp4-execution"')==1
    manifest.write_text(text.replace('name = "pocol-lp4-execution"',f'name = "{identity}"')+'\n[lib]\nname = "pocol_lp4_execution"\n')
    lock=folder/'Cargo.lock';lock.write_text(lock.read_text().replace('name = "pocol-lp4-execution"',f'name = "{identity}"'))
    path=folder/'src'/source;text=path.read_text();assert text.count(old)==1,(name,old);path.write_text(text.replace(old,new))
    env=dict(os.environ,CARGO_TARGET_DIR=str(root/'target'));prefix=['cargo','test','--offline','--locked','--manifest-path',str(manifest),'--test',suite]
    build=subprocess.run(prefix+['--no-run'],env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT);(out/(name+'-build.log')).write_bytes(build.stdout)
    run=subprocess.run(prefix+[test,'--','--exact','--nocapture'],env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT) if build.returncode==0 else None
    if run is not None:(out/(name+'-test.log')).write_bytes(run.stdout)
    killed=build.returncode==0 and run is not None and run.returncode!=0 and b'assertion' in run.stdout and b'1 test' in run.stdout
    results.append(dict(name=name,old=old,new=new,test=test,buildExit=build.returncode,testExit=None if run is None else run.returncode,assertionKilled=killed))
    (out/'results.json').write_text(json.dumps(results,indent=2)+'\n')
assert all(r['assertionKilled'] for r in results),results
print(f'PASS {len(results)} compiled profile/pool/RPC assertion controls; full P3 NOT PASSED')
