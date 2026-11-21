#!/usr/bin/env python3
"""Tests of the disk monitor's systemd units and of its installer and uninstaller (OWNER-DECISIONS-2026-11-21 D7).

The units are parsed and checked, then verified with `systemd-analyze verify` (and scored with `systemd-analyze security
--offline`) when systemd-analyze is installed. The installer and uninstaller are checked with `bash -n`, with shellcheck
when installed, and run in a temporary root with a fake systemctl and a fake id, never on this host's /. A missing tool
skips its test with the reason printed (a skip is not a pass); POCOL_MONITOR_REQUIRE_TOOLS=1 (CI) makes it a failure.
"""
import hashlib
import os
import re
import shutil
import stat
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
UNITS = os.path.join(HERE, 'systemd')
INSTALL = os.path.join(HERE, 'install-disk-monitor.sh')
UNINSTALL = os.path.join(HERE, 'uninstall-disk-monitor.sh')
REQUIRE_TOOLS = os.environ.get('POCOL_MONITOR_REQUIRE_TOOLS') == '1'
OWN_UNITS = ('pocol-disk-monitor.service', 'pocol-disk-monitor.timer', 'pocol-disk-monitor-failure.service')


def need_tool(case, name):
    path = shutil.which(name)
    if path:
        return path
    if REQUIRE_TOOLS:
        case.fail(f'{name} is required here (POCOL_MONITOR_REQUIRE_TOOLS=1) but not installed')
    print(f'\nSKIPPED (not passed): {name} is not installed', file=sys.stderr)
    case.skipTest(f'{name} is not installed')


def parse_unit(name):
    """{section: [(key, value), ...]} keeping repeated keys, as systemd reads them."""
    sections, current = {}, None
    with open(os.path.join(UNITS, name), encoding='utf-8') as handle:
        for line in handle:
            line = line.strip()
            if not line or line.startswith('#'):
                continue
            if line.startswith('['):
                current = sections.setdefault(line.strip('[]'), [])
                continue
            key, _, value = line.partition('=')
            current.append((key, value))
    return sections


def values(sections, section, key):
    return [v for k, v in sections.get(section, []) if k == key]


def one(sections, section, key):
    found = values(sections, section, key)
    if len(found) != 1:
        raise AssertionError(f'{section}.{key}: {found}')
    return found[0]


class Units(unittest.TestCase):
    def test_the_service_is_a_low_priority_hardened_oneshot(self):
        s = parse_unit('pocol-disk-monitor.service')
        expect = {'Type': 'oneshot', 'Nice': '19', 'IOSchedulingClass': 'idle', 'ProtectSystem': 'strict',
                  'ProtectHome': 'read-only', 'PrivateTmp': 'yes', 'NoNewPrivileges': 'yes', 'MemoryMax': '64M',
                  'MemorySwapMax': '0', 'CPUQuota': '20%', 'CapabilityBoundingSet': 'CAP_DAC_READ_SEARCH',
                  'AmbientCapabilities': '', 'StateDirectory': 'pocol-disk-monitor', 'StateDirectoryMode': '0700',
                  'PrivateDevices': 'yes', 'ProtectKernelTunables': 'yes', 'ProtectControlGroups': 'yes',
                  'RestrictSUIDSGID': 'yes', 'MemoryDenyWriteExecute': 'yes', 'UMask': '0077', 'User': 'root'}
        for key, value in expect.items():
            self.assertEqual(one(s, 'Service', key), value, key)
        self.assertEqual(values(s, 'Service', 'ReadWritePaths'), ['/var/lib/pocol-disk-monitor'],
                         'it may write only its own state directory')
        self.assertEqual(one(s, 'Service', 'ExecStart'),
                         '/usr/bin/python3 -I -B /opt/pocol-disk-monitor/pocol_disk_monitor.py --once')
        self.assertEqual(one(s, 'Unit', 'OnFailure'), 'pocol-disk-monitor-failure.service')
        # It runs one check and exits: no restart loop, no other command before or after it.
        for key in ('Restart', 'ExecStartPre', 'ExecStartPost', 'ExecStop', 'ExecStopPost', 'ExecReload',
                    'ExecCondition'):
            self.assertEqual(values(s, 'Service', key), [], key)
        self.assertIn('-/var/lib/pocol-standalone/identity.pocol-key', values(s, 'Service', 'InaccessiblePaths'),
                      'the miner key is masked from the monitor')
        self.assertNotIn('Install', s, 'started by the timer only')

    def test_key_material_is_masked_and_the_docs_say_what_is_not(self):
        for name in ('pocol-disk-monitor.service', 'pocol-disk-monitor-failure.service'):
            s = parse_unit(name)
            masked = values(s, 'Service', 'InaccessiblePaths')
            for path in ('/var/lib/pocol-standalone/identity.pocol-key',
                         '/var/lib/pocol-standalone/identity.pocol-key.restoring', '/etc/credstore.encrypted',
                         '/etc/credstore', '/var/lib/systemd/credential.secret', '/var/lib/pocol-faucet',
                         '/etc/pocol-testnet', '/var/backups', '/root'):
                self.assertIn('-' + path, masked, f'{name}: {path}')
            self.assertIn('time-sync.target', one(s, 'Unit', 'After').split(), name)
        with open(os.path.join(UNITS, 'pocol-disk-monitor.service'), encoding='utf-8') as handle:
            unit = handle.read()
        with open(os.path.join(HERE, 'README.md'), encoding='utf-8') as handle:
            readme = handle.read()
        for text in (unit, readme):
            self.assertIn('identity.pocol-key.replaced-', text)
            self.assertNotRegex(text, r'and (to )?read its own root-only\s+secret\.env', 'the capability is not for it')
        self.assertNotIn('the miner key file is masked from it', readme)

    def test_the_readme_matches_the_code(self):
        with open(os.path.join(HERE, 'README.md'), encoding='utf-8') as handle:
            readme = ' '.join(handle.read().split())
        for text in ('state.json.corrupt', 'state.reserve', 'sudo bash ./install-disk-monitor.sh',
                     'below 80% of either limit', 'REALERT_MONITOR_FAILED_HOURS'):
            self.assertIn(text, readme)
        self.assertNotIn('below 80% of both', readme)
        self.assertNotIn('sudo ./install-disk-monitor.sh', readme)

    def test_the_failure_unit_reports_and_has_the_same_sandbox(self):
        main = parse_unit('pocol-disk-monitor.service')
        failure = parse_unit('pocol-disk-monitor-failure.service')
        self.assertEqual(one(failure, 'Service', 'ExecStart'),
                         '/usr/bin/python3 -I -B /opt/pocol-disk-monitor/pocol_disk_monitor.py '
                         '--notify-failure pocol-disk-monitor.service')
        self.assertEqual(values(failure, 'Unit', 'OnFailure'), [], 'no failure loop')
        strip = lambda sec: [(k, v) for k, v in sec['Service'] if k != 'ExecStart']
        self.assertEqual(strip(failure), strip(main))

    def test_the_timer_runs_every_15_minutes_and_catches_up(self):
        t = parse_unit('pocol-disk-monitor.timer')
        self.assertEqual(one(t, 'Timer', 'OnCalendar'), '*:0/15')
        self.assertEqual(one(t, 'Timer', 'Persistent'), 'true')
        self.assertLessEqual(int(one(t, 'Timer', 'RandomizedDelaySec')), 120)
        self.assertEqual(one(t, 'Timer', 'Unit'), 'pocol-disk-monitor.service')
        self.assertEqual(one(t, 'Install', 'WantedBy'), 'timers.target')

    def test_the_gap_alert_covers_more_than_two_missed_runs(self):
        sys.path.insert(0, HERE)
        import pocol_disk_monitor as dm
        self.assertGreater(dm.SETTINGS['GAP_ALERT_MINUTES'][1], 15 + 2)
        self.assertLess(dm.SETTINGS['GAP_ALERT_MINUTES'][1], 60)

    def verify_dir(self):
        work = tempfile.mkdtemp(prefix='dm-units-')
        self.addCleanup(shutil.rmtree, work, True)
        for name in OWN_UNITS:
            shutil.copy(os.path.join(UNITS, name), work)
        return work

    def test_systemd_analyze_verify(self):
        analyze = need_tool(self, 'systemd-analyze')
        if not os.path.exists('/usr/bin/python3'):
            self.skipTest('/usr/bin/python3, the units\' interpreter, is missing on this host')
        work = self.verify_dir()
        env = dict(os.environ, SYSTEMD_UNIT_PATH=work + ':')
        run = subprocess.run([analyze, 'verify', '--man=no'] + [os.path.join(work, n) for n in OWN_UNITS],
                             capture_output=True, text=True, env=env)
        output = (run.stdout + run.stderr).strip()
        self.assertEqual(run.returncode, 0, output)
        ours = [line for line in output.splitlines() if 'pocol-disk-monitor' in line]
        self.assertEqual(ours, [], 'unknown keys and other warnings about these units')
        # The check sees a broken unit.
        bad = os.path.join(work, 'pocol-disk-monitor.service')
        with open(bad, 'a') as handle:
            handle.write('Nicee=19\n')
        run = subprocess.run([analyze, 'verify', '--man=no', bad], capture_output=True, text=True, env=env)
        self.assertIn("Unknown key name 'Nicee'", run.stdout + run.stderr)

    def test_systemd_analyze_security_exposure(self):
        analyze = need_tool(self, 'systemd-analyze')
        work = self.verify_dir()
        run = subprocess.run([analyze, 'security', '--offline=true', os.path.join(work, 'pocol-disk-monitor.service')],
                             capture_output=True, text=True)
        if 'offline' in run.stderr and run.returncode != 0 and 'Unknown option' in run.stderr:
            self.skipTest('this systemd-analyze has no --offline')
        m = re.search(r'Overall exposure level for pocol-disk-monitor\.service: ([\d.]+)', run.stdout)
        self.assertIsNotNone(m, run.stdout[-500:] + run.stderr[-500:])
        print(f'\nsystemd-analyze security --offline: exposure {m.group(1)}', file=sys.stderr)
        self.assertLessEqual(float(m.group(1)), 3.0)


def tree(root):
    """{relative path: (mode, sha256 or '')} of everything under root."""
    found = {}
    for directory, dirs, files in os.walk(root):
        for name in dirs + files:
            path = os.path.join(directory, name)
            st = os.lstat(path)
            digest = ''
            if stat.S_ISREG(st.st_mode):
                with open(path, 'rb') as handle:
                    digest = hashlib.sha256(handle.read()).hexdigest()
            found[os.path.relpath(path, root)] = (stat.S_IMODE(st.st_mode), digest, st.st_mtime_ns
                                                  if stat.S_ISREG(st.st_mode) else 0)
    return found


class Installer(unittest.TestCase):
    def setUp(self):
        self.root = tempfile.mkdtemp(prefix='dm-root-')
        self.addCleanup(shutil.rmtree, self.root, True)
        self.bin = tempfile.mkdtemp(prefix='dm-bin-')
        self.addCleanup(shutil.rmtree, self.bin, True)
        self.log = os.path.join(self.bin, 'systemctl.log')
        with open(os.path.join(self.bin, 'systemctl'), 'w') as handle:
            handle.write('#!/bin/sh\necho "$*" >> "%s"\n' % self.log)
        with open(os.path.join(self.bin, 'id'), 'w') as handle:
            handle.write('#!/bin/sh\nif [ "$1" = -u ]; then echo "${FAKE_UID:-0}"; else exec /usr/bin/id "$@"; fi\n')
        for name in ('systemctl', 'id'):
            os.chmod(os.path.join(self.bin, name), 0o755)
        # PoCol's own install, which neither script may touch.
        self.pocol = {}
        for rel, content in (('var/lib/pocol-standalone/state/blocks.journal', b'journal'),
                             ('var/lib/pocol-standalone/identity.pocol-key', b'secret key'),
                             ('etc/systemd/system/pocol-standalone.service', b'[Service]\n'),
                             ('opt/pocol-standalone/pocol', b'binary'),
                             ('etc/ufw/user.rules', b'-A ufw-user-input -p tcp --dport 22 -j ACCEPT\n')):
            path = os.path.join(self.root, rel)
            os.makedirs(os.path.dirname(path), exist_ok=True)
            with open(path, 'wb') as handle:
                handle.write(content)
            self.pocol[rel] = content

    def run_script(self, script, *args, uid='0'):
        env = dict(os.environ, PATH=self.bin + os.pathsep + os.environ['PATH'], POCOL_MONITOR_ROOT=self.root,
                   FAKE_UID=uid)
        return subprocess.run(['bash', script, *args], capture_output=True, text=True, env=env)

    def calls(self):
        if not os.path.exists(self.log):
            return []
        with open(self.log) as handle:
            return handle.read().splitlines()

    def assert_pocol_untouched(self):
        for rel, content in self.pocol.items():
            with open(os.path.join(self.root, rel), 'rb') as handle:
                self.assertEqual(handle.read(), content, rel)

    def path(self, rel):
        return os.path.join(self.root, rel)

    def mode(self, rel):
        return stat.S_IMODE(os.stat(self.path(rel)).st_mode)

    def test_bash_syntax(self):
        for script in (INSTALL, UNINSTALL):
            run = subprocess.run(['bash', '-n', script], capture_output=True, text=True)
            self.assertEqual(run.returncode, 0, run.stderr)
            with open(script) as handle:
                text = handle.read()
            self.assertTrue(text.startswith('#!/usr/bin/env bash\n'))
            self.assertIn('set -euo pipefail', text)

    def test_every_file_keeps_lf_on_every_checkout(self):
        # The owner may copy this directory from a Windows checkout (core.autocrlf=true): CRLF would break bash and
        # the units, so .gitattributes pins LF for deploy/monitoring.
        if subprocess.run(['git', 'rev-parse', '--git-dir'], cwd=HERE, capture_output=True).returncode != 0:
            self.skipTest('not a git checkout')
        names = sorted(os.path.relpath(os.path.join(d, f), HERE) for d, _, fs in os.walk(HERE) for f in fs
                       if '__pycache__' not in d)
        run = subprocess.run(['git', 'check-attr', 'eol', '--'] + names, cwd=HERE, capture_output=True, text=True)
        self.assertEqual(run.returncode, 0, run.stderr)
        for line in run.stdout.splitlines():
            self.assertTrue(line.endswith(': eol: lf'), line)
        for name in names:
            with open(os.path.join(HERE, name), 'rb') as handle:
                self.assertNotIn(b'\r', handle.read(), name)

    def test_shellcheck(self):
        shellcheck = need_tool(self, 'shellcheck')
        run = subprocess.run([shellcheck, '--shell=bash', INSTALL, UNINSTALL], capture_output=True, text=True)
        self.assertEqual(run.returncode, 0, run.stdout + run.stderr)

    def test_refuses_to_run_as_non_root(self):
        for script in (INSTALL, UNINSTALL):
            run = self.run_script(script, uid='1000')
            self.assertEqual(run.returncode, 1)
            self.assertIn('Run with sudo', run.stderr)
        self.assertEqual(self.calls(), [])
        self.assertFalse(os.path.exists(self.path('opt/pocol-disk-monitor')))

    def test_install_is_idempotent_and_keeps_the_owners_files(self):
        run = self.run_script(INSTALL)
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertEqual(self.calls(), ['daemon-reload', 'enable --now pocol-disk-monitor.timer'])
        for rel, mode in (('opt/pocol-disk-monitor/pocol_disk_monitor.py', 0o755),
                          ('opt/pocol-disk-monitor/uninstall-disk-monitor.sh', 0o755),
                          ('opt/pocol-disk-monitor/README.md', 0o644),
                          ('etc/pocol-disk-monitor/monitor.conf', 0o644),
                          ('etc/pocol-disk-monitor/monitor.conf.example', 0o644),
                          ('etc/pocol-disk-monitor/secret.env', 0o600),
                          ('var/lib/pocol-disk-monitor', 0o700)):
            self.assertEqual(self.mode(rel), mode, rel)
        for unit in OWN_UNITS:
            with open(self.path(f'etc/systemd/system/{unit}'), 'rb') as a, open(os.path.join(UNITS, unit), 'rb') as b:
                self.assertEqual(a.read(), b.read(), unit)
        for step in ('--test-alert', '--simulate-usage 96', 'systemctl list-timers', 'secret.env'):
            self.assertIn(step, run.stdout)
        # The owner writes the configuration and the secrets; a second run keeps them and changes nothing else.
        with open(self.path('etc/pocol-disk-monitor/monitor.conf'), 'w') as handle:
            handle.write('SERVER_NAME=founder-2\n')
        with open(self.path('etc/pocol-disk-monitor/secret.env'), 'w') as handle:
            handle.write('TELEGRAM_BOT_TOKEN=owner-token\n')
        os.chmod(self.path('etc/pocol-disk-monitor/secret.env'), 0o600)
        state = self.path('var/lib/pocol-disk-monitor/state.json')
        with open(state, 'w') as handle:
            handle.write('{"v": 1}')
        before = {k: v[:2] for k, v in tree(self.root).items()}
        run = self.run_script(INSTALL)
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertIn('Kept the existing', run.stdout)
        self.assertEqual({k: v[:2] for k, v in tree(self.root).items()}, before)
        with open(self.path('etc/pocol-disk-monitor/secret.env')) as handle:
            self.assertEqual(handle.read(), 'TELEGRAM_BOT_TOKEN=owner-token\n')
        self.assert_pocol_untouched()

    def test_the_installer_checks_the_interpreter_the_units_run(self):
        if not os.access('/usr/bin/python3', os.X_OK):
            self.skipTest('/usr/bin/python3, the units\' interpreter, is missing on this host')
        with open(os.path.join(self.bin, 'python3'), 'w') as handle:   # an older python3 first on PATH
            handle.write('#!/bin/sh\necho "fake python3 on PATH used" >&2\nexit 1\n')
        os.chmod(os.path.join(self.bin, 'python3'), 0o755)
        run = self.run_script(INSTALL)
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertNotIn('fake python3 on PATH used', run.stderr)
        with open(INSTALL) as handle:
            self.assertIn('python=/usr/bin/python3', handle.read())
        for unit in ('pocol-disk-monitor.service', 'pocol-disk-monitor-failure.service'):
            self.assertTrue(one(parse_unit(unit), 'Service', 'ExecStart').startswith('/usr/bin/python3 '))

    def test_the_installed_script_runs(self):
        self.assertEqual(self.run_script(INSTALL).returncode, 0)
        run = subprocess.run([sys.executable, '-I', '-B', self.path('opt/pocol-disk-monitor/pocol_disk_monitor.py'),
                              '--help'], capture_output=True, text=True)
        self.assertEqual(run.returncode, 0, run.stderr)
        for flag in ('--once', '--test-alert', '--simulate-usage', '--report', '--status', '--notify-failure'):
            self.assertIn(flag, run.stdout)

    def test_uninstall_removes_only_its_own_files(self):
        self.assertEqual(self.run_script(INSTALL).returncode, 0)
        run = self.run_script(UNINSTALL)
        self.assertEqual(run.returncode, 0, run.stderr)
        for unit in OWN_UNITS:
            self.assertFalse(os.path.exists(self.path(f'etc/systemd/system/{unit}')))
        self.assertFalse(os.path.exists(self.path('opt/pocol-disk-monitor')))
        self.assertTrue(os.path.exists(self.path('etc/pocol-disk-monitor/secret.env')), 'kept without --purge')
        self.assertTrue(os.path.exists(self.path('var/lib/pocol-disk-monitor')))
        self.assert_pocol_untouched()
        run = self.run_script(UNINSTALL, '--purge')
        self.assertEqual(run.returncode, 0, run.stderr)
        self.assertFalse(os.path.exists(self.path('etc/pocol-disk-monitor')))
        self.assertFalse(os.path.exists(self.path('var/lib/pocol-disk-monitor')))
        self.assert_pocol_untouched()
        self.assertTrue(os.path.exists(self.path('etc/systemd/system/pocol-standalone.service')))
        for call in self.calls():
            if call in ('daemon-reload',):
                continue
            targets = [w for w in call.split()[1:] if not w.startswith('-')]
            self.assertTrue(targets and all(t in OWN_UNITS for t in targets), call)
        self.assertEqual(self.run_script(UNINSTALL, '--bogus').returncode, 2)

    def test_the_scripts_name_nothing_of_pocols_own_install(self):
        with open(INSTALL) as handle:
            install = handle.read()
        with open(UNINSTALL) as handle:
            uninstall = handle.read()
        code = lambda text: '\n'.join(l for l in text.splitlines() if not l.lstrip().startswith('#'))
        for text in (code(install), code(uninstall)):
            for word in ('pocol-standalone', 'ufw', 'iptables', 'nft ', 'kill', 'reboot'):
                self.assertNotIn(word, text)
        self.assertNotRegex(code(install), r'\brm\b')
        self.assertEqual(re.findall(r'^systemctl (.*)', code(install), re.M),
                         ['daemon-reload', 'enable --now pocol-disk-monitor.timer'])
        for target in re.findall(r'\brm -r?f -- (.*)', code(uninstall)):
            for path in target.split():
                self.assertRegex(path, r'^"\$(?:units/\$unit|root/(?:opt|etc|var/lib)/pocol-disk-monitor)"$')


if __name__ == '__main__':
    unittest.main()
