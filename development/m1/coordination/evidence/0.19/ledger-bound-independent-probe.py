"""Independent RLP length arithmetic for the C12/C14 boundary (no old runner import)."""
import json
from pathlib import Path
def be(n):return n.to_bytes((n.bit_length()+7)//8,'big')
def enc(v):
 if isinstance(v,list):
  b=b''.join(enc(x) for x in v);k=0xc0;long=0xf7
 else:
  b=v
  if len(b)==1 and b[0]<128:return b
  k=0x80;long=0xb7
 if len(b)<=55:return bytes([k+len(b)])+b
 n=be(len(b));return bytes([long+len(n)])+n+b
rows=[]
for n in [2847,2848]:
 refs=[[be(i).rjust(20,b'\0'),b'\2'] for i in range(1,n+1)]
 f=[b'/a',b'\1',be(2*n),bytes(32),refs]
 stages=[enc(refs),enc(f),enc([f]),enc([b'\1',b'',[f]])]
 rows.append({'references':n,'levelBytes':[len(v) for v in stages],'manifestBytes':len(stages[-1]),'within65536':len(stages[-1])<=65536})
r={'lowerBoundReferences':(65536-54)//23,'viewerMaxRequests':3+(65536-54)//23,'rows':rows,'passed':rows[0]['manifestBytes']==65535 and rows[1]['manifestBytes']==65561}
Path(__file__).with_suffix('.json').write_text(json.dumps(r,indent=2)+'\n',encoding='utf-8');print(json.dumps(r));assert r['passed']
