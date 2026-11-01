"""Actual signed GST-negative controls; complete case identities, no fixture keys emitted.
Original public corpus signing keys are used only in memory. This does not read
credentials or use funded accounts. PoCol type3 rejection is explicitly distinct
from the original upstream category. Unmodified second-engine gaps stay NOT PASSED.
"""
import json,sys,pathlib,subprocess,collections,hashlib,rlp
from eth_keys import keys
from eth_hash.auto import keccak
from eth.db.atomic import AtomicDB
from eth.db.chain import ChainDB
from eth.consensus.context import ConsensusContext
from eth.vm.chain_context import ChainContext
from eth.vm.forks.cancun import CancunVM
from eth.vm.forks.cancun.blocks import CancunBlockHeader
from gst_exception_audit import verify
CHAIN=777902

def number(v):return int(v.removeprefix('0x:bigint '),16) if isinstance(v,str) else v

def sign(tx,ix):
 key=keys.PrivateKey(bytes.fromhex(tx['secretKey'].removeprefix('0x')))
 assert '0x'+key.public_key.to_canonical_address().hex()==tx['sender'].lower(),'public fixture sender/key mismatch'
 nonce=number(tx['nonce']);gas=number(tx['gasLimit'][ix['gas']]);to=bytes.fromhex(tx['to'].removeprefix('0x'));value=number(tx['value'][ix['value']]);data=bytes.fromhex(tx['data'][ix['data']].removeprefix('0x'))
 access_raw=(tx.get('accessLists') or [None]*len(tx['data']))[ix['data']];access=access_raw or []
 access=[[bytes.fromhex(a['address'].removeprefix('0x')),[bytes.fromhex(k.removeprefix('0x')) for k in a['storageKeys']]] for a in access]
 kind=3 if 'blobVersionedHashes' in tx else 2 if 'maxFeePerGas' in tx else 1 if access_raw is not None else 0
 if kind==0:base=[nonce,number(tx['gasPrice']),gas,to,value,data];message=rlp.encode(base+[CHAIN,0,0])
 elif kind==1:base=[CHAIN,nonce,number(tx['gasPrice']),gas,to,value,data,access];message=bytes([kind])+rlp.encode(base)
 else:
  base=[CHAIN,nonce,number(tx['maxPriorityFeePerGas']),number(tx['maxFeePerGas']),gas,to,value,data,access]
  if kind==3:base += [number(tx['maxFeePerBlobGas']),[bytes.fromhex(b.removeprefix('0x')) for b in tx['blobVersionedHashes']]]
  message=bytes([kind])+rlp.encode(base)
 sig=key.sign_msg_hash(keccak(message));raw=(bytes([kind]) if kind else b'')+rlp.encode(base+[sig.v+2*CHAIN+35 if kind==0 else sig.v,sig.r,sig.s]);return kind,raw

def independent(test,raw):
 e=test['env'];db=AtomicDB();header=CancunBlockHeader(difficulty=0,block_number=number(e['currentNumber']),timestamp=number(e['currentTimestamp']),gas_limit=number(e['currentGasLimit']),base_fee_per_gas=number(e.get('currentBaseFee',0)),coinbase=bytes.fromhex(e['currentCoinbase'].removeprefix('0x')),mix_hash=bytes.fromhex(e.get('currentRandom','0x'+'00'*32).removeprefix('0x')),excess_blob_gas=number(e.get('currentExcessBlobGas',0)))
 vm=CancunVM(header,ChainDB(db),ChainContext(CHAIN),ConsensusContext(db));s=vm.state
 for a,info in test['pre'].items():
  a=bytes.fromhex(a.removeprefix('0x'));s.set_balance(a,number(info['balance']));s.set_nonce(a,number(info['nonce']));s.set_code(a,bytes.fromhex(info['code'].removeprefix('0x')))
  for k,v in info['storage'].items():s.set_storage(a,number(k),number(v))
 s.persist()
 try:
  tx=CancunVM.get_transaction_builder().decode(raw);tx.validate();tx.check_signature_validity();vm.apply_transaction(header,tx)
 except Exception as exc:return dict(rejected=True,errorClass=type(exc).__name__,error=str(exc))
 return dict(rejected=False,errorClass=None)

def main():
 corpus,receipts,binary,output=map(pathlib.Path,sys.argv[1:]);output.mkdir(parents=True,exist_ok=True);counts=collections.Counter();last=None;tests=None;total=0;unmodified=[]
 with open(output/'rust-stderr.log','wb') as err,open(output/'cases.jsonl','w') as casefile,open(output/'raw-signed-inputs.jsonl','w') as rawfile:
  process=subprocess.Popen([str(binary)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=err,text=True)
  try:
   for line in receipts.open():
    row=json.loads(line)
    if not row.get('expectedException'):continue
    if row['file']!=last:tests=json.loads((corpus/row['file']).read_bytes());last=row['file']
    test=tests[row['name']];post=test['post']['Cancun'][row['variant']];assert post['indexes']==row['indexes'] and post['expectException']==row['expectedException']
    kind,raw=sign(test['transaction'],post['indexes']);payload=dict(raw=raw.hex(),pre={a:{k:v[k] for k in ('balance','nonce','code','storage')} for a,v in test['pre'].items()},env={k:v for k,v in test['env'].items() if k in ('currentNumber','currentTimestamp','currentGasLimit','currentBaseFee','currentRandom','currentCoinbase','currentExcessBlobGas')});text=json.dumps(payload,separators=(',',':'));rawfile.write(text+'\n');process.stdin.write(text+'\n');process.stdin.flush();actual=json.loads(process.stdout.readline());assert not actual.get('runnerFailure'),('runner failure',row['file'],row['name'],row['variant'])
    assert not actual.get('accepted',False),('unexpected accepted signed negative',row['file'],row['name'],row['variant'])
    if kind==3:assert actual==dict(stage='wire',error='UnsupportedType');category='POCOL_TYPE3_REJECTION_NOT_ORIGINAL_CATEGORY'
    elif row['expectedException']=='TR_RLP_WRONGVALUE':assert actual==dict(stage='wire',error='Integer');category='SIGNED_OVER_U256_WIRE_INTEGER'
    elif row['expectedException']=='TR_TipGtFeeCap':assert actual==dict(stage='wire',error='FeeCap');category='SIGNED_PRIORITY_WIRE_PRECEDENCE'
    else:
     assert actual['stage']=='execution' and actual['sender']==test['transaction']['sender'].lower() and actual['snapshotUnchanged'];verify(test,post,actual['error']);category='SIGNED_NATIVE_CATEGORY_AND_INPUT_PREDICATE'
    second=independent(test,raw);result={k:row[k] for k in ('file','name','variant','indexes','expectedException')};result.update(kind=kind,rawSHA256=hashlib.sha256(raw).hexdigest(),category=category,rust=actual,unmodifiedSecondEngine=second);casefile.write(json.dumps(result)+'\n');casefile.flush();counts[category]+=1;counts['unmodifiedRejected' if second['rejected'] else 'unmodifiedAccepted']+=1;total+=1
    if not second['rejected']:unmodified.append({k:result[k] for k in ('file','name','variant','kind','expectedException')})
   process.stdin.close();assert process.wait(timeout=10)==0
  finally:
   if process.poll() is None:process.kill();process.wait()
 assert total==1173,(total,counts)
 result=dict(scope='All1173 GST negative identities, actual recovered signed admission at synthetic chain777902; not full upstream or PoCol block acceptance',total=total,counts=dict(counts),unmodifiedAccepted=unmodified,status='NOT PASSED' if unmodified else 'SCOPED COMPARISON VERIFIED; FULL P2/P3 NOT PASSED',bootable=False)
 (output/'summary.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps({'total':total,'counts':dict(counts),'status':result['status']}))
if __name__=='__main__':main()
