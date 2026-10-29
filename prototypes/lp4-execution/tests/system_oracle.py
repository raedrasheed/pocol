"""Independent SYS-01 frame executions; synthetic profiles, not block acceptance.
Unmodified py-evm Cancun plus the separately implemented approved PAY hook.
No Rust execution or expected-state imports. Compare complete roots and gas.
"""
import json,sys,hashlib,rlp
from eth_utils import keccak
from pathlib import Path
from eth.db.atomic import AtomicDB
from eth.vm.chain_context import ChainContext
from eth.vm.forks.cancun.blocks import CancunBlockHeader
from eth.vm.message import Message
from eth.vm.transaction_context import BaseTransactionContext
from eth.vm.forks.spurious_dragon._utils import collect_touched_accounts
from pay_oracle import PayComputation,PayState,PayVM,ESCROW
from gst_control import root
SYSTEM=bytes([255])*19+b'\xfe'
def raw(v):return bytes.fromhex(v.removeprefix('0x'))
def number(v):return int(v,16) if isinstance(v,str) and v.startswith('0x') else int(v)
class Computation(PayComputation):opcodes=dict(PayComputation.opcodes)
def traced(op):
 def execute(computation):
  computation.state.counts[op]=computation.state.counts.get(op,0)+1
  if op in (0x54,0x55):
   key=computation._stack.values[-1];key=key if isinstance(key,int) else int.from_bytes(key,'big')
   computation.state.slots.setdefault((computation.msg.storage_address,op),[]).append(key)
  PayComputation.opcodes[op](computation=computation)
 return execute
for op in list(Computation.opcodes):Computation.opcodes[op]=traced(op)
class State(PayState):computation_class=Computation
class VM(PayVM):_state_class=State
source=Path(sys.argv[1]);vectors=json.loads(source.read_text());results=[]
assert vectors['semanticVersion']=='LP4-SYS-01' and vectors['bootable'] is False
spec=rlp.decode(raw(vectors['genesisSpec']));cp=[int.from_bytes(x,'big') for x in spec[2]]
assert len(cp)==52 and keccak(raw(vectors['genesisSpec']))==raw(vectors['genesisHash'])
assert int.from_bytes(spec[1],'big')==777902 and vectors['profileKind']=='Synthetic' and vectors['nodeKind']=='Simnet' and vectors['forkSchedule']==[[1,0]]
for sequence in vectors['sequences']:
 assert sequence['budget']==cp[44] and int(sequence['gasLimit'])==cp[30]
 if sequence['name'].startswith('compiled-registry'):
  slots=sequence['pre']['0x0000000000000000000000000000000000c0c004']['storage']
  assert all(number(slots.get(hex(i),'0x0'))==v for i,v in enumerate(cp)), 'ALL52 ChainCaps fields must match full identity'
 header=CancunBlockHeader(difficulty=0,block_number=int(sequence['height']),gas_limit=int(sequence['gasLimit']),timestamp=0,base_fee_per_gas=int(sequence['baseFee']),coinbase=ESCROW,excess_blob_gas=0)
 db=AtomicDB();s=VM.build_state(db,header,ChainContext(777902))
 for address,account in sequence['pre'].items():
  a=raw(address);s.set_nonce(a,account['nonce']);s.set_balance(a,int(account['balance']));s.set_code(a,raw(account['code']))
  for k,v in account['storage'].items():s.set_storage(a,number(k),number(v))
 s.persist();used=0
 for row in sequence['calls']:
  # Rebuild every epoch from persisted actual candidate; no warm/transient or
  # original-slot cache may carry. PAY has a separate complete epoch-start reader.
  current=s.state_root;s=VM.build_state(db,header.copy(state_root=current),ChainContext(777902));s.lock_changes()
  s._pay_original=State(s._db,s.execution_context,current);s.counts={};s.slots={}
  to=raw(row['to']);origin=raw(sequence['origin'])
  for a in [SYSTEM,to,ESCROW]+[n.to_bytes(20,'big') for n in range(1,11)]+([origin] if any(origin) else []):s.mark_address_warm(a)
  gas=min(30_000_000,sequence['budget']-used);assert gas>0
  message=Message(gas=gas,to=to,sender=SYSTEM,value=0,data=raw(row['input']),code=s.get_code(to))
  c=s.computation_class.apply_message(s,message,BaseTransactionContext(gas_price=0,origin=origin))
  assert not c.is_error,(sequence['name'],row['method'],str(c.error))
  assert c.get_gas_used()==row['rawGas'] and c.get_gas_refund()==row['refundCounter'],(sequence['name'],row['method'],'gas/refund',c.get_gas_used(),row['rawGas'])
  assert c.output==raw(row['output']) and not c.get_log_entries()
  used+=c.get_gas_used();assert used==row['systemUsed'] and used<=sequence['budget']
  for address,account in row['post'].items():
   a=raw(address);assert s.get_nonce(a)==account['nonce'] and s.get_balance(a)==int(account['balance']) and s.get_code(a)==raw(account['code']),(sequence['name'],address,'account',s.get_balance(a),account['balance'])
   keys=set(account['storage'])|set(sequence['pre'].get(address,{}).get('storage',{}))
   for k in keys:
    v=account['storage'].get(k,'0x0');assert s.get_storage(a,number(k))==number(v),(sequence['name'],address,k,s.get_storage(a,number(k)),number(v))
  # apply_message does not run the transaction finalizer. Apply only epoch
  # deletions/EIP161; never its nonce/fee/refund/intrinsic handlers.
  for account in c.get_accounts_for_deletion():s.delete_account(account)
  for account in collect_touched_accounts(c):
   if s.account_exists(account) and s.account_is_empty(account):s.delete_account(account)
  actual='0x'+s.make_state_root().hex();assert actual==row['stateRoot']==root({a:{**v,'balance':int(v['balance'])} for a,v in row['post'].items()}),(sequence['name'],row['method'],'root',actual,row['stateRoot'])
  # These are measured opcode counts, not a general resource bound or proof.
  trace={f'0x{a.hex()}:{op:02x}':[f'0x{k:064x}' for k in keys] for (a,op),keys in s.slots.items()}
  results.append(dict(sequence=sequence['name'],method=row['method'],rawGas=c.get_gas_used(),refundCounter=c.get_gas_refund(),allocation=gas,stateRoot=actual,opcodeCounts={f'{op:02x}':v for op,v in s.counts.items()},storageTrace=trace,logs=0,comparison='MATCH'))
  s.persist()
assert len(results)==13
report=dict(scope=__doc__,semanticVersion='LP4-SYS-01',profileIdentity={k:vectors[k] for k in ['genesisHash','genesisSpec','profileKind','nodeKind','forkSchedule','resourceModel']},cp=cp,inputSHA256=hashlib.sha256(source.read_bytes()).hexdigest(),comparisons=results,measuredOnly=True,generalR18='NOT PASSED',generalR21='NOT PASSED',caps64='NOT PASSED',validatedUTAndPhases='NOT PASSED',bootable=False)
Path(sys.argv[2]).write_text(json.dumps(report,indent=2)+'\n')
print(f'MATCH {len(results)} independently executed frames/full roots/raw gas/refunds; complete SYS/P2/P3/LP4 NOT PASSED')
