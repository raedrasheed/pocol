"""Explicit independent comparator adaptations; never installed into runtime.
CREATE-01: approved EIP-7610, complete fixture storage plus all actual overlay writes.
MODEXP: correct py-evm0.12.1b1's zero-base/zero-exponent shortcut using EIP-198
integer exponentiation (0**0 mod m); standard Cancun gas remains py-evm's own.
Standard Cancun admission checks supplement the unsigned/spoof adapter: EIP-2681,
3607,1559,3860,4844 plus intrinsic/block gas and complete maximal payment.
This is a patched independent comparator, not unmodified Cancun or signed-wire proof.
"""
from eth_utils import ValidationError
from eth.vm.forks.cancun import CancunVM
from eth.vm.forks.cancun.state import CancunState
from eth.vm.forks.cancun.computation import CancunComputation

MODEXP=(5).to_bytes(20,'big')

class ComparisonComputation(CancunComputation):
    _precompiles=dict(CancunComputation._precompiles)

def modexp(computation):
    # First perform the unchanged engine's gas calculation/charge and bounded output.
    result=CancunComputation._precompiles[MODEXP](computation)
    data=computation.msg.data_as_bytes
    base_len=int.from_bytes(data[:32].ljust(32,b'\0'),'big')
    exp_len=int.from_bytes(data[32:64].ljust(32,b'\0'),'big')
    mod_len=int.from_bytes(data[64:96].ljust(32,b'\0'),'big')
    if base_len==0 and mod_len and not any(data[96:96+exp_len]):
        modulus=int.from_bytes(data[96+exp_len:96+exp_len+mod_len].ljust(mod_len,b'\0'),'big')
        value=pow(0,0,modulus) if modulus else 0
        computation.output=value.to_bytes(mod_len,'big')
    return result

ComparisonComputation._precompiles[MODEXP]=modexp

class ComparisonState(CancunState):
    computation_class=ComparisonComputation
    def __init__(self,*args,**kwargs):
        self._complete_keys={}
        super().__init__(*args,**kwargs)
    def set_storage(self,address,slot,value):
        self._complete_keys.setdefault(address,set()).add(slot)
        return super().set_storage(address,slot,value)
    def has_code_or_nonce(self,address):
        # Key union need not roll back: every value is read from the CURRENT journal.
        # Cleared/deleted/reverted writes therefore cannot manufacture a collision.
        return super().has_code_or_nonce(address) or any(self.get_storage(address,k) for k in self._complete_keys.get(address,()))

class ComparisonVM(CancunVM):
    _state_class=ComparisonState


class AdmissionComparisonState(ComparisonState):
    def validate_transaction(self,transaction):
        # State.apply_transaction(unsigned/spoof) does not run every signed/header
        # validator in py-evm0.12.1b1. Supply standard Cancun admission checks here,
        # independently of Rust and of expected fixture outcome/error labels.
        # This does NOT authenticate the fixture sender or verify wire signatures.
        super().validate_transaction(transaction)
        if self.get_code(transaction.sender):
            raise ValidationError('EIP-3607 sender has code')
        if transaction.nonce>=2**64-1 or self.get_nonce(transaction.sender)>=2**64-1:
            raise ValidationError('EIP-2681 nonce cannot increment')
        if transaction.gas>self.execution_context.gas_limit:
            raise ValidationError('Transaction exceeds block gas limit')
        if transaction.max_priority_fee_per_gas>transaction.max_fee_per_gas:
            raise ValidationError('EIP-1559 priority fee exceeds fee cap')
        create=not transaction.to
        intrinsic=transaction.intrinsic_gas
        if create:
            if len(transaction.data)>49152:
                raise ValidationError('EIP-3860 initcode exceeds limit')
            intrinsic+=2*((len(transaction.data)+31)//32)
        if transaction.gas<intrinsic:
            raise ValidationError('Intrinsic gas exceeds transaction gas')
        payment=transaction.gas*transaction.max_fee_per_gas+transaction.value
        if transaction.type_id==3:
            blobs=transaction.blob_versioned_hashes
            if len(blobs)>6:
                raise ValidationError('EIP-4844 blobs exceed block allowance')
            payment+=len(blobs)*131072*transaction.max_fee_per_blob_gas
        if payment>=2**256 or payment>self.get_balance(transaction.sender):
            raise ValidationError('Maximal gas/value/blob payment unaffordable')


class AdmissionComparisonVM(CancunVM):
    _state_class=AdmissionComparisonState
