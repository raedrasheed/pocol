import hashlib,json,pathlib,subprocess
ROOT=pathlib.Path('/workspace/Proof-of-Collaboration')
def git(*args):return subprocess.check_output(['git',*args],cwd=ROOT)
head=git('rev-parse','HEAD').decode().strip()
checks=[]
groups=[('LP3 baseline','fd7a17f',['development/m1','development/lp1','development/lp2','development/decisions']),('LP2 dependencies and licenses','f5737e3',['prototypes/lp1-node/Cargo.lock','prototypes/lp1-node/fixtures']),('tested code and CI','4e26e4d',['prototypes','.github']),('round 1','c9364a2',['development/lp3/s1-genesisspec/author-r1']),('round 2','26ee0a4',['development/lp3/s1-genesisspec/author-r2']),('round 3','9c02c7f',['development/lp3/s1-genesisspec/author-r3-windows-ci']),('round 4','4e26e4d',['development/lp3/s1-genesisspec/author-r4-encoding-fix']),('failed Windows run','086a3ea',['development/lp3/s1-genesisspec/windows-run-37972422364'])]
for name,base,paths in groups:
    diff=git('diff','--name-status',base,head,'--',*paths).decode()
    checks.append(dict(name=name,base=git('rev-parse',base).decode().strip(),paths=paths,changedFiles=diff.splitlines(),ok=not diff))
    assert not diff,(name,diff)
manifest={}
for p in git('ls-files','-z','prototypes','.github').decode().split('\0'):
    if p:manifest[p]=hashlib.sha256((ROOT/p).read_bytes()).hexdigest()
print(json.dumps(dict(ok=True,reviewedHead=head,windowsTestedCommit=git('rev-parse','4e26e4d').decode().strip(),checks=checks,currentSourceSha256=manifest),indent=2))
