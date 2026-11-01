"""Independent unmodified py-evm field/ancestor execution, gas and complete roots.
Synthetic contexts only; stored parent/seal provenance and PoCol apply NOT PASSED.
"""
import json,sys
from eth.db.atomic import AtomicDB
from eth.vm.chain_context import ChainContext
from eth.vm.forks.cancun import CancunVM
from eth.vm.forks.cancun.blocks import CancunBlockHeader
from eth.vm.spoof import SpoofTransaction
from gst_control import root

rows=json.load(open(sys.argv[1]));results=[]
assert len(rows)==9 and len({r['name'] for r in rows})==9
for row in rows:
    header=CancunBlockHeader(difficulty=0,block_number=row['height'],gas_limit=row['gasLimit'],timestamp=row['timestamp'],base_fee_per_gas=row['baseFee'],coinbase=(0xc0c002).to_bytes(20,'big'),mix_hash=bytes.fromhex(row['prevrandao'][2:]),excess_blob_gas=0)
    # The second engine uses its own ordered ancestor lookup rather than the
    # native backend's cache or its output. Include all 256 ancestor positions.
    ancestors=[bytes.fromhex(row['cachedHash'][2:]) if row['height']-i-1==row['cachedNumber'] else bytes(32) for i in range(min(256,row['height']))]
    state=CancunVM.build_state(AtomicDB(),header,ChainContext(777902),previous_hashes=ancestors)
    for address,a in row['pre'].items():
        address=bytes.fromhex(address[2:]);state.set_balance(address,int(a['balance']));state.set_nonce(address,a['nonce']);state.set_code(address,bytes.fromhex(a['code'][2:]))
    state.persist()
    tx=CancunVM.create_unsigned_transaction(nonce=1,gas_price=200,gas=100000,to=bytes.fromhex(row['to'][2:]),value=0,data=b'')
    c=state.apply_transaction(SpoofTransaction(tx,from_=bytes.fromhex(row['caller'][2:])))
    used=tx.gas-c.get_gas_remaining();used-=min(c.get_gas_refund(),used//5)
    expected=root({a:{**v,'balance':int(v['balance'])} for a,v in row['post'].items()});actual='0x'+state.make_state_root().hex()
    assert not c.is_error and row['success'] and list(c.output)==row['output'],(row['name'],'output/status')
    assert used==row['gasUsed'] and expected==actual,(row['name'],'gas/root',used,row['gasUsed'],expected,actual)
    results.append(dict(name=row['name'],comparison='MATCH',gasUsed=used,stateRoot=actual))
json.dump(dict(scope='Explicit synthetic context, not validated PoCol headers/apply',cases=results,bootable=False),open(sys.argv[2],'w'),indent=2)
print('PASS 9 independently executed context/output/gas/full-state-root comparisons; apply NOT PASSED')
