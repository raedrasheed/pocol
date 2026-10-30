"""E4/E5 reference engine for M1 draft 0.13: RecoveryGate, ReadSlots, RecoverySlots,
SeqAlloc, DiskLedger and fmt-2 records within ONE live worker generation, plus death and late
application. REFERENCE / SPECIFICATION FIXTURE TOOLING ONLY. NOT executed by the author.

Not Chrome: manual clock and a FakeBackend whose sets, removes and reads are applied,
settled, failed or dropped only by the schedule (validation.md:438, 485). All byte figures
are model metrics, never Chrome bytes.

Sources: browser.md:544-610 (RecoveryGate, RecoverySlots, lemmas), 626-713 (disk, DiskLedger,
window, sweep), 716-773 (AdminDelete/AdminSlots: only a narrow adapter here, E6 boundary),
779 (stats); validation.md:437-509 (BR22d-g); governance.md:33-39, 132-142 (literals, BG1).

Reuse: names come from the 0.12 strict helpers (m1-draft-0.12/tools/sr_ref.py, imported
read-only as `sr_ref`); records are encoded and validated by fmt2_codec. The E1-E3 World of
0.12 is untouched. This engine is a separate per-generation component (boundary adapter);
the runner checks the wiring identity and re-runs the whole 0.12 suite.

Same-millisecond order (P-E5-1): placement (apply/drop) 0, death 1, settles 2, recovery
timers 3.0, queue-wait expiry 3.1, requests 4. Within a class, by scheduling order.
"""

import hashlib
import heapq
import json
import sys
from pathlib import Path

_HERE = Path(__file__).resolve().parent
_ROOT = _HERE.parent.parent
if 'sr_ref' not in sys.modules:
    sys.path.insert(0, str(_ROOT / 'm1-draft-0.12' / 'tools'))
import sr_ref as SR12                 # noqa: E402  (0.12 strict helpers, read-only)
sys.path.insert(0, str(_HERE))
import fmt2_codec as C                # noqa: E402

READ_DEADLINE = 5000            # RECOVERY_READ_DEADLINE (governance.md:39)
READ_SLOTS = 2
GETS_MAX = 4
REC_SLOTS = 2                   # RECOVERY_SLOTS (governance.md:38)
REC_SLOT_WAIT = 5000
REC_DEADLINE = 5000             # STORE_RECOVERY_DEADLINE
STORE_QUEUE_WAIT = 10000
STORE_WRITE_DEADLINE = 5000
STORE_ACTIVE = 2
ADMIN_SLOTS = 2
ADMIN_SLOT_WAIT = 5000
GATE_BOUND = READ_DEADLINE + 2 * (REC_SLOT_WAIT + REC_DEADLINE)          # 25000 (browser.md:575-576)
ADMIN_BOUND = GATE_BOUND + 2 * (ADMIN_SLOT_WAIT + REC_DEADLINE)         # 45000 (browser.md:767)

DISK_HARD = 268435456
RECORDS_MAX = 1024
LATE_RESERVE = 4 * (2 + 2) * C.RECORD_BYTES_MAX                          # 23072768
LATE_NAMES = 4 * (2 + 2 + 2)                                             # 24
EPOCH_NAMES_MAX = 8
META_RESERVE = 1048576

FAULTS = frozenset({
    'parallelRecovery',        # a second access starts its own checkpoint (validation.md:463)
    'seqFromCheckpoint',       # the data seq is derived from the checkpoint number (validation.md:464)
    'recoveryNoSlot',          # the second attempt is issued without a RecoverySlot (validation.md:483)
    'lateReadCheckpoint',      # a late read after readTimeout still issues a checkpoint (validation.md:509)
    'slotReleaseOnTimeout',    # author control: a RecoverySlot is freed at the checkpoint deadline
    'readSlotReleaseOnTimeout',  # author control: a ReadSlot is freed at the read deadline
    'staleReadyAfterFailed',   # author control: a late success turns failed into ready
    'seqReuseOnFail',          # author control: the second attempt reuses the first attempt's seq
})


class DiskLedger:
    """D105: synchronous reserve; tickets stay counted until a refresh ISSUED after their
    settle completes (browser.md:644-653). Sites (D112) are E7: not modelled."""

    def __init__(self, in_use=0, names=0):
        self.in_use, self.names = in_use, names
        self.r_bytes = self.r_names = 0
        self.tickets = []
        self.seq = 0
        self.refreshing = None
        self.pending_refresh = False
        self.rejects = 0

    def check(self, enc, kind='set'):
        if kind == 'remove':
            return None
        if kind != 'tomb' and self.in_use + self.r_bytes + enc + LATE_RESERVE + META_RESERVE > DISK_HARD:
            return {'reason': 'disk', 'limit': DISK_HARD, 'dim': 'bytes'}
        if self.names + self.r_names + 1 + LATE_NAMES + EPOCH_NAMES_MAX > RECORDS_MAX:
            return {'reason': 'disk', 'limit': RECORDS_MAX, 'dim': 'names'}
        return None

    def reserve(self, enc, kind='set'):
        bad = self.check(enc, kind)
        if bad:
            self.rejects += 1
            return None, bad
        if kind == 'remove':
            return {'bytes': 0, 'names': 0, 'settleSeq': None, 'kind': kind}, None
        t = {'bytes': 0 if kind == 'tomb' else enc, 'names': 1, 'settleSeq': None, 'kind': kind}
        self.r_bytes += t['bytes']
        self.r_names += 1
        self.tickets.append(t)
        return t, None

    def settled(self, t):
        if t is None or t.get('settleSeq') is not None:
            return
        self.seq += 1
        t['settleSeq'] = self.seq

    def refresh_issue(self):
        """Serialized: only one in flight; a request while busy is remembered."""
        if self.refreshing is not None:
            self.pending_refresh = True
            return None
        self.seq += 1
        self.refreshing = self.seq
        return self.seq

    def refresh_complete(self, in_use, names):
        issued = self.refreshing
        self.in_use, self.names = in_use, names
        keep = []
        for t in self.tickets:
            if t['settleSeq'] is not None and t['settleSeq'] < issued:
                self.r_bytes -= t['bytes']
                self.r_names -= t['names']
            else:
                keep.append(t)
        self.tickets = keep
        self.refreshing = None
        if self.pending_refresh:
            self.pending_refresh = False
            return self.refresh_issue()
        return None


class Op:
    __slots__ = ('n', 'kind', 'key', 'name', 'value', 'state', 'settled', 'purpose', 'attempt', 'gate', 'ticket', 'slot', 'issuedAt',
                 'readKind', 'result', 'msg', 'version', 'seqNo', 'lateCounted')

    def __init__(self, **kw):
        for s in self.__slots__:
            setattr(self, s, kw.get(s))


class Engine:
    def __init__(self, sites, records=None, E=2, faults=(), hold_reads=False, hold_removes=False, seq_start=0, disk=None):
        unknown = set(faults) - FAULTS
        if unknown:
            raise ValueError('unknown faults %s' % sorted(unknown))
        self.faults = frozenset(faults)
        self.sites = {k: tuple(v) for k, v in sites.items()}
        self.items = dict(records or {})
        self.E = E
        self.alive = True
        self.hold_reads, self.hold_removes = hold_reads, hold_removes
        self.seq_start = seq_start
        self.disk = disk or DiskLedger()
        self.q, self.n, self.t = [], 0, None
        self.sets = []            # backend set ops in issue order (set#1, set#2, ...)
        self.reads = []           # read ops in issue order (read#1, ...)
        self.removes = []
        self.keys = {k: self._fresh_key() for k in self.sites}
        self.read_fifo, self.read_held = [], 0
        self.rec_fifo, self.rec_held = [], []
        self.store_active, self.store_fifo = 0, []
        self.admin_fifo, self.admin_held = [], []
        self.replies, self.snapshots, self.trace, self.violations = [], [], [], []
        self.stats = {'recoveries': 0, 'recoveryCorrupt': 0, 'lateRecordsSeen': 0, 'recoveryBusy': 0, 'readTimeouts': 0,
                      'lateReadsDropped': 0, 'recoveryStaleDropped': 0, 'recoveryUnsettledMax': 0, 'readSlotsHeldMax': 0,
                      'storeRecoveryFail': 0, 'diskRejects': 0}
        self.msgs = {}
        self.names_issued = set()

    def _fresh_key(self):
        return {'status': 'unrecovered', 'gate': 0, 'phase': None, 'ts': None, 'cands': [], 'gets': 0, 'corrupt': 0,
                'readDict': None, 'attempts': {}, 'seq': self.seq_start if hasattr(self, 'seq_start') else 0,
                'confirmed': None, 'dict': None, 'waitSnaps': [], 'waitMsgs': [], 'outcome': None, 'deleting': None,
                'resolvedAt': None, 'firstReqAt': None}

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

    def unsettled_recovery(self):
        return sum(1 for o in self.sets if o.purpose == 'checkpoint' and o.settled is None and o.state == 'pending')

    def _check(self):
        u = self.unsettled_recovery()
        self.stats['recoveryUnsettledMax'] = max(self.stats['recoveryUnsettledMax'], u)
        if u > REC_SLOTS:
            self._violate('recoveryUnsettledOverSlots', count=u)
        self.stats['readSlotsHeldMax'] = max(self.stats['readSlotsHeldMax'], self.read_held)
        if self.read_held > READ_SLOTS:
            self._violate('readSlotsOver', count=self.read_held)

    # ---------------- schedule API
    PRIO = {'apply': 0, 'drop': 0, 'death': 1, 'settle': 2, 'settleRead': 2, 'settleRemoves': 2, 'snap': 4, 'set': 4, 'delete': 4, 'tick': 4}

    def load(self, events):
        for e in events:
            d = e['do']
            p = self.PRIO[d]
            if d == 'snap':
                self.at(e['t'], p, self.ev_access, e['key'], ('snap', e['frame']))
            elif d == 'set':
                self.at(e['t'], p, self.ev_access, e['key'], ('set', {'id': e['id'], 'k': e['k'], 'v': e['v']}))
            elif d == 'settle':
                self.at(e['t'], p, self.ev_settle, e['set'], e.get('ok', True))
            elif d == 'settleRead':
                self.at(e['t'], p, self.ev_settle_read, e['read'])
            elif d == 'settleRemoves':
                self.at(e['t'], p, self.ev_settle_removes)
            elif d == 'apply':
                self.at(e['t'], p, self.ev_apply, e['set'])
            elif d == 'drop':
                self.at(e['t'], p, self.ev_drop, e['set'])
            elif d == 'death':
                self.at(e['t'], p, self.ev_death)
            elif d == 'delete':
                self.at(e['t'], p, self.ev_delete, e['key'])
            elif d == 'tick':
                self.at(e['t'], p, lambda: None)
            else:
                raise ValueError(d)

    # ---------------- replies
    def _reply(self, msg, code, reason=None, sub=None):
        if msg['replied']:
            self._violate('duplicateReply', id=msg['id'])
            return
        msg['replied'] = True
        self.replies.append({'t': self.t, 'id': msg['id'], 'code': code, 'reason': reason, 'sub': sub})

    def _fail_msgs(self, ks, sub):
        for m in ks['waitMsgs']:
            if not m['replied']:
                self._reply(m, -32603, 'storeRecovery', sub)
        ks['waitMsgs'] = [m for m in ks['waitMsgs'] if not m['replied']]
        ks['waitSnaps'] = []

    # ---------------- access (snapshot or storage_set) -> RecoveryGate
    def ev_access(self, key, req):
        if not self.alive:
            return
        ks = self.keys[key]
        if req[0] == 'set':
            msg = dict(req[1], key=key, accepted=self.t, replied=False, issued=False)
            self.msgs[msg['id']] = msg
            self.at(self.t + STORE_QUEUE_WAIT, 3.1, self._queue_wait, msg)
        if ks['status'] == 'failed':
            if req[0] == 'set':
                self._reply(msg, -32603, 'storeRecovery', ks.get('failSub'))   # e.g. corrupt (browser.md:553)
            return
        if ks['status'] == 'ready':
            if req[0] == 'snap':
                self._snapshot(key, req[1])
            else:
                self._enqueue_write(key, msg)
            return
        if req[0] == 'snap':
            ks['waitSnaps'].append(req[1])
        else:
            ks['waitMsgs'].append(msg)
        if ks['status'] == 'recovering':
            if 'parallelRecovery' in self.faults:
                self._parallel_checkpoint(key)
            return
        self._open_gate(key)

    def _open_gate(self, key):
        ks = self.keys[key]
        ks.update(status='recovering', phase='read', ts=self.t, cands=[], gets=0, corrupt=0, readDict=None, attempts={},
                  outcome=None, resolvedAt=None)
        ks['gate'] += 1
        if ks['firstReqAt'] is None:
            ks['firstReqAt'] = self.t
        self.stats['recoveries'] += 1
        self._tr('gateOpen', key=key, gate=ks['gate'])
        self.at(self.t + READ_DEADLINE, 3.0, self._read_deadline, key, ks['gate'])
        self._request_read(key, 'keys', None)

    # ---------------- read phase (D110)
    def _request_read(self, key, kind, name):
        self.read_fifo.append({'key': key, 'gate': self.keys[key]['gate'], 'kind': kind, 'name': name})
        self._pump_reads()

    def _pump_reads(self):
        while self.read_held < READ_SLOTS and self.read_fifo:
            r = self.read_fifo.pop(0)
            self.read_held += 1
            op = Op(n=len(self.reads) + 1, kind='read', key=r['key'], name=r['name'], readKind=r['kind'], gate=r['gate'],
                    state='pending', issuedAt=self.t)
            self.reads.append(op)
            self._tr('readIssue', read=op.n, key=r['key'], kind=r['kind'])
            if not self.hold_reads:
                self.ev_settle_read(op.n)

    def ev_settle_read(self, n):
        op = self.reads[n - 1]
        if op.settled:
            return
        op.settled = True
        self.read_held -= 1
        ks = self.keys[op.key]
        net, addr = self.sites[op.key]
        if op.readKind == 'keys':
            op.result = sorted((v for v in (SR12.parse_record_name(x, net, addr) for x in self.items) if v), reverse=True)
        else:
            op.result = self.items.get(op.name)
        current = op.gate == ks['gate'] and ks['status'] == 'recovering' and ks['phase'] == 'read'
        if not current:
            self._pump_reads()
            if 'lateReadCheckpoint' in self.faults:
                # Fault: a late read result is used and a checkpoint is issued from it.
                r = C.lookup_fmt2(self.items, net, addr, SR12.parse_record_name)
                seq = ks['seq']
                ks['seq'] = seq + 1
                self._issue_set(op.key, SR12.record_name(net, addr, self.E, seq),
                                C.encode_value(self.E, seq, r.get('dict') or {}), 'checkpoint', (self.E, seq))
                return
            self.stats['lateReadsDropped'] += 1
            self._tr('lateReadDropped', read=n, key=op.key)
            return
        self._pump_reads()
        if op.readKind == 'keys':
            ks['cands'] = list(op.result)
            if not ks['cands']:
                return self._read_done(op.key, {})
            return self._request_read(op.key, 'get', SR12.record_name(net, addr, *ks['cands'][0]))
        ver = ks['cands'][ks['gets']]
        ks['gets'] += 1
        try:
            d, _tomb = C.decode_value(ver, op.result)
            return self._read_done(op.key, d)
        except C.CodecError as e:
            ks['corrupt'] += 1
            self.stats['recoveryCorrupt'] += 1
            self._tr('corrupt', key=op.key, version=list(ver), reason=e.reason)
        if ks['gets'] < GETS_MAX and ks['gets'] < len(ks['cands']):
            return self._request_read(op.key, 'get', SR12.record_name(net, addr, *ks['cands'][ks['gets']]))
        self._resolve(op.key, 'failed', sub='corrupt')

    def _read_done(self, key, d):
        ks = self.keys[key]
        ks['readDict'] = d
        ks['phase'] = 'slot'
        self._tr('readDone', key=key, dict=d)
        self._request_slot(key, 1)

    def _read_deadline(self, key, gate):
        ks = self.keys[key]
        if not self.alive or ks['gate'] != gate or ks['status'] != 'recovering' or ks['phase'] != 'read':
            return
        self.stats['readTimeouts'] += 1
        self.read_fifo = [r for r in self.read_fifo if not (r['key'] == key and r['gate'] == gate)]
        if 'readSlotReleaseOnTimeout' in self.faults:
            for o in self.reads:
                if o.key == key and o.gate == gate and not o.settled:
                    o.settled = True
                    self.read_held -= 1
            self._pump_reads()
        self._resolve(key, 'unrecovered', sub='readTimeout')

    # ---------------- RecoverySlots (D109) and checkpoint attempts
    def _request_slot(self, key, n):
        ks = self.keys[key]
        req = {'key': key, 'gate': ks['gate'], 'n': n, 'at': self.t, 'withdrawn': False, 'done': False}
        ks['attempts'][n] = {'state': 'requested', 'req': req, 'op': None, 'seq': None}
        if n == 2 and 'recoveryNoSlot' in self.faults:
            req['done'] = True
            self._issue_checkpoint(key, n, slot=None)
            return
        self.rec_fifo.append(req)
        self.at(self.t + REC_SLOT_WAIT, 3.0, self._slot_wait_expired, req)
        self._pump_slots()

    def _pump_slots(self):
        while len(self.rec_held) < REC_SLOTS and self.rec_fifo:
            req = self.rec_fifo.pop(0)
            if req['withdrawn']:
                continue
            ks = self.keys[req['key']]
            if ks['gate'] != req['gate'] or ks['status'] != 'recovering':
                self.stats['recoveryStaleDropped'] += 1
                continue
            req['done'] = True
            self._issue_checkpoint(req['key'], req['n'], slot=True)

    def _issue_checkpoint(self, key, n, slot):
        """Grant, guard, DiskLedger.reserve, alloc, issue: one synchronous step (browser.md:561)."""
        ks = self.keys[key]
        att = ks['attempts'][n]
        net, addr = self.sites[key]
        if 'seqReuseOnFail' in self.faults and n == 2 and ks['attempts'].get(1, {}).get('seq') is not None:
            seq = ks['attempts'][1]['seq']
        else:
            seq = ks['seq']
            if seq > C.SEQ_MAX:
                att['state'] = 'failed'
                return self._attempt_over(key, n)
        name = SR12.record_name(net, addr, self.E, seq)
        value = C.encode_value(self.E, seq, ks['readDict'])
        ticket, bad = self.disk.reserve(C.enc_size(name, value), 'set')
        if bad:
            self.stats['diskRejects'] += 1
            att['state'] = 'diskRejected'
            self._tr('diskReject', key=key, attempt=n)
            return self._attempt_over(key, n)
        if not ('seqReuseOnFail' in self.faults and n == 2 and seq == ks['attempts'].get(1, {}).get('seq')):
            ks['seq'] = seq + 1
        op = self._issue_set(key, name, value, 'checkpoint', (self.E, seq))
        op.attempt, op.gate, op.ticket = n, ks['gate'], ticket
        if slot:
            self.rec_held.append(op)
        att.update(state='issued', op=op, seq=seq)
        self._tr('checkpointIssue', key=key, attempt=n, version=[self.E, seq])
        self.at(self.t + REC_DEADLINE, 3.0, self._cp_deadline, key, op)

    def _issue_set(self, key, name, value, purpose, version):
        if name in self.names_issued:
            self._violate('recordNameReused', name=name)
        self.names_issued.add(name)
        op = Op(n=len(self.sets) + 1, kind='set', key=key, name=name, value=value, state='pending', purpose=purpose,
                issuedAt=self.t, version=version)
        self.sets.append(op)
        return op

    def _slot_wait_expired(self, req):
        if req['done'] or req['withdrawn'] or not self.alive:
            return
        if req in self.rec_fifo:
            self.rec_fifo.remove(req)
        req['done'] = True
        ks = self.keys[req['key']]
        if ks['gate'] != req['gate'] or ks['status'] != 'recovering':
            return
        ks['attempts'][req['n']]['state'] = 'slotExpired'
        if req['n'] == 1:
            self.stats['recoveryBusy'] += 1
            return self._resolve(req['key'], 'unrecovered', sub='busy')
        self._attempt_over(req['key'], 2)

    def _cp_deadline(self, key, op):
        if not self.alive or op.settled is not None:
            return
        ks = self.keys[key]
        att = ks['attempts'].get(op.attempt)
        if att is None or att['op'] is not op:
            return
        if 'slotReleaseOnTimeout' in self.faults and op in self.rec_held:
            self.rec_held.remove(op)
            self._pump_slots()
        if ks['gate'] != op.gate or ks['status'] != 'recovering':
            return
        att['state'] = 'expired'
        self._tr('checkpointDeadline', key=key, attempt=op.attempt)
        self._attempt_over(key, op.attempt)

    def _attempt_over(self, key, n):
        """Attempt n failed (settle failure, deadline, disk rejection or no slot for attempt 2)."""
        ks = self.keys[key]
        if ks['status'] != 'recovering':
            return
        if n == 1 and 2 not in ks['attempts']:
            return self._request_slot(key, 2)
        done = [ks['attempts'].get(i, {}).get('state') in ('failed', 'expired', 'diskRejected', 'slotExpired') for i in (1, 2)]
        if all(done):
            self._resolve(key, 'failed', sub=None)

    def _resolve(self, key, outcome, sub=None, version=None):
        ks = self.keys[key]
        ks['resolvedAt'] = self.t
        for req in self.rec_fifo:
            if req['key'] == key and req['gate'] == ks['gate']:
                req['withdrawn'] = True
        self._tr('resolved', key=key, outcome=outcome, sub=sub, gate=ks['gate'])
        if outcome == 'ready':
            ks.update(status='ready', phase=None, outcome='ready', confirmed=version, dict=dict(ks['readDict']))
            for fr in ks['waitSnaps']:
                self._snapshot(key, fr)
            ks['waitSnaps'] = []
            msgs, ks['waitMsgs'] = ks['waitMsgs'], []
            for m in msgs:
                if not m['replied']:
                    self._enqueue_write(key, m)
            self._sweep_records(key, version)
        elif outcome == 'failed':
            ks.update(status='failed', phase=None, outcome='failed', failSub=sub)
            self.stats['storeRecoveryFail'] += 1
            self._fail_msgs(ks, sub)
        else:
            ks.update(status='unrecovered', phase=None, outcome=sub)
            self._fail_msgs(ks, sub)
        if ks['deleting'] is not None:
            self._admin_proceed(key)
        self._pump_slots()

    def _parallel_checkpoint(self, key):
        """Fault only: a second, independent recovery for the same key."""
        ks = self.keys[key]
        net, addr = self.sites[key]
        r = C.lookup_fmt2(self.items, net, addr, SR12.parse_record_name)
        name = SR12.record_name(net, addr, self.E, self.seq_start)
        self._issue_set(key, name, C.encode_value(self.E, self.seq_start, r.get('dict') or {}), 'checkpoint', (self.E, self.seq_start))

    # ---------------- settle / apply
    def ev_settle(self, n, ok=True):
        if not self.alive:
            return
        op = self.sets[n - 1]
        if op.settled is not None:
            return
        op.settled = 'ok' if ok else 'fail'
        if ok:
            self._apply(op)
        self.disk.settled(op.ticket)
        if op in self.rec_held:
            self.rec_held.remove(op)
        ks = self.keys[op.key]
        if op.purpose == 'checkpoint':
            self._cp_settled(op, ok)
        elif op.purpose == 'data':
            self._data_settled(op, ok)
        elif op.purpose == 'tomb':
            if op in self.admin_held:
                self.admin_held.remove(op)
            if ok:
                ks.update(status='ready', confirmed=op.version, dict={})
                self._tr('tombSettled', key=op.key, version=list(op.version))
        self._pump_slots()
        self._pump_store()

    def _cp_settled(self, op, ok):
        ks = self.keys[op.key]
        same_gate = ks['gate'] == op.gate
        if ok:
            if same_gate and ks['status'] == 'recovering':
                return self._resolve(op.key, 'ready', version=op.version)
            if same_gate and ks['status'] == 'ready' and ks['confirmed'] is not None and op.version > ks['confirmed']:
                ks['confirmed'] = op.version              # monotonic; the dictionary is the same (browser.md:590)
                return
            if ks['status'] == 'failed' and 'staleReadyAfterFailed' in self.faults:
                ks.update(status='ready', confirmed=op.version, dict=dict(ks['readDict'] or {}))
            self._late_seen(op)
            return
        att = ks['attempts'].get(op.attempt)
        if same_gate and ks['status'] == 'recovering' and att and att['op'] is op and att['state'] == 'issued':
            att['state'] = 'failed'
            self._attempt_over(op.key, op.attempt)

    def _late_seen(self, op):
        if not op.lateCounted:
            op.lateCounted = True
            self.stats['lateRecordsSeen'] += 1
            self._tr('lateRecordSeen', key=op.key, version=list(op.version))

    def _apply(self, op):
        if op.state != 'pending':
            return False
        op.state = 'applied'
        if op.kind == 'set':
            self.items[op.name] = op.value
        else:
            self.items.pop(op.name, None)
        return True

    def ev_apply(self, n):
        op = self.sets[n - 1]
        if not self._apply(op):
            return
        self._tr('lateApply', set=n, name=op.name)
        if not self.alive:
            return
        ks = self.keys[op.key]
        if op.purpose == 'checkpoint':
            resolved = ks['gate'] != op.gate or ks['status'] in ('ready', 'failed', 'unrecovered')
            below = ks['confirmed'] is None or op.version < ks['confirmed']
            if resolved and (ks['status'] == 'failed' or below):
                self._late_seen(op)

    def ev_drop(self, n):
        op = self.sets[n - 1]
        if op.state == 'pending':
            op.state = 'dropped'

    def ev_death(self):
        self.alive = False
        self._tr('death')

    # ---------------- data writes (StoreQueue boundary adapter: per-key FIFO, <= 2 active)
    def _enqueue_write(self, key, msg):
        self.store_fifo.append((key, msg))
        self._pump_store()

    def _pump_store(self):
        if not self.alive:
            return
        busy = {o.key for o in self.sets if o.purpose == 'data' and o.settled is None and o.state != 'dropped'}
        i = 0
        while i < len(self.store_fifo) and self.store_active < STORE_ACTIVE:
            key, msg = self.store_fifo[i]
            if key in busy or msg['replied']:
                if msg['replied']:
                    self.store_fifo.pop(i)
                    continue
                i += 1
                continue
            self.store_fifo.pop(i)
            self._issue_data(key, msg)
            busy.add(key)

    def _issue_data(self, key, msg):
        ks = self.keys[key]
        net, addr = self.sites[key]
        if 'seqFromCheckpoint' in self.faults:
            seq = ks['attempts'][1]['seq'] + 1
        else:
            seq = ks['seq']
        if seq > C.SEQ_MAX:
            self._tr('seqExhausted', key=key, id=msg['id'])
            self._reply(msg, -32603, 'storeRecovery', None)          # literal shape (validation.md:467)
            return
        new = dict(ks['dict'])
        if msg['v'] is None:
            new.pop(msg['k'], None)
        else:
            new[msg['k']] = msg['v']
        name = SR12.record_name(net, addr, self.E, seq)
        value = C.encode_value(self.E, seq, new)
        ticket, bad = self.disk.reserve(C.enc_size(name, value), 'set')
        if bad:
            self._reply(msg, 4300, 'disk', bad['limit'])
            return
        if 'seqFromCheckpoint' not in self.faults:
            ks['seq'] = seq + 1
        op = self._issue_set(key, name, value, 'data', (self.E, seq))
        op.ticket, op.msg = ticket, msg
        msg['issued'] = True
        self.store_active += 1
        self._tr('dataIssue', key=key, id=msg['id'], version=[self.E, seq])
        self.at(self.t + STORE_WRITE_DEADLINE, 3.0, self._write_deadline, op)

    def _write_deadline(self, op):
        if self.alive and op.settled is None and not op.msg['replied']:
            self._reply(op.msg, -32603, 'storeTimeout')

    def _data_settled(self, op, ok):
        ks = self.keys[op.key]
        self.store_active -= 1
        if ok:
            if not op.msg['replied']:
                self._reply(op.msg, None)
            if ks['confirmed'] is None or op.version > ks['confirmed']:
                ks['confirmed'] = op.version
            d, _ = C.decode_value(op.version, op.value)
            ks['dict'] = d
            self._sweep_records(op.key, op.version)
        else:
            if not op.msg['replied']:
                self._reply(op.msg, -32603, 'transport')
            net, addr = self.sites[op.key]
            r = C.lookup_fmt2(self.items, net, addr, SR12.parse_record_name)
            ks['dict'] = r.get('dict') or {}

    def _queue_wait(self, msg):
        if not self.alive or msg['replied'] or msg['issued']:
            return
        self._reply(msg, -32005, 'store')
        ks = self.keys[msg['key']]
        ks['waitMsgs'] = [m for m in ks['waitMsgs'] if m is not msg]
        self.store_fifo = [(k, m) for k, m in self.store_fifo if m is not msg]

    # ---------------- sweep (browser.md:542, 565, 713)
    def _sweep_records(self, key, version):
        net, addr = self.sites[key]
        for name in sorted(self.items):
            v = SR12.parse_record_name(name, net, addr)
            if v and v < version:
                op = Op(n=len(self.removes) + 1, kind='remove', key=key, name=name, state='pending', issuedAt=self.t)
                self.removes.append(op)
                if not self.hold_removes:
                    self._apply(op)
                    op.settled = 'ok'

    def ev_settle_removes(self):
        for op in self.removes:
            if op.settled is None:
                op.settled = 'ok'
                self._apply(op)

    # ---------------- AdminDelete adapter (E6 boundary; browser.md:716-726, 740-744)
    def ev_delete(self, key):
        ks = self.keys[key]
        ks['deleting'] = {'requestedAt': self.t, 'tomb': None}
        self._tr('deleteRequested', key=key)
        if ks['status'] != 'recovering':
            self._admin_proceed(key)

    def _admin_proceed(self, key):
        ks = self.keys[key]
        if ks['deleting'] is None or ks['deleting']['tomb'] is not None:
            return
        if len(self.admin_held) >= ADMIN_SLOTS:
            return                                   # adminBusy path is E6; not exercised here
        net, addr = self.sites[key]
        seq = ks['seq']
        name = SR12.record_name(net, addr, self.E, seq)
        value = C.encode_value(self.E, seq, {}, tomb=True)
        ticket, _ = self.disk.reserve(C.enc_size(name, value), 'tomb')
        ks['seq'] = seq + 1
        op = self._issue_set(key, name, value, 'tomb', (self.E, seq))
        op.ticket = ticket
        self.admin_held.append(op)
        ks['deleting']['tomb'] = op
        self._tr('tombIssue', key=key, version=[self.E, seq])

    # ---------------- snapshots and summaries
    def _snapshot(self, key, frame):
        self.snapshots.append({'t': self.t, 'key': key, 'frame': frame, 'dict': dict(self.keys[key]['dict'])})

    def backend_records(self, key):
        net, addr = self.sites[key]
        return sorted([list(v) for v in (SR12.parse_record_name(n, net, addr) for n in self.items) if v])

    def backend_hash(self):
        return hashlib.sha256(C.compact_json(self.items).encode('utf-8')).hexdigest()
