#!/usr/bin/env python3
"""Prepare isolated multi-host simnet bundles and check them through SSH tunnels.

Does not provision machines, change firewalls or contact remote servers during prepare.
"""
import argparse
import datetime
import ipaddress
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time
import urllib.parse
import urllib.request

from runtime import digest, verify

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
PRIVATE = [ipaddress.ip_network(s) for s in ('10.0.0.0/8', '172.16.0.0/12', '192.168.0.0/16')]
LOCAL_ARGS = ['--delta', '1', '--phi', '1', '--t-exec', '1', '--delta-rel', '8', '--gamma', '5']
TIMING_ARGS = ['--t-blk', '10', '--tau', '600', '--d-att', '60', '--fb-wait', '6']
BINARIES = ('pocold', 'pocol-sign', 'pocol-site')


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2) + '\n')


def validate_inventory(value, rehearsal=False):
    if set(value) != {'name', 'chainId', 'nodes'}:
        raise ValueError('Inventory requires exactly name, chainId, nodes')
    if not re.fullmatch(r'[a-z][a-z0-9-]{0,31}', value['name']):
        raise ValueError('Invalid network name')
    cid = value['chainId']
    if type(cid) is not int or not 1 <= cid < 2**53 or cid == 777001:
        raise ValueError('Use a test chainId (not 777001), within JSON safe integer range')
    nodes = value['nodes']
    if not isinstance(nodes, list) or not 3 <= len(nodes) <= 8:
        raise ValueError('This pilot supports 3 to 8 trusted nodes')
    names, endpoints, ips, rpc_endpoints = set(), set(), set(), set()
    for node in nodes:
        if set(node) != {'name', 'ip', 'p2pPort', 'rpcPort'}:
            raise ValueError('Node requires exactly name, ip, p2pPort, rpcPort')
        if not re.fullmatch(r'[a-z][a-z0-9-]{0,31}', node['name']) or node['name'] in names:
            raise ValueError('Node names must be unique safe labels')
        names.add(node['name'])
        addr = ipaddress.IPv4Address(node['ip'])
        if not (addr.is_loopback if rehearsal else any(addr in net for net in PRIVATE)):
            raise ValueError('Use loopback for rehearsal; RFC1918 IPv4 for deployment')
        if not rehearsal and node['ip'] in ips:
            raise ValueError('Use a separate private IP per server')
        ips.add(node['ip'])
        for key in ('p2pPort', 'rpcPort'):
            if type(node[key]) is not int or not 1024 <= node[key] <= 65535:
                raise ValueError('Ports must be integers between 1024 and 65535')
        endpoint = (node['ip'], node['p2pPort'])
        if endpoint in endpoints:
            raise ValueError('Duplicate P2P endpoint')
        endpoints.add(endpoint)
        rpc_endpoint = node['rpcPort'] if rehearsal else node['ip']
        if rpc_endpoint in rpc_endpoints:
            raise ValueError('Rehearsal RPC ports must be unique')
        rpc_endpoints.add(rpc_endpoint)
        if node['ip'] == '127.0.0.1' and node['p2pPort'] == node['rpcPort']:
            raise ValueError('P2P and RPC listeners collide')
    if rehearsal and any(ip == '127.0.0.1' and port in rpc_endpoints for ip, port in endpoints):
        raise ValueError('P2P and RPC listeners collide')
    return value


def service(node, nodes):
    allow = '\n'.join('IPAddressAllow=' + ip + '/32' for ip in sorted({n['ip'] for n in nodes}))
    return f'''[Unit]
Description=PoCol private experimental testnet ({node['name']})
Wants=network-online.target
After=network-online.target
StartLimitIntervalSec=300
StartLimitBurst=3

[Service]
Type=simple
User=pocol
Group=pocol
WorkingDirectory=/opt/pocol-private
ExecStartPre=/usr/bin/python3 /opt/pocol-private/runtime.py verify
ExecStart=/usr/bin/python3 /opt/pocol-private/runtime.py run
Restart=on-failure
RestartSec=15
TimeoutStopSec=60
StateDirectory=pocol-private
StateDirectoryMode=0700
UMask=0077
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes
PrivateDevices=yes
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectControlGroups=yes
RestrictSUIDSGID=yes
RestrictAddressFamilies=AF_INET AF_UNIX
CapabilityBoundingSet=
MemoryMax=7G
TasksMax=128
LimitNOFILE=4096
IPAddressDeny=any
IPAddressAllow=127.0.0.0/8
{allow}

[Install]
WantedBy=multi-user.target
'''


def prepare(args):
    inventory = validate_inventory(json.loads(args.inventory.read_text()), args.rehearsal)
    if args.out.exists():
        raise ValueError('Output already exists; never overwrite a genesis or bundle')
    for name in BINARIES:
        if not (args.bin_dir / name).is_file():
            raise ValueError('Build --release --bins first: missing ' + name)
    # A single genesis is generated once, then copied byte-for-byte to every host.
    args.out.mkdir(parents=True, mode=0o700)
    net = args.out / 'network'
    genesis_time = args.genesis_time or int(time.time()) + 600
    command = [str(args.bin_dir.resolve() / 'pocold'), 'init-devnet', '--out', str(net),
               '--miners', ','.join(n['name'] for n in inventory['nodes']),
               '--chain-id', str(inventory['chainId']), '--genesis-time', str(genesis_time)]
    result = subprocess.run(command + TIMING_ARGS + LOCAL_ARGS, capture_output=True, text=True, timeout=120)
    (args.out / 'prepare.log').write_text(result.stderr + result.stdout)
    if result.returncode:
        raise ValueError('ParamGate/init failed; see prepare.log. No deployment bundles produced.')
    info = json.loads((net / 'devnet.json').read_text())
    if info['profileKind'] != 'simnet' or info['admission']['scope'] != 'experimental-devnet':
        raise ValueError('Unexpected admission scope')
    revision = subprocess.run(['git', 'rev-parse', 'HEAD'], cwd=ROOT, capture_output=True, text=True).stdout.strip()
    dirty = bool(subprocess.run(['git', 'status', '--porcelain'], cwd=ROOT, capture_output=True, text=True).stdout)
    manifests = {}
    for node in inventory['nodes']:
        bundle = args.out / node['name']
        bundle.mkdir()
        shutil.copytree(net, bundle / 'net')
        (bundle / 'bin').mkdir()
        for name in BINARIES:
            shutil.copy2(args.bin_dir / name, bundle / 'bin' / name)
        shutil.copy2(HERE / 'runtime.py', bundle / 'runtime.py')
        config = {'name': node['name'], 'listen': f"{node['ip']}:{node['p2pPort']}",
                  'rpc': f"127.0.0.1:{node['rpcPort']}", 'localArgs': LOCAL_ARGS,
                  'peers': [f"{peer['ip']}:{peer['p2pPort']}" for peer in inventory['nodes'] if peer != node]}
        write_json(bundle / 'node.json', config)
        (bundle / 'pocol-private.service').write_text(service(node, inventory['nodes']))
        profile = {'name': inventory['name'], 'chainId': info['chainId'],
                   'genesisHash': info['genesisHash'], 'genesisPre': (net / 'genesis.spec.hex').read_text().strip(),
                   'endpoint': f"http://127.0.0.1:{node['rpcPort']}", 'trustLevel': 'LN'}
        write_json(bundle / 'browser-profile.json', profile)
        manifest = {'schema': 'pocol-private-bundle-v1', 'network': inventory['name'],
                    'rehearsalOnly': args.rehearsal, 'sourceRevisionAtPackaging': revision,
                    'sourceDirtyAtPackaging': dirty, 'chainId': info['chainId'],
                    'genesisHash': info['genesisHash'], 'sha256': {
                        p.relative_to(bundle).as_posix(): digest(p)
                        for p in sorted(bundle.rglob('*')) if p.is_file()}}
        write_json(bundle / 'manifest.json', manifest)
        verify(bundle)
        manifests[node['name']] = digest(bundle / 'manifest.json')
    write_json(args.out / 'inventory.json', inventory)
    write_json(args.out / 'release.json', {'schema': 'pocol-private-release-v1',
               'createdAt': datetime.datetime.now(datetime.timezone.utc).isoformat(),
               'rehearsalOnly': args.rehearsal, 'chainId': info['chainId'], 'genesisHash': info['genesisHash'],
               'genesisTime': genesis_time, 'bundleManifestSha256': manifests,
               'admission': info['admission'], 'deploymentPerformed': False})
    print(json.dumps({'prepared': str(args.out), 'nodes': list(manifests),
                      'genesisHash': info['genesisHash'], 'deploymentPerformed': False}))


def rpc(url, method, params=None):
    parsed = urllib.parse.urlparse(url)
    if parsed.scheme != 'http' or parsed.hostname != '127.0.0.1' or not parsed.port or parsed.username or parsed.password:
        raise ValueError('Use http://127.0.0.1:PORT through a local SSH tunnel')
    request = urllib.request.Request(url, data=json.dumps({'jsonrpc': '2.0', 'id': 1,
        'method': method, 'params': params or []}).encode(), headers={'Content-Type': 'application/json'})
    # Local endpoints must not be sent to an HTTP proxy; reject redirects as well.
    class NoRedirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self, *args, **kwargs):
            raise ValueError('RPC redirect refused')
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())
    with opener.open(request, timeout=10) as response:
        value = json.loads(response.read(2 * 1024 * 1024))
    if value.get('id') != 1 or value.get('jsonrpc') != '2.0' or 'error' in value:
        raise ValueError('RPC failed: ' + str(value))
    return value['result']


def check(args):
    release = json.loads((args.release / 'release.json').read_text())
    urls = dict(item.split('=', 1) for item in args.rpc)
    if set(urls) != set(release['bundleManifestSha256']) or len(set(urls.values())) != len(urls):
        raise ValueError('Provide one distinct local RPC tunnel for every node in the release')
    if not 1 <= args.observe <= 3600:
        raise ValueError('Observation must be 1..3600 seconds')

    def snapshot():
        values = {name: rpc(url, 'pocol_status') for name, url in urls.items()}
        for name, status in values.items():
            if (status['chainId'], status['genesisHash']) != (release['chainId'], release['genesisHash']):
                raise ValueError('Wrong network on ' + name)
            if status['name'] != name or not status['stateStore']:
                raise ValueError('Wrong node identity or storage mode on ' + name)
            if status['admission']['scope'] != 'experimental-devnet' or status['admission']['specificationCompliant']:
                raise ValueError('Unexpected admission claim on ' + name)
            if not status['peers']:
                raise ValueError('No connected peers on ' + name)
        return values

    before = snapshot()
    time.sleep(args.observe)
    after = snapshot()
    if any(after[n]['head']['height'] <= before[n]['head']['height'] for n in urls):
        raise ValueError('A node did not advance during the observation')
    height = min(s['head']['height'] for s in after.values()) - 6
    if height < 1:
        raise ValueError('Wait for at least seven blocks on every node')
    blocks = [rpc(url, 'eth_getBlockByNumber', [hex(height), False]) for url in urls.values()]
    if any(b is None for b in blocks) or len({(b['hash'], b['stateRoot'], b['receiptsRoot']) for b in blocks}) != 1:
        raise ValueError('Nodes disagree at the comparison height')
    # Re-read to detect a reorg during the sequential queries.
    if any(rpc(url, 'eth_getBlockByNumber', [hex(height), False])['hash'] != blocks[0]['hash'] for url in urls.values()):
        raise ValueError('Comparison changed during the check; rerun after convergence')
    report = {'result': 'PASS', 'scope': 'operational-smoke-only', 'observedSeconds': args.observe,
              'chainId': release['chainId'], 'genesisHash': release['genesisHash'],
              'commonHeight': height, 'commonHash': blocks[0]['hash'],
              'heightsBefore': {n: s['head']['height'] for n, s in before.items()},
              'heightsAfter': {n: s['head']['height'] for n, s in after.items()},
              'realMultiHostProven': False,
              'note': 'RPC observations alone cannot prove host separation, isolation or timing bounds.'}
    if args.report:
        write_json(args.report, report)
    print(json.dumps(report, indent=2))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='command', required=True)
    prep = sub.add_parser('prepare')
    prep.add_argument('--inventory', type=Path, required=True)
    prep.add_argument('--bin-dir', type=Path, default=ROOT / 'prototypes/pocold/target/release')
    prep.add_argument('--out', type=Path, required=True)
    prep.add_argument('--genesis-time', type=int)
    prep.add_argument('--rehearsal', action='store_true', help='Loopback-only rehearsal, not a deployment')
    chk = sub.add_parser('check')
    chk.add_argument('--release', type=Path, required=True)
    chk.add_argument('--rpc', action='append', required=True, metavar='NODE=http://127.0.0.1:PORT')
    chk.add_argument('--observe', type=int, default=180)
    chk.add_argument('--report', type=Path)
    args = parser.parse_args()
    prepare(args) if args.command == 'prepare' else check(args)


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print('private-testnet: ' + str(error), file=sys.stderr)
        sys.exit(1)
