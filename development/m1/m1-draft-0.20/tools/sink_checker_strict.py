"""Strict SinkChecker for M1 draft 0.20 (C29 repair). Python standard library only.
REFERENCE / SPECIFICATION FIXTURE TOOLING ONLY. NOT executed by the author.

C29 (coordination/review-001/REVIEW-0.19.md): the 0.19 SinkChecker treated every event that is not
'begin' or 'piece' as a terminal, so begin(1) + ['unknownTerminal', 1, 'fake'] returned no violation.
browser.md:126 (S-a) requires exactly one terminal, abort or commit.

Closed grammar (contract browser.md:117-130). A timeline entry is
  {'seq': int|float (not bool), 'kind': 'req', 'req': ['H', 'latest'|int] | ['L', a:int, b:int, anchorHash:str]}
  {'seq': int|float (not bool), 'kind': 'sink', 'ev': EVENT}
EVENT, exactly one of (arity is exact; 'int' means type(x) is int, so bool and float are rejected):
  ['begin',  id:int, anchor:{'number': int >= 0, 'hash': str}]
  ['piece',  id:int, seq:int >= 0, [a:int, b:int] with a <= b, logs:list]
  ['abort',  id:int, reason:non-empty str]
  ['commit', id:int, summary:dict]
Rule mapping for malformed input (deterministic, reported with a reason):
  malformed or unknown sink event   -> S-a 'malformedEvent:<detail>'; it never begins, extends or settles an attempt
  malformed timeline entry          -> S-e 'malformedEntry:<detail>' (it corrupts the request accounting)
  commit summary not exactly {pieces, logs, logRequests, headRequests, totalRequests: int >= 0, anchor}
                                    -> S-e 'malformedSummary:<detail>'; the commit is still the terminal
  summary.anchor differs from the attempt's begin anchor -> S-e 'anchorMismatch'
Everything else is the 0.19 logic unchanged (S-a..S-e for well-formed events).
"""

SUMMARY_COUNTERS = ('pieces', 'logs', 'logRequests', 'headRequests', 'totalRequests')


def _int(x):
    return type(x) is int


def _num(x):
    return type(x) in (int, float)


def _anchor_ok(a):
    return type(a) is dict and set(a) == {'number', 'hash'} and _int(a['number']) and a['number'] >= 0 and type(a['hash']) is str


def event_shape(ev):
    """None if well-formed, else a short reason."""
    if type(ev) is not list:
        return 'notList'
    if not ev or type(ev[0]) is not str:
        return 'noKind'
    kind = ev[0]
    arity = {'begin': 3, 'piece': 5, 'abort': 3, 'commit': 3}
    if kind not in arity:
        return 'unknownKind:' + kind
    if len(ev) != arity[kind]:
        return 'arity:%s/%d' % (kind, len(ev))
    if not _int(ev[1]):
        return 'attemptIdNotInt:%s' % type(ev[1]).__name__
    if kind == 'begin' and not _anchor_ok(ev[2]):
        return 'beginAnchor'
    if kind == 'piece':
        if not _int(ev[2]) or ev[2] < 0:
            return 'pieceSeqNotInt:%s' % type(ev[2]).__name__
        r = ev[3]
        if type(r) is not list or len(r) != 2 or not _int(r[0]) or not _int(r[1]) or r[0] > r[1]:
            return 'pieceRange'
        if type(ev[4]) is not list:
            return 'pieceLogsNotList'
    if kind == 'abort' and (type(ev[2]) is not str or not ev[2]):
        return 'abortReason'
    if kind == 'commit' and type(ev[2]) is not dict:
        return None                    # shape of the event is fine; the summary is judged under S-e
    return None


def summary_shape(s):
    if type(s) is not dict:
        return 'notDict'
    keys = set(SUMMARY_COUNTERS) | {'anchor'}
    if set(s) != keys:
        return 'keys:' + ','.join(sorted(set(s) ^ keys))
    for k in SUMMARY_COUNTERS:
        if not _int(s[k]) or s[k] < 0:
            return 'counter:%s:%s' % (k, type(s[k]).__name__)
    if not _anchor_ok(s['anchor']):
        return 'anchor'
    return None


def entry_shape(e):
    if type(e) is not dict:
        return 'notDict'
    if not _num(e.get('seq')):
        return 'seq'
    if e.get('kind') == 'sink':
        return None if 'ev' in e else 'noEv'
    if e.get('kind') == 'req':
        q = e.get('req')
        if type(q) is list and len(q) == 2 and q[0] == 'H' and (q[1] == 'latest' or _int(q[1])):
            return None
        if type(q) is list and len(q) == 4 and q[0] == 'L' and _int(q[1]) and _int(q[2]) and type(q[3]) is str:
            return None
        return 'request'
    return 'kind'


class StrictSinkChecker:
    """(S-a)..(S-e) with a closed event grammar. Returns violations [{rule, why, ...}]."""

    def check(self, entries, frm, to):
        out = []
        good = []
        for e in entries:
            bad = entry_shape(e)
            if bad:
                out.append({'rule': 'S-e', 'why': 'malformedEntry:' + bad})
            else:
                good.append(e)
        sinks = []
        for e in good:
            if e['kind'] != 'sink':
                continue
            bad = event_shape(e['ev'])
            if bad:
                out.append({'rule': 'S-a', 'why': 'malformedEvent:' + bad, 'seq': e['seq']})
            else:
                sinks.append(e)
        attempts, order, commits = {}, [], []
        expect_next, open_id = 1, None
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
            else:                                                     # only 'abort' or 'commit' reach here
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
        reqs_all = [e for e in good if e['kind'] == 'req']
        for c in commits:
            at = attempts[c]
            rngs = [p['ev'][3] for p in at['pieces']]
            cur, ok = frm, bool(rngs)
            for a, b in rngs:
                if a != cur:
                    ok = False
                cur = b + 1
            if not ok or cur != to + 1:
                out.append({'rule': 'S-d', 'why': 'rangesNotExactPartition', 'ranges': rngs})
            summ = at['terminal']['ev'][2]
            bad = summary_shape(summ)
            if bad:
                out.append({'rule': 'S-e', 'why': 'malformedSummary:' + bad, 'attempt': c})
                continue
            if summ['anchor'] != at['begin']['ev'][2]:
                out.append({'rule': 'S-e', 'why': 'anchorMismatch', 'attempt': c})
            idx = order.index(c)
            prev = attempts[order[idx - 1]]['terminal'] if idx > 0 else None
            lo = prev['seq'] if prev is not None else float('-inf')
            hi = at['terminal']['seq']
            reqs = [q for q in reqs_all if lo < q['seq'] < hi]
            want = {'pieces': len(at['pieces']), 'logs': sum(len(p['ev'][4]) for p in at['pieces']),
                    'logRequests': sum(1 for q in reqs if q['req'][0] == 'L'), 'headRequests': sum(1 for q in reqs if q['req'][0] == 'H'),
                    'totalRequests': sum(1 for q in reqs_all if q['seq'] < hi)}
            got = {k: summ[k] for k in want}
            if got != want:
                out.append({'rule': 'S-e', 'why': 'summaryMismatch', 'summary': got, 'actual': want})
        return out
