"""External Geth compiled controls, isolated Go source overlays; no runtime changes.
Usage: python gst_geth_mutations.py OUTPUT GETH_SOURCE GO_BINARY CORPUS BASELINE_EVM
Reuses already verified Go dependency cache; builds offline. Geth source/license
remain external, pinned v1.14.12 (GPL-3.0-or-later CLI); no binary distribution.
"""
import pathlib,subprocess,sys,json,os,hashlib
out=pathlib.Path(sys.argv[1]).resolve();out.mkdir(parents=True,exist_ok=True);source=pathlib.Path(sys.argv[2]).resolve();go=sys.argv[3];corpus=pathlib.Path(sys.argv[4]).resolve();baseline=sys.argv[5]
env=os.environ.copy();env.update(GOPROXY='off',GOMAXPROCS='2');rows=[]
controls=[('mul','core/vm/instructions.go','y.Mul(&x, y)','y.Add(&x, y)','VMTests/vmArithmeticTest/mul.json'),('storage-collision','core/vm/evm.go','(storageRoot != (common.Hash{}) && storageRoot != types.EmptyRootHash)','(storageRoot != storageRoot)','stCreate2/create2collisionStorageParis.json')]
for name,file,old,new,fixture in controls:
    def run(binary):
        p=subprocess.run([str(binary),'statetest','--statetest.fork','Cancun',str(corpus/fixture)],capture_output=True,text=True)
        (out/(name+('-baseline' if str(binary)==baseline else '-mutant')+'-stdout.local')).write_text(p.stdout)
        (out/(name+('-baseline' if str(binary)==baseline else '-mutant')+'-stderr.local')).write_text(p.stderr)
        assert p.returncode==0;data=json.loads(p.stdout);assert data and all(x['fork']=='Cancun' for x in data);return data
    original=run(baseline);assert all(r['pass'] for r in original)
    text=(source/file).read_text();assert text.count(old)==1;mutated=out/(name+'.go');mutated.write_text(text.replace(old,new));overlay=out/(name+'-overlay.json');overlay.write_text(json.dumps({'Replace':{str(source/file):str(mutated)}}))
    binary=out/('evm-'+name)
    command=[go,'-C',str(source),'build','-mod=readonly','-buildvcs=false','-p','2','-overlay',str(overlay),'-o',str(binary),'./cmd/evm']
    with (out/(name+'-build.log')).open('w') as log:p=subprocess.run(command,env=env,stdout=log,stderr=subprocess.STDOUT)
    assert p.returncode==0,'compile failure is not a control pass'
    changed=run(binary);assert len(changed)==len(original);failures=[x for x in changed if not x['pass']]
    assert failures and any('post state root mismatch' in x.get('error','').lower() for x in failures),'meaningful native root assertion must fail'
    rows.append(dict(name=name,fixture=fixture,buildExit=0,baselinePasses=len(original),rootAssertionFailures=len(failures),assertionKilled=True,originalSourceSHA256=hashlib.sha256(text.encode()).hexdigest(),mutationSourceSHA256=hashlib.sha256(mutated.read_bytes()).hexdigest(),mutantBinarySHA256=hashlib.sha256(binary.read_bytes()).hexdigest(),old=old,new=new))
(out/'results.json').write_text(json.dumps(rows,indent=2)+'\n');print(f'PASS {len(rows)} separately compiled independent Geth controls rejected by actual state-root assertions')
