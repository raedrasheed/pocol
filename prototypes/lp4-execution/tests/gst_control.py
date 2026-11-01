"""Upstream Cancun control corpus runner. NOT PoCol exception/T2 acceptance.
Only Cancun post variants are executed. Secret keys never cross the runner boundary.
Independent Python trie/RLP/Keccak checks actual resulting full state and logs.
"""
import argparse, hashlib, json, pathlib, subprocess, time
import rlp
from eth_hash.auto import keccak
from trie import HexaryTrie

def raw(s):return bytes.fromhex(s.removeprefix('0x'))
def num(n):return int(n,16) if isinstance(n,str) else n
def root(state):
    accounts=HexaryTrie(db={})
    for address,a in state.items():
        storage=HexaryTrie(db={})
        for k,v in a['storage'].items():
            if num(v): storage[keccak(num(k).to_bytes(32,'big'))]=rlp.encode(num(v))
        accounts[keccak(raw(address))]=rlp.encode([num(a['nonce']),num(a['balance']),storage.root_hash,keccak(raw(a['code']))])
    return '0x'+accounts.root_hash.hex()
def loghash(logs):
    return '0x'+keccak(rlp.encode([[raw(l['address']),[raw(t) for t in l['topics']],raw(l['data'])] for l in logs])).hex()

def main():
    p=argparse.ArgumentParser();p.add_argument('corpus');p.add_argument('binary');p.add_argument('output');p.add_argument('--limit',type=int,default=0);a=p.parse_args()
    out=pathlib.Path(a.output);out.mkdir(parents=True,exist_ok=True)
    process=subprocess.Popen([a.binary],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=open(out/'runner-stderr.log','w'),text=True)
    start=time.time();counts=dict(total=0,passControl=0,failControl=0,runnerFailure=0,nonCancunVariants=0);sources=[]
    with (out/'cases.jsonl').open('w') as log:
      for file in sorted(pathlib.Path(a.corpus).rglob('*.json')):
        data=file.read_bytes();sources.append({'path':str(file.relative_to(a.corpus)),'sha256':hashlib.sha256(data).hexdigest()})
        for name,test in json.loads(data).items():
          counts['nonCancunVariants']+=sum(len(v) for k,v in test.get('post',{}).items() if k!='Cancun')
          for variant,post in enumerate(test.get('post',{}).get('Cancun',[])):
            tx={k:v for k,v in test['transaction'].items() if k!='secretKey'}
            payload=dict(pre=test['pre'],env=test['env'],transaction=tx,indexes=post['indexes'])
            process.stdin.write(json.dumps(payload)+'\n');process.stdin.flush()
            line=process.stdout.readline()
            if not line: raise RuntimeError('runner exited')
            result=json.loads(line);expectedException=post.get('expectException');counts['total']+=1
            record=dict(file=str(file.relative_to(a.corpus)),name=name,variant=variant,indexes=post['indexes'],expectedException=expectedException)
            if 'runnerFailure' in result: counts['runnerFailure']+=1;ok=False;record['result']=result
            elif 'error' in result:ok=bool(expectedException);record['error']=result['error']
            elif expectedException:ok=False;record['error']='expected exception but transaction executed'
            else:
                actual=root(result['state']);actualLogs=loghash(result['logs'])
                ok=actual==post['hash'] and actualLogs==post['logs']
                record.update(expectedRoot=post['hash'],actualRoot=actual,expectedLogs=post['logs'],actualLogs=actualLogs,gasUsed=result['gasUsed'])
            counts['passControl' if ok else 'failControl']+=1;record['controlPass']=ok;log.write(json.dumps(record)+'\n')
            if counts['total']%1000==0:log.flush();print(counts,flush=True)
            if a.limit and counts['total']>=a.limit:break
          if a.limit and counts['total']>=a.limit:break
        if a.limit and counts['total']>=a.limit:break
    process.stdin.close();process.wait()
    summary=dict(approvedStorageCollisionRule='LP4-CREATE-01',scope='Upstream Cancun only; NOT PoCol T2 acceptance. Exception reason equivalence not proven.',counts=counts,seconds=round(time.time()-start,3),runnerExit=process.returncode,limited=bool(a.limit),fullIndependentEVM='NOT RUN',bootable=False)
    (out/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');(out/'sources.json').write_text(json.dumps(sources,indent=2)+'\n');print(summary)
    assert counts['failControl']==counts['runnerFailure']==0
if __name__=='__main__':main()
