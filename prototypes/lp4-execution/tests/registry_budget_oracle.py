"""Independent py-evm SSTORE trace of lower-bound witnesses, not Registry acceptance."""
import json,sys
from eth_utils import keccak
from eth.db.atomic import AtomicDB
from eth.vm.chain_context import ChainContext
from eth.vm.forks.cancun import CancunVM
from eth.vm.forks.cancun.blocks import CancunBlockHeader
from eth.vm.forks.cancun.computation import CancunComputation
from eth.vm.forks.cancun.state import CancunState
from eth.vm.spoof import SpoofTransaction
USER=bytes([0x11])*20;REWARD=bytes([0x33])*20;MOVED=bytes([0x44])*20
SOURCE=(0xc0c001).to_bytes(20,'big');ESCROW=(0xc0c002).to_bytes(20,'big')
def record(a):return int.from_bytes(keccak(bytes(12)+a+(7).to_bytes(32,'big')),'big')
def base(n):return int.from_bytes(keccak(n.to_bytes(32,'big')),'big')
def trace(computation):
    top=computation._stack.values[-1]
    key=top if isinstance(top,int) else int.from_bytes(top,'big')
    computation.state.writes.add(key)
    CancunComputation.opcodes[0x55](computation=computation)
class WitnessComputation(CancunComputation):
    opcodes=dict(CancunComputation.opcodes)
    opcodes[0x55]=trace
class WitnessState(CancunState):computation_class=WitnessComputation
class WitnessVM(CancunVM):_state_class=WitnessState
rows=json.load(open(sys.argv[1]));artifact=json.load(open(sys.argv[2]))['output']['contracts']['RegistryWriteBudgetWitness.sol']['RegistryWriteBudgetWitness']
results=[]
for row in rows:
    state=WitnessVM.build_state(AtomicDB(),CancunBlockHeader(difficulty=0,block_number=1,gas_limit=30_000_000,timestamp=1,base_fee_per_gas=0,coinbase=ESCROW,excess_blob_gas=0),ChainContext(777902))
    state.set_balance(USER,10**24);state.set_nonce(USER,1);state.set_nonce(REWARD,1);state.set_nonce(bytes([0x22])*20,1)
    state.set_nonce(SOURCE,1);state.set_code(SOURCE,bytes.fromhex(artifact['evm']['deployedBytecode']['object']))
    before={int(k,16):int(v,16) for k,v in row['preStorage']}
    for k,v in before.items():state.set_storage(SOURCE,k,v)
    state.persist() # Commit fixture setup so EIP-2200 original storage is correct.
    state.writes=set()
    tx=WitnessVM.create_unsigned_transaction(nonce=1,gas_price=0,gas=1_000_000,to=SOURCE,value=int(row['value']),data=bytes(row['input']))
    c=state.apply_transaction(SpoofTransaction(tx,from_=USER));assert not c.is_error
    r=record(USER)
    expected={
      'register':{0,1,3,4,base(5)+1,base(6)+1,r,r+1,r+2,r+3},
      'setRewardAddress':{3,r+1},
      'claimBond-middle':{0,3,4,base(5),base(5)+2,record(MOVED),r,r+1,r+2,r+3},
    }[row['case']]
    assert state.writes==expected,(row['case'],state.writes,expected)
    changed=sorted([[f'0x{k:064x}',f'0x{state.get_storage(SOURCE,k):064x}'] for k in state.writes if state.get_storage(SOURCE,k)!=before.get(k,0)])
    used=tx.gas-c.get_gas_remaining();used-=min(c.get_gas_refund(),used//5)
    actual=dict(changedStorage=changed,actualChangedSlots=len(changed),gasUsed=used,output=list(c.output),registryBalance=str(state.get_balance(SOURCE)),userBalance=str(state.get_balance(USER)))
    assert {k:row[k] for k in actual}==actual,(row['case'],row,actual)
    ceiling={'register':9,'setRewardAddress':1,'claimBond-middle':9}[row['case']]
    assert row['originalCeiling']==ceiling and len(changed)>ceiling
    results.append(dict(case=row['case'],originalCeiling=ceiling,measuredChangedSlots=len(changed),SSTOREKeys=sorted(f'0x{k:064x}' for k in state.writes),comparison='MATCHED INDEPENDENT EXECUTION',acceptance='BLOCKED: ORIGINAL CEILING EXCEEDED',gasUsed=used))
json.dump(dict(scope='mandatory-write-set witnesses only; not full Registry',status='ACCEPTANCE BLOCKER REPRODUCED',bootable=False,cases=results),open(sys.argv[3],'w'),indent=2)
print('Reproduced 3 original-ceiling conflicts with independent SSTORE traces; acceptance NOT PASSED')
