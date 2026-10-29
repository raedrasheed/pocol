"""Compiled issuance/builder/pending controls; no build error is a mutation kill.
Solidity mutants rebuild the actual manifest/root before semantic word checks.
"""
import pathlib,os,sys,shutil,subprocess,json
root=pathlib.Path(__file__).resolve().parents[1];out=pathlib.Path(sys.argv[1]).resolve();out.mkdir(parents=True,exist_ok=True)
base='system::issuance_tests::';positive=base+'signed_user_cannot_see_current_issuance_exact_pending_and_replay_rollback';negative=base+'real_phase2_oog_and_late_commitment_error_erase_user_and_credit';supply=base+'actual_root_matching_supply_overflow_and_r23_bound_reject_before_acceptance'
controls=[
 ('missing-credit','src/system.rs','escrow.info.balance.checked_add(amount)','escrow.info.balance.checked_add(U256::ZERO)',positive,1),
 ('duplicate-credit','src/system.rs','self.issued=amount;','escrow.info.balance=escrow.info.balance.checked_add(amount).unwrap(); self.issued=amount;',positive,1),
 ('early-credit','src/system.rs','let users=crate::user_phase::execute_for_profile','self.state.accounts.get_mut(&crate::pay::escrow_address()).unwrap().info.balance+=U256::from_be_bytes(self.profile.spec().cp[15].to_be_bytes()); let users=crate::user_phase::execute_for_profile',positive,1),
 ('final-credit','src/system.rs','if !self.final_block{','if true{',base+'derived_final_host_credit_zero_observed_by_compiled_witness_not_real_finalization',1),
 ('wrong-recipient','src/system.rs','get_mut(&crate::pay::escrow_address()).ok_or(SystemError::Genesis)?','get_mut(&crate::pay::registry_address()).ok_or(SystemError::Genesis)?',positive,1),
 ('phase-guard-off','src/system.rs','if self.stage!=Stage::Ready','if false && self.stage!=Stage::Ready',base+'phase_order_manifest_and_zero_subsidy_fail_closed',1),
 ('manifest-guards-off','src/system.rs','image.verify_candidate(&self.state,self.profile).map_err(|_|SystemError::Genesis)?;','',negative,3),
 ('bad-root-accepted','src/system.rs','if !valid{self.poison();','if false && !valid{self.poison();',negative,1),
 ('r23-bound-off','src/genesis_builder.rs','if supply_bound>=','if false && supply_bound>=',supply,1),
 ('supply-wrapping','src/genesis_builder.rs','a.checked_add(b).ok_or(GenesisError::Supply)','Ok(a.wrapping_add(b))',supply,1),
 ('pending-kind','contracts/PendingEscrow.sol','tx.origin==address(0)?1:','tx.origin==address(0)?2:',positive,1),
 ('pending-offset','contracts/PendingEscrow.sol','uint160(tx.origin))<<8','uint160(tx.origin))<<16',positive,1),
 ('pending-padding','contracts/PendingEscrow.sol','put(b+1,meta);','put(b+1,meta|(uint256(1)<<255));',positive,1),
]
rows=[]
for name,file,old,new,test,count in controls:
 folder=out/name
 if folder.exists():raise RuntimeError(f'Preserve previous attempt; use a fresh output: {folder}')
 shutil.copytree(root,folder,ignore=shutil.ignore_patterns('target','work','__pycache__'))
 identity='pocol-lp4-execution-issuance-'+name;p=folder/'Cargo.toml';s=p.read_text();assert s.count('name = "pocol-lp4-execution"')==1;p.write_text(s.replace('name = "pocol-lp4-execution"',f'name = "{identity}"')+'\n[lib]\nname = "pocol_lp4_execution"\n');p=folder/'Cargo.lock';p.write_text(p.read_text().replace('name = "pocol-lp4-execution"',f'name = "{identity}"'))
 p=folder/file;s=p.read_text();assert s.count(old)==count,(name,s.count(old));p.write_text(s.replace(old,new))
 env=dict(os.environ,CARGO_TARGET_DIR=os.environ.get('CARGO_TARGET_DIR',str(root/'target')),CARGO_INCREMENTAL='0');env.pop('LP4_ISSUANCE_REPORT_PATH',None)
 solc=None
 if file.endswith('.sol'):
  solc=subprocess.run(['node',str(folder/'tests/compile_pending.cjs'),str(folder/'tests/data/pending-escrow.json')],env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT);(out/(name+'-solc.log')).write_bytes(solc.stdout)
 command=['cargo','test','--offline','--locked','--manifest-path',str(folder/'Cargo.toml'),'--lib']
 build=subprocess.run(command+['--no-run'],env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT) if solc is None or solc.returncode==0 else None
 if build is not None:(out/(name+'-build.log')).write_bytes(build.stdout)
 run=subprocess.run(command+[test,'--','--exact','--nocapture'],env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT) if build is not None and build.returncode==0 else None
 if run is not None:(out/(name+'-test.log')).write_bytes(run.stdout)
 killed=(solc is None or solc.returncode==0) and build is not None and build.returncode==0 and run is not None and run.returncode==101 and b'assertion' in run.stdout and b'1 test' in run.stdout
 rows.append(dict(name=name,source=file,old=old,new=new,test=test,sourceOccurrences=count,solcExit=None if solc is None else solc.returncode,buildExit=None if build is None else build.returncode,testExit=None if run is None else run.returncode,assertionKilled=killed));(out/'results.json').write_text(json.dumps(rows,indent=2)+'\n');print(name,killed,flush=True)
assert len(rows)==13 and all(r['assertionKilled'] for r in rows),rows
print('13 compiled issuance/pending/actual-builder assertion controls verified; full LP4 NOT PASSED')
