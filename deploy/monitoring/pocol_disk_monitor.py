#!/usr/bin/env python3
"""PoCol founder disk monitor (owner decision OWNER-DECISIONS-2026-11-21 D7).

Measures, on one founder server, every real mounted filesystem (space and inodes), the size of the PoCol data
directory and their growth, and sends alerts and a daily report off the host (Telegram Bot API over HTTPS and/or email
over SMTP with STARTTLS or SMTPS, certificates always verified). Python 3 standard library only.

It only reads and notifies. It never deletes, prunes, truncates or moves anything outside its own state directory,
never starts, stops or restarts a service, never changes a firewall rule and runs no external command: the only files
it writes are the state file (and its lock and temporary file) in its own state directory.
deploy/monitoring/test_disk_monitor.py checks that as a property of this source.

Usage (as root, as the timer runs it): --once, --status, --report, --test-alert, --simulate-usage PERCENT,
--notify-failure UNIT. See deploy/monitoring/README.md.
"""
import argparse
import fcntl
import json
import os
import re
import smtplib
import socket
import ssl
import stat
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from email.message import EmailMessage
from email.utils import formatdate, make_msgid

DEFAULT_CONFIG = '/etc/pocol-disk-monitor/monitor.conf'
DEFAULT_SECRETS = '/etc/pocol-disk-monitor/secret.env'
DEFAULT_STATE_DIR = '/var/lib/pocol-disk-monitor'
STATE_NAME = 'state.json'
STATE_TMP_NAME = 'state.json.tmp'
STATE_CORRUPT_NAME = 'state.json.corrupt'
LOCK_NAME = 'lock'
STATE_VERSION = 1
STATE_MAX_BYTES = 1_000_000      # a larger file is treated as corrupt
SAMPLES_MAX = 400                # about 26 h at 15 min plus hourly samples for 8 days
SCAN_MAX_ENTRIES = 200_000       # the PoCol data directory holds a few files; this bounds a pathological tree
LOCK_WAIT_SECONDS = 90

LEVELS = ('OK', 'WARNING', 'CRITICAL', 'EMERGENCY')
OK, WARNING, CRITICAL, EMERGENCY = range(4)
# Message kinds and their rank (the queue drops the lowest rank first when full).
RANK = {'REPORT': 0, 'INFO': 1, 'RECOVERED': 2, 'WARNING': 3, 'MONITOR FAILED': 4, 'CRITICAL': 5, 'EMERGENCY': 6,
        'TEST': 1}
ARABIC = {'EMERGENCY': 'طارئ', 'CRITICAL': 'حرج', 'WARNING': 'تحذير', 'RECOVERED': 'زال التنبيه', 'INFO': 'معلومة',
          'REPORT': 'التقرير اليومي', 'TEST': 'اختبار', 'MONITOR FAILED': 'تعطل نظام المراقبة'}

# Filesystems that hold no persistent data or cannot fill from PoCol's use (never measured).
PSEUDO_FS = frozenset('''autofs binfmt_misc bpf cgroup cgroup2 configfs debugfs devpts devtmpfs efivarfs fusectl
hugetlbfs mqueue nsfs overlay proc pstore ramfs rpc_pipefs securityfs selinuxfs squashfs sysfs tmpfs tracefs
fuse.lxcfs fuse.snapfuse fuse.gvfsd-fuse fuse.portal nfsd iso9660 udf'''.split())
SKIP_MOUNT_PREFIXES = ('/proc', '/sys', '/dev', '/run', '/snap')

# name: (type, default). Types: str, path, int, float, bool, list, choice:<a>|<b>.
SETTINGS = {
    'SERVER_NAME': ('str', ''),
    'SERVER_IP': ('str', ''),
    'POCOL_DATA_DIR': ('path', '/var/lib/pocol-standalone'),
    'POCOL_DATA_REQUIRED': ('bool', False),
    'DISK_WARNING_PERCENT': ('float', 70.0),
    'DISK_CRITICAL_PERCENT': ('float', 85.0),
    'DISK_EMERGENCY_PERCENT': ('float', 95.0),
    'INODE_WARNING_PERCENT': ('float', 70.0),
    'INODE_CRITICAL_PERCENT': ('float', 85.0),
    'INODE_EMERGENCY_PERCENT': ('float', 95.0),
    'PERCENT_HYSTERESIS': ('float', 2.0),
    'ETA_WARNING_DAYS': ('float', 30.0),
    'ETA_CRITICAL_DAYS': ('float', 7.0),
    'ETA_EMERGENCY_DAYS': ('float', 2.0),
    'ETA_HYSTERESIS_FACTOR': ('float', 1.2),
    'ETA_MIN_BYTES_PER_DAY': ('float', 1e6),
    'ACCEL_FACTOR': ('float', 3.0),
    'ACCEL_MIN_BYTES_PER_DAY': ('float', 500e6),
    'REALERT_WARNING_HOURS': ('float', 24.0),
    'REALERT_CRITICAL_HOURS': ('float', 6.0),
    'REALERT_EMERGENCY_HOURS': ('float', 1.0),
    'GAP_ALERT_MINUTES': ('float', 45.0),
    'DAILY_REPORT_HOUR': ('int', 8),
    'DAILY_REPORT_TZ': ('choice:UTC|local', 'UTC'),
    'CHANNELS': ('list', ['telegram', 'email']),
    'TELEGRAM_API_BASE': ('str', 'https://api.telegram.org'),
    'SMTP_HOST': ('str', ''),
    'SMTP_PORT': ('int', 587),
    'SMTP_SECURITY': ('choice:starttls|ssl', 'starttls'),
    'SMTP_USER': ('str', ''),
    'SMTP_FROM': ('str', ''),
    'SMTP_TO': ('list', []),
    'CA_FILE': ('path', ''),
    'NETWORK_TIMEOUT_SECONDS': ('float', 20.0),
    'MAX_MESSAGES_PER_HOUR': ('int', 12),
    'QUEUE_MAX': ('int', 50),
    'ARABIC_LINE': ('bool', True),
    'EXCLUDE_MOUNTS': ('list', []),
    'PEER_CHECK': ('bool', False),
    'PEERS': ('list', []),
    'PEER_FAIL_RUNS': ('int', 3),
}
SECRET_KEYS = ('TELEGRAM_BOT_TOKEN', 'TELEGRAM_CHAT_ID', 'SMTP_PASSWORD', 'HEARTBEAT_URL')
PLACEHOLDER = re.compile(r'(?i)^$|REPLACE|PLACEHOLDER|^<.*>$|^x+$|example\.invalid')
WINDOWS = (('1h', 3600, 2700), ('24h', 86400, 43200), ('7d', 7 * 86400, 2 * 86400))


class ConfigError(Exception):
    pass


class DeliveryError(Exception):
    pass


# ----------------------------------------------------------------------------------------------------------- config

def parse_env_text(text):
    """KEY=VALUE lines (comments with #, optional matching quotes). Returns (dict, [warnings])."""
    values, warnings = {}, []
    for number, raw in enumerate(text.splitlines(), 1):
        line = raw.strip()
        if not line or line.startswith('#'):
            continue
        if line.startswith('export '):
            line = line[len('export '):].strip()
        if '=' not in line:
            warnings.append(f'line {number}: not KEY=VALUE, ignored')
            continue
        key, value = line.split('=', 1)
        key, value = key.strip(), value.strip()
        if len(value) >= 2 and value[0] == value[-1] and value[0] in '"\'':
            value = value[1:-1]
        if not re.fullmatch(r'[A-Z][A-Z0-9_]*', key):
            warnings.append(f'line {number}: bad key name, ignored')
            continue
        values[key] = value
    return values, warnings


def convert(kind, raw):
    if kind in ('str', 'path'):
        return raw
    if kind == 'int':
        return int(raw)
    if kind == 'float':
        value = float(raw)
        if value != value or value in (float('inf'), float('-inf')):
            raise ValueError('not finite')
        return value
    if kind == 'bool':
        low = raw.lower()
        if low in ('1', 'yes', 'true', 'on'):
            return True
        if low in ('0', 'no', 'false', 'off', ''):
            return False
        raise ValueError('not a boolean')
    if kind == 'list':
        return [part.strip() for part in raw.split(',') if part.strip()]
    if kind.startswith('choice:'):
        options = kind[len('choice:'):].split('|')
        if raw not in options:
            raise ValueError('not one of ' + ', '.join(options))
        return raw
    raise AssertionError(kind)


class Config:
    """The non-secret settings (monitor.conf) and the secrets (secret.env). A bad value never stops the monitor: it
    is replaced by its default and reported (status, daily report, --test-alert)."""

    def __init__(self, conf_text='', secret_text='', secret_mode=None):
        self.warnings = []
        values, warns = parse_env_text(conf_text)
        self.warnings += [f'monitor.conf {w}' for w in warns]
        self.values = {name: default for name, (_, default) in SETTINGS.items()}
        for key, raw in values.items():
            if key in SECRET_KEYS:
                self.warnings.append(f'monitor.conf sets {key}, a secret: ignored there (put it in secret.env)')
                continue
            if key not in SETTINGS:
                self.warnings.append(f'monitor.conf: unknown setting {key} ignored')
                continue
            kind = SETTINGS[key][0]
            try:
                self.values[key] = convert(kind, raw)
            except ValueError as exc:
                self.warnings.append(f'monitor.conf: {key} has a bad value ({exc}); default used')
        self._check_order('DISK')
        self._check_order('INODE')
        eta = [self.values[f'ETA_{n}_DAYS'] for n in ('WARNING', 'CRITICAL', 'EMERGENCY')]
        if not eta[0] > eta[1] > eta[2] > 0:
            self.warnings.append('monitor.conf: ETA_*_DAYS must decrease from WARNING to EMERGENCY; defaults used')
            for n in ('WARNING', 'CRITICAL', 'EMERGENCY'):
                self.values[f'ETA_{n}_DAYS'] = SETTINGS[f'ETA_{n}_DAYS'][1]
        for key, low, high in (('DAILY_REPORT_HOUR', 0, 23), ('QUEUE_MAX', 1, 1000), ('MAX_MESSAGES_PER_HOUR', 1, 1000),
                               ('PEER_FAIL_RUNS', 1, 1000), ('SMTP_PORT', 1, 65535)):
            if not low <= self.values[key] <= high:
                self.warnings.append(f'monitor.conf: {key} must be in {low}..{high}; default used')
                self.values[key] = SETTINGS[key][1]
        for key in ('REALERT_WARNING_HOURS', 'REALERT_CRITICAL_HOURS', 'REALERT_EMERGENCY_HOURS',
                    'GAP_ALERT_MINUTES', 'NETWORK_TIMEOUT_SECONDS', 'ACCEL_FACTOR', 'ETA_HYSTERESIS_FACTOR'):
            if self.values[key] <= 0 or (key == 'ETA_HYSTERESIS_FACTOR' and self.values[key] < 1):
                self.warnings.append(f'monitor.conf: {key} must be positive; default used')
                self.values[key] = SETTINGS[key][1]
        secrets, warns = parse_env_text(secret_text)
        self.warnings += [f'secret.env {w}' for w in warns]
        self.secrets = {}
        for key, value in secrets.items():
            if key not in SECRET_KEYS:
                self.warnings.append(f'secret.env: unknown key {key} ignored')
            elif not PLACEHOLDER.search(value):
                self.secrets[key] = value
        self.secret_mode = secret_mode
        if secret_mode is not None and secret_mode & 0o077:
            self.warnings.append('secret.env is readable by group or others: run '
                                 '"sudo chmod 0600 /etc/pocol-disk-monitor/secret.env"')
        for key in ('TELEGRAM_API_BASE',):
            if not self.values[key].startswith('https://'):
                self.warnings.append(f'monitor.conf: {key} must be an https:// URL; default used')
                self.values[key] = SETTINGS[key][1]
        if 'HEARTBEAT_URL' in self.secrets and not self.secrets['HEARTBEAT_URL'].startswith('https://'):
            self.warnings.append('secret.env: HEARTBEAT_URL must be an https:// URL; heartbeat disabled')
            del self.secrets['HEARTBEAT_URL']
        for name in self.values['CHANNELS']:
            if name not in ('telegram', 'email'):
                self.warnings.append(f'monitor.conf: unknown channel {name} in CHANNELS ignored')

    def _check_order(self, prefix):
        names = [f'{prefix}_{n}_PERCENT' for n in ('WARNING', 'CRITICAL', 'EMERGENCY')]
        got = [self.values[n] for n in names]
        if not 0 < got[0] < got[1] < got[2] <= 100:
            self.warnings.append(f'monitor.conf: {prefix}_*_PERCENT must increase within 0..100; defaults used')
            for n in names:
                self.values[n] = SETTINGS[n][1]

    def __getitem__(self, key):
        return self.values[key]

    def secret_values(self):
        return [v for v in self.secrets.values() if v]

    @classmethod
    def load(cls, conf_path, secret_path, system):
        conf_text, secret_text, mode, notes = '', '', None, []
        try:
            conf_text = system.read_text(conf_path)
        except FileNotFoundError:
            notes.append(f'{conf_path} not found; defaults used')
        except OSError as exc:
            notes.append(f'{conf_path} unreadable ({exc.strerror}); defaults used')
        try:
            mode = stat.S_IMODE(system.stat(secret_path).st_mode)
            secret_text = system.read_text(secret_path)
        except FileNotFoundError:
            notes.append(f'{secret_path} not found; no channel has credentials')
        except OSError as exc:
            notes.append(f'{secret_path} unreadable ({exc.strerror}); no channel has credentials')
        config = cls(conf_text, secret_text, mode)
        config.warnings = notes + config.warnings
        return config


class Redactor:
    TOKEN = re.compile(r'bot\d+:[A-Za-z0-9_-]+')

    def __init__(self, secrets):
        self.secrets = sorted({s for s in secrets if len(s) >= 4}, key=len, reverse=True)

    def __call__(self, text):
        text = str(text)
        for secret in self.secrets:
            text = text.replace(secret, '[redacted]')
            quoted = urllib.parse.quote(secret, safe='')
            if quoted != secret:
                text = text.replace(quoted, '[redacted]')
        return self.TOKEN.sub('bot[redacted]', text)


# ------------------------------------------------------------------------------------------------------- the system

class System:
    """Everything the monitor reads from the host, in one seam the tests replace."""

    def now(self):
        return time.time()

    def read_text(self, path):
        with open(path, encoding='utf-8') as handle:
            return handle.read(STATE_MAX_BYTES + 1)

    def read_mountinfo(self):
        return self.read_text('/proc/self/mountinfo')

    def statvfs(self, path):
        return os.statvfs(path)

    def stat(self, path):
        return os.stat(path)

    def lstat(self, path):
        return os.lstat(path)

    def scandir(self, path):
        return os.scandir(path)

    def hostname(self):
        return socket.gethostname()

    def primary_ip(self):
        # A UDP connect sends no packet; it only selects the source address of the default route.
        try:
            with socket.socket(socket.AF_INET, socket.SOCK_DGRAM) as probe:
                probe.connect(('192.0.2.1', 9))
                return probe.getsockname()[0]
        except OSError:
            return ''

    def tcp_reachable(self, host, port, timeout):
        try:
            with socket.create_connection((host, port), timeout=timeout):
                return True
        except OSError:
            return False


# ---------------------------------------------------------------------------------------------------- measurements

def unescape_mount(text):
    return re.sub(r'\\([0-7]{3})', lambda m: chr(int(m.group(1), 8)), text)


def parse_mountinfo(text):
    """[(device 'major:minor', root, mount point, fs type)] from /proc/self/mountinfo."""
    mounts = []
    for line in text.splitlines():
        left, sep, right = line.partition(' - ')
        fields, tail = left.split(), right.split()
        if not sep or len(fields) < 5 or not tail:
            continue
        mounts.append((fields[2], unescape_mount(fields[3]), unescape_mount(fields[4]), tail[0]))
    return mounts


def real_mounts(mounts, exclude=()):
    """One mount point per device for every real filesystem: the shortest mount of the device's root, else the
    shortest mount (bind mounts, such as the ones systemd's sandboxing adds, share the device)."""
    chosen = {}
    for device, root, point, fstype in mounts:
        if fstype in PSEUDO_FS or point in exclude:
            continue
        if any(point == p or point.startswith(p + '/') for p in SKIP_MOUNT_PREFIXES):
            continue
        key = (root != '/', len(point), point)
        if device not in chosen or key < chosen[device][0]:
            chosen[device] = (key, point, fstype)
    return sorted((point, fstype, device) for device, (_, point, fstype) in chosen.items())


def measure_filesystem(system, point, fstype, device):
    st = system.statvfs(point)
    if st.f_blocks == 0:
        return None
    size = st.f_blocks * st.f_frsize
    free = st.f_bfree * st.f_frsize
    avail = st.f_bavail * st.f_frsize
    used = size - free
    usable = used + avail
    percent = used * 100.0 / usable if usable > 0 else 0.0
    inodes = st.f_files
    iused = inodes - st.f_ffree if inodes else 0
    ipercent = iused * 100.0 / inodes if inodes > 0 else None
    return {'mount': point, 'fstype': fstype, 'device': device, 'size': size, 'used': used, 'avail': avail,
            'percent': round(percent, 2), 'inodes': inodes, 'inodes_used': iused,
            'inode_percent': None if ipercent is None else round(ipercent, 2)}


def category(name):
    if name in ('history.redb', 'state.redb', 'canon.redb'):
        return name
    if 'journal' in name:
        return 'journal'
    return 'other'


def scan_data_dir(system, root):
    """Real disk use (st_blocks) of the PoCol data directory, by part. Reads only directory entries and metadata,
    never a file's content; follows no symbolic link; counts a hard-linked file once."""
    result = {'path': root, 'exists': False, 'bytes': 0, 'apparent': 0, 'files': 0, 'unreadable': 0,
              'truncated': False, 'parts': {'journal': 0, 'history.redb': 0, 'state.redb': 0, 'canon.redb': 0,
                                            'other': 0}}
    try:
        top = system.lstat(root)
    except FileNotFoundError:
        return result
    except OSError:
        result['unreadable'] += 1
        return result
    if not stat.S_ISDIR(top.st_mode):
        result['unreadable'] += 1
        return result
    result['exists'] = True
    seen, stack, entries = set(), [root], 0
    while stack:
        directory = stack.pop()
        try:
            iterator = system.scandir(directory)
        except OSError:
            result['unreadable'] += 1
            continue
        with iterator:
            for entry in iterator:
                entries += 1
                if entries > SCAN_MAX_ENTRIES:
                    result['truncated'] = True
                    stack.clear()
                    break
                try:
                    st = entry.stat(follow_symlinks=False)
                except OSError:
                    result['unreadable'] += 1
                    continue
                if st.st_nlink > 1 and not stat.S_ISDIR(st.st_mode):
                    if (st.st_dev, st.st_ino) in seen:
                        continue
                    seen.add((st.st_dev, st.st_ino))
                blocks = getattr(st, 'st_blocks', None)
                real = blocks * 512 if blocks is not None else st.st_size
                result['bytes'] += real
                result['apparent'] += st.st_size
                if stat.S_ISDIR(st.st_mode):
                    result['parts']['other'] += real
                    stack.append(entry.path)
                else:
                    result['files'] += 1
                    result['parts'][category(entry.name)] += real
    return result


def device_of(system, path):
    """'major:minor' of the filesystem holding path (or its nearest existing parent)."""
    while True:
        try:
            st = system.stat(path)
            return f'{os.major(st.st_dev)}:{os.minor(st.st_dev)}'
        except FileNotFoundError:
            parent = os.path.dirname(path.rstrip('/')) or '/'
            if parent == path:
                return None
            path = parent
        except OSError:
            return None


def measure(system, config):
    warnings = []
    try:
        mounts = parse_mountinfo(system.read_mountinfo())
    except OSError as exc:
        warnings.append(f'mountinfo unreadable ({exc.strerror}); measuring / only')
        mounts = []
    points = real_mounts(mounts, set(config['EXCLUDE_MOUNTS'])) if mounts else [('/', 'unknown', device_of(system, '/'))]
    filesystems = []
    for point, fstype, device in points:
        try:
            fs = measure_filesystem(system, point, fstype, device)
        except OSError as exc:
            warnings.append(f'{point}: statvfs failed ({exc.strerror})')
            continue
        if fs:
            filesystems.append(fs)
    if not filesystems:
        raise RuntimeError('no filesystem could be measured')
    data = scan_data_dir(system, config['POCOL_DATA_DIR'])
    data_device = device_of(system, config['POCOL_DATA_DIR'])
    data_mount = next((fs['mount'] for fs in filesystems if fs['device'] == data_device), None)
    data['mount'] = data_mount or ('/' if any(fs['mount'] == '/' for fs in filesystems) else filesystems[0]['mount'])
    return {'t': system.now(), 'filesystems': filesystems, 'data': data, 'warnings': warnings}


# -------------------------------------------------------------------------------------------------- growth and ETA

def compact_samples(samples, now):
    """Every sample of the last 26 h, then one per hour for 8 days; nothing older; at most SAMPLES_MAX."""
    recent, hourly, seen_hours = [], [], set()
    for sample in sorted(samples, key=lambda s: s['t']):
        age = now - sample['t']
        if age < 0 or age > 8 * 86400:
            continue
        if age <= 26 * 3600:
            recent.append(sample)
        else:
            hour = int(sample['t'] // 3600)
            if hour not in seen_hours:
                seen_hours.add(hour)
                hourly.append(sample)
    kept = hourly + recent
    return kept[-SAMPLES_MAX:]


def rate(samples, now, value, getter, window, min_span):
    """Bytes per day between the oldest sample inside the window and now, or None without enough history."""
    oldest = None
    for sample in samples:
        if now - window <= sample['t'] < now:
            old = getter(sample)
            if old is not None and (oldest is None or sample['t'] < oldest[0]):
                oldest = (sample['t'], old)
    if oldest is None or value is None or now - oldest[0] < min_span:
        return None
    return (value - oldest[1]) * 86400.0 / (now - oldest[0])


def growth(samples, now, value, getter):
    return {name: rate(samples, now, value, getter, window, span) for name, window, span in WINDOWS}


def eta_days(avail, rates, config):
    """Days until full at the fastest of the 24 h and 7 d rates (the 1 h rate is too noisy for an ETA: a store file
    can grow in one step). None while there is not enough history or nothing is filling."""
    candidates = [r for r in (rates.get('24h'), rates.get('7d')) if r is not None]
    if not candidates:
        return None
    fastest = max(candidates)
    if fastest < config['ETA_MIN_BYTES_PER_DAY']:
        return None
    return avail / fastest


# ------------------------------------------------------------------------------------------------- levels (state)

def level_high(value, thresholds, previous, hysteresis):
    """The level of a higher-is-worse value; an active level clears only `hysteresis` below its threshold."""
    level = OK
    if value is None:
        return OK
    for lvl, threshold in enumerate(thresholds, 1):
        if value >= threshold or (previous >= lvl and value >= threshold - hysteresis):
            level = lvl
    return level


def level_low(value, thresholds, previous, factor):
    """The level of a lower-is-worse value (days to full); an active level clears only above threshold x factor."""
    level = OK
    if value is None:
        return OK
    for lvl, threshold in enumerate(thresholds, 1):
        if value < threshold or (previous >= lvl and value < threshold * factor):
            level = lvl
    return level


def sanitize_state(state, base):
    """A state read from disk, with every item of an unexpected shape dropped, so a damaged but parseable file never
    stops the monitor."""
    num = (int, float)
    is_num = lambda v: isinstance(v, num) and not isinstance(v, bool) and v == v
    clean = base
    if isinstance(state.get('samples'), list):
        clean['samples'] = [
            {'t': s['t'], 'fs': {k: v for k, v in s['fs'].items() if isinstance(k, str) and is_num(v)},
             'p': s.get('p') if is_num(s.get('p')) else None}
            for s in state['samples'][-SAMPLES_MAX:]
            if isinstance(s, dict) and is_num(s.get('t')) and isinstance(s.get('fs'), dict)]
    if isinstance(state.get('alerts'), dict):
        clean['alerts'] = {k: {'level': a['level'], 'since': a['since'], 'last_sent': a['last_sent']}
                           for k, a in state['alerts'].items()
                           if isinstance(k, str) and isinstance(a, dict) and a.get('level') in (WARNING, CRITICAL, EMERGENCY)
                           and is_num(a.get('since')) and is_num(a.get('last_sent'))}
    if isinstance(state.get('queue'), dict):
        clean['queue'] = {n: [{'t': q['t'], 'kind': q['kind'], 'subject': q['subject'], 'text': q['text'],
                               'attempts': q.get('attempts') if is_num(q.get('attempts')) else 0}
                              for q in queue[-1000:]
                              if isinstance(q, dict) and is_num(q.get('t')) and q.get('kind') in RANK
                              and isinstance(q.get('subject'), str) and isinstance(q.get('text'), str)]
                          for n, queue in state['queue'].items() if isinstance(n, str) and isinstance(queue, list)}
    if isinstance(state.get('sent'), dict):
        clean['sent'] = {n: [t for t in ts[-1000:] if is_num(t)] for n, ts in state['sent'].items()
                         if isinstance(n, str) and isinstance(ts, list)}
    for key in ('dropped', 'peers'):
        if isinstance(state.get(key), dict):
            clean[key] = {n: int(v) for n, v in state[key].items() if isinstance(n, str) and is_num(v)}
    if is_num(state.get('last_success')):
        clean['last_success'] = state['last_success']
    if isinstance(state.get('last_report_date'), str):
        clean['last_report_date'] = state['last_report_date']
    if isinstance(state.get('announced'), bool):
        clean['announced'] = state['announced']
    if isinstance(state.get('notes'), list):
        clean['notes'] = [n for n in state['notes'][-20:] if isinstance(n, str)]
    return clean


def fmt_bytes(n):
    if n is None:
        return 'n/a'
    sign = '-' if n < 0 else ''
    n = abs(n)
    for unit, size in (('TB', 1e12), ('GB', 1e9), ('MB', 1e6), ('kB', 1e3)):
        if n >= size:
            return f'{sign}{n / size:.1f} {unit}'
    return f'{sign}{int(n)} B'


def fmt_rate(r):
    if r is None:
        return 'n/a'
    return ('+' if r >= 0 else '') + fmt_bytes(r) + '/day'


def fmt_days(d):
    if d is None:
        return 'n/a (not filling, or under 12 h of history)'
    if d > 3650:
        return 'more than 10 years'
    return f'about {d:.1f} days'


def fmt_duration(seconds):
    minutes = int(seconds // 60)
    hours, minutes = divmod(minutes, 60)
    days, hours = divmod(hours, 24)
    if days:
        return f'{days} d {hours} h'
    if hours:
        return f'{hours} h {minutes} min'
    return f'{minutes} min'


class Monitor:
    def __init__(self, config, system=None, state_dir=DEFAULT_STATE_DIR, channels=None, out=None, heartbeat=None):
        self.config = config
        self.system = system or System()
        self.state_dir = state_dir
        self.redact = Redactor(config.secret_values())
        self.out = out or sys.stderr
        transport = HttpsTransport(config['CA_FILE'], config['NETWORK_TIMEOUT_SECONDS'])
        self.channels = channels if channels is not None else build_channels(config, transport)
        self.heartbeat = heartbeat if heartbeat is not None else transport

    # ------------------------------------------------------------------ logging and identity
    def log(self, text):
        print(self.redact(text), file=self.out, flush=True)

    def identity(self):
        host = self.system.hostname()
        name = self.config['SERVER_NAME'] or host
        ip = self.config['SERVER_IP'] or self.system.primary_ip()
        label = name if name == host else f'{name} [{host}]'
        return f'{label} ({ip})' if ip else label

    def stamp(self, t):
        if self.config['DAILY_REPORT_TZ'] == 'local':
            return time.strftime('%Y-%m-%d %H:%M %Z', time.localtime(t))
        return time.strftime('%Y-%m-%d %H:%M UTC', time.gmtime(t))

    # ------------------------------------------------------------------ state file (the only writes)
    def state_path(self, name):
        path = os.path.join(self.state_dir, name)
        if os.path.dirname(path) != self.state_dir.rstrip('/') or name not in (STATE_NAME, STATE_TMP_NAME,
                                                                               STATE_CORRUPT_NAME, LOCK_NAME):
            raise AssertionError('a write outside the state directory')
        return path

    def lock(self):
        """An exclusive lock on the state directory's lock file, so a manual run and the timer never interleave."""
        fd = os.open(self.state_path(LOCK_NAME), os.O_RDWR | os.O_CREAT, 0o600)
        deadline = time.monotonic() + LOCK_WAIT_SECONDS
        while True:
            try:
                fcntl.flock(fd, fcntl.LOCK_EX | fcntl.LOCK_NB)
                return fd
            except BlockingIOError:
                if time.monotonic() > deadline:
                    os.close(fd)
                    raise RuntimeError('another run holds the lock')
                time.sleep(1)

    @staticmethod
    def unlock(fd):
        fcntl.flock(fd, fcntl.LOCK_UN)
        os.close(fd)

    def new_state(self):
        return {'v': STATE_VERSION, 'samples': [], 'alerts': {}, 'queue': {}, 'sent': {}, 'dropped': {},
                'last_success': None, 'last_report_date': None, 'announced': False, 'peers': {}, 'notes': []}

    def load_state(self):
        path = self.state_path(STATE_NAME)
        try:
            size = os.stat(path).st_size
        except FileNotFoundError:
            return self.new_state()
        problem = None
        if size > STATE_MAX_BYTES:
            problem = f'too large ({size} B)'
        else:
            try:
                with open(path, encoding='utf-8') as handle:
                    state = json.load(handle)
                if not isinstance(state, dict) or state.get('v') != STATE_VERSION:
                    problem = 'unknown format'
                else:
                    return sanitize_state(state, self.new_state())
            except (OSError, ValueError) as exc:
                problem = f'unreadable ({type(exc).__name__})'
        os.replace(path, self.state_path(STATE_CORRUPT_NAME))
        state = self.new_state()
        state['notes'].append(f'The state file was {problem}; it was set aside as {STATE_CORRUPT_NAME} in '
                              f'{self.state_dir} and a new one started (alert history reset).')
        self.log(f'warning: state file {problem}; started a new one')
        return state

    def save_state(self, state):
        data = json.dumps(state, separators=(',', ':'), sort_keys=True).encode('utf-8')
        if len(data) > STATE_MAX_BYTES:
            # Never expected (every list is bounded); shed the oldest samples and queued reports first.
            state['samples'] = state['samples'][-100:]
            data = json.dumps(state, separators=(',', ':'), sort_keys=True).encode('utf-8')
        tmp = self.state_path(STATE_TMP_NAME)
        fd = os.open(tmp, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
        try:
            os.write(fd, data)
            os.fsync(fd)
        finally:
            os.close(fd)
        os.replace(tmp, self.state_path(STATE_NAME))
        dir_fd = os.open(self.state_dir, os.O_RDONLY)
        try:
            os.fsync(dir_fd)
        finally:
            os.close(dir_fd)

    # ------------------------------------------------------------------ evaluation
    def analyse(self, m, samples):
        """Growth rates and ETA for every filesystem and the PoCol data directory."""
        now = m['t']
        for fs in m['filesystems']:
            point = fs['mount']
            fs['rates'] = growth(samples, now, fs['used'], lambda s, p=point: s.get('fs', {}).get(p))
            fs['eta_days'] = eta_days(fs['avail'], fs['rates'], self.config)
        data = m['data']
        data['rates'] = growth(samples, now, data['bytes'] if data['exists'] else None, lambda s: s.get('p'))
        return m

    def conditions(self, m, alerts, peers=None):
        """{key: (level, line)} for every condition, active or not."""
        c, out = self.config, {}
        hyst = c['PERCENT_HYSTERESIS']
        prev = lambda key: alerts.get(key, {}).get('level', OK)
        for fs in m['filesystems']:
            point = fs['mount']
            key = f'disk:{point}'
            level = level_high(fs['percent'], [c['DISK_WARNING_PERCENT'], c['DISK_CRITICAL_PERCENT'],
                                               c['DISK_EMERGENCY_PERCENT']], prev(key), hyst)
            out[key] = (level, f'disk {point} {fs["percent"]:.1f}% used, {fmt_bytes(fs["avail"])} free of '
                               f'{fmt_bytes(fs["size"])}')
            if fs['inode_percent'] is not None:
                key = f'inode:{point}'
                level = level_high(fs['inode_percent'], [c['INODE_WARNING_PERCENT'], c['INODE_CRITICAL_PERCENT'],
                                                         c['INODE_EMERGENCY_PERCENT']], prev(key), hyst)
                out[key] = (level, f'inodes {point} {fs["inode_percent"]:.1f}% used '
                                   f'({fs["inodes_used"]:,} of {fs["inodes"]:,})')
            key = f'eta:{point}'
            level = level_low(fs['eta_days'], [c['ETA_WARNING_DAYS'], c['ETA_CRITICAL_DAYS'], c['ETA_EMERGENCY_DAYS']],
                              prev(key), c['ETA_HYSTERESIS_FACTOR'])
            out[key] = (level, f'disk {point} full in {fmt_days(fs["eta_days"])} at {fmt_rate(self.eta_rate(fs))}')
            out[f'accel:{point}'] = self.acceleration(f'accel:{point}', f'disk {point}', fs['rates'], prev)
        data = m['data']
        out['accel:pocol'] = self.acceleration('accel:pocol', 'PoCol data', data['rates'], prev)
        if c['POCOL_DATA_REQUIRED']:
            out['pocol:missing'] = (CRITICAL if not data['exists'] else OK,
                                    f'PoCol data directory {data["path"]} '
                                    + ('is missing or unreadable' if not data['exists'] else 'is present'))
        for peer, failures in (peers or {}).items():
            key = f'peer:{peer}'
            out[key] = (CRITICAL if failures >= c['PEER_FAIL_RUNS'] else OK,
                        f'founder peer {peer} TCP ' + (f'unreachable for {failures} consecutive checks'
                                                       if failures else 'reachable'))
        return out

    def eta_rate(self, fs):
        rates = [r for r in (fs['rates'].get('24h'), fs['rates'].get('7d')) if r is not None]
        return max(rates) if rates else None

    def acceleration(self, key, what, rates, prev):
        c = self.config
        day, week = rates.get('24h'), rates.get('7d')
        line = f'{what} growth {fmt_rate(day)} (24 h) against {fmt_rate(week)} (7 d)'
        if day is None or week is None:
            return OK, line
        active = prev(key) >= WARNING
        factor = c['ACCEL_FACTOR'] * (0.8 if active else 1.0)
        minimum = c['ACCEL_MIN_BYTES_PER_DAY'] * (0.8 if active else 1.0)
        accelerating = day >= minimum and day >= factor * max(week, 0.0)
        return (WARNING if accelerating else OK), line + (' (accelerating)' if accelerating else '')

    def transitions(self, conditions, alerts, now):
        """Alert events: a level entered, eased or cleared, and re-alerts while a level persists."""
        c, events = self.config, []
        interval = {WARNING: c['REALERT_WARNING_HOURS'], CRITICAL: c['REALERT_CRITICAL_HOURS'],
                    EMERGENCY: c['REALERT_EMERGENCY_HOURS']}
        for key in sorted(set(conditions) | set(alerts)):
            level, line = conditions.get(key, (OK, f'{key.split(":", 1)[-1]} is no longer measured'))
            entry = alerts.get(key)
            previous = entry['level'] if entry else OK
            if level > previous:
                events.append((LEVELS[level], f'{LEVELS[level]}: {line}'))
                alerts[key] = {'level': level, 'since': now if not entry else entry['since'], 'last_sent': now}
            elif level < previous and level > OK:
                events.append((LEVELS[level], f'EASED to {LEVELS[level]} (was {LEVELS[previous]}): {line}'))
                alerts[key] = {'level': level, 'since': entry['since'], 'last_sent': now}
            elif level == OK and previous > OK:
                events.append(('RECOVERED', f'RECOVERED (was {LEVELS[previous]} since {self.stamp(entry["since"])}): '
                                            f'{line}'))
                del alerts[key]
            elif level > OK and now - entry['last_sent'] >= interval[level] * 3600 - 60:
                events.append((LEVELS[level], f'STILL {LEVELS[level]} since {self.stamp(entry["since"])}: {line}'))
                entry['last_sent'] = now
        return events

    # ------------------------------------------------------------------ messages
    def compose(self, events, m, now, test=False):
        kinds = [k for k, _ in events]
        top = max(kinds, key=lambda k: RANK[k])
        tag = f'[TEST] [{top}]' if test else f'[{top}]'
        subject = f'{tag} PoCol disk monitor: {self.identity()}'
        lines = [subject] + [f'- {line}' for _, line in events]
        if m is not None:
            lines.append(self.summary_line(m))
        if any(RANK[k] >= RANK['WARNING'] and k != 'MONITOR FAILED' for k in kinds):
            lines.append('The monitor deletes nothing and stops nothing; disk expansion or any data action needs '
                         'a separate owner decision (D7).')
        if test:
            lines.append('TEST ONLY: simulated values; the real state is unchanged.')
        lines.append(f'Time: {self.stamp(now)}')
        if self.config['ARABIC_LINE']:
            lines.append(f'PoCol: {ARABIC.get(top, top)}' + (' (اختبار)' if test and top != 'TEST' else '')
                         + f' - {self.identity()}')
        return top, subject, '\n'.join(lines)

    def summary_line(self, m):
        parts = []
        data = m['data']
        target = next((fs for fs in m['filesystems'] if fs['mount'] == data.get('mount')), m['filesystems'][0])
        parts.append(f'{target["mount"]}: {target["percent"]:.1f}% used, {fmt_bytes(target["avail"])} free, '
                     f'growth {fmt_rate(target.get("rates", {}).get("24h"))} (24 h), full in '
                     f'{fmt_days(target.get("eta_days"))}')
        parts.append(f'PoCol data {fmt_bytes(data["bytes"]) if data["exists"] else "absent"}')
        return '; '.join(parts)

    def report(self, m, state, now):
        """The daily report: usage, inodes, growth, remaining space, days to full, PoCol data, monitor health."""
        when = self.stamp(now)
        lines = [f'[REPORT] PoCol disk daily report {when}: {self.identity()}']
        for fs in m['filesystems']:
            r = fs.get('rates', {})
            inode = f'{fs["inode_percent"]:.1f}%' if fs['inode_percent'] is not None else 'n/a'
            lines.append(f'{fs["mount"]} ({fs["fstype"]}): {fs["percent"]:.1f}% used, {fmt_bytes(fs["used"])} used, '
                         f'{fmt_bytes(fs["avail"])} free of {fmt_bytes(fs["size"])}; inodes {inode}; growth '
                         f'1 h {fmt_rate(r.get("1h"))}, 24 h {fmt_rate(r.get("24h"))}, 7 d {fmt_rate(r.get("7d"))}; '
                         f'full in {fmt_days(fs.get("eta_days"))}')
        data = m['data']
        if data['exists']:
            parts = ', '.join(f'{k} {fmt_bytes(v)}' for k, v in data['parts'].items())
            r = data.get('rates', {})
            lines.append(f'PoCol data {data["path"]}: {fmt_bytes(data["bytes"])} on disk ({parts}); growth '
                         f'24 h {fmt_rate(r.get("24h"))}, 7 d {fmt_rate(r.get("7d"))}'
                         + (f'; {data["unreadable"]} entries unreadable' if data['unreadable'] else ''))
        else:
            lines.append(f'PoCol data {data["path"]}: not present (before install, or unreadable)')
        active = [f'{LEVELS[a["level"]]} {k} since {self.stamp(a["since"])}' for k, a in sorted(state['alerts'].items())]
        lines.append('Active alerts: ' + ('; '.join(active) if active else 'none'))
        queued = sum(len(q) for q in state['queue'].values())
        lines.append(f'Monitor: last successful check {self.stamp(state["last_success"]) if state["last_success"] else "this one"}; '
                     f'{queued} message(s) waiting for delivery; channels: {", ".join(self.channels) or "none"}')
        for warning in self.config.warnings + m.get('warnings', []):
            lines.append(f'Config/measurement warning: {warning}')
        lines.append('A missing daily report means the monitor or the host is down.')
        if self.config['ARABIC_LINE']:
            lines.append(f'PoCol: {ARABIC["REPORT"]} - {self.identity()}')
        return 'REPORT', lines[0], '\n'.join(lines)

    # ------------------------------------------------------------------ queue and delivery
    def enqueue(self, state, kind, subject, text, now):
        if not self.channels:
            self.log(f'no channel configured; message not sent: {subject}')
            return
        for name in self.channels:
            queue = state['queue'].setdefault(name, [])
            queue.append({'t': now, 'kind': kind, 'subject': self.redact(subject), 'text': self.redact(text),
                          'attempts': 0})
            while len(queue) > self.config['QUEUE_MAX']:
                victim = min(range(len(queue)), key=lambda i: (RANK.get(queue[i]['kind'], 0), queue[i]['t']))
                queue.pop(victim)
                state['dropped'][name] = state['dropped'].get(name, 0) + 1

    def deliver(self, state, now):
        """Send each channel's queue oldest first, within the hourly limit; a failure stops that channel until the
        next run (the message stays queued)."""
        failures = []
        for name, channel in self.channels.items():
            queue = state['queue'].setdefault(name, [])
            sent = [t for t in state['sent'].get(name, []) if now - 3600 < t <= now]
            while queue:
                if len(sent) >= self.config['MAX_MESSAGES_PER_HOUR']:
                    self.log(f'{name}: hourly limit reached; {len(queue)} message(s) wait for the next run')
                    break
                message = queue[0]
                text = message['text']
                dropped = state['dropped'].get(name, 0)
                if dropped:
                    text += f'\n({dropped} older message(s) were dropped from the full queue)'
                try:
                    channel.send(message['subject'], text)
                except Exception as exc:  # any failure of a channel is reported and retried, never fatal
                    message['attempts'] += 1
                    failures.append(name)
                    self.log(f'{name}: delivery failed ({self.redact(exc)}); {len(queue)} message(s) queued for retry')
                    break
                queue.pop(0)
                sent.append(now)
                state['dropped'][name] = 0
                self.log(f'{name}: sent {message["subject"]}')
            state['sent'][name] = sent
        return failures

    def ping_heartbeat(self, state):
        url = self.config.secrets.get('HEARTBEAT_URL')
        if not url:
            return None
        pending = sum(len(q) for q in state['queue'].values())
        target = url.rstrip('/') + '/fail' if pending else url
        try:
            status, _ = self.heartbeat('GET', target, None, {}, self.config['NETWORK_TIMEOUT_SECONDS'])
            ok = 200 <= status < 300
        except Exception as exc:
            self.log(f'heartbeat failed ({self.redact(exc)})')
            return False
        if not ok:
            self.log(f'heartbeat HTTP {status}')
        return ok

    # ------------------------------------------------------------------ the timer's run
    def run_once(self):
        fd = self.lock()
        try:
            return self._run_once()
        finally:
            self.unlock(fd)

    def _run_once(self):
        state = self.load_state()
        now = self.system.now()
        m = self.analyse(measure(self.system, self.config), state['samples'])
        events = []
        for note in state['notes']:
            events.append(('WARNING', note))
        state['notes'] = []
        last = state['last_success']
        gap = self.config['GAP_ALERT_MINUTES'] * 60
        if last is not None and now - last > gap:
            events.append(('WARNING', f'MONITORING GAP: no successful check for {fmt_duration(now - last)} (last at '
                                      f'{self.stamp(last)}): the timer was stopped, the monitor failed or the host '
                                      f'was down.'))
        if not state['announced'] and self.channels:
            # The first run with a channel: announce, and re-send every condition already active (it may have been
            # recorded while no channel was configured).
            events.append(('INFO', 'Monitoring active: checks every 15 minutes; daily report at '
                                   f'{self.config["DAILY_REPORT_HOUR"]:02d}:00 {self.config["DAILY_REPORT_TZ"]}.'))
            for entry in state['alerts'].values():
                entry['last_sent'] = 0
            state['announced'] = True
        peers = self.check_peers(state)
        events += self.transitions(self.conditions(m, state['alerts'], peers), state['alerts'], now)
        if events:
            kind, subject, text = self.compose(events, m, now)
            self.log(text.replace('\n', ' | '))
            self.enqueue(state, kind, subject, text, now)
        if self.report_due(state, now):
            kind, subject, text = self.report(m, state, now)
            self.enqueue(state, kind, subject, text, now)
            state['last_report_date'] = self.report_date(now)[0]
        sample = {'t': now, 'fs': {fs['mount']: fs['used'] for fs in m['filesystems']},
                  'p': m['data']['bytes'] if m['data']['exists'] else None}
        state['samples'] = compact_samples(state['samples'] + [sample], now)
        state['last_success'] = now
        # Saved before and after delivery; a failed save (a full disk is the case this monitor exists for) never
        # holds back the alert: the state stays in memory for this run and the error is logged.
        saved = self.try_save(state)
        failures = self.deliver(state, now)
        saved = self.try_save(state) or saved
        beat = self.ping_heartbeat(state)
        main = next((fs for fs in m['filesystems'] if fs['mount'] == m['data']['mount']), m['filesystems'][0])
        self.log(f'ok: {main["mount"]} {main["percent"]:.1f}% used, {fmt_bytes(main["avail"])} free; PoCol data '
                 f'{fmt_bytes(m["data"]["bytes"]) if m["data"]["exists"] else "absent"}; {len(events)} event(s); '
                 f'queued {sum(len(q) for q in state["queue"].values())}; delivery failures {len(failures)}; '
                 f'heartbeat {"off" if beat is None else ("ok" if beat else "failed")}')
        for warning in m['warnings']:
            self.log(f'warning: {warning}')
        if not saved:
            self.log('warning: the state file could not be saved; alert history will repeat next run')
        return 0

    def try_save(self, state):
        try:
            self.save_state(state)
            return True
        except OSError as exc:
            self.log(f'warning: saving the state failed ({exc.strerror or exc})')
            return False

    def report_date(self, now):
        t = time.localtime(now) if self.config['DAILY_REPORT_TZ'] == 'local' else time.gmtime(now)
        return time.strftime('%Y-%m-%d', t), t.tm_hour

    def report_due(self, state, now):
        date, hour = self.report_date(now)
        return hour >= self.config['DAILY_REPORT_HOUR'] and state['last_report_date'] != date

    def check_peers(self, state):
        if not self.config['PEER_CHECK']:
            state['peers'] = {}
            return {}
        failures = {}
        for peer in self.config['PEERS']:
            host, _, port = peer.rpartition(':')
            try:
                port = int(port)
            except ValueError:
                continue
            ok = self.system.tcp_reachable(host, port, min(self.config['NETWORK_TIMEOUT_SECONDS'], 10))
            failures[peer] = 0 if ok else state['peers'].get(peer, 0) + 1
        state['peers'] = failures
        return failures

    # ------------------------------------------------------------------ the owner's commands
    def send_direct(self, subject, text):
        """Send one message through every configured channel now (no queue); {channel: error or None}."""
        results = {}
        for name, channel in self.channels.items():
            try:
                channel.send(subject, text)
                results[name] = None
            except Exception as exc:
                results[name] = self.redact(exc)
        return results

    def test_alert(self):
        now = self.system.now()
        if self.config.secret_mode is not None and self.config.secret_mode & 0o077:
            self.log('FAILED: secret.env is readable by group or others; run "sudo chmod 0600 '
                     '/etc/pocol-disk-monitor/secret.env" and try again')
            return 1
        if not self.channels:
            self.log('FAILED: no channel is configured (see /etc/pocol-disk-monitor/secret.env and monitor.conf)')
            for warning in self.config.warnings:
                self.log(f'warning: {warning}')
            return 2
        subject = f'[TEST] PoCol disk monitor test notification: {self.identity()}'
        text = '\n'.join([subject, 'If you received this, this channel delivers the monitor\'s alerts.',
                          f'Time: {self.stamp(now)}'] +
                         ([f'PoCol: {ARABIC["TEST"]} - {self.identity()}'] if self.config['ARABIC_LINE'] else []))
        return self.report_results(self.send_direct(subject, text), check_heartbeat=True)

    def report_results(self, results, check_heartbeat=False):
        code = 0
        for name, error in results.items():
            self.log(f'{name}: ' + ('delivered' if error is None else f'FAILED ({error})'))
            if error is not None:
                code = 1
        if check_heartbeat and self.config.secrets.get('HEARTBEAT_URL'):
            beat = self.ping_heartbeat({'queue': {}})
            self.log('heartbeat: ' + ('ok' if beat else 'FAILED'))
            if not beat:
                code = 1
        for warning in self.config.warnings:
            self.log(f'warning: {warning}')
        return code

    def simulate(self, percent):
        """The whole alert path at a simulated usage of the PoCol data filesystem, against an empty state."""
        if not 0 <= percent <= 100:
            self.log('FAILED: the simulated percentage must be within 0..100')
            return 2
        if not self.channels:
            self.log('FAILED: no channel is configured')
            return 2
        now = self.system.now()
        m = self.analyse(measure(self.system, self.config), [])
        target = next((fs for fs in m['filesystems'] if fs['mount'] == m['data']['mount']), m['filesystems'][0])
        target['percent'] = float(percent)
        target['avail'] = int(target['size'] * (100 - percent) / 100)
        target['used'] = target['size'] - target['avail']
        alerts = {}
        events = self.transitions(self.conditions(m, alerts), alerts, now)
        if not events:
            events = [('INFO', f'simulated {percent:.1f}% is below every threshold: no alert would be sent')]
        kind, subject, text = self.compose(events, m, now, test=True)
        self.log(text.replace('\n', ' | '))
        return self.report_results(self.send_direct(subject, text))

    def send_report(self, send=True):
        fd = self.lock()
        try:
            state = self.load_state()
            now = self.system.now()
            m = self.analyse(measure(self.system, self.config), state['samples'])
            _, subject, text = self.report(m, state, now)
            print(text, file=self.out)
            if not send:
                return 0
            if not self.channels:
                self.log('no channel configured; report printed only')
                return 2
            return self.report_results(self.send_direct(subject, text))
        finally:
            self.unlock(fd)

    def status(self):
        now = self.system.now()
        state = self.new_state()
        try:
            with open(self.state_path(STATE_NAME), encoding='utf-8') as handle:
                loaded = json.load(handle)
            if isinstance(loaded, dict) and loaded.get('v') == STATE_VERSION:
                state = sanitize_state(loaded, state)
        except (OSError, ValueError):
            pass
        m = self.analyse(measure(self.system, self.config), state.get('samples', []))
        _, _, text = self.report(m, state, now)
        print(text.replace('[REPORT] PoCol disk daily report', '[STATUS] PoCol disk status', 1), file=self.out)
        for name in ('telegram', 'email'):
            print(f'channel {name}: ' + ('configured' if name in self.channels else 'not configured'), file=self.out)
        print('heartbeat: ' + ('configured' if self.config.secrets.get('HEARTBEAT_URL') else 'not configured'),
              file=self.out)
        print(f'peer check: {"on" if self.config["PEER_CHECK"] else "off"}', file=self.out)
        return 0

    def notify_failure(self, unit):
        """The OnFailure= path: queue a MONITOR FAILED alert and try to deliver it now."""
        now = self.system.now()
        subject = f'[MONITOR FAILED] PoCol disk monitor: {self.identity()}'
        text = '\n'.join([subject, f'- {unit} failed at {self.stamp(now)}: the last disk check did not complete.',
                          f'- Check on the server: journalctl -u {unit} -n 50',
                          'Until it is fixed no disk alert is sent from this server.',
                          f'Time: {self.stamp(now)}'] +
                         ([f'PoCol: {ARABIC["MONITOR FAILED"]} - {self.identity()}'] if self.config['ARABIC_LINE'] else []))
        self.log(text.replace('\n', ' | '))
        try:
            fd = self.lock()
        except Exception as exc:
            return self.failure_fallback(subject, text, exc)
        try:
            state = self.load_state()
            self.enqueue(state, 'MONITOR FAILED', subject, text, now)
            failures = self.deliver(state, now)
            self.try_save(state)
            return 1 if failures or not self.channels else 0
        except Exception as exc:
            return self.failure_fallback(subject, text, exc)
        finally:
            self.unlock(fd)

    def failure_fallback(self, subject, text, exc):
        """The state is unusable (the failure may be that): send the failure notice directly, without the queue."""
        self.log(f'state unavailable ({self.redact(exc)}); sending without the queue')
        results = self.send_direct(subject, text)
        for name, error in results.items():
            self.log(f'{name}: ' + ('delivered' if error is None else f'FAILED ({error})'))
        return 0 if results and not any(results.values()) else 1


# ------------------------------------------------------------------------------------------------------- channels

class HttpsTransport:
    """HTTPS requests with certificate and host name verification always on (ssl.create_default_context); a
    redirect to anything but https:// is refused."""

    def __init__(self, ca_file='', timeout=20.0):
        self.ca_file = ca_file or None
        self.timeout = timeout

    def context(self):
        context = ssl.create_default_context(cafile=self.ca_file)
        assert context.verify_mode == ssl.CERT_REQUIRED and context.check_hostname
        return context

    def __call__(self, method, url, data, headers, timeout=None):
        if urllib.parse.urlsplit(url).scheme != 'https':
            raise DeliveryError('refusing a URL that is not https://')

        class HttpsOnlyRedirect(urllib.request.HTTPRedirectHandler):
            def redirect_request(self, req, fp, code, msg, hdrs, newurl):
                if urllib.parse.urlsplit(newurl).scheme != 'https':
                    raise DeliveryError('refusing a redirect away from https://')
                return super().redirect_request(req, fp, code, msg, hdrs, newurl)

        opener = urllib.request.build_opener(urllib.request.HTTPSHandler(context=self.context()), HttpsOnlyRedirect)
        request = urllib.request.Request(url, data=data, method=method, headers=dict(headers or {}))
        request.add_header('User-Agent', 'pocol-disk-monitor')
        try:
            with opener.open(request, timeout=timeout or self.timeout) as response:
                return response.status, response.read(65536)
        except urllib.error.HTTPError as exc:
            body = exc.read(65536) if exc.fp else b''
            return exc.code, body
        except urllib.error.URLError as exc:
            raise DeliveryError(f'connection failed: {exc.reason}') from None


class Telegram:
    def __init__(self, token, chat_id, base, transport, timeout):
        self.token, self.chat_id, self.base, self.transport, self.timeout = token, chat_id, base.rstrip('/'), transport, timeout

    def send(self, subject, text):
        url = f'{self.base}/bot{self.token}/sendMessage'
        body = urllib.parse.urlencode({'chat_id': self.chat_id, 'text': text[:4000],
                                       'disable_web_page_preview': 'true'}).encode()
        status, payload = self.transport('POST', url, body, {'Content-Type': 'application/x-www-form-urlencoded'},
                                         self.timeout)
        try:
            answer = json.loads(payload.decode('utf-8', 'replace') or '{}')
        except ValueError:
            answer = {}
        if status != 200 or not answer.get('ok'):
            raise DeliveryError(f'Telegram answered HTTP {status}: {str(answer.get("description", ""))[:120]}')


class Email:
    def __init__(self, host, port, security, user, password, sender, recipients, ca_file, timeout, helo):
        self.host, self.port, self.security = host, port, security
        self.user, self.password, self.sender, self.recipients = user, password, sender, recipients
        self.ca_file, self.timeout, self.helo = ca_file or None, timeout, helo

    def context(self):
        context = ssl.create_default_context(cafile=self.ca_file)
        assert context.verify_mode == ssl.CERT_REQUIRED and context.check_hostname
        return context

    def send(self, subject, text):
        message = EmailMessage()
        message['Subject'] = subject
        message['From'] = self.sender
        message['To'] = ', '.join(self.recipients)
        message['Date'] = formatdate(localtime=False)
        message['Message-ID'] = make_msgid(domain='pocol-disk-monitor.invalid')
        message.set_content(text)
        if self.security == 'ssl':
            server = smtplib.SMTP_SSL(self.host, self.port, local_hostname=self.helo, timeout=self.timeout,
                                      context=self.context())
        else:
            server = smtplib.SMTP(self.host, self.port, local_hostname=self.helo, timeout=self.timeout)
        try:
            if self.security != 'ssl':
                server.ehlo()
                server.starttls(context=self.context())   # raises if the server offers no STARTTLS: never plain text
                server.ehlo()
            if self.user:
                server.login(self.user, self.password or '')
            server.send_message(message)
        finally:
            try:
                server.quit()
            except (smtplib.SMTPException, OSError):
                server.close()


def build_channels(config, transport):
    channels = {}
    wanted = config['CHANNELS']
    s = config.secrets
    if 'telegram' in wanted and s.get('TELEGRAM_BOT_TOKEN') and s.get('TELEGRAM_CHAT_ID'):
        channels['telegram'] = Telegram(s['TELEGRAM_BOT_TOKEN'], s['TELEGRAM_CHAT_ID'], config['TELEGRAM_API_BASE'],
                                        transport, config['NETWORK_TIMEOUT_SECONDS'])
    if 'email' in wanted and config['SMTP_HOST'] and config['SMTP_FROM'] and config['SMTP_TO']:
        if config['SMTP_USER'] and not s.get('SMTP_PASSWORD'):
            config.warnings.append('SMTP_USER is set but secret.env has no SMTP_PASSWORD; email disabled')
        else:
            channels['email'] = Email(config['SMTP_HOST'], config['SMTP_PORT'], config['SMTP_SECURITY'],
                                      config['SMTP_USER'], s.get('SMTP_PASSWORD'), config['SMTP_FROM'],
                                      config['SMTP_TO'], config['CA_FILE'], config['NETWORK_TIMEOUT_SECONDS'],
                                      'pocol-disk-monitor.localdomain')
    return channels


# ---------------------------------------------------------------------------------------------------------- main

def main(argv=None, system=None, out=None):
    parser = argparse.ArgumentParser(description='PoCol founder disk monitor (read-only; it only measures and notifies)')
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--once', action='store_true', help='one check, as the timer runs it')
    mode.add_argument('--status', action='store_true', help='print the current measurements (no secrets, no sending)')
    mode.add_argument('--report', action='store_true', help='print the daily report and send it now')
    mode.add_argument('--test-alert', action='store_true',
                      help='send a test notification through every configured channel; non-zero if any fails')
    mode.add_argument('--simulate-usage', type=float, metavar='PERCENT',
                      help='run the alert path at a simulated disk usage, marked TEST, real state untouched')
    mode.add_argument('--notify-failure', metavar='UNIT', help='the OnFailure= path: send a MONITOR FAILED alert')
    parser.add_argument('--print-only', action='store_true', help='with --report: print, do not send')
    parser.add_argument('--config', default=DEFAULT_CONFIG)
    parser.add_argument('--secrets', default=DEFAULT_SECRETS)
    parser.add_argument('--state-dir', default=DEFAULT_STATE_DIR)
    args = parser.parse_args(argv)
    system = system or System()
    out = out or sys.stderr
    config = Config.load(args.config, args.secrets, system)
    monitor = Monitor(config, system, os.path.abspath(args.state_dir), out=out)
    try:
        if args.once:
            if os.environ.get('POCOL_DISK_MONITOR_FORCE_FAILURE') == '1':
                # The acceptance test's forced failure (README.md): the unit fails and OnFailure= must report it.
                monitor.log('ERROR: forced failure for the acceptance test (POCOL_DISK_MONITOR_FORCE_FAILURE=1)')
                return 1
            return monitor.run_once()
        if args.status:
            return monitor.status()
        if args.report:
            return monitor.send_report(send=not args.print_only)
        if args.test_alert:
            return monitor.test_alert()
        if args.simulate_usage is not None:
            return monitor.simulate(args.simulate_usage)
        return monitor.notify_failure(args.notify_failure)
    except Exception as exc:  # the unit fails, and OnFailure= reports it
        monitor.log(f'ERROR: {type(exc).__name__}: {exc}')
        return 1


if __name__ == '__main__':
    sys.exit(main())
