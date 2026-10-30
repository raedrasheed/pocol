"""Reference model of BridgeAuth.check (Bpre, B0-B3) and the pendingReads counter.

Draft 0.4, annex batch 2 (R4-06: B1, B2, B3, B5). Specification tooling only:
an executable reading of FD:L1108-1205 and FD:L4858-4862, used to make the
literal fixtures falsifiable. It is not the extension's BridgeAuth and not the
BridgeRef of FD:L2659 (batch 4), though it is written to be extended into it.

check(raw, arrival_ms, bucket) returns
  {'bpre': 'pass'|'size'|'rate', 'stage': 'B0'..'B3'|'ok'|None,
   'code': int|None, 'data': dict|None, 'id': int|None, 'route': str|None,
   'tokensBefore': int|None, 'tokensAfter': int|None}
"""

import json
import re
from pathlib import Path

BRIDGE_MSG_MAX = 65536          # FD:L1128 (64 KiB); P: counted in UTF-8 bytes
BUCKET_CAP_MT = 50000           # FD:L1130
REFILL_MT_PER_MS = 50           # FD:L1131
MSG_COST_MT = 1000              # FD:L1132
DEPTH_MAX = 8                   # FD:L1134
ID_MAX = 2 ** 32 - 1            # FD:L1110
PENDING_SESSION_MAX = 4         # FD:L1140
PENDING_GLOBAL_MAX = 16         # FD:L1140
KINDS = ('rpc_read', 'wallet_req', 'storage_set', 'nav')

MATRIX_FILE = Path(__file__).resolve().parent.parent / 'annex' / 'bridge-matrix.json'
DOC = json.loads(MATRIX_FILE.read_text(encoding='utf-8'))
PRIM, COMP = DOC['primitives'], DOC['composites']
MATRIX = {k: dict(v) for k, v in DOC['matrix'].items()}     # exact str keys: no prototype lookup
ID_TOKEN = re.compile(r'(0|[1-9][0-9]*)\Z')


class IntTok(int):
    """JSON integer token, keeping its raw text."""
    raw = ''


class FloatTok(float):
    """JSON number with a fraction or exponent."""


class _Reject(Exception):
    pass


def _parse_int(s):
    v = IntTok(int(s))
    v.raw = s
    return v


def _no_constants(s):
    raise _Reject('constant %s' % s)              # NaN, Infinity, -Infinity are not JSON


def _pairs(pairs):
    seen = set()
    for k, _ in pairs:
        if k in seen:
            raise _Reject('duplicate key %r' % k)
        seen.add(k)
    return dict(pairs)


def json_depth(text):
    """Container nesting depth by a lexical scan (no recursion); strings are skipped."""
    depth = best = 0
    in_str = esc = False
    for ch in text:
        if in_str:
            if esc:
                esc = False
            elif ch == '\\':
                esc = True
            elif ch == '"':
                in_str = False
        elif ch == '"':
            in_str = True
        elif ch in '[{':
            depth += 1
            best = max(best, depth)
        elif ch in ']}':
            depth -= 1
    return best


def parse_strict(text):
    """B0 parseStrict: valid JSON, no duplicate keys at any level, depth <= 8."""
    if json_depth(text) > DEPTH_MAX:
        raise _Reject('depth')
    try:
        return json.loads(text, object_pairs_hook=_pairs, parse_int=_parse_int,
                          parse_float=FloatTok, parse_constant=_no_constants)
    except _Reject:
        raise
    except (ValueError, RecursionError) as e:
        raise _Reject(str(e))


def utf8_len(text):
    return len(text.encode('utf-8', 'surrogatepass'))


def utf16_prefix(s, units=64):
    """First `units` UTF-16 code units, as JavaScript String.prototype.slice would give."""
    b = s.encode('utf-16-le', 'surrogatepass')[:2 * units]
    return b.decode('utf-16-le', 'surrogatepass')


def valid_id(v):
    return isinstance(v, IntTok) and ID_TOKEN.match(v.raw) is not None and v <= ID_MAX


# --- B2 message bucket (FD:L1129-1133) --------------------------------------
class Bucket:
    def __init__(self, created_ms):
        self.tokens_mt, self.last_ms = BUCKET_CAP_MT, created_ms

    def take(self, arrival_ms):
        """Refill then try to consume. Returns (passed, tokensBefore, tokensAfter)."""
        before = self.tokens_mt
        elapsed = max(0, arrival_ms - self.last_ms)          # P: defensive; arrival is monotone
        self.tokens_mt = min(BUCKET_CAP_MT, self.tokens_mt + REFILL_MT_PER_MS * elapsed)
        self.last_ms = arrival_ms
        if self.tokens_mt >= MSG_COST_MT:
            self.tokens_mt -= MSG_COST_MT
            return True, before, self.tokens_mt
        return False, before, self.tokens_mt


# --- B3 schemas ------------------------------------------------------------
def _resolve(spec):
    if isinstance(spec, str):
        return PRIM.get(spec) or COMP[spec]
    return spec


def _is_num(v):
    return isinstance(v, (IntTok, FloatTok)) and not isinstance(v, bool)


def _https_url(v):
    """Approximation of the FD:L1165 WHATWG properties (see bridge-matrix.json modelNote)."""
    if not isinstance(v, str) or len(v) > 2048 or not v.startswith('https://'):
        return False
    if any(not 0x21 <= ord(c) <= 0x7e for c in v):
        return False
    rest = v[8:]
    cut = min([i for i in (rest.find('/'), rest.find('?'), rest.find('#'), rest.find('\\')) if i >= 0],
              default=len(rest))
    authority = rest[:cut]
    if '@' in authority:
        return False                                   # username or password present
    host = authority
    if host.startswith('['):
        end = host.find(']')
        if end < 0:
            return False
        port, host = host[end + 1:], host[:end + 1]
        if port and not (port.startswith(':') and port[1:].isdigit() and int(port[1:]) <= 65535):
            return False
    elif ':' in host:
        host, port = host.rsplit(':', 1)
        if port and not (port.isdigit() and int(port) <= 65535):
            return False
    return len(host) > 0


def accepts(v, spec):
    """Deep acceptance test for a non-object position."""
    s = _resolve(spec)
    if 'regex' in s:
        if not isinstance(v, str) or not re.fullmatch(s['regex'], v):   # no '$'-before-newline gap
            return False
        if 'maxBytes' in s and (len(v) - 2) // 2 > s['maxBytes']:
            return False
        return True
    if 'type' in s:
        return accepts(v, s['type']) and ('max' not in s or int(v, 16) <= s['max'])
    if 'enum' in s:
        return isinstance(v, str) and v in s['enum']
    if 'const' in s:
        return v is s['const'] if s['const'] in (None, False, True) else v == s['const']
    if 'oneOf' in s:
        return any(accepts(v, alt) for alt in s['oneOf'])
    if 'str' in s:
        if not isinstance(v, str):
            return False
        try:
            n = len(v.encode('utf-8'))                     # strict: lone surrogates fail
        except UnicodeEncodeError:
            return False
        return 1 <= n <= s['str']
    if 'number' in s:
        return _is_num(v) and s['number']['min'] <= v <= s['number']['max']
    if 'array' in s:
        a = s['array']
        if not isinstance(v, list) or len(v) < a.get('min', 0) or len(v) > a['max']:
            return False
        if not all(accepts(x, a['items']) for x in v):
            return False
        return not a.get('nonDecreasing') or all(x <= y for x, y in zip(v, v[1:]))
    if s.get('rule', '').startswith('printable ASCII 0x21'):      # pathStr
        return isinstance(v, str) and 1 <= len(v) <= 2048 and v.startswith('/') and \
            all(0x21 <= ord(c) <= 0x7e for c in v)
    if 'https://' in s.get('rule', ''):
        return _https_url(v)
    raise KeyError('unhandled spec %r' % (s,))


def _object_error(v, spec, i):
    o = spec['object']
    base = 'params[%d]' % i
    if not isinstance(v, dict):
        return base
    for k in v:                                           # input order (dicts keep it)
        if k not in o['fields']:
            return '%s.%s' % (base, k)
    for k in o['fields']:
        if k in o.get('required', ()) and k not in v:
            return '%s.%s' % (base, k)
    for k, fs in o['fields'].items():
        if k in v and not accepts(v[k], fs):
            return '%s.%s' % (base, k)
    for rule in o.get('exclusive', ()):
        if all(f in v for f in rule['fields']):
            return '%s.%s' % (base, rule['path'])
    return None


def element_error(v, spec, i):
    s = _resolve(spec)
    if 'select' in s:
        sel = s['select']
        name = sel['then'] if isinstance(v, dict) and sel['ifHasKey'] in v else sel['else']
        return _object_error(v, COMP[name], i)
    if 'object' in s:
        return _object_error(v, s, i)
    return None if accepts(v, spec) else 'params[%d]' % i


def params_error(params, mspec):
    if 'thirdParamPath' in mspec and len(params) >= 3:      # U32 reconciliation of BR8 and BR17d
        return mspec['thirdParamPath']
    if 'paramsOneOf' in mspec:
        alts = [a for a in mspec['paramsOneOf'] if len(a) == len(params)]
        if not alts:
            return 'params'
        alt = alts[0]
    else:
        alt = mspec['params']
        if len(params) != len(alt):
            return 'params'
    for i, (v, t) in enumerate(zip(params, alt)):
        e = element_error(v, t, i)
        if e:
            return e
    return None


# --- check: Bpre, B0-B3 (FD:L1126-1137) -------------------------------------
def check(raw, arrival_ms, bucket):
    out = {'bpre': None, 'stage': None, 'code': None, 'data': None, 'id': None, 'route': None,
           'tokensBefore': None, 'tokensAfter': None}
    if utf8_len(raw) > BRIDGE_MSG_MAX:
        out.update(bpre='size', code=-32600, data={'reason': 'size'})
        return out                                         # bucket untouched, id null
    passed, before, after = bucket.take(arrival_ms)
    out.update(tokensBefore=before, tokensAfter=after)
    if not passed:
        out.update(bpre='rate', code=-32005, data={'reason': 'rate'})
        return out
    out['bpre'] = 'pass'
    try:
        msg = parse_strict(raw)
    except _Reject:
        out.update(stage='B0', code=-32600)
        return out
    rid = msg.get('id') if isinstance(msg, dict) else None
    out['id'] = int(rid) if valid_id(rid) else None
    ok = (isinstance(msg, dict) and set(msg) == {'id', 'kind', 'payload'} and valid_id(msg['id'])
          and isinstance(msg['kind'], str) and isinstance(msg['payload'], dict)
          and set(msg['payload']) == {'method', 'params'}
          and isinstance(msg['payload']['method'], str) and isinstance(msg['payload']['params'], list))
    if not ok:
        out.update(stage='B0', code=-32600)
        return out
    kind, method, params = msg['kind'], msg['payload']['method'], msg['payload']['params']
    if kind not in KINDS:
        out.update(stage='B1', code=-32600, data={'reason': 'kind'})
        return out
    mspec = MATRIX[kind].get(method) if type(method) is str else None
    if mspec is None:
        out.update(stage='B2', code=4200, data={'method': utf16_prefix(method)})
        return out
    path = params_error(params, mspec)
    if path:
        out.update(stage='B3', code=-32602, data={'path': path})
        return out
    route = {'rpc_read': 'readClient', 'wallet_req': 'wallet', 'storage_set': 'siteStorage',
             'nav': 'navigator'}[kind]
    out.update(stage='ok', route=mspec.get('route', route))
    return out


# --- pendingReads (FD:L1140, FD:L1146-1155) ----------------------------------
class Pending:
    """Session and global pendingReads counters with the baseline events.

    P (R4-06): 'acceptance' is passing the pending check; the counter is
    incremented there, before RecvGuard.reserve; a 'busy' reply is a final
    reply and decrements it.
    """

    def __init__(self):
        self.session, self.global_ = {}, 0
        self.inflight = {}                    # req -> (session, state)

    def route(self, sess, req):
        if self.session.get(sess, 0) >= PENDING_SESSION_MAX or self.global_ >= PENDING_GLOBAL_MAX:
            return {'code': -32005, 'data': {'reason': 'pending'}}
        self.session[sess] = self.session.get(sess, 0) + 1
        self.global_ += 1
        self.inflight[req] = (sess, 'live')
        return {'accepted': True}

    def _dec(self, req):
        sess, _ = self.inflight.pop(req)
        self.session[sess] -= 1
        self.global_ -= 1

    def final_reply(self, req):             # success, remote error, transport error, timeout, busy
        if self.inflight[req][1] != 'live':
            raise AssertionError('orphan requests get no reply')
        self._dec(req)

    def teardown_frame(self, reqs):         # requests become orphans; no reset, no reply
        for r in reqs:
            if r in self.inflight:
                self.inflight[r] = (self.inflight[r][0], 'orphan')

    def settle_orphan(self, req):
        if self.inflight[req][1] != 'orphan':
            raise AssertionError('only orphans settle without reply')
        self._dec(req)

    def internal_retry(self, req):          # LogClient retries do not count
        return None

    def counts(self, sess):
        return {'session': self.session.get(sess, 0), 'global': self.global_}
