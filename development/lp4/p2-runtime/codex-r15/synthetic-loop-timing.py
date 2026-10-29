"""Synthetic scaled-workload timing only; NEVER original-case acceptance."""
import copy,json,pathlib,sys,time
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parents[4]/'prototypes/lp4-execution/tests'))
from gst_independent import execute
p=pathlib.Path(sys.argv[1])/'VMTests/vmPerformance/loopMul.json'
original=json.loads(p.read_text())['loopMul'];rows=[]
for variant in [0,1,2]:
 for loops in [1000,10000]:
  test=copy.deepcopy(original);test['transaction'].pop('secretKey',None)
  post=test['post']['Cancun'][variant];ix=post['indexes']['data'];data=test['transaction']['data'][ix]
  original_loops=int(data[-64:],16);test['transaction']['data'][ix]=data[:-64]+f'{loops:064x}'
  start=time.perf_counter();cpu=time.process_time();result=execute(test,post,'lp4-create01-modexp-admission')
  rows.append(dict(variant=variant,syntheticLoopCount=loops,originalLoopCount=original_loops,wallSeconds=time.perf_counter()-start,cpuSeconds=time.process_time()-cpu,executionSuccess=result['executionSuccess']))
  print(rows[-1],flush=True)
pathlib.Path(sys.argv[2]).write_text(json.dumps(dict(scope='SYNTHETIC scaled workload timing; original20661 acceptance NOT established',cases=rows,bootable=False),indent=2)+'\n')
