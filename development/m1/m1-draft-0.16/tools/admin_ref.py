"""AdminDelete / AdminSlots / DeleteOp reference for M1 draft 0.16 (rows E6/E7; D106, D108, D111).
REFERENCE / SPECIFICATION FIXTURE TOOLING ONLY. NOT executed by the author.

Extends the 0.13 recovery engine (m1-draft-0.13/tools/recovery_ref.py, imported read-only)
by subclassing; the 0.13 file is not modified. Replaced there: the narrow tomb adapter.
Sources: browser.md:714-773; validation.md:683-747 (BR24), 742 (BR22f-f5), 565 (d5).

AdminDelete(key), one shared operation per key (D106):
  1 deleting[key] = opId; new sessions for the key are held until resolution.
  2 every session of the key closes: frames torn with «حُذفت بيانات الموقع», no auto reload;
    waiting messages become suppressed and are released; ACTIVE writes keep their resources
    until settle (their replies are suppressed).
  3 wait for the RecoveryGate resolution (<= 25 s); unrecovered without a gate proceeds now.
  4 tomb attempt n: AdminFIFO request (wait <= 5000), then in ONE step grant, guard
    (request not withdrawn, op running, key deleting with this opId), DiskLedger.reserve,
    SeqAlloc.alloc, issue; deadline 5000. Attempt 2 after attempt 1 fails or times out; the
    first keeps its slot until settle. A slot is released only by settle.
  Resolution (one synchronous step): op settled, queued requests withdrawn, deleting left,
    held sessions released. deleted -> confirmed tomb, dict {}; otherwise the key becomes
    unrecovered (the next session recovers anew). Outcomes: deleted | failed | uncertain | adminBusy.
"""

import sys
from pathlib import Path

_HERE = Path(__file__).resolve().parent
_ROOT = _HERE.parent.parent
if 'sr_ref' not in sys.modules:
    sys.path.insert(0, str(_ROOT / 'm1-draft-0.12' / 'tools'))
sys.path.insert(0, str(_ROOT / 'm1-draft-0.13' / 'tools'))
import recovery_ref as R13        # noqa: E402  (0.13, read-only)
import fmt2_codec as C            # noqa: E402

BANNER = 'حُذفت بيانات الموقع'
EXTRA_FAULTS = frozenset({
    'adminNoCancel',       # withdrawal disabled (validation.md:735)
    'adminStaleIssue',     # withdrawal and guard disabled (validation.md:739)
    'adminEarlyRelease',   # slot freed at the tomb deadline (validation.md:744)
    'deleteUnordered',     # tomb issued without closing sessions first (validation.md:747)
    'recoveryNoCancel',    # BR22f-f5: RecoverySlots withdrawal disabled (validation.md:742)
})


class _Sticky(dict):
    """A queued request whose 'withdrawn' flag cannot be set (fault recoveryNoCancel only)."""

    def __setitem__(self, k, v):
        if k == 'withdrawn' and v:
            return
        super().__setitem__(k, v)


class AdminEngine(R13.Engine):
    def __init__(self, sites, records=None, faults=(), get_keys_available=True, **kw):
        base_faults = [f for f in faults if f in R13.FAULTS]
        unknown = set(faults) - R13.FAULTS - EXTRA_FAULTS
        if unknown:
            raise ValueError('unknown faults %s' % sorted(unknown))
        super().__init__(sites, records=records, faults=base_faults, **kw)
        self.faults = frozenset(faults)
        self.get_keys_available = get_keys_available
        self.frames = {}
        self.banners = []
        self.dops = {}                  # opId -> DeleteOp
        self.deleting = {}              # key -> opId
        self.admin_fifo2 = []
        self.admin_slots = []           # ops holding an AdminSlot
        self.held_opens = {}            # key -> [frame]
        self.admin_stats = {'adminStaleDropped': 0, 'adminTombsUnsettledMax': 0, 'adminBusy': 0, 'adminDeletes': 0,
                            'adminDeleteFailed': 0, 'adminDeleteUncertain': 0}

    # ---------------- guards on 0.13 hooks
    def ev_settle(self, n, ok=True):
        if n > len(self.sets):
            return                       # a settle for an op that this path never issued
        op = self.sets[n - 1]
        if op.purpose == 'tomb':
            return self._tomb_settle(op, ok)
        return super().ev_settle(n, ok)

    def ev_apply(self, n):
        if n > len(self.sets):
            return
        super().ev_apply(n)
        op = self.sets[n - 1]
        ks = self.keys[op.key]
        if self.alive and op.state == 'applied' and ks['status'] == 'ready' and ks['confirmed'] is not None \
                and op.version < ks['confirmed']:
            # P-E6-4: a late record below the confirmed version is removed by the sweep (x2, x3, x5a).
            self._sweep_records(op.key, ks['confirmed'])

    def _check(self):
        super()._check()
        u = sum(1 for o in self.sets if o.purpose == 'tomb' and o.settled is None and o.state == 'pending')
        self.admin_stats['adminTombsUnsettledMax'] = max(self.admin_stats['adminTombsUnsettledMax'], u)
        if u > R13.ADMIN_SLOTS:
            self._violate('adminTombsUnsettledOver2', count=u)

    def _reply(self, msg, code, reason=None, sub=None):
        if msg.get('suppressed'):
            msg['replied'] = True
            return
        super()._reply(msg, code, reason, sub)

    def _open_gate(self, key):
        if not self.get_keys_available:                         # d5: getKeys missing -> storeRecovery, no get(null)
            ks = self.keys[key]
            ks['gate'] += 1
            self._resolve(key, 'failed', sub=None)
            return
        super()._open_gate(key)

    def _resolve(self, key, outcome, sub=None, version=None):
        if 'recoveryNoCancel' in self.faults:
            # Fault (BR22f-f5): the request stays queued, unwithdrawn; the grant guard must drop it.
            self.rec_fifo = [_Sticky(r) if (r['key'] == key and r['gate'] == self.keys[key]['gate']) else r for r in self.rec_fifo]
        super()._resolve(key, outcome, sub, version)
        if key in self.deleting:
            self._admin_proceed(key)                              # step 3 done: the gate is resolved

    def _data_settled(self, op, ok):
        """As 0.13, but an older write settling after a newer confirmed record (e.g. a tomb)
        never replaces the dictionary (browser.md:726)."""
        ks = self.keys[op.key]
        before = ks['dict']
        conf = ks['confirmed']
        super()._data_settled(op, ok)
        if ok and conf is not None and op.version < conf:
            ks['dict'] = before
            ks['confirmed'] = conf
            self._sweep_records(op.key, conf)

    # ---------------- sessions
    def ev_access(self, key, req):
        if req[0] == 'snap':
            if key in self.deleting:
                self.held_opens.setdefault(key, []).append(req[1])
                self._tr('sessionHeld', key=key, frame=req[1])
                return
            self.frames[req[1]] = {'key': key, 'alive': True}
        elif key in self.deleting and 'deleteUnordered' not in self.faults:
            return                       # no session can send while deleting
        super().ev_access(key, req)

    def _snapshot(self, key, frame):
        if self.frames.get(frame, {}).get('alive', True):
            super()._snapshot(key, frame)

    # ---------------- AdminDelete
    def ev_delete(self, key):
        if key in self.deleting:
            self.dops[self.deleting[key]]['subscribers'] += 1           # second request shares the promise (x7)
            self._tr('deleteJoined', key=key)
            return
        op_id = 'op%d' % (len(self.dops) + 1)
        self.dops[op_id] = {'id': op_id, 'key': key, 'state': 'running', 'attempts': {}, 'outcome': None, 'requestedAt': self.t,
                            'resolvedAt': None, 'subscribers': 1}
        self.deleting[key] = op_id
        self.admin_stats['adminDeletes'] += 1
        self._tr('deleteRequested', key=key, op=op_id)
        if 'deleteUnordered' not in self.faults:
            self._close_sessions(key)
        ks = self.keys[key]
        if ks['status'] != 'recovering':
            self._start_attempt(op_id, 1)

    def _close_sessions(self, key):
        torn = sorted(f for f, fr in self.frames.items() if fr['key'] == key and fr['alive'])
        for f in torn:
            self.frames[f]['alive'] = False
        if torn:
            self.banners.append({'t': self.t, 'key': key, 'frames': torn, 'text': BANNER, 'autoReload': False})
        ks = self.keys[key]
        for m in list(self.msgs.values()):
            if m['key'] != key or m['replied']:
                continue
            m['suppressed'] = True
            if not m['issued']:
                m['replied'] = True                                   # released without reply
        ks['waitMsgs'] = []
        ks['waitSnaps'] = []
        self.store_fifo = [(k, m) for k, m in self.store_fifo if k != key]

    def _admin_proceed(self, key):
        """Called by 0.13 _resolve once the gate is resolved (step 3 done)."""
        op_id = self.deleting.get(key)
        if op_id and not self.dops[op_id]['attempts']:
            self._start_attempt(op_id, 1)

    def _start_attempt(self, op_id, n):
        dop = self.dops[op_id]
        req = {'op': op_id, 'n': n, 'at': self.t, 'withdrawn': False, 'done': False}
        dop['attempts'][n] = {'state': 'requested', 'req': req, 'set': None}
        self.admin_fifo2.append(req)
        self.at(self.t + R13.ADMIN_SLOT_WAIT, 3.0, self._admin_wait_expired, req)
        self._pump_admin()

    def _pump_admin(self):
        while len(self.admin_slots) < R13.ADMIN_SLOTS and self.admin_fifo2:
            req = self.admin_fifo2.pop(0)
            if req['withdrawn']:
                continue
            dop = self.dops[req['op']]
            guard_ok = dop['state'] == 'running' and self.deleting.get(dop['key']) == dop['id']
            if not guard_ok and 'adminStaleIssue' not in self.faults:
                self.admin_stats['adminStaleDropped'] += 1               # slot returned to the next request at once
                req['done'] = True
                self._tr('adminStaleDropped', op=dop['id'], n=req['n'])
                continue
            req['done'] = True
            self._issue_tomb(dop, req['n'])

    def _issue_tomb(self, dop, n):
        key = dop['key']
        ks = self.keys[key]
        net, addr = self.sites[key]
        seq = ks['seq']
        name = R13.SR12.record_name(net, addr, self.E, seq)
        value = C.encode_value(self.E, seq, {}, tomb=True)
        ticket, bad = self.disk.reserve(C.enc_size(name, value), 'tomb')
        if bad:
            dop['attempts'][n]['state'] = 'diskRejected'
            return self._attempt_done(dop, n)
        ks['seq'] = seq + 1
        op = self._issue_set(key, name, value, 'tomb', (self.E, seq))
        op.ticket, op.attempt, op.gate = ticket, n, dop['id']
        self.admin_slots.append(op)
        dop['attempts'][n].update(state='issued', set=op.n)
        self._tr('tombIssue', key=key, op=dop['id'], attempt=n, version=[self.E, seq])
        self.at(self.t + R13.REC_DEADLINE, 3.0, self._tomb_deadline, dop['id'], n, op)

    def _admin_wait_expired(self, req):
        if req['done'] or req['withdrawn']:
            return
        if req in self.admin_fifo2:
            self.admin_fifo2.remove(req)
        req['done'] = True
        dop = self.dops[req['op']]
        if dop['state'] != 'running':
            return
        dop['attempts'][req['n']]['state'] = 'slotExpired'
        if req['n'] == 1:
            self.admin_stats['adminBusy'] += 1
            return self._admin_resolve(dop, 'adminBusy')
        first = dop['attempts'][1]['state']
        self._admin_resolve(dop, 'uncertain' if first in ('issued', 'expired') else 'failed')

    def _tomb_deadline(self, op_id, n, op):
        dop = self.dops[op_id]
        if op.settled is not None or not self.alive:
            return
        if 'adminEarlyRelease' in self.faults and op in self.admin_slots:
            self.admin_slots.remove(op)
            self._pump_admin()
        if dop['state'] != 'running':
            return
        dop['attempts'][n]['state'] = 'expired'
        self._attempt_done(dop, n)

    def _attempt_done(self, dop, n):
        if dop['state'] != 'running':
            return
        if n == 1 and 2 not in dop['attempts']:
            return self._start_attempt(dop['id'], 2)
        states = [dop['attempts'].get(i, {}).get('state') for i in (1, 2)]
        if all(s in ('failed', 'expired', 'diskRejected', 'slotExpired') for s in states):
            self._admin_resolve(dop, 'uncertain' if 'expired' in states else 'failed')

    def _tomb_settle(self, op, ok):
        if not self.alive or op.settled is not None:
            return
        op.settled = 'ok' if ok else 'fail'
        if ok:
            self._apply(op)
        self.disk.settled(op.ticket)
        if op in self.admin_slots:
            self.admin_slots.remove(op)
        dop = self.dops[op.gate]
        ks = self.keys[op.key]
        if ok:
            if dop['state'] == 'running':
                ks.update(status='ready', confirmed=op.version, dict={}, phase=None, outcome='deleted')
                self._sweep_records(op.key, op.version)
                self._admin_resolve(dop, 'deleted')
            else:
                self._tr('lateTombSuccess', op=dop['id'], version=list(op.version))
        elif dop['state'] == 'running' and dop['attempts'][op.attempt]['state'] == 'issued':
            dop['attempts'][op.attempt]['state'] = 'failed'
            self._attempt_done(dop, op.attempt)
        self._pump_admin()
        self._pump_store()

    def _admin_resolve(self, dop, outcome):
        dop.update(state='settled', outcome=outcome, resolvedAt=self.t)
        if 'adminNoCancel' not in self.faults and 'adminStaleIssue' not in self.faults:
            for r in self.admin_fifo2:
                if r['op'] == dop['id']:
                    r['withdrawn'] = True
        key = dop['key']
        self.deleting.pop(key, None)
        if outcome == 'failed':
            self.admin_stats['adminDeleteFailed'] += 1
        if outcome == 'uncertain':
            self.admin_stats['adminDeleteUncertain'] += 1
        ks = self.keys[key]
        if outcome != 'deleted':
            ks.update(status='unrecovered', phase=None, outcome=outcome)
        self._tr('deleteResolved', key=key, op=dop['id'], outcome=outcome)
        for frame in self.held_opens.pop(key, []):
            self.ev_access(key, ('snap', frame))
        self._pump_admin()
