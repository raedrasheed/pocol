#!/usr/bin/env python3
"""Transactional installer of the explorer gateway host (Sites W8, owner decision 9): the read-only gateway service
(deploy/rewards/gateway.py with pocol_rewards.py, explorer_api.py, the transaction index pocol_transactions.py, the
holders index pocol_holders.py and the miner map's pocol_geoip.py) on 127.0.0.1:8088, the explorer snapshot's static
copy, and the explorer.pocol.net Caddy site that serves both (explorer_site.py). No node data, key or DNS record is
touched.

    D=$(mktemp -d) && python3 deploy/websites/tools/site_snapshot.py --out "$D" \
        && sudo python3 deploy/explorer/install_explorer_gateway.py --site "$D" --check
    sudo python3 deploy/explorer/install_explorer_gateway.py --site "$D" [--geoip-db geoip-country.bin]

The miner map (development/explorer/MINERS-MAP-HOLDERS.md §4): --geoip-db installs a GeoIP database built with
`python3 deploy/rewards/pocol_geoip.py build` from the operator's DB-IP "IP to Country Lite" CSV (checked record by
record first) as GEOIP_FILE; an update without the option keeps the installed one. Operator node reports need the
reporters' keys and the network salt in /etc/pocol-explorer-gateway/geo-reporters.json (root, 0600; never in the
repository): while that file exists the installer adds the unit drop-in GEO_DROPIN, which loads it as the systemd
credential `geo-reporters` (LoadCredential=; systemd 247 or later); without the file there is no drop-in and the report
route answers 503. LoadCredential= fails the service start when its file is missing, so remove the file only by
rerunning the installer after removing it (the drop-in then goes too).

The copy is built from the committed blobs into a new directory, so a failed build stops there and no older copy is
left to install. The installer also compares it with this checkout's explorer snapshot (its include list, checked
against its SHA256SUMS.txt) and refuses one that differs, unless --accept-site names that copy's site id (a copy meant
to differ, such as a bundle switched for a devnet).

Run it from a checkout of the release commit (it installs the gateway sources beside it). It needs Python 3.10+,
systemd, a running Caddy, the operator's /etc/pocol-explorer-gateway/network.json (deploy/rewards/README.md) and the
node's RPC on 127.0.0.1:8545. --check runs every preflight (the network file, the node's identity, the copy, the
free port, `caddy validate` of the candidate) and changes nothing. An install backs up what it replaces under
/var/backups/pocol-explorer-gateway, starts the service and checks it on loopback, reloads Caddy, then checks the
public routes over HTTPS with the public name and trusted certificates (waiting up to --https-wait seconds, default
600, for Caddy's first certificate); on any failure it restores the previous Caddyfile, files and service state. The
HTTPS check needs the certificate for explorer.pocol.net, so the full install runs once the owner has moved the DNS
record and public resolvers return this server for the name (only --check before that).
"""
import hashlib
import http.client
import json
import os
from pathlib import Path
import re
import shutil
import socket
import ssl
import stat
import subprocess
import sys
import tempfile
import time

HERE = Path(__file__).resolve().parent
REWARDS = HERE.parent / 'rewards'
sys.path.insert(0, str(REWARDS))
sys.path.insert(0, str(HERE.parent / 'studio'))
sys.path.insert(0, str(HERE.parent / 'faucet'))
sys.path.insert(0, str(HERE))
import explorer_site  # noqa: E402
import explorer_api  # noqa: E402
import site_snapshot  # noqa: E402  (deploy/websites/tools, on the path through explorer_site)

ROOT = HERE.parents[1]
SNAPSHOT = 'deploy/websites/explorer.pocol.net'
INCLUDE = ROOT / 'deploy/websites/tools/explorer-s1-include.txt'
BASE = Path('/opt/pocol-explorer-gateway')
SOURCES = ('gateway.py', 'pocol_rewards.py', 'explorer_api.py', 'pocol_transactions.py', 'pocol_holders.py', 'pocol_geoip.py')
MARKER = 'managed-by-explorer-v1'
SERVICE = 'pocol-explorer-gateway.service'
UNIT = Path('/etc/systemd/system') / SERVICE
CONFIG = Path('/etc/pocol-explorer-gateway')
NETWORK = CONFIG / 'network.json'
FORKS = CONFIG / 'forks.json'
CADDYFILE = Path('/etc/caddy/Caddyfile')
SITE_PARENT = Path(str(explorer_site.SITE_PARENT))
BACKUPS = Path('/var/backups/pocol-explorer-gateway')
PORT = 8088
ROOT_UID = 0
# How long the HTTPS check waits for Caddy's first certificate: longer than the ACME client's first retries (about one
# then two minutes after a failed validation), so a slow first validation right after the DNS move does not roll back
# the install and start a new issuance on every rerun (each failure counts against the CA's per-name limit).
HTTPS_WAIT, HTTPS_WAIT_RANGE = 600, (60, 3600)
ORIGIN = 'https://' + explorer_site.HOST
ARGUMENTS = (f'--bind 127.0.0.1 --port {PORT} --rpc-port 8545 --v3-network {NETWORK} --site-origin {ORIGIN}')
# The transaction index (pocol_transactions.py) in the service's own state directory (DynamicUser: /var/lib/private,
# reachable only by the service): its SQLite file, bounded by pocol_transactions.DEFAULT_MAX_ROWS.
TX_DB = '/var/lib/pocol-explorer-gateway/transactions.sqlite3'
# The holders index (pocol_holders.py) beside it, in the same state directory.
HOLDERS_DB = '/var/lib/pocol-explorer-gateway/holders.sqlite3'
# The miner map's GeoIP database (gateway.GEOIP_DB: BASE/geoip/dbip-country.bin), the reporters' credential
# (CONFIG/geo-reporters.json) and its unit drop-in (UNIT's .d directory/geo-reporters.conf); functions, so they follow
# BASE, CONFIG and UNIT.
GEOIP_NAME, GEO_CREDENTIAL_NAME, GEO_DROPIN_NAME = 'dbip-country.bin', 'geo-reporters.json', 'geo-reporters.conf'
def geoip_dir(): return BASE / 'geoip'
def geoip_file(): return geoip_dir() / GEOIP_NAME
def geo_credential(): return CONFIG / GEO_CREDENTIAL_NAME
def geo_dropin_dir(): return UNIT.with_name(UNIT.name + '.d')
def geo_dropin(): return geo_dropin_dir() / GEO_DROPIN_NAME
def geo_dropin_text():
    """The drop-in that loads the reporters' credential (written only while that file exists: LoadCredential= fails
    the start when its file is missing). systemd reads the file as root; the service reads
    $CREDENTIALS_DIRECTORY/geo-reporters (pocol_geoip.load_credentials)."""
    return ('# Managed by install_explorer_gateway.py while the reporters\' credential exists (operator node reports).\n'
            f'[Service]\nLoadCredential=geo-reporters:{geo_credential()}\n')
# MemoryMax=256M is kept with the holders index and the miner map: measured (ru_maxrss, Python 3.11) the holders
# snapshot publication at 100,000 accounts adds under 50 MB (a test process holding the fake node's 100,000 accounts
# too peaked at 71 MB), the miner map answer at REG_RECORDS_MAX (24,576) records under 45 MB (65 MB with the test's own
# data), and the GeoIP database is a read-only file mapping (DB-IP Lite country: about 10-20 MB, reclaimable page
# cache); with the transaction index, 16 request threads and a reward snapshot the gateway stays well below 256 MB.
UNIT_TEXT = f'''[Unit]
Description=PoCol explorer gateway (read-only /rpc, the explorer's /api reads, the transaction and holders indexes)
After=network.target pocol-standalone.service pocol-observer.service

[Service]
Type=simple
DynamicUser=yes
StateDirectory=pocol-explorer-gateway
StateDirectoryMode=0700
Environment=POCOL_TX_DB={TX_DB}
Environment=POCOL_HOLDERS_DB={HOLDERS_DB}
ExecStart=/usr/bin/python3 {BASE}/gateway.py {ARGUMENTS}
WorkingDirectory={BASE}
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
TasksMax=64
UMask=0077

[Install]
WantedBy=multi-user.target
'''


NETWORK_ARGUMENT, FORKS_ARGUMENT = f'--v3-network {NETWORK} ', f'--fork-schedule {FORKS} '
# The units earlier releases wrote: an update replaces them; any other text is a custom edit. RELEASE_UNIT_TEXT is the
# current release's (main at 04f77806: the transaction index, no holders index), PREVIOUS_UNIT_TEXT the one before it
# (main at 1bb11f4c: no transaction index).
RELEASE_UNIT_TEXT = (UNIT_TEXT
                     .replace("(read-only /rpc, the explorer's /api reads, the transaction and holders indexes)",
                              "(read-only /rpc, the explorer's /api reads and the transaction index)")
                     .replace(f'Environment=POCOL_HOLDERS_DB={HOLDERS_DB}\n', ''))
PREVIOUS_UNIT_TEXT = (RELEASE_UNIT_TEXT
                      .replace("(read-only /rpc, the explorer's /api reads and the transaction index)", "(read-only /rpc and the explorer's /api reads)")
                      .replace('After=network.target pocol-standalone.service pocol-observer.service', 'After=network.target pocol-standalone.service')
                      .replace(f'StateDirectory=pocol-explorer-gateway\nStateDirectoryMode=0700\nEnvironment=POCOL_TX_DB={TX_DB}\n', ''))


def unit_text(forks, text=None):
    """The unit, with --fork-schedule when the operator's forks.json exists (a scheduled cap raise)."""
    text = UNIT_TEXT if text is None else text
    if not forks:
        return text
    return text.replace(NETWORK_ARGUMENT, NETWORK_ARGUMENT + FORKS_ARGUMENT, 1)


def known_units():
    """The unit texts an update may replace: this release's and the two before it, with and without forks.json."""
    return {unit_text(f, t) for f in (False, True) for t in (UNIT_TEXT, RELEASE_UNIT_TEXT, PREVIOUS_UNIT_TEXT)}


def check_geo_credential():
    """The reporters' credential as systemd will load it, or None when the file does not exist: a root-owned regular
    file nobody else may read or write (it holds the reporters' keys and the network salt), valid for
    pocol_geoip.parse_credentials (whose messages name a field, never a secret)."""
    path = geo_credential()
    if path.is_symlink(): raise RuntimeError(f'Unexpected symbolic link {path}; no changes made')
    if not path.exists(): return None
    info = path.lstat()
    if not stat.S_ISREG(info.st_mode) or info.st_uid != ROOT_UID or info.st_mode & 0o077:
        raise RuntimeError(f'{path} must be a root-owned regular file that only root may read (0600: it holds the '
                           f'reporters\' keys), not {oct(info.st_mode & 0o777)}; no changes made')
    import pocol_geoip
    try:
        pocol_geoip.parse_credentials(path.read_bytes())
    except pocol_geoip.CredentialError as error:
        raise RuntimeError(f'{path}: {error}; no changes made') from None
    return path


def check_geoip_database(path):
    """The bytes of a built GeoIP database after pocol_geoip.GeoDatabase opened it (header, size, records digest) and
    verify() checked every record; RuntimeError otherwise."""
    import pocol_geoip
    try:
        raw = Path(path).read_bytes()
        with pocol_geoip.GeoDatabase(path) as database:
            database.verify()
            release, ranges = database.release, (database.ipv4_ranges, database.ipv6_ranges)
        if Path(path).read_bytes() != raw: raise ValueError(f'{path} changed while it was checked')
    except (OSError, ValueError) as error:
        raise RuntimeError(f'--geoip-db: {error}; no changes made') from None
    return raw, release, ranges


def check_forced_loopback():
    """The unit starts the gateway on loopback and the block proxies only to it (a guard against an edit of this file)."""
    for text in (UNIT_TEXT, unit_text(True)):
        if text.count(' --bind 127.0.0.1 ') != 1 or 'IPAddressAllow=localhost' not in text or f'--port {PORT} ' not in text:
            raise RuntimeError('The explorer gateway service must bind 127.0.0.1:8088')
    block = explorer_site.render_block('/srv/pocol-explorer/site-000000000000')
    if block.count('reverse_proxy 127.0.0.1:8088') != 3 or re.search(r'reverse_proxy (?!127\.0\.0\.1:8088\b)', block):
        raise RuntimeError('The explorer Caddy block must proxy only to the loopback gateway')


def same_identity(value, network):
    """chainId is the network's integer chain ID (not a bool or a float) and genesisHash its hash in any letter case."""
    if not isinstance(value, dict): return False
    chain, found = value.get('chainId'), value.get('genesisHash')
    return (type(chain) is int and chain == network.chain_id and isinstance(found, str)
            and re.fullmatch(r'0x[0-9a-fA-F]{64}', found) is not None and found.lower() == network.genesis)


def command(*args, check=True):
    return subprocess.run(args, check=check, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=45)


def atomic_write(path, raw, mode=0o644):
    fd, name = tempfile.mkstemp(prefix='.explorer-', dir=path.parent)
    try:
        with os.fdopen(fd, 'wb') as stream:
            stream.write(raw); stream.flush(); os.fsync(stream.fileno())
        os.chmod(name, mode); os.replace(name, path)
    finally: Path(name).unlink(missing_ok=True)


def make_directory(path):
    """mkdir -p, every directory it creates 0755 whatever root's umask (Caddy and the DynamicUser service read below
    SITE_PARENT and BASE; a umask of 027 would make them 0750)."""
    created, folder = [], path
    while not folder.exists():
        created.append(folder); folder = folder.parent
    for folder in reversed(created):
        folder.mkdir(mode=0o755); os.chmod(folder, 0o755)


def check_service_paths():
    """Refuse what Caddy (the caddy user) or the DynamicUser service could not read, or a local user could change:
    SITE_PARENT and BASE, if they exist, root-owned directories that others may enter and nobody else may write; the
    network file (and forks.json) readable by others in a directory they may enter (the service reads them as its own
    user; root reading them proves nothing)."""
    for folder in (SITE_PARENT, BASE):
        if not folder.exists(): continue
        info = folder.lstat()
        if not stat.S_ISDIR(info.st_mode) or info.st_uid != ROOT_UID or info.st_mode & 0o022 or info.st_mode & 0o005 != 0o005:
            raise RuntimeError(f'{folder} must be a root-owned directory (0755: others may read and enter it, only root may '
                               f'write it), not {oct(info.st_mode & 0o777)}; review it first (no changes made)')
    if CONFIG.stat().st_mode & 0o001 == 0:
        raise RuntimeError(f'{CONFIG} must be enterable by others (0755): the explorer gateway service reads its files as '
                           'its own unprivileged user')
    for path in (NETWORK, FORKS):
        if path.is_file() and path.stat().st_mode & 0o004 == 0:
            raise RuntimeError(f'{path} must be readable by others (0644, writable only by root): the explorer gateway '
                               'service reads it as its own unprivileged user')


def write_site(files, ident):
    """The static copy at SITE_PARENT/site-<ident> (root-owned, world-readable, written beside it then renamed).
    (path, created): an existing copy of the same id is reused when its files are exactly these."""
    target = SITE_PARENT / f'site-{ident}'
    if target.is_symlink(): raise RuntimeError(f'{target} is a symbolic link; no changes made')
    if target.exists():
        if explorer_site.site_files(target) != files:
            raise RuntimeError(f'{target} exists with other files; review it first')
        return target, False
    make_directory(SITE_PARENT)
    staging = Path(tempfile.mkdtemp(prefix='.site-', dir=SITE_PARENT))
    try:
        for name, raw in files.items():
            path = staging / name
            path.parent.mkdir(mode=0o755, parents=True, exist_ok=True)
            path.write_bytes(raw); os.chmod(path, 0o644)
        for folder in [staging, *(p for p in staging.rglob('*') if p.is_dir())]:
            os.chmod(folder, 0o755)
        os.rename(staging, target)
    except BaseException:
        shutil.rmtree(staging, ignore_errors=True); raise
    return target, True


def http_json(conn, method, path, payload=None, headers=None):
    body = None if payload is None else json.dumps(payload).encode()
    conn.request(method, path, body, {'Content-Type': 'application/json', 'Connection': 'close', **(headers or {})})
    response = conn.getresponse(); raw = response.read(4 * 1024 * 1024 + 1)
    return response.status, response, raw


def local(method, path, payload=None, headers=None):
    conn = http.client.HTTPConnection('127.0.0.1', PORT, timeout=20)
    try: return http_json(conn, method, path, payload, headers)
    finally: conn.close()


def verify_local(network):
    """The service on loopback: /health, /api/rpc.php (with the site Origin) and /api/health.php name the network."""
    status, _, raw = local('GET', '/health')
    if status != 200 or json.loads(raw).get('chainId') != network.chain_id:
        raise RuntimeError('Explorer gateway /health did not answer for the configured network')
    status, _, raw = local('POST', '/api/rpc.php', [{'jsonrpc': '2.0', 'id': 1, 'method': 'pocol_getParams', 'params': []}],
                           {'Origin': ORIGIN})
    answer = json.loads(raw) if status == 200 else None
    if not (isinstance(answer, list) and len(answer) == 1 and same_identity(answer[0].get('result'), network)):
        raise RuntimeError('Explorer gateway /api/rpc.php identity verification failed')
    status, _, raw = local('GET', '/api/health.php')
    if status != 200 or json.loads(raw).get('result') != 'PASS':
        raise RuntimeError('Explorer gateway /api/health.php did not PASS')


def tls(method, path, payload=None, headers=None):
    """One request to local Caddy with the public SNI and trusted certificate checks (no hairpin routing or DNS).
    It does not prove reachability from outside."""
    hostname = explorer_site.HOST
    context = ssl.create_default_context()
    context.minimum_version = ssl.TLSVersion.TLSv1_2
    conn = http.client.HTTPSConnection(hostname, 443, timeout=12, context=context)
    try:
        conn.sock = context.wrap_socket(socket.create_connection(('127.0.0.1', 443), timeout=12), server_hostname=hostname)
        return http_json(conn, method, path, payload, headers)
    finally: conn.close()


def verify_https(network, files, studio_route, faucet_route=False):
    """The public routes: every static file byte for byte with the explorer's headers, /api/health.php PASS,
    /api/rpc.php and /rpc naming the network, /health, /api/holders.php and /api/miner-geography.php (their data or
    their 503), hidden and server paths 404, and the Studio route and the faucet route (/faucet/status naming the
    network's chain ID; never a claim) if present."""
    for name, raw in sorted(files.items()):
        status, response, body = tls('GET', '/' + name)
        if status != 200 or body != raw:
            raise RuntimeError(f'HTTPS /{name} is not the installed file')
    status, response, body = tls('GET', '/')
    if status != 200 or body != files['index.html']:
        raise RuntimeError('HTTPS / is not the installed index.html')
    for name, value in (*explorer_api.SECURITY_HEADERS, ('Cache-Control', 'no-store')):
        if response.getheader(name) != value:
            raise RuntimeError(f'HTTPS / does not send {name}: {value}')
    status, response, body = tls('GET', '/api/health.php')
    if status != 200 or json.loads(body).get('result') != 'PASS' or response.getheader('Content-Security-Policy') != explorer_api.CONTENT_SECURITY_POLICY:
        raise RuntimeError('HTTPS /api/health.php did not PASS with the explorer headers')
    request = [{'jsonrpc': '2.0', 'id': 1, 'method': 'pocol_getParams', 'params': []}]
    status, _, body = tls('POST', '/api/rpc.php', request, {'Origin': ORIGIN})
    if status != 200 or not same_identity(json.loads(body)[0].get('result'), network):
        raise RuntimeError('HTTPS /api/rpc.php identity verification failed')
    status, _, body = tls('POST', '/rpc', request[0])
    if status != 200 or not same_identity(json.loads(body).get('result'), network):
        raise RuntimeError('HTTPS /rpc identity verification failed')
    status, _, body = tls('GET', '/health')
    if status != 200 or json.loads(body).get('service') != 'pocol-explorer-gateway':
        raise RuntimeError('HTTPS /health verification failed')
    # The holders and miner map reads answer through the gateway with the explorer headers: their data for this network,
    # or (while the holders index has no snapshot yet: right after a first install it indexes from genesis) the
    # route's JSON 503.
    for path, schema in (('/api/holders.php', 'pocol-holders-v1'), ('/api/miner-geography.php', 'pocol-miners-geography-v2')):
        status, response, body = tls('GET', path)
        value = json.loads(body) if status in (200, 503) else None
        data = value.get('data') if status == 200 and isinstance(value, dict) else None
        if (response.getheader('Content-Security-Policy') != explorer_api.CONTENT_SECURITY_POLICY or not isinstance(value, dict)
                or (status == 200 and not (same_identity(data, network) and data.get('schema') == schema))
                or (status == 503 and not isinstance(value.get('error'), str))):
            raise RuntimeError(f'HTTPS {path} did not answer for the network (its data, or 503 while the holders index syncs)')
    for path in ('/SHA256SUMS.txt', '/.htaccess', '/api/config.php', '/api/lib.php', '/index.php', '/site-identity.json'):
        if tls('GET', path)[0] != 404:
            raise RuntimeError(f'HTTPS {path} is not 404')
    if studio_route:
        status, _, body = tls('GET', '/studio/health')
        if status != 200 or json.loads(body).get('service') != 'pocol-studio-gateway':
            raise RuntimeError('The Studio HTTPS route stopped answering')
    if faucet_route:
        status, _, body = tls('GET', '/faucet/status')
        value = json.loads(body) if status == 200 else None
        if not isinstance(value, dict) or value.get('type') != 'status' or type(value.get('chainId')) is not int \
                or value['chainId'] != network.chain_id:
            raise RuntimeError('The faucet HTTPS route stopped answering for the network')


def verify_https_with_retry(network, files, studio_route, seconds=HTTPS_WAIT, faucet_route=False):
    """Caddy may still be obtaining the certificate right after the reload: retry until `seconds` have passed."""
    deadline, last = time.monotonic() + seconds, None
    while True:
        try: return verify_https(network, files, studio_route, faucet_route)
        except (OSError, RuntimeError, ValueError, KeyError, TypeError, IndexError, http.client.HTTPException) as error:
            last = error
        if time.monotonic() >= deadline: raise last
        time.sleep(3)


def rollback_install(original, previous, base_existed, new_site, old_active, old_enabled, created=()):
    """Attempt every recovery step, even if a service command fails or times out. `created`: the directories the
    install made inside BASE or beside UNIT (removed, deepest first, once their files are restored)."""
    failures = []
    def attempt(label, action):
        try: action()
        except Exception as exc: failures.append(label + ': ' + str(exc))
    attempt('restore Caddyfile', lambda: atomic_write(CADDYFILE, original))
    attempt('reload previous Caddy configuration', lambda: command('systemctl', 'reload', 'caddy'))
    if UNIT.exists():
        attempt('stop explorer gateway service', lambda: command('systemctl', 'stop', SERVICE))
        if not old_enabled:
            attempt('restore disabled explorer gateway boot state', lambda: command('systemctl', 'disable', SERVICE))
    for path, old in previous.items():
        attempt('restore ' + str(path), lambda p=path, raw=old: p.unlink(missing_ok=True) if raw is None else atomic_write(p, raw))
    for folder in sorted(created, key=lambda p: len(p.parts), reverse=True):
        attempt('remove new directory ' + str(folder), lambda f=folder: f.rmdir() if f.exists() else None)
    if new_site is not None:
        attempt('remove new site copy', lambda: shutil.rmtree(new_site))
    if not base_existed:
        attempt('remove new gateway directory', lambda: BASE.rmdir() if BASE.exists() else None)
    attempt('reload restored service definition', lambda: command('systemctl', 'daemon-reload'))
    if old_enabled:
        attempt('restore enabled explorer gateway boot state', lambda: command('systemctl', 'enable', SERVICE))
    if old_active:
        attempt('restart previous explorer gateway service', lambda: command('systemctl', 'restart', SERVICE))
    return failures


def prune_sites(keep):
    """Remove site copies whose names are not in `keep` (the new one and the one the previous Caddyfile served)."""
    removed = []
    for path in SITE_PARENT.iterdir():
        if re.fullmatch(r'site-[0-9a-f]{12}', path.name) and not path.is_symlink() and path.is_dir() and path.name not in keep:
            shutil.rmtree(path); removed.append(path.name)
    return removed


USAGE = ('Usage: sudo python3 deploy/explorer/install_explorer_gateway.py --site DIR [--check] [--accept-site site-<12 hex>] '
         f'[--https-wait SECONDS ({HTTPS_WAIT_RANGE[0]}-{HTTPS_WAIT_RANGE[1]}, default {HTTPS_WAIT})] [--geoip-db FILE]')


def usage(argv):
    """(site directory, check only, the site id --accept-site names or None, the HTTPS wait in seconds, the GeoIP
    database --geoip-db names or None); each option at most once."""
    if len(argv) < 2 or argv[0] != '--site' or argv[1].startswith('--'):
        raise RuntimeError(USAGE)
    rest, options = list(argv[2:]), {}
    while rest:
        flag = rest.pop(0)
        if flag in options: raise RuntimeError(USAGE)
        if flag == '--check': options[flag] = True
        elif flag == '--accept-site' and rest and re.fullmatch(r'site-[0-9a-f]{12}', rest[0]): options[flag] = rest.pop(0)
        elif (flag == '--https-wait' and rest and re.fullmatch(r'[0-9]{1,5}', rest[0])
              and HTTPS_WAIT_RANGE[0] <= int(rest[0]) <= HTTPS_WAIT_RANGE[1]): options[flag] = int(rest.pop(0))
        elif flag == '--geoip-db' and rest and rest[0] and not rest[0].startswith('--'): options[flag] = Path(rest.pop(0))
        else: raise RuntimeError(USAGE)
    return (Path(argv[1]), options.get('--check', False), options.get('--accept-site'), options.get('--https-wait', HTTPS_WAIT),
            options.get('--geoip-db'))


def checkout_site():
    """{path: bytes} of this checkout's explorer snapshot as site_snapshot.py publishes it: the include list's files,
    each checked against the snapshot's SHA256SUMS.txt. Read from the checkout's files, as the gateway sources are (git
    would refuse, under sudo, a checkout that another user owns)."""
    folder = ROOT / SNAPSHOT
    try:
        include = site_snapshot.parse_include_list(INCLUDE.read_text(encoding='utf-8'))
        sums = site_snapshot.checksum_entries({'/SHA256SUMS.txt': (folder / 'SHA256SUMS.txt').read_bytes()})
        files = {}
        for path in include:
            raw = (folder / path.lstrip('/')).read_bytes()
            if path in sums and hashlib.sha256(raw).hexdigest() != sums[path]:
                raise ValueError(f'{SNAPSHOT}{path} differs from its SHA256SUMS.txt')
            files[path.lstrip('/')] = raw
        return files
    except (ValueError, OSError) as error:
        raise RuntimeError(f'The explorer snapshot of this checkout could not be read: {error}; no changes made') from error


def check_copy(site, files, ident, accept):
    """Refuse a copy that is not this checkout's snapshot (a leftover or edited copy), unless --accept-site names it."""
    if accept is not None:
        if accept != f'site-{ident}':
            raise RuntimeError(f'--accept-site names {accept}, but {site} is site-{ident}; no changes made')
        return 'accepted with --accept-site'
    committed = checkout_site()
    if files != committed:
        raise RuntimeError(f'{site} (site-{ident}) is not the explorer snapshot of this checkout '
                           f'(site-{explorer_site.manifest(committed)[1]}). Build the copy again into a new directory '
                           '(site_snapshot.py --out), or rerun with --accept-site site-' + ident + ' if this copy is meant '
                           'to differ; no changes made')
    return "this checkout's snapshot"


def main(argv=None):
    site, check_only, accept, https_wait, geoip_db = usage(sys.argv[1:] if argv is None else argv)
    check_forced_loopback()
    if os.geteuid() != 0: raise RuntimeError('Run with sudo')
    for utility in ('caddy', 'systemctl'):
        if not shutil.which(utility): raise RuntimeError(f'{utility} must already be installed')
    sources = {}
    for name in SOURCES:
        raw = (REWARDS / name).read_bytes(); compile(raw, name, 'exec'); sources[name] = raw
    for p in (BASE, UNIT, CADDYFILE, SITE_PARENT, CONFIG, NETWORK, FORKS, geoip_dir(), geoip_file(), geo_dropin_dir(), geo_dropin()):
        if p.is_symlink(): raise RuntimeError(f'Unexpected symbolic link {p}; no changes made')
    if not CADDYFILE.is_file(): raise RuntimeError('Install Caddy first (its /etc/caddy/Caddyfile is missing)')
    if command('systemctl', 'is-active', '--quiet', 'caddy', check=False).returncode:
        raise RuntimeError('Caddy must be running')
    if BASE.exists() and not (BASE / MARKER).is_file():
        raise RuntimeError(f'Unrecognized {BASE}; no changes made')
    forks = FORKS.is_file()
    unit = unit_text(forks)
    if UNIT.exists() and UNIT.read_text() not in known_units():
        raise RuntimeError('The explorer gateway service has custom edits; review before updating')
    dropin = geo_dropin()
    if dropin.exists() and dropin.read_text() != geo_dropin_text():
        raise RuntimeError(f'{dropin} has custom edits; review before updating')
    if geo_dropin_dir().exists() and not geo_dropin_dir().is_dir():
        raise RuntimeError(f'{geo_dropin_dir()} is not a directory; no changes made')
    credential = check_geo_credential()
    geoip = check_geoip_database(geoip_db) if geoip_db is not None else None
    import gateway
    network = gateway.load_v3_network(NETWORK)
    if forks: gateway.load_fork_schedule(FORKS)
    gateway.Gateway(network=network).verify_network()
    check_service_paths()
    files = explorer_site.site_files(site)
    text, ident = explorer_site.manifest(files)
    origin = check_copy(site, files, ident, accept)
    import install_studio_gateway as studio
    import faucet_route as faucet
    original = CADDYFILE.read_bytes()
    candidate, old_root = explorer_site.plan_caddyfile(original.decode(), SITE_PARENT / f'site-{ident}', studio, faucet)
    studio_route, faucet_route = studio.MARKER in candidate, faucet.MARKER in candidate
    old_active = command('systemctl', 'is-active', '--quiet', SERVICE, check=False).returncode == 0
    old_enabled = command('systemctl', 'is-enabled', '--quiet', SERVICE, check=False).returncode == 0
    if not old_active:
        with socket.socket() as sock: sock.bind(('127.0.0.1', PORT))
    with tempfile.TemporaryDirectory(prefix='pocol-explorer-check-') as tmp:
        testfile = Path(tmp) / 'Caddyfile'; testfile.write_text(candidate)
        result = command('caddy', 'validate', '--config', str(testfile), '--adapter', 'caddyfile', check=False)
        if result.returncode: raise RuntimeError('Caddy candidate validation failed: ' + result.stderr.decode()[-1500:])
    installed_geoip = geoip_file().is_file()
    geo = ('the GeoIP database ' + (f'{geoip_db} (release {geoip[1]}, {geoip[2][0]} IPv4 and {geoip[2][1]} IPv6 ranges)'
                                    if geoip else 'kept' if installed_geoip else 'not installed (nothing is located)')
           + ', operator node reports ' + ('enabled (the geo-reporters credential)' if credential else 'disabled (no credential)'))
    if check_only:
        print(f'CHECK PASSED: network {network.chain_id} {network.genesis} on the node, site-{ident} ({len(files)} files, '
              f'{origin}), the loopback port, and the candidate Caddy configuration; {geo}.')
        print(text, end='')
        return
    BACKUPS.mkdir(parents=True, exist_ok=True, mode=0o700)
    backup = BACKUPS / (time.strftime('%Y%m%d-%H%M%S') + '-' + str(os.getpid()))
    backup.mkdir(mode=0o700)
    (backup / 'Caddyfile.before').write_bytes(original)
    paths = [BASE / name for name in SOURCES] + [BASE / MARKER, UNIT, geo_dropin()] + ([geoip_file()] if geoip else [])
    previous = {p: p.read_bytes() if p.is_file() else None for p in paths}
    for i, p in enumerate(paths):
        if previous[p] is not None: (backup / f'file-{i}.before').write_bytes(previous[p])
    (backup / 'state.json').write_text(json.dumps({'active': old_active, 'enabled': old_enabled, 'paths': list(map(str, paths)),
                                                   'previousSite': old_root, 'site': f'site-{ident}'}))
    base_existed = BASE.exists()
    make_directory(BASE)
    new_site, created_dirs = None, []
    try:
        site_root, created = write_site(files, ident)
        new_site = site_root if created else None
        for name, raw in sources.items():
            atomic_write(BASE / name, raw)
        atomic_write(BASE / MARKER, b'PoCol explorer gateway v1\n')
        if geoip:
            if not geoip_dir().exists(): created_dirs.append(geoip_dir()); make_directory(geoip_dir())
            atomic_write(geoip_file(), geoip[0])
        atomic_write(UNIT, unit.encode())
        if credential:
            if not geo_dropin_dir().exists(): created_dirs.append(geo_dropin_dir()); make_directory(geo_dropin_dir())
            atomic_write(geo_dropin(), geo_dropin_text().encode())
        else:
            dropin.unlink(missing_ok=True)
        command('systemctl', 'daemon-reload'); command('systemctl', 'restart', SERVICE)
        last = None
        for _ in range(20):
            try: verify_local(network); last = None; break
            except (OSError, RuntimeError, ValueError, AttributeError, http.client.HTTPException) as exc: last = exc; time.sleep(.5)
        if last: raise last
        atomic_write(CADDYFILE, candidate.encode())
        command('systemctl', 'reload', 'caddy')
        verify_https_with_retry(network, files, studio_route, https_wait, faucet_route=faucet_route)
        command('systemctl', 'enable', SERVICE)
    except BaseException as exc:
        failures = rollback_install(original, previous, base_existed, new_site, old_active, old_enabled, created_dirs)
        if failures:
            raise RuntimeError('Installation failed: ' + str(exc) + '; ROLLBACK INCOMPLETE: ' + '; '.join(failures)
                               + '; recovery backup: ' + str(backup)) from exc
        raise
    keep = {site_root.name} | ({Path(old_root).name} if old_root else set())
    try: removed = prune_sites(keep)
    except OSError as exc: removed = ['(not pruned: ' + str(exc) + ')']
    print(f'INSTALLED: explorer gateway on 127.0.0.1:{PORT} and the explorer site site-{ident} at https://{explorer_site.HOST}')
    print('PASS: every static file, /api/rpc.php, /api/health.php, /api/holders.php, /api/miner-geography.php, /rpc and /health '
          'over HTTPS with trusted certificates' + (', and the Studio route' if studio_route else '')
          + (', and the faucet route' if faucet_route else '') + '.')
    print(f'Miner map: {geo}.')
    if removed: print('Removed older site copies: ' + ', '.join(removed))
    print(f'Verify from outside: curl --fail --max-time 20 https://{explorer_site.HOST}/api/health.php')
    print('Backup: ' + str(backup))


if __name__ == '__main__':
    try: main()
    except Exception as exc: print('INSTALL FAILED: ' + str(exc), file=sys.stderr); sys.exit(1)
