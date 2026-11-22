#!/usr/bin/env python3
"""Bounded read-only PoCol explorer gateway: POST /rpc, GET /health and the explorer snapshot's same-origin reads
(/api/rpc.php, /api/health.php, /api/miner-locations.php, /api/transactions.php; explorer_api.py). Python 3.10+,
standard library only."""
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
# The reward and membership reader is part of the gateway (installed beside it), and so is the transaction index
# (pocol_transactions.py, used when POCOL_TX_DB names its file). GeoIP has no module in the repository: the network
# gives no truthful miner location (deploy/explorer/README.md, Miner map), so pocol_getNetworkGeography and
# /api/miner-locations.php stay unavailable unless the owner decides otherwise.
from pocol_rewards import (snapshot_v3 as reward_snapshot, validated_fork_schedule, validated_network, DEFAULT_FORK_SCHEDULE,
                           MAX_CALLS_V3, MAX_SECONDS_V3, keccak256, _abi_two_arrays, hex_bytes, REGISTRY, CAPS, CALL_GAS,
                           ACTIVE_SLICE_GAS, MAX_MEMBERS_V3, ACTIVE_PAGE)
# Sites W8: the explorer snapshot's same-origin /api reads (the retired PHP relays' paths), installed beside it.
import explorer_api
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from urllib.parse import urlsplit

MAX_REQUEST = 16384
MAX_RESPONSE = 4 * 1024 * 1024
NO_PARAMS = {'pocol_status', 'pocol_getParams', 'pocol_getMiners', 'eth_blockNumber', 'net_peerCount', 'eth_gasPrice', 'pocol_getNetworkGeography', 'pocol_getBlockReward',
             'pocol_getRemovable', 'pocol_finality'}
# Sites W6: v3 membership read through MinerRegistry views (PoColSystemV3.sol): pages of the active list with each
# member's record word w0 (activeSlice), and one record's four words, its clocks and the removal verdict
# (recordWords + recordClocks + removalStatus) at one block. recordWords returns w1 as the reward address alone (V3-09),
# so lastReactHeight and lastMaintHeight mod 2^32 come only from recordClocks. w1-w3 of every member would exceed the v3 read budget at M_max 10,000, so a page carries
# w0 fields only; bond and reward address come from pocol_getMember.
MEMBERS = {'pocol_getMembers', 'pocol_getMember'}
MEMBER_PAGE_MAX = 256
MEMBER_READS_MAX = 8
MEMBER_CACHE_MAX = 64
# pocol_getMember's proposerEligible is the draw's candidate rule for the next block (chain.rs membership_one_pass and
# assignment.rs Membership::from_parent / DrawWeights::new), which can depend on the whole proposer membership A_h*, the
# live (non-tomb) ActList members (v3 draws from all of them, D-V3-01): an ACCRUING record below B_reg/4 is excluded
# unless every member of A_h* is (the owner-approved all-probationer exception). Deciding that reads the ActList
# (lengths and at most DRAW_PAGES_MAX activeSlice pages, cached per head) and, on demand, the bond of other members (at
# most DRAW_RECORDS_MAX recordWords calls); past that bound proposerEligible is null.
DRAW_PAGES_MAX = -(-MAX_MEMBERS_V3 // ACTIVE_PAGE)
DRAW_RECORDS_MAX = 32
DRAW_READS_MAX = 3 + DRAW_PAGES_MAX + DRAW_RECORDS_MAX
DRAW_CACHE_MAX = 2
# Token cost per method (others cost 1): a reward snapshot and one member record (four calls and a fresh-block check).
COSTS = {'pocol_getBlockReward': 20, 'pocol_getMember': 20, 'pocol_getMembers': 5, 'pocol_getRemovable': 10}
# MinerRegistry record word w0 (REGISTRY-LAYOUT.md): status 0..7, tomb 8..15, bondState 16..23, recPos 24..55,
# actPos 56..87, lastShare 88..151, registeredAt 152..215 (earn-in). Every value the v3 programs write is labelled.
STATUS_LABELS = {0: 'NONE', 1: 'ACTIVE', 2: 'EXITED'}
BOND_STATE_LABELS = {0: 'NONE', 1: 'HELD', 2: 'BURNED', 3: 'PAID', 4: 'ACCRUING'}
# MinerRegistry.removal (FN-2b C4), in order: 0 removable; 1 not a live active record; 2 the set is not full; 3 no more
# than M_min active; 4 a credited share or a (re)activation within Theta_inact.
REMOVAL_LABELS = {0: 'REMOVABLE', 1: 'NOT_LIVE_ACTIVE', 2: 'SET_NOT_FULL', 3: 'AT_MINIMUM', 4: 'RECENTLY_ACTIVE'}
CP_B_REG = 16
U32, U64, U160 = (1 << 32) - 1, (1 << 64) - 1, (1 << 160) - 1
HASH_METHODS = {'eth_getTransactionByHash', 'eth_getTransactionReceipt'}
ACCOUNT_METHODS = {'eth_getBalance', 'eth_getTransactionCount', 'eth_getCode'}

def load_fork_schedule(filename=None):
    """Read operator configuration once at startup; never derive it from RPC. Without a file: [[1, 0]] (the v3 profiles
    name no fork; fork 2 is a no-op, fork 3 the cap raise)."""
    if filename is None:
        return DEFAULT_FORK_SCHEDULE
    with open(filename, 'rb') as source:
        raw = source.read(4097)
    if len(raw) > 4096:
        raise ValueError('Fork schedule file exceeds 4096 bytes')
    return validated_fork_schedule(json.loads(raw))


def load_v3_network(filename):
    """Read the operator's network configuration once at startup (required: there is no built-in network): a JSON object
    {"chainId": N, "genesisHash": "0x...", "specVersion": 2, "v3": true} and optionally "source" (an https URL the reward
    answer names). The network is never enabled by an RPC answer."""
    with open(filename, 'rb') as source:
        raw = source.read(4097)
    if len(raw) > 4096:
        raise ValueError('Network file exceeds 4096 bytes')
    return validated_network(json.loads(raw))


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
    if method == 'pocol_getMembers':
        return (len(params) == 2 and type(params[0]) is int and type(params[1]) is int
                and 0 <= params[0] < MAX_MEMBERS_V3 and 1 <= params[1] <= MEMBER_PAGE_MAX)
    if method == 'pocol_getMember': return len(params) == 1 and matches(r'0x[0-9a-fA-F]{40}', params[0])
    if method in HASH_METHODS: return len(params) == 1 and hash_value(params[0])
    if method in ACCOUNT_METHODS: return len(params) == 2 and matches(r'0x[0-9a-fA-F]{40}', params[0]) and tag(params[1])
    if method == 'eth_getBlockByNumber': return len(params) == 2 and tag(params[0]) and type(params[1]) is bool
    if method == 'eth_getBlockByHash': return len(params) == 2 and hash_value(params[0]) and type(params[1]) is bool
    return False

def valid_payload(value):
    return (0 < len(value) <= 20 and all(valid_call(v) for v in value)) if isinstance(value, list) else valid_call(value)

# The public consensus identity and wire range (VA-SPEC §2), so a wallet or explorer can tell why peers differ (A2-10).
IDENTITY_FIELDS = ('consensusIdentity', 'consensusRulesVersion', 'protocolVersionMin', 'maxForkVersion')

def sanitize(method, value):
    if not isinstance(value, dict): return value
    if method == 'pocol_status':
        keys = ('chainId', 'genesisHash', 'head', 'pool', 'lifetime', 'protocolVersion', 'compatibilitySha256',
                'forkSchedule', 'effectiveNonceMode', 'nextBlockNonceMode', 'consensusCompatibilitySha256',
                'networkName', 'networkEnvironment') + IDENTITY_FIELDS
        result = {k:value.get(k) for k in keys}
        result['peerCount'] = len(value.get('peers', []))
        # The launch approval, readiness and technical-readiness completeness (pocold readiness.rs, OWNER-DECISIONS-
        # 2026-11-20 D4), booleans only: each passes only as the boolean the node computed and is left out otherwise;
        # the blockers, the pending items and their details are always left out.
        for k in ('publicLaunchApproved', 'publicLaunchReady'):
            if type(value.get(k)) is bool: result[k] = value[k]
        technical = value.get('technicalReadiness')
        if isinstance(technical, dict) and type(technical.get('complete')) is bool: result['technicalReadiness'] = {'complete': technical['complete']}
        result['stats'] = {k:value.get('stats', {}).get(k) for k in ('reorgs', 'mined', 'imported')}
        return result
    if method == 'pocol_getParams':
        keys = ('chainId', 'genesisHash', 'profileKind', 'nodeKind', 'nonceMode', 'netKey', 'protocolVersion', 'compatibilitySha256', 'activeVersion', 'forkSchedule', 'effectiveNonceMode', 'nextBlockNonceMode', 'consensusCompatibilitySha256', 'networkName', 'networkEnvironment') + IDENTITY_FIELDS
        result = {k:value.get(k) for k in keys}
        # FN-5: a scheduled cap raise (with its fork-3 height) beside forkSchedule; absent without one, as before.
        if 'capRaise' in value: result['capRaise'] = value['capRaise']
        return result
    return value

class ChainMoved(RuntimeError):
    """The block a membership answer was read at is no longer the canonical block at its height."""

def _word_args(signature, *args):
    return '0x' + (keccak256(signature.encode())[:4] + b''.join(a.to_bytes(32, 'big') for a in args)).hex()

def label(table, value, what):
    if value not in table: raise RuntimeError('Unknown ' + what)
    return table[value]

def member_w0(w0):
    """The public fields of record word w0, every status and bondState labelled."""
    status, bond_state = w0 & 255, (w0 >> 16) & 255
    return {'status': status, 'statusLabel': label(STATUS_LABELS, status, 'record status'), 'tomb': bool((w0 >> 8) & 255),
            'bondState': bond_state, 'bondStateLabel': label(BOND_STATE_LABELS, bond_state, 'bond state'),
            'lastShare': (w0 >> 88) & U64, 'registeredAt': (w0 >> 152) & U64}

class DrawUndecided(Exception):
    """The candidate rule needs more member records than DRAW_RECORDS_MAX."""

def draw_candidate(target, t_words, quarter, scan, bond):
    """Whether `target` is a proposer candidate of the next block h = H + 1 drawn from state H, exactly as pocold
    decides it (chain.rs membership_one_pass, assignment.rs Membership::from_parent and DrawWeights::new on v3):
    A_h* is the live (non-tomb) ActList members; a member of A_h* is a candidate unless it is ACCRUING with a bond below
    B_reg/4 (`quarter`), and when every member of A_h* is such a probationer all of them are candidates.
    t_words: the target's (w0, bond). scan(): (the ActList at H as [(address, w0)], heldTotal), read only when the
    answer depends on other members; bond(address): the bond of another member, read on demand (DrawUndecided past its
    bound). heldTotal (MinerRegistry slot 4) is the sum of the held bonds, so below `quarter` every ACCRUING member is
    below it without reading the bonds."""
    t_w0, t_bond = t_words
    if t_w0 & 255 != 1 or (t_w0 >> 8) & 255:
        return False
    accruing = lambda w0: (w0 >> 16) & 255 == 4
    if not (accruing(t_w0) and t_bond < quarter):
        return True
    members, held = scan()
    if not any(a == target for a, _ in members): raise RuntimeError('Active record missing from the active list')
    members = [(a, w0) for a, w0 in members if not (w0 >> 8) & 255]
    # Any member of A_h* that is not a probationer ends the exception.
    if any(not accruing(w0) for _, w0 in members):
        return False
    if held < quarter:
        return True
    return not any((t_bond if a == target else bond(a)) >= quarter for a, _ in members)

def member_record(address, words, clocks, removal, b_reg):
    """pocol_getMember: recordWords' four words decoded (w0 fields, w1 reward address, w2 bond, w3 exit clocks),
    recordClocks (lastMaintHeight mod 2^32, lastReactHeight) and the removal verdict; proposerEligible is added by
    Gateway.membership (draw_candidate)."""
    w0, w1, w2, w3 = words
    maint_mod32, last_react = clocks
    # PoColSystemV3.recordWords masks w1 to 160 bits and recordClocks returns (x>>224, (x>>160)&LOW64): anything wider
    # is not the v3 registry this gateway decodes.
    if w1 > U160 or maint_mod32 > U32 or last_react > U64: raise RuntimeError('Malformed member record')
    record = member_w0(w0)
    record.update({
        'address': address, 'exists': w0 != 0, 'recPos': (w0 >> 24) & U32, 'actPos': (w0 >> 56) & U32,
        'rewardAddress': '0x' + w1.to_bytes(20, 'big').hex(), 'lastReactHeight': last_react,
        'lastMaintHeightMod32': maint_mod32, 'bondWei': str(w2), 'bondTargetWei': str(b_reg),
        'exitHeight': w3 & U64, 'dueHeight': (w3 >> 64) & U64, 'burnHeight': (w3 >> 128) & U64, 'graceStart': (w3 >> 192) & U64,
        'removal': removal, 'removalLabel': label(REMOVAL_LABELS, removal, 'removal verdict')})
    return record

class Gateway:
    def __init__(self, upstream_port=8545, geography=None, transactions=None, *, fork_schedule=DEFAULT_FORK_SCHEDULE, network):
        self.fork_schedule = validated_fork_schedule(fork_schedule)
        # The network only from the operator's validated configuration; there is no built-in one.
        self.network = validated_network(network)
        self.chain_id = self.network.chain_id
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
        self.member_cache = collections.OrderedDict()
        self.draw_cache = collections.OrderedDict()
    def charge(self, cost):
        with self.lock:
            now = time.monotonic()
            self.tokens = min(80, self.tokens + (now-self.token_time)*20)
            self.token_time = now
            if self.tokens < cost: return False
            self.tokens -= cost
            return True
    def query(self, method, params, deadline=None):
        """One node read. With `deadline` (a time.monotonic() value) the read is refused once it has passed and its
        socket timeout is at most the time left, so a caller's total limit holds for every read it makes."""
        timeout = 5
        if deadline is not None:
            timeout = min(timeout, deadline - time.monotonic())
            if timeout <= 0: raise RuntimeError('Explorer read time limit reached')
        conn = http.client.HTTPConnection('127.0.0.1', self.port, timeout=timeout)
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
    def read(self, method, params, deadline):
        """query(), with the caller's deadline when it has one (a call without one reads exactly as /rpc always has)."""
        return self.query(method, params) if deadline is None else self.query(method, params, deadline=deadline)
    def verify_network(self):
        params = self.query('pocol_getParams', []).get('result', {})
        genesis = params.get('genesisHash')
        if params.get('chainId') != self.network.chain_id or not isinstance(genesis, str) or genesis.lower() != self.network.genesis:
            raise RuntimeError('The local RPC does not belong to the configured PoCol network')
    def membership(self, method, params, deadline=None):
        """pocol_getMembers / pocol_getMember at the head block, every read at that one height, cached per head (block
        hash) and refused with ChainMoved when the block at that height changed while it was read. Every read keeps
        to `deadline` (Gateway.query)."""
        remaining = MEMBER_READS_MAX + (DRAW_READS_MAX if method == 'pocol_getMember' else 0)
        def read(read_method, read_params):
            nonlocal remaining
            if remaining <= 0: raise RuntimeError('Membership read budget exceeded')
            remaining -= 1
            answer = self.read(read_method, read_params, deadline)
            if not isinstance(answer, dict) or 'result' not in answer or 'error' in answer: raise RuntimeError('Node read failed')
            return answer['result']
        def block(at):
            value = read('eth_getBlockByNumber', [at, False])
            if (not isinstance(value, dict) or not matches(r'0x[0-9a-fA-F]{64}', value.get('hash'))
                    or not matches(r'0x(?:0|[1-9a-fA-F][0-9a-fA-F]{0,15})', value.get('number'))):
                raise RuntimeError('Malformed block')
            return int(value['number'], 16), value['hash'].lower()
        height, head = block('latest')
        key = (head, method, json.dumps(params, separators=(',', ':')))
        with self.lock:
            if key in self.member_cache:
                self.member_cache.move_to_end(key); return json.loads(self.member_cache[key])
        def call(to, data, gas, words=None):
            raw = hex_bytes(read('eth_call', [{'to': to, 'data': data, 'gas': hex(gas)}, hex(height)]), None if words is None else 32 * words)
            return raw if words is None else [int.from_bytes(raw[i:i+32], 'big') for i in range(0, len(raw), 32)]
        if method == 'pocol_getMembers':
            start, count = params
            active = call(REGISTRY, _word_args('lengths()'), CALL_GAS, 4)[1]
            if active > MAX_MEMBERS_V3: raise RuntimeError('Unsupported active membership size')
            expected = max(0, min(count, active - start))
            members = []
            if expected:
                ids, words = _abi_two_arrays(call(REGISTRY, _word_args('activeSlice(uint256,uint256)', start, count), ACTIVE_SLICE_GAS), expected)
                for ident, w0 in zip(ids, words):
                    record = member_w0(w0)
                    if not 0 < ident <= U160 or record['status'] != 1: raise RuntimeError('Invalid active record')
                    members.append({'id': '0x' + ident.to_bytes(20, 'big').hex(), **record})
            result = {'height': height, 'blockHash': head, 'active': active, 'start': start, 'count': len(members), 'members': members}
        else:
            member = params[0].lower()
            words = call(REGISTRY, _word_args('recordWords(address)', int(member, 16)), CALL_GAS, 4)
            clocks = call(REGISTRY, _word_args('recordClocks(address)', int(member, 16)), CALL_GAS, 2)
            removal = call(REGISTRY, _word_args('removalStatus(address)', int(member, 16)), CALL_GAS, 1)[0]
            b_reg = call(CAPS, _word_args('get(uint256)', CP_B_REG), CALL_GAS, 1)[0]
            result = {'height': height, 'blockHash': head, **member_record(member, words, clocks, removal, b_reg)}
            draw = self.draw_state(head, height, call)
            try:
                result['proposerEligible'] = draw_candidate(member, (words[0], words[2]), b_reg // 4, draw['scan'], draw['bond'])
            except DrawUndecided:
                result['proposerEligible'] = None
        if block(hex(height))[1] != head: raise ChainMoved('Chain changed while membership was read')
        if method == 'pocol_getMember':
            with self.lock:
                self.draw_cache[head] = draw['cached']
                while len(self.draw_cache) > DRAW_CACHE_MAX: self.draw_cache.popitem(last=False)
        encoded = json.dumps(result, separators=(',', ':'))
        with self.lock:
            self.member_cache[key] = encoded
            while len(self.member_cache) > MEMBER_CACHE_MAX: self.member_cache.popitem(last=False)
        return result
    def draw_state(self, head, height, call):
        """The inputs of draw_candidate at state `height` (block `head`), each read once and only when needed: the
        ActList with heldTotal (lengths and activeSlice pages) and other members' bonds. Values from a previous request
        at the same head are reused; `cached` is stored only after the caller's final block check."""
        with self.lock:
            cached = dict(self.draw_cache.get(head, {}))
        cached['bonds'] = dict(cached.get('bonds', {}))
        record_reads = 0
        def scan():
            if 'members' not in cached:
                lengths = call(REGISTRY, _word_args('lengths()'), CALL_GAS, 4)
                active, held = lengths[1], lengths[3]
                if active > MAX_MEMBERS_V3: raise RuntimeError('Unsupported active membership size')
                members, seen = [], set()
                for start in range(0, active, ACTIVE_PAGE):
                    count = min(ACTIVE_PAGE, active - start)
                    ids, w0s = _abi_two_arrays(call(REGISTRY, _word_args('activeSlice(uint256,uint256)', start, ACTIVE_PAGE), ACTIVE_SLICE_GAS), count)
                    for ident, w0 in zip(ids, w0s):
                        address = '0x' + ident.to_bytes(20, 'big').hex()
                        if not 0 < ident <= U160 or address in seen: raise RuntimeError('Invalid active record')
                        member_w0(w0)
                        if w0 & 255 != 1: raise RuntimeError('Invalid active record')
                        seen.add(address); members.append((address, w0))
                cached['members'] = (members, held)
            return cached['members']
        def bond(address):
            nonlocal record_reads
            if address not in cached['bonds']:
                if record_reads >= DRAW_RECORDS_MAX: raise DrawUndecided()
                record_reads += 1
                cached['bonds'][address] = call(REGISTRY, _word_args('recordWords(address)', int(address, 16)), CALL_GAS, 4)[2]
            return cached['bonds'][address]
        return {'scan': scan, 'bond': bond, 'cached': cached}
    def call(self, request, deadline=None):
        """One allowed read, answered from the cache or the node. `deadline` bounds every node read it makes (the
        explorer's /api reads pass theirs; /rpc keeps the per-read timeouts)."""
        method, params = request['method'], request['params']
        key = json.dumps([method, params], separators=(',', ':'))
        now = time.monotonic()
        with self.lock:
            # Membership answers are cached per head block instead (Gateway.membership).
            item = None if method in MEMBERS else self.cache.get(key)
            if item and item[0] > now:
                self.cache.move_to_end(key)
                answer = json.loads(item[1]); answer['id'] = request['id']; return answer
        if not self.rpc_slots.acquire(blocking=False):
            return {'jsonrpc':'2.0','id':request['id'],'error':{'code':-32021,'message':'Explorer busy; retry shortly'}}
        reward_locked = False
        try:
            if method == 'pocol_getBlockReward':
                if time.monotonic() - self.reward_failed_at < 3:
                    raise RuntimeError('Reward summary temporarily unavailable')
                reward_locked = self.reward_slot.acquire(blocking=False)
                if not reward_locked:
                    return {'jsonrpc':'2.0','id':request['id'],'error':{'code':-32021,'message':'Reward snapshot is being prepared; retry shortly'}}
                reward_deadline, remaining = time.monotonic() + MAX_SECONDS_V3, MAX_CALLS_V3
                # eth_getTransactionReceipt: the block's tips (V3-20).
                reads = {'pocol_status', 'pocol_getParams', 'eth_getBlockByNumber', 'eth_getBlockByHash', 'pocol_getBlockRlp', 'eth_call',
                         'eth_getCode', 'eth_getBalance', 'eth_getTransactionCount', 'eth_getTransactionReceipt'}
                def bounded_reward_query(read_method, read_params):
                    nonlocal remaining
                    if remaining <= 0 or time.monotonic() >= reward_deadline:
                        raise RuntimeError('Reward read budget exceeded')
                    if read_method not in reads:
                        raise RuntimeError('Unsupported reward read')
                    remaining -= 1
                    return self.query(read_method, read_params)
                try:
                    value = reward_snapshot(bounded_reward_query, fork_schedule=self.fork_schedule, network=self.network)
                except Exception as error:
                    self.reward_failed_at = time.monotonic()
                    if isinstance(error, (OSError, ValueError, RuntimeError, KeyError, sqlite3.Error, http.client.HTTPException)):
                        raise
                    # Any other failure (a malformed node answer of an unexpected type) is still a structured error.
                    raise RuntimeError('Reward summary failed') from error
                answer = {'jsonrpc':'2.0','id':request['id'],'result':value}
            elif method == 'pocol_getTransactions':
                if self.transactions is None: raise RuntimeError('Transaction index is not installed')
                answer = {'jsonrpc':'2.0','id':request['id'],'result':self.transactions.page(params[0])}
            elif method in MEMBERS:
                answer = {'jsonrpc':'2.0','id':request['id'],'result':self.membership(method, params, deadline)}
            elif method == 'pocol_getNetworkGeography':
                if self.geography is None: raise RuntimeError('Automatic GeoIP is not installed')
                raw_status = self.read('pocol_status', [], deadline).get('result')
                answer = {'jsonrpc':'2.0','id':request['id'],'result':self.geography.snapshot(raw_status)}
            else:
                answer = self.read(method, params, deadline)
            if 'result' in answer: answer['result'] = sanitize(method, answer['result'])
            answer['id'] = request['id']
            if 'error' not in answer and method not in MEMBERS:
                encoded = json.dumps(answer, separators=(',', ':')).encode()
                with self.lock:
                    previous = self.cache.pop(key, None)
                    if previous: self.cache_size -= len(previous[1])
                    self.cache[key] = (time.monotonic()+3, encoded)
                    self.cache_size += len(encoded)
                    while len(self.cache)>256 or self.cache_size>16*1024*1024:
                        _, removed = self.cache.popitem(last=False); self.cache_size -= len(removed[1])
            return answer
        except ChainMoved:
            return {'jsonrpc':'2.0','id':request['id'],'error':{'code':-32022,'message':'Chain changed. Retry the membership read.'}}
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
    def reply(self, status, value, head=False):
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
        if head: return
        try: self.wfile.write(body)
        except (BrokenPipeError, ConnectionResetError, socket.timeout): pass
    def dispatch(self, method):
        """The explorer snapshot's /api paths (explorer_api: every method, so a wrong one gets the PHP relays' 405 and
        Allow; another /api path is its 404), GET /health and POST /rpc; anything else is 404."""
        path = urlsplit(self.path).path
        if path.startswith('/api/'):
            self.explorer(method)
        elif method == 'GET' and self.path == '/health':
            self.reply(200, {'service':'pocol-explorer-gateway','readOnly':True,'chainId':self.server.gateway.chain_id,'note':'Service health only; the explorer checks upstream identity and freshness.'})
        elif method == 'POST' and self.path == '/rpc':
            self.rpc()
        else:
            self.reply(404, {'error':'Not found'}, head=method == 'HEAD')
    def do_GET(self): self.dispatch('GET')
    def do_HEAD(self): self.dispatch('HEAD')
    def do_POST(self): self.dispatch('POST')
    def do_PUT(self): self.dispatch('PUT')
    def do_DELETE(self): self.dispatch('DELETE')
    def do_PATCH(self): self.dispatch('PATCH')
    def do_OPTIONS(self): self.dispatch('OPTIONS')
    def __getattr__(self, name):
        # Any other method token (BaseHTTPRequestHandler would answer 501): 405 on an /api path, 404 elsewhere.
        if re.fullmatch(r'do_[A-Z]{1,16}', name): return lambda: self.dispatch(name[3:])
        raise AttributeError(name)
    def explorer(self, method):
        response = self.server.explorer.handle(method, self.path, self.headers, self.rfile)
        self.send_response(response.status)
        for name, value in response.headers: self.send_header(name, value)
        body = b'' if method == 'HEAD' or response.body is None else response.body
        if method != 'HEAD': self.send_header('Content-Length', str(len(body)))
        self.send_header('Connection', 'close')
        self.end_headers()
        self.close_connection = True
        try: self.wfile.write(body)
        except (BrokenPipeError, ConnectionResetError, socket.timeout): pass
    def rpc(self):
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
        # RecursionError: a deeply nested body (within 16 KiB) is a bad request, not a dropped connection.
        except (ValueError, OSError, RecursionError): self.reply(400, {'error':'Unsupported explorer read'}); return
        items = value if isinstance(value, list) else [value]
        if not self.server.gateway.charge(sum(COSTS.get(item['method'], 1) for item in items)): self.reply(429, {'error':'Explorer request budget exceeded; retry shortly'}); return
        answers, size = [], 2
        for item in items:
            answer = self.server.gateway.call(item)
            size += len(json.dumps(answer)) + 1
            if size > MAX_RESPONSE: self.reply(413, {'error':'Explorer response limit exceeded'}); return
            answers.append(answer)
        self.reply(200, answers if isinstance(value, list) else answers[0])

# The answer to a connection beyond the 16 request slots: the explorer relays' JSON busy answer (lib.php answered it
# before a request reached the gateway; behind Caddy every request is one connection here), never a dropped connection
# that Caddy turns into an empty 502.
BUSY_BODY = json.dumps({'error': explorer_api.BUSY}, separators=(',', ':')).encode()
BUSY_REPLY = (b'HTTP/1.1 429 Too Many Requests\r\nContent-Type: application/json; charset=utf-8\r\nCache-Control: no-store\r\n'
              b'X-Content-Type-Options: nosniff\r\nRetry-After: 3\r\nConnection: close\r\nContent-Length: '
              + str(len(BUSY_BODY)).encode() + b'\r\n\r\n' + BUSY_BODY)
BUSY_SLOTS, BUSY_SECONDS = 8, 1.0

class Server(ThreadingHTTPServer):
    daemon_threads = True
    request_queue_size = 16
    def __init__(self, address, gateway, site_origin=explorer_api.SITE_ORIGIN):
        self.gateway = gateway
        # The explorer's /api reads: the Origin a browser request may carry (the explorer site), the gateway's call
        # costs and the transaction index's option check.
        self.explorer = explorer_api.Relay(gateway, site_origin, cost=lambda call: COSTS.get(call['method'], 1),
                                           transaction_options_valid=lambda options: valid_options(options))
        self.slots = threading.BoundedSemaphore(16)
        self.busy_slots = threading.BoundedSemaphore(BUSY_SLOTS)
        super().__init__(address, Handler)
    def process_request(self, request, client_address):
        if not self.slots.acquire(blocking=False):
            # A few busy answers at a time on their own threads (the accept loop never waits on a client); beyond
            # those the connection is closed as before.
            if not self.busy_slots.acquire(blocking=False): self.shutdown_request(request); return
            try: threading.Thread(target=self.answer_busy, args=(request,), daemon=True).start()
            except BaseException: self.busy_slots.release(); self.shutdown_request(request); raise
            return
        try: super().process_request(request, client_address)
        except BaseException: self.slots.release(); raise
    def answer_busy(self, request):
        """BUSY_REPLY, after reading the request (its head and a body of up to MAX_REQUEST bytes, for at most
        BUSY_SECONDS) so that closing does not reset the connection before the client has read the answer."""
        try:
            deadline, seen, need = time.monotonic() + BUSY_SECONDS, b'', None
            while need is None or len(seen) < need:
                left = deadline - time.monotonic()
                if left <= 0 or len(seen) > MAX_REQUEST + 65536: break
                request.settimeout(left)
                chunk = request.recv(65536)
                if not chunk: break
                seen += chunk
                if need is None and b'\r\n\r\n' in seen:
                    head, _, _ = seen.partition(b'\r\n\r\n')
                    found = re.search(rb'(?im)^content-length:[ \t]*([0-9]{1,12})[ \t]*\r?$', head)
                    need = len(head) + 4 + min(int(found.group(1)) if found else 0, MAX_REQUEST)
            request.settimeout(BUSY_SECONDS)
            request.sendall(BUSY_REPLY)
        except OSError: pass
        finally:
            self.shutdown_request(request); self.busy_slots.release()
    def process_request_thread(self, request, client_address):
        try: super().process_request_thread(request, client_address)
        finally: self.slots.release()

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--bind', default='127.0.0.1')
    parser.add_argument('--port', type=int, default=8088)
    parser.add_argument('--rpc-port', type=int, default=8545)
    parser.add_argument('--fork-schedule', metavar='FILE', help='Trusted JSON fork schedule; default [[1,0]] (the v3 profiles name no fork). '
                        'Fork 2 is a no-op, fork 3 the cap raise: [[1,0],[2,h2],[3,raise_height]]')
    parser.add_argument('--v3-network', metavar='FILE', required=True,
                        help='Required trusted JSON {"chainId":N,"genesisHash":"0x..","specVersion":2,"v3":true[,"source":"https://.."]}: the network served')
    parser.add_argument('--site-origin', default=explorer_api.SITE_ORIGIN,
                        help='The only Origin a browser request to /api/rpc.php may carry (default %(default)s; '
                        'http:// only on loopback, for a test copy)')
    args = parser.parse_args()
    if not explorer_api.valid_origin(args.site_origin):
        parser.error('--site-origin: https://HOST[:PORT], or http://127.0.0.1[:PORT] for a test copy')
    try:
        fork_schedule = load_fork_schedule(args.fork_schedule)
        network = load_v3_network(args.v3_network)
    except (OSError, ValueError, RuntimeError) as error:
        parser.error(str(error))
    geography = Geography.from_config() if Geography is not None and Path('/opt/pocol-explorer-gateway/geoip-config.json').is_file() else None
    gateway = Gateway(args.rpc_port, geography, fork_schedule=fork_schedule, network=network)
    gateway.verify_network()
    if os.environ.get('POCOL_TX_DB'):
        # The transaction index (pocol_transactions.py): a follower thread on this host's node, its SQLite file at
        # POCOL_TX_DB (bound by POCOL_TX_MAX_ROWS, default pocol_transactions.DEFAULT_MAX_ROWS) and the configured network.
        if TransactionIndex is None: raise RuntimeError('Transaction index module is missing')
        import pocol_transactions
        rows = os.environ.get('POCOL_TX_MAX_ROWS', str(pocol_transactions.DEFAULT_MAX_ROWS))
        if not re.fullmatch(r'[1-9][0-9]{0,9}', rows): parser.error('POCOL_TX_MAX_ROWS: a positive integer')
        gateway.transactions = TransactionIndex(os.environ['POCOL_TX_DB'], gateway.query, gateway.network, max_rows=int(rows))
        gateway.transactions.start()
    server = Server((args.bind, args.port), gateway, args.site_origin)
    print('PoCol explorer read-only gateway ready', flush=True)
    server.serve_forever(poll_interval=0.5)
