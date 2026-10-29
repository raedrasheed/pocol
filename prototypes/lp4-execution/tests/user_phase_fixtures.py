"""Unmodified py-evm sequential signed USER fixture; not full PoCol apply."""
import sys,json
from eth.db.atomic import AtomicDB
from eth.db.chain import ChainDB
from eth.consensus.context import ConsensusContext
from eth.vm.chain_context import ChainContext
from eth.vm.forks.cancun import CancunVM
from eth.vm.forks.cancun.blocks import CancunBlockHeader
from eth_hash.auto import keccak
from trie import HexaryTrie
import rlp
from wire_fixtures import B,K,CHAIN,TARGET
sender=K.public_key.to_canonical_address();escrow=(0xc0c002).to_bytes(20,'big');code=bytes.fromhex('5f355f5560203611600c57005b5f5ffd')
h=CancunBlockHeader(difficulty=0,block_number=300,timestamp=12345,gas_limit=8000000,base_fee_per_gas=100,coinbase=escrow,mix_hash=b'\x77'*32,excess_blob_gas=0)
db=AtomicDB();vm=CancunVM(h,ChainDB(db),ChainContext(CHAIN),ConsensusContext(db));s=vm.state;s.set_balance(sender,10**9);s.set_nonce(sender,1);s.set_code(TARGET,code);s.set_nonce(TARGET,1);s.persist()
raw=[];receipts=[];gas=[];refund=[];tips=0;cum=0;txroot=HexaryTrie(db={});rr=HexaryTrie(db={})
for i in range(4):
 data=(1 if i==0 else 0 if i==1 else 2).to_bytes(32,'big')+(b'\x01' if i==2 else b'')
 args=dict(nonce=i+1,gas=100000,to=TARGET if i<3 else bytes([0x33])*20,value=5 if i<3 else 0,data=data if i<3 else b'')
 if i==0:tx=B.create_unsigned_transaction(gas_price=200,**args).as_signed_transaction(K,chain_id=CHAIN)
 elif i==1:tx=B.new_unsigned_access_list_transaction(chain_id=CHAIN,gas_price=200,access_list=(),**args).as_signed_transaction(K)
 else:tx=B.new_unsigned_dynamic_fee_transaction(chain_id=CHAIN,max_priority_fee_per_gas=10 if i<3 else 0,max_fee_per_gas=300,access_list=(),**args).as_signed_transaction(K)
 tx.validate();tx.check_signature_validity();r,c=vm.apply_transaction(h.copy(gas_used=cum),tx);used=r.gas_used-cum;cum=r.gas_used;gas.append(used);refund.append(c.get_gas_refund());tips+=used*(100 if i<2 else 10 if i==2 else 0)
 raw.append(tx.encode().hex());receipts.append(r.encode().hex());txroot[rlp.encode(i)]=tx.encode();rr[rlp.encode(i)]=r.encode()
 assert c.is_error==(i==2)
post={}
for a in [sender,TARGET,escrow]:post[a.hex()]=dict(nonce=s.get_nonce(a),balance=str(s.get_balance(a)),codeHash=keccak(s.get_code(a)).hex(),slot0=str(s.get_storage(a,0)))
result=dict(scope='Synthetic sequential signed USER phase only; no system/final-header/persistence acceptance',sender=sender.hex(),target=TARGET.hex(),code=code.hex(),raw=raw,receipts=receipts,gas=gas,refund=refund,gasUsed=cum,tips=str(tips),transactionsRoot=txroot.root_hash.hex(),receiptsRoot=rr.root_hash.hex(),postStateRoot=s.make_state_root().hex(),post=post,bootable=False)
with open(sys.argv[1],'w') as f:json.dump(result,f,indent=2);f.write('\n')
print('Generated4 sequential signed user transactions, storage clear refund late revert and empty-account cleanup; full apply NOT PASSED')
