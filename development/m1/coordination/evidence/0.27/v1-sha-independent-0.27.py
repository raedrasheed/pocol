from pathlib import Path
import importlib.util,json,hashlib,sys
root=Path(__file__).resolve().parents[2]
def load(p,n):
 s=importlib.util.spec_from_file_location(n,p);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);return m
m=load(root/"coordination/review-001/m1-draft-0.27/tools/run_checks_027.py","review27")
_,cfg,_=m.v1_network();v=m.V
tr=json.loads((root/"coordination/review-001/m1-draft-0.27/results/v1-transcripts-0.27.json").read_text())
original=hashlib.sha256
rows=[]
for t in tr["sha"]:
 for cache in (True,False):
  seen=[]
  def observed(data=b"",*args,**kwargs):
   seen.append(bytes(data));return original(data,*args,**kwargs)
  hashlib.sha256=observed
  try:
   ctr=v.Counters();rpc=v.ScriptedRpc(t["replies"]);out=v.check_window(cfg,rpc,1700000205,ctr,cache)
  finally:hashlib.sha256=original
  want=t["counters" if cache else "countersUncached"]
  rows.append({"case":t["case"],"cache":cache,"observedActualShaCalls":len(seen),"distinctInputs":len(set(seen)),"reportedShaCalls":want["sha256Total"],"outcome":out,"pass":len(seen)==want["sha256Total"] and len(set(seen))==want["sha256UniquePreimages"] and out==t["outcome"]})
# Fresh boundary retry sequences use real saved valid encoded headers; expectations are root-selected.
headers=next(t for t in tr["sha"] if t["case"]=="SHA-W")["replies"]
retry=[]
for reason,delay in [("busy",250),("busy",2000),("rate",0),("rate",2000)]:
 e=json.dumps({"jsonrpc":"2.0","id":2,"error":{"code":-32021,"message":reason,"data":{"reason":reason,"retryAfterMs":delay}}})
 for count in (1,3,4):
  ctr=v.Counters();rpc=v.ScriptedRpc([headers[0]]+[e]*count+[headers[1]]);out=v.check_window(cfg,rpc,1700000205,ctr)
  good=out.get("ok") is True if count<=3 else out.get("rule")=="viewIncomplete" and out.get("frame") is False
  retry.append({"reason":reason,"delay":delay,"replies":count,"times":[x["t"] for x in rpc.calls],"waitMs":rpc.t,"pass":good and rpc.t==min(count,3)*delay and len(rpc.calls)==(count+2 if count<=3 else 5)})
result={"shaCases":rows,"retryCases":retry,"checks":len(rows)+len(retry),"passed":sum(x["pass"] for x in rows+retry),"failed":sum(not x["pass"] for x in rows+retry),"rateCasesAreConditionalOnUnapprovedCap":True}
p=root/"coordination/review-001/v1-sha-independent-0.27.json";assert not p.exists();p.write_text(json.dumps(result,indent=2));print(json.dumps({k:result[k] for k in ("checks","passed","failed")}))
