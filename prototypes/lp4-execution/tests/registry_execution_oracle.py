"""Independent py-evm full-bytecode/state/root differential plus actual SSTORE traces.
Synthetic execution fixtures, not full CP/profile/resource/GenesisBuilder acceptance.
"""
import json,sys
from eth.db.atomic import AtomicDB
from eth.vm.chain_context import ChainContext
from eth.vm.forks.cancun.blocks import CancunBlockHeader
from eth.vm.spoof import SpoofTransaction
from pay_oracle import PayComputation,PayState,PayVM,ESCROW
from gst_control import root
from registry_transition_model import transition
REG=(0xc0c001).to_bytes(20,'big')
class TracedComputation(PayComputation):
    opcodes=dict(PayComputation.opcodes)
def wrap(op):
    def execute(computation):
        a='0x'+computation.msg.storage_address.hex();v=computation._stack.values[-1]
        k=v if isinstance(v,int) else int.from_bytes(v,'big')
        computation.state.accesses.setdefault(a,{'SLOAD':set(),'SSTORE':[]})
        if op==0x55:computation.state.accesses[a]['SSTORE'].append(k)
        else:computation.state.accesses[a]['SLOAD'].add(k)
        PayComputation.opcodes[op](computation=computation)
    return execute
TracedComputation.opcodes[0x54]=wrap(0x54)
TracedComputation.opcodes[0x55]=wrap(0x55)
class TracedState(PayState):computation_class=TracedComputation
class TracedVM(PayVM):_state_class=TracedState
rows=json.load(open(sys.argv[1]));results=[]
from eth_utils import keccak
limits={keccak(s.encode())[:4]:n for s,n in [('register(address)',10),('setRewardAddress(address)',2),('claimBond()',10),('deregister()',7),('removeInactive(address)',7),('reactivate()',5)]}
for row in rows:
    header=CancunBlockHeader(difficulty=0,block_number=row['height'],gas_limit=30_000_000,timestamp=0,base_fee_per_gas=0,coinbase=ESCROW,excess_blob_gas=0)
    state=TracedVM.build_state(AtomicDB(),header,ChainContext(777902))
    for address,account in row['pre'].items():
        a=bytes.fromhex(address[2:]);state.set_nonce(a,account['nonce']);state.set_balance(a,int(account['balance']));state.set_code(a,bytes.fromhex(account['code'][2:]))
        for key,value in account['storage'].items():state.set_storage(a,int(key,16),int(value,16))
    state.accesses={}
    caller=bytes.fromhex(row['caller'][2:]);to=bytes.fromhex(row['to'][2:]);data=bytes(row['input'])
    tx=TracedVM.create_unsigned_transaction(nonce=row['pre'][row['caller']]['nonce'],gas_price=0,gas=30_000_000,to=to,value=row['value'],data=data)
    c=state.apply_transaction(SpoofTransaction(tx,from_=caller));used=tx.gas-c.get_gas_remaining();used-=min(c.get_gas_refund(),used//5)
    assert (not c.is_error)==row['success'],(row['name'],'status',c.error if c.is_error else '')
    assert list(c.output)==row['output'] and used==row['gasUsed'],(row['name'],'output/gas',used,row['gasUsed'])
    expected_root=root({a:{**v,'balance':int(v['balance'])} for a,v in row['post'].items()});actual_root='0x'+state.make_state_root().hex()
    for address,account in row['post'].items():
        a=bytes.fromhex(address[2:]);assert state.get_balance(a)==int(account['balance']) and state.get_nonce(a)==account['nonce'] and state.get_code(a).hex()==account['code'][2:],(row['name'],address,'account')
        for key,value in account['storage'].items():assert state.get_storage(a,int(key,16))==int(value,16)
    assert expected_root==actual_root,(row['name'],'complete state root',expected_root,actual_root)
    predicted,ok,out=transition(row)
    assert (ok,out)==(row['success'],row['output']),(row['name'],'domain model status/output')
    model_root=root({a:{**v,'balance':int(v['balance'])} for a,v in predicted.items()})
    assert model_root==actual_root,(row['name'],'domain model state',model_root,actual_root)
    trace=state.accesses.get('0x'+REG.hex(),{'SLOAD':set(),'SSTORE':[]});writes=trace['SSTORE'];cap=limits.get(data[:4])
    if row['success'] and cap is not None:assert len(writes)<=cap,(row['name'],'SSTORE operations',len(writes),cap)
    results.append(dict(name=row['name'],fixtureClass=row['fixtureClass'],comparison='PASS',domainModel='PASS',success=row['success'],stateRoot=actual_root,gasUsed=used,SSTOREOperations=len(writes),distinctSSTOREKeys=len(set(writes)),changedRegistryKeys=row['changedRegistryKeys'],applicableWriteCeiling=cap,accesses={a:{'SLOAD':sorted(f'0x{k:064x}' for k in v['SLOAD']),'SSTORE':[f'0x{k:064x}' for k in v['SSTORE']]} for a,v in state.accesses.items()}))
json.dump(dict(scope='full compiled Registry/user/bond bridge bytecode; synthetic profile; full LP4 NOT PASSED',bootable=False,cases=results),open(sys.argv[2],'w'),indent=2)
print(f'PASS {len(rows)} independent complete-state-root/gas/output comparisons and successful-call SSTORE ceilings; general resources NOT PASSED')
