"""Independent LP4-CODE-01 signed execution fixtures, not full integration.

py-evm source is unmodified. A local computation subclass replaces ONLY runtime
validation with the approved runtime/logical limits; EIP3860 stays upstream.
Independent complete-address inventories use actual get_code, including addresses
seen at nested creation. Public synthetic signer comes from wire_fixtures; no
credentials, private key fields, RPC or network are emitted/used.
"""
import json,sys,rlp,hashlib,re
from pathlib import Path
from eth.db.atomic import AtomicDB
from eth.db.chain import ChainDB
from eth.consensus.context import ConsensusContext
from eth.vm.chain_context import ChainContext
from eth.vm.forks.cancun import CancunVM
from eth.vm.forks.cancun.computation import CancunComputation
from eth.vm.forks.cancun.state import CancunState
from eth.vm.forks.cancun.blocks import CancunBlockHeader
from eth.exceptions import OutOfGas,ReservedBytesInCode
from eth_hash.auto import keccak
from eth_keys import keys

B=CancunVM.get_transaction_builder()
# Public scalar1, used only in synthetic local test states; never broadcast.
K=keys.PrivateKey((1).to_bytes(32,'big'));CHAIN=777902;TARGET=bytes.fromhex('22'*20)

HERE=Path(__file__).parent
artifact=json.loads((HERE/'data/code-caps-witness.json').read_text())
factory=bytes.fromhex(artifact['output']['contracts']['CodeCapsWitness.sol']['CodeCapsWitness']['evm']['deployedBytecode']['object'])
sender=K.public_key.to_canonical_address();escrow=(0xc0c002).to_bytes(20,'big')
base=rlp.decode(bytes.fromhex((HERE/'data/execution-profile.hex').read_text().strip()))
rows=[]

def word(n):return n.to_bytes(32,'big')
def dynamic(b):return word(len(b))+b+bytes(-len(b)%32)
def init(n,byte=0):
    # RETURN zero-filled memory; EF prefix is set with MSTORE8 when requested.
    return (bytes.fromhex('60ef5f53') if byte==0xef else b'')+b'\x61'+n.to_bytes(2,'big')+bytes.fromhex('5ff3')
def make(code,use2=False,undo=False):
    return keccak(b'make(bytes,uint256,bool,bool)')[:4]+word(128)+word(7)+word(use2)+word(undo)+dynamic(code)
def twice(code):return keccak(b'twice(bytes)')[:4]+word(32)+dynamic(code)
def destroy_make(address,code):return keccak(b'destroyThenMake(address,bytes)')[:4]+address.rjust(32,b'\0')+word(64)+dynamic(code)
def create_destroy_make(first,second):
    a=dynamic(first)
    return keccak(b'makeDestroyMake(bytes,bytes)')[:4]+word(64)+word(64+len(a))+a+dynamic(second)

def run(name,cap,data,to=b'',logical=2**30,extras=(),gas=8000000,expected=None,kind=2):
    addresses={sender,escrow};pre={sender:dict(balance=10**10,nonce=1,code=b'')}
    if to:pre[TARGET]=dict(balance=0,nonce=1,code=factory);addresses.add(TARGET)
    for address,code in extras:pre[address]=dict(balance=0,nonce=1,code=code);addresses.add(address)
    class Computation(CancunComputation):
        @classmethod
        def apply_create_message(cls,state,message,transaction_context,parent_computation=None):
            addresses.add(message.storage_address)
            return super().apply_create_message(state,message,transaction_context,parent_computation)
        @classmethod
        def validate_contract_code(cls,code):
            if code[:1]==b'\xef':raise ReservedBytesInCode('LP4 unchanged EIP3541')
            if len(code)>cap:raise OutOfGas('LP4 B_code_max')
            # Current computation carries state at the ordinary deposit point.
            total=sum(len(active_state.get_code(a)) for a in addresses)
            if total+len(code)>logical:raise OutOfGas('LP4 logical CODE_CAP')
    State=CancunState.configure(__name__='LP4CodeState',computation_class=Computation)
    VM=CancunVM.configure(__name__='LP4CodeVM',_state_class=State)
    h=CancunBlockHeader(difficulty=0,block_number=300,timestamp=12345,gas_limit=8000000,base_fee_per_gas=100,coinbase=escrow,mix_hash=b'\x77'*32,excess_blob_gas=0)
    db=AtomicDB();vm=VM(h,ChainDB(db),ChainContext(CHAIN),ConsensusContext(db));active_state=vm.state
    for a,p in pre.items():active_state.set_balance(a,p['balance']);active_state.set_nonce(a,p['nonce']);active_state.set_code(a,p['code'])
    active_state.persist();pre_root=active_state.make_state_root().hex()
    args=dict(nonce=1,gas=gas,to=to,value=0,data=data)
    if kind==0:tx=B.create_unsigned_transaction(gas_price=200,**args).as_signed_transaction(K,chain_id=CHAIN)
    elif kind==1:tx=B.new_unsigned_access_list_transaction(chain_id=CHAIN,gas_price=200,access_list=(),**args).as_signed_transaction(K)
    else:tx=B.new_unsigned_dynamic_fee_transaction(chain_id=CHAIN,max_priority_fee_per_gas=10,max_fee_per_gas=300,access_list=(),**args).as_signed_transaction(K)
    fields=list(base);fields[1]=CHAIN.to_bytes((CHAIN.bit_length()+7)//8,'big');cp=list(fields[2]);cp[34]=cap.to_bytes((cap.bit_length()+7)//8,'big');cp[35]=logical.to_bytes((logical.bit_length()+7)//8,'big');fields[2]=cp;spec=rlp.encode(fields)
    row=dict(name=name,runtime=cap,logical=logical,gasLimit=gas,genesisSpec=spec.hex(),genesisHash=keccak(spec).hex(),raw=tx.encode().hex(),rawSHA256=hashlib.sha256(tx.encode()).hexdigest(),pre={a.hex():dict(balance=str(p['balance']),nonce=p['nonce'],code=p['code'].hex()) for a,p in pre.items()},preRoot=pre_root,bootable=False)
    try:r,c=vm.apply_transaction(h,tx)
    except Exception as error:
        # Only EIP3860 admission is expected here; do not turn arbitrary engine
        # failures into accepted negative cases.
        assert len(data)>49152 and to==b'' and '49152' in str(error),(name,type(error).__name__,str(error))
        # This library raises after mutating its isolated candidate on oversized
        # initcode. Preserve that observation, discard the entire candidate and
        # independently rebuild the pre-state. Do not claim upstream atomicity.
        discarded=active_state.make_state_root().hex()
        fresh=VM(h,ChainDB(AtomicDB()),ChainContext(CHAIN),ConsensusContext(AtomicDB())).state
        for a,p in pre.items():fresh.set_balance(a,p['balance']);fresh.set_nonce(a,p['nonce']);fresh.set_code(a,p['code'])
        assert fresh.make_state_root().hex()==pre_root
        row.update(admissionError='CreateInitCodeSizeLimit',stateRoot=pre_root,independentCandidateDiscarded=True,independentDiscardedRoot=discarded)
    else:
        row.update(success=not c.is_error,gasUsed=r.gas_used,receipt=r.encode().hex(),output=c.output.hex(),stateRoot=active_state.make_state_root().hex(),post={a.hex():dict(exists=active_state.account_exists(a),balance=str(active_state.get_balance(a)),nonce=active_state.get_nonce(a),code=active_state.get_code(a).hex()) for a in sorted(addresses)},logicalAfter=sum(len(active_state.get_code(a)) for a in addresses))
        if expected is not None:assert row['success']==expected,(name,row)
    if name in ['retained-selfdestruct','created-selfdestruct-no-early-credit']:
        # A second real transaction proves charge is released only after actual
        # deletion at commit; pre-existing Cancun SELFDESTRUCT never releases it.
        next_tx=B.new_unsigned_dynamic_fee_transaction(chain_id=CHAIN,nonce=2,max_priority_fee_per_gas=10,max_fee_per_gas=300,gas=100000,to=TARGET,value=0,data=make(init(10)),access_list=()).as_signed_transaction(K)
        next_receipt,next_computation=vm.apply_transaction(h.copy(gas_used=row['gasUsed']),next_tx)
        assert not next_computation.is_error
        assert any(next_computation.output)==(name=='created-selfdestruct-no-early-credit')
        row['followUp']=dict(raw=next_tx.encode().hex(),rawSHA256=hashlib.sha256(next_tx.encode()).hexdigest(),gasUsed=next_receipt.gas_used,receipt=next_receipt.encode().hex(),stateRoot=active_state.make_state_root().hex(),logicalAfter=sum(len(active_state.get_code(a)) for a in addresses))
    rows.append(row);return row

for cap in [24577,32768]:
    for n in dict.fromkeys([0,24576,24577,cap,cap+1]):
        run(f'top-runtime-{cap}-{n}',cap,init(n),expected=n<=cap)
        for use2 in [False,True]:run(f'nested-{int(use2)}-runtime-{cap}-{n}',cap,make(init(n),use2),TARGET,expected=True)
    for n in [49151,49152,49153]:
        code=init(1).ljust(n,b'\0');run(f'top-init-{cap}-{n}',cap,code)
        for use2 in [False,True]:run(f'nested-{int(use2)}-init-{cap}-{n}',cap,make(code,use2),TARGET,expected=n<=49152)
    run(f'ef-{cap}',cap,init(1,0xef),expected=False)
    run(f'nested-undo-{cap}',cap,make(init(cap),undo=True),TARGET,expected=False)
    good=run(f'gas-probe-{cap}',cap,init(cap),expected=True)
    for delta in [-1,0]:run(f'gas-boundary-{cap}-{delta}',cap,init(cap),gas=good['gasUsed']+delta,expected=delta==0)

for kind in [0,1]:
    for n in [32768,32769]:run(f'type{kind}-runtime-{n}',32768,init(n),kind=kind,expected=n==32768)
    run(f'type{kind}-init-49153',32768,init(1).ljust(49153,b'\0'),kind=kind)

for delta in [-1,0,1]:
    run(f'logical-top-{delta}',32768,init(10),logical=20+delta,extras=[(b'\x33'*20,b'\0'*10)],expected=delta>=0)
    # Two addresses sharing exactly the same code must each count, not dedup.
    run(f'logical-shared-{delta}',32768,init(10),logical=30+delta,extras=[(b'\x33'*20,b'\0'*10),(b'\x44'*20,b'\0'*10)],expected=delta>=0)
    run(f'logical-nested-{delta}',32768,make(init(10)),TARGET,logical=len(factory)+10+delta,expected=True)
    run(f'logical-twice-{delta}',32768,twice(init(10)),TARGET,logical=len(factory)+20+delta,expected=True)

run('logical-nested-undo',32768,make(init(10),undo=True),TARGET,logical=len(factory)+10,expected=False)
# Existing Cancun SELFDESTRUCT retains its code, hence releases no charge.
suicide=b'\x73'+sender+b'\xff'
run('retained-selfdestruct',32768,destroy_make(b'\x33'*20,init(10)),TARGET,logical=len(factory)+len(suicide)+9,extras=[(b'\x33'*20,suicide)],expected=True)
# Freshly created code is still observable before actual end-of-tx deletion.
first=b'\x60'+bytes([len(suicide)])+b'\x60\x0c\x60\x00\x39\x60'+bytes([len(suicide)])+b'\x60\x00\xf3'+suicide
run('created-selfdestruct-no-early-credit',32768,create_destroy_make(first,init(10)),TARGET,logical=len(factory)+len(suicide)+9,expected=True)

assert len(rows)==len({r['name'] for r in rows})
result=dict(scope='LP4-CODE-01 independent synthetic signed execution; complete GenesisBuilder/resources/import/crash/reorg still NOT PASSED; modified local computation subclass, upstream source unchanged; small logical-cap profiles are schema-bound synthetic controls, not ParamGate-valid boot profiles',factoryBytes=len(factory),cases=rows,bootable=False)
def pack(v):
    # Transparent lossless zero-byte runs keep synthetic padded initcode/runtime
    # readable in git. All signed bytes reconstruct exactly; no cases omitted.
    if isinstance(v,list):return [pack(x) for x in v]
    if isinstance(v,dict):return {k:pack(x) for k,x in v.items()}
    if isinstance(v,str) and len(v)%2==0 and re.fullmatch('[0-9a-f]+',v):
        matches=[m for m in re.finditer(r'(?:00){32,}',v) if m.start()%2==0]
        if matches:
            runs=[];start=0
            for m in matches:runs.extend([v[start:m.start()],len(m[0])//2]);start=m.end()
            runs.append(v[start:]);return {'hexRuns':runs}
    return v
def unpack(v):
    if isinstance(v,list):return [unpack(x) for x in v]
    if isinstance(v,dict):
        if set(v)=={'hexRuns'}:return ''.join('00'*x if isinstance(x,int) else x for x in v['hexRuns'])
        return {k:unpack(x) for k,x in v.items()}
    return v
result['fixtureEncoding']='hexRuns: alternating literal hex / integer zero-byte run; exact reversible signed bytes'
packed=pack(result);assert unpack(packed)==result
if len(sys.argv)>2:Path(sys.argv[2]).write_text(json.dumps(result,indent=2)+'\n')
Path(sys.argv[1]).write_text(json.dumps(packed,indent=2)+'\n');print(json.dumps(dict(cases=len(rows),scope=result['scope'])))
