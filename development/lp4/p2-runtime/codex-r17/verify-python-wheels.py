import pathlib,json,urllib.request,urllib.parse,hashlib,zipfile,io
from importlib import metadata
from pip._vendor.packaging.tags import sys_tags
from pip._vendor.packaging.utils import parse_wheel_filename
out=pathlib.Path('/workspace/work/lp4/p2-registry-full/r16-python-sources');out.mkdir(exist_ok=True);order={tag:i for i,tag in enumerate(sys_tags())};rows=[]
for dist in sorted(metadata.distributions(),key=lambda d:d.metadata['Name'].lower()):
 name=dist.metadata['Name'];version=dist.version
 url='https://pypi.org/pypi/'+urllib.parse.quote(name,safe='')+'/'+urllib.parse.quote(version,safe='')+'/json'
 info=json.load(urllib.request.urlopen(url,timeout=60));candidates=[]
 for f in info['urls']:
  if not f['filename'].endswith('.whl'):continue
  _,_,_,tags=parse_wheel_filename(f['filename']);rank=min([order[t] for t in tags if t in order],default=None)
  if rank is not None:candidates.append((rank,f))
 checked=None;attempts=[]
 for _,f in sorted(candidates,key=lambda x:(x[0],x[1]['filename'])):
  assert f['url'].startswith('https://files.pythonhosted.org/')
  data=urllib.request.urlopen(f['url'],timeout=60).read();digest=hashlib.sha256(data).hexdigest();assert digest==f['digests']['sha256']
  failures=[];count=0;notices=[]
  with zipfile.ZipFile(io.BytesIO(data)) as z:
   for path in z.namelist():
    if path.endswith('/'):continue
    p=pathlib.PurePosixPath(path);assert not p.is_absolute() and '..' not in p.parts
    if path.endswith('.dist-info/RECORD'):continue
    if '.data/' in path:raise AssertionError('explicit scheme mapping required')
    installed=dist.locate_file(path);original=z.read(path)
    if not installed.is_file() or installed.read_bytes()!=original:failures.append(path);continue
    count+=1
    if '.dist-info/' in path and any(w in path.upper() for w in ['LICENSE','COPYING','NOTICE','AUTHORS']):notices.append(dict(path=path,sha256=hashlib.sha256(original).hexdigest()))
  attempts.append(dict(filename=f['filename'],mismatchFiles=len(failures)))
  if not failures:
   (out/f['filename']).write_bytes(data);checked=dict(package=name,version=version,wheel=f['filename'],wheelSHA256=digest,source=f['url'],installedArchiveFilesVerified=count,notices=notices,priorCandidates=attempts[:-1]);break
 assert checked is not None,(name,'no upstream wheel reproduces installed bytes',attempts)
 rows.append(checked);print(name,checked['installedArchiveFilesVerified'],'installed bytes match SHA256-verified upstream wheel',flush=True)
 (out/'source-proof.json').write_text(json.dumps(dict(scope='Installed test-only Python tools match exact PyPI digest-verified wheel payloads; generated RECORD/pyc/installer metadata not counted; source rebuild/distribution audit NOT PASSED',packages=rows,bootable=False),indent=2)+'\n')
