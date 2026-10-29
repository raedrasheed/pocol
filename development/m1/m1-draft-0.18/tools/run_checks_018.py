"""Reference-only runner for M1 draft 0.18: C28 repair (observation snapshot alias).
Python standard library only. NOT executed by the author.

Usage, from D:\\PoCol-Development:
  coordination\\runtime\\python311\\python.exe m1-draft-0.18\\tools\\run_checks_018.py
Writes only m1-draft-0.18/results/run-results-0.18.json and exits 1 on any FAIL.

Steps:
  1. Load m1-draft-0.16/tools/run_checks_016.py and m1-draft-0.17/tools/run_checks_017.py as modules
     (no main() called, no result file written) and install the observation-time capture
     (tools/observe_capture.py) into their globals.
  2. C28 regressions: adversarial nested-mutation engine; earlier cells stay byte-identical, later
     cells see the change, expected objects are isolated, logger diagnostics are isolated, and the
     PRISTINE 0.16 admin_cells (separately loaded, unpatched) is shown to exhibit the defect.
  3. Replay of run_checks_017.main() body: the whole 0.17 scope (C27, RF-E6-1, conventions,
     coverage017) including the whole 0.16 suite and the read-only 0.15 replay. Entries copied
     with prefix 'suite017.'. Models, faults, expected goldens unchanged.
  4. Per-row history of BR24-x9 adminEarlyRelease violations, 0.18 coverage guard, preserved files.
"""

import ast
import copy
import hashlib
import importlib.util
import json
import platform
import sys
import time
import types
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
PKG, ROOT = HERE.parent, HERE.parent.parent
RES = PKG / 'results'
OUT = RES / 'run-results-0.18.json'
CORE_STATUS = ('pass', 'recorded', 'FAIL')
RENAMED = {'name': 'diagName', 'check': 'diagCheck', 'status': 'proposalStatus', 'ok': 'diagOk',
           'label': 'diagLabel', 'condition': 'diagCondition'}
PRESERVED = ['m1-draft-0.13/results/run-results-0.13.json', 'm1-draft-0.14/results/run-results-0.14.json',
             'm1-draft-0.15/results/run-results-0.15.json', 'm1-draft-0.16/results/run-results-0.16.json',
             'm1-draft-0.17/results/run-results-0.17.json',
             'coordination/review-001/m1-draft-0.16/results/run-results-0.16.json',
             'coordination/review-001/m1-draft-0.17/results/run-results-0.17.json']
INPUTS = ['coordination/task-016.md', 'coordination/review-001/REVIEW-0.17.md',
          'm1-draft-0.16/tools/run_checks_016.py', 'm1-draft-0.17/tools/run_checks_017.py', 'm1-draft-0.17/tools/admin_trace.py',
          'm1-draft-0.17/vectors/c27-x9-early-release.json', 'm1-draft-0.17/vectors/rf-e6-1-x12-timelines.json',
          'm1-draft-0.18/tools/observe_capture.py', 'm1-draft-0.18/tools/run_checks_018.py']
X9_HISTORY = {'row@5000.violations': [], 'row@6000.violations': [{'t': 6000, 'kind': 'adminTombsUnsettledOver2', 'count': 3}],
              'final.violations': [{'t': 6000, 'kind': 'adminTombsUnsettledOver2', 'count': 3}]}   # the cells the C27 fixture asserts
X9_PREFIX = 'suite017.suite016.BR24-x9.faultyPath.adminEarlyRelease.'

results = []


def make_entry(label, core_status, diag):
    if core_status not in CORE_STATUS:
        raise ValueError('core status must be one of %s, got %r' % (CORE_STATUS, core_status))
    entry = {}
    for k, v in diag.items():
        entry[RENAMED.get(k, k)] = v
    entry['check'] = label
    entry['status'] = core_status            # core schema written last
    return entry


sys.path.insert(0, str(HERE))
import observe_capture as OC          # noqa: E402

record, check = OC.make_safe_capture(results, make_entry)


def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest() if p.exists() else None


def load_module(rel, modname):
    spec = importlib.util.spec_from_file_location(modname, str(ROOT / rel))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def dumps(v):
    return json.dumps(v, sort_keys=True, ensure_ascii=True, default=str)


def boundary():
    for rel in INPUTS:
        record('input.sha256 ' + rel, 'recorded', exists=(ROOT / rel).exists(), sha256=sha(ROOT / rel))
    check('boundary.positionalOnly', record.__code__.co_posonlyargcount == 2 and check.__code__.co_posonlyargcount == 2)
    calls = [n.lineno for n in ast.walk(ast.parse(Path(__file__).read_text(encoding='utf-8')))
             if isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute) and n.func.attr == 'main']
    check('boundary.noOldMainCalled', not calls, attributeMainCallsAtLines=calls)


# ------------------------------------------------------------------ adversarial engine

class MutatingEngine:
    """Minimal engine surface read by admin_actual for the fields used here. Every run() mutates
    nested state IN PLACE (append to lists, update dicts, mutate dicts inside lists), and a
    snapshot record aliases the live dictionary on purpose."""

    def __init__(self):
        self.sets = []
        self.violations = []
        self.admin_stats = {'adminTombsUnsettledMax': 0}
        self.keys = {'K': {'dict': {'a': '1', 'nest': ['x']}}}
        self.snapshots = []
        self.t = 0

    def run(self, t):
        while self.t < t:
            self.t += 1
            if self.t == 2:
                self.violations.append({'t': 2, 'kind': 'k', 'count': 3})
                self.admin_stats['adminTombsUnsettledMax'] = 3
                self.keys['K']['dict']['a'] = '2'
                self.keys['K']['dict']['nest'].append('y')
                self.snapshots.append({'t': 2, 'key': 'K', 'frame': 'F', 'dict': self.keys['K']['dict']})
            elif self.t == 3:
                self.violations[0]['count'] = 4                              # nested in-place mutation
                self.keys['K']['dict']['nest'].append('z')
            elif self.t == 4:
                self.violations.append({'t': 4, 'kind': 'k2', 'count': 9})
                self.keys['K']['dict'].clear()


def adversarial_spec():
    return {'rows': [{'t': 1, 'violations': [], 'adminStats': {'adminTombsUnsettledMax': 0}, 'dict': {'K': {'a': '1', 'nest': ['x']}}},
                     {'t': 2, 'violations': [{'t': 2, 'kind': 'k', 'count': 3}], 'adminStats': {'adminTombsUnsettledMax': 3},
                      'dict': {'K': {'a': '2', 'nest': ['x', 'y']}}, 'snapshots': [[2, 'K', 'F', {'a': '2', 'nest': ['x', 'y']}]]}],
            'final': {'violations': [{'t': 2, 'kind': 'k', 'count': 4}], 'dict': {'K': {'a': '2', 'nest': ['x', 'y', 'z']}},
                      'snapshots': [[2, 'K', 'F', {'a': '2', 'nest': ['x', 'y', 'z']}]]}}


STUB_A = types.SimpleNamespace(R13=types.SimpleNamespace(SR12=None), C=None)     # admin_actual reads A.R13.SR12 and A.C first


def regressions(R16, pristine):
    # 1. patched admin_cells: every cell equals the value at its own observation instant
    spec = adversarial_spec()
    e = MutatingEngine()
    cells = R16.admin_cells(STUB_A, e, spec, 3)
    bad = [c for c in cells if c[1] != c[2]]
    check('C28.regression.capturedCellsMatchTheirInstant', not bad, mismatches=bad, cells=len(cells))
    row1 = {c[0]: c[2] for c in cells if c[0].startswith('row@1.')}
    check('C28.regression.earlierRowUnaffectedByLaterMutation', row1 == {'row@1.violations': [], 'row@1.adminStats': {'adminTombsUnsettledMax': 0},
                                                                         'row@1.dict': {'K': {'a': '1', 'nest': ['x']}}}, row1=row1)
    row2 = {c[0]: c[2] for c in cells if c[0].startswith('row@2.')}
    check('C28.regression.laterRowReflectsChange', row2.get('row@2.violations') == [{'t': 2, 'kind': 'k', 'count': 3}]
          and row2.get('row@2.dict') == {'K': {'a': '2', 'nest': ['x', 'y']}}, row2=row2)
    before = dumps(cells)
    e.run(4)                                                                 # mutate the engine further, in place
    check('C28.regression.capturedCellsByteIdenticalAfterLaterMutation', dumps(cells) == before)
    # 2. expected isolation: mutating the fixture objects after capture changes nothing captured
    spec['rows'][0]['violations'].append({'injected': True})
    spec['rows'][1]['dict']['K']['nest'].append('injected')
    spec['final']['violations'][0]['count'] = -1
    check('C28.regression.expectedIsolated', dumps(cells) == before)
    # 3. logger isolation (patched 0.16 and 0.17 loggers and this runner's own)
    probe = {'v': [1], 'd': {'n': ['a']}}
    sink = []
    rec, _ = OC.make_safe_capture(sink, make_entry)
    rec('probe', 'recorded', obj=probe)
    snap = dumps(sink)
    probe['v'].append(2)
    probe['d']['n'].append('b')
    check('C28.regression.loggerDiagnosticsIsolated', dumps(sink) == snap, entry=sink[0])
    # 4. pristine control: the unpatched 0.16 admin_cells shows the defect on the same input
    e2 = MutatingEngine()
    cells2 = pristine.admin_cells(STUB_A, e2, adversarial_spec(), 3)
    bad2 = [c[0] for c in cells2 if c[1] != c[2]]
    check('C28.control.pristineAdminCellsAliases', 'row@1.violations' in bad2 and 'row@1.dict' in bad2 and 'row@2.dict' in bad2,
          mismatchingCells=bad2, note='the 0.16 checker as published; detection proves the regression is sensitive')
    # 5. patched admin_actual alone returns independent objects
    e3 = MutatingEngine()
    v = R16.admin_actual(STUB_A, e3, 'violations', [])
    e3.run(2)
    check('C28.regression.adminActualReturnsCopy', v == [] and len(e3.violations) == 1)


def install_checks(R16, R17, orig):
    check('C28.install.r16AdminCells', getattr(R16.admin_cells, 'c28_capture', False) and R16.admin_cells is not orig['admin_cells'])
    check('C28.install.r16AdminActual', getattr(R16.admin_actual, 'c28_capture', False))
    check('C28.install.r16LedgerCells', getattr(R16.ledger_cells, 'c28_capture', False))
    check('C28.install.r16Logger', R16.record is not orig['record16'] and R16.check is not orig['check16']
          and R16.record.__code__.co_posonlyargcount == 2 and R16.check.__code__.co_posonlyargcount == 2)
    check('C28.install.r17Logger', R17.record is not orig['record17'] and R17.check is not orig['check17']
          and R17.record.__code__.co_posonlyargcount == 2)
    check('C28.install.globalsResolvedAtCallTime', R16.admin_suite.__globals__ is vars(R16) and R17.c27_checks.__globals__ is vars(R17))
    record('C28.audit.callsites', 'recorded', patched=[
        'run_checks_016.admin_cells (rows/final; runs future events between observations) -> capture per cell',
        'run_checks_016.admin_actual (all callers incl. baselineWithoutFault, control_check) -> copy',
        'run_checks_016.ledger_cells (single observation after the run) -> copy',
        'run_checks_016.record/check/gap (diagnostics such as actual=, audit=, witnesses=) -> copy at call time',
        'run_checks_017.record/check (c27_checks violations=e.violations, fifo logs, rf_e61 dict=) -> copy at call time'],
        unchanged=['run_checks_015 and nested 0.10-0.13 loggers (accepted 0.15 replay; counts checked 365/118/0)'])


# ------------------------------------------------------------------ the 0.17 scope, replayed

def replay_017(R16, R17):
    """run_checks_017.main() body (lines 307-332) without its result-file write."""
    R17.boundary()
    c27 = R17.load_json('m1-draft-0.17/vectors/c27-x9-early-release.json')
    admin = R17.corrected_admin(R16, c27)
    A, n_cov = R17.suite_016(R16, admin)
    T = None
    try:
        sys.path.insert(0, str(ROOT / 'm1-draft-0.17' / 'tools'))
        import admin_trace as T
        R17.check('wiring.traceUses016Model', T.A is A and issubclass(T.TracedAdminEngine, A.AdminEngine))
    except Exception as e:                                                   # record, never hide
        R17.check('step.completed import_admin_trace', False, exception='%s: %s' % (type(e).__name__, e))
    if T is not None:
        tl = R17.load_json('m1-draft-0.17/vectors/rf-e6-1-x12-timelines.json')
        for name, fn in (('c27', lambda: R17.c27_checks(T, A, admin, c27)), ('rfE61', lambda: R17.rf_e61_checks(T, A, admin, tl))):
            try:
                fn()
            except Exception as e:                                           # record, never hide
                R17.check('step.completed ' + name, False, exception='%s: %s' % (type(e).__name__, e))
    R17.conventions()
    R17.coverage(n_cov)
    for r in R17.results:
        r = dict(r)
        r['check'] = 'suite017.' + r['check']
        results.append(r)


def history_and_coverage():
    have = {r['check']: r for r in results}
    hist = {}
    for cell, want in X9_HISTORY.items():
        r = have.get(X9_PREFIX + cell)
        hist[cell] = r and r.get('actual')
        check('C28.x9History.' + cell, r is not None and r['status'] == 'pass' and r.get('actual') == want and r.get('expected') == want,
              entry=r)
    record('C28.x9History', 'recorded', violationsPerRow=hist)
    required = [X9_PREFIX + 'row@%d.%s' % (t, f) for t, f in ((5000, 'held'), (5000, 'sets'), (5000, 'adminStats'), (6000, 'adminStats'),
                                                              (6000, 'setLogTail'), (11000, 'setLogTail'), (11000, 'adminStats'))]
    required += [X9_PREFIX + 'final.' + f for f in ('violationKinds', 'violations', 'adminStats', 'setLog', 'dops')]
    required += ['suite017.suite016.rerun015.countsMatchAccepted', 'suite017.suite016.rerun015.oldResultFilesUnchanged',
                 'suite017.suite016.control.adminEarlyRelease.BR24-x9', 'suite017.C27.firstWitness@6000', 'suite017.C27.finalMaximum4',
                 'suite017.RF-E6-1.x12b.literalWitnessOp1', 'suite017.coverage017.allRequiredPresent',
                 'suite017.coverage017.every016VariantGuarded', 'suite017.coverage017.partialGapsKept',
                 'suite017.coverage017.no016StepAborted']
    missing = [x for x in required if x not in have]
    notpass = [x for x in required if x in have and have[x]['status'] != 'pass']
    check('coverage018.requiredPresentAndPassing', not missing and not notpass, missing=missing, notPassing=notpass)
    lit = have.get('suite017.RF-E6-1.x12b.literalTotal')
    check('coverage018.rfE61StillOpenGap', lit is not None and lit['status'] == 'recorded' and lit.get('partialGap') is True)
    gaps = sorted(r['check'] for r in results if r.get('partialGap'))
    record('coverage018.partialGaps', 'recorded', gaps=gaps)
    check('coverage018.noStepAborted', not [r['check'] for r in results if 'step.completed' in r['check']])


def main():
    t0 = time.time()
    before = {rel: sha(ROOT / rel) for rel in PRESERVED}
    boundary()
    try:
        R16 = load_module('m1-draft-0.16/tools/run_checks_016.py', 'run_checks_016_suite018')        # main() not called
        R17 = load_module('m1-draft-0.17/tools/run_checks_017.py', 'run_checks_017_suite018')        # main() not called
        pristine = load_module('m1-draft-0.16/tools/run_checks_016.py', 'run_checks_016_pristine018')  # never patched, never run
        orig = OC.install(R16, R17)
        install_checks(R16, R17, orig)
        regressions(R16, pristine)
        replay_017(R16, R17)
    except Exception as e:                                                   # record, never hide
        check('step.completed suite017', False, exception='%s: %s' % (type(e).__name__, e))
    history_and_coverage()
    after = {rel: sha(ROOT / rel) for rel in PRESERVED}
    check('preserved.olderResultFilesUnchanged', before == after, before=before, after=after)
    failed = [r for r in results if r['status'] == 'FAIL']
    out = {'package': 'M1 draft 0.18 (C28 observation-time capture; full 0.17 scope, 0.16 suite and 0.15 replay)',
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
                       'suite017Entries': sum(1 for r in results if r['check'].startswith('suite017.')),
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
