import json,sys,pathlib,subprocess
sys.path.insert(0,str(pathlib.Path(__file__).resolve().parent));from gst_control import root,loghash
corpus,binary,receipts,output=sys.argv[1:];out=[]
for line in open(receipts):
 row=json.loads(line)
 if row['controlPass']:continue
 t=json.load(open(pathlib.Path(corpus)/row['file']))[row['name']];post=t['post']['Cancun'][row['variant']]
 record={'file':row['file'],'name':row['name'],'variant':row['variant'],'expectedRoot':post.get('hash'),'expectedException':post.get('expectException')}
 for probe in [False,True]:
  payload={'pre':t['pre'],'env':t['env'],'transaction':{k:v for k,v in t['transaction'].items() if k!='secretKey'},'indexes':post['indexes'],'storageCollisionProbe':probe}
  r=json.loads(subprocess.check_output([binary],input=json.dumps(payload)+'\n',text=True))
  if r.get('collisionRule'):raise RuntimeError('Historical alternatives require the original R2 diagnostic binary; current execution is approved LP4-CREATE-01')
  record['probe' if probe else 'unamended']={'error':r.get('error')} if 'error'in r else {'root':root(r['state']),'gasUsed':r['gasUsed'],'logs':loghash(r['logs'])}
 out.append(record)
pathlib.Path(output).write_text(json.dumps(out,indent=2)+'\n')
for r in out:print(r['file'],r['variant'],'probeMatches',r['probe'].get('root')==r['expectedRoot'] if not r['expectedException'] else 'error' in r['probe'])
