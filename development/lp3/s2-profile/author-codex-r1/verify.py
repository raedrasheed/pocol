import os,pathlib,subprocess,time,json,hashlib,shutil,tempfile
root=pathlib.Path('/workspace/Proof-of-Collaboration'); crate=root/'prototypes/lp1-node'; out=pathlib.Path('/workspace/pocol-s2')
env=dict(os.environ,CARGO_HOME='/workspace/pocol-tools/cargo',RUSTUP_HOME='/workspace/pocol-tools/rustup',CARGO_TARGET_DIR='/workspace/pocol-tools/target')
env['PATH']='/workspace/pocol-tools/cargo/bin:'+env['PATH']
records=[]
def run(name,args,cwd=crate):
 t=time.monotonic()
 with (out/(name+'.log')).open('w') as f:p=subprocess.run(args,cwd=cwd,env=env,stdout=f,stderr=subprocess.STDOUT)
 r=dict(name=name,command=args,cwd=str(cwd),exitCode=p.returncode,seconds=round(time.monotonic()-t,3));records.append(r)
 (out/'commands.json').write_text(json.dumps(records,indent=2)+'\n');print(name,p.returncode,flush=True);return p.returncode
assert run('rust-version',['rustc','+1.58.1','-vV'])==0
assert run('cargo-version',['cargo','+1.58.1','-V'])==0
assert run('build',['cargo','+1.58.1','build','--locked','--offline'])==0
assert run('profile-tests',['cargo','+1.58.1','test','--locked','--offline','--test','profile'])==0
assert run('fmt',['cargo','+1.58.1','fmt','--','--check'])==0
source=(crate/'src/profile.rs').read_text()
mutants={
 'allow-production-simnet':('node_kind == NodeKind::Node && kind != ProfileKind::Network','false'),
 'allow-repeated-heights':('height <= h','height < h'),
 'allow-unknown-version':('if version != 1 {','if false {'),
 'skip-hash-binding':('genesis_spec::verify_hash(bytes, expected_hash)','genesis_spec::decode(bytes)'),
 'bootable-true':('pub fn bootable(&self) -> bool {\n        false','pub fn bootable(&self) -> bool {\n        true'),
}
results=[]
for name,(old,new) in mutants.items():
 assert source.count(old)==1,(name,old)
 with tempfile.TemporaryDirectory(prefix='pocol-s2-mutant-',dir='/workspace/pocol-s2') as d:
  copy=pathlib.Path(d)/'crate';shutil.copytree(crate,copy,ignore=shutil.ignore_patterns('target','__pycache__'))
  (copy/'src/profile.rs').write_text(source.replace(old,new))
  code=run('mutant-'+name,['cargo','+1.58.1','test','--locked','--offline','--test','profile'],copy)
  log=(out/('mutant-'+name+'.log')).read_text()
  caught=code==101 and 'test result: FAILED' in log and 'could not compile' not in log
  results.append(dict(name=name,caught=caught,exitCode=code))
  assert caught,name
(out/'mutations.json').write_text(json.dumps(results,indent=2)+'\n')
# Record identity and protected trees without modifying them.
def git(*a):return subprocess.check_output(['git',*a],cwd=root).decode().strip()
base='d17273af35880f0e1d8a438e529e69c22a18f731'
protected=['development/m1','development/lp1','development/lp2','development/lp3/s1-genesisspec','development/decisions','.github']
changes=git('diff','--name-only',base,'HEAD','--',*protected)
assert not changes,changes
manifest={p:hashlib.sha256((crate/p).read_bytes()).hexdigest() for p in ['src/lib.rs','src/profile.rs','tests/profile.rs','Cargo.toml','Cargo.lock']}
(out/'source-and-preservation.json').write_text(json.dumps(dict(base=base,testedCommit=git('rev-parse','HEAD'),protectedPaths=protected,protectedChanges=[],sourceSha256=manifest),indent=2)+'\n')
