#!/usr/bin/env python3
"""`pocol register` with the miner key in an encrypted systemd credential (Stage 4 PR A1), on a real systemd host.

Root only, on a disposable Linux host whose PID 1 is systemd (CI runs it on the ubuntu-24.04 runner in
public-mainnet-e2e.yml); it refuses anywhere else, and refuses a host that already has a pocol-standalone install.
It follows the documented procedure with the real scripts, not stand-ins:
- install-linux.sh installs the release binaries, the pocol-standalone user, its identity and the service unit;
- the service joins a local three-founder v3 devnet (the founders run as plain processes);
- `pocol export-key` writes the passphrase-encrypted backup (passphrase through a pipe), and encrypt-server-key.sh
  --backup seals the key into /etc/credstore.encrypted/pocol-identity, restarts the service with
  LoadCredentialEncrypted= and shreds the plaintext key file;
- with the service running, the README's command registers the identity:
  systemd-run --pipe --wait -p User=pocol-standalone -p LoadCredentialEncrypted=pocol-identity:CRED
  pocol register confirm --home /var/lib/pocol-standalone --key-credential pocol-identity.
It checks the refusals first (no key file, no $CREDENTIALS_DIRECTORY, the wrong user, a "no" answer), that the service
is never restarted and then mines with the identity, that nothing in the home is written, and that neither the secret
key (from the credential, decrypted in memory only) nor the backup passphrase appears in any output or journal line.
"""
import json, os, shutil, subprocess, sys, tempfile, time
from pathlib import Path
from test_launcher import BIN, call, identity, port, rpc

HERE = Path(__file__).resolve().parent
OPT, STATE = Path('/opt/pocol-standalone'), Path('/var/lib/pocol-standalone')
UNIT, DROPIN_DIR = Path('/etc/systemd/system/pocol-standalone.service'), Path('/etc/systemd/system/pocol-standalone.service.d')
CRED, USER, SERVICE = Path('/etc/credstore.encrypted/pocol-identity'), 'pocol-standalone', 'pocol-standalone.service'
RPC_PORT, CHAIN = 8545, 777937
PASSPHRASE = 'register credential test passphrase 7c1e'

def sh(*args, input=None, ok=True, timeout=300):
    p = subprocess.run([str(a) for a in args], input=input, capture_output=True, text=True, timeout=timeout)
    if ok and p.returncode:
        raise AssertionError(f'{args} exited {p.returncode}: {p.stderr}\n{p.stdout}')
    return p

def until(what, predicate, seconds, label):
    end = time.time() + seconds
    while time.time() < end:
        try:
            value = what()
            if predicate(value): return value
        except (OSError, ValueError, KeyError, AssertionError): pass
        time.sleep(1)
    raise AssertionError(f'timed out after {seconds} s: {label}')

def lines(text):
    return [json.loads(l) for l in text.splitlines() if l.strip().startswith('{')]

def preconditions():
    if os.geteuid() != 0: sys.exit('test_register_credential.py runs as root on a disposable systemd host (CI)')
    if not Path('/run/systemd/system').is_dir(): sys.exit('PID 1 is not systemd: LoadCredentialEncrypted= cannot run here')
    for path in (OPT, UNIT, STATE, CRED):
        if path.exists(): sys.exit(f'{path} exists: this test installs pocol-standalone and never touches an existing install')
    if subprocess.run(['getent', 'passwd', USER], capture_output=True).returncode == 0: sys.exit(f'user {USER} exists')

def run():
    preconditions()
    outputs = []  # every stdout/stderr the register runs and the setup produced, searched for secrets at the end
    with tempfile.TemporaryDirectory(prefix='pocol-register-credential-') as tmp:
        root = Path(tmp); os.chmod(root, 0o755); processes = []; logs = []
        try:
            # 1. The release package as install-linux.sh expects it, and the real installer.
            pkg = root / 'package'; pkg.mkdir()
            for name in ('pocol', 'pocold', 'pocol-key', 'pocol-sign', 'pocol-site'): shutil.copy2(BIN / name, pkg / name)
            shutil.copy2(HERE / 'install-linux.sh', pkg / 'install-linux.sh')
            sums = subprocess.run(['sha256sum', 'pocol', 'pocold', 'pocol-key', 'pocol-sign', 'pocol-site'], cwd=pkg, capture_output=True, text=True, check=True)
            (pkg / 'SHA256SUMS').write_text(sums.stdout)
            p = sh('bash', pkg / 'install-linux.sh'); outputs.append(p.stdout + p.stderr)
            a = (OPT / 'identity.address').read_text().strip()
            assert p.stdout.count('pocol register status|quote|confirm') == 1, p.stdout
            # 2. A local devnet whose three founders are plain processes; the service is a fourth, unregistered node.
            homes = [root / f'founder{i}' for i in range(3)]; ids = [identity(h) for h in homes]
            p2p = [port() for _ in range(3)]; api = [port() for _ in range(3)]
            cfg = {'chainId': CHAIN, 'genesisTime': int(time.time()) + 5, 'registrationPerBlock': 1,
                   'miners': [{'id': x, 'reward': x} for x in ids], 'accounts': [{'address': x, 'wei': str(10**24)} for x in ids]}
            (root / 'config.json').write_text(json.dumps(cfg))
            call('pocold', 'init-testnet', '--config', root / 'config.json', '--out', root / 'genesis')
            args = ['network', '--from', root / 'genesis', '--out', root / 'network.pocol.json']
            for q in p2p: args += ['--seed', f'127.0.0.1:{q}']
            call('pocol', *args)
            shutil.copy2(root / 'network.pocol.json', OPT / 'network.pocol.json'); os.chmod(OPT / 'network.pocol.json', 0o644)
            for i in range(3):
                log = open(root / f'founder{i}.log', 'w'); logs.append(log)
                processes.append(subprocess.Popen([str(BIN / 'pocol'), 'start', '--home', str(homes[i]), '--network', str(root / 'network.pocol.json'),
                                                   '--listen', f'127.0.0.1:{p2p[i]}', '--rpc', f'127.0.0.1:{api[i]}'], stdout=log, stderr=log))
            sh('systemctl', 'start', SERVICE)
            until(lambda: (int(rpc(RPC_PORT, 'eth_blockNumber'), 16), int(rpc(RPC_PORT, 'net_peerCount'), 16)), lambda v: v[0] >= 3 and v[1] > 0, 300, 'service synced')
            # 3. The passphrase-encrypted backup (service stopped: export-key takes the home's lock), then the real K1 script.
            sh('systemctl', 'stop', SERVICE)
            backups = root / 'backup'; backups.mkdir(); shutil.chown(backups, USER, USER); os.chmod(backups, 0o700)
            backup = backups / 'identity.pocol-backup'
            p = sh('runuser', '-u', USER, '--', OPT / 'pocol', 'export-key', '--home', STATE, '--out', backup, input=f'{PASSPHRASE}\n{PASSPHRASE}\n')
            outputs.append(p.stdout + p.stderr); assert json.loads(p.stdout)['address'] == a, p.stdout
            sh('systemctl', 'start', SERVICE)
            p = sh('bash', HERE / 'encrypt-server-key.sh', '--backup', backup, input=f'{PASSPHRASE}\n', timeout=600); outputs.append(p.stdout + p.stderr)
            assert CRED.is_file() and not (STATE / 'identity.pocol-key').exists(), 'the key lives only in the credential now'
            assert (DROPIN_DIR / '50-encrypted-key.conf').is_file()
            secret = subprocess.run(['systemd-creds', 'decrypt', '--name=pocol-identity', str(CRED), '-'], capture_output=True, check=True).stdout
            assert len(secret) == 32, 'a key file holds 32 bytes'
            until(lambda: int(rpc(RPC_PORT, 'eth_blockNumber'), 16), lambda h: h >= 3, 300, 'service up with the credential')
            invocation = sh('systemctl', 'show', '-p', 'InvocationID', '--value', SERVICE).stdout.strip()
            # 4. Fund the identity with the fee faucet's 0.002 PCOL (signed by founder 0).
            nonce = int(rpc(api[0], 'eth_getTransactionCount', [ids[0], 'latest']), 16)
            raw = sh(BIN / 'pocol-sign', '--key-file', homes[0] / 'identity.pocol-key', '--chain-id', CHAIN, '--to', a, '--value', 2 * 10**15, '--nonce', nonce).stdout.strip()
            rpc(api[0], 'eth_sendRawTransaction', [raw])
            reg = lambda *extra, input='', ok=True: sh('runuser', '-u', USER, '--', OPT / 'pocol', 'register', *extra, '--home', STATE, input=input, ok=ok)
            until(lambda: lines(reg('status').stdout)[0], lambda v: v['balance'] == '0.002', 300, 'funded')
            names = sorted(x.name for x in STATE.iterdir() if x.name.startswith('identity'))
            # 5. Refusals before anything is signed. status and quote need no key; confirm needs the credential.
            for extra, expect in [((), 'key file is missing'), (('--key-credential', 'pocol-identity'), 'needs $CREDENTIALS_DIRECTORY')]:
                p = reg('confirm', *extra, input='yes\n', ok=False); outputs.append(p.stdout + p.stderr)
                assert p.returncode and expect in p.stderr and not lines(p.stdout), (extra, p.stderr, p.stdout)
            p = sh(OPT / 'pocol', 'register', 'status', '--home', STATE, ok=False)
            assert p.returncode and 'run the command as the user the node runs as' in p.stderr, p.stderr
            documented = ['systemd-run', '--pipe', '--wait', '--quiet', '-p', f'User={USER}', '-p', f'LoadCredentialEncrypted=pocol-identity:{CRED}',
                          OPT / 'pocol', 'register', 'confirm', '--home', STATE, '--key-credential', 'pocol-identity']
            p = sh(*documented, input='no\n', ok=False); outputs.append(p.stdout + p.stderr)
            assert p.returncode and [x['type'] for x in lines(p.stdout)] == ['quote'] and 'nothing was signed or sent' in p.stderr, (p.stdout, p.stderr)
            assert rpc(RPC_PORT, 'eth_getTransactionCount', [a, 'pending']) == '0x0', 'nothing was sent'
            # 6. The documented registration, beside the running service.
            p = sh(*documented, input='yes\n'); outputs.append(p.stdout + p.stderr)
            got = lines(p.stdout)
            assert [x['type'] for x in got] == ['quote', 'sent', 'receipt'], p.stdout
            assert got[0]['method'] == 'register' and got[0]['from'].lower() == a.lower(), got[0]
            assert got[2]['status'] == 'included' and got[2]['registered'] and got[2]['miningEligible'], got[2]
            p = sh(*documented, input='yes\n', ok=False); outputs.append(p.stdout + p.stderr)
            assert 'already has a registry record' in p.stderr, p.stderr
            # 7. The service was never restarted, still has no key file, and mines with the registered identity.
            assert sh('systemctl', 'show', '-p', 'InvocationID', '--value', SERVICE).stdout.strip() == invocation, 'the service was not restarted'
            assert sorted(x.name for x in STATE.iterdir() if x.name.startswith('identity')) == names, 'register wrote nothing in the home'
            assert not (STATE / 'identity.pocol-key').exists()
            mined = until(lambda: lines(reg('status').stdout)[0], lambda v: v['record'] and v['record']['lastShare'] > v['record']['registeredAt'], 900, 'the service mines')
            # 8. No secret anywhere: register and setup output, the service journal and the transient units' journal.
            journal = sh('journalctl', '-q', '-o', 'cat', '--no-pager', '-u', SERVICE).stdout + sh('journalctl', '-q', '-o', 'cat', '--no-pager', '--since', '-1h').stdout
            haystack = '\n'.join(outputs) + journal
            lowered = haystack.lower()
            for needle in (secret.hex(), '0x' + secret.hex()):
                assert needle not in lowered, 'the secret key appears in an output or journal line'
            assert secret not in haystack.encode('utf-8', 'surrogateescape'), 'raw key bytes appear in an output'
            assert PASSPHRASE not in haystack, 'the backup passphrase appears in an output or journal line'
            print(json.dumps({'result': 'PASS', 'scope': 'real install-linux.sh, export-key and encrypt-server-key.sh on a systemd host; pocol register through systemd-run LoadCredentialEncrypted= beside the running service',
                              'withKey': 'host' if 'host+tpm2' not in '\n'.join(outputs) else 'host+tpm2', 'registeredAt': mined['record']['registeredAt'],
                              'lastShare': mined['record']['lastShare'], 'serviceRestarted': False, 'secretFound': False}))
        except BaseException:
            for log in logs: log.flush()
            print(sh('journalctl', '-q', '-o', 'cat', '--no-pager', '-n', '80', '-u', SERVICE, ok=False).stdout[-6000:])
            for path in root.glob('*.log'): print(path.name, path.read_text(errors='replace')[-2000:])
            raise
        finally:
            for p in processes:
                if p.poll() is None: p.terminate()
            for p in processes:
                try: p.wait(timeout=12)
                except subprocess.TimeoutExpired: p.kill(); p.wait()
            for log in logs: log.close()
            # Disposable host, but leave it as found.
            subprocess.run(['systemctl', 'stop', SERVICE], capture_output=True)
            subprocess.run(['systemctl', 'disable', SERVICE], capture_output=True)
            for path in (DROPIN_DIR, UNIT, OPT, STATE, CRED):
                if path.is_dir(): shutil.rmtree(path, ignore_errors=True)
                elif path.exists(): path.unlink()
            subprocess.run(['systemctl', 'daemon-reload'], capture_output=True)
            subprocess.run(['userdel', USER], capture_output=True)
if __name__ == '__main__': run()
