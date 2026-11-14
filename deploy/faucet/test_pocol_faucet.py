#!/usr/bin/env python3
"""Fee faucet rules: once per address, new accounts only, daily and per-client limits, chain ID pinned,
durable ledger before the send, and nothing sent on any refusal. The node and signer are fakes here;
deploy/standalone/test_wallet_earnin.py drives the faucet against real nodes and the real signer."""
import http.client, json, os, shutil, tempfile, threading, unittest
from pathlib import Path
import pocol_faucet as pf

CHAIN = 261597237
OP = '0x' + '11' * 20
NEW = '0x' + 'ab' * 20

class Node:
    def __init__(self):
        self.chain = CHAIN; self.nonces = {OP: 5}; self.balances = {OP: 10**21}; self.code = {}; self.sent = []; self.fail_send = None
    def __call__(self, method, params):
        if method == 'eth_chainId': return hex(self.chain)
        if method == 'eth_gasPrice': return hex(10**9)
        if method == 'eth_getTransactionCount': return hex(self.nonces.get(params[0], 0) + sum(1 for s in self.sent if s[0] == params[0]))
        if method == 'eth_getBalance': return hex(self.balances.get(params[0], 0))
        if method == 'eth_getCode': return self.code.get(params[0], '0x')
        if method == 'eth_sendRawTransaction':
            if self.fail_send: raise self.fail_send
            self.sent.append((OP, params[0])); return '0x' + f'{len(self.sent):064x}'
        raise AssertionError(method)

class Signer:
    def __init__(self): self.calls = []
    def __call__(self, args):
        self.calls.append(args)
        if args == ['--address']: return OP.upper().replace('X', 'x')
        a = dict(zip(args[::2], args[1::2])); return '0x02' + a['--to'][2:] + format(int(a['--nonce']), '04x')

class FaucetRules(unittest.TestCase):
    def setUp(self):
        self.dir = Path(tempfile.mkdtemp(prefix='pocol-faucet-')); self.addCleanup(shutil.rmtree, self.dir)
        self.key = self.dir / 'operator.pocol-key'; self.key.write_text('k'); os.chmod(self.key, 0o600)
        self.node = Node(); self.signer = Signer()
    def faucet(self, **cfg):
        return pf.Faucet({'chainId': CHAIN, 'keyFile': str(self.key), 'ledger': str(self.dir / 'ledger.json'), **cfg}, rpc=self.node, sign=self.signer)
    def refused(self, f, address, status, text, client='c1'):
        before = len(self.node.sent)
        with self.assertRaises(pf.Refused) as e: f.claim(address, client)
        self.assertEqual(e.exception.status, status, e.exception.message); self.assertIn(text, e.exception.message)
        self.assertEqual(len(self.node.sent), before, 'a refusal sends nothing')

    def test_new_account_gets_the_amount_once(self):
        f = self.faucet()
        r = f.claim(NEW.upper().replace('X', 'x'), 'c1')
        self.assertEqual((r['to'], r['amountWei']), (NEW, str(2 * 10**15)))
        sign = dict(zip(self.signer.calls[-1][::2], self.signer.calls[-1][1::2]))
        self.assertEqual((sign['--to'], sign['--value'], sign['--nonce'], sign['--max-fee']), (NEW, str(2 * 10**15), '5', str(2 * 10**9)))
        self.refused(f, NEW, 409, 'already received')
        # The ledger survives a restart.
        self.refused(self.faucet(), NEW, 409, 'already received')
        self.assertEqual(json.loads((self.dir / 'ledger.json').read_text())['claims'][NEW]['state'], 'sent')

    def test_only_new_ordinary_accounts(self):
        f = self.faucet()
        old = '0x' + '22' * 20; self.node.nonces[old] = 1; self.refused(f, old, 409, 'new accounts')
        rich = '0x' + '33' * 20; self.node.balances[rich] = 2 * 10**15; self.refused(f, rich, 409, 'already holds')
        contract = '0x' + '44' * 20; self.node.code[contract] = '0x6000'; self.refused(f, contract, 400, 'ordinary accounts')
        for bad in ('0x' + '00' * 20, '0x' + '00' * 17 + 'c0c001', '0x' + '00' * 17 + 'c0c0ff', '0x' + 'ff' * 19 + 'fe', OP):
            self.refused(f, bad, 400, 'cannot receive')
        for bad in ('ab' * 20, '0x' + 'ab' * 19, '0x' + 'zz' * 20, 7, None):
            self.refused(f, bad, 400, '42 characters')

    def test_daily_and_per_client_limits(self):
        f = self.faucet(dailyLimit=3, perClientDaily=2)
        addrs = ['0x' + f'{i:040x}' for i in range(0x100001, 0x100005)]
        f.claim(addrs[0], 'a'); f.claim(addrs[1], 'a')
        self.refused(f, addrs[2], 429, 'connection', client='a')
        f.claim(addrs[2], 'b')
        self.refused(f, addrs[3], 429, "Today's faucet limit", client='c')
        self.assertEqual(f.status()['remainingToday'], 0)

    def test_wrong_chain_or_empty_faucet_stops(self):
        f = self.faucet(); self.node.chain = CHAIN + 1
        self.refused(f, NEW, 503, 'different network')
        self.node.chain = CHAIN; self.node.balances[OP] = 2 * 10**15
        self.refused(f, NEW, 503, 'empty')
        self.assertNotIn(NEW, json.loads((self.dir / 'ledger.json').read_text())['claims'] if (self.dir / 'ledger.json').exists() else {})

    def test_send_refused_rolls_back_but_unanswered_send_keeps_the_claim(self):
        f = self.faucet()
        self.node.fail_send = pf.Refused(502, 'node refused eth_sendRawTransaction: pool full')
        with self.assertRaises(pf.Refused): f.claim(NEW, 'c1')
        self.assertNotIn(NEW, f.ledger.claims, 'the node refused, so the address may claim again')
        self.node.fail_send = TimeoutError('no answer')
        with self.assertRaises(TimeoutError): f.claim(NEW, 'c1')
        self.assertEqual(json.loads((self.dir / 'ledger.json').read_text())['claims'][NEW]['state'], 'unknown')
        self.node.fail_send = None
        self.refused(f, NEW, 409, 'already received')

    def test_ledger_is_durable_before_the_send(self):
        f = self.faucet(); seen = {}
        def send(method, params, inner=self.node):
            if method == 'eth_sendRawTransaction':
                seen['ledger'] = json.loads((self.dir / 'ledger.json').read_text())['claims'].get(NEW)
            return inner(method, params)
        f.rpc = send; f.claim(NEW, 'c1')
        self.assertEqual(seen['ledger']['state'], 'sending')

    def test_configuration_is_checked(self):
        os.chmod(self.key, 0o644)
        if os.name != 'nt':
            with self.assertRaises(ValueError): self.faucet()
        os.chmod(self.key, 0o600)
        for bad in ({'amountWei': '0'}, {'amountWei': str(2 * 10**18)}):
            with self.assertRaises(ValueError): self.faucet(**bad)
        with self.assertRaises(ValueError): pf.Faucet({'chainId': CHAIN, 'ledger': 'x'}, rpc=self.node, sign=self.signer)
        (self.dir / 'ledger.json').write_text('{"claims": []}')
        with self.assertRaises(ValueError): self.faucet()

    def test_http_surface(self):
        f = self.faucet(allowOrigin='https://pocol.net')
        server = pf.Server(('127.0.0.1', 0), pf.handler(f)); port = server.server_address[1]
        thread = threading.Thread(target=server.serve_forever, daemon=True); thread.start()
        def stop(): server.shutdown(); server.server_close(); thread.join(2)
        self.addCleanup(stop)
        def call(method, path, body=None, headers=None):
            c = http.client.HTTPConnection('127.0.0.1', port, timeout=5)
            try:
                c.request(method, path, body=body, headers=headers or {'Content-Type': 'application/json'}); r = c.getresponse()
                return r.status, json.loads(r.read() or b'null'), r.getheader('Access-Control-Allow-Origin')
            finally: c.close()
        self.assertEqual(call('GET', '/status')[1]['chainId'], CHAIN)
        status, value, origin = call('POST', '/claim', json.dumps({'address': NEW}))
        self.assertEqual((status, value['to'], origin), (200, NEW, 'https://pocol.net'))
        self.assertEqual(call('POST', '/claim', json.dumps({'address': NEW}))[0], 409)
        self.assertEqual(call('POST', '/claim', json.dumps({'address': NEW, 'amount': '9'}))[0], 400)
        self.assertEqual(call('POST', '/claim', 'x' * 600)[0], 413)
        self.assertEqual(call('POST', '/claim', '{bad')[0], 400)
        self.assertEqual(call('GET', '/claim')[0], 404)
        self.node.fail_send = ConnectionResetError()
        status, value, _ = call('POST', '/claim', json.dumps({'address': '0x' + 'cd' * 20}))
        self.assertEqual(status, 502); self.assertIn('could not reach', value['message'])

if __name__ == '__main__':
    unittest.main()
