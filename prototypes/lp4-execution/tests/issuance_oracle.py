"""Independent actual genesis MPT and signed USER/native issuance/SYS execution.
Synthetic manifest has one witness role; Escrow settlement/finalization incomplete; not real five-program/UT/P5/P6 acceptance.
"""
import sys,json,copy,hashlib,rlp
from pathlib import Path
from eth_utils import keccak
from eth.db.atomic import AtomicDB
from eth.vm.chain_context import ChainContext
from eth.vm.forks.cancun.blocks import CancunBlockHeader
from eth.vm.message import Message
from eth.vm.transaction_context import BaseTransactionContext
from eth.vm.forks.spurious_dragon._utils import collect_touched_accounts
from pay_oracle import PayVM,PayState,PayComputation,ESCROW
class TraceComputation(PayComputation):opcodes=dict(PayComputation.opcodes)
def traced(op):
 def execute(computation):
  computation.state.opcodes[op]=computation.state.opcodes.get(op,0)+1
  if op in (0x54,0x55):
   key=computation._stack.values[-1];key=key if isinstance(key,int) else int.from_bytes(key,'big')
   computation.state.slots.setdefault((computation.msg.storage_address,op),[]).append(key)
  PayComputation.opcodes[op](computation=computation)
 return execute
for op in list(TraceComputation.opcodes):TraceComputation.opcodes[op]=traced(op)
class TraceState(PayState):computation_class=TraceComputation
class TraceVM(PayVM):_state_class=TraceState
from gst_control import root
SYSTEM=bytes([255])*19+b'\xfe';REG=(0xc0c001).to_bytes(20,'big');CAPS=(0xc0c004).to_bytes(20,'big')
def raw(s):return bytes.fromhex(s.removeprefix('0x'))
def number(s):return int(s,16) if isinstance(s,str) and s.startswith('0x') else int(s)
def world_root(v):return root({a:{**x,'balance':number(x['balance'])} for a,x in v.items()})
def clean(v):
 return {a:{'nonce':number(x['nonce']),'balance':number(x['balance']),'code':raw(x['code']).hex(),'storage':{hex(number(k)):number(v) for k,v in x['storage'].items() if number(v)}} for a,x in v.items()}
source=Path(sys.argv[1]);v=json.loads(source.read_text());assert v['bootable'] is False and v['semanticVersion']==['LP4-SYS-01','LP4-ISSUANCE-01'];genesis=[]
for g in v['genesis']:
 spec=rlp.decode(raw(g['spec']));cp=[int.from_bytes(x,'big') for x in spec[2]];assert len(cp)==52 and len(spec[4])==g['members'];assert keccak(raw(g['spec']))==raw(g['genesisHash']);programs=list(map(raw,g['programs']));assert keccak(rlp.encode(programs+[b'']))==spec[5]
 state=copy.deepcopy(g['alloc']);alloc_sum=sum(number(x['balance']) for x in state.values());assert alloc_sum+len(spec[4])*cp[16]+cp[15]*cp[45]<2**127
 for i,code in enumerate(programs):
  addr='0x'+(0xc0c001+i).to_bytes(20,'big').hex();state.setdefault(addr,dict(nonce=0,balance=0,code='0x',storage={}));state[addr]['code']='0x'+code.hex()
 def put(addr,k,value):state['0x'+addr.hex()]['storage'][hex(k)]=hex(value)
 for i,value in enumerate(cp):put(CAPS,i,value)
 bonds=len(spec[4])*cp[16]
 for k,value in [(0,len(spec[4])),(1,len(spec[4])),(2,0),(3,0),(4,bonds)]:put(REG,k,value)
 recarr=int.from_bytes(keccak((5).to_bytes(32,'big')),'big');actarr=int.from_bytes(keccak((6).to_bytes(32,'big')),'big')
 for i,(miner,reward) in enumerate(spec[4]):
  a=state['0x'+reward.hex()];assert number(a['nonce'])>0 or raw(a['code'])
  slot=int.from_bytes(keccak(miner.rjust(32,b'\0')+(7).to_bytes(32,'big')),'big')
  for k,value in [(0,1+(1<<16)+(i<<24)+(i<<56)),(1,int.from_bytes(reward,'big')),(2,cp[16]),(3,0)]:put(REG,slot+k,value)
  put(REG,recarr+i,int.from_bytes(miner,'big'));put(REG,actarr+i,int.from_bytes(miner,'big'))
 state['0x'+ESCROW.hex()]['balance']=str(number(state['0x'+ESCROW.hex()]['balance'])+bonds)
 actual=world_root(state);assert actual==g['root']=='0x'+spec[3].hex() and clean(state)==clean(g['post']);genesis.append(dict(members=g['members'],root=actual,heldTotal=bonds,codeLogicalBytes=sum(len(raw(a['code'])) for a in state.values()),nonzeroSlots=sum(len(a['storage']) for a in clean(state).values())))
base=v['genesis'][0];spec=rlp.decode(raw(base['spec']));cp=[int.from_bytes(x,'big') for x in spec[2]];results=[]
header=CancunBlockHeader(difficulty=0,block_number=1,gas_limit=cp[30],timestamp=0,base_fee_per_gas=100,coinbase=ESCROW,excess_blob_gas=0)
def initial():
 db=AtomicDB();s=TraceVM.build_state(db,header,ChainContext(int.from_bytes(spec[1],'big')))
 for addr,a in base['post'].items():
  addr=raw(addr);s.set_nonce(addr,a['nonce']);s.set_balance(addr,number(a['balance']));s.set_code(addr,raw(a['code']))
  for k,value in a['storage'].items():s.set_storage(addr,number(k),number(value))
 s.persist();return db,s
def epoch(db,s):
 s.persist();s=TraceVM.build_state(db,header.copy(state_root=s.state_root),ChainContext(777902));s.lock_changes();s._pay_original=TraceState(s._db,s.execution_context,s.state_root);s.opcodes={};s.slots={};return s
def message(s,to,input,origin,gas):
 for a in [SYSTEM,to,ESCROW]+[n.to_bytes(20,'big') for n in range(1,11)]+([origin] if any(origin) else []):s.mark_address_warm(a)
 c=s.computation_class.apply_message(s,Message(gas=gas,to=to,sender=SYSTEM,value=0,data=input,code=s.get_code(to)),BaseTransactionContext(gas_price=0,origin=origin));assert not c.is_error,str(c.error) if c.is_error else '';assert not c.get_log_entries()
 for a in c.get_accounts_for_deletion():s.delete_account(a)
 for a in collect_touched_accounts(c):
  if s.account_exists(a) and s.account_is_empty(a):s.delete_account(a)
 return c
for row in v['cases']:
 db,s=initial();origin=raw(row['origin']);used=0
 for call in row['phase0']:
  s=epoch(db,s);c=message(s,REG,keccak(call['method'].encode())[:4],origin,min(30_000_000,cp[44]-used));used+=c.get_gas_used();assert c.get_gas_used()==call['rawGas'] and c.get_gas_refund()==call['refundCounter'] and '0x'+s.make_state_root().hex()==call['root']
 s=epoch(db,s);tx=PayVM.get_transaction_builder().decode(raw(row['raw']));tx.validate();assert tx.chain_id==777902
 # Actual signature recovery, nonce/fees/receipt rules are library execution.
 computation=s.apply_transaction(tx);assert not computation.is_error;gas=PayVM.finalize_gas_used(tx,computation);tip=gas*3;assert gas==row['user']['gasUsed'] and tip==int(row['user']['tips']) and '0x'+s.make_state_root().hex()==row['user']['root']
 receipt=rlp.encode([1,gas,bytes(256),[]]);receipt=bytes([row['kind']])+receipt if row['kind'] else receipt;assert receipt.hex()==row['user']['receipts'][0]
 # Protocol credit ONLY now, after USER, before fresh SYSTEM/PAY epoch.
 s.delta_balance(ESCROW,cp[15]);s=epoch(db,s);c=message(s,ESCROW,raw(row['phase2']['input']),origin,min(30_000_000,cp[44]-used));used+=c.get_gas_used();actual='0x'+s.make_state_root().hex()
 assert c.get_gas_used()==row['phase2']['rawGas'] and c.get_gas_refund()==row['phase2']['refundCounter'] and used==row['phase2']['sysUsed'] and cp[15]==int(row['phase2']['issued']) and actual==row['phase2']['root']==world_root(row['phase2']['post'])
 for addr,a in row['phase2']['post'].items():
  addr=raw(addr);assert s.get_nonce(addr)==a['nonce'] and s.get_balance(addr)==number(a['balance']) and s.get_code(addr)==raw(a['code'])
  for k,value in a['storage'].items():assert s.get_storage(addr,number(k))==number(value)
 trace={f'0x{addr.hex()}:{op:02x}':[hex(k) for k in keys] for (addr,op),keys in s.slots.items()}
 results.append(dict(phase2OpcodeCounts={hex(k):n for k,n in s.opcodes.items()},phase2StorageTrace=trace,kind=row['kind'],origin=row['origin'],stateRoot=actual,userGas=gas,authenticatedTips=tip,sysRawGas=used,phase2RawGas=c.get_gas_used(),issued=cp[15],comparison='MATCH'))
assert len(genesis)==2 and len(results)==8
out=dict(scope=__doc__,inputSHA256=hashlib.sha256(source.read_bytes()).hexdigest(),genesis=genesis,cases=results,semanticVersion=v['semanticVersion'],fullGenesisBuilderAcceptance='NOT PASSED: one synthetic manifest role; Escrow settlement/finalization incomplete',fullValidatedPhases='NOT PASSED',realFinalizationSV6='NOT PASSED',generalR18R21caps64='NOT PASSED',bootable=False)
Path(sys.argv[2]).write_text(json.dumps(out,indent=2)+'\n');print('MATCH actual genesis roots4/1024 and8 signed USER/issuance/compiled-SYS full-state/gas cases; full LP4 NOT PASSED')
