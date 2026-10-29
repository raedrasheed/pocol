"""Reference-only runner for M1 draft 0.15: complete C26 repair of the diagnostic logging
boundary. Python standard library only. NOT executed by the author.

Usage, from D:\\PoCol-Development:
  coordination\\runtime\\python311\\python.exe m1-draft-0.15\\tools\\run_checks_015.py
Writes only m1-draft-0.15/results/run-results-0.15.json. No older file is edited or written,
and no older main() is called.

C26 history (preserved):
  0.13  record('e4.surrogateCollision.status', 'recorded', status=...)  -> TypeError (status twice)
  0.14  repaired record only; check('e4.maxRecord', ..., name=len(name)) -> TypeError (name twice)
        -> 9 E4 checks never ran (maximum record, 7 disk boundaries, ticket lifecycle).

Repair. EVERY runner module loaded during this run (0.13, and the nested 0.12, 0.11, 0.10
runners it loads) gets collision-free `record` and `check` installed in its own module
globals, immediately after the module executes. This is done by a temporary hook on
importlib.util.spec_from_file_location that wraps loader.exec_module for files named
run_checks_0*.py. The hook is removed afterwards.
  record(label, core_status, /, **diag)   check(label, condition, /, **diag)
Label, core status and condition are POSITIONAL-ONLY, so no keyword can bind to them.
Reserved diagnostic keys are moved: name->diagName, check->diagCheck, status->proposalStatus,
ok->diagOk, label->diagLabel, condition->diagCondition. The core fields `check` and `status`
are written LAST; status must be one of pass/recorded/FAIL.
Not changed: models, expected vectors, codec, engine, schedules, scheduler, budgets, gates,
CR-E4-01/02, owner policies.
"""

import ast
import contextlib
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
CORE_STATUS = ('pass', 'recorded', 'FAIL')
RENAMED = {'name': 'diagName', 'check': 'diagCheck', 'status': 'proposalStatus', 'ok': 'diagOk',
           'label': 'diagLabel', 'condition': 'diagCondition'}
RUNNERS = ['m1-draft-0.13/tools/run_checks_013.py', 'm1-draft-0.12/tools/run_checks_012.py',
           'm1-draft-0.11/tools/run_checks_011.py', 'm1-draft-0.10/tools/run_checks_010.py']
PREVIOUSLY_SKIPPED = ['e4.maxRecord', 'e4.disk names-boundary-accept', 'e4.disk names-boundary-reject', 'e4.disk bytes-boundary-accept',
                      'e4.disk bytes-boundary-reject', 'e4.disk tomb-exempt-from-bytes', 'e4.disk tomb-still-counts-names',
                      'e4.disk remove-zero-ticket', 'e4.disk.ticketLifecycle']

results = []
INPUTS = ['coordination/review-001/REVIEW-0.13.md', 'coordination/review-001/REVIEW-0.14.md',
          'm1-draft-0.14/tools/run_checks_014.py', 'm1-draft-0.13/tools/fmt2_codec.py', 'm1-draft-0.13/tools/recovery_ref.py',
          'm1-draft-0.13/vectors/e4-units.json', 'm1-draft-0.13/vectors/e5-schedules.json', 'm1-draft-0.12/tools/sr_ref.py',
          'm1-draft-0.15/tools/run_checks_015.py'] + RUNNERS


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


own_record, own_check = make_safe(results)

PATCHED = []                                  # (module name, module) for every runner patched


def install_safe(mod):
    rec, chk = make_safe(mod.results)
    mod.record, mod.check = rec, chk
    PATCHED.append((mod.__name__, mod, rec, chk))


@contextlib.contextmanager
def safe_runner_hook():
    original = importlib.util.spec_from_file_location

    def hooked(name, location=None, *a, **kw):
        spec = original(name, location, *a, **kw)
        if spec is None or location is None or not Path(str(location)).name.startswith('run_checks_0'):
            return spec
        inner = spec.loader.exec_module

        def exec_module(mod):
            inner(mod)
            install_safe(mod)
        spec.loader.exec_module = exec_module
        return spec
    importlib.util.spec_from_file_location = hooked
    try:
        yield
    finally:
        importlib.util.spec_from_file_location = original


def load_plain(rel, modname):
    spec = importlib.util.spec_from_file_location(modname, str(ROOT / rel))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


# ------------------------------------------------------------------ evidence steps

def input_hashes():
    for rel in INPUTS:
        p = ROOT / rel
        own_record('input.sha256 ' + rel, 'recorded', exists=p.exists(), sha256=hashlib.sha256(p.read_bytes()).hexdigest() if p.exists() else None)
    for rel in ('m1-draft-0.13/results/run-results-0.13.json', 'm1-draft-0.14/results/run-results-0.14.json'):
        own_record('preserved.resultFile ' + rel, 'recorded', exists=(ROOT / rel).exists(), note='not read for evidence, not overwritten')


def scan_callers():
    """Static scan of every runner for calls to record/check that pass a reserved keyword."""
    reserved = set(RENAMED)
    hits = []
    for rel in RUNNERS + ['m1-draft-0.14/tools/run_checks_014.py']:
        tree = ast.parse((ROOT / rel).read_text(encoding='utf-8'))
        for node in ast.walk(tree):
            if isinstance(node, ast.Call):
                fn = node.func.id if isinstance(node.func, ast.Name) else (node.func.attr if isinstance(node.func, ast.Attribute) else None)
                if fn in ('record', 'check', 'own', 'ok'):
                    bad = [k.arg for k in node.keywords if k.arg in reserved]
                    if bad:
                        hits.append({'file': rel, 'line': node.lineno, 'call': fn, 'keywords': bad})
    own_record('scan.reservedKeywordCallers', 'recorded', hits=hits)
    known = {(h['file'], h['call'], tuple(h['keywords'])) for h in hits}
    own_check('scan.knownC26CallersFound', ('m1-draft-0.13/tools/run_checks_013.py', 'record', ('status',)) in known
              and ('m1-draft-0.13/tools/run_checks_013.py', 'check', ('name',)) in known, hits=len(hits))
    return hits


def reproduce_pristine():
    """Controlled pristine functions: the 0.13 status TypeError and the 0.14-path name TypeError."""
    p13 = load_plain('m1-draft-0.13/tools/run_checks_013.py', 'run_checks_013_pristine')
    try:
        p13.record('probe', 'recorded', status='open')
        e1 = None
    except TypeError as e:
        e1 = str(e)
    own_check('c26.preserved.0.13RecordStatusTypeError', e1 is not None, error=e1)
    try:
        p13.check('probe', True, name=142)
        e2 = None
    except TypeError as e:
        e2 = str(e)
    own_check('c26.preserved.0.13CheckNameTypeError', e2 is not None, error=e2)
    p14 = load_plain('m1-draft-0.14/tools/run_checks_014.py', 'run_checks_014_pristine')
    sink = []

    def safe14(name, core_status, /, **diag):           # 0.14's repaired record, applied the way 0.14 applied it
        sink.append(p14.make_entry(name, core_status, diag))
    p13b = load_plain('m1-draft-0.13/tools/run_checks_013.py', 'run_checks_013_pristine_b')
    p13b.record = safe14
    try:
        p13b.check('probe', True, name=142)
        e3 = None
    except TypeError as e:
        e3 = str(e)
    own_check('c26.preserved.0.14PathCheckNameTypeError', e3 is not None, error=e3)
    return p14


def boundary_tests():
    sink = []
    rec, chk = make_safe(sink)
    chk('lbl', True, name='n', status='s', check='c', ok='o', label='l', condition='x', other=1)
    chk('lbl2', False, name='n2', status='recorded', check='c2', ok=True)
    rec('lbl3', 'recorded', name='n3', status='FAIL', check='c3', ok=False, label='l3')
    a, b, c = sink
    own_check('boundary.allReservedAtOnce.pass', a['check'] == 'lbl' and a['status'] == 'pass' and a['diagName'] == 'n' and a['proposalStatus'] == 's'
              and a['diagCheck'] == 'c' and a['diagOk'] == 'o' and a['diagLabel'] == 'l' and a['diagCondition'] == 'x' and a['other'] == 1, entry=a)
    own_check('boundary.allReservedAtOnce.fail', b['check'] == 'lbl2' and b['status'] == 'FAIL' and b['proposalStatus'] == 'recorded'
              and b['diagName'] == 'n2' and b['diagCheck'] == 'c2' and b['diagOk'] is True, entry=b)
    own_check('boundary.recordDiagnosticCannotChangeEnum', c['status'] == 'recorded' and c['proposalStatus'] == 'FAIL'
              and c['check'] == 'lbl3' and c['diagOk'] is False, entry=c)
    try:
        rec('bad', 'open')
        rejected = False
    except ValueError:
        rejected = True
    own_check('boundary.nonCoreStatusRejected', rejected)
    own_check('boundary.positionalOnly', rec.__code__.co_posonlyargcount == 2 and chk.__code__.co_posonlyargcount == 2)


def main():
    t0 = time.time()
    input_hashes()
    scan_callers()
    p14 = None
    try:
        p14 = reproduce_pristine()
        boundary_tests()
    except Exception as e:                                                   # record, never hide
        own_check('step.completed preamble', False, exception='%s: %s' % (type(e).__name__, e))
    R13 = None
    with safe_runner_hook():
        try:
            spec = importlib.util.spec_from_file_location('run_checks_013_rerun', str(ROOT / RUNNERS[0]))
            R13 = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(R13)                                    # patched on load; main() not called
        except Exception as e:                                               # record, never hide
            own_check('step.completed load_r13', False, exception='%s: %s' % (type(e).__name__, e))
        if R13 is not None:
            mine = [(rec, chk) for _, m, rec, chk in PATCHED if m is R13]
            own_check('wiring.r13GlobalsPatched', bool(mine) and R13.e4_checks.__globals__['check'] is mine[0][1]
                      and R13.e4_checks.__globals__['record'] is mine[0][0] and R13.e5_checks.__globals__['check'] is mine[0][1])
            for name in ('input_hashes', 'wiring', 'e4_checks', 'e5_checks', 'inherited_012'):
                try:
                    getattr(R13, name)()
                except Exception as e:                                       # record, never hide
                    R13.check('step.completed ' + name, False, exception='%s: %s' % (type(e).__name__, e))
    if R13 is not None:
        names = sorted(n for n, _, _, _ in PATCHED)
        own_record('wiring.patchedRunners', 'recorded', modules=names)
        own_check('wiring.nestedRunnersPatched', all(any(n.startswith(p) for n in names) for p in
                                                     ('run_checks_013', 'run_checks_012', 'run_checks_011', 'run_checks_010')), modules=names)
        rerun = R13.results
        own_check('schema.coreStatusEnumOnly', all(r.get('status') in CORE_STATUS for r in rerun),
                  offending=[r.get('check') for r in rerun if r.get('status') not in CORE_STATUS][:10])
        aborted = [r['check'] for r in rerun if 'step.completed' in r['check']]
        own_check('schema.noStepAborted', not aborted, aborted=aborted)
        have = {r['check']: r['status'] for r in rerun}
        u = json.loads((ROOT / 'm1-draft-0.13' / 'vectors' / 'e4-units.json').read_text(encoding='utf-8'))
        s = json.loads((ROOT / 'm1-draft-0.13' / 'vectors' / 'e5-schedules.json').read_text(encoding='utf-8'))
        if p14 is None:
            p14 = load_plain('m1-draft-0.14/tools/run_checks_014.py', 'run_checks_014_pristine')
        e4_want, e5_want = p14.expected_e4_names(u), p14.expected_e5_names(s)          # 0.14 guards reused unchanged
        own_check('coverage.e4AllExpectedChecksPresent', all(n in have for n in e4_want), expected=len(e4_want),
                  missing=[n for n in e4_want if n not in have])
        own_check('coverage.e5AllExpectedChecksPresent', all(n in have for n in e5_want), expected=len(e5_want),
                  missing=[n for n in e5_want if n not in have])
        own_check('coverage.previouslySkippedNineExecuted', all(n in have for n in PREVIOUSLY_SKIPPED),
                  statuses={n: have.get(n) for n in PREVIOUSLY_SKIPPED})
        mr = [r for r in rerun if r['check'] == 'e4.maxRecord']
        own_check('coverage.maxRecordDiagnosticRetained', len(mr) == 1 and 'diagName' in mr[0], entry=mr[0] if mr else None)
        inh = [r['check'] for r in rerun if r['check'].startswith('inherited012.')]
        own_check('coverage.inheritedChainPresent', 'inherited012.summary' in inh and any(c.startswith('inherited012.inherited011.') for c in inh)
                  and any('inherited010.' in c for c in inh), entries=len(inh))
        own_record('coverage.counts', 'recorded', e4Expected=len(e4_want), e5Expected=len(e5_want), rerunEntries=len(rerun),
                   inherited012Entries=len(inh), byStatus={k: sum(r['status'] == k for r in rerun) for k in CORE_STATUS})
        for r in rerun:
            r = dict(r)
            r['check'] = 'rerun013.' + r['check']
            results.append(r)
    failed = [r for r in results if r['status'] == 'FAIL']
    out = {'package': 'M1 draft 0.15 (complete C26 repair of the diagnostic logging boundary; full E4/E5 and inherited re-run)',
           'executedAtUtc': datetime.now(timezone.utc).isoformat(timespec='seconds'),
           'environment': {'python': sys.version.split()[0], 'executable': sys.executable, 'platform': platform.platform(),
                           'thirdPartyPackages': 'none'},
           'statusSchema': {'coreStatus': list(CORE_STATUS), 'diagnosticRenames': RENAMED},
           'evidenceKind': 'reference models of the specification text; 0.13 and nested runners re-run with collision-free record/check',
           'notExecuted': ['BR22c (Chrome + CDP)', 'TypeScript implementation', 'chrome.storage behaviour', 'JS Number / TextEncoder behaviour',
                           'E6/E7 (AdminDelete, AdminSlots, sites, TombReaper)'],
           'summary': {'checks': len(results), 'passed': sum(r['status'] == 'pass' for r in results),
                       'recorded': sum(r['status'] == 'recorded' for r in results), 'failed': len(failed),
                       'seconds': round(time.time() - t0, 1)},
           'results': results}
    RES.mkdir(parents=True, exist_ok=True)
    (RES / 'run-results-0.15.json').write_text(json.dumps(out, indent=2, ensure_ascii=True, default=str) + '\n', encoding='utf-8')
    print(json.dumps(out['summary']))
    for f in failed:
        print('FAIL', json.dumps(f, ensure_ascii=True, default=str)[:400])
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
