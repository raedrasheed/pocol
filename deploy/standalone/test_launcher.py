#!/usr/bin/env python3
"""Portable black-box identity/invitation tests, plus opt-in five-process validation."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import socket
import subprocess
import tempfile
import time
import unittest
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
BIN = Path(os.environ.get('POCOLD_BIN_DIR', ROOT / 'prototypes/pocold/target/release'))
EXE = '.exe' if os.name == 'nt' else ''

def call(name, *args, ok=True):
    p = subprocess.run([str(BIN / (name + EXE)), *map(str, args)], capture_output=True, text=True, encoding="utf-8", timeout=90)
    if ok and p.returncode: raise AssertionError(p.stderr)
    return p

def identity(home):
    return json.loads(call('pocol', 'identity', '--home', home).stdout)['address']

def network(root, homes, seeds):
    ids = [identity(h) for h in homes]
    public = root / 'public.json'
    public.write_text(json.dumps({'chainId': 777915, 'genesisTime': int(time.time()) + 5,
        'registrationPerBlock': 1, 'miners': [{'id': a, 'reward': a} for a in ids[:3]],
        'accounts': [{'address': a, 'wei': str(10**24)} for a in ids]}), encoding='utf-8')
    call('pocold', 'init-testnet', '--config', public, '--out', root / 'genesis')
    args = ['network', '--from', root / 'genesis', '--out', root / 'network.pocol.json']
    for seed in seeds: args += ['--seed', seed]
    call('pocol', *args)
    return root / 'network.pocol.json'

class Launcher(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory(); self.addCleanup(self.tmp.cleanup); self.root = Path(self.tmp.name)
    def test_identity_is_stable_distinct_and_never_replaces_bad_key(self):
        home = self.root / 'node'
        first = identity(home); self.assertEqual(first, identity(home)); self.assertNotEqual(first, identity(self.root / 'other'))
        key = home / 'identity.pocol-key'; encoded = key.read_bytes()
        if os.name == 'nt': self.assertTrue(encoded.startswith(b'POCOL-DPAPI-USER-V1\0'))
        else: self.assertEqual(key.stat().st_mode & 0o777, 0o600); self.assertEqual(len(encoded), 32)
        key.write_bytes(b'broken')
        self.assertNotEqual(call('pocol', 'identity', '--home', home, ok=False).returncode, 0)
        self.assertEqual(key.read_bytes(), b'broken')
        key.write_bytes(encoded)
        self.assertEqual(first, identity(home))
        key.unlink()
        self.assertNotEqual(call('pocol', 'identity', '--home', home, ok=False).returncode, 0)
        self.assertFalse(key.exists())
        key.write_bytes(encoded)
        if os.name != 'nt': key.chmod(0o600)
        other = (self.root / 'other/identity.pocol-key').read_bytes()
        key.write_bytes(other)
        self.assertNotEqual(call('pocol', 'identity', '--home', home, ok=False).returncode, 0)
        key.write_bytes(encoded)
        hard = self.root / 'linked-key'; os.link(key, hard)
        self.assertNotEqual(call('pocol', 'identity', '--home', home, ok=False).returncode, 0)
        hard.unlink()
        if os.name == 'nt':
            corrupted = bytearray(encoded); corrupted[-1] ^= 1; key.write_bytes(corrupted)
            self.assertNotEqual(call('pocol', 'identity', '--home', home, ok=False).returncode, 0)
    def test_invitation_rejects_other_build_extra_paths_and_bad_seeds(self):
        path = network(self.root, [self.root / 'a'], ['127.0.0.1:30300'])
        original = json.loads(path.read_text(encoding='utf-8')); fingerprint = json.loads(call('pocol', 'inspect', '--network', path).stdout)['networkFingerprint']
        path.write_text(json.dumps(original, indent=4), encoding='utf-8'); self.assertEqual(json.loads(call('pocol', 'inspect', '--network', path).stdout)['networkFingerprint'], fingerprint)
        mutations = [dict(original, compatibilitySha256='0'*64), dict(original, seeds=[]), dict(original, seeds=['0.0.0.0:0']), dict(original, files={**original['files'], '../identity.pocol-key':'bad'}), dict(original, unexpected=True)]
        for changed in mutations:
            path.write_text(json.dumps(changed), encoding='utf-8'); self.assertNotEqual(call('pocol', 'inspect', '--network', path, ok=False).returncode, 0)
        path.write_bytes(b' ' * (1048576 + 1)); self.assertNotEqual(call('pocol', 'inspect', '--network', path, ok=False).returncode, 0)
    def test_build_digest_is_canonical_across_platforms(self):
        info=json.loads(call('pocol','build-info').stdout)
        h=hashlib.sha256(b'pocol-testnet-source-compatibility-v1\0')
        for name, expected in sorted(info['sourceSha256'].items()):
            data=(ROOT/'prototypes'/name).read_bytes(); encoded=name.encode('utf-8')
            self.assertEqual(hashlib.sha256(data).hexdigest(),expected,name)
            h.update(len(encoded).to_bytes(8,'big')); h.update(encoded)
            h.update(len(data).to_bytes(8,'big')); h.update(data)
        h.update(','.join(info['features']).encode('utf-8'))
        self.assertEqual(h.hexdigest(),info['compatibilitySha256'])
    def test_cli_rejects_ambiguous_options(self):
        for args in [('identity', '--home'), ('identity', '--unknown', 'x'), ('identity', '--home', 'a', '--home', 'b')]:
            self.assertNotEqual(call('pocol', *args, ok=False).returncode, 0)

def port():
    with socket.socket() as s: s.bind(('127.0.0.1', 0)); return s.getsockname()[1]

def rpc(port, method, params=None):
    req = urllib.request.Request(f'http://127.0.0.1:{port}', data=json.dumps({'jsonrpc':'2.0','id':1,'method':method,'params':params or []}).encode(), headers={'Content-Type':'application/json'})
    with urllib.request.urlopen(req, timeout=3) as response:
        value=json.load(response)
        if 'error' in value: raise RuntimeError(value['error'])
        return value['result']

def integration():
    with tempfile.TemporaryDirectory(prefix='pocol-standalone-') as tmp:
        root = Path(tmp); homes = [root / f'node{i}' for i in range(5)]
        # Reserve distinct ports until all selections have been made.
        reserved=[]
        for _ in range(10):
            s=socket.socket(); s.bind(('127.0.0.1',0)); reserved.append(s)
        ports=[s.getsockname()[1] for s in reserved]; p2p, api=ports[:5], ports[5:]
        invitation=network(root, homes, [f'127.0.0.1:{p}' for p in p2p[:3]])
        for s in reserved: s.close()
        procs=[]; logs=[]
        def start(i):
            log=open(root/f'node{i}.log','a',encoding='utf-8'); logs.append(log)
            return subprocess.Popen([str(BIN/('pocol'+EXE)), 'start', '--home', str(homes[i]), '--network', str(invitation), '--listen', f'127.0.0.1:{p2p[i]}', '--rpc', f'127.0.0.1:{api[i]}'], stdout=log, stderr=log)
        try:
            procs=[start(i) for i in range(5)]
            # Eight blocks can include multiple D_att/fallback windows after nonce exhaustion.
            # This is a liveness test, not an assertion of an eight-block 240-second SLA.
            deadline=time.monotonic()+600
            while time.monotonic()<deadline:
                if any(p.poll() is not None for p in procs): raise AssertionError('node exited: '+ '\n'.join((root/f'node{i}.log').read_text(encoding='utf-8',errors='replace')[-3000:] for i in range(5)))
                try:
                    heights=[int(rpc(p,'eth_blockNumber'),16) for p in api]
                    if min(heights)>=8: break
                except Exception: pass
                time.sleep(1)
            else: raise AssertionError('five nodes did not advance; last heights='+repr(locals().get('heights'))+'\n'+'\n'.join((root/f'node{i}.log').read_text(encoding='utf-8',errors='replace')[-5000:] for i in range(5)))
            h=min(heights)-6
            blocks=[rpc(p,'eth_getBlockByNumber',[hex(h),False]) for p in api]
            for field in ['hash','stateRoot','receiptsRoot']:
                assert len({b[field] for b in blocks})==1, field
            assert call('pocol','identity','--home',homes[4],ok=False).returncode != 0, 'live launcher lock'
            before=hashlib.sha256((homes[4]/'identity.pocol-key').read_bytes()).hexdigest()
            procs[4].terminate(); procs[4].wait(timeout=30); procs[4]=start(4)
            deadline=time.monotonic()+90
            while time.monotonic()<deadline:
                try:
                    if rpc(api[4],'eth_getBlockByNumber',[hex(h),False])['hash']==blocks[0]['hash']: break
                except Exception: pass
                time.sleep(1)
            else: raise AssertionError('restart did not recover confirmed block')
            assert hashlib.sha256((homes[4]/'identity.pocol-key').read_bytes()).hexdigest()==before
            print(json.dumps({'result':'PASS','scope':'five local native processes; three genesis miners, two independent validating joiners','height':h,'hash':blocks[0]['hash'],'identityPreserved':True,'realMultiHostProven':False,'androidDeviceProven':False}))
        finally:
            for p in procs:
                if p.poll() is None: p.terminate()
            for p in procs:
                try: p.wait(timeout=30)
                except subprocess.TimeoutExpired: p.kill(); p.wait()
            for log in logs: log.close()

if __name__=='__main__':
    parser=argparse.ArgumentParser(); parser.add_argument('--integration',action='store_true'); args,remaining=parser.parse_known_args()
    if args.integration: integration()
    else: unittest.main(argv=[__file__,*remaining])
