"""Reproduce LP4 P1 roots only, not execution/resource/GenesisBuilder acceptance."""
import hashlib,json,os,subprocess,sys
from pathlib import Path
HERE=Path(__file__).resolve().parent;ROOT=HERE.parents[3];CRATE=ROOT/'prototypes/lp1-node';OUT=Path(sys.argv[1]).resolve();OUT.mkdir(parents=True,exist_ok=True)
env=dict(os.environ,CARGO_HOME='/workspace/pocol-tools/cargo',RUSTUP_HOME='/workspace/pocol-tools/rustup',CARGO_MANIFEST_DIR=str(CRATE));env['PATH']='/workspace/pocol-tools/cargo/bin:'+env['PATH'];records=[]
def run(name,args):
 with (OUT/(name+'.log')).open('w')as f:p=subprocess.run(args,cwd=ROOT,env=env,stdout=f,stderr=subprocess.STDOUT)
 records.append(dict(name=name,args=args,exitCode=p.returncode));(OUT/'commands.json').write_text(json.dumps(records,indent=2)+'\n');print(name,p.returncode,flush=True);assert p.returncode==0,name
manifest=['--manifest-path',str(CRATE/'Cargo.toml')]
run('rust-version',['rustc','+1.58.1','-vV']);run('python-version',['python3','--version'])
run('release-build',['cargo','+1.58.1','build','--release','--offline','--locked']+manifest)
run('format',['cargo','+1.58.1','fmt']+manifest+['--','--check'])
run('regression',['cargo','+1.58.1','test','--release','--offline','--locked']+manifest+['--lib',*[x for n in ['gate','param_gate','assignment','adversarial','adversarial_lp3','fixtures_integration','genesis_spec','profile','timing','asert_named','rewards','asert_alloc','envelope','finality_core','trie']for x in ['--test',n]]])
deps=CRATE/'target/release/deps';serde=list(deps.glob('libserde_json-*.rlib'));assert len(serde)==1
run('probe-compile',['rustc','+1.58.1','--edition=2021','-O',str(HERE/'probe.rs'),'--extern','lp1_node='+str(CRATE/'target/release/liblp1_node.rlib'),'-L','dependency='+str(deps),'--extern','serde_json='+str(serde[0]),'-o',str(OUT/'probe')])
run('oracle',['python3',str(HERE/'oracle.py'),str(OUT)])
assert hashlib.sha256((OUT/'vectors.json').read_bytes()).digest()==hashlib.sha256((CRATE/'tests/data/lp4-trie-vectors.json').read_bytes()).digest()
run('mutations',['python3',str(HERE/'mutations.py'),str(OUT/'mutations')])
for row in json.loads((OUT/'mutations/RESULTS.json').read_text()):
 assert row['caught'] and row['commands'][0]['exitCode']==row['commands'][1]['exitCode']==0
 assert 'assertion failed' in (OUT/'mutations'/(row['name']+'-2.log')).read_text()
print('PASS P1 root primitive only; all deferred LP4 execution/resource gates NOT PASSED; boot off')
