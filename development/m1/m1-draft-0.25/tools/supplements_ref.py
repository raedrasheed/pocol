"""Reference helpers for the M1 draft 0.25 supplements and representation alternatives.
SPECIFICATION FIXTURE TOOLING ONLY. Python standard library only. NOT executed by the author.

Every helper is labelled with the supplement or proposal it serves. None of them is approved.

  S25-SWEEPMAX  sweepmax_world(SR): the BR22a/BR23 negative control 'sweepMax' (validation.md:682)
                as a subclass of the 0.12 World; the World itself is not changed.
  S25-LCLIT     RecordingServer / LiteralServer: literal request -> reply transcripts for the 0.19
                LogClient cases, and a replay server that serves only those literals.
  S25-RG3B      HookConsumer: the RG3b hook protocol (validation.md:1007) on the reference LogClient.
  S25-E07       select_version(): the 0.2 section 4.3 version-selection table as a pure function.
  CR-E4-01      hex16 epoch in the fmt-2 value (alternative A).
  CR-E4-02      WTF-8 over UTF-16 code units (alternative C), TextEncoder length, lone-surrogate test.
  CID-1         exact JSON integer reading of the profile chainId versus IEEE-754 double reading.
"""

import copy
import json
import re

U64_MAX = 2 ** 64 - 1


# ------------------------------------------------------------------ S25-SWEEPMAX

def br22a_positions(combo):
    """Times of L1..L5 for a BR22a combination, as m1-draft-0.10/tools/run_checks_010.py br22a_events."""
    td = {'P0': 10, 'P1': 20, 'P2': 20, 'P3': 5020, 'fail': 20}[combo['first']]
    r = td + 50
    if combo['second'] == 'P4':
        r = r + 9 + 50
    elif combo['second'] == 'P5':
        r = r + 7 + 50
    rR = r
    rW = rR + 9 + 50 if combo['extraDeath'] == 'yes' else rR
    return {'L1': rR - 1, 'L2': rR + 2, 'L3': rR + 5, 'L4': rR + 8, 'L5': rW + 15}


def sweepmax_world(SR):
    """Return a World subclass with an optional sweepMax fault (S25-SWEEPMAX).

    sweepMax: when a data record (E, s) is issued, the key's sweep runs at once against (E, s),
    so every record below the one being written is removed, the current maximum included,
    before the new record is applied. The correct sweep (browser.md:542) runs only after a
    record succeeds (World._sweep_records after _settle_data / _settle_cp).
    P-SM-1 (convention): a remove issued together with a set of a generation whose records do not
    settle by themselves (cfg settleRecords = False, the BR22a gen1) is held exactly like that set,
    until the schedule places it ('place' event, category 'remove'). Otherwise it settles 1 ms later,
    as every other remove of the World does.
    With sweep_max False the subclass adds nothing: it is the World."""
    base = SR.World
    B = SR.BASE_MODULE

    class SweepMaxWorld(base):
        def __init__(self, *a, sweep_max=False, **kw):
            base.__init__(self, *a, **kw)
            self.sweep_max = sweep_max

        def _issue_data(self, g, key, msg):
            base._issue_data(self, g, key, msg)
            if self.sweep_max:
                self._sweep_at_issue(g, key, self._ks(g, key)['inflight'][3])

        def _sweep_at_issue(self, g, key, ver):
            net, addr = self.sites[key]
            for n in sorted(self.be.items):
                v = B.parse_record_name(n, net, addr)
                if v and v < ver:
                    op = self.be.issue(g.id, 'remove', n, None, self.t)
                    self._trace('sweepAtIssue', gen=g.id, name=n)
                    if g.cfg.get('settleRecords', True):
                        self.at(self.t + 1, 2, self._settle_remove, g, op)

    return SweepMaxWorld


def br22a_run(R10, cls, doc, combo, sweep_max=False):
    """br22a_world of run_checks_010 with a chosen World class and the extra pending label RM
    (gen1 removes, placed at L like A). Returns (world, events, R, W)."""
    init = doc['initial']
    items = dict(init['items'])
    base_L = {k: v for k, v in combo['L'].items() if k != 'RM'}
    ev, until, R, W, _ = R10.br22a_events(combo['first'], combo['second'], combo['extraDeath'], base_L)
    rm = combo['L'].get('RM', 'L0')
    if rm != 'L0':
        ev = list(ev) + [{'t': br22a_positions(combo)[rm], 'do': 'place', 'gen': 1, 'cat': 'remove', 'action': 'apply'}]
    w = cls(items=items, faults=(), sweep_max=sweep_max)
    g1 = init['gen1']
    w.preload_gen(1, g1['boot'], g1['E'], {'settleRecords': False}, 'SA', g1['dict'], tuple(g1['version']), g1['seqNext'], g1['tabs'])
    w.load(ev)
    w.run(until)
    return w, ev, R, W


def world_fingerprint(w):
    return json.dumps([sorted(w.be.items), [s['dict'] for s in w.snapshots], [[r['id'], r['code']] for r in w.replies],
                       [v['kind'] for v in w.violations], [[x['ev'], x.get('gen'), x.get('name'), x['t']] for x in w.trace]],
                      sort_keys=True, default=str)


# ------------------------------------------------------------------ S25-LCLIT

class RecordingServer:
    """Wraps the 0.19 MockLogServer and records every served request with its literal reply."""

    def __init__(self, inner):
        self.inner = inner
        self.transcript = []

    @property
    def timeline(self):
        return self.inner.timeline

    @property
    def log(self):
        return self.inner.log

    def handle(self, req, t):
        reply = self.inner.handle(req, t)
        e = self.inner.log[-1]
        self.transcript.append({'k': e['k'], 't': t, 'req': list(req), 'branch': e['branch'], 'reply': copy.deepcopy(reply)})
        return reply


class LiteralServer:
    """Serves request k from a literal transcript only. A request that differs from the transcript
    (method, range, anchor or time) is answered -32603 literalReplayMismatch and recorded."""

    def __init__(self, transcript, timeline=None):
        self.script = transcript
        self.timeline = timeline
        self.log = []
        self.mismatches = []

    def handle(self, req, t):
        k = len(self.log) + 1
        e = self.script[k - 1] if k <= len(self.script) else None
        self.log.append({'k': k, 't': t, 'req': list(req), 'branch': e['branch'] if e else None})
        if self.timeline is not None:
            self.timeline.add('req', k=k, t=t, req=list(req))
        if e is None or e['req'] != list(req) or e['t'] != t:
            self.mismatches.append({'k': k, 't': t, 'req': list(req), 'expected': None if e is None else [e['t'], e['req']]})
            return {'error': {'code': -32603, 'data': {'reason': 'literalReplayMismatch'}}}
        return copy.deepcopy(e['reply'])


def run_lc(M, Tee, case, server, faults=()):
    """run_case of m1-draft-0.19/tools/run_checks_019.py with the server passed in."""
    clock = M.Clock()
    ref, faulty = M.RefConsumer(), M.FaultyConsumer()
    out = {'mock': server, 'timeline': server.timeline, 'ref': ref, 'faulty': faulty, 'client': None, 'sink': None}
    if case['mode'] == 'bridge':
        out['bridgeReply'] = M.Bridge(server, clock).eth_getLogs(case['from'], case['to'])
        return out
    if case['mode'] == 'fetchAll':
        cl, sink, res = M.fetch_all(server, clock, case['from'], case['to'], faults=faults)
    else:
        params = dict(M.FETCH_EACH)
        params.update(case.get('params', {}))
        cl, sink, res = M.run_client(server, clock, params, case['from'], case['to'], Tee(ref, faulty), faults)
        res = {'ok': True, 'logs': ref.result} if res['ok'] else {'ok': False, 'error': res['error']}
    out.update(client=cl, sink=sink, result=res)
    return out


def run_outcome(run):
    """The observable outcome of a run: requests with times, sink events, result or bridge reply."""
    out = {'requests': [[e['t']] + list(e['req']) for e in run['mock'].log]}
    if 'bridgeReply' in run:
        out['bridgeReply'] = run['bridgeReply']
    else:
        out['events'] = run['sink'].events
        out['result'] = run['result']
    return out


# ------------------------------------------------------------------ S25-RG3B

class HookConsumer:
    """RG3b hook (S25-RG3B, model form): the sink call that matches 'on' does not return until the
    branch switch is done; the switch stands for 'submit B and wait until it is adopted'."""

    def __init__(self, inner, server, hooks):
        self.inner, self.server = inner, server
        self.hooks = [dict(h, fired=False) for h in hooks]

    def on(self, ev):
        self.inner.on(ev)
        for h in self.hooks:
            on = h['on']
            if not h['fired'] and ev[0] == on[0] and ev[1] == on[1] and (len(on) < 3 or ev[2] == on[2]):
                self.server.branch = h['switchTo']
                h['fired'] = True


def event_skeleton(events):
    out = []
    for e in events:
        if e[0] in ('piece', 'abort'):
            out.append([e[0], e[1], e[2]])
        else:
            out.append([e[0], e[1]])
    return out


# ------------------------------------------------------------------ S25-E07

def select_version(state, request, revoked_policy):
    """0.2 section 4.3. state = {versionCount, currentVersion, status: {"n": draft|published|revoked}};
    request = {kind: default} | {kind: explicit, n} | {kind: malformed}.
    revoked_policy: 'interstitial' (U02 preferred, P) or 'refuse' (U02 alternative)."""
    if request['kind'] == 'malformed':
        return {'result': 'malformedSelector', 'frame': False, 'rpc': False}
    status = state['status']
    cur = state['currentVersion']
    if request['kind'] == 'default':
        if cur == 0:
            return {'result': 'noPublishedSite', 'frame': False}
        if status.get(str(cur)) != 'published':
            return {'result': 'stateInvariant', 'frame': False}
        return {'result': 'load', 'version': cur, 'frame': True, 'banner': None}
    n = request['n']
    if n > state['versionCount']:
        return {'result': 'versionNotFound', 'frame': False}
    st = status[str(n)]
    if st == 'draft':
        return {'result': 'refuseDraft', 'frame': False}
    if st == 'published':
        return {'result': 'load', 'version': n, 'frame': True, 'banner': None if n == cur else 'notCurrent'}
    if revoked_policy == 'refuse':
        return {'result': 'refuseRevoked', 'frame': False}
    return {'result': 'interstitialRevoked', 'frame': False, 'chunkFetches': 0,
            'afterExplicitClick': {'result': 'load', 'version': n, 'frame': True, 'banner': 'revoked'}}


# ------------------------------------------------------------------ CR-E4-01 (alternative A: hex16 epoch)

_HEX16 = re.compile(r'[0-9a-f]{16}')


class ReprError(ValueError):
    def __init__(self, reason):
        super().__init__(reason)
        self.reason = reason


def encode_value_hex(C, E, seq, d, tomb=False):
    """fmt-2 value with epoch as the same 16 lowercase hex digits as the record name."""
    if type(E) is not int or not 1 <= E <= U64_MAX:
        raise ReprError('epochRange')
    if type(seq) is not int or not 0 <= seq <= C.SEQ_MAX:
        raise ReprError('seqRange')
    if type(tomb) is not bool:
        raise ReprError('tombType')
    if tomb and d:
        raise ReprError('tombWithPairs')
    return {'fmt': 2, 'epoch': format(E, '016x'), 'seq': seq, 'tomb': tomb, 'b64': C.b64_encode(C.serialize_pairs(d))}


def decode_value_hex(C, ver, value):
    if type(value) is not dict:
        raise ReprError('notObject')
    if set(value) != C.VALUE_KEYS:
        raise ReprError('fields')
    if not (type(value['fmt']) is int and value['fmt'] == 2):
        raise ReprError('fmt')
    ep = value['epoch']
    if not (type(ep) is str and _HEX16.fullmatch(ep) and 1 <= int(ep, 16) and int(ep, 16) == ver[0]):
        raise ReprError('epoch')
    sq = value['seq']
    if not (type(sq) is int and 0 <= sq <= C.SEQ_MAX and sq == ver[1]):
        raise ReprError('seq')
    if type(value['tomb']) is not bool:
        raise ReprError('tomb')
    try:
        d = C.parse_pairs(C.b64_decode_strict(value['b64']))
    except C.CodecError as e:
        raise ReprError(e.reason)
    if value['tomb'] and d:
        raise ReprError('tombWithPairs')
    return d, value['tomb']


def js_number_int(s):
    """The integer a JS Number holds after JSON.parse of the integer literal s: the nearest
    IEEE-754 binary64 value (round half to even), as Python float(s) also computes."""
    return int(float(s))


def js_like_loads(text):
    """JSON.parse model: every integer literal becomes the integer value of its nearest double."""
    return json.loads(text, parse_int=js_number_int)


# ------------------------------------------------------------------ CR-E4-02 (alternative C: WTF-8)

def code_units(s):
    """UTF-16 code units of a Python string. Lone surrogate code points stay single units."""
    out = []
    for ch in s:
        cp = ord(ch)
        if cp > 0xFFFF:
            cp -= 0x10000
            out += [0xD800 + (cp >> 10), 0xDC00 + (cp & 0x3FF)]
        else:
            out.append(cp)
    return out


def units_from_hex(h):
    return [int(h[i:i + 4], 16) for i in range(0, len(h), 4)]


def has_lone_surrogate(u):
    i = 0
    while i < len(u):
        c = u[i]
        if 0xD800 <= c <= 0xDBFF and i + 1 < len(u) and 0xDC00 <= u[i + 1] <= 0xDFFF:
            i += 2
            continue
        if 0xD800 <= c <= 0xDFFF:
            return True
        i += 1
    return False


def _utf8_of(cp):
    if cp < 0x80:
        return bytes([cp])
    if cp < 0x800:
        return bytes([0xC0 | cp >> 6, 0x80 | cp & 0x3F])
    if cp < 0x10000:
        return bytes([0xE0 | cp >> 12, 0x80 | (cp >> 6) & 0x3F, 0x80 | cp & 0x3F])
    return bytes([0xF0 | cp >> 18, 0x80 | (cp >> 12) & 0x3F, 0x80 | (cp >> 6) & 0x3F, 0x80 | cp & 0x3F])


def wtf8_encode(u):
    """WTF-8: a valid surrogate pair is one 4-byte sequence; a lone surrogate is its own 3-byte
    sequence (ED A0..BF xx). For a string without lone surrogates this is exactly UTF-8."""
    out, i = bytearray(), 0
    while i < len(u):
        c = u[i]
        if 0xD800 <= c <= 0xDBFF and i + 1 < len(u) and 0xDC00 <= u[i + 1] <= 0xDFFF:
            out += _utf8_of(0x10000 + ((c - 0xD800) << 10) + (u[i + 1] - 0xDC00))
            i += 2
        else:
            out += _utf8_of(c)
            i += 1
    return bytes(out)


def wtf8_decode(b):
    """Strict WTF-8 decoder to UTF-16 code units. Rejects: invalid lead, truncation, bad continuation,
    overlong forms, values above U+10FFFF, and a lead-surrogate sequence directly followed by a
    trail-surrogate sequence (that pair has exactly one canonical form, the 4-byte one)."""
    out, i, n, prev_lead = [], 0, len(b), False
    while i < n:
        x = b[i]
        if x < 0x80:
            ln, cp = 1, x
        elif 0xC2 <= x <= 0xDF:
            ln, cp = 2, x & 0x1F
        elif 0xE0 <= x <= 0xEF:
            ln, cp = 3, x & 0x0F
        elif 0xF0 <= x <= 0xF4:
            ln, cp = 4, x & 0x07
        else:
            raise ReprError('invalidLead')
        if i + ln > n:
            raise ReprError('truncated')
        for j in range(1, ln):
            c = b[i + j]
            if c & 0xC0 != 0x80:
                raise ReprError('invalidContinuation')
            cp = cp << 6 | c & 0x3F
        if (ln == 3 and cp < 0x800) or (ln == 4 and cp < 0x10000):
            raise ReprError('overlong')
        if cp > 0x10FFFF:
            raise ReprError('outOfRange')
        lead, trail = 0xD800 <= cp <= 0xDBFF, 0xDC00 <= cp <= 0xDFFF
        if trail and prev_lead:
            raise ReprError('nonCanonicalPair')
        prev_lead = lead
        if cp > 0xFFFF:
            cp -= 0x10000
            out += [0xD800 + (cp >> 10), 0xDC00 + (cp & 0x3FF)]
        else:
            out.append(cp)
        i += ln
    return out


def textencoder_len(u):
    """Byte length TextEncoder produces (lone surrogate -> U+FFFD, 3 bytes; implementation.md:206)."""
    n, i = 0, 0
    while i < len(u):
        c = u[i]
        if 0xD800 <= c <= 0xDBFF and i + 1 < len(u) and 0xDC00 <= u[i + 1] <= 0xDFFF:
            n += 4
            i += 2
            continue
        n += 3 if 0xD800 <= c <= 0xDFFF else (1 if c < 0x80 else 2 if c < 0x800 else 3)
        i += 1
    return n


def wtf8_pairs(d_units):
    """fmt-2 pair bytes with WTF-8 keys and values, sorted by key bytes. d_units = [[key units, value units], ...]."""
    pairs = sorted(((wtf8_encode(k), wtf8_encode(v)) for k, v in d_units), key=lambda p: p[0])
    keys = [k for k, _ in pairs]
    if len(set(keys)) != len(keys):
        raise ReprError('duplicateKey')
    return b''.join(len(k).to_bytes(4, 'big') + k + len(v).to_bytes(4, 'big') + v for k, v in pairs)


def wtf8_parse_pairs(b):
    out, pos, prev = [], 0, None
    while pos < len(b):
        if pos + 4 > len(b):
            raise ReprError('truncated')
        lk = int.from_bytes(b[pos:pos + 4], 'big')
        k = b[pos + 4:pos + 4 + lk]
        pos += 4 + lk
        if pos + 4 > len(b):
            raise ReprError('truncated')
        lv = int.from_bytes(b[pos:pos + 4], 'big')
        v = b[pos + 4:pos + 4 + lv]
        pos += 4 + lv
        if len(k) != lk or len(v) != lv:
            raise ReprError('truncated')
        if prev is not None and k <= prev:
            raise ReprError('duplicateKey' if k == prev else 'unsortedKeys')
        prev = k
        out.append([wtf8_decode(k), wtf8_decode(v)])
    return out


# ------------------------------------------------------------------ CID-1 (profile chainId)

_INT_LITERAL = re.compile(r'[1-9][0-9]{0,19}')


def chainid_exact(text):
    """Exact reading (CID-1 alternative A): the profile's chainId token must be a JSON integer literal
    without sign, fraction or exponent, read exactly, within [1, 2^64-1] (consensus.md:9)."""
    def keep(s):
        return ('int', s)

    def reject_float(s):
        return ('nonInteger', s)
    try:
        doc = json.loads(text, parse_int=keep, parse_float=reject_float)
    except ValueError:
        return {'ok': False, 'reason': 'json'}
    tok = doc.get('chainId') if type(doc) is dict else None
    if type(tok) is not tuple:
        return {'ok': False, 'reason': 'type'}
    kind, s = tok
    if kind != 'int' or not _INT_LITERAL.fullmatch(s):
        return {'ok': False, 'reason': 'notCanonicalInteger'}
    v = int(s)
    if not 1 <= v <= U64_MAX:
        return {'ok': False, 'reason': 'range'}
    return {'ok': True, 'value': v}


def chainid_double(text):
    """What a reader that goes through IEEE-754 doubles (JSON.parse to Number) obtains, then the
    same range rule."""
    try:
        doc = json.loads(text, parse_int=js_number_int, parse_float=float)
    except ValueError:
        return {'ok': False, 'reason': 'json'}
    v = doc.get('chainId') if type(doc) is dict else None
    if type(v) is float:
        if not v.is_integer():
            return {'ok': False, 'reason': 'notInteger'}
        v = int(v)
    if type(v) is not int:
        return {'ok': False, 'reason': 'type'}
    if not 1 <= v <= U64_MAX:
        return {'ok': False, 'reason': 'range', 'value': v}
    return {'ok': True, 'value': v}
