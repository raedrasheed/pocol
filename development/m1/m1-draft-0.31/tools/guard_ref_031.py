"""Guarded HeaderNetCheck receive adapter for M1 draft 0.31 (issue C35). SPECIFICATION FIXTURE TOOLING ONLY.
Runtime: Python standard library PLUS the locally installed Node.js (the M1 reference runtime, v22.13.1) running the
fixed codec tools/json_codec_031.mjs. This is NOT pure Python, and it is not a production dependency. NOT executed by the author.

0.30 (tools/guard_ref_030.py, unchanged) measured error.data with a Python serializer (surrogatepass / Python float repr),
which undercounted lone surrogates (3347 bytes for 6647 native) and miscounted numbers, so an oversized busy delay was kept
and slept. 0.31 keeps every 0.30 rule and order and replaces ONLY that measurement and the error clipping with the native
JavaScript JSON.parse / JSON.stringify, fed the original validated raw bytes:
  1-5  declared length, received length, fatal UTF-8 (+ one BOM), depth <= 16, parseStrict      (Python, unchanged 0.30 rules)
  6    envelope: object, id present, C19 value equal to the request id                         (Python, unchanged 0.30 rule)
  7    only for a reply whose 'error' is an object: native codec -> message cut to 256 UTF-16 units, data dropped if its
       well-formed JSON.stringify UTF-8 length > 4096; re-serialized only if something changed     (Node, browser.md:283-285)
The codec never sees bytes that failed steps 1-6. If the codec is missing, times out, writes too much, exits non-zero or
returns a malformed report, CodecError is raised: the run FAILs; there is no Python fallback and no silent acceptance.
"""

import json
import os
import subprocess

STDOUT_MAX = 4 * 98304 + 4096
TIMEOUT_S = 10
REPORT_KEYS = {'ok', 'node', 'changed', 'errorObject', 'messageUnits', 'messageClipped', 'dataBytes', 'dataDropped', 'textForChecker'}


class CodecError(Exception):
    pass


class NodeCodec:
    """Fixed command [node, json_codec_031.mjs]; the reply goes only through stdin; shell=False; bounded time and output;
    NODE_OPTIONS / NODE_PATH removed from the child environment so nothing is preloaded."""

    def __init__(self, node, script):
        self.node, self.script = str(node), str(script)
        self.env = {k: v for k, v in os.environ.items() if k not in ('NODE_OPTIONS', 'NODE_PATH')}
        self.calls = 0

    def version(self):
        p = subprocess.run([self.node, '--version'], input=b'', capture_output=True, timeout=TIMEOUT_S, shell=False, env=self.env)
        return p.stdout.decode('utf-8', 'replace').strip() if p.returncode == 0 else None

    def run(self, raw):
        self.calls += 1
        try:
            p = subprocess.run([self.node, self.script], input=raw, capture_output=True, timeout=TIMEOUT_S, shell=False, env=self.env)
        except (OSError, subprocess.TimeoutExpired) as e:
            raise CodecError('codec unavailable: %s' % type(e).__name__)
        if len(p.stdout) > STDOUT_MAX:
            raise CodecError('codec output too large')
        try:
            rep = json.loads(p.stdout.decode('utf-8', 'strict'))
        except (UnicodeDecodeError, ValueError):
            raise CodecError('codec report unreadable (exit %d)' % p.returncode)
        if p.returncode != 0 or not isinstance(rep, dict) or rep.get('ok') is not True or set(rep) != REPORT_KEYS:
            raise CodecError('codec refused: %r' % (rep.get('error') if isinstance(rep, dict) else rep))
        if rep['changed'] != (rep['textForChecker'] is not None) or rep['changed'] != (rep['messageClipped'] or rep['dataDropped']):
            raise CodecError('codec report inconsistent')
        return rep


def receive_guard(G30, B04, codec, raw, method, request_id, declared_length=None):
    """raw: bytes. Returns (text_for_checker, info) or raises G30.GuardFail / CodecError."""
    def fail(reason, stage):
        raise G30.GuardFail(reason, stage, method, request_id)

    limit = G30.RECV_LIMIT[method]
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
    if B04.json_depth(text) > G30.DEPTH_MAX:
        fail('recvDepth', 'depth')
    try:
        msg = json.loads(text, object_pairs_hook=B04._pairs, parse_int=B04._parse_int,
                         parse_float=B04.FloatTok, parse_constant=B04._no_constants)
    except (B04._Reject, ValueError, RecursionError):
        fail('recvParse', 'parseStrict')
    if not isinstance(msg, dict):
        fail('envelope', 'notObject')
    if 'id' not in msg:
        fail('envelope', 'idMissing')
    n = G30.integral_value(B04, msg['id'])
    if n is None or not 0 <= n <= G30.ID_MAX or n != request_id:
        fail('envelope', 'idMismatch')
    info = {'bytes': len(raw), 'codecCalled': False, 'messageUnits': None, 'clippedMessageUnits': None, 'dataBytes': None,
            'dataDropped': False, 'passedOriginal': True}
    out = text
    if isinstance(msg.get('error'), dict):
        rep = codec.run(raw)
        info.update(codecCalled=True, messageUnits=rep['messageUnits'], dataBytes=rep['dataBytes'], dataDropped=rep['dataDropped'],
                    clippedMessageUnits=G30.MESSAGE_UNITS_MAX if rep['messageClipped'] else None, node=rep['node'])
        if rep['changed']:
            out = rep['textForChecker']
            info['passedOriginal'] = False
    return out, info


class GuardedRpc:
    """0.30 GuardedRpc with the 0.31 receive guard. script = [(latency | None, raw bytes | str[, declaredLength])]."""

    def __init__(self, G30, O, B04, codec, budget, script):
        self.G30, self.O, self.B04, self.codec, self.b, self.script = G30, O, B04, codec, budget, list(script)
        self.calls, self.ids, self.guard, self.texts = [], [], [], []

    def call(self, method, params):
        rid = self.G30.REQUEST_ID[method]
        self.calls.append({'t': self.b.t, 'method': method, 'params': list(params)})
        self.ids.append(rid)
        item = self.script.pop(0) if self.script else (0, self.O.UNSCRIPTED)
        lat, raw = item[0], item[1]
        declared = item[2] if len(item) > 2 else None
        self.b.reply(lat, method)
        if isinstance(raw, str):
            raw = raw.encode('utf-8', 'surrogatepass')
        try:
            text, info = receive_guard(self.G30, self.B04, self.codec, raw, method, rid, declared)
        except self.G30.GuardFail:
            self.guard.append({'codecCalled': False})
            raise
        self.guard.append(info)
        self.texts.append(text)
        return text

    def sleep(self, ms):
        self.b.sleep(ms, 'retryAfterMs')


def guarded_header_net_check(V, O, B04, G30, codec, cfg, script, clock_s, compute_ms, t0=0, cache=True, ctr=None):
    """The unchanged 0.27 check_window behind the 0.31 receive guard, inside the 0.29 virtual budget."""
    b = O.Budget(t0)
    rpc = GuardedRpc(G30, O, B04, codec, b, script)
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
    except G30.GuardFail as g:
        guard = {'reason': g.reason, 'stage': g.stage, 'method': g.method, 'requestId': g.request_id}
        verdict, at, why = O._vi({'receiveGuard': guard}), b.elapsed(), 'receiveGuard'
    return {'verdict': verdict, 'atMs': at, 'why': why, 'guard': guard, 'sleptMs': b.slept, 'requests': rpc.calls, 'ids': rpc.ids,
            'guardInfo': rpc.guard, 'texts': rpc.texts, 'counters': ctr}
