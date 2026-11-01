"""Reviewer-authored byte construction and Python integer oracle; no project harness imports."""
import hashlib, json, pathlib, random, subprocess, sys, tempfile
sys.set_int_max_str_digits(0)
ROOT=pathlib.Path('/workspace/Proof-of-Collaboration')
BIN='/workspace/pocol-tools/target/debug/lp1-node'
def head(n,base):
    if n<56:return bytes([base+n])
    b=n.to_bytes((n.bit_length()+7)//8,'big')
    return bytes([base+55+len(b)])+b
def item(b):
    return b if len(b)==1 and b[0]<128 else head(len(b),128)+b
def seq(items):
    b=b''.join(items);return head(len(b),192)+b
def literal(t):
    if isinstance(t,list):return seq([literal(x) for x in t])
    if isinstance(t,str):return bytes.fromhex(t)
    def raw(x):
        if isinstance(x,str):return bytes.fromhex(x)
        if 'repeat' in x:return bytes.fromhex(x['repeat'])*x['count']
        return b''.join(raw(y) for y in x['concat'])
    return raw(t)
cases=[]
def add(name,b,code,detail):cases.append((name,b,code,detail))
rng=random.Random(2026101701)
sizes=[0,1,2,16,17,31,32,33,63,64,65,255,256,257,511,512,513,1023,1024,1025,4095,4096,4097,8192]
sizes += [rng.randrange(1,10000) for _ in range(80)]
for i,n in enumerate(sizes):
    b=rng.randbytes(n)
    if int.from_bytes(b,'big')==1:b=b'\x02'
    for prefix in [b'',b'\x00\x00']:
        v=prefix+b
        add(f'decimal-{i}-{len(prefix)}',seq([item(v)]),'gsVersion',str(int.from_bytes(v,'big')))
for name,b,detail in [('str64',b'\xbf'+b'\xff'*8,'truncated string'),('list64',b'\xff'+b'\xff'*8,'truncated list'),('cross',b'\xc1\x82\x01\x02','item crosses list end')]:
    add(name,b,'L0',detail)
add('wrapped-version-count',seq([b'\x81\x01']),'gsCount','top 1')
add('structure-before-version',seq([item(b'\x02'),seq([])]),'gsStructure','top[1]')
for depth in [1500,1501,100000]:
    length=1;heads=[]
    for _ in range(depth):
        h=head(length,192);heads.append(h);length+=len(h)
    add(f'deep-{depth}',b''.join(reversed(heads))+b'\x01','gsStructure','top[0]')
doc=json.loads((ROOT/'development/m1/m1-draft-0.21/vectors/v3-gsv1.json').read_text())
g=literal(doc['tree'])
assert len(g)==341
with tempfile.TemporaryDirectory() as d:
    p=pathlib.Path(d); inp=p/'input';out=p/'output'
    data=b''.join(b.hex().encode()+b'\n' for _,b,_,_ in cases)+g.hex().encode()+b'\n'
    inp.write_bytes(data)
    proc=subprocess.run([BIN,'genesis-decode','--in',str(inp),'--out',str(out)],capture_output=True)
    assert proc.returncode==0,(proc.returncode,proc.stderr)
    rows=[json.loads(s) for s in out.read_text().splitlines()]
    assert len(rows)==len(cases)+2
    for (name,b,code,detail),row in zip(cases,rows):
        assert row.get('code')==code and row.get('detail')==detail,(name,row,code,detail[:80])
    good=rows[-2]
    assert good['ok'] and good['reencodeEqual'] and good['bootable'] is False
    print(json.dumps(dict(ok=True,rejectionCases=len(cases),validGsv1=good,seed=2026101701,inputSha256=hashlib.sha256(data).hexdigest(),summary=rows[-1]),indent=2))
