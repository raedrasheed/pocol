"""Independent C30 structural-depth and framing probes against the actual repair."""
import importlib.util,json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
def load(n,p):
 s=importlib.util.spec_from_file_location(n,p);m=importlib.util.module_from_spec(s);s.loader.exec_module(m);return m
np=load('np_c30_root',ROOT/'m1-draft-0.21/tools/netprofile_ref.py')
ip=load('ip_c30_root',ROOT/'m1-draft-0.22/tools/iterative_parse.py')
raw=bytes.fromhex(next(r['hex'] for r in json.loads((ROOT/'coordination/review-001/e05-hash-inputs.json').read_text())['inputs'] if r['id']=='GSV1'))
spec=np.decode_genesis(raw);np.parse=ip.make_parse(np.GsError)
R=np.RLP
def wrap(b):
 n=len(b)
 if n<=55:return bytes([0xc0+n])+b
 v=n.to_bytes((n.bit_length()+7)//8,'big');return bytes([0xf7+len(v)])+v+b
cp=[R.encode(R.uint(spec['CP'][n])) for n,*_ in np.CP_FIELDS]
members=[R.encode([a,b]) for a,b in spec['M_0List']]
parts=[R.encode(R.uint(1)),R.encode(R.uint(spec['chainId'])),wrap(b''.join(cp)),R.encode(spec['allocRoot']),wrap(b''.join(members)),R.encode(spec['sysCodeHash'])]
assert wrap(b''.join(parts))==raw
rows=[]
def ck(name,b,want):
 try:actual='ok' if np.decode_genesis(b) else 'ok'
 except np.GsError as e:actual=e.code
 except Exception as e:actual='exception:'+type(e).__name__
 rows.append({'check':name,'bytes':len(b),'expected':want,'actual':actual,'pass':actual==want})
for depth in [1,10,1500,5000]:
 deep=b'\x80'
 for _ in range(depth):deep=wrap(deep)
 ck('deep-root-'+str(depth),deep,'gsVersion' if depth==1 else 'gsStructure')
 v=list(parts);v[0]=deep;ck('deep-top-field-'+str(depth),wrap(b''.join(v)),'gsStructure')
 v=list(parts);v[2]=wrap(deep+b''.join(cp[1:]));ck('deep-cp-leaf-'+str(depth),wrap(b''.join(v)),'gsStructure')
 v=list(parts);v[4]=wrap(wrap(deep+R.encode(spec['M_0List'][0][1]))+b''.join(members[1:]));ck('deep-member-id-'+str(depth),wrap(b''.join(v)),'gsStructure')
 ck('deep-truncated-'+str(depth),deep[:-1],'L0')
ck('wrapped-version-integer',wrap(b'\x81\x01'+b''.join(parts[1:])),'gsInt')
ck('valid-gsv-after-rejections',raw,'ok')
rows.append({'check':'valid-round-trip','expected':raw.hex(),'actual':np.encode_genesis(np.decode_genesis(raw)).hex(),'pass':np.encode_genesis(np.decode_genesis(raw))==raw})
r={'checks':len(rows),'passed':sum(x['pass'] for x in rows),'failed':sum(not x['pass'] for x in rows),'results':rows}
Path(__file__).with_suffix('.json').write_text(json.dumps(r,indent=2)+'\n',encoding='utf-8');print(json.dumps({k:v for k,v in r.items() if k!='results'}))
for x in rows:
 if not x['pass']:print(json.dumps(x))
assert not r['failed']
