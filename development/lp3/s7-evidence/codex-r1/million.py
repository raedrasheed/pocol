"""Streaming independent native-integer oracle, full traces in Rust and TS."""
import hashlib,json,random,subprocess,sys,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[4];OUT=Path(sys.argv[1]).resolve();COUNT=int(sys.argv[2]) if len(sys.argv)>2 else 1000000;BATCH=10000;seed=0x53372d4d494c4c;rng=random.Random(seed);M=(1<<256)-1
def calc(target,g,T,tau,ts,h):
 dt=ts-g-T*h;num=dt*65536;e=num//tau;s=e//65536;f=e-65536*s;mb=max(abs(num).bit_length(),abs(e).bit_length())
 if s>=256 or s<=-257:return dict(target=str(M if s>=256 else 1),dt=str(dt),e=str(e),s=str(s),f=str(f),F=None,early='high' if s>=256 else 'low',maxBits=mb)
 poly=195766423245049*f+971821376*f*f+5127*f*f*f+(1<<47);F=65536+(poly>>48);X=target*F;k=s-16;Y=X<<k if k>=0 else X>>(-k)
 return dict(target=str(min(M,max(1,Y))),dt=str(dt),e=str(e),s=str(s),f=str(f),F=str(F),early=None,maxBits=max(mb,poly.bit_length(),X.bit_length(),Y.bit_length()))
hashes=[hashlib.sha256() for _ in range(3)];batches=[];maxbits=0;counts={};start=time.monotonic()
for offset in range(0,COUNT,BATCH):
 rows=[];want=[]
 for i in range(min(BATCH,COUNT-offset)):
  s=rng.randrange(-300,301);tau=rng.choice([1,600,3600,65536,65537]);T=rng.choice([10,60]);h=rng.randrange(2**32);g=1700000000;ts=g+h*T+s*tau+rng.randrange(tau);target=rng.choice([1,M,1<<240,rng.getrandbits(256) or 1]);expected=calc(target,g,T,tau,ts,h);assert -300<=int(expected['s'])<=300;maxbits=max(maxbits,expected['maxBits']);counts[expected['s']]=counts.get(expected['s'],0)+1
  rows.append(dict(type='asert',target=str(target),g=str(g),T=str(T),tau=str(tau),ts=str(ts),h=str(h)));want.append(expected)
 payload=''.join(json.dumps(r,separators=(',',':'))+'\n' for r in rows).encode();hashes[0].update(payload)
 for j,args in enumerate([['node','--experimental-strip-types',str(ROOT/'prototypes/lp3-model/probe.ts')],[str(OUT/'asert-probe')]],1):
  p=subprocess.run(args,input=payload,capture_output=True);assert p.returncode==0,(offset,args,p.stderr[:1000]);hashes[j].update(p.stdout);actual=[json.loads(l) for l in p.stdout.splitlines()];assert len(actual)==len(want)
  for k,(expected,got) in enumerate(zip(want,actual)):assert expected==got,(offset+k,args,expected,got)
 batches.append(dict(first=offset,cases=len(rows),matchedRust=len(rows),matchedTS=len(rows),inputSha256=hashlib.sha256(payload).hexdigest()))
 if (offset+BATCH)%100000==0:print('matched',offset+BATCH,flush=True)
result=dict(cases=COUNT,seed=seed,batchSize=BATCH,sRange=[-300,300],distinctS=len(counts),sCounts=counts,maxBits=maxbits,mismatchesRust=0,mismatchesTS=0,inputSha256=hashes[0].hexdigest(),tsOutputSha256=hashes[1].hexdigest(),rustOutputSha256=hashes[2].hexdigest(),seconds=round(time.monotonic()-start,3),batches=batches)
(OUT/'million.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:v for k,v in result.items() if k not in ['batches','sCounts']}),flush=True)
