#!/usr/bin/env python3
"""Installer of the operator geography reporter (pocol_geo_reporter.py; development/explorer/MINERS-MAP-HOLDERS.md §4)
on a PoCol node host (the intended hosts are the three founder servers). Only the owner runs it, on a server, by hand.

    sudo python3 deploy/explorer/install_geo_reporter.py --database geoip-country.bin --check
    sudo python3 deploy/explorer/install_geo_reporter.py --database geoip-country.bin
    sudo python3 deploy/explorer/install_geo_reporter.py --uninstall

Before it runs, the owner writes the reporter's credential, root-owned, mode 0600:
/etc/pocol-geo-reporter/credential.json = {"reporter": "founder-1", "key": "<hex>", "salt": "<hex>"} (the key the
explorer gateway's geo-reporters credential holds for that reporter, and the gateway's network salt). The installer
never writes, prints or copies a secret. --database is the binary GeoIP database built from the DB-IP Lite CSV
(`python3 deploy/rewards/pocol_geoip.py build ...`); without it the one already installed is kept.

--check runs every preflight and changes nothing: /usr/bin/python3 is 3.10 or later; the node on 127.0.0.1:8545
answers pocol_getParams with the network this repository pins (deploy/websites/site-identity.json; --other-network
takes the node's own network on purpose, for a test network); the database opens and verifies; the credential file
exists, is a regular root-owned file of mode 0600 and parses; the explorer URL (--explorer-url, default
https://explorer.pocol.net/api/geo-report.php) is https. An install backs up what it replaces under
/var/backups/pocol-geo-reporter, writes /opt/pocol-geo-reporter/{pocol_geo_reporter.py, pocol_geoip.py},
/etc/pocol-geo-reporter/{config.json, geoip-country.bin} and the units pocol-geo-reporter.service (one report per
run: DynamicUser, ProtectSystem=strict, LoadCredential=geo-reporter, Nice=19, MemoryMax=64M) and
pocol-geo-reporter.timer (every 60 s), runs one --dry-run (reads the node once, sends nothing) and enables the
timer; on any failure it restores the previous files and boot state. --uninstall stops and removes the units and the
installed files and keeps the owner's credential.json.

The unit has no IPAddressAllow=: systemd's IP filter takes addresses, not host names, and the explorer's address is
resolved by DNS at each run (pinning it would silently stop the reporter when it changes). The code itself connects
to 127.0.0.1:8545 and to the configured https URL only. Standard library only; contacts nothing but 127.0.0.1:8545.
"""
import http.client
import json
import os
from pathlib import Path
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import time

HERE = Path(__file__).resolve().parent
SOURCES = {'pocol_geo_reporter.py': HERE / 'pocol_geo_reporter.py', 'pocol_geoip.py': HERE.parent / 'rewards' / 'pocol_geoip.py'}
sys.path.insert(0, str(HERE.parent / 'rewards'))
sys.path.insert(0, str(HERE))
import pocol_geoip  # noqa: E402
import pocol_geo_reporter as reporter  # noqa: E402

BASE = Path('/opt/pocol-geo-reporter')
CONFIG = Path('/etc/pocol-geo-reporter')
CONFIG_FILE = CONFIG / 'config.json'
DATABASE = CONFIG / 'geoip-country.bin'
CREDENTIAL = CONFIG / 'credential.json'
SERVICE, TIMER = 'pocol-geo-reporter.service', 'pocol-geo-reporter.timer'
UNITS = Path('/etc/systemd/system')
BACKUPS = Path('/var/backups/pocol-geo-reporter')
REPO_IDENTITY = HERE.parent / 'websites' / 'site-identity.json'
MARKER = 'managed-by-geo-reporter-v1'
PYTHON = '/usr/bin/python3'
DEFAULT_URL = 'https://explorer.pocol.net/api/geo-report.php'
ROOT_UID = 0


def command(*args, check=True):
    return subprocess.run(args, check=check, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=60)


def service_text():
    return f'''# PoCol operator geography reporter: one signed report of this node's head, its own miner address and its peers'
# countries (keyed ids, never an IP) to the explorer. Installed by deploy/explorer/install_geo_reporter.py and started
# by {TIMER} every 60 s. It reads the node once per run over loopback and never affects mining.
[Unit]
Description=PoCol geography reporter (one signed report to the explorer)
After=network-online.target
Wants=network-online.target

[Service]
Type=oneshot
ExecStart={PYTHON} -I -B {BASE}/pocol_geo_reporter.py --config {CONFIG_FILE}
LoadCredential=geo-reporter:{CREDENTIAL}
DynamicUser=yes
TimeoutStartSec=30
# Low priority and small limits: the node and its miner run beside it.
Nice=19
CPUWeight=20
CPUQuota=10%
IOSchedulingClass=idle
MemoryMax=64M
MemorySwapMax=0
TasksMax=8
CapabilityBoundingSet=
AmbientCapabilities=
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes
PrivateDevices=yes
PrivateIPC=yes
ProtectKernelTunables=yes
ProtectKernelModules=yes
ProtectKernelLogs=yes
ProtectControlGroups=yes
ProtectClock=yes
ProtectHostname=yes
ProtectProc=invisible
ProcSubset=pid
RestrictNamespaces=yes
RestrictRealtime=yes
RestrictSUIDSGID=yes
LockPersonality=yes
MemoryDenyWriteExecute=yes
RemoveIPC=yes
KeyringMode=private
# AF_NETLINK: the C library's resolver may ask the kernel for the host's addresses (as the disk monitor's unit).
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX AF_NETLINK
SystemCallArchitectures=native
SystemCallFilter=@system-service
SystemCallFilter=~@privileged
SystemCallErrorNumber=EPERM
UMask=0077
# Defense in depth (the code opens none of these): the node's data and miner key, credential stores, backups.
# The credential file itself (root, 0600) is unreadable to the dynamic user: it arrives through LoadCredential= in
# $CREDENTIALS_DIRECTORY only.
InaccessiblePaths=-/var/lib/pocol-standalone
InaccessiblePaths=-/etc/credstore.encrypted
InaccessiblePaths=-/etc/credstore
InaccessiblePaths=-/var/lib/systemd/credential.secret
InaccessiblePaths=-/var/lib/pocol-faucet
InaccessiblePaths=-/etc/pocol-testnet
InaccessiblePaths=-/var/backups
InaccessiblePaths=-/root
# No IPAddressAllow=: it takes addresses, not host names, and the explorer is resolved by DNS at each run. The code
# connects to 127.0.0.1:8545 and to the configured https explorer URL only.
'''


def timer_text():
    return f'''# Every 60 s: an operator report is fresh for 180 s on the explorer, so one missed run changes nothing.
[Unit]
Description=PoCol geography reporter every 60 seconds

[Timer]
OnBootSec=2min
OnUnitActiveSec=60s
AccuracySec=5s
Unit={SERVICE}

[Install]
WantedBy=timers.target
'''


def rpc_identity(timeout=5):
    """(chainId, genesisHash) of the node on 127.0.0.1:8545 (pocol_getParams)."""
    conn = http.client.HTTPConnection('127.0.0.1', 8545, timeout=timeout)
    try:
        conn.request('POST', '/', json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': 'pocol_getParams', 'params': []}),
                     {'Content-Type': 'application/json'})
        reply = conn.getresponse()
        answer = json.loads(reply.read(1 << 22))
        if reply.status != 200 or not isinstance(answer.get('result'), dict):
            raise RuntimeError('pocol_getParams failed')
        return answer['result'].get('chainId'), str(answer['result'].get('genesisHash', '')).lower()
    finally:
        conn.close()


def pinned_identity(path):
    """(chainId, genesisHash) the repository pins (site-identity.json), or None when the file is absent."""
    if not Path(path).is_file():
        return None
    value = json.loads(Path(path).read_bytes()[:1048576])
    chain, genesis = value.get('chainId'), str(value.get('genesisHash', '')).lower()
    if type(chain) is not int or not re.fullmatch(r'0x[0-9a-f]{64}', genesis):
        raise RuntimeError(f'{path} does not name a chain ID and a genesis hash; no changes made')
    return chain, genesis


def file_status(path):
    return os.lstat(path)


def check_credential(path):
    """The credential file is a regular root-owned file of mode 0600 that parses; returns the reporter name."""
    try:
        info = file_status(path)
    except FileNotFoundError:
        raise RuntimeError(f'{path} is missing: write the reporter credential (root, 0600) first; no changes made') from None
    if not stat.S_ISREG(info.st_mode):
        raise RuntimeError(f'{path} must be a regular file (not a link); no changes made')
    if info.st_uid != ROOT_UID or stat.S_IMODE(info.st_mode) != 0o600:
        raise RuntimeError(f'{path} must be owned by root with mode 0600; no changes made')
    try:
        return reporter.load_credential(path)[0]
    except reporter.Failure as failure:
        raise RuntimeError(f'{path}: {failure}; no changes made') from None


def check_database(path):
    try:
        with pocol_geoip.GeoDatabase(path) as db:
            db.verify()
            return db.release, db.ipv4_ranges, db.ipv6_ranges
    except (OSError, ValueError) as error:
        raise RuntimeError(f'GeoIP database {path}: {error}; no changes made') from None


def atomic_write(path, raw, mode=0o644):
    fd, name = tempfile.mkstemp(prefix='.geo-reporter-', dir=str(path.parent))
    try:
        with os.fdopen(fd, 'wb') as stream:
            stream.write(raw); stream.flush(); os.fsync(stream.fileno())
        os.chmod(name, mode); os.replace(name, path)
    finally:
        Path(name).unlink(missing_ok=True)


USAGE = ('Usage: sudo python3 deploy/explorer/install_geo_reporter.py [--database FILE] [--explorer-url URL] '
         '[--other-network] [--check] | --uninstall')


def usage(argv):
    options, rest = {}, list(argv)
    while rest:
        flag = rest.pop(0)
        if flag in options:
            raise RuntimeError(USAGE)
        if flag in ('--check', '--other-network', '--uninstall'):
            options[flag] = True
        elif flag == '--database' and rest and not rest[0].startswith('--'):
            options[flag] = Path(rest.pop(0))
        elif flag == '--explorer-url' and rest and reporter.EXPLORER_URL.fullmatch(rest[0]):
            options[flag] = rest.pop(0)
        else:
            raise RuntimeError(USAGE)
    if options.get('--uninstall') and len(options) > 1:
        raise RuntimeError(USAGE)
    return options


def preflight(options):
    """Every check; returns (config bytes, database bytes or None, summary). Changes nothing."""
    if command(PYTHON, '-c', 'import sys; sys.exit(0 if sys.version_info >= (3, 10) else 1)', check=False).returncode != 0:
        raise RuntimeError(f'{PYTHON} must be Python 3.10 or later (the unit runs it); no changes made')
    url = options.get('--explorer-url', DEFAULT_URL)
    if reporter.EXPLORER_URL.fullmatch(url) is None:
        raise RuntimeError('--explorer-url must be https://HOST[:PORT]/api/geo-report.php; no changes made')
    try:
        chain, genesis = rpc_identity()
    except (OSError, ValueError, RuntimeError, http.client.HTTPException) as error:
        raise RuntimeError(f'The node on 127.0.0.1:8545 cannot be read ({error}); no changes made') from None
    if type(chain) is not int or not re.fullmatch(r'0x[0-9a-f]{64}', genesis):
        raise RuntimeError('The node on 127.0.0.1:8545 names no chain ID and genesis hash; no changes made')
    if not options.get('--other-network'):
        pinned = pinned_identity(REPO_IDENTITY)
        if pinned is None:
            raise RuntimeError(f'{REPO_IDENTITY} is missing: run the installer from the repository checkout, or pass '
                               '--other-network for a test network; no changes made')
        if pinned != (chain, genesis):
            raise RuntimeError(f'The node serves chain {chain} genesis {genesis}, not the network this repository pins '
                               f'(chain {pinned[0]}); no changes made')
    if CONFIG.is_dir() and not stat.S_IMODE(file_status(CONFIG).st_mode) & 0o001:
        # The service's dynamic user reads config.json and the database in it (the credential stays root 0600).
        raise RuntimeError(f'{CONFIG} must be searchable by the service (sudo chmod 0755 {CONFIG}); no changes made')
    source = options.get('--database')
    if source is None and not DATABASE.is_file():
        raise RuntimeError(f'No GeoIP database: pass --database FILE (none is installed at {DATABASE}); no changes made')
    release, n4, n6 = check_database(source if source is not None else DATABASE)
    name = check_credential(CREDENTIAL)
    for name_ in SOURCES:
        if not SOURCES[name_].is_file():
            raise RuntimeError(f'{SOURCES[name_]} is missing: run the installer from the repository checkout; no changes made')
    config = json.dumps({'chainId': chain, 'genesisHash': genesis, 'explorerUrl': url, 'database': str(DATABASE)},
                        indent=1, sort_keys=True).encode() + b'\n'
    summary = (f'reporter {name}, chain {chain} genesis {genesis}, database release {release} ({n4} IPv4 and {n6} IPv6 '
               f'ranges), explorer {url}')
    return config, (source.read_bytes() if source is not None else None), summary


def guard_paths():
    for p in (BASE, CONFIG, CONFIG_FILE, DATABASE, UNITS / SERVICE, UNITS / TIMER):
        if p.is_symlink():
            raise RuntimeError(f'Unexpected symbolic link {p}; no changes made')
    if BASE.exists() and not (BASE / MARKER).is_file():
        raise RuntimeError(f'Unrecognized {BASE}; no changes made')
    for unit in (SERVICE, TIMER):
        if (UNITS / unit).exists() and not (BASE / MARKER).is_file():
            raise RuntimeError(f'{UNITS / unit} exists without this installer\'s marker; review it first; no changes made')


def uninstall():
    guard_paths()
    if not (BASE / MARKER).is_file():
        print('Nothing to remove: the geography reporter is not installed.')
        return
    command('systemctl', 'disable', '--now', TIMER, check=False)
    command('systemctl', 'stop', SERVICE, check=False)
    for path in (UNITS / TIMER, UNITS / SERVICE, CONFIG_FILE, DATABASE):
        path.unlink(missing_ok=True)
    shutil.rmtree(BASE)
    command('systemctl', 'daemon-reload')
    print(f'REMOVED: {TIMER}, {SERVICE}, {BASE}, {CONFIG_FILE} and {DATABASE}. Kept {CREDENTIAL} (the owner\'s secret: '
          'remove it with sudo rm when the reporter is retired, and its key from the explorer gateway\'s credential).')


def main(argv=None):
    options = usage(sys.argv[1:] if argv is None else argv)
    if os.geteuid() != ROOT_UID:
        raise RuntimeError('Run with sudo')
    if not shutil.which('systemctl'):
        raise RuntimeError('systemctl must be installed')
    if options.get('--uninstall'):
        return uninstall()
    guard_paths()
    config, database, summary = preflight(options)
    service, timer = service_text(), timer_text()
    if options.get('--check'):
        print(f'CHECK PASSED: {summary}.')
        print(service, end='')
        print(timer, end='')
        return
    old_active = command('systemctl', 'is-active', '--quiet', TIMER, check=False).returncode == 0
    old_enabled = command('systemctl', 'is-enabled', '--quiet', TIMER, check=False).returncode == 0
    BACKUPS.mkdir(parents=True, exist_ok=True, mode=0o700)
    backup = Path(tempfile.mkdtemp(prefix=time.strftime('%Y%m%d-%H%M%S') + '-', dir=str(BACKUPS)))
    paths = [BASE / name for name in SOURCES] + [BASE / MARKER, CONFIG_FILE, DATABASE, UNITS / SERVICE, UNITS / TIMER]
    previous = {p: p.read_bytes() if p.is_file() else None for p in paths}
    for i, p in enumerate(paths):
        if previous[p] is not None:
            (backup / f'file-{i}.before').write_bytes(previous[p])
    (backup / 'state.json').write_text(json.dumps({'active': old_active, 'enabled': old_enabled, 'paths': list(map(str, paths))}))
    created = [p for p in (BASE, CONFIG) if not p.exists()]
    try:
        for folder in (BASE, CONFIG):
            folder.mkdir(mode=0o755, parents=True, exist_ok=True)
        for name, source in SOURCES.items():
            atomic_write(BASE / name, source.read_bytes(), 0o644)
        atomic_write(BASE / MARKER, b'PoCol geography reporter v1\n')
        atomic_write(CONFIG_FILE, config)
        if database is not None:
            atomic_write(DATABASE, database)
        atomic_write(UNITS / SERVICE, service.encode())
        atomic_write(UNITS / TIMER, timer.encode())
        command('systemctl', 'daemon-reload')
        # One local run that reads the node and builds and signs a report, and sends nothing.
        run = command(PYTHON, '-I', '-B', str(BASE / 'pocol_geo_reporter.py'), '--config', str(CONFIG_FILE),
                      '--credential', str(CREDENTIAL), '--dry-run', check=False)
        if run.returncode != 0:
            # The reporter's one FAILED line (it never prints a secret or an IP).
            raise RuntimeError('the reporter dry run failed: ' + run.stderr.decode('utf-8', 'replace').strip()[-400:])
        report = run.stdout.decode('utf-8', 'replace').strip()[:20000]
        command('systemctl', 'enable', '--now', TIMER)
    except BaseException as exc:
        failures = []

        def attempt(label, action):
            try: action()
            except Exception as error: failures.append(f'{label}: {error}')
        attempt('stop the timer', lambda: command('systemctl', 'stop', TIMER, check=False))
        for path, old in previous.items():
            attempt('restore ' + str(path), lambda p=path, b=old: p.unlink(missing_ok=True) if b is None else atomic_write(p, b))
        for folder in reversed(created):
            attempt('remove ' + str(folder), lambda f=folder: f.rmdir() if f.exists() else None)
        attempt('reload the restored units', lambda: command('systemctl', 'daemon-reload'))
        if not old_enabled:
            attempt('restore the disabled boot state', lambda: command('systemctl', 'disable', TIMER, check=False))
        if old_active:
            attempt('restart the previous timer', lambda: command('systemctl', 'restart', TIMER))
        if failures:
            raise RuntimeError(f'Installation failed: {exc}; ROLLBACK INCOMPLETE: {"; ".join(failures)}; backup: {backup}') from exc
        raise RuntimeError(f'Installation failed and was rolled back: {exc}') from exc
    # The dry run's report, for the owner to review: the node's head, its own mining address, its peers' keyed ids and
    # countries; never an IP address (pocol_geo_reporter.build_report). Nothing of it was sent.
    print('Dry run report (not sent):')
    print(report)
    print(f'INSTALLED: {TIMER} (every 60 s) and {SERVICE}: {summary}.')
    print(f'Check its runs with: journalctl -u {SERVICE} -n 5  (each run prints one line: SENT, or FAILED and why).')
    print('Backup: ' + str(backup))


if __name__ == '__main__':
    try:
        main()
    except Exception as exc:
        print('INSTALL FAILED: ' + str(exc), file=sys.stderr)
        sys.exit(1)
