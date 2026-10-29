from pathlib import Path
import os,subprocess,time,json,hashlib
root=Path('/workspace/Proof-of-Collaboration');out=Path('/workspace/pocol-s2-review');crate=root/'prototypes/lp1-node';target='/workspace/pocol-s2-review/target'
env=dict(os.environ,CARGO_HOME='/workspace/pocol-tools/cargo',RUSTUP_HOME='/workspace/pocol-tools/rustup',CARGO_TARGET_DIR=target);env['PATH']='/workspace/pocol-tools/cargo/bin:'+env['PATH']
records=[]
def run(name,args,cwd=crate):
 t=time.monotonic()
 with (out/(name+'.log')).open('w') as f:p=subprocess.run(args,cwd=cwd,env=env,stdout=f,stderr=subprocess.STDOUT)
 records.append(dict(name=name,args=args,cwd=str(cwd),exitCode=p.returncode,seconds=round(time.monotonic()-t,3)))
 (out/'commands.json').write_text(json.dumps(records,indent=2)+'\n');print(name,p.returncode,flush=True);return p.returncode
assert run('rust-version',['rustc','+1.58.1','-vV'])==0
assert run('build',['cargo','+1.58.1','build','--offline','--locked'])==0
assert run('regression',['cargo','+1.58.1','test','--offline','--locked','--lib','--test','profile','--test','genesis_spec','--test','fixtures_integration'])==0
assert run('review-compile',['rustc','+1.58.1','--edition=2021','--test',str(out/'review_checks.rs'),'--extern','lp1_node='+target+'/debug/liblp1_node.rlib','-L','dependency='+target+'/debug/deps','-o',str(out/'review-checks')])==0
assert run('review-checks',[str(out/'review-checks'),'--nocapture'])==0
assert run('verify-fixtures',[target+'/debug/lp1-node','verify-fixtures'])==0
assert run('linux-store-explicit-rejection',['cargo','+1.58.1','test','--offline','--locked','--test','store_journal','writer_is_explicitly_unsupported_off_windows','--','--exact'])==0
assert run('fmt',['cargo','+1.58.1','fmt','--','--check'])==0
# An external caller cannot construct a validated profile using struct literal syntax.
(out/'privacy.rs').write_text('extern crate lp1_node; fn main(){ let _ = lp1_node::profile::ForkSchedule(vec![(1,0)]); }')
code=run('privacy-negative',['rustc','+1.58.1','--edition=2021',str(out/'privacy.rs'),'--extern','lp1_node='+target+'/debug/liblp1_node.rlib','-L','dependency='+target+'/debug/deps','-o',str(out/'privacy')])
assert code==1 and 'private' in (out/'privacy-negative.log').read_text()
(out/'privacy-profile.rs').write_text('extern crate lp1_node; fn attack(p: &mut lp1_node::profile::ProfileIdentity){p.kind=lp1_node::profile::ProfileKind::Network;} fn main(){}')
code=run('privacy-profile-negative',['rustc','+1.58.1','--edition=2021',str(out/'privacy-profile.rs'),'--extern','lp1_node='+target+'/debug/liblp1_node.rlib','-L','dependency='+target+'/debug/deps','-o',str(out/'privacy-profile')])
assert code==1 and 'private' in (out/'privacy-profile-negative.log').read_text()
def git(*a):return subprocess.check_output(['git',*a],cwd=root).decode().strip()
head='f7b521591fa5b3d9a039c8157e0a50d1acee2a86'
protected=['development/m1','development/lp1','development/lp2','development/lp3/s1-genesisspec','development/decisions','.github','prototypes/lp1-node/Cargo.toml','prototypes/lp1-node/Cargo.lock']
assert not git('diff','--name-only','d17273a',head,'--',*protected)
assert not git('diff','--name-only','a9f0460',head,'--','prototypes','.github')
assert not git('diff','--name-only',head,'--','prototypes','development/m1')
files=git('ls-files','prototypes').splitlines()
manifest={p:hashlib.sha256((root/p).read_bytes()).hexdigest() for p in files}
(out/'preservation.json').write_text(json.dumps(dict(reviewedCommit=head,base=git('rev-parse','d17273a'),protectedPaths=protected,protectedChanges=[],implementationEqualsAuthorTestedCommit=git('rev-parse','a9f0460'),filesSha256=manifest),indent=2)+'\n')
