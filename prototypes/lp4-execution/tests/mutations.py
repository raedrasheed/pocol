"""Compile each control before running its targeted assertion. Preserve failures."""
import json, os, pathlib, shutil, subprocess, sys
root=pathlib.Path(__file__).resolve().parents[1]
out=pathlib.Path(sys.argv[1]).resolve(); out.mkdir(parents=True,exist_ok=True)
source=(root/'src/lib.rs').read_text()
cases=[('wrong-chain','cfg.chain_id = CHAIN_ID','cfg.chain_id = CHAIN_ID + 1','chain_id_opcode_and_invalid_chain'),('unguarded-pay','if evm.context.external.seen','if false && evm.context.external.seen','direct_pay_is_not_accepted_as_empty_account_success'),('wrong-spec','SpecId::CANCUN','SpecId::BYZANTIUM','chain_id_opcode_and_invalid_chain')]
results=[]
for name,old,new,test in cases:
    assert source.count(old)==1
    folder=out/name
    if folder.exists():shutil.rmtree(folder)
    shutil.copytree(root,folder,ignore=shutil.ignore_patterns('target','work','__pycache__'))
    manifest=folder/'Cargo.toml'
    manifest.write_text(manifest.read_text().replace('name = "pocol-lp4-execution"', 'name = "pocol-lp4-execution-'+name+'"')+'\n[lib]\nname = "pocol_lp4_execution"\n')
    lock=folder/'Cargo.lock'
    lock.write_text(lock.read_text().replace('name = "pocol-lp4-execution"', 'name = "pocol-lp4-execution-'+name+'"'))
    (folder/'src/lib.rs').write_text(source.replace(old,new))
    env=dict(os.environ,CARGO_TARGET_DIR=str(root/'target'))
    prefix=['cargo','test','--offline','--locked','--manifest-path',str(folder/'Cargo.toml'),'--lib']
    build=subprocess.run(prefix+['--no-run'],env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT)
    (out/(name+'-build.log')).write_bytes(build.stdout)
    result=subprocess.run(prefix+['tests::'+test,'--','--exact','--nocapture'],env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT) if build.returncode==0 else None
    if result: (out/(name+'-test.log')).write_bytes(result.stdout)
    killed=build.returncode==0 and result is not None and result.returncode!=0 and b'assertion' in result.stdout and b'1 test' in result.stdout
    results.append(dict(name=name,buildExit=build.returncode,testExit=None if result is None else result.returncode,assertionKilled=killed))
(out/'mutations.json').write_text(json.dumps(results,indent=2)+'\n')
assert all(r['assertionKilled'] for r in results),results
print('PASS: 3 compiled assertion controls; no LP4 gate acceptance implied')
