"""Compiled SYS-01 primitive controls. Build failures never count as kills."""
import os,pathlib,shutil,subprocess,json,sys
root=pathlib.Path(__file__).resolve().parents[1];out=pathlib.Path(sys.argv[1]).resolve();out.mkdir(parents=True,exist_ok=True)
prefix_test='system::tests::'
controls=[
 ('origin-system','let origin=self.proposer.origin()?;','let origin=system_address();','system_origin_and_caller_do_not_create_origin_or_system_accounts'),
 ('caller-origin','input.caller=system_address();','input.caller=origin;','system_origin_and_caller_do_not_create_origin_or_system_accounts'),
 ('signed-zero','Self::Signed(a) if a!=Address::ZERO','Self::Signed(a) if true','system_origin_and_caller_do_not_create_origin_or_system_accounts'),
 ('intrinsic-gas','let raw=result.gas.spent();','let raw=result.gas.spent()+21000;','fresh_epochs_match_independent_compiled_witness_gas_and_root'),
 ('refund-credit','self.used=self.used.checked_add(raw)','self.used=self.used.checked_add(raw.saturating_sub(result.gas.refunded().max(0) as u64))','raw_budget_exact_minus_one_remaining_zero_and_refund_no_credit'),
 ('allocation-cap-off','remaining.min(PER_CALL_GAS)','remaining','allocation_is_minimum_of_30m_and_remaining_not_block_gaslimit'),
 ('remaining-budget-off','let allocation=remaining.min(PER_CALL_GAS);','let allocation=PER_CALL_GAS;','allocation_is_minimum_of_30m_and_remaining_not_block_gaslimit'),
 ('nonce-increment','commit_epoch(&mut self.state,state);','commit_epoch(&mut self.state,state); self.state.accounts.entry(system_address()).or_default().info.nonce+=1;','fresh_epochs_match_independent_compiled_witness_gas_and_root'),
 ('fee-deduction','commit_epoch(&mut self.state,state);','commit_epoch(&mut self.state,state); let a=self.state.accounts.entry(system_address()).or_default(); a.info.balance=a.info.balance.saturating_sub(U256::from(raw));','fresh_epochs_match_independent_compiled_witness_gas_and_root'),
 ('warm-slot','let mut input=CallInputs::new_boxed','evm.context.evm.load_account(to).unwrap(); evm.context.evm.sload(to,U256::ZERO).unwrap(); let mut input=CallInputs::new_boxed','fresh_epochs_match_independent_compiled_witness_gas_and_root'),
 ('transient-carry','let mut input=CallInputs::new_boxed','if self.used>0 {evm.context.evm.tstore(to,U256::ZERO,U256::from(7));} let mut input=CallInputs::new_boxed','fresh_epochs_match_independent_compiled_witness_gas_and_root'),
 ('failed-candidate-reusable','fn poison(&mut self){self.failed=true;','fn poison(&mut self){self.failed=false;','later_revert_or_log_invalidates_all_candidate_effects'),
 ('logs-discarded','if !logs.is_empty()','if false && !logs.is_empty()','later_revert_or_log_invalidates_all_candidate_effects'),
 ('pending-offset','origin.as_slice())<<8','origin.as_slice())<<16','independent_pending_encoder_and_invalid_context'),
 ('pending-kind','U256::from(matches!(self,Self::Fallback) as u64)','U256::ZERO','independent_pending_encoder_and_invalid_context'),
]
if len(sys.argv)>2 and sys.argv[2]=='boundary':
 controls=[
  ('nested-gas-overallocation','CallInputs::new_boxed(&evm.context.evm.env.tx,allocation)','CallInputs::new_boxed(&evm.context.evm.env.tx,allocation+allocation/63)','compiled_nested_matches_independent_complete_roots_and_exact_gas'),
  ('nested-revert-committed','// No transact(), caller deduction, intrinsic gas or transaction post-handler.','let previous=evm.handler.execution.call_return.clone(); evm.handler.execution.call_return=std::sync::Arc::new(move |context,frame,mut result|{if result.result==revm::interpreter::InstructionResult::Revert{result.result=revm::interpreter::InstructionResult::Return;}previous(context,frame,result)});','compiled_nested_matches_independent_complete_roots_and_exact_gas'),
 ]
results=[]
for name,old,new,test in controls:
 folder=out/name
 if folder.exists():raise RuntimeError(f'Preserve previous attempt; choose a new output: {folder}')
 shutil.copytree(root,folder,ignore=shutil.ignore_patterns('target','work','__pycache__'))
 identity='pocol-lp4-execution-system-'+name
 manifest=folder/'Cargo.toml';s=manifest.read_text();assert s.count('name = "pocol-lp4-execution"')==1
 manifest.write_text(s.replace('name = "pocol-lp4-execution"',f'name = "{identity}"')+'\n[lib]\nname = "pocol_lp4_execution"\n')
 lock=folder/'Cargo.lock';lock.write_text(lock.read_text().replace('name = "pocol-lp4-execution"',f'name = "{identity}"'))
 source=folder/'src/system.rs';s=source.read_text();assert s.count(old)==1,(name,old);source.write_text(s.replace(old,new))
 env=dict(os.environ,CARGO_TARGET_DIR=os.environ.get('CARGO_TARGET_DIR',str(root/'target')),CARGO_INCREMENTAL='0');env.pop('LP4_SYS_REPORT_PATH',None)
 command=['cargo','test','--offline','--locked','--manifest-path',str(manifest),'--lib']
 build=subprocess.run(command+['--no-run'],env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT);(out/(name+'-build.log')).write_bytes(build.stdout)
 run=subprocess.run(command+[prefix_test+test,'--','--exact','--nocapture'],env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT) if build.returncode==0 else None
 if run is not None:(out/(name+'-test.log')).write_bytes(run.stdout)
 killed=build.returncode==0 and run is not None and run.returncode!=0 and b'assertion' in run.stdout and b'1 test' in run.stdout
 results.append(dict(name=name,old=old,new=new,test=prefix_test+test,buildExit=build.returncode,testExit=None if run is None else run.returncode,assertionKilled=killed))
 (out/'results.json').write_text(json.dumps(results,indent=2)+'\n')
 print(name,build.returncode,None if run is None else run.returncode,killed,flush=True)
assert len(results)==len(controls) and all(r['assertionKilled'] for r in results),results
print(f'{len(results)} compiled SYS-01 primitive assertion controls verified; complete integration NOT PASSED')
