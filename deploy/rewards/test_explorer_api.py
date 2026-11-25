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
import os
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
        # lib.php's 17 reads, and pocol_getMemberCounts (2026-11-22: pocol.net's summary reads the membership counts in
        # one call instead of paging pocol_getMembers), parameterless as /rpc has it.
        self.assertEqual(len(api.METHODS), 18)
        self.assertIn('pocol_getMemberCounts', api.METHODS)
        self.assertTrue(api.valid_call(call('pocol_getMemberCounts')))
        for params in (['latest'], [0], [0, 256]):
            self.assertFalse(api.valid_call(call('pocol_getMemberCounts', params)))
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
        """pocol.net/api/network.php: no Origin, [status, params, member counts] then 20 block reads; HTTP 200 exactly
        and every answer a result. The counts are the gateway's own membership read (an empty ActList here: lengths()
        answers four zero words), charged 20 tokens; the first request costs 22 and the blocks 20 (2026-11-22 review)."""
        self.node.answers['eth_call'] = {'result': '0x' + '00' * 128}
        with patch.object(gateway.time, 'monotonic', return_value=1000.0):  # no refill during the check
            self.gateway.tokens, self.gateway.token_time = 80.0, 1000.0
            status, _, first = self.post([call('pocol_status', ident=1), call('pocol_getParams', ident=2), call('pocol_getMemberCounts', ident=3)])
            self.assertEqual(self.gateway.tokens, 58.0)
        self.assertEqual(status, 200)
        self.assertTrue(all('result' in a and 'error' not in a for a in first), first)
        self.assertEqual(first[0]['result']['head'], {'height': 16, 'hash': F.HASH})
        self.assertEqual(first[2]['result'], {'height': 16, 'blockHash': F.HASH, 'active': 0, 'live': 0, 'eligible': 0, 'tomb': 0,
                                              'bondStates': {'NONE': 0, 'HELD': 0, 'BURNED': 0, 'PAID': 0, 'ACCRUING': 0}})
        # The whole check runs on a controlled clock: on the real one, a host that booted less than 1000 s ago (a fresh
        # CI runner) sees time go back to before 1000.0 and refills nothing. Sent at once, the block batch exceeds the
        # client's burst (22 + 20 > CLIENT_BURST 40) and is refused busy; network.php resends it after Retry-After.
        blocks_batch = [call('eth_getBlockByNumber', ['0x%x' % (100 - i), False], 4 + i) for i in range(20)]
        with patch.object(gateway.time, 'monotonic', return_value=1000.0):
            status, headers, busy = self.post(blocks_batch)
        self.assertEqual((status, headers.get('Retry-After')), (429, '3'), busy)
        with patch.object(gateway.time, 'monotonic', return_value=1003.0):
            status, _, blocks = self.post(blocks_batch)
        self.assertEqual((status, [b['id'] for b in blocks]), (200, list(range(4, 24))))
        self.assertTrue(all(b['result']['hash'] == F.HASH for b in blocks))

    def test_a_spent_budget_is_each_paths_busy_answer(self):
        """As documented (README, Explorer reads): Retry-After only on /api/rpc.php, as rpc.php sent it."""
        with patch.object(gateway.time, 'monotonic', return_value=1000.0):  # no refill during the check
            self.gateway.tokens, self.gateway.token_time = 0.0, 1000.0
            status, headers, answer = self.post(call())
            self.assertEqual((status, headers.get('Retry-After'), answer), (429, '3', {'error': api.BUSY}))
            for path, expected in (('/api/health.php', 429), ('/api/miner-locations.php', 429), ('/api/transactions.php', 503)):
                with self.subTest(path=path):
                    status, headers, _ = self.send('GET', path)
                    self.assertEqual((status, dict(headers).get('Retry-After')), (expected, None))
        self.assertEqual(self.node.calls, [])

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


import hashlib  # noqa: E402
import hmac  # noqa: E402
import pocol_geoip as G  # noqa: E402
import pocol_holders as H  # noqa: E402
import holders_fakes as HF  # noqa: E402

SAMPLE_GEOIP = ROOT / 'testdata' / 'geoip-sample.csv'
REPORTER_KEY = 'b1' * 32
CREDENTIALS = json.dumps({'salt': 'aa' * 32, 'reporters': {'founder-1': REPORTER_KEY, 'founder-2': 'b2' * 32}}).encode()


def holder_get(case, path, headers=()):
    status, answer_headers, raw = case.send('GET', path, headers)
    return status, dict(answer_headers), json.loads(raw) if raw else None


def report(miner=F.MINER, at=None, reporter='founder-1', network=F.NET, peers=(), height=16, **changes):
    value = {'schema': 'pocol-geo-report-v1', 'reporter': reporter, 'chainId': network['chainId'],
             'genesisHash': network['genesisHash'], 'at': int(time.time()) if at is None else at,
             'head': {'height': height, 'hash': '0x' + 'cd' * 32}, 'miner': miner, 'peers': list(peers), 'unlocated': 0}
    value.update(changes)
    return value


def signed(value, reporter='founder-1', key=REPORTER_KEY, raw=None):
    body = raw if raw is not None else json.dumps(value, separators=(',', ':')).encode()
    signature = hmac.new(bytes.fromhex(key), body, hashlib.sha256).hexdigest()
    return [('Content-Type', 'application/json'), ('Content-Length', str(len(body))), ('X-PoCol-Reporter', reporter),
            ('X-PoCol-Signature', signature)], body


class HolderRulesTests(unittest.TestCase):
    """The new routes' query rules, answer checks and reachability, without a server."""

    def test_holder_query_rules(self):
        options = api.holder_options
        self.assertEqual(options(''), {'page': 1, 'size': 25, 'order': 'balance_desc', 'type': 'all', 'q': ''})
        self.assertEqual(options('page=007&size=100&order=balance_asc&type=miner&q=0xABC&height=0&hash=0x' + 'AB' * 32 + '&lang=en'),
                         {'page': 7, 'size': 100, 'order': 'balance_asc', 'type': 'miner', 'q': '0xabc',
                          'snapshot': {'height': 0, 'hash': '0x' + 'ab' * 32}})
        self.assertEqual(options('page=1000000000&q=0x' + 'f' * 40)['page'], 10 ** 9)
        self.assertEqual(options('height=9007199254740991&hash=0x' + '1' * 64)['snapshot']['height'], 2 ** 53 - 1)
        bad = ('page=0', 'page=1000000001', 'page=%2B1', 'page=1&page=1', 'page[]=1', 'type=all&type=all', 'size=10',
               'order=newest', 'type=holder', 'type=ALL', 'q=0X1', 'q=abc', 'q=0x' + 'f' * 41, 'q=0x',
               'height=9007199254740992&hash=0x' + '1' * 64, 'height=1', 'hash=0x' + '1' * 64, 'height=x&hash=0x' + '1' * 64,
               'x=' + 'a' * 5000, 'page=%EF%BC%91')
        for query in bad:
            with self.subTest(query=query[:40]):
                self.assertIsNone(options(query))
                self.assertIsNone(H.holder_options(query))
        corpus = ['', 'page=3&size=50', 'q=0xAbC&type=reward', 'order=balance_asc&page=9', 'height=7&hash=0x' + 'Cd' * 32,
                  'size=100&type=system&x=1&y=2&q=0x0'] + list(bad)
        for query in corpus:
            self.assertEqual(options(query), H.holder_options(query), query[:40])
        # Everything the parser builds the holders index takes (gateway.valid_holder_options is the index's own check).
        self.assertIs(gateway.valid_holder_options, H.valid_options)
        for query in corpus[:6]:
            self.assertTrue(H.valid_options(options(query)), query)
        self.assertTrue(api.plain_query('') and api.plain_query('_=1&x=2') and api.plain_query('x' * 4096))
        self.assertFalse(api.plain_query('x' * 4097))

    def test_the_holder_reads_are_reachable_only_from_their_routes(self):
        for method in sorted(gateway.HOLDER_METHODS):
            params = [api.holder_options('')] if method == 'pocol_getHolders' else []
            with self.subTest(method):
                self.assertNotIn(method, api.METHODS)
                self.assertFalse(api.valid_call(call(method, params)), '/api/rpc.php refuses it')
                self.assertFalse(gateway.valid_call(call(method, params)), '/rpc refuses it')
                self.assertIn(method, gateway.COSTS)
        self.assertEqual(len(api.METHODS), 18)
        self.assertEqual((gateway.RPC_SLOTS, gateway.TX_SLOTS, gateway.HOLDER_SLOTS), (4, 2, 2))
        self.assertEqual({m: gateway.COSTS[m] for m in gateway.HOLDER_METHODS},
                         {'pocol_getHolders': 2, 'pocol_getHolderStats': 1, 'pocol_getMinerGeography': 5})

    def test_answer_checks(self):
        network = gateway.Gateway(1, network=F.NET).network
        options = api.holder_options('page=2&size=25&type=wallet')
        self.assertIsNotNone(api.holders_page(F.holders_page(options), network, options))
        pinned = api.holder_options('height=16&hash=0x' + 'ab' * 32)
        self.assertIsNotNone(api.holders_page(F.holders_page(pinned), network, pinned))
        bad_pages = [F.holders_page(options, chainId=True), F.holders_page(options, chainId=F.NET['chainId'] + 1),
                     F.holders_page(options, genesisHash='0x' + '00' * 32), F.holders_page(options, schema='pocol-holders-v2'),
                     F.holders_page(options, page=3), F.holders_page(options, size=True), F.holders_page(options, type='all'),
                     F.holders_page(options, rows=[F.holder_row()] * 26), F.holders_page(options, ip='203.0.113.9'),
                     F.holders_page(options, rows=[F.holder_row(ip='203.0.113.9')]),
                     F.holders_page(options, rows=[F.holder_row(type='miner')]),
                     F.holders_page(options, rows=[F.holder_row(totalWei='1.5')]),
                     F.holders_page(options, rows=[F.holder_row(totalWei='-1')]),
                     F.holders_page(options, rows=[F.holder_row(address='0x' + 'AB' * 20)]),
                     F.holders_page(options, rows=[F.holder_row(label='Other')]),
                     F.holders_page(options, rows=[F.holder_row(minerIds=['0x1'])]),
                     F.holders_page(options, supply={'currentSupplyWei': 1}),
                     F.holders_page(options, verification={'complete': 1}),
                     F.holders_page(options, index={'state': 'ready'}),
                     F.holders_page(options, total=-1), None, [], 'x']
        for value in bad_pages:
            with self.subTest(value=json.dumps(value)[:120]):
                self.assertIsNone(api.holders_page(value, network, options))
        moved = F.holders_page(pinned, snapshot={'height': 16, 'hash': '0x' + 'cd' * 32, 'timestamp': 1})
        self.assertIsNone(api.holders_page(moved, network, pinned), 'a page of another snapshot than the pinned one')
        self.assertIsNotNone(api.holders_stats(F.holders_stats(), network))
        stats = F.holders_stats()
        del stats['rewards']
        self.assertIsNotNone(api.holders_stats(stats, network), 'the contract has no rewards range')
        unknown = F.holders_stats(verification=dict(F.holders_stats()['verification'], accountsOnChain=None, accountsMissing=None,
                                                    unlocatedWei='-5'))
        self.assertIsNotNone(api.holders_stats(unknown, network), 'the account count is null when not read at the snapshot')
        for change in ({'extra': 1}, {'schema': 'pocol-holders-v1'}, {'supply': {}}, {'distribution': []},
                       {'history': {'from': None, 'points': [{}]}}, {'holders': {'count': 1}},
                       {'concentration': {'top1': {'totalWei': '1', 'sharePercentE10': '0'}}}, {'rewards': {'from': 'x', 'to': None}}):
            with self.subTest(change=change):
                self.assertIsNone(api.holders_stats(F.holders_stats(**change), network))

    def test_the_map_answer_check_keeps_addresses_and_ids_out(self):
        network = gateway.Gateway(1, network=F.NET).network
        geography = G.Geography(None, None, F.NET, gateway.HolderMapInputs(F.FakeHolders()), None)
        good = geography.answer()
        self.assertIsNotNone(api.miner_geography(good, network))
        def changed(**kw):
            value = json.loads(json.dumps(good)); value.update(kw); return value
        country = {'code': 'FR', 'miners': {'located': 1, 'active': 1, 'inactive': 0, 'connected': 0}, 'nodes': 1, 'blocks': 1,
                   'blocksSharePercentE10': '1', 'rewardsWei': None}
        self.assertIsNotNone(api.miner_geography(changed(countries=[country]), network))
        for value in (changed(observerAddress='203.0.113.9'), changed(nodes={**good['nodes'], 'ids': ['a' * 24]}),
                      changed(sources=[{'kind': 'observer', 'name': '203.0.113.9', 'fresh': True, 'reportedAt': 1}]),
                      changed(sources=[{'kind': 'peer', 'name': 'x', 'fresh': True, 'reportedAt': 1}]),
                      changed(sources=[{'kind': 'operator', 'name': 'founder-1', 'fresh': True, 'reportedAt': 1, 'ip': '5.0.0.1'}]),
                      changed(countries=[dict(country, miner=F.MINER)]), changed(countries=[dict(country, code='fr')]),
                      changed(countries=[dict(country, miners={**country['miners'], 'ids': [F.MINER]})]),
                      changed(database={'provider': 'DB-IP Lite', 'release': '2026-11', 'license': 'CC BY 4.0', 'path': '/x'}),
                      changed(blockHash='0x' + 'AB' * 32), changed(schema='pocol-network-geography-v1'), changed(chainId=True)):
            with self.subTest(value=json.dumps(value)[:100]):
                self.assertIsNone(api.miner_geography(value, network))

    def test_holder_map_inputs_adapt_the_index_and_cache_per_snapshot(self):
        holders = F.FakeHolders()
        inputs = gateway.HolderMapInputs(holders)
        registry = inputs.registry()
        self.assertEqual([(r['tomb'], r['graceStart']) for r in registry['records']], [(0, 0), (0, 0)])
        self.assertEqual(inputs.winners(G.BLOCKS_WINDOW), {'from': 1, 'to': 16, 'blocks': 16, 'miners': {F.MINER: 10}})
        self.assertEqual(inputs.miner_rewards(), {F.MINER: 5 * 10 ** 18 + 7}, 'paid plus bond accrued')
        self.assertEqual(holders.reads, ['registry', 'winners', 'rewards'], 'one read of each per snapshot')
        holders.stats_answer = lambda: F.holders_stats(snapshot={'height': 17, 'hash': '0x' + 'ee' * 32, 'timestamp': 1})
        holders.map['registry'] = F.map_registry(17, '0x' + 'ee' * 32)
        self.assertEqual(inputs.registry()['height'], 17)
        self.assertEqual(holders.reads, ['registry', 'winners', 'rewards'] * 2, 'a new snapshot is read again')
        holders.map['rewards'] = None
        holders.stats_answer = lambda: F.holders_stats(snapshot={'height': 18, 'hash': '0x' + 'ef' * 32, 'timestamp': 1})
        self.assertIsNone(inputs.miner_rewards(), 'rewards not exact: null')
        holders.stats_answer = H.Unavailable
        self.assertEqual((inputs.registry(), inputs.winners(G.BLOCKS_WINDOW), inputs.miner_rewards()), (None, None, None))
        with self.assertRaises(ValueError):
            inputs.winners(100)
        holders.stats_answer = F.holders_stats
        holders.map['registry']['records'][0]['tomb'] = 0
        inputs.key = None
        with self.assertRaises(ValueError):
            inputs.registry()


class HolderRouteTests(GatewayServerCase):
    """/api/holders.php, /api/holders-stats.php and /api/miner-geography.php over HTTP against a fake holders index and
    the real miner map (pocol_geoip.Geography, no GeoIP database, no reporters)."""

    def setUp(self):
        super().setUp()
        self.holders = F.FakeHolders()
        self.gateway.holders = self.holders
        self.gateway.miner_map = G.Geography(None, None, F.NET, gateway.HolderMapInputs(self.holders), self.gateway.observer_status)
        self.holder_valid = patch.object(gateway, 'valid_holder_options', H.valid_options)
        self.holder_valid.start()
        self.addCleanup(self.holder_valid.stop)

    def test_a_holders_page(self):
        target = '/api/holders.php?page=2&size=50&order=balance_asc&type=wallet&q=0xAB&height=12&hash=0x' + 'EE' * 32
        status, headers, answer = holder_get(self, target)
        expected = {'page': 2, 'size': 50, 'order': 'balance_asc', 'type': 'wallet', 'q': '0xab',
                    'snapshot': {'height': 12, 'hash': '0x' + 'ee' * 32}}
        self.assertEqual((status, self.holders.asked), (200, [expected]))
        self.assertEqual(answer, {'data': F.holders_page(expected)})
        self.assertEqual((headers['Content-Type'], headers['Cache-Control']), (api.JSON_TYPE, 'no-store'))
        for name, value in api.SECURITY_HEADERS:
            self.assertEqual(headers[name], value)
        status, headers, raw = self.send('HEAD', '/api/holders.php')
        self.assertEqual((status, raw, dict(headers)['Content-Type']), (200, b'', api.JSON_TYPE))
        self.assertNotIn('Content-Length', dict(headers))
        self.assertEqual(self.node.calls, [], 'a page reads no node')

    def test_bad_queries_are_400_before_any_read(self):
        for query in ('page=0', 'size=10', 'page=1&page=2', 'type[]=all', 'type=x', 'q=0X1', 'order=newest', 'height=1',
                      'x=' + 'a' * 5000):
            with self.subTest(query=query[:30]):
                status, _, answer = holder_get(self, '/api/holders.php?' + query)
                self.assertEqual((status, answer), (400, {'error': 'Invalid holder query.'}))
        for path in ('/api/holders-stats.php', '/api/miner-geography.php'):
            self.assertEqual(holder_get(self, path + '?x=' + 'a' * 5000)[0], 400)
            self.assertEqual(holder_get(self, path + '?_=12345')[0], 200, 'another key is ignored')
        self.assertEqual(self.holders.asked, [])

    def test_a_changed_snapshot_is_409_reset(self):
        self.holders.page_answer = H.SnapshotChanged
        status, headers, answer = holder_get(self, '/api/holders.php?height=12&hash=0x' + 'ee' * 32)
        self.assertEqual((status, answer), (409, {'error': 'The holder snapshot changed. Refreshing.', 'reset': True}))
        self.assertEqual(api.HOLDERS_RESET, 'The holder snapshot changed. Refreshing.')

    def test_503_without_data(self):
        unavailable = {'error': api.HOLDERS_UNAVAILABLE}
        self.holders.page_answer = H.Unavailable
        self.holders.stats_answer = H.Unavailable
        self.assertEqual(holder_get(self, '/api/holders.php')[::2], (503, unavailable))
        self.assertEqual(holder_get(self, '/api/holders-stats.php')[::2], (503, unavailable))
        self.assertEqual(holder_get(self, '/api/miner-geography.php')[::2], (503, {'error': api.MAP_UNAVAILABLE}))
        self.assertEqual(self.send('HEAD', '/api/holders.php')[0], 503)
        # Answers of another shape or network are never relayed.
        for answer in (lambda o: F.holders_page(o, chainId=1), lambda o: F.holders_page(o, rows=[F.holder_row(ip='5.0.0.1')]),
                       lambda o: F.holders_page(o, page=o['page'] + 1), lambda o: {'error': 'x'}):
            self.scenario({}); self.holders.page_answer = answer
            self.assertEqual(holder_get(self, '/api/holders.php')[::2], (503, unavailable))
        self.holders.stats_answer = lambda: F.holders_stats(genesisHash='0x' + '00' * 32)
        self.assertEqual(holder_get(self, '/api/holders-stats.php')[::2], (503, unavailable))
        # Without the index (or its module: the option check refuses everything) and without the miner map.
        self.scenario({}); self.gateway.holders = None
        self.assertEqual(holder_get(self, '/api/holders.php')[::2], (503, unavailable))
        self.assertEqual(holder_get(self, '/api/holders-stats.php')[::2], (503, unavailable))
        with patch.object(gateway, 'valid_holder_options', lambda options: False):
            self.gateway.holders = self.holders; self.holders.asked.clear()
            self.assertEqual(holder_get(self, '/api/holders.php')[::2], (503, unavailable))
            self.assertEqual(self.holders.asked, [])
        self.scenario({}); self.gateway.miner_map = None
        self.assertEqual(holder_get(self, '/api/miner-geography.php')[::2], (503, {'error': api.MAP_UNAVAILABLE}))

    def test_budget_slots_and_costs(self):
        with patch.object(gateway.time, 'monotonic', return_value=1000.0):  # no refill during the check
            for path, cost in (('/api/holders.php', 2), ('/api/holders-stats.php', 1), ('/api/miner-geography.php', 5)):
                with self.subTest(path=path):
                    self.gateway.tokens, self.gateway.token_time = 50.0, 1000.0
                    self.gateway.clients.clear(); self.gateway.cache.clear()
                    self.assertEqual(holder_get(self, path)[0], 200)
                    self.assertEqual(self.gateway.tokens, 50.0 - cost)
            self.gateway.tokens = 0.0
            for path in ('/api/holders.php', '/api/holders-stats.php', '/api/miner-geography.php'):
                status, headers, answer = holder_get(self, path)
                self.assertEqual((status, headers.get('Retry-After'), answer), (429, '3', {'error': api.BUSY}))
            self.assertEqual(self.gateway.tokens, 0.0, 'a refused request is not charged')
        # The gateway's holder slots busy: 429 with Retry-After, the node reads unaffected.
        for _ in range(gateway.HOLDER_SLOTS): self.assertTrue(self.gateway.holder_slots.acquire(blocking=False))
        try:
            self.gateway.cache.clear()
            for path in ('/api/holders.php', '/api/holders-stats.php', '/api/miner-geography.php'):
                status, headers, _ = holder_get(self, path)
                self.assertEqual((status, headers.get('Retry-After')), (429, '3'))
            self.assertEqual(self.post(call())[0], 200)
        finally:
            for _ in range(gateway.HOLDER_SLOTS): self.gateway.holder_slots.release()
        # The relay's slots busy: 429 as on every /api read.
        for _ in range(api.RELAY_SLOTS): self.assertTrue(self.server.explorer.slots.acquire(blocking=False))
        try:
            self.assertEqual(holder_get(self, '/api/holders.php')[0], 429)
        finally:
            for _ in range(api.RELAY_SLOTS): self.server.explorer.slots.release()

    def test_the_miner_map_without_reporters_or_database(self):
        self.node.answers['pocol_status'] = {'result': {'chainId': F.NET['chainId'], 'genesisHash': F.NET['genesisHash'],
                                                        'head': {'height': 16, 'hash': F.HASH},
                                                        'peers': ['9.9.9.9:30300', '10.0.0.1:30300', '[2a00:1450::1]:30300']}}
        status, headers, answer = holder_get(self, '/api/miner-geography.php')
        self.assertEqual(status, 200)
        data = answer['data']
        self.assertEqual((data['schema'], data['height'], data['blockHash'], data['database']),
                         ('pocol-miners-geography-v2', 16, F.HASH, None))
        self.assertEqual(data['miners'], {'registered': 2, 'active': 1, 'inactive': 0, 'exited': 1, 'punished': 0,
                                          'located': 0, 'unknown': 2, 'connected': 0, 'thetaInact': 360})
        self.assertEqual(data['nodes'], {'observed': 3, 'located': 0, 'unlocated': 3})
        self.assertEqual(data['sources'][0]['kind'], 'observer')
        self.assertEqual((data['countries'], data['unknown']), ([], {'miners': 2, 'blocks': 16}))
        text = json.dumps(answer)
        for secret in (F.MINER, F.ADDRESS, '9.9.9.9', '2a00:1450', G.keyed_id(self.gateway.miner_map.salt, '9.9.9.9')):
            self.assertNotIn(secret, text)
        self.assertEqual(self.send('HEAD', '/api/miner-geography.php')[0], 200)
        # The observer's node is read at most once per OBSERVE_SECONDS whatever the request rate (cache cleared).
        reads = sum(1 for m, _ in self.node.calls if m == 'pocol_status')
        for _ in range(3):
            self.gateway.cache.clear(); self.assertEqual(holder_get(self, '/api/miner-geography.php')[0], 200)
        self.assertEqual(sum(1 for m, _ in self.node.calls if m == 'pocol_status'), reads)
        self.assertEqual(self.holders.reads.count('registry'), 1, 'the registry is read once per snapshot')


class GeoReportTests(GatewayServerCase):
    """POST /api/geo-report.php through the real Server and the real miner map, with a GeoIP database built from the
    synthetic sample (testdata/geoip-sample.csv) and the geo-reporters credential."""

    def setUp(self):
        super().setUp()
        folder = tempfile.TemporaryDirectory()
        self.addCleanup(folder.cleanup)
        G.build(SAMPLE_GEOIP, '2026-11', Path(folder.name) / 'geo.bin')
        self.database = G.GeoDatabase(Path(folder.name) / 'geo.bin')
        self.addCleanup(self.database.close)   # before the folder goes (Windows cannot delete a mapped file)
        self.holders = F.FakeHolders()
        self.gateway.holders = self.holders
        self.credentials = G.parse_credentials(CREDENTIALS)
        self.gateway.miner_map = G.Geography(self.database, None, F.NET, gateway.HolderMapInputs(self.holders),
                                             self.gateway.observer_status, credentials=self.credentials)

    def post_report(self, value=None, headers=None, body=None, source='5.0.2.7', extra=()):
        if headers is None:
            headers, body = signed(report() if value is None else value)
        status, answer_headers, raw = self.send('POST', '/api/geo-report.php',
                                                [*headers, *extra] + ([('X-Forwarded-For', source)] if source else []), body)
        return status, dict(answer_headers), raw

    def test_a_signed_report_places_its_miner_in_the_country_of_its_source(self):
        status, headers, raw = self.post_report(source='5.0.2.7')
        self.assertEqual((status, raw), (204, b''))
        self.assertNotIn('Content-Length', headers)
        self.assertNotIn('Content-Type', headers)
        self.assertEqual(headers['Cache-Control'], 'no-store')
        status, _, answer = holder_get(self, '/api/miner-geography.php')
        data = answer['data']
        self.assertEqual(status, 200)
        self.assertEqual(data['database'], {'provider': 'DB-IP Lite', 'release': '2026-11', 'license': 'CC BY 4.0'})
        self.assertEqual(data['countries'], [{'code': 'FR', 'miners': {'located': 1, 'active': 1, 'inactive': 0, 'connected': 1},
                                              'nodes': 1, 'blocks': 10, 'blocksSharePercentE10': str(10 * 10 ** 12 // 16),
                                              'rewardsWei': str(5 * 10 ** 18 + 7)}])
        self.assertEqual((data['miners']['located'], data['miners']['unknown'], data['unknown']), (1, 1, {'miners': 1, 'blocks': 6}))
        operator = [s for s in data['sources'] if s['kind'] == 'operator']
        self.assertEqual([(s['name'], s['fresh']) for s in operator], [('founder-1', True), ('founder-2', False)])
        # Neither the source address nor its keyed id is in the answer or kept by the gateway.
        text = json.dumps(answer)
        for secret in ('5.0.2.7', G.keyed_id(self.credentials.salt, '5.0.2.7'), F.MINER):
            self.assertNotIn(secret, text)
        self.assertNotIn('5.0.2.7', repr(self.gateway.miner_map.reports))

    def test_an_ipv6_source_is_looked_up_by_its_full_address(self):
        self.assertEqual(self.post_report(source='2a00:1450::5')[0], 204)
        self.gateway.cache.clear()
        self.assertEqual([c['code'] for c in holder_get(self, '/api/miner-geography.php')[2]['data']['countries']], ['IE'])

    def test_refusals(self):
        headers, body = signed(report())
        def without(name): return [h for h in headers if h[0] != name]
        cases = [
            ('Origin', 400, headers, body, [('Origin', ORIGIN)]),
            ('two Content-Type', 415, headers, body, [('Content-Type', 'application/json')]),
            ('text/plain', 415, [('Content-Type', 'text/plain')] + without('Content-Type'), body, []),
            ('no Content-Length', 400, without('Content-Length'), body, []),
            ('two Content-Length', 400, headers, body, [('Content-Length', str(len(body)))]),
            ('Transfer-Encoding', 400, headers, body, [('Transfer-Encoding', 'chunked')]),
            ('Content-Length +1', 400, [('Content-Length', '+' + str(len(body)))] + without('Content-Length'), body, []),
            ('oversized', 413, [('Content-Length', '16385')] + without('Content-Length'), b'', []),
            ('bad signature', 401, signed(report(), key='c3' * 32)[0], body, []),
            ('unknown reporter', 401, *signed(report(reporter='founder-9'), reporter='founder-9'), []),
            ('no reporter header', 400, without('X-PoCol-Reporter'), body, []),
            ('another network', 400, *signed(report(network={'chainId': 1, 'genesisHash': F.NET['genesisHash']})), []),
            ('stale', 409, *signed(report(at=int(time.time()) - 600)), []),
        ]
        for name, expected, case_headers, case_body, extra in cases:
            with self.subTest(name):
                status, answer_headers, raw = self.post_report(headers=case_headers, body=case_body, extra=extra)
                self.assertEqual((status, json.loads(raw)), (expected, {'error': api.REPORT_ERRORS[expected]}))
                self.assertEqual(answer_headers['Content-Type'], api.JSON_TYPE)
        self.assertEqual(self.post_report(headers=headers, body=body)[0], 204)
        self.assertEqual(self.post_report(headers=headers, body=body)[0], 409, 'a replayed report')
        self.assertEqual(self.node.calls, [], 'a report reads no node')

    def test_503_without_the_credential_or_the_miner_map_and_429_when_the_budget_is_spent(self):
        self.gateway.miner_map = G.Geography(self.database, None, F.NET, gateway.HolderMapInputs(self.holders), None)
        status, _, raw = self.post_report()
        self.assertEqual((status, json.loads(raw)), (503, {'error': 'Operator reports are not enabled.'}))
        self.gateway.miner_map = None
        self.assertEqual(self.post_report()[0], 503)
        self.gateway.miner_map = G.Geography(self.database, None, F.NET, gateway.HolderMapInputs(self.holders), None,
                                             credentials=self.credentials)
        with patch.object(gateway.time, 'monotonic', return_value=1000.0):
            self.gateway.tokens, self.gateway.token_time = 0.0, 1000.0
            status, headers, raw = self.post_report()
            self.assertEqual((status, headers.get('Retry-After'), json.loads(raw)), (429, '3', {'error': api.BUSY}))
            self.gateway.tokens = 5.0
            self.assertEqual(self.post_report()[0], 204)
            self.assertEqual(self.gateway.tokens, 4.0, 'a report costs one token')
        # The relay's slots do not hold reports back.
        for _ in range(api.RELAY_SLOTS): self.assertTrue(self.server.explorer.slots.acquire(blocking=False))
        try:
            self.assertEqual(self.post_report(report(at=int(time.time()) + 1))[0], 204)
        finally:
            for _ in range(api.RELAY_SLOTS): self.server.explorer.slots.release()

    def test_the_route_allows_post_only(self):
        for method in ('GET', 'HEAD', 'PUT'):
            status, headers, _ = self.send(method, '/api/geo-report.php', [('Content-Length', '0')], b'')
            self.assertEqual((status, dict(headers)['Allow']), (405, 'POST'))
        for path in ('/api/holders.php', '/api/holders-stats.php', '/api/miner-geography.php'):
            status, headers, raw = self.send('POST', path, [('Content-Length', '0')], b'')
            self.assertEqual((status, dict(headers)['Allow']), (405, 'GET, HEAD'))


class HolderIndexServerTests(unittest.TestCase):
    """The real holders index (pocol_holders.HolderIndex over holders_fakes' in-process v3 node) behind the real
    Server: pages, statistics, the miner map's registry and a reorganization's 409 reset. Loopback only; the index's
    SQLite file is closed before its folder is removed (Windows)."""

    def setUp(self):
        folder = tempfile.TemporaryDirectory()
        self.addCleanup(folder.cleanup)
        self.chain = HF.FakeHolderChain()
        self.chain.mine(txs=[{'from': HF.FOUNDERS[1][1], 'to': '0x' + f'a{i:039x}', 'value': (i + 1) * 10 ** 18} for i in range(30)])
        self.index = H.HolderIndex(Path(folder.name) / 'holders.sqlite3', self.chain.query, self.chain.network,
                                   rewards=self.chain.rewards, pace=0, slowdown=0)
        self.addCleanup(self.index.close)
        self.settle()
        self.node = F.FakeNode()
        self.addCleanup(self.node.stop)
        self.gateway = gateway.Gateway(self.node.server_address[1], network=self.chain.network, holders=self.index)
        self.gateway.miner_map = G.Geography(None, None, self.chain.network, gateway.HolderMapInputs(self.index), None)
        self.server = gateway.Server(('127.0.0.1', 0), self.gateway, ORIGIN)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()
        self.addCleanup(lambda: (self.server.shutdown(), self.server.server_close(), self.thread.join()))

    def settle(self):
        for _ in range(4):
            self.index.step()
            heights = self.index.status()['heights']
            if heights['snapshot'] == heights['history']:
                return
        raise AssertionError('no snapshot at the tip')

    def get(self, path):
        self.gateway.cache.clear()
        status, _, raw = F.send(self.server.server_address[1], 'GET', path)
        return status, json.loads(raw)

    def test_pages_statistics_map_and_reset(self):
        status, first = self.get('/api/holders.php?size=25')
        self.assertEqual(status, 200, first)
        page = first['data']
        self.assertEqual((page['total'], len(page['rows']), page['rows'][0]['rank']), (38, 25, 1))
        status, second = self.get('/api/holders.php?size=25&page=2')
        self.assertEqual([r['rank'] for r in second['data']['rows']], list(range(26, 39)))
        status, wallets = self.get('/api/holders.php?type=wallet&size=100&order=balance_asc')
        self.assertEqual((len(wallets['data']['rows']), wallets['data']['rows'][0]['type']), (30, 'wallet'))
        status, stats = self.get('/api/holders-stats.php')
        self.assertEqual(status, 200, stats)
        self.assertTrue(stats['data']['verification']['complete'])
        self.assertEqual(stats['data']['supply']['currentSupplyWei'], page['supply']['currentSupplyWei'])
        status, geography = self.get('/api/miner-geography.php')
        self.assertEqual(status, 200, geography)
        self.assertEqual((geography['data']['miners']['registered'], geography['data']['miners']['active']), (4, 4))
        self.assertEqual(geography['data']['blocksWindow']['to'], page['snapshot']['height'])
        # A page pinned to a snapshot that a reorganization removed is the 409 reset; the kept snapshot still answers.
        snap = page['snapshot']
        pinned = f"/api/holders.php?size=25&height={snap['height']}&hash={snap['hash']}"
        self.assertEqual(self.get(pinned)[0], 200)
        self.chain.reorg(snap['height'] - 1)
        self.chain.mine_many(2)
        self.settle()
        self.assertEqual(self.get(pinned), (409, {'error': 'The holder snapshot changed. Refreshing.', 'reset': True}))


class IndexReadTests(unittest.TestCase):
    """The holders index's own node reads (H-2, H-4): Gateway.index_query reads a full-size registry that the public
    limit refuses, Gateway.index_batch is one pocold batch, and main()'s holder_index uses both."""

    def setUp(self):
        self.node = F.FakeNode()
        self.addCleanup(self.node.stop)
        self.gateway = gateway.Gateway(self.node.server_address[1], network=F.NET)

    def test_a_full_registry_is_read_whole(self):
        record = lambda i: {'id': '0x%040x' % (0xabc000 + i), 'status': 1, 'tomb': False, 'bondState': 1,
                            'lastShare': 3_000_000, 'bond': hex(20 * 10 ** 18), 'registeredAt': 2_000_000,
                            'reward': '0x%040x' % (0xdef000 + i), 'rewardBalance': hex(37 * 10 ** 18)}
        records = [record(i) for i in range(H.MAX_RECORDS)]
        self.node.answers['pocol_getMiners'] = {'result': records}
        size = len(json.dumps({'jsonrpc': '2.0', 'id': 1, 'result': records}))
        self.assertGreater(size, gateway.MAX_RESPONSE)
        self.assertLess(size, 8 * 1024 * 1024)   # pocold RESP_MAX
        with self.assertRaisesRegex(RuntimeError, 'exceeds explorer limit'):
            self.gateway.query('pocol_getMiners', [])
        answer = self.gateway.index_query('pocol_getMiners', [])
        self.assertEqual(len(H.parse_miners(answer['result'])), H.MAX_RECORDS)
        self.node.answers['pocol_getMiners'] = {'raw': b'{"jsonrpc":"2.0","id":1,"result":"' + b'a' * gateway.INDEX_RESPONSE_MAX + b'"}'}
        with self.assertRaisesRegex(RuntimeError, 'exceeds explorer limit'):
            self.gateway.index_query('pocol_getMiners', [])

    def test_one_batch_answers_every_read_in_order(self):
        calls = [('eth_getBalance', ['0x' + '%040x' % i, '0x10']) for i in range(gateway.INDEX_BATCH_MAX)]
        calls[3] = ('eth_getCode', ['0x' + '11' * 20, '0x10'])
        answers = self.gateway.index_batch(calls)
        self.assertEqual(self.node.batches, [20])
        self.assertEqual([a['id'] for a in answers], list(range(1, 21)))
        self.assertEqual(answers[3]['result'], '0x')
        self.assertEqual(answers[0]['result'], '0xde0b6b3a7640000')
        self.assertEqual(self.node.calls, [(m, p) for m, p in calls])
        for bad in (lambda r, a: json.dumps(a[:-1]).encode(), lambda r, a: json.dumps(a + a[:1]).encode(),
                    lambda r, a: json.dumps([dict(x, id=1) for x in a]).encode(),
                    lambda r, a: json.dumps({'jsonrpc': '2.0', 'id': None, 'error': {'code': -32021}}).encode(),
                    lambda r, a: json.dumps([dict(x, id=str(x['id'])) for x in a]).encode()):
            self.node.answers['batch'] = bad
            with self.assertRaisesRegex(RuntimeError, 'Invalid node batch response'):
                self.gateway.index_batch(calls)
        for size in (0, 21):
            with self.assertRaises(ValueError):
                self.gateway.index_batch(calls[:1] * size)

    def test_main_builds_the_index_on_the_batched_index_reads(self):
        chain = HF.FakeHolderChain()
        with tempfile.TemporaryDirectory() as folder:
            inner = gateway.Gateway(self.node.server_address[1], network=chain.network)
            index = gateway.holder_index(inner, Path(folder) / 'holders.sqlite3')
            try:
                self.assertEqual((index.query, index.batch), (inner.index_query, inner.index_batch))
            finally:
                index.close()


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

    def test_main_wires_the_holders_index_the_geoip_database_and_the_reporters_credential(self):
        """gateway.py as the unit starts it: POCOL_HOLDERS_DB starts the holders index (no snapshot yet: the holders and
        map routes answer 503), POCOL_GEOIP_DB the database, $CREDENTIALS_DIRECTORY/geo-reporters the reports (a signed
        report is accepted, 204); a damaged credential is named on stderr without its values and reports stay off."""
        node = F.FakeNode()
        self.addCleanup(node.stop)
        for damaged in (False, True):
            with self.subTest(damaged=damaged), tempfile.TemporaryDirectory() as directory:
                folder = Path(directory)
                (folder / 'network.json').write_text(json.dumps(F.NET))
                G.build(SAMPLE_GEOIP, '2026-11', folder / 'geo.bin')
                (folder / 'credentials').mkdir()
                (folder / 'credentials' / 'geo-reporters').write_bytes(CREDENTIALS[:-2] if damaged else CREDENTIALS)
                with socket.socket() as probe:
                    probe.bind(('127.0.0.1', 0)); port = probe.getsockname()[1]
                env = dict(os.environ, POCOL_HOLDERS_DB=str(folder / 'holders.sqlite3'), POCOL_GEOIP_DB=str(folder / 'geo.bin'),
                           CREDENTIALS_DIRECTORY=str(folder / 'credentials'))
                env.pop('POCOL_TX_DB', None)
                process = subprocess.Popen([sys.executable, '-B', str(ROOT / 'gateway.py'), '--v3-network', str(folder / 'network.json'),
                                            '--port', str(port), '--rpc-port', str(node.server_address[1]), '--site-origin', ORIGIN],
                                           env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
                try:
                    self.assertEqual(process.stdout.readline().strip(), 'PoCol explorer read-only gateway ready')
                    for path in ('/api/holders.php', '/api/holders-stats.php', '/api/miner-geography.php'):
                        self.assertEqual(F.send(port, 'GET', path)[0], 503, path)
                    headers, body = signed(report())
                    status = F.send(port, 'POST', '/api/geo-report.php', headers, body)[0]
                    self.assertEqual(status, 503 if damaged else 204)
                finally:
                    process.terminate()
                    _, stderr = process.communicate(timeout=30)   # the index's file is closed with the process (Windows)
                if damaged:
                    self.assertIn('Operator node reports disabled', stderr)
                    self.assertNotIn(REPORTER_KEY, stderr); self.assertNotIn('aa' * 32, stderr)
                self.assertNotIn('GeoIP database not used', stderr)


if __name__ == '__main__':
    unittest.main()
