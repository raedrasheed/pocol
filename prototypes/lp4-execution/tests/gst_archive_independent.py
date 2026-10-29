"""Validate and retain every independent comparison row; no summary substitutes.
Reads public fixture identities/results only, never transaction secretKey fields.
This verifies unsigned fixture execution and exception presence, not signed-wire
rejection equality or complete PoCol acceptance.
"""
from pathlib import Path
import collections,gzip,hashlib,json,sys

def archive(results,corpus):
    results,corpus=Path(results),Path(corpus)
    summary=json.loads((results/'summary.json').read_text())
    assert not summary['limited'] and summary['selectedCases'] is None
    sources=json.loads((results/'sources.json').read_text())
    assert len(sources)==len({x['path'] for x in sources})==2807
    expected={}
    for source in sources:
        path=Path(source['path']);assert not path.is_absolute() and '..' not in path.parts
        content=(corpus/path).read_bytes();assert hashlib.sha256(content).hexdigest()==source['sha256']
        for name,test in json.loads(content).items():
            for variant,post in enumerate(test.get('post',{}).get('Cancun',[])):
                key=(source['path'],name,variant);assert key not in expected;expected[key]=post
    assert len(expected)==20661
    fields={'file','name','variant','indexes','expectedException','actualRoot','actualLogs','executionSuccess','expectedRoot','expectedLogs','comparison','errorType','timeoutSeconds'}
    seen=set();counts=collections.Counter();raw=results/'cases.jsonl'
    with raw.open() as stream:
        for line in stream:
            row=json.loads(line);assert not set(row)-fields,'Unexpected publication field'
            key=(row['file'],row['name'],row['variant']);assert key not in seen and key in expected
            seen.add(key);post=expected[key]
            assert row['indexes']==post['indexes'] and row['expectedException']==post.get('expectException')
            if post.get('expectException'):
                assert row['comparison']=='EXCEPTION_PRESENCE_ONLY' and row.get('errorType')
            else:
                assert row['comparison']=='MATCH'
                assert row['actualRoot']==row['expectedRoot']==post['hash']
                assert row['actualLogs']==row['expectedLogs']==post['logs']
            counts[row['comparison']]+=1;counts['total']+=1
    assert seen==set(expected) and dict(counts)==summary['counts']
    assert dict(counts)=={'MATCH':19488,'EXCEPTION_PRESENCE_ONLY':1173,'total':20661}
    assert (results/'incomplete.jsonl').read_bytes()==b''
    target=results/'cases.jsonl.gz';assert not target.exists(),'Preserve existing archive'
    with target.open('xb') as output:
        with gzip.GzipFile(filename='',fileobj=output,mode='wb',mtime=0) as compressed:
            with raw.open('rb') as source:
                while chunk:=source.read(1048576):compressed.write(chunk)
    digest=lambda p:hashlib.file_digest(p.open('rb'),'sha256').hexdigest()
    receipt=dict(scope=__doc__,cases=20661,identitiesVerified=True,sourceFiles=2807,
        excluded=0,blocked=0,incomplete=0,counts=dict(counts),
        rawBytes=raw.stat().st_size,rawSHA256=digest(raw),gzipBytes=target.stat().st_size,
        gzipSHA256=digest(target),publicationFields=sorted(fields),bootable=False)
    with gzip.open(target,'rb') as stream:assert hashlib.file_digest(stream,'sha256').hexdigest()==receipt['rawSHA256']
    (results/'cases-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n')
    print('Verified and archived20661 independent identities/results;1173 exception-presence-only; full LP4 NOT PASSED')

if __name__=='__main__':archive(sys.argv[1],sys.argv[2])
