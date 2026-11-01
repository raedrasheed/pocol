"""P2 typed execution views derived from actual public synthetic signed bytes.
P2 native engine does not authenticate these views; P3 separately verifies wire.
Keep the complete independent fixture, including all76 cases and two follow-ups.
"""
import sys,json,hashlib,re
from pathlib import Path
from eth.vm.forks.cancun import CancunVM
B=CancunVM.get_transaction_builder()
def unpack(v):
 if isinstance(v,list):return [unpack(x) for x in v]
 if isinstance(v,dict):
  if set(v)=={'hexRuns'}:return ''.join('00'*x if isinstance(x,int) else x for x in v['hexRuns'])
  return {k:unpack(x) for k,x in v.items()}
 return v
def pack(v):
 if isinstance(v,list):return [pack(x) for x in v]
 if isinstance(v,dict):return {k:pack(x) for k,x in v.items()}
 if isinstance(v,str) and len(v)%2==0 and re.fullmatch('[0-9a-f]+',v):
  matches=[m for m in re.finditer(r'(?:00){32,}',v) if m.start()%2==0]
  if matches:
   runs=[];start=0
   for m in matches:runs.extend([v[start:m.start()],len(m[0])//2]);start=m.end()
   runs.append(v[start:]);return {'hexRuns':runs}
 return v
def view(row):
 raw=bytes.fromhex(row['raw']);assert hashlib.sha256(raw).hexdigest()==row['rawSHA256'];tx=B.decode(raw);tx.check_signature_validity();kind=getattr(tx,'type_id',None)
 return dict(caller=tx.sender.hex(),to=tx.to.hex(),input=tx.data.hex(),nonce=tx.nonce,gas=tx.gas,gasPrice=str(tx.max_fee_per_gas if kind==2 else tx.gas_price),priority=str(tx.max_priority_fee_per_gas) if kind==2 else None,chain=tx.chain_id,value=str(tx.value))
source=Path(sys.argv[1]);data=unpack(json.loads(source.read_text()));assert len(data['cases'])==76
for row in data['cases']:
 row['nativeView']=view(row)
 if 'followUp' in row:row['followUp']['nativeView']=view(row['followUp'])
data['nativeViewsScope']=__doc__;data['signedFixtureSHA256']=hashlib.sha256(source.read_bytes()).hexdigest();packed=pack(data);assert unpack(packed)==data
Path(sys.argv[2]).write_text(json.dumps(packed,indent=2)+'\n');print(json.dumps(dict(cases=76,followUps=2,wireAcceptance=False,bootable=False)))
