import sys
"""Independent native integer distribution/route/conservation checks."""
import hashlib,json,random,subprocess
from pathlib import Path
rng=random.Random(0x53375356);rows=[];want=[]
for case in range(4000):
 pot=rng.choice([0,1,(1<<256)-1,rng.getrandbits(256)]);alpha=rng.randrange(10001);gamma=rng.randrange(10001-alpha);kappa=rng.choice([1,2,255]);m=rng.choice([1,32,65535]);counts=[];left=257
 for n in range(rng.randrange(20)):
  if not left:break
  count=rng.randrange(1,min(left,40)+1);left-=count;counts.append([f'{n+1:040x}',count,rng.choice([True,False]),rng.choice([True,False])])
 winner=pot*alpha//10000;proposer=pot*gamma//10000;R=pot-winner-proposer;C=sum(c for _,c,_,_ in counts);Q=max(C,kappa*m);A=R*C//Q;paid=penalty=pay=0;payments=[]
 for id,c,tomb,exists in counts:
  amount=R*c//Q;route='penaltyBurn' if tomb else 'payBurn' if not exists else 'paid';payments.append(dict(id=id,amount=str(amount),route=route))
  if tomb:penalty+=amount
  elif not exists:pay+=amount
  else:paid+=amount
 base=R-A;rounding=A-paid-penalty-pay;assert winner+proposer+paid+penalty+pay+base+rounding==pot
 rows.append(dict(pot=str(pot),alpha=alpha,gamma=gamma,kappa=kappa,m=m,counts=counts));want.append(dict(winner=str(winner),proposer=str(proposer),remainder=str(R),amount=str(A),baseBurn=str(base),roundBurn=str(rounding),paid=str(paid),penaltyBurn=str(penalty),payBurn=str(pay),payments=payments))
payload=''.join(json.dumps(r,separators=(',',':'))+'\n' for r in rows).encode();results=[]
for args in [[str(Path(sys.argv[1]).resolve()/'rewards-probe')],['node','--experimental-strip-types',str(Path(__file__).resolve().parent/'rewards_probe.mjs')]]:
 p=subprocess.run(args,input=payload,capture_output=True);assert p.returncode==0,p.stderr;actual=[json.loads(x) for x in p.stdout.splitlines()];assert len(actual)==len(want)
 bad=[(i,a,w) for i,(a,w) in enumerate(zip(actual,want)) if a!=w];assert not bad,bad[:1];results.append(dict(command=args,matched=len(actual),outputSha256=hashlib.sha256(p.stdout).hexdigest()))
result=dict(cases=len(rows),seed=0x53375356,mismatches=0,inputSha256=hashlib.sha256(payload).hexdigest(),implementations=results);(Path(sys.argv[1]).resolve()/'rewards-oracle.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
