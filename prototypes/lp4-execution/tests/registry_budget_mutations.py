"""Compiled controls for the reproduced write-bound contradiction, not acceptance."""
import os,pathlib,shutil,subprocess,json,sys
root=pathlib.Path(__file__).resolve().parents[1];out=pathlib.Path(sys.argv[1]).resolve();out.mkdir(parents=True,exist_ok=True)
results=[]
for index,name in enumerate(['omit-register-counter','omit-reward-counter','omit-claim-counter']):
    folder=out/name
    if folder.exists():shutil.rmtree(folder)
    shutil.copytree(root,folder,ignore=shutil.ignore_patterns('target','work','node_modules','__pycache__'))
    source=folder/'contracts/RegistryWriteBudgetWitness.sol';text=source.read_text()
    needle='sstore(3,add(sload(3),1))' if index else 'sstore(3, add(sload(3),1))'
    if index==0:assert text.count(needle)==1;text=text.replace(needle,'',1)
    else:
        offsets=[];start=0
        while (at:=text.find(needle,start))!=-1:offsets.append(at);start=at+len(needle)
        assert len(offsets)==2
        at=offsets[index-1];text=text[:at]+text[at+len(needle):]
    source.write_text(text)
    manifest=folder/'Cargo.toml';manifest.write_text(manifest.read_text().replace('name = "pocol-lp4-execution"',f'name = "pocol-lp4-budget-{name}"')+'\n[lib]\nname = "pocol_lp4_execution"\n')
    lock=folder/'Cargo.lock';lock.write_text(lock.read_text().replace('name = "pocol-lp4-execution"',f'name = "pocol-lp4-budget-{name}"'))
    compile_sol=subprocess.run(['node',str(folder/'tests/compile_budget_witness.cjs'),str(folder/'tests/data/registry-write-budget.json')],stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
    (out/(name+'-solc.log')).write_bytes(compile_sol.stdout)
    assert compile_sol.returncode==0,(name,'Solidity build failed')
    prefix=['cargo','run','--offline','--locked','--manifest-path',str(manifest),'--example','registry_write_budget']
    env=dict(os.environ,CARGO_TARGET_DIR=str(root/'target'))
    build=subprocess.run(['cargo','build','--offline','--locked','--manifest-path',str(manifest),'--example','registry_write_budget'],env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
    (out/(name+'-build.log')).write_bytes(build.stdout)
    run=subprocess.run(prefix,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT) if build.returncode==0 else None
    if run is not None:(out/(name+'-run.log')).write_bytes(run.stdout)
    killed=build.returncode==0 and run is not None and run.returncode!=0 and b'assertion' in run.stdout and b'left:' in run.stdout
    results.append(dict(name=name,solidityBuildExit=compile_sol.returncode,rustBuildExit=build.returncode,runExit=None if run is None else run.returncode,assertionKilled=killed,meaning='Removing an approved budget write changes the measured lower bound; not a valid resolution'))
    (out/'results.json').write_text(json.dumps(results,indent=2)+'\n')
assert all(r['assertionKilled'] for r in results),results
print('3 compiled assertion controls detected omitted budget writes; original acceptance remains BLOCKED')
