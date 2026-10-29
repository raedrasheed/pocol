"""Standard Cancun unsigned-adapter admission boundaries; no signature/wire claim."""
from eth.db.atomic import AtomicDB
from eth.vm.chain_context import ChainContext
from eth.vm.forks.cancun.blocks import CancunBlockHeader
from eth.vm.spoof import SpoofTransaction
from eth_utils import ValidationError
from gst_independent_vm import AdmissionComparisonVM

SENDER=bytes.fromhex('12'*20);DEST=bytes.fromhex('34'*20)
def main():
    state=AdmissionComparisonVM.build_state(AtomicDB(),CancunBlockHeader(difficulty=0,block_number=1,gas_limit=30000000,timestamp=1,base_fee_per_gas=1,excess_blob_gas=0),ChainContext(1))
    builder=AdmissionComparisonVM.get_transaction_builder();count=0
    def check(expected,**change):
        nonlocal count
        values=dict(chain_id=1,nonce=0,max_priority_fee_per_gas=0,max_fee_per_gas=2,gas=21000,to=DEST,value=7,data=b'',access_list=[])
        balance=change.pop('balance',10**18);code=change.pop('code',b'');nonce=change.pop('state_nonce',0);blob_count=change.pop('blob_count',None)
        values.update(change);state.set_balance(SENDER,balance);state.set_code(SENDER,code);state.set_nonce(SENDER,nonce)
        if blob_count is not None:
            values.update(blob_versioned_hashes=[b'\x01'+bytes(31)]*blob_count,max_fee_per_blob_gas=1)
            tx=builder.new_unsigned_blob_transaction(**values)
        else:tx=builder.new_unsigned_dynamic_fee_transaction(**values)
        try:state.validate_transaction(SpoofTransaction(tx,from_=SENDER));actual=True
        except ValidationError:actual=False
        assert actual==expected,('admission boundary',count,actual,expected)
        count+=1
    check(True,balance=42007);check(False,balance=42006)
    check(True,value=0,gas=21000);check(False,value=0,gas=20999)
    check(True,to=b'',data=b'\0',gas=53006);check(False,to=b'',data=b'\0',gas=53005)
    check(True,to=b'',data=bytes(49152),gas=1000000);check(False,to=b'',data=bytes(49153),gas=1000000)
    check(True,gas=30000000);check(False,gas=30000001)
    check(True,nonce=2**64-2,state_nonce=2**64-2);check(False,nonce=2**64-1,state_nonce=2**64-1)
    check(True,code=b'');check(False,code=b'\0')
    check(True,max_priority_fee_per_gas=2);check(False,max_priority_fee_per_gas=3)
    check(True,blob_count=6);check(False,blob_count=7)
    check(True,blob_count=1,balance=173079);check(False,blob_count=1,balance=173078)
    print(f'PASS {count} actual engine/adaptor acceptance, equality and negative checks; signed-wire proof NOT PASSED')
if __name__=='__main__':main()
