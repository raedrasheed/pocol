"""Separate compiled signed-wire/fee/receipt controls; compile errors never count as kills."""
import os,pathlib,shutil,subprocess,json,sys
root=pathlib.Path(__file__).resolve().parents[1];out=pathlib.Path(sys.argv[1]).resolve();out.mkdir(parents=True,exist_ok=True)
fields=[
 ('eip161-cleanup','user_phase.rs','for address in removed{','for address in removed.into_iter().filter(|_|false){','user_phase','sequential_signed_execution_refund_revert_and_commitments'),
 ('user-commitments','user_phase.rs','if candidate.commitments()!=expected','if false && candidate.commitments()!=expected','user_phase','wrong_commitments_and_late_invalid_transaction_leave_snapshot_unchanged'),
 ('gas-reservation','user_phase.rs','if tx.gas_limit>limit.checked_sub(used)','if false && tx.gas_limit>limit.checked_sub(used)','user_phase','wrong_commitments_and_late_invalid_transaction_leave_snapshot_unchanged'),
 ('block-tips','user_phase.rs','tip.checked_mul(U256::from(gas))','U256::ZERO.checked_mul(U256::from(gas))','user_phase','sequential_signed_execution_refund_revert_and_commitments'),
 ('cumulative-receipt','user_phase.rs','result.result.is_success(),used,','result.result.is_success(),gas,','user_phase','sequential_signed_execution_refund_revert_and_commitments'),
 ('state-commit','user_phase.rs','state.commit(result.state);','let _ignored=result.state;','user_phase','sequential_signed_execution_refund_revert_and_commitments'),
]
results=[]
for name,source,old,new,suite,test in fields:
    folder=out/name
    if folder.exists():shutil.rmtree(folder)
    shutil.copytree(root,folder,ignore=shutil.ignore_patterns('target','work','__pycache__'))
    identity='pocol-lp4-execution-user-'+name
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
print(f'PASS {len(results)} compiled USER-phase assertion controls; full P3 NOT PASSED')
