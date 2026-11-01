"""Actual independent comparator boundaries; not a whole LP4 acceptance gate."""
from eth.db.atomic import AtomicDB
from eth.vm.chain_context import ChainContext
from eth.vm.forks.cancun.blocks import CancunBlockHeader
from eth.vm.message import Message
from eth.vm.transaction_context import BaseTransactionContext
from gst_independent_vm import ComparisonVM, ComparisonComputation, MODEXP

def main():
    state=ComparisonVM.build_state(AtomicDB(),CancunBlockHeader(difficulty=0,block_number=1,gas_limit=30000000,timestamp=1,base_fee_per_gas=0,excess_blob_gas=0),ChainContext(1))
    address=bytes.fromhex('12'*20)
    assert not state.has_code_or_nonce(address)
    state.set_storage(address,1,7); assert state.has_code_or_nonce(address)
    snapshot=state.snapshot();state.set_storage(address,1,0);assert not state.has_code_or_nonce(address);state.revert(snapshot);assert state.has_code_or_nonce(address)
    snapshot=state.snapshot();state.set_storage(address,2,8);state.revert(snapshot);assert state.get_storage(address,2)==0 and state.has_code_or_nonce(address)
    state.set_storage(address,1,0);assert not state.has_code_or_nonce(address)
    state.set_nonce(address,1);assert state.has_code_or_nonce(address)
    for base,exponent,modulus,expected in [(b'',b'',b'\x02',b'\x01'),(b'',b'',b'\0',b'\0'),(b'',b'',b'\x01',b'\0'),(b'',b'\x01',b'\x02',b'\0'),(b'',b'\0',b'\x02',b'\x01'),(b'\x02',b'\x05',b'\x07',b'\x04')]:
        data=b''.join(len(v).to_bytes(32,'big') for v in (base,exponent,modulus))+base+exponent+modulus
        message=Message(gas=100000,to=MODEXP,sender=address,value=0,data=data,code=b'',code_address=MODEXP)
        computation=ComparisonComputation.apply_message(state,message,BaseTransactionContext(gas_price=0,origin=address))
        assert not computation.is_error and computation.output==expected
    print('PASS comparator storage/clear/rollback/nonce boundaries and6 actual MODEXP precompile executions; full P2 NOT PASSED')
if __name__=='__main__':main()
