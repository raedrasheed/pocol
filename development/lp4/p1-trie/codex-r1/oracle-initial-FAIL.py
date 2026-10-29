"""Independent insertion-based hexary trie, then unary path compression.
Rust groups sorted nibble slices. This oracle instead mutates an uncompressed tree.
Keccak comes from frozen M1; RLP below is independent of Rust and M1 encoders.
"""
import hashlib,importlib.util,json,random,subprocess,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[4];OUT=Path(sys.argv[1]).resolve();OUT.mkdir(parents=True,exist_ok=True)
spec=importlib.util.spec_from_file_location('k',ROOT/'development/m1/m1-draft-0.2/tools/keccak.py');K=importlib.util.module_from_spec(spec);spec.loader.exec_module(K);keccak=K.keccak256
# Optional independently installed primitive, checked against all frozen K1-K3 vectors.
try:
 from Crypto.Hash import keccak as fast
 def keccak(b):q=fast.new(digest_bits=256);q.update(b);return q.digest()
except ImportError:pass
for x,y in json.loads((ROOT/'development/m1/m1-draft-0.21/vectors/v3-gsv1.json').read_text())['keccakVectors'].values():assert keccak(bytes.fromhex(x)).hex()==y

def bint(n):return n.to_bytes((n.bit_length()+7)//8,'big')
def enc(v):
 if isinstance(v,int):v=bint(v)
 if isinstance(v,list):p=b''.join(map(enc,v));base=192
 else:
  if len(v)==1 and v[0]<128:return v
  p=v;base=128
 return bytes([base+len(p)])+p if len(p)<56 else bytes([base+55+len(bint(len(p)))])+bint(len(p))+p

def root(entries):
 t={}
 for key,value in entries:
  n=t
  for byte in key:
   for digit in (byte//16,byte%16):n=n.setdefault(digit,{})
  n['v']=value
 def compact(path,leaf):
  head=[2*int(leaf)+len(path)%2]+([] if len(path)%2 else [0]);digits=head+path
  return bytes(a*16+b for a,b in zip(digits[::2],digits[1::2]))
 def ref(v):
  encoded=enc(v);return v if len(encoded)<32 else keccak(encoded)
 def compress(n):
  if 'v'in n and len(n)==1:return [compact([],True),n['v']]
  path=[]
  while 'v' not in n and len(n)==1:
   k=next(iter(n));path.append(k);n=n[k]
  if path:
   if 'v'in n and len(n)==1:return[compact(path,True),n['v']]
   return[compact(path,False),ref(compress(n))]
  return [ref(compress(n[i])) if i in n else b'' for i in range(16)]+[n.get('v',b'')]
 return keccak(enc(compress(t))) if entries else keccak(enc(b''))

rng=random.Random(0x4c50345031);rows=[];want=[]
def add(kind,items,expected=None,budget=None):
 budget=len(items) if budget is None else budget;rows.append(dict(type=kind,entries=items,budget=budget))
 if expected:want.append(dict(error=expected));return
 if kind=='raw':pairs=[(bytes.fromhex(k),bytes.fromhex(v)) for k,v in items]
 elif kind=='ordered':pairs=[(enc(i),bytes.fromhex(v)) for i,v in enumerate(items)]
 elif kind=='storage':pairs=[(keccak(int(k).to_bytes(32,'big')),enc(int(v))) for k,v in items if int(v)]
 else:pairs=[(keccak(bytes.fromhex(a)),enc([int(n),int(b),bytes.fromhex(s),bytes.fromhex(c)]))for a,n,b,s,c in items]
 want.append(dict(root=root(pairs).hex()))
add('raw',[])
# Short references span exactly 31/32 encoded bytes; prefixes/empty key and full64B key.
for n in range(1,65):add('raw',[['10','01'*n],['11','02']])
for items in [[['','01'],['12','02'],['1234','03'],['1235','04'*32]],[['ff'*64,'01'],['ff'*63+'fe','02']]]:add('raw',items)
for _ in range(450):
 count=rng.randrange(1,35);d={rng.randbytes(rng.randrange(0,20)).hex():rng.randbytes(rng.randrange(1,90)).hex()for _ in range(count)};items=list(d.items());add('raw',items);rng.shuffle(items);add('raw',items)
for _ in range(120):
 keys=rng.sample(range(10000),rng.randrange(1,25));add('storage',[[str(k),str(rng.choice([0,1,127,128,(1<<256)-1,rng.getrandbits(256)]))]for k in keys]);add('ordered',[rng.randbytes(rng.randrange(1,80)).hex()for _ in range(rng.randrange(0,80))])
empty=root([]).hex();code=keccak(b'').hex()
for count in [0,1,4,32,1024]:add('state',[[i.to_bytes(20,'big').hex(),str(i),str(i*17),empty,code]for i in range(count)])
add('raw',[['00','01'],['00','02']],'DuplicateKey');add('raw',[['00','']],'EmptyValue');add('raw',[['00'*65,'01']],'KeyTooLong');add('raw',[['00','01']],'EntryBudget',0);add('storage',[['1','0'],['1','0']],'DuplicateKey');add('ordered',['01'],'EntryBudget',0)
data=''.join(json.dumps(v,separators=(',',':'))+'\n' for v in rows);p=subprocess.run([str(OUT/'probe')],input=data,text=True,capture_output=True);assert p.returncode==0,p.stderr
actual=[json.loads(s)for s in p.stdout.splitlines()];assert len(actual)==len(want)
for i,(a,b)in enumerate(zip(actual,want)):assert a==b,(i,rows[i],a,b)
result=dict(cases=len(rows),seed='0x4c50345031',mismatches=0,includes1024Accounts=True,inputSha256=hashlib.sha256(data.encode()).hexdigest(),outputSha256=hashlib.sha256(p.stdout.encode()).hexdigest(),independentAlgorithm='insert uncompressed nibble tree, then compress',resourceProof=False,GenesisBuilderPassed=False)
(OUT/'oracle.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
