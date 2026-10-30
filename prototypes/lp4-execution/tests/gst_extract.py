"""Safely extract the pinned upstream Cancun control corpus; never display keys."""
import hashlib,json,pathlib,sys,tarfile
archive=pathlib.Path(sys.argv[1]);out=pathlib.Path(sys.argv[2]);receipts=json.load(open(sys.argv[3]))
expected=next(x['sha256'] for x in receipts['archives'] if x['name']=='legacy.tar.gz')
assert hashlib.sha256(archive.read_bytes()).hexdigest()==expected,'archive mismatch'
prefix='legacytests-'+receipts['legacyTestsGitlink']+'/Cancun/GeneralStateTests/'
files=variants=0
with tarfile.open(archive) as tar:
 for member in tar:
  if not member.isfile() or not member.name.startswith(prefix) or not member.name.endswith('.json'):continue
  rel=pathlib.PurePosixPath(member.name[len(prefix):]);assert not rel.is_absolute() and '..' not in rel.parts
  data=tar.extractfile(member).read();target=out.joinpath(*rel.parts);target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(data)
  files+=1;variants+=sum(len(x.get('post',{}).get('Cancun',[])) for x in json.loads(data).values())
assert (files,variants)==(2807,20661),(files,variants)
print(f'Extracted {files} files, {variants} Cancun post variants; no fixture key values displayed')
