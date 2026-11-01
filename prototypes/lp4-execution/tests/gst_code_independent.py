"""Independent amended execution of EACH enumerated CODE-01 corpus delta.
Local explicit py-evm subclasses, upstream source unchanged. Unsigned/spoof
comparison, not signed-wire, ParamGate or full PoCol system execution acceptance.
"""
import sys,json,rlp,hashlib
from pathlib import Path
from eth.exceptions import OutOfGas,ReservedBytesInCode
from eth_utils import ValidationError
from gst_independent import execute
from gst_independent_vm import AdmissionComparisonState,ComparisonComputation
from eth.vm.forks.cancun import CancunVM

corpus,delta_dir,profile,output=map(Path,sys.argv[1:]);output.mkdir(parents=True,exist_ok=False)
summary=json.loads((delta_dir/'summary.json').read_text());assert summary['total']==20661 and summary['excluded']==summary['blocked']==summary['runnerFailure']==0
spec=rlp.decode(bytes.fromhex(profile.read_text().strip()));runtime=int.from_bytes(spec[2][34],'big');logical=int.from_bytes(spec[2][35],'big');assert runtime>=24577
class CodeSnapshotRejected(ValidationError):pass
class CodeState(AdmissionComparisonState):
    def __init__(self,*args,**kwargs):
        self._code_addresses=set()
        super().__init__(*args,**kwargs)
    def set_code(self,address,code):
        self._code_addresses.add(address)
        return super().set_code(address,code)
    def validate_transaction(self,transaction):
        # Independently inspect the fully constructed actual VM state. These
        # source fixtures may contain pre-existing 50000-byte runtime, which
        # cannot be an admitted CODE-01 snapshot. This is not GenesisBuilder.
        lengths=[len(self.get_code(a)) for a in self._code_addresses]
        if any(n>runtime for n in lengths):raise CodeSnapshotRejected('snapshot exceeds B_code_max')
        if sum(lengths)>logical:raise CodeSnapshotRejected('snapshot exceeds CODE_CAP')
        return super().validate_transaction(transaction)
class CodeComputation(ComparisonComputation):
    active=[]
    @classmethod
    def apply_create_message(cls,state,message,transaction_context,parent_computation=None):
        state._code_addresses.add(message.storage_address);cls.active.append(state)
        try:return super().apply_create_message(state,message,transaction_context,parent_computation)
        finally:cls.active.pop()
    @classmethod
    def validate_contract_code(cls,code):
        if code[:1]==b'\xef':raise ReservedBytesInCode('LP4 unchanged EIP3541')
        if len(code)>runtime:raise OutOfGas('LP4-CODE-01 B_code_max')
        state=cls.active[-1]
        if sum(len(state.get_code(a)) for a in state._code_addresses)+len(code)>logical:raise OutOfGas('LP4-CODE-01 CODE_CAP')
CodeState.computation_class=CodeComputation
class CodeVM(CancunVM):_state_class=CodeState

source_hashes={r['path']:r['sha256'] for r in json.loads((delta_dir/'sources.json').read_text())}
rows=[];seen=set()
for line in (delta_dir/'changed-cases.jsonl').open():
    row=json.loads(line);key=(row['file'],row['name'],row['variant']);assert key not in seen;seen.add(key)
    data=(corpus/row['file']).read_bytes();assert hashlib.sha256(data).hexdigest()==source_hashes[row['file']]
    original=json.loads(data)[row['name']];test=dict(original,transaction={k:v for k,v in original['transaction'].items() if k!='secretKey'});post=test['post']['Cancun'][row['variant']]
    assert post['indexes']==row['indexes']
    try:actual=execute(test,post,'lp4-create01-modexp-admission',vm_override=CodeVM,include_gas=True)
    except CodeSnapshotRejected as error:
        assert row['code01']['error']=='InvalidCodeSnapshot:'+str(error)
        actual=dict(error=row['code01']['error'],proof='Actual VM complete code inventory rejects input snapshot; NOT GenesisBuilder execution')
        comparison='MATCH_SNAPSHOT_ADMISSION_ONLY'
    else:
        for k in ['actualRoot','actualLogs','gasUsed']:assert actual[k]==row['code01'][k],(key,k,actual[k],row['code01'][k])
        assert actual['executionSuccess']==row['code01']['success'];comparison='MATCH_EXECUTION'
    rows.append(dict(file=row['file'],name=row['name'],variant=row['variant'],indexes=row['indexes'],sourceSHA256=source_hashes[row['file']],baseline=row['baseline'],code01=row['code01'],independent=actual,comparison=comparison,scope=__doc__,bootable=False))
    (output/'cases.json').write_text(json.dumps(rows,indent=2)+'\n')
assert len(rows)==summary['changed']
result=dict(total=len(rows),matchedExecution=sum(r['comparison']=='MATCH_EXECUTION' for r in rows),matchedSnapshotAdmissionOnly=sum(r['comparison']=='MATCH_SNAPSHOT_ADMISSION_ONLY' for r in rows),excluded=0,blocked=0,profileFileSHA256=hashlib.sha256(profile.read_bytes()).hexdigest(),deltaCasesSHA256=hashlib.sha256((delta_dir/'changed-cases.jsonl').read_bytes()).hexdigest(),scope=__doc__,status='Scoped independent execution/inventory evidence; actual GenesisBuilder and complete T2/P2/P3 NOT PASSED',bootable=False)
(output/'summary.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result))
