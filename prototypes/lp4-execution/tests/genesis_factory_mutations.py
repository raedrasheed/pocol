"""Compile integration omissions with recomputed genesis identity; no hash-only kill."""
import json,os,pathlib,shutil,subprocess,sys
root=pathlib.Path(__file__).resolve().parents[1];out=pathlib.Path(sys.argv[1]).resolve();out.mkdir(parents=True,exist_ok=True)
test='system::issuance_tests::sealed_genesis_real_factory_signed_user_creation_issuance_and_late_rollback'
controls=[
 ('witness-factory','artifact("ChunkFactory.sol","ChunkFactory").original_bytes()]}','artifact("SystemBoundaryWitness.sol","SystemBoundaryWitness").original_bytes()]}'),
 ('skip-user-factory','c.execute_users(&[raw(v["chunkStore"].as_str().unwrap())])','c.execute_users(&[])'),
]
rows=[]
for name,old,new in controls:
 folder=out/name;assert not folder.exists(),'Preserve previous evidence';shutil.copytree(root,folder,ignore=shutil.ignore_patterns('target','work','__pycache__'))
 identity='pocol-lp4-execution-genesis-'+name;p=folder/'Cargo.toml';s=p.read_text();assert s.count('name = "pocol-lp4-execution"')==1;p.write_text(s.replace('name = "pocol-lp4-execution"',f'name = "{identity}"')+'\n[lib]\nname = "pocol_lp4_execution"\n');p=folder/'Cargo.lock';p.write_text(p.read_text().replace('name = "pocol-lp4-execution"',f'name = "{identity}"'))
 p=folder/'src/system/issuance_tests.rs';s=p.read_text();assert s.count(old)==1;p.write_text(s.replace(old,new));env=dict(os.environ,CARGO_INCREMENTAL='0');env.pop('LP4_ISSUANCE_REPORT_PATH',None)
 command=['cargo','test','--offline','--locked','--manifest-path',str(folder/'Cargo.toml'),'--lib'];build=subprocess.run(command+['--no-run'],env=env,capture_output=True);(out/(name+'-build.log')).write_bytes(build.stdout+build.stderr)
 run=subprocess.run(command+[test,'--','--exact','--nocapture'],env=env,capture_output=True) if build.returncode==0 else None
 if run is not None:(out/(name+'-test.log')).write_bytes(run.stdout+run.stderr)
 killed=build.returncode==0 and run is not None and run.returncode==101 and b'assertion' in run.stdout+run.stderr and b'1 test' in run.stdout
 rows.append(dict(name=name,source='src/system/issuance_tests.rs',old=old,new=new,test=test,buildExit=build.returncode,testExit=None if run is None else run.returncode,assertionKilled=killed));(out/'results.json').write_text(json.dumps(rows,indent=2)+'\n');print(name,killed,flush=True)
assert len(rows)==2 and all(r['assertionKilled'] for r in rows),rows
print('2 compiled actual genesis/factory/signed USER integration controls verified; full LP4 NOT PASSED')
