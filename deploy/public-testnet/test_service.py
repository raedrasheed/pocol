#!/usr/bin/env python3
"""Root-only CI drill on a fresh disposable systemd host, using loopback fixtures.

Installs with private preparation modes, runs as the separate service account,
then backs up/restores as root and runs from the restored StateDirectory. Refuses
pre-existing installation/state/credentials; never prints or uploads key contents.
"""
import argparse
import json
import os
from pathlib import Path
import pwd
import shutil
import subprocess
import tempfile
import time
import node_operator as op
from monitor import rpc

TARGETS = [Path('/opt/pocol-testnet'), Path('/etc/pocol-testnet'), Path('/var/lib/pocol-testnet'),
           Path('/var/lib/pocol-testnet-restored'), Path('/etc/systemd/system/pocol-testnet.service'),
           Path('/run/pocol-testnet')]


def command(args):
    return subprocess.run(args, check=True, capture_output=True, text=True).stdout


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bundle', type=Path, required=True); parser.add_argument('--key-file', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True); a = parser.parse_args()
    if os.geteuid() != 0: raise RuntimeError('This CI service drill requires root')
    if any(p.exists() or p.is_symlink() for p in TARGETS): raise RuntimeError('Disposable fresh host required')
    manifest, config = op.verify(a.bundle)
    if not manifest['rehearsalOnly'] or not config['listen'].startswith('127.') or not config['minerAddress']:
        raise RuntimeError('Only a loopback miner fixture may run in this drill')
    command(['systemctl','show','--property=Version'])
    with tempfile.TemporaryDirectory(prefix='pocol-service-') as temp:
        work = Path(temp); bundle = work / 'bundle'; shutil.copytree(a.bundle, bundle)
        # Isolated installer fixture only: retain loopback bind/peers but exercise its server path.
        manifest['rehearsalOnly'] = False
        (bundle / 'manifest.json').unlink(); op.write_json(bundle / 'manifest.json', manifest)
        for path in [bundle, *bundle.rglob('*')]: path.chmod(0o700 if path.is_dir() else 0o600)
        for name in op.BINARIES: (bundle / 'bin' / name).chmod(0o700)
        try:
            command(['bash',str(Path(__file__).parent / 'install.sh'),str(bundle),op.digest(bundle / 'manifest.json')])
            secret = Path('/etc/pocol-testnet/miner.pocol-key')
            with a.key_file.open('rb') as src, secret.open('xb') as dst: shutil.copyfileobj(src,dst)
            secret.chmod(0o600)
            user = pwd.getpwnam('pocol-testnet')
            # Verify the public bundle as the actual service UID before starting any node.
            command(['runuser','-u','pocol-testnet','--','python3','-B','/opt/pocol-testnet/node_operator.py','verify'])
            def wait_node():
                until = time.monotonic()+45
                while time.monotonic() < until:
                    try:
                        status = rpc('http://'+config['rpc'], 'pocol_status')
                        if status['miner'] == config['minerAddress'] and status['compatibilitySha256'] == manifest['identity']['compatibilitySha256']:
                            return status
                    except (OSError, ValueError): pass
                    time.sleep(.2)
                journal = command(['journalctl','-u','pocol-testnet','--no-pager','-n','40'])
                raise RuntimeError('Service did not start: '+journal)
            command(['systemctl','start','pocol-testnet']); before = wait_node()
            runtime = Path('/run/pocol-testnet')
            metadata = (runtime / 'miner.pocol-key').stat()
            assert metadata.st_uid == user.pw_uid and metadata.st_gid == user.pw_gid
            assert metadata.st_mode & 0o777 == 0o600 and metadata.st_nlink == 1
            assert runtime.stat().st_mode & 0o777 == 0o700
            frozen = rpc('http://'+config['rpc'],'eth_getBlockByNumber',[hex(before['head']['height']),False])
            command(['systemctl','stop','pocol-testnet'])
            assert not runtime.exists(), 'Runtime key must be removed on stop'
            saved = op.backup(Path('/var/lib/pocol-testnet'), work / 'backup')
            restored = Path('/var/lib/pocol-testnet-restored')
            op.restore(Path('/opt/pocol-testnet'), work / 'backup', restored, saved['backupManifestSha256'], 'pocol-testnet')
            for path in [restored,*restored.rglob('*')]:
                assert (path.stat().st_uid,path.stat().st_gid) == (user.pw_uid,user.pw_gid), path
            unit = Path('/etc/systemd/system/pocol-testnet.service')
            unit.write_text(unit.read_text().replace('StateDirectory=pocol-testnet\n','StateDirectory=pocol-testnet-restored\n')
                .replace('node_operator.py run','node_operator.py run --data /var/lib/pocol-testnet-restored'))
            command(['systemctl','daemon-reload']); command(['systemctl','start','pocol-testnet']); after = wait_node()
            block = rpc('http://'+config['rpc'],'eth_getBlockByNumber',[frozen['number'],False])
            assert (block['hash'],block['stateRoot'],block['receiptsRoot']) == (frozen['hash'],frozen['stateRoot'],frozen['receiptsRoot'])
            result = {'result':'PASS','scope':'disposable systemd host, loopback only', 'serviceUser':'pocol-testnet',
                      'checks':['restrictive umask bundle readable by service','credential copied to service-owned private runtime key',
                                'runtime key removed on stop',
                                'root restore assigns service UID/GID','restored StateDirectory starts and preserves roots'],
                      'restoredHeight':after['head']['height'],'publicLaunchReady':False}
            op.write_json(a.out,result); print(json.dumps(result))
        finally:
            subprocess.run(['systemctl','stop','pocol-testnet'],capture_output=True)
            for path in reversed(TARGETS):
                if path.is_dir(): shutil.rmtree(path)
                elif path.exists(): path.unlink()
            subprocess.run(['systemctl','daemon-reload'],capture_output=True)


if __name__ == '__main__': main()
