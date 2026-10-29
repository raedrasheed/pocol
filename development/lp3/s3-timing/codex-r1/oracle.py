"""Separate review oracle: Python unlimited-integer timing formulas, seed fixed below."""
import hashlib,json,random,subprocess,sys
from pathlib import Path
out=Path('/workspace/pocol-s3/evidence'); rng=random.Random(0x533320261017)
L=1<<63; M=(1<<64)-1; A=(1<<32)-1; Q=(1<<128)-1
def expected(v):
 p,a,d,c,g,f,r,rel,delta,phi,exe,arrival=v
 hi=p+(a+1)*d; lo=p+a*d+1
 valid=d>0 and hi<L
 ans=['-']*7; tj=None
 if valid:
  fb=lo+(f if a==0 else 0)
  if fb>hi:fb=None
  prop=max(lo,c) if a==0 and max(lo,c)<=hi else None
  if a==0 and fb is not None and r+1000*rel<=Q:tj=max(r+1000*rel,1000*fb)
  ans=[lo,hi,fb,prop,tj,int(c>hi+g),int(a>=1 and c>=lo)]
 if d==0 or p>=L:maxa='E'
 else:
  count=(L-1-p)//d
  maxa='N' if count==0 else min(count-1,A)
 ans += [maxa,int(c>p+phi),max(delta,phi+1)+exe,max(delta+exe,phi+1)+delta+2*exe,max(phi+1+f,delta+exe+rel),None if tj is None else int(arrival<=tj)]
 return ['-' if x is None else str(x) for x in ans]
rows=[]
for p in [0,1000,L-301,L-2,L-1,L,M]:
 for a in [0,1,A]:
  for d in [0,1,300,A]:
   for c in [0,min(M,p+1),M]:rows.append([p,a,d,c,M,25,Q,1,5,10,3,Q])
# Fine-grained boundaries, both sides of 2^63 and millisecond equality/overflow.
for p in [0,1000,L-301,L-2]:
 for d in [1,300]:
  for f in [0,1,25,299,300,A]:
   for r in [0,1020123,Q-1000,Q]:
    v=[p,0,d,p,30,f,r,1,5,10,3,0]
    e=expected(v)
    if e[4]!='-':
     tj=int(e[4])
     for off in [-1,0,1]:
      if 0<=tj+off<=Q:rows.append(v[:-1]+[tj+off])
    else:rows.append(v)
for _ in range(10000):
 p=rng.choice([rng.randrange(L//2),rng.getrandbits(64),L-rng.randrange(1000)])
 a=rng.choice([0,1,rng.getrandbits(32)]);d=rng.choice([0,1,60,300,rng.getrandbits(32)])
 c=rng.choice([rng.getrandbits(64),min(M,p+(a+1)*d),min(M,p+(a+1)*d+31)])
 rows.append([p,a,d,c,rng.choice([0,30,M]),rng.choice([0,6,25,A]),rng.choice([0,Q,rng.getrandbits(90)]),rng.getrandbits(64),rng.getrandbits(64),rng.getrandbits(64),rng.getrandbits(64),rng.getrandbits(128)])
data=''.join(' '.join(map(str,v))+'\n' for v in rows).encode()
proc=subprocess.run([sys.argv[1]],input=data,capture_output=True)
got=proc.stdout.decode().splitlines();bad=[]
for i,v in enumerate(rows):
 if i>=len(got) or got[i].split()!=expected(v):bad.append({'row':i,'input':v,'expected':expected(v),'got':got[i] if i<len(got) else None})
result=dict(ok=proc.returncode==0 and len(got)==len(rows) and not bad,seed=0x533320261017,cases=len(rows),validWindows=sum(d>0 and p+(a+1)*d<L for p,a,d,*_ in rows),mismatches=len(bad),firstMismatches=bad[:5],inputSha256=hashlib.sha256(data).hexdigest(),outputSha256=hashlib.sha256(proc.stdout).hexdigest(),rustExit=proc.returncode)
(out/'review-oracle.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result));sys.exit(0 if result['ok'] else 1)
