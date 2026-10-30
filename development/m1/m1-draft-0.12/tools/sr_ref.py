"""SR-model for M1 draft 0.12: C25 repair (physical epoch-namespace accounting) on top of
the 0.11 C24 repair. REFERENCE / SPECIFICATION FIXTURE TOOLING ONLY. NOT executed by the author.

C25 (coordination/review-001/REVIEW-0.11.md). browser.md:509-510 lists ALL 'pocol:epoch:*'
items with getKeys; EN is their count, and EN+1 <= 8 is checked BEFORE issuing. In 0.11,
EN was the number of CANONICAL epoch items, so F5 plus 7 LF-tailed names (8 physical names)
still let a new F6 through: 9 physical names, and no reported violation.

Repair. Two counts are kept apart:
  1. RAW namespace count: every str name that startswith 'pocol:epoch:', canonical or not.
     It drives the EN gate (before any set), the name-budget invariant and the
     maxEpochNames statistic.
  2. CANONICAL epochs (the strict C24 parser): the maximum M, E = M+1, the generation
     window, lateGens and the sweep. Invalid names never contribute an epoch number and are
     never normalized.
Sweep: unchanged. Only canonical names below the canonical maximum are removed, under the
existing T_LATE rule. Malformed items are PRESERVED: there is no approved cleanup policy.
After the sweep the raw EN is re-read; if it is still 8, nothing is issued and the
persistent epochNames failure with action 25 follows (existing rule).
Invariant: raw count > 8 is a violation when it INCREASES (an overfull initial state is
reported, never made worse).

Unchanged: clock, scheduler, retry caps, checkpoints, seq, disk/site adapters, the
invalid-bootMs model choice (P-C24-3), canonical name ranges, owner policy. No E4-E7.

Mechanics: the 0.11 module file is loaded read-only (it in turn loads 0.10 read-only and
applies C24). Methods of its World class are then rebound in memory. Nothing older is written.
"""

import importlib.util
import json
from pathlib import Path

_HERE = Path(__file__).resolve().parent
_ROOT = _HERE.parent.parent
SOURCE_011 = _ROOT / 'm1-draft-0.11' / 'tools' / 'sr_ref.py'


def _load(path, name):
    spec = importlib.util.spec_from_file_location(name, str(path))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def load_pristine_011():
    """An unpatched copy of 0.11 (C24 repaired, C25 present), used to reproduce C25."""
    return _load(SOURCE_011, 'sr_ref_011_pristine')


_m011 = _load(SOURCE_011, 'sr_ref_011_impl')
B = _m011.BASE_MODULE                     # the 0.10 module object, with the C24 helpers installed
EPOCH_PREFIX = B.EPOCH_PREFIX
EPOCH_NAMES_MAX = B.EPOCH_NAMES_MAX
GEN_WINDOW_MAX = B.GEN_WINDOW_MAX
T_LATE = B.T_LATE


def raw_epoch_names(self):
    """Every name in the epoch namespace, canonical or malformed (browser.md:509)."""
    return [n for n in self.be.items if type(n) is str and n.startswith(EPOCH_PREFIX)]


_orig_init = B.World.__init__
_orig_gen_summary = B.World.gen_summary


def _init(self, *a, **kw):
    _orig_init(self, *a, **kw)
    self.reports = []
    self._reported = set()
    self.initial_raw = len(raw_epoch_names(self))
    self._prev_raw = self.initial_raw
    self.max_names = max(self.max_names, self.initial_raw)
    if self.initial_raw > EPOCH_NAMES_MAX:
        self._report('initialNamespaceOverfull', count=self.initial_raw)


def _report(self, kind, **kw):
    if kind in self._reported:
        return
    self._reported.add(kind)
    rec = {'t': self.t, 'kind': kind}
    rec.update(kw)
    self.reports.append(rec)


def _attempt_raw(self, g, second):
    """0.10 _attempt with EN = RAW namespace count (gate) and M = canonical maximum."""
    if not g.alive or g.state in ('fencing', 'confirmed', 'disabled'):
        return
    eps = self.epochs()                                   # canonical (strict C24 parser)
    raw = raw_epoch_names(self)
    EN = len(raw)
    M = max((e for e, _, _ in eps), default=0)
    gated = not ({'epochNoGate', 'epochNonceNames'} & self.faults)
    if gated and EN + 1 > EPOCH_NAMES_MAX:
        if second:
            g.state, g.reason = 'disabled', 'epochNames'
            self.stats['action25'] += 1
            self._trace('disabled', gen=g.id, reason='epochNames', EN=EN, ENcanonical=len(eps))
            self._fail_requests(g)
            return
        self.stats['epochGateBlocks'] += 1
        g.state = 'gateBlocked'
        self._trace('gateBlocked', gen=g.id, EN=EN, ENcanonical=len(eps))
        self._schedule_sweep(g)
        return
    window = [b for _, _, b in eps if b is not None and b >= self.t - T_LATE]
    if len(window) >= GEN_WINDOW_MAX:
        g.state = 'windowWait'
        until = min(window) + T_LATE + 1
        self._trace('windowWait', gen=g.id, count=len(window), until=until)
        self.at(until, 3, self._attempt, g, second)
        return
    g.E, g.EN, g.ENraw = M + 1, len(eps), EN               # EN (summary) = canonical count, as in 0.10/0.11; ENraw = gate count
    g.state = 'fencing'
    self._trace('fenceIssue', gen=g.id, E=g.E, EN=g.EN, ENraw=EN)
    self._issue_fence(g)


def _observe_raw(self):
    """0.10 _observe with the name budget measured on the RAW namespace."""
    eps = self.epochs()
    raw = len(raw_epoch_names(self))
    self.max_names = max(self.max_names, raw)
    if raw > EPOCH_NAMES_MAX:
        if raw > self._prev_raw:
            self._violate('epochNamesOverMax', count=raw, previous=self._prev_raw)
        else:
            self._report('namespaceOverfullNotWorsened', count=raw)
    self._prev_raw = raw
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
    if nitems > B.STORE_RECORDS_MAX or nbytes > B.STORE_DISK_HARD:
        self._violate('fakeBackendBoundExceeded', items=nitems, bytes=nbytes)
    if 'singleItem' not in self.faults:
        for k, (net, addr) in self.sites.items():
            vers = [B.parse_record_name(n, net, addr) for n in self.be.items]
            vers = [v for v in vers if v]
            cur = max(vers) if vers else None
            ever = self.ever_max.get(k)
            if cur is not None and (ever is None or cur > ever):
                self.ever_max[k] = cur
            elif ever is not None and (cur is None or cur < ever):
                self._violate('greatestRecordLost', key=k, ever=list(ever), now=list(cur) if cur else None)


def _gen_summary(self):
    out = _orig_gen_summary(self)
    for g in self.gens.values():
        out[g.id]['ENraw'] = getattr(g, 'ENraw', None)
    return out


B.World.__init__ = _init
B.World._report = _report
B.World.raw_epoch_names = raw_epoch_names
B.World._attempt = _attempt_raw
B.World._observe = _observe_raw
B.World.gen_summary = _gen_summary

# Re-export the whole repaired surface (0.11 helpers + World), then the C25 additions.
for _k in dir(_m011):
    if not _k.startswith('__') and _k not in globals():
        globals()[_k] = getattr(_m011, _k)
World = B.World
BASE_MODULE = B
load_pristine_010 = _m011.load_pristine_010


def self_check():
    sc = dict(_m011.self_check())
    sc.update({
        'attemptUsesRawNamespace': B.World._attempt is _attempt_raw,
        'observeUsesRawNamespace': B.World._observe is _observe_raw,
        'initRecordsInitialRaw': B.World.__init__ is _init,
        'sweepUnchangedCanonicalOnly': B.World._sweep.__qualname__ == 'World._sweep',
        'source011': str(SOURCE_011),
    })
    return sc
