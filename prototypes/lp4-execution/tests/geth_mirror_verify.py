"""Verify official mirror module bytes against ORIGINAL pinned Geth go.sum h1.
No network, version substitution, ignored mismatches or EVM-source modification.
Usage: python geth_mirror_verify.py ORIGINAL_GETH_SOURCE GOMODCACHE MIRRORS.json OUTPUT.json TARGET_CLOSURE.json
"""
import pathlib,json,sys,hashlib,base64,zipfile

def hash1(entries):
    digest=hashlib.sha256()
    for name,data in sorted(entries.items()):
        assert '\n' not in name
        digest.update((hashlib.sha256(data).hexdigest()+'  '+name+'\n').encode())
    return 'h1:'+base64.b64encode(digest.digest()).decode()

def main():
    original=pathlib.Path(sys.argv[1]);cache=pathlib.Path(sys.argv[2]);mapping=json.loads(pathlib.Path(sys.argv[3]).read_text());sums={}
    for line in (original/'go.sum').read_text().splitlines():
        name,version,digest=line.split();sums[name,version]=digest
    closure=json.loads(pathlib.Path(sys.argv[5]).read_text());used={(m['Path'],m['Version']) for m in closure}
    rows=[];unused=[]
    for item in mapping:
        old=item['original'];version=item['version'];new=item['officialGitHubMirror']
        if (old,version) not in used:
            unused.append(dict(original=old,version=version,mirror=new,status='NOT IN TARGET CLOSURE; not credited as verified'))
            continue
        path=cache/'cache/download'/new/'@v'/(version+'.zip');data=path.read_bytes()
        entries={};canonical={};prefix=new+'@'+version+'/';replacement=old+'@'+version+'/'
        with zipfile.ZipFile(path) as z:
            for entry in z.infolist():
                assert not entry.is_dir() and entry.filename.startswith(prefix)
                name=entry.filename;relative=name[len(prefix):];assert '..' not in pathlib.PurePosixPath(relative).parts
                contents=z.read(entry);assert name not in entries;entries[name]=contents;canonical[replacement+relative]=contents
        mirror_h1=hash1(entries);assert mirror_h1==path.with_suffix('.ziphash').read_text().strip()
        original_h1=hash1(canonical);assert original_h1==sums[old,version],('official mirror differs from pinned original',old,version)
        rows.append(dict(original=old,version=version,mirror=new,files=len(entries),zipSHA256=hashlib.sha256(data).hexdigest(),mirrorH1=mirror_h1,originalH1=original_h1,identicalToOriginalGoSum=True))
    result=dict(scope='Module ZIP source content identity only; compiler/binary and remaining provenance separate',originalGoSumSHA256=hashlib.sha256((original/'go.sum').read_bytes()).hexdigest(),targetClosureSHA256=hashlib.sha256(pathlib.Path(sys.argv[5]).read_bytes()).hexdigest(),declaredMappings=len(mapping),unusedMappings=unused,verified=len(rows),modules=rows)
    pathlib.Path(sys.argv[4]).write_text(json.dumps(result,indent=2)+'\n');print(f'PASS {len(rows)} SAME-version official mirror source trees match canonical pinned Go module hashes')
if __name__=='__main__':main()
