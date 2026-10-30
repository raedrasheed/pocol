"""Independent unmodified py-evm API diagnostics; no accepted SystemCall rule.
Compare real compiled bytecode, raw frame gas/refunds/output, full roots and old
unsigned control nonce effects. No signature, real block/phase or boot proof.
"""
import sys,json
from pathlib import Path
from eth.db.atomic import AtomicDB
from eth.db.chain import ChainDB
from eth.consensus.context import ConsensusContext
from eth.vm.forks.cancun import CancunVM
from eth.vm.forks.cancun.blocks import CancunBlockHeader
from eth.vm.chain_context import ChainContext
from eth.vm.message import Message
from eth.vm.transaction_context import BaseTransactionContext
from eth.vm.spoof import SpoofTransaction
from eth_hash.auto import keccak
from eth_utils import ValidationError
from gst_control import root

actual_path,artifact_path,output=map(Path,sys.argv[1:]);actual=json.loads(actual_path.read_text());artifact=json.loads(artifact_path.read_text())
code=bytes.fromhex(artifact['output']['contracts']['SystemEpochWitness.sol']['SystemEpochWitness']['evm']['deployedBytecode']['object']);system=(2**160-2).to_bytes(20,'big');target=b'\x22'*20;escrow=(0xc0c002).to_bytes(20,'big')
header=CancunBlockHeader(difficulty=0,block_number=1,timestamp=0,gas_limit=8000000,base_fee_per_gas=0,coinbase=escrow,mix_hash=bytes(32),excess_blob_gas=0)
pre={system.hex():dict(nonce=1,balance=1000000,code='',storage={}),target.hex():dict(nonce=1,balance=0,code=code.hex(),storage={'0x0':'0xb'})}
def raw(v):return bytes.fromhex(v.removeprefix('0x'))
def num(v):return int(v,16) if isinstance(v,str) and v.startswith('0x') else int(v)
def initial(pre,gaslimit=8000000,basefee=0):
 db=AtomicDB();s=CancunVM.build_state(db,header.copy(gas_limit=gaslimit,base_fee_per_gas=basefee),ChainContext(777902))
 for address,a in pre.items():
  address=raw(address);s.set_nonce(address,num(a['nonce']));s.set_balance(address,num(a['balance']));s.set_code(address,raw(a['code']))
  for k,v in a['storage'].items():s.set_storage(address,num(k),num(v))
 s.persist();s.lock_changes();return db,s
def prepare(s):
 for a in [system,target,escrow]:s.mark_address_warm(a)
def execute(s,address,name,gas=100000,origin=system):
 msg=Message(gas=gas,to=address,sender=system,value=0,data=keccak(name.encode())[:4],code=s.get_code(address));c=s.computation_class.apply_message(s,msg,BaseTransactionContext(gas_price=0,origin=origin));assert not c.is_error,(name,type(c.error).__name__ if c.is_error else '')
 return dict(method=name,gasSpentBeforeRefund=c.get_gas_used(),refundCounter=c.get_gas_refund(),output='0x'+c.output.hex(),success=not c.is_error,systemNonce=s.get_nonce(system),slot0=str(s.get_storage(address,0)),slot1=str(s.get_storage(address,1)))
def compare(expected,got):
 for key in ['method','gasSpentBeforeRefund','refundCounter','output','success','systemNonce','slot0','slot1']:assert got[key]==expected[key],(key,expected,got)
results=[]
for model in ['shared-frame-epoch','fresh-frame-epoch']:
 db,s=initial(pre);prepare(s)
 for row in [x for x in actual['rows'] if x['model']==model]:
  got=execute(s,target,row['result']['method']);compare(row['result'],got);results.append(dict(model=model,**got))
  if model=='fresh-frame-epoch':
   s.persist();current=s.state_root;s=CancunVM.build_state(db,header.copy(state_root=current),ChainContext(777902));s.lock_changes();prepare(s)
 expected_state=actual['sharedState' if model=='shared-frame-epoch' else 'freshState'];assert '0x'+s.make_state_root().hex()==root(expected_state)
for row in [x for x in actual['rows'] if x['model']=='explicit-origin-fixed-SYSTEM-caller']:
 db,s=initial(pre);prepare(s);got=execute(s,target,'originAndCaller()',origin=raw(row['origin']));compare(row['result'],got);results.append(dict(model=row['model'],**got))
for model,gas,basefee in [('direct-frame30M-under-8M-GASLIMIT',30000000,0),('direct-frame-zero-price-basefee100',100000,100)]:
 row=next(x for x in actual['rows'] if x['model']==model);db,s=initial(pre,basefee=basefee);prepare(s);got=execute(s,target,'readSlot()',gas=gas);compare(row['result'],got);results.append(dict(model=model,**got))
tx=CancunVM.get_transaction_builder().create_unsigned_transaction(nonce=1,gas_price=0,gas=30000000,to=target,value=0,data=keccak(b'readSlot()')[:4])
validation_vm=CancunVM(header,ChainDB(db),ChainContext(777902),ConsensusContext(db))
try:validation_vm.validate_transaction_against_header(header,tx)
except ValidationError as e:assert 'gas limit' in str(e).lower()
else:raise AssertionError('Independent header validation must reject30M under8M')
db,s=initial(pre,basefee=100);tx=CancunVM.get_transaction_builder().create_unsigned_transaction(nonce=1,gas_price=0,gas=100000,to=target,value=0,data=keccak(b'readSlot()')[:4])
try:s.validate_transaction(SpoofTransaction(tx,from_=system))
except ValidationError as e:assert 'base fee' in str(e).lower()
else:raise AssertionError('Independent state validation must reject zero price below baseFee100')
db,s=initial(pre);prepare(s);tx=CancunVM.get_transaction_builder().create_unsigned_transaction(nonce=1,gas_price=0,gas=100000,to=target,value=0,data=keccak(b'readSlot()')[:4]);c=s.apply_transaction(SpoofTransaction(tx,from_=system));ordinary=next(x for x in actual['rows'] if x['model']=='ordinary-tx');assert CancunVM.finalize_gas_used(tx,c)==ordinary['gasUsed'] and s.get_nonce(system)==ordinary['systemNonce']==2
# Real compiled Registry direct calls use the same independently built state.
db,s=initial(actual['registryPreState']);prepare(s);reg=(0xc0c001).to_bytes(20,'big')
for row in [x for x in actual['rows'] if x['model']=='real-compiled-Registry-direct-frame']:
 s.mark_address_warm(reg);got=execute(s,reg,row['result']['method'],gas=30000000);compare(row['result'],got);assert '0x'+s.make_state_root().hex()==root(row['state']);results.append(dict(model=row['model'],**got));s.persist();s=CancunVM.build_state(db,header.copy(state_root=s.state_root),ChainContext(777902));s.lock_changes();prepare(s)
db,s=initial(actual['registryPreState'],gaslimit=30000000);tx=CancunVM.get_transaction_builder().create_unsigned_transaction(nonce=1,gas_price=0,gas=30000000,to=reg,value=0,data=keccak(b'markClose()')[:4]);c=s.apply_transaction(SpoofTransaction(tx,from_=system));old=next(x for x in actual['rows'] if x['model']=='real-compiled-Registry-old-unsigned-tx-control');assert CancunVM.finalize_gas_used(tx,c)==old['gasUsed'] and s.get_nonce(system)==old['systemNonce']==2 and '0x'+s.make_state_root().hex()==root(old['state'])
result=dict(scope=__doc__,matchedFrameExecutions=len(results),matchedUnsignedControls=2,independentValidationRejections=2,sharedRoot=root(actual['sharedState']),freshRoot=root(actual['freshState']),rootsDiffer=root(actual['sharedState'])!=root(actual['freshState']),results=results,allRowsUnapproved=all(not row.get('acceptedRule',row.get('result',{}).get('acceptedRule',True)) for row in actual['rows']),status='DIAGNOSTIC ONLY; proposed rule NOT APPROVED; no SystemCall/P2/P3/LP4 acceptance',bootable=False);assert result['rootsDiffer'] and result['allRowsUnapproved'] and len(results)+4==len(actual['rows']);output.write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({k:result[k] for k in ['matchedFrameExecutions','matchedUnsignedControls','independentValidationRejections','rootsDiffer','allRowsUnapproved']}))
