"""Guarded HeaderNetCheck receive adapter for M1 draft 0.30 (author turn 028, issue C33). SPECIFICATION FIXTURE TOOLING ONLY.
Python standard library only. NOT executed by the author.

browser.md:270-288 (D98) puts every HeaderNetCheck request through HttpTransport/RecvGuard; P-V1-11 (DELEGATED-M1-CONVENTIONS-030)
binds the response id to the current request. This adapter applies that boundary to every raw reply (eth_blockNumber, every
pocol_getHeaders attempt, errors and retries) BEFORE the unchanged 0.27 semantic checker (m1-draft-0.27/tools/v1_ref_027.py)
sees it. Order, per reply:
  1. declared Content-Length > recvLimit           -> recvLimit (before any byte is read)
  2. received bytes > recvLimit                    -> recvLimit       limits: browser.md:299 (4096), :301-302 (98304)
  3. TextDecoder fatal (one leading BOM dropped)    -> recvParse       browser.md:280; WHATWG TextDecoder default ignoreBOM=false
  4. depth > 16                                     -> recvDepth       browser.md:281
  5. parseStrict: duplicate key at any depth, NaN/Infinity, any JSON syntax error -> recvParse   browser.md:281
     (the 0.4 reference parser m1-draft-0.4/tools/bridge_ref.py: _pairs, _parse_int, FloatTok, _no_constants, json_depth)
  6. bounded error: message cut to 256 UTF-16 units; data dropped if its compact encoding > 4096 bytes   browser.md:283-285
  7. envelope id: the reply must be an object whose id has the C19 value (m1-draft-0.6/tools/bridge_ref_06.py
     integral_value: finite, integral, never bool/string) equal to the current request id, 0..2^32-1   P-V1-11
Request ids are the reference fixture convention of P-V1-11: eth_blockNumber 1, pocol_getHeaders 2, every retry 2.
A failure raises GuardFail: controlled viewIncomplete, no frame, 4901, no retry, no wait, no header work.
The text handed to the checker is the decoded reply itself; only a clipped error reply is re-serialized.
No allocation, pool reservation or memory claim is made: the pool step of browser.md:274 is outside this fixture model.
"""

import json
import math

RECV_LIMIT = {'eth_blockNumber': 4096, 'pocol_getHeaders': 98304}
REQUEST_ID = {'eth_blockNumber': 1, 'pocol_getHeaders': 2}
DEPTH_MAX = 16
MESSAGE_UNITS_MAX = 256
DATA_BYTES_MAX = 4096
ID_MAX = 2 ** 32 - 1


class GuardFail(Exception):
    def __init__(self, reason, stage, method, request_id):
        super().__init__(reason)
        self.reason, self.stage, self.method, self.request_id = reason, stage, method, request_id


def integral_value(B04, v):
    """C19 value semantics (bridge_ref_06.integral_value) over the 0.4 parser's number tokens."""
    if isinstance(v, bool) or not isinstance(v, (B04.IntTok, B04.FloatTok)):
        return None
    if isinstance(v, B04.FloatTok):
        if not math.isfinite(v) or not float(v).is_integer():
            return None
        return int(v)
    return int(v)


def utf16_units(s):
    return len(s.encode('utf-16-le', 'surrogatepass')) // 2


def compact_bytes(v):
    return len(json.dumps(v, separators=(',', ':'), ensure_ascii=False).encode('utf-8', 'surrogatepass'))


def receive_guard(B04, raw, method, request_id, declared_length=None):
    """raw: bytes. Returns (text_for_checker, info) or raises GuardFail."""
    def fail(reason, stage):
        raise GuardFail(reason, stage, method, request_id)

    limit = RECV_LIMIT[method]
    if declared_length is not None and declared_length > limit:
        fail('recvLimit', 'contentLength')
    if len(raw) > limit:
        fail('recvLimit', 'stream')
    try:
        text = raw.decode('utf-8', 'strict')
    except UnicodeDecodeError:
        fail('recvParse', 'utf8')
    if text.startswith('﻿'):
        text = text[1:]
    if B04.json_depth(text) > DEPTH_MAX:
        fail('recvDepth', 'depth')
    try:
        msg = json.loads(text, object_pairs_hook=B04._pairs, parse_int=B04._parse_int,
                         parse_float=B04.FloatTok, parse_constant=B04._no_constants)
    except (B04._Reject, ValueError, RecursionError):
        fail('recvParse', 'parseStrict')
    info = {'bytes': len(raw), 'clippedMessageUnits': None, 'dataDropped': False}
    out = text
    if isinstance(msg, dict) and isinstance(msg.get('error'), dict):
        e, changed = msg['error'], False
        if isinstance(e.get('message'), str) and utf16_units(e['message']) > MESSAGE_UNITS_MAX:
            e['message'] = B04.utf16_prefix(e['message'], MESSAGE_UNITS_MAX)
            info['clippedMessageUnits'] = MESSAGE_UNITS_MAX
            changed = True
        if 'data' in e and compact_bytes(e['data']) > DATA_BYTES_MAX:
            del e['data']
            info['dataDropped'] = True
            changed = True
        if changed:
            out = json.dumps(msg, separators=(',', ':'), ensure_ascii=False)
    if not isinstance(msg, dict):
        fail('envelope', 'notObject')
    if 'id' not in msg:
        fail('envelope', 'idMissing')
    n = integral_value(B04, msg['id'])
    if n is None or not 0 <= n <= ID_MAX or n != request_id:
        fail('envelope', 'idMismatch')
    return out, info


class GuardedRpc:
    """O.DeadlineRpc plus the receive guard. script = [(latency | None, raw bytes | str[, declaredLength])]."""

    def __init__(self, O, B04, budget, script):
        self.O, self.B04, self.b, self.script = O, B04, budget, list(script)
        self.calls, self.ids, self.guard = [], [], []

    def call(self, method, params):
        rid = REQUEST_ID[method]
        self.calls.append({'t': self.b.t, 'method': method, 'params': list(params)})
        self.ids.append(rid)
        item = self.script.pop(0) if self.script else (0, self.O.UNSCRIPTED)
        lat, raw = item[0], item[1]
        declared = item[2] if len(item) > 2 else None
        self.b.reply(lat, method)
        if isinstance(raw, str):
            raw = raw.encode('utf-8', 'surrogatepass')
        text, info = receive_guard(self.B04, raw, method, rid, declared)
        self.guard.append(info)
        return text

    def sleep(self, ms):
        self.b.sleep(ms, 'retryAfterMs')


def guarded_header_net_check(V, O, B04, cfg, script, clock_s, compute_ms, t0=0, cache=True, ctr=None):
    """O.header_net_check with GuardedRpc: the unchanged 0.27 check_window runs only on guard-accepted replies."""
    b = O.Budget(t0)
    rpc = GuardedRpc(O, B04, b, script)
    ctr = ctr if ctr is not None else V.Counters()
    why, guard = None, None
    try:
        verdict = V.check_window(cfg, rpc, clock_s, ctr, cache)
        b.compute(compute_ms, 'verdict')
        at = b.elapsed()
    except O.Expired as e:
        verdict, at, why = O._vi({'deadline': 'expired', 'where': e.where}), e.at - t0, 'expired'
    except O.DoesNotFit as e:
        verdict, at, why = O._vi({'deadline': 'retryDelayDoesNotFit', 'delayMs': e.delay}), e.at - t0, 'retryDelayDoesNotFit'
    except GuardFail as g:
        guard = {'reason': g.reason, 'stage': g.stage, 'method': g.method, 'requestId': g.request_id}
        verdict, at, why = O._vi({'receiveGuard': guard}), b.elapsed(), 'receiveGuard'
    return {'verdict': verdict, 'atMs': at, 'why': why, 'guard': guard, 'sleptMs': b.slept, 'requests': rpc.calls, 'ids': rpc.ids,
            'guardInfo': rpc.guard, 'counters': ctr}
