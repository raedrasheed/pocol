"""Preserved M1/LP3 format decoder permits reward addresses without lifecycle policy.
Format only: no ParamGate/GenesisBuilder/boot acceptance is claimed.
"""
import json,sys,pathlib,hashlib,subprocess
import rlp
m1=pathlib.Path(sys.argv[1]);binary=sys.argv[2];out=pathlib.Path(sys.argv[3])
def leaf(v):
 if isinstance(v,str):return bytes.fromhex(v)
 if 'repeat'in v:return bytes.fromhex(v['repeat'])*v['count']
 if 'concat'in v:return b''.join(map(leaf,v['concat']))
 raise AssertionError('unknown literal')
doc=json.loads((m1/'m1-draft-0.21/vectors/v3-gsv1.json').read_bytes());baseline=b''.join(map(leaf,doc['flatSegments']));assert len(baseline)==341
rows=[]
for name,reward in [('zero',bytes(20)),('not-proven-present',bytes([0x44])*20)]:
 tree=rlp.decode(baseline);tree[4][0][1]=reward;data=rlp.encode(tree)
 scratch=out.with_suffix('.input.hex');scratch.write_text(data.hex()+'\n')
 result=subprocess.run([binary,'genesis-decode','--in',str(scratch)],stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
 scratch.unlink()
 messages=[json.loads(line) for line in result.stdout.splitlines() if line];parsed=messages[0];assert result.returncode==0 and parsed['ok'] and parsed['bootable']==False
 rows.append(dict(name=name,inputSHA256=hashlib.sha256(data).hexdigest(),exit=result.returncode,formatResult=parsed,allMessages=messages))
out.write_text(json.dumps(dict(scope='FORMAT ONLY; allocation/account eligibility and boot NOT PASSED',cases=rows),indent=2)+'\n')
print('Verified 2 syntax-only witnesses; no GenesisBuilder/ParamGate/boot acceptance')
