"""The explorer gateway host's Caddy site, static copy and installer (explorer_site.py, install_explorer_gateway.py):
the rendered block against the Studio installer in both orders, the update and refusal rules, the copy's checks on
the committed snapshot, and the install, check and rollback paths with every host path in a temporary directory and
every system command faked. Nothing here changes a host or contacts a network."""
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import MagicMock, patch

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE))
import explorer_site as site  # noqa: E402
import install_explorer_gateway as installer  # noqa: E402
import explorer_api  # noqa: E402  (deploy/rewards, on the path through explorer_site)
import gateway  # noqa: E402
import install_studio_gateway as studio  # noqa: E402  (deploy/studio, on the path through the installer)
sys.path.insert(0, str(ROOT / 'deploy' / 'websites' / 'tools'))
import site_snapshot  # noqa: E402

NET = {'chainId': 777963, 'genesisHash': '0x' + '5a' * 32, 'specVersion': 2, 'v3': True}
ROOT_A, ROOT_B = '/srv/pocol-explorer/site-' + 'a' * 12, '/srv/pocol-explorer/site-' + 'b' * 12


def snapshot_copy(directory):
    """The explorer copy as the operator builds it: site_snapshot.py from the committed blobs and the include list."""
    out = Path(directory) / 'explorer-site'
    site_snapshot.build(out, 'deploy/websites/explorer.pocol.net', ROOT / 'deploy/websites/tools/explorer-s1-include.txt')
    return out


class CaddyBlockTests(unittest.TestCase):
    def test_the_block_routes_and_headers(self):
        block = site.render_block(ROOT_A)
        self.assertTrue(block.startswith(site.BEGIN + '\nexplorer.pocol.net {\n') and block.endswith('}\n' + site.END + '\n'))
        for route in ('@rpc path /rpc\n', '@health path /health\n', '@explorer_api path /api/*\n'):
            self.assertEqual(block.count(route), 1)
        self.assertEqual(block.count('reverse_proxy 127.0.0.1:8088'), 3)
        self.assertEqual(block.count('respond @oversized "Request body exceeds 16 KiB" 413'), 1)
        self.assertEqual(block.count('respond @api_oversized "Request body exceeds 16 KiB" 413'), 1)
        self.assertEqual(block.count('int({http.request.header.Content-Length}) > 16384'), 2)
        self.assertEqual(block.count(f'root * {ROOT_A}\n        file_server\n'), 1)
        self.assertNotIn('browse', block)
        self.assertIn('    request_body {\n        max_size 16384\n    }\n', block)
        # Caddy 2.7+ refuses a token after '{' on the same line: every '{' ends its line, every '}' is alone on one.
        for line in block.splitlines():
            if '{' in line.replace('{http.request.header.Content-Length}', ''):
                self.assertTrue(line.endswith(' {'), line)
            if '}' in line.replace('{http.request.header.Content-Length}', ''):
                self.assertEqual(line.strip(), '}', line)
        # The explorer's .htaccess headers on its files and its /api reads, verbatim, and not site-wide.
        for name, value in explorer_api.SECURITY_HEADERS:
            if name == 'X-Content-Type-Options': continue
            self.assertEqual(block.count(f'{name} "{value}"'), 2, name)
        site_header = block.split('request_body', 1)[0]
        self.assertIn('Cache-Control "no-store"', site_header); self.assertIn('X-Content-Type-Options "nosniff"', site_header)
        self.assertNotIn('Content-Security-Policy', site_header)
        self.assertIn("connect-src 'self'", block)
        for path in ('/.htaccess', '/api/.htaccess', '/a/.git/HEAD', '/x.php', '/x.PHP', '/x.php5', '/SHA256SUMS.txt',
                     '/sha256sums', '/x.json', '/assets/index.js.map', '/README.md', '/a.zip', '/a.tar', '/a.gz', '/a.bak',
                     '/a.log', '/a.sh', '/x.phtml'):
            self.assertRegex(path, site.HIDDEN)
        for path in ('/', '/index.html', '/assets/index-038724d30ca3.js', '/site.webmanifest', '/pocol-logo.png', '/favicon.ico'):
            self.assertNotRegex(path, site.HIDDEN)
        for value in ('/srv/pocol-explorer/site-x', '/var/www', ROOT_A + '/'):
            with self.assertRaises(ValueError): site.render_block(value)
        with self.assertRaises(ValueError): site.render_block(ROOT_A, gateway='0.0.0.0:8088')

    def test_the_studio_installer_accepts_the_block_in_either_order(self):
        block = site.render_block(ROOT_A)
        patched = studio.patch_caddy(block)
        self.assertIn(studio.ROUTE, patched); self.assertIn('max_size 131072', patched)
        self.assertEqual(studio.patch_caddy(patched), patched)
        # An explorer update keeps the Studio route and its body size, and changes only the site root.
        updated, old = site.plan_caddyfile(patched, ROOT_B, studio)
        self.assertEqual(old, ROOT_A)
        self.assertEqual(updated, studio.patch_caddy(site.render_block(ROOT_B)))
        self.assertEqual(updated, patched.replace(ROOT_A, ROOT_B))
        # Explorer first, then Studio, equals Studio after the explorer update.
        self.assertEqual(studio.patch_caddy(site.plan_caddyfile(block, ROOT_B, studio)[0]), updated)
        self.assertEqual(site.plan_caddyfile(updated, ROOT_B, studio), (updated, ROOT_B))
        tampered = patched.replace('response_header_timeout 25s', 'response_header_timeout 99s')
        with self.assertRaisesRegex(RuntimeError, 'Studio route .* custom edits'): site.plan_caddyfile(tampered, ROOT_B, studio)

    def test_caddyfile_plans(self):
        block = site.render_block(ROOT_A)
        stock = (':80 {\n\t# Set this path to your site\'s directory.\n\troot * /usr/share/caddy\n\n\t# Enable the static file '
                 'server.\n\tfile_server\n\n\t# reverse_proxy localhost:8080\n}\n\n# Refer to the Caddy docs\n')
        self.assertEqual(site.plan_caddyfile('', ROOT_A, studio), (block, None))
        self.assertEqual(site.plan_caddyfile(stock, ROOT_A, studio), (block, None))
        other = 'example.org {\n    respond "hi"\n}\n'
        self.assertEqual(site.plan_caddyfile(other, ROOT_A, studio), (other.rstrip('\n') + '\n\n' + block, None))
        new, old = site.plan_caddyfile(other + '\n' + block + '\n# after\n', ROOT_B, studio)
        self.assertEqual((new, old), (other + '\n' + site.render_block(ROOT_B) + '\n# after\n', ROOT_A))
        refused = {
            'unmanaged': studio_fixture(),
            'gateway port elsewhere': other.replace('respond "hi"', 'reverse_proxy 127.0.0.1:8088'),
            'two blocks': block + block,
            'end before begin': site.END + '\n' + site.BEGIN + '\n',
            'only one marker': block.replace(site.END, ''),
            'edited block': block.replace('dial_timeout 3s', 'dial_timeout 30s'),
            'edited header': block.replace('X-Frame-Options "DENY"', 'X-Frame-Options "SAMEORIGIN"', 1),
            'browse added': block.replace('file_server\n', 'file_server browse\n'),
            'second host outside': block + 'explorer.pocol.net:8443 {\n}\n',
            'route to the gateway outside': block + 'other.example {\n    reverse_proxy 127.0.0.1:8088\n}\n',
            'two roots': block.replace('file_server\n', f'file_server\n        root * {ROOT_B}\n'),
        }
        for name, text in refused.items():
            with self.subTest(name), self.assertRaises(RuntimeError):
                site.plan_caddyfile(text, ROOT_B, studio)
        self.assertTrue(site.is_stock_caddyfile(stock))
        self.assertFalse(site.is_stock_caddyfile(stock.replace('/usr/share/caddy', '/srv')))

    def test_the_loopback_test_config(self):
        with tempfile.TemporaryDirectory() as tmp:
            text = site.loopback_config(tmp, '127.0.0.1:8661', '127.0.0.1:8662')
            self.assertTrue(text.startswith('{\n    admin off\n    persist_config off\n}\n'))
            self.assertIn('\nhttp://127.0.0.1:8661 {\n', text)
            self.assertNotIn('explorer.pocol.net', text); self.assertNotIn('8088', text)
            self.assertEqual(text.count('reverse_proxy 127.0.0.1:8662'), 3)
            self.assertIn('root * ' + str(Path(tmp).resolve()).replace('\\', '/') + '\n', text)
            # Everything but the address, the ports and the root is the production block.
            production = site.render_block(ROOT_A)
            self.assertEqual(text.split('}\n\n', 1)[1].replace('http://127.0.0.1:8661', 'explorer.pocol.net')
                             .replace('127.0.0.1:8662', '127.0.0.1:8088').replace(str(Path(tmp).resolve()).replace('\\', '/'), ROOT_A), production)
            with self.assertRaises(ValueError): site.loopback_config(tmp, '0.0.0.0:8661', '127.0.0.1:8662')


def studio_fixture():
    return CaddyFixture.BASE


class CaddyFixture:
    # The Studio installer tests' fixture of the lost host block (deploy/studio/test_studio_gateway.py).
    BASE = '''explorer.pocol.net {
        header {
            Cache-Control "no-store"
            X-Content-Type-Options "nosniff"
            -Server
        }
        request_body { max_size 16384 }
        @rpc { path /rpc }
        handle @rpc {
            @oversized expression `{http.request.header.Content-Length} != "" && int({http.request.header.Content-Length}) > 16384`
            respond @oversized "Request body exceeds 16 KiB" 413
            reverse_proxy 127.0.0.1:8088
        }
        handle { respond "Not found" 404 }
    }'''


class SiteCopyTests(unittest.TestCase):
    def test_the_committed_snapshot_is_a_valid_copy(self):
        with tempfile.TemporaryDirectory() as tmp:
            files = site.site_files(snapshot_copy(tmp))
            self.assertEqual(len(files), 12)
            self.assertIn('index.html', files)
            self.assertFalse(any(name.startswith('api/') or name.endswith('.php') or name.startswith('.') for name in files))
            text, ident = site.manifest(files)
            self.assertRegex(ident, r'^[0-9a-f]{12}$')
            self.assertEqual(ident, hashlib.sha256(text.encode()).hexdigest()[:12])
            self.assertEqual(len(text.splitlines()), 12)
            with patch('builtins.print') as output:
                self.assertEqual(site.main(['manifest', '--site', str(Path(tmp) / 'explorer-site')]), 0)
            self.assertEqual(output.call_args.args[0], text + f'site-{ident}')

    def test_the_copy_refuses_hidden_server_and_oversized_files(self):
        with tempfile.TemporaryDirectory() as tmp:
            base = snapshot_copy(tmp)
            def broken(change):
                target = Path(tmp) / 'case'
                shutil.rmtree(target, ignore_errors=True); shutil.copytree(base, target)
                change(target)
                return target
            cases = {
                'dotfile': lambda t: (t / '.htaccess').write_text('deny'),
                'api': lambda t: ((t / 'api').mkdir(), (t / 'api' / 'rpc.php').write_text('<?php')),
                'php': lambda t: (t / 'index.php').write_text('<?php'),
                'config': lambda t: (t / 'config.js').write_text('x'),
                'checksums': lambda t: (t / 'SHA256SUMS.txt').write_text('x'),
                'map': lambda t: (t / 'assets' / 'a.js.map').write_text('{}'),
                'json': lambda t: (t / 'site-identity.json').write_text('{}'),
                'hidden folder': lambda t: ((t / '.git').mkdir(), (t / '.git' / 'HEAD').write_text('x')),
                'deep': lambda t: ((t / 'a' / 'b' / 'c').mkdir(parents=True), (t / 'a' / 'b' / 'c' / 'x.js').write_text('x')),
                'many': lambda t: [(t / f'f{i}.js').write_text('x') for i in range(60)],
                'large': lambda t: (t / 'big.js').write_bytes(b'x' * (site.MAX_FILE + 1)),
                'no index': lambda t: (t / 'index.html').unlink(),
                'missing script': lambda t: next(t.glob('assets/*.js')).unlink(),
                'space in name': lambda t: (t / 'a b.js').write_text('x'),
            }
            if os.name != 'nt':
                cases['symlink'] = lambda t: (t / 'link.js').symlink_to(t / 'index.html')
                cases['fifo'] = lambda t: os.mkfifo(t / 'pipe.js')
            for name, change in cases.items():
                with self.subTest(name), self.assertRaises(ValueError):
                    site.site_files(broken(change))
            with self.assertRaises(ValueError): site.site_files(Path(tmp) / 'missing')


class InstallerTests(unittest.TestCase):
    def test_the_unit_binds_loopback_and_its_arguments_parse(self):
        installer.check_forced_loopback()
        self.assertIn('ExecStart=/usr/bin/python3 /opt/pocol-explorer-gateway/gateway.py --bind 127.0.0.1 --port 8088 --rpc-port 8545 '
                      '--v3-network /etc/pocol-explorer-gateway/network.json --site-origin https://explorer.pocol.net\n', installer.UNIT_TEXT)
        self.assertIn('--v3-network /etc/pocol-explorer-gateway/network.json --fork-schedule /etc/pocol-explorer-gateway/forks.json --site-origin',
                      installer.unit_text(True))
        with patch.object(installer, 'UNIT_TEXT', installer.UNIT_TEXT.replace('--bind 127.0.0.1 ', '--bind 0.0.0.0 ')), \
                self.assertRaisesRegex(RuntimeError, 'bind 127.0.0.1'):
            installer.check_forced_loopback()
        # The unit's arguments are ones the gateway accepts: with a test network and a closed RPC port it passes its
        # argument and network checks and stops at the node connection.
        with tempfile.TemporaryDirectory() as tmp:
            for forks in (False, True):
                args = installer.unit_text(forks).split('gateway.py ', 1)[1].split('\n', 1)[0].split()
                Path(tmp, 'network.json').write_text(json.dumps(NET)); Path(tmp, 'forks.json').write_text('[[1, 0]]')
                args = [a.replace(str(installer.NETWORK), str(Path(tmp, 'network.json'))).replace(str(installer.FORKS), str(Path(tmp, 'forks.json')))
                        for a in args]
                args[args.index('--rpc-port') + 1] = '9'
                result = subprocess.run([sys.executable, '-B', str(ROOT / 'deploy/rewards/gateway.py'), *args],
                                        capture_output=True, text=True, timeout=60)
                self.assertEqual(result.returncode, 1, result.stderr)
                self.assertIn('ConnectionRefusedError', result.stderr)

    def test_identity_compare_is_canonical(self):
        network = gateway.validated_network(NET)
        self.assertTrue(installer.same_identity({'chainId': 777963, 'genesisHash': '0x' + '5A' * 32}, network))
        for value in ({'chainId': '777963', 'genesisHash': NET['genesisHash']}, {'chainId': True, 'genesisHash': NET['genesisHash']},
                      {'chainId': 777963.0, 'genesisHash': NET['genesisHash']}, {'chainId': 777963, 'genesisHash': NET['genesisHash'][2:]},
                      {'chainId': 777963}, None, []):
            self.assertFalse(installer.same_identity(value, network))

    def test_tls_uses_the_public_hostname_trust_and_loopback_transport(self):
        context, connection = MagicMock(), MagicMock()
        connection.getresponse.return_value.status = 200
        connection.getresponse.return_value.read.return_value = b'{}'
        with patch.object(installer.ssl, 'create_default_context', return_value=context) as trust, \
             patch.object(installer.socket, 'create_connection', return_value='local-socket') as sock, \
             patch.object(installer.http.client, 'HTTPSConnection', return_value=connection) as https:
            self.assertEqual(installer.tls('GET', '/')[0], 200)
            trust.assert_called_once_with()
            sock.assert_called_once_with(('127.0.0.1', 443), timeout=12)
            context.wrap_socket.assert_called_once_with('local-socket', server_hostname='explorer.pocol.net')
            https.assert_called_once_with('explorer.pocol.net', 443, timeout=12, context=context)
            connection.close.assert_called_once()
        self.assertNotIn('CERT_NONE', Path(installer.__file__).read_text())
        self.assertNotIn('check_hostname', Path(installer.__file__).read_text())

    def test_https_checks(self):
        network = gateway.validated_network(NET)
        files = {'index.html': b'<html>', 'assets/a.js': b'1'}
        params = {'chainId': NET['chainId'], 'genesisHash': NET['genesisHash']}
        good_headers = dict(explorer_api.SECURITY_HEADERS, **{'Cache-Control': 'no-store'})
        def answers(changes={}):
            def tls(method, path, payload=None, headers=None):
                key = (method, path)
                if key in changes: return changes[key]
                response = SimpleNamespace(getheader=good_headers.get)
                if path == '/': return 200, response, files['index.html']
                if path.lstrip('/') in files: return 200, response, files[path.lstrip('/')]
                if path == '/api/health.php': return 200, response, b'{"result":"PASS"}'
                if path == '/api/rpc.php' and headers == {'Origin': 'https://explorer.pocol.net'}:
                    return 200, response, json.dumps([{'jsonrpc': '2.0', 'id': 1, 'result': params}]).encode()
                if path == '/rpc': return 200, response, json.dumps({'jsonrpc': '2.0', 'id': 1, 'result': params}).encode()
                if path == '/health': return 200, response, b'{"service":"pocol-explorer-gateway"}'
                if path == '/studio/health': return 200, response, b'{"service":"pocol-studio-gateway"}'
                return 404, response, b'Not found'
            return tls
        with patch.object(installer, 'tls', side_effect=answers()) as tls:
            installer.verify_https(network, files, True)
            asked = [c.args[:2] for c in tls.call_args_list]
            for path in ('/SHA256SUMS.txt', '/.htaccess', '/api/config.php', '/studio/health', '/assets/a.js'):
                self.assertTrue(any(p == path for _, p in asked), path)
        bare = SimpleNamespace(getheader={'Cache-Control': 'no-store'}.get)
        for changes, message in (({('GET', '/assets/a.js'): (200, bare, b'2')}, 'installed file'),
                                 ({('GET', '/'): (200, bare, b'<html>')}, 'does not send'),
                                 ({('GET', '/api/health.php'): (503, bare, b'{"result":"NOT_CONNECTED"}')}, 'did not PASS'),
                                 ({('GET', '/.htaccess'): (200, bare, b'deny')}, 'not 404'),
                                 ({('GET', '/studio/health'): (502, bare, b'')}, 'Studio')):
            with self.subTest(message), patch.object(installer, 'tls', side_effect=answers(changes)), \
                    self.assertRaisesRegex(RuntimeError, message):
                installer.verify_https(network, files, True)
        with patch.object(installer, 'tls', side_effect=answers({('GET', '/studio/health'): (502, bare, b'')})):
            installer.verify_https(network, files, False)

    def test_rollback_continues_after_a_caddy_command_timeout(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); base = root / 'opt'; unit = root / 'gateway.service'; caddy = root / 'Caddyfile'
            base.mkdir(); new_site = root / 'srv' / 'site-cccccccccccc'; new_site.mkdir(parents=True)
            paths = [base / 'gateway.py', base / installer.MARKER, unit]
            for path in paths: path.write_bytes(b'new')
            caddy.write_bytes(b'new caddy')
            commands = []
            def command(*args, **kwargs):
                commands.append(args)
                if args == ('systemctl', 'reload', 'caddy'): raise subprocess.TimeoutExpired(args, 45)
                return SimpleNamespace(returncode=0)
            with patch.multiple(installer, BASE=base, UNIT=unit, CADDYFILE=caddy), patch.object(installer, 'command', side_effect=command):
                failures = installer.rollback_install(b'old caddy', {p: None for p in paths}, False, new_site, False, False)
            self.assertEqual(caddy.read_bytes(), b'old caddy')
            self.assertFalse(base.exists()); self.assertFalse(unit.exists()); self.assertFalse(new_site.exists())
            self.assertEqual(len(failures), 1); self.assertIn('reload previous Caddy configuration', failures[0])
            self.assertIn(('systemctl', 'stop', installer.SERVICE), commands)
            self.assertIn(('systemctl', 'disable', installer.SERVICE), commands)
            self.assertEqual(commands[-1], ('systemctl', 'daemon-reload'))

    def run_install(self, fail_https=False, existing=False, unit_text=None, caddy_text=None, check=False, studio_route=False, forks=False):
        """installer.main() on temporary host paths: (Caddyfile after, commands, printed text, root)."""
        temporary = tempfile.TemporaryDirectory(); self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        base, unit, caddy, srv, backups, config = (root / 'opt', root / 'gateway.service', root / 'Caddyfile', root / 'srv',
                                                   root / 'backups', root / 'etc')
        config.mkdir(); (config / 'network.json').write_text(json.dumps(NET))
        if forks: (config / 'forks.json').write_text('[[1, 0]]')
        copy = snapshot_copy(root)
        files = site.site_files(copy); ident = site.manifest(files)[1]
        prior_root = srv / ('site-' + 'a' * 12)
        if caddy_text is None:
            caddy_text = site.render_block(ROOT_A) if existing else ':80 {\n\troot * /usr/share/caddy\n\tfile_server\n}\n'
            if studio_route: caddy_text = studio.patch_caddy(caddy_text)
        caddy.write_text(caddy_text)
        prior_source = b'# existing gateway preserved\n'
        if existing:
            base.mkdir(); (base / installer.MARKER).write_bytes(b'PoCol explorer gateway v1\n'); (base / 'gateway.py').write_bytes(prior_source)
            unit.write_text(unit_text or installer.UNIT_TEXT)
            prior_root.mkdir(parents=True); (prior_root / 'index.html').write_text('old')
        commands = []
        def command(*args, **kwargs):
            commands.append(args)
            absent = args[:3] in [('systemctl', 'is-active', '--quiet'), ('systemctl', 'is-enabled', '--quiet')] and args[-1] == installer.SERVICE and not existing
            return SimpleNamespace(returncode=1 if absent else 0, stdout=b'', stderr=b'')
        gateway_instance = MagicMock()
        # The copy is written under `srv`; the Caddyfile names its production root (/srv/pocol-explorer/site-<id>).
        real_plan = site.plan_caddyfile
        def plan(text, site_root, studio_module):
            return real_plan(text, '/srv/pocol-explorer/' + Path(site_root).name, studio_module)
        argv = ['--site', str(copy)] + (['--check'] if check else [])
        with patch.multiple(installer, BASE=base, UNIT=unit, CADDYFILE=caddy, SITE_PARENT=srv, BACKUPS=backups, CONFIG=config,
                            NETWORK=config / 'network.json', FORKS=config / 'forks.json'), \
             patch.object(installer.os, 'geteuid', return_value=0, create=True), \
             patch.object(installer.shutil, 'which', return_value='/test/tool'), \
             patch.object(installer.socket, 'socket', return_value=MagicMock()), \
             patch.object(installer, 'command', side_effect=command), \
             patch.object(installer.explorer_site, 'plan_caddyfile', side_effect=plan), \
             patch.object(gateway, 'Gateway', return_value=gateway_instance), \
             patch.object(installer, 'verify_local'), \
             patch.object(installer, 'verify_https_with_retry', side_effect=RuntimeError('TLS route failed') if fail_https else None) as verify, \
             patch('builtins.print') as output:
            printed = lambda: ' '.join(str(c) for c in output.call_args_list)
            if fail_https:
                with self.assertRaisesRegex(RuntimeError, 'TLS route failed'): installer.main(argv)
                self.assertEqual(caddy.read_text(), caddy_text)
                self.assertFalse((srv / f'site-{ident}').exists())
                if existing:
                    self.assertEqual((base / 'gateway.py').read_bytes(), prior_source)
                    self.assertTrue(unit.exists()); self.assertTrue(prior_root.exists())
                    self.assertEqual(commands[-1], ('systemctl', 'restart', installer.SERVICE))
                else:
                    self.assertFalse(base.exists()); self.assertFalse(unit.exists())
                self.assertNotIn('INSTALLED:', printed())
            else:
                installer.main(argv)
                if not check:
                    verify.assert_called_once()
                    self.assertEqual(verify.call_args.args[1], files)
                    self.assertEqual(verify.call_args.args[2], studio_route)
            gateway_instance.verify_network.assert_called_once_with()
            return caddy.read_text(), commands, printed(), root, ident, files

    def test_first_install_replaces_the_stock_caddyfile_and_reports_success_after_https(self):
        text, commands, printed, root, ident, files = self.run_install()
        self.assertEqual(text, site.render_block(f'/srv/pocol-explorer/site-{ident}'))
        self.assertEqual((root / 'gateway.service').read_text(), installer.UNIT_TEXT)
        for name in installer.SOURCES:
            self.assertEqual((root / 'opt' / name).read_bytes(), (ROOT / 'deploy/rewards' / name).read_bytes())
        self.assertEqual(site.site_files(root / 'srv' / f'site-{ident}'), files)
        if os.name != 'nt':
            self.assertEqual(oct((root / 'srv' / f'site-{ident}' / 'index.html').stat().st_mode & 0o777), '0o644')
            self.assertEqual(oct((root / 'srv' / f'site-{ident}' / 'assets').stat().st_mode & 0o777), '0o755')
        self.assertIn(('systemctl', 'enable', installer.SERVICE), commands)
        self.assertLess(commands.index(('systemctl', 'restart', installer.SERVICE)), commands.index(('systemctl', 'reload', 'caddy')))
        self.assertIn('INSTALLED:', printed)
        self.assertEqual(len(list((root / 'backups').glob('*/Caddyfile.before'))), 1)

    def test_an_update_keeps_the_studio_route_and_prunes_older_copies(self):
        text, commands, printed, root, ident, _ = self.run_install(existing=True, studio_route=True)
        self.assertEqual(text, studio.patch_caddy(site.render_block(f'/srv/pocol-explorer/site-{ident}')))
        self.assertTrue((root / 'srv' / ('site-' + 'a' * 12)).exists(), 'the previous copy stays for a rollback')

    def test_forks_json_adds_the_fork_schedule(self):
        root = self.run_install(forks=True)[3]
        self.assertEqual((root / 'gateway.service').read_text(), installer.unit_text(True))

    def test_check_changes_nothing(self):
        text, commands, printed, root, _, _ = self.run_install(check=True)
        self.assertIn('CHECK PASSED', printed)
        self.assertEqual(text, ':80 {\n\troot * /usr/share/caddy\n\tfile_server\n}\n')
        self.assertFalse((root / 'opt').exists()); self.assertFalse((root / 'gateway.service').exists())
        self.assertFalse((root / 'backups').exists()); self.assertFalse((root / 'srv').exists())
        self.assertIn(('caddy', 'validate', '--config', commands[-1][3], '--adapter', 'caddyfile'), commands)

    def test_first_install_restores_everything_on_tls_failure(self): self.run_install(fail_https=True)
    def test_an_update_restores_the_prior_service_and_copy_on_tls_failure(self): self.run_install(fail_https=True, existing=True)

    def test_refusals(self):
        with self.assertRaisesRegex(RuntimeError, 'custom edits'):
            self.run_install(existing=True, unit_text=installer.UNIT_TEXT.replace('MemoryMax=256M', 'MemoryMax=1G'))
        with self.assertRaisesRegex(RuntimeError, 'unmanaged explorer.pocol.net'):
            self.run_install(caddy_text=CaddyFixture.BASE)
        for argv in ([], ['--check'], ['--site'], ['--site', 'x', '--force'], ['--site', 'x', '--check', '--check']):
            with self.assertRaisesRegex(RuntimeError, 'Usage'): installer.main(argv)

    def test_write_site_reuses_an_identical_copy_and_refuses_another(self):
        with tempfile.TemporaryDirectory() as tmp:
            files = {'index.html': b'<p>'}
            ident = site.manifest(files)[1]
            with patch.object(installer, 'SITE_PARENT', Path(tmp) / 'srv'):
                path, created = installer.write_site(files, ident)
                self.assertTrue(created)
                self.assertEqual(installer.write_site(files, ident), (path, False))
                (path / 'index.html').write_bytes(b'<p>changed')
                with self.assertRaisesRegex(RuntimeError, 'other files'): installer.write_site(files, ident)
                (Path(tmp) / 'srv' / ('site-' + 'd' * 12)).mkdir(); (Path(tmp) / 'srv' / 'keep-me').mkdir()
                self.assertEqual(installer.prune_sites({path.name}), ['site-' + 'd' * 12])
                self.assertTrue((Path(tmp) / 'srv' / 'keep-me').exists())
                self.assertEqual(sorted(p.name for p in (Path(tmp) / 'srv').iterdir()), sorted([path.name, 'keep-me']))


if __name__ == '__main__':
    unittest.main()
