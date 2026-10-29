"""Independent malformed GenesisSpec with bounded bytes and deep canonical RLP."""
import importlib.util,json,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
p=ROOT/'m1-draft-0.21/tools/netprofile_ref.py';s=importlib.util.spec_from_file_location('depth_v3',p);m=importlib.util.module_from_spec(s);s.loader.exec_module(m)
def wrap_list(b):
 n=len(b)
 if n<=55:return bytes([0xc0+n])+b
 size=n.to_bytes((n.bit_length()+7)//8,'big');return bytes([0xf7+len(size)])+size+b
b=b'\x80'
for _ in range(1500):b=wrap_list(b)
try:
 m.decode_genesis(b);actual={'accepted':True}
except m.GsError as e:actual={'error':e.code,'controlled':True}
except Exception as e:actual={'exception':type(e).__name__,'controlled':False}
r={'depth':1500,'bytes':len(b),'within47104':len(b)<=47104,'expected':'controlled gsStructure rejection (nested list in top[0])','actual':actual,'pass':actual.get('error')=='gsStructure'}
Path(__file__).with_suffix('.json').write_text(json.dumps(r,indent=2)+'\n',encoding='utf-8');print(json.dumps(r));sys.exit(0 if r['pass'] else 1)
