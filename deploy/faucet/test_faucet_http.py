"""The faucet's HTTP answers: the machine-readable code of every error, CORS on every answer, the preflight, Retry-After
on a busy claim only, the per-client key behind the proxy, and unknown_outcome against node_unavailable. Loopback only;
fake signer and node, never real keys or transfers."""
import ast
import http.client
import io
import json
import socket
import threading
import time
import unittest
import urllib.error
from pathlib import Path
from unittest.mock import patch

import pocol_faucet as pf
import test_pocol_faucet as fixtures

NEW, OP, CHAIN = fixtures.NEW, fixtures.OP, fixtures.CHAIN
ORIGIN = 'https://pocol.net'
# The shared error-code contract (the pocol.net page translates these).
CONTRACT = {'invalid_address', 'reserved_address', 'contract_account', 'bad_request', 'too_large', 'unsupported_media',
            'not_found', 'deadline', 'already_claimed', 'not_new_account', 'already_funded', 'busy', 'daily_limit',
            'client_limit', 'signer_failed', 'node_refused', 'node_unavailable', 'unknown_outcome', 'wrong_network',
            'over_float', 'empty'}


def address(i):
    return '0x' + f'{0x100000 + i:040x}'


class FaucetHttp(unittest.TestCase):
    setUp = fixtures.FaucetRules.setUp
    faucet = fixtures.FaucetRules.faucet

    def serve(self, faucet, timeout=2):
        server = pf.Server(('127.0.0.1', 0), pf.handler(faucet)); server.request_timeout = timeout
        thread = threading.Thread(target=lambda: server.serve_forever(poll_interval=.01), daemon=True); thread.start()
        def stop(): server.shutdown(); server.server_close(); thread.join(2)
        self.addCleanup(stop)
        self.port = server.server_address[1]
        return faucet

    def call(self, method, path, body=None, headers=None):
        """(status, JSON value or None, headers) of one request."""
        c = http.client.HTTPConnection('127.0.0.1', self.port, timeout=5)
        try:
            c.request(method, path, body=body, headers={'Content-Type': 'application/json', **(headers or {})})
            r = c.getresponse(); raw = r.read()
            return r.status, json.loads(raw) if raw else None, {k.lower(): v for k, v in r.getheaders()}
        finally: c.close()

    def claim(self, who, headers=None):
        return self.call('POST', '/claim', json.dumps({'address': who}), headers)

    def error(self, answer, status, code):
        got, value, headers = answer
        self.assertEqual((got, value['type'], value['code']), (status, 'error', code), value)
        self.assertIsInstance(value['message'], str)
        self.assertEqual(headers['access-control-allow-origin'], ORIGIN, 'every error carries the CORS header')
        self.assertEqual(headers['content-type'], 'application/json')
        self.assertEqual('retry-after' in headers, code == 'busy', 'only a busy claim invites a retry')
        return value

    def test_the_code_set_is_the_contract_and_every_refusal_names_one(self):
        self.assertEqual(pf.ERROR_CODES, CONTRACT)
        tree = ast.parse(Path(pf.__file__).read_text(encoding='utf-8'))
        calls = [n for n in ast.walk(tree) if isinstance(n, ast.Call) and getattr(n.func, 'id', None) == 'Refused']
        self.assertGreater(len(calls), 20)
        for call in calls:
            self.assertEqual(len(call.args), 3, ast.unparse(call))
            self.assertIn(call.args[2].value, CONTRACT, ast.unparse(call))
        handler_codes = {n.args[1].value for n in ast.walk(tree) if isinstance(n, ast.Call)
                         and getattr(n.func, 'attr', None) == 'error' and len(n.args) >= 3 and isinstance(n.args[1], ast.Constant)}
        used = {c.args[2].value for c in calls} | handler_codes
        self.assertEqual(used, CONTRACT, 'every code of the contract is answered somewhere')

    def test_request_errors(self):
        self.serve(self.faucet(allowOrigin=ORIGIN))
        self.error(self.claim('0x12'), 400, 'invalid_address')
        self.error(self.claim(7), 400, 'invalid_address')
        self.error(self.claim('0x' + '00' * 20), 400, 'reserved_address')
        self.error(self.claim(OP), 400, 'reserved_address')
        self.node.code[address(1)] = '0x6000'
        self.error(self.claim(address(1)), 400, 'contract_account')
        self.error(self.call('POST', '/claim', '{bad'), 400, 'bad_request')
        self.error(self.call('POST', '/claim', json.dumps({'address': NEW, 'amount': 1})), 400, 'bad_request')
        self.error(self.call('POST', '/claim', '[]'), 400, 'bad_request')
        self.error(self.call('POST', '/claim', 'x' * 600), 413, 'too_large')
        self.error(self.call('POST', '/claim', json.dumps({'address': NEW}), {'Content-Type': 'text/plain'}), 415, 'unsupported_media')
        for method, path in (('GET', '/claim'), ('GET', '/'), ('GET', '/status?x=1'), ('GET', '/faucet/status'), ('POST', '/status'),
                             ('POST', '/faucet/claim')):
            with self.subTest(path=path): self.error(self.call(method, path, '{}'), 404, 'not_found')
        # Framing: a chunked body, two lengths, a length that is not a number.
        for headers in ([('Transfer-Encoding', 'chunked')], [('Content-Length', '20'), ('Content-Length', '20')],
                        [('Content-Length', '-1')]):
            with self.subTest(headers=headers), socket.create_connection(('127.0.0.1', self.port), timeout=3) as s:
                s.sendall(b'POST /claim HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\n'
                          + b''.join(f'{k}: {v}\r\n'.encode() for k, v in headers) + b'\r\n')
                r = http.client.HTTPResponse(s); r.begin(); value = json.loads(r.read())
                self.assertEqual(value['code'], 'too_large' if headers == [('Content-Length', '-1')] else 'bad_request')
                self.assertEqual(r.getheader('Access-Control-Allow-Origin'), ORIGIN)
        self.assertFalse(self.node.sent); self.assertEqual(self.faucet_claims(), {})

    def faucet_claims(self):
        path = self.dir / 'ledger.json'
        return json.loads(path.read_text())['claims'] if path.exists() else {}

    def test_an_incomplete_body_past_the_deadline_is_408(self):
        self.serve(self.faucet(allowOrigin=ORIGIN), timeout=.3)
        with socket.create_connection(('127.0.0.1', self.port), timeout=3) as s:
            s.sendall(b'POST /claim HTTP/1.1\r\nHost: x\r\nContent-Type: application/json\r\nContent-Length: 50\r\n\r\n{"addr')
            r = http.client.HTTPResponse(s); r.begin(); value = json.loads(r.read())
        self.assertEqual((r.status, value['code']), (408, 'deadline'))
        self.assertEqual(r.getheader('Access-Control-Allow-Origin'), ORIGIN)

    def test_account_and_limit_refusals(self):
        f = self.serve(self.faucet(allowOrigin=ORIGIN, dailyLimit=3, perClientDaily=3))
        status, value, headers = self.claim(NEW)
        self.assertEqual((status, value['type'], headers['access-control-allow-origin']), (200, 'sent', ORIGIN))
        self.error(self.claim(NEW), 409, 'already_claimed')
        self.node.nonces[address(2)] = 1
        self.error(self.claim(address(2)), 409, 'not_new_account')
        self.node.balances[address(3)] = 2 * 10**15
        self.error(self.claim(address(3)), 409, 'already_funded')
        f.cfg['perClientDaily'] = 1
        self.error(self.claim(address(4)), 429, 'client_limit')
        f.cfg['perClientDaily'] = 5; f.cfg['dailyLimit'] = 1
        self.error(self.claim(address(4)), 429, 'daily_limit')
        f.cfg['dailyLimit'] = 3
        # Busy: another claim holds the slot. Only this 429 sends Retry-After.
        f.lock.acquire()
        try: answer = self.claim(address(4))
        finally: f.lock.release()
        self.error(answer, 429, 'busy'); self.assertEqual(answer[2]['retry-after'], '2')
        self.assertEqual(len(self.node.sent), 1)

    def test_network_float_empty_and_signer_refusals(self):
        f = self.serve(self.faucet(allowOrigin=ORIGIN, maxFloatWei=str(10**21)))
        self.node.chain = CHAIN + 1
        self.error(self.claim(NEW), 503, 'wrong_network')
        self.node.chain = CHAIN; self.node.balances[OP] = 10**21 + 1
        self.error(self.claim(NEW), 503, 'over_float')
        self.node.balances[OP] = 10**15
        self.error(self.claim(NEW), 503, 'empty')
        self.node.balances[OP] = 10**20
        f.sign = lambda args: 'not a transaction'
        self.error(self.claim(NEW), 500, 'signer_failed')
        f.sign = self.signer
        with patch.object(pf.subprocess, 'run', side_effect=pf.subprocess.TimeoutExpired('pocol-sign', 30)):
            with self.assertRaises(pf.Refused) as e: f._sign(['--address'])
        self.assertEqual((e.exception.status, e.exception.code), (500, 'signer_failed'))
        self.assertFalse(self.node.sent); self.assertEqual(self.faucet_claims(), {})

    def node_over_http(self, refuse=None, unreachable=None):
        """urlopen as the faucet's own _rpc sees it: the fake node's answers as JSON-RPC, a JSON-RPC error for the
        method `refuse`, a connection error for the method `unreachable`."""
        def urlopen(request, timeout):
            call = json.loads(request.data)
            if call['method'] == unreachable: raise urllib.error.URLError(ConnectionRefusedError(111, 'refused'))
            if call['method'] == refuse: answer = {'jsonrpc': '2.0', 'id': 1, 'error': {'code': -32000, 'message': 'pool full'}}
            else: answer = {'jsonrpc': '2.0', 'id': 1, 'result': self.node(call['method'], call['params'])}
            return io.BytesIO(json.dumps(answer).encode())
        return patch.object(pf.urllib.request, 'urlopen', side_effect=urlopen)

    def test_node_refused_node_unavailable_and_unknown_outcome(self):
        f = self.faucet(allowOrigin=ORIGIN); f.rpc = f._rpc
        self.serve(f)
        # The node refuses a check before the send, or the send itself: nothing was sent, the address may claim again.
        for method in ('eth_getBalance', 'eth_sendRawTransaction'):
            with self.subTest(refuse=method), self.node_over_http(refuse=method):
                value = self.error(self.claim(NEW), 502, 'node_refused')
                self.assertIn('pool full', value['message'])
            self.assertNotIn(NEW, f.ledger.claims); self.assertNotIn(NEW, self.faucet_claims())
        # The node cannot be reached before anything is sent: node_unavailable, no claim kept, no "may already".
        for method in ('eth_chainId', 'eth_getTransactionCount'):
            with self.subTest(unreachable=method), self.node_over_http(unreachable=method):
                value = self.error(self.claim(NEW), 502, 'node_unavailable')
                self.assertNotIn('may already', value['message']); self.assertIn('No transfer was submitted', value['message'])
            self.assertNotIn(NEW, self.faucet_claims())
        # The send gets no answer: unknown_outcome, the claim kept as unknown.
        with self.node_over_http(unreachable='eth_sendRawTransaction'):
            value = self.error(self.claim(NEW), 502, 'unknown_outcome')
        self.assertIn('may already', value['message']); self.assertNotIn('Nothing was sent', value['message'])
        self.assertEqual(self.faucet_claims()[NEW]['state'], 'unknown')
        with self.node_over_http(): self.error(self.claim(NEW), 409, 'already_claimed')
        # The node takes the transfer but the ledger cannot record its answer: unknown_outcome, the file still refuses.
        other, save = address(5), f.ledger.save
        def failing_save():
            if f.ledger.claims.get(other, {}).get('state') == 'sent': raise OSError('disk full')
            save()
        with self.node_over_http(), patch.object(f.ledger, 'save', side_effect=failing_save):
            self.error(self.claim(other), 502, 'unknown_outcome')
        self.assertEqual(self.faucet_claims()[other]['state'], 'sending')
        self.assertEqual(len(self.node.sent), 1)
        # If even the 'unknown' save fails, the answer is still unknown_outcome (the file keeps 'sending').
        third = address(6)
        def no_save():
            if f.ledger.claims.get(third, {}).get('state') == 'unknown': raise OSError('disk full')
            save()
        with self.node_over_http(unreachable='eth_sendRawTransaction'), patch.object(f.ledger, 'save', side_effect=no_save):
            self.error(self.claim(third), 502, 'unknown_outcome')
        self.assertEqual(self.faucet_claims()[third]['state'], 'sending')
        # A ledger that cannot be written before the send: nothing sent, node_unavailable.
        fourth = address(7)
        with self.node_over_http(), patch.object(f.ledger, 'save', side_effect=OSError('read-only')):
            self.error(self.claim(fourth), 502, 'node_unavailable')
        self.assertEqual(len(self.node.sent), 1)
        self.assertNotIn(fourth, f.ledger.claims); self.assertNotIn(fourth, self.faucet_claims())
        # The promise holds: with a working ledger the same address claims again.
        with self.node_over_http():
            status, value, _ = self.claim(fourth)
        self.assertEqual((status, value['type'], len(self.node.sent)), (200, 'sent', 2))
        self.assertEqual(self.faucet_claims()[fourth]['state'], 'sent')

    def test_claim_raises_unknown_outcome_only_from_the_send_on(self):
        f = self.faucet(); self.node.fail_send = ConnectionResetError('reset')
        with self.assertRaises(pf.UnknownOutcome) as e: f.claim(NEW, 'c1')
        self.assertIsInstance(e.exception.__cause__, ConnectionResetError)
        self.assertEqual(f.ledger.claims[NEW]['state'], 'unknown')
        def down(method, params): raise ConnectionRefusedError('node down')
        f.rpc = down
        with self.assertRaises(ConnectionRefusedError): f.claim(address(1), 'c1')
        self.assertNotIn(address(1), f.ledger.claims)

    def test_status(self):
        self.serve(self.faucet(allowOrigin=ORIGIN, dailyLimit=7, perClientDaily=2))
        status, value, headers = self.call('GET', '/status')
        self.assertEqual((status, headers['access-control-allow-origin']), (200, ORIGIN))
        self.assertEqual(value, {'type': 'status', 'chainId': CHAIN, 'amountWei': str(2 * 10**15), 'faucet': OP,
                                 'remainingToday': 7, 'dailyLimit': 7, 'perClientDaily': 2})

    def test_the_preflight(self):
        self.serve(self.faucet(allowOrigin=ORIGIN))
        for path in ('/claim', '/status'):
            c = http.client.HTTPConnection('127.0.0.1', self.port, timeout=5)
            c.request('OPTIONS', path, headers={'Origin': ORIGIN, 'Access-Control-Request-Method': 'POST',
                                                'Access-Control-Request-Headers': 'content-type'})
            r = c.getresponse(); body = r.read(); c.close()
            self.assertEqual((r.status, body), (204, b''))
            self.assertEqual({k: r.getheader(k) for k in ('Access-Control-Allow-Origin', 'Access-Control-Allow-Methods',
                                                          'Access-Control-Allow-Headers', 'Access-Control-Max-Age', 'Content-Length')},
                             {'Access-Control-Allow-Origin': ORIGIN, 'Access-Control-Allow-Methods': 'GET, POST',
                              'Access-Control-Allow-Headers': 'Content-Type', 'Access-Control-Max-Age': '600', 'Content-Length': '0'})

    def test_without_allow_origin_no_cors_header_is_sent(self):
        self.serve(self.faucet())
        c = http.client.HTTPConnection('127.0.0.1', self.port, timeout=5)
        c.request('OPTIONS', '/claim'); r = c.getresponse(); r.read(); c.close()
        self.assertEqual((r.status, r.getheader('Content-Length'), r.getheader('Access-Control-Allow-Origin')), (204, '0', None))
        self.assertNotIn('access-control-allow-origin', self.call('GET', '/x')[2])

    def test_the_client_key_behind_the_proxy(self):
        f = self.serve(self.faucet(allowOrigin=ORIGIN, trustProxy=True, perClientDaily=1))
        day = pf.today()
        # The last entry is the one the route writes; earlier ones are the client's own.
        self.assertEqual(self.claim(address(1), {'X-Forwarded-For': '6.6.6.6, 203.0.113.7'})[0], 200)
        self.assertIn((day, '203.0.113.7'), f.clients)
        self.error(self.claim(address(2), {'X-Forwarded-For': '203.0.113.7'}), 429, 'client_limit')
        self.error(self.claim(address(2), {'X-Forwarded-For': '203.0.113.7, 203.0.113.7'}), 429, 'client_limit')
        # An entry that is not an IP address falls back to the peer, never a fresh counter.
        for i, bad in enumerate(('garbage', '203.0.113.8, not-an-ip', '', '203.0.113.9:80', '203.0.113.10/32')):
            with self.subTest(bad=bad):
                answer = self.claim(address(10 + i), {'X-Forwarded-For': bad})
                if i: self.error(answer, 429, 'client_limit')
                else: self.assertEqual(answer[0], 200)
                self.assertEqual(f.clients.get((day, '127.0.0.1')), 1)
        self.assertNotIn((day, 'garbage'), f.clients)
        # IPv6 clients share one counter per /64; an IPv4-mapped address is its IPv4 address.
        self.assertEqual(self.claim(address(4), {'X-Forwarded-For': '2001:db8:1:2::5'})[0], 200)
        self.assertIn((day, '2001:db8:1:2::/64'), f.clients)
        self.error(self.claim(address(5), {'X-Forwarded-For': '2001:DB8:1:2:ffff:ffff:ffff:9'}), 429, 'client_limit')
        self.assertEqual(self.claim(address(5), {'X-Forwarded-For': '2001:db8:1:3::5'})[0], 200)
        self.error(self.claim(address(6), {'X-Forwarded-For': '::ffff:203.0.113.7'}), 429, 'client_limit')
        self.assertEqual((pf.client_key('192.0.2.1'), pf.client_key('fe80::1%eth0')), ('192.0.2.1', 'fe80::/64'))
        with self.assertRaises(ValueError): pf.client_key('example.org')

    def test_without_trust_proxy_the_header_is_ignored(self):
        f = self.serve(self.faucet(allowOrigin=ORIGIN, perClientDaily=1))
        self.assertEqual(self.claim(address(1), {'X-Forwarded-For': '203.0.113.7'})[0], 200)
        self.assertEqual(set(f.clients), {(pf.today(), '127.0.0.1')})
        self.error(self.claim(address(2), {'X-Forwarded-For': '203.0.113.99'}), 429, 'client_limit')


if __name__ == '__main__': unittest.main()
