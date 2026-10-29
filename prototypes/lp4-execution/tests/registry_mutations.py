"""Compiled Solidity + Rust actual Registry/bridge controls; no compile-error credit."""
import os,pathlib,shutil,subprocess,json,sys,hashlib
root=pathlib.Path(__file__).resolve().parents[1];out=pathlib.Path(sys.argv[1]).resolve();out.mkdir(parents=True,exist_ok=True)
cases=[
 ('paid-as-held','&& ((w>>16)&255)==1;','&& ((w>>16)&255)!=2;','strict_calldata_status_exclusions_and_maximum_bond_failure_are_atomic'),
 ('regops','(c&65535)>=cfg(23)','(c&65535)>cfg(23)','budgets_exact_limits_failed_calls_and_phase_replay_cannot_reset'),
 ('rmax','((c>>16)&255)>=cfg(17)','((c>>16)&255)>cfg(17)','budgets_exact_limits_failed_calls_and_phase_replay_cannot_reset'),
 ('counter-exempt','put(3,c+1+','put(3,c+0+','budgets_exact_limits_failed_calls_and_phase_replay_cannot_reset'),
 ('probe','if(!SystemAddress.pay(arg,0))revert RewardNotDurable();','','zero_bond_durable_code_only_and_balance_only_admission'),
 ('closing','if(c&CLOSING!=0)revert Closing();','if(false)revert Closing();','prefix_precedence_budgets_and_durable_probe_fail_without_state_commit'),
 ('bridge-auth','if(msg.sender!=SystemAddress.REGISTRY) revert Unauthorized();','','bridge_authorization_and_missing_refund_destination_roll_back_all_effects'),
 ('bond-amount','SystemAddress.pay(reward,bond)','SystemAddress.pay(reward,bond+1)','full_register_reward_exit_middle_claim_use_approved_ceilings_and_escrow'),
 ('swallow-refund','IEscrow(SystemAddress.ESCROW).releaseBond(id);','try IEscrow(SystemAddress.ESCROW).releaseBond(id) {} catch {}','bridge_authorization_and_missing_refund_destination_roll_back_all_effects'),
 ('due-equality','height<((get(r+3)>>64)&LOW64)','height<=((get(r+3)>>64)&LOW64)','full_register_reward_exit_middle_claim_use_approved_ceilings_and_escrow'),
 ('reactivate-equality','height<sumHeight(clocks&LOW64,cfg(24))','height<=sumHeight(clocks&LOW64,cfg(24))','reactivation_inactivity_and_closing_refunds_actual_record_amounts'),
 ('inactive-equality','height<sumHeight(last>grace?last:grace,cfg(25))','height<=sumHeight(last>grace?last:grace,cfg(25))','reactivation_inactivity_and_closing_refunds_actual_record_amounts'),
 ('ret','uint256 end=cursor+cfg(50)','uint256 end=cursor+cfg(50)+1','closing_ret_counts_all_records_and_never_pays_twice'),
 ('sweep-scan','i<scans &&','i<=scans &&','actual_sweep_scan_and_deletion_caps_preserve_swapped_cursor'),
 ('sweep-delete','removed<deletes &&','removed<=deletes &&','actual_sweep_scan_and_deletion_caps_preserve_swapped_cursor'),
 ('phase-reset','c&BEGIN_FLAG!=0 && height<=((c>>24)&LOW64)','false && height<=((c>>24)&LOW64)','budgets_exact_limits_failed_calls_and_phase_replay_cannot_reset'),
 ('reward-extra-write','put(r+1,uint160(arg));}','put(r+1,uint160(arg));put(8,1);}','full_register_reward_exit_middle_claim_use_approved_ceilings_and_escrow'),
]
baseline=out/'baseline'
if baseline.exists():shutil.rmtree(baseline)
shutil.copytree(root,baseline,ignore=shutil.ignore_patterns('target','work','node_modules','__pycache__'))
source_hashes={name:hashlib.sha256((baseline/name).read_bytes()).hexdigest() for name in ['contracts/RegistryPayment.sol','tests/registry_payment.rs','src/native.rs','Cargo.lock']}
results=[]
for name,old,new,test in cases:
 folder=out/name
 if folder.exists():shutil.rmtree(folder)
 shutil.copytree(baseline,folder,ignore=shutil.ignore_patterns('target','work','node_modules','__pycache__'))
 path=folder/'contracts/RegistryPayment.sol';source=path.read_text();count=source.count(old);assert count==(2 if name=='probe' else 1),(name,count);path.write_text(source.replace(old,new))
 manifest=folder/'Cargo.toml';manifest.write_text(manifest.read_text().replace('name = "pocol-lp4-execution"',f'name = "pocol-lp4-registry-{name}"')+'\n[lib]\nname = "pocol_lp4_execution"\n')
 lock=folder/'Cargo.lock';lock.write_text(lock.read_text().replace('name = "pocol-lp4-execution"',f'name = "pocol-lp4-registry-{name}"'))
 compile_sol=subprocess.run(['node',str(folder/'tests/compile_registry.cjs'),str(folder/'tests/data/registry-payment.json')],stdout=subprocess.PIPE,stderr=subprocess.STDOUT);(out/(name+'-solc.log')).write_bytes(compile_sol.stdout)
 env=dict(os.environ,CARGO_TARGET_DIR=str(root/'target'));prefix=['cargo','test','--offline','--locked','--manifest-path',str(manifest),'--test','registry_payment']
 build=subprocess.run(prefix+['--no-run'],env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT) if compile_sol.returncode==0 else None
 if build is not None:(out/(name+'-build.log')).write_bytes(build.stdout)
 run=subprocess.run(prefix+[test,'--','--exact','--nocapture'],env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT) if build is not None and build.returncode==0 else None
 if run is not None:(out/(name+'-test.log')).write_bytes(run.stdout)
 killed=compile_sol.returncode==0 and build is not None and build.returncode==0 and run is not None and run.returncode!=0 and b'assertion' in run.stdout and b'1 test' in run.stdout
 results.append(dict(baselineSHA256=source_hashes,name=name,old=old,new=new,occurrences=count,test=test,solidityBuildExit=compile_sol.returncode,rustBuildExit=None if build is None else build.returncode,testExit=None if run is None else run.returncode,assertionKilled=killed))
 (out/'results.json').write_text(json.dumps(results,indent=2)+'\n')
assert all(r['assertionKilled'] for r in results),results
print(f'PASS {len(results)} separately Solidity/Rust compiled Registry controls; full LP4 NOT PASSED')
