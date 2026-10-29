"""Root independently selected malformed reply corpus; does not import author test expectations."""
from pathlib import Path
import importlib.util,json,sys
root=Path(__file__).resolve().parents[2]
rev=sys.argv[1]
spec=importlib.util.spec_from_file_location("vbusy",root/("m1-draft-"+rev)/"tools/v1_ref_027.py")
v=importlib.util.module_from_spec(spec);spec.loader.exec_module(v)
cases={"missingData":{"error":{"code":-32021}},"nullData":{"error":{"code":-32021,"data":None}},"emptyData":{"error":{"code":-32021,"data":{}}},"nullError":{"error":None},"boolError":{"error":True},"stringError":{"error":"busy"},"listError":{"error":[]},"nonObjectReply":None,"listReply":[],"bothResultError":{"result":"0xc0","error":{"code":-32021,"data":{"reason":"busy","retryAfterMs":250}}}}
for name,val in [("bool",True),("false",False),("string","250"),("negative",-1),("fraction",250.5),("huge",2**100),("zero",0),("belowBusy",249),("aboveBusy",2001)]:
 cases["busyDelay_"+name]={"error":{"code":-32021,"data":{"reason":"busy","retryAfterMs":val}}}
for name,val in [("bool",True),("str","-32021"),("floatFraction",-32021.5),("missing",None)]:
 cases["code_"+name]={"error":{"code":val,"data":{"reason":"busy","retryAfterMs":250}}}
for name,val in [("missing",None),("unknown","other"),("bool",True)]:
 cases["reason_"+name]={"error":{"code":-32021,"data":{"reason":val,"retryAfterMs":250}}}
rows=[]
for name,reply in cases.items():
 if isinstance(reply,dict):
  reply={"jsonrpc":"2.0","id":2,**reply}
  if isinstance(reply.get("error"),dict):reply["error"]={"message":"busy",**reply["error"]}
 text=json.dumps(reply,separators=(",",":"))
 rpc=v.ScriptedRpc(['{"jsonrpc":"2.0","id":1,"result":"0x1"}',text]);ctr=v.Counters()
 try:
  out=v.check_window({},rpc,0,ctr)
  ok=out.get("rule")=="viewIncomplete" and out.get("frame") is False and rpc.t==0 and len(rpc.calls)==2
  rows.append({"case":name,"input":reply,"outcome":out,"elapsedMs":rpc.t,"calls":len(rpc.calls),"pass":ok})
 except Exception as e:rows.append({"case":name,"input":reply,"exception":type(e).__name__,"message":str(e),"pass":False})
data={"checks":len(rows),"passed":sum(x["pass"] for x in rows),"failed":sum(not x["pass"] for x in rows),"results":rows}
out=root/"coordination/review-001"/("busy-independent-probes-"+rev+".json")
assert not out.exists();out.write_text(json.dumps(data,indent=2));print(json.dumps({k:data[k] for k in ("checks","passed","failed")}))
