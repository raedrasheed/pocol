"""Independent signed-envelope fixtures using unmodified py-evm and eth-keys.
The public scalar1 is a synthetic test constant; no account credentials are read.
No RPC/network is invoked. Expected envelope errors do not imply pool/block acceptance.
"""
import copy,json,sys
import rlp
from eth_keys import keys
from eth_keys.constants import SECPK1_N as N
from eth_hash.auto import keccak
from eth.vm.forks.cancun import CancunVM

B=CancunVM.get_transaction_builder();K=keys.PrivateKey((1).to_bytes(32,'big'));CHAIN=777902;TARGET=bytes.fromhex('22'*20)
def transaction(kind,variant=0,chain=CHAIN):
    to=b'' if variant==2 else TARGET;data=b'\x5f\x5f\xf3' if variant==2 else (b'\x00\x01\x00\xff' if variant==1 else b'')
    access=((TARGET,(0,0)),(TARGET,(0,))) if variant==3 and kind else ()
    args=dict(nonce=1,gas=100000,to=to,value=5,data=data)
    if kind==0:return B.create_unsigned_transaction(gas_price=200,**args).as_signed_transaction(K,chain_id=chain)
    if kind==1:u=B.new_unsigned_access_list_transaction(chain_id=chain,gas_price=200,access_list=access,**args)
    else:u=B.new_unsigned_dynamic_fee_transaction(chain_id=chain,max_priority_fee_per_gas=10,max_fee_per_gas=300,access_list=access,**args)
    return u.as_signed_transaction(K)
def encode(kind,fields):return (bytes([kind]) if kind else b'')+rlp.encode(fields)
def fields(kind,raw):return rlp.decode(raw[1:] if kind else raw)
def view(kind,tx):
    tx.validate();tx.check_signature_validity()
    access=getattr(tx,'access_list',())
    return dict(kind=kind,sender='0x'+tx.sender.hex(),hash='0x'+bytes(tx.hash).hex(),signingHash='0x'+keccak(tx.get_message_for_signing()).hex(),nonce=tx.nonce,gasLimit=tx.gas,maxFee=str(tx.max_fee_per_gas if kind==2 else tx.gas_price),priority=str(tx.max_priority_fee_per_gas) if kind==2 else None,value=str(tx.value),to='0x'+tx.to.hex() if tx.to else None,input='0x'+tx.data.hex(),accessList=[dict(address='0x'+a.hex(),storageKeys=['0x'+int(k).to_bytes(32,'big').hex() for k in ks]) for a,ks in access])

def generate():
    rows=[]
    def good(name,kind,raw):
        tx=B.decode(raw);rows.append(dict(name=name,raw=raw.hex(),view=view(kind,tx)))
    def bad(name,raw,error):rows.append(dict(name=name,raw=raw.hex(),error=error))
    for kind in [0,1,2]:
        for variant in range(4):good(f'type{kind}-variant{variant}',kind,transaction(kind,variant).encode())
        bad(f'wrong-chain-{kind}',transaction(kind,chain=777001).encode(),'ChainId')
        raw=transaction(kind).encode();f=fields(kind,raw)
        high=copy.deepcopy(f);high[-1]=(N-int.from_bytes(f[-1],'big')).to_bytes(32,'big')
        parity=int.from_bytes(high[-3],'big');high[-3]=((parity-35)^1)+35 if kind==0 else parity^1
        bad(f'high-s-{kind}',encode(kind,high),'HighS')
        for n,suffix in [(N//2,'equal'),(N//2-1,'minus1')]:
            boundary=copy.deepcopy(f);boundary[-1]=n.to_bytes(32,'big');good(f'low-s-{suffix}-{kind}',kind,encode(kind,boundary))
        for index in [-2,-1]:
            invalid=copy.deepcopy(f);invalid[index]=b'';bad(f'zero-scalar-{kind}-{index}',encode(kind,invalid),'Signature')
            invalid=copy.deepcopy(f);invalid[index]=N.to_bytes(32,'big');bad(f'order-scalar-{kind}-{index}',encode(kind,invalid),'Signature')
        invalid=copy.deepcopy(f);invalid[-2]=b'\x00'+invalid[-2];bad(f'leading-scalar-{kind}',encode(kind,invalid),'Integer')
        nonce=0 if kind==0 else 1;invalid=copy.deepcopy(f);invalid[nonce]=b'\x00';bad(f'zero-byte-nonce-{kind}',encode(kind,invalid),'Integer')
        invalid=copy.deepcopy(f);invalid[nonce]=b'\x01'+bytes(8);bad(f'wide-nonce-{kind}',encode(kind,invalid),'Integer')
        dest={0:3,1:4,2:5}[kind]
        for size in [19,21]:
            invalid=copy.deepcopy(f);invalid[dest]=bytes(size);bad(f'address-{size}-{kind}',encode(kind,invalid),'Address')
        invalid=copy.deepcopy(f);invalid[dest+1]=b'\x01'+bytes(32);bad(f'wide-value-{kind}',encode(kind,invalid),'Integer')
        bad(f'trailing-{kind}',raw+b'\x00','Rlp')
        bad(f'field-count-{kind}',encode(kind,f[:-1]),'Shape')
        if kind:
            invalid=copy.deepcopy(f);invalid[-3]=2;bad(f'parity-{kind}',encode(kind,invalid),'Parity')
            ai=7 if kind==1 else 8
            for name,a in [('entry',[[TARGET]]),('address',[[bytes(19),[]]]),('key',[[TARGET,[bytes(31)]]]),('keys-list',[[TARGET,b'']])]:
                invalid=copy.deepcopy(f);invalid[ai]=a;bad(f'access-{name}-{kind}',encode(kind,invalid),'AccessList')
    bad('unprotected-legacy',transaction(0,chain=None).encode(),'UnprotectedLegacy')
    blob=B.new_unsigned_blob_transaction(chain_id=CHAIN,nonce=1,max_priority_fee_per_gas=10,max_fee_per_gas=300,gas=100000,to=TARGET,value=5,data=b'',access_list=(),max_fee_per_blob_gas=1,blob_versioned_hashes=(b'\x01'+bytes(31),)).as_signed_transaction(K)
    bad('type3-valid-ethereum',blob.encode(),'UnsupportedType')
    for first in [0,4,127]:bad(f'unsupported-{first}',bytes([first])+rlp.encode([]),'UnsupportedType')
    bad('empty',b'','Rlp');bad('string-root',b'\x80','Rlp');bad('noncanonical-rlp',b'\x01\xf8\x01\x80','Rlp')
    invalid=fields(2,transaction(2).encode());invalid[2]=301;bad('priority-above-cap',encode(2,invalid),'FeeCap')
    return dict(scope='Synthetic real signed envelope codec controls, not full P3 admission/fees/receipts/roots',bootable=False,cases=rows)
if __name__=='__main__':
    result=generate();json.dump(result,open(sys.argv[1],'w'),indent=2);print(f'Generated{len(result["cases"])} independent signed envelope cases; no private scalar emitted')
