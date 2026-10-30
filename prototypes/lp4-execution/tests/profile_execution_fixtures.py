"""Independent schema-bound signed execution via unmodified CancunVM; no boot."""
import json,sys,rlp
from pathlib import Path
from eth.db.atomic import AtomicDB
from eth.db.chain import ChainDB
from eth.consensus.context import ConsensusContext
from eth.vm.chain_context import ChainContext
from eth.vm.forks.cancun import CancunVM
from eth.vm.forks.cancun.blocks import CancunBlockHeader
from eth_hash.auto import keccak
from wire_fixtures import transaction,TARGET,K
base=rlp.decode(bytes.fromhex(Path(__file__).with_name('data').joinpath('execution-profile.hex').read_text().strip()));rows=[];sender=K.public_key.to_canonical_address();escrow=(0xc0c002).to_bytes(20,'big');code=bytes.fromhex('465f5500')
for chain in [777001,777902,2**64-1]:
 fields=list(base);fields[1]=chain.to_bytes((chain.bit_length()+7)//8,'big');spec=rlp.encode(fields)
 for kind in [0,1,2]:
  h=CancunBlockHeader(difficulty=0,block_number=300,timestamp=12345,gas_limit=8000000,base_fee_per_gas=100,coinbase=escrow,mix_hash=b'\x77'*32,excess_blob_gas=0);db=AtomicDB();vm=CancunVM(h,ChainDB(db),ChainContext(chain),ConsensusContext(db));s=vm.state
  s.set_balance(sender,10**9);s.set_nonce(sender,1);s.set_nonce(TARGET,1);s.set_code(TARGET,code);s.persist()
  tx=transaction(kind,chain=chain);tx.validate();tx.check_signature_validity();receipt,c=vm.apply_transaction(h,tx);assert not c.is_error and s.get_storage(TARGET,0)==chain
  rows.append(dict(chain=chain,kind=kind,genesisSpec=spec.hex(),genesisHash=keccak(spec).hex(),raw=tx.encode().hex(),hash=bytes(tx.hash).hex(),stateRoot=s.make_state_root().hex(),receipt=receipt.encode().hex(),gasUsed=receipt.gas_used,bootable=False))
with open(sys.argv[1],'w') as f:json.dump(rows,f,indent=2);f.write('\n')
print('Generated9 independent actual signed profile-bound execution cases, including max-u64 legacy EIP155 v; boot false')
