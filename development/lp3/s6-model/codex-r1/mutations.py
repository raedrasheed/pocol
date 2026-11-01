import sys
"""Independent source review controls: isolated TypeScript source, real assertions."""
import hashlib,json,os,shutil,subprocess,tempfile,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[4];MODEL=ROOT/'prototypes/lp3-model';OUT=Path(sys.argv[1]).resolve();OUT.mkdir(exist_ok=True)
mutants=[
 ('truncate-negative','model.ts','n>=0n?n/d:-((-n+d-1n)/d)','n/d'),
 ('wrong-polynomial','model.ts','195766423245049n*f','19576642324504n*f'),
 ('biased-sample','model.ts','x<((1n<<256n)/total)*total?x%total:null','x%total'),
 ('include-parent-weight','model.ts','r.height<=h-2n','r.height<=h-1n'),
 ('exclusive-membership','model.ts','m.last>=h-w||m.grace>=h-w','m.last>h-w||m.grace>h-w'),
 ('partition-gap','model.ts','start:BigInt(i)*maximum/BigInt(ids.length)','start:(BigInt(i)*maximum+BigInt(ids.length)-1n)/BigInt(ids.length)'),
 ('allow-reserved','model.ts','n===0n||n===(1n<<160n)-2n||(n>=0xc0c001n&&n<=0xc0c0ffn)','false'),
 ('late-deadline-equality','model.ts','return arrival<=deadline','return arrival<deadline'),
 ('allow-timestamp-limit','model.ts','if(upper>=(1n<<63n))','if(upper>(1n<<63n))'),
 ('skip-final-counter','model.ts','k<=0xffffffffn','k<0xffffffffn'),
 ('schema-ignore-width','genesis.ts','values[i]>>BigInt(width)','0n'),
 ('sha3-instead-of-keccak','genesis.ts','pad[0]=1;','pad[0]=6;'),
]
results=[]
for name,file,old,new in mutants:
 source=(MODEL/file).read_text();assert source.count(old)==1,(name,source.count(old))
 # Keep copied test files at the same repository-relative depth for immutable fixture URLs.
 with tempfile.TemporaryDirectory(prefix='ts-'+name+'-',dir=OUT) as td:
  td=Path(td);target=td/'prototypes/lp3-model';shutil.copytree(MODEL,target,ignore=shutil.ignore_patterns('node_modules'));os.symlink(ROOT/'prototypes/lp1-node',td/'prototypes/lp1-node',target_is_directory=True);os.symlink(ROOT/'development',td/'development',target_is_directory=True)
  (target/file).write_text(source.replace(old,new));args=['node','--experimental-strip-types',str(target/'tests.ts')];t=time.monotonic()
  with (OUT/(name+'.log')).open('w') as f:p=subprocess.run(args,stdout=f,stderr=subprocess.STDOUT)
  log=(OUT/(name+'.log')).read_text();caught=p.returncode!=0 and 'AssertionError' in log and ('tests 8' in log or '# tests 8' in log)
  results.append(dict(name=name,caught=caught,exitCode=p.returncode,seconds=round(time.monotonic()-t,3),args=args));(OUT/'RESULTS.json').write_text(json.dumps(results,indent=2)+'\n');print(name,caught,flush=True)
assert all(r['caught'] for r in results),[r for r in results if not r['caught']]
