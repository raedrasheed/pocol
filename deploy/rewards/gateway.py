#!/usr/bin/env python3
"""Bounded read-only PoCol explorer gateway. Python 3.10+, standard library only."""
import argparse
import collections
import http.client
import json
import re
import socket
import threading
import time
import sys
import os
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parent))
import sqlite3
try:
    from pocol_geoip import Geography
except ImportError:
    Geography = None
try:
    from pocol_transactions import TransactionIndex, SnapshotChanged, valid_options
except ImportError:
    TransactionIndex = None
    class SnapshotChanged(RuntimeError): pass
    def valid_options(_): return False
try:
    from pocol_rewards import (snapshot as reward_snapshot, validated_fork_schedule,
                               DEFAULT_FORK_SCHEDULE)
except ImportError:
    reward_snapshot = None
    validated_fork_schedule = None
    DEFAULT_FORK_SCHEDULE = ((1, 0),)
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

CHAIN_ID = 1599846715
GENESIS = '0xbe137bfdb4fdf332f2e1500ee1a67e0b22ce6bdf57efa46fb4ea7b0debe509ca'
MAX_REQUEST = 16384
MAX_RESPONSE = 4 * 1024 * 1024
NO_PARAMS = {'pocol_status', 'pocol_getParams', 'pocol_getMiners', 'eth_blockNumber', 'net_peerCount', 'eth_gasPrice', 'pocol_getNetworkGeography', 'pocol_getBlockReward'}
HASH_METHODS = {'eth_getTransactionByHash', 'eth_getTransactionReceipt'}
ACCOUNT_METHODS = {'eth_getBalance', 'eth_getTransactionCount', 'eth_getCode'}

def trusted_fork_schedule(value):
    # Preserve the gateway's optional reward module in legacy-only installations.
    if validated_fork_schedule is None:
        if value != DEFAULT_FORK_SCHEDULE:
            raise RuntimeError('Install the reward module before configuring a fork schedule')
        return DEFAULT_FORK_SCHEDULE
    return validated_fork_schedule(value)


def load_fork_schedule(filename=None):
    """Read operator configuration once at startup; never derive it from RPC."""
    if filename is None:
        return DEFAULT_FORK_SCHEDULE
    with open(filename, 'rb') as source:
        raw = source.read(4097)
    if len(raw) > 4096:
        raise ValueError('Fork schedule file exceeds 4096 bytes')
    return trusted_fork_schedule(json.loads(raw))


def matches(pattern, value):
    return isinstance(value, str) and re.fullmatch(pattern, value) is not None

def valid_call(request):
    if not isinstance(request, dict) or request.get('jsonrpc') != '2.0': return False
    ident, method, params = request.get('id'), request.get('method'), request.get('params')
    if type(ident) is not int or abs(ident) > 9007199254740991 or not isinstance(params, list): return False
    if not isinstance(method, str): return False
    tag = lambda v: v == 'latest' or matches(r'0x[0-9a-fA-F]{1,16}', v)
    hash_value = lambda v: matches(r'0x[0-9a-fA-F]{64}', v)
    if method in NO_PARAMS: return len(params) == 0
    if method == 'pocol_getTransactions': return len(params) == 1 and valid_options(params[0])
    if method in HASH_METHODS: return len(params) == 1 and hash_value(params[0])
    if method in ACCOUNT_METHODS: return len(params) == 2 and matches(r'0x[0-9a-fA-F]{40}', params[0]) and tag(params[1])
    if method == 'eth_getBlockByNumber': return len(params) == 2 and tag(params[0]) and type(params[1]) is bool
    if method == 'eth_getBlockByHash': return len(params) == 2 and hash_value(params[0]) and type(params[1]) is bool
    return False

def valid_payload(value):
    return (0 < len(value) <= 20 and all(valid_call(v) for v in value)) if isinstance(value, list) else valid_call(value)

def sanitize(method, value):
    if not isinstance(value, dict): return value
    if method == 'pocol_status':
        keys = ('chainId', 'genesisHash', 'head', 'pool', 'lifetime', 'protocolVersion', 'compatibilitySha256',
                'forkSchedule', 'effectiveNonceMode', 'nextBlockNonceMode', 'consensusCompatibilitySha256',
                'networkName', 'networkEnvironment')
        result = {k:value.get(k) for k in keys}
        result['peerCount'] = len(value.get('peers', []))
        result['stats'] = {k:value.get('stats', {}).get(k) for k in ('reorgs', 'mined', 'imported')}
        return result
    if method == 'pocol_getParams':
        keys = ('chainId', 'genesisHash', 'profileKind', 'nodeKind', 'nonceMode', 'netKey', 'protocolVersion', 'compatibilitySha256', 'activeVersion', 'forkSchedule', 'effectiveNonceMode', 'nextBlockNonceMode', 'consensusCompatibilitySha256', 'networkName', 'networkEnvironment')
        return {k:value.get(k) for k in keys}
    return value

class Gateway:
    def __init__(self, upstream_port=8545, geography=None, transactions=None, *, fork_schedule=DEFAULT_FORK_SCHEDULE):
        self.fork_schedule = trusted_fork_schedule(fork_schedule)
        self.port = upstream_port
        self.geography = geography
        self.transactions = transactions
        self.cache = collections.OrderedDict()
        self.cache_size = 0
        self.lock = threading.Lock()
        self.rpc_slots = threading.BoundedSemaphore(4)
        self.reward_slot = threading.Lock()
        self.reward_failed_at = -float('inf')
        self.tokens, self.token_time = 80.0, time.monotonic()
    def charge(self, cost):
        with self.lock:
            now = time.monotonic()
            self.tokens = min(80, self.tokens + (now-self.token_time)*20)
            self.token_time = now
            if self.tokens < cost: return False
            self.tokens -= cost
            return True
    def query(self, method, params):
        conn = http.client.HTTPConnection('127.0.0.1', self.port, timeout=5)
        try:
            body = json.dumps({'jsonrpc':'2.0','id':1,'method':method,'params':params}, separators=(',', ':')).encode()
            conn.request('POST', '/', body=body, headers={'Content-Type':'application/json','Connection':'close'})
            reply = conn.getresponse()
            if reply.status != 200: raise RuntimeError('Node temporarily unavailable')
            raw = reply.read(MAX_RESPONSE + 1)
            if len(raw) > MAX_RESPONSE: raise RuntimeError('Node response exceeds explorer limit')
            result = json.loads(raw)
            if not isinstance(result, dict) or result.get('id') != 1: raise RuntimeError('Invalid node response')
            return result
        finally: conn.close()
    def verify_network(self):
        params = self.query('pocol_getParams', []).get('result', {})
        if params.get('chainId') != CHAIN_ID or params.get('genesisHash', '').lower() != GENESIS:
            raise RuntimeError('The local RPC does not belong to the configured PoCol network')
    def call(self, request):
        method, params = request['method'], request['params']
        key = json.dumps([method, params], separators=(',', ':'))
        now = time.monotonic()
        with self.lock:
            item = self.cache.get(key)
            if item and item[0] > now:
                self.cache.move_to_end(key)
                answer = json.loads(item[1]); answer['id'] = request['id']; return answer
        if not self.rpc_slots.acquire(blocking=False):
            return {'jsonrpc':'2.0','id':request['id'],'error':{'code':-32021,'message':'Explorer busy; retry shortly'}}
        reward_locked = False
        try:
            if method == 'pocol_getBlockReward':
                if reward_snapshot is None: raise RuntimeError('Reward summary is not installed')
                if time.monotonic() - self.reward_failed_at < 3:
                    raise RuntimeError('Reward summary temporarily unavailable')
                reward_locked = self.reward_slot.acquire(blocking=False)
                if not reward_locked:
                    return {'jsonrpc':'2.0','id':request['id'],'error':{'code':-32021,'message':'Reward snapshot is being prepared; retry shortly'}}
                deadline, remaining = time.monotonic() + 20, 512
                def bounded_reward_query(read_method, read_params):
                    nonlocal remaining
                    if remaining <= 0 or time.monotonic() >= deadline:
                        raise RuntimeError('Reward read budget exceeded')
                    if read_method not in {'pocol_status', 'pocol_getParams', 'eth_getBlockByNumber', 'eth_getBlockByHash', 'pocol_getBlockRlp', 'eth_call', 'eth_getCode', 'eth_getBalance', 'eth_getTransactionCount'}:
                        raise RuntimeError('Unsupported reward read')
                    remaining -= 1
                    return self.query(read_method, read_params)
                try:
                    value = reward_snapshot(bounded_reward_query, fork_schedule=self.fork_schedule)
                except Exception:
                    self.reward_failed_at = time.monotonic()
                    raise
                answer = {'jsonrpc':'2.0','id':request['id'],'result':value}
            elif method == 'pocol_getTransactions':
                if self.transactions is None: raise RuntimeError('Transaction index is not installed')
                answer = {'jsonrpc':'2.0','id':request['id'],'result':self.transactions.page(params[0])}
            elif method == 'pocol_getNetworkGeography':
                if self.geography is None: raise RuntimeError('Automatic GeoIP is not installed')
                raw_status = self.query('pocol_status', []).get('result')
                answer = {'jsonrpc':'2.0','id':request['id'],'result':self.geography.snapshot(raw_status)}
            else:
                answer = self.query(method, params)
            if 'result' in answer: answer['result'] = sanitize(method, answer['result'])
            answer['id'] = request['id']
            if 'error' not in answer:
                encoded = json.dumps(answer, separators=(',', ':')).encode()
                with self.lock:
                    previous = self.cache.pop(key, None)
                    if previous: self.cache_size -= len(previous[1])
                    self.cache[key] = (time.monotonic()+3, encoded)
                    self.cache_size += len(encoded)
                    while len(self.cache)>256 or self.cache_size>16*1024*1024:
                        _, removed = self.cache.popitem(last=False); self.cache_size -= len(removed[1])
            return answer
        except SnapshotChanged:
            return {'jsonrpc':'2.0','id':request['id'],'error':{'code':-32022,'message':'Chain changed. Refresh transactions.'}}
        except (OSError, ValueError, RuntimeError, KeyError, sqlite3.Error, http.client.HTTPException):
            return {'jsonrpc':'2.0','id':request['id'],'error':{'code':-32000,'message':'Node data temporarily unavailable'}}
        finally:
            if reward_locked: self.reward_slot.release()
            self.rpc_slots.release()

class Handler(BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'
    server_version = 'PoColExplorer/1.0'
    sys_version = ''
    def setup(self):
        super().setup(); self.connection.settimeout(6)
    def log_message(self, *_args): pass
    def reply(self, status, value):
        body = json.dumps(value, separators=(',', ':')).encode()
        if len(body) > MAX_RESPONSE:
            status, body = 413, b'{"error":"Explorer response limit exceeded"}'
        self.send_response(status)
        self.send_header('Content-Type','application/json')
        self.send_header('Content-Length',str(len(body)))
        self.send_header('Cache-Control','no-store')
        self.send_header('X-Content-Type-Options','nosniff')
        self.send_header('Connection','close')
        self.end_headers()
        self.close_connection = True
        try: self.wfile.write(body)
        except (BrokenPipeError, ConnectionResetError, socket.timeout): pass
    def do_GET(self):
        if self.path == '/health':
            self.reply(200, {'service':'pocol-explorer-gateway','readOnly':True,'chainId':CHAIN_ID,'note':'Service health only; the explorer checks upstream identity and freshness.'})
        else: self.reply(404, {'error':'Not found'})
    def do_POST(self):
        if self.path != '/rpc': self.reply(404, {'error':'Not found'}); return
        if self.headers.get('Transfer-Encoding') or len(self.headers.get_all('Content-Length', [])) != 1:
            self.reply(400, {'error':'A single Content-Length is required'}); return
        try: size = int(self.headers.get('Content-Length', '0'))
        except ValueError: size = 0
        if not 0 < size <= MAX_REQUEST: self.reply(413, {'error':'Request size limit'}); return
        if self.headers.get('Content-Type','').split(';')[0].strip().lower() != 'application/json':
            self.reply(415, {'error':'Use application/json'}); return
        try:
            raw = self.rfile.read(size)
            if len(raw) != size: raise ValueError()
            value = json.loads(raw)
            if not valid_payload(value): raise ValueError()
        except (ValueError, OSError): self.reply(400, {'error':'Unsupported explorer read'}); return
        items = value if isinstance(value, list) else [value]
        if not self.server.gateway.charge(sum(20 if item['method'] == 'pocol_getBlockReward' else 1 for item in items)): self.reply(429, {'error':'Explorer request budget exceeded; retry shortly'}); return
        answers, size = [], 2
        for item in items:
            answer = self.server.gateway.call(item)
            size += len(json.dumps(answer)) + 1
            if size > MAX_RESPONSE: self.reply(413, {'error':'Explorer response limit exceeded'}); return
            answers.append(answer)
        self.reply(200, answers if isinstance(value, list) else answers[0])

class Server(ThreadingHTTPServer):
    daemon_threads = True
    request_queue_size = 16
    def __init__(self, address, gateway):
        self.gateway = gateway
        self.slots = threading.BoundedSemaphore(16)
        super().__init__(address, Handler)
    def process_request(self, request, client_address):
        if not self.slots.acquire(blocking=False): self.shutdown_request(request); return
        try: super().process_request(request, client_address)
        except BaseException: self.slots.release(); raise
    def process_request_thread(self, request, client_address):
        try: super().process_request_thread(request, client_address)
        finally: self.slots.release()

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bind', default='127.0.0.1')
    parser.add_argument('--port', type=int, default=8088)
    parser.add_argument('--rpc-port', type=int, default=8545)
    parser.add_argument('--fork-schedule', metavar='FILE', help='Trusted JSON [[1,0],[2,activation_height]] schedule; default is version 1 only')
    args = parser.parse_args()
    try:
        fork_schedule = load_fork_schedule(args.fork_schedule)
    except (OSError, ValueError, RuntimeError) as error:
        parser.error(str(error))
    geography = Geography.from_config() if Geography is not None and Path('/opt/pocol-explorer-gateway/geoip-config.json').is_file() else None
    gateway = Gateway(args.rpc_port, geography, fork_schedule=fork_schedule)
    gateway.verify_network()
    if os.environ.get('POCOL_TX_DB'):
        if TransactionIndex is None: raise RuntimeError('Transaction index module is missing')
        gateway.transactions = TransactionIndex(os.environ['POCOL_TX_DB'], gateway.query)
        gateway.transactions.start()
    server = Server((args.bind, args.port), gateway)
    print('PoCol explorer read-only gateway ready', flush=True)
    server.serve_forever(poll_interval=0.5)
