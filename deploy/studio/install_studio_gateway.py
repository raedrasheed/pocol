#!/usr/bin/env python3
"""Transactional installer for the separate Studio gateway; no node data is edited."""
import hashlib
import http.client
import json
import os
from pathlib import Path
import shutil
import socket
import ssl
import subprocess
import sys
import tempfile
import time

BASE = Path('/opt/pocol-studio-gateway')
SERVICE = 'pocol-studio-gateway.service'
UNIT = Path('/etc/systemd/system') / SERVICE
CADDYFILE = Path('/etc/caddy/Caddyfile')
BACKUPS = Path('/var/backups/pocol-studio-gateway')
MARKER = '# BEGIN POCOL STUDIO GATEWAY v1'
# Every per-client bucket of the gateway (read, write and chunk) keys on this header, so the only route to the relay
# port must overwrite it with the connecting address; the relay itself listens on loopback only (--bind 127.0.0.1).
CLIENT_IP_HEADER = 'header_up X-Pocol-Client-IP {http.request.remote.host}'
RELAY = '127.0.0.1:8089'
ROUTE = '''
    # BEGIN POCOL STUDIO GATEWAY v1
    @pocol_studio {
        path /studio/rpc /studio/health
        method GET POST OPTIONS
    }
    handle @pocol_studio {
        reverse_proxy 127.0.0.1:8089 {
            header_up X-Pocol-Client-IP {http.request.remote.host}
            transport http {
                dial_timeout 3s
                response_header_timeout 25s
            }
        }
    }
    # END POCOL STUDIO GATEWAY v1
'''
UNIT_TEXT = '''[Unit]
Description=PoCol Studio bounded RPC gateway
After=network.target

[Service]
Type=simple
DynamicUser=yes
ExecStart=/usr/bin/python3 /opt/pocol-studio-gateway/pocol_studio_gateway.py --bind 127.0.0.1 --port 8089 --rpc-port 8545
WorkingDirectory=/opt/pocol-studio-gateway
Restart=on-failure
RestartSec=3
NoNewPrivileges=yes
PrivateTmp=yes
ProtectSystem=strict
ProtectHome=yes
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectControlGroups=yes
RestrictSUIDSGID=yes
RestrictRealtime=yes
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
IPAddressDeny=any
IPAddressAllow=localhost
MemoryMax=256M
TasksMax=32
UMask=0077

[Install]
WantedBy=multi-user.target
'''
# The unit text earlier installers wrote (no explicit loopback arguments, MemoryMax=128M before the chunk cache): an
# update replaces exactly that text; any other edit is refused for review.
PRIOR_UNIT_TEXTS = (UNIT_TEXT.replace(' --bind 127.0.0.1 --port 8089 --rpc-port 8545', '').replace('MemoryMax=256M', 'MemoryMax=128M'),)


def check_forced_loopback():
    """The route this installer writes sets the client-IP header and proxies only to the loopback relay, and the unit
    starts the relay on loopback; refused otherwise (a guard against an edit of this file)."""
    if ROUTE.count(CLIENT_IP_HEADER) != 1 or ROUTE.count('reverse_proxy ' + RELAY + ' {') != 1 or ROUTE.count('8089') != 1:
        raise RuntimeError('The Studio route must set X-Pocol-Client-IP and proxy only to the loopback relay')
    if UNIT_TEXT.count(' --bind 127.0.0.1 ') != 1 or 'IPAddressAllow=localhost' not in UNIT_TEXT:
        raise RuntimeError('The Studio service must bind the relay to 127.0.0.1')


def same_identity(value, chain_id, genesis):
    """An answer's chainId is the integer chain_id (not a bool or a string) and its genesisHash is genesis in any
    letter case."""
    if not isinstance(value, dict): return False
    chain, found = value.get('chainId'), value.get('genesisHash')
    return (type(chain) is int and chain == chain_id and isinstance(found, str) and len(found) == 66
            and found[:2] == '0x' and all(c in '0123456789abcdefABCDEF' for c in found[2:]) and found.lower() == genesis.lower())


def command(*args, check=True):
    return subprocess.run(args, check=check, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=45)


def patch_caddy(text):
    """Add one bounded route, preserving the Explorer's explicit 16 KiB guard. The relay port may be reached only
    through this route, which overwrites X-Pocol-Client-IP."""
    check_forced_loopback()
    if MARKER in text:
        if text.count(ROUTE) != 1: raise RuntimeError('The existing Studio Caddy route has custom edits; review it first')
        if '8089' in text.replace(ROUTE, ''): raise RuntimeError('Another Caddy route reaches the Studio relay port; review it first')
        return text
    if '8089' in text: raise RuntimeError('Another Caddy route reaches the Studio relay port; review it first')
    host = 'gateway.pocol.net {'
    if text.count(host) != 1: raise RuntimeError('Expected one dedicated gateway.pocol.net Caddy block')
    if text.count('max_size 16384') != 1: raise RuntimeError('Unexpected Caddy request-body configuration')
    if 'Request body exceeds 16 KiB' not in text or 'int({http.request.header.Content-Length}) > 16384' not in text:
        raise RuntimeError('The existing Explorer 16 KiB request guard was not found')
    if 'reverse_proxy 127.0.0.1:8088' not in text:
        raise RuntimeError('The existing Explorer loopback route was not found')
    # The Studio storage chunks need up to ~52 KiB JSON. Explorer's route keeps its own guard.
    return text.replace('max_size 16384', 'max_size 131072', 1).replace(host, host + ROUTE, 1)


def atomic_write(path, raw, mode=0o644):
    fd, name = tempfile.mkstemp(prefix='.studio-', dir=path.parent)
    try:
        with os.fdopen(fd, 'wb') as stream:
            stream.write(raw); stream.flush(); os.fsync(stream.fileno())
        os.chmod(name, mode); os.replace(name, path)
    finally: Path(name).unlink(missing_ok=True)


def local_json(port, path, payload=None):
    conn = http.client.HTTPConnection('127.0.0.1', port, timeout=20)
    try:
        conn.request('GET' if payload is None else 'POST', path,
                     None if payload is None else json.dumps(payload), {'Content-Type': 'application/json'})
        response = conn.getresponse(); raw = response.read(2*1024*1024+1)
        if response.status != 200 or len(raw) > 2*1024*1024: raise RuntimeError('Unexpected local health response')
        return json.loads(raw)
    finally: conn.close()


def verify_health():
    value = local_json(8089, '/studio/health')
    if value.get('ready') is not True or not same_identity(value, 1599846715, '0xbe137bfdb4fdf332f2e1500ee1a67e0b22ce6bdf57efa46fb4ea7b0debe509ca'):
        raise RuntimeError('Studio gateway identity verification failed')


def tls_json(path, payload=None):
    """Connect to local Caddy using the public SNI and trusted certificate checks.

    This verifies the exact HTTPS routes without relying on hairpin routing/DNS.
    It does not prove reachability from the user's machine or hosting provider.
    """
    hostname = 'gateway.pocol.net'
    context = ssl.create_default_context()
    context.minimum_version = ssl.TLSVersion.TLSv1_2
    conn = http.client.HTTPSConnection(hostname, 443, timeout=12, context=context)
    try:
        conn.sock = context.wrap_socket(socket.create_connection(('127.0.0.1', 443), timeout=12),
                                        server_hostname=hostname)
        conn.request('GET' if payload is None else 'POST', path,
                     None if payload is None else json.dumps(payload),
                     {'Content-Type': 'application/json', 'Connection': 'close'})
        response = conn.getresponse(); raw = response.read(2*1024*1024+1)
        if response.status != 200 or len(raw) > 2*1024*1024:
            raise RuntimeError('Unexpected HTTPS route response: ' + path)
        value = json.loads(raw)
        if not isinstance(value, dict): raise RuntimeError('Invalid HTTPS route response: ' + path)
        return value
    finally: conn.close()


def verify_https_routes():
    expected_genesis = '0xbe137bfdb4fdf332f2e1500ee1a67e0b22ce6bdf57efa46fb4ea7b0debe509ca'
    health = tls_json('/studio/health')
    if (health.get('service') != 'pocol-studio-gateway' or health.get('ready') is not True
            or not same_identity(health, 1599846715, expected_genesis)):
        raise RuntimeError('Studio HTTPS route or network identity verification failed')
    answer = tls_json('/rpc', {'jsonrpc': '2.0', 'id': 1, 'method': 'pocol_getParams', 'params': []})
    params = answer.get('result')
    if (answer.get('jsonrpc') != '2.0' or answer.get('id') != 1 or 'error' in answer
            or not same_identity(params, 1599846715, expected_genesis)):
        raise RuntimeError('Existing Explorer HTTPS route or network identity verification failed')


def rollback_install(original, previous, base_existed, old_active, old_enabled):
    """Attempt every recovery step, even if a service command fails or times out."""
    failures = []

    def attempt(label, action):
        try:
            action()
        except Exception as exc:
            failures.append(label + ': ' + str(exc))

    attempt('restore Caddyfile', lambda: atomic_write(CADDYFILE, original))
    attempt('reload previous Caddy configuration', lambda: command('systemctl', 'reload', 'caddy'))
    if UNIT.exists():
        attempt('stop Studio service', lambda: command('systemctl', 'stop', SERVICE))
        if not old_enabled:
            attempt('restore disabled Studio boot state', lambda: command('systemctl', 'disable', SERVICE))
    for path, old in previous.items():
        attempt('restore ' + str(path), lambda p=path, raw=old:
                p.unlink(missing_ok=True) if raw is None else atomic_write(p, raw))
    if not base_existed:
        attempt('remove new Studio directory', lambda: BASE.rmdir() if BASE.exists() else None)
    attempt('reload restored service definition', lambda: command('systemctl', 'daemon-reload'))
    if old_enabled:
        attempt('restore enabled Studio boot state', lambda: command('systemctl', 'enable', SERVICE))
    if old_active:
        attempt('restart previous Studio service', lambda: command('systemctl', 'restart', SERVICE))
    return failures


def main():
    if sys.argv[1:] not in ([], ['--check']): raise RuntimeError('Usage: sudo bash Install-PoCol-Studio-Gateway.sh [--check]')
    if os.geteuid() != 0: raise RuntimeError('Run with sudo')
    for utility in ('caddy', 'systemctl'): 
        if not shutil.which(utility): raise RuntimeError(f'{utility} must already be installed')
    source = Path(__file__).with_name('pocol_studio_gateway.py')
    raw = source.read_bytes(); compile(raw, source.name, 'exec')
    for p in (BASE, UNIT, CADDYFILE):
        if p.is_symlink(): raise RuntimeError('Unexpected symlink; no changes made')
    if not CADDYFILE.is_file(): raise RuntimeError('Install the existing PoCol HTTPS gateway first')
    if command('systemctl', 'is-active', '--quiet', 'caddy', check=False).returncode:
        raise RuntimeError('The existing Caddy HTTPS gateway must be running')
    if BASE.exists() and not (BASE / 'managed-by-studio-v1').is_file():
        raise RuntimeError('Unrecognized Studio directory; no changes made')
    if UNIT.exists() and UNIT.read_text() not in (UNIT_TEXT, *PRIOR_UNIT_TEXTS):
        raise RuntimeError('Studio service has custom edits; review before updating')
    from pocol_studio_gateway import Gateway
    Gateway().verify_network(force=True)
    original = CADDYFILE.read_bytes(); candidate = patch_caddy(original.decode())
    old_active = command('systemctl', 'is-active', '--quiet', SERVICE, check=False).returncode == 0
    old_enabled = command('systemctl', 'is-enabled', '--quiet', SERVICE, check=False).returncode == 0
    if not old_active:
        with socket.socket() as sock: sock.bind(('127.0.0.1', 8089))
    with tempfile.TemporaryDirectory(prefix='pocol-studio-check-') as tmp:
        testfile = Path(tmp) / 'Caddyfile'; testfile.write_text(candidate)
        result = command('caddy', 'validate', '--config', str(testfile), '--adapter', 'caddyfile', check=False)
        if result.returncode: raise RuntimeError('Caddy candidate validation failed: ' + result.stderr.decode()[-1500:])
    if sys.argv[1:]:
        print('CHECK PASSED: network identity, separate loopback port, existing HTTPS route, and candidate Caddy configuration.')
        return
    BACKUPS.mkdir(parents=True, exist_ok=True, mode=0o700)
    backup = BACKUPS / (time.strftime('%Y%m%d-%H%M%S') + '-' + str(os.getpid()))
    backup.mkdir(mode=0o700)
    (backup / 'Caddyfile.before').write_bytes(original)
    paths = [BASE / source.name, BASE / 'managed-by-studio-v1', UNIT]
    previous = {p: p.read_bytes() if p.is_file() else None for p in paths}
    for i, p in enumerate(paths):
        if previous[p] is not None: (backup / f'file-{i}.before').write_bytes(previous[p])
    (backup / 'state.json').write_text(json.dumps({'active': old_active, 'enabled': old_enabled, 'paths': list(map(str, paths))}))
    base_existed = BASE.exists()
    BASE.mkdir(parents=True, exist_ok=True)
    try:
        atomic_write(BASE / source.name, raw)
        atomic_write(BASE / 'managed-by-studio-v1', b'PoCol Studio gateway v1\n')
        atomic_write(UNIT, UNIT_TEXT.encode())
        command('systemctl', 'daemon-reload'); command('systemctl', 'restart', SERVICE)
        last = None
        for _ in range(12):
            try: verify_health(); last = None; break
            except (OSError, RuntimeError, ValueError, http.client.HTTPException) as exc: last = exc; time.sleep(.5)
        if last: raise last
        atomic_write(CADDYFILE, candidate.encode())
        command('systemctl', 'reload', 'caddy')
        verify_https_routes()
        command('systemctl', 'enable', SERVICE)
    except BaseException as exc:
        failures = rollback_install(original, previous, base_existed, old_active, old_enabled)
        if failures:
            raise RuntimeError('Installation failed: ' + str(exc) + '; ROLLBACK INCOMPLETE: '
                               + '; '.join(failures) + '; recovery backup: ' + str(backup)) from exc
        raise
    print('INSTALLED: separate PoCol Studio relay; existing node and Explorer services were preserved.')
    print('PASS: both Studio and existing Explorer HTTPS routes passed trusted certificate and network-identity verification.')
    print('Endpoint: https://gateway.pocol.net/studio/rpc')
    print('Verify external reachability from Windows/cPanel: curl --fail --max-time 20 https://gateway.pocol.net/studio/health')
    print('Backup: ' + str(backup))


if __name__ == '__main__':
    try: main()
    except Exception as exc: print('INSTALL FAILED: ' + str(exc), file=sys.stderr); sys.exit(1)
