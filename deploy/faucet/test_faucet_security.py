"""Bounded loopback regressions; fake signer/node, never real keys or transfers."""
import http.client
import json
import socket
import socketserver
import sys
import threading
import time
import unittest

import pocol_faucet as pf
import test_pocol_faucet as fixtures

NEW, OP = fixtures.NEW, fixtures.OP


class FaucetSecurity(unittest.TestCase):
    setUp = fixtures.FaucetRules.setUp
    faucet = fixtures.FaucetRules.faucet

    def serve(self, handler, *, connections=2, timeout=.15):
        cls = getattr(pf, 'Server', pf.http.server.ThreadingHTTPServer)
        if cls is pf.http.server.ThreadingHTTPServer:
            server = cls(('127.0.0.1', 0), handler)
        else:
            server = cls(('127.0.0.1', 0), handler, max_connections=connections)
        server.request_timeout = timeout
        thread = threading.Thread(target=lambda: server.serve_forever(poll_interval=.01), daemon=True)
        thread.start()
        def stop():
            server.shutdown(); server.server_close(); thread.join(2)
        self.addCleanup(stop)
        return server.server_address[1]

    def test_connections_are_capped_before_spawning_another_worker(self):
        entered, release = threading.Event(), threading.Event()
        lock = threading.Lock()
        count = 0
        class Hold(socketserver.BaseRequestHandler):
            def handle(self):
                nonlocal count
                with lock:
                    count += 1
                    if count >= 2: entered.set()
                release.wait(2)
        port = self.serve(Hold)
        clients = []
        try:
            clients = [socket.create_connection(('127.0.0.1', port), timeout=1) for _ in range(2)]
            self.assertTrue(entered.wait(1))
            excess = socket.create_connection(('127.0.0.1', port), timeout=.3)
            clients.append(excess)
            try: closed = excess.recv(1) == b''
            except ConnectionResetError: closed = True
            except TimeoutError: closed = False
            self.assertTrue(closed, f'excess connection stayed open; worker count={count}')
            self.assertEqual(count, 2)
        finally:
            release.set()
            for client in clients: client.close()

    def test_header_trickling_cannot_extend_the_absolute_read_deadline(self):
        port = self.serve(pf.handler(self.faucet()), timeout=.15)
        with socket.create_connection(('127.0.0.1', port), timeout=.6) as client:
            client.sendall(b'GET /status HTTP/1.1\r\nX-Pad: ')
            # Each byte arrives before the inactivity timeout; the total still expires.
            for _ in range(6):
                time.sleep(.04)
                try: client.sendall(b'x')
                except (BrokenPipeError, ConnectionResetError): break
            try: closed = client.recv(1) == b''
            except ConnectionResetError: closed = True
            except TimeoutError: closed = False
            self.assertTrue(closed, 'trickled headers held an admission slot beyond the read deadline')

    def test_duplicate_length_is_rejected_before_any_claim(self):
        faucet = self.faucet()
        port = self.serve(pf.handler(faucet), timeout=2)
        body = json.dumps({'address': NEW})
        conn = http.client.HTTPConnection('127.0.0.1', port, timeout=2)
        try:
            conn.putrequest('POST', '/claim')
            conn.putheader('Content-Type', 'application/json')
            conn.putheader('Content-Length', str(len(body)))
            conn.putheader('Content-Length', str(len(body)))
            conn.endheaders(body.encode())
            response = conn.getresponse(); response.read()
            self.assertEqual(response.status, 400)
            self.assertNotIn(NEW, faucet.ledger.claims)
            self.assertFalse(self.node.sent)
        finally: conn.close()

    def test_truncated_body_is_rejected_before_signing(self):
        faucet = self.faucet()
        port = self.serve(pf.handler(faucet), timeout=2)
        body = json.dumps({'address': NEW}).encode()
        with socket.create_connection(('127.0.0.1', port), timeout=2) as client:
            request = (f'POST /claim HTTP/1.1\r\nHost: localhost\r\nContent-Type: application/json\r\nContent-Length: {len(body)+1}\r\n\r\n').encode()
            client.sendall(request + body); client.shutdown(socket.SHUT_WR)
            response = http.client.HTTPResponse(client); response.begin(); response.read()
            self.assertEqual(response.status, 400)
        self.assertFalse(self.node.sent)
        self.assertNotIn(NEW, faucet.ledger.claims)

    def test_an_unanswered_send_never_claims_that_nothing_was_sent(self):
        faucet = self.faucet()
        original = faucet.rpc
        def rpc(method, params):
            if method == 'eth_sendRawTransaction':
                self.node.sent.append((OP, params[0]))
                raise TimeoutError('reply lost after accepting the transaction')
            return original(method, params)
        faucet.rpc = rpc
        port = self.serve(pf.handler(faucet), timeout=2)
        conn = http.client.HTTPConnection('127.0.0.1', port, timeout=2)
        try:
            conn.request('POST', '/claim', json.dumps({'address': NEW}), {'Content-Type': 'application/json'})
            response = conn.getresponse(); value = json.loads(response.read())
            self.assertEqual(response.status, 502)
            self.assertNotIn('Nothing was sent', value['message'])
            self.assertIn('may already', value['message'])
            self.assertEqual(faucet.ledger.claims[NEW]['state'], 'unknown')
            with self.assertRaises(pf.Refused): faucet.claim(NEW, 'same-client')
            self.assertEqual(len(self.node.sent), 1)
        finally: conn.close()

    def test_a_busy_signer_does_not_queue_another_claim(self):
        faucet = self.faucet()
        entered, release, finished = threading.Event(), threading.Event(), threading.Event()
        original = faucet.rpc
        errors = []
        def rpc(method, params):
            if method == 'eth_chainId': entered.set(); release.wait(2)
            return original(method, params)
        faucet.rpc = rpc
        def first():
            try: faucet.claim(NEW, 'one')
            except Exception as error: errors.append(error)
        def second():
            try: faucet.claim('0x'+'cd'*20, 'two')
            except pf.Refused as error: errors.append(error)
            finally: finished.set()
        a = threading.Thread(target=first); a.start()
        b = None
        try:
            self.assertTrue(entered.wait(1))
            b = threading.Thread(target=second); b.start()
            self.assertTrue(finished.wait(.3), 'second request waited behind the signer')
            self.assertTrue(any(isinstance(e, pf.Refused) and e.status == 429 for e in errors))
        finally:
            release.set(); a.join(3)
            if b: b.join(3)

    def test_status_count_has_a_stable_snapshot_during_a_claim(self):
        faucet = self.faucet()
        faucet.ledger.claims['0x'+'ef'*20] = {'day': pf.today(), 'state': 'sent'}
        entered, release = threading.Event(), threading.Event()
        errors, results = [], []
        # Pause the count's genuine dict iterator, without replacing ledger data.
        def trace(frame, event, arg):
            if (event == 'line' and frame.f_code.co_name == '<genexpr>'
                    and frame.f_code.co_filename == pf.__file__ and not entered.is_set()):
                entered.set(); release.wait(2)
            return trace
        def status():
            sys.settrace(trace)
            try: results.append(faucet.status())
            except Exception as error: errors.append(error)
            finally: sys.settrace(None)
        thread = threading.Thread(target=status); thread.start()
        try:
            self.assertTrue(entered.wait(1))
            faucet.claim(NEW, 'another-client')
        finally:
            release.set(); thread.join(3)
        self.assertFalse(thread.is_alive())
        self.assertFalse(errors, repr(errors))
        self.assertEqual(results[0]['remainingToday'], int(faucet.cfg['dailyLimit'])-1)
        self.assertEqual(faucet.status()['remainingToday'], int(faucet.cfg['dailyLimit'])-2)


if __name__ == '__main__': unittest.main()
