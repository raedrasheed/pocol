#!/usr/bin/env python3
"""Tests of the PoCol founder disk monitor (deploy/monitoring/pocol_disk_monitor.py, OWNER-DECISIONS-2026-11-21 D7).

Standard library only. The host is faked (mountinfo, statvfs, stat, clock, hostname, TCP checks); the PoCol data
directory is a real temporary tree; the channels are fakes, or real Telegram/heartbeat/SMTP clients talking to local
servers on 127.0.0.1 with a throw-away CA (openssl): nothing leaves this host and no real message is sent.
A test that needs a missing tool is skipped with its reason; POCOL_MONITOR_REQUIRE_TOOLS=1 (CI) makes it a failure.
"""
import ast
import base64
import hashlib
import http.server
import io
import json
import os
import shutil
import socket
import ssl
import subprocess
import sys
import tempfile
import threading
import time
import types
import unittest
from unittest import mock

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import pocol_disk_monitor as dm  # noqa: E402

REQUIRE_TOOLS = os.environ.get('POCOL_MONITOR_REQUIRE_TOOLS') == '1'
GB = 10 ** 9
T0 = 1_795_000_000 - (1_795_000_000 % 86400) + 6 * 3600   # 06:00 UTC on some day

MOUNTINFO = '\n'.join([
    r'22 1 8:1 / / rw,relatime shared:1 - ext4 /dev/sda1 rw',
    r'23 22 0:5 / /dev rw - devtmpfs udev rw',
    r'24 22 0:24 / /proc rw - proc proc rw',
    r'25 22 0:23 / /sys rw - sysfs sysfs rw',
    r'26 22 0:26 / /run rw - tmpfs tmpfs rw',
    r'27 22 8:2 / /boot rw - ext4 /dev/sda2 rw',
    r'28 27 8:15 / /boot/efi rw - vfat /dev/sda15 rw',
    r'29 22 7:1 / /snap/core22/1 ro - squashfs /dev/loop1 ro',
    r'30 22 8:1 /var/lib/pocol-disk-monitor /var/lib/pocol-disk-monitor rw - ext4 /dev/sda1 rw',
    r'31 22 8:1 /tmp/systemd-private-x/tmp /tmp rw - ext4 /dev/sda1 rw',
    r'32 22 0:40 / /var/lib/docker/overlay2/x/merged rw - overlay overlay rw',
    r'33 22 8:17 / /mnt/data\040disk rw - xfs /dev/sdb1 rw',
    r'34 22 0:41 / /dev/shm rw - tmpfs tmpfs rw',
]) + '\n'


def need_tool(case, name):
    path = shutil.which(name)
    if path:
        return path
    if REQUIRE_TOOLS:
        case.fail(f'{name} is required here (POCOL_MONITOR_REQUIRE_TOOLS=1) but not installed')
    print(f'\nSKIPPED (not passed): {name} is not installed', file=sys.stderr)
    case.skipTest(f'{name} is not installed')


class StatVfs:
    def __init__(self, size, used, avail=None, inodes=1_000_000, iused=10_000, frsize=4096):
        free = size - used
        avail = free if avail is None else avail
        self.f_frsize = self.f_bsize = frsize
        self.f_blocks = size // frsize
        self.f_bfree = free // frsize
        self.f_bavail = avail // frsize
        self.f_files = inodes
        self.f_ffree = inodes - iused
        self.f_favail = inodes - iused


class FakeSystem(dm.System):
    """The host: a fixed mount table, settable filesystems, a settable clock, fixed names."""

    def __init__(self, data_dir, mountinfo=MOUNTINFO):
        self.t = float(T0)
        self.mountinfo = mountinfo
        self.data_dir = data_dir
        self.fs = {'/': StatVfs(35 * GB, 10 * GB), '/boot': StatVfs(2 * GB, GB // 5),
                   '/boot/efi': StatVfs(100 * 10 ** 6, 6 * 10 ** 6, inodes=0, iused=0),
                   '/mnt/data disk': StatVfs(100 * GB, GB)}
        self.statvfs_errors = set()
        self.peers = {}
        self.tcp_calls = []
        self.reads = []

    def now(self):
        return self.t

    def read_mountinfo(self):
        return self.mountinfo

    def read_text(self, path):
        self.reads.append(path)
        return super().read_text(path)

    def statvfs(self, path):
        if path in self.statvfs_errors or '*' in self.statvfs_errors:
            raise OSError(5, 'Input/output error')
        return self.fs[path]

    def stat(self, path):
        real = os.stat(path)
        if path == self.data_dir or path.startswith(self.data_dir + os.sep) or path.startswith(os.path.dirname(self.data_dir)):
            return types.SimpleNamespace(st_dev=os.makedev(8, 1), st_mode=real.st_mode, st_size=real.st_size)
        return real

    def hostname(self):
        return 'srv-test'

    def primary_ip(self):
        return '192.0.2.10'

    def tcp_reachable(self, host, port, timeout):
        self.tcp_calls.append((host, port))
        return self.peers.get(f'{host}:{port}', True)

    def set_used(self, mount, used, avail=None):
        old = self.fs[mount]
        self.fs[mount] = StatVfs(old.f_blocks * old.f_frsize, used, avail, old.f_files, old.f_files - old.f_ffree)

    def set_percent(self, mount, percent):
        size = self.fs[mount].f_blocks * self.fs[mount].f_frsize
        self.set_used(mount, int(size * percent / 100))


class FakeChannel:
    def __init__(self, fail=False, error='connection refused'):
        self.sent = []
        self.fail = fail
        self.error = error

    def send(self, subject, text):
        if self.fail:
            raise dm.DeliveryError(self.error)
        self.sent.append((subject, text))


class FakeHeartbeat:
    def __init__(self, status=200):
        self.calls = []
        self.status = status

    def __call__(self, method, url, data, headers, timeout=None):
        self.calls.append((method, url))
        return self.status, b'OK'


class Base(unittest.TestCase):
    CONF = 'SERVER_NAME=founder-1\nSERVER_IP=108.165.128.89\n'

    def setUp(self):
        self.tmp = tempfile.mkdtemp(prefix='dm-test-')
        self.addCleanup(shutil.rmtree, self.tmp, True)
        self.data = os.path.join(self.tmp, 'pocol-standalone')
        self.state_dir = os.path.join(self.tmp, 'state')
        os.mkdir(self.state_dir, 0o700)
        self.system = FakeSystem(self.data)
        self.out = io.StringIO()

    def make_data(self, journal=4 * 10 ** 6, history=10 ** 6):
        os.makedirs(os.path.join(self.data, 'state'), exist_ok=True)
        for name, size in (('state/blocks.journal', journal), ('state/history.redb', history),
                           ('state/state.redb', 300_000), ('state/canon.redb', 200_000),
                           ('identity.address', 40)):
            with open(os.path.join(self.data, name), 'wb') as handle:
                handle.write(b'\1' * size)

    def monitor(self, conf='', secrets='', channels=None, heartbeat=None, mode=0o600):
        config = dm.Config(self.CONF + f'POCOL_DATA_DIR={self.data}\n' + conf, secrets, mode)
        self.channel = FakeChannel() if channels is None else None
        chans = {'telegram': self.channel} if channels is None else channels
        self.heartbeat = heartbeat or FakeHeartbeat()
        return dm.Monitor(config, self.system, self.state_dir, channels=chans, out=self.out, heartbeat=self.heartbeat)

    def state(self):
        with open(os.path.join(self.state_dir, dm.STATE_NAME), encoding='utf-8') as handle:
            return json.load(handle)

    def texts(self, channel=None, reports=True):
        return [text for _, text in (channel or self.channel).sent if reports or not text.startswith('[REPORT]')]

    def run_every(self, monitor, seconds, count, step=900, before=None):
        for _ in range(count):
            if before:
                before()
            monitor.run_once()
            self.system.t += step


# ------------------------------------------------------------------------------------------------ 1. measurement

class Measurement(Base):
    def test_real_filesystems_only_one_per_device(self):
        mounts = dm.real_mounts(dm.parse_mountinfo(MOUNTINFO))
        self.assertEqual([m[0] for m in mounts], ['/', '/boot', '/boot/efi', '/mnt/data disk'])
        self.assertEqual(dict((m[0], m[1]) for m in mounts)['/mnt/data disk'], 'xfs')
        # The bind mounts of /dev/sda1 (systemd's ReadWritePaths= and PrivateTmp=) are not measured again.
        self.assertNotIn('/tmp', [m[0] for m in mounts])
        self.assertEqual(dm.real_mounts(dm.parse_mountinfo(MOUNTINFO), {'/boot/efi'})[2][0], '/mnt/data disk')

    def test_percent_is_dfs_and_inodes_are_measured(self):
        # 35 GB, 30 GB used, 3 GB available (2 GB reserved for root): df's Use% is used / (used + avail).
        self.system.fs['/'] = StatVfs(35 * GB, 30 * GB, 3 * GB, inodes=2_000_000, iused=1_500_000)
        fs = dm.measure_filesystem(self.system, '/', 'ext4', '8:1')
        self.assertAlmostEqual(fs['percent'], 30 / 33 * 100, places=1)
        self.assertEqual(fs['avail'], 3 * GB - (3 * GB) % 4096)
        self.assertEqual(fs['inode_percent'], 75.0)
        self.assertIsNone(dm.measure_filesystem(self.system, '/boot/efi', 'vfat', '8:15')['inode_percent'])

    def test_one_failing_filesystem_is_a_warning_and_none_is_an_error(self):
        self.system.statvfs_errors = {'/boot'}
        m = dm.measure(self.system, dm.Config(f'POCOL_DATA_DIR={self.data}'))
        self.assertEqual([fs['mount'] for fs in m['filesystems']], ['/', '/boot/efi', '/mnt/data disk'])
        self.assertIn('/boot: statvfs failed', m['warnings'][0])
        self.system.statvfs_errors = {'*'}
        with self.assertRaises(RuntimeError):
            dm.measure(self.system, dm.Config(f'POCOL_DATA_DIR={self.data}'))

    def test_pocol_data_by_part_with_real_blocks(self):
        self.make_data()
        sparse = os.path.join(self.data, 'state', 'sparse.journal')
        with open(sparse, 'wb') as handle:
            handle.seek(500 * 10 ** 6)
            handle.write(b'x')
        os.link(os.path.join(self.data, 'state', 'history.redb'), os.path.join(self.data, 'history-link.redb'))
        outside = os.path.join(self.tmp, 'outside.bin')
        with open(outside, 'wb') as handle:
            handle.write(b'\0' * 3_000_000)
        os.symlink(outside, os.path.join(self.data, 'state', 'link-out'))
        d = dm.scan_data_dir(self.system, self.data)
        self.assertTrue(d['exists'])
        self.assertGreaterEqual(d['parts']['journal'], 4 * 10 ** 6)
        self.assertLess(d['parts']['journal'], 4 * 10 ** 6 + 10 ** 6, 'the sparse file counts its real blocks')
        self.assertGreater(d['apparent'], 500 * 10 ** 6)
        linked = d['parts']['history.redb'] + d['parts']['other']
        self.assertGreaterEqual(linked, 10 ** 6)
        self.assertLess(linked, 10 ** 6 + 200_000, 'a hard link counts once and a symbolic link is not followed')
        for part in ('state.redb', 'canon.redb'):
            self.assertGreater(d['parts'][part], 0)
        self.assertEqual(d['bytes'], sum(d['parts'].values()))

    def test_missing_pocol_dir_before_install(self):
        d = dm.scan_data_dir(self.system, self.data)
        self.assertEqual((d['exists'], d['bytes'], d['unreadable']), (False, 0, 0))
        mon = self.monitor()
        self.assertEqual(mon.run_once(), 0)
        self.assertIn('PoCol data absent', self.out.getvalue())
        self.assertNotIn('pocol:missing', self.state()['alerts'])

    def test_missing_pocol_dir_is_critical_once_required(self):
        mon = self.monitor('POCOL_DATA_REQUIRED=1\n')
        mon.run_once()
        self.assertEqual(self.state()['alerts']['pocol:missing']['level'], dm.CRITICAL)
        self.assertIn('CRITICAL: PoCol data directory', self.texts()[0])
        self.make_data()
        self.system.t += 900
        mon.run_once()
        self.assertIn('RECOVERED', self.texts()[-1])

    def test_unreadable_entries_are_counted_not_fatal(self):
        self.make_data()
        real = self.system.scandir

        def scandir(path):
            if path.endswith('state'):
                raise PermissionError(13, 'Permission denied')
            return real(path)
        self.system.scandir = scandir
        d = dm.scan_data_dir(self.system, self.data)
        self.assertEqual(d['unreadable'], 1)
        self.assertTrue(d['exists'])

    def test_the_scan_never_opens_a_pocol_file(self):
        self.make_data()
        mon = self.monitor()
        with mock.patch('builtins.open', wraps=open) as opened, mock.patch('os.open', wraps=os.open) as raw:
            mon.run_once()
        paths = [str(c.args[0]) for c in opened.call_args_list] + [str(c.args[0]) for c in raw.call_args_list]
        self.assertTrue(paths)
        self.assertFalse([p for p in paths if p.startswith(self.data)], paths)

    def test_the_data_filesystem_is_found_by_device(self):
        self.make_data()
        m = dm.measure(self.system, dm.Config(f'POCOL_DATA_DIR={self.data}'))
        self.assertEqual(m['data']['mount'], '/')


# ---------------------------------------------------------------------------------- 3. thresholds and hysteresis

class Thresholds(Base):
    def level_at(self, percent, mon):
        self.system.set_percent('/', percent)
        mon.run_once()
        self.system.t += 900
        return self.state()['alerts'].get('disk:/', {}).get('level', dm.OK)

    def test_levels_at_70_85_95(self):
        mon = self.monitor()
        self.assertEqual(self.level_at(69.9, mon), dm.OK)
        self.assertEqual(self.level_at(70.0, mon), dm.WARNING)
        self.assertEqual(self.level_at(85.0, mon), dm.CRITICAL)
        self.assertEqual(self.level_at(95.0, mon), dm.EMERGENCY)
        sent = self.texts()
        self.assertIn('[WARNING]', sent[1])
        self.assertIn('[CRITICAL]', sent[2])
        self.assertIn('[EMERGENCY]', sent[3])
        self.assertIn('EMERGENCY: disk / 95.0% used', sent[3])

    def test_any_disk_alerts_not_only_root(self):
        mon = self.monitor()
        self.system.set_percent('/boot', 96)
        mon.run_once()
        self.assertIn('EMERGENCY: disk /boot 96.0% used', self.texts()[0])

    def test_inode_levels(self):
        mon = self.monitor()
        self.system.fs['/'] = StatVfs(35 * GB, 10 * GB, inodes=1_000_000, iused=860_000)
        mon.run_once()
        self.assertEqual(self.state()['alerts']['inode:/']['level'], dm.CRITICAL)
        self.assertIn('CRITICAL: inodes / 86.0% used', self.texts()[0])

    def test_hysteresis_and_recovery(self):
        mon = self.monitor()
        self.level_at(71, mon)
        self.assertEqual(self.level_at(69, mon), dm.WARNING, 'stays until 2 points below')
        self.assertEqual(self.level_at(68.5, mon), dm.WARNING)
        self.assertEqual(self.level_at(67.9, mon), dm.OK)
        self.assertIn('RECOVERED (was WARNING', self.texts()[-1])
        self.assertIn('[RECOVERED]', self.texts()[-1])
        self.level_at(96, mon)
        self.assertEqual(self.level_at(94, mon), dm.EMERGENCY)
        self.assertEqual(self.level_at(92.5, mon), dm.CRITICAL)
        self.assertIn('EASED to CRITICAL (was EMERGENCY)', self.texts()[-1])

    def test_level_functions(self):
        self.assertEqual(dm.level_high(None, [70, 85, 95], dm.EMERGENCY, 2), dm.OK)
        self.assertEqual(dm.level_high(84, [70, 85, 95], dm.CRITICAL, 2), dm.CRITICAL)
        self.assertEqual(dm.level_high(84, [70, 85, 95], dm.WARNING, 2), dm.WARNING)
        self.assertEqual(dm.level_low(1.5, [30, 7, 2], dm.OK, 1.2), dm.EMERGENCY)
        self.assertEqual(dm.level_low(2.3, [30, 7, 2], dm.EMERGENCY, 1.2), dm.EMERGENCY)
        self.assertEqual(dm.level_low(2.5, [30, 7, 2], dm.EMERGENCY, 1.2), dm.CRITICAL)
        self.assertEqual(dm.level_low(40, [30, 7, 2], dm.WARNING, 1.2), dm.OK)

    def test_custom_thresholds_and_bad_values(self):
        c = dm.Config('DISK_WARNING_PERCENT=60\nDISK_CRITICAL_PERCENT=nope\nINODE_WARNING_PERCENT=99\nFOO=1\n'
                      'TELEGRAM_BOT_TOKEN=123:abc\nDAILY_REPORT_HOUR=25\nTELEGRAM_API_BASE=http://x\n')
        self.assertEqual(c['DISK_WARNING_PERCENT'], 60)
        self.assertEqual(c['DISK_CRITICAL_PERCENT'], 85)
        self.assertEqual(c['INODE_WARNING_PERCENT'], 70, 'out of order: defaults')
        self.assertEqual(c['DAILY_REPORT_HOUR'], 8)
        self.assertEqual(c['TELEGRAM_API_BASE'], 'https://api.telegram.org')
        self.assertNotIn('TELEGRAM_BOT_TOKEN', c.secrets, 'a secret in monitor.conf is ignored')
        joined = ' '.join(c.warnings)
        for text in ('DISK_CRITICAL_PERCENT has a bad value', 'unknown setting FOO', 'TELEGRAM_BOT_TOKEN, a secret',
                     'DAILY_REPORT_HOUR must be in', 'INODE_*_PERCENT must increase', 'must be an https:// URL'):
            self.assertIn(text, joined)
        self.assertNotIn('123:abc', joined)


# ---------------------------------------------------------------------------------- 6. de-duplication, re-alerts

class Realerts(Base):
    def count_for(self, percent, hours, interval_text=None):
        mon = self.monitor('GAP_ALERT_MINUTES=100000\n')
        self.system.set_percent('/', percent)
        self.run_every(mon, 900, int(hours * 4))
        return len(self.texts(reports=False))

    def test_emergency_every_hour(self):
        # 1 + one re-alert per hour over 6 h (the announcement rides with the first alert in one message).
        self.assertEqual(self.count_for(96, 6), 6)
        self.assertIn('STILL EMERGENCY since', self.texts()[-1])

    def test_critical_every_six_hours(self):
        self.assertEqual(self.count_for(86, 12), 2)

    def test_warning_every_day(self):
        self.assertEqual(self.count_for(72, 24 * 2), 2, 'one alert and one re-alert in 48 h')

    def test_no_repeat_while_unchanged_within_interval(self):
        mon = self.monitor()
        self.system.set_percent('/', 86)
        self.run_every(mon, 900, 8)
        self.assertEqual(len(self.channel.sent), 1)

    def test_one_message_per_run_bundles_every_event(self):
        mon = self.monitor()
        self.system.set_percent('/', 96)
        self.system.set_percent('/boot', 88)
        mon.run_once()
        self.assertEqual(len(self.channel.sent), 1)
        text = self.texts()[0]
        self.assertTrue(text.startswith('[EMERGENCY]'))
        for line in ('EMERGENCY: disk / 96.0%', 'CRITICAL: disk /boot 88.0%', 'Monitoring active'):
            self.assertIn(line, text)

    def test_hourly_rate_limit_keeps_the_rest_queued(self):
        mon = self.monitor('MAX_MESSAGES_PER_HOUR=2\n')
        state = mon.new_state()
        for n in range(5):
            mon.enqueue(state, 'WARNING', f'subject {n}', f'text {n}', self.system.t)
        mon.deliver(state, self.system.t)
        self.assertEqual(len(self.channel.sent), 2)
        self.assertEqual(len(state['queue']['telegram']), 3)
        mon.deliver(state, self.system.t + 3601)
        self.assertEqual([s for s, _ in self.channel.sent], ['subject 0', 'subject 1', 'subject 2', 'subject 3'])


# ------------------------------------------------------------------------------------- growth, ETA, acceleration

class Growth(Base):
    def seed(self, mon, points):
        """A history of (age seconds, used bytes of /, PoCol bytes)."""
        state = mon.new_state()
        for age, used, pocol in points:
            state['samples'].append({'t': self.system.t - age, 'fs': {'/': used}, 'p': pocol})
        state['last_success'] = self.system.t - 900
        state['announced'] = True
        mon.save_state(state)

    def steady(self, rate_per_day, days, used_now, pocol_now=None):
        points = []
        for k in range(1, int(days * 24) + 1):
            age = k * 3600
            points.append((age, used_now - rate_per_day * age / 86400,
                           None if pocol_now is None else pocol_now - rate_per_day * age / 86400))
        return points

    def test_rates_over_1h_24h_7d(self):
        samples = [{'t': T0 - k * 900, 'fs': {'/': 10 * GB - k * 900 * 1.7 * GB / 86400}, 'p': None}
                   for k in range(1, 8 * 96)]
        r = dm.growth(samples, T0, 10 * GB, lambda s: s['fs'].get('/'))
        for name in ('1h', '24h', '7d'):
            self.assertAlmostEqual(r[name] / GB, 1.7, places=3)
        short = [s for s in samples if T0 - s['t'] <= 6 * 3600]
        r = dm.growth(short, T0, 10 * GB, lambda s: s['fs'].get('/'))
        self.assertIsNotNone(r['1h'])
        self.assertIsNone(r['24h'], 'under 12 h of history')
        self.assertIsNone(r['7d'])

    def test_worst_case_growth_on_35_gb_gives_the_eta_levels(self):
        # The journal's worst case, about 1.7 GB a day (REHEARSAL.md risk 11), against the free space.
        for free, level, tag in ((25 * GB, dm.WARNING, 'WARNING'), (10 * GB, dm.CRITICAL, 'CRITICAL'),
                                 (3 * GB, dm.EMERGENCY, 'EMERGENCY')):
            with self.subTest(free=free):
                shutil.rmtree(self.state_dir)
                os.mkdir(self.state_dir)
                mon = self.monitor()
                used = 35 * GB - free
                self.system.set_used('/', used)
                self.seed(mon, self.steady(1.7 * GB, 8, used))
                mon.run_once()
                alert = self.state()['alerts']['eta:/']
                self.assertEqual(alert['level'], level)
                text = self.texts()[-1]
                self.assertIn(f'{tag}: disk / full in about {free / (1.7 * GB):.1f} days at +1.7 GB/day', text)

    def test_35_gb_fills_in_about_three_weeks_at_the_worst_case(self):
        fs = {'avail': 35 * GB, 'rates': {'24h': 1.7 * GB, '7d': 1.7 * GB}}
        days = dm.eta_days(fs['avail'], fs['rates'], dm.Config())
        self.assertAlmostEqual(days, 20.6, places=1)

    def test_idle_growth_raises_nothing(self):
        mon = self.monitor()
        self.system.set_used('/', 10 * GB)
        self.seed(mon, self.steady(8 * 10 ** 6, 8, 10 * GB))   # the membership projection: 8.0 MB a day idle
        mon.run_once()
        self.assertEqual(self.state()['alerts'], {})

    def test_no_eta_from_a_one_hour_jump(self):
        mon = self.monitor()
        self.system.set_used('/', 12 * GB)
        self.seed(mon, [(1800, 10 * GB, None), (2700, 10 * GB, None), (3600, 10 * GB, None)])
        mon.run_once()
        self.assertNotIn('eta:/', self.state()['alerts'])

    def test_acceleration(self):
        mon = self.monitor()
        used = 15 * GB
        week = [(age, used - 1 * GB - 10 * 10 ** 6 * (age - 86400) / 86400, None) for age in range(90000, 8 * 86400, 3600)]
        day = [(age, used - 1 * GB * age / 86400, None) for age in range(3600, 86401, 3600)]
        self.system.set_used('/', used)
        self.seed(mon, day + week)
        mon.run_once()
        self.assertEqual(self.state()['alerts']['accel:/']['level'], dm.WARNING)
        self.assertIn('(accelerating)', self.texts()[-1])

    def test_acceleration_needs_a_minimum_rate(self):
        mon = self.monitor()
        used = 15 * GB
        week = [(age, used - 100 * 10 ** 6 - 10 ** 6 * (age - 86400) / 86400, None) for age in range(90000, 8 * 86400, 3600)]
        day = [(age, used - 100 * 10 ** 6 * age / 86400, None) for age in range(3600, 86401, 3600)]
        self.system.set_used('/', used)
        self.seed(mon, day + week)
        mon.run_once()
        self.assertNotIn('accel:/', self.state()['alerts'], '100 MB/day is under the 500 MB/day minimum')

    def test_pocol_data_growth_is_tracked(self):
        self.make_data(journal=50 * 10 ** 6)
        mon = self.monitor()
        size = dm.scan_data_dir(self.system, self.data)['bytes']
        self.seed(mon, self.steady(2 * GB, 1, 10 * GB, pocol_now=size))
        m = mon.analyse(dm.measure(self.system, mon.config), self.state()['samples'])
        self.assertAlmostEqual(m['data']['rates']['24h'] / GB, 2.0, places=2)


# ------------------------------------------------------------------------------------------------ daily report

class DailyReport(Base):
    def reports(self):
        return [t for t in self.texts() if t.startswith('[REPORT]')]

    def test_once_a_day_at_the_configured_hour(self):
        mon = self.monitor('GAP_ALERT_MINUTES=100000\n')
        self.make_data()
        self.run_every(mon, 900, 7)                 # 06:00 to 07:30 UTC
        self.assertEqual(self.reports(), [])
        self.run_every(mon, 900, 4 * 24)            # through the next day's 07:30
        self.assertEqual(len(self.reports()), 1)
        self.run_every(mon, 900, 4)
        self.assertEqual(len(self.reports()), 2)
        report = self.reports()[0]
        for text in ('[REPORT] PoCol disk daily report', 'founder-1 [srv-test] (108.165.128.89)', '/ (ext4): 28.6% used',
                     'free of 35.0 GB', 'inodes 1.0%', 'growth 1 h', '24 h', '7 d', 'full in', 'PoCol data',
                     'journal', 'history.redb', 'state.redb', 'canon.redb', 'Active alerts: none',
                     'A missing daily report means the monitor or the host is down.', 'التقرير اليومي'):
            self.assertIn(text, report)
        self.assertIn('08:00 UTC', report)

    def test_a_late_run_still_sends_that_days_report(self):
        mon = self.monitor('GAP_ALERT_MINUTES=100000\n')
        mon.run_once()                               # 06:00
        self.system.t += 4 * 3600                    # host down until 10:00
        mon.run_once()
        self.assertEqual(len(self.reports()), 1)
        self.assertIn('10:00 UTC', self.reports()[0])

    def test_local_time_zone(self):
        with mock.patch.dict(os.environ, {'TZ': 'Asia/Riyadh'}):
            time.tzset()
            try:
                mon = self.monitor('DAILY_REPORT_TZ=local\n')
                self.system.t = T0 - 6 * 3600 + 5 * 3600 + 60   # 05:01 UTC = 08:01 in Riyadh
                mon.run_once()
                self.assertEqual(len(self.reports()), 1)
                self.assertIn('08:01 +03', self.reports()[0])
            finally:
                os.environ.pop('TZ', None)
        time.tzset()

    def test_report_command_prints_and_sends_without_changing_the_schedule(self):
        mon = self.monitor()
        mon.run_once()
        before = self.state()['last_report_date']
        self.assertEqual(mon.send_report(), 0)
        self.assertIn('[REPORT]', self.out.getvalue())
        self.assertEqual(len(self.reports()), 1)
        self.assertEqual(self.state()['last_report_date'], before)
        self.assertEqual(mon.send_report(send=False), 0)
        self.assertEqual(len(self.reports()), 1)


# ------------------------------------------------------------------------------- 7. monitoring of the monitor

class SelfFailure(Base):
    def test_gap_since_the_last_success_is_reported(self):
        mon = self.monitor()
        mon.run_once()
        self.system.t += 2 * 3600 + 600
        mon.run_once()
        alerts = self.texts(reports=False)
        self.assertIn('MONITORING GAP: no successful check for 2 h 10 min', alerts[-1])
        self.assertTrue(alerts[-1].startswith('[WARNING]'))
        self.system.t += 900
        mon.run_once()
        self.assertEqual(len(self.texts(reports=False)), 2, 'a normal interval reports nothing')

    def test_the_gap_threshold_is_configurable(self):
        mon = self.monitor('GAP_ALERT_MINUTES=10\n')
        mon.run_once()
        self.system.t += 900
        mon.run_once()
        self.assertIn('MONITORING GAP', self.texts()[-1])

    def test_on_failure_path_sends_monitor_failed(self):
        mon = self.monitor()
        self.assertEqual(mon.notify_failure('pocol-disk-monitor.service'), 0)
        text = self.texts()[-1]
        self.assertTrue(text.startswith('[MONITOR FAILED] PoCol disk monitor: founder-1 [srv-test] (108.165.128.89)'))
        self.assertIn('journalctl -u pocol-disk-monitor.service', text)
        self.assertIn('تعطل نظام المراقبة', text)

    def test_on_failure_message_is_queued_when_no_channel_answers(self):
        mon = self.monitor(channels={'telegram': FakeChannel(fail=True)})
        self.assertEqual(mon.notify_failure('pocol-disk-monitor.service'), 1)
        self.assertEqual(self.state()['queue']['telegram'][0]['kind'], 'MONITOR FAILED')
        ok = FakeChannel()
        mon.channels = {'telegram': ok}
        mon.run_once()
        self.assertTrue(ok.sent[0][1].startswith('[MONITOR FAILED]'))

    def test_a_crash_exits_non_zero_so_on_failure_runs(self):
        conf = os.path.join(self.tmp, 'monitor.conf')
        with open(conf, 'w') as handle:
            handle.write(f'POCOL_DATA_DIR={self.data}\n')
        self.system.statvfs_errors = {'*'}
        code = dm.main(['--once', '--config', conf, '--secrets', os.path.join(self.tmp, 'none.env'),
                        '--state-dir', self.state_dir], system=self.system, out=self.out)
        self.assertEqual(code, 1)
        self.assertIn('ERROR: RuntimeError: no filesystem could be measured', self.out.getvalue())

    def test_the_acceptance_tests_forced_failure(self):
        conf = os.path.join(self.tmp, 'monitor.conf')
        with open(conf, 'w') as handle:
            handle.write(f'POCOL_DATA_DIR={self.data}\n')
        args = ['--once', '--config', conf, '--secrets', os.path.join(self.tmp, 'none.env'), '--state-dir', self.state_dir]
        with mock.patch.dict(os.environ, {'POCOL_DISK_MONITOR_FORCE_FAILURE': '1'}):
            self.assertEqual(dm.main(args, system=self.system, out=self.out), 1)
        self.assertIn('forced failure for the acceptance test', self.out.getvalue())
        self.assertEqual(os.listdir(self.state_dir), [], 'a forced failure measures and writes nothing')
        self.assertEqual(dm.main(args, system=self.system, out=self.out), 0)

    def test_heartbeat_on_success_and_fail_url_with_undelivered_messages(self):
        url = 'https://hc-ping.test/0f9b2c-uuid'
        mon = self.monitor(secrets=f'HEARTBEAT_URL={url}\n')
        mon.run_once()
        self.assertEqual(self.heartbeat.calls[-1], ('GET', url))
        mon.channels = {'telegram': FakeChannel(fail=True)}
        self.system.set_percent('/', 96)
        self.system.t += 900
        mon.run_once()
        self.assertEqual(self.heartbeat.calls[-1], ('GET', url + '/fail'))

    def test_heartbeat_must_be_https(self):
        c = dm.Config('', 'HEARTBEAT_URL=http://hc.example/ping\n')
        self.assertNotIn('HEARTBEAT_URL', c.secrets)
        self.assertIn('HEARTBEAT_URL must be an https:// URL', ' '.join(c.warnings))

    def test_first_run_with_a_channel_announces_and_resends_active_alerts(self):
        mon = self.monitor(channels={})
        self.system.set_percent('/', 90)
        mon.run_once()
        self.assertIn('no channel configured; message not sent', self.out.getvalue())
        mon.channels = {'telegram': FakeChannel()}
        self.system.t += 900
        mon.run_once()
        text = mon.channels['telegram'].sent[0][1]
        self.assertIn('Monitoring active', text)
        self.assertIn('STILL CRITICAL', text)

    def test_peer_check_is_off_by_default(self):
        mon = self.monitor('PEERS=108.165.128.90:30300\n')
        mon.run_once()
        self.assertEqual(self.system.tcp_calls, [])

    def test_peer_unreachable_for_n_runs_then_recovery(self):
        mon = self.monitor('PEER_CHECK=1\nPEERS=108.165.128.90:30300,108.165.128.91:30300\nPEER_FAIL_RUNS=3\n')
        self.system.peers['108.165.128.91:30300'] = False
        self.run_every(mon, 900, 2)
        self.assertNotIn('peer:108.165.128.91:30300', self.state()['alerts'])
        self.run_every(mon, 900, 1)
        self.assertIn('CRITICAL: founder peer 108.165.128.91:30300 TCP unreachable for 3 consecutive checks',
                      self.texts()[-1])
        self.system.peers['108.165.128.91:30300'] = True
        self.run_every(mon, 900, 1)
        self.assertIn('RECOVERED', self.texts()[-1])
        self.assertIn(('108.165.128.90', 30300), self.system.tcp_calls)


# --------------------------------------------------------------------------------- queue, retry and state file

class QueueAndState(Base):
    def test_failed_delivery_is_queued_and_retried_without_losing_the_level(self):
        down = FakeChannel(fail=True)
        mon = self.monitor(channels={'telegram': down})
        self.system.set_percent('/', 96)
        mon.run_once()
        self.assertEqual(self.state()['alerts']['disk:/']['level'], dm.EMERGENCY)
        self.assertEqual(len(self.state()['queue']['telegram']), 1)
        self.assertIn('delivery failed (connection refused); 1 message(s) queued for retry', self.out.getvalue())
        self.system.set_percent('/', 50)
        self.system.t += 900
        mon.run_once()
        queue = self.state()['queue']['telegram']
        self.assertEqual(len(queue), 1, 'the newer alert message replaces the undelivered one')
        self.assertEqual((queue[0]['superseded'], queue[0]['superseded_top']), (1, 'EMERGENCY'))
        down.fail = False
        self.system.t += 900
        mon.run_once()
        self.assertEqual(self.state()['queue']['telegram'], [])
        self.assertEqual(len(down.sent), 1)
        text = down.sent[0][1]
        self.assertTrue(text.startswith('[RECOVERED]'))
        self.assertIn('RECOVERED (was EMERGENCY since', text)
        self.assertIn('replaces 1 earlier undelivered message(s)', text)
        self.assertIn('the most severe was EMERGENCY', text)
        self.assertIn('Active now: none', text)

    def test_queue_is_bounded_and_drops_the_lowest_severity_first(self):
        mon = self.monitor('QUEUE_MAX=3\n', channels={'telegram': FakeChannel(fail=True)})
        state = mon.new_state()
        for n, kind in enumerate(['EMERGENCY', 'REPORT', 'WARNING', 'REPORT', 'CRITICAL']):
            mon.enqueue(state, kind, f'{kind} {n}', f'{kind} {n}', T0 + n)
        self.assertEqual([q['kind'] for q in state['queue']['telegram']], ['EMERGENCY', 'WARNING', 'CRITICAL'])
        self.assertEqual(state['dropped']['telegram'], 2)
        ok = FakeChannel()
        mon.channels = {'telegram': ok}
        mon.deliver(state, T0 + 10)
        self.assertIn('(2 older message(s) were dropped from the full queue)', ok.sent[0][1])
        self.assertNotIn('dropped', ok.sent[1][1])

    def test_one_channel_down_does_not_hold_the_other(self):
        a, b = FakeChannel(fail=True), FakeChannel()
        mon = self.monitor(channels={'telegram': a, 'email': b})
        self.system.set_percent('/', 90)
        mon.run_once()
        self.assertEqual(len(b.sent), 1)
        self.assertEqual(len(self.state()['queue']['telegram']), 1)

    def test_state_file_stays_bounded_over_days_with_a_dead_channel(self):
        mon = self.monitor('QUEUE_MAX=20\nGAP_ALERT_MINUTES=100000\n', channels={'telegram': FakeChannel(fail=True)})
        self.make_data()
        size = 10 * GB

        def grow():
            nonlocal size
            size += int(1.7 * GB / 96)
            self.system.set_used('/', size)
        self.run_every(mon, 900, 3 * 96, before=grow)
        state = self.state()
        self.assertLessEqual(len(state['samples']), dm.SAMPLES_MAX)
        self.assertLessEqual(len(state['queue']['telegram']), 20)
        self.assertLess(os.path.getsize(os.path.join(self.state_dir, dm.STATE_NAME)), 200_000)
        kept = dm.compact_samples([{'t': T0 - k * 900, 'fs': {}, 'p': None} for k in range(30 * 96)], T0)
        self.assertLessEqual(len(kept), dm.SAMPLES_MAX)
        self.assertGreater(T0 - min(s['t'] for s in kept), 7 * 86400, 'hourly samples cover the 7 d window')
        self.assertLess(T0 - min(s['t'] for s in kept), 8 * 86400 + 3600)

    def test_state_file_mode_and_atomic_write(self):
        mon = self.monitor()
        mon.run_once()
        st = os.stat(os.path.join(self.state_dir, dm.STATE_NAME))
        self.assertEqual(st.st_mode & 0o777, 0o600)
        self.assertEqual(sorted(os.listdir(self.state_dir)), ['lock', 'state.json', 'state.reserve'])
        self.assertEqual(os.path.getsize(os.path.join(self.state_dir, dm.STATE_RESERVE_NAME)), dm.RESERVE_BYTES)

    def test_corrupt_state_file_is_set_aside_and_reported(self):
        with open(os.path.join(self.state_dir, dm.STATE_NAME), 'w') as handle:
            handle.write('{"v": 1, "samples": [tru')
        mon = self.monitor()
        self.assertEqual(mon.run_once(), 0)
        self.assertTrue(os.path.exists(os.path.join(self.state_dir, dm.STATE_CORRUPT_NAME)))
        self.assertIn('The state file was unreadable (JSONDecodeError)', self.texts()[0])
        self.assertEqual(self.state()['v'], dm.STATE_VERSION)

    def test_oversized_or_foreign_state_file_is_set_aside(self):
        for content in ('x' * (dm.STATE_MAX_BYTES + 1), '[1, 2, 3]', '{"v": 99}'):
            with self.subTest(content=content[:10]):
                with open(os.path.join(self.state_dir, dm.STATE_NAME), 'w') as handle:
                    handle.write(content)
                self.monitor().run_once()
                self.assertEqual(self.state()['v'], dm.STATE_VERSION)

    def test_parseable_but_damaged_state_is_sanitised(self):
        damaged = {'v': 1, 'samples': [{'t': 'x'}, {'t': T0 - 900, 'fs': {'/': 'big'}, 'p': 'n'}, 5],
                   'alerts': {'disk:/': {'level': 7}, 'eta:/': 'x', 'inode:/': {'level': 2, 'since': 1, 'last_sent': 1}},
                   'queue': {'telegram': [{'t': 1}, 'x', {'t': 1, 'kind': 'WARNING', 'subject': 's', 'text': 't'}]},
                   'sent': {'telegram': ['x', 1]}, 'dropped': [], 'last_success': 'yesterday',
                   'last_report_date': 5, 'announced': 'yes', 'peers': {'a': 'b'}, 'notes': 'n'}
        with open(os.path.join(self.state_dir, dm.STATE_NAME), 'w') as handle:
            json.dump(damaged, handle)
        mon = self.monitor()
        self.assertEqual(mon.run_once(), 0)
        state = self.state()
        self.assertNotIn('disk:/', state['alerts'])
        self.assertEqual(state['alerts'], {})
        self.assertIn('RECOVERED (was CRITICAL', self.texts()[1], 'the one valid alert entry was kept and cleared')
        self.assertEqual(state['queue']['telegram'], [], 'the one valid queued message was kept and delivered')
        self.assertEqual(self.channel.sent[0][0], 's')
        self.assertEqual(len(state['samples']), 2)

    def test_a_full_disk_that_refuses_the_state_write_still_gets_its_alert_out(self):
        mon = self.monitor()
        self.system.set_percent('/', 100)
        with mock.patch.object(dm.Monitor, 'save_state', side_effect=OSError(28, 'No space left on device')):
            self.assertEqual(mon.run_once(), 0)
        self.assertTrue(self.texts()[0].startswith('[EMERGENCY]'))
        self.assertIn('saving the state failed (No space left on device)', self.out.getvalue())

    def test_the_failure_notice_goes_out_even_without_a_usable_state_directory(self):
        mon = self.monitor()
        mon.state_dir = os.path.join(self.tmp, 'missing-state-dir')
        self.assertEqual(mon.notify_failure('pocol-disk-monitor.service'), 0)
        self.assertTrue(self.texts()[0].startswith('[MONITOR FAILED]'))
        self.assertIn('sending without the queue', self.out.getvalue())
        mon.channels = {'telegram': FakeChannel(fail=True)}
        self.assertEqual(mon.notify_failure('pocol-disk-monitor.service'), 1)

    def test_a_second_run_waits_for_the_lock(self):
        mon = self.monitor()
        fd = mon.lock()
        with mock.patch.object(dm, 'LOCK_WAIT_SECONDS', 0.2):
            with self.assertRaises(RuntimeError):
                self.monitor().run_once()
        mon.unlock(fd)
        self.assertEqual(self.monitor().run_once(), 0)


# ---------------------------------------------------------------------------------------- secrets never shown

class Secrets(Base):
    TOKEN = '7012345678:AAHsecretTOKENvalue_Xyz-123'
    CHAT = '-1009876543210'
    PASSWORD = 'Sup3r-Secret-PW!'
    BEAT = 'https://hc-ping.example.test/5b3d9a7e-secret-uuid'

    def secret_text(self):
        return (f'TELEGRAM_BOT_TOKEN={self.TOKEN}\nTELEGRAM_CHAT_ID={self.CHAT}\nSMTP_PASSWORD="{self.PASSWORD}"\n'
                f'HEARTBEAT_URL={self.BEAT}\n')

    def assert_clean(self, text):
        for secret in (self.TOKEN, self.CHAT, self.PASSWORD, self.BEAT, '5b3d9a7e-secret-uuid'):
            self.assertNotIn(secret, text)

    def test_secrets_never_reach_logs_messages_or_state(self):
        leaky = FakeChannel(fail=True, error=f'POST https://api.telegram.org/bot{self.TOKEN}/sendMessage: '
                                             f'chat {self.CHAT} auth {self.PASSWORD} {self.BEAT}')
        mon = self.monitor(secrets=self.secret_text(), channels={'telegram': leaky},
                           heartbeat=lambda *a, **k: (_ for _ in ()).throw(OSError(f'cannot reach {self.BEAT}')))
        self.system.set_percent('/', 96)
        mon.run_once()
        self.assertEqual(mon.test_alert(), 1)
        self.assertEqual(mon.simulate(96), 1)
        mon.status()
        mon.notify_failure('pocol-disk-monitor.service')
        out = self.out.getvalue()
        self.assertIn('[redacted]', out)
        self.assert_clean(out)
        with open(os.path.join(self.state_dir, dm.STATE_NAME), encoding='utf-8') as handle:
            self.assert_clean(handle.read())

    def test_messages_carry_no_secret(self):
        good = FakeChannel()
        mon = self.monitor(secrets=self.secret_text(), channels={'telegram': good})
        self.system.set_percent('/', 96)
        mon.run_once()
        mon.test_alert()
        mon.simulate(96)
        self.assertGreaterEqual(len(good.sent), 3)
        for subject, text in good.sent:
            self.assert_clean(subject + text)

    def test_status_lists_channels_without_secrets(self):
        mon = self.monitor(secrets=self.secret_text())
        self.assertEqual(mon.status(), 0)
        out = self.out.getvalue()
        self.assertIn('channel telegram: configured', out)
        self.assertIn('heartbeat: configured', out)
        self.assert_clean(out)

    def test_placeholders_are_unset(self):
        with open(os.path.join(HERE, 'secret.env.example'), encoding='utf-8') as handle:
            c = dm.Config('', handle.read())
        self.assertEqual(c.secrets, {})
        self.assertEqual(dm.build_channels(c, None), {})

    def test_an_insecure_secret_file_is_reported_and_test_alert_refuses(self):
        mon = self.monitor(secrets=self.secret_text(), mode=0o644)
        self.assertIn('secret.env is readable by group or others', ' '.join(mon.config.warnings))
        self.assertEqual(mon.test_alert(), 1)
        self.assertEqual(self.channel.sent, [])

    def test_redactor(self):
        r = dm.Redactor([self.TOKEN, 'abc'])
        self.assertEqual(r(f'x {self.TOKEN} y'), 'x [redacted] y')
        self.assertEqual(r('https://api.telegram.org/bot1:zz/sendMessage'), 'https://api.telegram.org/bot[redacted]/sendMessage')
        self.assertEqual(r('abc'), 'abc', 'too short to be a secret')


# --------------------------------------------------------------------------------------------- 9. the commands

class Commands(Base):
    def test_test_alert_exit_codes(self):
        mon = self.monitor(channels={'telegram': FakeChannel(), 'email': FakeChannel()})
        self.assertEqual(mon.test_alert(), 0)
        self.assertIn('telegram: delivered', self.out.getvalue())
        self.assertTrue(mon.channels['email'].sent[0][1].startswith('[TEST] PoCol disk monitor test notification'))
        mon = self.monitor(channels={'telegram': FakeChannel(), 'email': FakeChannel(fail=True)})
        self.assertEqual(mon.test_alert(), 1)
        self.assertIn('email: FAILED (connection refused)', self.out.getvalue())
        self.assertEqual(self.monitor(channels={}).test_alert(), 2)

    def test_test_alert_also_checks_the_heartbeat(self):
        mon = self.monitor(secrets='HEARTBEAT_URL=https://hc.example.test/abcdef12\n', heartbeat=FakeHeartbeat(500))
        self.assertEqual(mon.test_alert(), 1)
        self.assertIn('heartbeat: FAILED', self.out.getvalue())

    def test_simulate_usage_runs_the_alert_path_marked_test_and_keeps_the_state(self):
        mon = self.monitor()
        mon.run_once()
        with open(os.path.join(self.state_dir, dm.STATE_NAME), 'rb') as handle:
            before = handle.read()
        self.assertEqual(mon.simulate(96), 0)
        text = self.texts()[-1]
        self.assertTrue(text.startswith('[TEST] [EMERGENCY] PoCol disk monitor: founder-1 [srv-test] (108.165.128.89)'))
        self.assertIn('EMERGENCY: disk / 96.0% used', text)
        self.assertIn('TEST ONLY: simulated values; the real state is unchanged.', text)
        self.assertIn('(اختبار)', text)
        with open(os.path.join(self.state_dir, dm.STATE_NAME), 'rb') as handle:
            self.assertEqual(handle.read(), before)
        self.assertEqual(mon.simulate(50), 0)
        self.assertIn('below every threshold', self.texts()[-1])
        self.assertEqual(mon.simulate(101), 2)

    def test_simulate_without_any_state(self):
        mon = self.monitor()
        self.assertEqual(mon.simulate(86), 0)
        self.assertEqual(os.listdir(self.state_dir), [])
        self.assertIn('[TEST] [CRITICAL]', self.texts()[-1])

    def test_status_writes_nothing(self):
        mon = self.monitor()
        self.assertEqual(mon.status(), 0)
        self.assertEqual(os.listdir(self.state_dir), [])
        self.assertIn('[STATUS] PoCol disk status', self.out.getvalue())

    def test_messages_identify_the_server(self):
        mon = self.monitor()
        self.assertEqual(mon.identity(), 'founder-1 [srv-test] (108.165.128.89)')
        self.CONF = ''
        self.assertEqual(self.monitor().identity(), 'srv-test (192.0.2.10)')

    def test_messages_are_short(self):
        mon = self.monitor()
        self.system.set_percent('/', 96)
        mon.run_once()
        self.assertLess(len(self.texts()[0]), 1200)
        self.assertLessEqual(len(self.texts()[0].splitlines()), 12)


# ------------------------------------------------------------------------------- 8. it never changes anything

class NothingDestructive(Base):
    SOURCE = os.path.join(HERE, 'pocol_disk_monitor.py')
    ALLOWED_IMPORTS = {'argparse', 'errno', 'fcntl', 'json', 'os', 're', 'smtplib', 'socket', 'ssl', 'stat', 'sys', 'time',
                       'urllib.error', 'urllib.parse', 'urllib.request', 'email.message', 'email.utils'}
    ALLOWED_OS = {'open', 'close', 'write', 'pwrite', 'pread', 'fsync', 'fstat', 'replace', 'stat', 'lstat',
                  'statvfs', 'scandir', 'major',
                  'minor', 'path', 'environ', 'O_RDWR', 'O_CREAT', 'O_WRONLY', 'O_TRUNC', 'O_RDONLY', 'sep'}
    FORBIDDEN_TEXT = ('systemctl', 'ufw', 'iptables', 'nft ', 'rm -', 'kill', 'prune', 'truncate ', 'truncate(',
                      'pocol-standalone.service', 'shutdown', 'reboot')

    def tree(self):
        with open(self.SOURCE, encoding='utf-8') as handle:
            return ast.parse(handle.read())

    def test_imports_and_os_calls_are_read_or_own_state_only(self):
        tree = self.tree()
        for node in ast.walk(tree):
            if isinstance(node, ast.Import):
                for alias in node.names:
                    self.assertIn(alias.name, self.ALLOWED_IMPORTS)
            elif isinstance(node, ast.ImportFrom):
                self.assertIn(node.module, self.ALLOWED_IMPORTS)
            elif isinstance(node, ast.Attribute) and isinstance(node.value, ast.Name) and node.value.id == 'os':
                self.assertIn(node.attr, self.ALLOWED_OS, f'os.{node.attr} at line {node.lineno}')
            elif isinstance(node, ast.Attribute):
                self.assertNotIn(node.attr, ('unlink', 'rmdir', 'rmtree', 'remove', 'removedirs', 'kill', 'killpg',
                                             'popen', 'truncate', 'chmod', 'chown', 'rename', 'symlink',
                                             'link', 'mkdir', 'makedirs', 'write_text', 'write_bytes', 'execv',
                                             'spawnv'), f'.{node.attr} at line {node.lineno}')
            elif isinstance(node, ast.Name):
                self.assertNotIn(node.id, ('eval', 'exec', '__import__', 'compile', 'subprocess', 'shutil'))

    def test_every_write_goes_to_the_state_directory(self):
        """os.open for writing and os.replace appear only in lock/load_state/save_state and the reserve copy's two
        functions, on paths built by state_path(); builtins.open is only ever called to read."""
        for func in ast.walk(self.tree()):
            if not isinstance(func, ast.FunctionDef):
                continue
            from_state = {t.id for n in ast.walk(func) if isinstance(n, ast.Assign)
                          for t in n.targets if isinstance(t, ast.Name)
                          and isinstance(n.value, ast.Call) and getattr(n.value.func, 'attr', '') == 'state_path'}
            for call in [n for n in ast.walk(func) if isinstance(n, ast.Call)]:
                name = getattr(call.func, 'attr', getattr(call.func, 'id', ''))
                owner = getattr(getattr(call.func, 'value', None), 'id', None)
                if owner == 'os' and name in ('replace', 'open'):
                    self.assertIn(func.name, ('lock', 'load_state', 'save_state', 'prepare_reserve', 'save_reserve'),
                                  f'line {call.lineno}')
                    for arg in call.args[:2 if name == 'replace' else 1]:
                        ok = (isinstance(arg, ast.Call) and getattr(arg.func, 'attr', '') == 'state_path') or \
                             (isinstance(arg, ast.Name) and arg.id in from_state) or \
                             (isinstance(arg, ast.Attribute) and arg.attr == 'state_dir' and name == 'open'
                              and 'O_RDONLY' in ast.dump(call.args[1]))
                        self.assertTrue(ok, f'{func.name} line {call.lineno}: {ast.dump(arg)}')
                if owner is None and name == 'open':
                    modes = [a for a in call.args[1:2]] + [k.value for k in call.keywords if k.arg == 'mode']
                    for mode in modes:
                        self.assertTrue(isinstance(mode, ast.Constant) and set(mode.value) <= set('rbt'),
                                        f'open() for writing at line {call.lineno}')

    def test_no_command_and_no_destructive_text(self):
        tree = self.tree()
        docstrings = {id(n.body[0].value) for n in ast.walk(tree)
                      if isinstance(n, (ast.Module, ast.FunctionDef, ast.ClassDef)) and n.body
                      and isinstance(n.body[0], ast.Expr) and isinstance(n.body[0].value, ast.Constant)}
        for node in ast.walk(tree):
            if isinstance(node, ast.Constant) and isinstance(node.value, str) and id(node) not in docstrings:
                for word in self.FORBIDDEN_TEXT:
                    self.assertNotIn(word, node.value.lower(), f'line {node.lineno}')

    def test_state_path_refuses_any_other_name(self):
        mon = self.monitor()
        for name in ('../x', 'other.json', '/etc/passwd'):
            with self.assertRaises(AssertionError):
                mon.state_path(name)

    def snapshot(self, base, skip):
        found = {}
        for directory, dirs, files in os.walk(base):
            if os.path.abspath(directory).startswith(skip):
                continue
            for name in files + [d for d in dirs]:
                path = os.path.join(directory, name)
                if os.path.abspath(path) == skip:
                    continue
                st = os.lstat(path)
                digest = ''
                if os.path.isfile(path) and not os.path.islink(path):
                    with open(path, 'rb') as handle:
                        digest = hashlib.sha256(handle.read()).hexdigest()
                found[os.path.relpath(path, base)] = (st.st_mode, st.st_size, st.st_mtime_ns, digest)
        return found

    def test_every_path_leaves_the_pocol_data_and_everything_else_unchanged(self):
        self.make_data()
        other = os.path.join(self.tmp, 'etc-like')
        os.mkdir(other)
        with open(os.path.join(other, 'pocol-standalone.service'), 'w') as handle:
            handle.write('[Service]\n')
        before = self.snapshot(self.tmp, self.state_dir)
        down = FakeChannel(fail=True)
        mon = self.monitor(channels={'telegram': down})
        for percent in (50, 96, 99.9, 100, 10):
            self.system.set_percent('/', percent)
            mon.run_once()
            self.system.t += 900
        mon.notify_failure('pocol-disk-monitor.service')
        mon.simulate(100)
        mon.status()
        mon.send_report()
        mon.test_alert()
        down.fail = False
        mon.run_once()
        self.assertEqual(self.snapshot(self.tmp, self.state_dir), before)
        self.assertTrue(set(os.listdir(self.state_dir)) <= {'lock', 'state.json', 'state.reserve'})


# ------------------------------------------------------------------------- review fixes (outage, full disk, clock)

class PoisonChannel(FakeChannel):
    """Refuses every message whose subject starts with one of `refuse` (a mail server rejecting it as spam)."""

    def __init__(self, refuse):
        super().__init__()
        self.refuse = refuse
        self.refused = 0

    def send(self, subject, text):
        if subject.startswith(self.refuse):
            self.refused += 1
            raise dm.DeliveryError('554 rejected')
        super().send(subject, text)


class Outages(Base):
    def test_after_an_outage_the_current_state_comes_first_not_a_backlog(self):
        down = FakeChannel(fail=True)
        mon = self.monitor(channels={'telegram': down})
        self.system.set_percent('/', 96)
        self.run_every(mon, 900, 24 * 4)                 # 24 h at 96% with the channel down
        queue = self.state()['queue']['telegram']
        self.assertLessEqual(len(queue), 2, 'one alert message and one report, not one per re-alert')
        self.system.set_percent('/', 50)
        mon.run_once()                                   # the disk was freed, still down
        down.fail = False
        self.system.t += 900
        mon.run_once()
        self.assertLessEqual(len(down.sent), 3)
        first = down.sent[0][1]
        self.assertTrue(first.startswith('[RECOVERED]'), first[:80])
        self.assertIn('the most severe was EMERGENCY', first)
        self.assertEqual(self.state()['queue']['telegram'], [])

    def test_the_newest_alert_lists_every_active_alert(self):
        down = FakeChannel(fail=True)
        mon = self.monitor('GAP_ALERT_MINUTES=100000\n', channels={'telegram': down})
        self.system.set_percent('/', 96)
        mon.run_once()
        self.system.set_percent('/boot', 72)              # a new, lower event while the EMERGENCY is undelivered
        self.system.t += 900
        mon.run_once()
        down.fail = False
        self.system.t += 900
        mon.run_once()
        alerts = self.texts(down, reports=False)
        self.assertEqual(len(alerts), 1)
        self.assertIn('Active now: EMERGENCY disk:/ since', alerts[0])
        self.assertIn('WARNING disk:/boot since', alerts[0])

    def test_a_message_the_channel_keeps_refusing_does_not_block_the_rest(self):
        mail = PoisonChannel('[REPORT]')
        mon = self.monitor('GAP_ALERT_MINUTES=100000\n', channels={'email': mail})
        self.system.t = T0 + 2 * 3600                    # 08:00 UTC: the report is due and refused
        self.system.set_percent('/', 96)
        self.run_every(mon, 900, 24 * 4)
        emergencies = [t for t in self.texts(mail) if t.startswith('[EMERGENCY]')]
        self.assertGreaterEqual(len(emergencies), 23)
        # A refused message of higher severity than the rest is skipped after POISON_ATTEMPTS failures.
        mail = PoisonChannel('[MONITOR FAILED]')
        shutil.rmtree(self.state_dir)
        os.mkdir(self.state_dir)
        mon = self.monitor('GAP_ALERT_MINUTES=100000\n', channels={'email': mail})
        mon.notify_failure('pocol-disk-monitor.service')
        self.system.set_percent('/', 72)
        self.run_every(mon, 900, dm.POISON_ATTEMPTS + 1)
        self.assertTrue(any(t.startswith('[WARNING]') for t in self.texts(mail)), self.out.getvalue()[-400:])
        self.assertEqual(self.state()['queue']['email'][0]['kind'], 'MONITOR FAILED', 'kept, still retried')

    def test_monitor_failed_is_repeated_at_its_interval_then_recovers(self):
        mon = self.monitor()
        for _ in range(24 * 4):                          # 24 h of failing checks, every 15 minutes
            mon.notify_failure('pocol-disk-monitor.service')
            self.system.t += 900
        failed = [t for t in self.texts() if t.startswith('[MONITOR FAILED]')]
        self.assertEqual(len(failed), 24, 'the first and one per hour')
        self.assertIn('STILL FAILING: 5 failed check(s) since', failed[1])
        mon.run_once()
        self.assertIn('MONITOR RECOVERED: the check succeeds again after 96 failed check(s)', self.texts()[-1])
        self.assertIsNone(self.state()['failure'])
        mon.notify_failure('pocol-disk-monitor.service')
        self.assertIn('failed at', self.texts()[-1], 'a new failure is reported at once')

    def test_monitor_failed_keeps_one_queued_message_while_the_channel_is_down(self):
        mon = self.monitor('REALERT_MONITOR_FAILED_HOURS=0.25\n', channels={'telegram': FakeChannel(fail=True)})
        for _ in range(8):
            mon.notify_failure('pocol-disk-monitor.service')
            self.system.t += 900
        queue = self.state()['queue']['telegram']
        self.assertEqual([q['kind'] for q in queue], ['MONITOR FAILED'])
        self.assertEqual(queue[0]['superseded'], 7)


class FullDisk(Base):
    def run_unsaveable(self, mon, runs):
        with mock.patch.object(dm.Monitor, 'save_state', side_effect=OSError(28, 'No space left on device')):
            self.run_every(mon, 900, runs)

    def test_an_unsaveable_state_file_does_not_flood(self):
        mon = self.monitor()
        self.system.t = T0 + 2 * 3600                    # 08:00 UTC: today's report goes with the first run
        self.system.set_percent('/', 96)
        mon.run_once()
        self.system.t += 900
        self.run_unsaveable(mon, 8)                      # 2 h with the state file refusing every write
        texts = self.texts()
        self.assertEqual(len([t for t in texts if t.startswith('[REPORT]')]), 1)
        self.assertEqual(len([t for t in texts if 'STILL EMERGENCY' in t]), 2, 'one re-alert per hour')
        self.assertFalse(any('MONITORING GAP' in t for t in texts))
        self.assertIn('the state was saved in the reserve copy instead', self.out.getvalue())
        # Space again: the state file wins over the reserve, which is blanked so it is never read again.
        mon.run_once()
        with open(os.path.join(self.state_dir, dm.STATE_RESERVE_NAME), 'rb') as handle:
            self.assertEqual(handle.read(1), b' ')
        self.assertEqual(os.path.getsize(os.path.join(self.state_dir, dm.STATE_RESERVE_NAME)), dm.RESERVE_BYTES)

    def test_the_reserve_is_never_created_or_grown_when_the_disk_is_full(self):
        mon = self.monitor()
        with self.assertRaises(FileNotFoundError):
            mon.save_reserve(mon.new_state())
        self.assertFalse(os.path.exists(os.path.join(self.state_dir, dm.STATE_RESERVE_NAME)))
        self.run_unsaveable(mon, 1)
        self.assertIn('saving the reserve state copy failed too', self.out.getvalue())
        mon.run_once()
        state = mon.new_state()
        state['samples'] = [{'t': T0 - k, 'fs': {'/mount/' + 'x' * 1000: k}, 'p': k} for k in range(dm.SAMPLES_MAX)]
        mon.save_reserve(state)                          # too big: the oldest samples are shed to fit
        self.assertEqual(os.path.getsize(os.path.join(self.state_dir, dm.STATE_RESERVE_NAME)), dm.RESERVE_BYTES)
        loaded, problem = mon.read_state_file(dm.STATE_RESERVE_NAME)
        self.assertIsNone(problem)
        self.assertLess(len(loaded['samples']), dm.SAMPLES_MAX)
        self.assertEqual(loaded['samples'][-1]['t'], T0 - dm.SAMPLES_MAX + 1)

    def short_writes(self, behaviour):
        real = os.write
        calls = []

        def fake(fd, data):
            calls.append(len(data))
            if behaviour == 'short-then-enospc':
                if len(calls) > 1:
                    raise OSError(28, 'No space left on device')
                return real(fd, bytes(data[:100]))
            if behaviour == 'zero':
                return 0
            real(fd, bytes(data[:100]))                   # a write that claims more than it wrote
            return len(data)
        return mock.patch.object(dm.os, 'write', fake)

    def test_a_short_write_never_replaces_the_good_state(self):
        mon = self.monitor()
        self.system.set_percent('/', 96)
        mon.run_once()
        path = os.path.join(self.state_dir, dm.STATE_NAME)
        with open(path, 'rb') as handle:
            good = handle.read()
        for behaviour in ('short-then-enospc', 'zero', 'lying'):
            with self.subTest(behaviour=behaviour):
                state = mon.load_state()
                with self.short_writes(behaviour), self.assertRaises(OSError):
                    mon.save_state(state)
                with open(path, 'rb') as handle:
                    self.assertEqual(handle.read(), good)
        self.assertEqual(mon.load_state()['alerts']['disk:/']['level'], dm.EMERGENCY)


class Clock(Base):
    def test_a_clock_that_was_ahead_once_does_not_silence_re_alerts(self):
        mon = self.monitor()
        self.system.set_percent('/', 96)
        self.run_every(mon, 900, 4 * 24)                 # one day of history
        real = self.system.t
        self.system.t = real + 30 * 86400                # one run 30 days ahead (before NTP)
        mon.run_once()
        self.system.t = real
        before = len(self.channel.sent)
        self.run_every(mon, 900, 4 * 48)                 # 48 h at the right time
        later = [t for t in self.texts()[before:] if 'STILL EMERGENCY' in t]
        self.assertGreaterEqual(len(later), 47)
        self.assertIn('CLOCK STEPPED BACK', self.texts()[before])
        self.assertGreater(len(self.state()['samples']), 4 * 24, 'the history from before was kept')

    def test_a_clock_behind_keeps_the_history(self):
        mon = self.monitor()
        self.run_every(mon, 900, 4 * 24)
        samples = len(self.state()['samples'])
        real = self.system.t
        self.system.t = real - 365 * 86400               # one run a year behind
        self.assertEqual(mon.run_once(), 0)
        self.assertEqual(len(self.state()['samples']), samples, 'nothing dropped, nothing recorded')
        self.assertIn('WARNING: the system clock is behind the last recorded check', self.texts()[-1])
        self.system.t = real + 900
        mon.run_once()
        self.assertEqual(len(self.state()['samples']), samples + 1)
        self.assertIn('RECOVERED', self.texts()[-1])
        self.assertNotIn('MONITORING GAP', self.texts()[-1])


class AccelerationSteps(Base):
    def test_a_one_off_step_is_not_accelerating_growth(self):
        used = 15 * GB
        for step_age in (6 * 3600, 13 * 3600, 1800):
            with self.subTest(step_age=step_age):
                shutil.rmtree(self.state_dir)
                os.mkdir(self.state_dir)
                mon = self.monitor()
                points = [(age, used - (600 * 10 ** 6 if age > step_age else 0), None)
                          for age in list(range(900, 26 * 3600, 900)) + list(range(26 * 3600, 8 * 86400, 3600))]
                self.system.set_used('/', used)
                Growth.seed(self, mon, points)
                mon.run_once()
                self.assertNotIn('accel:/', self.state()['alerts'])


class Encoding(Base):
    def write(self, name, data):
        path = os.path.join(self.tmp, name)
        with open(path, 'wb') as handle:
            handle.write(data)
        os.chmod(path, 0o600)
        return path

    def test_a_byte_that_is_not_utf8_drops_its_line_only(self):
        conf = self.write('monitor.conf', b'SERVER_NAME=founder-2\nSMTP_FROM=caf\xe9@x\n')
        secret = self.write('secret.env', b'TELEGRAM_BOT_TOKEN=123:abcdef\nTELEGRAM_CHAT_ID=42\n'
                                          b'SMTP_PASSWORD=p\xa0wd-secret\n')
        config = dm.Config.load(conf, secret, self.system)
        self.assertEqual(config['SERVER_NAME'], 'founder-2')
        self.assertEqual(config.secrets, {'TELEGRAM_BOT_TOKEN': '123:abcdef', 'TELEGRAM_CHAT_ID': '42'})
        joined = ' '.join(config.warnings)
        self.assertIn('monitor.conf line 2: not valid UTF-8, ignored', joined)
        self.assertIn('secret.env line 3: not valid UTF-8, ignored', joined)
        self.assertNotIn('wd-secret', joined)

    def test_the_commands_still_run_and_report(self):
        conf = self.write('monitor.conf', f'POCOL_DATA_DIR={self.data}\n'.encode() + b'X=\xff\n')
        secret = self.write('secret.env', b'TELEGRAM_BOT_TOKEN=\xa0\xa0\n')
        args = ['--config', conf, '--secrets', secret, '--state-dir', self.state_dir]
        self.assertEqual(dm.main(['--notify-failure', 'pocol-disk-monitor.service'] + args, system=self.system,
                                 out=self.out), 1, 'no channel left: non-zero, but no crash')
        self.assertEqual(dm.main(['--once'] + args, system=self.system, out=self.out), 0)
        self.assertNotIn('Traceback', self.out.getvalue())
        self.assertIn('no channel configured', self.out.getvalue())


# ------------------------------------------------------------------------------- 4. real channels, local servers

class LocalCA:
    """A throw-away CA and server certificates made with openssl, for servers on 127.0.0.1 only."""

    def __init__(self, case):
        openssl = need_tool(case, 'openssl')
        self.dir = tempfile.mkdtemp(prefix='dm-ca-')
        case.addCleanup(shutil.rmtree, self.dir, True)
        p = lambda n: os.path.join(self.dir, n)
        run = lambda *a: subprocess.run([openssl, *a], check=True, capture_output=True)
        run('req', '-x509', '-newkey', 'ec', '-pkeyopt', 'ec_paramgen_curve:prime256v1', '-nodes', '-keyout',
            p('ca.key'), '-out', p('ca.pem'), '-days', '2', '-subj', '/CN=dm-test-ca', '-addext',
            'basicConstraints=critical,CA:TRUE', '-addext', 'keyUsage=critical,keyCertSign,cRLSign')
        for name, san in (('good', 'DNS:localhost,IP:127.0.0.1'), ('wrong', 'DNS:wrong-name.invalid')):
            run('req', '-newkey', 'ec', '-pkeyopt', 'ec_paramgen_curve:prime256v1', '-nodes', '-keyout',
                p(f'{name}.key'), '-out', p(f'{name}.csr'), '-subj', '/CN=dm-test-server')
            with open(p(f'{name}.ext'), 'w') as handle:
                handle.write(f'subjectAltName={san}\nbasicConstraints=CA:FALSE\nkeyUsage=digitalSignature\n'
                             'extendedKeyUsage=serverAuth\nsubjectKeyIdentifier=hash\n'
                             'authorityKeyIdentifier=keyid,issuer\n')
            run('x509', '-req', '-in', p(f'{name}.csr'), '-CA', p('ca.pem'), '-CAkey', p('ca.key'),
                '-CAcreateserial', '-out', p(f'{name}.pem'), '-days', '2', '-extfile', p(f'{name}.ext'))
        self.ca = p('ca.pem')
        self.cert = lambda name: (p(f'{name}.pem'), p(f'{name}.key'))

    def server_context(self, name='good'):
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(*self.cert(name))
        return context


class FakeHTTPS:
    """A local HTTPS server that answers like the Telegram Bot API and a heartbeat service."""

    def __init__(self, case, context):
        self.requests = []
        outer = self

        class Handler(http.server.BaseHTTPRequestHandler):
            def log_message(self, *a):
                pass

            def answer(self, body):
                length = int(self.headers.get('Content-Length') or 0)
                outer.requests.append((self.command, self.path, self.rfile.read(length).decode()))
                payload = json.dumps(body).encode()
                self.send_response(200 if body.get('ok', True) else 400)
                self.send_header('Content-Length', str(len(payload)))
                self.end_headers()
                self.wfile.write(payload)

            def do_POST(self):
                self.answer({'ok': '/bot' in self.path and not self.path.startswith('/botBAD'),
                             'description': 'Unauthorized'})

            def do_GET(self):
                self.answer({'ok': True})

        self.server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        self.server.socket = context.wrap_socket(self.server.socket, server_side=True)
        self.port = self.server.server_address[1]
        thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        thread.start()
        case.addCleanup(self.server.shutdown)
        case.addCleanup(self.server.server_close)


class FakeSMTP:
    """A minimal local SMTP server: EHLO, STARTTLS (or implicit TLS), AUTH PLAIN, MAIL, RCPT, DATA, QUIT."""

    def __init__(self, case, context, implicit_tls=False, offer_starttls=True, user='monitor', password='pw',
                 refuse_rcpt=(), sessions=1):
        self.context, self.implicit, self.offer = context, implicit_tls, offer_starttls
        self.refuse_rcpt, self.sessions, self.rcpts = set(refuse_rcpt), sessions, []
        self.user, self.password = user, password
        self.messages, self.auth, self.tls, self.plain_commands = [], [], [], []
        self.sock = socket.socket()
        self.sock.bind(('127.0.0.1', 0))
        self.sock.listen(5)
        self.sock.settimeout(10)
        self.port = self.sock.getsockname()[1]
        self.thread = threading.Thread(target=self.serve, daemon=True)
        self.thread.start()
        case.addCleanup(self.sock.close)

    def serve(self):
        for _ in range(self.sessions):
            try:
                conn, _ = self.sock.accept()
            except OSError:
                return
            self.conns = [conn]
            self.rcpts.append([])
            try:
                self.session(conn)
            except (OSError, ssl.SSLError):
                pass
            finally:
                for item in self.conns:
                    item.close()

    def session(self, conn):
        tls = False
        if self.implicit:
            conn = self.context.wrap_socket(conn, server_side=True)
            self.conns.append(conn)
            tls = True
        buf = b''

        def send(line):
            conn.sendall(line.encode() + b'\r\n')

        def readline():
            nonlocal buf
            while b'\r\n' not in buf:
                chunk = conn.recv(4096)
                if not chunk:
                    raise OSError('closed')
                buf += chunk
            line, buf = buf.split(b'\r\n', 1)
            return line.decode()
        send('220 fake ESMTP')
        while True:
            line = readline()
            verb = line.split(' ', 1)[0].upper()
            if not tls:
                self.plain_commands.append(verb)
            if verb in ('EHLO', 'HELO'):
                caps = ['250-fake']
                if self.offer and not tls:
                    caps.append('250-STARTTLS')
                if tls:
                    caps.append('250-AUTH PLAIN')
                send('\r\n'.join(caps + ['250 OK']))
            elif verb == 'STARTTLS':
                send('220 go ahead')
                conn = self.context.wrap_socket(conn, server_side=True)
                self.conns.append(conn)
                tls, buf = True, b''
            elif verb == 'AUTH':
                _, mech, data = (line.split(' ') + [''])[:3]
                _, user, password = base64.b64decode(data).decode().split('\0')
                self.auth.append((user, password, tls))
                send('235 ok' if (user, password) == (self.user, self.password) else '535 bad credentials')
            elif verb == 'RCPT':
                address = line.split(':', 1)[1].strip().strip('<>') if ':' in line else ''
                if address in self.refuse_rcpt:
                    send('450 mailbox busy, try later')
                else:
                    self.rcpts[-1].append(address)
                    send('250 ok')
            elif verb in ('MAIL', 'RSET', 'NOOP'):
                send('250 ok')
            elif verb == 'DATA':
                send('354 end with .')
                lines = []
                while True:
                    item = readline()
                    if item == '.':
                        break
                    lines.append(item[1:] if item.startswith('..') else item)
                self.messages.append(('\n'.join(lines), tls))
                self.tls.append(tls)
                send('250 queued')
            elif verb == 'QUIT':
                send('221 bye')
                return
            else:
                send('502 no')


@mock.patch.dict(os.environ, {'no_proxy': '127.0.0.1,localhost', 'NO_PROXY': '127.0.0.1,localhost'})
class RealChannels(Base):
    def setUp(self):
        super().setUp()
        self.ca = LocalCA(self)

    def telegram(self, token='123456:TESTtoken', ca=True, cert='good', base_host='127.0.0.1'):
        server = FakeHTTPS(self, self.ca.server_context(cert))
        transport = dm.HttpsTransport(self.ca.ca if ca else '', 5)
        return server, dm.Telegram(token, '42', f'https://{base_host}:{server.port}', transport, 5)

    def test_telegram_over_verified_https(self):
        server, channel = self.telegram()
        channel.send('subject', '[CRITICAL] text\nسطر عربي')
        method, path, body = server.requests[0]
        self.assertEqual((method, path), ('POST', '/bot123456:TESTtoken/sendMessage'))
        self.assertIn('chat_id=42', body)
        self.assertIn('%5BCRITICAL%5D+text', body)

    def test_telegram_refusal_is_a_failure(self):
        server, channel = self.telegram(token='BAD')
        with self.assertRaises(dm.DeliveryError) as caught:
            channel.send('s', 't')
        self.assertIn('HTTP 400', str(caught.exception))

    def test_telegram_untrusted_or_wrong_name_certificate_fails(self):
        _, channel = self.telegram(ca=False)
        with self.assertRaises(dm.DeliveryError) as caught:
            channel.send('s', 't')
        self.assertIn('CERTIFICATE_VERIFY_FAILED', str(caught.exception))
        _, channel = self.telegram(cert='wrong')
        with self.assertRaises(dm.DeliveryError):
            channel.send('s', 't')

    def test_transport_refuses_plain_http_and_always_verifies(self):
        transport = dm.HttpsTransport()
        with self.assertRaises(dm.DeliveryError):
            transport('GET', 'http://127.0.0.1:1/x', None, {})
        context = transport.context()
        self.assertEqual(context.verify_mode, ssl.CERT_REQUIRED)
        self.assertTrue(context.check_hostname)

    def test_email_with_starttls_and_auth(self):
        server = FakeSMTP(self, self.ca.server_context())
        channel = dm.Email('localhost', server.port, 'starttls', 'monitor', 'pw', 'mon@example.test',
                           ['owner@example.test'], self.ca.ca, 5, 'test.local')
        channel.send('[WARNING] subject', 'body line\nسطر')
        server.thread.join(5)
        self.assertEqual(server.auth, [('monitor', 'pw', True)], 'the password only travels inside TLS')
        self.assertEqual(server.tls, [True])
        self.assertEqual(server.plain_commands, ['EHLO', 'STARTTLS'])
        self.assertIn('Subject: [WARNING] subject', server.messages[0][0])

    def test_email_refusing_some_recipients_is_a_partial_failure_naming_them(self):
        server = FakeSMTP(self, self.ca.server_context(), refuse_rcpt={'second@example.test'})
        channel = dm.Email('localhost', server.port, 'starttls', 'monitor', 'pw', 'mon@example.test',
                           ['owner@example.test', 'second@example.test'], self.ca.ca, 5, 'test.local')
        with self.assertRaises(dm.PartialDelivery) as caught:
            channel.send('[CRITICAL] s', 't')
        server.thread.join(5)
        self.assertEqual(caught.exception.refused, ['second@example.test'])
        self.assertIn('1 of 2 recipient(s) refused (SMTP 450)', str(caught.exception))
        self.assertEqual(server.rcpts, [['owner@example.test']])
        self.assertEqual(len(server.messages), 1, 'the accepted recipient got it')

    def test_a_partly_refused_alert_stays_queued_and_only_the_refused_recipient_gets_it_again(self):
        server = FakeSMTP(self, self.ca.server_context(), refuse_rcpt={'second@example.test'}, sessions=2)
        channel = dm.Email('localhost', server.port, 'starttls', 'monitor', 'pw', 'mon@example.test',
                           ['owner@example.test', 'second@example.test'], self.ca.ca, 5, 'test.local')
        mon = self.monitor(channels={'email': channel})
        self.system.set_percent('/', 96)
        mon.run_once()
        queue = self.state()['queue']['email']
        self.assertEqual(len(queue), 1, 'a partial delivery is not a delivery')
        self.assertEqual(queue[0]['pending_to'], ['second@example.test'], 'kept across the state file')
        self.assertIn('partly delivered (1 of 2 recipient(s) refused (SMTP 450))', self.out.getvalue())
        server.refuse_rcpt.clear()
        self.system.t += 60
        mon2 = self.monitor(channels={'email': channel})   # a later run reloads the queue from the state file
        mon2.run_once()
        server.thread.join(5)
        self.assertEqual(server.rcpts, [['owner@example.test'], ['second@example.test']],
                         'the retry goes to the refused recipient only')
        self.assertEqual(self.state()['queue']['email'], [])

    def test_test_alert_fails_when_a_recipient_is_refused(self):
        server = FakeSMTP(self, self.ca.server_context(), refuse_rcpt={'second@example.test'})
        channel = dm.Email('localhost', server.port, 'starttls', 'monitor', 'pw', 'mon@example.test',
                           ['owner@example.test', 'second@example.test'], self.ca.ca, 5, 'test.local')
        mon = self.monitor(channels={'email': channel})
        self.assertEqual(mon.test_alert(), 1)
        server.thread.join(5)
        self.assertIn('email: FAILED (1 of 2 recipient(s) refused (SMTP 450))', self.out.getvalue())

    def test_email_smtps(self):
        server = FakeSMTP(self, self.ca.server_context(), implicit_tls=True)
        channel = dm.Email('127.0.0.1', server.port, 'ssl', 'monitor', 'pw', 'mon@example.test',
                           ['owner@example.test'], self.ca.ca, 5, 'test.local')
        channel.send('s', 't')
        server.thread.join(5)
        self.assertEqual(server.tls, [True])

    def test_email_never_falls_back_to_plain_text(self):
        server = FakeSMTP(self, self.ca.server_context(), offer_starttls=False)
        channel = dm.Email('localhost', server.port, 'starttls', 'monitor', 'pw', 'mon@example.test',
                           ['owner@example.test'], self.ca.ca, 5, 'test.local')
        with self.assertRaises(Exception):
            channel.send('s', 't')
        server.thread.join(5)
        self.assertEqual(server.messages, [])
        self.assertEqual(server.auth, [])

    def test_email_untrusted_certificate_fails(self):
        server = FakeSMTP(self, self.ca.server_context())
        channel = dm.Email('localhost', server.port, 'starttls', 'monitor', 'pw', 'mon@example.test',
                           ['owner@example.test'], '', 5, 'test.local')
        with self.assertRaises(ssl.SSLError):
            channel.send('s', 't')
        server.thread.join(5)
        self.assertEqual(server.auth, [])

    def write_files(self, conf, secrets):
        conf_path = os.path.join(self.tmp, 'monitor.conf')
        secret_path = os.path.join(self.tmp, 'secret.env')
        with open(conf_path, 'w') as handle:
            handle.write(self.CONF + f'POCOL_DATA_DIR={self.data}\nCA_FILE={self.ca.ca}\n' + conf)
        with open(secret_path, 'w') as handle:
            handle.write(secrets)
        os.chmod(secret_path, 0o600)
        return ['--config', conf_path, '--secrets', secret_path, '--state-dir', self.state_dir]

    def test_end_to_end_cli_through_both_channels(self):
        https = FakeHTTPS(self, self.ca.server_context())
        smtp = [FakeSMTP(self, self.ca.server_context()) for _ in range(3)]
        args = self.write_files(
            f'TELEGRAM_API_BASE=https://127.0.0.1:{https.port}\nSMTP_HOST=localhost\nSMTP_PORT={smtp[0].port}\n'
            'SMTP_USER=monitor\nSMTP_FROM=mon@example.test\nSMTP_TO=owner@example.test\n',
            f'TELEGRAM_BOT_TOKEN=111:e2eTOKEN\nTELEGRAM_CHAT_ID=42\nSMTP_PASSWORD=pw\n'
            f'HEARTBEAT_URL=https://127.0.0.1:{https.port}/ping/e2e-uuid\n')
        self.assertEqual(dm.main(['--test-alert'] + args, system=self.system, out=self.out), 0)
        out = self.out.getvalue()
        self.assertIn('telegram: delivered', out)
        self.assertIn('email: delivered', out)
        self.assertIn('heartbeat: ok', out)
        self.assertNotIn('e2eTOKEN', out)
        paths = [p for _, p, _ in https.requests]
        self.assertIn('/bot111:e2eTOKEN/sendMessage', paths)
        self.assertIn('/ping/e2e-uuid', paths)
        smtp[0].thread.join(5)
        self.assertIn('[TEST] PoCol disk monitor test notification', smtp[0].messages[0][0])
        # --simulate-usage 96 through the same channels (the next SMTP server answers the next connection).
        args[args.index('--config') + 1:args.index('--config') + 2] = [self.write_files(
            f'TELEGRAM_API_BASE=https://127.0.0.1:{https.port}\nSMTP_HOST=localhost\nSMTP_PORT={smtp[1].port}\n'
            'SMTP_USER=monitor\nSMTP_FROM=mon@example.test\nSMTP_TO=owner@example.test\n',
            'TELEGRAM_BOT_TOKEN=111:e2eTOKEN\nTELEGRAM_CHAT_ID=42\nSMTP_PASSWORD=pw\n')[1]]
        self.assertEqual(dm.main(['--simulate-usage', '96'] + args, system=self.system, out=self.out), 0)
        smtp[1].thread.join(5)
        self.assertIn('[TEST] [EMERGENCY]', smtp[1].messages[0][0])
        self.assertEqual(os.listdir(self.state_dir), [], 'simulation keeps the real state untouched')
        # A wrong SMTP password: --test-alert exits non-zero and says which channel failed.
        self.write_files(
            f'TELEGRAM_API_BASE=https://127.0.0.1:{https.port}\nSMTP_HOST=localhost\nSMTP_PORT={smtp[2].port}\n'
            'SMTP_USER=monitor\nSMTP_FROM=mon@example.test\nSMTP_TO=owner@example.test\n',
            'TELEGRAM_BOT_TOKEN=111:e2eTOKEN\nTELEGRAM_CHAT_ID=42\nSMTP_PASSWORD=wrong-password-value\n')
        self.out = io.StringIO()
        self.assertEqual(dm.main(['--test-alert'] + args, system=self.system, out=self.out), 1)
        self.assertIn('email: FAILED', self.out.getvalue())
        self.assertIn('telegram: delivered', self.out.getvalue())
        self.assertNotIn('wrong-password-value', self.out.getvalue())


if __name__ == '__main__':
    unittest.main()
