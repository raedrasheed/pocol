"""Reference-only runner for M1 draft 0.17: C27 repair (BR24-x9 adminEarlyRelease full timeline)
and RF-E6-1 x12/x12b traceability. Python standard library only. NOT executed by the author.

Usage, from D:\\PoCol-Development:
  coordination\\runtime\\python311\\python.exe m1-draft-0.17\\tools\\run_checks_017.py
Writes only m1-draft-0.17/results/run-results-0.17.json and exits 1 on any FAIL.

Steps:
  1. The whole 0.16 suite, replayed from m1-draft-0.16/tools/run_checks_016.py loaded as a module
     (its main() is NOT called and its result file is NOT written). That replay includes the
     read-only replay of the accepted 0.15 suite, every 0.16 golden, control, enumeration, audit and
     coverage guard. The ONLY fixture change is C27: the adminEarlyRelease faulty path of BR24-x9 is
     replaced by vectors/c27-x9-early-release.json (full hand-derived timeline). Entries are
     copied with the prefix 'suite016.'.
  2. C27: exact first witness at 6000 (3 unsettled), full timeline to 14000 (maximum 4, six sets),
     AdminFIFO trace, timer order at 10000/11000, death at 13000, traced = untraced engine.
  3. RF-E6-1: supplemental full timelines for x12, x12b (global adminNoCancel) and the op1-only
     analysis mode; per-op and total stale drops, zero alloc/ticket/set for stale requests, the
     complete FIFO guard trace, final recreated K1. The literal total stays a recorded partial gap.
  4. Conventions CONV-E7-1 / CONV-E7-2 (RF-E7-1, RF-E7-2) and the 0.17 coverage guard.
"""

import ast
import copy
import hashlib
import importlib.util
import json
import platform
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
PKG, ROOT = HERE.parent, HERE.parent.parent
RES = PKG / 'results'
OUT = RES / 'run-results-0.17.json'
CORE_STATUS = ('pass', 'recorded', 'FAIL')
RENAMED = {'name': 'diagName', 'check': 'diagCheck', 'status': 'proposalStatus', 'ok': 'diagOk',
           'label': 'diagLabel', 'condition': 'diagCondition'}
PRESERVED = ['m1-draft-0.13/results/run-results-0.13.json', 'm1-draft-0.14/results/run-results-0.14.json',
             'm1-draft-0.15/results/run-results-0.15.json', 'm1-draft-0.16/results/run-results-0.16.json',
             'coordination/review-001/m1-draft-0.16/results/run-results-0.16.json']
INPUTS = ['coordination/task-015.md', 'coordination/review-001/REVIEW-0.16.md', 'reference/validation.md', 'reference/browser.md',
          'm1-draft-0.16/tools/run_checks_016.py', 'm1-draft-0.16/tools/admin_ref.py', 'm1-draft-0.16/tools/sites_ref.py',
          'm1-draft-0.16/vectors/e6-admin.json', 'm1-draft-0.16/vectors/e6e7-ledger.json',
          'm1-draft-0.17/tools/admin_trace.py', 'm1-draft-0.17/tools/run_checks_017.py',
          'm1-draft-0.17/vectors/c27-x9-early-release.json', 'm1-draft-0.17/vectors/rf-e6-1-x12-timelines.json']
EXPECTED_016_GAPS = ['BR22a-sweepMax.gap', 'BR23-d15c.boundary@10100', 'BR23-d15c.boundary@60300', 'BR23-d13.scope',
                     'BR24-x12b.literalConflict.RF-E6-1', 'coverage.BR22a-sweepMax']

results = []


# ------------------------------------------------------------------ the safe boundary (as 0.15/0.16)

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


def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest() if p.exists() else None


def load_module(rel, modname):
    spec = importlib.util.spec_from_file_location(modname, str(ROOT / rel))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def load_json(rel):
    return json.loads((ROOT / rel).read_text(encoding='utf-8'))


def provenance(label, src):
    lines = (ROOT / src['file']).read_text(encoding='utf-8').splitlines()
    a, b = src['lines']
    text = '\n'.join(lines[a - 1:b])
    missing = [lit for lit in src.get('literals', []) if lit not in text]
    check(label + '.provenance', not missing, file=src['file'], lines=src['lines'], missingLiterals=missing)


def boundary():
    for rel in INPUTS:
        record('input.sha256 ' + rel, 'recorded', exists=(ROOT / rel).exists(), sha256=sha(ROOT / rel))
    check('boundary.positionalOnly', record.__code__.co_posonlyargcount == 2 and check.__code__.co_posonlyargcount == 2)
    calls = [n.lineno for n in ast.walk(ast.parse(Path(__file__).read_text(encoding='utf-8')))
             if isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute) and n.func.attr == 'main']
    check('boundary.noOldMainCalled', not calls, attributeMainCallsAtLines=calls)


# ------------------------------------------------------------------ step 1: the 0.16 suite with the C27 correction

def strip_tags(fp):
    out = {k: copy.deepcopy(v) for k, v in fp.items()}
    out['rows'] = [{k: v for k, v in r.items() if k != 'tag'} for r in fp.get('rows', [])]
    return out


def corrected_admin(R16, c27):
    original = R16.load_json('m1-draft-0.16/vectors/e6-admin.json')
    admin = copy.deepcopy(original)
    x9 = next(c for c in admin['cases'] if c['id'] == 'BR24-x9')
    idx = [i for i, fp in enumerate(x9['faultyPaths']) if fp['fault'] == 'adminEarlyRelease']
    old = x9['faultyPaths'][idx[0]] if len(idx) == 1 else None
    rep = c27['target']['replacedCells']
    check('C27.targetIsTheReviewedFixture', old is not None and old['final'].get('adminStats') == rep['final.adminStats']
          and len(old['final'].get('setLog', [])) == rep['final.setLog.length'], old=old)
    new = strip_tags(c27['faultyPath'])
    x9['faultyPaths'][idx[0]] = new
    o9 = next(c for c in original['cases'] if c['id'] == 'BR24-x9')
    others_same = ([c for c in admin['cases'] if c['id'] != 'BR24-x9'] == [c for c in original['cases'] if c['id'] != 'BR24-x9']
                   and admin['controls'] == original['controls'] and admin['units'] == original['units']
                   and admin['records'] == original['records']
                   and {k: v for k, v in x9.items() if k != 'faultyPaths'} == {k: v for k, v in o9.items() if k != 'faultyPaths'})
    check('C27.onlyTheFaultyPathReplaced', others_same, note='all other goldens, controls and the x9 correct path unchanged')
    check('C27.firstWitnessAssertionKept', any(r['t'] == 6000 and r.get('adminStats') == {'adminTombsUnsettledMax': 3}
                                               and r.get('violations') == [{'t': 6000, 'kind': 'adminTombsUnsettledOver2', 'count': 3}]
                                               for r in new['rows'])
          and new['final']['violationKinds'] == ['adminTombsUnsettledOver2'])
    return admin


def suite_016(R16, admin):
    """run_checks_016.main() body without its result-file write; admin is the C27-corrected doc."""
    R16.inputs()
    R16.boundary()
    R16.rerun_015()
    ledger = R16.load_json('m1-draft-0.16/vectors/e6e7-ledger.json')
    adapters = R16.load_json('m1-draft-0.16/vectors/e6e7-adapters.json')
    assume = R16.load_json('m1-draft-0.16/vectors/e7-assumption-violations.json')
    A = None
    try:
        import admin_ref as A                                                # 0.16 model (sys.path set by the 0.16 module)
        R16.wiring(A)
    except Exception as e:                                                   # record, never hide
        R16.check('step.completed import_admin_ref', False, exception='%s: %s' % (type(e).__name__, e))
    steps = [('ledger', lambda: R16.ledger_suite(ledger)), ('assumptions', lambda: R16.assumption_suite(assume))]
    if A is not None:
        steps += [('admin', lambda: R16.admin_suite(A, admin)), ('adapters', lambda: R16.adapter_suite(A, adapters))]
    for name, fn in steps:
        try:
            fn()
        except Exception as e:                                               # record, never hide
            R16.check('step.completed ' + name, False, exception='%s: %s' % (type(e).__name__, e))
    controls = ['control.%s.%s' % (c['fault'], c['case']) for c in ledger['controls'] + admin['controls']]
    R16.coverage(controls)
    for r in R16.results:
        r = dict(r)
        r['check'] = 'suite016.' + r['check']
        results.append(r)
    return A, len(R16.REQUIRED) + len(controls)


# ------------------------------------------------------------------ engines

def build(T, admin, case, faults=(), no_cancel_ops=(), traced=True, A=None):
    SR12 = T.R13.SR12
    items = {SR12.record_name(*admin['records'][rid]['name']): copy.deepcopy(admin['records'][rid]['value']) for rid in case['records']}
    cls = T.TracedAdminEngine if traced else A.AdminEngine
    kw = {'no_cancel_ops': no_cancel_ops} if traced else {}
    e = cls(case['sites'], records=items, faults=list(faults), **kw)
    e.load(copy.deepcopy(case['events']))
    return e


def unsettled_tombs(e):
    return sum(1 for o in e.sets if o.purpose == 'tomb' and o.settled is None and o.state == 'pending')


def set_log(e):
    return [[o.n, o.purpose, o.key, list(o.version), o.issuedAt] for o in e.sets]


def same_engine(a, b):
    return (set_log(a) == set_log(b) and a.admin_stats == b.admin_stats and a.stats == b.stats and a.trace == b.trace
            and a.violations == b.violations and a.items == b.items and a.replies == b.replies and a.snapshots == b.snapshots)


# ------------------------------------------------------------------ step 2: C27

def c27_checks(T, A, admin, c27):
    provenance('C27', c27['source'])
    x9 = next(c for c in admin['cases'] if c['id'] == 'BR24-x9')
    e = build(T, admin, x9, faults=['adminEarlyRelease'])
    e.run(5999)
    check('C27.before6000.noViolation', e.violations == [] and e.admin_stats['adminTombsUnsettledMax'] == 2,
          maxUnsettled=e.admin_stats['adminTombsUnsettledMax'])
    e.run(6000)
    check('C27.firstWitness@6000', e.violations == [{'t': 6000, 'kind': 'adminTombsUnsettledOver2', 'count': 3}]
          and unsettled_tombs(e) == 3 and len(e.sets) == 5, tag='source literal (validation.md:745)', violations=e.violations)
    e.run(10000)
    check('C27.op1Uncertain@10000', e.dops['op1']['outcome'] == 'uncertain' and e.dops['op1']['resolvedAt'] == 10000
          and len(e.admin_slots) == 1 and unsettled_tombs(e) == 3, tag='source+fault (B freed at 10000)')
    e.run(10999)
    check('C27.noSixthSetBefore11000', len(e.sets) == 5)
    e.run(11000)
    at11 = [x for x in e.fifo_log if x[1] == 11000]
    check('C27.order@11000', at11 == [['request', 11000, 'op2', 2], ['grant', 11000, 'op2', 2, 6]]
          and e.admin_stats['adminBusy'] == 0 and unsettled_tombs(e) == 4,
          tag='derived: the op2 attempt-1 wait timer (scheduled first) has no effect; the T2a deadline frees its slot and issues T2b',
          fifoAt11000=at11)
    e.run(13000)
    check('C27.death@13000', not e.alive and unsettled_tombs(e) == c27['lateTombsAtDeath']['adminEarlyRelease']
          and unsettled_tombs(e) > c27['lateTombsAtDeath']['reserve'], lateTombs=unsettled_tombs(e), tag='source: late tombs exceed the reserve')
    e.run(x9['until'])
    fin = c27['faultyPath']['final']
    check('C27.finalMaximum4', e.admin_stats['adminTombsUnsettledMax'] == fin['adminStats']['adminTombsUnsettledMax'],
          actual=e.admin_stats['adminTombsUnsettledMax'], tag='derived (not a source literal)')
    check('C27.finalSixSets', set_log(e) == fin['setLog'], actual=set_log(e))
    check('C27.fifoLog.adminEarlyRelease', e.fifo_log == c27['fifoLog']['adminEarlyRelease'], actual=e.fifo_log)
    ok = build(T, admin, x9)
    ok.run(x9['until'])
    check('C27.fifoLog.correct', ok.fifo_log == c27['fifoLog']['correct'], actual=ok.fifo_log)
    check('C27.correctPathLateTombsWithinReserve', unsettled_tombs(ok) == c27['lateTombsAtDeath']['correct'], lateTombs=unsettled_tombs(ok))
    for faults in ([], ['adminEarlyRelease']):
        a = build(T, admin, x9, faults=faults)
        b = build(T, admin, x9, faults=faults, traced=False, A=A)
        a.run(x9['until'])
        b.run(x9['until'])
        check('C27.tracedEqualsUntraced.%s' % (faults[0] if faults else 'correct'), same_engine(a, b))


# ------------------------------------------------------------------ step 3: RF-E6-1

def rf_e61_checks(T, A, admin, tl):
    provenance('RF-E6-1', tl['source'])
    x12 = next(c for c in admin['cases'] if c['id'] == tl['eventsFrom']['case'])
    com = tl['common']
    for m in tl['modes']:
        p = 'RF-E6-1.%s' % m['id']
        e = build(T, admin, x12, faults=m['faults'], no_cancel_ops=m['noCancelOps'])
        held = {}
        for t in sorted(com['heldAt'], key=int):
            e.run(int(t))
            held[t] = len(e.admin_slots)
        e.run(tl['until'])
        check(p + '.heldAt', held == com['heldAt'], actual=held)
        check(p + '.fifoLog', e.fifo_log == m['fifoLog'], actual=e.fifo_log, tag=m['tag'])
        by_op = {op: sum(1 for x in e.fifo_log if x[0] == 'staleDrop' and x[2] == op) for op in m['staleDroppedByOp']}
        by_trace = {op: sum(1 for r in e.trace if r['ev'] == 'adminStaleDropped' and r['op'] == op) for op in m['staleDroppedByOp']}
        check(p + '.staleDroppedByOp', by_op == m['staleDroppedByOp'] and by_trace == m['staleDroppedByOp'], fifo=by_op, trace=by_trace)
        check(p + '.adminStaleDroppedTotal', e.admin_stats['adminStaleDropped'] == m['adminStaleDropped'],
              actual=e.admin_stats['adminStaleDropped'])
        check(p + '.setLog', set_log(e) == com['setLog'], actual=set_log(e))
        seq = {k: e.keys[k]['seq'] for k in com['seqNext']}
        check(p + '.zeroAllocForStale', seq == com['seqNext'], seqNext=seq)
        tk = {'total': len(e.disk.tickets), 'tomb': sum(1 for x in e.disk.tickets if x['kind'] == 'tomb')}
        check(p + '.zeroTicketForStale', tk == com['diskTickets'], tickets=tk)
        after = {op: sum(1 for o in e.sets if o.purpose == 'tomb' and o.gate == op and o.issuedAt > e.dops[op]['resolvedAt'])
                 for op in com['tombSetsAfterResolution']}
        check(p + '.zeroSetAfterResolution', after == com['tombSetsAfterResolution'], actual=after)
        dops = {op: {'outcome': e.dops[op]['outcome'], 'resolvedAt': e.dops[op]['resolvedAt']} for op in com['dops']}
        check(p + '.outcomes', dops == com['dops'], actual=dops)
        d = {k: e.keys[k]['dict'] for k in com['dict']}
        nxt = {k: T.A.C.lookup_fmt2(e.items, *e.sites[k], T.R13.SR12.parse_record_name).get('dict') for k in com['nextGen']}
        check(p + '.recreatedK1', d == com['dict'] and nxt == com['nextGen'], dict=d, nextGen=nxt)
        rep = [[r['id'], r['code'], r['reason'], r['sub'], r['t']] for r in e.replies]
        check(p + '.repliesOnce', rep == com['replies'] and all(mm['replied'] for mm in e.msgs.values()), replies=rep)
        check(p + '.noViolations', e.violations == [] and e.admin_stats['adminTombsUnsettledMax'] <= 2, violations=e.violations)
        if not m['noCancelOps']:
            b = build(T, admin, x12, faults=m['faults'], traced=False, A=A)
            b.run(tl['until'])
            e2 = build(T, admin, x12, faults=m['faults'])
            e2.run(tl['until'])
            check(p + '.tracedEqualsUntraced', same_engine(e2, b))
        if 'literalWitness' in m:
            lw = m['literalWitness']
            check(p + '.literalWitnessOp1', by_op['op1'] == lw['value'], cell=lw['cell'], readAs=lw['readAs'], tag='source literal')
            record(p + '.literalTotal', 'recorded', partialGap=True, literal=1, model=e.admin_stats['adminStaleDropped'],
                   blocker=tl['reconciliation']['blocker'], options={k: tl['reconciliation'][k] for k in ('optionA', 'optionB', 'optionC')},
                   status_note=tl['reconciliation']['status'])
    derived = [c for c in tl['cells'] if c['tag'] == 'derived']
    check('RF-E6-1.supplementalCellsTagged', all(c['tag'] in ('source', 'derived') for c in tl['cells'])
          and any(c['t'] == 5001 for c in derived), derivedCells=[c['t'] for c in derived])


# ------------------------------------------------------------------ step 4: conventions and coverage

def conventions():
    have = {r['check']: r for r in results}
    a = have.get('suite016.BR23-d1.autoReaperVariant')
    check('CONV-E7-1.autoReaperVariantPresent', a is not None and a['status'] == 'pass', entry=a)
    record('CONV-E7-1', 'recorded', convention=True,
           text='d1 golden keeps the literal reaper remove at 300 via an explicit scan; with D113 scanning at every adopted refresh the remove '
                'issues at 250. Same states at 400 and 450. Convention, not a source change.')
    w = have.get('suite016.BR23-d13.witnesses')
    check('CONV-E7-2.witnessesPresent', w is not None and w['status'] == 'recorded', entry=w)
    record('CONV-E7-2', 'recorded', convention=True,
           text='d13: the live generation tries a new site every 1000 ms and fills capacity; later dead generations\' creating ops may be '
                'rejected. Bound checked over all 729 placements regardless; histogram recorded. Convention, not a source change.')


REQUIRED_017 = ['C27.targetIsTheReviewedFixture', 'C27.onlyTheFaultyPathReplaced', 'C27.firstWitnessAssertionKept', 'C27.firstWitness@6000',
                'C27.order@11000', 'C27.death@13000', 'C27.finalMaximum4', 'C27.finalSixSets', 'C27.fifoLog.adminEarlyRelease',
                'RF-E6-1.x12.fifoLog', 'RF-E6-1.x12.adminStaleDroppedTotal', 'RF-E6-1.x12b.fifoLog', 'RF-E6-1.x12b.staleDroppedByOp',
                'RF-E6-1.x12b.adminStaleDroppedTotal', 'RF-E6-1.x12b.literalWitnessOp1', 'RF-E6-1.x12b.literalTotal',
                'RF-E6-1.x12b-op1only.adminStaleDroppedTotal', 'RF-E6-1.x12b.zeroAllocForStale', 'RF-E6-1.x12b.zeroTicketForStale',
                'RF-E6-1.x12b.zeroSetAfterResolution', 'RF-E6-1.x12b.recreatedK1', 'CONV-E7-1', 'CONV-E7-2',
                'suite016.BR24-x9.faultyPath.adminEarlyRelease.final.adminStats', 'suite016.BR24-x9.faultyPath.adminEarlyRelease.final.setLog',
                'suite016.BR24-x9.faultyPath.adminEarlyRelease.row@6000.violations', 'suite016.control.adminEarlyRelease.BR24-x9',
                'suite016.rerun015.countsMatchAccepted', 'suite016.rerun015.oldResultFilesUnchanged']


def coverage(n_cov016):
    have = {r['check']: r['status'] for r in results}
    missing = [x for x in REQUIRED_017 if x not in have]
    check('coverage017.allRequiredPresent', not missing, missing=missing)
    cov = [r for r in results if r['check'].startswith('suite016.coverage.')]
    check('coverage017.every016VariantGuarded', len(cov) == n_cov016 and all(r['status'] in ('pass', 'recorded') for r in cov),
          guards=len(cov), expected=n_cov016)
    gaps = sorted(r['check'][len('suite016.'):] for r in results if r['check'].startswith('suite016.') and r.get('partialGap'))
    check('coverage017.partialGapsKept', all(g in gaps for g in EXPECTED_016_GAPS), gaps=gaps)
    check('coverage017.no016StepAborted', not [r for r in results if r['check'].startswith('suite016.step.completed')])


# ------------------------------------------------------------------ main

def main():
    t0 = time.time()
    before = {rel: sha(ROOT / rel) for rel in PRESERVED}
    boundary()
    R16 = A = T = None
    n_cov = 0
    try:
        R16 = load_module('m1-draft-0.16/tools/run_checks_016.py', 'run_checks_016_suite017')     # main() not called
        c27 = load_json('m1-draft-0.17/vectors/c27-x9-early-release.json')
        admin = corrected_admin(R16, c27)
        A, n_cov = suite_016(R16, admin)
    except Exception as e:                                                   # record, never hide
        check('step.completed suite016', False, exception='%s: %s' % (type(e).__name__, e))
    if A is not None:
        try:
            sys.path.insert(0, str(HERE))
            import admin_trace as T                                         # 0.17 trace subclass of the 0.16 model
            check('wiring.traceUses016Model', T.A is A and issubclass(T.TracedAdminEngine, A.AdminEngine))
        except Exception as e:                                               # record, never hide
            check('step.completed import_admin_trace', False, exception='%s: %s' % (type(e).__name__, e))
    if T is not None:
        tl = load_json('m1-draft-0.17/vectors/rf-e6-1-x12-timelines.json')
        for name, fn in (('c27', lambda: c27_checks(T, A, admin, c27)), ('rfE61', lambda: rf_e61_checks(T, A, admin, tl))):
            try:
                fn()
            except Exception as e:                                           # record, never hide
                check('step.completed ' + name, False, exception='%s: %s' % (type(e).__name__, e))
    conventions()
    coverage(n_cov)
    after = {rel: sha(ROOT / rel) for rel in PRESERVED}
    check('preserved.olderResultFilesUnchanged', before == after, before=before, after=after)
    failed = [r for r in results if r['status'] == 'FAIL']
    out = {'package': 'M1 draft 0.17 (C27 BR24-x9 adminEarlyRelease full timeline; RF-E6-1 x12/x12b traceability; full 0.16 suite and 0.15 replay)',
           'executedAtUtc': datetime.now(timezone.utc).isoformat(timespec='seconds'),
           'environment': {'python': sys.version.split()[0], 'executable': sys.executable, 'platform': platform.platform(),
                           'thirdPartyPackages': 'none'},
           'statusSchema': {'coreStatus': list(CORE_STATUS), 'diagnosticRenames': RENAMED, 'partialGap': 'recorded entries with partialGap=true'},
           'evidenceKind': 'reference models of the specification text (manual clock, FakeBackend); not Chrome, not chrome.storage',
           'notExecuted': ['BR22c (Chrome + CDP; A15c/A15d measurement)', 'TypeScript implementation', 'chrome.storage behaviour',
                           'BR22a sweepMax control'],
           'summary': {'checks': len(results), 'passed': sum(r['status'] == 'pass' for r in results),
                       'recorded': sum(r['status'] == 'recorded' for r in results), 'failed': len(failed),
                       'partialGaps': sum(1 for r in results if r.get('partialGap')),
                       'suite016Entries': sum(1 for r in results if r['check'].startswith('suite016.')),
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
