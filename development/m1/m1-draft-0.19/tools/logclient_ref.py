"""V2 LogClient reference for M1 draft 0.19 (D89, D91). Python standard library only.
REFERENCE / SPECIFICATION FIXTURE TOOLING ONLY. NOT executed by the author. Not browser code.

Sources: reference/browser.md:74-141 (LogClient, sink contract, collectors);
validation.md:818-861 (LC-unit, LC1-LC18); FINAL_DESIGN.md:2040-2068 (-32020 shapes, P1/P3 lemmas,
pocol_getLogs -32022); implementation.md:30-32, 293, 359.

Components (manual clock; no sockets, no real RPC):
  step(range, reply, last, hashes)  pure transition (annex V2 section 3)
  LogClient                         attempt driver with counters, restart, pre-commit check
  MockLogServer                     scripted deterministic server over branches A / B / A'
  SinkChecker                       independent (S-a)..(S-e) checker over the shared timeline
  RefConsumer / FaultyConsumer      consumer semantics (drop aborted attempts / keep everything)
  fetch_all / fetch_each / Bridge   collectors and the page path
Faults (controls only): see FAULTS.
"""

import copy
import json

WINDOW = 1024
MAX_RESTARTS = 3
SINGLETON_RETRY_MS = 500
SINGLETON_RETRIES = 3
P1 = frozenset({'resultBytes', 'resultCount', 'scanBytes'})
RES = frozenset({'deadline', 'scratch'})
FETCH_ALL = {'maxBytes': 4194304, 'timeoutMs': 30000, 'maxRequests': 4096}
FETCH_EACH = {'maxBytes': None, 'timeoutMs': 600000, 'maxRequests': 65536}

FAULTS = frozenset({
    'tailFirst',              # Replace issues the tail before the prefix
    'markNotSet',             # the P1 prefix [a, lc] is not marked withinLimit
    'noMarkInherit',          # sub-ranges do not inherit withinLimit
    'countBusyRetries',       # -32021 increments the singleton retry counter
    'maxCheckAfterSend',      # maxRequests checked after sending
    'noPrecommitCheck',       # commit without eth_getBlockByNumber(anchor.number)
    'resetTotalOnRestart',    # totalRequests restarts at each attempt
    'acceptInvalidComplete',  # Complete is appended without validation
    'maxRestarts4',           # allows a fourth restart (five attempts)
})


def compact(v):
    return json.dumps(v, separators=(',', ':'), sort_keys=True, ensure_ascii=False)


def enc_bytes(logs):
    """P-V2-5: bytes of a piece = UTF-8 bytes of the compact, key-sorted JSON array of its logs."""
    return len(compact(logs).encode('utf-8'))


def _int(x):
    return type(x) is int


def _hexq(s):
    if type(s) is not str or not s.startswith('0x') or len(s) < 3:
        return None
    try:
        v = int(s[2:], 16)
    except ValueError:
        return None
    return v if '0x' + format(v, 'x') == s else None


def violation(why):
    return {'act': 'violation', 'why': why}


# ------------------------------------------------------------------ pure step

def check_complete(a, b, logs, last, hashes):
    if type(logs) is not list:
        return 'resultNotList'
    prev = last
    seen = dict(hashes)
    for lg in logs:
        if type(lg) is not dict:
            return 'malformedLog'
        bn, li, bh = _hexq(lg.get('blockNumber')), _hexq(lg.get('logIndex')), lg.get('blockHash')
        if bn is None or li is None or type(bh) is not str:
            return 'malformedLog'
        if not a <= bn <= b:
            return 'outOfRange'
        if prev is not None and not (bn, li) > prev:
            return 'notStrictlyIncreasing'
        if seen.get(bn, bh) != bh:
            return 'blockHashChanged'
        seen[bn] = bh
        prev = (bn, li)
    return None


def step(rng, reply, last=None, hashes=None, faults=frozenset()):
    """rng = {a, b, withinLimit, retries}. Returns one action dict (annex V2 section 3)."""
    a, b, mark, retries = rng['a'], rng['b'], rng['withinLimit'], rng['retries']
    if type(reply) is not dict:
        return violation('malformedReply')
    if 'result' in reply and 'error' not in reply:
        if 'acceptInvalidComplete' not in faults:
            bad = check_complete(a, b, reply['result'], last, hashes or {})
            if bad:
                return violation(bad)
        return {'act': 'append', 'logs': reply['result']}
    err = reply.get('error')
    if type(err) is not dict or not _int(err.get('code')):
        return violation('malformedError')
    code, data = err['code'], err.get('data')
    if code == -32020:
        if type(data) is not dict:
            return violation('limitWithoutData')
        r, lc = data.get('reason'), data.get('lastCompleteBlock')
        if r not in P1 and r not in RES:
            return violation('unknownReason')
        if not _int(lc):
            return violation('invalidLc')
        if data.get('fromBlock') != a or data.get('nextFromBlock') != lc + 1:
            return violation('inconsistentLimit')                         # P-V2-2 (proposed supplement)
        inherit = mark if 'noMarkInherit' not in faults else False
        if r in P1:
            if mark:
                return violation('p1OnWithinLimit')                         # lemma P3
            if not a <= lc <= b - 1:
                return violation('p1LcOutOfBounds')
            first_mark = True if 'markNotSet' not in faults else mark
            return {'act': 'replace', 'first': [a, lc, first_mark], 'second': [lc + 1, b, inherit]}
        if not a - 1 <= lc <= b - 1:
            return violation('resLcOutOfBounds')
        if lc >= a:
            return {'act': 'replace', 'first': [a, lc, inherit], 'second': [lc + 1, b, inherit]}
        if b > a:
            mid = a + (b - a) // 2
            return {'act': 'split', 'first': [a, mid, inherit], 'second': [mid + 1, b, inherit]}
        if retries >= SINGLETON_RETRIES:
            return {'act': 'error', 'reason': r, 'block': a}
        return {'act': 'retry', 'delayMs': SINGLETON_RETRY_MS, 'retries': retries + 1}
    if code == -32021:
        ms = data.get('retryAfterMs') if type(data) is dict else None
        if not _int(ms) or ms < 0 or data.get('reason') not in ('busy', 'rate'):
            return violation('malformedBusy')                               # P-V2-3
        return {'act': 'retry', 'delayMs': ms, 'retries': retries + 1 if 'countBusyRetries' in faults else retries}
    if code == -32022:
        if type(data) is not dict or data.get('reason') not in ('anchorNotCanonical', 'beyondAnchor'):
            return violation('malformedReorg')                              # P-V2-3
        return {'act': 'restart', 'reason': data['reason']}
    if code == -32601:
        return {'act': 'error', 'reason': 'unsupported'}
    return {'act': 'error', 'reason': 'rpcError', 'code': code}


# ------------------------------------------------------------------ clock, timeline

class Clock:
    def __init__(self, t=0):
        self.t = t

    def advance(self, ms):
        self.t += ms


class Timeline:
    """Shared order of every RPC request and sink call (SinkChecker attribution)."""

    def __init__(self):
        self.entries = []

    def add(self, kind, **kw):
        e = {'seq': len(self.entries), 'kind': kind}
        e.update(kw)
        self.entries.append(e)
        return e


# ------------------------------------------------------------------ MockLogServer

ADDRESS = '0x00000000000000000000000000000000000000c0'
Z54 = '0' * 54


class MockLogServer:
    """Branches: {name: {'code', 'divergeAt', 'logs': [[block, tag], ...]}}; shared code '5a' below
    divergeAt. hash(branch, h) = '0x' + code + format(h, '08x') + 54 zeros (fixture rule).
    script = {'replies': {k: reply}, 'switches': {k: branch}} keyed by the 1-based request number k,
    a switch applying BEFORE request k is served."""

    def __init__(self, branches, branch='A', head=3000, script=None, timeline=None, shared_code='5a'):
        self.branches, self.branch, self.head = branches, branch, head
        self.shared_code = shared_code
        sc = script or {}
        self.replies = {int(k): v for k, v in sc.get('replies', {}).items()}
        self.switches = {int(k): v for k, v in sc.get('switches', {}).items()}
        self.log = []
        self.timeline = timeline

    def hash_at(self, branch, h):
        br = self.branches[branch]
        code = self.shared_code if h < br['divergeAt'] else br['code']
        return '0x' + code + format(h, '08x') + Z54

    def make_log(self, branch, block, index, tag=None):
        if tag is None:
            tag = next(t for blk, t in self.branches[branch]['logs'] if blk == block)
        return {'address': ADDRESS, 'blockHash': self.hash_at(branch, block), 'blockNumber': hex(block),
                'data': '0x%02x%02x' % (tag, index), 'logIndex': hex(index), 'topics': []}

    def expand(self, v):
        if isinstance(v, dict):
            if 'gen' in v:
                g = v['gen']
                return self.make_log(g[0], g[1], g[2], g[3] if len(g) > 3 else None)
            if 'repeat' in v:
                return v['prefix'] + v['repeat'] * v['times']
            return {k: self.expand(x) for k, x in v.items()}
        if isinstance(v, list):
            return [self.expand(x) for x in v]
        if isinstance(v, str) and v.startswith('@hash:'):
            br, h = v[6:].split(':')
            return self.hash_at(br, int(h))
        return v

    def handle(self, req, t):
        k = len(self.log) + 1
        if k in self.switches:
            self.branch = self.switches[k]
        self.log.append({'k': k, 't': t, 'req': list(req), 'branch': self.branch})
        if self.timeline is not None:
            self.timeline.add('req', k=k, t=t, req=list(req))
        if k in self.replies:
            return copy.deepcopy(self.expand(self.replies[k]))
        if req[0] == 'H':
            n = self.head if req[1] == 'latest' else req[1]
            return {'result': {'number': hex(n), 'hash': self.hash_at(self.branch, n)}}
        _, a, b, anchor = req
        h = int(anchor[4:12], 16)
        if h > self.head or self.hash_at(self.branch, h) != anchor:
            return {'error': {'code': -32022, 'data': {'reason': 'anchorNotCanonical', 'head': self.head}}}
        if b > h:
            return {'error': {'code': -32022, 'data': {'reason': 'beyondAnchor', 'head': self.head}}}
        logs = [self.make_log(self.branch, blk, i) for blk, _ in self.branches[self.branch]['logs'] if a <= blk <= b for i in (0, 1)]
        return {'result': logs}


# ------------------------------------------------------------------ LogClient driver

class _Stop(Exception):
    def __init__(self, reason):
        super().__init__(reason)
        self.reason = reason


class LogClient:
    def __init__(self, server, sink, clock, *, max_requests, timeout_ms=None, max_bytes=None, max_restarts=MAX_RESTARTS, faults=()):
        unknown = set(faults) - FAULTS
        if unknown:
            raise ValueError('unknown faults %s' % sorted(unknown))
        self.server, self.sink, self.clock = server, sink, clock
        self.max_requests, self.timeout_ms, self.max_bytes = max_requests, timeout_ms, max_bytes
        self.max_restarts = max_restarts + (1 if 'maxRestarts4' in faults else 0)
        self.faults = frozenset(faults)
        self.total = 0
        self.steps = []                          # [attempt, a, b, withinLimit, act] for every log reply

    def _send(self, req):
        if self.timeout_ms is not None and self.clock.t - self.start > self.timeout_ms:
            raise _Stop('timeout')                                          # P-V2-4: inclusive deadline, checked at send
        if 'maxCheckAfterSend' not in self.faults and self.total + 1 > self.max_requests:
            raise _Stop('tooManyRequests')
        reply = self.server.handle(req, self.clock.t)
        self.total += 1
        if req[0] == 'H':
            self.head_requests += 1
        else:
            self.log_requests += 1
        if 'maxCheckAfterSend' in self.faults and self.total > self.max_requests:
            raise _Stop('tooManyRequests')
        return reply

    @staticmethod
    def _anchor(reply):
        r = reply.get('result') if type(reply) is dict else None
        n = _hexq(r.get('number')) if type(r) is dict else None
        if n is None or type(r.get('hash')) is not str:
            return None
        return {'number': n, 'hash': r['hash']}

    def fetch(self, frm, to):
        self.start = self.clock.t
        attempt, restarts = 0, 0
        while True:
            attempt += 1
            if 'resetTotalOnRestart' in self.faults:
                self.total = 0
            self.log_requests = self.head_requests = 0
            try:
                anchor = self._anchor(self._send(('H', 'latest')))
            except _Stop as s:
                return {'ok': False, 'error': {'reason': s.reason}}          # P-V2-1: no attempt open, no begin
            if anchor is None:
                return {'ok': False, 'error': {'reason': 'headViolation'}}   # P-V2-6
            self.sink.begin(attempt, anchor)
            if to > anchor['number']:
                self.sink.abort(attempt, 'beyondHead')
                return {'ok': False, 'error': {'reason': 'beyondHead'}}
            queue = [{'a': x, 'b': min(x + WINDOW - 1, to), 'withinLimit': False, 'retries': 0} for x in range(frm, to + 1, WINDOW)]
            seq = nlogs = nbytes = 0
            last, hashes = None, {}
            restart = False
            try:
                while queue:
                    r = queue[0]
                    reply = self._send(('L', r['a'], r['b'], anchor['hash']))
                    act = step(r, reply, last, hashes, self.faults)
                    self.steps.append([attempt, r['a'], r['b'], r['withinLimit'], act['act']])
                    k = act['act']
                    if k == 'append':
                        logs = act['logs']
                        nbytes += enc_bytes(logs)
                        if self.max_bytes is not None and nbytes > self.max_bytes:
                            raise _Stop('tooLarge')
                        self.sink.piece(attempt, seq, [r['a'], r['b']], logs)
                        seq += 1
                        nlogs += len(logs)
                        for lg in logs:
                            hashes[_hexq(lg['blockNumber'])] = lg['blockHash']
                        if logs:
                            last = (_hexq(logs[-1]['blockNumber']), _hexq(logs[-1]['logIndex']))
                        queue.pop(0)
                    elif k in ('replace', 'split'):
                        f, s = act['first'], act['second']
                        kids = [{'a': f[0], 'b': f[1], 'withinLimit': f[2], 'retries': 0},
                                {'a': s[0], 'b': s[1], 'withinLimit': s[2], 'retries': 0}]
                        if 'tailFirst' in self.faults:
                            kids.reverse()
                        queue[0:1] = kids
                    elif k == 'retry':
                        r['retries'] = act['retries']
                        self.clock.advance(act['delayMs'])
                    elif k == 'restart':
                        restart = True
                        break
                    elif k == 'error':
                        self.sink.abort(attempt, act['reason'])
                        err = {x: act[x] for x in ('reason', 'block', 'code') if x in act}
                        return {'ok': False, 'error': err}
                    else:
                        self.sink.abort(attempt, 'serverViolation')
                        return {'ok': False, 'error': {'reason': 'serverViolation', 'why': act['why']}}
                if not restart:
                    if 'noPrecommitCheck' not in self.faults:
                        chk = self._anchor(self._send(('H', anchor['number'])))
                        restart = chk is None or chk['hash'] != anchor['hash'] or chk['number'] != anchor['number']
                    if not restart:
                        self.sink.commit(attempt, {'pieces': seq, 'logs': nlogs, 'logRequests': self.log_requests,
                                                   'headRequests': self.head_requests, 'totalRequests': self.total, 'anchor': anchor})
                        return {'ok': True}
            except _Stop as s:
                self.sink.abort(attempt, s.reason)
                return {'ok': False, 'error': {'reason': s.reason}}
            if restarts >= self.max_restarts:
                self.sink.abort(attempt, 'reorgUnstable')
                return {'ok': False, 'error': {'reason': 'reorgUnstable'}}
            self.sink.abort(attempt, 'reorg')
            restarts += 1


# ------------------------------------------------------------------ sinks and consumers

class RecordingSink:
    """Records sink calls into the shared timeline and forwards them to a consumer."""

    def __init__(self, timeline, consumer=None):
        self.timeline, self.consumer, self.events = timeline, consumer, []

    def _ev(self, ev):
        self.events.append(ev)
        self.timeline.add('sink', ev=copy.deepcopy(ev))
        if self.consumer is not None:
            self.consumer.on(ev)

    def begin(self, i, anchor):
        self._ev(['begin', i, dict(anchor)])

    def piece(self, i, seq, rng, logs):
        self._ev(['piece', i, seq, list(rng), copy.deepcopy(logs)])

    def abort(self, i, reason):
        self._ev(['abort', i, reason])

    def commit(self, i, summary):
        self._ev(['commit', i, copy.deepcopy(summary)])


class RefConsumer:
    """Pieces are provisional until commit; abort drops every piece of that attempt."""

    def __init__(self):
        self.buf, self.result, self.discarded = {}, None, []

    def on(self, ev):
        if ev[0] == 'piece':
            self.buf.setdefault(ev[1], []).extend(ev[4])
        elif ev[0] == 'abort':
            self.discarded.append([ev[1], self.buf.pop(ev[1], [])])
        elif ev[0] == 'commit':
            self.result = self.buf.pop(ev[1], [])


class FaultyConsumer:
    """Control: keeps every piece of every attempt."""

    def __init__(self):
        self.all = []

    def on(self, ev):
        if ev[0] == 'piece':
            self.all.extend(ev[4])


class SinkChecker:
    """(S-a)..(S-e), browser.md:125-130, over the shared timeline. Returns violations [{rule, ...}]."""

    def check(self, entries, frm, to):
        out = []
        sinks = [e for e in entries if e['kind'] == 'sink']
        attempts, order, terminal = {}, [], {}
        commits = []
        expect_next = 1
        open_id = None
        for e in sinks:
            ev = e['ev']
            i = ev[1]
            if ev[0] == 'begin':
                if i in attempts:
                    out.append({'rule': 'S-a', 'why': 'secondBegin', 'attempt': i})
                    continue
                if open_id is not None:
                    out.append({'rule': 'S-b', 'why': 'beginBeforePreviousTerminal', 'attempt': i})
                if i != expect_next:
                    out.append({'rule': 'S-b', 'why': 'attemptIdNotSequential', 'attempt': i, 'expected': expect_next})
                expect_next = i + 1
                attempts[i] = {'begin': e, 'pieces': [], 'terminal': None}
                order.append(i)
                open_id = i
                continue
            if i not in attempts:
                out.append({'rule': 'S-a', 'why': 'eventWithoutBegin', 'attempt': i})
                continue
            at = attempts[i]
            if at['terminal'] is not None:
                out.append({'rule': 'S-b', 'why': 'eventAfterTerminal', 'attempt': i, 'event': ev[0]})
                continue
            if ev[0] == 'piece':
                if ev[2] != len(at['pieces']):
                    out.append({'rule': 'S-a', 'why': 'pieceSeq', 'attempt': i})
                at['pieces'].append(e)
            else:
                at['terminal'] = e
                if open_id == i:
                    open_id = None
                if ev[0] == 'commit':
                    commits.append(i)
        for i in order:
            if attempts[i]['terminal'] is None:
                out.append({'rule': 'S-a', 'why': 'noTerminal', 'attempt': i})
        if len(commits) > 1:
            out.append({'rule': 'S-c', 'why': 'moreThanOneCommit', 'commits': commits})
        if commits and order and commits[-1] != order[-1]:
            out.append({'rule': 'S-c', 'why': 'commitNotLastAttempt', 'commit': commits[-1]})
        for c in commits:
            at = attempts[c]
            rngs = [p['ev'][3] for p in at['pieces']]
            cur = frm
            ok = bool(rngs)
            for a, b in rngs:
                if a != cur or b < a:
                    ok = False
                cur = b + 1
            if not ok or cur != to + 1:
                out.append({'rule': 'S-d', 'why': 'rangesNotExactPartition', 'ranges': rngs})
            summ = at['terminal']['ev'][2]
            # attribution: requests after the previous attempt's terminal up to this attempt's terminal
            idx = order.index(c)
            lo = attempts[order[idx - 1]]['terminal']['seq'] if idx > 0 and attempts[order[idx - 1]]['terminal'] else -1
            hi = at['terminal']['seq']
            reqs = [e for e in entries if e['kind'] == 'req' and lo < e['seq'] < hi]
            want = {'pieces': len(at['pieces']), 'logs': sum(len(p['ev'][4]) for p in at['pieces']),
                    'logRequests': sum(1 for q in reqs if q['req'][0] == 'L'), 'headRequests': sum(1 for q in reqs if q['req'][0] == 'H'),
                    'totalRequests': sum(1 for e in entries if e['kind'] == 'req' and e['seq'] < hi)}
            got = {k: summ.get(k) for k in want}
            if got != want:
                out.append({'rule': 'S-e', 'why': 'summaryMismatch', 'summary': got, 'actual': want})
        return out


# ------------------------------------------------------------------ collectors and the page path

def run_client(server, clock, params, frm, to, consumer=None, faults=(), timeline=None):
    tl = timeline if timeline is not None else server.timeline
    sink = RecordingSink(tl, consumer)
    cl = LogClient(server, sink, clock, max_requests=params['maxRequests'], timeout_ms=params['timeoutMs'],
                   max_bytes=params['maxBytes'], faults=faults)
    res = cl.fetch(frm, to)
    return cl, sink, res


def fetch_all(server, clock, frm, to, faults=()):
    """Bridge collector: internal sink emptied at abort; the page receives logs after commit only."""
    cons = RefConsumer()
    cl, sink, res = run_client(server, clock, FETCH_ALL, frm, to, cons, faults)
    if res['ok']:
        return cl, sink, {'ok': True, 'logs': cons.result}
    return cl, sink, {'ok': False, 'error': res['error']}                  # never a partial result


def fetch_each(server, clock, frm, to, consumer=None, faults=()):
    """Test tool (Node.js): external sink, 600 s, 65536 requests. Not reachable from pages."""
    return run_client(server, clock, FETCH_EACH, frm, to, consumer, faults)


class Bridge:
    """The only page path for eth_getLogs: range check before sending, then fetchAll."""

    def __init__(self, server, clock):
        self._server, self._clock = server, clock

    def eth_getLogs(self, frm, to):
        if to - frm + 1 > WINDOW:
            return {'error': {'code': -32020, 'data': {'reason': 'range'}}}
        _, _, res = fetch_all(self._server, self._clock, frm, to)
        return {'result': res['logs']} if res['ok'] else {'error': res['error']}
