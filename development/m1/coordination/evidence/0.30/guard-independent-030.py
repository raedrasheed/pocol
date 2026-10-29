from pathlib import Path
import importlib.util,json
root=Path(__file__).resolve().parents[2]
def load(p,n):
 s=importlib.util.spec_from_file_location(n,p);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);return m
G=load(root/"m1-draft-0.30/tools/guard_ref_030.py","gind30");O=load(root/"m1-draft-0.29/tools/overlay_ref_029.py","oind30");B=load(root/"m1-draft-0.4/tools/bridge_ref.py","bind30")
R=load(root/"coordination/review-001/m1-draft-0.27/tools/run_checks_027.py","r27ind30");_,cfg,_=R.v1_network()
tr=json.loads((root/"coordination/review-001/m1-draft-0.27/results/v1-transcripts-0.27.json").read_text(encoding="utf8"));pair=next(x for x in tr["sha"] if x["case"]=="SHA-S1")["replies"]
corpus=json.loads((root/"coordination/review-001/guard-corpus-independent-030.json").read_text(encoding="utf8"));rows=[]
for c in corpus["cases"]:
 method=c.get("method","pocol_getHeaders");rid=1 if method=="eth_blockNumber" else 2
 if "mutation" in c:
  d=json.loads(pair[1]);d.pop("id",None) if c["id"]=="missingId" else None
  if c["id"]=="wrongId":d["id"]=9999
  text=json.dumps(d,separators=(",",":"))
  if c["id"]=="duplicateResult":text=pair[1].replace('"result":','"result":"0xc0","result":',1)
 elif "padSavedValidHeaderToBytes" in c:text=pair[1]+" "*(c["padSavedValidHeaderToBytes"]-len(pair[1].encode("utf8")))
 else:text=c.get("literal","")
 if "padLiteralToBytes" in c:text+=" "*(c["padLiteralToBytes"]-len(text.encode("utf8")))
 raw=bytes.fromhex(c["bytesHex"]) if "bytesHex" in c else text.encode("utf8")
 try:
  clean,info=G.receive_guard(B,raw,method,rid);kind=R.V.classify_reply(clean)[0];passed=c["expected"]!="reject" and (c["expected"]!="retry" or kind=="retry");actual={"accepted":True,"semanticKind":kind,"info":info}
 except G.GuardFail as e:passed=c["expected"]=="reject";actual={"accepted":False,"reason":e.reason,"stage":e.stage}
 rows.append({"case":c["id"],"expected":c["expected"],"actual":actual,"pass":passed})
# Actual wrapper replays for original three C33 cases, independent of author's expectations.
integ=[]
for case in ("wrongId","missingId","duplicateResult"):
 d=json.loads(pair[1]);
 if case=="wrongId":d["id"]=9999
 if case=="missingId":d.pop("id")
 text=pair[1].replace('"result":','"result":"0xc0","result":',1) if case=="duplicateResult" else json.dumps(d,separators=(",",":"))
 res=G.guarded_header_net_check(R.V,O,B,cfg,[(0,pair[0]),(0,text)],1700000205,0)
 integ.append({"case":case,"frame":res["verdict"].get("frame"),"rule":res["verdict"].get("rule"),"wait":res["sleptMs"],"guard":res["guard"],"pass":res["verdict"].get("frame") is False and res["sleptMs"]==0 and res["counters"].decodes==0})
# Unicode error-data encoding cross-check is a separate unresolved issue.
oracle=json.loads((root/"coordination/review-001/error-data-native-oracle-030.json").read_text(encoding="utf8"));x=oracle["rows"][0]
err=json.dumps({"jsonrpc":"2.0","id":2,"error":{"code":-32021,"message":"busy","data":x["data"]}},separators=(",",":"))
res=G.guarded_header_net_check(R.V,O,B,cfg,[(0,pair[0]),(0,err),(0,pair[1])],1700000205,0)
data={"checks":len(rows)+len(integ),"passed":sum(x["pass"] for x in rows+integ),"failed":sum(not x["pass"] for x in rows+integ),"receiveBoundary":rows,"originalC33":integ,"unicodeDataProbe":{"nativeDataBytes":x["expectedJsonUtf8Bytes"],"modelBytes":G.compact_bytes(x["data"]),"frame":res["verdict"].get("frame"),"waitMs":res["sleptMs"],"dataDropped":any(i["dataDropped"] for i in res["guardInfo"]),"expected":"data dropped; busy becomes malformed; no wait/frame","pass":res["verdict"].get("frame") is False and res["sleptMs"]==0}}
p=root/"coordination/review-001/guard-independent-030.json";assert not p.exists();p.write_text(json.dumps(data,indent=2),encoding="utf8");print(json.dumps({k:data[k] for k in ("checks","passed","failed","unicodeDataProbe")}))
