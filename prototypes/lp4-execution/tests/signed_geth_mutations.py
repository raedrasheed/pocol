"""Compiled independent signed/header controls; infrastructure errors never PASS.
Usage: python signed_geth_mutations.py GO BUILT_RUNNER_DIR SIGNED_INPUTS OUTPUT
"""
import pathlib,json,subprocess,os,sys,hashlib
binary,built,data,out=sys.argv[1],pathlib.Path(sys.argv[2]).resolve(),pathlib.Path(sys.argv[3]).resolve(),pathlib.Path(sys.argv[4]).resolve();out.mkdir(parents=True,exist_ok=True);module=built/'module';source=module/'main.go';original=source.read_text();rows=[]
controls=[('blob-header','if err:=eip4844.VerifyEIP4844Header(parent,h);err!=nil','if err:=eip4844.VerifyEIP4844Header(parent,h);false && err!=nil'),('sender-eoa',' st,err:=state.New',' msg.SkipFromEOACheck=true\n st,err:=state.New'),('nonce',' st,err:=state.New',' msg.SkipNonceChecks=true\n st,err:=state.New')]
for name,old,new in controls:
 assert original.count(old)==1;mutant=out/(name+'.go');mutant.write_text(original.replace(old,new));overlay=out/(name+'-overlay.json');overlay.write_text(json.dumps({'Replace':{str(source):str(mutant)}}));target=out/(name+'-control');command=[binary,'build','-mod=readonly','-buildvcs=false','-p','2','-overlay',str(overlay),'-o',str(target),'.']
 with (out/(name+'-build.log')).open('w') as log:p=subprocess.run(command,cwd=module,stdout=log,stderr=subprocess.STDOUT)
 if p.returncode==0:r=subprocess.run([sys.executable,str(pathlib.Path(__file__).with_name('gst_signed_geth_verify.py')),str(target),str(data),str(out/(name+'-results'))],capture_output=True,text=True);(out/(name+'-assertion.log')).write_text(r.stdout+r.stderr)
 else:r=None
 killed=p.returncode==0 and r is not None and r.returncode!=0 and 'AssertionError' in r.stderr
 rows.append(dict(name=name,buildExit=p.returncode,assertionExit=None if r is None else r.returncode,assertionKilled=killed,old=old,new=new,mutatedSourceSHA256=hashlib.sha256(mutant.read_bytes()).hexdigest()));(out/'results.json').write_text(json.dumps(rows,indent=2)+'\n')
assert len(rows)==3 and all(r['assertionKilled'] for r in rows),rows
print('PASS3 compiled independent Geth signed/header assertion controls; full P2/P3 NOT PASSED')
