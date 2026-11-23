"""The explorer gateway host's Caddy site, static copy and installer (explorer_site.py, install_explorer_gateway.py):
the rendered block against the Studio installer and the faucet route in every order, the update and refusal rules, the copy's checks on
the committed snapshot, and the install, check and rollback paths with every host path in a temporary directory and
every system command faked. Nothing here changes a host or contacts a network."""
import hashlib
import json
import os
from pathlib import Path
import re
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
import faucet_route as faucet  # noqa: E402  (deploy/faucet, on the path through the installer)
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
        # The /api guard answers as rpc.php did; the /rpc guard keeps the text the Studio installer looks for.
        self.assertEqual(block.count('respond @api_oversized `{"error":"Request size limit exceeded."}` 413'), 1)
        self.assertEqual(block.count('header @api_oversized Content-Type "application/json; charset=utf-8"'), 1)
        self.assertEqual(json.loads(site.API_TOO_LARGE), {'error': 'Request size limit exceeded.'})
        self.assertEqual(block.count('Request body exceeds 16 KiB'), 1)
        self.assertEqual(block.count('int({http.request.header.Content-Length}) > 16384'), 2)
        self.assertEqual(block.count(f'root * {ROOT_A}\n        file_server\n'), 1)
        self.assertNotIn('browse', block)
        self.assertIn('    request_body {\n        max_size 16384\n    }\n', block)
        # Caddy 2.7+ refuses a token after '{' on the same line: every '{' ends its line, every '}' is alone on one.
        # (Placeholders and backtick-quoted tokens are single tokens, not blocks.)
        for line in block.splitlines():
            bare = re.sub(r'`[^`]*`|\{[A-Za-z][A-Za-z0-9_.-]*\}', '', line)
            if '{' in bare:
                self.assertTrue(line.endswith(' {'), line)
            if '}' in bare:
                self.assertEqual(line.strip(), '}', line)
        # The explorer's .htaccess headers on its files, its /api reads and the error answers Caddy writes itself,
        # verbatim, and not site-wide.
        for name, value in explorer_api.SECURITY_HEADERS:
            if name == 'X-Content-Type-Options': continue
            self.assertEqual(block.count(f'{name} "{value}"'), 3, name)
        # Caddy writes a missing file's 404 and an unreachable gateway's 502 outside the routes, where their deferred
        # header blocks never apply (.htaccess: `Header always set`): handle_errors sets the site's headers, and the
        # explorer's on every path but /rpc, /health and the Studio relay, before it answers the status.
        errors = block.split('    handle_errors {\n', 1)[1].split('\n    }\n', 1)[0]
        self.assertEqual(block.count('handle_errors'), 1)
        self.assertIn('        header {\n            Cache-Control "no-store"\n            X-Content-Type-Options "nosniff"\n'
                      '            -Server\n        }\n', errors)
        self.assertIn('        @explorer_error not path /rpc /health /studio/*\n        header @explorer_error {\n', errors)
        for name, value in explorer_api.SECURITY_HEADERS:
            if name == 'X-Content-Type-Options': continue
            self.assertIn(f'            {name} "{value}"\n', errors)
        self.assertIn('            defer\n', errors)
        self.assertTrue(errors.endswith('        respond "{err.status_code} {err.status_text}"'), errors[-80:])
        self.assertLess(block.index('handle_errors'), block.index('@rpc path /rpc'))
        site_header = block.split('request_body', 1)[0]
        self.assertIn('Cache-Control "no-store"', site_header); self.assertIn('X-Content-Type-Options "nosniff"', site_header)
        self.assertNotIn('Content-Security-Policy', site_header)
        self.assertIn("connect-src 'self'", block)
        for path in ('/.htaccess', '/api/.htaccess', '/a/.git/HEAD', '/x.php', '/x.PHP', '/x.php5', '/SHA256SUMS.txt',
                     '/sha256sums', '/x.json', '/assets/index.js.map', '/README.md', '/a.zip', '/a.tar', '/a.gz', '/a.bak',
                     '/a.log', '/a.sh', '/x.phtml'):
            self.assertRegex(path, site.HIDDEN)
        # The bundle index.html loads, read from the page (a rename by the W9 tool needs no change here).
        bundle = re.findall(r'<script\b[^>]*\bsrc="(/assets/index-[A-Za-z0-9_-]+\.js)"',
                            (ROOT / 'deploy/websites/explorer.pocol.net/index.html').read_text(encoding='utf-8'))
        self.assertEqual(len(bundle), 1, bundle)
        for path in ('/', '/index.html', bundle[0], '/site.webmanifest', '/pocol-logo.png', '/favicon.ico'):
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

    def test_the_faucet_route_and_the_studio_route_in_any_order(self):
        block = site.render_block(ROOT_A)
        both = faucet.patch_caddy(studio.patch_caddy(block))
        # Either install order gives the same file, and each patch is idempotent.
        self.assertEqual(studio.patch_caddy(faucet.patch_caddy(block)), both)
        self.assertEqual(faucet.patch_caddy(both), both); self.assertEqual(studio.patch_caddy(both), both)
        self.assertEqual(both.count(faucet.ROUTE), 1); self.assertEqual(both.count(studio.ROUTE), 1)
        self.assertIn(faucet.ROUTE + '    @rpc path /rpc\n', both)
        # The faucet route leaves the body size to Studio: alone it keeps 16 KiB.
        self.assertIn('max_size 16384', faucet.patch_caddy(block)); self.assertIn('max_size 131072', both)
        # An explorer update keeps whichever routes are installed and changes only the site root.
        for text in (faucet.patch_caddy(block), both):
            with self.subTest(studio=studio.MARKER in text):
                updated, old = site.plan_caddyfile(text, ROOT_B, studio, faucet)
                self.assertEqual((updated, old), (text.replace(ROOT_A, ROOT_B), ROOT_A))
                self.assertEqual(site.plan_caddyfile(updated, ROOT_B, studio, faucet), (updated, ROOT_B))
        # Explorer first, then the routes, equals the routes after an explorer update.
        fresh = site.plan_caddyfile(block, ROOT_B, studio, faucet)[0]
        self.assertEqual(faucet.patch_caddy(studio.patch_caddy(fresh)), both.replace(ROOT_A, ROOT_B))
        self.assertEqual(site.plan_caddyfile(block, ROOT_B, studio, faucet)[0], site.render_block(ROOT_B))
        # The route stripped, the block is the rendered one again.
        self.assertEqual(faucet.patch_caddy(block).replace(faucet.ROUTE, ''), block)
        # A caller that does not know the faucet route, an edited, damaged or duplicated one: refused.
        refused = {
            'faucet unknown to the caller': (both, None),
            'edited timeout': (both.replace('response_header_timeout 45s', 'response_header_timeout 99s'), faucet),
            'header removed': (both.replace('            ' + faucet.CLIENT_IP_HEADER + '\n', ''), faucet),
            'end marker removed': (both.replace('    ' + faucet.END + '\n', ''), faucet),
            'begin marker removed': (both.replace('    ' + faucet.MARKER + '\n', ''), faucet),
            'duplicated': (both.replace(faucet.ROUTE, faucet.ROUTE * 2), faucet),
        }
        for name, (text, module) in refused.items():
            with self.subTest(name), self.assertRaisesRegex(RuntimeError, 'custom edits'):
                site.plan_caddyfile(text, ROOT_B, studio, module)
        with self.assertRaisesRegex(RuntimeError, 'faucet port'):
            site.plan_caddyfile(both + 'other.example {\n    reverse_proxy 127.0.0.1:8547\n}\n', ROOT_B, studio, faucet)

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
            self.assertTrue(text.startswith('{\n    admin off\n}\n'))
            self.assertIn('\nhttp://127.0.0.1:8661 {\n', text)
            self.assertNotIn('explorer.pocol.net', text); self.assertNotIn('8088', text)
            self.assertEqual(text.count('reverse_proxy 127.0.0.1:8662'), 3)
            self.assertIn('root * ' + str(Path(tmp).resolve()).replace('\\', '/') + '\n', text)
            # Everything but the address, the ports and the root is the production block.
            production = site.render_block(ROOT_A)
            self.assertEqual(text.split('}\n\n', 1)[1].replace('http://127.0.0.1:8661', 'explorer.pocol.net')
                             .replace('127.0.0.1:8662', '127.0.0.1:8088').replace(str(Path(tmp).resolve()).replace('\\', '/'), ROOT_A), production)
            with self.assertRaises(ValueError): site.loopback_config(tmp, '0.0.0.0:8661', '127.0.0.1:8662')


class umask_during:
    """The process umask set to `mask` inside the block (no change for None)."""
    def __init__(self, mask): self.mask = mask
    def __enter__(self): self.old = None if self.mask is None else os.umask(self.mask)
    def __exit__(self, *_):
        if self.old is not None: os.umask(self.old)


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
                if path == '/faucet/status': return 200, response, json.dumps({'type': 'status', 'chainId': NET['chainId']}).encode()
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
        # The faucet route: /faucet/status names the network's chain ID; nothing is ever posted to /faucet/claim.
        with patch.object(installer, 'tls', side_effect=answers()) as tls:
            installer.verify_https(network, files, False, True)
            asked = [c.args[:2] for c in tls.call_args_list]
            self.assertIn(('GET', '/faucet/status'), asked)
            self.assertFalse(any(p.startswith('/faucet/claim') for _, p in asked))
        for answer in ((502, bare, b''), (200, bare, b'{"type":"status","chainId":"777963"}'),
                       (200, bare, json.dumps({'type': 'error', 'chainId': NET['chainId']}).encode()),
                       (200, bare, json.dumps({'type': 'status', 'chainId': NET['chainId'] + 1}).encode())):
            with self.subTest(answer=answer[2]), patch.object(installer, 'tls', side_effect=answers({('GET', '/faucet/status'): answer})), \
                    self.assertRaisesRegex(RuntimeError, 'faucet'):
                installer.verify_https(network, files, False, True)
        with patch.object(installer, 'tls', side_effect=answers({('GET', '/faucet/status'): (502, bare, b'')})):
            installer.verify_https(network, files, False, False)

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

    def run_install(self, fail_https=False, existing=False, unit_text=None, caddy_text=None, check=False, studio_route=False, forks=False,
                    change_copy=None, accept=None, prepare=None, umask=None, root_uid=None, https_wait=None, faucet_route=False):
        """installer.main() on temporary host paths: (Caddyfile after, commands, printed text, root). `change_copy(dir)`
        edits the copy after it is built; `accept` is the --accept-site value (the copy's id when True); `prepare(root)`
        runs after the host paths are set up; `umask` is root's umask during main(); `root_uid` the owner the installer
        takes for root (default: the test's own user, which owns the temporary paths)."""
        temporary = tempfile.TemporaryDirectory(); self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        base, unit, caddy, srv, backups, config = (root / 'opt', root / 'gateway.service', root / 'Caddyfile', root / 'srv',
                                                   root / 'backups', root / 'etc')
        config.mkdir(); os.chmod(config, 0o755); (config / 'network.json').write_text(json.dumps(NET)); os.chmod(config / 'network.json', 0o644)
        if forks: (config / 'forks.json').write_text('[[1, 0]]'); os.chmod(config / 'forks.json', 0o644)
        if prepare: prepare(root)
        copy = snapshot_copy(root)
        if change_copy: change_copy(copy)
        files = site.site_files(copy); ident = site.manifest(files)[1]
        prior_root = srv / ('site-' + 'a' * 12)
        if caddy_text is None:
            caddy_text = site.render_block(ROOT_A) if existing else ':80 {\n\troot * /usr/share/caddy\n\tfile_server\n}\n'
            if studio_route: caddy_text = studio.patch_caddy(caddy_text)
            if faucet_route: caddy_text = faucet.patch_caddy(caddy_text)
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
        def plan(text, site_root, studio_module, faucet_module=None):
            return real_plan(text, '/srv/pocol-explorer/' + Path(site_root).name, studio_module, faucet_module)
        argv = ['--site', str(copy)] + (['--check'] if check else []) + (['--accept-site', 'site-' + ident if accept is True else accept] if accept else [])
        argv += ['--https-wait', str(https_wait)] if https_wait else []
        with patch.multiple(installer, BASE=base, UNIT=unit, CADDYFILE=caddy, SITE_PARENT=srv, BACKUPS=backups, CONFIG=config,
                            NETWORK=config / 'network.json', FORKS=config / 'forks.json',
                            ROOT_UID=os.getuid() if root_uid is None else root_uid), \
             umask_during(umask), \
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
                    self.assertEqual(verify.call_args.args[3], https_wait or installer.HTTPS_WAIT)
                    self.assertEqual(verify.call_args.kwargs, {'faucet_route': faucet_route})
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

    def test_an_update_keeps_the_faucet_and_studio_routes(self):
        for routes in ({'faucet_route': True}, {'faucet_route': True, 'studio_route': True}):
            with self.subTest(**routes):
                text, _, printed, _, ident, _ = self.run_install(existing=True, **routes)
                expected = site.render_block(f'/srv/pocol-explorer/site-{ident}')
                if routes.get('studio_route'): expected = studio.patch_caddy(expected)
                self.assertEqual(text, faucet.patch_caddy(expected))
                self.assertIn('and the faucet route', printed)
        self.run_install(existing=True, faucet_route=True, fail_https=True)

    # The unit the previous release (main at 1bb11f4c) wrote, verbatim: an update replaces it with this release's unit.
    PREVIOUS_UNIT = ("[Unit]\nDescription=PoCol explorer gateway (read-only /rpc and the explorer's /api reads)\nAfter=network.target pocol-standalone.service\n\n"
                     "[Service]\nType=simple\nDynamicUser=yes\nExecStart=/usr/bin/python3 /opt/pocol-explorer-gateway/gateway.py --bind 127.0.0.1 --port 8088 "
                     "--rpc-port 8545 --v3-network /etc/pocol-explorer-gateway/network.json --site-origin https://explorer.pocol.net\n"
                     "WorkingDirectory=/opt/pocol-explorer-gateway\nRestart=on-failure\nRestartSec=3\nNoNewPrivileges=yes\nPrivateTmp=yes\nProtectSystem=strict\n"
                     "ProtectHome=yes\nProtectKernelTunables=yes\nProtectKernelModules=yes\nProtectControlGroups=yes\nRestrictSUIDSGID=yes\nRestrictRealtime=yes\n"
                     "RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX\nIPAddressDeny=any\nIPAddressAllow=localhost\nMemoryMax=256M\nTasksMax=64\nUMask=0077\n\n"
                     "[Install]\nWantedBy=multi-user.target\n")

    def test_the_unit_runs_the_transaction_index_in_its_state_directory(self):
        self.assertIn('pocol_transactions.py', installer.SOURCES)
        for forks in (False, True):
            text = installer.unit_text(forks)
            self.assertIn('StateDirectory=pocol-explorer-gateway\nStateDirectoryMode=0700\n', text)
            self.assertIn('Environment=POCOL_TX_DB=/var/lib/pocol-explorer-gateway/transactions.sqlite3\n', text)
            self.assertIn('After=network.target pocol-standalone.service pocol-observer.service\n', text)
            self.assertIn('IPAddressAllow=localhost\n', text)
        self.assertEqual(installer.PREVIOUS_UNIT_TEXT, self.PREVIOUS_UNIT)

    def test_an_update_from_the_previous_release_unit_writes_this_release_unit(self):
        for forks in (False, True):
            with self.subTest(forks=forks):
                root = self.run_install(existing=True, unit_text=installer.unit_text(forks, self.PREVIOUS_UNIT), forks=forks)[3]
                self.assertEqual((root / 'gateway.service').read_text(), installer.unit_text(forks))
                self.assertEqual((root / 'opt' / 'pocol_transactions.py').read_bytes(), (ROOT / 'deploy/rewards/pocol_transactions.py').read_bytes())
        with self.assertRaisesRegex(RuntimeError, 'custom edits'):
            self.run_install(existing=True, unit_text=self.PREVIOUS_UNIT.replace('TasksMax=64', 'TasksMax=640'))
        # A failed update restores the previous release's unit.
        root = self.run_install(existing=True, unit_text=self.PREVIOUS_UNIT, fail_https=True)[3]
        self.assertEqual((root / 'gateway.service').read_text(), self.PREVIOUS_UNIT)
        self.assertFalse((root / 'opt' / 'pocol_transactions.py').exists())

    def test_https_wait_reaches_the_https_check(self):
        self.run_install(https_wait=1800)

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
        for argv in ([], ['--check'], ['--site'], ['--site', 'x', '--force'], ['--site', 'x', '--check', '--check'],
                     ['--site', '--check'], ['--site', 'x', '--accept-site'], ['--site', 'x', '--accept-site', 'site-xyz'],
                     ['--site', 'x', '--accept-site', 'site-' + 'a' * 12, '--accept-site', 'site-' + 'a' * 12]):
            with self.assertRaisesRegex(RuntimeError, 'Usage'): installer.main(argv)
        for argv in (['--site', 'x', '--https-wait'], ['--site', 'x', '--https-wait', '59'], ['--site', 'x', '--https-wait', '3601'],
                     ['--site', 'x', '--https-wait', '-1'], ['--site', 'x', '--https-wait', '600', '--https-wait', '600']):
            with self.assertRaisesRegex(RuntimeError, 'Usage'): installer.main(argv)
        self.assertEqual(installer.usage(['--site', 'x', '--accept-site', 'site-' + 'a' * 12, '--check']),
                         (Path('x'), True, 'site-' + 'a' * 12, 600))
        self.assertEqual(installer.usage(['--site', 'x']), (Path('x'), False, None, 600))
        self.assertEqual(installer.usage(['--site', 'x', '--https-wait', '1200'])[3], 1200)

    def test_the_https_check_waits_past_the_first_acme_retries(self):
        # Right after the DNS move the CA's first validation can fail; the ACME client retries after about one, then two
        # minutes. A fixed 120 s window rolled the install back (and stopped the issuance) before the second retry.
        self.assertGreaterEqual(installer.HTTPS_WAIT, 600)
        clock = [0.0]
        attempts = []
        def verify(*_):
            attempts.append(clock[0])
            if clock[0] < 400: raise OSError('certificate not yet issued')
        with patch.object(installer, 'verify_https', side_effect=verify), \
             patch.object(installer.time, 'monotonic', side_effect=lambda: clock[0]), \
             patch.object(installer.time, 'sleep', side_effect=lambda s: clock.__setitem__(0, clock[0] + s)):
            installer.verify_https_with_retry(None, {}, False)
            self.assertGreaterEqual(attempts[-1], 400)
            clock[0] = 0.0
            with self.assertRaisesRegex(OSError, 'not yet issued'):
                installer.verify_https_with_retry(None, {}, False, 120)

    def test_a_copy_that_is_not_the_checkouts_snapshot_is_refused_unless_named(self):
        # A leftover copy of an earlier release, or one edited by hand, passes the copy's own checks: refused.
        stale = lambda copy: (copy / 'index.html').write_bytes((copy / 'index.html').read_bytes().replace(b'<title>', b'<title>STALE ', 1))
        for change in (stale, lambda copy: (copy / 'icon-512.png').unlink(), lambda copy: (copy / 'extra.js').write_text('x')):
            with self.subTest(change=change), self.assertRaisesRegex(RuntimeError, 'is not the explorer snapshot of this checkout'):
                self.run_install(check=True, change_copy=change)
        with self.assertRaisesRegex(RuntimeError, '--accept-site names'):
            self.run_install(check=True, change_copy=stale, accept='site-' + 'f' * 12)
        # A copy meant to differ (a switched or rebuilt bundle) is installed when --accept-site names its id.
        printed = self.run_install(check=True, change_copy=stale, accept=True)[2]
        self.assertIn('accepted with --accept-site', printed)
        text, _, printed, _, ident, _ = self.run_install(change_copy=stale, accept=True)
        self.assertEqual(text, site.render_block(f'/srv/pocol-explorer/site-{ident}'))
        self.assertIn("this checkout's snapshot", self.run_install(check=True)[2])
        # The checkout's own snapshot is read from its files and checked against its SHA256SUMS.txt (as site_snapshot.py
        # checks the committed blobs): the copy built from git equals it, and a checkout file that differs is refused.
        with tempfile.TemporaryDirectory() as tmp:
            self.assertEqual(installer.checkout_site(), site.site_files(snapshot_copy(tmp)))
            fake = Path(tmp) / 'checkout'
            shutil.copytree(ROOT / installer.SNAPSHOT, fake / installer.SNAPSHOT)
            (fake / installer.SNAPSHOT / 'favicon.ico').write_bytes(b'changed')
            with patch.object(installer, 'ROOT', fake), self.assertRaisesRegex(RuntimeError, 'differs from its SHA256SUMS'):
                installer.checkout_site()

    def test_directories_are_0755_whatever_the_umask(self):
        # A hardened host's root umask of 027 would make /srv/pocol-explorer and /opt/pocol-explorer-gateway 0750,
        # which Caddy and the DynamicUser service cannot enter.
        _, _, _, root, ident, _ = self.run_install(umask=0o027)
        for folder in (root / 'srv', root / 'opt', root / 'srv' / f'site-{ident}', root / 'srv' / f'site-{ident}' / 'assets'):
            self.assertEqual(oct(folder.stat().st_mode & 0o777), '0o755', folder)
        with tempfile.TemporaryDirectory() as tmp, umask_during(0o077):
            installer.make_directory(Path(tmp) / 'a' / 'b')
            self.assertEqual([oct((Path(tmp) / p).stat().st_mode & 0o777) for p in ('a', 'a/b')], ['0o755', '0o755'])
            installer.make_directory(Path(tmp) / 'a' / 'b')

    def test_paths_the_service_and_caddy_cannot_read_or_others_could_change_are_refused(self):
        cases = {
            'network.json 0600': lambda root: os.chmod(root / 'etc' / 'network.json', 0o600),
            'forks.json 0600': lambda root: ((root / 'etc' / 'forks.json').write_text('[[1, 0]]'), os.chmod(root / 'etc' / 'forks.json', 0o600)),
            'config 0700': lambda root: os.chmod(root / 'etc', 0o700),
            'site parent 0750': lambda root: ((root / 'srv').mkdir(), os.chmod(root / 'srv', 0o750)),
            'site parent group-writable': lambda root: ((root / 'srv').mkdir(), os.chmod(root / 'srv', 0o775)),
            'gateway directory world-writable': lambda root: ((root / 'opt').mkdir(), (root / 'opt' / installer.MARKER).write_text('x'),
                                                              os.chmod(root / 'opt', 0o777)),
        }
        for name, prepare in cases.items():
            with self.subTest(name), self.assertRaisesRegex(RuntimeError, 'must be'):
                self.run_install(check=True, prepare=prepare)
        with self.assertRaisesRegex(RuntimeError, 'root-owned'):
            self.run_install(check=True, prepare=lambda root: (root / 'srv').mkdir(mode=0o755), root_uid=os.getuid() + 1)
        self.assertIn('CHECK PASSED', self.run_install(check=True, prepare=lambda root: os.chmod(root / 'etc', 0o711))[2])

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
