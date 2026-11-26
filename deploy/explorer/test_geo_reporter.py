"""pocol_geo_reporter.py and install_geo_reporter.py (development/explorer/MINERS-MAP-HOLDERS.md §4, §4.1): the
configuration and credential checks, one --dry-run against a fake loopback node (keyed ids, never an IP), the
signature round trip with the gateway's verify_report and Geography, the HTTPS POST (certificate verification
required) and its exit codes, and the installer's --check, install, rollback and --uninstall on temporary paths with
every system command faked. Standard library only; nothing leaves 127.0.0.1."""
import contextlib
import hashlib
import hmac
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import io
import json
import os
from pathlib import Path
import re
import ssl
import stat
import sys
import tempfile
import threading
from types import SimpleNamespace
import unittest
from unittest.mock import patch

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'rewards'))
sys.path.insert(0, str(HERE))
import pocol_geoip as geo  # noqa: E402
import pocol_geo_reporter as reporter  # noqa: E402
import install_geo_reporter as installer  # noqa: E402

SAMPLE = HERE.parent / 'rewards' / 'testdata' / 'geoip-sample.csv'
CHAIN, GENESIS = 777963, '0x' + '5a' * 32
# What a dry run prints (the fake reporter's stdout in the installer tests).
DRY_RUN_REPORT = (b'{"schema":"pocol-geo-report-v1","reporter":"founder-1","head":{"height":30,"hash":"0x' + b'ab' * 32
                  + b'"},"miner":"0x' + b'12' * 20 + b'","peers":[{"id":"' + b'c' * 24 + b'","country":"DE"}]}')
SALT_HEX, KEY_HEX = 'aa' * 32, 'b1' * 32
MINER = '0x' + 'AbCd' * 10
HEAD = {'height': 1000, 'hash': '0x' + 'cd' * 32, 'ts': 1, 'signed': True, 'final': False, 'totalWork': '0x1'}
PEERS = ['5.0.0.10:30300', '5.0.0.10:41234', '[2a00:1450::1]:30300', '10.0.0.7:30300', '[fe80::1%eth0]:30300',
         '45.0.1.1:30300', '[::ffff:9.9.9.9]:30300']
NOW = 1_795_300_000


def node_status(**changes):
    value = {'name': 'founder-1', 'chainId': CHAIN, 'genesisHash': GENESIS, 'head': dict(HEAD), 'peers': list(PEERS),
             'production': 'idle', 'miner': MINER.lower()}
    value.update(changes)
    return value


class FakeNode:
    """A loopback JSON-RPC server answering pocol_status and pocol_getParams (batches included)."""

    def __init__(self, status=None, params=None, http_status=200):
        self.status = status if status is not None else node_status()
        self.params = params if params is not None else {'chainId': CHAIN, 'genesisHash': GENESIS}
        self.http_status = http_status
        self.requests = []
        node = self

        class Handler(BaseHTTPRequestHandler):
            def do_POST(self):
                raw = self.rfile.read(int(self.headers.get('Content-Length', '0')))
                node.requests.append(json.loads(raw))
                calls = node.requests[-1]
                answers = [{'jsonrpc': '2.0', 'id': c['id'], 'result': {'pocol_status': node.status,
                                                                       'pocol_getParams': node.params}[c['method']]}
                           for c in (calls if isinstance(calls, list) else [calls])]
                body = json.dumps(answers).encode()
                self.send_response(node.http_status)
                self.send_header('Content-Length', str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, *_):
                pass

        self.server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        self.port = self.server.server_address[1]
        self.thread = threading.Thread(target=self.server.serve_forever, kwargs={'poll_interval': 0.05}, daemon=True)
        self.thread.start()

    def close(self):
        self.server.shutdown()
        self.server.server_close()


class Files(unittest.TestCase):
    """A temporary configuration, credential and database; the database is closed before the folder is removed."""

    def setUp(self):
        tmp = tempfile.TemporaryDirectory()
        self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name)
        self.database = self.root / 'geoip-country.bin'
        geo.build(SAMPLE, '2026-11', self.database)
        self.config = self.root / 'config.json'
        self.write_config()
        self.credential = self.root / 'credential.json'
        self.credential.write_text(json.dumps({'reporter': 'founder-1', 'key': KEY_HEX, 'salt': SALT_HEX}))

    def write_config(self, **changes):
        value = {'chainId': CHAIN, 'genesisHash': GENESIS, 'explorerUrl': 'https://explorer.example/api/geo-report.php',
                 'database': str(self.database.resolve())}
        value.update(changes)
        self.config.write_text(json.dumps(value))

    def node(self, **kwargs):
        node = FakeNode(**kwargs)
        self.addCleanup(node.close)
        patcher = patch.object(reporter, 'RPC_PORT', node.port)
        patcher.start(); self.addCleanup(patcher.stop)
        return node


class ConfigTests(Files):
    def test_a_valid_configuration(self):
        config = reporter.load_config(self.config)
        self.assertEqual((config['chainId'], config['genesisHash']), (CHAIN, GENESIS))

    def test_refused_configurations(self):
        cases = [dict(explorerUrl='http://explorer.example/api/geo-report.php'),
                 dict(explorerUrl='https://explorer.example/api/rpc.php'),
                 dict(explorerUrl='https://user@explorer.example/api/geo-report.php'),
                 dict(explorerUrl='https://explorer.example/api/geo-report.php?x=1'),
                 dict(explorerUrl='https://EXPLORER.example/api/geo-report.php'),
                 dict(genesisHash=GENESIS.upper().replace('0X', '0x')), dict(genesisHash='0x12'), dict(chainId='777963'),
                 dict(chainId=0), dict(database='relative/geoip.bin'), dict(extra=1)]
        for change in cases:
            with self.subTest(change=change):
                self.write_config(**change)
                with self.assertRaises(reporter.Failure) as caught:
                    reporter.load_config(self.config)
                self.assertEqual(caught.exception.code, reporter.SETUP)
        self.config.write_text('{"chainId": 1, "chainId": 2}')
        with self.assertRaises(reporter.Failure):
            reporter.load_config(self.config)
        with self.assertRaises(reporter.Failure):
            reporter.load_config(self.root / 'absent.json')

    def test_the_credential(self):
        self.assertEqual(reporter.load_credential(self.credential), ('founder-1', bytes.fromhex(KEY_HEX), bytes.fromhex(SALT_HEX)))
        folder = self.root / 'creds'; folder.mkdir()
        (folder / 'geo-reporter').write_bytes(self.credential.read_bytes())
        with patch.dict(os.environ, {'CREDENTIALS_DIRECTORY': str(folder)}):
            self.assertEqual(reporter.load_credential()[0], 'founder-1')
        with patch.dict(os.environ, {}, clear=True), self.assertRaisesRegex(reporter.Failure, 'CREDENTIALS_DIRECTORY'):
            reporter.load_credential()
        bad = [{'reporter': 'founder-1', 'key': KEY_HEX}, {'reporter': 'Founder 1', 'key': KEY_HEX, 'salt': SALT_HEX},
               {'reporter': 'founder-1', 'key': KEY_HEX.upper(), 'salt': SALT_HEX},
               {'reporter': 'founder-1', 'key': 'ab' * 16, 'salt': SALT_HEX},
               {'reporter': 'founder-1', 'key': KEY_HEX, 'salt': KEY_HEX},
               {'reporter': 'founder-1', 'key': KEY_HEX, 'salt': SALT_HEX, 'extra': 1}]
        for value in bad + ['not json']:
            with self.subTest(value=value):
                self.credential.write_text(value if isinstance(value, str) else json.dumps(value))
                with self.assertRaises(reporter.Failure) as caught:
                    reporter.load_credential(self.credential)
                self.assertEqual(caught.exception.code, reporter.SETUP)
                for secret in (KEY_HEX, SALT_HEX, KEY_HEX.upper()):
                    self.assertNotIn(secret, str(caught.exception))


class RunTests(Files):
    def run_reporter(self, *extra):
        out, err = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = reporter.main(['--config', str(self.config), '--credential', str(self.credential), *extra])
        return code, out.getvalue(), err.getvalue()

    def test_dry_run_prints_the_report_with_keyed_ids_and_no_ip(self):
        node = self.node()
        code, out, err = self.run_reporter('--dry-run')
        self.assertEqual((code, err), (0, ''))
        body, summary = out.splitlines()
        report = json.loads(body)
        salt = bytes.fromhex(SALT_HEX)
        self.assertEqual(report, {
            'schema': 'pocol-geo-report-v1', 'reporter': 'founder-1', 'chainId': CHAIN, 'genesisHash': GENESIS,
            'at': report['at'], 'head': {'height': 1000, 'hash': HEAD['hash']}, 'miner': MINER.lower(),
            'peers': sorted([{'id': geo.keyed_id(salt, '5.0.0.10'), 'country': 'DE'},
                             {'id': geo.keyed_id(salt, '2a00:1450::1'), 'country': 'IE'},
                             {'id': geo.keyed_id(salt, '45.0.1.1'), 'country': None},
                             {'id': geo.keyed_id(salt, '9.9.9.9'), 'country': 'US'}], key=lambda p: p['id']),
            'unlocated': 2})
        self.assertEqual(summary, 'DRY RUN: not sent; reporter founder-1, head 1000, miner yes, 4 peers (3 located, 2 not locatable)')
        for ip in ('5.0.0.10', '2a00', '10.0.0.7', 'fe80', '45.0.1.1', '9.9.9.9', '30300'):
            self.assertNotIn(ip, out)
        self.assertNotIn(KEY_HEX, out); self.assertNotIn(SALT_HEX, out)
        # One read of the node per run: one batch of pocol_status and pocol_getParams.
        self.assertEqual([[c['method'] for c in r] for r in node.requests], [['pocol_status', 'pocol_getParams']])

    def test_the_signed_report_is_accepted_by_the_gateway_and_places_the_miner(self):
        self.node()
        config, credential = reporter.load_config(self.config), reporter.load_credential(self.credential)
        with geo.GeoDatabase(self.database) as db:
            status, params = reporter.read_node()
            body, signature, report = reporter.build_report(status, params, config, credential, db, NOW)
            credentials = geo.parse_credentials(json.dumps({'salt': SALT_HEX, 'reporters': {'founder-1': KEY_HEX}}).encode())
            holders = SimpleNamespace(
                registry=lambda: {'height': 1000, 'blockHash': HEAD['hash'], 'thetaInact': 360, 'records': [
                    {'id': MINER, 'status': 1, 'tomb': 0, 'bondState': 1, 'lastShare': 999, 'graceStart': 0}]},
                winners=lambda window: {'from': 1, 'to': 1000, 'blocks': 1000, 'miners': {MINER.lower(): 1000}})
            geography = geo.Geography(db, None, {'chainId': CHAIN, 'genesisHash': GENESIS}, holders, lambda: None,
                                      lambda: NOW + 3, credentials=credentials)
            headers = [('Content-Type', 'application/json'), ('Content-Length', str(len(body))),
                       ('X-PoCol-Reporter', 'founder-1'), ('X-PoCol-Signature', signature)]
            self.assertEqual(signature, hmac.new(bytes.fromhex(KEY_HEX), body, hashlib.sha256).hexdigest())
            self.assertEqual(geography.report(headers, body, '5.0.1.20'), 204)
            self.assertEqual(geography.report(headers, body, '5.0.1.20'), 409, 'a replayed report')
            answer = geography.answer()
        self.assertEqual(answer['miners']['located'], 1)
        self.assertEqual(answer['countries'][0]['code'], 'DE')
        self.assertEqual(answer['countries'][0]['miners'], {'located': 1, 'active': 1, 'inactive': 0, 'connected': 1})
        # 5.0.0.10, 2a00:1450::1, 45.0.1.1, 9.9.9.9 and the reporter's own 5.0.1.20; two peers not locatable.
        self.assertEqual(answer['nodes'], {'observed': 7, 'located': 4, 'unlocated': 3})
        self.assertNotIn(MINER.lower(), json.dumps(answer))

    def test_a_node_of_another_network_or_unreadable_fails_with_code_2(self):
        for kwargs in (dict(params={'chainId': 1, 'genesisHash': GENESIS}), dict(status=node_status(genesisHash='0x' + '00' * 32)),
                       dict(status=node_status(head={'height': 'x'})), dict(status=node_status(miner='nope')),
                       dict(status=node_status(peers='x')), dict(http_status=500)):
            with self.subTest(kwargs=list(kwargs)):
                self.node(**kwargs)
                code, out, err = self.run_reporter('--dry-run')
                self.assertEqual((code, out), (2, ''))
                self.assertTrue(err.startswith('FAILED: '))
        with patch.object(reporter, 'RPC_PORT', 1):
            code, _, err = self.run_reporter('--dry-run')
        self.assertEqual(code, 2); self.assertIn('cannot be read', err)

    def test_local_setup_failures_exit_1(self):
        self.node()
        self.write_config(database=str((self.root / 'absent.bin').resolve()))
        self.assertEqual(self.run_reporter('--dry-run')[0], 1)
        self.write_config()
        self.credential.write_text('{}')
        self.assertEqual(self.run_reporter('--dry-run')[0], 1)

    def test_a_node_with_no_miner_reports_none(self):
        self.node(status=node_status(miner=None, peers=[]))
        code, out, _ = self.run_reporter('--dry-run')
        report = json.loads(out.splitlines()[0])
        self.assertEqual((code, report['miner'], report['peers'], report['unlocated']), (0, None, [], 0))

    def test_a_peer_list_beyond_the_body_limit_is_trimmed_and_the_rest_counted_unlocated(self):
        peers = [f'11.0.{i // 256}.{i % 256}:30300' for i in range(600)]
        with geo.GeoDatabase(self.database) as db:
            body, _, report = reporter.build_report(node_status(peers=peers), {'chainId': CHAIN, 'genesisHash': GENESIS},
                                                    reporter.load_config(self.config), reporter.load_credential(self.credential), db, NOW)
        self.assertLessEqual(len(body), geo.MAX_REPORT_BYTES)
        self.assertGreater(len(report['peers']), 300)
        self.assertEqual(len(report['peers']) + report['unlocated'], 600)

    def test_post_requires_certificate_verification_and_maps_the_answer(self):
        self.node()
        seen = []

        class Connection:
            answer = 204
            def __init__(self, host, port, timeout=None, context=None):
                seen.append(dict(host=host, port=port, timeout=timeout, context=context))
            def request(self, method, path, body, headers):
                seen[-1].update(method=method, path=path, body=body, headers=headers)
                if isinstance(Connection.answer, Exception):
                    raise Connection.answer
            def getresponse(self):
                return SimpleNamespace(status=Connection.answer, read=lambda n: b'')
            def close(self):
                pass

        with patch.object(reporter.http.client, 'HTTPSConnection', Connection):
            code, out, err = self.run_reporter()
            self.assertEqual((code, err), (0, ''))
            self.assertTrue(out.startswith('SENT: reporter founder-1, head 1000'))
            call = seen[-1]
            self.assertEqual((call['host'], call['port'], call['method'], call['path']),
                             ('explorer.example', 443, 'POST', '/api/geo-report.php'))
            self.assertLessEqual(call['timeout'], 10)
            context = call['context']
            self.assertIsInstance(context, ssl.SSLContext)
            self.assertEqual((context.verify_mode, context.check_hostname), (ssl.CERT_REQUIRED, True))
            self.assertGreaterEqual(context.minimum_version, ssl.TLSVersion.TLSv1_2)
            headers = call['headers']
            self.assertEqual(headers['Content-Type'], 'application/json')
            self.assertEqual(headers['X-PoCol-Reporter'], 'founder-1')
            self.assertEqual(headers['X-PoCol-Signature'], hmac.new(bytes.fromhex(KEY_HEX), call['body'], hashlib.sha256).hexdigest())
            self.assertNotIn('Origin', headers)
            self.assertNotIn(b'5.0.0.10', call['body'])
            for answer, expected in ((400, 3), (401, 3), (409, 3), (413, 3), (415, 3), (429, 4), (500, 4), (503, 4),
                                     (ssl.SSLCertVerificationError('bad certificate'), 4), (OSError('unreachable'), 4)):
                with self.subTest(answer=answer):
                    Connection.answer = answer
                    code, out, err = self.run_reporter()
                    self.assertEqual((code, out), (expected, ''))
                    self.assertTrue(err.startswith('FAILED: '))
                    self.assertNotIn(KEY_HEX, err)


class InstallerTests(unittest.TestCase):
    def setUp(self):
        tmp = tempfile.TemporaryDirectory(); self.addCleanup(tmp.cleanup)
        self.root = Path(tmp.name)
        self.paths = dict(BASE=self.root / 'opt', CONFIG=self.root / 'etc', CONFIG_FILE=self.root / 'etc' / 'config.json',
                          DATABASE=self.root / 'etc' / 'geoip-country.bin', CREDENTIAL=self.root / 'etc' / 'credential.json',
                          UNITS=self.root / 'units', BACKUPS=self.root / 'backups', REPO_IDENTITY=self.root / 'repo' / 'site-identity.json')
        self.paths['CONFIG'].mkdir(mode=0o755); os.chmod(self.paths['CONFIG'], 0o755)
        self.paths['UNITS'].mkdir()
        (self.root / 'repo').mkdir()
        self.paths['REPO_IDENTITY'].write_text(json.dumps({'chainId': CHAIN, 'genesisHash': GENESIS}))
        self.paths['CREDENTIAL'].write_text(json.dumps({'reporter': 'founder-1', 'key': KEY_HEX, 'salt': SALT_HEX}))
        os.chmod(self.paths['CREDENTIAL'], 0o600)
        self.source = self.root / 'in' / 'geoip-country.bin'
        self.source.parent.mkdir()
        geo.build(SAMPLE, '2026-11', self.source)

    def run_install(self, *argv, identity=(CHAIN, GENESIS), owner=0, dry_run=0, existing=False, python_ok=True):
        commands = []

        def command(*args, check=True):
            commands.append(args)
            if args[0] == installer.PYTHON and args[1] == '-c':
                return SimpleNamespace(returncode=0 if python_ok else 1, stdout=b'', stderr=b'')
            if args[0] == installer.PYTHON:
                return SimpleNamespace(returncode=dry_run, stdout=b'' if dry_run else DRY_RUN_REPORT,
                                       stderr=b'FAILED: the node does not serve the configured network')
            absent = args[:2] in (('systemctl', 'is-active'), ('systemctl', 'is-enabled')) and not existing
            return SimpleNamespace(returncode=1 if absent else 0, stdout=b'', stderr=b'')

        def file_status(path):
            real = os.lstat(path)
            return SimpleNamespace(st_mode=real.st_mode, st_uid=owner)

        error = None
        with patch.multiple(installer, **self.paths), \
             patch.object(installer.os, 'geteuid', return_value=0, create=True), \
             patch.object(installer.shutil, 'which', return_value='/bin/systemctl'), \
             patch.object(installer, 'command', side_effect=command), \
             patch.object(installer, 'rpc_identity', return_value=identity), \
             patch.object(installer, 'file_status', side_effect=file_status), \
             patch('builtins.print') as output:
            try:
                installer.main(list(argv))
            except RuntimeError as exc:
                error = str(exc)
        return commands, ' '.join(str(c) for c in output.call_args_list), error

    def test_the_units(self):
        with patch.multiple(installer, BASE=Path('/opt/pocol-geo-reporter'), CONFIG_FILE=Path('/etc/pocol-geo-reporter/config.json'),
                            CREDENTIAL=Path('/etc/pocol-geo-reporter/credential.json')):
            service = installer.service_text()
        self.assertIn('ExecStart=/usr/bin/python3 -I -B /opt/pocol-geo-reporter/pocol_geo_reporter.py --config '
                      '/etc/pocol-geo-reporter/config.json\n', service)
        for setting in ('Type=oneshot', 'LoadCredential=geo-reporter:/etc/pocol-geo-reporter/credential.json', 'DynamicUser=yes',
                        'ProtectSystem=strict', 'ProtectHome=yes', 'NoNewPrivileges=yes', 'CapabilityBoundingSet=', 'Nice=19',
                        'IOSchedulingClass=idle', 'MemoryMax=64M', 'MemorySwapMax=0', 'CPUQuota=10%', 'TimeoutStartSec=30',
                        'InaccessiblePaths=-/var/lib/pocol-standalone', 'PrivateTmp=yes', 'UMask=0077'):
            self.assertIn(setting + '\n', service)
        self.assertNotRegex(service, r'(?m)^(IPAddressAllow|IPAddressDeny|ReadWritePaths|StateDirectory|User)=')
        timer = installer.timer_text()
        for setting in ('OnUnitActiveSec=60s', 'OnBootSec=2min', 'Unit=pocol-geo-reporter.service', 'WantedBy=timers.target'):
            self.assertIn(setting + '\n', timer)

    def test_check_changes_nothing(self):
        commands, printed, error = self.run_install('--database', str(self.source), '--check')
        self.assertIsNone(error)
        self.assertIn('CHECK PASSED: reporter founder-1, chain 777963', printed)
        self.assertFalse(self.paths['BASE'].exists()); self.assertFalse(self.paths['CONFIG_FILE'].exists())
        self.assertEqual(list(self.paths['UNITS'].iterdir()), [])
        self.assertTrue(all(c[0] == installer.PYTHON and c[1] == '-c' for c in commands), commands)

    def test_install_writes_the_files_and_units_then_enables_the_timer_after_a_dry_run(self):
        commands, printed, error = self.run_install('--database', str(self.source))
        self.assertIsNone(error)
        base = self.paths['BASE']
        self.assertEqual((base / 'pocol_geo_reporter.py').read_bytes(), (HERE / 'pocol_geo_reporter.py').read_bytes())
        self.assertEqual((base / 'pocol_geoip.py').read_bytes(), (HERE.parent / 'rewards' / 'pocol_geoip.py').read_bytes())
        self.assertEqual(self.paths['DATABASE'].read_bytes(), self.source.read_bytes())
        config = json.loads(self.paths['CONFIG_FILE'].read_text())
        self.assertEqual(config, {'chainId': CHAIN, 'genesisHash': GENESIS, 'explorerUrl': installer.DEFAULT_URL,
                                  'database': str(self.paths['DATABASE'])})
        self.assertEqual(stat.S_IMODE(os.stat(self.paths['CONFIG_FILE']).st_mode), 0o644)
        with patch.multiple(installer, **self.paths):
            self.assertEqual((self.paths['UNITS'] / installer.SERVICE).read_text(), installer.service_text())
            self.assertEqual((self.paths['UNITS'] / installer.TIMER).read_text(), installer.timer_text())
        self.assertIn(f'LoadCredential=geo-reporter:{self.paths["CREDENTIAL"]}', (self.paths['UNITS'] / installer.SERVICE).read_text())
        self.assertTrue((self.paths['UNITS'] / installer.TIMER).is_file())
        dry = next(i for i, c in enumerate(commands) if c[0] == installer.PYTHON and '--dry-run' in c)
        self.assertIn('--credential', commands[dry])
        self.assertLess(commands.index(('systemctl', 'daemon-reload')), dry)
        self.assertLess(dry, commands.index(('systemctl', 'enable', '--now', installer.TIMER)))
        self.assertIn('INSTALLED', printed)
        # The dry run's report is shown to the owner before the timer runs (DEPLOY-INSIGHTS.md §7 step 3).
        self.assertIn("call('Dry run report (not sent):') call(" + repr(DRY_RUN_REPORT.decode()) + ')', printed)
        self.assertLess(printed.index('Dry run report'), printed.index('INSTALLED'))
        # The credential is never copied, printed or backed up.
        backups = [p for p in self.paths['BACKUPS'].rglob('*') if p.is_file()]
        for path in backups + [base / 'pocol_geo_reporter.py', self.paths['CONFIG_FILE']]:
            self.assertNotIn(KEY_HEX.encode(), path.read_bytes())
        self.assertNotIn(KEY_HEX, printed)
        # A second install (an update) keeps the database when none is given.
        commands, printed, error = self.run_install(existing=True)
        self.assertIsNone(error)
        self.assertEqual(self.paths['DATABASE'].read_bytes(), self.source.read_bytes())

    def test_refusals_change_nothing(self):
        cases = {
            'missing credential': (lambda: self.paths['CREDENTIAL'].unlink(), {}, 'is missing'),
            'credential mode': (lambda: os.chmod(self.paths['CREDENTIAL'], 0o640), {}, 'mode 0600'),
            'credential owner': (None, dict(owner=1000), 'owned by root'),
            'credential link': (lambda: (self.paths['CREDENTIAL'].rename(self.root / 'real.json'),
                                         self.paths['CREDENTIAL'].symlink_to(self.root / 'real.json')), {}, 'regular file'),
            'credential content': (lambda: self.paths['CREDENTIAL'].write_text('{}'), {}, 'exactly reporter, key and salt'),
            'other network': (None, dict(identity=(1, GENESIS)), 'not the network this repository pins'),
            'no identity': (None, dict(identity=(None, '')), 'names no chain ID'),
            'old python': (None, dict(python_ok=False), 'Python 3.10'),
            'config folder': (lambda: os.chmod(self.paths['CONFIG'], 0o700), {}, 'searchable'),
            'unmarked base': (lambda: self.paths['BASE'].mkdir(), {}, 'Unrecognized'),
            'unmarked unit': (lambda: (self.paths['UNITS'] / installer.TIMER).write_text('[Timer]\n'), {}, 'marker'),
        }
        for name, (prepare, kwargs, message) in cases.items():
            with self.subTest(name):
                self.setUp()
                if prepare:
                    prepare()
                commands, printed, error = self.run_install('--database', str(self.source), **kwargs)
                self.assertIn(message, error or '')
                self.assertNotIn('INSTALLED', printed)
                self.assertFalse(self.paths['CONFIG_FILE'].exists())
                self.assertFalse(any(c[:2] == ('systemctl', 'enable') for c in commands))
        self.setUp()
        bad = self.root / 'bad.bin'; bad.write_bytes(b'POCOLGEO' + b'\0' * 100)
        _, _, error = self.run_install('--database', str(bad))
        self.assertIn('GeoIP database', error)
        _, _, error = self.run_install()
        self.assertIn('No GeoIP database', error)
        _, _, error = self.run_install('--database', str(self.source), '--other-network')
        self.assertIsNone(error, 'another network on purpose')

    def test_usage(self):
        for argv in (['--explorer-url', 'http://explorer.example/api/geo-report.php'], ['--explorer-url', 'https://x/other'],
                     ['--check', '--check'], ['--uninstall', '--check'], ['--database'], ['--what']):
            with self.subTest(argv=argv), self.assertRaises(RuntimeError):
                installer.usage(argv)
        self.assertEqual(installer.usage(['--explorer-url', 'https://explorer.example/api/geo-report.php'])['--explorer-url'],
                         'https://explorer.example/api/geo-report.php')

    def test_a_failed_dry_run_rolls_back_a_new_install(self):
        commands, printed, error = self.run_install('--database', str(self.source), dry_run=2)
        self.assertRegex(error, 'rolled back: the reporter dry run failed: FAILED: the node does not serve')
        self.assertFalse(self.paths['BASE'].exists())
        self.assertFalse(self.paths['CONFIG_FILE'].exists()); self.assertFalse(self.paths['DATABASE'].exists())
        self.assertEqual(list(self.paths['UNITS'].iterdir()), [])
        self.assertTrue(self.paths['CREDENTIAL'].is_file(), "the owner's credential is never touched")
        self.assertFalse(any(c[:3] == ('systemctl', 'enable', '--now') for c in commands))
        self.assertIn(('systemctl', 'disable', installer.TIMER), commands)

    def test_a_failed_update_restores_the_previous_install_and_restarts_its_timer(self):
        self.assertIsNone(self.run_install('--database', str(self.source))[2])
        before = {p: p.read_bytes() for p in self.paths['BASE'].iterdir()}
        old_service = (self.paths['UNITS'] / installer.SERVICE).read_bytes()
        other = self.root / 'other.bin'
        geo.build(SAMPLE, '2026-10', other)
        commands, _, error = self.run_install('--database', str(other), existing=True, dry_run=2)
        self.assertRegex(error, 'rolled back')
        self.assertEqual(self.paths['DATABASE'].read_bytes(), self.source.read_bytes())
        self.assertEqual({p: p.read_bytes() for p in self.paths['BASE'].iterdir()}, before)
        self.assertEqual((self.paths['UNITS'] / installer.SERVICE).read_bytes(), old_service)
        self.assertEqual(commands[-1], ('systemctl', 'restart', installer.TIMER))

    def test_uninstall_removes_everything_but_the_credential(self):
        self.assertIsNone(self.run_install('--database', str(self.source))[2])
        commands, printed, error = self.run_install('--uninstall', existing=True)
        self.assertIsNone(error)
        self.assertFalse(self.paths['BASE'].exists()); self.assertFalse(self.paths['CONFIG_FILE'].exists())
        self.assertFalse(self.paths['DATABASE'].exists()); self.assertEqual(list(self.paths['UNITS'].iterdir()), [])
        self.assertTrue(self.paths['CREDENTIAL'].is_file())
        self.assertIn(('systemctl', 'disable', '--now', installer.TIMER), commands)
        self.assertIn('REMOVED', printed)
        _, printed, error = self.run_install('--uninstall')
        self.assertIsNone(error); self.assertIn('Nothing to remove', printed)

    def test_the_installer_reads_the_pin_the_websites_use(self):
        repo = HERE.parent / 'websites' / 'site-identity.json'
        declared = json.loads(repo.read_text())
        self.assertEqual(installer.pinned_identity(repo), (declared['chainId'], declared['genesisHash'].lower()))
        self.assertEqual(installer.REPO_IDENTITY, repo)


if __name__ == '__main__':
    unittest.main()
