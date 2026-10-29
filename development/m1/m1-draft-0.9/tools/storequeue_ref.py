"""StoreQueue / SiteStorageRef reference model for M1 draft 0.9 (annex rows Q1-Q4).

SPECIFICATION FIXTURE TOOLING ONLY. This is not the extension implementation: no
chrome.storage, no sockets, no timers, no Chrome. Time is a manual fake clock (every
call carries its millisecond), and the backend is a fake that holds every set until the
schedule settles or fails it explicitly (the FakeStorage of BR21a).

Sources (Arabic baseline; line numbers of the files as read for 0.9):
  reference/browser.md        393-405 (SiteStorage accounting), 407-495 (StoreQueue D101,
                               ownership cycle D102/I54, I55, I56, teardown D99)
  reference/implementation.md 204-228 (SiteStorage.check/set, StoreQueue API)
  reference/governance.md     121-144 (BG1, I55 TQ release rejection)
  reference/validation.md     286-389 (BR21a-f), 197-209 (BR17e)

Ambiguities are not resolved silently; each modelled choice that the baseline does not
state is tagged P-Qn-m here and listed in annex/Q4-REFERENCE-EXTENSION.md.
"""

from types import MappingProxyType

QMSG_OVERHEAD = 64                       # browser.md:410
KEY_MAX, VALUE_MAX = 256, 61440
MAX_QBYTES = QMSG_OVERHEAD + KEY_MAX + VALUE_MAX   # 61760, browser.md:412
SITE_STORE_MAX = 1048576                 # browser.md:395
NAV_SNAPSHOT_WAIT = 5000                 # browser.md:478 (navigationStart + STORE_WRITE_DEADLINE)

# Production defaults are immutable (browser.md:415-418, 428, 441-442).
DEFAULTS = MappingProxyType({'sessMsgs': 8, 'sessBytes': 262144, 'globMsgs': 64, 'globBytes': 4194304,
                             'active': 2, 'wait': 10000, 'write': 5000})
# BR21e test-only parameters (validation.md:347).
TQ = MappingProxyType({'sessMsgs': 8, 'sessBytes': 262144, 'globMsgs': 64, 'globBytes': 300000,
                       'active': 2, 'wait': 10000, 'write': 5000})
FIELDS = ('sessN', 'sessBytes', 'globN', 'globBytes')   # storeViolated vocabulary, validation.md:289

# Fault controls (negative controls). Each must be detected by a literal golden case.
FAULTS = frozenset({
    'storeUnbounded',        # admission ignores the four counters            (validation.md:323, 358)
    'storeEarlyRelease',     # releases counters when the storeTimeout reply is sent (validation.md:314, 336)
    'quotaNoRelease',        # quota rejection replies 4300 but never releases (validation.md:389)
    'quotaAfterSeat',        # quota checked only when a seat is granted       (validation.md:389, f2)
    'readyFifoByReadyTime',  # global ready FIFO ordered by readiness, not by acceptance (browser.md:433)
    'perItemKey',            # storage key from the item key, not the frame    (browser.md:425, 495)
    'sessionKey',            # storage key per session (tab), not per site     (browser.md:425)
    'keyFromMessage',        # storage key taken from a field of the message   (browser.md:495; implementation.md:228)
})


class ConfigRejected(Exception):
    pass


# ---------------------------------------------------------------- Q1: byte accounting

def utf8len(s):
    """TextEncoder-compatible UTF-8 length (implementation.md:206). None counts 0
    (browser.md:410). A lone surrogate code point is encoded by TextEncoder as U+FFFD,
    i.e. 3 bytes. A valid surrogate pair arriving through JSON is one code point in
    Python (4 bytes), matching TextEncoder on the JS string."""
    if s is None:
        return 0
    n = 0
    for ch in s:
        c = ord(ch)
        if c < 0x80:
            n += 1
        elif c < 0x800:
            n += 2
        elif 0xD800 <= c <= 0xDFFF:
            n += 3
        elif c < 0x10000:
            n += 3
        else:
            n += 4
    return n


def qbytes(op):
    """browser.md:410-411. op = {'op': 'set', 'k', 'v'} or {'op': 'clear'}."""
    if op['op'] == 'clear':
        return QMSG_OVERHEAD
    return QMSG_OVERHEAD + utf8len(op['k']) + utf8len(op['v'])


def admission(p, sess, glob, q):
    """The four inclusive conditions (browser.md:414-420). Returns the literal violated
    field set in FIELDS order; empty means admit. Equality is accepted."""
    v = []
    if sess[0] + 1 > p['sessMsgs']:
        v.append('sessN')
    if sess[1] + q > p['sessBytes']:
        v.append('sessBytes')
    if glob[0] + 1 > p['globMsgs']:
        v.append('globN')
    if glob[1] + q > p['globBytes']:
        v.append('globBytes')
    return v


def validate_config(p, build):
    """BG1 StoreQueue lines (governance.md:132-133, 142). build in {'release', 'test'}.
    A release build accepts only the immutable defaults. A test build may violate only the
    global-bytes bound (TQ, validation.md:348); every other BG1 condition still binds it
    (P-Q1-3: the baseline does not say whether the other conditions bind test builds)."""
    viol = []
    if p['sessBytes'] < MAX_QBYTES:
        viol.append('sessBytesFitsMax')
    if p['globBytes'] < p['globMsgs'] * MAX_QBYTES:
        viol.append('globBytesBound')
    if not (p['globMsgs'] >= p['sessMsgs'] >= 1):
        viol.append('msgsOrder')
    if p['active'] < 1:
        viol.append('activeMin')
    if not p['wait'] > p['write']:
        viol.append('waitAfterWrite')
    if build == 'release':
        if dict(p) != dict(DEFAULTS):
            viol.insert(0, 'nonDefaultRelease')
    elif build == 'test':
        viol = [x for x in viol if x != 'globBytesBound']
    else:
        viol = ['unknownBuild']
    return {'ok': not viol, 'violations': viol}


# ---------------------------------------------------------------- SiteStorageRef

def entry(k, v):
    return utf8len(k) + utf8len(v)


def total_of(d):
    return sum(entry(k, v) for k, v in d.items())


class SiteStorageRef:
    """Literal quota accounting (browser.md:395-400; implementation.md:207-208).
    Independent of StoreQueue: a pure function of (dictionary, message).
    D104 'entries'/'disk'/'sites' reasons belong to rows E1-E7 and are out of scope."""

    @staticmethod
    def apply(d, op):
        before = total_of(d)
        if op['op'] == 'clear':
            return {'decision': None, 'totalBefore': before, 'totalAfter': 0, 'newDict': {}}
        k, v = op['k'], op['v']
        old = entry(k, d[k]) if k in d else 0
        if v is None:
            nd = {x: y for x, y in d.items() if x != k}
            return {'decision': None, 'totalBefore': before, 'totalAfter': before - old, 'newDict': nd}
        new_total = before - old + entry(k, v)
        if new_total > SITE_STORE_MAX:
            return {'decision': 4300, 'totalBefore': before, 'totalAfter': before, 'newDict': dict(d),
                    'attempted': new_total}
        nd = dict(d)
        nd[k] = v
        return {'decision': None, 'totalBefore': before, 'totalAfter': new_total, 'newDict': nd}


# ---------------------------------------------------------------- StoreQueue

class Msg:
    __slots__ = ('id', 'sid', 'frame', 'skey', 'op', 'q', 'accept', 'seq', 'own', 'reply', 'start', 'readyAt',
                 'evalTotal', 'deadlineFired', 'call', 'countersReleased', 'blockedNoRelease', 'replyCount')

    def __init__(self, **kw):
        for s in self.__slots__:
            setattr(self, s, kw.get(s))
        self.replyCount = 0


class StoreQueue:
    """D101/D102 model over a fake clock and a holding fake backend.

    Same-millisecond order (P-Q1-4, not stated by the baseline): timers due at a given
    millisecond run before the scheduled event of that millisecond, in the order
    queue-wait expiry, write deadline, navigator deadline, each by acceptance sequence."""

    def __init__(self, params=DEFAULTS, build='release', faults=(), dicts=None):
        verdict = validate_config(params, build)
        if not verdict['ok']:
            raise ConfigRejected(verdict['violations'])
        unknown = set(faults) - FAULTS
        if unknown:
            raise ValueError('unknown faults %s' % sorted(unknown))
        self.p = dict(params)
        self.faults = frozenset(faults)
        self.backend = {k: dict(v) for k, v in (dicts or {}).items()}   # FakeStorage: confirmed bytes
        self.confirmed = {}            # loaded dictionaries (reloaded from backend after every settle)
        self.sessions = {}
        self.frames = {}
        self.glob = [0, 0]
        self.msgs = {}
        self.order = []                # msg ids by acceptance
        self.keyq = {}                 # storage key -> unreleased msg ids, FIFO by acceptance
        self.active = {}               # storage key -> msg id
        self.ready = []
        self.seats = 0
        self.calls = []                # backend set calls
        self.seq = 0
        self.now = None
        self.replies = []
        self.snapshots = []
        self.pending_nav = []
        self.rejected = set()
        self.transitions = []
        self.decisions = []            # (msg id, SiteStorageRef decision, totalBefore, totalAfter)
        self.stats = {'releaseCount': 0, 'doubleReleaseCount': 0, 'quotaRecheckMismatch': 0,
                      'backendSetCalls': 0, 'maxActive': 0}
        self.event_replies = []
        self.event_snapshots = []

    # ---- sessions and frames (SiteSession, D99)
    def open_session(self, sid, net_key, address):
        self.sessions[sid] = {'site': 'site:%s:%s' % (net_key, address), 'n': 0, 'bytes': 0, 'live': True}

    def attach(self, frame, sid):
        self.frames[frame] = {'sid': sid, 'alive': True}

    def _key(self, frame, op):
        """Logical storage key derived from the FRAME context (browser.md:394, 495;
        implementation.md:228). The message never names it."""
        sid = self.frames[frame]['sid']
        base = self.sessions[sid]['site']
        if 'keyFromMessage' in self.faults and op.get('site'):
            return 'site:' + op['site']
        if 'perItemKey' in self.faults and op['op'] == 'set':
            return base + '#' + op['k']
        if 'sessionKey' in self.faults:
            return base + '@' + sid
        return base

    def _dict(self, skey):
        if skey not in self.confirmed:
            self.confirmed[skey] = dict(self.backend.get(skey, {}))
        return self.confirmed[skey]

    # ---- bookkeeping
    def _set(self, m, own=None, reply=None, cause=''):
        before = (m.reply, m.own)
        if own is not None:
            m.own = own
        if reply is not None:
            m.reply = reply
        self.transitions.append({'t': self.now, 'id': m.id, 'from': list(before), 'to': [m.reply, m.own], 'cause': cause})

    def _emit(self, rec):
        rec['t'] = self.now
        self.replies.append(rec)
        self.event_replies.append(rec)

    def _reply(self, m, code, reason=None, cause='', **extra):
        rec = {'id': m.id, 'frame': m.frame, 'code': code, 'reason': reason,
               'delivered': self.frames[m.frame]['alive']}
        rec.update(extra)
        m.replyCount += 1
        self._emit(rec)
        self._set(m, reply='sent', cause=cause)

    def _sub(self, m):
        s = self.sessions.get(m.sid)
        if s is not None:
            s['n'] -= 1
            s['bytes'] -= m.q
        self.glob[0] -= 1
        self.glob[1] -= m.q

    def _release(self, m, cause):
        """The only place counters, seat and key lock are released (browser.md:457)."""
        if m.countersReleased:
            self.stats['doubleReleaseCount'] += 1
            self._sub(m)                  # a second subtraction, as the fault would do
        else:
            self._sub(m)
            m.countersReleased = True
        self.stats['releaseCount'] += 1
        q = self.keyq.get(m.skey, [])
        if m.id in q:
            q.remove(m.id)
        if m.id in self.ready:
            self.ready.remove(m.id)
        if self.active.get(m.skey) == m.id:
            del self.active[m.skey]
            self.seats -= 1
        self._set(m, own='released', cause=cause)
        s = self.sessions.get(m.sid)
        if s is not None and not s['live'] and not self._unreleased(m.sid):
            del self.sessions[m.sid]

    def _unreleased(self, sid):
        return [i for i in self.order if self.msgs[i].sid == sid and self.msgs[i].own != 'released']

    # ---- head evaluation (I56) and pump
    def _eval_head(self, skey):
        while skey not in self.active:
            ids = self.keyq.get(skey)
            if not ids:
                return
            m = self.msgs[ids[0]]
            if m.own != 'queued' or m.blockedNoRelease:
                return
            if 'quotaAfterSeat' in self.faults:
                m.readyAt = self.now
                self.ready.append(m.id)
                self._set(m, own='ready', cause='evalHead(no quota check: fault)')
                return
            r = SiteStorageRef.apply(self._dict(skey), m.op)
            self.decisions.append((m.id, r['decision'], r['totalBefore'], r['totalAfter']))
            if r['decision'] == 4300:
                self._reply(m, 4300, 'quota', cause='evalHead quota', limit=SITE_STORE_MAX)
                if 'quotaNoRelease' in self.faults:
                    m.blockedNoRelease = True
                    return
                self._release(m, 'evalHead quota')
                continue
            m.evalTotal = r['totalAfter']
            m.readyAt = self.now
            self.ready.append(m.id)
            self._set(m, own='ready', cause='evalHead ok')
            return

    def _pump(self):
        while self.seats < self.p['active'] and self.ready:
            if 'readyFifoByReadyTime' in self.faults:
                mid = min(self.ready, key=lambda i: (self.msgs[i].readyAt, self.msgs[i].seq))
            else:
                mid = min(self.ready, key=lambda i: (self.msgs[i].accept, self.msgs[i].seq))
            m = self.msgs[mid]
            self.ready.remove(mid)
            r = SiteStorageRef.apply(self._dict(m.skey), m.op)
            if 'quotaAfterSeat' in self.faults:
                self.decisions.append((m.id, r['decision'], r['totalBefore'], r['totalAfter']))
                if r['decision'] == 4300:
                    self._reply(m, 4300, 'quota', cause='quota at seat (fault)', limit=SITE_STORE_MAX)
                    self._release(m, 'quota at seat (fault)')
                    self._eval_head(m.skey)
                    continue
            elif r['totalAfter'] != m.evalTotal or r['decision'] is not None:
                self.stats['quotaRecheckMismatch'] += 1      # implementation.md:212
            m.start = self.now
            self.seats += 1
            self.active[m.skey] = m.id
            self.calls.append({'no': len(self.calls) + 1, 'msg': m.id, 'skey': m.skey, 'dict': r['newDict'],
                               'total': r['totalAfter'], 'at': self.now, 'settled': None})
            m.call = len(self.calls)
            self.stats['backendSetCalls'] += 1
            self.stats['maxActive'] = max(self.stats['maxActive'], self.seats)
            self._set(m, own='active', cause='seat + set')

    # ---- timers
    def _advance(self, t):
        if self.now is not None and t < self.now:
            raise ValueError('clock went backwards: %s < %s' % (t, self.now))
        while True:
            due = []
            for mid in self.order:
                m = self.msgs[mid]
                if m.own in ('queued', 'ready') and not m.blockedNoRelease:
                    due.append((m.accept + self.p['wait'], 0, m.seq, 'expire', mid))
                elif m.own == 'active' and not m.deadlineFired:
                    due.append((m.start + self.p['write'], 1, m.seq, 'deadline', mid))
            for n, nav in enumerate(self.pending_nav):
                due.append((nav['deadline'], 2, n, 'nav', n))
            due = [d for d in due if d[0] <= t]
            if not due:
                break
            at, _, _, kind, ref = min(due)
            self.now = at
            if kind == 'expire':
                m = self.msgs[ref]
                self._reply(m, -32005, 'store', cause='STORE_QUEUE_WAIT')
                self._release(m, 'STORE_QUEUE_WAIT')
                self._eval_head(m.skey)
                self._pump()
            elif kind == 'deadline':
                m = self.msgs[ref]
                m.deadlineFired = True
                if m.reply == 'none':
                    self._reply(m, -32603, 'storeTimeout', cause='STORE_WRITE_DEADLINE')
                    if 'storeEarlyRelease' in self.faults:
                        self._sub(m)
                        m.countersReleased = True
                        self.stats['releaseCount'] += 1
            else:
                nav = self.pending_nav.pop(ref)
                self._snapshot(nav['frame'], nav['skey'], badge=True)
        self.now = t

    def _snapshot(self, frame, skey, badge):
        d = self._dict(skey)
        rec = {'t': self.now, 'frame': frame, 'keys': sorted(d), 'total': total_of(d), 'badge': badge}
        self.snapshots.append(rec)
        self.event_snapshots.append(rec)

    # ---- events
    def tick(self, t):
        self._advance(t)

    def submit(self, t, frame, mid, op):
        """Bridge storage_set / site_storageClear at B4 (browser.md:414-420)."""
        self._advance(t)
        if not self.frames[frame]['alive']:
            raise ValueError('message from a torn-down frame')
        sid = self.frames[frame]['sid']
        s = self.sessions[sid]
        q = qbytes(op)
        viol = [] if 'storeUnbounded' in self.faults else admission(self.p, (s['n'], s['bytes']), self.glob, q)
        self.seq += 1
        if viol:
            self.rejected.add(mid)
            self._emit({'id': mid, 'frame': frame, 'code': -32005, 'reason': 'store', 'violated': viol,
                        'delivered': True, 'storeQBefore': [s['n'], s['bytes'], self.glob[0], self.glob[1]]})
            return
        skey = self._key(frame, op)
        m = Msg(id=mid, sid=sid, frame=frame, skey=skey, op=op, q=q, accept=t, seq=self.seq, own='queued',
                reply='none', deadlineFired=False, countersReleased=False, blockedNoRelease=False)
        self.msgs[mid] = m
        self.order.append(mid)
        s['n'] += 1
        s['bytes'] += q
        self.glob[0] += 1
        self.glob[1] += q
        self.keyq.setdefault(skey, []).append(mid)
        self.transitions.append({'t': t, 'id': mid, 'from': ['-', '-'], 'to': ['none', 'queued'], 'cause': 'admit'})
        self._eval_head(skey)
        self._pump()

    def settle(self, t, call_no, ok=True):
        """FakeStorage resolves set call `call_no` (release(i) / fail(i) of BR21a)."""
        self._advance(t)
        c = self.calls[call_no - 1]
        if c['settled'] is not None:
            raise ValueError('call %d settled twice' % call_no)
        c['settled'] = 'ok' if ok else 'fail'
        m = self.msgs[c['msg']]
        if ok:
            self.backend[c['skey']] = dict(c['dict'])
        # Reload the confirmed dictionary and recompute total before releasing (browser.md:444, 467, 469).
        self.confirmed[c['skey']] = dict(self.backend.get(c['skey'], {}))
        if m.reply == 'none':
            if ok:
                self._reply(m, None, None, cause='settle ok')
            else:
                self._reply(m, -32603, 'transport', cause='settle fail')
        self._release(m, 'settle ' + c['settled'])
        for n, nav in enumerate(list(self.pending_nav)):
            if nav['msg'] == m.id:
                self.pending_nav.remove(nav)
                self._snapshot(nav['frame'], nav['skey'], badge=False)
        self._eval_head(c['skey'])
        self._pump()

    def oldest_unsettled_call(self):
        for c in self.calls:
            if c['settled'] is None:
                return c['no']
        return None

    def cancel_frame(self, frame):
        """Waiting messages: suppressed and released. Active: never released early; its
        reply is suppressed if not already sent (browser.md:465, 470, 476-477)."""
        self.frames[frame]['alive'] = False
        touched = []
        for mid in list(self.order):
            m = self.msgs[mid]
            if m.frame != frame:
                continue
            if m.own in ('queued', 'ready'):
                self._set(m, reply='suppressed', cause='cancelFrame')
                self._release(m, 'cancelFrame')
                touched.append(m.skey)
            elif m.own == 'active' and m.reply == 'none':
                self._set(m, reply='suppressed', cause='cancelFrame (active kept)')
        for k in dict.fromkeys(touched):
            self._eval_head(k)
        self._pump()

    def navigate(self, t, frame, new_frame):
        """Navigator step 3 (implementation.md:269): cancelFrame, then wait for the key's
        active write (as of navigation) until settle or navigationStart+5000 (browser.md:478).
        P-Q2-1: the wait covers the write active at navigation time only."""
        self._advance(t)
        sid = self.frames[frame]['sid']
        skey = self._key(frame, {'op': 'clear'})
        self.cancel_frame(frame)
        self.frames[new_frame] = {'sid': sid, 'alive': True}
        act = self.active.get(skey)
        if act is not None:
            self.pending_nav.append({'frame': new_frame, 'skey': skey, 'msg': act, 'deadline': t + NAV_SNAPSHOT_WAIT})
        else:
            self._snapshot(new_frame, skey, badge=False)

    def close_session(self, t, sid):
        """Tab close (browser.md:479): same rule; the session record is kept until its
        active writes settle (validation.md:330)."""
        self._advance(t)
        self.sessions[sid]['live'] = False
        for f, fr in self.frames.items():
            if fr['sid'] == sid and fr['alive']:
                self.cancel_frame(f)
        if sid in self.sessions and not self._unreleased(sid):
            del self.sessions[sid]

    # ---- observation
    def invariant_violations(self):
        """Criterion G (validation.md:292-296)."""
        out = []
        for sid, s in self.sessions.items():
            ids = self._unreleased(sid)
            want = [len(ids), sum(self.msgs[i].q for i in ids)]
            if [s['n'], s['bytes']] != want:
                out.append('sess %s counters %s != unreleased %s' % (sid, [s['n'], s['bytes']], want))
            if s['n'] < 0 or s['bytes'] < 0:
                out.append('sess %s negative' % sid)
            if self.glob[0] < s['n'] or self.glob[1] < s['bytes']:
                out.append('glob < sess %s' % sid)
        ids = [i for i in self.order if self.msgs[i].own != 'released']
        want = [len(ids), sum(self.msgs[i].q for i in ids)]
        if self.glob != want:
            out.append('glob %s != unreleased %s' % (self.glob, want))
        if self.glob[0] < 0 or self.glob[1] < 0:
            out.append('glob negative')
        for m in self.msgs.values():
            if m.replyCount > 1:
                out.append('msg %s replied %d times' % (m.id, m.replyCount))
        return out
