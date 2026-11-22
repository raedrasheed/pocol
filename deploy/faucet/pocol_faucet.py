#!/usr/bin/env python3
"""PoCol fee faucet, on the rehearsal network and on mainnet (owner decision 2026-11-18, superseding the
rehearsal-only decision of 2026-11-16).

On the mainnet chain ID (MAINNET_CHAIN_IDS, deploy/mainnet/v3-profile.json) the owner-approved limits are ceilings
(MAINNET_LIMITS): a config above them is refused before the key file or the signer is touched, and a claim is refused
while the operator account holds more than the mainnet float, so only a dedicated key with a small balance can serve.
Every claim still checks that the node's chain ID is the configured one.

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
    POST /claim {"address": "0x..."}  ->  {"type": "sent", "hash": "0x...", "amountWei": "...", "to": "0x..."}
    GET  /status                      ->  {"type": "status", "chainId": ..., "amountWei": ..., "remainingToday": ..., ...}

Every error answer is {"type": "error", "code": CODE, "message": TEXT}, CODE one of ERROR_CODES (README.md lists them).
`unknown_outcome` means a transfer may already have been submitted (the ledger keeps the claim): check the address
before retrying. `node_unavailable` means the node could not be reached (or the ledger written) before anything was
sent.
"""
import argparse, datetime, http.server, io, ipaddress, json, os, re, subprocess, sys, tempfile, threading, time, urllib.request
from contextlib import contextmanager
from pathlib import Path

ADDRESS = re.compile(r'^0x[0-9a-fA-F]{40}$')
DEFAULTS = {'listen': '127.0.0.1', 'port': 8547, 'rpc': 'http://127.0.0.1:8545', 'amountWei': str(2 * 10**15),
            'dailyLimit': 200, 'perClientDaily': 3, 'trustProxy': False, 'allowOrigin': None, 'signer': 'pocol-sign'}
# The mainnet chain ID (deploy/mainnet/v3-profile.json chainId; a test pins it).
MAINNET_CHAIN_IDS = frozenset({2656089850})
# Owner decision 2026-11-18: on mainnet, at most 0.002 PCOL per address, 200 transfers a UTC day and 3 per client a
# UTC day, from a dedicated operator key holding at most a 10 PCOL float. A config may set lower values, never higher.
MAINNET_LIMITS = {'amountWei': 2 * 10**15, 'dailyLimit': 200, 'perClientDaily': 3, 'maxFloatWei': 10 * 10**18}
MAX_BODY = 512
MAX_CONNECTIONS = 16
REQUEST_TIMEOUT = 12
GWEI = 10**9

# The machine-readable `code` of every error answer (a page translates these, not the English message).
ERROR_CODES = frozenset({
    'invalid_address', 'reserved_address', 'contract_account', 'bad_request', 'too_large', 'unsupported_media',
    'not_found', 'deadline', 'already_claimed', 'not_new_account', 'already_funded', 'busy', 'daily_limit',
    'client_limit', 'signer_failed', 'node_refused', 'node_unavailable', 'unknown_outcome', 'wrong_network',
    'over_float', 'empty'})

class Refused(Exception):
    """A refusal answered with `status` and `code`; nothing was sent. A Refused raised without a code is a node's
    refusal (an injected rpc may raise one), so it answers node_refused."""
    def __init__(self, status, message, code='node_refused'):
        super().__init__(message); self.status = status; self.message = message; self.code = code

class UnknownOutcome(TimeoutError):
    """A transfer may have been submitted: the node did not answer the send, or its answer could not be saved. The
    ledger keeps the claim (`unknown`, or `sent` when only saving the answer failed), so the address cannot claim again
    until the operator has checked the nonce. A TimeoutError: the send's outcome did not arrive."""

def client_key(value):
    """The per-client counter key of an IP address: an IPv4 address as is, an IPv6 address by its /64 (one host can
    rotate addresses inside its /64). ValueError if `value` is not an IP address."""
    ip = ipaddress.ip_address(value.strip())
    if ip.version == 6 and ip.ipv4_mapped: ip = ip.ipv4_mapped
    if ip.version == 4: return str(ip)
    return str(ipaddress.IPv6Network((int(ip) >> 64 << 64, 64)))

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
        # Optional (recommended): the network's genesis hash (its invitation's devnet.json genesisHash). With it, a claim
        # also needs the node's pocol_getParams to name it: a chain ID alone does not tell two networks apart.
        self.genesis = self.cfg.get('genesisHash')
        if self.genesis is not None and not (isinstance(self.genesis, str) and re.fullmatch(r'0x[0-9a-f]{64}', self.genesis)):
            raise ValueError('genesisHash must be 0x and 64 lower-case hex digits')
        if self.amount <= 0 or self.amount > 10**18: raise ValueError('amountWei must be positive and at most 1 PCOL')
        self.max_float = None if self.cfg.get('maxFloatWei') is None else int(self.cfg['maxFloatWei'])
        if self.max_float is not None and self.max_float <= 0: raise ValueError('maxFloatWei must be positive')
        if self.chain_id in MAINNET_CHAIN_IDS:
            if self.max_float is None: self.max_float = MAINNET_LIMITS['maxFloatWei']
            values = {'amountWei': self.amount, 'dailyLimit': int(self.cfg['dailyLimit']),
                      'perClientDaily': int(self.cfg['perClientDaily']), 'maxFloatWei': self.max_float}
            for k, ceiling in MAINNET_LIMITS.items():
                if values[k] > ceiling:
                    raise ValueError(f'on mainnet {k} may be at most {ceiling} (owner decision 2026-11-18)')
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
        if 'error' in value:
            error = value['error']; text = error.get('message', error) if isinstance(error, dict) else error
            raise Refused(502, f'node refused {method}: {text}', 'node_refused')
        return value['result']

    def _sign(self, args):
        cmd = [self.cfg['signer'], '--key-file', str(self.cfg['keyFile']), '--chain-id', str(self.chain_id), *args]
        try: p = subprocess.run(cmd, capture_output=True, text=True, timeout=30)
        except (OSError, subprocess.SubprocessError): raise Refused(500, 'the signer failed', 'signer_failed')
        if p.returncode: raise Refused(500, 'the signer failed', 'signer_failed')
        return p.stdout.strip()

    def status(self):
        day = today()
        return {'type': 'status', 'chainId': self.chain_id, 'amountWei': str(self.amount), 'faucet': self.address,
                'remainingToday': max(0, int(self.cfg['dailyLimit']) - self.ledger.count(day)),
                'dailyLimit': int(self.cfg['dailyLimit']), 'perClientDaily': int(self.cfg['perClientDaily'])}

    @contextmanager
    def claim_slot(self):
        # An unavailable node/signer must not build a queue of HTTP worker threads.
        if not self.lock.acquire(blocking=False):
            raise Refused(429, 'The faucet is processing a claim. Retry shortly.', 'busy')
        try: yield
        finally: self.lock.release()

    def claim(self, address, client):
        if not isinstance(address, str) or not ADDRESS.match(address): raise Refused(400, 'Enter the full 0x address (42 characters)', 'invalid_address')
        address = address.lower()
        if reserved(address) or address == self.address: raise Refused(400, 'This address cannot receive faucet transfers', 'reserved_address')
        with self.claim_slot():
            day = today()
            if address in self.ledger.claims: raise Refused(409, 'This address has already received a faucet transfer', 'already_claimed')
            if self.ledger.count(day) >= int(self.cfg['dailyLimit']): raise Refused(429, "Today's faucet limit is reached. Try again tomorrow (UTC).", 'daily_limit')
            used = self.clients.get((day, client), 0)
            if used >= int(self.cfg['perClientDaily']): raise Refused(429, 'This connection has reached its daily faucet limit', 'client_limit')
            if int(self.rpc('eth_chainId', []), 16) != self.chain_id: raise Refused(503, 'The node is on a different network; the faucet is stopped', 'wrong_network')
            if self.genesis is not None:
                params = self.rpc('pocol_getParams', [])
                found = params.get('genesisHash') if isinstance(params, dict) else None
                if not isinstance(found, str) or found.lower() != self.genesis:
                    raise Refused(503, 'The node is on a different network; the faucet is stopped', 'wrong_network')
            if int(self.rpc('eth_getTransactionCount', [address, 'pending']), 16) != 0: raise Refused(409, 'The faucet funds only new accounts that have not sent a transaction', 'not_new_account')
            if self.rpc('eth_getCode', [address, 'latest']) not in ('0x', '0x0', ''): raise Refused(400, 'The faucet funds only ordinary accounts', 'contract_account')
            if int(self.rpc('eth_getBalance', [address, 'pending']), 16) >= self.amount: raise Refused(409, 'This account already holds enough PCOL for its first fee', 'already_funded')
            price = int(self.rpc('eth_gasPrice', []), 16); max_fee = max(2 * price, 2 * GWEI)
            held = int(self.rpc('eth_getBalance', [self.address, 'pending']), 16)
            if self.max_float is not None and held > self.max_float:
                raise Refused(503, 'The faucet operator account holds more than the faucet float; the faucet is stopped', 'over_float')
            if held < self.amount + 21000 * max_fee: raise Refused(503, 'The faucet is empty', 'empty')
            nonce = int(self.rpc('eth_getTransactionCount', [self.address, 'pending']), 16)
            raw = self.sign(['--to', address, '--value', str(self.amount), '--nonce', str(nonce), '--max-fee', str(max_fee)])
            if not re.fullmatch(r'0x[0-9a-fA-F]+', raw): raise Refused(500, 'the signer returned no transaction', 'signer_failed')
            # Durable before the send: a crash after this line can never pay the same address twice.
            self.ledger.claims[address] = {'day': day, 'nonce': nonce, 'state': 'sending'}
            try: self.ledger.save()
            except BaseException:
                # Nothing was sent, so the address may claim again (node_unavailable). A retry saves 'sending' again
                # before it sends; the rollback save is best effort.
                del self.ledger.claims[address]
                try: self.ledger.save()
                except Exception: pass
                raise
            try:
                tx = self.rpc('eth_sendRawTransaction', [raw])
            except Refused:
                # The node answered and refused: nothing was sent, so the address may claim again.
                del self.ledger.claims[address]; self.ledger.save(); raise
            except BaseException as error:
                # No answer: the transfer may be in the pool. Keep the claim; the operator checks the nonce. If even
                # this save fails, the file still holds the claim as 'sending', which refuses the address as well.
                self.ledger.claims[address]['state'] = 'unknown'
                try: self.ledger.save()
                except Exception: pass
                if not isinstance(error, Exception): raise
                raise UnknownOutcome(f'no answer to eth_sendRawTransaction: {error!r}') from error
            try:
                self.ledger.claims[address].update({'state': 'sent', 'hash': tx}); self.ledger.save()
            except Exception as error:
                # The node took the transfer; only its record failed. The file still refuses the address ('sending' or 'sent').
                raise UnknownOutcome('the transfer was submitted but the ledger could not record it') from error
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
            """The per-client key (client_key). With trustProxy, the last X-Forwarded-For entry: the route on
            explorer.pocol.net (faucet_route.py) overwrites the header with the connecting address. An entry that is
            not an IP address falls back to the peer address, so it cannot open a fresh counter."""
            peer = self.client_address[0]
            if faucet.cfg['trustProxy']:
                forwarded = ','.join(self.headers.get_all('X-Forwarded-For', []))
                try: return client_key(forwarded.split(',')[-1])
                except ValueError: pass
            try: return client_key(peer)
            except ValueError: return peer
        def reply(self, status, value, headers=()):
            body = json.dumps(value).encode()
            self.send_response(status); self.send_header('Content-Type', 'application/json'); self.send_header('Content-Length', str(len(body)))
            self.send_header('Cache-Control', 'no-store'); self.send_header('X-Content-Type-Options', 'nosniff')
            if faucet.cfg['allowOrigin']: self.send_header('Access-Control-Allow-Origin', faucet.cfg['allowOrigin'])
            for name, text in headers: self.send_header(name, text)
            try: self.end_headers(); self.wfile.write(body)
            except (BrokenPipeError, ConnectionResetError, TimeoutError): pass
        def error(self, status, code, message, headers=()):
            self.reply(status, {'type': 'error', 'code': code, 'message': message}, headers)
        def do_OPTIONS(self):
            self.send_response(204)
            if faucet.cfg['allowOrigin']:
                self.send_header('Access-Control-Allow-Origin', faucet.cfg['allowOrigin']); self.send_header('Access-Control-Allow-Methods', 'GET, POST')
                self.send_header('Access-Control-Allow-Headers', 'Content-Type'); self.send_header('Access-Control-Max-Age', '600')
            self.send_header('Content-Length', '0')
            try: self.end_headers()
            except (BrokenPipeError, ConnectionResetError, TimeoutError): pass
        def do_GET(self):
            if self.path != '/status': return self.error(404, 'not_found', 'not found')
            self.reply(200, faucet.status())
        def do_POST(self):
            try:
                if self.path != '/claim': raise Refused(404, 'not found', 'not_found')
                lengths = self.headers.get_all('Content-Length', [])
                if self.headers.get('Transfer-Encoding') is not None or len(lengths) != 1:
                    raise Refused(400, 'A single Content-Length is required; chunked requests are not supported', 'bad_request')
                if re.fullmatch(r'[0-9]{1,3}', lengths[0]) is None:
                    raise Refused(413, 'request body size is invalid', 'too_large')
                length = int(lengths[0])
                if length <= 0 or length > MAX_BODY: raise Refused(413, 'request body too large', 'too_large')
                if self.headers.get('Content-Type', '').split(';')[0].strip().lower() != 'application/json':
                    raise Refused(415, 'Use application/json', 'unsupported_media')
                raw = self.rfile.read(length)
                if len(raw) != length: raise Refused(400, 'Incomplete request body', 'bad_request')
                try: body = json.loads(raw)
                except ValueError: raise Refused(400, 'invalid JSON', 'bad_request')
                if not isinstance(body, dict) or set(body) != {'address'}: raise Refused(400, 'send {"address": "0x..."}', 'bad_request')
                self.reply(200, faucet.claim(body['address'], self.client()))
            except Refused as e:
                # Only a busy claim slot invites an automatic retry; the daily and per-client limits do not.
                self.error(e.status, e.code, e.message, [('Retry-After', '2')] if e.code == 'busy' else ())
            except RequestDeadline:
                self.error(408, 'deadline', 'Request read deadline exceeded; no claim was processed.')
            except UnknownOutcome:
                self.error(502, 'unknown_outcome', 'The faucet could not reach the node or save its response. A transfer may already have been submitted; check the address before retrying.')
            except Exception:
                # Raised before the send (claim() turns every failure from the send on into UnknownOutcome).
                self.error(502, 'node_unavailable', 'The faucet could not reach the node or write its ledger before sending. No transfer was submitted; retry later.')
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
