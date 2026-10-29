"""Independent actual D27 CREATE2 execution/gas/full roots; not full PoCol apply."""
import hashlib,json,sys
from pathlib import Path
from eth_utils import keccak
from eth.db.atomic import AtomicDB
from eth.vm.chain_context import ChainContext
from eth.vm.forks.cancun.blocks import CancunBlockHeader
from eth.vm.spoof import SpoofTransaction
from gst_independent_vm import ComparisonVM,ComparisonState,ComparisonComputation
from gst_control import root
class TraceComputation(ComparisonComputation):opcodes=dict(ComparisonComputation.opcodes)
def traced(op):
 def run(computation):
  computation.state.counts[op]=computation.state.counts.get(op,0)+1
  ComparisonComputation.opcodes[op](computation=computation)
 return run
for op in TraceComputation.opcodes:TraceComputation.opcodes[op]=traced(op)
class TraceState(ComparisonState):computation_class=TraceComputation
class TraceVM(ComparisonVM):_state_class=TraceState
def raw(s):return bytes.fromhex(s.removeprefix('0x'))
source=Path(sys.argv[1]);v=json.loads(source.read_text());assert v['bootable'] is False
USER=bytes([0x11])*20;FACTORY=(0xc0c005).to_bytes(20,'big');ESCROW=(0xc0c002).to_bytes(20,'big')
code=raw(v['artifact']['output']['contracts']['ChunkFactory.sol']['ChunkFactory']['evm']['deployedBytecode']['object'])
header=CancunBlockHeader(difficulty=0,block_number=1,gas_limit=30_000_000,timestamp=0,base_fee_per_gas=0,coinbase=ESCROW,excess_blob_gas=0)
rows=[]
for row in v['cases']:
 data=bytes([0xaa])*row['bytes'];init=b'\x61'+(len(data)+1).to_bytes(2,'big')+bytes.fromhex('80600a5f395ff300')+data
 address=keccak(b'\xff'+FACTORY+keccak(data)+keccak(init))[-20:]
 assert init==raw(row['initcode']) and address==raw(row['address'])
 db=AtomicDB();s=TraceVM.build_state(db,header,ChainContext(777902))
 s.set_balance(USER,1_000_000_000);s.set_nonce(USER,1);s.set_nonce(FACTORY,1);s.set_code(FACTORY,code);assert '0x'+s.make_state_root().hex()==row['initialRoot']
 measured=[]
 for i in range(2):
  s.persist();s=TraceVM.build_state(db,header.copy(state_root=s.state_root),ChainContext(777902));s.counts={}
  calldata=keccak(b'store(bytes)')[:4]+(32).to_bytes(32,'big')+len(data).to_bytes(32,'big')+data+bytes((-len(data))%32)
  tx=TraceVM.get_transaction_builder().create_unsigned_transaction(nonce=1+i,gas_price=0,gas=8_000_000,to=FACTORY,value=0,data=calldata)
  c=s.apply_transaction(SpoofTransaction(tx,from_=USER));assert not c.is_error
  assert c.output==bytes(12)+address and not c.get_log_entries() and s.counts.get(0x55,0)==0
  gas=TraceVM.finalize_gas_used(tx,c);expected=row['firstGas' if i==0 else 'secondGas'];assert gas==expected
  assert '0x'+s.make_state_root().hex()==row['firstRoot' if i==0 else 'secondRoot']
  assert s.get_code(address)==b'\0'+data and s.get_nonce(address)==1 and s.get_nonce(FACTORY)==2
  measured.append(dict(gas=gas,sstoreInstructions=0,create2Instructions=s.counts.get(0xf5,0)))
 assert measured[0]['create2Instructions']==1 and measured[1]['create2Instructions']==0
 normalized={a:{**x,'balance':int(x['balance'])} for a,x in row['post'].items()}
 assert root(normalized)==row['secondRoot']
 for a,x in row['post'].items():
  a=raw(a);assert s.get_nonce(a)==x['nonce'] and s.get_balance(a)==int(x['balance']) and s.get_code(a)==raw(x['code'])
  assert all(s.get_storage(a,int(k,16))==int(value,16) for k,value in x['storage'].items())
 rows.append(dict(bytes=len(data),address=row['address'],root=row['secondRoot'],executions=measured,comparison='MATCH'))
assert len(rows)==3
negative=[]
for label,length,value in [('empty',0,0),('oversize',24576,0),('value',1,1),('storage-collision',1,0)]:
 data=bytes([0xaa])*length;db=AtomicDB();s=TraceVM.build_state(db,header,ChainContext(777902))
 s.set_balance(USER,1_000_000_000);s.set_nonce(USER,1);s.set_nonce(FACTORY,1);s.set_code(FACTORY,code)
 if label=='storage-collision':
  init=b'\x61'+(length+1).to_bytes(2,'big')+bytes.fromhex('80600a5f395ff300')+data
  address=keccak(b'\xff'+FACTORY+keccak(data)+keccak(init))[-20:];s.set_storage(address,0,1)
 s.persist();s.lock_changes();s.counts={}
 calldata=keccak(b'store(bytes)')[:4]+(32).to_bytes(32,'big')+length.to_bytes(32,'big')+data+bytes((-length)%32)
 tx=TraceVM.get_transaction_builder().create_unsigned_transaction(nonce=1,gas_price=0,gas=8_000_000,to=FACTORY,value=value,data=calldata)
 c=s.apply_transaction(SpoofTransaction(tx,from_=USER));assert c.is_error and s.get_nonce(FACTORY)==1 and s.counts.get(0x55,0)==0
 negative.append(dict(case=label,rejected=True,comparison='Same rejection outcome as Rust; gas/root not compared',sstoreInstructions=0))
Path(sys.argv[2]).write_text(json.dumps(dict(scope=__doc__,inputSHA256=hashlib.sha256(source.read_bytes()).hexdigest(),cases=rows,negative=negative,fullPoColAcceptance='NOT PASSED',bootable=False),indent=2)+'\n')
print('MATCH3 D27 lengths,6 actual gas/state/root comparisons and4 independent rejections, zero SSTORE; full LP4 NOT PASSED')
