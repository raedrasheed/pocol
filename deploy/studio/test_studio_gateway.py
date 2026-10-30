import http.client
import json
from pathlib import Path
import subprocess
import tempfile
import threading
from types import SimpleNamespace
import unittest
from unittest.mock import patch, MagicMock
import pocol_studio_gateway as g
import install_studio_gateway as installer
from install_studio_gateway import patch_caddy, ROUTE


def enc(value):
    if isinstance(value, list):
        raw = b''.join(map(enc, value)); offset = 0xc0
    else:
        raw = value; offset = 0x80
        if len(raw) == 1 and raw[0] < 128: return raw
    if len(raw) <= 55: return bytes([offset+len(raw)])+raw
    size = len(raw).to_bytes((len(raw).bit_length()+7)//8, 'big')
    return bytes([offset+55+len(size)])+size+raw


def tx(chain=g.CHAIN_ID, data=b''):
    return '0x'+(b'\x02'+enc([chain.to_bytes(4, 'big'), b'', b'\x01', b'\x02', b'\x52\x08', b'a'*20, b'', data, [], b'', b'\x01', b'\x02'])).hex()


def request(method='eth_chainId', params=None, ident=1):
    return {'jsonrpc': '2.0', 'id': ident, 'method': method, 'params': params or []}


class ValidationTests(unittest.TestCase):
    def test_chunk_transaction_and_wrong_network(self):
        self.assertTrue(g.valid_raw_transaction(tx(data=b'x'*24643)))
        self.assertFalse(g.valid_raw_transaction(tx(chain=1)))
        self.assertFalse(g.valid_raw_transaction(tx()+'00'))
        self.assertFalse(g.valid_raw_transaction('0x01'+tx()[4:]))
        self.assertFalse(g.valid_raw_transaction(tx(data=b'x'*60000)))

    def test_privileged_calls_and_arbitrary_upstream_rejected(self):
        for method in ['eth_sendTransaction', 'personal_unlockAccount', 'admin_peers', 'miner_start', 'debug_traceTransaction']:
            self.assertFalse(g.valid_call(request(method)))
        self.assertFalse(g.valid_call({**request(), 'url': 'http://attacker'}))
        self.assertFalse(g.valid_call(request(ident=True)))
        self.assertFalse(g.valid_call(request('eth_getBlockByNumber', ['latest', True])))

    def test_batch_write_and_duplicate_ids_refused(self):
        self.assertFalse(g.valid_payload([request('eth_sendRawTransaction', [tx()])]))
        self.assertFalse(g.valid_payload([request(), request()]))
        self.assertTrue(g.valid_payload([request(), request('pocol_status', ident=2)]))

    def test_call_budget_and_pending_nonce(self):
        self.assertTrue(g.valid_call(request('eth_getTransactionCount', ['0x'+'1'*40, 'pending'])))
        self.assertTrue(g.valid_call(request('eth_call', [{'to': '0x'+'1'*40, 'data': '0x1234'}, 'latest'])))
        self.assertFalse(g.valid_call(request('eth_call', [{'to': '0x'+'1'*40, 'gas': '0xffffff'}, 'latest'])))
        self.assertFalse(g.valid_call(request('eth_call', [{'to': '0x'+'1'*40, 'accessList': []}, 'latest'])))

    def test_private_status_redacted(self):
        self.assertEqual(g.sanitize('pocol_status', {'chainId': g.CHAIN_ID, 'peers': ['private'], 'rpcAddr': 'secret'}), {'chainId': g.CHAIN_ID})


class GatewayTests(unittest.TestCase):
    def test_identity_failure_prevents_broadcast(self):
        gateway = g.Gateway(); calls = []
        def query(method, params):
            calls.append(method)
            return {'result': {'chainId': g.CHAIN_ID, 'genesisHash': 'wrong'}} if method == 'pocol_status' else {'result': hex(g.CHAIN_ID)}
        gateway.query = query
        answer = gateway.call(request('eth_sendRawTransaction', [tx()]))
        self.assertIn('error', answer); self.assertNotIn('eth_sendRawTransaction', calls)

    def test_write_timeout_is_never_retried(self):
        gateway = g.Gateway(); calls = []
        def query(method, params):
            calls.append(method)
            if method == 'pocol_status': return {'result': {'chainId': g.CHAIN_ID, 'genesisHash': g.GENESIS}}
            if method == 'eth_chainId': return {'result': hex(g.CHAIN_ID)}
            raise TimeoutError()
        gateway.query = query
        self.assertIn('error', gateway.call(request('eth_sendRawTransaction', [tx()])))
        self.assertEqual(calls.count('eth_sendRawTransaction'), 1)

    def test_write_network_rechecked_even_after_read(self):
        gateway = g.Gateway(); checks = []
        gateway.verify_network = lambda **kw: checks.append(kw)
        gateway.query = lambda method, params: {'jsonrpc': '2.0', 'id': 1, 'result': '0x1'}
        gateway.call(request()); gateway.call(request('eth_sendRawTransaction', [tx()]))
        self.assertEqual(checks, [{'force': False}, {'force': True}])

    def test_read_and_write_rate_limits_are_bounded(self):
        gateway = g.Gateway()
        with patch('pocol_studio_gateway.time.monotonic', return_value=100):
            for _ in range(6): self.assertTrue(gateway.charge('client', write=True))
            self.assertFalse(gateway.charge('client', write=True))
            self.assertTrue(gateway.charge('different', write=True))
            self.assertTrue(gateway.charge('client', write=False))

    def test_public_listener_refused(self):
        with self.assertRaises(ValueError): g.Server(('0.0.0.0', 0), g.Gateway())


class HTTPTests(unittest.TestCase):
    def setUp(self):
        self.gateway = g.Gateway()
        self.gateway.verify_network = lambda **kw: None
        self.gateway.query = lambda method, params: {'jsonrpc': '2.0', 'id': 1, 'result': hex(g.CHAIN_ID)}
        self.server = g.Server(('127.0.0.1', 0), self.gateway)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True); self.thread.start()

    def tearDown(self):
        self.server.shutdown(); self.server.server_close(); self.thread.join()

    def call(self, method, path, body=None, headers=None):
        conn = http.client.HTTPConnection('127.0.0.1', self.server.server_port, timeout=3)
        conn.request(method, path, body, headers or {})
        response = conn.getresponse(); data = response.read(); status = response.status; conn.close()
        return status, data

    def test_actual_http_relay_health_and_allowlist(self):
        status, data = self.call('GET', '/studio/health'); self.assertEqual(status, 200); self.assertTrue(json.loads(data)['ready'])
        status, data = self.call('POST', '/studio/rpc', json.dumps(request()), {'Content-Type': 'application/json'})
        self.assertEqual(status, 200); self.assertEqual(json.loads(data)['result'], hex(g.CHAIN_ID))
        status, _ = self.call('POST', '/studio/rpc', json.dumps(request('admin_peers')), {'Content-Type': 'application/json'})
        self.assertEqual(status, 400)
        status, _ = self.call('POST', '/rpc', '{}', {'Content-Type': 'application/json'}); self.assertEqual(status, 404)

    def test_size_content_type_and_bad_json(self):
        status, _ = self.call('POST', '/studio/rpc', 'x'*(g.MAX_REQUEST+1), {'Content-Type': 'application/json'}); self.assertEqual(status, 413)
        status, _ = self.call('POST', '/studio/rpc', '{}'); self.assertEqual(status, 415)
        status, _ = self.call('POST', '/studio/rpc', '{', {'Content-Type': 'application/json'}); self.assertEqual(status, 400)


class CaddyTests(unittest.TestCase):
    BASE = '''gateway.pocol.net {
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

    def test_preserve_original_explorer_guard_and_idempotence(self):
        patched = patch_caddy(self.BASE)
        self.assertIn(ROUTE, patched); self.assertIn('max_size 131072', patched)
        self.assertIn('int({http.request.header.Content-Length}) > 16384', patched)
        self.assertIn('X-Content-Type-Options "nosniff"', patched)
        self.assertIn('Cache-Control "no-store"', patched)
        self.assertEqual(patch_caddy(patched), patched)
        self.assertEqual(patched.replace(ROUTE, '').replace('max_size 131072', 'max_size 16384'), self.BASE)

    def test_unrecognized_configs_refused(self):
        for text in ('example.com {}', self.BASE.replace('max_size 16384', 'max_size 20000'), self.BASE.replace('Request body exceeds 16 KiB', 'changed')):
            with self.assertRaises(RuntimeError): patch_caddy(text)
        with self.assertRaises(RuntimeError): patch_caddy(patch_caddy(self.BASE).replace('127.0.0.1:8089', '0.0.0.0:8089'))


class InstallerTests(unittest.TestCase):
    def test_rollback_continues_after_caddy_command_timeout(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); base = root/'opt'; unit = root/'gateway.service'; caddy = root/'Caddyfile'
            base.mkdir(); source = base/'pocol_studio_gateway.py'; marker = base/'managed-by-studio-v1'
            for path in (source, marker, unit): path.write_bytes(b'new')
            caddy.write_bytes(b'new caddy')
            commands = []
            def command(*args):
                commands.append(args)
                if args == ('systemctl', 'reload', 'caddy'):
                    raise subprocess.TimeoutExpired(args, 45)
                return SimpleNamespace(returncode=0)
            with patch.multiple(installer, BASE=base, UNIT=unit, CADDYFILE=caddy), \
                 patch.object(installer, 'command', side_effect=command):
                failures = installer.rollback_install(b'old caddy', {source: None, marker: None, unit: None}, False, False, False)
            self.assertEqual(caddy.read_bytes(), b'old caddy')
            self.assertFalse(base.exists()); self.assertFalse(unit.exists())
            self.assertEqual(len(failures), 1)
            self.assertIn('reload previous Caddy configuration', failures[0])
            self.assertIn(('systemctl', 'stop', installer.SERVICE), commands)
            self.assertIn(('systemctl', 'disable', installer.SERVICE), commands)
            self.assertEqual(commands[-1], ('systemctl', 'daemon-reload'))

    def test_https_checks_both_routes_and_both_identities(self):
        health = {'service': 'pocol-studio-gateway', 'ready': True, 'chainId': g.CHAIN_ID, 'genesisHash': g.GENESIS}
        explorer = {'jsonrpc': '2.0', 'id': 1, 'result': {'chainId': g.CHAIN_ID, 'genesisHash': g.GENESIS}}
        with patch.object(installer, 'tls_json', side_effect=[health, explorer]) as get:
            installer.verify_https_routes()
            self.assertEqual(get.call_args_list[0].args, ('/studio/health',))
            self.assertEqual(get.call_args_list[1].args, ('/rpc', request('pocol_getParams')))
        with patch.object(installer, 'tls_json', side_effect=[health, {**explorer, 'result': {'chainId': 1, 'genesisHash': g.GENESIS}}]):
            with self.assertRaisesRegex(RuntimeError, 'Existing Explorer HTTPS'):
                installer.verify_https_routes()
        with patch.object(installer, 'tls_json', return_value={**health, 'ready': False}):
            with self.assertRaisesRegex(RuntimeError, 'Studio HTTPS'):
                installer.verify_https_routes()

    def test_tls_uses_public_hostname_trust_and_loopback_transport(self):
        context = MagicMock()
        connection = MagicMock()
        connection.getresponse.return_value.status = 200
        connection.getresponse.return_value.read.return_value = b'{"ready":true}'
        with patch.object(installer.ssl, 'create_default_context', return_value=context) as trust, \
             patch.object(installer.socket, 'create_connection', return_value='local-socket') as sock, \
             patch.object(installer.http.client, 'HTTPSConnection', return_value=connection) as https:
            self.assertEqual(installer.tls_json('/studio/health'), {'ready': True})
            trust.assert_called_once_with()
            sock.assert_called_once_with(('127.0.0.1', 443), timeout=12)
            context.wrap_socket.assert_called_once_with('local-socket', server_hostname='gateway.pocol.net')
            https.assert_called_once_with('gateway.pocol.net', 443, timeout=12, context=context)
            connection.close.assert_called_once()

    def run_install(self, fail_https=False, existing=False):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary); base = root/'opt'; unit = root/'gateway.service'; caddy = root/'Caddyfile'; backups = root/'backups'
            original = CaddyTests.BASE.encode(); caddy.write_bytes(original)
            prior_source = b'# existing source preserved\n'
            if existing:
                base.mkdir(); (base/'managed-by-studio-v1').write_bytes(b'PoCol Studio gateway v1\n')
                (base/'pocol_studio_gateway.py').write_bytes(prior_source); unit.write_text(installer.UNIT_TEXT)
            commands = []
            def command(*args, **kwargs):
                commands.append(args)
                absent = args[:3] in [('systemctl', 'is-active', '--quiet'), ('systemctl', 'is-enabled', '--quiet')] and args[-1] == installer.SERVICE and not existing
                return SimpleNamespace(returncode=1 if absent else 0, stdout=b'', stderr=b'')
            gateway = MagicMock()
            with patch.multiple(installer, BASE=base, UNIT=unit, CADDYFILE=caddy, BACKUPS=backups), \
                 patch.object(installer.os, 'geteuid', return_value=0), \
                 patch.object(installer.shutil, 'which', return_value='/test/tool'), \
                 patch.object(installer.sys, 'argv', ['installer']), \
                 patch.object(installer.socket, 'socket', return_value=MagicMock()), \
                 patch.object(installer, 'command', side_effect=command), \
                 patch.object(g, 'Gateway', return_value=gateway), \
                 patch.object(installer, 'verify_health'), \
                 patch.object(installer, 'verify_https_routes', side_effect=RuntimeError('TLS route failed') if fail_https else None) as verify, \
                 patch('builtins.print') as output:
                if fail_https:
                    with self.assertRaisesRegex(RuntimeError, 'TLS route failed'): installer.main()
                    self.assertEqual(caddy.read_bytes(), original)
                    if existing:
                        self.assertEqual((base/'pocol_studio_gateway.py').read_bytes(), prior_source)
                        self.assertTrue(unit.exists())
                        self.assertEqual(commands[-1], ('systemctl', 'restart', installer.SERVICE))
                    else:
                        self.assertFalse(base.exists()); self.assertFalse(unit.exists())
                    self.assertFalse(any('INSTALLED:' in str(c) for c in output.call_args_list))
                else:
                    installer.main(); verify.assert_called_once_with()
                    self.assertEqual(caddy.read_text(), patch_caddy(original.decode()))
                    self.assertTrue(unit.exists())
                    self.assertIn(('systemctl', 'enable', installer.SERVICE), commands)
                    self.assertTrue(any('both Studio and existing Explorer HTTPS' in str(c) for c in output.call_args_list))
                gateway.verify_network.assert_called_once_with(force=True)
                self.assertEqual(len(list(backups.glob('*/Caddyfile.before'))), 1)

    def test_install_reports_success_only_after_https_routes_pass(self): self.run_install()
    def test_first_install_restores_caddy_and_removes_new_service_on_tls_failure(self): self.run_install(fail_https=True)
    def test_update_restores_prior_service_source_and_active_state_on_tls_failure(self): self.run_install(fail_https=True, existing=True)


if __name__ == '__main__': unittest.main()
