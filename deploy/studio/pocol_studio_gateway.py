#!/usr/bin/env python3
"""PoCol Studio's bounded public RPC relay. Raw node RPC remains loopback-only.

This endpoint accepts signed EIP-1559 transactions for the pinned test network.
It never creates accounts, signs transactions, mines, or exposes admin methods.
Python 3.10+, standard library only.
"""
import argparse
import collections
import http.client
import ipaddress
import json
import re
import socket
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

CHAIN_ID = 1599846715
GENESIS = '0xbe137bfdb4fdf332f2e1500ee1a67e0b22ce6bdf57efa46fb4ea7b0debe509ca'
MAX_REQUEST = 131072
MAX_RESPONSE = 2 * 1024 * 1024
MAX_RAW_BYTES = 60000
MAX_BATCH = 10
NO_PARAMS = {'eth_chainId', 'pocol_status', 'pocol_getParams', 'eth_blockNumber', 'eth_gasPrice'}
HASH_METHODS = {'eth_getTransactionByHash', 'eth_getTransactionReceipt'}
ACCOUNT_METHODS = {'eth_getBalance', 'eth_getTransactionCount', 'eth_getCode'}


def matches(pattern, value):
    return isinstance(value, str) and re.fullmatch(pattern, value) is not None


def quantity(value):
    return matches(r'0x(?:0|[1-9a-fA-F][0-9a-fA-F]{0,63})', value)


def tag(value):
    return value in ('latest', 'pending', 'earliest') if isinstance(value, str) and not value.startswith('0x') else quantity(value)


def hexdata(value, maximum=MAX_RAW_BYTES):
    return isinstance(value, str) and len(value) <= maximum * 2 + 2 and matches(r'0x(?:[0-9a-fA-F]{2})*', value)


def rlp_item(data, at=0, depth=0):
    """Strict, bounded RLP decoding for chain-id checking before forwarding."""
    if depth > 5 or at >= len(data): raise ValueError('RLP bounds')
    b = data[at]
    if b <= 0x7f: return data[at:at+1], at+1
    if b <= 0xb7:
        size, start = b-0x80, at+1
        end = start+size
        if end > len(data) or (size == 1 and data[start] <= 0x7f): raise ValueError('RLP string')
        return data[start:end], end
    if b <= 0xbf:
        width, start = b-0xb7, at+1
        if width > 4 or start+width > len(data) or data[start] == 0: raise ValueError('RLP length')
        size = int.from_bytes(data[start:start+width], 'big'); start += width
        if size <= 55 or start+size > len(data): raise ValueError('RLP long string')
        return data[start:start+size], start+size
    if b <= 0xf7: size, start = b-0xc0, at+1
    else:
        width, start = b-0xf7, at+1
        if width > 4 or start+width > len(data) or data[start] == 0: raise ValueError('RLP list length')
        size = int.from_bytes(data[start:start+width], 'big'); start += width
        if size <= 55: raise ValueError('RLP long list')
    end = start+size
    if end > len(data): raise ValueError('RLP list bounds')
    result = []
    while start < end:
        value, start = rlp_item(data, start, depth+1)
        if start > end or len(result) >= 1024: raise ValueError('RLP list bounds')
        result.append(value)
    return result, end


def valid_raw_transaction(value):
    if not hexdata(value) or len(value) < 22: return False
    try:
        raw = bytes.fromhex(value[2:])
        if raw[0] != 2: return False
        fields, end = rlp_item(raw, 1)
        if end != len(raw) or not isinstance(fields, list) or len(fields) != 12: return False
        if not isinstance(fields[0], bytes) or fields[0] != CHAIN_ID.to_bytes(4, 'big'): return False
        # Reject malformed integer encodings; the node verifies signature/replay rules.
        for index in (0, 1, 2, 3, 4, 6, 9, 10, 11):
            v = fields[index]
            if not isinstance(v, bytes) or len(v) > 32 or (v and v[0] == 0): return False
        if not isinstance(fields[5], bytes) or len(fields[5]) not in (0, 20): return False
        if not isinstance(fields[7], bytes) or not isinstance(fields[8], list): return False
        if fields[9] not in (b'', b'\x01'): return False
        return bool(fields[10]) and bool(fields[11])
    except (ValueError, IndexError): return False


def valid_transaction(value, *, call=False):
    if not isinstance(value, dict): return False
    allowed = {'to', 'from', 'data', 'value', 'gas', 'gasPrice', 'maxFeePerGas', 'maxPriorityFeePerGas'}
    if not value or set(value) - allowed or (call and 'to' not in value): return False
    for key, val in value.items():
        if key in ('to', 'from') and not matches(r'0x[0-9a-fA-F]{40}', val): return False
        if key == 'data' and not hexdata(val): return False
        if key not in ('to', 'from', 'data') and not quantity(val): return False
    # Limit potentially expensive eth_call/estimateGas execution.
    return 'gas' not in value or int(value['gas'], 16) <= 8_000_000


def valid_call(request):
    if not isinstance(request, dict) or set(request) != {'jsonrpc', 'id', 'method', 'params'}: return False
    if request['jsonrpc'] != '2.0': return False
    ident, method, params = request['id'], request['method'], request['params']
    if type(ident) is not int or abs(ident) > 9007199254740991 or not isinstance(params, list): return False
    if not isinstance(method, str): return False
    if method in NO_PARAMS: return not params
    if method in HASH_METHODS: return len(params) == 1 and matches(r'0x[0-9a-fA-F]{64}', params[0])
    if method in ACCOUNT_METHODS: return len(params) == 2 and matches(r'0x[0-9a-fA-F]{40}', params[0]) and tag(params[1])
    if method == 'eth_getBlockByNumber': return len(params) == 2 and tag(params[0]) and params[1] is False
    if method == 'eth_call': return len(params) == 2 and valid_transaction(params[0], call=True) and tag(params[1])
    if method == 'eth_estimateGas': return len(params) in (1, 2) and valid_transaction(params[0]) and (len(params) == 1 or tag(params[1]))
    if method == 'eth_sendRawTransaction': return len(params) == 1 and valid_raw_transaction(params[0])
    return False


def valid_payload(value):
    if not isinstance(value, list): return valid_call(value)
    return (0 < len(value) <= MAX_BATCH and all(valid_call(v) and v['method'] != 'eth_sendRawTransaction' for v in value)
            and len({v['id'] for v in value}) == len(value))


def sanitize(method, value):
    if method not in ('pocol_status', 'pocol_getParams') or not isinstance(value, dict): return value
    keys = ('chainId', 'genesisHash', 'head', 'pool', 'protocolVersion', 'compatibilitySha256',
            'profileKind', 'nodeKind', 'nonceMode', 'netKey', 'activeVersion',
            'forkSchedule', 'effectiveNonceMode', 'nextBlockNonceMode', 'consensusCompatibilitySha256',
            'networkName', 'networkEnvironment')
    return {k: value[k] for k in keys if k in value}


def error(ident, code, message):
    return {'jsonrpc': '2.0', 'id': ident, 'error': {'code': code, 'message': message}}


class Gateway:
    def __init__(self, upstream_port=8545):
        self.port = upstream_port
        self.lock = threading.Lock()
        self.identity_lock = threading.Lock()
        self.rpc_slots = threading.BoundedSemaphore(4)
        self.buckets = collections.OrderedDict()
        self.last_identity = 0.0

    def charge(self, client, cost=1, write=False):
        now = time.monotonic()
        # All callers share global budgets; per-IP buckets bound a single visitor.
        checks = [('global-read', 100, 30, cost), (('client', client), 40, 10, cost)]
        if write: checks += [('global-write', 12, 2, 1), (('write', client), 6, 1, 1)]
        with self.lock:
            updated = []
            for key, capacity, rate, amount in checks:
                old, at = self.buckets.get(key, (capacity, now))
                available = min(capacity, old + (now-at)*rate)
                if available < amount: return False
                updated.append((key, available-amount))
            for key, remaining in updated:
                self.buckets[key] = (remaining, now); self.buckets.move_to_end(key)
            # Keep global buckets even when many unique public clients arrive.
            while len(self.buckets) > 4096:
                key = next(k for k in self.buckets if k not in ('global-read', 'global-write'))
                self.buckets.pop(key)
        return True

    def query(self, method, params):
        conn = http.client.HTTPConnection('127.0.0.1', self.port, timeout=8)
        try:
            body = json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': method, 'params': params}, separators=(',', ':')).encode()
            conn.request('POST', '/', body, {'Content-Type': 'application/json', 'Connection': 'close'})
            reply = conn.getresponse()
            raw = reply.read(MAX_RESPONSE+1)
            if reply.status != 200 or len(raw) > MAX_RESPONSE: raise RuntimeError('Node response limit/status')
            answer = json.loads(raw)
            if (not isinstance(answer, dict) or answer.get('jsonrpc') != '2.0' or answer.get('id') != 1
                    or (('result' in answer) == ('error' in answer))): raise RuntimeError('Invalid node response')
            return answer
        finally: conn.close()

    def verify_network(self, *, force=False):
        with self.identity_lock:
            if not force and time.monotonic()-self.last_identity < 10: return
            value = self.query('pocol_status', []).get('result', {})
            chain = self.query('eth_chainId', []).get('result')
            if (not isinstance(value, dict) or value.get('chainId') != CHAIN_ID
                    or value.get('genesisHash') != GENESIS or chain != hex(CHAIN_ID)):
                self.last_identity = 0
                raise RuntimeError('Unexpected network identity')
            self.last_identity = time.monotonic()

    def call(self, request):
        ident, method, params = request['id'], request['method'], request['params']
        if not self.rpc_slots.acquire(blocking=False): return error(ident, -32021, 'Studio gateway busy; retry shortly')
        try:
            self.verify_network(force=method == 'eth_sendRawTransaction')
            # Clamp default execution gas even if the caller omits the field.
            if method in ('eth_call', 'eth_estimateGas'):
                params = [{**params[0], 'gas': params[0].get('gas', '0x7a1200')}, *params[1:]]
            answer = self.query(method, params)
            if 'result' in answer: answer['result'] = sanitize(method, answer['result'])
            if 'error' in answer:
                upstream = answer['error']
                code = upstream.get('code', -32000) if isinstance(upstream, dict) else -32000
                message = upstream.get('message', 'Node rejected request') if isinstance(upstream, dict) else 'Node rejected request'
                answer['error'] = {'code': code if type(code) is int else -32000, 'message': str(message)[:300]}
            answer['id'] = ident
            return answer
        except (OSError, ValueError, RuntimeError, RecursionError, http.client.HTTPException):
            # Never retry writes here: a timed-out broadcast may have reached the node.
            return error(ident, -32000, 'Node unavailable or network identity mismatch; check transaction hash before resubmitting')
        finally: self.rpc_slots.release()


class Handler(BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'
    server_version = 'PoColStudio/1.0'
    sys_version = ''

    def setup(self):
        super().setup(); self.connection.settimeout(12)

    def log_message(self, *_args): pass

    def reply(self, status, value):
        body = json.dumps(value, separators=(',', ':')).encode()
        if len(body) > MAX_RESPONSE: status, body = 413, b'{"error":"Response limit exceeded"}'
        self.send_response(status)
        self.send_header('Content-Type', 'application/json')
        self.send_header('Content-Length', str(len(body)))
        self.send_header('Cache-Control', 'no-store')
        self.send_header('X-Content-Type-Options', 'nosniff')
        # This is a public, credential-free relay. CORS is not authentication.
        self.send_header('Access-Control-Allow-Origin', '*')
        self.send_header('Connection', 'close')
        if status == 429: self.send_header('Retry-After', '2')
        self.end_headers(); self.close_connection = True
        try: self.wfile.write(body)
        except (BrokenPipeError, ConnectionResetError, socket.timeout): pass

    def client_key(self):
        # Caddy overwrites this header with the actual remote IP. The listener is loopback-only.
        value = self.headers.get('X-Pocol-Client-IP', self.client_address[0])
        try: return str(ipaddress.ip_address(value))
        except ValueError: return 'unknown'

    def do_OPTIONS(self):
        if self.path != '/studio/rpc': return self.reply(404, {'error': 'Not found'})
        self.send_response(204)
        self.send_header('Access-Control-Allow-Origin', '*')
        self.send_header('Access-Control-Allow-Methods', 'POST, OPTIONS')
        self.send_header('Access-Control-Allow-Headers', 'Content-Type')
        self.send_header('Access-Control-Max-Age', '600')
        self.send_header('Content-Length', '0'); self.end_headers()

    def do_GET(self):
        if self.path != '/studio/health': return self.reply(404, {'error': 'Not found'})
        if not self.server.gateway.charge(self.client_key()): return self.reply(429, {'error': 'Request budget exceeded'})
        try:
            self.server.gateway.verify_network()
            self.reply(200, {'service': 'pocol-studio-gateway', 'ready': True, 'chainId': CHAIN_ID,
                             'genesisHash': GENESIS, 'rpcPath': '/studio/rpc', 'trust': 'remote-node',
                             'signedTransactions': True})
        except (OSError, ValueError, RuntimeError, http.client.HTTPException):
            self.reply(503, {'service': 'pocol-studio-gateway', 'ready': False})

    def do_POST(self):
        if self.path != '/studio/rpc': return self.reply(404, {'error': 'Not found'})
        if self.headers.get('Transfer-Encoding') or len(self.headers.get_all('Content-Length', [])) != 1:
            return self.reply(400, {'error': 'A single Content-Length is required'})
        try: size = int(self.headers.get('Content-Length', '0'))
        except ValueError: size = 0
        if not 0 < size <= MAX_REQUEST: return self.reply(413, {'error': 'Request size limit'})
        if self.headers.get('Content-Type', '').split(';')[0].strip().lower() != 'application/json':
            return self.reply(415, {'error': 'Use application/json'})
        try:
            raw = self.rfile.read(size)
            if len(raw) != size: raise ValueError()
            value = json.loads(raw)
            if not valid_payload(value): raise ValueError()
        except (ValueError, OSError, RecursionError): return self.reply(400, {'error': 'Unsupported or invalid Studio RPC request'})
        items = value if isinstance(value, list) else [value]
        write = len(items) == 1 and items[0]['method'] == 'eth_sendRawTransaction'
        if not self.server.gateway.charge(self.client_key(), len(items), write=write):
            return self.reply(429, {'error': 'Request budget exceeded; retry shortly'})
        answers, size = [], 2
        for item in items:
            answer = self.server.gateway.call(item); size += len(json.dumps(answer)) + 1
            if size > MAX_RESPONSE: return self.reply(413, {'error': 'Response limit exceeded'})
            answers.append(answer)
        return self.reply(200, answers if isinstance(value, list) else answers[0])


class Server(ThreadingHTTPServer):
    daemon_threads = True
    request_queue_size = 16

    def __init__(self, address, gateway):
        self.gateway = gateway
        self.slots = threading.BoundedSemaphore(16)
        if address[0] != '127.0.0.1': raise ValueError('The relay listener must stay loopback-only')
        super().__init__(address, Handler)

    def process_request(self, request, client_address):
        if not self.slots.acquire(blocking=False): self.shutdown_request(request); return
        try: super().process_request(request, client_address)
        except BaseException: self.slots.release(); raise

    def process_request_thread(self, request, client_address):
        try: super().process_request_thread(request, client_address)
        finally: self.slots.release()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--port', type=int, default=8089)
    parser.add_argument('--rpc-port', type=int, default=8545)
    args = parser.parse_args()
    if not 1 <= args.port <= 65535 or not 1 <= args.rpc_port <= 65535: parser.error('Invalid port')
    gateway = Gateway(args.rpc_port); gateway.verify_network(force=True)
    server = Server(('127.0.0.1', args.port), gateway)
    print('PoCol Studio gateway ready on loopback; expected testnet identity verified', flush=True)
    server.serve_forever(poll_interval=.5)


if __name__ == '__main__': main()
