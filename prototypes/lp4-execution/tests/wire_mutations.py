"""Separate compiled signed-wire/fee/receipt controls; compile errors never count as kills."""
import os,pathlib,shutil,subprocess,json,sys
root=pathlib.Path(__file__).resolve().parents[1];out=pathlib.Path(sys.argv[1]).resolve();out.mkdir(parents=True,exist_ok=True)
fields=[
 ('wire-budget','wire.rs','if raw.len()>byte_budget','if false && raw.len()>byte_budget','wire','byte_budget_equality_and_minus_one'),
 ('high-s','wire.rs','if s>n/U256::from(2)','if false && s>n/U256::from(2)','wire','signed_envelopes_match_independent_oracle'),
 ('typed-chain','wire.rs','if uint(&f[0])?!=U256::from(chain_id)','if false && uint(&f[0])?!=U256::from(chain_id)','wire','signed_envelopes_match_independent_oracle'),
 ('signing-domain','wire.rs','if kind!=0{preimage.push(kind)}','if false && kind!=0{preimage.push(kind)}','wire','signed_envelopes_match_independent_oracle'),
 ('canonical-integer','wire.rs','b.len()>32 || b.first()==Some(&0)','b.len()>32','wire','signed_envelopes_match_independent_oracle'),
 ('priority-cap','wire.rs','if priority.map(|p|p>max).unwrap_or(false)','if false && priority.map(|p|p>max).unwrap_or(false)','wire','signed_envelopes_match_independent_oracle'),
 ('fee-denominator','fees.rs','/U512::from(8)','/U512::from(7)','fees','london_fixed_boundaries_and_wide_intermediate'),
 ('fee-minimum','fees.rs','delta.max(U512::from(1))','delta','fees','london_fixed_boundaries_and_wide_intermediate'),
 ('fee-tip','fees.rs','p.min(max_fee-base)','p','fees','fee_cap_priority_and_equality'),
 ('bloom-bit-order','receipts.rs','out[255-bit/8]','out[bit/8]','receipts','independent_bloom_and_indexed_root'),
 ('ordered-index','receipts.rs','trie::ordered_root(values,entry_budget)','trie::ordered_root(&values.iter().rev().cloned().collect::<Vec<_>>(),entry_budget)','receipts','independent_bloom_and_indexed_root'),
 ('receipt-type','receipts.rs','if kind!=0{out.push(kind)}','if false && kind!=0{out.push(kind)}','receipts','empty_root_and_budget_and_supported_types'),
 ('receipt-status','receipts.rs','if success{&[1]}else{&[]}','if success{&[1]}else{&[1]}','receipts','empty_root_and_budget_and_supported_types'),
]
results=[]
for name,source,old,new,suite,test in fields:
    folder=out/name
    if folder.exists():shutil.rmtree(folder)
    shutil.copytree(root,folder,ignore=shutil.ignore_patterns('target','work','__pycache__'))
    identity='pocol-lp4-execution-wire-'+name
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
print(f'PASS {len(results)} compiled wire/fee/receipt assertion controls; full P3 NOT PASSED')
