"""S4 review oracle: hashlib + unlimited Python integer arithmetic; no repository oracle imports."""
import copy,hashlib,json,random,subprocess,sys
from pathlib import Path
out=Path(sys.argv[1]).resolve();rng=random.Random(0x533420261017)
SPACE=1<<256;N=1<<64;M=N-1
def addr(i):return i.to_bytes(20,'big').hex()
def sha(b):return hashlib.sha256(b).digest()
def expected(v):
 h=v['height']; act=v['act'];seal=bytes.fromhex(v['seal'])
 if h==0 or v['parentHeight']!=h-1:return {'error':'ParentHeight'}
 if v['mMax']==0 or len(act)>v['mMax']:return {'error':'MemberBound'}
 seen=set()
 for id,tomb,last,grace in act:
  if id in seen:return {'error':'DuplicateMember'}
  seen.add(id);n=int(id,16)
  if n==0 or n==2**160-2 or 0xC0C001<=n<=0xC0C0FF:return {'error':'ReservedMember'}
 cutoff=h-v['wAct'];live=[id for id,tomb,_,_ in act if not tomb]
 active=[id for id,tomb,last,grace in act if not tomb and (last>=cutoff or grace>=cutoff)]
 ids=sorted(active or live);t=int(v['target'],16)
 if t==0 or v['c']==0:return {'error':'ZeroTargetOrC'}
 maximum=N if v['mode']==1 else min(N,v['c']*(SPACE//(t+1)))
 seed=sha(b'PoColNonce'+seal+h.to_bytes(8,'big')+v['attempt'].to_bytes(4,'big'))
 tickets=sorted((sha(seed+bytes.fromhex(id)),id) for id in ids)
 ranges=[[id,ticket.hex(),str(i*maximum//len(ids)),str((i+1)*maximum//len(ids))] for i,(ticket,id) in enumerate(tickets)]
 owners=[next((id for id,_,lo,hi in ranges if int(lo)<=q<int(hi)),None) for q in v['queries']]
 weights=[]
 for id in ids:
  weights.append([id,sum(n for j,owner,n in v['history'] if owner==id and h-1-v['ww']<=j<=h-2)])
 if weights and not any(w for _,w in weights):weights=[[id,1] for id in ids]
 total=sum(w for _,w in weights)
 def choose(index):
  if not 0<=index<total:return None
  cumulative=0
  for id,w in weights:
   cumulative+=w
   if cumulative>index:return id
 choices=[choose(int(s)) for s in v['indices']];digests=[sha(b'PoColProposer'+seal+h.to_bytes(8,'big')+bytes(4)+k.to_bytes(4,'big')) for k in [0,1,2**32-1]]
 draw=None
 if total:
  k=0
  while True:
   digest=sha(b'PoColProposer'+seal+h.to_bytes(8,'big')+bytes(4)+k.to_bytes(4,'big'));x=int.from_bytes(digest,'big')
   if x<(SPACE//total)*total:draw=[choose(x%total),k];break
   k+=1
 samples=[]
 for w,digest in v['samples']:
  w=int(w);x=int(digest,16);samples.append(str(x%w) if x<SPACE//w*w else None)
 return dict(members=ids,seed=seed.hex(),nonceMax=str(maximum),ranges=ranges,owners=owners,weights=[[id,str(w)] for id,w in weights] if weights else None,choices=choices,draw=draw,samples=samples,digests=[d.hex() for d in digests])
rows=[]
for i in range(640):
 h=rng.choice([1,2,3,10,1000,M]);n=rng.randrange(13);wact=rng.choice([0,1,3,2**32-1]);ww=rng.choice([0,1,3,65535])
 act=[]
 for j in range(n):
  last=rng.choice([0,h-1,max(0,h-wact),max(0,h-wact-1)])
  grace=rng.choice([0,h-1,max(0,h-wact)])
  act.append([addr(j+1),rng.random()<.2,last,grace])
 rng.shuffle(act)
 hist=[]
 for j in sorted(set([0,max(0,h-2),max(0,h-1),max(0,h-1-ww),max(0,h-2-ww)])):
  for member in [addr(k+1) for k in range(n)]+[addr(1000)]:hist.append([j,member,rng.choice([0,1,3,257,M])])
 target=rng.choice([1,SPACE-1,1<<240,(1<<192)-1,1<<192,(1<<255)-1,rng.getrandbits(256) or 1])
 v=dict(seal=rng.randbytes(32).hex(),height=h,parentHeight=h-1,attempt=rng.choice([0,1,2**32-1,rng.getrandbits(32)]),target=f'{target:064x}',mode=rng.randrange(2),c=rng.choice([1,2,255]),wAct=wact,mMax=65535,act=act,ww=ww,history=hist,queries=[],indices=[],samples=[])
 # Independent expected ranges determine a boundary-focused query set; actual outputs are compared in full.
 e=expected(v);queries={0,M}
 if int(e['nonceMax'])<N:queries.add(int(e['nonceMax']))
 for _,_,lo,hi in e['ranges']:
  lo,hi=int(lo),int(hi)
  if lo<hi:queries|={lo,hi-1}
 v['queries']=sorted(queries)
 cumulative=0;indices={0}
 for _,w in e['weights'] or []:
  cumulative+=int(w);indices|={max(0,cumulative-1),cumulative}
 v['indices']=list(map(str,sorted(indices)))
 for w in rng.sample([1,2,3,5,257,2**64+1,2**127+1,2**128-1],3):
  limit=SPACE//w*w
  for x in sorted(set([0,min(SPACE-1,limit),limit-1,SPACE-1,rng.getrandbits(256)])):
   v['samples'].append([str(w),f'{x:064x}'])
 rows.append(v)
 # Same state in a different enumeration order must produce the identical mapping and choice.
 if i%16==0:
  r=copy.deepcopy(v);r['act'].reverse();r['history'].reverse();rows.append(r)
# Negative input-shape checks: no fabricated parent-state validity is implied by successful parsing.
base=copy.deepcopy(rows[0]);base['act']=[[addr(1),False,0,0]];base['mMax']=10
for change in [{'height':0},{'parentHeight':M},{'mMax':0},{'act':base['act']*2},{'act':[[addr(0),False,0,0]]},{'target':'00'*32},{'c':0}]:
 v=copy.deepcopy(base);v.update(change);rows.append(v)
# Maximum ActList and one over the specified u16 member bound.
big=dict(seal='11'*32,height=100,parentHeight=99,attempt=2**32-1,target='ff'*32,mode=0,c=1,wAct=1,mMax=65535,act=[[addr(i),False,99,0] for i in range(1,65536)],ww=65535,history=[],queries=[0,1,M],indices=['0','65534','65535'],samples=[['3','ff'*32],['3',f'{SPACE-2:064x}']])
rows.append(big)
too_many=copy.deepcopy(big);too_many['act'].append([addr(65536),False,99,0]);rows.append(too_many)
for n in [0xC0C000,0xC0C001,0xC0C0FF,0xC0C100,2**160-3,2**160-2,2**160-1]:
 v=copy.deepcopy(base);v['act']=[[addr(n),False,0,0]];rows.append(v)
v=copy.deepcopy(base);v['act']=[[addr(1),False,0,0],[addr(2),False,0,0]];v['mMax']=1;rows.append(v)
data=''.join(json.dumps(v,separators=(',',':'))+'\n' for v in rows).encode()
proc=subprocess.run(['node','--experimental-strip-types',str(Path(sys.argv[2])/'probe.ts')],input=data,capture_output=True)
lines=proc.stdout.decode().splitlines();bad=[]
for i,v in enumerate(rows):
 want=expected(v);got=json.loads(lines[i]) if i<len(lines) else None
 if got!=want:bad.append(dict(case=i,input=v,expected=want,got=got))
result=dict(ok=proc.returncode==0 and len(lines)==len(rows) and not bad,seed=0x533420261017,cases=len(rows),emptyMembershipCases=sum(not expected(v).get('members',[1]) for v in rows),samplingCases=sum(len(v['samples']) for v in rows if 'error' not in expected(v)),mismatches=len(bad),firstMismatches=bad[:2],inputSha256=hashlib.sha256(data).hexdigest(),outputSha256=hashlib.sha256(proc.stdout).hexdigest(),rustExit=proc.returncode,stderr=proc.stderr.decode()[:500])
(out/'review-oracle.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result));sys.exit(0 if result['ok'] else 1)
