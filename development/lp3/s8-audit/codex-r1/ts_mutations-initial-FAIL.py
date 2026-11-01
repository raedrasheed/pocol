"""Isolated compilable TS CF controls; a compile/import failure never counts as detection."""
import json,shutil,subprocess,sys,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[4];MODEL=ROOT/'prototypes/lp3-model';OUT=Path(sys.argv[1]).resolve();OUT.mkdir(parents=True,exist_ok=True)
mutants=[('truncate-negative', 'model.ts', 'n>=0n?n/d:-((-n+d-1n)/d)', 'n/d'), ('wrong-polynomial', 'model.ts', '195766423245049n*f', '19576642324504n*f'), ('biased-sample', 'model.ts', 'x<((1n<<256n)/total)*total?x%total:null', 'x%total'), ('include-parent-weight', 'model.ts', 'r.height<=h-2n', 'r.height<=h-1n'), ('exclusive-membership', 'model.ts', 'm.last>=h-w||m.grace>=h-w', 'm.last>h-w||m.grace>h-w'), ('partition-gap', 'model.ts', 'start:BigInt(i)*maximum/BigInt(ids.length)', 'start:(BigInt(i)*maximum+BigInt(ids.length)-1n)/BigInt(ids.length)'), ('allow-reserved', 'model.ts', 'n===0n||n===(1n<<160n)-2n||(n>=0xc0c001n&&n<=0xc0c0ffn)', 'false'), ('late-deadline-equality', 'model.ts', 'return arrival<=deadline', 'return arrival<deadline'), ('allow-timestamp-limit', 'model.ts', 'if(upper>=(1n<<63n))', 'if(upper>(1n<<63n))'), ('skip-final-counter', 'model.ts', 'k<=0xffffffffn', 'k<0xffffffffn'), ('schema-ignore-width', 'genesis.ts', 'values[i]>>BigInt(width)', '0n'), ('sha3-instead-of-keccak', 'genesis.ts', 'pad[0]=1;', 'pad[0]=6;')]
results=[]
for name,file,old,new in mutants:
 source=(MODEL/file).read_text();assert source.count(old)==1
 with tempfile.TemporaryDirectory(prefix='.cf-mut-',dir=ROOT/'prototypes') as td:
  td=Path(td)
  for f in MODEL.glob('*.ts'):shutil.copy2(f,td/f.name)
  shutil.copy2(MODEL/'package.json',td/'package.json');(td/file).write_text(source.replace(old,new))
  compiler=Path(sys.argv[2])
  a=['node',str(compiler),'--noEmit','--strict','--target','ES2022','--module','NodeNext','--moduleResolution','NodeNext','--allowImportingTsExtensions','--typeRoots',str(compiler.parents[2]/'@types'),*[str(f) for f in td.glob('*.ts')]]
  p=subprocess.run(a,capture_output=True,text=True);(OUT/(name+'-compile.log')).write_text(p.stdout+p.stderr);assert p.returncode==0,(name,p.stdout,p.stderr)
  p=subprocess.run(['node','--experimental-strip-types',str(td/'tests.ts')],capture_output=True,text=True);log=p.stdout+p.stderr;(OUT/(name+'-test.log')).write_text(log)
  caught=p.returncode!=0 and 'AssertionError' in log and 'ℹ tests 11' in log
  results.append(dict(name=name,typecheckExit=0,testExit=p.returncode,caught=caught));print(name,caught,flush=True)
(OUT/'RESULTS.json').write_text(json.dumps(results,indent=2)+'\n');assert all(v['caught'] for v in results)
