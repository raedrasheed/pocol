"""Isolated compilable TS CF controls; a compile/import failure never counts as detection."""
import json,shutil,subprocess,sys,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[4];MODEL=ROOT/'prototypes/lp3-model';OUT=Path(sys.argv[1]).resolve();OUT.mkdir(parents=True,exist_ok=True)
mutants=[('cf1-time','h>=hClose||ts>=tClose','h>=hClose'),('cf2-sticky','if(first!==null)return cfU64(first);','if(false)return cfU64(first!);'),('cf3-rounding','(records+ret-1n)/ret','records/ret'),('cf4-equality','h>=a||ts>=b','h>a||ts>b'),('cf5-cursor','p.closing&&p.cursorDone','p.closing&&true'),('cf6-final-parent','!parentFinal&&h<=hEnd','h<=hEnd')]
results=[]
for name,old,new in mutants:
 source=(MODEL/'model.ts').read_text();assert source.count(old)==1
 with tempfile.TemporaryDirectory(prefix='.cf-mut-',dir=ROOT/'prototypes') as td:
  td=Path(td)
  for f in MODEL.glob('*.ts'):shutil.copy2(f,td/f.name)
  shutil.copy2(MODEL/'package.json',td/'package.json');(td/'model.ts').write_text(source.replace(old,new))
  compiler=Path(sys.argv[2])
  a=['node',str(compiler),'--noEmit','--strict','--target','ES2022','--module','NodeNext','--moduleResolution','NodeNext','--allowImportingTsExtensions','--typeRoots',str(compiler.parents[2]/'@types'),*[str(f) for f in td.glob('*.ts')]]
  p=subprocess.run(a,capture_output=True,text=True);(OUT/(name+'-compile.log')).write_text(p.stdout+p.stderr);assert p.returncode==0,(name,p.stdout,p.stderr)
  p=subprocess.run(['node','--experimental-strip-types',str(td/'tests.ts')],capture_output=True,text=True);log=p.stdout+p.stderr;(OUT/(name+'-test.log')).write_text(log)
  caught=p.returncode!=0 and '✖ newly approved CF1-CF6' in log and 'ℹ fail 1' in log
  results.append(dict(name=name,typecheckExit=0,testExit=p.returncode,caught=caught));print(name,caught,flush=True)
(OUT/'RESULTS.json').write_text(json.dumps(results,indent=2)+'\n');assert all(v['caught'] for v in results)
