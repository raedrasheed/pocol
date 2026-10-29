"""Root prefix-sum oracle for delegated virtual-time deadline; expectations do not import author fixtures."""
from pathlib import Path
import importlib.util,json,random
root=Path(__file__).resolve().parents[2]
s=importlib.util.spec_from_file_location("overlay_root",root/"m1-draft-0.29/tools/overlay_ref_029.py");m=importlib.util.module_from_spec(s);s.loader.exec_module(m)
rng=random.Random(20291008);cases=[]
for kind in ("reply","sleep","compute"):
 for before in (0,1,8000,9998,9999):
  for delta in (0,1,2,1999,2000,9999,10000,10001):cases.append([(kind,before),(kind,delta)])
for _ in range(1000):cases.append([(rng.choice(("reply","sleep","compute")),rng.randrange(0,4001)) for i in range(rng.randrange(1,9))])
rows=[]
for i,ops in enumerate(cases):
 t0=rng.randrange(0,1000000);b=m.Budget(t0);prefix=0;expected={"kind":"ok","atMs":sum(v for _,v in ops)}
 for kind,v in ops:
  if prefix+v>=10000:expected={"kind":"DoesNotFit" if kind=="sleep" else "Expired","atMs":prefix if kind=="sleep" else 10000};break
  prefix+=v
 try:
  for kind,v in ops:
   if kind=="reply":b.reply(v,"root")
   elif kind=="sleep":b.sleep(v,"root")
   else:b.compute(v,"root")
  actual={"kind":"ok","atMs":b.elapsed()}
 except (m.Expired,m.DoesNotFit) as e:actual={"kind":type(e).__name__,"atMs":e.at-t0}
 rows.append({"id":i,"operations":ops,"expected":expected,"actual":actual,"pass":actual==expected})
out={"checks":len(rows),"passed":sum(x["pass"] for x in rows),"failed":sum(not x["pass"] for x in rows),"seed":20291008,"results":rows}
p=root/"coordination/review-001/deadline-independent-029.json";assert not p.exists();p.write_text(json.dumps(out,indent=2));print(json.dumps({k:out[k] for k in ("checks","passed","failed")}))
