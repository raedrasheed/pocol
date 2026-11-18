"""The faucet's Caddy route on explorer.pocol.net (faucet_route.py): its text and guards, its place in the explorer block
beside the Studio route, `caddy validate` and a live loopback Caddy in front of a faucet with fakes (when Caddy is
installed), and the installer's check, install and rollback paths with every host path in a temporary directory and
every system command faked. Nothing here changes a host, contacts a network or sends a real transfer."""
import http.client
import json
import os
from pathlib import Path
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import time
from types import SimpleNamespace
import unittest
from unittest.mock import MagicMock, patch

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parent / 'studio'))
sys.path.insert(0, str(HERE.parent / 'explorer'))
import faucet_route as route  # noqa: E402
import pocol_faucet as pf  # noqa: E402
import test_pocol_faucet as fixtures  # noqa: E402
import explorer_site as site  # noqa: E402
import install_studio_gateway as studio  # noqa: E402

ROOT_A = '/srv/pocol-explorer/site-' + 'a' * 12
ORIGIN = 'https://pocol.net'
CADDY = '/usr/bin/caddy'


def free_port():
    with socket.socket() as s:
        s.bind(('127.0.0.1', 0)); return s.getsockname()[1]


class RouteTests(unittest.TestCase):
    def test_the_route_text(self):
        route.check_forced_loopback()
        self.assertEqual(route.ROUTE.count(route.CLIENT_IP_HEADER), 1)
        self.assertIn('        path /faucet/claim /faucet/status\n        method GET POST OPTIONS\n', route.ROUTE)
        self.assertIn('    handle @pocol_faucet {\n        uri strip_prefix /faucet\n        reverse_proxy 127.0.0.1:8547 {\n', route.ROUTE)
        self.assertIn('                dial_timeout 3s\n                response_header_timeout 45s\n', route.ROUTE)
        # The signer alone may take 30 s (pocol_faucet.py), so Caddy waits longer than that for the answer.
        signer = int(re.search(r'subprocess\.run\(cmd, [^\n]*timeout=([0-9]+)\)', Path(pf.__file__).read_text()).group(1))
        self.assertGreater(int(re.search(r'response_header_timeout ([0-9]+)s', route.ROUTE).group(1)), signer)
        self.assertNotIn('max_size', route.ROUTE); self.assertNotIn('8088', route.ROUTE); self.assertNotIn('8089', route.ROUTE)
        # Caddy 2.7+ refuses a token after '{' on the same line: every '{' ends its line, every '}' is alone on one.
        for line in route.ROUTE.splitlines():
            bare = re.sub(r'`[^`]*`|\{[A-Za-z][A-Za-z0-9_.-]*\}', '', line)
            if '{' in bare: self.assertTrue(line.endswith(' {'), line)
            if '}' in bare: self.assertEqual(line.strip(), '}', line)
        for edit in (lambda r: r.replace('            ' + route.CLIENT_IP_HEADER + '\n', ''),
                     lambda r: r.replace(route.CLIENT_IP_HEADER, route.CLIENT_IP_HEADER + '\n            ' + route.CLIENT_IP_HEADER),
                     lambda r: r.replace('127.0.0.1:8547', '0.0.0.0:8547'),
                     lambda r: r.replace('        uri strip_prefix /faucet\n', ''),
                     lambda r: r.replace('reverse_proxy 127.0.0.1:8547 {', 'reverse_proxy 127.0.0.1:8547 127.0.0.1:9 {'),
                     lambda r: r.replace('    ' + route.END + '\n', '')):
            with patch.object(route, 'ROUTE', edit(route.ROUTE)), self.assertRaisesRegex(RuntimeError, 'X-Forwarded-For'):
                route.patch_caddy(site.render_block(ROOT_A))

    def test_patch_inserts_before_the_rpc_route_and_is_idempotent(self):
        block = site.render_block(ROOT_A)
        patched = route.patch_caddy(block)
        self.assertEqual(patched, block.replace('    @rpc path /rpc\n', route.ROUTE + '    @rpc path /rpc\n'))
        self.assertEqual(route.patch_caddy(patched), patched)
        self.assertEqual(patched.replace(route.ROUTE, ''), block)
        self.assertEqual(patched.count(route.CLIENT_IP_HEADER), 1)
        self.assertEqual(patched.count('header_up'), 1)
        self.assertIn('max_size 16384', patched); self.assertNotIn('max_size 131072', patched)
        # The site's error matcher is unchanged (render_block's @explorer_error line).
        self.assertIn('        @explorer_error not path /rpc /health /studio/*\n', patched)
        # With the Studio route, in either order.
        both = route.patch_caddy(studio.patch_caddy(block))
        self.assertEqual(studio.patch_caddy(patched), both)
        self.assertEqual(both.count('header_up X-Forwarded-For'), 1)
        for line in both.splitlines():
            bare = re.sub(r'`[^`]*`|\{[A-Za-z][A-Za-z0-9_.-]*\}', '', line)
            if '{' in bare: self.assertTrue(line.endswith(' {'), line)
            if '}' in bare: self.assertEqual(line.strip(), '}', line)

    def test_refusals(self):
        block = site.render_block(ROOT_A)
        patched = route.patch_caddy(block)
        other = 'other.example {\n    reverse_proxy 127.0.0.1:8547\n}\n'
        cases = {
            'port elsewhere before': (block + other, 'faucet port'),
            'port elsewhere after': (patched + other, 'faucet port'),
            'localhost port after': (patched + 'other.example {\n    reverse_proxy localhost:8547\n}\n', 'faucet port'),
            'edited route': (patched.replace('response_header_timeout 45s', 'response_header_timeout 99s'), 'custom edits'),
            'header removed': (patched.replace('            ' + route.CLIENT_IP_HEADER + '\n', ''), 'custom edits'),
            'duplicated route': (patched.replace(route.ROUTE, route.ROUTE * 2), 'custom edits'),
            'end marker only': (block + '# END POCOL FAUCET v1\n', 'custom edits'),
            'begin marker damaged': (patched.replace('    ' + route.END + '\n', ''), 'custom edits'),
            'no explorer block': ('example.org {\n    respond "hi"\n}\n', 'explorer.pocol.net'),
            'two explorer blocks': (block + block.replace(site.BEGIN, '# other').replace(site.END, '# other end'), 'explorer.pocol.net'),
            'legacy host block (no @rpc line)': (fixtures_base(), '/rpc route'),
            'rpc line before the host': ('    @rpc path /rpc\n' + block.replace('    @rpc path /rpc\n', '    @rpc path /rpc2\n'), '/rpc route'),
            'no explorer gateway route': (block.replace('reverse_proxy 127.0.0.1:8088', 'reverse_proxy 127.0.0.1:8090'), 'loopback route'),
        }
        for name, (text, message) in cases.items():
            with self.subTest(name), self.assertRaisesRegex(RuntimeError, message):
                route.patch_caddy(text)

    def loopback(self, directory, listen, faucet_port, gateway_port):
        """The production block with both routes as a plain-HTTP loopback copy (as explorer_site.loopback_config)."""
        both = route.patch_caddy(studio.patch_caddy(site.render_block(ROOT_A)))
        return ('{\n    admin off\n}\n\n' + both.replace('explorer.pocol.net {', f'http://{listen} {{', 1)
                .replace(ROOT_A, str(Path(directory).resolve()), 1).replace('127.0.0.1:8547', f'127.0.0.1:{faucet_port}', 1)
                .replace('127.0.0.1:8088', f'127.0.0.1:{gateway_port}'))

    def caddy_env(self, tmp):
        return {**os.environ, 'HOME': tmp, 'XDG_DATA_HOME': tmp, 'XDG_CONFIG_HOME': tmp}

    @unittest.skipUnless(os.path.exists(CADDY), 'Caddy is not installed')
    def test_caddy_validates_the_patched_block(self):
        with tempfile.TemporaryDirectory() as tmp:
            config = Path(tmp) / 'Caddyfile'
            config.write_text(self.loopback(tmp, '127.0.0.1:8661', 8547, 8088))
            result = subprocess.run([CADDY, 'validate', '--config', str(config), '--adapter', 'caddyfile'], capture_output=True,
                                    text=True, timeout=60, env=self.caddy_env(tmp))
            self.assertEqual(result.returncode, 0, result.stderr[-2000:])

    @unittest.skipUnless(os.path.exists(CADDY), 'Caddy is not installed')
    def test_a_live_loopback_caddy_strips_the_prefix_and_overwrites_the_client_header(self):
        rules = fixtures.FaucetRules('test_http_surface'); rules.setUp()
        faucet = rules.faucet(allowOrigin=ORIGIN, trustProxy=True, perClientDaily=1)
        server = pf.Server(('127.0.0.1', 0), pf.handler(faucet))
        threading.Thread(target=server.serve_forever, daemon=True).start()
        self.addCleanup(lambda: (server.shutdown(), server.server_close()))
        tmp = tempfile.mkdtemp(prefix='pocol-faucet-caddy-'); self.addCleanup(shutil.rmtree, tmp, True)
        self.addCleanup(rules.doCleanups)
        listen = f'127.0.0.1:{free_port()}'
        (Path(tmp) / 'index.html').write_text('<p>explorer</p>')
        config = Path(tmp) / 'Caddyfile'; config.write_text(self.loopback(tmp, listen, server.server_address[1], free_port()))
        log = open(Path(tmp) / 'caddy.log', 'w'); self.addCleanup(log.close)
        caddy = subprocess.Popen([CADDY, 'run', '--config', str(config), '--adapter', 'caddyfile'], stdout=log, stderr=log,
                                 env=self.caddy_env(tmp))
        self.addCleanup(lambda: (caddy.terminate(), caddy.wait(10)))
        host, port = listen.split(':')
        def call(method, path, body=None, headers=None):
            c = http.client.HTTPConnection(host, int(port), timeout=10)
            try:
                c.request(method, path, body=body, headers={'Content-Type': 'application/json', **(headers or {})})
                r = c.getresponse(); return r.status, r.read(), r
            finally: c.close()
        deadline = time.monotonic() + 20
        while True:
            try: call('GET', '/index.html'); break
            except OSError:
                if time.monotonic() > deadline or caddy.poll() is not None:
                    self.fail('Caddy did not start: ' + (Path(tmp) / 'caddy.log').read_text()[-2000:])
                time.sleep(.2)
        status, body, r = call('GET', '/faucet/status')
        self.assertEqual((status, json.loads(body)['type'], r.getheader('Access-Control-Allow-Origin')), (200, 'status', ORIGIN))
        self.assertIsNone(r.getheader('Server'))
        status, body, r = call('OPTIONS', '/faucet/claim', None, {'Origin': ORIGIN, 'Access-Control-Request-Method': 'POST'})
        self.assertEqual((status, r.getheader('Access-Control-Max-Age'), r.getheader('Access-Control-Allow-Origin')), (204, '600', ORIGIN))
        # A spoofed X-Forwarded-For is overwritten with the connecting address: the counter is the real client's.
        status, body, r = call('POST', '/faucet/claim', json.dumps({'address': fixtures.NEW}), {'X-Forwarded-For': '6.6.6.6'})
        self.assertEqual((status, json.loads(body)['to']), (200, fixtures.NEW))
        self.assertEqual(set(faucet.clients), {(pf.today(), '127.0.0.1')})
        status, body, r = call('POST', '/faucet/claim', json.dumps({'address': '0x' + 'cd' * 20}), {'X-Forwarded-For': '7.7.7.7'})
        self.assertEqual((status, json.loads(body)['code'], r.getheader('Access-Control-Allow-Origin')), (429, 'client_limit', ORIGIN))
        status, body, _ = call('POST', '/faucet/claim', json.dumps({'address': '0x'}))
        self.assertEqual((status, json.loads(body)['code']), (400, 'invalid_address'))
        # Only the two paths and three methods reach the faucet; everything else is the explorer site's.
        for method, path in (('GET', '/faucet/other'), ('GET', '/faucet/'), ('GET', '/status'), ('POST', '/claim'),
                             ('PUT', '/faucet/claim')):
            with self.subTest(path=path):
                status, body, r = call(method, path, '{}')
                self.assertIsNone(r.getheader('Access-Control-Allow-Origin'), (method, path, status, body))
        self.assertEqual(len(rules.node.sent), 1)


def fixtures_base():
    # The Studio installer tests' fixture of the lost host block (deploy/studio/test_studio_gateway.py): no managed
    # explorer block, no `@rpc path /rpc` line.
    return '''explorer.pocol.net {
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


class InstallerTests(unittest.TestCase):
    CHAIN = fixtures.CHAIN

    def answers(self, changes=None, posted=None):
        """A fake `local`/`tls`: the faucet's and the explorer's answers, with `changes` by (method, path)."""
        changes = changes or {}
        def call(method, path, payload=None, headers=None):
            if posted is not None and method == 'POST': posted.append((path, payload))
            if (method, path) in changes: return changes[(method, path)]
            response = SimpleNamespace(getheader={'Access-Control-Allow-Origin': ORIGIN, 'Access-Control-Max-Age': '600'}.get)
            bare = path[len('/faucet'):] if path.startswith('/faucet/') else path
            if (method, bare) == ('GET', '/status'):
                return 200, response, json.dumps({'type': 'status', 'chainId': self.CHAIN}).encode()
            if (method, bare) == ('OPTIONS', '/claim'): return 204, response, b''
            if (method, bare) == ('POST', '/claim') and payload == route.PROBE:
                return 400, response, b'{"type":"error","code":"invalid_address","message":"x"}'
            if (method, path) == ('GET', '/health'): return 200, response, b'{"service":"pocol-explorer-gateway"}'
            return 404, response, b'{}'
        return call

    def run_main(self, argv=(), caddy_text=None, config=None, fail_https=False, changes=None):
        tmp = tempfile.TemporaryDirectory(); self.addCleanup(tmp.cleanup)
        root = Path(tmp.name); caddy = root / 'Caddyfile'; cfg = root / 'config.json'; backups = root / 'backups'
        caddy_text = site.render_block(ROOT_A) if caddy_text is None else caddy_text
        caddy.write_text(caddy_text)
        cfg.write_text(json.dumps(config if config is not None else
                                  {'chainId': self.CHAIN, 'listen': '127.0.0.1', 'port': 8547, 'trustProxy': True, 'allowOrigin': ORIGIN}))
        commands, posted = [], []
        def command(*args, **kwargs):
            commands.append(args); return SimpleNamespace(returncode=0, stdout=b'', stderr=b'')
        verify = patch.object(route, 'verify_https_with_retry', side_effect=RuntimeError('TLS route failed')) if fail_https else \
            patch.object(route.time, 'sleep')
        with patch.multiple(route, CADDYFILE=caddy, CONFIG=cfg, BACKUPS=backups), \
             patch.object(route.os, 'geteuid', return_value=0, create=True), \
             patch.object(route.shutil, 'which', return_value='/test/tool'), \
             patch.object(route, 'command', side_effect=command), \
             patch.object(route, 'local', side_effect=self.answers(changes, posted)), \
             patch.object(route, 'tls', side_effect=self.answers(changes, posted)), \
             verify, patch('builtins.print') as output:
            try: route.main(list(argv))
            finally:
                self.printed = ' '.join(str(c) for c in output.call_args_list)
                self.commands, self.posted, self.root = commands, posted, root
                # Only the probe address is ever posted; the faucet refuses it before any other check.
                self.assertTrue(all(payload == route.PROBE for _, payload in posted), posted)
        return caddy.read_text()

    def test_the_probe_is_never_a_valid_claim(self):
        self.assertIsNone(pf.ADDRESS.match(route.PROBE['address']))
        self.assertEqual(set(route.PROBE), {'address'})

    def test_install(self):
        for start in (site.render_block(ROOT_A), studio.patch_caddy(site.render_block(ROOT_A))):
            with self.subTest(studio=studio.MARKER in start):
                text = self.run_main(caddy_text=start)
                self.assertEqual(text, route.patch_caddy(start))
                self.assertIn('INSTALLED:', self.printed)
                self.assertIn(('systemctl', 'reload', 'caddy'), self.commands)
                validate = [c for c in self.commands if c[:2] == ('caddy', 'validate')]
                self.assertEqual(len(validate), 1); self.assertEqual(validate[0][-2:], ('--adapter', 'caddyfile'))
                self.assertLess(self.commands.index(validate[0]), self.commands.index(('systemctl', 'reload', 'caddy')))
                self.assertEqual([p.read_text() for p in self.root.glob('backups/*/Caddyfile.before')], [start])
                self.assertEqual({p for p, _ in self.posted}, {'/claim', '/faucet/claim'}, 'loopback first, then HTTPS')
        # Already installed: verified over HTTPS, nothing written or reloaded.
        patched = route.patch_caddy(site.render_block(ROOT_A))
        self.assertEqual(self.run_main(caddy_text=patched), patched)
        self.assertIn('already installed', self.printed); self.assertNotIn(('systemctl', 'reload', 'caddy'), self.commands)
        self.assertFalse((self.root / 'backups').exists())

    def test_check_changes_nothing(self):
        block = site.render_block(ROOT_A)
        self.assertEqual(self.run_main(['--check'], caddy_text=block), block)
        self.assertIn('CHECK PASSED', self.printed)
        self.assertNotIn(('systemctl', 'reload', 'caddy'), self.commands)
        self.assertTrue(any(c[:2] == ('caddy', 'validate') for c in self.commands))
        self.assertFalse((self.root / 'backups').exists())

    def test_a_failed_https_check_restores_the_caddyfile(self):
        block = site.render_block(ROOT_A)
        with self.assertRaisesRegex(RuntimeError, 'TLS route failed'): self.run_main(caddy_text=block, fail_https=True)
        self.assertEqual((self.root / 'Caddyfile').read_text(), block)
        self.assertEqual(self.commands.count(('systemctl', 'reload', 'caddy')), 2, 'reloaded, then reloaded back')
        self.assertNotIn('INSTALLED:', self.printed)

    def test_https_answers(self):
        with patch.object(route, 'tls', side_effect=self.answers()) as tls:
            route.verify_https(self.CHAIN, ORIGIN)
            asked = [c.args[:2] for c in tls.call_args_list]
            self.assertEqual(asked, [('GET', '/faucet/status'), ('OPTIONS', '/faucet/claim'), ('POST', '/faucet/claim'), ('GET', '/health')])
        no_cors = SimpleNamespace(getheader={}.get)
        good = SimpleNamespace(getheader={'Access-Control-Allow-Origin': ORIGIN}.get)
        for changes, message in (({('GET', '/faucet/status'): (502, no_cors, b'')}, 'status'),
                                 ({('GET', '/faucet/status'): (200, no_cors, json.dumps({'type': 'status', 'chainId': self.CHAIN}).encode())}, 'allow'),
                                 ({('GET', '/faucet/status'): (200, good, json.dumps({'type': 'status', 'chainId': self.CHAIN + 1}).encode())}, 'chain'),
                                 ({('OPTIONS', '/faucet/claim'): (204, good, b'')}, 'preflight'),
                                 ({('POST', '/faucet/claim'): (400, no_cors, b'{"code":"invalid_address"}')}, 'probe'),
                                 ({('POST', '/faucet/claim'): (502, good, b'')}, 'probe'),
                                 ({('GET', '/health'): (502, good, b'')}, 'explorer')):
            with self.subTest(message), patch.object(route, 'tls', side_effect=self.answers(changes)), \
                    self.assertRaisesRegex(RuntimeError, message):
                route.verify_https(self.CHAIN, ORIGIN)

    def test_loopback_answers_of_a_real_faucet(self):
        rules = fixtures.FaucetRules('test_http_surface'); rules.setUp(); self.addCleanup(rules.doCleanups)
        for allow, ok in ((ORIGIN, True), (None, False)):
            with self.subTest(allowOrigin=allow):
                faucet = rules.faucet(allowOrigin=allow, trustProxy=True)
                server = pf.Server(('127.0.0.1', 0), pf.handler(faucet))
                threading.Thread(target=server.serve_forever, daemon=True).start()
                try:
                    with patch.object(route, 'PORT', server.server_address[1]):
                        if ok: route.check_answers(route.local, '', fixtures.CHAIN, ORIGIN)
                        else:
                            with self.assertRaisesRegex(RuntimeError, 'allow'): route.check_answers(route.local, '', fixtures.CHAIN, ORIGIN)
                        with self.assertRaisesRegex(RuntimeError, 'chain'): route.check_answers(route.local, '', fixtures.CHAIN + 1, ORIGIN)
                finally: server.shutdown(); server.server_close()
                self.assertEqual(faucet.ledger.claims, {}); self.assertFalse(rules.node.sent)

    def test_refusals(self):
        good = {'chainId': self.CHAIN, 'listen': '127.0.0.1', 'port': 8547, 'trustProxy': True, 'allowOrigin': ORIGIN}
        for edit, message in (({'trustProxy': False}, 'trustProxy'), ({'trustProxy': 'true'}, 'trustProxy'),
                              ({'allowOrigin': None}, 'allowOrigin'), ({'allowOrigin': 'https://pocol.net/'}, 'allowOrigin'),
                              ({'allowOrigin': 'http://pocol.net'}, 'allowOrigin'), ({'allowOrigin': '*'}, 'allowOrigin'),
                              ({'port': 8548}, 'listen on'), ({'listen': '0.0.0.0'}, 'listen on'), ({'chainId': '1'}, 'chainId')):
            with self.subTest(edit), self.assertRaisesRegex(RuntimeError, message):
                self.run_main(['--check'], config={**good, **edit})
        block = site.render_block(ROOT_A)
        for text, message in ((block.replace('dial_timeout 3s', 'dial_timeout 30s'), 'custom edits'),
                              (block + 'other.example {\n    reverse_proxy 127.0.0.1:8547\n}\n', 'faucet port'),
                              (fixtures_base(), '/rpc route')):
            with self.subTest(message), self.assertRaisesRegex(RuntimeError, message):
                self.run_main(['--check'], caddy_text=text)
        for argv in (['--force'], ['--check', '--check']):
            with self.assertRaisesRegex(RuntimeError, 'Usage'): route.main(argv)
        # The loopback faucet must answer before anything is planned.
        with self.assertRaisesRegex(RuntimeError, 'status'):
            self.run_main(['--check'], changes={('GET', '/status'): (502, SimpleNamespace(getheader={}.get), b'')})

    def test_the_mainnet_example_config_fits_the_route(self):
        example = json.loads((HERE / 'config.mainnet.example.json').read_text(encoding='utf-8'))
        self.assertEqual(route.check_config(example), (example['chainId'], 'https://pocol.net'))

    def test_tls_uses_the_public_hostname_trust_and_loopback_transport(self):
        context, connection = MagicMock(), MagicMock()
        connection.getresponse.return_value.status = 200
        connection.getresponse.return_value.read.return_value = b'{}'
        with patch.object(route.ssl, 'create_default_context', return_value=context) as trust, \
             patch.object(route.socket, 'create_connection', return_value='local-socket') as sock, \
             patch.object(route.http.client, 'HTTPSConnection', return_value=connection) as https:
            self.assertEqual(route.tls('GET', '/faucet/status')[0], 200)
            trust.assert_called_once_with()
            sock.assert_called_once_with(('127.0.0.1', 443), timeout=12)
            context.wrap_socket.assert_called_once_with('local-socket', server_hostname='explorer.pocol.net')
            https.assert_called_once_with('explorer.pocol.net', 443, timeout=12, context=context)
            connection.close.assert_called_once()
        source = Path(route.__file__).read_text()
        self.assertNotIn('CERT_NONE', source); self.assertNotIn('check_hostname', source); self.assertNotIn('_create_unverified', source)


if __name__ == '__main__': unittest.main()
