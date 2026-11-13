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

import pocol_identity
from node_operator import BINARIES, SCHEMA, digest, files, identity, read_json, verify, write_json

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]


def allowed_ip(address, rehearsal):
    return address.is_loopback if rehearsal else (address.is_global and not address.is_multicast and not address.is_reserved)


def freeze_height(node):
    # V3-33 re-genesis conduct: the height S after which this node builds no block with user transactions and its pool
    # refuses them (pocold --freeze-user-txs-after S). Node-local; other nodes' blocks are judged as before. It names
    # the network it is for (`freezeGenesisHash`, that network's genesis hash): `prepare` refuses a bundle of another
    # network, so an inventory reused for the new network after the re-genesis cannot freeze it at the old S.
    value, named = node.get('freezeUserTxsAfter'), node.get('freezeGenesisHash')
    if type(value) is not int or not 0 <= value < 2**64: raise ValueError('freezeUserTxsAfter is a block height')
    if not isinstance(named, str) or not re.fullmatch(r'0x[0-9a-fA-F]{64}', named):
        raise ValueError('freezeUserTxsAfter needs freezeGenesisHash, the genesis hash of the network it is for')
    return value


def freeze_config(node, genesis_hash):
    """The node.json fields of a validated inventory node's freeze for the network of `genesis_hash`: none without
    one; a freeze set for another network is refused (remove both fields for the new network)."""
    if 'freezeUserTxsAfter' not in node: return {}
    if node['freezeGenesisHash'].lower() != str(genesis_hash).lower():
        raise ValueError(f"{node['name']}: freezeUserTxsAfter was set for another network (freezeGenesisHash "
                         f"{node['freezeGenesisHash']}, this network {genesis_hash}); remove both fields")
    return {'freezeUserTxsAfter': node['freezeUserTxsAfter'], 'freezeGenesisHash': node['freezeGenesisHash']}


def validate_inventory(value, rehearsal=False, join=False):
    if set(value) != {'name', 'nodes'}: raise ValueError('Inventory requires exactly name and nodes')
    if not isinstance(value['name'], str) or not re.fullmatch(r'[a-z][a-z0-9-]{0,31}', value['name']):
        raise ValueError('Unsafe network name')
    nodes = value['nodes']
    if not isinstance(nodes, list) or not (len(nodes) == 1 if join else 3 <= len(nodes) <= 8):
        raise ValueError('Use 3..8 bootstrap nodes, or --join for one new node')
    names, endpoints, rpc_ports, miners, ips = set(), set(), set(), set(), set()
    for node in nodes:
        # PR2b (D17, D23): optional `bootstrap` (community listeners, at most 16, never a seed) and `seedsRetired`.
        # V3-33: optional `freezeUserTxsAfter` with `freezeGenesisHash` (node-local; pocold --freeze-user-txs-after).
        if set(node) - {'bootstrap', 'seedsRetired', 'freezeUserTxsAfter', 'freezeGenesisHash'} != {'name', 'ip', 'p2pPort', 'rpcPort', 'rpcHost', 'minerAddress', 'seeds'}:
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
        boot = node.get('bootstrap', [])
        if not isinstance(boot, list) or len(boot) > 16 or len(set(boot)) != len(boot) or any(b in node['seeds'] for b in boot):
            raise ValueError('At most 16 distinct bootstrap addresses, none a seed')
        if type(node.get('seedsRetired', False)) is not bool: raise ValueError('seedsRetired is true or false')
        if 'freezeUserTxsAfter' in node or 'freezeGenesisHash' in node: freeze_height(node)
        for b in boot:
            ip, _, port = b.rpartition(':')
            if not allowed_ip(ipaddress.IPv4Address(ip), rehearsal) or not port.isdecimal() or str(int(port)) != port or not 1024 <= int(port) <= 65535:
                raise ValueError('Invalid bootstrap endpoint')
        for seed in node['seeds']:
            ip, port = seed.split(':'); address = ipaddress.IPv4Address(ip)
            if not allowed_ip(address, rehearsal) or not port.isdecimal() or str(int(port)) != port or not 1024 <= int(port) <= 65535 or seed == endpoint:
                raise ValueError('Invalid seed endpoint')
    if rehearsal and any(n['ip'] == '127.0.0.1' and n['p2pPort'] in rpc_ports for n in nodes):
        raise ValueError('A P2P listener collides with a loopback RPC port')
    return value


def service(node):
    # TasksMax holds pocold's A13 thread inventory, 2*(peers_max + 8) + 4*peers + 3*S + 1 + 40 tasks with S = 5 automatic
    # outbound slots (PR2b; 164 at the default 32 established inbound and 7 peers, 368 at --peers-max 128 with S = 9);
    # pocold refuses to boot past it. Seeds and joining
    # nodes run with the default 32 established inbound: 64 at seeds would move the live chain's A13 halt by more
    # than a week of records (PR3 seed rule, tests/network_budget.rs a13_live_chain_halt_records), so no
    # POCOL_PEERS_MAX or POCOL_PER_GROUP_MAX line is written.
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
TasksMax=512
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
    # A tree whose consensus sources are not pinned peers with no released node (VA-SPEC §2.4).
    if build.get('consensusPinned') is not True or 'consensusRulesVersion' not in build:
        raise ValueError('Use a release build with pinned consensus sources')
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
        for name in ('node_operator.py', 'pocol_identity.py', 'install.sh'): shutil.copy2(HERE / name, bundle / name)
        # PR2b: the bootstrap (seed) nodes keep their static mesh; a joining node dials the published seeds as seeds
        # (`--seed`: only to bootstrap and refill, VA-SPEC §1a.8), none once they are retired, and tries the bootstrap list.
        peers = [] if join else sorted(set(node['seeds'] + [f"{n['ip']}:{n['p2pPort']}" for n in nodes if n != node]))
        if len(peers) > 7: raise ValueError('At most seven configured outbound peers in this release')
        config = {'name': node['name'], 'minerAddress': node['minerAddress'],
            'listen': f"{node['ip'] if rehearsal else '0.0.0.0'}:{node['p2pPort']}",
            'rpc': f"127.0.0.1:{node['rpcPort']}", 'peers': peers}
        if join: config.update(seeds=[] if node.get('seedsRetired') else list(node['seeds']), bootstrap=list(node.get('bootstrap', [])))
        config.update(freeze_config(node, info['genesisHash']))
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
        'protocolVersion': build['protocolVersion'], 'protocolVersionMin': build['protocolVersionMin'],
        # The build digest is informational; nodes peer on the consensus identity (VA-SPEC §2.1).
        'compatibilitySha256': build['compatibilitySha256'], 'consensusRulesVersion': build['consensusRulesVersion'],
        'consensusIdentity': pocol_identity.for_network(info, build),
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
