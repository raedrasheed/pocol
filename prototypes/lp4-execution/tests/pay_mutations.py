"""Compiled PAY mutation controls. Distinct package identity, shared dependency cache.
Test names select actual compiled Solidity invocation and state assertions.
"""
import os,pathlib,shutil,subprocess,json,sys
root=pathlib.Path(__file__).resolve().parents[1];out=pathlib.Path(sys.argv[1]).resolve();out.mkdir(parents=True,exist_ok=True)
cases=[
 ('auth','pay.rs','|| ![registry_address(),escrow_address()].contains(&inputs.caller)','', 'auth_modes_value_and_direct_reject'),
 ('code-identity','pay.rs','|| context.external.active_code!=Some(inputs.caller)','', 'delegated_foreign_code_cannot_spend_registry_balance'),
 ('gas','pay.rs','PAY_GAS: u64 = 5_000','PAY_GAS: u64 = 4_999','funding_zero_self_and_exact_boundaries'),
 ('padding','pay.rs','|| inputs.input[..12].iter().any(|b|*b!=0)','', 'recipients_overflow_and_encoding_reject_without_effects'),
 ('length','pay.rs','inputs.input.len()!=64','inputs.input.len()<64','recipients_overflow_and_encoding_reject_without_effects'),
 ('value','pay.rs','|| inputs.call_value()!=U256::ZERO','', 'auth_modes_value_and_direct_reject'),
 ('static','pay.rs','|| inputs.is_static','', 'inherited_static_call_cannot_write_balances'),
 ('no-transfer','pay.rs','amount!=U256::ZERO && recipient!=inputs.caller','false && amount!=U256::ZERO && recipient!=inputs.caller','both_roles_transfer_no_callback_and_no_snapshot_commit'),
 # PAY-02 makes absent-account rejection redundant with PAY-01's emptiness guard.
 # Mutate the new independent eligibility predicates directly; historical R2/R4
 # account-create control remains preserved, not asserted as re-run here.
 ('durability-omitted','pay.rs','|| !context.external.durable(recipient)','','pay02_requires_actual_pretransaction_nonce_or_code_even_for_zero'),
 ('durability-balance','pay.rs','account.info.nonce > 0','account.info.balance > U256::ZERO || account.info.nonce > 0','pay02_requires_actual_pretransaction_nonce_or_code_even_for_zero'),
 ('durability-and','pay.rs','|| (account.info.code_hash','&& (account.info.code_hash','pay02_requires_actual_pretransaction_nonce_or_code_even_for_zero'),
 ('zero-address','pay.rs','if recipient==Address::ZERO','if false && recipient==Address::ZERO','recipients_overflow_and_encoding_reject_without_effects'),
 ('journal','pay.rs','let error=evm.journaled_state.transfer(&inputs.caller,&recipient,amount,&mut evm.db)?;', 'evm.journaled_state.state.get_mut(&inputs.caller).unwrap().info.balance-=amount; evm.journaled_state.state.get_mut(&recipient).unwrap().info.balance+=amount; let error:Option<InstructionResult>=None;', 'nested_revert_restores_both_balances'),
 ('callback','native.rs','pay::call(context,&inputs)', 'let _=pay::call(context,&inputs)?; let mut callback=(*inputs).clone(); callback.bytecode_address=revm::primitives::Address::from_slice(&inputs.input[12..32]); callback.target_address=callback.bytecode_address; previous(context,Box::new(callback))', 'both_roles_transfer_no_callback_and_no_snapshot_commit'),
 ('native-bypass' ,'native.rs','if inputs.bytecode_address==pay_address() || inputs.target_address==pay_address()','if false && (inputs.bytecode_address==pay_address() || inputs.target_address==pay_address())','both_roles_transfer_no_callback_and_no_snapshot_commit'),
]
# A composite mutation removes both independent no-creation barriers. Removing
# only the old emptiness guard is now semantically equivalent on valid state;
# never call a surviving redundant mutant a passing control.
source=(root/'src/pay.rs').read_text()
start=source.index('    if recipient==Address::ZERO || !context.external.durable(recipient)')
end=source.index(' || source.info.balance<amount',start)
old=source[start:end]
new=old.replace(' || !context.external.durable(recipient)','').replace(' || dest.is_empty()','')
cases.append(('account-create-both-guards','pay.rs',old,new,'recipients_overflow_and_encoding_reject_without_effects'))
results=[]
for name,file,old,new,test in cases:
 folder=out/name
 if folder.exists():shutil.rmtree(folder)
 shutil.copytree(root,folder,ignore=shutil.ignore_patterns('target','work','__pycache__'))
 manifest=folder/'Cargo.toml';text=manifest.read_text();assert text.count('name = "pocol-lp4-execution"')==1
 manifest.write_text(text.replace('name = "pocol-lp4-execution"',f'name = "pocol-lp4-execution-pay-{name}"')+'\n[lib]\nname = "pocol_lp4_execution"\n')
 lock=folder/'Cargo.lock';lock.write_text(lock.read_text().replace('name = "pocol-lp4-execution"',f'name = "pocol-lp4-execution-pay-{name}"'))
 path=folder/'src'/file;text=path.read_text();assert text.count(old)==1,(name,old);path.write_text(text.replace(old,new))
 env=dict(os.environ,CARGO_TARGET_DIR=str(root/'target'));prefix=['cargo','test','--offline','--locked','--manifest-path',str(manifest),'--test','pay_boundary']
 build=subprocess.run(prefix+['--no-run'],env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT);(out/(name+'-build.log')).write_bytes(build.stdout)
 run=subprocess.run(prefix+[test,'--','--exact','--nocapture'],env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT) if build.returncode==0 else None
 if run is not None:(out/(name+'-test.log')).write_bytes(run.stdout)
 killed=build.returncode==0 and run is not None and run.returncode!=0 and b'assertion' in run.stdout and b'1 test' in run.stdout
 results.append(dict(name=name,file=file,old=old,new=new,test=test,buildExit=build.returncode,testExit=None if run is None else run.returncode,assertionKilled=killed))
 (out/'results.json').write_text(json.dumps(results,indent=2)+'\n')
assert all(r['assertionKilled'] for r in results),results
print(f'PASS: {len(results)} compiled PAY assertion controls; full PAY acceptance remains separate')
