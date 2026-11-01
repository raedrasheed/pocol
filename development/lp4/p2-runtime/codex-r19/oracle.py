"""Independent compiled forced-surplus/bond execution, no full settlement acceptance."""
import json,sys
from eth.db.atomic import AtomicDB
from eth.vm.chain_context import ChainContext
from eth.vm.forks.cancun.blocks import CancunBlockHeader
from eth.vm.spoof import SpoofTransaction
from pay_oracle import PayVM,ESCROW
from gst_control import root
rows=json.load(open(sys.argv[1]));results=[]
assert len(rows)==22
for row in rows:
 h=CancunBlockHeader(difficulty=0,block_number=row['height'],timestamp=0,gas_limit=30000000,base_fee_per_gas=0,coinbase=ESCROW,excess_blob_gas=0);state=PayVM.build_state(AtomicDB(),h,ChainContext(777902))
 for address,a in row['pre'].items():
  address=bytes.fromhex(address[2:]);state.set_balance(address,int(a['balance']));state.set_nonce(address,a['nonce']);state.set_code(address,bytes.fromhex(a['code'][2:]))
  for k,v in a['storage'].items():state.set_storage(address,int(k,16),int(v,16))
 caller=bytes.fromhex(row['caller'][2:]);tx=PayVM.create_unsigned_transaction(nonce=row['pre'][row['caller']]['nonce'],gas_price=0,gas=30000000,to=bytes.fromhex(row['to'][2:]),value=row['value'],data=bytes(row['input']));c=state.apply_transaction(SpoofTransaction(tx,from_=caller));used=tx.gas-c.get_gas_remaining();used-=min(c.get_gas_refund(),used//5)
 assert (not c.is_error)==row['success'] and used==row['gasUsed'] and list(c.output)==row['output'] and len(c.get_log_entries())==row['logs'],(row['name'],'execution/output/gas')
 actual='0x'+state.make_state_root().hex();expected=root({a:{**v,'balance':int(v['balance'])} for a,v in row['post'].items()});assert actual==expected,(row['name'],'complete root',actual,expected)
 results.append(dict(name=row['name'],comparison='MATCH',stateRoot=actual,gasUsed=used))
json.dump(dict(scope='22actual compiled forced-transfer/ordinary/closing bond path comparisons only; synthetic complete snapshots, no Genesis/settlement/caps/resource acceptance',cases=results,bootable=False),open(sys.argv[2],'w'),indent=2)
print('PASS22 independent full-state-root/output/gas/compiled surplus-path comparisons; full P2/T2b/LP4 NOT PASSED')
