"""Independent compiled SYS nested/EIP150 measurements, not full acceptance."""
import json,sys,hashlib,rlp
from pathlib import Path
from eth.db.atomic import AtomicDB
from eth.vm.chain_context import ChainContext
from eth.vm.forks.cancun import CancunVM
from eth.vm.forks.cancun.blocks import CancunBlockHeader
from eth.vm.message import Message
from eth.vm.transaction_context import BaseTransactionContext
from eth.vm.forks.spurious_dragon._utils import collect_touched_accounts
from eth.vm.spoof import SpoofTransaction
from eth_utils import keccak
from gst_control import root
SYSTEM=bytes([255])*19+b'\xfe';ESCROW=(0xc0c002).to_bytes(20,'big');TARGET=bytes([0x66])*20
base=Path(__file__).resolve().parent
artifact=base/'data/system-boundary-witness.json';epoch=base/'data/system-epoch-witness.json'
code=bytes.fromhex(json.loads(artifact.read_text())['output']['contracts']['SystemBoundaryWitness.sol']['SystemBoundaryWitness']['evm']['deployedBytecode']['object'])
epoch_code=bytes.fromhex(json.loads(epoch.read_text())['output']['contracts']['SystemEpochWitness.sol']['SystemEpochWitness']['evm']['deployedBytecode']['object'])
pre={'0x'+bytes([0x22]*20).hex():dict(nonce=1,balance=0,code='0x'+epoch_code.hex(),storage={'0x0':'0xb'}),'0x'+TARGET.hex():dict(nonce=1,balance=0,code='0x'+code.hex(),storage={})}
header=CancunBlockHeader(difficulty=0,block_number=1,gas_limit=8_000_000,timestamp=0,base_fee_per_gas=100,coinbase=ESCROW,excess_blob_gas=0)
def raw(v):return bytes.fromhex(v.removeprefix('0x'))
def num(v):return int(v,16) if isinstance(v,str) and v.startswith('0x') else int(v)
def state(pre,header):
 s=CancunVM.build_state(AtomicDB(),header,ChainContext(777902))
 for address,a in pre.items():
  address=raw(address);s.set_nonce(address,num(a['nonce']));s.set_balance(address,num(a['balance']));s.set_code(address,raw(a['code']))
  for k,v in a['storage'].items():s.set_storage(address,num(k),num(v))
 s.persist();s.lock_changes();return s

def run(gas,required=100000,abort_child=False,abort_outer=False):
 s=state(pre,header)
 for a in [SYSTEM,TARGET,ESCROW]+[i.to_bytes(20,'big') for i in range(1,11)]:s.mark_address_warm(a)
 data=keccak(b'forward(uint256,uint256,bool,bool)')[:4]+b''.join(int(v).to_bytes(32,'big') for v in [30000000,required,abort_child,abort_outer])
 c=s.computation_class.apply_message(s,Message(gas=gas,to=TARGET,sender=SYSTEM,value=0,data=data,code=code),BaseTransactionContext(gas_price=0,origin=bytes(20)))
 for a in c.get_accounts_for_deletion():s.delete_account(a)
 for a in collect_touched_accounts(c):
  if s.account_exists(a) and s.account_is_empty(a):s.delete_account(a)
 return dict(gas=gas,required=required,abortChild=abort_child,abortOuter=abort_outer,frameSuccess=not c.is_error,childSuccess=not c.is_error and bool(int.from_bytes(c.output[:32],'big')),rawGas=c.get_gas_used(),refundCounter=c.get_gas_refund(),output='0x'+c.output.hex(),stateRoot='0x'+s.make_state_root().hex(),slot0=s.get_storage(TARGET,0),error=None if not c.is_error else type(c.error).__name__)
lo,hi=30000,200000
assert not run(lo)['childSuccess'] and run(hi)['childSuccess']
while lo<hi:
 mid=lo+(hi-lo)//2
 if run(mid)['childSuccess']:hi=mid
 else:lo=mid+1
minimum=lo
cases=[run(minimum),run(minimum-1),run(30_000_000,0,True),run(30_000_000),run(30_000_000,0,False,True)]
assert cases[0]['childSuccess'] and not cases[1]['childSuccess'] and cases[2]['slot0']==0 and cases[3]['slot0']==9 and not cases[4]['frameSuccess'] and cases[4]['slot0']==0
report=dict(scope=__doc__,semanticVersion='LP4-SYS-01',minimumGas=minimum,compiledArtifactSHA256=hashlib.sha256(artifact.read_bytes()).hexdigest(),preRoot=root(pre),cases=cases,bootable=False)
Path(sys.argv[1]).write_text(json.dumps(report,indent=2)+'\n')
if len(sys.argv)>2:
 source=Path(sys.argv[2]);actual=json.loads(source.read_text());assert actual['bootable'] is False;spec=rlp.decode(raw(actual['genesisSpec']));assert keccak(raw(actual['genesisSpec']))==raw(actual['genesisHash']);subsidy=int.from_bytes(spec[2][15],'big');comparisons=[]
 for row in actual['rows']:
  assert row['approved'] is False and int(row['subsidy'])==subsidy
  s=state(row['pre'],header.copy(base_fee_per_gas=0));caller=raw(row['caller']);to=raw(row['to'])
  tx=CancunVM.create_unsigned_transaction(nonce=1,gas_price=0,gas=row['gas'],to=to,value=0,data=raw(row['input']))
  c=s.apply_transaction(SpoofTransaction(tx,from_=caller));assert not c.is_error
  observed=s.get_storage(TARGET,1);assert observed==int(row['observed'])
  if row['model']=='credit-after-USER-before-closeBlock':s.delta_balance(ESCROW,subsidy)
  else:assert row['model']=='credit-before-USER'
  actual_root='0x'+s.make_state_root().hex();expected=root({a:{**v,'balance':int(v['balance'])} for a,v in row['post'].items()})
  assert actual_root==expected==row['stateRoot'] and CancunVM.finalize_gas_used(tx,c)==row['gasUsed']
  comparisons.append(dict(model=row['model'],observed=observed,stateRoot=actual_root,gasUsed=row['gasUsed'],approved=False))
 assert len(comparisons)==2 and comparisons[0]['stateRoot']!=comparisons[1]['stateRoot']
 Path(sys.argv[3]).write_text(json.dumps(dict(scope='Unapproved issuance countermodels, unsigned EVM controls, not PoCol block acceptance',inputSHA256=hashlib.sha256(source.read_bytes()).hexdigest(),comparisons=comparisons,rootsDiffer=True,bootable=False),indent=2)+'\n')
print('Independent measurements: compiled EIP150 minimum',minimum,'cases',len(cases),'; no full acceptance')
