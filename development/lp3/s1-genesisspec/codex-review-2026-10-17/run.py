import datetime, json, os, pathlib, subprocess, time
ROOT = pathlib.Path('/workspace/Proof-of-Collaboration')
OUT = pathlib.Path('/workspace/pocol-review')
env = dict(os.environ, CARGO_HOME='/workspace/pocol-tools/cargo', RUSTUP_HOME='/workspace/pocol-tools/rustup', CARGO_TARGET_DIR='/workspace/pocol-tools/target', PYTHONDONTWRITEBYTECODE='1')
env['PATH'] = '/workspace/pocol-tools/cargo/bin:' + env['PATH']
records = []
def run(name, args, cwd=ROOT / 'prototypes/lp1-node'):
    start=time.time()
    with (OUT/(name+'.log')).open('w') as f:
        p=subprocess.run(args,cwd=cwd,env=env,stdout=f,stderr=subprocess.STDOUT)
    records.append(dict(name=name,command=args,cwd=str(cwd),exitCode=p.returncode,seconds=round(time.time()-start,2)))
    (OUT/'commands.json').write_text(json.dumps(records,indent=2)+'\n')
    print(name,p.returncode,flush=True)
    return p.returncode
run('rust-version',['rustc','-vV'])
run('cargo-version',['cargo','-V'])
run('python-version',['python3','--version'])
assert run('build',['cargo','build','--locked','--offline'])==0
assert run('targeted-tests',['cargo','test','--locked','--offline','--lib','--test','genesis_spec','--test','genesis_batch_alloc'])==0
assert run('fresh-seed-differential',['python3','tests/lp3_genesis_diff.py','--bin','/workspace/pocol-tools/target/debug/lp1-node','--m1','../../development/m1','--seed','20261017','--out',str(OUT/'fresh-seed-differential.json')])==0
assert run('cli-memory',['python3','tests/lp3_genesis_cli.py','--bin','/workspace/pocol-tools/target/debug/lp1-node','--m1','../../development/m1','--out',str(OUT/'cli-memory.json')])==0
assert run('windows-evidence',['python3','-I','development/lp3/s1-genesisspec/windows-run-37976178670/check_evidence.py','--repo','.'],ROOT)==0
