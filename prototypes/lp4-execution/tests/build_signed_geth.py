"""Build test-only Geth signed runner without modifying pinned upstream source.
Usage: python build_signed_geth.py GO GETH_SOURCE OUTPUT
Mirror replacements, if present in the supplied source build metadata, remain
explicit and need the existing separate mirror proof; no replacement is invented.
"""
import pathlib,subprocess,sys,json,hashlib,os,shutil,zipfile
binary,source,out=sys.argv[1],pathlib.Path(sys.argv[2]).resolve(),pathlib.Path(sys.argv[3]).resolve();out.mkdir(parents=True,exist_ok=True)
module=out/'module';module.mkdir(exist_ok=True);helper=pathlib.Path(__file__).with_name('gst_signed_geth.go');(module/'main.go').write_bytes(helper.read_bytes())
original=(source/'go.mod').read_text();assert original.startswith('module github.com/ethereum/go-ethereum\n')
(module/'go.mod').write_text(original.replace('module github.com/ethereum/go-ethereum','module pocol-signed-admission',1)+'\nrequire github.com/ethereum/go-ethereum v1.14.12\nreplace github.com/ethereum/go-ethereum => '+str(source)+'\n');shutil.copyfile(source/'go.sum',module/'go.sum')
command=[binary,'build','-mod=mod','-buildvcs=false','-p','2','-o',str(out/'signed-geth-control'),'.']
with (out/'build.log').open('w') as log:p=subprocess.run(command,cwd=module,stdout=log,stderr=subprocess.STDOUT)
assert p.returncode==0,'build failure is not acceptance'
info=subprocess.check_output([binary,'version','-m',str(out/'signed-geth-control')],text=True);(out/'build-info.txt').write_text(info)
cache=pathlib.Path(subprocess.check_output([binary,'env','GOMODCACHE'],text=True).strip());archive=cache/'cache/download/github.com/ethereum/go-ethereum/@v/v1.14.12.zip';same=0;metadata=[]
with zipfile.ZipFile(archive) as z:
 prefix='github.com/ethereum/go-ethereum@v1.14.12/'
 for n in z.namelist():
  if n.endswith('/'):continue
  assert n.startswith(prefix);rel=n.removeprefix(prefix);path=source/rel;assert path.is_file() and path.resolve().is_relative_to(source)
  if path.read_bytes()==z.read(n):same+=1
  else:assert rel in ['go.mod','go.sum'];metadata.append(rel)
receipt=dict(upstream='go-ethereum v1.14.12',upstreamZipSHA256=hashlib.sha256(archive.read_bytes()).hexdigest(),unchangedSourceFiles=same,changedBuildMetadataOnly=metadata,helperSHA256=hashlib.sha256(helper.read_bytes()).hexdigest(),binarySHA256=hashlib.sha256((out/'signed-geth-control').read_bytes()).hexdigest(),commands=command,buildExit=0,scope='external test tool only; no PoCol runtime linking, binary distribution or boot',bootable=False)
(out/'source-proof.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps({k:receipt[k] for k in ('unchangedSourceFiles','changedBuildMetadataOnly','helperSHA256','binarySHA256')}))
