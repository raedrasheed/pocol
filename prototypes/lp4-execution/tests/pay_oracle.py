"""Independent py-evm Cancun execution with separately written LP4-PAY-02 hook.
Compiled harnesses only, not PoCol registry/closing acceptance. No Rust imports.
"""
import json,sys
from eth.db.atomic import AtomicDB
from eth.vm.chain_context import ChainContext
from eth.vm.forks.cancun import CancunVM
from eth.vm.forks.cancun.blocks import CancunBlockHeader
from eth.vm.forks.cancun.computation import CancunComputation
from eth.vm.forks.cancun.state import CancunState
from eth.vm.spoof import SpoofTransaction
from eth.exceptions import OutOfGas

PAY=(0xc0c0ff).to_bytes(20,'big');ROLES={(0xc0c001).to_bytes(20,'big'),(0xc0c002).to_bytes(20,'big')}
USER=bytes([0x11])*20;DEST=bytes([0x33])*20;ESCROW=(0xc0c002).to_bytes(20,'big')
class PayComputation(CancunComputation):
    @classmethod
    def apply_computation(cls,state,message,transaction_context,parent_computation=None):
        if message.code_address!=PAY and message.storage_address!=PAY:
            return super().apply_computation(state,message,transaction_context,parent_computation)
        with cls(state,message,transaction_context) as computation:
            data=bytes(message.data)
            valid=(message.code_address==message.storage_address==PAY and not message.is_static
                   and message.should_transfer_value and message.value==0 and len(data)==64
                   and data[:12]==bytes(12) and message.sender in ROLES
                   and parent_computation is not None
                   and parent_computation.msg.code_address==message.sender)
            if not valid:raise OutOfGas('invalid PAY boundary')
            recipient=data[12:32];amount=int.from_bytes(data[32:],'big')
            if (recipient==bytes(20) or not state.pretransaction_durable(recipient) or not state.account_exists(recipient) or state.account_is_empty(recipient)
                or state.get_balance(message.sender)<amount
                or (recipient!=message.sender and state.get_balance(recipient)+amount>=2**256)):
                raise OutOfGas('invalid PAY resource precondition')
            computation.consume_gas(5000,'LP4-PAY-01')
            if recipient!=message.sender:
                state.delta_balance(message.sender,-amount);state.delta_balance(recipient,amount)
        return computation
class PayState(CancunState):
    computation_class=PayComputation
    def apply_transaction(self,transaction):
        # Immutable root of the actual state before any transaction execution.
        # Separate reader: journal writes/CREATE cannot alter eligibility facts.
        self.persist()
        root=self.state_root
        self._pay_original=type(self)(self._db,self.execution_context,root)
        return super().apply_transaction(transaction)
    def pretransaction_durable(self,address):
        original=self._pay_original
        return original.get_nonce(address)>0 or bool(original.get_code(address))
class PayVM(CancunVM):_state_class=PayState

def run(row,artifact):
    source=bytes.fromhex(row['source'][2:])
    header=CancunBlockHeader(difficulty=0,block_number=0,gas_limit=30_000_000,timestamp=1,base_fee_per_gas=0,coinbase=ESCROW,excess_blob_gas=0)
    state=PayVM.build_state(AtomicDB(),header,ChainContext(777902))
    for a,balance,code in [(USER,1_000_000,b''),(source,100,bytes.fromhex(artifact['PayBoundaryHarness']['evm']['deployedBytecode']['object'])),(DEST,7,bytes.fromhex(artifact['RecipientTrap']['evm']['deployedBytecode']['object']))]:
        state.set_balance(a,balance);state.set_nonce(a,1);state.set_code(a,code)
    if 'recipientAccount' in row:
        account=row['recipientAccount']
        state.set_balance(DEST,account['balance']);state.set_nonce(DEST,account['nonce']);state.set_code(DEST,bytes(account['code']))
    unsigned=PayVM.create_unsigned_transaction(nonce=1,gas_price=0,gas=1_000_000,to=source,value=0,data=bytes(row['input']))
    c=state.apply_transaction(SpoofTransaction(unsigned,from_=USER))
    assert not c.is_error,(row['name'],str(c.error))
    # Cancun refund cap; these harnesses have no refunds, but calculate explicitly.
    gas_used=unsigned.gas-c.get_gas_remaining();refund=min(c.get_gas_refund(),gas_used//5)
    return dict(output=list(c.output),gasUsed=gas_used-refund,sourceBalance=str(state.get_balance(source)),destBalance=str(state.get_balance(DEST)),logs=len(c.get_log_entries()))

if __name__=='__main__':
    rows=json.load(open(sys.argv[1]));artifact=json.load(open(sys.argv[2]))['output']['contracts']['PayBoundaryHarness.sol']
    results=[]
    for row in rows:
        expected=run(row,artifact);actual={k:row[k] for k in expected};assert actual==expected,(row['name'],row['source'],actual,expected)
        results.append(dict(name=row['name'],source=row['source'],comparison='PASS',**expected))
    json.dump(results,open(sys.argv[3],'w'),indent=2)
    print(f'PASS {len(results)} independently executed compiled-harness cases; full PAY/T2b NOT PASSED')
