"""Independent fixture/input predicates for rejection categories, not signed-wire proof.
Unknown labels/errors fail closed. The sole over-u256 adapter rejection is separate.
"""
import argparse, collections, json, pathlib, re

LABELS = {
 'IntrinsicGas': {'CallGasCostMoreThanGasLimit'},
 'TR_IntrinsicGas': {'CallGasCostMoreThanGasLimit'},
 'TransactionException.INTRINSIC_GAS_TOO_LOW': {'CallGasCostMoreThanGasLimit'},
 'TR_NoFundsOrGas': {'CallGasCostMoreThanGasLimit','LackOfFundForMaxFee'},
 'TR_NoFunds': {'LackOfFundForMaxFee'},
 'TransactionException.INSUFFICIENT_ACCOUNT_FUNDS': {'LackOfFundForMaxFee'},
 'TR_NoFundsX': {'OverflowPaymentInTransaction'},
 'SenderNotEOA': {'RejectCallerWithCode'},
 'TR_BLOBCREATE': {'BlobCreateTransaction'},
 'TR_BLOBLIST_OVERSIZE': {'TooManyBlobs'},
 'TransactionException.TYPE_3_TX_BLOB_COUNT_EXCEEDED': {'TooManyBlobs'},
 'TR_BLOBVERSION_INVALID': {'BlobVersionNotSupported'},
 'TransactionException.TYPE_3_TX_INVALID_BLOB_VERSIONED_HASH': {'BlobVersionNotSupported'},
 'TR_EMPTYBLOB': {'EmptyBlobs'},
 'TransactionException.TYPE_3_TX_ZERO_BLOBS': {'EmptyBlobs'},
 'TR_FeeCapLessThanBlocks': {'GasPriceLessThanBasefee'},
 'TransactionException.INSUFFICIENT_MAX_FEE_PER_GAS': {'GasPriceLessThanBasefee'},
 'TR_GasLimitReached': {'CallerGasLimitMoreThanBlock'},
 'TR_NonceHasMaxValue': {'NonceOverflowInTransaction'},
 'TR_TipGtFeeCap': {'PriorityFeeGreaterThanMaxFee'},
 'TransactionException.INITCODE_SIZE_EXCEEDED': {'CreateInitCodeSizeLimit'},
 'TransactionException.INSUFFICIENT_MAX_FEE_PER_BLOB_GAS': {'BlobGasPriceGreaterThanMax'},
}
SIMPLE = {x for v in LABELS.values() for x in v} - {'LackOfFundForMaxFee','TooManyBlobs'}

def num(v): return int(v,16) if isinstance(v,str) else v

def fake_exponential(numerator):
    denominator=3338477; accum=denominator; output=0; i=1
    while accum:
        output+=accum; accum=accum*numerator//(denominator*i); i+=1
    return output//denominator

def verify(test, post, error):
    tx=test['transaction']; ix=post['indexes']; env=test['env']; label=post['expectException']
    chosen=tx['value'][ix['value']]
    if label=='TR_RLP_WRONGVALUE':
        # Frozen GST bigint notation is an adapter format, not wire RLP.
        assert isinstance(chosen,str) and chosen.startswith('0x:bigint 0x')
        value=int(chosen.removeprefix('0x:bigint '),16)
        assert error=='ControlInputValueOverflow' and value>=2**256
        return 'ADAPTER_INPUT_ONLY'
    value=num(chosen)
    assert label in LABELS, ('unknown expected category',label)
    m=re.fullmatch(r'Transaction\(([A-Za-z]+)(?: \{ (.*) \})?\)',error)
    assert m, 'unrecognized actual error syntax'
    kind,fields=m.groups(); assert kind in LABELS[label], ('wrong rejection category',label,kind)
    if kind in SIMPLE: assert fields is None
    gas=num(tx['gasLimit'][ix['gas']]); fee=num(tx.get('maxFeePerGas',tx.get('gasPrice')))
    sender=test['pre'].get(tx['sender'],dict(balance=0,nonce=0,code='0x'))
    balance=num(sender['balance']); data=bytes.fromhex(tx['data'][ix['data']].removeprefix('0x'))
    create=not tx['to']; access=(tx.get('accessLists') or [None]*len(tx['data']))[ix['data']] or []
    intrinsic=21000+(32000 if create else 0)+sum(4 if b==0 else 16 for b in data)
    intrinsic+=sum(2400+1900*len(a['storageKeys']) for a in access)
    if create: intrinsic+=2*((len(data)+31)//32)
    blobs=tx.get('blobVersionedHashes',[])
    payment=gas*fee+value+len(blobs)*131072*num(tx.get('maxFeePerBlobGas',0))
    predicates={
      'CallGasCostMoreThanGasLimit': gas<intrinsic,
      'LackOfFundForMaxFee': payment>balance and payment<2**256,
      'OverflowPaymentInTransaction': payment>=2**256,
      'RejectCallerWithCode': bool(bytes.fromhex(sender['code'].removeprefix('0x'))),
      'BlobCreateTransaction': create and 'blobVersionedHashes' in tx,
      'TooManyBlobs': len(blobs)>6,
      'BlobVersionNotSupported': any(bytes.fromhex(b.removeprefix('0x'))[0]!=1 for b in blobs),
      'EmptyBlobs': 'blobVersionedHashes' in tx and not blobs,
      'GasPriceLessThanBasefee': fee<num(env.get('currentBaseFee',0)),
      'CallerGasLimitMoreThanBlock': gas>num(env['currentGasLimit']),
      'NonceOverflowInTransaction': num(tx['nonce'])==2**64-1 or num(sender['nonce'])==2**64-1,
      'PriorityFeeGreaterThanMaxFee': num(tx.get('maxPriorityFeePerGas',0))>fee,
      'CreateInitCodeSizeLimit': create and len(data)>49152,
      'BlobGasPriceGreaterThanMax': num(tx.get('maxFeePerBlobGas',0))<fake_exponential(num(env.get('currentExcessBlobGas',0))),
    }
    assert predicates[kind], ('rejection predicate false',label,kind)
    if kind=='LackOfFundForMaxFee':
        assert fields==f'fee: {payment}, balance: {balance}', 'actual fee/balance differ from independent complete input'
    if kind=='TooManyBlobs': assert fields==f'have: {len(blobs)}'
    return 'CATEGORY_AND_INPUT_PREDICATE'

def main():
    p=argparse.ArgumentParser();p.add_argument('corpus');p.add_argument('receipts');p.add_argument('output');a=p.parse_args()
    cases=[];counts=collections.Counter();last=None;tests=None
    for line in open(a.receipts):
        row=json.loads(line)
        if not row.get('expectedException'):continue
        if row['file']!=last: tests=json.loads((pathlib.Path(a.corpus)/row['file']).read_bytes());last=row['file']
        test=tests[row['name']];post=test['post']['Cancun'][row['variant']]
        assert post['indexes']==row['indexes'] and post['expectException']==row['expectedException']
        status=verify(test,post,row['error']);counts[status]+=1
        cases.append({k:row[k] for k in ('file','name','variant','indexes','expectedException','error')}|dict(status=status))
    out=dict(scope='Fixture rejection categories and independent numeric predicates only; no signature/wire/error-precedence equivalence proof',counts=dict(counts),cases=cases,P2='NOT PASSED',bootable=False)
    pathlib.Path(a.output).write_text(json.dumps(out,indent=2)+'\n');print(dict(counts))

if __name__=='__main__':main()
