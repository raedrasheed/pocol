"""Independent Python/native-integer ASERT, compared to both authored models."""
import hashlib,json,random,subprocess,sys,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[4];OUT=Path(sys.argv[1]).resolve();COUNT=int(sys.argv[2]) if len(sys.argv)>2 else 12000
rng=random.Random(0x53364153455254);MOD=(1<<256)-1;rows=[];want=[];maximum=0
def calc(target,g,T,tau,ts,h):
 dt=ts-g-T*h;num=dt*65536;e=num//tau;s=e//65536;f=e-65536*s;mb=max(abs(num).bit_length(),abs(e).bit_length())
 if s>=256 or s<=-257:return dict(target=str(MOD if s>=256 else 1),dt=str(dt),e=str(e),s=str(s),f=str(f),F=None,early='high' if s>=256 else 'low',maxBits=mb),0
 poly=195766423245049*f+971821376*f*f+5127*f*f*f+(1<<47);F=65536+(poly>>48);X=target*F;k=s-16;Y=X<<k if k>=0 else X>>(-k);mb=max(mb,poly.bit_length(),X.bit_length(),Y.bit_length());shift=max(X.bit_length(),max(1,Y.bit_length()))
 return dict(target=str(min(MOD,max(1,Y))),dt=str(dt),e=str(e),s=str(s),f=str(f),F=str(F),early=None,maxBits=mb),shift
for j in range(COUNT):
 s=rng.randrange(-300,301);tau=rng.choice([1,600,3600,65536,65537]);T=rng.choice([10,60]);h=rng.randrange(2**32);g=1700000000;ts=g+h*T+s*tau+rng.randrange(tau);target=rng.choice([1,MOD,1<<240,rng.getrandbits(256) or 1]);expected,shift=calc(target,g,T,tau,ts,h);assert -300<=int(expected['s'])<=300
 rows.append(dict(type='asert',target=str(target),g=str(g),T=str(T),tau=str(tau),ts=str(ts),h=str(h)));want.append((expected,shift));maximum=max(maximum,expected['maxBits'])
payload=''.join(json.dumps(r,separators=(',',':'))+'\n' for r in rows).encode();start=time.monotonic()
ts=subprocess.run(['node','--experimental-strip-types',str(ROOT/'prototypes/lp3-model/probe.ts')],input=payload,capture_output=True);assert ts.returncode==0,ts.stderr
actual=[json.loads(l) for l in ts.stdout.splitlines()];bad=[dict(index=i,expected=w[0],got=a) for i,(w,a) in enumerate(zip(want,actual)) if a!=w[0]];assert len(actual)==COUNT
rust_rows=[dict(targetG=r['target'],gts=r['g'],blockTime=r['T'],tau=r['tau'],ts=r['ts'],h=r['h'],target=w['target'],early=w['early'],maxShiftBits=shift) for r,(w,shift) in zip(rows,want)]
inp=OUT/'asert-input.json';inp.write_text(json.dumps(dict(rows=rust_rows),separators=(',',':')))
rs=subprocess.run([str(ROOT/'prototypes/lp1-node/target/debug/lp1-node'),'asert-batch','--in',str(inp)],capture_output=True);assert rs.returncode==0,rs.stderr
rust=[json.loads(l) for l in rs.stdout.splitlines()];assert rust[-1]['matched']==COUNT
assert all(row['match'] and row['maxBits']<=512 for row in rust[:-1]);assert not bad,bad[:2]
result=dict(seed=0x53364153455254,cases=COUNT,sRange=[-300,300],tsMismatches=len(bad),rustMatched=rust[-1]['matched'],maxBits=maximum,seconds=round(time.monotonic()-start,3),inputSha256=hashlib.sha256(payload).hexdigest(),tsOutputSha256=hashlib.sha256(ts.stdout).hexdigest(),rustOutputSha256=hashlib.sha256(rs.stdout).hexdigest())
(OUT/'arithmetic-oracle.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
