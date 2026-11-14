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
try:
    from pocol_rewards import validated_network, MAX_CALLS_V3, MAX_SECONDS_V3
except ImportError:
    validated_network = None
    MAX_CALLS_V3, MAX_SECONDS_V3 = 0, 0
try:
    from pocol_rewards import (keccak256, _abi_two_arrays, hex_bytes, REGISTRY, CAPS, CALL_GAS, ACTIVE_SLICE_GAS,
                               MAX_MEMBERS_V3, ACTIVE_PAGE, active_version, full_nonce_rule)
except ImportError:
    keccak256 = None
    MAX_MEMBERS_V3, ACTIVE_PAGE = 10000, 512
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer

CHAIN_ID = 1599846715
GENESIS = '0xbe137bfdb4fdf332f2e1500ee1a67e0b22ce6bdf57efa46fb4ea7b0debe509ca'
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
# assignment.rs Membership::from_parent / DrawWeights::new), which can depend on the whole proposer membership A_h*: an
# ACCRUING record below B_reg/4 is excluded unless every member of A_h* is (the owner-approved all-probationer
# exception), and on a bounded nonce domain A_h* is the members active within W_act unless none is. Deciding that reads
# the ActList (lengths and at most DRAW_PAGES_MAX activeSlice pages, cached per head) and, on demand, the bond and
# graceStart of other members (at most DRAW_RECORDS_MAX recordWords calls); past that bound proposerEligible is null.
DRAW_PAGES_MAX = -(-MAX_MEMBERS_V3 // ACTIVE_PAGE)
DRAW_RECORDS_MAX = 32
DRAW_READS_MAX = 3 + DRAW_PAGES_MAX + DRAW_RECORDS_MAX
DRAW_CACHE_MAX = 2
CP_NONCE_MODE, CP_W_ACT = 4, 24
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


def load_v3_network(filename=None):
    """Read the operator's v3 network configuration once at startup (V3-PR-G; no v3 network identity is pinned yet): a JSON object
    {"chainId": N, "genesisHash": "0x...", "specVersion": 2, "v3": true}. Without a file the gateway serves only the
    pinned spec-1 Mainnet; a v3 genesis is never enabled by an RPC answer."""
    if filename is None:
        return None
    if validated_network is None:
        raise RuntimeError('Install the reward module before configuring a v3 network')
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
        result['stats'] = {k:value.get('stats', {}).get(k) for k in ('reorgs', 'mined', 'imported')}
        return result
    if method == 'pocol_getParams':
        keys = ('chainId', 'genesisHash', 'profileKind', 'nodeKind', 'nonceMode', 'netKey', 'protocolVersion', 'compatibilitySha256', 'activeVersion', 'forkSchedule', 'effectiveNonceMode', 'nextBlockNonceMode', 'consensusCompatibilitySha256', 'networkName', 'networkEnvironment') + IDENTITY_FIELDS
        return {k:value.get(k) for k in keys}
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

def draw_candidate(target, t_words, quarter, all_live, cutoff, scan, words):
    """Whether `target` is a proposer candidate of the next block h = H + 1 drawn from state H, exactly as pocold
    decides it (chain.rs membership_one_pass, assignment.rs Membership::from_parent and DrawWeights::new on v3):
    A_h* is the live (non-tomb) ActList members, or on a bounded nonce domain (all_live() false) those with lastShare or
    graceStart >= cutoff() = max(0, h - W_act) unless none is; a member of A_h* is a candidate unless it is ACCRUING with
    a bond below B_reg/4 (`quarter`), and when every member of A_h* is such a probationer all of them are candidates.
    t_words: the target's (w0, bond, graceStart). scan(): (the ActList at H as [(address, w0)], heldTotal), read only
    when the answer depends on other members; words(address): (bond, graceStart) of another member, read on demand
    (DrawUndecided past its bound). heldTotal (MinerRegistry slot 4) is the sum of the held bonds, so below `quarter`
    every ACCRUING member is below it without reading the bonds."""
    t_w0, t_bond, t_grace = t_words
    if t_w0 & 255 != 1 or (t_w0 >> 8) & 255:
        return False
    all_live = all_live()
    cutoff = 0 if all_live else cutoff()
    share = lambda w0: (w0 >> 88) & U64
    accruing = lambda w0: (w0 >> 16) & 255 == 4
    def live():
        members, held = scan()
        if not any(a == target for a, _ in members): raise RuntimeError('Active record missing from the active list')
        return [(a, w0) for a, w0 in members if not (w0 >> 8) & 255], held
    known = lambda a: (t_bond, t_grace) if a == target else words(a)
    active = lambda a, w0: share(w0) >= cutoff or known(a)[1] >= cutoff
    # pool_active: A_h* is the active subset (bounded domain with an active member); otherwise every live member.
    pool_active = False
    if not all_live:
        if share(t_w0) >= cutoff or t_grace >= cutoff:
            pool_active = True
        else:
            members, _ = live()
            if any(share(w0) >= cutoff for _, w0 in members) or any(active(a, w0) for a, w0 in members):
                return False
    if not (accruing(t_w0) and t_bond < quarter):
        return True
    members, held = live()
    in_pool = lambda a, w0: not pool_active or active(a, w0)
    cheap = lambda w0: not pool_active or share(w0) >= cutoff
    # Any member of A_h* that is not a probationer ends the exception: first those decided by w0 alone.
    if any(not accruing(w0) and cheap(w0) for _, w0 in members) or any(not accruing(w0) and in_pool(a, w0) for a, w0 in members):
        return False
    if held < quarter:
        return True
    return not any(in_pool(a, w0) and known(a)[0] >= quarter for a, w0 in members)

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
    def __init__(self, upstream_port=8545, geography=None, transactions=None, *, fork_schedule=DEFAULT_FORK_SCHEDULE, network=None):
        self.fork_schedule = trusted_fork_schedule(fork_schedule)
        if network is not None and validated_network is None:
            raise RuntimeError('Install the reward module before configuring a v3 network')
        # None: the pinned spec-1 Mainnet. A v3 network only from the operator's validated configuration.
        self.network = None if network is None else validated_network(network)
        self.chain_id = CHAIN_ID if self.network is None else self.network.chain_id
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
        chain_id, genesis = (CHAIN_ID, GENESIS) if self.network is None else (self.network.chain_id, self.network.genesis)
        if params.get('chainId') != chain_id or params.get('genesisHash', '').lower() != genesis:
            raise RuntimeError('The local RPC does not belong to the configured PoCol network')
    def membership(self, method, params):
        """pocol_getMembers / pocol_getMember at the head block, every read at that one height, cached per head (block
        hash) and refused with ChainMoved when the block at that height changed while it was read."""
        if self.network is None or keccak256 is None:
            raise RuntimeError('v3 membership needs the reward module and a configured --v3-network')
        remaining = MEMBER_READS_MAX + (DRAW_READS_MAX if method == 'pocol_getMember' else 0)
        def read(read_method, read_params):
            nonlocal remaining
            if remaining <= 0: raise RuntimeError('Membership read budget exceeded')
            remaining -= 1
            answer = self.query(read_method, read_params)
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
                result['proposerEligible'] = draw_candidate(member, (words[0], words[2], (words[3] >> 192) & U64), b_reg // 4,
                                                            draw['allLive'], draw['cutoff'], draw['scan'], draw['words'])
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
        """The inputs of draw_candidate at state `height` (block `head`), each read once and only when needed:
        all_live (v3 with the full nonce domain at the next height: the operator's fork schedule, else CP nonceMode),
        the W_act cutoff, the ActList with heldTotal (lengths and activeSlice pages) and other members' (bond,
        graceStart). Values from a previous request at the same head are reused; `cached` is stored only after the
        caller's final block check."""
        with self.lock:
            cached = dict(self.draw_cache.get(head, {}))
        cached['words'] = dict(cached.get('words', {}))
        record_reads = 0
        def cap(index):
            return call(CAPS, _word_args('get(uint256)', index), CALL_GAS, 1)[0]
        def all_live():
            if 'allLive' not in cached:
                version = active_version(height + 1, self.fork_schedule)
                cached['allLive'] = full_nonce_rule(version, None if version >= 2 else cap(CP_NONCE_MODE))
            return cached['allLive']
        def cutoff():
            if 'wAct' not in cached: cached['wAct'] = cap(CP_W_ACT)
            return max(0, height + 1 - cached['wAct'])
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
        def words(address):
            nonlocal record_reads
            if address not in cached['words']:
                if record_reads >= DRAW_RECORDS_MAX: raise DrawUndecided()
                record_reads += 1
                w = call(REGISTRY, _word_args('recordWords(address)', int(address, 16)), CALL_GAS, 4)
                cached['words'][address] = (w[2], (w[3] >> 192) & U64)
            return cached['words'][address]
        return {'allLive': all_live, 'cutoff': cutoff, 'scan': scan, 'words': words, 'cached': cached}
    def call(self, request):
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
                if reward_snapshot is None: raise RuntimeError('Reward summary is not installed')
                if time.monotonic() - self.reward_failed_at < 3:
                    raise RuntimeError('Reward summary temporarily unavailable')
                reward_locked = self.reward_slot.acquire(blocking=False)
                if not reward_locked:
                    return {'jsonrpc':'2.0','id':request['id'],'error':{'code':-32021,'message':'Reward snapshot is being prepared; retry shortly'}}
                deadline, remaining = time.monotonic() + (20 if self.network is None else MAX_SECONDS_V3), 512 if self.network is None else MAX_CALLS_V3
                reads = {'pocol_status', 'pocol_getParams', 'eth_getBlockByNumber', 'eth_getBlockByHash', 'pocol_getBlockRlp', 'eth_call', 'eth_getCode', 'eth_getBalance', 'eth_getTransactionCount'}
                if self.network is not None: reads = reads | {'eth_getTransactionReceipt'}   # V3-20: the block's tips
                def bounded_reward_query(read_method, read_params):
                    nonlocal remaining
                    if remaining <= 0 or time.monotonic() >= deadline:
                        raise RuntimeError('Reward read budget exceeded')
                    if read_method not in reads:
                        raise RuntimeError('Unsupported reward read')
                    remaining -= 1
                    return self.query(read_method, read_params)
                try:
                    if self.network is None:
                        value = reward_snapshot(bounded_reward_query, fork_schedule=self.fork_schedule)
                    else:
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
                answer = {'jsonrpc':'2.0','id':request['id'],'result':self.membership(method, params)}
            elif method == 'pocol_getNetworkGeography':
                if self.geography is None: raise RuntimeError('Automatic GeoIP is not installed')
                raw_status = self.query('pocol_status', []).get('result')
                answer = {'jsonrpc':'2.0','id':request['id'],'result':self.geography.snapshot(raw_status)}
            else:
                answer = self.query(method, params)
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
            self.reply(200, {'service':'pocol-explorer-gateway','readOnly':True,'chainId':self.server.gateway.chain_id,'note':'Service health only; the explorer checks upstream identity and freshness.'})
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
        if not self.server.gateway.charge(sum(COSTS.get(item['method'], 1) for item in items)): self.reply(429, {'error':'Explorer request budget exceeded; retry shortly'}); return
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
    parser.add_argument('--v3-network', metavar='FILE', help='Trusted JSON {"chainId":N,"genesisHash":"0x..","specVersion":2,"v3":true}: serve a v3 (spec-2) network instead of Mainnet')
    args = parser.parse_args()
    try:
        fork_schedule = load_fork_schedule(args.fork_schedule)
        network = load_v3_network(args.v3_network)
    except (OSError, ValueError, RuntimeError) as error:
        parser.error(str(error))
    geography = Geography.from_config() if Geography is not None and Path('/opt/pocol-explorer-gateway/geoip-config.json').is_file() else None
    gateway = Gateway(args.rpc_port, geography, fork_schedule=fork_schedule, network=network)
    gateway.verify_network()
    if os.environ.get('POCOL_TX_DB'):
        if TransactionIndex is None: raise RuntimeError('Transaction index module is missing')
        gateway.transactions = TransactionIndex(os.environ['POCOL_TX_DB'], gateway.query)
        gateway.transactions.start()
    server = Server((args.bind, args.port), gateway)
    print('PoCol explorer read-only gateway ready', flush=True)
    server.serve_forever(poll_interval=0.5)
