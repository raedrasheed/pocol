"""Reference-only runner for M1 draft 0.16 (rows E6 and E7). Python standard library only.
NOT executed by the author.

Usage, from D:\\PoCol-Development:
  coordination\\runtime\\python311\\python.exe m1-draft-0.16\\tools\\run_checks_016.py
Writes only m1-draft-0.16/results/run-results-0.16.json and exits 1 on any FAIL.

Steps:
  1. Read-only re-run of the accepted 0.15 suite. run_checks_015.py is loaded as a module and
     the body of its main() (lines 206-263) is replayed here step by step, WITHOUT calling
     main() and without its result-file write (lines 264-282). The 0.15 safe load hook patches
     every nested runner (0.13 -> 0.12 -> 0.11 -> 0.10) as before. Old result files are hashed
     before and after and must be unchanged.
  2. E6: constants, DiskLedger units, AdminDelete/AdminSlots/DeleteOp goldens (BR24 x1-x12,
     x12b, BR22f-f5, BR23 d5), controls, x9 after-death placements.
  3. E7: SiteLedger goldens (BR23 d1-d15c, BR24 x6 sites), generated cases d2/d6, property
     enumerations d4 (256) and d13 (729), controls, assumption-violated cases, d3 (0.12 World)
     and d7 (0.13 codec).
  4. Coverage: every historical variant must have an executed check or an explicit partial gap.

Safe logger: record(label, core_status, /, **diag) and check(label, condition, /, **diag).
Label, core status and condition are positional-only; reserved diagnostic keys are renamed
(name->diagName, check->diagCheck, status->proposalStatus, ok->diagOk, label->diagLabel,
condition->diagCondition) and the core fields are written last.
"""

import ast
import copy
import hashlib
import importlib.util
import itertools
import json
import platform
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
PKG, ROOT = HERE.parent, HERE.parent.parent
RES = PKG / 'results'
OUT = RES / 'run-results-0.16.json'
CORE_STATUS = ('pass', 'recorded', 'FAIL')
RENAMED = {'name': 'diagName', 'check': 'diagCheck', 'status': 'proposalStatus', 'ok': 'diagOk',
           'label': 'diagLabel', 'condition': 'diagCondition'}
ACCEPTED_015 = {'pass': 365, 'recorded': 118, 'FAIL': 0}          # coordination/task-014.md:3
OLD_RESULTS = ['m1-draft-0.13/results/run-results-0.13.json', 'm1-draft-0.14/results/run-results-0.14.json',
               'm1-draft-0.15/results/run-results-0.15.json']
INPUTS = ['coordination/task-014.md', 'reference/validation.md', 'reference/browser.md', 'reference/threat.md',
          'm1-draft-0.12/tools/sr_ref.py', 'm1-draft-0.13/tools/fmt2_codec.py', 'm1-draft-0.13/tools/recovery_ref.py',
          'm1-draft-0.15/tools/run_checks_015.py',
          'm1-draft-0.16/tools/sites_ref.py', 'm1-draft-0.16/tools/admin_ref.py', 'm1-draft-0.16/tools/run_checks_016.py',
          'm1-draft-0.16/vectors/e6e7-ledger.json', 'm1-draft-0.16/vectors/e6-admin.json',
          'm1-draft-0.16/vectors/e6e7-adapters.json', 'm1-draft-0.16/vectors/e7-assumption-violations.json']

results = []


# ------------------------------------------------------------------ the safe boundary

def make_entry(label, core_status, diag):
    if core_status not in CORE_STATUS:
        raise ValueError('core status must be one of %s, got %r' % (CORE_STATUS, core_status))
    entry = {}
    for k, v in diag.items():
        entry[RENAMED.get(k, k)] = v
    entry['check'] = label
    entry['status'] = core_status            # core schema written last
    return entry


def make_safe(sink):
    def record(label, core_status, /, **diag):
        sink.append(make_entry(label, core_status, diag))

    def check(label, condition, /, **diag):
        record(label, 'pass' if condition else 'FAIL', **diag)
        return condition
    return record, check


record, check = make_safe(results)


def gap(label, /, **diag):
    """An explicit, named partial gap: never green, never silent."""
    record(label, 'recorded', partialGap=True, **diag)


def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest() if p.exists() else None


def load_module(rel, modname):
    spec = importlib.util.spec_from_file_location(modname, str(ROOT / rel))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def load_json(rel):
    return json.loads((ROOT / rel).read_text(encoding='utf-8'))


def provenance(cid, src):
    if not src:
        return
    lines = (ROOT / src['file']).read_text(encoding='utf-8').splitlines()
    a, b = src['lines']
    text = '\n'.join(lines[a - 1:b])
    missing = [lit for lit in src.get('literals', []) if lit not in text]
    check(cid + '.provenance', not missing, file=src['file'], lines=src['lines'], missingLiterals=missing)


# ------------------------------------------------------------------ step 0: inputs and boundary

def inputs():
    for rel in INPUTS:
        p = ROOT / rel
        record('input.sha256 ' + rel, 'recorded', exists=p.exists(), sha256=sha(p))


def boundary():
    sink = []
    rec, chk = make_safe(sink)
    chk('lbl', True, name='n', status='s', check='c', ok='o', label='l', condition='x', other=1)
    rec('lbl2', 'recorded', name='n2', status='FAIL', check='c2')
    a, b = sink
    check('boundary.reservedKeysRenamed', a['check'] == 'lbl' and a['status'] == 'pass' and a['diagName'] == 'n'
          and a['proposalStatus'] == 's' and a['diagCheck'] == 'c' and a['diagOk'] == 'o' and a['diagLabel'] == 'l'
          and a['diagCondition'] == 'x' and a['other'] == 1, entry=a)
    check('boundary.recordDiagnosticCannotChangeEnum', b['status'] == 'recorded' and b['proposalStatus'] == 'FAIL', entry=b)
    try:
        rec('bad', 'open')
        rejected = False
    except ValueError:
        rejected = True
    check('boundary.nonCoreStatusRejected', rejected)
    check('boundary.positionalOnly', record.__code__.co_posonlyargcount == 2 and check.__code__.co_posonlyargcount == 2
          and gap.__code__.co_posonlyargcount == 1)
    calls = []
    for node in ast.walk(ast.parse(Path(__file__).read_text(encoding='utf-8'))):
        if isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute) and node.func.attr == 'main':
            calls.append(node.lineno)
    check('boundary.noOldMainCalled', not calls, attributeMainCallsAtLines=calls)


# ------------------------------------------------------------------ step 1: read-only 0.15 re-run

def replay_015(R15):
    """run_checks_015.main() lines 206-263, replayed against the loaded 0.15 module; no file write."""
    R15.input_hashes()
    R15.scan_callers()
    p14 = None
    try:
        p14 = R15.reproduce_pristine()
        R15.boundary_tests()
    except Exception as e:                                                   # record, never hide
        R15.own_check('step.completed preamble', False, exception='%s: %s' % (type(e).__name__, e))
    R13 = None
    with R15.safe_runner_hook():
        try:
            spec = importlib.util.spec_from_file_location('run_checks_013_rerun', str(ROOT / R15.RUNNERS[0]))
            R13 = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(R13)                                    # patched on load; main() not called
        except Exception as e:                                               # record, never hide
            R15.own_check('step.completed load_r13', False, exception='%s: %s' % (type(e).__name__, e))
        if R13 is not None:
            mine = [(rec, chk) for _, m, rec, chk in R15.PATCHED if m is R13]
            R15.own_check('wiring.r13GlobalsPatched', bool(mine) and R13.e4_checks.__globals__['check'] is mine[0][1]
                          and R13.e4_checks.__globals__['record'] is mine[0][0] and R13.e5_checks.__globals__['check'] is mine[0][1])
            for name in ('input_hashes', 'wiring', 'e4_checks', 'e5_checks', 'inherited_012'):
                try:
                    getattr(R13, name)()
                except Exception as e:                                       # record, never hide
                    R13.check('step.completed ' + name, False, exception='%s: %s' % (type(e).__name__, e))
    if R13 is not None:
        names = sorted(n for n, _, _, _ in R15.PATCHED)
        R15.own_record('wiring.patchedRunners', 'recorded', modules=names)
        R15.own_check('wiring.nestedRunnersPatched', all(any(n.startswith(p) for n in names) for p in
                                                         ('run_checks_013', 'run_checks_012', 'run_checks_011', 'run_checks_010')), modules=names)
        rerun = R13.results
        R15.own_check('schema.coreStatusEnumOnly', all(r.get('status') in R15.CORE_STATUS for r in rerun),
                      offending=[r.get('check') for r in rerun if r.get('status') not in R15.CORE_STATUS][:10])
        aborted = [r['check'] for r in rerun if 'step.completed' in r['check']]
        R15.own_check('schema.noStepAborted', not aborted, aborted=aborted)
        have = {r['check']: r['status'] for r in rerun}
        u = json.loads((ROOT / 'm1-draft-0.13' / 'vectors' / 'e4-units.json').read_text(encoding='utf-8'))
        s = json.loads((ROOT / 'm1-draft-0.13' / 'vectors' / 'e5-schedules.json').read_text(encoding='utf-8'))
        if p14 is None:
            p14 = R15.load_plain('m1-draft-0.14/tools/run_checks_014.py', 'run_checks_014_pristine')
        e4_want, e5_want = p14.expected_e4_names(u), p14.expected_e5_names(s)
        R15.own_check('coverage.e4AllExpectedChecksPresent', all(n in have for n in e4_want), expected=len(e4_want),
                      missing=[n for n in e4_want if n not in have])
        R15.own_check('coverage.e5AllExpectedChecksPresent', all(n in have for n in e5_want), expected=len(e5_want),
                      missing=[n for n in e5_want if n not in have])
        R15.own_check('coverage.previouslySkippedNineExecuted', all(n in have for n in R15.PREVIOUSLY_SKIPPED),
                      statuses={n: have.get(n) for n in R15.PREVIOUSLY_SKIPPED})
        mr = [r for r in rerun if r['check'] == 'e4.maxRecord']
        R15.own_check('coverage.maxRecordDiagnosticRetained', len(mr) == 1 and 'diagName' in mr[0], entry=mr[0] if mr else None)
        inh = [r['check'] for r in rerun if r['check'].startswith('inherited012.')]
        R15.own_check('coverage.inheritedChainPresent', 'inherited012.summary' in inh and any(c.startswith('inherited012.inherited011.') for c in inh)
                      and any('inherited010.' in c for c in inh), entries=len(inh))
        R15.own_record('coverage.counts', 'recorded', e4Expected=len(e4_want), e5Expected=len(e5_want), rerunEntries=len(rerun),
                       inherited012Entries=len(inh), byStatus={k: sum(r['status'] == k for r in rerun) for k in R15.CORE_STATUS})
        for r in rerun:
            r = dict(r)
            r['check'] = 'rerun013.' + r['check']
            R15.results.append(r)


def rerun_015():
    before = {rel: sha(ROOT / rel) for rel in OLD_RESULTS}
    try:
        R15 = load_module('m1-draft-0.15/tools/run_checks_015.py', 'run_checks_015_rerun016')
    except Exception as e:                                                   # record, never hide
        check('rerun015.load', False, exception='%s: %s' % (type(e).__name__, e))
        return
    try:
        replay_015(R15)
        replayed = True
    except Exception as e:                                                   # record, never hide
        check('rerun015.replayCompleted', False, exception='%s: %s' % (type(e).__name__, e))
        replayed = False
    rr = R15.results
    by = {k: sum(r.get('status') == k for r in rr) for k in CORE_STATUS}
    if replayed:
        check('rerun015.replayCompleted', True, entries=len(rr))
    check('rerun015.statusEnumOnly', all(r.get('status') in CORE_STATUS for r in rr))
    check('rerun015.countsMatchAccepted', by == ACCEPTED_015, counts=by, accepted=ACCEPTED_015)
    for r in rr:
        r = dict(r)
        r['check'] = 'rerun015.' + r['check']
        results.append(r)
    after = {rel: sha(ROOT / rel) for rel in OLD_RESULTS}
    check('rerun015.oldResultFilesUnchanged', before == after, before=before, after=after)


# ------------------------------------------------------------------ E7 ledger with event audits

sys.path.insert(0, str(HERE))
import sites_ref as S                 # noqa: E402  (0.16; no import of older modules)


class AuditedLedger(S.SiteLedger):
    """SiteLedger plus independent event invariants (annex E6-E7 section 6)."""

    def __init__(self, *a, audit=True, **kw):
        self.audit_on = audit
        self.audit = []
        self.dead_ops = set()
        self.seat_count = 0
        self.seat_ids = set()
        super().__init__(*a, **kw)

    def _fail(self, kind, **kw):
        r = {'t': self.t, 'kind': kind}
        r.update(kw)
        self.audit.append(r)

    def ev_op(self, e):
        if self.audit_on and self.alive and e['id'].startswith('reap-'):
            key = e['key']
            its = [n for n, i in self.items.items() if i['key'] == key]
            ok = (len(its) == 1 and self.items[its[0]]['kind'] == 'tomb' and key not in self.sessions
                  and not any(t['live'] and t['key'] == key for t in self.tickets)
                  and all(i['E'] in self.dropped for i in self.items.values() if i['kind'] == 'epoch' and i['E'] < (self.live_E or 0)))
            if not ok:
                self._fail('reaperIneligible', key=key)
        n = len(self.tickets)
        super().ev_op(e)
        if len(self.tickets) > n and self.tickets[-1]['seat']:
            self.seat_count += 1
            self.seat_ids.add(e['id'])

    def ev_death(self, e):
        self.dead_ops |= {o for o, op in self.ops.items() if op['state'] == 'pending' and op['kind'] in ('data', 'checkpoint')}
        super().ev_death(e)
        self.seat_count, self.seat_ids = 0, set()

    def ev_refreshComplete(self, e):
        r = self.refreshing
        before = set(self.dropped)
        super().ev_refreshComplete(e)
        if r is None:
            return
        for E in self.dropped - before:
            boot = self.items.get('epoch#%d' % E, {}).get('bootMs')
            if boot is None or boot + S.T_LATE > r['t']:
                self._fail('windowDroppedEarly', E=E, issuedAt=r['t'])
        for oid in sorted(self.seat_ids):
            tk = self.ops[oid]['ticket']
            if tk['settleSeq'] is not None and tk['settleSeq'] < r['seq']:
                self.seat_ids.discard(oid)
                self.seat_count -= 1

    def _check(self):
        super()._check()
        if not self.audit_on or not self.alive:
            return
        if self.seat_count != self.r_sites():
            self._fail('seatsNotIssuedMinusRetired', counted=self.seat_count, rSites=self.r_sites())
        keys = self.disk_keys()
        live_keys = {t['key'] for t in self.live_tickets()}
        dead_keys = {self.ops[o]['key'] for o in self.dead_ops}
        case3 = keys - self.sites_live - live_keys
        if case3 - dead_keys:
            self._fail('siteLemmaStrayKey', keys=sorted(case3 - dead_keys))
        if len(case3) > self.late_sites():
            self._fail('siteLemmaLateExceeded', count=len(case3), lateSites=self.late_sites())
        if self._in_use() > self.measured['inUse'] + self.r_bytes() + S.LATE_RESERVE:
            self._fail('bytesNotConservative', inUse=self._in_use())
        if self._names() > self.measured['names'] + self.r_names() + S.LATE_NAMES + S.EPOCH_NAMES_MAX:
            self._fail('namesNotConservative', names=self._names())


def build_ledger(st, faults=(), audit=True):
    k = st.get('keys', {})
    keys = ['%s%d' % (k['prefix'], i) for i in range(1, k['count'] + 1)] if 'count' in k else []
    keys += list(k.get('list', [])) + list(st.get('extra', []))
    return AuditedLedger(keys_on_disk=keys, in_use=st.get('inUse', 0), names=st.get('names'), epochs=st.get('epochs', ()),
                         live_E=st.get('liveE'), dropped_epochs=st.get('dropped', ()), faults=faults, auto_reaper=st.get('autoReaper', False),
                         item_bytes=st.get('itemBytes', 1), sessions=st.get('sessions', ()), tomb_keys=st.get('tombKeys', ()),
                         refresh_delay=st.get('refreshDelay'), audit=audit)


def run_ledger(case, events, faults=(), audit=True):
    L = build_ledger(case['setup'], faults, audit)
    L.load(copy.deepcopy(events))
    L.run(max(e['t'] for e in events) + 1)
    return L


def ledger_cells(L, exp):
    out = []
    dec, seen = {}, {}
    for d in L.decisions:
        dec.setdefault(d['id'], d['decision'])
        seen[d['id']] = seen.get(d['id'], 0) + 1
    for oid, want in exp.get('decisions', {}).items():
        out.append(('decision.' + oid, want, dec.get(oid, 'absent')))
    reps = {r['label']: r['snapshot'] for r in L.reports}
    for lbl, fields in exp.get('probes', {}).items():
        snap = reps.get(lbl)
        for f, v in fields.items():
            out.append(('probe.%s.%s' % (lbl, f), v, None if snap is None else snap.get(f)))
    snap = L.snapshot()
    for f, v in exp.get('final', {}).items():
        if f == 'violations':
            act = L.violations
        elif f == 'violationKinds':
            act, v = sorted({x['kind'] for x in L.violations}), sorted(v)
        elif f == 'maxDiskKeys':
            act = L.max_disk_keys
        elif f == 'tombReaps':
            act = L.stats['tombReaps']
        elif f == 'seatsIssued':
            act = sum(1 for r in L.trace if r['ev'] == 'issued' and r.get('seat'))
        elif f == 'firstReaperRemoveAt':
            act = next((r['t'] for r in L.trace if r['ev'] == 'reaperRemove'), None)
        else:
            act = snap.get(f)
        out.append(('final.' + f, v, act))
    out.append(('responsesOnceOnly', [], sorted(o for o, n in seen.items() if n > 1)))
    return out


def report_cells(prefix, cells):
    for cell, want, act in cells:
        check('%s.%s' % (prefix, cell), want == act, expected=want, actual=act)


def mismatches(cells):
    return [{'cell': c, 'expected': w, 'actual': a} for c, w, a in cells if w != a]


CONTROL_WITNESS = {
    ('noLateSites', 'BR23-d12'): ['decision.B1', 'final.violations'],
    ('dropPinned', 'BR23-d15c'): ['probe.at30050.sitesLive', 'decision.X2', 'final.maxDiskKeys'],
    ('seatOnCounted', 'BR23-d15'): ['decision.C'],
    ('epochReserveOmitted', 'BR23-d11'): ['decision.o2'],
    ('epochReserveOmitted', 'BR23-d11b'): ['decision.o1'],
    ('diskNoReserve', 'BR23-d8'): ['decision.o2'],
    ('diskNoReserve', 'BR23-d9'): ['decision.B'],
    ('diskNoReserve', 'BR23-d11'): ['decision.o2'],
    ('diskNoReserve', 'BR23-d10'): ['decision.Y'],
    ('noDiskGate', 'BR23-d2'): ['accepted'],
    ('noDiskGate', 'BR23-d6'): ['accepted'],
    ('deleteUnordered', 'BR24-x1'): ['final.dict'],
    ('adminEarlyRelease', 'BR24-x9'): ['final.violations'],
    ('adminStaleIssue', 'BR24-x12'): ['final.nextGen'],
    ('adminNoCancel', 'BR24-x12'): ['final.adminStats'],
}


def control_check(fault, cid, cells, expect_violation=None, violations=()):
    mm = mismatches(cells)
    cells_hit = {m['cell'] for m in mm}
    want = CONTROL_WITNESS.get((fault, cid), [])
    viol_ok = expect_violation is None or any(v['kind'] == expect_violation for v in violations)
    check('control.%s.%s' % (fault, cid), bool(mm) and all(w in cells_hit for w in want) and viol_ok,
          mismatchingCells=mm[:10], requiredWitnessCells=want, expectViolation=expect_violation)


def ledger_suite(doc):
    by_id = {c['id']: c for c in doc['cases']}
    for case in doc['cases']:
        cid = case['id']
        provenance(cid, case.get('source'))
        events = by_id[case['sameEventsAs']]['events'] if 'sameEventsAs' in case else case['events']
        faults = tuple(case.get('faults', ()))
        L = run_ledger(case, events, faults, audit=not faults)
        report_cells(cid, ledger_cells(L, case['expect']))
        if not faults:
            check(cid + '.invariants', not L.audit, audit=L.audit[:5])
        fp = case.get('faultyPath')
        if fp:
            Lf = run_ledger(case, events, (fp['fault'],), audit=False)
            report_cells('%s.faultyPath.%s' % (cid, fp['fault']), ledger_cells(Lf, {'decisions': fp['decisions']}))
        for b in case.get('boundaryCells', []):
            gap('%s.boundary@%d' % (cid, b['t']), cell=b['cell'], coveredBy=b['coveredBy'])
        if cid == 'BR23-d1':
            # RF-E7-1: D113 scans at every accepted refresh, so the reaper may already issue at 250.
            auto = dict(case['setup'], autoReaper=True)
            La = run_ledger(dict(case, setup=auto), [e for e in events if e['do'] != 'scan'])
            first = next((r['t'] for r in La.trace if r['ev'] == 'reaperRemove'), None)
            rep = {r['label']: r['snapshot'] for r in La.reports}
            check('BR23-d1.autoReaperVariant', first == 250 and rep.get('at400', {}).get('sitesLive') == 63
                  and La.decisions[-1]['decision'] is None, finding='RF-E7-1', firstReaperRemoveAt=first,
                  literalReaperAt=300, note='same outcome at 400 and 450; only the remove issue time differs')
    gen = generated_suite(doc)
    for c in doc['controls']:
        fault, cid = c['fault'], c['case']
        if c.get('generated'):
            cells = gen[cid](fault)
            control_check(fault, cid, cells)
            continue
        case = by_id[cid]
        L = run_ledger(case, case['events'], (fault,), audit=False)
        control_check(fault, cid, ledger_cells(L, case['expect']), c.get('expectViolation'), L.violations)
    for g in doc['explicitGaps']:
        if g['status'] == 'gap':
            gap('BR22a-sweepMax.gap', item=g['item'], source=g['source'], why=g['why'])


# ------------------------------------------------------------------ generated cases and property enumerations

def d2_cells(gdoc, fault=None):
    exp = gdoc['expect']
    L = AuditedLedger(keys_on_disk=['K'], item_bytes=1442048, faults=(fault,) if fault else (), audit=not fault)
    ev = []
    for j in range(1, 171):
        ev += [{'t': 10 * j, 'do': 'op', 'id': 'j%d' % j, 'key': 'K', 'kind': 'data', 'bytes': 1442048},
               {'t': 10 * j + 1, 'do': 'settle', 'id': 'j%d' % j}, {'t': 10 * j + 2, 'do': 'refreshComplete'}]
    L.load(ev)
    L.run(1800)
    acc = [d['id'] for d in L.decisions if d['decision'] is None]
    rej = [d for d in L.decisions if d['decision'] is not None]
    cells = [('accepted', exp['accepted'], len(acc)),
             ('firstReject', exp['firstReject'], int(rej[0]['id'][1:]) if rej else None),
             ('rejectDecision', exp['rejectDecision'], rej[0]['decision'] if rej else None),
             ('maxDiskBytes', exp['maxDiskBytes'], L.max_disk_bytes),
             ('lastConfirmedPresent', True, exp['lastConfirmedPresent'] in L.items),
             ('violations', exp['violations'], L.violations)]
    if not fault:
        cells.append(('invariants', [], L.audit[:5]))
    return cells


def d6_cells(gdoc, fault=None):
    exp = gdoc['expect']
    L = AuditedLedger(keys_on_disk=['K'], names=900, item_bytes=0, faults=(fault,) if fault else (), audit=not fault)
    ev = []
    for j in range(1, 96):
        ev += [{'t': 10 * j, 'do': 'op', 'id': 'n%d' % j, 'key': 'K', 'kind': 'data'},
               {'t': 10 * j + 1, 'do': 'settle', 'id': 'n%d' % j}, {'t': 10 * j + 2, 'do': 'refreshComplete'}]
    L.load(ev)
    L.run(1000)
    acc = [d['id'] for d in L.decisions if d['decision'] is None]
    rej = [d for d in L.decisions if d['decision'] is not None]
    cells = [('accepted', exp['accepted'], len(acc)),
             ('firstReject', exp['firstReject'], int(rej[0]['id'][1:]) if rej else None),
             ('rejectDecision', exp['rejectDecision'], rej[0]['decision'] if rej else None),
             ('noOpForRejected', True, all(('K#' + d['id']) not in L.items for d in rej))]
    if not fault:
        cells.append(('invariants', [], L.audit[:5]))
    return cells


def d4_run(place):
    L = AuditedLedger(keys_on_disk=['K'], in_use=232777728, item_bytes=0)
    ev, ops = [], []
    for g in range(1, 5):
        b = 1000 * g
        ev.append({'t': b, 'do': 'boot', 'E': g})
        for i, dt in (('a', 10), ('b', 20)):
            oid = 'g%d%s' % (g, i)
            ev.append({'t': b + dt, 'do': 'op', 'id': oid, 'key': 'K', 'kind': 'data', 'bytes': 1442048})
            ops.append((oid, 1000 * (g + 1) + 100))
        ev.append({'t': b + 500, 'do': 'death'})
    ev.append({'t': 5000, 'do': 'boot', 'E': 5})
    for (oid, t), p in zip(ops, place):
        ev.append({'t': t, 'do': p, 'id': oid})
    L.load(ev)
    L.run(6000)
    return L


def d13_run(place):
    keys = ['S%d' % i for i in range(1, 51)]
    L = AuditedLedger(keys_on_disk=keys, epochs=[{'E': 6, 'bootMs': -120000}], live_E=6, dropped_epochs=[6], refresh_delay=20)
    ev = []
    gens = [(6, None, 50, 100), (7, 100, 20050, 20100), (8, 20100, 40050, 40100), (9, 40100, None, None)]
    pend = []
    for E, boot, death, nxt in gens:
        b = boot if boot is not None else 0
        if boot is not None:
            ev.append({'t': boot, 'do': 'boot', 'E': E})
        if nxt is not None:
            ev.append({'t': b + 10, 'do': 'op', 'id': 'P%da' % E, 'key': 'P%da' % E, 'kind': 'data'})
            ev.append({'t': b + 20, 'do': 'op', 'id': 'P%db' % E, 'key': 'P%db' % E, 'kind': 'checkpoint'})
            pend += [('P%da' % E, nxt), ('P%db' % E, nxt)]
            ev.append({'t': death, 'do': 'death'})
    for k in range(1, 121):
        ev.append({'t': 1000 * k, 'do': 'op', 'id': 'try%d' % k, 'key': 'N%d' % k, 'kind': 'data', 'autoSettle': 10})
    for (oid, nxt), p in zip(pend, place):
        if p == 'before':
            ev.append({'t': nxt - 1, 'do': 'apply', 'id': oid})
        elif p == 'late':
            ev.append({'t': nxt + S.T_LATE - 1, 'do': 'apply', 'id': oid})
        else:
            ev.append({'t': nxt - 1, 'do': 'drop', 'id': oid})
    L.load(ev)
    L.run(125000)
    return L


def generated_suite(doc):
    g = doc['generated']
    for cid in ('BR23-d2', 'BR23-d6', 'BR23-d4', 'BR23-d13'):
        provenance(cid, g[cid]['source'])
    report_cells('BR23-d2', d2_cells(g['BR23-d2']))
    report_cells('BR23-d6', d6_cells(g['BR23-d6']))

    e4 = g['BR23-d4']['expect']
    runs = [(p, d4_run(p)) for p in itertools.product(('apply', 'drop'), repeat=8)]
    check('BR23-d4.placements', len(runs) == e4['placements'], placements=len(runs))
    check('BR23-d4.allAdmitted', all(sum(d['decision'] is None for d in L.decisions) == e4['allAdmitted'] for _, L in runs))
    check('BR23-d4.diskWithinHardEveryInstant', all(L.max_disk_bytes <= e4['maxDiskBytesBound'] and not L.violations for _, L in runs),
          worst=max(L.max_disk_bytes for _, L in runs))
    allp = [L for p, L in runs if all(x == 'apply' for x in p)]
    check('BR23-d4.maxWhenAllApplied', len(allp) == 1 and allp[0].max_disk_bytes == e4['maxDiskBytesAllApplied'],
          actual=allp[0].max_disk_bytes if allp else None)
    check('BR23-d4.invariants', all(not L.audit for _, L in runs), firstAudit=next((L.audit[:3] for _, L in runs if L.audit), None))

    e13 = g['BR23-d13']['expect']
    runs = [(p, d13_run(p)) for p in itertools.product(('before', 'late', 'drop'), repeat=6)]
    check('BR23-d13.placements', len(runs) == e13['placements'], placements=len(runs))
    check('BR23-d13.diskKeysAtMost64', all(L.max_disk_keys <= e13['maxDiskKeysAtMost'] for _, L in runs),
          worst=max(L.max_disk_keys for _, L in runs))
    check('BR23-d13.lateSitesAtMost12', all(L.max_late_sites <= e13['maxLateSitesAtMost'] for _, L in runs),
          worst=max(L.max_late_sites for _, L in runs))
    check('BR23-d13.noViolations', all(not L.violations for _, L in runs))
    check('BR23-d13.invariants', all(not L.audit for _, L in runs), firstAudit=next((L.audit[:3] for _, L in runs if L.audit), None))
    adm = [sum(1 for d in L.decisions if d['id'].startswith('P') and d['decision'] is None) for _, L in runs]
    record('BR23-d13.witnesses', 'recorded', placementsWithLateSites12=sum(L.max_late_sites == 12 for _, L in runs),
           placementsAtExactly64=sum(L.max_disk_keys == 64 for _, L in runs),
           deadGenOpsAdmittedHistogram={str(n): adm.count(n) for n in sorted(set(adm))},
           lateRejectsWithRetry=sum(L.stats['sitesRejectsLate'] for _, L in runs),
           note='RF-E7-2: tries of the live generation fill capacity, so later dead-generation creating ops may be rejected; A15c placement = next boot + T_LATE - 1')
    gap('BR23-d13.scope', why='validation.md:611-619 gives properties, not a literal cell table; checked as properties over 729 placements')
    return {'BR23-d2': lambda f: d2_cells(g['BR23-d2'], f), 'BR23-d6': lambda f: d6_cells(g['BR23-d6'], f)}


def assumption_suite(doc):
    for case in doc['cases']:
        cid = case['id']
        provenance(cid, case['source'])
        L = run_ledger(case, case['events'], audit=False)
        cells = ledger_cells(L, case['expect'])
        ok = all(w == a for _, w, a in cells)
        check(cid + '.violationReproduced', ok, violates=case['violates'], mismatches=mismatches(cells),
              note='assumption deliberately broken; NOT evidence for the sites lemma')


# ------------------------------------------------------------------ E6 AdminDelete

def admin_events(doc, case):
    if 'events' in case:
        return case['events']
    return next(c for c in doc['cases'] if c['id'] == case['sameEventsAs'])['events']


def admin_build(A, doc, case, faults=None, events=None, E=2, items=None):
    SR12 = A.R13.SR12
    if items is None:
        items = {}
        for rid in case.get('records', []):
            r = doc['records'][rid]
            items[SR12.record_name(*r['name'])] = copy.deepcopy(r['value'])
    cfg = case.get('config', {})
    fl = list(case.get('faults', [])) if faults is None else list(faults)
    e = A.AdminEngine(case['sites'], records=items, faults=fl, get_keys_available=cfg.get('getKeysAvailable', True), E=E)
    e.load(copy.deepcopy(events if events is not None else admin_events(doc, case)))
    return e


def backend_of(A, e, key):
    SR12 = A.R13.SR12
    net, addr = e.sites[key]
    out = []
    for n, v in e.items.items():
        ver = SR12.parse_record_name(n, net, addr)
        if ver:
            out.append([ver[0], ver[1], v['tomb'], v['b64']])
    return sorted(out)


def admin_actual(A, e, field, want):
    SR12, C = A.R13.SR12, A.C
    log = [[o.n, o.purpose, o.key, list(o.version), o.issuedAt] for o in e.sets]
    if field == 'sets':
        return len(e.sets)
    if field == 'setLog':
        return log
    if field == 'setLogTail':
        return log[-1] if log else None
    if field == 'setLogContains':
        return [x for x in want if x in log]
    if field == 'held':
        return len(e.admin_slots)
    if field == 'adminFifo':
        return sum(1 for r in e.admin_fifo2 if not r['withdrawn'] and not r['done'])
    if field == 'status':
        return {k: e.keys[k]['status'] for k in want}
    if field == 'dict':
        return {k: e.keys[k]['dict'] for k in want}
    if field == 'confirmed':
        return {k: list(e.keys[k]['confirmed']) if e.keys[k]['confirmed'] is not None else None for k in want}
    if field == 'snapshots':
        return [[s['t'], s['key'], s['frame'], s['dict']] for s in e.snapshots]
    if field == 'replies':
        return [[r['id'], r['code'], r['reason'], r['sub'], r['t']] for r in e.replies]
    if field == 'banners':
        return [[b['t'], b['key'], b['frames'], b['autoReload']] for b in e.banners]
    if field == 'dops':
        return {op: {f: e.dops.get(op, {}).get(f) for f in fields} for op, fields in want.items()}
    if field == 'adminStats':
        return {k: e.admin_stats.get(k) for k in want}
    if field == 'stats':
        return {k: e.stats.get(k) for k in want}
    if field == 'backend':
        return {k: backend_of(A, e, k) for k in want}
    if field == 'nextGen':
        return {k: C.lookup_fmt2(e.items, *e.sites[k], SR12.parse_record_name).get('dict') for k in want}
    if field == 'violations':
        return e.violations
    if field == 'violationKinds':
        return sorted({v['kind'] for v in e.violations})
    if field == 'reads':
        return len(e.reads)
    if field == 'msgsReleasedOnce':
        ids = [r['id'] for r in e.replies]
        return (all(m['replied'] for m in e.msgs.values()) and len(ids) == len(set(ids))
                and not any(v['kind'] == 'duplicateReply' for v in e.violations))
    if field == 'staleDroppedByOp':
        return {op: sum(1 for r in e.trace if r['ev'] == 'adminStaleDropped' and r['op'] == op) for op in want}
    if field == 'setsAtOrAfter0':
        return sum(1 for o in e.sets if o.issuedAt >= 0)
    if field == 'tombSetsForOpAfter':
        return {op: [t, sum(1 for o in e.sets if o.purpose == 'tomb' and o.gate == op and o.issuedAt > t)] for op, (t, _) in want.items()}
    if field == 'traceHas':
        return [w for w in want if any(all(r.get(k) == v for k, v in w.items()) for r in e.trace)]
    raise ValueError('unknown expectation field %s' % field)


def admin_norm(field, want):
    return sorted(want) if field == 'violationKinds' else want


def admin_cells(A, e, spec, until):
    out = []
    for row in spec.get('rows', []):
        e.run(row['t'])
        for f, v in row.items():
            if f != 't':
                out.append(('row@%d.%s' % (row['t'], f), admin_norm(f, v), admin_actual(A, e, f, v)))
    e.run(until)
    for f, v in spec.get('final', {}).items():
        out.append(('final.' + f, admin_norm(f, v), admin_actual(A, e, f, v)))
    return out


def admin_invariants(A, e):
    bad = []
    if e.admin_stats['adminTombsUnsettledMax'] > A.R13.ADMIN_SLOTS:
        bad.append('adminTombsUnsettledMax')
    for d in e.dops.values():
        if d['resolvedAt'] is not None and d['resolvedAt'] - d['requestedAt'] > A.R13.ADMIN_BOUND:
            bad.append('adminBound ' + d['id'])
    if any(o.purpose == 'tomb' and o.ticket is None for o in e.sets):
        bad.append('tombWithoutTicket')
    last = {}
    for o in e.sets:
        if o.key in last and not o.version > last[o.key]:
            bad.append('seqNotIncreasing ' + o.key)
        last[o.key] = o.version
    if any(v['kind'] in ('duplicateReply', 'recordNameReused') for v in e.violations):
        bad.append('violation')
    return bad


def x9_after_death(A, doc, case):
    ad = case['afterDeath']
    k1_empty = k1_kept = 0
    bad = []
    witnesses = {}
    for combo in itertools.product(ad['positions'], repeat=len(ad['tombs'])):
        e = admin_build(A, doc, case)
        e.run(case['until'])
        for n, p in zip(ad['tombs'], combo):
            if p == 'beforeRead':
                e.ev_apply(n)                       # dead generation: late application only
        nxt = admin_build(A, doc, case, faults=[], E=ad['nextE'], items=copy.deepcopy(e.items),
                          events=[{'t': 0, 'do': 'snap', 'key': 'K1', 'frame': 'N1'}, {'t': 10, 'do': 'settle', 'set': 1},
                                  {'t': 20, 'do': 'snap', 'key': 'K2', 'frame': 'N2'}, {'t': 30, 'do': 'settle', 'set': 2}])
        nxt.run(100)
        for n, p in zip(ad['tombs'], combo):
            if p == 'afterCheckpoint':
                op = e.sets[n - 1]
                nxt.items[op.name] = op.value       # lands after the next generation's checkpoint
        snaps = {s['key']: s['dict'] for s in nxt.snapshots}
        look = {k: A.C.lookup_fmt2(nxt.items, *nxt.sites[k], A.R13.SR12.parse_record_name).get('dict') for k in ('K1', 'K2')}
        want_k1 = {} if 'beforeRead' in combo else {'a': '1'}
        late = sum(1 for p in combo if p != 'drop')
        ok = snaps.get('K1') == want_k1 and look['K1'] == want_k1 and snaps.get('K2') == ad['expect']['K2'] \
            and look['K2'] == ad['expect']['K2'] and late <= 2
        if not ok:
            bad.append({'combo': combo, 'snapshots': snaps, 'lookup': look})
        k1_empty += want_k1 == {}
        k1_kept += want_k1 != {}
        witnesses['/'.join(combo)] = snaps.get('K1')
    n = len(witnesses)
    check('BR24-x9.afterDeath.placements', n == ad['expect']['placements'] and k1_empty == ad['expect']['K1EmptyCount']
          and k1_kept == ad['expect']['K1KeptCount'], placements=n, K1Empty=k1_empty, K1Kept=k1_kept)
    check('BR24-x9.afterDeath.matchesRule', not bad, rule=ad['rule'], failures=bad[:4], witnesses=witnesses)


def admin_suite(A, doc):
    SR12, C, R13 = A.R13.SR12, A.C, A.R13
    for rid, r in doc['records'].items():
        check('E6-records.%s.literalBytes' % rid, C.encode_value(r['name'][2], r['name'][3], r['dict']) == r['value'], value=r['value'])
    for lbl, lit in doc['b64Literals'].items():
        d = {} if lbl == '{}' else dict([lbl.strip('{}').split(':')])
        check('E6-records.b64 ' + lbl, C.b64_encode(C.serialize_pairs(d)) == lit, literal=lit)

    u = {x['id']: x for x in doc['units']}
    for x in doc['units']:
        provenance(x['id'], x.get('source'))
    ex = u['E6-const']['expect']
    mc = ex['metaConstraint']
    tomb_name, tomb_val = SR12.record_name('nA', '0xa', 2, 2), C.encode_value(2, 2, {}, tomb=True)
    act = {'LATE_RESERVE': R13.LATE_RESERVE, 'LATE_NAMES': R13.LATE_NAMES, 'EPOCH_NAMES_MAX': R13.EPOCH_NAMES_MAX, 'META_RESERVE': R13.META_RESERVE}
    for k, v in act.items():
        check('E6-const.' + k, v == ex[k] and getattr(S, k) == ex[k], recovery_ref=v, sites_ref=getattr(S, k), expected=ex[k])
    calc = {'tombs': R13.RECORDS_MAX * ex['TOMB_BYTES_MAX'], 'unsettledTombs': (SR12.GEN_WINDOW_MAX + 1) * R13.ADMIN_SLOTS * ex['TOMB_BYTES_MAX'],
            'epochItems': R13.EPOCH_NAMES_MAX * SR12.BASE_MODULE.EPOCH_BYTES_MAX}
    calc['total'] = sum(calc.values())
    check('E6-const.metaConstraint', calc == mc and calc['total'] <= R13.META_RESERVE, derived=calc, expected=mc,
          gen_window_max=SR12.GEN_WINDOW_MAX, epoch_bytes_max=SR12.BASE_MODULE.EPOCH_BYTES_MAX)
    check('E6-const.tombEncWithinTombBytesMax', C.enc_size(tomb_name, tomb_val) <= ex['TOMB_BYTES_MAX'], enc=C.enc_size(tomb_name, tomb_val))
    eb = u['E6-bound']['expect']
    check('E6-bound.gateAndAdmin', R13.GATE_BOUND == eb['GATE_BOUND'] and R13.ADMIN_BOUND == eb['ADMIN_BOUND'],
          gate=R13.GATE_BOUND, admin=R13.ADMIN_BOUND)
    D = R13.DiskLedger
    x = u['E6-tombBytesExempt']
    L = D(in_use=x['setup']['inUse'], names=x['setup']['names'])
    bad_set = L.check(1, 'set')
    tk, bad = L.reserve(100, 'tomb')
    check('E6-tombBytesExempt.setRejected', bad_set == x['expect']['setEnc1'], actual=bad_set)
    check('E6-tombBytesExempt.tombAccepted', bad is None and {'bytes': tk['bytes'], 'names': tk['names']} == x['expect']['tomb'], ticket=tk)
    x = u['E6-tombCountsNames']
    L = D(in_use=0, names=x['setup']['names'])
    t1, b1 = L.reserve(100, 'tomb')
    t2, b2 = L.reserve(100, 'tomb')
    check('E6-tombCountsNames.first', b1 is None and {'bytes': t1['bytes'], 'names': t1['names']} == x['expect']['tomb1'], ticket=t1)
    check('E6-tombCountsNames.second', t2 is None and b2 == x['expect']['tomb2'], actual=b2)
    x = u['E6-removeZeroTicket']
    L = D()
    tr, br = L.reserve(50, 'remove')
    check('E6-removeZeroTicket.zero', br is None and {'bytes': tr['bytes'], 'names': tr['names']} == x['expect']['ticket']
          and len(L.tickets) == x['expect']['liveTickets'] and L.r_bytes == x['expect']['rBytes'] and L.r_names == x['expect']['rNames'], ticket=tr)
    x = u['E6-ticketLifecycle']['expect']
    L = D()
    tk, _ = L.reserve(1000)
    s1 = L.refresh_issue()
    L.settled(tk)
    s2 = L.refresh_issue()
    s3 = L.refresh_complete(0, 0)
    rb1 = L.r_bytes
    L.refresh_complete(0, 0)
    rb2 = L.r_bytes
    got = {'firstIssue': s1, 'issueWhileBusy': s2, 'reissuedOnComplete': s3, 'rBytesAfterFirstComplete': rb1, 'rBytesAfterSecondComplete': rb2}
    check('E6-ticketLifecycle.settleSeqSerializedRefresh', got == x, actual=got, expected=x)

    by_id = {c['id']: c for c in doc['cases']}
    for case in doc['cases']:
        cid = case['id']
        provenance(cid, case.get('source'))
        e = admin_build(A, doc, case)
        report_cells(cid, admin_cells(A, e, case, case['until']))
        if not case.get('faults'):
            inv = admin_invariants(A, e)
            check(cid + '.invariants', not inv, failures=inv)
        for fp in case.get('faultyPaths', []):
            ef = admin_build(A, doc, case, faults=[fp['fault']])
            report_cells('%s.faultyPath.%s' % (cid, fp['fault']), admin_cells(A, ef, fp, case['until']))
        if 'literalConflict' in case:
            lc = case['literalConflict']
            record('%s.literalConflict.%s' % (cid, lc['id']), 'recorded', partialGap=True, cell=lc['cell'], literal=lc['literal'],
                   model=e.admin_stats['adminStaleDropped'], modelExpected=lc['model'], why=lc['why'])
        if 'baselineWithoutFault' in case:
            eb = admin_build(A, doc, case, faults=[])
            eb.run(case['rows'][0]['t'])
            report_cells(cid + '.baselineWithoutFault', [(f, v, admin_actual(A, eb, f, v)) for f, v in case['baselineWithoutFault'].items()])
        if 'afterDeath' in case:
            x9_after_death(A, doc, case)

    for c in doc['controls']:
        fault, cid = c['fault'], c['case']
        case = by_id[cid]
        if c.get('variantCase') == cid:
            ef = admin_build(A, doc, case, faults=[fault])
            ef.run(case['until'])
            eb = admin_build(A, doc, case, faults=[])
            eb.run(case['until'])
            check('control.%s.%s' % (fault, cid), ef.stats['recoveryStaleDropped'] != eb.stats['recoveryStaleDropped']
                  and not any(o.key == 'K1' and o.purpose == 'checkpoint' and o.attempt == 2 for o in ef.sets),
                  withFault=ef.stats['recoveryStaleDropped'], without=eb.stats['recoveryStaleDropped'],
                  note='guard must absorb the fault; the observable must still change')
            continue
        ef = admin_build(A, doc, case, faults=[fault])
        control_check(fault, cid, admin_cells(A, ef, case, case['until']), c.get('expectViolation'), ef.violations)


def adapter_suite(A, doc):
    SR12, C = A.R13.SR12, A.C
    by_id = {c['id']: c for c in doc['cases']}
    d3 = by_id['BR23-d3']
    provenance('BR23-d3', d3['source'])
    w = SR12.World()
    w.load(copy.deepcopy(d3['events']))
    w.run(d3['until'])
    summ = w.gen_summary()
    ex = d3['expect']
    for gid, fields in ex['gens'].items():
        act = {f: summ.get(int(gid), {}).get(f) for f in fields}
        check('BR23-d3.gen%s' % gid, act == fields, expected=fields, actual=act)
    ww = [r for r in w.trace if r['ev'] == 'windowWait' and r['gen'] == 5]
    exw = ex['windowWait']
    check('BR23-d3.windowWait', bool(ww) and ww[0]['t'] == exw['t'] and ww[0]['count'] == exw['count'] and ww[0]['until'] == exw['until'],
          actual=ww[:1])
    first5 = min((op.issued for op in w.be.ops if op.gen == 5 and op.kind == 'set'), default=None)
    check('BR23-d3.noEpochItemBeforeWindowExit', first5 == ex['gen5FirstEpochOpAt'], actual=first5)
    check('BR23-d3.violations', w.violations == ex['violations'] and w.max_names <= ex['maxEpochNamesAtMost'],
          violations=w.violations, maxNames=w.max_names)
    check('BR23-d3.componentIs012World', Path(sys.modules['sr_ref'].__file__).resolve() == (ROOT / 'm1-draft-0.12' / 'tools' / 'sr_ref.py').resolve())

    d7 = by_id['BR23-d7']
    provenance('BR23-d7', d7['source'])

    def admit_pairs(n):
        try:
            C.serialize_pairs({'k%d' % i: 'v' for i in range(n)})
            return 'ok'
        except C.CodecError as err:                                          # P-E6-5: codec limit -> 4300 reply
            return {'code': 4300, 'reason': err.reason, 'limit': err.detail.get('limit')}
    check('BR23-d7.entries4096', admit_pairs(4096) == d7['expect']['entries4096'])
    check('BR23-d7.entries4097', admit_pairs(4097) == d7['expect']['entries4097'], actual=admit_pairs(4097))


def wiring(A):
    check('wiring.adminUses013Engine', Path(A.R13.__file__).resolve() == (ROOT / 'm1-draft-0.13' / 'tools' / 'recovery_ref.py').resolve()
          and sys.modules.get('recovery_ref') is A.R13, file=str(A.R13.__file__))
    check('wiring.adminUses013Codec', Path(A.C.__file__).resolve() == (ROOT / 'm1-draft-0.13' / 'tools' / 'fmt2_codec.py').resolve()
          and A.R13.C is A.C)
    check('wiring.adminUses012Strict', A.R13.SR12 is sys.modules.get('sr_ref')
          and Path(A.R13.SR12.__file__).resolve() == (ROOT / 'm1-draft-0.12' / 'tools' / 'sr_ref.py').resolve())
    check('wiring.adminEngineSubclass', issubclass(A.AdminEngine, A.R13.Engine) and A.EXTRA_FAULTS.isdisjoint(A.R13.FAULTS))
    check('wiring.sitesFaultsNamed', S.FAULTS == {'noLateSites', 'dropPinned', 'seatOnCounted', 'epochReserveOmitted', 'diskNoReserve', 'noDiskGate'})


# ------------------------------------------------------------------ coverage

REQUIRED = (['BR23-d%d' % i for i in range(1, 16)] + ['BR23-d9-variant', 'BR23-d11b', 'BR23-d12a', 'BR23-d12b', 'BR23-d15b', 'BR23-d15c',
            'BR23-d15c-dropPinned']
            + ['BR24-x%d' % i for i in (1, 2, 3, 4, 6, 7, 8, 9, 10, 11, 12)] + ['BR24-x5a', 'BR24-x5b', 'BR24-x6-sites', 'BR24-x12b', 'BR22f-f5']
            + ['E6-const', 'E6-bound', 'E6-tombBytesExempt', 'E6-tombCountsNames', 'E6-removeZeroTicket', 'E6-ticketLifecycle',
               'assumption.A15c', 'assumption.A15d', 'BR22a-sweepMax'])


def coverage(control_ids):
    for vid in REQUIRED + control_ids:
        own = [r for r in results if r['check'] == vid or r['check'].startswith(vid + '.')]
        executed = [r for r in own if r['status'] in ('pass', 'FAIL') and not r['check'].endswith('.provenance')]
        gaps = [r['check'] for r in own if r.get('partialGap')]
        if executed:
            check('coverage.' + vid, True, executedChecks=len(executed), failed=sum(r['status'] == 'FAIL' for r in executed), partialGaps=gaps)
        elif gaps:
            record('coverage.' + vid, 'recorded', partialGap=True, executedChecks=0, partialGaps=gaps)
        else:
            check('coverage.' + vid, False, reason='no executed check and no explicit gap')


# ------------------------------------------------------------------ main

def main():
    t0 = time.time()
    inputs()
    boundary()
    rerun_015()                                    # first, so module state matches the accepted 0.15 run
    ledger = load_json('m1-draft-0.16/vectors/e6e7-ledger.json')
    admin = load_json('m1-draft-0.16/vectors/e6-admin.json')
    adapters = load_json('m1-draft-0.16/vectors/e6e7-adapters.json')
    assume = load_json('m1-draft-0.16/vectors/e7-assumption-violations.json')
    A = None
    try:
        import admin_ref as A                                                # imported after the re-run (cached 0.12/0.13 modules)
        wiring(A)
    except Exception as e:                                                   # record, never hide
        check('step.completed import_admin_ref', False, exception='%s: %s' % (type(e).__name__, e))
    steps = [('ledger', lambda: ledger_suite(ledger)), ('assumptions', lambda: assumption_suite(assume))]
    if A is not None:
        steps += [('admin', lambda: admin_suite(A, admin)), ('adapters', lambda: adapter_suite(A, adapters))]
    for name, fn in steps:
        try:
            fn()
        except Exception as e:                                               # record, never hide
            check('step.completed ' + name, False, exception='%s: %s' % (type(e).__name__, e))
    controls = ['control.%s.%s' % (c['fault'], c['case']) for c in ledger['controls'] + admin['controls']]
    coverage(controls)
    failed = [r for r in results if r['status'] == 'FAIL']
    out = {'package': 'M1 draft 0.16 (rows E6 and E7: DiskLedger, AdminDelete/AdminSlots/DeleteOp, sites, TombReaper; 0.15 re-run read-only)',
           'executedAtUtc': datetime.now(timezone.utc).isoformat(timespec='seconds'),
           'environment': {'python': sys.version.split()[0], 'executable': sys.executable, 'platform': platform.platform(),
                           'thirdPartyPackages': 'none'},
           'statusSchema': {'coreStatus': list(CORE_STATUS), 'diagnosticRenames': RENAMED, 'partialGap': 'recorded entries with partialGap=true'},
           'evidenceKind': 'reference models of the specification text (manual clock, FakeBackend); not Chrome, not chrome.storage',
           'notExecuted': ['BR22c (Chrome + CDP; A15c/A15d measurement)', 'TypeScript implementation', 'chrome.storage behaviour',
                           'BR22a sweepMax control (E3 World has no sweepMax mutation)'],
           'summary': {'checks': len(results), 'passed': sum(r['status'] == 'pass' for r in results),
                       'recorded': sum(r['status'] == 'recorded' for r in results), 'failed': len(failed),
                       'partialGaps': sum(1 for r in results if r.get('partialGap')),
                       'rerun015Entries': sum(1 for r in results if r['check'].startswith('rerun015.')),
                       'seconds': round(time.time() - t0, 1)},
           'results': results}
    RES.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(out, indent=2, ensure_ascii=True, default=str) + '\n', encoding='utf-8')
    print(json.dumps(out['summary']))
    for f in failed:
        print('FAIL', json.dumps(f, ensure_ascii=True, default=str)[:400])
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
