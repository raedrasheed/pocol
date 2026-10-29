"""Second-engine Cancun execution; not PoCol wire/exception/T2 acceptance.
Uses unmodified py-evm, deliberately preserving CREATE-01 disagreements for review.
Only sanitized per-case identities/results are emitted; no fixture secret keys.
"""
import argparse, collections, hashlib, json, pathlib, time
import rlp
from eth.db.atomic import AtomicDB
from eth.vm.chain_context import ChainContext
from eth.vm.forks.cancun import CancunVM
from eth.vm.forks.cancun.blocks import CancunBlockHeader
from eth.vm.spoof import SpoofTransaction
from eth_hash.auto import keccak


def num(v):
    return int(v, 16) if isinstance(v, str) else v


def raw(v):
    return bytes.fromhex(v.removeprefix('0x'))


def execute(test, post):
    env = test['env']; tx = test['transaction']; ix = post['indexes']
    header = CancunBlockHeader(
        difficulty=0, block_number=num(env['currentNumber']),
        gas_limit=num(env['currentGasLimit']), timestamp=num(env['currentTimestamp']),
        coinbase=raw(env['currentCoinbase']), base_fee_per_gas=num(env.get('currentBaseFee', 0)),
        mix_hash=raw(env.get('currentRandom', '0x'+'00'*32)),
        excess_blob_gas=num(env.get('currentExcessBlobGas', 0)))
    state = CancunVM.build_state(AtomicDB(), header, ChainContext(1))
    for addr, account in test['pre'].items():
        addr = raw(addr)
        state.set_nonce(addr, num(account['nonce']))
        state.set_balance(addr, num(account['balance']))
        state.set_code(addr, raw(account['code']))
        for key, value in account['storage'].items():
            state.set_storage(addr, num(key), num(value))
    # Persist initial allocation before transaction-wide original/current storage reads.
    state.persist()
    args = dict(nonce=num(tx['nonce']), gas=num(tx['gasLimit'][ix['gas']]),
                to=raw(tx['to']), value=num(tx['value'][ix['value']]), data=raw(tx['data'][ix['data']]))
    access = tx.get('accessLists', [None]*len(tx['data']))[ix['data']]
    access_list = [(raw(a['address']), tuple(num(k) for k in a['storageKeys'])) for a in (access or [])]
    builder = CancunVM.get_transaction_builder()
    if 'maxFeePerGas' in tx:
        args.update(chain_id=1, max_fee_per_gas=num(tx['maxFeePerGas']),
                    max_priority_fee_per_gas=num(tx['maxPriorityFeePerGas']), access_list=access_list)
        if 'blobVersionedHashes' in tx:
            args.update(max_fee_per_blob_gas=num(tx['maxFeePerBlobGas']),
                        blob_versioned_hashes=tuple(raw(x) for x in tx['blobVersionedHashes']))
            unsigned = builder.new_unsigned_blob_transaction(**args)
        else:
            unsigned = builder.new_unsigned_dynamic_fee_transaction(**args)
    elif access is not None:
        unsigned = builder.new_unsigned_access_list_transaction(chain_id=1, gas_price=num(tx['gasPrice']), access_list=access_list, **args)
    else:
        unsigned = builder.create_unsigned_transaction(gas_price=num(tx['gasPrice']), **args)
    computation = state.apply_transaction(SpoofTransaction(unsigned, from_=raw(tx['sender'])))
    logs = [[a, [t.to_bytes(32, 'big') for t in topics], data] for a, topics, data in computation.get_log_entries()]
    return dict(actualRoot='0x'+state.make_state_root().hex(), actualLogs='0x'+keccak(rlp.encode(logs)).hex(),
                executionSuccess=not computation.is_error)


def main():
    p=argparse.ArgumentParser(); p.add_argument('corpus'); p.add_argument('output'); p.add_argument('--limit',type=int,default=0); a=p.parse_args()
    out=pathlib.Path(a.output); out.mkdir(parents=True,exist_ok=True); start=time.time(); counts=collections.Counter(); sources=[]
    with (out/'cases.jsonl').open('w') as log:
        for file in sorted(pathlib.Path(a.corpus).rglob('*.json')):
            data=file.read_bytes(); sources.append(dict(path=str(file.relative_to(a.corpus)),sha256=hashlib.sha256(data).hexdigest()))
            for name,test in json.loads(data).items():
                for variant,post in enumerate(test.get('post',{}).get('Cancun',[])):
                    row=dict(file=str(file.relative_to(a.corpus)),name=name,variant=variant,indexes=post['indexes'],expectedException=post.get('expectException'))
                    try:
                        result=execute(test,post); row.update(result)
                        ok=not row['expectedException'] and result['actualRoot']==post['hash'] and result['actualLogs']==post['logs']
                        row.update(expectedRoot=post['hash'],expectedLogs=post['logs'],comparison='MATCH' if ok else 'MISMATCH')
                    except Exception as e:
                        # Error text may contain raw transaction values; retain only type.
                        row.update(errorType=type(e).__name__,comparison='EXCEPTION_PRESENCE_ONLY' if row['expectedException'] else 'ENGINE_ERROR')
                    counts[row['comparison']]+=1; counts['total']+=1; log.write(json.dumps(row)+'\n')
                    if counts['total']%250==0: log.flush(); print(dict(counts),flush=True)
                    if a.limit and counts['total']>=a.limit: break
                if a.limit and counts['total']>=a.limit: break
            if a.limit and counts['total']>=a.limit: break
    summary=dict(scope='Unmodified independent py-evm Cancun; CREATE-01 differences retained, no PoCol wire/PAY acceptance',counts=dict(counts),limited=bool(a.limit),seconds=round(time.time()-start,3),status='REQUIRES REVIEW; NOT PASSED',bootable=False)
    (out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n'); (out/'sources.json').write_text(json.dumps(sources,indent=2)+'\n'); print(summary)

if __name__=='__main__': main()
