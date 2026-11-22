#!/usr/bin/env python3
"""Installer of the explorer gateway host's own node: a non-mining PoCol node (`pocold run` without a miner key)
that follows the network from its invitation's seeds and answers RPC on 127.0.0.1:8545 only, for the explorer gateway
(install_explorer_gateway.py) on the same host. It holds no key: it never mines and never signs.

    sudo python3 deploy/explorer/install_observer_node.py --invitation network.pocol.json \\
        --pocold /path/to/release/pocold --pocold-sha256 HEX [--invitation-sha256 HEX] [--listen 0.0.0.0:30300] --check
    sudo python3 deploy/explorer/install_observer_node.py ...same arguments without --check

--pocold is the pocold binary of the signed release (verify the release signature first, deploy/release); its
SHA-256 must be --pocold-sha256 (from the release's verified SHA256SUMS). --invitation is the network's invitation
file; with --invitation-sha256 its digest must match (the owner publishes it; PoCol Mainnet's is a74c1cf4...f233).
The installer copies both, writes the network files exactly as the invitation carries them (as `pocol start` does),
and writes pocol-observer.service:

- `pocold run --network /etc/pocol-observer/network --data /var/lib/pocol-observer/data --name explorer-observer
  --rpc 127.0.0.1:8545 --seed <each invitation seed>` and no --miner or --miner-key-file (the installer refuses to
  write a unit with either, or with an RPC address that is not 127.0.0.1:8545);
- inbound P2P only with --listen (default: outbound connections to the seeds only);
- DynamicUser, StateDirectory, ProtectSystem=strict and the standalone node's memory sizing (MemoryMax=7G for the
  6 GiB A13 budget: a host with 8 GiB of RAM or more).

--check runs every preflight and changes nothing: the binary digest and `pocold --help`-free version check, the
invitation (schema pocol-network-v2, its files, its seeds, chain ID and genesis hash, and the explorer gateway's
/etc/pocol-explorer-gateway/network.json naming the same network when it exists), the free RPC port. An install backs
up what it replaces under /var/backups/pocol-observer, starts the unit and waits up to --wait seconds (default 120)
for the node's RPC to name the invitation's chain ID and genesis hash with no miner; on any failure it restores the
previous unit and files. Syncing the chain takes longer than that check: the explorer gateway's install
(install_explorer_gateway.py) waits for nothing more than the identity, and its pages show the node's head as it syncs.
Standard library only; contacts nothing but 127.0.0.1:8545.
"""
import hashlib
import http.client
import ipaddress
import json
import os
from pathlib import Path
import re
import shutil
import socket
import stat
import subprocess
import sys
import tempfile
import time

BASE = Path('/opt/pocol-observer')
CONFIG = Path('/etc/pocol-observer')
NETWORK_DIR = CONFIG / 'network'
INVITATION = CONFIG / 'network.pocol.json'
STATE = Path('/var/lib/pocol-observer')
SERVICE = 'pocol-observer.service'
UNIT = Path('/etc/systemd/system') / SERVICE
GATEWAY_NETWORK = Path('/etc/pocol-explorer-gateway/network.json')
BACKUPS = Path('/var/backups/pocol-observer')
MARKER = 'managed-by-observer-v1'
RPC = '127.0.0.1:8545'
FILES = ('alloc.json', 'devnet.json', 'genesis.spec.hex')
INVITATION_MAX = 1 << 20
SEEDS_MAX = 16
WAIT, WAIT_RANGE = 120, (10, 1800)
ROOT_UID = 0


def command(*args, check=True):
    return subprocess.run(args, check=check, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=45)


def sha256(raw):
    return hashlib.sha256(raw).hexdigest()


def parse_invitation(raw):
    """(chainId, genesisHash, seeds, {file: text}) of a pocol-network-v2 invitation, or ValueError. Seeds are IP:PORT
    (what pocold --seed takes); the chain ID and genesis hash are devnet.json's."""
    if len(raw) > INVITATION_MAX:
        raise ValueError('The invitation exceeds 1 MiB')
    value = json.loads(raw)
    if not isinstance(value, dict) or value.get('schema') != 'pocol-network-v2':
        raise ValueError('Not a pocol-network-v2 invitation')
    files, seeds = value.get('files'), value.get('seeds')
    if not isinstance(files, dict) or any(not isinstance(files.get(name), str) for name in FILES):
        raise ValueError('The invitation does not carry ' + ', '.join(FILES))
    if not isinstance(seeds, list) or not 1 <= len(seeds) <= SEEDS_MAX or len(set(seeds)) != len(seeds):
        raise ValueError('The invitation needs 1 to 16 distinct seeds')
    for seed in seeds:
        host, _, port = seed.rpartition(':') if isinstance(seed, str) else ('', '', '')
        try:
            address = ipaddress.ip_address(host.strip('[]'))
        except ValueError:
            raise ValueError(f'Seed {seed!r}: expected IP:PORT') from None
        if not re.fullmatch(r'[1-9][0-9]{0,4}', port) or int(port) > 65535 or address.is_unspecified or address.is_multicast:
            raise ValueError(f'Seed {seed!r}: not a usable address')
    declared = json.loads(files['devnet.json'])
    chain, genesis = declared.get('chainId') if isinstance(declared, dict) else None, declared.get('genesisHash') if isinstance(declared, dict) else None
    if type(chain) is not int or chain <= 0 or not isinstance(genesis, str) or not re.fullmatch(r'0x[0-9a-f]{64}', genesis):
        raise ValueError('devnet.json does not name a chain ID and a lower-case genesis hash')
    if not re.fullmatch(r'(0x)?[0-9a-fA-F]+\s*', files['genesis.spec.hex']):
        raise ValueError('genesis.spec.hex is not hex')
    return chain, genesis, list(seeds), {name: files[name] for name in FILES}


def exec_args(seeds, listen=None, pocold=BASE / 'pocold'):
    """The node's command line: no miner, RPC on loopback only, the invitation's seeds."""
    args = [str(pocold), 'run', '--network', str(NETWORK_DIR), '--data', str(STATE / 'data'), '--name', 'explorer-observer',
            '--rpc', RPC]
    if listen:
        args += ['--listen', listen]
    for seed in seeds:
        args += ['--seed', seed]
    return args


def check_args(args):
    """The guard against an edit of this file: no miner identity, RPC 127.0.0.1:8545 and nothing else."""
    if any(a in ('--miner', '--miner-key-file') or a.startswith('--miner') for a in args):
        raise RuntimeError('The observer node must not mine: no --miner or --miner-key-file')
    if args.count('--rpc') != 1 or args[args.index('--rpc') + 1] != RPC:
        raise RuntimeError('The observer node RPC must be 127.0.0.1:8545 only')


def unit_text(seeds, listen=None):
    args = exec_args(seeds, listen)
    check_args(args)
    return f'''[Unit]
Description=PoCol observer node for the explorer gateway (non-mining, no key; RPC on {RPC} only)
After=network-online.target
Wants=network-online.target
StartLimitIntervalSec=300
StartLimitBurst=5

[Service]
Type=exec
DynamicUser=yes
UMask=0077
StateDirectory=pocol-observer
StateDirectoryMode=0700
ExecStart={' '.join(args)}
Restart=on-failure
RestartSec=10
TimeoutStopSec=30
KillMode=control-group
NoNewPrivileges=yes
PrivateTmp=yes
PrivateDevices=yes
ProtectHome=yes
ProtectSystem=strict
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectControlGroups=yes
RestrictSUIDSGID=yes
RestrictRealtime=yes
LockPersonality=yes
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
TasksMax=256
LimitNOFILE=4096
# The standalone node's sizing (deploy/standalone/install-linux.sh): the A13 model at the v3 profile fits the 6 GiB
# budget (POCOL_MEMORY_BUDGET_BYTES, default node::MEM_BUDGET); the cap adds 1 GiB the budget does not count. A host
# with at least 8 GiB of RAM.
MemoryMax=7G
MemorySwapMax=0

[Install]
WantedBy=multi-user.target
'''


def rpc_identity(timeout=5):
    """(chainId, genesisHash, miner) of the node on 127.0.0.1:8545 (pocol_getParams and pocol_status)."""
    def call(method):
        conn = http.client.HTTPConnection('127.0.0.1', 8545, timeout=timeout)
        try:
            conn.request('POST', '/', json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': method, 'params': []}),
                         {'Content-Type': 'application/json'})
            reply = conn.getresponse()
            answer = json.loads(reply.read(1 << 22))
            if reply.status != 200 or 'result' not in answer:
                raise RuntimeError(f'{method} failed')
            return answer['result']
        finally:
            conn.close()
    params, status = call('pocol_getParams'), call('pocol_status')
    return params.get('chainId'), str(params.get('genesisHash', '')).lower(), status.get('miner')


def wait_identity(chain, genesis, seconds):
    deadline, last = time.monotonic() + seconds, None
    while time.monotonic() < deadline:
        try:
            found = rpc_identity()
            if found[0] == chain and found[1] == genesis:
                if found[2]:
                    raise RuntimeError('The observer node reports a miner identity; it must not mine')
                return
            last = RuntimeError(f'The node on {RPC} names chain {found[0]} genesis {found[1]}, not the invitation\'s')
        except (OSError, ValueError, http.client.HTTPException) as error:
            last = error
        time.sleep(2)
    raise RuntimeError(f'The observer node did not answer for the invitation\'s network within {seconds} s: {last}')


def atomic_write(path, raw, mode=0o644):
    fd, name = tempfile.mkstemp(prefix='.observer-', dir=path.parent)
    try:
        with os.fdopen(fd, 'wb') as stream:
            stream.write(raw); stream.flush(); os.fsync(stream.fileno())
        os.chmod(name, mode); os.replace(name, path)
    finally:
        Path(name).unlink(missing_ok=True)


def make_directory(path):
    created, folder = [], path
    while not folder.exists():
        created.append(folder); folder = folder.parent
    for folder in reversed(created):
        folder.mkdir(mode=0o755); os.chmod(folder, 0o755)


USAGE = ('Usage: sudo python3 deploy/explorer/install_observer_node.py --invitation FILE --pocold FILE --pocold-sha256 HEX '
         '[--invitation-sha256 HEX] [--listen IP:PORT] [--wait SECONDS] [--check]')


def usage(argv):
    options, rest = {}, list(argv)
    while rest:
        flag = rest.pop(0)
        if flag in options:
            raise RuntimeError(USAGE)
        if flag == '--check':
            options[flag] = True
        elif flag in ('--invitation', '--pocold') and rest and not rest[0].startswith('--'):
            options[flag] = Path(rest.pop(0))
        elif flag in ('--pocold-sha256', '--invitation-sha256') and rest and re.fullmatch(r'[0-9a-f]{64}', rest[0]):
            options[flag] = rest.pop(0)
        elif flag == '--listen' and rest and re.fullmatch(r'[0-9.]{7,15}:[1-9][0-9]{0,4}', rest[0]):
            options[flag] = rest.pop(0)
        elif flag == '--wait' and rest and re.fullmatch(r'[0-9]{1,4}', rest[0]) and WAIT_RANGE[0] <= int(rest[0]) <= WAIT_RANGE[1]:
            options[flag] = int(rest.pop(0))
        else:
            raise RuntimeError(USAGE)
    if not all(k in options for k in ('--invitation', '--pocold', '--pocold-sha256')):
        raise RuntimeError(USAGE)
    return options


def main(argv=None):
    options = usage(sys.argv[1:] if argv is None else argv)
    if os.geteuid() != 0:
        raise RuntimeError('Run with sudo')
    if not shutil.which('systemctl'):
        raise RuntimeError('systemctl must be installed')
    for p in (BASE, CONFIG, NETWORK_DIR, INVITATION, UNIT):
        if p.is_symlink():
            raise RuntimeError(f'Unexpected symbolic link {p}; no changes made')
    binary = options['--pocold'].read_bytes()
    if sha256(binary) != options['--pocold-sha256']:
        raise RuntimeError('--pocold does not have the --pocold-sha256 digest; no changes made')
    raw = options['--invitation'].read_bytes()
    if '--invitation-sha256' in options and sha256(raw) != options['--invitation-sha256']:
        raise RuntimeError('--invitation does not have the --invitation-sha256 digest; no changes made')
    chain, genesis, seeds, files = parse_invitation(raw)
    if GATEWAY_NETWORK.is_file():
        gateway_network = json.loads(GATEWAY_NETWORK.read_bytes()[:4097])
        if gateway_network.get('chainId') != chain or str(gateway_network.get('genesisHash', '')).lower() != genesis:
            raise RuntimeError(f'{GATEWAY_NETWORK} names another network than the invitation; no changes made')
    if BASE.exists() and not (BASE / MARKER).is_file():
        raise RuntimeError(f'Unrecognized {BASE}; no changes made')
    if INVITATION.is_file() and INVITATION.read_bytes() != raw:
        raise RuntimeError(f'{INVITATION} holds another invitation; a network change is not an update (review it); no changes made')
    unit = unit_text(seeds, options.get('--listen'))
    if UNIT.exists() and not (BASE / MARKER).is_file():
        raise RuntimeError(f'{UNIT} exists without this installer\'s marker; review it first; no changes made')
    old_active = command('systemctl', 'is-active', '--quiet', SERVICE, check=False).returncode == 0
    old_enabled = command('systemctl', 'is-enabled', '--quiet', SERVICE, check=False).returncode == 0
    if not old_active:
        with socket.socket() as sock:
            sock.bind(('127.0.0.1', 8545))  # the port is free (no other node answers the gateway)
    if options.get('--check'):
        print(f'CHECK PASSED: pocold {options["--pocold-sha256"][:16]}..., invitation chain {chain} genesis {genesis} '
              f'with {len(seeds)} seeds, RPC {RPC} (no miner), {"inbound " + options["--listen"] if options.get("--listen") else "outbound P2P only"}.')
        print(unit, end='')
        return
    BACKUPS.mkdir(parents=True, exist_ok=True, mode=0o700)
    backup = BACKUPS / (time.strftime('%Y%m%d-%H%M%S') + '-' + str(os.getpid()))
    backup.mkdir(mode=0o700)
    paths = [BASE / 'pocold', BASE / MARKER, INVITATION, UNIT] + [NETWORK_DIR / name for name in FILES]
    previous = {p: p.read_bytes() if p.is_file() else None for p in paths}
    for i, p in enumerate(paths):
        if previous[p] is not None:
            (backup / f'file-{i}.before').write_bytes(previous[p])
    (backup / 'state.json').write_text(json.dumps({'active': old_active, 'enabled': old_enabled, 'paths': list(map(str, paths))}))
    created = [p for p in (BASE, CONFIG, NETWORK_DIR) if not p.exists()]
    try:
        for folder in (BASE, NETWORK_DIR):
            make_directory(folder)
        atomic_write(BASE / 'pocold', binary, 0o755)
        atomic_write(BASE / MARKER, b'PoCol observer node v1\n')
        atomic_write(INVITATION, raw)
        for name in FILES:
            atomic_write(NETWORK_DIR / name, files[name].encode())
        atomic_write(UNIT, unit.encode())
        command('systemctl', 'daemon-reload')
        command('systemctl', 'restart', SERVICE)
        wait_identity(chain, genesis, options.get('--wait', WAIT))
        command('systemctl', 'enable', SERVICE)
    except BaseException as exc:
        failures = []
        def attempt(label, action):
            try: action()
            except Exception as error: failures.append(f'{label}: {error}')
        attempt('stop the observer node', lambda: command('systemctl', 'stop', SERVICE))
        for path, old in previous.items():
            attempt('restore ' + str(path), lambda p=path, b=old: p.unlink(missing_ok=True) if b is None else atomic_write(p, b, 0o755 if p.name == 'pocold' else 0o644))
        for folder in reversed(created):
            attempt('remove ' + str(folder), lambda f=folder: f.rmdir() if f.exists() else None)
        attempt('reload the restored unit', lambda: command('systemctl', 'daemon-reload'))
        if not old_enabled:
            attempt('restore the disabled boot state', lambda: command('systemctl', 'disable', SERVICE, check=False))
        if old_active:
            attempt('restart the previous observer node', lambda: command('systemctl', 'restart', SERVICE))
        if failures:
            raise RuntimeError(f'Installation failed: {exc}; ROLLBACK INCOMPLETE: {"; ".join(failures)}; backup: {backup}') from exc
        raise
    print(f'INSTALLED: {SERVICE}, a non-mining node of chain {chain} (genesis {genesis}) on {RPC}, following {len(seeds)} seeds.')
    print('It syncs in the background: compare its head with the network (pocol_status) before announcing the explorer.')
    print('Backup: ' + str(backup))


if __name__ == '__main__':
    try:
        main()
    except Exception as exc:
        print('INSTALL FAILED: ' + str(exc), file=sys.stderr)
        sys.exit(1)
