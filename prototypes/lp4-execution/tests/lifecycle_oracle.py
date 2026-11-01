"""Independent py-evm execution of both compiled PAY-02 lifetime attack controls."""
import json,sys,rlp
from eth_utils import keccak
from eth.db.atomic import AtomicDB
from eth.vm.chain_context import ChainContext
from eth.vm.forks.cancun.blocks import CancunBlockHeader
from eth.vm.spoof import SpoofTransaction
from pay_oracle import PayVM,USER,ESCROW
SOURCE=(0xc0c001).to_bytes(20,'big')
artifact=json.load(open(sys.argv[2]))['output']['contracts']['RewardDurabilityWitness.sol']['RewardDurabilityWitness']
rows=json.load(open(sys.argv[1]));results=[]
for row in rows:
    state=PayVM.build_state(AtomicDB(),CancunBlockHeader(difficulty=0,block_number=0,gas_limit=30_000_000,timestamp=1,base_fee_per_gas=0,coinbase=ESCROW,excess_blob_gas=0),ChainContext(777902))
    state.set_balance(USER,1_000_000)
    state.set_balance(SOURCE,100);state.set_nonce(SOURCE,1);state.set_code(SOURCE,bytes.fromhex(artifact['evm']['deployedBytecode']['object']))
    child=keccak(rlp.encode([SOURCE,1]))[-20:]
    if row['prefunded']:state.set_balance(child,1)
    tx=PayVM.create_unsigned_transaction(nonce=0,gas_price=0,gas=800_000,to=SOURCE,value=0,data=keccak(b'probeThenErase()')[:4])
    c=state.apply_transaction(SpoofTransaction(tx,from_=USER));assert not c.is_error
    used=tx.gas-c.get_gas_remaining();used-=min(c.get_gas_refund(),used//5)
    expected=dict(output=list(c.output),gasUsed=used,logs=len(c.get_log_entries()),sourceBalance=str(state.get_balance(SOURCE)),userBalance=str(state.get_balance(USER)),childAbsent=not state.account_exists(child))
    assert c.output[:32]==bytes(32), 'PAY-02 must reject both attacks'
    assert {k:row[k] for k in expected}==expected,(row,expected)
    results.append(dict(prefunded=row['prefunded'],comparison='PASS',**expected))
json.dump(results,open(sys.argv[3],'w'),indent=2)
print(f'PASS {len(results)} independent compiled lifetime controls; lifecycle acceptance NOT PASSED')
