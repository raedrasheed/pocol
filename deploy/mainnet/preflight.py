#!/usr/bin/env python3
"""Read-only, local seed inventory for a same-chain PoCol Mainnet rollout.

Reads public bundle metadata, the public state binding, and loopback node RPC.
Never reads keys, service environments, process command lines, or wallet balances.
Does not choose an activation height, stop/start a service, or change node files.
"""
import argparse
from datetime import datetime, timezone
import hashlib
import http.client
import json
import os
from pathlib import Path
import re
import stat
import subprocess
import sys
from urllib.parse import urlsplit

CHAIN_ID = 1599846715
GENESIS = '0xbe137bfdb4fdf332f2e1500ee1a67e0b22ce6bdf57efa46fb4ea7b0debe509ca'
READ_METHODS = {'pocol_getParams', 'pocol_status', 'eth_getBlockByNumber'}
PUBLIC_FILES = {'manifest.json', 'node.json', 'build-info.json', 'node_operator.py',
                'install.sh', 'pocol-testnet.service', 'nginx.conf', 'README.md', 'nonce_upgrade.py',
                'net/genesis.spec.hex', 'net/devnet.json', 'net/alloc.json',
                'bin/pocold', 'bin/pocol-key', 'bin/pocol-sign', 'bin/pocol-site'}
REQUIRED_FILES = {'manifest.json', 'node.json', 'build-info.json', 'node_operator.py',
                  'pocol-testnet.service', 'net/genesis.spec.hex', 'net/devnet.json', 'net/alloc.json',
                  'bin/pocold', 'bin/pocol-key', 'bin/pocol-sign', 'bin/pocol-site'}


class PreflightError(ValueError):
    """Messages are fixed diagnostics, never untrusted RPC/service output."""


def require(condition, message):
    if not condition: raise PreflightError(message)


def unique(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, 'duplicate JSON keys')
        result[key] = value
    return result


def read_json(path):
    metadata = path.lstat()
    require(stat.S_ISREG(metadata.st_mode) and metadata.st_nlink == 1, 'metadata must be a regular single-link file')
    with path.open('rb') as stream: raw = stream.read(512 * 1024 + 1)
    require(len(raw) <= 512 * 1024, 'metadata exceeds size limit')
    try: value = json.loads(raw, object_pairs_hook=unique)
    except (UnicodeError, json.JSONDecodeError): raise PreflightError('invalid JSON metadata') from None
    require(isinstance(value, dict), 'metadata must be a JSON object')
    return value


def digest(path):
    require(path.stat().st_size <= 256 * 1024 * 1024, 'public bundle file exceeds size limit')
    result = hashlib.sha256()
    with path.open('rb') as stream:
        for content in iter(lambda: stream.read(1024 * 1024), b''): result.update(content)
    return result.hexdigest()


def hex_hash(value):
    require(isinstance(value, str) and re.fullmatch(r'0x[0-9a-fA-F]{64}', value), 'invalid chain hash')
    return value.lower()


def source_hash(value):
    require(isinstance(value, str) and re.fullmatch(r'[0-9a-f]{64}', value), 'invalid source compatibility hash')
    return value


def integer(value):
    require(type(value) is int and 0 <= value < 2**64, 'invalid unsigned integer')
    return value


def forks(value):
    require(isinstance(value, list) and len(value) in (1, 2), 'invalid fork schedule')
    for entry in value:
        require(isinstance(entry, list) and len(entry) == 2, 'invalid fork entry')
        integer(entry[0]); integer(entry[1])
    require(value[0] == [1, 0] and (len(value) == 1 or value[1][0] == 2 and value[1][1] > 0),
            'unsupported fork schedule')
    return value


def identity(value):
    require(isinstance(value, dict), 'invalid identity metadata')
    return {'chainId': integer(value.get('chainId')), 'genesisHash': hex_hash(value.get('genesisHash')),
            'compatibilitySha256': source_hash(value.get('compatibilitySha256'))}


def binding_identity(value):
    """Validate the exact public identity used by node_operator.launch."""
    public = identity(value)
    require(set(value) == set(public) | {'protocolVersion', 'stateStore', 'node', 'miner'},
            'unexpected state binding fields')
    require(type(value.get('protocolVersion')) is int and value['protocolVersion'] == 2,
            'unsupported binding protocol version')
    require(value.get('stateStore') is True, 'state binding has an unexpected storage mode')
    require(isinstance(value.get('node'), str) and re.fullmatch(r'[A-Za-z0-9_.-]{1,64}', value['node']),
            'invalid public node name')
    require(value['miner'] is None or isinstance(value['miner'], str)
            and re.fullmatch(r'0x[0-9a-fA-F]{40}', value['miner']),
            'invalid public miner address')
    return dict(value)


class LocalRpc:
    def __init__(self, url):
        parsed = urlsplit(url)
        require(parsed.scheme == 'http' and parsed.hostname in ('127.0.0.1', '::1')
                and parsed.port and not parsed.username and not parsed.password
                and not parsed.query and not parsed.fragment and parsed.path in ('', '/'),
                'use only http://127.0.0.1:PORT or http://[::1]:PORT')
        self.host, self.port = parsed.hostname, parsed.port

    def __call__(self, method, params=None):
        require(method in READ_METHODS, 'RPC method is not an approved preflight read')
        connection = http.client.HTTPConnection(self.host, self.port, timeout=8)
        try:
            payload = json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': method, 'params': params or []})
            connection.request('POST', '/', payload, {'Content-Type': 'application/json', 'Connection': 'close'})
            response = connection.getresponse()
            require(response.status == 200, 'local node RPC unavailable')
            raw = response.read(512 * 1024 + 1)
            require(len(raw) <= 512 * 1024, 'RPC response exceeds size limit')
            try: value = json.loads(raw, object_pairs_hook=unique)
            except (UnicodeError, json.JSONDecodeError): raise PreflightError('invalid RPC JSON') from None
            require(isinstance(value, dict) and value.get('jsonrpc') == '2.0'
                    and type(value.get('id')) is int and value['id'] == 1
                    and 'result' in value and 'error' not in value, 'node refused the preflight RPC read')
            return value['result']
        finally:
            connection.close()


def bundle_report(bundle):
    require(bundle.is_dir() and not bundle.is_symlink(), 'public bundle directory is unavailable')
    actual = {}
    for path in bundle.rglob('*'):
        meta = path.lstat()
        if stat.S_ISDIR(meta.st_mode): continue
        name = path.relative_to(bundle).as_posix()
        # Refuse unexpected files before hashing or reading their contents.
        require(name in PUBLIC_FILES and stat.S_ISREG(meta.st_mode) and meta.st_nlink == 1,
                'bundle contains an unexpected file or link')
        actual[name] = path
    require(REQUIRED_FILES <= actual.keys(), 'public bundle is incomplete')
    manifest = read_json(bundle / 'manifest.json')
    require(manifest.get('schema') == 'pocol-public-testnet-bundle-v1', 'unsupported bundle manifest')
    hashes = manifest.get('sha256')
    require(isinstance(hashes, dict) and set(hashes) == set(actual) - {'manifest.json'}, 'bundle inventory mismatch')
    require(all(source_hash(expected) == digest(actual[name]) for name, expected in hashes.items()), 'bundle checksum mismatch')
    build, node, network = (read_json(bundle / name) for name in ('build-info.json', 'node.json', 'net/devnet.json'))
    require(build.get('features') == [] and build.get('protocolVersion') == 2, 'unsupported or test-feature native build')
    public = identity({**network, 'compatibilitySha256': build.get('compatibilitySha256')})
    binding = binding_identity(manifest.get('identity', {}))
    require(identity(binding) == public, 'bundle identity mismatch')
    require(binding['node'] == node.get('name') and binding['miner'] == node.get('minerAddress')
            and binding['protocolVersion'] == build.get('protocolVersion'), 'bundle node binding mismatch')
    schedule = forks(network.get('forks', [[1, 0]]))
    name = node.get('name')
    require(isinstance(name, str) and re.fullmatch(r'[A-Za-z0-9_.-]{1,64}', name), 'invalid public node name')
    return {**public, 'manifestSha256': digest(bundle / 'manifest.json'), 'nodeName': name, 'binding': binding,
            'forkSchedule': [[v, str(h)] for v, h in schedule], 'filesVerified': len(hashes)}


def service_report(name):
    require(re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.@-]{0,120}\.service', name), 'invalid service unit name')
    fields = ('Id', 'LoadState', 'ActiveState', 'SubState', 'FragmentPath', 'WorkingDirectory', 'User')
    try:
        result = subprocess.run(['systemctl', 'show', name, '--no-pager', '--property=' + ','.join(fields)],
                                capture_output=True, text=True, timeout=5, check=False)
        if result.returncode or len(result.stdout) > 32768:
            return {'unit': name, 'available': False}
    except (OSError, subprocess.SubprocessError):
        return {'unit': name, 'available': False}
    values = dict(line.split('=', 1) for line in result.stdout.splitlines() if '=' in line)
    # Never request Environment, ExecStart, credentials, unit contents or process args.
    report = {'unit': name, 'available': True}
    for key in ('LoadState', 'ActiveState', 'SubState'):
        value = values.get(key, '')
        report[key[0].lower() + key[1:]] = value if re.fullmatch(r'[a-z-]{1,32}', value) else 'unknown'
    for key in ('FragmentPath', 'WorkingDirectory'):
        value = values.get(key, '')
        if value.startswith('/') and len(value) <= 4096 and all(ord(c) >= 32 for c in value):
            report[key[0].lower() + key[1:]] = value
    user = values.get('User', '')
    if re.fullmatch(r'[A-Za-z0-9_-]{1,64}', user): report['user'] = user
    return report


def collect(rpc, bundle, data, main_service='pocol-testnet.service', services=None, check_height=None):
    report = {'schema': 'pocol-mainnet-preflight-v1',
              'capturedAt': datetime.now(timezone.utc).isoformat(timespec='seconds'),
              'requestedNetworkName': 'PoCol Mainnet',
              'expectedIdentity': {'chainId': CHAIN_ID, 'genesisHash': GENESIS},
              'paths': {'bundle': str(bundle.absolute()), 'state': str(data.absolute())},
              'readOnly': True, 'deploymentPerformed': False, 'activationHeightSelected': False,
              'publicLaunchReady': False, 'checks': {}, 'issues': []}
    def fail(stage, message):
        report['checks'][stage] = False; report['issues'].append({'stage': stage, 'reason': message})
    try:
        params, status = rpc('pocol_getParams', []), rpc('pocol_status', [])
        require(isinstance(params, dict) and isinstance(status, dict), 'invalid node metadata')
        public = identity(params)
        require(public == identity(status), 'node status and parameter identities disagree')
        runtime_binding = binding_identity({**public, 'protocolVersion': status.get('protocolVersion'),
                                            'stateStore': status.get('stateStore'),
                                            'node': status.get('name'), 'miner': status.get('miner')})
        require(type(params.get('protocolVersion')) is int
                and params['protocolVersion'] == runtime_binding['protocolVersion'], 'node protocol versions disagree')
        require((public['chainId'], public['genesisHash']) == (CHAIN_ID, GENESIS), 'node belongs to another chain')
        head = status.get('head', {})
        require(isinstance(head, dict), 'invalid node head metadata')
        height, block_hash = integer(head.get('height')), hex_hash(head.get('hash'))
        block = rpc('eth_getBlockByNumber', [hex(height), False])
        require(isinstance(block, dict) and block.get('number') == hex(height)
                and hex_hash(block.get('hash')) == block_hash, 'captured canonical head changed; collect again')
        comparison_height = height if check_height is None else integer(check_height)
        require(comparison_height <= height, 'comparison height is above the observed head')
        comparison = block if comparison_height == height else rpc('eth_getBlockByNumber', [hex(comparison_height), False])
        require(isinstance(comparison, dict) and comparison.get('number') == hex(comparison_height),
                'comparison block is unavailable')
        comparison_report = {'height': str(comparison_height), 'hash': hex_hash(comparison.get('hash')),
                             'stateRoot': hex_hash(comparison.get('stateRoot')),
                             'receiptsRoot': hex_hash(comparison.get('receiptsRoot'))}
        confirmed = rpc('eth_getBlockByNumber', [hex(height), False])
        require(isinstance(confirmed, dict) and confirmed.get('number') == hex(height)
                and hex_hash(confirmed.get('hash')) == block_hash, 'captured canonical head changed; collect again')
        schedule = forks(params.get('forkSchedule', [[1, 0]]))
        active = integer(params.get('activeVersion'))
        require(active in (1, 2), 'unsupported active consensus version')
        require(active == next(version for version, start in reversed(schedule) if height >= start),
                'active consensus version disagrees with captured head; collect again')
        stats = status.get('stats', {})
        require(isinstance(stats, dict), 'invalid node health metadata')
        history_damaged = stats.get('historyDamaged')
        require(type(history_damaged) is bool and not history_damaged, 'node history is damaged or its health is unavailable')
        peers = integer(status.get('compatiblePeers'))
        require(status.get('stateStore') is True, 'node is not using the expected state store')
        report['comparisonBlock'] = comparison_report
        report['node'] = {**public, 'binding': runtime_binding, 'head': {'height': str(height), 'hash': block_hash},
                          'forkSchedule': [[v, str(h)] for v, h in schedule], 'reportedActiveVersion': active,
                          'compatiblePeers': peers, 'stateStore': status.get('stateStore') is True,
                          'historyDamaged': False}
        for key, allowed in (('networkName', {'PoCol Mainnet'}), ('networkEnvironment', {'mainnet'})):
            report['node'][key] = params.get(key) if params.get(key) in allowed else None
        for key in ('nonceMode', 'effectiveNonceMode', 'nextBlockNonceMode'):
            value = params.get(key)
            report['node'][key] = value if type(value) is int and value in (0, 1) else None
        report['checks']['node'] = True
        if peers == 0: fail('peers', 'no compatible peers observed')
        else: report['checks']['peers'] = True
    except PreflightError as error: fail('node', str(error))
    except (OSError, ValueError, TypeError, http.client.HTTPException): fail('node', 'local RPC could not be read safely')
    try:
        report['bundle'] = bundle_report(bundle)
        report['checks']['bundle'] = True
        if 'node' in report:
            require(identity(report['bundle']) == identity(report['node']), 'running source differs from the installed public bundle')
            require(report['bundle']['binding'] == report['node']['binding'], 'running node binding differs from the installed bundle')
            require(report['bundle']['forkSchedule'] == report['node']['forkSchedule'], 'installed and running fork schedules differ')
    except PreflightError as error: fail('bundle', str(error))
    except (OSError, ValueError, KeyError, TypeError): fail('bundle', 'public bundle could not be verified')
    try:
        require(data.is_dir() and not data.is_symlink(), 'state directory is unavailable')
        saved = read_json(data / 'network-binding.json')
        binding = binding_identity(saved)
        report['stateBinding'] = binding
        require('bundle' in report and binding == report['bundle']['binding'], 'state release binding differs from the bundle')
        space = os.statvfs(data)
        report['storage'] = {'availableBytes': str(space.f_bavail * space.f_frsize)}
        report['checks']['stateBinding'] = True
    except PreflightError as error: fail('stateBinding', str(error))
    except (OSError, ValueError, KeyError, TypeError): fail('stateBinding', 'public state binding could not be read')
    units = [main_service, 'pocol-explorer-gateway.service', 'pocol-studio-gateway.service', 'nginx.service', 'caddy.service']
    report['services'] = services if services is not None else [service_report(unit) for unit in units]
    main = next((service for service in report['services'] if service.get('unit') == main_service), {})
    if main.get('activeState') != 'active': fail('service', 'the selected node service is not observed active')
    else: report['checks']['service'] = True
    report['result'] = 'PASS' if all(report['checks'].values()) and not report['issues'] else 'FAIL'
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--rpc', default='http://127.0.0.1:8545')
    parser.add_argument('--bundle', type=Path, default=Path('/opt/pocol-testnet'))
    parser.add_argument('--data', type=Path, default=Path('/var/lib/pocol-testnet'))
    parser.add_argument('--service', default='pocol-testnet.service')
    parser.add_argument('--check-height', type=int, help='Read the same canonical block height on every seed for comparison; this is not an activation height')
    parser.add_argument('--out', type=Path, help='Create a new sanitized report file; existing files are never replaced')
    args = parser.parse_args()
    try:
        report = collect(LocalRpc(args.rpc), args.bundle, args.data, args.service, check_height=args.check_height)
        text = json.dumps(report, indent=2) + '\n'
        if args.out:
            with args.out.open('x', encoding='utf-8') as stream: stream.write(text)
        print(text, end='')
        return 0 if report['result'] == 'PASS' else 2
    except (PreflightError, OSError, ValueError) as error:
        parser.error(str(error) if isinstance(error, PreflightError) else 'preflight input or report output is unavailable')


if __name__ == '__main__':
    sys.exit(main())
