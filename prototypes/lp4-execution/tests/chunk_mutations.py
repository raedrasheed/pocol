"""Compile actual D27 contract mutations; build errors are never assertion kills."""
import json,os,pathlib,shutil,subprocess,sys
root=pathlib.Path(__file__).resolve().parents[1];out=pathlib.Path(sys.argv[1]).resolve();out.mkdir(parents=True,exist_ok=True)
prefix='system::chunk_tests::';positive=prefix+'compiled_d27_boundaries_exact_initcode_idempotence_and_no_storage';negative=prefix+'compiled_d27_invalid_lengths_value_and_storage_collision_reject'
controls=[
 ('salt-zero','bytes32 salt=keccak256(data);','bytes32 salt=bytes32(0);',positive),
 ('init-offset','80600a5f395ff300','8060095f395ff300',positive),
 ('runtime-prefix','80600a5f395ff300','80600a5f395ff301',positive),
 ('larger-data-cap','data.length>24575','data.length>24576',prefix+'d27_length_limit_is_independent_of_approved_larger_runtime_cap'),
 ('repeat-create','if(chunk.code.length!=0)','if(false)',positive),
 ('factory-storage','bytes32 salt=keccak256(data);','assembly{sstore(0,1)} bytes32 salt=keccak256(data);',positive),
 ('accept-value','external returns(address chunk)','external payable returns(address chunk)',negative),
 ('ignore-create-failure','if(deployed==address(0)||deployed!=chunk)','if(false)',negative),
]
rows=[]
for name,old,new,test in controls:
 folder=out/name;assert not folder.exists(),'Preserve previous attempt'
 shutil.copytree(root,folder,ignore=shutil.ignore_patterns('target','work','__pycache__'))
 identity='pocol-lp4-execution-chunk-'+name
 p=folder/'Cargo.toml';s=p.read_text();assert s.count('name = "pocol-lp4-execution"')==1;p.write_text(s.replace('name = "pocol-lp4-execution"',f'name = "{identity}"')+'\n[lib]\nname = "pocol_lp4_execution"\n')
 p=folder/'Cargo.lock';p.write_text(p.read_text().replace('name = "pocol-lp4-execution"',f'name = "{identity}"'))
 p=folder/'contracts/ChunkFactory.sol';s=p.read_text();assert s.count(old)==1;p.write_text(s.replace(old,new))
 env=dict(os.environ,CARGO_INCREMENTAL='0');env.pop('LP4_CHUNK_REPORT_PATH',None)
 solc=subprocess.run(['node',str(folder/'tests/compile_chunk.cjs'),str(folder/'tests/data/chunk-factory.json')],env=env,capture_output=True);(out/(name+'-solc.log')).write_bytes(solc.stdout+solc.stderr)
 command=['cargo','test','--offline','--locked','--manifest-path',str(folder/'Cargo.toml'),'--lib']
 build=subprocess.run(command+['--no-run'],env=env,capture_output=True) if solc.returncode==0 else None
 if build is not None:(out/(name+'-build.log')).write_bytes(build.stdout+build.stderr)
 run=subprocess.run(command+[test,'--','--exact','--nocapture'],env=env,capture_output=True) if build is not None and build.returncode==0 else None
 if run is not None:(out/(name+'-test.log')).write_bytes(run.stdout+run.stderr)
 killed=solc.returncode==0 and build is not None and build.returncode==0 and run is not None and run.returncode==101 and b'assertion' in run.stderr+run.stdout and b'1 test' in run.stdout
 rows.append(dict(name=name,test=test,source='contracts/ChunkFactory.sol',old=old,new=new,solcExit=solc.returncode,buildExit=None if build is None else build.returncode,testExit=None if run is None else run.returncode,assertionKilled=killed));(out/'results.json').write_text(json.dumps(rows,indent=2)+'\n');print(name,killed,flush=True)
assert len(rows)==8 and all(x['assertionKilled'] for x in rows),rows
print('8 source-compiled D27 assertion controls verified; full LP4 NOT PASSED')
