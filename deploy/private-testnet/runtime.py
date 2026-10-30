#!/usr/bin/env python3
"""Verify an operator-distributed bundle, bind its data, then exec pocold."""
import argparse
import fcntl
import hashlib
import json
import os
from pathlib import Path
import sys


def digest(path):
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()


def verify(bundle):
    manifest = json.loads((bundle / 'manifest.json').read_text())
    if manifest['schema'] != 'pocol-private-bundle-v1':
        raise ValueError('Unsupported bundle schema')
    for name, expected in manifest['sha256'].items():
        path = bundle / name
        if path.is_symlink() or not path.resolve().is_relative_to(bundle.resolve()):
            raise ValueError('Unsafe bundle path: ' + name)
        if digest(path) != expected:
            raise ValueError('Bundle checksum mismatch: ' + name)
    required = {'node.json', 'runtime.py', 'bin/pocold', 'net/devnet.json',
                'net/genesis.spec.hex', 'net/alloc.json'}
    if not required.issubset(manifest['sha256']):
        raise ValueError('Manifest does not cover required inputs')
    return json.loads((bundle / 'node.json').read_text())


def launch(bundle, data):
    config = verify(bundle)
    info = json.loads((bundle / 'net/devnet.json').read_text())
    data.mkdir(parents=True, exist_ok=True, mode=0o700)
    # Held across exec; prevents a second process from touching admission scratch stores.
    lock = (data / '.operator.lock').open('a')
    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    os.set_inheritable(lock.fileno(), True)
    marker = data / 'network-binding.json'
    identity = {'chainId': info['chainId'], 'genesisHash': info['genesisHash'],
                'stateStore': True, 'node': config['name']}
    if marker.exists():
        if json.loads(marker.read_text()) != identity:
            raise ValueError('Data belongs to a different network, node or storage mode')
    else:
        if set(p.name for p in data.iterdir()) - {'.operator.lock'}:
            raise ValueError('Refusing existing data without a network binding')
        with marker.open('x') as out:
            json.dump(identity, out)
            out.flush()
            os.fsync(out.fileno())
    args = [str(bundle / 'bin/pocold'), 'run', '--network', str(bundle / 'net'),
            '--data', str(data), '--name', config['name'], '--miner', config['name'],
            '--listen', config['listen'], '--rpc', config['rpc'], '--state-store',
            '--checkpoint-every', '128', '--log'] + config['localArgs']
    for peer in config['peers']:
        args += ['--peer', peer]
    os.execv(args[0], args)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('command', choices=['verify', 'run'])
    parser.add_argument('--bundle', type=Path, default=Path(__file__).resolve().parent)
    parser.add_argument('--data', type=Path, default=Path('/var/lib/pocol-private'))
    args = parser.parse_args()
    if args.command == 'verify':
        config = verify(args.bundle.resolve())
        print(json.dumps({'verified': True, 'node': config['name']}))
    else:
        launch(args.bundle.resolve(), args.data.resolve())


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, KeyError) as error:
        print('private-testnet: ' + str(error), file=sys.stderr)
        sys.exit(1)
