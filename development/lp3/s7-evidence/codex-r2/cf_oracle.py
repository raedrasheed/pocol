"""Independent native integer CF oracle. No imports of either implementation."""
import hashlib,json,random,subprocess,sys
from pathlib import Path
HERE=Path(__file__).resolve().parent;OUT=Path(sys.argv[1]).resolve();OUT.mkdir(parents=True,exist_ok=True)
r=random.Random(0x43463136);U=(1<<64)-1
rows=[];expected=[]
for i in range(16000):
 def num():return r.choice([0,1,2,39,40,41,42,99,100,128,129,553,680,697,698,700,999,1000,9979,9980,10000,U-1,U,r.getrandbits(64)])
 a=[num() for _ in range(12)]
 for j in [2,7,8,11]:a[j]=r.choice([0,1,2,8,10,128,(1<<32)-1,r.getrandbits(32)])
 closing,done,final=[bool(r.getrandbits(1)) for _ in range(3)];first=None if i%2 else num()
 c,n,ret,h,ts,he,te,p2,tb,hc,tc,d=a
 rounds=(n+ret-1)//ret if ret else None
 cursor={'error':'ret'} if not ret else ({'error':'overflow'} if c+max(0,rounds-1)>U else str(c+max(0,rounds-1)))
 gate={'error':'threshold'} if p2>he or p2*tb>te else h>=he-p2 or ts>=te-p2*tb
 expected.append(dict(closing=str(first) if first is not None else(str(h) if h>=hc or ts>=tc else None),cursor=cursor,gate=gate,final=closing and done and h>=c+d and(h>=he or ts>=te),child=not final and h<=he))
 rows.append(dict(a=list(map(str,a)),closing=closing,done=done,final=final,first=None if first is None else str(first)))
data=''.join(json.dumps(v,separators=(',',':'))+'\n' for v in rows)
for name,args in [('rust',[str(OUT/'cf-probe')]),('ts',['node','--experimental-strip-types',str(HERE/'cf_probe.mjs')])]:
 p=subprocess.run(args,input=data,text=True,capture_output=True);(OUT/(name+'-oracle-stderr.log')).write_text(p.stderr);assert p.returncode==0,p.stderr
 actual=[json.loads(s) for s in p.stdout.splitlines()];assert len(actual)==len(expected)
 for i,(x,y) in enumerate(zip(actual,expected)):assert x==y,(name,i,rows[i],x,y)
 (OUT/(name+'-output.sha256')).write_text(hashlib.sha256(p.stdout.encode()).hexdigest()+'\n')
result=dict(cases=len(rows),seed='0x43463136',rustMismatches=0,tsMismatches=0,inputSha256=hashlib.sha256(data.encode()).hexdigest(),oracleSha256=hashlib.sha256(json.dumps(expected,sort_keys=True).encode()).hexdigest(),scope='newly approved pure core definitions; no execution certificate')
(OUT/'oracle.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
