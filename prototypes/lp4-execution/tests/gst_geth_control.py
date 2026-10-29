"""Pinned external Geth statetest control; does not execute PoCol apply.
Never emit fixture keys or arbitrary engine error messages. All raw engine output
stays local for review; published comparison identities/hashes contain no signing keys.
"""
import argparse,collections,hashlib,json,pathlib,subprocess,time

def main():
    p=argparse.ArgumentParser();p.add_argument('corpus');p.add_argument('binary');p.add_argument('output');p.add_argument('--limit-files',type=int,default=0);a=p.parse_args()
    corpus=pathlib.Path(a.corpus).resolve();out=pathlib.Path(a.output);out.mkdir(parents=True,exist_ok=True)
    files=sorted(corpus.rglob('*.json'))
    if a.limit_files:files=files[:a.limit_files]
    sources=[];expected=0
    for f in files:
        data=f.read_bytes();sources.append(dict(path=str(f.relative_to(corpus)),sha256=hashlib.sha256(data).hexdigest()))
        expected+=sum(len(t.get('post',{}).get('Cancun',[])) for t in json.loads(data).values())
    start=time.time()
    with (out/'engine-stdout.local').open('w') as stdout,(out/'engine-stderr.local').open('w') as stderr:
        proc=subprocess.run([a.binary,'statetest','--statetest.fork','Cancun'],input=''.join(str(f)+'\n' for f in files),stdout=stdout,stderr=stderr,text=True)
    receipt=dict(exit=proc.returncode,seconds=round(time.time()-start,3),inputFiles=len(files),expectedCases=expected,binarySHA256=hashlib.sha256(pathlib.Path(a.binary).read_bytes()).hexdigest(),stdoutSHA256=hashlib.sha256((out/'engine-stdout.local').read_bytes()).hexdigest(),stderrSHA256=hashlib.sha256((out/'engine-stderr.local').read_bytes()).hexdigest())
    (out/'process-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
    # CLI emits one JSON array per input file. Per-name variant order is source-bound
    # to tests.StateTest.Subtests() ascending Index and cmd/evm collector iteration.
    text=(out/'engine-stdout.local').read_text();decoder=json.JSONDecoder();offset=0;counts=collections.Counter();cases=[]
    for f in files:
        while offset<len(text) and text[offset].isspace():offset+=1
        if offset==len(text):break
        group,end=decoder.raw_decode(text,offset);offset=end
        tests=json.loads(f.read_bytes());seen=collections.Counter()
        if group is None:
            assert not any(t.get('post',{}).get('Cancun') for t in tests.values()), 'null native output cannot omit required cases'
            group=[]
        assert isinstance(group,list)
        for r in group:
            assert r['fork']=='Cancun' and isinstance(r['pass'],bool)
            name=r['name'];index=seen[name];seen[name]+=1;post=tests[name]['post']['Cancun'][index]
            error_hash=hashlib.sha256(r.get('error','').encode()).hexdigest() if r.get('error') else None
            row=dict(file=str(f.relative_to(corpus)),name=name,variant=index,indexes=post['indexes'],expectedException=post.get('expectException'),enginePass=r['pass'],errorSHA256=error_hash)
            if not post.get('expectException'):
                row.update(actualRoot=r.get('stateRoot'),expectedRoot=post['hash'])
                ok=r['pass'] and row['actualRoot']==row['expectedRoot']
                kind='STATE_AND_NATIVE_LOG_ASSERTIONS' if ok else 'FAILURE'
            else:kind='EXPECTED_REJECTION_PRESENCE_ONLY' if r['pass'] else 'FAILURE'
            row['comparison']=kind;counts[kind]+=1;counts['total']+=1;cases.append(row)
        assert dict(seen)=={n:len(t.get('post',{}).get('Cancun',[])) for n,t in tests.items() if t.get('post',{}).get('Cancun')}
    assert not text[offset:].strip(),'unexpected extra engine stdout'
    summary=dict(scope='External Geth upstream Cancun controls; not PoCol wire/apply/deviation acceptance',expected=expected,counts=dict(counts),exit=proc.returncode,limited=bool(a.limit_files),seconds=round(time.time()-start,3),binarySHA256=hashlib.sha256(pathlib.Path(a.binary).read_bytes()).hexdigest(),complete=proc.returncode==0 and counts['total']==expected,status='REQUIRES REVIEW; P2/LP4 NOT PASSED',bootable=False)
    (out/'cases.jsonl').write_text(''.join(json.dumps(r)+'\n' for r in cases));(out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');(out/'sources.json').write_text(json.dumps(sources,indent=2)+'\n');print(summary)
    assert summary['complete'] and not counts['FAILURE']
if __name__=='__main__':main()
