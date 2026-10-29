"""SR-model reference for M1 draft 0.10 (annex rows E1-E3: worker-generation recovery,
D103 / D134 / I57 / I74). SPECIFICATION FIXTURE TOOLING ONLY.

Not a Chrome implementation: no chrome.storage, no service worker, no timers. Time is a
manual clock (every event carries its millisecond). The fake backend holds every set and
remove until the schedule applies it, settles it (ok = applied, fail = not applied) or
drops it, as BR22a/BR22h describe. "Worker generation" here is the MV3 service-worker
lifetime; it is NOT the protocol-chain epoch and NOT the local UI broker's recovery.

Sources (Arabic baseline, line numbers as read for 0.10):
  reference/browser.md      497-531 (fence, name gate, sweep, lemmas), 533-542 (records),
                             544-610 (RecoveryGate, lemmas), 612-624 (memory, frames),
                             626-713 (disk boundary used only through explicit adapters)
  reference/validation.md   390-436 (BR22a/b/c), 510-551 (BR22h h1-h6 and controls)
  reference/governance.md   33-39 (defaults), 276-278 (controls)
  reference/threat.md       48-49 (A15c, A15d)

Modelling choices that the baseline does not fix are tagged P-E… and listed in
annex/E-REFERENCE-AND-FINDINGS.md. Out of scope (E4-E7): fmt 2 byte codec and
RECORD_BYTES_MAX, DiskLedger, ReadSlots/RecoverySlots, AdminDelete, tombstone reaping,
second checkpoint attempt. Where E1-E3 must touch them, explicit adapters are used.
"""

import heapq
import json
import re

T_LATE = 60000                 # governance.md:35
EPOCH_NAMES_MAX = 8            # browser.md:510
GEN_WINDOW_MAX = 4             # browser.md:707
STORE_EPOCH_RETRY = 3          # browser.md:514-516
EPOCH_BYTES_MAX = 256          # governance.md:37
STORE_WRITE_DEADLINE = 5000
RECOVERY_GETS_MAX = 4          # browser.md:550
STORE_RECORDS_MAX = 1024       # governance.md:34
STORE_DISK_HARD = 268435456
STORE_SITES_MAX = 64
LATE_SITES_PER_GEN = 4
SEQ_MAX = 2 ** 32 - 1          # browser.md:539
EPOCH_MAX = 2 ** 64 - 1
EPOCH_PREFIX = 'pocol:epoch:'

_EPOCH_RE = re.compile(r'^pocol:epoch:([0-9a-f]{16})$')
_REC_TAIL_RE = re.compile(r'^([0-9a-f]{16}):([0-9a-f]{8})$')

FAULTS = frozenset({
    'epochNonceNames',     # per-boot nonce names and no name gate (governance.md:276)
    'epochNoGate',         # shared name without the name gate (governance.md:277); task alias epochNoNameGate
    'epochSweepEarly',     # sweep right at confirmation, not after T_LATE (governance.md:278)
    'epochSweepByBootMs',  # sweep by stored bootMs instead of by name order (author control; task-named)
    'singleItem',          # one storage item per key, overwritten in place (alternative (b), browser.md:503)
    'noEpochConfirm',      # data before the fence item is confirmed (validation.md:419)
    'noCheckpoint',        # snapshot from the read, no checkpoint record (validation.md:420)
    'reuseSeq',            # SeqAlloc gives a failed seq again (validation.md:421)
})


# ------------------------------------------------------------------ E1: names and records

def epoch_name(E):
    """'pocol:epoch:'+hex16(E), lowercase, zero-padded, no nonce (browser.md:508; P-E1-1)."""
    if not (isinstance(E, int) and 1 <= E <= EPOCH_MAX):
        raise ValueError('epoch out of range')
    return EPOCH_PREFIX + format(E, '016x')


def parse_epoch_name(name):
    """Strict inverse; anything else is not an epoch item of this format (no normalization)."""
    m = _EPOCH_RE.match(name) if isinstance(name, str) else None
    if not m:
        return None
    E = int(m.group(1), 16)
    return E if E >= 1 else None


def key_prefix(net, addr):
    return 'site:%s:%s:r:' % (net, addr)


def record_name(net, addr, E, seq):
    """'site:'+netKey+':'+addr+':r:'+hex16(E)+':'+hex8(seq) (browser.md:534)."""
    if not (isinstance(E, int) and 1 <= E <= EPOCH_MAX):
        raise ValueError('epoch out of range')
    if not (isinstance(seq, int) and 0 <= seq <= SEQ_MAX):
        raise ValueError('seq out of range')
    return key_prefix(net, addr) + format(E, '016x') + ':' + format(seq, '08x')


def parse_record_name(name, net, addr):
    p = key_prefix(net, addr)
    if not isinstance(name, str) or not name.startswith(p):
        return None
    m = _REC_TAIL_RE.match(name[len(p):])
    if not m:
        return None
    E, seq = int(m.group(1), 16), int(m.group(2), 16)
    return (E, seq) if E >= 1 else None


def single_name(net, addr):
    """Name used only by the singleItem fault."""
    return 'site:%s:%s:single' % (net, addr)


def record_value(E, seq, d, tomb=False):
    """ABSTRACT value {fmt: 2, epoch, seq, tomb, dict}. The real value carries b64 of the
    D104 byte serialization; that codec is row E4 and is not validated here (P-E1-3)."""
    return {'fmt': 2, 'epoch': E, 'seq': seq, 'tomb': bool(tomb), 'dict': dict(d)}


def valid_record(ver, v):
    if not isinstance(v, dict) or v.get('fmt') != 2:
        return False
    if v.get('epoch') != ver[0] or v.get('seq') != ver[1] or not isinstance(v.get('tomb'), bool):
        return False
    d = v.get('dict')
    if not isinstance(d, dict) or not all(isinstance(k, str) and isinstance(x, str) for k, x in d.items()):
        return False
    return not (v['tomb'] and d)          # a tomb carries the empty dictionary (P-E1-4)


def lookup(items, net, addr, gets_max=RECOVERY_GETS_MAX):
    """Confirmed state = dict of the largest present VALID record (browser.md:541, 550-553).
    Reads the largest name, then the next lower ones, up to gets_max reads."""
    vers = sorted(((parse_record_name(n, net, addr), n) for n in items if parse_record_name(n, net, addr)), reverse=True)
    gets = corrupt = 0
    for ver, n in vers:
        if gets == gets_max:
            break
        gets += 1
        if valid_record(ver, items[n]):
            v = items[n]
            return {'failed': False, 'version': ver, 'dict': dict(v['dict']), 'tomb': v['tomb'], 'gets': gets, 'corrupt': corrupt}
        corrupt += 1
    if corrupt:
        return {'failed': True, 'reason': 'corrupt', 'gets': gets, 'corrupt': corrupt}
    return {'failed': False, 'version': None, 'dict': {}, 'tomb': False, 'gets': 0, 'corrupt': 0}


def epoch_value_ok(v):
    """{bootMs} within EPOCH_BYTES_MAX, measured as compact JSON (P-E1-2)."""
    return (isinstance(v, dict) and set(v) == {'bootMs'} and isinstance(v['bootMs'], int)
            and len(json.dumps(v, separators=(',', ':'))) <= EPOCH_BYTES_MAX)


def late_gens(epochs, live_E, now):
    """D112 adapter (browser.md:665): visible epoch items X <= E with bootMs >= now-T_LATE."""
    return sum(1 for E, _, b in epochs if E <= live_E and b is not None and b >= now - T_LATE)


def site_admission(sites_live, r_sites, lg):
    """D112 adapter (browser.md:676-679) for one creating operation."""
    need = sites_live + r_sites + LATE_SITES_PER_GEN * lg + 1
    if need <= STORE_SITES_MAX:
        return {'decision': None, 'need': need}
    return {'decision': 4300, 'reason': 'sites', 'limit': STORE_SITES_MAX, 'need': need,
            'retryPossible': sites_live + r_sites + 1 <= STORE_SITES_MAX}


# ------------------------------------------------------------------ fake backend

class Op:
    __slots__ = ('id', 'gen', 'kind', 'name', 'value', 'issued', 'state', 'settled')

    def __init__(self, **kw):
        for s in self.__slots__:
            setattr(self, s, kw.get(s))

    @property
    def cat(self):
        if self.kind == 'remove':
            return 'remove'
        return 'fence' if self.name.startswith(EPOCH_PREFIX) else 'record'


class Backend:
    """FakeBackend: holds each op pending until apply / settle / drop (validation.md:391, 512).
    A settle with ok applies; a failed settle does not apply (A15d); apply without settle
    is the D107 'apply' event."""

    def __init__(self, items=None):
        self.items = dict(items or {})
        self.ops = []
        self.removed_epoch = set()
        self.resurrected = 0

    def issue(self, gen, kind, name, value, t):
        op = Op(id=len(self.ops) + 1, gen=gen, kind=kind, name=name, value=value, issued=t, state='pending', settled=None)
        self.ops.append(op)
        return op

    def apply(self, op, world):
        if op.state != 'pending':
            return False
        op.state = 'applied'
        if op.kind == 'set':
            if op.name.startswith(EPOCH_PREFIX) and op.name in self.removed_epoch:
                self.resurrected += 1
            self.items[op.name] = op.value
        elif op.name in self.items:
            world._on_remove(op.name)
            del self.items[op.name]
            if op.name.startswith(EPOCH_PREFIX):
                self.removed_epoch.add(op.name)
        return True

    @staticmethod
    def drop(op):
        if op.state == 'pending':
            op.state = 'dropped'


# ------------------------------------------------------------------ generations

class Gen:
    def __init__(self, gid, boot, cfg):
        self.id, self.boot, self.cfg = gid, boot, dict(cfg or {})
        self.alive = True
        self.E = None
        self.EN = None
        self.state = 'booting'          # booting|gateBlocked|windowWait|fencing|confirmed|disabled
        self.reason = None
        self.fence_fails = 0
        self.fence_ops = []
        self.sweep_at = None
        self.keys = {}
        self.confirmed_at = None
        self.fence_issues = 0


class World:
    """One extension profile: a backend plus a sequence of worker generations.

    Fixed timing conventions (P-E3-1), all relative to the triggering event:
      fence issued at the attempt; alive-generation ops settle 1 ms after issue
      (unless the generation's cfg says they never settle); recovery read 1 ms after the
      first request once the fence is confirmed; checkpoint issued 2 ms after the read;
      data set issued when the key is ready and no write is in flight; removes settle 1 ms
      after issue. Same-millisecond priority: placement (apply/drop) 0, death 1, settle 2,
      generation steps 3, user/page events 4."""

    def __init__(self, items=None, sites=None, faults=()):
        unknown = set(faults) - FAULTS
        if unknown:
            raise ValueError('unknown faults %s' % sorted(unknown))
        self.faults = frozenset(faults)
        self.be = Backend(items)
        self.sites = dict(sites or {'SA': ('n1', '0xa1')})
        self.t = None
        self.q = []
        self.n = 0
        self.gens = {}
        self.stats = {'epochGateBlocks': 0, 'action25': 0, 'lateRecordsSeen': 0}
        self.replies = []
        self.snapshots = []
        self.tabs = {}
        self.banners = []
        self.clicks = 0
        self.undelivered = []
        self.msgs = {}
        self.violations = []
        self._vkinds = {}
        self.max_visible = 0
        self.max_names = 0
        self.max_items = 0
        self.max_bytes = 0
        self.ever_max = {}
        self.issued_records = set()
        self.record_writers = []
        self.trace = []
        self.probes = []
        for k, (net, addr) in self.sites.items():
            r = lookup(self.be.items, net, addr)
            if r.get('version'):
                self.ever_max[k] = r['version']

    # ---- infrastructure
    def at(self, t, prio, fn, *args):
        self.n += 1
        heapq.heappush(self.q, (t, prio, self.n, fn, args))

    def run(self, until):
        while self.q and self.q[0][0] <= until:
            t, _, _, fn, args = heapq.heappop(self.q)
            self.t = t
            fn(*args)
            self._observe()
        self.t = until

    def _trace(self, ev, **kw):
        rec = {'t': self.t, 'ev': ev}
        rec.update(kw)
        self.trace.append(rec)

    def _violate(self, kind, **kw):
        if kind not in self._vkinds:
            rec = {'t': self.t, 'kind': kind}
            rec.update(kw)
            self.violations.append(rec)
            self._vkinds[kind] = 0
        self._vkinds[kind] += 1

    def epochs(self):
        out = []
        for n, v in self.be.items.items():
            if not n.startswith(EPOCH_PREFIX):
                continue
            E = parse_epoch_name(n)
            if E is None and 'epochNonceNames' in self.faults:
                E = parse_epoch_name(n[:len(EPOCH_PREFIX) + 16])
            if E is None:
                continue                      # not an epoch item of this format (P-E1-1)
            b = v.get('bootMs') if isinstance(v, dict) else None
            out.append((E, n, b))
        return out

    def _observe(self):
        eps = self.epochs()
        self.max_names = max(self.max_names, len(eps))
        if len(eps) > EPOCH_NAMES_MAX:
            self._violate('epochNamesOverMax', count=len(eps))
        M = max((e for e, _, _ in eps), default=0)
        if M < self.max_visible:
            self._violate('maxVisibleDecreased', was=self.max_visible, now=M)
        self.max_visible = max(self.max_visible, M)
        if self.be.resurrected:
            self._violate('epochResurrected', count=self.be.resurrected)
        nitems = len(self.be.items)
        nbytes = sum(len(n) + len(json.dumps(v, sort_keys=True, separators=(',', ':'))) for n, v in self.be.items.items())
        self.max_items = max(self.max_items, nitems)
        self.max_bytes = max(self.max_bytes, nbytes)
        if nitems > STORE_RECORDS_MAX or nbytes > STORE_DISK_HARD:
            self._violate('fakeBackendBoundExceeded', items=nitems, bytes=nbytes)
        if 'singleItem' not in self.faults:
            for k, (net, addr) in self.sites.items():
                vers = [parse_record_name(n, net, addr) for n in self.be.items]
                vers = [v for v in vers if v]
                cur = max(vers) if vers else None
                ever = self.ever_max.get(k)
                if cur is not None and (ever is None or cur > ever):
                    self.ever_max[k] = cur
                elif ever is not None and (cur is None or cur < ever):
                    self._violate('greatestRecordLost', key=k, ever=list(ever), now=list(cur) if cur else None)

    def _on_remove(self, name):
        for k, (net, addr) in self.sites.items():
            r = parse_record_name(name, net, addr)
            if r is None:
                continue
            higher = [v for v in (parse_record_name(n, net, addr) for n in self.be.items if n != name) if v and v > r]
            if not higher:
                self._violate('removedWithoutGreater', key=k, removed=list(r))

    # ---- scenario events
    def ev_boot(self, gid, cfg=None):
        g = Gen(gid, self.t, cfg)
        self.gens[gid] = g
        self._trace('boot', gen=gid)
        self._attempt(g, False)

    def ev_death(self, gid):
        g = self.gens.get(gid)
        if g is None or not g.alive:
            return
        g.alive = False
        self._trace('death', gen=gid)
        torn = []
        for tab, fr in self.tabs.items():
            if fr['gen'] == gid and fr['alive']:
                fr['alive'] = False
                torn.append(tab)
        if torn:
            # browser.md:619-623: tear down, show the banner, never reload automatically.
            self.banners.append({'t': self.t, 'gen': gid, 'tabs': sorted(torn),
                                 'text': 'انقطع عامل الإضافة؛ قد تكون آخر الكتابات غير محفوظة', 'button': 'إعادة التحميل'})

    def ev_place(self, gid, cat, action):
        for op in self.be.ops:
            if op.gen == gid and op.state == 'pending' and op.cat == cat:
                if action == 'apply':
                    self.be.apply(op, self)
                    self._trace('lateApply', gen=gid, name=op.name)
                else:
                    self.be.drop(op)
                    self._trace('drop', gen=gid, name=op.name)

    def ev_settle(self, gid, cat, ok):
        g = self.gens[gid]
        for op in self.be.ops:
            if op.gen == gid and op.cat == cat and op.settled is None and op.state == 'pending':
                if cat == 'record':
                    for key, ks in g.keys.items():
                        if ks['inflight'] and ks['inflight'][0] is op:
                            self._settle_data(g, key, op, ok)
                            return
                return

    def ev_click(self, tab, key='SA'):
        alive = [g for g in self.gens.values() if g.alive]
        if not alive:
            self._trace('clickNoWorker', tab=tab)
            return
        g = max(alive, key=lambda x: x.boot)
        self.clicks += 1
        fr = {'id': '%s@g%d#%d' % (tab, g.id, self.clicks), 'gen': g.id, 'alive': True, 'tab': tab}
        self.tabs[tab] = fr
        if g.state == 'disabled':
            return
        self._ks(g, key)['queue'].append(('snap', fr))
        self._pump(g, key)

    def ev_set(self, mid, k, v, tab=None, gen=None, key='SA'):
        if gen is not None:
            g = self.gens.get(gen)
            fr = {'id': 'direct@g%d' % gen, 'gen': gen, 'alive': True}
        else:
            fr = self.tabs.get(tab)
            g = self.gens.get(fr['gen']) if fr else None
        if fr is None or g is None or not g.alive or not fr['alive']:
            self.undelivered.append(mid)          # torn frame or dead worker: never answered (browser.md:615)
            return
        msg = {'id': mid, 'gen': g.id, 'frame': fr['id'], 'key': key, 'k': k, 'v': v, 'replies': 0}
        self.msgs[mid] = msg
        if g.state == 'disabled':
            self._reply(msg, -32603, 'storeRecovery', g.reason)
            return
        self._ks(g, key)['queue'].append(('set', msg))
        self._pump(g, key)

    def ev_site_check(self, label, gid, sites_live, r_sites=0):
        g = self.gens[gid]
        eps = self.epochs()
        lg = late_gens(eps, g.E, self.t)
        res = site_admission(sites_live, r_sites, lg)
        disk_keys = sites_live + sum(1 for k, (net, addr) in self.sites.items() if k != 'SA'
                                     and any(parse_record_name(n, net, addr) for n in self.be.items))
        self.probes.append({'t': self.t, 'label': label, 'lateGens': lg, 'LATE_SITES': LATE_SITES_PER_GEN * lg,
                            'decision': res, 'diskKeys': disk_keys,
                            'epochs': sorted([[E, b] for E, _, b in eps])})

    def ev_probe(self, label):
        self.probes.append({'t': self.t, 'label': label, 'epochs': sorted([[E, b] for E, _, b in self.epochs()]),
                            'items': sorted(self.be.items)})

    # ---- fence (D134)
    def _attempt(self, g, second):
        if not g.alive or g.state in ('fencing', 'confirmed', 'disabled'):
            return
        eps = self.epochs()
        EN = len(eps)
        M = max((e for e, _, _ in eps), default=0)
        gated = not ({'epochNoGate', 'epochNonceNames'} & self.faults)
        if gated and EN + 1 > EPOCH_NAMES_MAX:
            if second:
                g.state, g.reason = 'disabled', 'epochNames'
                self.stats['action25'] += 1
                self._trace('disabled', gen=g.id, reason='epochNames', EN=EN)
                self._fail_requests(g)
                return
            self.stats['epochGateBlocks'] += 1
            g.state = 'gateBlocked'
            self._trace('gateBlocked', gen=g.id, EN=EN)
            self._schedule_sweep(g)
            return
        window = [b for _, _, b in eps if b is not None and b >= self.t - T_LATE]
        if len(window) >= GEN_WINDOW_MAX:
            g.state = 'windowWait'
            until = min(window) + T_LATE + 1
            self._trace('windowWait', gen=g.id, count=len(window), until=until)
            self.at(until, 3, self._attempt, g, second)
            return
        g.E, g.EN = M + 1, EN
        g.state = 'fencing'
        self._trace('fenceIssue', gen=g.id, E=g.E, EN=EN)
        self._issue_fence(g)

    def _issue_fence(self, g):
        name = epoch_name(g.E)
        if 'epochNonceNames' in self.faults:
            name = name + ':' + format(g.id, '08x')
        op = self.be.issue(g.id, 'set', name, {'bootMs': g.boot}, self.t)
        g.fence_ops.append(op)
        g.fence_issues += 1
        if 'noEpochConfirm' in self.faults:
            self._confirm(g)
        if g.cfg.get('settleFence', True):
            ok = g.fence_fails >= g.cfg.get('fenceFailures', 0)
            self.at(self.t + 1, 2, self._settle_fence, g, op, ok)

    def _settle_fence(self, g, op, ok):
        if not g.alive or op.settled is not None:
            return
        op.settled = 'ok' if ok else 'fail'
        if ok:
            self.be.apply(op, self)
            self._confirm(g)
            return
        g.fence_fails += 1
        self._trace('fenceFail', gen=g.id, fails=g.fence_fails)
        if g.fence_fails > STORE_EPOCH_RETRY:            # P-E2-1: one write plus 3 same-name retries
            g.state, g.reason = 'disabled', None
            self._trace('disabled', gen=g.id, reason='storeRecovery')
            self._fail_requests(g)
        elif g.state != 'confirmed':
            self._issue_fence(g)

    def _confirm(self, g):
        if g.state == 'confirmed':
            return
        g.state = 'confirmed'
        g.confirmed_at = self.t
        self._trace('confirmed', gen=g.id, E=g.E)
        self._schedule_sweep(g)
        if 'epochSweepEarly' in self.faults:
            self._sweep(g)
        for key in list(g.keys):
            self._pump(g, key)

    def _schedule_sweep(self, g):
        if g.sweep_at is None:
            g.sweep_at = max(g.boot + T_LATE, self.t)
            self.at(g.sweep_at, 3, self._sweep, g)

    def _sweep(self, g):
        if not g.alive:
            return
        eps = self.epochs()
        victims = []
        if eps:
            M = max(e for e, _, _ in eps)
            if 'epochSweepByBootMs' in self.faults:
                victims = sorted(n for _, n, b in eps if b is not None and b < self.t - T_LATE)
            else:
                victims = sorted(n for e, n, _ in eps if e < M)
            for n in victims:
                op = self.be.issue(g.id, 'remove', n, None, self.t)
                self.at(self.t + 1, 2, self._settle_remove, g, op)
        self._trace('sweep', gen=g.id, removed=victims)
        if g.state == 'gateBlocked':
            self.at(self.t + 1, 3, self._attempt, g, True)

    def _settle_remove(self, g, op):
        if not g.alive or op.settled is not None:
            return
        ok = not g.cfg.get('removeFails', False)
        op.settled = 'ok' if ok else 'fail'
        if ok:
            self.be.apply(op, self)

    # ---- recovery (D103 §3) and data
    def _ks(self, g, key):
        return g.keys.setdefault(key, {'status': 'unrecovered', 'dict': None, 'ver': None, 'seq': g.cfg.get('seqStart', 0),
                                       'queue': [], 'inflight': None, 'cp': False, 'readDict': None})

    def _pump(self, g, key):
        if not g.alive or g.state != 'confirmed':
            return
        ks = self._ks(g, key)
        if ks['status'] == 'unrecovered':
            if ks['queue']:
                ks['status'] = 'recovering'
                self.at(self.t + 1, 3, self._read, g, key)
            return
        if ks['status'] != 'ready':
            return
        while ks['queue'] and ks['inflight'] is None:
            kind, obj = ks['queue'].pop(0)
            if kind == 'snap':
                self._snapshot(g, key, obj)
            else:
                self._issue_data(g, key, obj)

    def _read(self, g, key):
        if not g.alive:
            return
        net, addr = self.sites[key]
        ks = self._ks(g, key)
        if 'singleItem' in self.faults:
            v = self.be.items.get(single_name(net, addr))
            r = {'failed': False, 'dict': dict(v['dict']) if v else {}, 'version': None}
        else:
            r = lookup(self.be.items, net, addr)
        if r['failed']:
            ks['status'] = 'failed'
            self._trace('recoveryFailed', gen=g.id, key=key)
            return
        ks['readDict'] = dict(r['dict'])
        self._trace('read', gen=g.id, key=key, version=list(r['version']) if r.get('version') else None, dict=sorted(r['dict'].items()))
        if 'noCheckpoint' in self.faults:
            ks.update(status='ready', dict=dict(r['dict']), ver=r.get('version'), cp=False)
            self._pump(g, key)
            return
        self.at(self.t + 2, 3, self._checkpoint, g, key)

    def _alloc(self, ks):
        s = ks['seq']
        if s > SEQ_MAX:
            raise ValueError('seq exhausted')
        ks['seq'] = s + 1
        return s

    def _name(self, g, key, seq):
        net, addr = self.sites[key]
        if 'singleItem' in self.faults:
            return single_name(net, addr)
        return record_name(net, addr, g.E, seq)

    def _issue_record(self, g, name, value):
        if name in self.issued_records:
            self._violate('recordNameReused', name=name)
        self.issued_records.add(name)
        self.record_writers.append({'gen': g.id, 'E': g.E, 't': self.t, 'name': name})
        return self.be.issue(g.id, 'set', name, value, self.t)

    def _checkpoint(self, g, key):
        if not g.alive:
            return
        ks = self._ks(g, key)
        seq = self._alloc(ks)
        op = self._issue_record(g, self._name(g, key, seq), record_value(g.E, seq, ks['readDict']))
        self._trace('checkpointIssue', gen=g.id, key=key, version=[g.E, seq])
        if g.cfg.get('settleRecords', True):
            self.at(self.t + 1, 2, self._settle_cp, g, key, op, (g.E, seq))

    def _settle_cp(self, g, key, op, ver):
        if not g.alive or op.settled is not None:
            return
        op.settled = 'ok'
        self.be.apply(op, self)
        ks = self._ks(g, key)
        ks.update(status='ready', dict=dict(ks['readDict']), ver=ver, cp=True)
        self._trace('ready', gen=g.id, key=key, version=list(ver))
        self._sweep_records(g, key, ver)
        self._pump(g, key)

    def _issue_data(self, g, key, msg):
        ks = self._ks(g, key)
        new = dict(ks['dict'])
        if msg['v'] is None:
            new.pop(msg['k'], None)
        else:
            new[msg['k']] = msg['v']
        seq = self._alloc(ks)
        op = self._issue_record(g, self._name(g, key, seq), record_value(g.E, seq, new))
        ks['inflight'] = (op, msg, new, (g.E, seq))
        self._trace('dataIssue', gen=g.id, key=key, id=msg['id'], version=[g.E, seq])
        self.at(self.t + STORE_WRITE_DEADLINE, 3, self._timeout, g, op, msg)
        if g.cfg.get('settleRecords', True):
            self.at(self.t + 1, 2, self._settle_data, g, key, op, True)

    def _settle_data(self, g, key, op, ok):
        if not g.alive or op.settled is not None:
            return
        op.settled = 'ok' if ok else 'fail'
        ks = self._ks(g, key)
        _, msg, new, ver = ks['inflight']
        if ok:
            self.be.apply(op, self)
            ks['dict'], ks['ver'] = new, ver
            if msg['replies'] == 0:
                self._reply(msg, None)
            self._sweep_records(g, key, ver)
        else:
            if msg['replies'] == 0:
                self._reply(msg, -32603, 'transport')
            net, addr = self.sites[key]
            if 'singleItem' in self.faults:
                v = self.be.items.get(single_name(net, addr))
                ks['dict'] = dict(v['dict']) if v else {}
            else:
                r = lookup(self.be.items, net, addr)
                ks['dict'], ks['ver'] = dict(r.get('dict', {})), r.get('version')
            if 'reuseSeq' in self.faults:
                ks['seq'] -= 1
        ks['inflight'] = None
        self._pump(g, key)

    def _timeout(self, g, op, msg):
        if g.alive and op.settled is None and msg['replies'] == 0:
            self._reply(msg, -32603, 'storeTimeout')

    def _sweep_records(self, g, key, ver):
        """After a record succeeds, best-effort remove of the key's lower records (browser.md:542, 713)."""
        if 'singleItem' in self.faults:
            return
        net, addr = self.sites[key]
        for n in sorted(self.be.items):
            v = parse_record_name(n, net, addr)
            if v and v < ver:
                op = self.be.issue(g.id, 'remove', n, None, self.t)
                self.at(self.t + 1, 2, self._settle_remove, g, op)

    def _snapshot(self, g, key, fr):
        if not fr['alive']:
            return
        ks = self._ks(g, key)
        self.snapshots.append({'t': self.t, 'gen': g.id, 'frame': fr['id'], 'tab': fr.get('tab'), 'key': key,
                               'dict': dict(ks['dict']), 'afterCheckpoint': ks['cp']})

    def _reply(self, msg, code, reason=None, detail=None):
        msg['replies'] += 1
        g = self.gens[msg['gen']]
        self.replies.append({'t': self.t, 'id': msg['id'], 'gen': msg['gen'], 'emitterAlive': g.alive,
                             'code': code, 'reason': reason, 'detail': detail})

    def _fail_requests(self, g):
        for key, ks in g.keys.items():
            for kind, obj in ks['queue']:
                if kind == 'set':
                    self._reply(obj, -32603, 'storeRecovery', g.reason)
            ks['queue'] = []

    # ---- preload of a generation that already exists at t0 (BR22a)
    def preload_gen(self, gid, boot, E, cfg, key, dict_, ver, seq_next, tabs):
        g = Gen(gid, boot, cfg)
        g.E, g.state, g.confirmed_at = E, 'confirmed', boot
        self.gens[gid] = g
        ks = self._ks(g, key)
        ks.update(status='ready', dict=dict(dict_), ver=ver, seq=seq_next, cp=True)
        self.record_writers.append({'gen': gid, 'E': E, 't': boot, 'name': '(preloaded)'})
        for tab in tabs:
            self.tabs[tab] = {'id': '%s@g%d#0' % (tab, gid), 'gen': gid, 'alive': True, 'tab': tab}

    # ---- loading a schedule
    PRIO = {'place': 0, 'death': 1, 'settle': 2, 'boot': 3, 'click': 4, 'set': 4, 'siteCheck': 4, 'probe': 4}

    def load(self, events):
        for e in events:
            d = e['do']
            p = self.PRIO[d]
            if d == 'boot':
                self.at(e['t'], p, self.ev_boot, e['gen'], e.get('cfg'))
            elif d == 'death':
                self.at(e['t'], p, self.ev_death, e['gen'])
            elif d == 'place':
                self.at(e['t'], p, self.ev_place, e['gen'], e['cat'], e['action'])
            elif d == 'settle':
                self.at(e['t'], p, self.ev_settle, e['gen'], e['cat'], e['ok'])
            elif d == 'click':
                self.at(e['t'], p, self.ev_click, e['tab'], e.get('key', 'SA'))
            elif d == 'set':
                self.at(e['t'], p, self.ev_set, e['id'], e['k'], e['v'], e.get('tab'), e.get('gen'), e.get('key', 'SA'))
            elif d == 'siteCheck':
                self.at(e['t'], p, self.ev_site_check, e['label'], e['gen'], e['sitesLive'], e.get('rSites', 0))
            elif d == 'probe':
                self.at(e['t'], p, self.ev_probe, e['label'])
            else:
                raise ValueError('unknown event %s' % d)

    # ---- summaries
    def gen_summary(self):
        return {g.id: {'E': g.E, 'EN': g.EN, 'state': g.state, 'reason': g.reason, 'confirmedAt': g.confirmed_at,
                       'fenceIssues': g.fence_issues} for g in self.gens.values()}

    def final_dict(self, key='SA'):
        net, addr = self.sites[key]
        if 'singleItem' in self.faults:
            v = self.be.items.get(single_name(net, addr))
            return dict(v['dict']) if v else {}
        return lookup(self.be.items, net, addr).get('dict')
