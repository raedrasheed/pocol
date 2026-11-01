"""Actual compiled EVM counterexample; neither retention policy is approved.
One-owner synthetic state only, not full ShareLedger or an acceptance gate.
"""
import json,sys,hashlib
from pathlib import Path
from eth_utils import keccak
from eth.db.atomic import AtomicDB
from eth.vm.chain_context import ChainContext
from eth.vm.forks.cancun import CancunVM
from eth.vm.forks.cancun.blocks import CancunBlockHeader
from eth.vm.message import Message
from eth.vm.transaction_context import BaseTransactionContext
artifact=Path(sys.argv[1]);v=json.loads(artifact.read_text());code=bytes.fromhex(v['output']['contracts']['LedgerRetentionCountermodel.sol']['LedgerRetentionCountermodel']['evm']['deployedBytecode']['object'])
SYSTEM=bytes([255])*19+b'\xfe';TARGET=bytes([0x22])*20;ESCROW=(0xc0c002).to_bytes(20,'big')
header=CancunBlockHeader(difficulty=0,block_number=5,gas_limit=8_000_000,timestamp=0,base_fee_per_gas=0,coinbase=ESCROW,excess_blob_gas=0)
def slot(j):return int.from_bytes(keccak(j.to_bytes(32,'big')+bytes(32)),'big')
rows=[]
for extra in [False,True]:
 state=CancunVM.build_state(AtomicDB(),header,ChainContext(777902));state.set_nonce(TARGET,1);state.set_code(TARGET,code)
 for j,n in [(1,7),(2,11),(3,13)]:state.set_storage(TARGET,slot(j),n)
 state.persist();state.lock_changes();before='0x'+state.make_state_root().hex()
 for a in [SYSTEM,TARGET,ESCROW]+[i.to_bytes(20,'big') for i in range(1,11)]:state.mark_address_warm(a)
 calldata=keccak(b'prune(bool)')[:4]+int(extra).to_bytes(32,'big')
 c=state.computation_class.apply_message(state,Message(gas=2_000_000,to=TARGET,sender=SYSTEM,value=0,data=calldata,code=code),BaseTransactionContext(gas_price=0,origin=bytes(20)))
 assert not c.is_error and not c.get_log_entries() and not state.account_exists(SYSTEM)
 counts={str(j):state.get_storage(TARGET,slot(j)) for j in [1,2,3]};assert counts['2']==11 and counts['3']==13
 rows.append(dict(policy='retain-one-extra' if extra else 'minimal-needed-window',approved=False,input='0x'+calldata.hex(),beforeRoot=before,afterRoot='0x'+state.make_state_root().hex(),rawFrameGas=c.get_gas_used(),refundCounter=c.get_gas_refund(),counts=counts,parentAssignmentKnownWeight=31,currentSettlementCount=13,nextAssignmentKnownWeight=24))
assert rows[0]['beforeRoot']==rows[1]['beforeRoot'] and rows[0]['afterRoot']!=rows[1]['afterRoot']
assert rows[0]['counts']['1']==0 and rows[1]['counts']['1']==7
Path(sys.argv[2]).write_text(json.dumps(dict(scope=__doc__,artifactSHA256=hashlib.sha256(artifact.read_bytes()).hexdigest(),height=5,D=2,W_w=3,cases=rows,rootsDiffer=True,decision='NOT APPROVED',bootable=False),indent=2)+'\n')
print('Verified same compiled code/prestate, two retention hypotheses preserve needed counts but change state roots; neither is approved; boot=false')
