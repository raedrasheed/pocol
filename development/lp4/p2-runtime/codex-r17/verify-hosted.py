import pathlib,json,re,gzip,hashlib
repo=pathlib.Path('/workspace/Proof-of-Collaboration');root=pathlib.Path('/workspace/work/lp4/p2-registry-full/hosted-39394783398');checks={}
for label,p in [('linux',root/'lp4-p2-controls/lp4-controls/rust-tests.log'),('windows',root/'lp4-native-windows/rust-tests.log')]:
 values=re.findall(r'test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored',p.read_text());assert sum(int(v[0]) for v in values)==43 and all(v[1:]==('0','0') for v in values);checks[label+'RustTests']=43
base=root/'lp4-p2-controls/lp4-controls';context=json.loads((base/'context-oracle.json').read_text());assert len(context['cases'])==9 and all(x['comparison']=='MATCH' for x in context['cases']);checks['contextIndependentRoots']=9
reg=json.loads((base/'registry-oracle.json').read_text());prior=json.loads(gzip.decompress((repo/'development/lp4/p2-runtime/codex-r8/oracle-expanded.json.gz').read_bytes()));assert reg['cases']==prior['cases'];checks['registryCompleteComparisons']=63
raw=(base/'gst/cases.jsonl').read_bytes();old=gzip.decompress((repo/'development/lp4/p2-runtime/codex-r4/gst-source-final-cases.jsonl.gz').read_bytes());assert raw==old;assert len(raw.splitlines())==20661;checks['primaryCancun']=dict(total=20661,rawSHA256=hashlib.sha256(raw).hexdigest(),matchesPublishedR4=True)
for p in root.rglob('*results.json'):
 d=json.loads(p.read_text())
 if not isinstance(d,list) or not d or not all('assertionKilled' in x for x in d):continue
 for x in d:
  keys={k for k in ['buildExit','solidityBuildExit','rustBuildExit'] if k in x};assert keys in [{'buildExit'},{'solidityBuildExit','rustBuildExit'}],str(p)
  assert x['assertionKilled'] and all(x[k]==0 for k in keys)
  if 'testExit' in x:assert x['testExit']!=0
  elif 'runExit' in x:assert x['runExit']!=0
  else:assert x['rootAssertionFailures']>0 and x['baselinePasses']>0
 checks[str(p.relative_to(root))]=len(d)
geth=json.loads((root/'lp4-native-cancun-reference/geth-results/summary.json').read_text());assert geth['counts']['total']==20661 and geth['complete'] and geth['exit']==0 and not geth['limited'];checks['nativeGeth']=geth
checks.update(sourceHead='c6e56b069ad3d5a7c1323cae81ea53004f542512',sourceTree='d643038c4a6210587c14c364a041f161c14980b7',run=39394783398,independentPython='RUNNING; NOT PASSED',P2='NOT PASSED',LP4='NOT PASSED',bootable=False)
(root/'verified-scope.json').write_text(json.dumps(checks,indent=2)+'\n');print(json.dumps(checks))
