import json,hashlib,shutil
from pathlib import Path
from Crypto.Hash import keccak
import rlp
root=Path(__file__).resolve().parent
(root/'vectors').mkdir(exist_ok=True)
k=lambda b:keccak.new(digest_bits=256,data=b).digest()
hx=lambda b:'0x'+b.hex()
factory=bytes.fromhex('0000000000000000000000000000000000c0c005')
html=b'<!doctype html><html><head><meta charset="utf-8"><title>PoCol</title></head><body><h1>PoCol</h1></body></html>\n'
vectors=[]
for name,data in [('single-byte',b'a'),('max-chunk',b'\xaa'*24575),('html',html)]:
 runtime=b'\0'+data;init=b'\x61'+len(runtime).to_bytes(2,'big')+bytes.fromhex('80600a5f395ff3')+runtime
 salt=k(data);addr=k(b'\xff'+factory+salt+k(init))[12:]
 v={'id':name,'factory':hx(factory),'data':hx(data),'dataLen':len(data),'salt':hx(salt),'initcode':hx(init),'initcodeHash':hx(k(init)),'runtime':hx(runtime),'runtimeLen':len(runtime),'address':hx(addr)}
 vectors.append(v)
(root/'vectors/chunks.json').write_text(json.dumps(vectors,indent=2)+'\n')
# Minimal big endian integers, RLP zero is empty bytes.
u=lambda n:n.to_bytes((n.bit_length()+7)//8,'big')
v=vectors[-1];entry=[b'/index.html',u(1),u(len(html)),k(html),[[bytes.fromhex(v['address'][2:]),u(len(html))]]]
manifest=rlp.encode([u(1),u(0),[entry]])
(root/'vectors/index.html').write_bytes(html)
(root/'vectors/manifest.json').write_text(json.dumps({'encodingDecision':'P02 pending review','entryIndex':0,'rlp':hx(manifest),'length':len(manifest),'hash':hx(k(manifest)),'fileHash':hx(k(html))},indent=2)+'\n')
# Official CREATE2 known-answer check; does not replace independent EVM execution.
assert hx(k(b'\xff'+bytes(20)+bytes(32)+k(b'\0'))[12:])=='0x4d1a2e2bb4f88f0250f26ffff098b0b30b26bf38'
assert rlp.decode(manifest)==[u(1),u(0),[entry]]
for v in vectors:
 init=bytes.fromhex(v['initcode'][2:]);rt=bytes.fromhex(v['runtime'][2:]);assert init[10:]==rt;assert int.from_bytes(init[1:3],'big')==len(rt)
neg=[]
def add(id,changed,reason):neg.append({'id':id,'manifest':hx(rlp.encode(changed)),'expected':'reject','reason':reason})
import copy
for id,field,value,why in [('unknown-mime',1,u(11),'unknown MIME'),('entry-css',1,u(2),'entry must be HTML'),('wrong-size',2,u(len(html)+1),'sum of chunk lengths differs from size'),('wrong-content-hash',3,bytes(32),'reconstructed content hash mismatch'),('dot-segment',0,b'/./index.html','dot segment'),('non-ascii',0,'/صفحة.html'.encode(),'non ASCII path')]:
 e=copy.deepcopy(entry);e[field]=value;add(id,[u(1),u(0),[e]],why)
add('duplicate-path',[u(1),u(0),[entry,entry]],'strict path ordering violated')
e=copy.deepcopy(entry);e[0]=b'/a.html';add('unsorted-path',[u(1),u(0),[entry,e]],'strict path ordering violated')
e=copy.deepcopy(entry);e[4][0][1]=u(0);add('zero-chunk-length',[u(1),u(0),[e]],'chunk length lower bound')
neg += [{'id':'trailing-byte','manifest':hx(manifest+b'\0'),'expected':'reject','reason':'trailing input'}, {'id':'noncanonical-rlp','manifest':'0x8101','expected':'reject','reason':'non-minimal scalar and not manifest shape'}]
(root/'vectors/negative-manifests.json').write_text(json.dumps(neg,indent=2)+'\n')

print("Generated 3 chunk vectors and 11 negative manifest fixtures; no EVM execution.")
