"""Second-engine fixture execution; not PoCol wire/exception/T2 acceptance.
Explicit unmodified and CREATE-01/MODEXP comparator profiles; no blanket exclusions.
Only sanitized per-case identities/results are emitted; no fixture secret keys.
"""
import argparse, collections, hashlib, json, pathlib, time, signal
from concurrent.futures import ProcessPoolExecutor
from collections import deque
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


def execute(test, post, profile="unmodified", vm_override=None, include_gas=False):
    from gst_independent_vm import ComparisonVM, AdmissionComparisonVM
    vm = {"unmodified":CancunVM,"lp4-create01-modexp":ComparisonVM,"lp4-create01-modexp-admission":AdmissionComparisonVM}[profile]
    if vm_override is not None:vm=vm_override
    env = test['env']; tx = test['transaction']; ix = post['indexes']
    header = CancunBlockHeader(
        difficulty=0, block_number=num(env['currentNumber']),
        gas_limit=num(env['currentGasLimit']), timestamp=num(env['currentTimestamp']),
        coinbase=raw(env['currentCoinbase']), base_fee_per_gas=num(env.get('currentBaseFee', 0)),
        mix_hash=raw(env.get('currentRandom', '0x'+'00'*32)),
        excess_blob_gas=num(env.get('currentExcessBlobGas', 0)))
    state = vm.build_state(AtomicDB(), header, ChainContext(1))
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
    builder = vm.get_transaction_builder()
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
    result=dict(actualRoot='0x'+state.make_state_root().hex(), actualLogs='0x'+keccak(rlp.encode(logs)).hex(),
                executionSuccess=not computation.is_error)
    if include_gas:result['gasUsed']=vm.finalize_gas_used(unsigned,computation)
    return result


class ComparisonTimeout(Exception):
    pass


def compare(item):
    row, test, post, timeout, profile = item
    def expired(signum, frame):
        raise ComparisonTimeout()
    signal.signal(signal.SIGALRM, expired)
    signal.setitimer(signal.ITIMER_REAL, timeout)
    try:
        result=execute(test,post,profile); row.update(result)
        ok=not row['expectedException'] and result['actualRoot']==post['hash'] and result['actualLogs']==post['logs']
        row.update(expectedRoot=post['hash'],expectedLogs=post['logs'],comparison='MATCH' if ok else 'MISMATCH')
    except ComparisonTimeout:
        row.update(comparison='BLOCKED_TIMEOUT',timeoutSeconds=timeout)
    except Exception as e:
        # Error text may contain raw transaction values; retain only type.
        row.update(errorType=type(e).__name__,comparison='EXCEPTION_PRESENCE_ONLY' if row['expectedException'] else 'ENGINE_ERROR')
    finally:
        signal.setitimer(signal.ITIMER_REAL, 0)
    return row


def items(corpus, sources, timeout, limit, profile, selected):
    count=0
    for file in sorted(pathlib.Path(corpus).rglob('*.json')):
        data=file.read_bytes(); sources.append(dict(path=str(file.relative_to(corpus)),sha256=hashlib.sha256(data).hexdigest()))
        for name,test in json.loads(data).items():
            for variant,post in enumerate(test.get('post',{}).get('Cancun',[])):
                row=dict(file=str(file.relative_to(corpus)),name=name,variant=variant,indexes=post['indexes'],expectedException=post.get('expectException'))
                if selected is not None and (row['file'],row['name'],variant) not in selected: continue
                # Public fixture signing keys are unnecessary and never sent to workers.
                sanitized=dict(test,transaction={k:v for k,v in test['transaction'].items() if k!='secretKey'})
                yield row,sanitized,post,timeout,profile
                count+=1
                if limit and count>=limit: return


def main():
    p=argparse.ArgumentParser(); p.add_argument('corpus'); p.add_argument('output'); p.add_argument('--limit',type=int,default=0)
    p.add_argument('--profile',choices=['unmodified','lp4-create01-modexp','lp4-create01-modexp-admission'],default='unmodified'); p.add_argument('--selected-receipts')
    p.add_argument('--workers',type=int,default=4); p.add_argument('--timeout',type=int,default=30); a=p.parse_args()
    assert 1<=a.workers<=4 and a.timeout>0 and a.limit>=0
    selected=None
    if a.selected_receipts:
        selected={(r['file'],r['name'],r['variant']) for r in map(json.loads,open(a.selected_receipts))}
    out=pathlib.Path(a.output); out.mkdir(parents=True,exist_ok=True); start=time.time(); counts=collections.Counter(); sources=[]
    # At most two queued cases per worker, preserving deterministic source order.
    with ProcessPoolExecutor(max_workers=a.workers) as pool, (out/'cases.jsonl').open('w') as log, (out/'incomplete.jsonl').open('w') as incomplete:
        iterator=iter(items(a.corpus,sources,a.timeout,a.limit,a.profile,selected)); pending=deque(); exhausted=False
        while pending or not exhausted:
            while not exhausted and len(pending)<2*a.workers:
                try: item=next(iterator)
                except StopIteration: exhausted=True; break
                pending.append(pool.submit(compare,item))
            if not pending: break
            row=pending.popleft().result()
            counts[row['comparison']]+=1; counts['total']+=1; log.write(json.dumps(row)+'\n'); log.flush()
            # Public metadata only: retain the exact failed identities even when the
            # full per-case receipt is kept local. Never convert timeout to success.
            if row['comparison'] in ('BLOCKED_TIMEOUT', 'ENGINE_ERROR', 'MISMATCH'):
                incomplete.write(json.dumps(row)+'\n'); incomplete.flush()
            if counts['total']%250==0: print(dict(counts),flush=True)
    if selected is not None: assert counts['total']==len(selected), 'selected case missing from frozen corpus'
    summary=dict(scope='Independent py-evm fixture state/log comparison only; no PoCol wire/PAY acceptance',profile=a.profile,selectedCases=len(selected) if selected is not None else None,counts=dict(counts),limited=bool(a.limit),workers=a.workers,timeoutSeconds=a.timeout,seconds=round(time.time()-start,3),status='REQUIRES REVIEW; NOT PASSED',bootable=False)
    (out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n'); (out/'sources.json').write_text(json.dumps(sources,indent=2)+'\n'); print(summary)

if __name__=='__main__': main()
