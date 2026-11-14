#!/usr/bin/env python3
"""Portable black-box identity/invitation tests, plus opt-in five-process validation."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import time
import unittest
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'deploy/public-testnet'))
import pocol_identity  # noqa: E402  (the reference consensus identity, VA-SPEC §2.1)
BIN = Path(os.environ.get('POCOLD_BIN_DIR', ROOT / 'prototypes/pocold/target/release'))
EXE = '.exe' if os.name == 'nt' else ''

def call(name, *args, ok=True):
    p = subprocess.run([str(BIN / (name + EXE)), *map(str, args)], capture_output=True, text=True, encoding="utf-8", timeout=90)
    if ok and p.returncode: raise AssertionError(p.stderr)
    return p

def identity(home):
    return json.loads(call('pocol', 'identity', '--home', home).stdout)['address']

def network(root, homes, seeds, full_nonce=True):
    """A test network. `full_nonce` (default) gives nonceMode 1, the full nonce range, so the chain never
    stalls on an exhausted attempt; the full-nonce upgrade tests pass False to start from the v1 range."""
    ids = [identity(h) for h in homes]
    public = root / 'public.json'
    config = {'chainId': 777915, 'genesisTime': int(time.time()) + 5,
        'registrationPerBlock': 1, 'miners': [{'id': a, 'reward': a} for a in ids[:3]],
        'accounts': [{'address': a, 'wei': str(10**24)} for a in ids]}
    if full_nonce: config['fullNonce'] = True
    public.write_text(json.dumps(config), encoding='utf-8')
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
        mutations = [dict(original, compatibilitySha256='0'*64), dict(original, schema='pocol-network-v1'), dict(original, seeds=[]), dict(original, seeds=['0.0.0.0:0']), dict(original, files={**original['files'], '../identity.pocol-key':'bad'}), dict(original, unexpected=True)]
        for changed in mutations:
            path.write_text(json.dumps(changed), encoding='utf-8'); self.assertNotEqual(call('pocol', 'inspect', '--network', path, ok=False).returncode, 0)
        # A file of another schema says so (with or without that schema's other fields), not that it is malformed.
        for changed in (dict(original, schema='pocol-network-v1'), dict(original, schema='pocol-network-v1', compatibilitySha256='0'*64)):
            path.write_text(json.dumps(changed), encoding='utf-8'); p = call('pocol', 'inspect', '--network', path, ok=False)
            self.assertNotEqual(p.returncode, 0); self.assertIn('uses schema pocol-network-v1, which this release does not read', p.stderr)
        path.write_bytes(b' ' * (1048576 + 1)); self.assertNotEqual(call('pocol', 'inspect', '--network', path, ok=False).returncode, 0)
    def test_pocol_network_binds_the_rules_and_inspect_upgrade_is_a_dry_run(self):
        # T27 (plan §1.7): `pocol network` writes v2 bound to the consensus rules, never to the build digest; the dry
        # run names the change and the identity a released node will report, and writes nothing.
        home = self.root / 'node'
        path = network(self.root, [home], ['127.0.0.1:30300'], full_nonce=False)
        saved = json.loads(path.read_text(encoding='utf-8'))
        rules = json.loads(call('pocol', 'build-info').stdout)['consensusRulesVersion']
        self.assertEqual((saved['schema'], saved['consensusRulesVersion']), ('pocol-network-v2', rules))
        self.assertNotIn('compatibilitySha256', saved)
        call('pocol', 'import', '--home', home, '--network', path)
        binding = (home / 'network.pocol.json').read_bytes()
        info = json.loads(saved['files']['devnet.json'])
        same = json.loads(call('pocol', 'inspect-upgrade', '--home', home, '--network', path).stdout)
        self.assertEqual((same['change'], same['appended'], same['invitationSchema']), ('none', [], 'pocol-network-v2'))
        self.assertEqual(same['consensusIdentity'], pocol_identity.consensus_id(info['chainId'], info['genesisHash'], rules, 0, [(1, 0)]))
        target = self.root / 'upgraded.pocol.json'
        target.write_text(json.dumps(dict(saved, files=dict(saved['files'], **{'devnet.json': json.dumps(dict(info, forks=[[1, 0], [2, 100000]]))}))), encoding='utf-8')
        dry = json.loads(call('pocol', 'inspect-upgrade', '--home', home, '--network', target).stdout)
        self.assertEqual((dry['change'], dry['appended'], dry['activations']), ('activation', [[2, 100000]], [[2, 100000]]))
        self.assertEqual(dry['consensusIdentity'], pocol_identity.consensus_id(info['chainId'], info['genesisHash'], rules, 0, [(1, 0), (2, 100000)]))
        self.assertEqual((home / 'network.pocol.json').read_bytes(), binding, 'the dry run writes nothing')
        self.assertFalse((home / 'network-upgrade.pending.json').exists())
        absent = self.root / 'absent'
        self.assertNotEqual(call('pocol', 'inspect-upgrade', '--home', absent, '--network', target, ok=False).returncode, 0)
        self.assertFalse(absent.exists())
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
    def test_build_inventory_binds_local_crypto_dependencies(self):
        info = json.loads(call('pocold', 'build-info').stdout)
        required = {
            f'third-party/evm-crypto/{crate}/{name}'
            for crate in ('alloy-primitives-0.8.26', 'alloy-eip7702-0.5.1', 'revm-precompile-16.2.0')
            for name in ('Cargo.toml', 'src/lib.rs')
        }
        self.assertTrue(required <= info['sourceSha256'].keys(),
                        'Local dependency code must be bound to the native compatibility identity')
    def test_network_writes_a_bootstrap_list_and_retired_flag(self):
        # PR2b (D17, D23): `pocol network` writes the optional bootstrap list and seedsRetired into a v2 file, outside
        # the network's identity; `inspect` shows them; a bad list or flag is refused.
        args = ['network', '--from', None, '--seed', '127.0.0.1:30300', '--bootstrap', '127.0.0.1:30301', '--bootstrap', '127.0.0.1:30302', '--seeds-retired', 'true']
        plain = network(self.root, [self.root / 'a'], ['127.0.0.1:30300'])
        listed = self.root / 'listed.pocol.json'
        call('pocol', *[self.root / 'genesis' if a is None else a for a in args], '--out', listed)
        value = json.loads(listed.read_text(encoding='utf-8'))
        self.assertEqual((value['bootstrap'], value['seedsRetired']), (['127.0.0.1:30301', '127.0.0.1:30302'], True))
        self.assertNotIn('bootstrap', json.loads(plain.read_text(encoding='utf-8')), 'absent when not given')
        shown = json.loads(call('pocol', 'inspect', '--network', listed).stdout)
        self.assertEqual((shown['bootstrap'], shown['seedsRetired']), (['127.0.0.1:30301', '127.0.0.1:30302'], True))
        for bad in (['--bootstrap', '127.0.0.1:80'], ['--bootstrap', '127.0.0.1:30300'], ['--seeds-retired', 'yes']):
            out = self.root / 'bad.pocol.json'
            self.assertNotEqual(call('pocol', 'network', '--from', self.root / 'genesis', '--seed', '127.0.0.1:30300', *bad, '--out', out, ok=False).returncode, 0, bad)
            self.assertFalse(out.exists())
    def test_start_accepts_repeated_peers_and_rejects_bad_ones(self):
        # PR2b: `pocol start --peer` may repeat (static peers besides the invitation's seeds); a bad one is refused
        # before the node starts.
        home = self.root / 'node'; path = network(self.root, [home], ['127.0.0.1:30300'])
        call('pocol', 'import', '--home', home, '--network', path)
        for bad in ('0.0.0.0:30300', '127.0.0.1:0', 'x'):
            p = call('pocol', 'start', '--home', home, '--peer', '127.0.0.1:30301', '--peer', bad, ok=False)
            self.assertNotEqual(p.returncode, 0); self.assertIn('--peer', p.stderr)
        p = call('pocol', 'start', '--home', home, '--peer', '127.0.0.1:30301', '--peer', '127.0.0.1:30301', ok=False)
        self.assertIn('given twice', p.stderr)
    def test_cli_rejects_ambiguous_options(self):
        for args in [('identity', '--home'), ('identity', '--unknown', 'x'), ('identity', '--home', 'a', '--home', 'b')]:
            self.assertNotEqual(call('pocol', *args, ok=False).returncode, 0)
    def test_key_backup_restores_the_account_into_another_home(self):
        # CRY-02: the same encrypted format on Linux (0600 raw key) and Windows (DPAPI key); passphrases on stdin.
        def run(*args, stdin, ok=True):
            p = subprocess.run([str(BIN / ('pocol' + EXE)), *map(str, args)], input=stdin, capture_output=True,
                               text=True, encoding='utf-8', timeout=90)
            if ok and p.returncode: raise AssertionError(p.stderr)
            return p
        old, new, backup = self.root / 'old', self.root / 'new', self.root / 'wallet.pocol-backup'
        phrase = 'a long backup passphrase ✓'
        self.assertIn('existing PoCol home', run('export-key', '--home', old, '--out', backup, stdin=f'{phrase}\n{phrase}\n', ok=False).stderr)
        self.assertFalse(old.exists(), 'export never creates a wallet')
        address = identity(old)
        self.assertIn('differ', run('export-key', '--home', old, '--out', backup, stdin=f'{phrase}\nother passphrase!\n', ok=False).stderr)
        self.assertIn('at least 12', run('export-key', '--home', old, '--out', backup, stdin='short\nshort\n', ok=False).stderr)
        self.assertIn('too easy to guess', run('export-key', '--home', old, '--out', backup, stdin='Password2024!\nPassword2024!\n', ok=False).stderr)
        self.assertFalse(backup.exists())
        out = json.loads(run('export-key', '--home', old, '--out', backup, stdin=f'{phrase}\n{phrase}\n').stdout)
        self.assertEqual(out['address'], address)
        data = backup.read_bytes()
        self.assertEqual((len(data), data[:17]), (98, b'POCOL-KEY-BACKUP\x01'))
        if os.name != 'nt':
            self.assertEqual(backup.stat().st_mode & 0o777, 0o600)
            self.assertNotIn((old / 'identity.pocol-key').read_bytes(), data)
        self.assertIn('never replaced', run('export-key', '--home', old, '--out', backup, stdin=f'{phrase}\n{phrase}\n', ok=False).stderr)
        self.assertEqual(backup.read_bytes(), data)
        absent = self.root / 'absent'
        self.assertIn('existing PoCol home', run('inspect-key-backup', '--home', absent, '--backup', backup, stdin=phrase + '\n', ok=False).stderr)
        self.assertFalse(absent.exists(), 'inspecting never creates a home')
        current = identity(new)
        preview = json.loads(run('inspect-key-backup', '--home', new, '--backup', backup, stdin=phrase + '\n').stdout)
        self.assertEqual((preview['address'], preview['current'], preview['replaces']), (address, current, True))
        self.assertIn('Wrong passphrase', run('import-key', '--home', new, '--backup', backup, '--replace', current, stdin='a wrong passphrase\n', ok=False).stderr)
        self.assertIn('Confirm replacing', run('import-key', '--home', new, '--backup', backup, stdin=phrase + '\n', ok=False).stderr)
        self.assertEqual(identity(new), current)
        # Text-mode stdin writes os.linesep on Windows; a CRLF line ending is accepted on every platform.
        ending = '\n' if os.name == 'nt' else '\r\n'
        done = json.loads(run('import-key', '--home', new, '--backup', backup, '--replace', current, stdin=phrase + ending).stdout)
        self.assertEqual((done['changed'], done['previous']), (True, current))
        self.assertEqual(identity(new), address)
        self.assertTrue((new / done['previousKeyFile']).is_file(), 'the replaced key is kept')

def port():
    with socket.socket() as s: s.bind(('127.0.0.1', 0)); return s.getsockname()[1]

BUSY_RETRIES = 20

def rpc(port, method, params=None):
    # -32021 is the node's load-shedding answer; a client waits retryAfterMs and asks again.
    for attempt in range(BUSY_RETRIES + 1):
        req = urllib.request.Request(f'http://127.0.0.1:{port}', data=json.dumps({'jsonrpc':'2.0','id':1,'method':method,'params':params or []}).encode(), headers={'Content-Type':'application/json'})
        with urllib.request.urlopen(req, timeout=3) as response:
            value=json.load(response)
        error=value.get('error')
        if error is None: return value['result']
        delay=(error.get('data') or {}).get('retryAfterMs') if isinstance(error, dict) else None
        if error.get('code')!=-32021 or not isinstance(delay, int) or attempt==BUSY_RETRIES: raise RuntimeError(error)
        time.sleep(delay/1000)

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
