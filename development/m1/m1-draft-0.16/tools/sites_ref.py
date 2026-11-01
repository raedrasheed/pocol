"""E6/E7 disk-and-site ledger reference for M1 draft 0.16 (D104, D105, D112, D113, D114).
REFERENCE / SPECIFICATION FIXTURE TOOLING ONLY. NOT executed by the author.

Model only (not Chrome bytes, not chrome.storage): a manual clock, a FakeBackend that holds
operations until the schedule settles, applies or drops them, and a DiskLedger with
reserve/ticket/settleSeq/serialized refresh, site seats, pins, lateGens/LATE_SITES and
TombReaper. Operations are explicit schedule events (the RecoveryGate/AdminDelete
machines that would issue them are modelled in recovery_ref (0.13) and admin_ref (0.16);
this engine is the admission and accounting boundary they call).

Sources: browser.md:626-713 (disk, DiskLedger, sites, TombReaper, lemmas), 740-776;
validation.md:552-682 (BR23); governance.md:33-39, 134-142.
Same-millisecond order (P-E7-1): apply/drop 0, death 1, settle 2, refresh completion 3,
window-expiry refresh issue 3.5, requests and scans 4, in schedule order.
"""

import heapq

DISK_HARD = 268435456
RECORDS_MAX = 1024
LATE_RESERVE = 23072768
META_RESERVE = 1048576
LATE_NAMES = 24
EPOCH_NAMES_MAX = 8
SITES_MAX = 64
LATE_SITES_PER_GEN = 4
T_LATE = 60000

FAULTS = frozenset({
    'noLateSites',          # LATE_SITES = 0 (validation.md:608)
    'dropPinned',           # a measured-absent pinned key leaves sitesLive (validation.md:675)
    'seatOnCounted',        # a seat is requested for a counted key (validation.md:674)
    'epochReserveOmitted',  # EPOCH_NAMES_MAX left out of the names rule (validation.md:584)
    'diskNoReserve',        # reserve does not add to R; ticket dropped at settle (validation.md:681)
    'noDiskGate',           # no disk/names admission check (validation.md:682)
})


class SiteLedger:
    def __init__(self, keys_on_disk=(), measured_keys=None, in_use=0, names=None, epochs=(), live_E=None, dropped_epochs=(),
                 faults=(), auto_reaper=False, item_bytes=1, sessions=(), tomb_keys=(), refresh_delay=None):
        unknown = set(faults) - FAULTS
        if unknown:
            raise ValueError('unknown faults %s' % sorted(unknown))
        self.faults = frozenset(faults)
        self.t = None
        self.q, self.n = [], 0
        # backend items: name -> {'key', 'kind', 'bytes', 'E', 'bootMs'}
        self.items = {}
        for k in keys_on_disk:
            self.items['%s#init' % k] = {'key': k, 'kind': 'tomb' if k in tomb_keys else 'record', 'bytes': item_bytes}
        self.base_in_use = in_use                  # bytes of everything not modelled item by item
        for e in epochs:
            self.items['epoch#%d' % e['E']] = {'key': None, 'kind': 'epoch', 'bytes': 0, 'E': e['E'], 'bootMs': e['bootMs']}
        self.base_names = (names - len(self.items)) if names is not None else 0
        self.refresh_delay = refresh_delay
        self.live_E = live_E
        self.dropped = set(dropped_epochs)         # epoch E whose window contribution has been dropped
        mk = set(measured_keys if measured_keys is not None else keys_on_disk)
        self.measured = {'inUse': self._in_use(), 'names': self._names(), 'keys': mk}
        self.sites_live = set(mk)
        self.tickets = []
        self.ops = {}
        self.seq = 0
        self.refreshing = None                     # {'seq', 't'}
        self.pending_refresh = False
        self.sessions = set(sessions)
        self.auto_reaper = auto_reaper
        self.decisions, self.trace, self.violations, self.reports = [], [], [], []
        self.stats = {'diskRejects': 0, 'sitesRejects': 0, 'sitesRejectsLate': 0, 'tombReaps': 0, 'lateRecordsSeen': 0}
        self.max_disk_keys = len(self.disk_keys())
        self.max_late_sites = self.late_sites()
        self.max_disk_bytes = self._in_use()
        self.alive = True
        self.schedule_expiries()

    # ---------------- measurement helpers
    def _in_use(self):
        return self.base_in_use + sum(i['bytes'] for i in self.items.values())

    def _names(self):
        return self.base_names + len(self.items)

    def disk_keys(self):
        return {i['key'] for i in self.items.values() if i['kind'] in ('record', 'tomb')}

    def live_tickets(self):
        return [t for t in self.tickets if t['live']]

    def r_bytes(self):
        return 0 if 'diskNoReserve' in self.faults else sum(t['bytes'] for t in self.live_tickets())

    def r_names(self):
        return 0 if 'diskNoReserve' in self.faults else sum(t['names'] for t in self.live_tickets())

    def r_sites(self):
        return 0 if 'diskNoReserve' in self.faults else sum(1 for t in self.live_tickets() if t['seat'])

    def pinned(self):
        return {t['key'] for t in self.live_tickets() if t['pin']}

    def late_gens(self):
        if self.live_E is None:
            return 0
        return sum(1 for i in self.items.values() if i['kind'] == 'epoch' and i['E'] <= self.live_E and i['E'] not in self.dropped)

    def late_sites(self):
        return 0 if 'noLateSites' in self.faults else LATE_SITES_PER_GEN * self.late_gens()

    def counted(self, key):
        return key in self.sites_live or any(t['live'] and t['seat'] and t['key'] == key for t in self.tickets)

    # ---------------- infrastructure
    def at(self, t, prio, fn, *args):
        self.n += 1
        heapq.heappush(self.q, (t, prio, self.n, fn, args))

    def run(self, until):
        while self.q and self.q[0][0] <= until:
            t, _, _, fn, args = heapq.heappop(self.q)
            self.t = t
            fn(*args)
            self._check()
        self.t = until

    def _tr(self, ev, **kw):
        r = {'t': self.t, 'ev': ev}
        r.update(kw)
        self.trace.append(r)

    def _violate(self, kind, **kw):
        if not any(v['kind'] == kind for v in self.violations):
            r = {'t': self.t, 'kind': kind}
            r.update(kw)
            self.violations.append(r)

    def _check(self):
        dk = len(self.disk_keys())
        self.max_disk_keys = max(self.max_disk_keys, dk)
        if dk > SITES_MAX:
            self._violate('diskKeysOver64', count=dk)
        self.max_late_sites = max(self.max_late_sites, self.late_sites())
        b = self._in_use()
        self.max_disk_bytes = max(self.max_disk_bytes, b)
        if b > DISK_HARD:
            self._violate('diskBytesOverHard', bytes=b)
        if self._names() > RECORDS_MAX:
            self._violate('namesOverMax', names=self._names())

    PRIO = {'apply': 0, 'drop': 0, 'death': 1, 'settle': 2, 'refreshComplete': 3, 'refreshNow': 4, 'op': 4, 'scan': 4,
            'sessionOpen': 4, 'sessionClose': 4, 'boot': 4, 'probe': 4}

    def load(self, events):
        for e in events:
            d = e['do']
            self.at(e['t'], self.PRIO[d], getattr(self, 'ev_' + d), e)

    # ---------------- admission (D104 / D105 / D112)
    def ev_op(self, e):
        """An operation the generation wants to issue: kind in data|checkpoint|tomb|remove."""
        if not self.alive:
            return
        key, kind, enc = e['key'], e['kind'], e.get('bytes', 1)
        dec = self.admit(key, kind, enc)
        rec = {'t': self.t, 'id': e['id'], 'decision': dec}
        self.decisions.append(rec)
        if dec is not None:
            self._tr('rejected', id=e['id'], decision=dec)
            return
        counted_before = self.counted(key)
        seat = kind in ('data', 'checkpoint') and (not counted_before or 'seatOnCounted' in self.faults)
        ticket = {'id': e['id'], 'key': key, 'bytes': 0 if kind in ('tomb', 'remove') else enc,
                  'names': 0 if kind == 'remove' else 1, 'seat': seat, 'pin': (kind != 'remove' and counted_before and not seat),
                  'settleSeq': None, 'live': kind != 'remove'}
        self.tickets.append(ticket)
        self.ops[e['id']] = {'key': key, 'kind': kind, 'bytes': enc, 'ticket': ticket, 'state': 'pending', 'settled': None,
                             'target': e.get('target')}
        self._tr('issued', id=e['id'], key=key, kind=kind, seat=seat, pin=ticket['pin'])
        if e.get('autoSettle') is not None:
            self.at(self.t + e['autoSettle'], 2, self.ev_settle, {'id': e['id'], 'ok': True})

    def admit(self, key, kind, enc):
        if kind == 'remove':
            return None
        if 'noDiskGate' not in self.faults:
            epoch_term = 0 if 'epochReserveOmitted' in self.faults else EPOCH_NAMES_MAX
            if kind != 'tomb' and self._meas('inUse') + self.r_bytes() + enc + LATE_RESERVE + META_RESERVE > DISK_HARD:
                self.stats['diskRejects'] += 1
                return {'code': 4300, 'reason': 'disk', 'limit': DISK_HARD}
            if self._meas('names') + self.r_names() + 1 + LATE_NAMES + epoch_term > RECORDS_MAX:
                self.stats['diskRejects'] += 1
                return {'code': 4300, 'reason': 'disk', 'limit': RECORDS_MAX}
        creating = kind in ('data', 'checkpoint') and (not self.counted(key) or 'seatOnCounted' in self.faults)
        if creating:
            base = len(self.sites_live) + self.r_sites()
            if base + self.late_sites() + 1 > SITES_MAX:
                self.stats['sitesRejects'] += 1
                dec = {'code': 4300, 'reason': 'sites', 'limit': SITES_MAX}
                if base + 1 <= SITES_MAX:                         # late contributions alone block
                    self.stats['sitesRejectsLate'] += 1
                    dec['retryAfterMs'] = self._retry_after(base)
                return dec
        return None

    def _meas(self, f):
        return self.measured[f]

    def _retry_after(self, base):
        """Time until the earliest window expiry after which the admission would hold."""
        contrib = sorted(i['bootMs'] + T_LATE for i in self.items.values()
                         if i['kind'] == 'epoch' and i['E'] <= (self.live_E or 0) and i['E'] not in self.dropped)
        late = self.late_sites()
        for exp in contrib:
            late -= LATE_SITES_PER_GEN
            if base + late + 1 <= SITES_MAX:
                return max(0, exp - self.t)
        return None

    # ---------------- backend events
    def ev_settle(self, e):
        op = self.ops.get(e['id'])
        if op is None or op['settled'] is not None or not self.alive:
            return
        op['settled'] = 'ok' if e.get('ok', True) else 'fail'
        if op['settled'] == 'ok':
            self._apply(e['id'])
        self.seq += 1
        op['ticket']['settleSeq'] = self.seq
        if 'diskNoReserve' in self.faults:
            op['ticket']['live'] = False
        self._tr('settled', id=e['id'], ok=op['settled'])
        self.issue_refresh()

    def ev_apply(self, e):
        if e['id'] in self.ops:
            self._apply(e['id'])

    def _apply(self, oid):
        op = self.ops[oid]
        if op['state'] != 'pending':
            return
        op['state'] = 'applied'
        if op['kind'] == 'remove':
            tgt = op['target']
            if tgt in self.items:
                del self.items[tgt]
        else:
            self.items['%s#%s' % (op['key'], oid)] = {'key': op['key'], 'kind': 'tomb' if op['kind'] == 'tomb' else 'record',
                                                     'bytes': 0 if op['kind'] == 'tomb' else op['bytes']}
        self._tr('applied', id=oid)

    def ev_drop(self, e):
        op = self.ops.get(e['id'])
        if op and op['state'] == 'pending':
            op['state'] = 'dropped'

    def ev_death(self, e):
        """The generation's ledger dies with it (browser.md:613): tickets, refresh, sessions."""
        self.alive = False
        for t in self.tickets:
            t['live'] = False
        self.refreshing, self.pending_refresh = None, False
        self.sessions = set()
        self._tr('death')

    def ev_boot(self, e):
        """A new generation: its epoch item appears (confirmed) and a boot measurement is adopted."""
        self.alive = True
        self.live_E = e['E']
        self.items['epoch#%d' % e['E']] = {'key': None, 'kind': 'epoch', 'bytes': 0, 'E': e['E'], 'bootMs': self.t}
        self.measured = {'inUse': self._in_use(), 'names': self._names(), 'keys': self.disk_keys()}
        self.sites_live = set(self.measured['keys'])
        for i in self.items.values():
            if i['kind'] == 'epoch' and i['E'] <= self.live_E:
                self.at(i['bootMs'] + T_LATE, 3.5, self._window_expiry, i['E'])
        self._tr('boot', E=e['E'], sitesLive=len(self.sites_live), lateGens=self.late_gens())

    def schedule_expiries(self):
        for i in self.items.values():
            if i['kind'] == 'epoch' and self.live_E is not None and i['E'] <= self.live_E and i['E'] not in self.dropped:
                self.at(i['bootMs'] + T_LATE, 3.5, self._window_expiry, i['E'])

    def _window_expiry(self, E):
        if self.alive and E not in self.dropped:
            self._tr('windowExpiry', E=E)
            self.issue_refresh()

    # ---------------- refresh (serialized)
    def issue_refresh(self):
        if not self.alive:
            return
        if self.refreshing is not None:
            self.pending_refresh = True
            return
        self.seq += 1
        self.refreshing = {'seq': self.seq, 't': self.t}
        self._tr('refreshIssued', seq=self.seq)
        if self.refresh_delay is not None:
            self.at(self.t + self.refresh_delay, 3, self.ev_refreshComplete, {})

    def ev_refreshNow(self, e):
        self.issue_refresh()

    def ev_refreshComplete(self, e):
        if not self.alive or self.refreshing is None:
            return
        r = self.refreshing
        self.refreshing = None
        keys = self.disk_keys()
        self.measured = {'inUse': self._in_use(), 'names': self._names(), 'keys': keys}
        for t in self.tickets:
            if t['live'] and t['settleSeq'] is not None and t['settleSeq'] < r['seq']:
                t['live'] = False
        if 'dropPinned' in self.faults:
            self.sites_live = set(keys)
        else:
            pins = self.pinned()
            self.sites_live = set(keys) | {k for k in self.sites_live if k in pins}
        for i in self.items.values():
            if i['kind'] == 'epoch' and i['bootMs'] + T_LATE <= r['t']:
                self.dropped.add(i['E'])
        self._tr('refreshComplete', seq=r['seq'], sitesLive=len(self.sites_live), pinned=len(self.pinned()), lateGens=self.late_gens())
        if self.auto_reaper:
            self.scan()
        if self.pending_refresh:
            self.pending_refresh = False
            self.issue_refresh()

    # ---------------- sessions and TombReaper (D113)
    def ev_sessionOpen(self, e):
        self.sessions.add(e['key'])

    def ev_sessionClose(self, e):
        self.sessions.discard(e['key'])

    def ev_scan(self, e):
        self.scan()

    def scan(self):
        if not self.alive:
            return
        older_in = any(i['kind'] == 'epoch' and i['E'] < (self.live_E or 0) and i['E'] not in self.dropped for i in self.items.values())
        for key in sorted(self.disk_keys()):
            its = [n for n, i in self.items.items() if i['key'] == key]
            if len(its) != 1 or self.items[its[0]]['kind'] != 'tomb':
                continue
            busy = key in self.sessions or any(t['live'] and t['key'] == key for t in self.tickets) or older_in
            if busy:
                continue
            if any(op['kind'] == 'remove' and op['target'] == its[0] and op['state'] == 'pending' for op in self.ops.values()):
                continue
            oid = 'reap-%s-%d' % (key, self.stats['tombReaps'] + 1)
            self.stats['tombReaps'] += 1
            self.ev_op({'id': oid, 'key': key, 'kind': 'remove', 'target': its[0]})
            self._tr('reaperRemove', key=key, id=oid)

    def ev_probe(self, e):
        self.reports.append({'t': self.t, 'label': e.get('label'), 'snapshot': self.snapshot()})

    def snapshot(self):
        return {'sitesLive': len(self.sites_live), 'rSites': self.r_sites(), 'rBytes': self.r_bytes(), 'rNames': self.r_names(),
                'lateGens': self.late_gens(), 'lateSites': self.late_sites(), 'pinnedKeys': len(self.pinned()),
                'diskKeys': len(self.disk_keys()), 'measuredInUse': self.measured['inUse'], 'measuredNames': self.measured['names'],
                'liveTickets': len(self.live_tickets())}
