#!/usr/bin/env python3
"""Package one admitted public-input testnet for seed operators or a joining node."""
import argparse
import ipaddress
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys

from node_operator import BINARIES, SCHEMA, digest, files, identity, read_json, verify, write_json

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]


def allowed_ip(address, rehearsal):
    return address.is_loopback if rehearsal else (address.is_global and not address.is_multicast and not address.is_reserved)


def validate_inventory(value, rehearsal=False, join=False):
    if set(value) != {'name', 'nodes'}: raise ValueError('Inventory requires exactly name and nodes')
    if not isinstance(value['name'], str) or not re.fullmatch(r'[a-z][a-z0-9-]{0,31}', value['name']):
        raise ValueError('Unsafe network name')
    nodes = value['nodes']
    if not isinstance(nodes, list) or not (len(nodes) == 1 if join else 3 <= len(nodes) <= 8):
        raise ValueError('Use 3..8 bootstrap nodes, or --join for one new node')
    names, endpoints, rpc_ports, miners, ips = set(), set(), set(), set(), set()
    for node in nodes:
        if set(node) != {'name', 'ip', 'p2pPort', 'rpcPort', 'rpcHost', 'minerAddress', 'seeds'}:
            raise ValueError('Invalid node fields')
        if not isinstance(node['name'], str) or not re.fullmatch(r'[a-z][a-z0-9-]{0,31}', node['name']) or node['name'] in names:
            raise ValueError('Node names must be unique safe labels')
        names.add(node['name'])
        addr = ipaddress.IPv4Address(node['ip'])
        if not allowed_ip(addr, rehearsal):
            raise ValueError('Deployment needs public IPv4; rehearsal needs loopback')
        if not rehearsal and addr in ips: raise ValueError('Bootstrap servers need distinct public addresses')
        ips.add(addr)
        for field in ['p2pPort', 'rpcPort']:
            if type(node[field]) is not int or not 1024 <= node[field] <= 65535: raise ValueError('Invalid port')
        if node['p2pPort'] == node['rpcPort']: raise ValueError('RPC/P2P ports collide')
        endpoint = f"{addr}:{node['p2pPort']}"
        if endpoint in endpoints: raise ValueError('Duplicate endpoint')
        endpoints.add(endpoint)
        if rehearsal and node['rpcPort'] in rpc_ports: raise ValueError('Rehearsal RPC ports must be distinct')
        rpc_ports.add(node['rpcPort'])
        host = node['rpcHost']
        if not isinstance(host, str) or len(host) > 253 or not re.fullmatch(r'[a-z0-9](?:[a-z0-9.-]*[a-z0-9])?', host) or '.' not in host:
            raise ValueError('Use a lowercase DNS name for rpcHost')
        if any(not label or len(label) > 63 or label.startswith('-') or label.endswith('-') for label in host.split('.')):
            raise ValueError('Invalid RPC hostname')
        miner = node['minerAddress']
        if miner is not None:
            if not isinstance(miner, str) or not re.fullmatch(r'0x[0-9a-f]{40}', miner) or int(miner[2:], 16) == 0 or miner in miners:
                raise ValueError('Each miner needs a distinct lowercase public address')
            miners.add(miner)
        if not isinstance(node['seeds'], list) or len(node['seeds']) > 7: raise ValueError('At most seven outbound seeds')
        if join and not node['seeds']: raise ValueError('A joining node needs a published seed')
        if len(set(node['seeds'])) != len(node['seeds']): raise ValueError('Duplicate seeds')
        for seed in node['seeds']:
            ip, port = seed.split(':'); address = ipaddress.IPv4Address(ip)
            if not allowed_ip(address, rehearsal) or not port.isdecimal() or str(int(port)) != port or not 1024 <= int(port) <= 65535 or seed == endpoint:
                raise ValueError('Invalid seed endpoint')
    if rehearsal and any(n['ip'] == '127.0.0.1' and n['p2pPort'] in rpc_ports for n in nodes):
        raise ValueError('A P2P listener collides with a loopback RPC port')
    return value


def service(node):
    # systemd credentials are readable by the service but their filesystem metadata
    # need not satisfy pocol-key's single-owner/single-link checks. Materialize a
    # service-owned 0600 copy in a private, stop-cleaned runtime directory instead
    # of weakening the Rust key loader or placing a secret in state/backups.
    credentials = '''LoadCredential=miner.key:/etc/pocol-testnet/miner.pocol-key
RuntimeDirectory=pocol-testnet
RuntimeDirectoryMode=0700
RuntimeDirectoryPreserve=no
ExecStartPre=/usr/bin/install -m 0600 ${CREDENTIALS_DIRECTORY}/miner.key /run/pocol-testnet/miner.pocol-key
''' if node['minerAddress'] else ''
    key_arg = ' --key-file /run/pocol-testnet/miner.pocol-key' if node['minerAddress'] else ''
    return f'''[Unit]
Description=PoCol experimental public testnet ({node['name']})
Wants=network-online.target time-sync.target
After=network-online.target time-sync.target
StartLimitIntervalSec=300
StartLimitBurst=3

[Service]
Type=simple
User=pocol-testnet
Group=pocol-testnet
WorkingDirectory=/opt/pocol-testnet
{credentials}ExecStartPre=/usr/bin/python3 -B /opt/pocol-testnet/node_operator.py verify
ExecStart=/usr/bin/python3 -B /opt/pocol-testnet/node_operator.py run{key_arg}
Restart=on-failure
RestartSec=15
TimeoutStopSec=60
StateDirectory=pocol-testnet
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

[Install]
WantedBy=multi-user.target
'''


def nginx(node):
    # Standalone config for a dedicated gateway host. Not installed or enabled automatically.
    return f'''worker_processes 2;
pid /run/nginx-pocol.pid;
error_log /var/log/nginx/pocol-error.log warn;
events {{ worker_connections 256; }}
http {{
    access_log off;
    server_tokens off;
    limit_req_zone $binary_remote_addr zone=pocol_ip_rate:4m rate=2r/s;
    limit_req_zone $server_name zone=pocol_total_rate:1m rate=10r/s;
    limit_conn_zone $binary_remote_addr zone=pocol_ip_conn:4m;
    limit_conn_zone $server_name zone=pocol_total_conn:1m;
    client_header_timeout 5s;
    client_body_timeout 5s;
    client_max_body_size 256k;
    client_body_buffer_size 256k;
    keepalive_timeout 5s;
    keepalive_requests 100;
    send_timeout 10s;
    server {{
        listen 443 ssl default_server;
        server_name {node['rpcHost']};
        ssl_certificate /etc/pocol-testnet/tls/fullchain.pem;
        ssl_certificate_key /etc/pocol-testnet/tls/privkey.pem;
        ssl_protocols TLSv1.2 TLSv1.3;
        ssl_session_cache shared:POCOL_TLS:4m;
        ssl_session_timeout 10m;
        if ($host != {node['rpcHost']}) {{ return 444; }}
        location = / {{
            if ($request_method != POST) {{ return 405; }}
            limit_req zone=pocol_ip_rate burst=4 nodelay;
            limit_req zone=pocol_total_rate burst=10 nodelay;
            limit_req_status 429;
            limit_conn pocol_ip_conn 2;
            limit_conn pocol_total_conn 16;
            limit_conn_status 429;
            proxy_pass http://127.0.0.1:{node['rpcPort']};
            proxy_http_version 1.1;
            proxy_set_header Connection close;
            proxy_set_header Host 127.0.0.1;
            # Keep the client's Origin. Stripping it would defeat the node's origin policy.
            proxy_set_header Origin $http_origin;
            proxy_set_header X-Forwarded-For "";
            proxy_connect_timeout 2s;
            proxy_send_timeout 5s;
            proxy_read_timeout 15s;
            proxy_buffering off;
            proxy_request_buffering on;
            proxy_max_temp_file_size 0;
        }}
        location / {{ return 404; }}
    }}
}}
'''


def prepare(inventory, network, bin_dir, out, rehearsal=False, join=False):
    inventory = validate_inventory(inventory, rehearsal, join)
    if out.exists(): raise ValueError('Never overwrite a release')
    if set(files(network)) != {'genesis.spec.hex', 'alloc.json', 'devnet.json'}:
        raise ValueError('Network directory must contain exactly the three public genesis files')
    info = read_json(network / 'devnet.json')
    if info.get('keyMode') != 'independent-key-files' or info['admission']['scope'] != 'experimental-devnet':
        raise ValueError('Generate independent genesis with init-testnet, not label-based init-devnet')
    build = json.loads(subprocess.check_output([str(bin_dir / 'pocold'), 'build-info'], text=True, timeout=10))
    if build['features'] or build['protocolVersion'] != 2: raise ValueError('Use the feature-free wire-v2 release build')
    # Catch a stale binary packaged from a changed checkout. No dirty-tree override.
    for name, expected in build['sourceSha256'].items():
        path = ROOT / 'prototypes' / name
        if not path.is_file() or digest(path) != expected: raise ValueError('Rebuild stale binary: ' + name)
    for name in BINARIES:
        if (bin_dir / name).is_symlink() or not (bin_dir / name).is_file(): raise ValueError('Missing regular binary: ' + name)
    out.mkdir(parents=True, mode=0o700)
    manifests = {}
    nodes = inventory['nodes']
    for node in nodes:
        bundle = out / node['name']; bundle.mkdir()
        shutil.copytree(network, bundle / 'net'); (bundle / 'bin').mkdir()
        for name in BINARIES: shutil.copy2(bin_dir / name, bundle / 'bin' / name)
        for name in ('node_operator.py', 'install.sh'): shutil.copy2(HERE / name, bundle / name)
        peers = sorted(set(node['seeds'] + [f"{n['ip']}:{n['p2pPort']}" for n in nodes if n != node]))
        if len(peers) > 7: raise ValueError('At most seven configured outbound peers in this release')
        config = {'name': node['name'], 'minerAddress': node['minerAddress'],
            'listen': f"{node['ip'] if rehearsal else '0.0.0.0'}:{node['p2pPort']}",
            'rpc': f"127.0.0.1:{node['rpcPort']}", 'peers': peers}
        write_json(bundle / 'node.json', config); write_json(bundle / 'build-info.json', build)
        (bundle / 'pocol-testnet.service').write_text(service(node))
        (bundle / 'nginx.conf').write_text(nginx(node))
        (bundle / 'README.md').write_text((HERE / 'README.md').read_text())
        manifest = {'schema': SCHEMA, 'rehearsalOnly': rehearsal,
                    'identity': identity(config, info, build),
                    'sha256': {name: digest(path) for name, path in files(bundle).items()}}
        write_json(bundle / 'manifest.json', manifest); verify(bundle)
        manifests[node['name']] = digest(bundle / 'manifest.json')
    write_json(out / 'inventory.json', inventory)
    release = {'schema': 'pocol-public-testnet-release-v1', 'chainId': info['chainId'],
        'genesisHash': info['genesisHash'], 'genesisTime': info['genesisTime'],
        'protocolVersion': build['protocolVersion'], 'compatibilitySha256': build['compatibilitySha256'],
        'bundleManifestSha256': manifests, 'rehearsalOnly': rehearsal,
        'admission': info['admission'], 'publicLaunchReady': False, 'deploymentPerformed': False,
        'seeds': sorted(set(p for n in nodes for p in n['seeds']) | {f"{n['ip']}:{n['p2pPort']}" for n in nodes})}
    write_json(out / 'release.json', release)
    return release


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--inventory', type=Path, required=True); p.add_argument('--network', type=Path, required=True)
    p.add_argument('--bin-dir', type=Path, default=ROOT / 'prototypes/pocold/target/release')
    p.add_argument('--out', type=Path, required=True)
    p.add_argument('--rehearsal', action='store_true'); p.add_argument('--join', action='store_true')
    a = p.parse_args()
    print(json.dumps(prepare(read_json(a.inventory), a.network.resolve(), a.bin_dir.resolve(), a.out.absolute(), a.rehearsal, a.join), indent=2))


if __name__ == '__main__':
    try: main()
    except (OSError, ValueError, KeyError, TypeError, subprocess.SubprocessError) as error:
        print('testnet prepare: ' + str(error), file=sys.stderr); sys.exit(1)
