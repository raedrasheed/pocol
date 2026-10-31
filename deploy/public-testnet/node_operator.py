#!/usr/bin/env python3
"""Verify and run an independently keyed testnet node; offline backup/restore.

No key generation, private key backup, remote connections or firewall changes here.
"""
import argparse
import contextlib
import fcntl
import hashlib
import json
import os
from pathlib import Path
import pwd
import shutil
import stat
import subprocess
import sys

SCHEMA = 'pocol-public-testnet-bundle-v1'
LOCAL_ARGS = ['--delta', '1', '--phi', '1', '--t-exec', '1', '--delta-rel', '8', '--gamma', '5']
BINARIES = ('pocold', 'pocol-key', 'pocol-sign', 'pocol-site')
LOCKS = {'.operator.lock', 'LOCK'}


def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''): h.update(block)
    return h.hexdigest()


def read_json(path):
    def unique(pairs):
        out = {}
        for key, value in pairs:
            if key in out: raise ValueError('Duplicate JSON key: ' + key)
            out[key] = value
        return out
    with path.open('rb') as stream:
        raw = stream.read(4 * 1024 * 1024 + 1)
    if len(raw) > 4 * 1024 * 1024: raise ValueError('JSON file too large')
    return json.loads(raw, object_pairs_hook=unique)


def write_json(path, value):
    with path.open('x') as stream:
        json.dump(value, stream, indent=2); stream.write('\n')
        stream.flush(); os.fsync(stream.fileno())
    sync_dir(path.parent)


def sync_dir(path):
    fd = os.open(path, os.O_RDONLY | os.O_DIRECTORY)
    try: os.fsync(fd)
    finally: os.close(fd)


def files(root):
    if root.is_symlink() or not root.is_dir(): raise ValueError('Expected a regular directory')
    result = {}
    for path in sorted(root.rglob('*')):
        mode = path.lstat().st_mode
        if stat.S_ISDIR(mode): continue
        if not stat.S_ISREG(mode) or path.stat().st_nlink != 1:
            raise ValueError('Links and special files are not allowed: ' + path.name)
        result[path.relative_to(root).as_posix()] = path
    return result


def verify_files(root, checksums, excluded):
    actual = files(root)
    if set(actual) - set(excluded) != set(checksums): raise ValueError('Unexpected or missing files')
    for name, expected in checksums.items():
        # Exact inventory matching above also rejects absolute and parent-traversal paths.
        if name in excluded or digest(actual[name]) != expected:
            raise ValueError('Checksum mismatch: ' + name)


def verify(bundle, expected=None):
    if expected is not None and digest(bundle / 'manifest.json') != expected:
        raise ValueError('Untrusted manifest identity')
    manifest = read_json(bundle / 'manifest.json')
    if manifest['schema'] != SCHEMA: raise ValueError('Unsupported bundle schema')
    required = {'node.json', 'node_operator.py', 'build-info.json', 'pocol-testnet.service',
                'net/genesis.spec.hex', 'net/devnet.json', 'net/alloc.json'} | {'bin/' + n for n in BINARIES}
    if not required.issubset(manifest['sha256']): raise ValueError('Incomplete bundle manifest')
    verify_files(bundle, manifest['sha256'], {'manifest.json'})
    config = read_json(bundle / 'node.json')
    info = read_json(bundle / 'net/devnet.json')
    build = read_json(bundle / 'build-info.json')
    if build['features'] or build['protocolVersion'] != 2: raise ValueError('Unsupported or test-feature binary')
    if info['admission']['scope'] != 'experimental-devnet' or info['chainId'] == 777001:
        raise ValueError('Only the experimental testnet profile is supported')
    if manifest['identity'] != identity(config, info, build): raise ValueError('Bundle identity mismatch')
    return manifest, config


def identity(config, info, build):
    return {'chainId': info['chainId'], 'genesisHash': info['genesisHash'],
            'protocolVersion': build['protocolVersion'], 'compatibilitySha256': build['compatibilitySha256'],
            'stateStore': True, 'node': config['name'], 'miner': config['minerAddress']}


@contextlib.contextmanager
def locked(data, journal=True):
    """Same flock as the Rust journal on Linux; also fences calibration before journal open."""
    handles = []
    try:
        for name in (['.operator.lock', 'LOCK'] if journal else ['.operator.lock']):
            fd = os.open(data / name, os.O_RDWR | os.O_CREAT | os.O_NOFOLLOW, 0o600)
            handles.append(fd)
            if not stat.S_ISREG(os.fstat(fd).st_mode): raise ValueError('Invalid lock file')
            fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
        yield handles
    finally:
        for fd in handles: os.close(fd)


def launch(bundle, data, key):
    manifest, config = verify(bundle)
    expected = manifest['identity']
    if config['minerAddress']:
        if key is None: raise ValueError('This miner needs a locally held --key-file')
        if key.is_symlink(): raise ValueError('Key must not be a symlink')
        key = key.resolve(strict=True)
        if key.is_relative_to(bundle) or key.is_relative_to(data):
            raise ValueError('Keep keys outside bundles and state backups')
        address = subprocess.check_output([str(bundle / 'bin/pocol-key'), 'address', '--key-file', str(key)],
                                          text=True, timeout=10).strip()
        if address.lower() != config['minerAddress']: raise ValueError('Key does not match the node public address')
    elif key is not None: raise ValueError('Observer config cannot accept a miner key')
    actual = json.loads(subprocess.check_output([str(bundle / 'bin/pocold'), 'build-info'], text=True, timeout=10))
    if actual != read_json(bundle / 'build-info.json'): raise ValueError('Binary build identity mismatch')
    data.mkdir(parents=True, exist_ok=True, mode=0o700)
    if data.is_symlink(): raise ValueError('State directory must not be a symlink')
    with locked(data, journal=False) as handles:
        marker = data / 'network-binding.json'
        if marker.exists():
            if read_json(marker) != expected: raise ValueError('State belongs to another release, network, node or key')
        else:
            if set(p.name for p in data.iterdir()) - {'.operator.lock'}:
                raise ValueError('Refusing unbound existing data')
            write_json(marker, expected)
        args = [str(bundle / 'bin/pocold'), 'run', '--network', str(bundle / 'net'), '--data', str(data),
                '--name', config['name'], '--listen', config['listen'], '--rpc', config['rpc'],
                '--state-store', '--checkpoint-every', '128', '--log'] + LOCAL_ARGS
        if key is not None: args += ['--miner-key-file', str(key)]
        for peer in config['peers']: args += ['--peer', peer]
        os.set_inheritable(handles[0], True)
        os.execv(args[0], args)


def copy_files(source, target, names, owner=None):
    target.mkdir(mode=0o700)
    for name in sorted(names):
        dest = target / name; dest.parent.mkdir(parents=True, exist_ok=True, mode=0o700)
        with (source / name).open('rb') as inp, dest.open('xb') as out:
            shutil.copyfileobj(inp, out, 1024 * 1024)
            os.fchmod(out.fileno(), 0o600)
            if owner is not None: os.fchown(out.fileno(), *owner)
            out.flush(); os.fsync(out.fileno())
    # Transfer directories only after their contents; the root stays private until complete.
    for path in sorted((p for p in target.rglob('*') if p.is_dir()), reverse=True):
        if owner is not None: os.chown(path, *owner)
        sync_dir(path)
    if owner is not None: os.chown(target, *owner)
    sync_dir(target); sync_dir(target.parent)


def backup(data, out):
    if not data.is_dir() or out.exists() or out.is_relative_to(data): raise ValueError('Use existing state and a new, separate backup path')
    with locked(data):
        binding = read_json(data / 'network-binding.json')
        entries = files(data)
        names = set(entries) - LOCKS
        if not {'blocks.journal', 'blocks.commit', 'network-binding.json'}.issubset(names): raise ValueError('Incomplete node state')
        if any(n.endswith('.pocol-key') for n in names): raise ValueError('Key found in state; backup refused')
        checksums = {name: digest(entries[name]) for name in sorted(names)}
        out.mkdir(mode=0o700)
        copy_files(data, out / 'state', names)
        verify_files(out / 'state', checksums, set())
        write_json(out / 'backup.json', {'schema': 'pocol-testnet-backup-v1', 'identity': binding, 'sha256': checksums})
        return {'backup': str(out), 'backupManifestSha256': digest(out / 'backup.json'), 'keysIncluded': False}


def restore(bundle, source, data, expected, owner=None):
    ownership = None
    if owner is not None:
        if os.geteuid() != 0: raise ValueError('--owner requires root')
        account = pwd.getpwnam(owner); ownership = (account.pw_uid, account.pw_gid)
    manifest, _ = verify(bundle)
    if data.exists(): raise ValueError('Restore target must not exist; existing state is never overwritten')
    if digest(source / 'backup.json') != expected: raise ValueError('Backup manifest digest mismatch')
    saved = read_json(source / 'backup.json')
    if saved['schema'] != 'pocol-testnet-backup-v1' or saved['identity'] != manifest['identity']:
        raise ValueError('Backup belongs to another network, node, release or key')
    verify_files(source / 'state', saved['sha256'], set())
    if read_json(source / 'state/network-binding.json') != saved['identity']: raise ValueError('Backup binding mismatch')
    copy_files(source / 'state', data, saved['sha256'], ownership)
    verify_files(data, saved['sha256'], set())
    return {'restored': str(data), 'ownerUid': data.stat().st_uid, 'ownerGid': data.stat().st_gid, 'started': False}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['verify', 'run', 'backup', 'restore'])
    parser.add_argument('--bundle', type=Path, default=Path(__file__).absolute().parent)
    parser.add_argument('--data', type=Path, default=Path('/var/lib/pocol-testnet'))
    parser.add_argument('--key-file', type=Path)
    parser.add_argument('--out', type=Path)
    parser.add_argument('--backup', type=Path)
    parser.add_argument('--expected-sha256')
    parser.add_argument('--owner', help='Service account receiving restored state (root only)')
    args = parser.parse_args()
    bundle, data = args.bundle.absolute(), args.data.absolute()
    if args.command == 'verify':
        manifest, _ = verify(bundle, args.expected_sha256); result = {'verified': True, 'identity': manifest['identity']}
    elif args.command == 'run': launch(bundle, data, args.key_file); return
    elif args.command == 'backup':
        if args.out is None: parser.error('backup requires --out')
        result = backup(data, args.out.absolute())
    else:
        if args.backup is None or not args.expected_sha256: parser.error('restore requires --backup and --expected-sha256')
        if os.geteuid() == 0 and args.owner is None: parser.error('root restore requires --owner SERVICE_ACCOUNT')
        result = restore(bundle, args.backup.absolute(), data, args.expected_sha256, args.owner)
    print(json.dumps(result))


if __name__ == '__main__':
    try: main()
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        print('testnet operator: ' + str(error), file=sys.stderr); sys.exit(1)
