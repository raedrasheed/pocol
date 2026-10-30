import concurrent.futures,hashlib,json,os,shutil,subprocess,tempfile,time
from pathlib import Path
root=Path('/workspace/Proof-of-Collaboration');crate=root/'prototypes/lp1-node';work=Path('/workspace/pocol-s3');out=work/'evidence'
env=dict(os.environ,CARGO_HOME='/workspace/pocol-tools/cargo',RUSTUP_HOME='/workspace/pocol-tools/rustup',CARGO_BUILD_JOBS='2')
env['PATH']='/workspace/pocol-tools/cargo/bin:'+env['PATH']; records=[]
def run(name,args,cwd=crate,target=work/'review-target'):
 t=time.monotonic();e=dict(env,CARGO_TARGET_DIR=str(target))
 with (out/(name+'.log')).open('w') as f:p=subprocess.run(args,cwd=cwd,env=e,stdout=f,stderr=subprocess.STDOUT)
 rec=dict(name=name,args=args,cwd=str(cwd),target=str(target),exitCode=p.returncode,seconds=round(time.monotonic()-t,3))
 records.append(rec);print(name,p.returncode,flush=True);return rec
def passed(name,args,**kw):assert run(name,args,**kw)['exitCode']==0,name
passed('rust-version',['rustc','+1.58.1','-vV'])
passed('cargo-version',['cargo','+1.58.1','-V'])
passed('author-timing',['cargo','+1.58.1','test','--offline','--locked','--test','timing'],target=work/'author-target')
# Separate review phase: fresh target and new Python unlimited-integer oracle.
passed('review-build',['cargo','+1.58.1','build','--offline','--locked'])
passed('regression',['cargo','+1.58.1','test','--offline','--locked','--lib','--test','fixtures_integration','--test','genesis_spec','--test','profile','--test','timing'])
passed('oracle-compile',['rustc','+1.58.1','--edition=2021',str(work/'oracle_probe.rs'),'--extern','lp1_node='+str(work/'review-target/debug/liblp1_node.rlib'),'-L','dependency='+str(work/'review-target/debug/deps'),'-o',str(work/'oracle-probe')])
passed('review-oracle',['python3',str(work/'oracle.py'),str(work/'oracle-probe')])
passed('verify-fixtures',[str(work/'review-target/debug/lp1-node'),'verify-fixtures'])
passed('fmt',['cargo','+1.58.1','fmt','--','--check'])
source=(crate/'src/timing.rs').read_text()
mutants={
 'accept-2pow63':('if upper >= TIMESTAMP_LIMIT {','if upper > TIMESTAMP_LIMIT {'),
 'reject-tj-equality':('arrival_ms <= deadline_ms','arrival_ms < deadline_ms'),
 'future-at-equality':('ts as u128 > clock as u128 + phi as u128','ts as u128 >= clock as u128 + phi as u128'),
 'advance-at-equality':('clock as u128 > self.upper as u128 + gamma as u128','clock as u128 >= self.upper as u128 + gamma as u128'),
 'late-fallback-delay':('if self.attempt == 0 { fb_wait as u128 } else { 0 }','if self.attempt == 0 { fb_wait as u128 } else { fb_wait as u128 }'),
 'wrong-xh':('x_h: (d + t).max(p + 1) + d + 2 * t','x_h: (d + t).max(p + 1) + d + t'),
}
def mutation(item):
 name,(old,new)=item;assert source.count(old)==1,(name,old)
 with tempfile.TemporaryDirectory(prefix='mutant-'+name+'-',dir=work) as d:
  copy=Path(d)/'crate';shutil.copytree(crate,copy,ignore=shutil.ignore_patterns('target','__pycache__'))
  (copy/'src/timing.rs').write_text(source.replace(old,new))
  target=work/('mutant-target-'+name)
  r=run('mutant-'+name,['cargo','+1.58.1','test','--offline','--locked','--test','timing'],cwd=copy,target=target)
  text=(out/('mutant-'+name+'.log')).read_text()
  caught=r['exitCode']==101 and 'test result: FAILED' in text and 'could not compile' not in text
  return dict(name=name,caught=caught,target=str(target),exitCode=r['exitCode'])
with concurrent.futures.ThreadPoolExecutor(max_workers=2) as pool:results=list(pool.map(mutation,mutants.items()))
(out/'mutations.json').write_text(json.dumps(results,indent=2)+'\n');assert all(r['caught'] for r in results)
def git(*a):return subprocess.check_output(['git',*a],cwd=root).decode().strip()
base='30daceb7e8c632041fd44a40afec57553f85fff2'
protected=['development/m1','development/lp1','development/lp2','development/lp3/s1-genesisspec','development/lp3/s2-profile','development/decisions','.github','prototypes/lp1-node/Cargo.toml','prototypes/lp1-node/Cargo.lock']
assert not git('diff','--name-only',base,'HEAD','--',*protected)
assert not git('diff','--name-only','HEAD','--','prototypes')
manifest={p:hashlib.sha256((root/p).read_bytes()).hexdigest() for p in git('ls-files','prototypes').splitlines()}
(out/'preservation.json').write_text(json.dumps(dict(base=base,testedCommit=git('rev-parse','HEAD'),protectedPaths=protected,protectedChanges=[],sourceSha256=manifest),indent=2)+'\n')
(out/'commands.json').write_text(json.dumps(sorted(records,key=lambda r:r['name']),indent=2)+'\n')
