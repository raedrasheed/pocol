"""Enumerate ALL CODE-01 deltas without dropping original Cancun controls.

Unsigned delta runner isolates code limits using the supplied profile's exact CP.
Synthetic fixture context/chain1 is preserved; this is not signed PoCol apply or
complete ParamGate. Changed cases are individual observations, not blanket PASS.
Independent replacement execution remains mandatory beyond root reconstruction.
"""
import sys,json,hashlib,subprocess,time
from pathlib import Path
from gst_control import root,loghash
corpus,binary,profile,baseline,output=map(Path,sys.argv[1:]);output.mkdir(parents=True,exist_ok=False)
base={}
for line in baseline.open():
    row=json.loads(line);key=(row['file'],row['name'],row['variant']);assert key not in base;base[key]=row
assert len(base)==20661
start=time.monotonic();seen=set();changed=0;unchanged=0;sources=[]
process=subprocess.Popen([str(binary),str(profile)],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=(output/'stderr.log').open('w'),text=True)
with (output/'all-cases.jsonl').open('w') as all_cases,(output/'changed-cases.jsonl').open('w') as deltas:
    for file in sorted(corpus.rglob('*.json')):
        data=file.read_bytes();relative=str(file.relative_to(corpus));sources.append(dict(path=relative,sha256=hashlib.sha256(data).hexdigest()))
        for name,test in json.loads(data).items():
            for variant,post in enumerate(test.get('post',{}).get('Cancun',[])):
                key=(relative,name,variant);assert key not in seen and key in base;seen.add(key)
                # Explicit whitelist: public fixture secretKey never crosses.
                tx={k:v for k,v in test['transaction'].items() if k!='secretKey'}
                payload=dict(pre=test['pre'],env=test['env'],transaction=tx,indexes=post['indexes'])
                process.stdin.write(json.dumps(payload)+'\n');process.stdin.flush();line=process.stdout.readline();assert line,'runner exited'
                actual=json.loads(line);assert not actual.get('runnerFailure'),key
                assert actual['codeRule']=='LP4-CODE-01'
                before=base[key];assert before['controlPass']
                if 'error' in actual:after=dict(error=actual['error'])
                else:after=dict(actualRoot=root(actual['state']),actualLogs=loghash(actual['logs']),gasUsed=actual['gasUsed'],success=actual['success'])
                same=(after.get('error')==before.get('error') if 'error' in after or 'error' in before else all(after[k]==before[k] for k in ['actualRoot','actualLogs','gasUsed']))
                record=dict(file=relative,name=name,variant=variant,indexes=post['indexes'],expectedException=post.get('expectException'),baseline=before,code01=after,changed=not same)
                all_cases.write(json.dumps(record)+'\n')
                if same:unchanged+=1
                else:
                    changed+=1;record['replacementAcceptance']='NOT PASSED: independent amended execution and integrated PoCol admission required';deltas.write(json.dumps(record)+'\n')
process.stdin.close();assert process.wait()==0;assert seen==set(base)
summary=dict(rule='LP4-CODE-01',total=len(seen),changed=changed,unchanged=unchanged,excluded=0,blocked=0,runnerFailure=0,sourceFiles=len(sources),profileFileSHA256=hashlib.sha256(profile.read_bytes()).hexdigest(),baselineSHA256=hashlib.sha256(baseline.read_bytes()).hexdigest(),seconds=round(time.monotonic()-start,3),scope=__doc__,status='NOT PASSED: observations only; independent replacement gate remains open',bootable=False)
(output/'summary.json').write_text(json.dumps(summary,indent=2)+'\n');(output/'sources.json').write_text(json.dumps(sources,indent=2)+'\n');print(json.dumps(summary))
