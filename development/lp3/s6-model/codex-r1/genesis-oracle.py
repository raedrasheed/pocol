import sys
"""Preserved independent specification oracle versus new TS and Rust implementations."""
import copy,hashlib,importlib.util,json,random,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[4];OUT=Path(sys.argv[1]).resolve()
path=ROOT/'development/m1/m1-draft-0.21/tools/netprofile_ref.py';spec=importlib.util.spec_from_file_location('independent_spec',path);ref=importlib.util.module_from_spec(spec);spec.loader.exec_module(ref)
base=json.loads((ROOT/'prototypes/lp1-node/fixtures/profile.json').read_text());original=ref.decode_genesis(bytes.fromhex(base['genesis_pre_hex']));rng=random.Random(0x53364753);rows=[];want=[]
for j in range(1200):
 s=copy.deepcopy(original);field=rng.randrange(52);name,width,lo,hi=ref.CP_FIELDS[field];high=(10000-s['CP']['alpha_bp']) if hi=='gamma' else hi if hi is not None else (1<<width)-1
 s['CP'][name]=rng.choice([lo,high,rng.randrange(lo,high+1)])
 if name=='alpha_bp':s['CP']['gamma_bp']=min(s['CP']['gamma_bp'],10000-s['CP']['alpha_bp'])
 s['chainId']=rng.choice([1,777001,777902,(1<<64)-1]);s['M_0List']=[[(n+1).to_bytes(20,'big'),(n+1).to_bytes(20,'big')] for n in range(rng.randrange(1,12))]
 if j%7==0:s['CP']['nonceMode']=2
 if j%11==0:s['M_0List'][0][0]=bytes(20)
 if j%13==0:s['M_0List'].reverse()
 b=ref.encode_genesis(s);rows.append(b)
 try:ref.decode_genesis(b);want.append(dict(encoded=b.hex(),hash=ref.keccak256(b).hex(),bootable=False))
 except ref.GsError as e:want.append(dict(error=e.code))
payload=''.join(json.dumps(dict(type='genesis',hex=b.hex()))+'\n' for b in rows).encode();p=subprocess.run(['node','--experimental-strip-types',str(ROOT/'prototypes/lp3-model/probe.ts')],input=payload,capture_output=True);assert p.returncode==0,p.stderr
actual=[json.loads(x) for x in p.stdout.splitlines()];assert len(actual)==len(want)
bad=[dict(index=i,want=w,got=a) for i,(w,a) in enumerate(zip(want,actual)) if w!=a]
inp=OUT/'genesis-input.hex';inp.write_text(''.join(b.hex()+'\n' for b in rows));rust=subprocess.run([str(ROOT/'prototypes/lp1-node/target/debug/lp1-node'),'genesis-decode','--in',str(inp)],capture_output=True)
rs=[json.loads(x) for x in rust.stdout.splitlines()];assert len(rs)==len(rows)+1
for i,(w,a) in enumerate(zip(want,rs)):
 assert (dict(error=a['code']) if not a['ok'] else dict(encoded=rows[i].hex() if a['reencodeEqual'] else 'MISMATCH',hash=a['genesisHash'][2:],bootable=a['bootable']))==w,(i,a,w)
result=dict(seed=0x53364753,cases=len(rows),tsMismatches=len(bad),firstMismatches=bad[:2],rustMatched=len(rows),valid=sum('error' not in w for w in want),invalid=sum('error'in w for w in want),inputSha256=hashlib.sha256(payload).hexdigest(),tsOutputSha256=hashlib.sha256(p.stdout).hexdigest(),rustOutputSha256=hashlib.sha256(rust.stdout).hexdigest(),rustExit=rust.returncode)
(OUT/'genesis-oracle.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result));assert not bad
