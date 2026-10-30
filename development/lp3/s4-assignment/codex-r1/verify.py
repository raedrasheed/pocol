import concurrent.futures,hashlib,json,os,shutil,subprocess,tempfile,threading,time
from pathlib import Path
root=Path('/workspace/Proof-of-Collaboration');crate=root/'prototypes/lp1-node';work=Path('/workspace/pocol-s4');out=work/'evidence'
env=dict(os.environ,CARGO_HOME='/workspace/pocol-tools/cargo',RUSTUP_HOME='/workspace/pocol-tools/rustup',CARGO_BUILD_JOBS='2')
env['PATH']='/workspace/pocol-tools/cargo/bin:'+env['PATH']
tracked=subprocess.check_output(['git','ls-files','prototypes'],cwd=root,text=True).splitlines()
snapshot={p:hashlib.sha256((root/p).read_bytes()).hexdigest() for p in tracked}
for name in ['oracle.py','oracle_probe.rs','verify.py']:
 snapshot['helper:'+name]=hashlib.sha256((work/name).read_bytes()).hexdigest()
marker=out/'resume-source.json'
if marker.exists():
 assert json.loads(marker.read_text())==snapshot,'Source/helpers changed; use a fresh evidence directory.'
elif (out/'commands.json').exists():
 raise RuntimeError('Existing commands lack a source snapshot; use a fresh evidence directory.')
else:marker.write_text(json.dumps(snapshot,indent=2)+'\n')
records=json.loads((out/'commands.json').read_text()) if (out/'commands.json').exists() else [];lock=threading.Lock()
def run(name,args,cwd=crate,target=work/'review-target'):
 t=time.monotonic();e=dict(env,CARGO_TARGET_DIR=str(target))
 with (out/(name+'.log')).open('w') as f:p=subprocess.run(args,cwd=cwd,env=e,stdout=f,stderr=subprocess.STDOUT)
 r=dict(name=name,args=args,cwd=str(cwd),target=str(target),exitCode=p.returncode,seconds=round(time.monotonic()-t,3))
 with lock:
  records.append(r);(out/'commands.json').write_text(json.dumps(sorted(records,key=lambda r:r['name']),indent=2)+'\n')
 print(name,p.returncode,flush=True);return r
def passed(name,args,**kw):
 if any(r['name']==name and r['exitCode']==0 for r in records):return
 assert run(name,args,**kw)['exitCode']==0,name
passed('rust-version',['rustc','+1.58.1','-vV'])
passed('cargo-version',['cargo','+1.58.1','-V'])
passed('python-version',['python3','--version'])
passed('author-tests',['cargo','+1.58.1','test','--offline','--locked','--lib','--test','assignment'],target=work/'author-target')
passed('review-build',['cargo','+1.58.1','build','--offline','--locked'])
passed('regression',['cargo','+1.58.1','test','--offline','--locked','--lib','--test','assignment','--test','adversarial','--test','fixtures_integration','--test','genesis_spec','--test','profile','--test','timing'])
serde=list((work/'review-target/debug/deps').glob('libserde_json-*.rlib'));assert len(serde)==1
passed('oracle-compile',['rustc','+1.58.1','--edition=2021',str(work/'oracle_probe.rs'),'--extern','lp1_node='+str(work/'review-target/debug/liblp1_node.rlib'),'--extern','serde_json='+str(serde[0]),'-L','dependency='+str(work/'review-target/debug/deps'),'-o',str(work/'oracle-probe')])
passed('review-oracle',['python3',str(work/'oracle.py'),str(work/'oracle-probe')])
passed('verify-fixtures',[str(work/'review-target/debug/lp1-node'),'verify-fixtures'])
passed('fmt',['cargo','+1.58.1','fmt','--','--check'])
source=(crate/'src/assignment.rs').read_text()
mutants={
 'exclusive-membership-cutoff':('member.last_share >= cutoff || member.grace_start >= cutoff','member.last_share > cutoff || member.grace_start > cutoff'),
 'allow-tombs':('if !member.tomb {','if true {'),
 'wrong-seed-endianness':('bytes.extend_from_slice(&attempt.to_be_bytes());','bytes.extend_from_slice(&attempt.to_le_bytes());'),
 'partition-gap':('start: i * nonce_max / n','start: (i * nonce_max + n - 1) / n'),
 'include-parent-counts':('let upper = members.height.checked_sub(2);','let upper = members.height.checked_sub(1);'),
 'wrong-cumulative-boundary':('if sum > index {','if sum >= index {'),
 'modulo-without-rejection':('if x > self.last {','if false {'),
 'wrong-64bit-cap':('if product.bits() > 64','if product.bits() > 65'),
}
def mutation(item):
 name,(old,new)=item;assert source.count(old)==1,(name,old)
 with tempfile.TemporaryDirectory(prefix='mutant-'+name+'-',dir=work) as d:
  d=Path(d);copy=d/'crate';shutil.copytree(crate,copy,ignore=shutil.ignore_patterns('target','__pycache__'))
  (copy/'src/assignment.rs').write_text(source.replace(old,new));target=d/'target'
  r=run('mutant-'+name,['cargo','+1.58.1','test','--offline','--locked','--lib','--test','assignment'],cwd=copy,target=target)
  text=(out/('mutant-'+name+'.log')).read_text()
  caught=r['exitCode']==101 and 'test result: FAILED' in text and 'could not compile' not in text
  return dict(name=name,caught=caught,target=str(target),exitCode=r['exitCode'])
with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:results=list(pool.map(mutation,mutants.items()))
(out/'mutations.json').write_text(json.dumps(results,indent=2)+'\n');assert all(r['caught'] for r in results)
def git(*a):return subprocess.check_output(['git',*a],cwd=root).decode().strip()
base='866666e2f129c26575409d07d8b7b4fa11899f78'
protected=['development/m1','development/lp1','development/lp2','development/lp3/s1-genesisspec','development/lp3/s2-profile','development/lp3/s3-timing','development/decisions','.github','prototypes/lp1-node/Cargo.toml','prototypes/lp1-node/Cargo.lock']
assert not git('diff','--name-only',base,'HEAD','--',*protected)
assert not git('diff','--name-only','HEAD','--','prototypes')
manifest={p:hashlib.sha256((root/p).read_bytes()).hexdigest() for p in git('ls-files','prototypes').splitlines()}
(out/'preservation.json').write_text(json.dumps(dict(base=base,testedCommit=git('rev-parse','HEAD'),protectedPaths=protected,protectedChanges=[],sourceSha256=manifest),indent=2)+'\n')
