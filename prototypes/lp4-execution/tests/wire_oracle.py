"""Actual signed py-evm execution: no SpoofTransaction/sender success flags.
Complete state roots, fee/burn/tip balances, gas, output and logs compared to Rust.
Synthetic context only; receipts/bloom/body/block/persistence acceptance still open.
"""
import json,sys
from eth.db.atomic import AtomicDB
from eth.vm.chain_context import ChainContext
from eth.vm.forks.cancun import CancunVM
from eth.vm.forks.cancun.blocks import CancunBlockHeader
from gst_control import root,loghash
ESCROW=(0xc0c002).to_bytes(20,'big')
rows=json.load(open(sys.argv[1]));results=[]
for row in rows:
    header=CancunBlockHeader(difficulty=0,block_number=300,timestamp=12345,gas_limit=8000000,base_fee_per_gas=100,coinbase=ESCROW,mix_hash=b'\x77'*32,excess_blob_gas=0)
    state=CancunVM.build_state(AtomicDB(),header,ChainContext(777902))
    for address,a in row['pre'].items():
        address=bytes.fromhex(address[2:]);state.set_balance(address,int(a['balance']));state.set_nonce(address,a['nonce']);state.set_code(address,bytes.fromhex(a['code'][2:]))
    state.persist();tx=CancunVM.get_transaction_builder().decode(bytes.fromhex(row['raw']));tx.validate();tx.check_signature_validity()
    assert '0x'+tx.sender.hex()==row['decoded']['sender']
    c=state.apply_transaction(tx);used=tx.gas-c.get_gas_remaining();used-=min(c.get_gas_refund(),used//5)
    assert (not c.is_error)==row['success'] and list(c.output)==row['output'] and used==row['gasUsed'],(row['name'],'execution/gas')
    logs=[dict(address='0x'+a.hex(),topics=['0x'+t.to_bytes(32,'big').hex() for t in topics],data='0x'+data.hex()) for a,topics,data in c.get_log_entries()]
    assert logs==row['logs'],(row['name'],'logs')
    actual='0x'+state.make_state_root().hex();expected=root({a:{**v,'balance':int(v['balance'])} for a,v in row['post'].items()});assert actual==expected,(row['name'],'complete root',actual,expected)
    tip=min(tx.max_priority_fee_per_gas,tx.max_fee_per_gas-100) if row['decoded']['kind']==2 else tx.gas_price-100
    assert state.get_balance(ESCROW)==used*tip,(row['name'],'tip')
    assert state.get_balance(tx.sender)==1000000000-(tx.value if not c.is_error else 0)-used*(100+tip),(row['name'],'fee/value')
    receipt=CancunVM.make_receipt(header,tx,c,state)
    assert list(receipt.encode())==row['receipt'] and list(receipt.bloom.to_bytes(256,'big'))==row['bloom'],(row['name'],'typed receipt/bloom')
    assert int(row['effectivePrice'])==100+tip and int(row['effectiveTip'])==tip,(row['name'],'fee helper')
    # This compares actual burned base fee, not an accounting success flag.
    before=sum(int(a['balance']) for a in row['pre'].values());after=sum(int(a['balance']) for a in row['post'].values());assert before-after==used*100,(row['name'],'base fee burn')
    results.append(dict(name=row['name'],comparison='MATCH',stateRoot=actual,logHash=loghash(logs),gasUsed=used,baseFeeBurn=used*100,escrowTip=used*tip))
assert len(results)==18
json.dump(dict(scope='Actual signed synthetic transactions; full P3 NOT PASSED',cases=results,bootable=False),open(sys.argv[2],'w'),indent=2)
print('PASS18 actual signed second-engine state/root/gas/log/fee/burn/tip comparisons; full P3 NOT PASSED')
