import sys
import hashlib,json,random,subprocess
from pathlib import Path
rng=random.Random(0x5336544a);rows=[];expected=[]
for _ in range(10000):
 p=rng.choice([0,1000,(1<<63)-1000,(1<<64)-1,rng.getrandbits(64)]);a=rng.choice([0,1,(1<<32)-1,rng.getrandbits(32)]);d=rng.choice([0,1,60,300,(1<<32)-1]);w=rng.choice([0,6,25,300])
 rows.append(dict(type='timing',parent=str(p),attempt=str(a),duration=str(d),wait=str(w)))
 lo=p+a*d+1;hi=p+(a+1)*d;fb=lo+(w if a==0 else 0)
 expected.append(dict(error='duration') if d==0 else dict(error='timestampLimit') if hi>=1<<63 else dict(error='window') if fb>hi else dict(window=dict(lower=str(lo),upper=str(hi)),fallback=str(fb)))
payload=''.join(json.dumps(r,separators=(',',':'))+'\n' for r in rows).encode();p=subprocess.run(['node','--experimental-strip-types',str(Path(__file__).resolve().parents[4]/'prototypes/lp3-model/probe.ts')],input=payload,capture_output=True);assert p.returncode==0,p.stderr
actual=[json.loads(l) for l in p.stdout.splitlines()];assert len(actual)==len(expected);mismatch=[(i,a,e) for i,(a,e) in enumerate(zip(actual,expected)) if a!=e]
result=dict(cases=len(rows),seed=0x5336544a,mismatches=len(mismatch),firstMismatches=mismatch[:2],inputSha256=hashlib.sha256(payload).hexdigest(),outputSha256=hashlib.sha256(p.stdout).hexdigest());(Path(sys.argv[1]).resolve()/'timing-oracle.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result));assert not mismatch
