"""Independent signed Geth outcomes joined to every already recorded GST identity."""
import json,pathlib,subprocess,sys,collections,hashlib
EXPECTED={
 'SenderNotEOA':{'sender-not-eoa'},'TR_NonceHasMaxValue':{'nonce-max'},
 'IntrinsicGas':{'intrinsic-gas'},'TR_IntrinsicGas':{'intrinsic-gas'},'TransactionException.INTRINSIC_GAS_TOO_LOW':{'intrinsic-gas'},
 'TR_NoFunds':{'insufficient-funds'},'TR_NoFundsX':{'insufficient-funds'},'TR_NoFundsOrGas':{'insufficient-funds','intrinsic-gas'},
 'TR_FeeCapLessThanBlocks':{'fee-cap'},'TR_GasLimitReached':{'block-gas'},'TransactionException.INITCODE_SIZE_EXCEEDED':{'initcode-size'},
}
binary,source,out=map(pathlib.Path,sys.argv[1:]);out.mkdir(parents=True,exist_ok=True);counts=collections.Counter();accepted=[];total=0
with (out/'stderr.log').open('wb') as err,(out/'cases.jsonl').open('w') as result:
 p=subprocess.Popen([str(binary)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=err,text=True)
 try:
  for rawline,case in zip((source/'raw-signed-inputs.jsonl').open(),(source/'cases.jsonl').open(),strict=True):
   case=json.loads(case);raw=json.loads(rawline)['raw'];assert hashlib.sha256(bytes.fromhex(raw)).hexdigest()==case['rawSHA256']
   p.stdin.write(rawline);p.stdin.flush();actual=json.loads(p.stdout.readline());row={k:case[k] for k in ('file','name','variant','indexes','expectedException','kind','rawSHA256','category')};row['independentGeth']=actual
   if case['category']=='SIGNED_NATIVE_CATEGORY_AND_INPUT_PREDICATE':
    assert not actual['accepted'] and actual['stage']=='execution' and actual['errorKind'] in EXPECTED[case['expectedException']],('independent error category',case['file'],case['name'],case['variant'],actual)
    assert actual['sender']==case['rust']['sender'] and actual['hash']==case['rust']['hash'] and actual['snapshotRestored']
   result.write(json.dumps(row)+'\n');result.flush();total+=1
   counts['ACCEPTED' if actual['accepted'] else 'REJECTED']+=1
   if actual['accepted']:accepted.append({k:row[k] for k in ('file','name','variant','kind','expectedException')})
  p.stdin.close();assert p.wait(timeout=10)==0
 finally:
  if p.poll() is None:p.kill();p.wait()
assert total==1173
summary=dict(scope='Actual recovered signed unmodified Geth Cancun admission; not full PoCol apply or proof of exact upstream error precedence',total=total,counts=dict(counts),accepted=accepted,status='NOT PASSED' if accepted else 'SCOPED REJECTION VERIFIED; FULL P2/P3 NOT PASSED',binarySHA256=hashlib.sha256(binary.read_bytes()).hexdigest(),bootable=False)
(out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps({k:summary[k] for k in ('total','counts','status')}));assert not accepted,('signed independent validation accepted invalid fixtures',accepted)
