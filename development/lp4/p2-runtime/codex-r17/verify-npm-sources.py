import pathlib,json,urllib.request,hashlib,base64,tarfile,io
repo=pathlib.Path('/workspace/Proof-of-Collaboration');modules=pathlib.Path('/workspace/work/lp4/p2-pay/tools/node_modules');out=pathlib.Path('/workspace/work/lp4/p2-registry-full/r16-npm-sources');out.mkdir(exist_ok=True);rows=[]
lock=json.loads((repo/'prototypes/lp4-execution/tools/package-lock.json').read_text())
for location,p in lock['packages'].items():
 if not location:continue
 name=location.removeprefix('node_modules/');url=p['resolved'];assert url.startswith('https://registry.npmjs.org/')
 data=urllib.request.urlopen(url,timeout=60).read();alg,expected=p['integrity'].split('-',1);assert base64.b64encode(hashlib.new(alg,data).digest()).decode()==expected
 (out/(name.replace('/','_')+'.tgz')).write_bytes(data);checked=0;notices=[]
 with tarfile.open(fileobj=io.BytesIO(data),mode='r:gz') as tar:
  for m in tar.getmembers():
   assert not pathlib.PurePosixPath(m.name).is_absolute() and '..' not in pathlib.PurePosixPath(m.name).parts
   if not m.isfile():continue
   assert m.name.startswith('package/');rel=m.name[len('package/'):];local=modules/name/rel;original=tar.extractfile(m).read();assert local.is_file() and local.read_bytes()==original,(name,rel,'archive/installed mismatch');checked+=1
   if any(w in pathlib.Path(rel).name.upper() for w in ['LICENSE','NOTICE','COPYING','COPYRIGHT']):notices.append(dict(path=rel,sha256=hashlib.sha256(original).hexdigest()))
 rows.append(dict(package=name,version=p['version'],archiveSHA256=hashlib.sha256(data).hexdigest(),lockedIntegrity=p['integrity'],filesVerified=checked,notices=notices));print(name,checked,'source bytes match locked archive',flush=True)
(out/'source-proof.json').write_text(json.dumps(dict(scope='All9 locked npm test/build-tool archives and installed file byte equivalence; compiler rebuild/distribution audit NOT PASSED',packages=rows,bootable=False),indent=2)+'\n')
