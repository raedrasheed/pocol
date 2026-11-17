"""The explorer snapshot's /api reads on the gateway (explorer_api.py, Sites W8): the allow-list and payload checks of
the retired lib.php (the 43 explorer checks api_security.php had), every handler over HTTP against a loopback fake node
(methods, Origin, Content-Type, sizes, budgets, relay slots, field filtering, error mapping, headers), and the recorded
PHP differential: testdata/explorer-php-vectors.json holds 131 requests answered by the PHP relays themselves (php -S
on the snapshot at e8974fa, api/config.php generated from its example, in front of this gateway's /rpc on the same
fakes; recorded by testdata/record_explorer_php_vectors.py) next to the answers of these handlers. Each case either
matches PHP or carries the note that says why the gateway answers otherwise. Loopback only."""
import http.client
import importlib.util
import json
from pathlib import Path
import socket
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))
spec = importlib.util.spec_from_file_location('explorer_gateway_under_test', ROOT / 'gateway.py')
gateway = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gateway)
import explorer_api as api  # noqa: E402  (the module gateway.py imports)
import explorer_fakes as F  # noqa: E402

VECTORS = json.loads((ROOT / 'testdata' / 'explorer-php-vectors.json').read_text(encoding='utf-8'))
ORIGIN = VECTORS['origin']


def call(method='eth_blockNumber', params=None, ident=1):
    return {'jsonrpc': '2.0', 'id': ident, 'method': method, 'params': [] if params is None else params}


class ExplorerAllowListTests(unittest.TestCase):
    """api_security.php's lib.php checks, one for one, then the decoding edges PHP's json_decode refused."""

    def test_the_php_explorer_checks(self):
        checks = 0
        def check(name, value):
            nonlocal checks
            with self.subTest(name): self.assertTrue(value, name)
            checks += 1
        check('ordinary explorer read accepted', api.valid_call(call()))
        for method in ('eth_sendRawTransaction', 'eth_sendTransaction', 'personal_unlockAccount', 'admin_peers',
                       'debug_traceTransaction', 'pocol_submitShare'):
            check('privileged method refused: ' + method, not api.valid_call(call(method)))
        for method in ('pocol_getRemovable', 'pocol_finality'):
            check('parameterless read accepted: ' + method, api.valid_call(call(method)))
            check('parameters refused: ' + method, not api.valid_call(call(method, ['latest'])))
        check('members page accepted', api.valid_call(call('pocol_getMembers', [0, 256])) and api.valid_call(call('pocol_getMembers', [9999, 1])))
        for params in ([0, 257], [0, 0], [-1, 1], [10000, 1], [True, 1], [0, 1.0], ['0', 1], ['0x0', 1], [0], [0, 1, 2], [None, 1]):
            check('members page bounds refused: ' + json.dumps(params), not api.valid_call(call('pocol_getMembers', params)))
        big = api.decode_request(b'{"jsonrpc":"2.0","id":1,"method":"pocol_getMembers","params":[0,99999999999999999999]}')
        check('members page oversized integer refused', not api.valid_call(big))
        member = '0x' + 'ab' * 20
        check('member record accepted', api.valid_call(call('pocol_getMember', [member])))
        check('member record 0X prefix refused', not api.valid_call(call('pocol_getMember', [member.upper()])))
        check('member record mixed-case address accepted', api.valid_call(call('pocol_getMember', ['0x' + 'aB' * 20])))
        for params in ([], [member, 'latest'], ['0x11'], [member + '00'], [1], [{}]):
            check('member record shape refused: ' + json.dumps(params), not api.valid_call(call('pocol_getMember', params)))
        check('duplicate request IDs refused', not api.valid_payload([call(), call()]))
        check('empty batch refused', not api.valid_payload([]))
        check('batch above twenty refused', not api.valid_payload([call(ident=i) for i in range(1, 22)]))
        check('account shape refused', not api.valid_call(call('eth_getBalance', ['0x1', 'latest'])))
        check('state overrides refused', not api.valid_call(call('eth_call', [{}, 'latest', {}])))
        requests = [call(ident=1), call(ident=2)]
        responses = [{'jsonrpc': '2.0', 'id': 2, 'result': None}, {'jsonrpc': '2.0', 'id': 1, 'result': '0x1'}]
        check('response IDs may be reordered', api.valid_answers(responses, requests))
        check('duplicate response IDs refused', not api.valid_answers([responses[0], responses[0]], requests))
        check('foreign response ID refused', not api.valid_answers([{'jsonrpc': '2.0', 'id': 2, 'result': '0x1'}], [call()]))
        check('response cannot contain both result and error',
              not api.valid_answers([{'jsonrpc': '2.0', 'id': 1, 'result': None, 'error': {'code': 1, 'message': 'x'}}], [call()]))
        check('untyped error refused', not api.valid_answers([{'jsonrpc': '2.0', 'id': 1, 'error': {'code': '1', 'message': 'x'}}], [call()]))
        self.assertEqual(checks, 43, 'api_security.php had 43 explorer relay checks')

    def test_the_allow_list_is_lib_php_and_narrower_than_rpc(self):
        self.assertEqual(len(api.METHODS), 17)
        self.assertLess(api.METHODS, set(gateway.NO_PARAMS) | gateway.HASH_METHODS | gateway.ACCOUNT_METHODS | gateway.MEMBERS
                        | {'eth_getBlockByNumber', 'eth_getBlockByHash', 'pocol_getTransactions'})
        with patch.object(gateway, 'valid_options', lambda options: True):
            for method, params in (('pocol_getBlockReward', []), ('pocol_getNetworkGeography', []), ('pocol_getTransactions', [{}])):
                with self.subTest(method):
                    self.assertTrue(gateway.valid_call(call(method, params)), '/rpc keeps it')
                    self.assertFalse(api.valid_call(call(method, params)), '/api/rpc.php refuses it, as lib.php did')
        # Owner question Q5's default: /rpc keeps its own rules (it accepts duplicate ids); only /api/rpc.php refuses them.
        self.assertTrue(gateway.valid_payload([call(), call()]))
        for method in api.METHODS:
            self.assertFalse(api.valid_call({**call(method), 'jsonrpc': '1.0'}))

    def test_ids_and_tags(self):
        for ident in (True, False, 1.0, '1', None, 2 ** 53, -(2 ** 53)):
            with self.subTest(ident=ident): self.assertFalse(api.valid_call(call(ident=ident)))
        for ident in (0, -1, 2 ** 53 - 1, -(2 ** 53 - 1)):
            self.assertTrue(api.valid_call(call(ident=ident)))
        for tag in ('latest', '0x0', '0x' + 'f' * 16, '0xAbC'):
            self.assertTrue(api.valid_call(call('eth_getBlockByNumber', [tag, False])))
        for tag in ('pending', 'safe', '0x', '0x' + 'f' * 17, '1', 'latest\n', '0x1\n', 16, None):
            with self.subTest(tag=tag): self.assertFalse(api.valid_call(call('eth_getBlockByNumber', [tag, False])))
        self.assertFalse(api.valid_call(call('eth_getBlockByNumber', ['latest', 0])))
        self.assertFalse(api.valid_call(call('eth_getBlockByHash', ['0x' + 'a' * 63, True])))
        self.assertFalse(api.valid_call(call('eth_getTransactionByHash', ['0x' + 'a' * 64 + '\n'])))

    def test_decoding_follows_php_json_decode(self):
        decode = api.decode_request
        self.assertEqual(decode(('[' * 31 + '1' + ']' * 31).encode()), json.loads('[' * 31 + '1' + ']' * 31))
        for raw in (('[' * 32 + '1' + ']' * 32).encode(), ('[' * 16000).encode(), ('{"a":' * 40 + '1' + '}' * 40).encode(),
                    b'NaN', b'[Infinity]', b'[-Infinity]', b'"\\ud800"', b'{"\\ud800":1}', b'{"\\u0000a":1}',
                    b'\xef\xbb\xbf[1]', b'"\xff"', b'', b'[1,]', b'{"a" 1}'):
            with self.subTest(raw=raw[:20]), self.assertRaises((ValueError, RecursionError)):
                decode(raw)
        # A '[' inside a string is not nesting.
        self.assertEqual(decode(json.dumps({'x': '[' * 100}).encode()), {'x': '[' * 100})
        # JSON_BIGINT_AS_STRING: outside the 64-bit range an integer is a string; a 5000-digit one is no error.
        self.assertEqual(decode(b'[9223372036854775807,-9223372036854775808,9223372036854775808]'),
                         [2 ** 63 - 1, -(2 ** 63), '9223372036854775808'])
        self.assertEqual(decode(b'[' + b'9' * 5000 + b']'), ['9' * 5000])
        self.assertEqual(decode(b'{"a":1,"a":2}'), {'a': 2})

    def test_transaction_query_rules(self):
        options = api.transaction_options
        self.assertEqual(options(''), {'page': 1, 'size': 25, 'order': 'newest', 'q': ''})
        self.assertEqual(options('page=007&size=100&order=value_asc&q=0xABC&height=0&hash=0x' + 'AB' * 32 + '&lang=en'),
                         {'page': 7, 'size': 100, 'order': 'value_asc', 'q': '0xabc', 'snapshot': {'height': 0, 'hash': '0x' + 'ab' * 32}})
        self.assertEqual(options('page=1000000000')['page'], 10 ** 9)
        self.assertEqual(options('height=9007199254740991&hash=0x' + '1' * 64)['snapshot']['height'], 2 ** 53 - 1)
        for query in ('page=0', 'page=1000000001', 'page=%2B1', 'page=1&page=1', 'page[]=1', 'size=25&size=25', 'q=0X1',
                      'height=9007199254740992&hash=0x' + '1' * 64, 'height=1', 'hash=0x' + '1' * 64, 'q=' + 'a' * 5000,
                      'page=%EF%BC%91'):
            with self.subTest(query=query[:40]): self.assertIsNone(options(query))

    def test_geography_keeps_only_public_fields_in_their_order(self):
        network = gateway.Gateway(1, network=F.NET).network
        public = api.public_geography(F.geography(), network)
        self.assertEqual(list(public), list(api.GEOGRAPHY_KEYS) + ['database', 'nodes'])
        self.assertEqual(public['database'], {'provider': 'DB-IP Lite', 'release': '2026-11', 'updatedAt': 1699999999})
        self.assertEqual(public['nodes'][1], {'country': None, 'connections': 2, 'observer': False, 'miner': None,
                                              'id': 'ip-' + 'b' * 24})
        self.assertNotIn('observerAddress', json.dumps(public))
        self.assertNotIn('203.0.113.9', json.dumps(public))
        self.assertIsNone(api.public_geography(F.geography(chainId=True), network))
        self.assertIsNone(api.public_geography(F.geography(chainId=float(F.NET['chainId'])), network))

    def test_site_origin_values(self):
        for origin in ('https://explorer.pocol.net', 'https://explorer.pocol.net:8443', 'http://127.0.0.1:8661', 'http://localhost'):
            self.assertTrue(api.valid_origin(origin), origin)
        for origin in ('http://explorer.pocol.net', 'https://explorer.pocol.net/', 'https://Explorer.pocol.net', 'https://a:0',
                       'https://a:65536', 'ftp://a', '', None, 'https://explorer.pocol.net#x', 'https://user@explorer.pocol.net'):
            self.assertFalse(api.valid_origin(origin), origin)
        with self.assertRaises(ValueError):
            api.Relay(None, 'http://example.com')


class GatewayServerCase(unittest.TestCase):
    """A gateway Server on 127.0.0.1:0 over a fake node, GeoIP module and transaction index."""

    def setUp(self):
        self.node = F.FakeNode()
        self.geo, self.tx = F.FakeGeography(), F.FakeTransactions()
        self.gateway = gateway.Gateway(self.node.server_address[1], self.geo, self.tx, network=F.NET)
        self.server = gateway.Server(('127.0.0.1', 0), self.gateway, ORIGIN)
        self.port = self.server.server_address[1]
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.options_valid = patch.object(gateway, 'valid_options', lambda options: True)
        self.options_valid.start()

    def tearDown(self):
        self.options_valid.stop()
        self.server.shutdown(); self.server.server_close(); self.thread.join()
        self.node.stop()

    def scenario(self, scenario):
        F.apply_scenario(gateway, self.gateway, self.node, self.geo, self.tx, scenario)
        flag = scenario.get('txOptionsValid', True)
        self.options_valid.stop()
        self.options_valid = patch.object(gateway, 'valid_options', lambda options: flag)
        self.options_valid.start()

    def send(self, method, path, headers=(), body=None):
        return F.send(self.port, method, path, headers, body)

    def post(self, value, origin=None, path='/api/rpc.php', extra=()):
        status, headers, body = F.post_json(self.port, path, value, origin, extra)
        return status, dict(headers), json.loads(body) if body else None


class ExplorerVectorTests(GatewayServerCase):
    def test_every_recorded_php_case(self):
        self.assertEqual(len(VECTORS['cases']), 131)
        for case in VECTORS['cases']:
            with self.subTest(case['name']):
                self.scenario(case['scenario'])
                self.assertEqual(F.run_case(self.port, case['request']), case['python'])
                php = json.loads(json.dumps(case['php']))
                if isinstance(php['body'], dict):
                    for key in case.get('ignore', []): php['body'].pop(key, None)
                # A case without a note is answered as PHP answered it; a note documents every difference.
                self.assertEqual(php == case['python'], 'note' not in case, case.get('note'))

    def test_the_cases_cover_every_path_and_status(self):
        seen = {(c['request']['path'].split('?')[0], c['python']['status']) for c in VECTORS['cases']}
        for item in [('/api/rpc.php', s) for s in (200, 400, 403, 405, 413, 415, 429, 502, 503)] + \
                    [('/api/health.php', s) for s in (200, 405, 429, 503)] + \
                    [('/api/miner-locations.php', s) for s in (200, 405, 429, 503)] + \
                    [('/api/transactions.php', s) for s in (200, 400, 405, 409, 503)]:
            self.assertIn(item, seen)


class ExplorerRelayTests(GatewayServerCase):
    def test_every_path_refuses_other_methods_with_the_php_allow_header(self):
        for path, (methods, message) in api.ROUTES.items():
            for method in ('GET', 'HEAD', 'POST', 'PUT', 'DELETE', 'PATCH', 'OPTIONS', 'TRACE', 'PROPFIND'):
                if method in methods: continue
                with self.subTest(path=path, method=method):
                    status, headers, body = self.send(method, path, [('Content-Length', '0')], b'')
                    headers = dict(headers)
                    self.assertEqual((status, headers['Allow']), (405, ', '.join(methods)))
                    self.assertEqual(json.loads(body) if method != 'HEAD' else body, {'error': message} if method != 'HEAD' else b'')
                    self.assertNotIn('Access-Control-Allow-Origin', headers)

    def test_answers_carry_the_relay_and_site_headers(self):
        answers = [self.send('POST', '/api/rpc.php', [('Content-Type', 'application/json'), ('Content-Length', '2')], b'[]'),
                   self.send('GET', '/api/rpc.php'), self.send('GET', '/api/health.php'), self.send('GET', '/api/miner-locations.php'),
                   self.send('HEAD', '/api/miner-locations.php'), self.send('GET', '/api/transactions.php'),
                   self.send('GET', '/api/transactions.php?page=0'), self.send('GET', '/api/nothing.php')]
        answers.append(F.post_json(self.port, '/api/rpc.php', call()))
        for status, headers, _ in answers:
            headers = dict(headers)
            with self.subTest(status=status):
                self.assertEqual(headers['Content-Type'], 'application/json; charset=utf-8')
                self.assertEqual(headers['Cache-Control'], 'no-store')
                for name, value in api.SECURITY_HEADERS:
                    self.assertEqual(headers[name], value)
                self.assertFalse(any(k.lower().startswith('access-control-') for k in headers))
        self.assertEqual(dict(api.SECURITY_HEADERS)['Content-Security-Policy'],
                         "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline' https://fonts.googleapis.com; "
                         "font-src 'self' https://fonts.gstatic.com; img-src 'self' data:; connect-src 'self'; object-src 'none'; "
                         "base-uri 'self'; frame-ancestors 'none'; form-action 'self'")

    def test_origin(self):
        self.assertEqual(self.post(call())[0], 200, 'no Origin: a server-side caller (pocol.net network.php)')
        self.assertEqual(self.post(call(), ORIGIN)[0], 200)
        for origin in ('https://evil.example', 'null', ORIGIN + '/', ORIGIN.upper(), ''):
            with self.subTest(origin=origin):
                self.assertEqual(self.post(call(), origin)[:3:2], (403, {'error': 'Cross-origin requests are not enabled.'}))
        self.assertEqual(self.post(call(), ORIGIN, extra=[('Origin', ORIGIN)])[0], 403, 'two Origin headers')

    def test_content_type_and_size(self):
        body = json.dumps(call()).encode()
        for ctype in ('text/plain', 'application/json-seq', 'application/x-www-form-urlencoded', ''):
            status, _, raw = self.send('POST', '/api/rpc.php', [('Content-Type', ctype), ('Content-Length', str(len(body)))], body)
            self.assertEqual((status, json.loads(raw)), (415, {'error': 'Content-Type must be application/json.'}))
        status, _, _ = self.send('POST', '/api/rpc.php', [('Content-Type', 'application/json'), ('Content-Type', 'application/json'),
                                                          ('Content-Length', str(len(body)))], body)
        self.assertEqual(status, 415, 'two Content-Type headers')
        padded = body + b' ' * (16384 - len(body))
        self.assertEqual(self.post(padded)[0], 200)
        status, _, answer = self.post(padded + b' ')
        self.assertEqual((status, answer), (413, {'error': 'Request size limit exceeded.'}))
        # The size is refused from Content-Length alone: nothing above 16 KiB is read.
        status, _, raw = self.send('POST', '/api/rpc.php', [('Content-Type', 'application/json'), ('Content-Length', '999999999')], b'')
        self.assertEqual(status, 413)
        for headers in ([('Content-Length', '2'), ('Content-Length', '2')], [('Transfer-Encoding', 'chunked'), ('Content-Length', '2')],
                        [('Content-Length', '+2')], [('Content-Length', '0x2')], []):
            with self.subTest(headers=headers):
                status, _, raw = self.send('POST', '/api/rpc.php', [('Content-Type', 'application/json'), *headers], b'[]')
                self.assertEqual((status, json.loads(raw)), (400, {'error': api.REFUSED}))

    def test_deep_nesting_is_400_on_both_rpc_paths(self):
        for path in ('/api/rpc.php', '/rpc'):
            with self.subTest(path=path):
                status, _, answer = self.post(b'[' * 16000, path=path)
                self.assertEqual(status, 400, 'a RecursionError must not drop the connection')

    def test_batch_shape_order_and_costs(self):
        status, _, answer = self.post([call('pocol_getParams', ident=9), call(ident=-4), call('net_peerCount', ident=0)], ORIGIN)
        self.assertEqual((status, [a['id'] for a in answer]), (200, [9, -4, 0]))
        status, _, answer = self.post(call(ident=5))
        self.assertEqual((status, answer), (200, {'jsonrpc': '2.0', 'id': 5, 'result': '0x10'}))
        # The token cost is the gateway's (pocol_getRemovable 10, pocol_getMember 20, pocol_getMembers 5, others 1).
        with patch.object(gateway.time, 'monotonic', return_value=1000.0):  # no refill during the check
            self.gateway.tokens, self.gateway.token_time = 35.0, 1000.0
            self.assertEqual(self.post([call('pocol_getRemovable', ident=1), call('pocol_getMembers', [0, 1], 2),
                                        call('pocol_getMember', ['0x' + '1' * 40], 3)])[0], 200)
            self.assertEqual(self.gateway.tokens, 0.0)
            status, headers, answer = self.post(call())
            self.assertEqual((status, headers['Retry-After'], answer), (429, '3', {'error': api.BUSY}))
            self.assertEqual(self.gateway.tokens, 0.0, 'a refused request is not charged')

    def test_network_summary_batches_of_pocol_net(self):
        """pocol.net/api/network.php: no Origin, [status, params, miners] then 20 block reads; HTTP 200 exactly and every
        answer a result (network.php:49-66)."""
        status, _, first = self.post([call('pocol_status', ident=1), call('pocol_getParams', ident=2), call('pocol_getMiners', ident=3)])
        self.assertEqual(status, 200)
        self.assertTrue(all('result' in a and 'error' not in a for a in first))
        self.assertEqual(first[0]['result']['head'], {'height': 16, 'hash': F.HASH})
        status, _, blocks = self.post([call('eth_getBlockByNumber', ['0x%x' % (100 - i), False], 4 + i) for i in range(20)])
        self.assertEqual((status, [b['id'] for b in blocks]), (200, list(range(4, 24))))
        self.assertTrue(all(b['result']['hash'] == F.HASH for b in blocks))

    def test_relay_slots_are_shared_and_mapped_as_php(self):
        for _ in range(api.RELAY_SLOTS): self.assertTrue(self.server.explorer.slots.acquire(blocking=False))
        try:
            status, headers, answer = self.post(call())
            self.assertEqual((status, headers['Retry-After'], answer), (429, '3', {'error': api.BUSY}))
            status, headers, raw = self.send('GET', '/api/health.php')
            self.assertEqual((status, json.loads(raw)['result'], json.loads(raw)['gatewayReachable']), (429, 'NOT_CONNECTED', False))
            self.assertNotIn('Retry-After', dict(headers))
            self.assertEqual(self.send('GET', '/api/miner-locations.php')[0], 429)
            self.assertEqual(self.send('HEAD', '/api/miner-locations.php')[0], 503)
            status, _, raw = self.send('GET', '/api/transactions.php')
            self.assertEqual((status, json.loads(raw)), (503, {'error': api.TRANSACTIONS_UNAVAILABLE}))
            self.assertEqual(self.send('GET', '/api/transactions.php?page=0')[0], 400, 'the query is checked first')
            self.assertEqual(self.node.calls, [])
        finally:
            for _ in range(api.RELAY_SLOTS): self.server.explorer.slots.release()
        self.assertEqual(self.post(call())[0], 200)

    def test_connections_beyond_the_gateway_slots_get_the_json_busy_answer(self):
        """Behind Caddy every /api request is one connection to the gateway: one beyond its 16 request slots gets the
        PHP relays' JSON 429 (lib.php answered it before the gateway was reached), not a closed connection that Caddy
        would turn into an empty 502."""
        idle = []
        try:
            for _ in range(16): idle.append(socket.create_connection(('127.0.0.1', self.port), timeout=5))
            deadline = time.monotonic() + 5
            while self.server.slots.acquire(blocking=False):
                self.server.slots.release()
                self.assertLess(time.monotonic(), deadline, 'the 16 connections were not accepted')
                time.sleep(0.02)
            for method, path, body in (('GET', '/api/health.php', None), ('POST', '/api/rpc.php', json.dumps(call()).encode()),
                                       ('POST', '/rpc', json.dumps(call()).encode())):
                with self.subTest(path=path):
                    headers = [('Content-Type', 'application/json'), ('Content-Length', str(len(body)))] if body else []
                    status, answer_headers, raw = self.send(method, path, headers, body)
                    answer_headers = dict(answer_headers)
                    self.assertEqual((status, json.loads(raw)), (429, {'error': api.BUSY}))
                    self.assertEqual((answer_headers['Retry-After'], answer_headers['Content-Type'], answer_headers['Cache-Control']),
                                     ('3', 'application/json; charset=utf-8', 'no-store'))
            self.assertEqual(self.node.calls, [])
        finally:
            for sock in idle: sock.close()
        deadline = time.monotonic() + 5
        while self.send('GET', '/api/health.php')[0] != 200:
            self.assertLess(time.monotonic(), deadline, 'the slots were not released'); time.sleep(0.05)

    def test_forward_failures(self):
        with patch.object(api, 'FORWARD_SECONDS', -1):
            self.assertEqual(self.post([call(ident=1), call(ident=2)])[:3:2], (503, {'error': api.FAILED}))
        with patch.object(self.gateway, 'call', side_effect=TypeError('unexpected node answer')):
            self.assertEqual(self.post(call())[:3:2], (503, {'error': api.FAILED}))
        # lib.php decoded the gateway's reply at depth 64.
        nested = 'x'
        for _ in range(62): nested = [nested]
        self.scenario({'node': {'eth_blockNumber': {'result': nested}}})
        self.assertEqual(self.post(call())[0], 200)
        self.scenario({'node': {'eth_blockNumber': {'result': [nested]}}})
        self.assertEqual(self.post(call())[:3:2], (502, {'error': api.INVALID}))
        self.scenario({'node': {'eth_blockNumber': {'error': {'code': 2 ** 63, 'message': 'x'}}}})
        self.assertEqual(self.post(call())[0], 502)
        self.scenario({'node': {'eth_blockNumber': {'error': {'code': True, 'message': 'x'}}}})
        self.assertEqual(self.post(call())[0], 502)
        # A node answer with NaN, Infinity or an overflowing number (json.loads accepts them; 1e999 is inf) is not JSON:
        # lib.php's json_decode of it failed and pocol_forward answered 502, never a 200 that a strict parser refuses.
        for value in (float('nan'), float('inf'), float('-inf')):
            with self.subTest(value=value):
                self.scenario({'node': {'eth_gasPrice': {'result': value}}})
                self.assertEqual(self.post([call(ident=1), call('eth_gasPrice', ident=2)])[:3:2], (502, {'error': api.INVALID}))
        self.scenario({'node': {'eth_gasPrice': {'raw': b'{"jsonrpc":"2.0","id":1,"result":1e999}'}}})
        self.assertEqual(self.post(call('eth_gasPrice'))[:3:2], (502, {'error': api.INVALID}))

    def test_the_relay_time_limit_is_a_deadline(self):
        """lib.php's CURLOPT_TIMEOUT 9 bounded the whole request: the client had the 503 by then, before the bundle's
        and network.php's 12 s timeouts. Here every node read keeps to the same deadline (scaled down to 1 s)."""
        batch = [call('eth_getBlockByNumber', ['0x%x' % i, False], i) for i in range(1, 4)]
        with patch.object(api, 'FORWARD_SECONDS', 1.0):
            # Three reads of 0.45 s each: the third starts before the limit and would end after it.
            self.node.delay = 0.45
            start = time.monotonic()
            self.assertEqual(self.post(batch)[:3:2], (503, {'error': api.FAILED}))
            self.assertLess(time.monotonic() - start, 1.3)
            # One read slower than the whole limit (a stalled node), on a plain read and on each membership read
            # (pocol_getMember makes up to MEMBER_READS_MAX + DRAW_READS_MAX of them).
            self.node.delay = 3
            for request in (call('eth_gasPrice'), call('pocol_getMember', ['0x' + '1' * 40]), call('pocol_getMembers', [0, 4])):
                with self.subTest(method=request['method']):
                    self.scenario({}); self.node.delay = 3
                    start = time.monotonic()
                    self.assertEqual(self.post(request)[:3:2], (503, {'error': api.FAILED}))
                    self.assertLess(time.monotonic() - start, 1.8)
        self.scenario({})
        self.assertEqual(self.post(batch)[0], 200)

    def test_health(self):
        status, _, raw = self.send('GET', '/api/health.php')
        self.assertEqual((status, json.loads(raw)), (200, {
            'service': 'PoCol Explorer', 'gatewayReachable': True, 'networkMatch': True, 'result': 'PASS',
            'message': 'Expected PoCol network reached. Check head freshness in Explorer separately.'}))
        for result in ({'chainId': True, 'genesisHash': F.NET['genesisHash']}, {'chainId': F.NET['chainId'], 'genesisHash': 5},
                       {'chainId': F.NET['chainId']}, None, [F.NET]):
            with self.subTest(result=result):
                self.scenario({'node': {'pocol_getParams': {'result': result}}})
                status, _, raw = self.send('GET', '/api/health.php')
                self.assertEqual((status, json.loads(raw)['networkMatch']), (503, False))

    def test_geography(self):
        status, _, raw = self.send('GET', '/api/miner-locations.php')
        self.assertEqual(status, 200)
        self.assertEqual(json.loads(raw), {'data': api.public_geography(F.geography(), self.gateway.network)})
        status, headers, raw = self.send('HEAD', '/api/miner-locations.php')
        self.assertEqual((status, raw, dict(headers)['Content-Type']), (200, b'', 'application/json; charset=utf-8'))
        self.assertNotIn('Content-Length', dict(headers))
        self.scenario({'geography': {'changes': {'nodes': [{'id': 'ip-' + 'c' * 24, 'ip': '198.51.100.1'}]}}})
        self.assertEqual(json.loads(self.send('GET', '/api/miner-locations.php')[2])['data']['nodes'], [{'id': 'ip-' + 'c' * 24}])

    def test_transactions(self):
        status, _, raw = self.send('GET', '/api/transactions.php?page=3&size=50&order=value_desc&q=0xDEAD&height=12&hash=0x' + 'EE' * 32)
        self.assertEqual(status, 200)
        expected = {'page': 3, 'size': 50, 'order': 'value_desc', 'q': '0xdead', 'snapshot': {'height': 12, 'hash': '0x' + 'ee' * 32}}
        self.assertEqual(self.tx.asked, [expected])
        self.assertEqual(json.loads(raw), {'data': F.transactions_page(expected)})
        self.scenario({'transactions': 'moved'})
        status, _, raw = self.send('GET', '/api/transactions.php')
        self.assertEqual((status, json.loads(raw)), (409, {'error': 'The chain changed. Refreshing transaction history.', 'reset': True}))
        # The index's own option check (gateway.valid_options) still applies; without the module it refuses all: 503.
        self.scenario({'txOptionsValid': False})
        self.assertEqual(self.send('GET', '/api/transactions.php')[0], 503)
        if gateway.TransactionIndex is None:
            self.assertFalse(gateway.valid_options(expected))

    def test_other_paths_are_404_and_rpc_and_health_are_unchanged(self):
        for method, path in (('GET', '/api/'), ('GET', '/api/config.php'), ('POST', '/api/lib.php'), ('GET', '/api/rpc.php/x'),
                             ('GET', '/api//rpc.php'), ('GET', '/api/RPC.php')):
            with self.subTest(path=path):
                status, headers, raw = self.send(method, path, [('Content-Length', '0')], b'')
                self.assertEqual((status, json.loads(raw)), (404, {'error': 'Not found'}))
        self.assertEqual(self.send('GET', '/api/rpc.php?x=1')[0], 405, 'the query string is not part of the path')
        status, _, raw = self.send('GET', '/health')
        self.assertEqual((status, json.loads(raw)['service']), (200, 'pocol-explorer-gateway'))
        self.assertEqual(self.post(call(), path='/rpc')[0], 200)
        for method, path in (('HEAD', '/health'), ('PUT', '/rpc'), ('GET', '/rpc'), ('POST', '/health'), ('GET', '/')):
            with self.subTest(method=method, path=path):
                status, _, raw = self.send(method, path, [('Content-Length', '0')], b'')
                self.assertEqual((status, raw if method == 'HEAD' else json.loads(raw)), (404, b'' if method == 'HEAD' else {'error': 'Not found'}))


class ExplorerCliTests(unittest.TestCase):
    def test_site_origin_option(self):
        with tempfile.TemporaryDirectory(dir=ROOT) as directory:
            network = Path(directory) / 'network.json'
            network.write_text(json.dumps(F.NET))
            for origin in ('http://example.com', 'https://explorer.pocol.net/'):
                result = subprocess.run([sys.executable, '-B', str(ROOT / 'gateway.py'), '--v3-network', str(network),
                                         '--site-origin', origin, '--rpc-port', '1'], capture_output=True, text=True, timeout=60)
                self.assertEqual(result.returncode, 2, result.stderr)
                self.assertIn('--site-origin', result.stderr)


if __name__ == '__main__':
    unittest.main()
