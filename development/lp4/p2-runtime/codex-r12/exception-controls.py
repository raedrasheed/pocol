import pathlib,sys,json,shutil,subprocess,os,hashlib
root=pathlib.Path('/workspace/Proof-of-Collaboration/prototypes/lp4-execution');work=pathlib.Path('/workspace/work/lp4/p2-registry-full');sys.path.insert(0,str(root/'tests'));from gst_exception_audit import verify
corpus=pathlib.Path('/workspace/work/lp4/p2-pay/gst/corpus');rows=[json.loads(l) for l in (work/'hosted-b767/lp4-p2-controls/lp4-controls/gst/cases.jsonl').open()]
controls=[('nonce-guard','src/native.rs','if nonce==Some(u64::MAX) || snapshot.accounts.get(&caller).map(|a|a.info.nonce==u64::MAX).unwrap_or(false) {','if false {','TR_NonceHasMaxValue'),('wrong-reason','src/bin/cancun_control.rs','format!("{e:?}")','"Transaction(LackOfFundForMaxFee { fee: 1, balance: 0 })"','TR_IntrinsicGas')];results=[]
env=os.environ.copy();env.update(CARGO_HOME='/workspace/work/lp4/p2/cargo-home',RUSTUP_HOME='/workspace/pocol-tools/rustup',CARGO_TARGET_DIR=str(root/'target'),PATH='/workspace/pocol-tools/rustup/toolchains/1.99.1-x86_64-unknown-linux-gnu/bin:'+env['PATH'])
for name,file,old,new,label in controls:
 clone=work/('exception-control-'+name);shutil.copytree(root,clone,ignore=shutil.ignore_patterns('target','__pycache__'),dirs_exist_ok=True)
 p=clone/file;s=p.read_text();assert old in s;p.write_text(s.replace(old,new))
 cargo=clone/'Cargo.toml';s=cargo.read_text().replace('name = "pocol-lp4-execution"','name = "pocol-exception-'+name+'"');s+='\n[lib]\nname = "pocol_lp4_execution"\n\n[[bin]]\nname = "exception-'+name+'"\npath = "src/bin/cancun_control.rs"\n';cargo.write_text(s)
 lock=clone/'Cargo.lock';lock.write_text(lock.read_text().replace('name = "pocol-lp4-execution"','name = "pocol-exception-'+name+'"'))
 cmd=['/workspace/pocol-tools/cargo/bin/cargo','+1.99.1','build','--offline','--locked','--manifest-path',str(cargo),'--bin','exception-'+name]
 with (work/('exception-'+name+'-build.log')).open('w') as log:build=subprocess.run(cmd,env=env,stdout=log,stderr=subprocess.STDOUT).returncode
 assert build==0
 selected=[r for r in rows if r.get('expectedException')==label][:2];cases=[]
 for row in selected:
  t=json.loads((corpus/row['file']).read_bytes())[row['name']];post=t['post']['Cancun'][row['variant']];payload=dict(pre=t['pre'],env=t['env'],transaction={k:v for k,v in t['transaction'].items() if k!='secretKey'},indexes=post['indexes'])
  b=subprocess.check_output([str(root/'target/debug'/('exception-'+name))],input=json.dumps(payload)+'\n',text=True);actual=json.loads(b)
  try:
   assert 'error' in actual,'mutant executed invalid transaction'
   verify(t,post,actual['error']);killed=False
  except AssertionError:killed=True
  cases.append(dict(file=row['file'],name=row['name'],variant=row['variant'],assertionKilled=killed,error=actual.get('error'),executed='state' in actual))
 assert any(c['assertionKilled'] for c in cases)
 results.append(dict(name=name,buildExit=build,baselineSHA256={f:hashlib.sha256((root/f).read_bytes()).hexdigest() for f in ['Cargo.lock','src/native.rs','src/bin/cancun_control.rs','tests/gst_exception_audit.py']},cases=cases))
(work/'exception-compiled-controls.json').write_text(json.dumps(results,indent=2)+'\n');print('2 separately compiled controls killed meaningful independent rejection assertions')
