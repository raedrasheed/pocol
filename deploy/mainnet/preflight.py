#!/usr/bin/env python3
"""Read-only, local seed inventory before the PoCol Earn-in transition.

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
PUBLIC_FILES = {'manifest.json', 'node.json', 'build-info.json', 'node_operator.py', 'pocol_identity.py',
                'install.sh', 'pocol-testnet.service', 'nginx.conf', 'README.md', 'nonce_upgrade.py',
                'net/genesis.spec.hex', 'net/devnet.json', 'net/alloc.json',
                'bin/pocold', 'bin/pocol-key', 'bin/pocol-sign', 'bin/pocol-site'}
REQUIRED_FILES = {'manifest.json', 'node.json', 'build-info.json', 'node_operator.py',
                  'pocol-testnet.service', 'net/genesis.spec.hex', 'net/devnet.json', 'net/alloc.json',
                  'bin/pocold', 'bin/pocol-key', 'bin/pocol-sign', 'bin/pocol-site'}
# The published handoff-2026-11-01 Linux package, not hashes learned from a host.
# Its fresh installer copies only these binaries and identity.address into /opt.
LEGACY_SOURCE = '6cf38a14aa01ca3d824b0309564942cc926ca7232e2a5b2672a17e8c99e35476'
LEGACY_BINARIES = {
    'pocol': '26ad2761d19137542cc642369416289b8d2ddd56ac52c225ccd0cfd1c6cd0364',
    'pocold': '82d95f4511345848b4a318368ec5c266e2ccf91d0a44fd9bd0aff7359e0b1cb6',
    'pocol-key': '0f80b3e989a5919196ef6420d5221af21697d19c05ad70f6506d52a5e3434f09',
    'pocol-sign': 'ea1c4e66c03398a625f128dd9fe2d33fb73dca6d8453c50a6e44a2b888e3a4ad',
    'pocol-site': 'c13b285d431b1e4bb88c8f957a86869ef8b0bee14e1f8dc2ec5f45e54acffca8',
}
LEGACY_SPEC_SHA256 = '2aa7c71b0aab924002281bfb05f2cda6c06998999ebe4511774a219932668725'
# The first release that peers on the consensus identity (R1, plan §6). Empty, so its standalone profile fails closed
# until the release commit pins the published package's binaries and build digest here and in collect_seeds.
R1_BINARIES = {}
R1_SOURCE = None
# What pocold accepts: fork versions 1..MAX_FORK_VERSION and wire versions from PROTOCOL_VERSION_MIN (VA-SPEC §2).
MAX_FORK_VERSION = 2
PROTOCOL_VERSION_MIN = 2
# The GenesisSpec version a release (feature-free) pocold decodes (genesis_spec.rs SPEC_VERSION). A spec-2 (v3)
# genesis opens only in a v3-preview build, which a seed never runs, so this release-mode inventory refuses it until
# activation (V3-PR-I).
SPEC_VERSION = 1
# A spec-2 genesis is identified as a v3 genesis only with six top items, CP schema v2's 67 items, 32-byte roots and the
# v3 program set's sysCodeHash (deploy/public-testnet/pocol_identity.V3_SYS_CODE_HASH; test_preflight compares them).
V3_CP_ITEMS = 67
V3_SYS_CODE_HASH = '72b5af64a2023c2c1662e011a4dbac454bc06b02b4fa321c8ba1c305461ca849'
# The state binding fields node_operator.launch compares (plan §4); the build digest is informational.
BOUND = ('chainId', 'genesisHash', 'consensusIdentity', 'stateStore', 'node', 'miner')
LAYOUTS = {
    'manifest': (Path('/opt/pocol-testnet'), Path('/var/lib/pocol-testnet'), 'pocol-testnet.service'),
    'standalone': (Path('/opt/pocol-standalone'), Path('/var/lib/pocol-standalone'), 'pocol-standalone.service'),
}


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


def real_path(path, directory=False):
    require(path.is_absolute() and '..' not in path.parts, 'public path must be absolute without parent traversal')
    for parent in (path, *path.parents):
        require(not parent.is_symlink(), 'public path contains a symbolic link')
    if directory: require(path.is_dir(), 'public directory is unavailable')


def open_public(path, limit=None):
    """Open a known public file only; refuse links and nonregular files before reading."""
    real_path(path)
    metadata = path.lstat()
    require(stat.S_ISREG(metadata.st_mode) and metadata.st_nlink == 1, 'metadata must be a regular single-link file')
    if limit is not None: require(metadata.st_size <= limit, 'public file exceeds size limit')
    fd = os.open(path, os.O_RDONLY | getattr(os, 'O_NOFOLLOW', 0) | getattr(os, 'O_NONBLOCK', 0))
    try:
        opened = os.fstat(fd)
        require(stat.S_ISREG(opened.st_mode) and opened.st_nlink == 1
                and os.path.samestat(metadata, opened), 'public file changed during inspection')
        return os.fdopen(fd, 'rb')
    except BaseException:
        os.close(fd)
        raise


def public_bytes(path, limit=512 * 1024):
    with open_public(path, limit) as stream: raw = stream.read(limit + 1)
    require(len(raw) <= limit, 'public file exceeds size limit')
    return raw


def json_object(raw):
    require(len(raw) <= 512 * 1024, 'metadata exceeds size limit')
    try: value = json.loads(raw, object_pairs_hook=unique)
    except (UnicodeError, json.JSONDecodeError): raise PreflightError('invalid JSON metadata') from None
    require(isinstance(value, dict), 'metadata must be a JSON object')
    return value


def read_json(path):
    return json_object(public_bytes(path))


def digest(path):
    result = hashlib.sha256()
    with open_public(path, 256 * 1024 * 1024) as stream:
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
    """pocold configured_forks' runtime shape: (1, 0), then versions 2, 3, ... in order up to MAX_FORK_VERSION, at
    strictly increasing heights, 1..16 entries."""
    require(isinstance(value, list) and 1 <= len(value) <= 16, 'invalid fork schedule')
    for entry in value:
        require(isinstance(entry, list) and len(entry) == 2, 'invalid fork entry')
        integer(entry[0]); integer(entry[1])
    require(value[0] == [1, 0] and all(v == i + 1 <= MAX_FORK_VERSION for i, (v, _) in enumerate(value))
            and all(a[1] < b[1] for a, b in zip(value, value[1:])), 'unsupported fork schedule')
    return value


def spec_version(network, spec_text):
    """The GenesisSpec version of a public network: the first item of genesis.spec.hex (a canonical one-byte integer
    at the head of its top-level RLP list), which must equal what devnet.json declares (deploy/public-testnet/
    pocol_identity.spec_version: `specVersion: 2` and `v3: true` together for a v3 genesis, neither for spec 1). The
    spec version is no identity input (genesisHash commits the whole spec), but a release seed refuses spec 2."""
    declared = network.get('specVersion', 1)
    require(type(declared) is int and declared in (1, 2) and network.get('v3', False) is (declared == 2),
            'unsupported or inconsistent GenesisSpec version')
    text = spec_text.strip()
    text = text[2:] if text[:2].lower() == '0x' else text
    require(re.fullmatch(r'(?:[0-9a-fA-F]{2})+', text) is not None, 'invalid public GenesisSpec')
    spec = bytes.fromhex(text)
    start = 1 if spec[0] <= 0xf7 else 1 + spec[0] - 0xf7
    require(spec[0] >= 0xc0 and start < len(spec) and 0 < spec[start] < 0x80, 'invalid public GenesisSpec version')
    require(spec[start] == declared, 'GenesisSpec version differs from the network metadata')
    if declared == 2:
        v3_genesis(spec)
    require(declared == SPEC_VERSION, 'a release seed refuses a spec-2 (v3) genesis until activation')
    return declared


def v3_genesis(spec):
    """The shape that identifies a spec-2 GenesisSpec as a v3 genesis: one canonical top list of six items whose CP is a
    list of 67 strings, 32-byte allocRoot and sysCodeHash, and the v3 program set's sysCodeHash (the only set pocold runs
    on a spec-2 genesis). The full decode is the node's (and pocol_identity.decode_genesis_spec's); a seed refuses spec 2
    either way, so this only tells a well-formed v3 genesis from a malformed one in the report."""
    def item(i, end):
        require(i < end, 'invalid spec-2 (v3) GenesisSpec')
        p = spec[i]
        if p < 0x80: return False, i, i + 1
        short = p - (0xc0 if p >= 0xc0 else 0x80)
        if short <= 55: s, n = i + 1, short
        else:
            k = short - 55
            require(i + 1 + k <= end and spec[i + 1] != 0, 'invalid spec-2 (v3) GenesisSpec')
            s, n = i + 1 + k, int.from_bytes(spec[i + 1:i + 1 + k], 'big')
            require(n > 55, 'invalid spec-2 (v3) GenesisSpec')
        require(s + n <= end, 'invalid spec-2 (v3) GenesisSpec')
        return p >= 0xc0, s, s + n
    def children(s, e):
        out = []
        while s < e:
            out.append(item(s, e)); s = out[-1][2]
        return out
    is_list, s, e = item(0, len(spec))
    require(is_list and e == len(spec), 'invalid spec-2 (v3) GenesisSpec')
    top = children(s, e)
    require(len(top) == 6 and top[2][0] and top[4][0] and not any(top[k][0] for k in (0, 1, 3, 5)),
            'invalid spec-2 (v3) GenesisSpec')
    cp = children(top[2][1], top[2][2])
    require(len(cp) == V3_CP_ITEMS and not any(x[0] for x in cp), 'invalid spec-2 (v3) GenesisSpec')
    require(all(top[k][2] - top[k][1] == 32 for k in (3, 5)), 'invalid spec-2 (v3) GenesisSpec')
    require(spec[top[5][1]:top[5][2]].hex() == V3_SYS_CODE_HASH, 'a spec-2 GenesisSpec that does not name the v3 program set')


def consensus_id(chain_id, genesis, rules, schedule):
    """An inline copy of deploy/public-testnet/pocol_identity.consensus_id (VA-SPEC §2.1), so this file still runs
    alone over stdin; test_preflight cross-checks the two. Flags are 0: a seed runs a pinned, feature-free build."""
    data = (b'pocol-consensus-identity-v1\0' + chain_id.to_bytes(8, 'big') + bytes.fromhex(genesis[2:])
            + rules.to_bytes(4, 'big') + bytes([0, len(schedule)])
            + b''.join(v.to_bytes(2, 'big') + h.to_bytes(8, 'big') for v, h in schedule))
    return hashlib.sha256(data).hexdigest()


def identity(value, schedule=None):
    """The public identity a node, bundle or binding names. A 0.4.0 one has no consensus identity and is accepted only
    as the published legacy source; any other carries consensusRulesVersion and consensusIdentity, recomputed from
    the chain, genesis, rules and fork schedule whenever the schedule is known."""
    require(isinstance(value, dict), 'invalid identity metadata')
    public = {'chainId': integer(value.get('chainId')), 'genesisHash': hex_hash(value.get('genesisHash')),
              'compatibilitySha256': source_hash(value.get('compatibilitySha256'))}
    if 'consensusIdentity' not in value:
        require(public['compatibilitySha256'] == LEGACY_SOURCE and 'consensusRulesVersion' not in value,
                'node reports no consensus identity')
        return public
    rules = value.get('consensusRulesVersion')
    require(type(rules) is int and 1 <= rules < 2**32, 'invalid consensus rules version')
    claimed = source_hash(value.get('consensusIdentity'))
    if schedule is not None:
        require(consensus_id(public['chainId'], public['genesisHash'], rules, schedule) == claimed,
                'consensus identity does not match its chain, rules and fork schedule')
    return {**public, 'consensusRulesVersion': rules, 'consensusIdentity': claimed}


def consensus(value):
    """What peers compare (VA-SPEC §2.1); the build digest is only an install-integrity fact."""
    return {k: v for k, v in identity(value).items() if k != 'compatibilitySha256'}


def binding_identity(value):
    """Validate the exact public identity used by node_operator.launch."""
    public = identity(value)
    require(set(value) == set(public) | {'protocolVersion', 'stateStore', 'node', 'miner'},
            'unexpected state binding fields')
    require(type(value.get('protocolVersion')) is int and value['protocolVersion'] >= PROTOCOL_VERSION_MIN,
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


def release_identity(network, build, schedule):
    """The identity a node of build-info `build` has on devnet.json `network`: a build that reports its consensus rules
    must be pinned (VA-SPEC §2.4), and its consensus identity is computed here, never taken from the host."""
    value = {'chainId': network.get('chainId'), 'genesisHash': network.get('genesisHash'),
             'compatibilitySha256': build.get('compatibilitySha256')}
    if 'consensusRulesVersion' in build:
        rules = build['consensusRulesVersion']
        require(build.get('consensusPinned') is True, 'the installed build has unpinned consensus sources')
        require(type(rules) is int and 1 <= rules < 2**32, 'invalid consensus rules version')
        value.update(consensusRulesVersion=rules, consensusIdentity=consensus_id(
            integer(value['chainId']), hex_hash(value['genesisHash']), rules, schedule))
    return identity(value, schedule)


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
    require(build.get('features') == [] and type(build.get('protocolVersion')) is int
            and build['protocolVersion'] >= PROTOCOL_VERSION_MIN, 'unsupported or test-feature native build')
    schedule = forks(network.get('forks', [[1, 0]]))
    # Hex of at most MAX_SPEC_BYTES (about 2.8 MB of bytes, about 5.6 MB as hex) plus a line ending.
    spec_version(network, public_bytes(bundle / 'net/genesis.spec.hex', 8 * 1024 * 1024).decode('ascii', 'replace'))
    public = release_identity(network, build, schedule)
    binding = binding_identity(manifest.get('identity', {}))
    require(identity(binding) == public, 'bundle identity mismatch')
    require(binding['node'] == node.get('name') and binding['miner'] == node.get('minerAddress')
            and binding['protocolVersion'] == build.get('protocolVersion'), 'bundle node binding mismatch')
    name = node.get('name')
    require(isinstance(name, str) and re.fullmatch(r'[A-Za-z0-9_.-]{1,64}', name), 'invalid public node name')
    return {**public, 'manifestSha256': digest(bundle / 'manifest.json'), 'nodeName': name, 'binding': binding,
            'forkSchedule': [[v, str(h)] for v, h in schedule], 'filesVerified': len(hashes)}


def service_report(name):
    require(re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.@-]{0,120}\.service', name), 'invalid service unit name')
    fields = ('Id', 'LoadState', 'ActiveState', 'SubState', 'FragmentPath', 'WorkingDirectory', 'StateDirectory', 'User', 'MainPID')
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
    state = values.get('StateDirectory', '')
    if re.fullmatch(r'[A-Za-z0-9_.-]{1,128}', state): report['stateDirectory'] = state
    pid = values.get('MainPID', '')
    if re.fullmatch(r'[0-9]{1,10}', pid): report['mainPID'] = int(pid)
    return report


def additional_node_units():
    """Return bounded unit names only, so an unknown installation remains diagnosable."""
    try:
        result = subprocess.run(['systemctl', 'list-units', '--all', '--type=service', '--no-legend',
                                 '--plain', 'pocol*.service'], capture_output=True, text=True, timeout=5, check=False)
        if result.returncode or len(result.stdout) > 32768: return []
        names = [line.split()[0] for line in result.stdout.splitlines() if line.split()]
        if len(names) > 24: return []
        return [name for name in names if re.fullmatch(r'pocol[A-Za-z0-9_.@-]{0,114}\.service', name)]
    except (OSError, subprocess.SubprocessError): return []


def choose_layout(layout, bundle, data, main_service, services):
    require(layout in (*LAYOUTS, 'auto'), 'unsupported installation layout')
    if layout != 'auto': return layout, bundle, data, main_service
    require((bundle, data, main_service) == LAYOUTS['manifest'], 'automatic discovery requires the default paths and service')
    active = [kind for kind, (_, _, unit) in LAYOUTS.items()
              if any(s.get('unit') == unit and s.get('activeState') == 'active' for s in services)]
    require(len(active) == 1, 'expected exactly one active known node service; inspect the service metadata')
    kind = active[0]
    return (kind, *LAYOUTS[kind])


def standalone_report(bundle, home):
    """Check only the installer's public allowlist; never enumerate the home. The 0.4.0 profile accepts the published
    legacy package and its v1 invitation; the R1 profile (closed until R1_BINARIES is pinned) accepts equal v2
    bindings, or a saved 0.4.0 binding with its installed v2 twin."""
    for directory in (bundle, home, home / 'network', home / 'state'): real_path(directory, directory=True)
    binaries = {name: digest(bundle / name) for name in LEGACY_BINARIES}
    legacy = binaries == LEGACY_BINARIES
    # The R1 profile stays closed until the release commit pins R1_BINARIES and R1_SOURCE.
    require(legacy or (R1_BINARIES and R1_SOURCE and binaries == R1_BINARIES),
            'standalone binary differs from the published legacy release')
    installed, saved = (read_json(path / 'network.pocol.json') for path in (bundle, home))
    v1 = lambda value: (set(value) == {'schema', 'compatibilitySha256', 'seeds', 'files'}
                        and value['schema'] == 'pocol-network-v1' and value['compatibilitySha256'] == LEGACY_SOURCE)
    if legacy:
        require(installed == saved, 'installed and saved standalone invitations differ')
        require(v1(saved), 'unsupported standalone invitation or source')
    else:
        rules = installed.get('consensusRulesVersion')
        # PR2b (D17, D23): a v2 invitation may also carry the advisory `bootstrap` list and `seedsRetired`.
        require(set(installed) - {'bootstrap', 'seedsRetired'} == {'schema', 'consensusRulesVersion', 'seeds', 'files'}
                and installed['schema'] == 'pocol-network-v2' and type(rules) is int and 1 <= rules < 2**32,
                'unsupported standalone invitation or source')
        # Equal v2 bindings, or a saved 0.4.0 binding and its installed v2 twin (same seeds and files, plan §6).
        require(installed == saved or v1(saved) and (saved['seeds'], saved['files']) == (installed['seeds'], installed['files']),
                'installed and saved standalone invitations differ')
    files = saved['files']
    require(isinstance(files, dict) and set(files) == {'alloc.json', 'devnet.json', 'genesis.spec.hex'}
            and all(isinstance(v, str) for v in files.values()), 'invalid standalone public file inventory')
    # The old genesis is pinned independently of both the RPC and the host's metadata.
    require(saved['schema'] != 'pocol-network-v1'
            or hashlib.sha256(files['genesis.spec.hex'].encode()).hexdigest() == LEGACY_SPEC_SHA256,
            'standalone GenesisSpec differs from the published old network')
    for name, content in files.items():
        require(public_bytes(home / 'network' / name) == content.encode(), 'standalone public network file differs from its invitation')
    network = json_object(files['devnet.json'])
    json_object(files['alloc.json'])
    schedule = forks(network.get('forks', [[1, 0]]))
    spec_version(network, files['genesis.spec.hex'])
    build = ({'compatibilitySha256': LEGACY_SOURCE} if legacy else
             {'compatibilitySha256': R1_SOURCE, 'consensusRulesVersion': rules, 'consensusPinned': True})
    public = release_identity(network, build, schedule)
    require((public['chainId'], public['genesisHash']) == (CHAIN_ID, GENESIS)
            and network.get('keyMode') == 'independent-key-files', 'standalone network identity or key mode differs')
    require(saved['seeds'] == [f'{host}:30300' for host in ('108.165.128.89', '108.165.128.90', '108.165.128.91')],
            'standalone seed addresses differ from the reviewed network')
    addresses = [public_bytes(path / 'identity.address', 128).decode('ascii').strip() for path in (bundle, home)]
    require(addresses[0] == addresses[1] and re.fullmatch(r'0x[0-9a-f]{40}', addresses[0]),
            'standalone public miner addresses disagree or are invalid')
    binding = binding_identity({**public, 'protocolVersion': 2, 'stateStore': True, 'node': 'pocol', 'miner': addresses[0]})
    return {**public, 'binding': binding, 'nodeName': 'pocol', 'release': 'handoff-2026-11-01' if legacy else 'R1',
            'binarySha256': binaries, 'invitationSha256': digest(home / 'network.pocol.json'),
            'forkSchedule': [[v, str(h)] for v, h in schedule], 'filesVerified': 12}


def standalone_state(home, bundle, main):
    """Bind live storage to the known executable without opening a key or database."""
    state = home / 'state'
    require(main.get('activeState') == 'active', 'standalone service is not active')
    with open_public(state / 'blocks.journal') as stream: header = stream.read(40)
    require(header == b'POCOLJ2\n' + bytes.fromhex(GENESIS[2:]), 'standalone journal genesis or version differs')
    require(main.get('user') == 'pocol-standalone' and main.get('stateDirectory') == 'pocol-standalone',
            'standalone service account or StateDirectory differs from the installer')
    pid = main.get('mainPID')
    require(type(pid) is int and pid > 1, 'standalone service has no running main process')
    process = Path('/proc') / str(pid)
    image = os.readlink(process / 'exe')
    require(image in (str(bundle / 'pocol'), str(bundle / 'pocold')), 'standalone main process uses another executable')
    require(os.path.samestat((process / 'exe').stat(), Path(image).stat())
            and home.stat().st_uid > 0 and process.stat().st_uid == home.stat().st_uid,
            'standalone process image or storage ownership differs')
    wanted = {str(state / name): name for name in ('LOCK', 'state.redb')}
    for path in wanted:
        real_path(Path(path))
        meta = Path(path).lstat()
        require(stat.S_ISREG(meta.st_mode) and meta.st_nlink == 1, 'standalone storage must use regular single-link files')
    opened = set()
    for count, fd in enumerate((process / 'fd').iterdir(), 1):
        require(count <= 4096, 'standalone process descriptor inventory exceeds the limit')
        try:
            target = os.readlink(fd)
            if target in wanted and os.path.samestat(fd.stat(), Path(target).stat()): opened.add(wanted[target])
        except FileNotFoundError: continue  # Unrelated sockets may close during the read.
    require(opened == set(wanted.values()), 'standalone main process is not holding the selected state and lock files')
    confirmed = service_report(main['unit'])
    require(confirmed.get('activeState') == 'active' and confirmed.get('mainPID') == pid
            and os.readlink(process / 'exe') == image, 'standalone process changed during inspection; collect again')
    return {'kind': 'standalone-public-invitation-journal-runtime', 'journalGenesisVerified': True,
            'mainPID': pid, 'executable': image, 'openStateFilesVerified': sorted(opened)}


def collect(rpc, bundle, data, main_service='pocol-testnet.service', services=None, check_height=None, layout='manifest'):
    bundle, data = bundle.absolute(), data.absolute()
    report = {'schema': 'pocol-mainnet-preflight-v1',
              'capturedAt': datetime.now(timezone.utc).isoformat(timespec='seconds'),
              'requestedNetworkName': 'PoCol Mainnet',
              'expectedIdentity': {'chainId': CHAIN_ID, 'genesisHash': GENESIS},
              'paths': {'bundle': str(bundle.absolute()), 'state': str(data.absolute())},
              'readOnly': True, 'deploymentPerformed': False, 'activationHeightSelected': False,
              'publicLaunchReady': False, 'checks': {}, 'issues': [], 'warnings': []}
    def fail(stage, message):
        report['checks'][stage] = False; report['issues'].append({'stage': stage, 'reason': message})
    units = [main_service, 'pocol-standalone.service', 'pocol-explorer-gateway.service',
             'pocol-studio-gateway.service', 'nginx.service', 'caddy.service']
    if services is None: units += additional_node_units()
    report['services'] = services if services is not None else [service_report(unit) for unit in dict.fromkeys(units)]
    try:
        layout, bundle, data, main_service = choose_layout(layout, bundle, data, main_service, report['services'])
        report['layout'] = layout
        report['paths'] = {'bundle': str(bundle), 'state': str(data / 'state' if layout == 'standalone' else data),
                           'service': main_service}
        if layout == 'standalone': report['paths'].update(home=str(data), network=str(data / 'network'))
        report['checks']['layout'] = True
    except PreflightError as error: fail('layout', str(error))
    try:
        params, status = rpc('pocol_getParams', []), rpc('pocol_status', [])
        require(isinstance(params, dict) and isinstance(status, dict), 'invalid node metadata')
        schedule = forks(params.get('forkSchedule', [[1, 0]]))
        public = identity(params, schedule)
        require(public == identity(status), 'node status and parameter identities disagree')
        require('consensusIdentity' not in public or params.get('consensusPinned') is True and status.get('consensusPinned') is True,
                'node runs a build with unpinned consensus sources')
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
        active = integer(params.get('activeVersion'))
        require(1 <= active <= MAX_FORK_VERSION, 'unsupported active consensus version')
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
        # A 0.4.0 node names only its build digest; its rules are revision 1 (plan §6).
        if 'consensusIdentity' not in public: report['node']['legacyRules'] = 1
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
        require(report['checks'].get('layout'), 'installation layout was not identified')
        report['bundle'] = standalone_report(bundle, data) if layout == 'standalone' else bundle_report(bundle)
        report['checks']['bundle'] = True
        if 'node' in report:
            require(consensus(report['bundle']) == consensus(report['node']), 'running consensus identity differs from the installed public bundle')
            require(identity(report['bundle']) == identity(report['node']), 'running source differs from the installed public bundle')
            require(report['bundle']['binding'] == report['node']['binding'], 'running node binding differs from the installed bundle')
            require(report['bundle']['forkSchedule'] == report['node']['forkSchedule'], 'installed and running fork schedules differ')
    except PreflightError as error: fail('bundle', str(error))
    except (OSError, ValueError, KeyError, TypeError): fail('bundle', 'public bundle could not be verified')
    try:
        require(data.is_dir() and not data.is_symlink(), 'state directory is unavailable')
        require(report['checks'].get('layout'), 'installation layout was not identified')
        if layout == 'standalone':
            require('bundle' in report and report['checks'].get('bundle'), 'standalone release and public invitation were not verified')
            main = next((s for s in report['services'] if s.get('unit') == main_service), {})
            report['bindingEvidence'] = standalone_state(data, bundle, main)
            binding = report['bundle']['binding']
        else:
            binding = binding_identity(read_json(data / 'network-binding.json'))
        report['stateBinding'] = binding
        require('bundle' in report, 'state release binding differs from the bundle')
        installed = report['bundle']['binding']
        if 'consensusIdentity' in binding:
            # node_operator starts on a marker with the same consensus binding and never rewrites it (plan §4).
            require(all(binding.get(k) == installed.get(k) for k in BOUND), 'state release binding differs from the bundle')
            if binding != installed:
                report['warnings'].append('the state binding names another build of the same consensus rules')
        else:
            # A 0.4.0 marker is bound to its own release exactly; this check knows no certified legacy build.
            require(binding == installed, 'state release binding differs from the bundle')
        space = os.statvfs(data / 'state' if layout == 'standalone' else data)
        report['storage'] = {'availableBytes': str(space.f_bavail * space.f_frsize)}
        report['checks']['stateBinding'] = True
    except PreflightError as error: fail('stateBinding', str(error))
    except (OSError, ValueError, KeyError, TypeError): fail('stateBinding', 'public state binding could not be read')
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
    parser.add_argument('--layout', choices=['manifest', 'standalone', 'auto'], default='manifest',
                        help='auto selects exactly one active known installer; no arbitrary filesystem discovery')
    parser.add_argument('--check-height', type=int, help='Read the same canonical block height on every seed for comparison; this is not an activation height')
    parser.add_argument('--out', type=Path, help='Create a new sanitized report file; existing files are never replaced')
    args = parser.parse_args()
    try:
        if args.layout == 'standalone' and (args.bundle, args.data, args.service) == LAYOUTS['manifest']:
            args.bundle, args.data, args.service = LAYOUTS['standalone']
        report = collect(LocalRpc(args.rpc), args.bundle, args.data, args.service,
                         check_height=args.check_height, layout=args.layout)
        text = json.dumps(report, indent=2) + '\n'
        if args.out:
            with args.out.open('x', encoding='utf-8') as stream: stream.write(text)
        print(text, end='')
        return 0 if report['result'] == 'PASS' else 2
    except (PreflightError, OSError, ValueError) as error:
        parser.error(str(error) if isinstance(error, PreflightError) else 'preflight input or report output is unavailable')


if __name__ == '__main__':
    sys.exit(main())
