"""Public scalar1 synthetic signed USER fixtures; no credentials/network/broadcast."""
import json,sys,rlp,hashlib
from eth_keys import keys
from eth.vm.forks.cancun import CancunVM
from eth_utils import keccak
from pathlib import Path
key=keys.PrivateKey((1).to_bytes(32,'big'));b=CancunVM.get_transaction_builder();to=bytes([0x66])*20;data=keccak(b'captureCoinbaseBalance()')[:4]
rows=[]
for kind in [0,1,2]:
 if kind==0:tx=b.create_unsigned_transaction(nonce=1,gas_price=103,gas=100000,to=to,value=0,data=data).as_signed_transaction(key,chain_id=777902)
 elif kind==1:tx=b.new_unsigned_access_list_transaction(777902,1,103,100000,to,0,data,()).as_signed_transaction(key)
 else:tx=b.new_unsigned_dynamic_fee_transaction(777902,1,3,200,100000,to,0,data,()).as_signed_transaction(key)
 raw=rlp.encode(tx) if kind==0 else tx.encode();rows.append(dict(kind=kind,raw=raw.hex(),rawSHA256=hashlib.sha256(raw).hexdigest(),sender=key.public_key.to_canonical_address().hex(),input=data.hex()))
u=b.create_unsigned_transaction(nonce=1,gas_price=103,gas=100000,to=(0xc0c002).to_bytes(20,'big'),value=0,data=keccak(b'closeBlock(uint256)')[:4]+(77777).to_bytes(32,'big')).as_signed_transaction(key,chain_id=777902)
chunkdata=keccak(b'store(bytes)')[:4]+(32).to_bytes(32,'big')+(1).to_bytes(32,'big')+b'\xaa'+bytes(31)
chunk=b.create_unsigned_transaction(nonce=1,gas_price=103,gas=100000,to=(0xc0c005).to_bytes(20,'big'),value=0,data=chunkdata).as_signed_transaction(key,chain_id=777902)
Path(sys.argv[1]).write_text(json.dumps({'scope':__doc__,'rows':rows,'unauthorizedClose':rlp.encode(u).hex(),'chunkStore':rlp.encode(chunk).hex(),'bootable':False},indent=2)+'\n')
