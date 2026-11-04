#!/usr/bin/env python3
"""PoCol fee faucet (earn-in network, decision A2).

A new miner on the earn-in network registers with a 0 PCOL bond, but its first transaction still pays
a fee. This service sends a small fixed amount (the configured `amountWei`) to a new account:
- once per address, ever (the claim ledger is a file written atomically before anything is sent);
- only to an account that has never sent a transaction, holds no code and holds less than the amount;
- within a daily limit (UTC day) and a per-client daily limit;
- from an operator account whose balance was moved to the new network. It creates no coins.

The operator key never enters this process: each transfer is signed by `pocol-sign --key-file`, and the
signed bytes are relayed to the local node with `eth_sendRawTransaction`. Before every transfer the node's
chain ID must equal the configured one.

    python3 pocol_faucet.py --config /etc/pocol-faucet/config.json
    POST /claim {"address": "0x..."}  ->  {"hash": "0x...", "amountWei": "..."}
    GET  /status                      ->  {"chainId": ..., "amountWei": ..., "remainingToday": ...}
"""
import argparse, datetime, http.server, io, json, os, re, subprocess, sys, tempfile, threading, time, urllib.request
from contextlib import contextmanager
from pathlib import Path

ADDRESS = re.compile(r'^0x[0-9a-fA-F]{40}$')
DEFAULTS = {'listen': '127.0.0.1', 'port': 8547, 'rpc': 'http://127.0.0.1:8545', 'amountWei': str(2 * 10**15),
            'dailyLimit': 200, 'perClientDaily': 3, 'trustProxy': False, 'allowOrigin': None, 'signer': 'pocol-sign'}
MAX_BODY = 512
MAX_CONNECTIONS = 16
REQUEST_TIMEOUT = 12
GWEI = 10**9

class Refused(Exception):
    def __init__(self, status, message):
        super().__init__(message); self.status = status; self.message = message

def reserved(address):
    """System programs, PAY, precompiles and the zero address never receive faucet transfers."""
    n = int(address, 16)
    return n < 0x10000 or 0xc0c001 <= n <= 0xc0c0ff or n == 2**160 - 2

def today():
    return datetime.datetime.now(datetime.timezone.utc).date().isoformat()

class Ledger:
    """Claims by address, written atomically (temp file, fsync, rename) and durable before a send."""
    def __init__(self, path):
        self.path = Path(path)
        if self.path.exists():
            data = json.loads(self.path.read_text(encoding='utf-8'))
            if not isinstance(data.get('claims'), dict): raise ValueError('faucet ledger is damaged; preserve it before recovery')
            self.claims = data['claims']
        else:
            self.claims = {}
    def count(self, day):
        # /status can race a claim's insert/remove. Count an owned dict snapshot.
        return sum(1 for c in self.claims.copy().values() if c.get('day') == day)
    def save(self):
        fd, temp = tempfile.mkstemp(prefix='.faucet-', dir=self.path.parent)
        try:
            with os.fdopen(fd, 'w', encoding='utf-8') as f:
                json.dump({'claims': self.claims}, f, sort_keys=True); f.flush(); os.fsync(f.fileno())
            os.replace(temp, self.path)
            if os.name != 'nt':
                d = os.open(self.path.parent, os.O_RDONLY)
                try: os.fsync(d)
                finally: os.close(d)
        except BaseException:
            if os.path.exists(temp): os.unlink(temp)
            raise

class Faucet:
    def __init__(self, cfg, rpc=None, sign=None):
        self.cfg = {**DEFAULTS, **cfg}
        for k in ('chainId', 'keyFile', 'ledger'):
            if k not in self.cfg: raise ValueError(f'config needs {k}')
        self.chain_id = int(self.cfg['chainId']); self.amount = int(self.cfg['amountWei'])
        if self.amount <= 0 or self.amount > 10**18: raise ValueError('amountWei must be positive and at most 1 PCOL')
        key = Path(self.cfg['keyFile'])
        if not key.is_file(): raise ValueError('the operator key file does not exist')
        if os.name != 'nt' and key.stat().st_mode & 0o077: raise ValueError('the operator key file must not be readable by group or others')
        self.rpc = rpc or self._rpc; self.sign = sign or self._sign
        self.ledger = Ledger(self.cfg['ledger']); self.lock = threading.Lock(); self.clients = {}
        self.address = self.sign(['--address']).lower()
        if not ADDRESS.match(self.address): raise ValueError('the signer did not return the operator address')

    def _rpc(self, method, params):
        body = json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': method, 'params': params}).encode()
        req = urllib.request.Request(self.cfg['rpc'], data=body, headers={'Content-Type': 'application/json'})
        with urllib.request.urlopen(req, timeout=10) as r: value = json.load(r)
        if 'error' in value: raise Refused(502, f"node refused {method}: {value['error'].get('message', value['error'])}")
        return value['result']

    def _sign(self, args):
        cmd = [self.cfg['signer'], '--key-file', str(self.cfg['keyFile']), '--chain-id', str(self.chain_id), *args]
        p = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
        if p.returncode: raise Refused(500, 'the signer failed')
        return p.stdout.strip()

    def status(self):
        day = today()
        return {'type': 'status', 'chainId': self.chain_id, 'amountWei': str(self.amount), 'faucet': self.address,
                'remainingToday': max(0, int(self.cfg['dailyLimit']) - self.ledger.count(day))}

    @contextmanager
    def claim_slot(self):
        # An unavailable node/signer must not build a queue of HTTP worker threads.
        if not self.lock.acquire(blocking=False):
            raise Refused(429, 'The faucet is processing a claim. Retry shortly.')
        try: yield
        finally: self.lock.release()

    def claim(self, address, client):
        if not isinstance(address, str) or not ADDRESS.match(address): raise Refused(400, 'Enter the full 0x address (42 characters)')
        address = address.lower()
        if reserved(address) or address == self.address: raise Refused(400, 'This address cannot receive faucet transfers')
        with self.claim_slot():
            day = today()
            if address in self.ledger.claims: raise Refused(409, 'This address has already received a faucet transfer')
            if self.ledger.count(day) >= int(self.cfg['dailyLimit']): raise Refused(429, "Today's faucet limit is reached. Try again tomorrow (UTC).")
            used = self.clients.get((day, client), 0)
            if used >= int(self.cfg['perClientDaily']): raise Refused(429, 'This connection has reached its daily faucet limit')
            if int(self.rpc('eth_chainId', []), 16) != self.chain_id: raise Refused(503, 'The node is on a different network; the faucet is stopped')
            if int(self.rpc('eth_getTransactionCount', [address, 'pending']), 16) != 0: raise Refused(409, 'The faucet funds only new accounts that have not sent a transaction')
            if self.rpc('eth_getCode', [address, 'latest']) not in ('0x', '0x0', ''): raise Refused(400, 'The faucet funds only ordinary accounts')
            if int(self.rpc('eth_getBalance', [address, 'pending']), 16) >= self.amount: raise Refused(409, 'This account already holds enough PCOL for its first fee')
            price = int(self.rpc('eth_gasPrice', []), 16); max_fee = max(2 * price, 2 * GWEI)
            if int(self.rpc('eth_getBalance', [self.address, 'pending']), 16) < self.amount + 21000 * max_fee: raise Refused(503, 'The faucet is empty')
            nonce = int(self.rpc('eth_getTransactionCount', [self.address, 'pending']), 16)
            raw = self.sign(['--to', address, '--value', str(self.amount), '--nonce', str(nonce), '--max-fee', str(max_fee)])
            if not re.fullmatch(r'0x[0-9a-fA-F]+', raw): raise Refused(500, 'the signer returned no transaction')
            # Durable before the send: a crash after this line can never pay the same address twice.
            self.ledger.claims[address] = {'day': day, 'nonce': nonce, 'state': 'sending'}; self.ledger.save()
            try:
                tx = self.rpc('eth_sendRawTransaction', [raw])
            except Refused:
                # The node answered and refused: nothing was sent, so the address may claim again.
                del self.ledger.claims[address]; self.ledger.save(); raise
            except BaseException:
                # No answer: the transfer may be in the pool. Keep the claim; the operator checks the nonce.
                self.ledger.claims[address]['state'] = 'unknown'; self.ledger.save(); raise
            self.ledger.claims[address].update({'state': 'sent', 'hash': tx}); self.ledger.save()
            self.clients[(day, client)] = used + 1
            for key in [k for k in self.clients if k[0] != day]: del self.clients[key]
            return {'type': 'sent', 'hash': tx, 'amountWei': str(self.amount), 'to': address}


class RequestDeadline(TimeoutError): pass


class DeadlineReader(io.RawIOBase):
    """Socket input with one absolute header/body deadline, not a per-byte timeout."""
    def __init__(self, connection, timeout):
        self.connection = connection
        self.deadline = time.monotonic() + timeout
    def readable(self): return True
    def readinto(self, buffer):
        remaining = self.deadline - time.monotonic()
        if remaining <= 0: raise RequestDeadline('Request read deadline')
        self.connection.settimeout(remaining)
        try: return self.connection.recv_into(buffer)
        except TimeoutError as error: raise RequestDeadline('Request read deadline') from error


class Server(http.server.ThreadingHTTPServer):
    daemon_threads = True
    request_queue_size = MAX_CONNECTIONS
    request_timeout = REQUEST_TIMEOUT

    def __init__(self, address, request_handler, *, max_connections=MAX_CONNECTIONS):
        self.slots = threading.BoundedSemaphore(max_connections)
        super().__init__(address, request_handler)
    def process_request(self, request, client_address):
        if not self.slots.acquire(blocking=False):
            self.shutdown_request(request); return
        try: super().process_request(request, client_address)
        except BaseException: self.slots.release(); raise
    def process_request_thread(self, request, client_address):
        try: super().process_request_thread(request, client_address)
        finally: self.slots.release()


def handler(faucet):
    class Handler(http.server.BaseHTTPRequestHandler):
        server_version = 'PoColFaucet'; sys_version = ''
        def setup(self):
            super().setup()
            timeout = getattr(self.server, 'request_timeout', REQUEST_TIMEOUT)
            self.connection.settimeout(timeout)
            self.rfile.close()
            self.rfile = io.BufferedReader(DeadlineReader(self.connection, timeout))
        def log_message(self, fmt, *args): pass
        def client(self):
            if faucet.cfg['trustProxy']:
                forwarded = self.headers.get('X-Forwarded-For', '')
                if forwarded: return forwarded.split(',')[-1].strip()
            return self.client_address[0]
        def reply(self, status, value):
            body = json.dumps(value).encode()
            self.send_response(status); self.send_header('Content-Type', 'application/json'); self.send_header('Content-Length', str(len(body)))
            self.send_header('Cache-Control', 'no-store'); self.send_header('X-Content-Type-Options', 'nosniff')
            if faucet.cfg['allowOrigin']: self.send_header('Access-Control-Allow-Origin', faucet.cfg['allowOrigin'])
            try: self.end_headers(); self.wfile.write(body)
            except (BrokenPipeError, ConnectionResetError, TimeoutError): pass
        def do_OPTIONS(self):
            self.send_response(204)
            if faucet.cfg['allowOrigin']:
                self.send_header('Access-Control-Allow-Origin', faucet.cfg['allowOrigin']); self.send_header('Access-Control-Allow-Methods', 'GET, POST')
                self.send_header('Access-Control-Allow-Headers', 'Content-Type')
            self.end_headers()
        def do_GET(self):
            if self.path != '/status': return self.reply(404, {'type': 'error', 'message': 'not found'})
            self.reply(200, faucet.status())
        def do_POST(self):
            try:
                if self.path != '/claim': raise Refused(404, 'not found')
                lengths = self.headers.get_all('Content-Length', [])
                if self.headers.get('Transfer-Encoding') is not None or len(lengths) != 1:
                    raise Refused(400, 'A single Content-Length is required; chunked requests are not supported')
                if re.fullmatch(r'[0-9]{1,3}', lengths[0]) is None:
                    raise Refused(413, 'request body size is invalid')
                length = int(lengths[0])
                if length <= 0 or length > MAX_BODY: raise Refused(413, 'request body too large')
                if self.headers.get('Content-Type', '').split(';')[0].strip().lower() != 'application/json':
                    raise Refused(415, 'Use application/json')
                raw = self.rfile.read(length)
                if len(raw) != length: raise Refused(400, 'Incomplete request body')
                try: body = json.loads(raw)
                except ValueError: raise Refused(400, 'invalid JSON')
                if not isinstance(body, dict) or set(body) != {'address'}: raise Refused(400, 'send {"address": "0x..."}')
                self.reply(200, faucet.claim(body['address'], self.client()))
            except Refused as e:
                self.reply(e.status, {'type': 'error', 'message': e.message})
            except RequestDeadline:
                self.reply(408, {'type': 'error', 'message': 'Request read deadline exceeded; no claim was processed.'})
            except Exception:
                self.reply(502, {'type': 'error', 'message': 'The faucet could not reach the node or save its response. A transfer may already have been submitted; check the address before retrying.'})
    return Handler

def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument('--config', required=True, type=Path)
    a = p.parse_args(argv)
    faucet = Faucet(json.loads(a.config.read_text(encoding='utf-8')))
    server = Server((faucet.cfg['listen'], int(faucet.cfg['port'])), handler(faucet))
    print(json.dumps({'listening': f"{faucet.cfg['listen']}:{server.server_address[1]}", **faucet.status()}), flush=True)
    try: server.serve_forever()
    except KeyboardInterrupt: pass
    return 0

if __name__ == '__main__':
    sys.exit(main())
