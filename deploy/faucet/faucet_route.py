#!/usr/bin/env python3
"""The fee faucet's public route on explorer.pocol.net, and its transactional installer.

https://explorer.pocol.net/faucet/claim (POST) and /faucet/status (GET), with their CORS preflights, are proxied to
the faucet on 127.0.0.1:8547 with the /faucet prefix removed. The route is a marked part of the explorer's managed
Caddy block (deploy/explorer/explorer_site.py), as the Studio route is (deploy/studio/install_studio_gateway.py):
- it is inserted right before the block's single `    @rpc path /rpc` line (Studio inserts its route right after the
  host line), so installing it and the Studio route in either order gives the same file;
- an explorer update (install_explorer_gateway.py, through explorer_site.plan_caddyfile) keeps it, and refuses an
  edited or duplicated copy;
- it overwrites X-Forwarded-For with the connecting address, the key of the faucet's per-client limit (trustProxy);
- it does not change the site's request-body size (the faucet takes at most 512 bytes).

    sudo python3 deploy/faucet/faucet_route.py --check
    sudo python3 deploy/faucet/faucet_route.py

Run it from a checkout, on the explorer gateway server, once the faucet runs there (README.md). It needs the explorer
block installed, a running Caddy and /etc/pocol-faucet/config.json with listen 127.0.0.1, port 8547, trustProxy true
and an https allowOrigin. --check runs every preflight (the config, the faucet on loopback, the managed block, `caddy
validate` of the candidate) and changes nothing. An install backs up the Caddyfile under /var/backups/pocol-faucet-route,
writes the candidate, reloads Caddy and checks the route over HTTPS with the public name and the default trust store;
on any failure it restores the previous Caddyfile and reloads it. It never sends a valid claim: its only POST carries
an address the faucet refuses before any other check (PROBE).
"""
import http.client
import json
import os
from pathlib import Path
import re
import shutil
import socket
import ssl
import subprocess
import sys
import tempfile
import time

HERE = Path(__file__).resolve().parent
CADDYFILE = Path('/etc/caddy/Caddyfile')
CONFIG = Path('/etc/pocol-faucet/config.json')
BACKUPS = Path('/var/backups/pocol-faucet-route')
HOST = 'explorer.pocol.net'
PORT = 8547
MARKER = '# BEGIN POCOL FAUCET v1'
END = '# END POCOL FAUCET v1'
# The faucet's per-client limit keys on the last X-Forwarded-For entry (trustProxy), so the only route to the faucet
# port must overwrite the header with the connecting address, whatever a client sent or a trusted_proxies setting kept.
CLIENT_IP_HEADER = 'header_up X-Forwarded-For {http.request.remote.host}'
BACKEND = f'127.0.0.1:{PORT}'
# The explorer block's /rpc line, which occurs once in it; the route goes right before it.
ANCHOR = '    @rpc path /rpc\n'
# response_header_timeout 45s: a claim waits for the signer (pocol_faucet.py runs it with a 30 s limit) and eight
# node calls, each well under a second on a healthy local node (each has a 10 s limit). 45 s leaves room for those
# behind a slow signer; Studio's 25 s would cut off a claim whose signer is merely slow. A longer wait still gets
# Caddy's own error answer while the faucet may go on and send, so a page treats that as an unknown outcome.
ROUTE = '''    # BEGIN POCOL FAUCET v1
    @pocol_faucet {
        path /faucet/claim /faucet/status
        method GET POST OPTIONS
    }
    handle @pocol_faucet {
        uri strip_prefix /faucet
        reverse_proxy 127.0.0.1:8547 {
            header_up X-Forwarded-For {http.request.remote.host}
            transport http {
                dial_timeout 3s
                response_header_timeout 45s
            }
        }
    }
    # END POCOL FAUCET v1
'''
# The installer's only claim: the faucet refuses it with 400 invalid_address before the ledger, the node or the signer.
PROBE = {'address': '0x'}
ORIGIN = re.compile(r'https://[a-z0-9]([a-z0-9.-]{0,251}[a-z0-9])?')


def check_forced_loopback():
    """The route sets the client header once, proxies only to the loopback faucet and strips the prefix (a guard
    against an edit of this file)."""
    if (ROUTE.count(CLIENT_IP_HEADER) != 1 or ROUTE.count('reverse_proxy ' + BACKEND + ' {') != 1 or ROUTE.count(str(PORT)) != 1
            or ROUTE.count('reverse_proxy') != 1 or ROUTE.count('        uri strip_prefix /faucet\n') != 1
            or not ROUTE.startswith('    ' + MARKER + '\n') or not ROUTE.endswith('    ' + END + '\n')):
        raise RuntimeError('The faucet route must set X-Forwarded-For and proxy only to the loopback faucet')


def patch_caddy(text):
    """`text` with the faucet route in the explorer block (unchanged if it is there). Refused: an edited or duplicated
    route, another route to the faucet port, or no single explorer block with its /rpc route."""
    check_forced_loopback()
    if MARKER in text or END in text:
        if text.count(ROUTE) != 1 or text.count(MARKER) != 1 or text.count(END) != 1:
            raise RuntimeError('The existing faucet Caddy route has custom edits; review it first')
        if str(PORT) in text.replace(ROUTE, ''): raise RuntimeError('Another Caddy route reaches the faucet port; review it first')
        return text
    if str(PORT) in text: raise RuntimeError('Another Caddy route reaches the faucet port; review it first')
    host = HOST + ' {'
    if text.count(host) != 1: raise RuntimeError('Expected one dedicated explorer.pocol.net Caddy block')
    if text.count(ANCHOR) != 1 or text.index(ANCHOR) < text.index(host):
        raise RuntimeError("The explorer block's /rpc route was not found")
    if 'reverse_proxy 127.0.0.1:8088' not in text: raise RuntimeError('The existing Explorer loopback route was not found')
    return text.replace(ANCHOR, ROUTE + ANCHOR, 1)


def check_managed(candidate):
    """The candidate is exactly what an explorer update would keep: the managed explorer block (no custom edits), its
    Studio route if present, and this route."""
    sys.path.insert(0, str(HERE.parent / 'studio'))
    sys.path.insert(0, str(HERE.parent / 'explorer'))
    import explorer_site
    import install_studio_gateway as studio
    roots = explorer_site.SITE_ROOT.findall(candidate)
    if len(roots) != 1: raise RuntimeError('Expected the managed explorer Caddy block with one site root')
    if explorer_site.plan_caddyfile(candidate, roots[0], studio, sys.modules[__name__])[0] != candidate:
        raise RuntimeError('The explorer Caddy block would change on its next update; review it first')


def check_config(cfg):
    """(chain ID, allowed origin) of the faucet config the route needs; refused otherwise."""
    if not isinstance(cfg, dict): raise RuntimeError(f'{CONFIG} is not a JSON object')
    if cfg.get('listen', '127.0.0.1') != '127.0.0.1' or int(cfg.get('port', PORT)) != PORT:
        raise RuntimeError(f'The faucet must listen on {BACKEND} (listen, port in {CONFIG})')
    if cfg.get('trustProxy') is not True:
        raise RuntimeError(f'Set "trustProxy": true in {CONFIG}: behind this route every client would otherwise share one '
                           'per-client counter (the route sets X-Forwarded-For)')
    origin = cfg.get('allowOrigin')
    if not isinstance(origin, str) or not ORIGIN.fullmatch(origin):
        raise RuntimeError(f'Set "allowOrigin" in {CONFIG} to the page origin, such as "https://pocol.net"')
    chain = cfg.get('chainId')
    if type(chain) is not int: raise RuntimeError(f'{CONFIG} needs an integer chainId')
    return chain, origin


def command(*args, check=True):
    return subprocess.run(args, check=check, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=45)


def atomic_write(path, raw, mode=0o644):
    fd, name = tempfile.mkstemp(prefix='.faucet-route-', dir=path.parent)
    try:
        with os.fdopen(fd, 'wb') as stream:
            stream.write(raw); stream.flush(); os.fsync(stream.fileno())
        os.chmod(name, mode); os.replace(name, path)
    finally: Path(name).unlink(missing_ok=True)


def http_json(conn, method, path, payload=None, headers=None):
    body = None if payload is None else json.dumps(payload).encode()
    conn.request(method, path, body, {'Content-Type': 'application/json', 'Connection': 'close', **(headers or {})})
    response = conn.getresponse(); raw = response.read(64 * 1024 + 1)
    return response.status, response, raw


def local(method, path, payload=None, headers=None):
    conn = http.client.HTTPConnection('127.0.0.1', PORT, timeout=20)
    try: return http_json(conn, method, path, payload, headers)
    finally: conn.close()


def tls(method, path, payload=None, headers=None):
    """One request to local Caddy with the public SNI and trusted certificate checks (no hairpin routing or DNS), as
    install_explorer_gateway.tls does. It does not prove reachability from outside."""
    context = ssl.create_default_context()
    context.minimum_version = ssl.TLSVersion.TLSv1_2
    conn = http.client.HTTPSConnection(HOST, 443, timeout=12, context=context)
    try:
        conn.sock = context.wrap_socket(socket.create_connection(('127.0.0.1', 443), timeout=12), server_hostname=HOST)
        return http_json(conn, method, path, payload, headers)
    finally: conn.close()


def check_answers(call, prefix, chain, origin):
    """The faucet's answers through `call` (local or tls) under `prefix`: /status names the chain, the preflight and the
    probe claim carry the allowed origin, the probe is refused as invalid_address."""
    status, response, raw = call('GET', prefix + '/status')
    value = json.loads(raw) if status == 200 else None
    if not isinstance(value, dict) or value.get('type') != 'status' or type(value.get('chainId')) is not int or value['chainId'] != chain:
        raise RuntimeError(f'{prefix}/status did not answer for chain {chain}')
    if response.getheader('Access-Control-Allow-Origin') != origin: raise RuntimeError(f'{prefix}/status does not allow {origin}')
    status, response, _ = call('OPTIONS', prefix + '/claim', None, {'Origin': origin, 'Access-Control-Request-Method': 'POST',
                                                                     'Access-Control-Request-Headers': 'content-type'})
    if status != 204 or response.getheader('Access-Control-Allow-Origin') != origin or response.getheader('Access-Control-Max-Age') != '600':
        raise RuntimeError(f'The {prefix}/claim preflight does not allow {origin}')
    status, response, raw = call('POST', prefix + '/claim', PROBE, {'Origin': origin})
    value = json.loads(raw) if status == 400 else None
    if not isinstance(value, dict) or value.get('code') != 'invalid_address' or response.getheader('Access-Control-Allow-Origin') != origin:
        raise RuntimeError(f'{prefix}/claim did not refuse the probe address as invalid_address')


def verify_https(chain, origin):
    check_answers(tls, '/faucet', chain, origin)
    status, _, raw = tls('GET', '/health')
    if status != 200 or json.loads(raw).get('service') != 'pocol-explorer-gateway':
        raise RuntimeError('The explorer HTTPS route stopped answering')


def verify_https_with_retry(chain, origin, seconds=30):
    """Caddy applies a reload within moments: retry until `seconds` have passed."""
    deadline = time.monotonic() + seconds
    while True:
        try: return verify_https(chain, origin)
        except (OSError, RuntimeError, ValueError, http.client.HTTPException) as error:
            if time.monotonic() >= deadline: raise error
        time.sleep(2)


def main(argv=None):
    argv = sys.argv[1:] if argv is None else argv
    if argv not in ([], ['--check']): raise RuntimeError('Usage: sudo python3 deploy/faucet/faucet_route.py [--check]')
    check_forced_loopback()
    if os.geteuid() != 0: raise RuntimeError('Run with sudo')
    for utility in ('caddy', 'systemctl'):
        if not shutil.which(utility): raise RuntimeError(f'{utility} must already be installed')
    for p in (CADDYFILE, CONFIG):
        if p.is_symlink(): raise RuntimeError(f'Unexpected symbolic link {p}; no changes made')
    if not CADDYFILE.is_file(): raise RuntimeError('Install the explorer gateway first (its /etc/caddy/Caddyfile is missing)')
    if command('systemctl', 'is-active', '--quiet', 'caddy', check=False).returncode: raise RuntimeError('Caddy must be running')
    chain, origin = check_config(json.loads(CONFIG.read_text(encoding='utf-8')))
    check_answers(local, '', chain, origin)
    original = CADDYFILE.read_bytes()
    candidate = patch_caddy(original.decode())
    check_managed(candidate)
    with tempfile.TemporaryDirectory(prefix='pocol-faucet-route-check-') as tmp:
        testfile = Path(tmp) / 'Caddyfile'; testfile.write_text(candidate)
        result = command('caddy', 'validate', '--config', str(testfile), '--adapter', 'caddyfile', check=False)
        if result.returncode: raise RuntimeError('Caddy candidate validation failed: ' + result.stderr.decode()[-1500:])
    if argv:
        print(f'CHECK PASSED: the faucet config and loopback answers (chain {chain}, origin {origin}), the explorer block, '
              'and the candidate Caddy configuration' + (' (the route is already installed).' if candidate == original.decode() else '.'))
        return
    if candidate == original.decode():
        verify_https_with_retry(chain, origin)
        print('PASS: the faucet route was already installed and answers over HTTPS; nothing changed.')
        return
    BACKUPS.mkdir(parents=True, exist_ok=True, mode=0o700)
    backup = BACKUPS / (time.strftime('%Y%m%d-%H%M%S') + '-' + str(os.getpid()))
    backup.mkdir(mode=0o700)
    (backup / 'Caddyfile.before').write_bytes(original)
    try:
        atomic_write(CADDYFILE, candidate.encode())
        command('systemctl', 'reload', 'caddy')
        verify_https_with_retry(chain, origin)
    except BaseException as exc:
        failures = []
        for label, action in (('restore Caddyfile', lambda: atomic_write(CADDYFILE, original)),
                              ('reload previous Caddy configuration', lambda: command('systemctl', 'reload', 'caddy'))):
            try: action()
            except Exception as error: failures.append(label + ': ' + str(error))
        if failures:
            raise RuntimeError('Installation failed: ' + str(exc) + '; ROLLBACK INCOMPLETE: ' + '; '.join(failures)
                               + '; recovery backup: ' + str(backup)) from exc
        raise
    print(f'INSTALLED: https://{HOST}/faucet/claim and /faucet/status, proxied to the faucet on {BACKEND}.')
    print(f'PASS: /faucet/status (chain {chain}), the preflight and a refused probe claim for {origin}, and /health, over '
          'HTTPS with trusted certificates.')
    print(f'Verify from outside: curl --fail --max-time 20 https://{HOST}/faucet/status')
    print('Backup: ' + str(backup))


if __name__ == '__main__':
    try: main()
    except Exception as exc: print('INSTALL FAILED: ' + str(exc), file=sys.stderr); sys.exit(1)
