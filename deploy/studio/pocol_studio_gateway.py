#!/usr/bin/env python3
"""PoCol Studio's bounded public RPC relay. Raw node RPC remains loopback-only.

This endpoint accepts signed EIP-1559 transactions for the pinned PoCol network, or for the network a trusted
--network file names (the explorer gateway's --v3-network format). It never creates accounts, signs transactions,
mines, or exposes admin methods. Site chunks (code of at most 24,576 bytes starting 0x00) read at `latest` or `safe`
are answered from a bounded in-memory cache filled only from the node's `safe` state, with their own request budget
and reserved upstream slots. Python 3.10+, standard library only.
"""
import argparse
import collections
import http.client
import ipaddress
import json
import math
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
# pocol_getCaps (FN-1, C9): the chain caps' occupancy, read by Studio and the extension before a publication is signed.
NO_PARAMS = {'eth_chainId', 'pocol_status', 'pocol_getParams', 'eth_blockNumber', 'eth_gasPrice', 'pocol_getCaps',
             'eth_maxPriorityFeePerGas'}
# Answered by the gateway itself: no upstream call, and the node's version is never exposed.
LOCAL = {'net_version', 'web3_clientVersion'}
CLIENT_VERSION = 'PoColStudioGateway/1.0'
CAPS_FIELDS = ('number', 'hash', 'v3', 'cumulative', 'alarmPercent', 'alarm', 'occupancy',
               'accounts', 'accountsCap', 'storageSlots', 'storageSlotsCap', 'codeBytes', 'codeBytesCap',
               'txCount', 'txCap', 'txBytes', 'txBytesCap', 'logBytes', 'logCap')
HASH_METHODS = {'eth_getTransactionByHash', 'eth_getTransactionReceipt'}
ACCOUNT_METHODS = {'eth_getBalance', 'eth_getTransactionCount', 'eth_getCode'}
# The extension's bridge schema (src/lib/bridge.js ALLOW) and LogClient (sw.js LOG_RANGE_MAX).
FEE_HISTORY_MAX = 128
PERCENTILES_MAX = 16
LOG_RANGE_MAX = 1024
LOG_LIST_MAX = 16
# Site chunks: ChunkFactory runtime code is 0x00 followed by at most 24,575 bytes (site.js CHUNK_MAX). Under CANCUN
# (EIP-6780) code at an address never changes once created, so a chunk present at `safe` stays until a reorg deeper
# than `safe` (an alarm condition); viewers verify every chunk's factory address, length and keccak anyway.
CHUNK_CODE_MAX = 24576
CHUNK_TAGS = ('latest', 'safe')
CHUNK_CACHE_BYTES = 24 * 1024 * 1024
CHUNK_ENTRY_OVERHEAD = 256   # counted per entry so tiny chunks cannot grow the entry count without bound
CHUNK_SLOTS = 2              # upstream slots reserved for chunk misses, separate from the 4 general ones
GENERAL_SLOTS = 4
IDENTITY_TTL = 10
# Token buckets (capacity, refill per second). A 4 MiB site is about 175 chunk reads: one client opens one within the
# per-client burst and then about one more every three seconds; the global bucket admits several viewers at once.
# Owner question Q8 (viewing target) sizes the final numbers.
READ_BUCKETS = (('global-read', 100, 30), ('client', 40, 10))
WRITE_BUCKETS = (('global-write', 12, 2), ('write', 6, 1))
CHUNK_BUCKETS = (('global-chunk', 1200, 300), ('chunk', 240, 60))
GLOBAL_BUCKETS = ('global-read', 'global-write', 'global-chunk')
LOGS_COST = 4
NETWORK_FILE_MAX = 4096


def matches(pattern, value):
    return isinstance(value, str) and re.fullmatch(pattern, value) is not None


def quantity(value):
    return matches(r'0x(?:0|[1-9a-fA-F][0-9a-fA-F]{0,63})', value)


def block_number(value):
    return matches(r'0x(?:0|[1-9a-fA-F][0-9a-fA-F]{0,15})', value)


def tag(value):
    if isinstance(value, str) and not value.startswith('0x'):
        return value in ('latest', 'pending', 'earliest', 'safe', 'finalized')
    return quantity(value)


def hexdata(value, maximum=MAX_RAW_BYTES):
    return isinstance(value, str) and len(value) <= maximum * 2 + 2 and matches(r'0x(?:[0-9a-fA-F]{2})*', value)


def address(value):
    return matches(r'0x[0-9a-fA-F]{40}', value)


def hash32(value):
    return matches(r'0x[0-9a-fA-F]{64}', value)


def canonical_chain_id(value, chain_id):
    """An RLP chain-id field equals chain_id in minimal encoding: 1..8 bytes, no leading zero (the extension signs with
    minimal RLP, so any id below 2^24 is shorter than four bytes)."""
    return isinstance(value, bytes) and 0 < len(value) <= 8 and value[0] != 0 and int.from_bytes(value, 'big') == chain_id


def chain_id_quantity(value, chain_id):
    """eth_chainId's answer: a strict hex quantity equal to chain_id."""
    return quantity(value) and int(value, 16) == chain_id


def same_genesis(value, genesis):
    return hash32(value) and value.lower() == genesis.lower()


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


def valid_raw_transaction(value, chain_id=CHAIN_ID):
    if not hexdata(value) or len(value) < 22: return False
    try:
        raw = bytes.fromhex(value[2:])
        if raw[0] != 2: return False
        fields, end = rlp_item(raw, 1)
        if end != len(raw) or not isinstance(fields, list) or len(fields) != 12: return False
        if not canonical_chain_id(fields[0], chain_id): return False
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
        if key in ('to', 'from') and not address(val): return False
        if key == 'data' and not hexdata(val): return False
        if key not in ('to', 'from', 'data') and not quantity(val): return False
    # Limit potentially expensive eth_call/estimateGas execution.
    return 'gas' not in value or int(value['gas'], 16) <= 8_000_000


def valid_percentiles(value):
    """eth_feeHistory reward percentiles as the bridge allows them: at most 16 finite numbers in 0..100, non-decreasing."""
    if not isinstance(value, list) or len(value) > PERCENTILES_MAX: return False
    previous = 0
    for v in value:
        if type(v) not in (int, float) or not math.isfinite(v) or not previous <= v <= 100: return False
        previous = v
    return True


def valid_log_filter(value):
    """pocol_getLogs (what the extension's eth_getLogs maps to): {blockHash, address?, topics?} or a numeric
    {fromBlock, toBlock, address?, topics?} range of at most LOG_RANGE_MAX blocks."""
    if not isinstance(value, dict): return False
    by_hash = 'blockHash' in value
    allowed = {'blockHash', 'address', 'topics'} if by_hash else {'fromBlock', 'toBlock', 'address', 'topics'}
    if set(value) - allowed: return False
    if by_hash:
        if not hash32(value['blockHash']): return False
    else:
        start, end = value.get('fromBlock'), value.get('toBlock')
        if not block_number(start) or not block_number(end): return False
        start, end = int(start, 16), int(end, 16)
        if end < start or end - start + 1 > LOG_RANGE_MAX: return False
    if 'address' in value:
        a = value['address']
        if not (address(a) or (isinstance(a, list) and len(a) <= LOG_LIST_MAX and all(map(address, a)))): return False
    if 'topics' in value:
        t = value['topics']
        if not isinstance(t, list) or len(t) > 4: return False
        for x in t:
            if not (x is None or hash32(x) or (isinstance(x, list) and len(x) <= LOG_LIST_MAX and all(map(hash32, x)))):
                return False
    return True


def valid_call(request, chain_id=CHAIN_ID):
    if not isinstance(request, dict) or set(request) != {'jsonrpc', 'id', 'method', 'params'}: return False
    if request['jsonrpc'] != '2.0': return False
    ident, method, params = request['id'], request['method'], request['params']
    if type(ident) is not int or abs(ident) > 9007199254740991 or not isinstance(params, list): return False
    if not isinstance(method, str): return False
    if method in NO_PARAMS or method in LOCAL: return not params
    if method in HASH_METHODS: return len(params) == 1 and hash32(params[0])
    if method in ACCOUNT_METHODS: return len(params) == 2 and address(params[0]) and tag(params[1])
    if method == 'eth_getStorageAt':
        return len(params) == 3 and address(params[0]) and matches(r'0x[0-9a-fA-F]{1,64}', params[1]) and tag(params[2])
    if method == 'eth_getBlockByNumber': return len(params) == 2 and tag(params[0]) and params[1] is False
    if method == 'eth_getBlockByHash': return len(params) == 2 and hash32(params[0]) and params[1] is False
    if method == 'eth_feeHistory':
        return (len(params) == 3 and block_number(params[0]) and int(params[0], 16) <= FEE_HISTORY_MAX
                and tag(params[1]) and valid_percentiles(params[2]))
    if method == 'pocol_getLogs': return len(params) == 1 and valid_log_filter(params[0])
    if method == 'eth_call': return len(params) == 2 and valid_transaction(params[0], call=True) and tag(params[1])
    if method == 'eth_estimateGas': return len(params) in (1, 2) and valid_transaction(params[0]) and (len(params) == 1 or tag(params[1]))
    if method == 'eth_sendRawTransaction': return len(params) == 1 and valid_raw_transaction(params[0], chain_id)
    return False


def valid_payload(value, chain_id=CHAIN_ID):
    if not isinstance(value, list): return valid_call(value, chain_id)
    return (0 < len(value) <= MAX_BATCH and all(valid_call(v, chain_id) and v['method'] != 'eth_sendRawTransaction' for v in value)
            and len({v['id'] for v in value}) == len(value))


def classify(request):
    """'local' (answered here), 'chunk' (eth_getCode at latest/safe: the chunk budget, cache and reserved slots),
    'write' (eth_sendRawTransaction) or 'read'. Every item of a request is classified before anything is charged."""
    method = request['method']
    if method in LOCAL: return 'local'
    if method == 'eth_getCode' and request['params'][1] in CHUNK_TAGS: return 'chunk'
    if method == 'eth_sendRawTransaction': return 'write'
    return 'read'


def read_cost(request):
    return LOGS_COST if request['method'] == 'pocol_getLogs' else 1


def chunk_code(value):
    """The raw bytes of a chunk-shaped eth_getCode answer (0x00 first, at most CHUNK_CODE_MAX bytes), else None.
    Empty code (0x) is never a chunk, so it is never cached."""
    if not hexdata(value, CHUNK_CODE_MAX) or len(value) < 4: return None
    raw = bytes.fromhex(value[2:])
    return raw if raw[0] == 0 else None


def sanitize(method, value):
    if method == 'pocol_getCaps' and isinstance(value, dict): return {k: value[k] for k in CAPS_FIELDS if k in value}
    if method not in ('pocol_status', 'pocol_getParams') or not isinstance(value, dict): return value
    keys = ('chainId', 'genesisHash', 'head', 'pool', 'protocolVersion', 'compatibilitySha256',
            'profileKind', 'nodeKind', 'nonceMode', 'netKey', 'activeVersion',
            'forkSchedule', 'effectiveNonceMode', 'nextBlockNonceMode', 'consensusCompatibilitySha256',
            'networkName', 'networkEnvironment',
            # The public consensus identity and wire range (VA-SPEC §2; A2-10).
            'consensusIdentity', 'consensusRulesVersion', 'protocolVersionMin', 'maxForkVersion')
    return {k: value[k] for k in keys if k in value}


def error(ident, code, message):
    return {'jsonrpc': '2.0', 'id': ident, 'error': {'code': code, 'message': message}}


def _no_duplicates(pairs):
    keys = [k for k, _ in pairs]
    if len(keys) != len(set(keys)): raise ValueError('Duplicate key')
    return dict(pairs)


def _no_constant(name):
    raise ValueError('Non-finite number ' + name)


def strict_json(raw):
    """JSON without NaN/Infinity constants or duplicate object keys."""
    return json.loads(raw, parse_constant=_no_constant, object_pairs_hook=_no_duplicates)


def validated_network(value):
    """A trusted network file in the explorer gateway's --v3-network format: {"chainId": N, "genesisHash": "0x<64 lower
    hex>", "specVersion": 2, "v3": true} and optionally "source" (an https URL). Returns (chain_id, genesis)."""
    keys = {'chainId', 'genesisHash', 'specVersion', 'v3'}
    if not isinstance(value, dict) or not keys <= set(value) <= keys | {'source'}:
        raise ValueError('Invalid trusted network configuration')
    chain_id, genesis = value['chainId'], value['genesisHash']
    if type(chain_id) is not int or not 0 < chain_id < 1 << 53: raise ValueError('Invalid trusted network chain ID')
    if not matches(r'0x[0-9a-f]{64}', genesis) or genesis == GENESIS: raise ValueError('Invalid trusted network genesis hash')
    if type(value['specVersion']) is not int or value['specVersion'] != 2 or value['v3'] is not True:
        raise ValueError('Only a spec-2 (v3) network is configured explicitly')
    source = value.get('source')
    if source is not None and not (isinstance(source, str) and len(source) <= 200 and
                                   matches(r'https://[A-Za-z0-9.-]+(?::[0-9]{1,5})?(?:/[A-Za-z0-9._~/-]*)?', source)):
        raise ValueError('Invalid trusted network source')
    return chain_id, genesis


def load_network(filename=None):
    """Read the operator's --network file once at startup; None keeps the pinned CHAIN_ID/GENESIS."""
    if filename is None: return None
    with open(filename, 'rb') as source:
        raw = source.read(NETWORK_FILE_MAX + 1)
    if len(raw) > NETWORK_FILE_MAX: raise ValueError('Network file exceeds 4096 bytes')
    return validated_network(strict_json(raw))


class ChunkCache:
    """Raw chunk bytes by lowercase address, LRU-bounded at `limit` bytes (each entry also counts
    CHUNK_ENTRY_OVERHEAD), in memory only and bound to one verified genesis hash: binding another identity or
    flushing drops everything, and a fill started before a flush (an older epoch) is discarded."""

    def __init__(self, limit=CHUNK_CACHE_BYTES):
        self.limit = limit
        self.lock = threading.Lock()
        self.entries = collections.OrderedDict()
        self.size = 0
        self.identity = None
        self.epoch = 0
        self.hits = 0

    def bind(self, identity):
        with self.lock:
            if identity != self.identity:
                self.entries.clear(); self.size = 0; self.identity = identity; self.epoch += 1
            return self.epoch

    def flush(self):
        with self.lock:
            self.entries.clear(); self.size = 0; self.identity = None; self.epoch += 1

    def get(self, identity, key):
        with self.lock:
            if identity is None or identity != self.identity: return None
            value = self.entries.get(key)
            if value is not None:
                self.entries.move_to_end(key); self.hits += 1
            return value

    def put(self, epoch, key, code):
        cost = len(code) + CHUNK_ENTRY_OVERHEAD
        with self.lock:
            if epoch != self.epoch or self.identity is None or cost > self.limit: return False
            old = self.entries.pop(key, None)
            if old is not None: self.size -= len(old) + CHUNK_ENTRY_OVERHEAD
            self.entries[key] = code; self.size += cost
            while self.size > self.limit:
                _, removed = self.entries.popitem(last=False); self.size -= len(removed) + CHUNK_ENTRY_OVERHEAD
            return True


class Gateway:
    def __init__(self, upstream_port=8545, network=None, clock=None):
        self.port = upstream_port
        chain_id, genesis = (CHAIN_ID, GENESIS) if network is None else validated_network(
            {'chainId': network[0], 'genesisHash': network[1], 'specVersion': 2, 'v3': True})
        self.chain_id, self.genesis = chain_id, genesis.lower()
        self.clock = clock
        self.lock = threading.Lock()
        self.identity_lock = threading.Lock()
        self.rpc_slots = threading.BoundedSemaphore(GENERAL_SLOTS)
        self.chunk_slots = threading.BoundedSemaphore(CHUNK_SLOTS)
        self.buckets = collections.OrderedDict()
        self.last_identity = None
        self.cache = ChunkCache()

    def now(self):
        return self.clock() if self.clock is not None else time.monotonic()

    def charge(self, client, cost=1, write=False, chunks=0):
        """Take `cost` read tokens, `chunks` chunk tokens and (for a write) one write token from the global and the
        per-client buckets, all or nothing."""
        now = self.now()
        # All callers share global budgets; per-IP buckets bound a single visitor.
        checks = []
        for buckets, amount in ((READ_BUCKETS, cost), (CHUNK_BUCKETS, chunks), (WRITE_BUCKETS, 1 if write else 0)):
            if amount <= 0: continue
            (glob, gcap, grate), (name, ccap, crate) = buckets
            checks += [(glob, gcap, grate, amount), ((name, client), ccap, crate, amount)]
        with self.lock:
            updated = []
            for key, capacity, rate, amount in checks:
                old, at = self.buckets.get(key, (capacity, now))
                available = min(capacity, old + max(0, now-at)*rate)
                if available < amount: return False
                updated.append((key, available-amount))
            for key, remaining in updated:
                self.buckets[key] = (remaining, now); self.buckets.move_to_end(key)
            # Keep global buckets even when many unique public clients arrive.
            while len(self.buckets) > 4096:
                key = next(k for k in self.buckets if k not in GLOBAL_BUCKETS)
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

    def identity_fresh(self):
        return self.last_identity is not None and 0 <= self.now()-self.last_identity < IDENTITY_TTL

    def verify_network(self, *, force=False):
        """The node's pocol_status chainId (an integer) and eth_chainId (a strict hex quantity) equal the configured
        chain ID, and its genesisHash equals the configured one in any letter case. On success the chunk cache is
        bound to that genesis and its epoch returned (a cache fill made under it); on any failure it is flushed."""
        with self.identity_lock:
            if not force and self.identity_fresh(): return self.cache.epoch
            try:
                value = self.query('pocol_status', []).get('result', {})
                chain = self.query('eth_chainId', []).get('result')
                ok = (isinstance(value, dict) and type(value.get('chainId')) is int and value['chainId'] == self.chain_id
                      and same_genesis(value.get('genesisHash'), self.genesis) and chain_id_quantity(chain, self.chain_id))
            except BaseException:
                self.last_identity = None; self.cache.flush(); raise
            if not ok:
                self.last_identity = None; self.cache.flush()
                raise RuntimeError('Unexpected network identity')
            epoch = self.cache.bind(self.genesis)
            self.last_identity = self.now()
            return epoch

    def relay(self, ident, method, answer):
        if 'result' in answer: answer['result'] = sanitize(method, answer['result'])
        if 'error' in answer:
            upstream = answer['error']
            code = upstream.get('code', -32000) if isinstance(upstream, dict) else -32000
            message = upstream.get('message', 'Node rejected request') if isinstance(upstream, dict) else 'Node rejected request'
            answer['error'] = {'code': code if type(code) is int else -32000, 'message': str(message)[:300]}
        answer['id'] = ident
        return answer

    def local(self, request):
        value = str(self.chain_id) if request['method'] == 'net_version' else CLIENT_VERSION
        return {'jsonrpc': '2.0', 'id': request['id'], 'result': value}

    def hit(self, ident, key):
        code = self.cache.get(self.genesis, key)
        return None if code is None else {'jsonrpc': '2.0', 'id': ident, 'result': '0x' + code.hex()}

    def code(self, request):
        """eth_getCode at latest or safe. A hit (code cached from `safe` under the verified genesis) needs no upstream
        call and no slot. A miss takes a reserved chunk slot and asks the node for the code at `safe`: chunk-shaped
        code there is cached and answered; otherwise a `latest` request is forwarded as asked and nothing is cached."""
        ident, (addr, at) = request['id'], request['params']
        key = addr.lower()
        if self.identity_fresh():
            answer = self.hit(ident, key)
            if answer is not None: return answer
        if not self.chunk_slots.acquire(blocking=False): return error(ident, -32021, 'Studio gateway busy; retry shortly')
        try:
            epoch = self.verify_network()
            answer = self.hit(ident, key)
            if answer is not None: return answer
            safe = self.query('eth_getCode', [addr, 'safe'])
            code = chunk_code(safe.get('result'))
            if code is not None:
                self.cache.put(epoch, key, code)
                return {'jsonrpc': '2.0', 'id': ident, 'result': '0x' + code.hex()}
            return self.relay(ident, 'eth_getCode', safe if at == 'safe' else self.query('eth_getCode', [addr, at]))
        except (OSError, ValueError, RuntimeError, RecursionError, http.client.HTTPException):
            return error(ident, -32000, 'Node unavailable or network identity mismatch')
        finally: self.chunk_slots.release()

    def call(self, request):
        ident, method, params = request['id'], request['method'], request['params']
        kind = classify(request)
        if kind == 'local': return self.local(request)
        if kind == 'chunk': return self.code(request)
        if not self.rpc_slots.acquire(blocking=False): return error(ident, -32021, 'Studio gateway busy; retry shortly')
        try:
            self.verify_network(force=method == 'eth_sendRawTransaction')
            # Clamp default execution gas even if the caller omits the field.
            if method in ('eth_call', 'eth_estimateGas'):
                params = [{**params[0], 'gas': params[0].get('gas', '0x7a1200')}, *params[1:]]
            return self.relay(ident, method, self.query(method, params))
        except (OSError, ValueError, RuntimeError, RecursionError, http.client.HTTPException):
            # Never retry writes here: a timed-out broadcast may have reached the node.
            return error(ident, -32000, 'Node unavailable or network identity mismatch; check transaction hash before resubmitting')
        finally: self.rpc_slots.release()


def encode(value):
    return json.dumps(value, separators=(',', ':')).encode()


class Handler(BaseHTTPRequestHandler):
    protocol_version = 'HTTP/1.1'
    server_version = 'PoColStudio/1.0'
    sys_version = ''

    def setup(self):
        super().setup(); self.connection.settimeout(12)

    def log_message(self, *_args): pass

    def reply(self, status, value):
        self.reply_raw(status, encode(value))

    def reply_raw(self, status, body):
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
        # Caddy overwrites this header with the actual remote IP (the installer's route sets it with header_up).
        # The listener is loopback-only.
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
        gateway = self.server.gateway
        if not gateway.charge(self.client_key()): return self.reply(429, {'error': 'Request budget exceeded'})
        try:
            gateway.verify_network()
            self.reply(200, {'service': 'pocol-studio-gateway', 'ready': True, 'chainId': gateway.chain_id,
                             'genesisHash': gateway.genesis, 'rpcPath': '/studio/rpc', 'trust': 'remote-node',
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
        gateway = self.server.gateway
        try:
            raw = self.rfile.read(size)
            if len(raw) != size: raise ValueError()
            value = strict_json(raw)
            if not valid_payload(value, gateway.chain_id): raise ValueError()
        except (ValueError, OSError, RecursionError): return self.reply(400, {'error': 'Unsupported or invalid Studio RPC request'})
        items = value if isinstance(value, list) else [value]
        # Classify every item first, then charge each class to its own budget in one step.
        kinds = [classify(item) for item in items]
        reads = sum(read_cost(item) for item, kind in zip(items, kinds) if kind != 'chunk')
        chunks = kinds.count('chunk')
        if not gateway.charge(self.client_key(), reads, write=kinds == ['write'], chunks=chunks):
            return self.reply(429, {'error': 'Request budget exceeded; retry shortly'})
        parts, size = [], 2
        for item in items:
            part = encode(gateway.call(item)); size += len(part) + 1
            if size > MAX_RESPONSE: return self.reply(413, {'error': 'Response limit exceeded'})
            parts.append(part)
        # Each answer is encoded once; the batch body is joined from those bytes.
        return self.reply_raw(200, b'[' + b','.join(parts) + b']' if isinstance(value, list) else parts[0])


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


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bind', default='127.0.0.1', choices=['127.0.0.1'], help='Loopback only; Caddy terminates HTTPS')
    parser.add_argument('--port', type=int, default=8089)
    parser.add_argument('--rpc-port', type=int, default=8545)
    parser.add_argument('--network', metavar='FILE',
                        help='Trusted JSON {"chainId":N,"genesisHash":"0x..","specVersion":2,"v3":true} (the explorer '
                             "gateway's --v3-network format): serve that network instead of the pinned one")
    args = parser.parse_args(argv)
    if not 1 <= args.port <= 65535 or not 1 <= args.rpc_port <= 65535: parser.error('Invalid port')
    try: network = load_network(args.network)
    except (OSError, ValueError) as exc: parser.error(str(exc))
    gateway = Gateway(args.rpc_port, network); gateway.verify_network(force=True)
    server = Server((args.bind, args.port), gateway)
    print('PoCol Studio gateway ready on loopback; expected network identity verified', flush=True)
    server.serve_forever(poll_interval=.5)


if __name__ == '__main__': main()
