"""Reference-only runner for M1 draft 0.14: the C26 checker-only repair of 0.13.
Python standard library only. NOT executed by the author.

Usage, from D:\\PoCol-Development:
  coordination\\runtime\\python311\\python.exe m1-draft-0.14\\tools\\run_checks_014.py
Writes only m1-draft-0.14/results/run-results-0.14.json. No older file is edited or written:
0.13's runner is loaded as a module and its main() is never called.

C26 (coordination/review-001/REVIEW-0.13.md). run_checks_013.py:165 calls
    record('e4.surrogateCollision.status', 'recorded', status=sc['status'])
against `def record(name, status, **kw)`. Python raises TypeError (two values for
'status') before the function body runs, so e4_checks stopped there. The later parse,
base64, value, encode, maximum-size and disk checks never ran.

Repair (in memory only). The 0.13 module-global `record` is replaced by `safe_record`, whose
signature takes the core status positionally only (`def safe_record(name, core_status, /,
**diag)`). Any DIAGNOSTIC keyword that would collide with a core result field is renamed:
`status` becomes `proposalStatus` and `check` becomes `diagCheck`. The core field
`status` is set LAST, from the positional argument, and must be in {pass, recorded, FAIL}.
So a diagnostic can never overwrite the result status.

Not changed: expected values, codec, engine, schedules, vectors, CR-E4-01/02, the Unicode
and u64 assumptions, storage policies, gates, slots, budgets, seq, scheduler, owner decisions.
"""

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
RENAMED = {'status': 'proposalStatus', 'check': 'diagCheck'}

results = []
INPUTS = [
    'coordination/review-001/REVIEW-0.13.md',
    'm1-draft-0.13/tools/run_checks_013.py', 'm1-draft-0.13/tools/fmt2_codec.py', 'm1-draft-0.13/tools/recovery_ref.py',
    'm1-draft-0.13/vectors/e4-units.json', 'm1-draft-0.13/vectors/e5-schedules.json',
    'm1-draft-0.13/M1-SPEC-0.13-AMENDMENT.md', 'm1-draft-0.13/annex/E4-E5-RECOVERY.md',
    'm1-draft-0.12/tools/sr_ref.py', 'm1-draft-0.12/tools/run_checks_012.py',
    'm1-draft-0.14/tools/run_checks_014.py',
]


def own(name, core_status, **diag):
    """This runner's own entries use the same safe shape."""
    results.append(make_entry(name, core_status, diag))


def make_entry(name, core_status, diag):
    if core_status not in CORE_STATUS:
        raise ValueError('core status must be one of %s, got %r' % (CORE_STATUS, core_status))
    entry = {}
    for k, v in diag.items():
        entry[RENAMED.get(k, k)] = v
    entry['check'] = name
    entry['status'] = core_status            # set last: nothing can overwrite it
    return entry


def ok(name, cond, **diag):
    own(name, 'pass' if cond else 'FAIL', **diag)
    return cond


# ------------------------------------------------------------------ load 0.13 runner read-only

def load_r13():
    path = ROOT / 'm1-draft-0.13' / 'tools' / 'run_checks_013.py'
    spec = importlib.util.spec_from_file_location('run_checks_013_rerun', str(path))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)          # imports 0.12 sr_ref, fmt2_codec, recovery_ref; main() NOT called
    return mod


def install_safe_record(R13):
    original = R13.record
    sink = R13.results

    def safe_record(name, core_status, /, **diag):
        sink.append(make_entry(name, core_status, diag))
    R13.record = safe_record              # check() and every step resolve `record` through module globals
    return original, safe_record


def c26_unit(original, safe_record):
    """C26 reproduced on the original function, and the repair's behaviour shown."""
    try:
        original('probe.c26', 'recorded', status='open')
        raised = None
    except TypeError as e:
        raised = str(e)
    ok('c26.preserved.originalRecordRaisesTypeError', raised is not None, error=raised)
    probe = []
    entry = make_entry('probe.c26', 'recorded', {'status': 'open', 'check': 'diag', 'x': 1})
    probe.append(entry)
    ok('c26.repair.diagnosticRenamedCoreKept', entry['status'] == 'recorded' and entry['proposalStatus'] == 'open'
       and entry['check'] == 'probe.c26' and entry['diagCheck'] == 'diag' and entry['x'] == 1, entry=entry)
    try:
        make_entry('probe.bad', 'open', {})
        rejected = False
    except ValueError:
        rejected = True
    ok('c26.repair.nonCoreStatusRejected', rejected)
    ok('c26.repair.signaturePositionalOnly', safe_record.__code__.co_posonlyargcount == 2)


def input_hashes():
    for rel in INPUTS:
        p = ROOT / rel
        own('input.sha256 ' + rel, 'recorded', exists=p.exists(), sha256=hashlib.sha256(p.read_bytes()).hexdigest() if p.exists() else None)
    prior = ROOT / 'm1-draft-0.13' / 'results' / 'run-results-0.13.json'
    own('preserved.0.13resultFilePresent', 'recorded', exists=prior.exists(),
        note='independent 0.13 run: 409 entries, 309 pass, 99 recorded, 1 FAIL (REVIEW-0.13.md); not overwritten')


# ------------------------------------------------------------------ coverage expectations (from the 0.13 vectors)

def expected_e4_names(u):
    names = ['provenance e4.source%d' % i for i in range(len(u['sources']))] + ['provenance e4.disk', 'e4.constants']
    names += ['e4.knownAnswer ' + k['id'] for k in u['knownAnswers']]
    names += ['e4.serializeReject ' + r['id'] for r in u['serializeRejects']]
    names += ['e4.surrogateCollision', 'e4.surrogateCollision.status']
    names += ['e4.parseReject ' + r['id'] for r in u['parseRejects']]
    names += ['e4.base64Reject ' + r['id'] for r in u['base64Rejects']]
    names += ['e4.value ' + r['id'] for r in u['valueChecks']]
    names += ['e4.encodeReject ' + r['id'] for r in u['encodeRejects']]
    names += ['e4.maxRecord']
    names += ['e4.disk ' + a['id'] for a in u['disk']['accept']]
    names += ['e4.disk.ticketLifecycle']
    return names


def expected_e5_names(s):
    names = []
    for c in s['cases']:
        names += ['provenance e5.' + c['id'], 'e5.case ' + c['id'], 'e5.recorded ' + c['id']]
    names += ['e5.control %s on %s' % (c['fault'], c['case']) for c in s['controls']]
    return names


def main():
    t0 = time.time()
    input_hashes()
    R13 = None
    try:
        R13 = load_r13()
        original, safe = install_safe_record(R13)
        ok('wiring.r13RecordPatched', R13.record is safe and R13.check.__globals__['record'] is safe)
        c26_unit(original, safe)
    except Exception as e:                                                  # record, never hide
        ok('step.completed load_r13', False, exception='%s: %s' % (type(e).__name__, e))
    if R13 is not None:
        for name in ('input_hashes', 'wiring', 'e4_checks', 'e5_checks', 'inherited_012'):
            try:
                getattr(R13, name)()
            except Exception as e:                                          # record, never hide
                R13.check('step.completed ' + name, False, exception='%s: %s' % (type(e).__name__, e))
        rerun = R13.results
        bad_status = [r for r in rerun if r.get('status') not in CORE_STATUS]
        ok('schema.coreStatusEnumOnly', not bad_status, offending=[r.get('check') for r in bad_status][:10])
        step_failures = [r['check'] for r in rerun if 'step.completed' in r['check']]      # includes nested inherited steps
        ok('schema.noStepAborted', not step_failures, aborted=step_failures)
        have = {r['check'] for r in rerun}
        u = json.loads((ROOT / 'm1-draft-0.13' / 'vectors' / 'e4-units.json').read_text(encoding='utf-8'))
        s = json.loads((ROOT / 'm1-draft-0.13' / 'vectors' / 'e5-schedules.json').read_text(encoding='utf-8'))
        e4_want, e5_want = expected_e4_names(u), expected_e5_names(s)
        e4_missing = [n for n in e4_want if n not in have]
        e5_missing = [n for n in e5_want if n not in have]
        ok('coverage.e4AllExpectedChecksPresent', not e4_missing, expected=len(e4_want), missing=e4_missing)
        ok('coverage.e5AllExpectedChecksPresent', not e5_missing, expected=len(e5_want), missing=e5_missing)
        sc = [r for r in rerun if r['check'] == 'e4.surrogateCollision.status']
        ok('coverage.surrogateStatusIsRecordedWithProposalStatus', len(sc) == 1 and sc[0]['status'] == 'recorded'
           and 'proposalStatus' in sc[0], entry=sc[0] if sc else None)
        inh = [r for r in rerun if r['check'].startswith('inherited012.')]
        ok('coverage.inherited012Present', any(r['check'] == 'inherited012.summary' for r in inh) and len(inh) > 1, entries=len(inh))
        own('coverage.counts', 'recorded', e4Expected=len(e4_want), e5Expected=len(e5_want), inherited012Entries=len(inh),
            rerunEntries=len(rerun), byStatus={k: sum(r['status'] == k for r in rerun) for k in CORE_STATUS})
        for r in rerun:
            r = dict(r)
            r['check'] = 'rerun013.' + r['check']
            results.append(r)
    failed = [r for r in results if r['status'] == 'FAIL']
    out = {'package': 'M1 draft 0.14 (C26 checker-only repair of 0.13; full E4/E5 and inherited re-run)',
           'executedAtUtc': datetime.now(timezone.utc).isoformat(timespec='seconds'),
           'environment': {'python': sys.version.split()[0], 'executable': sys.executable, 'platform': platform.platform(),
                           'thirdPartyPackages': 'none'},
           'statusSchema': {'coreStatus': list(CORE_STATUS), 'diagnosticRenames': RENAMED},
           'evidenceKind': 'reference models of the specification text; 0.13 checks re-run with a safe result recorder',
           'notExecuted': ['BR22c (Chrome + CDP)', 'TypeScript implementation', 'chrome.storage behaviour', 'JS Number / TextEncoder behaviour',
                           'E6/E7 (AdminDelete, AdminSlots, sites, TombReaper)'],
           'summary': {'checks': len(results), 'passed': sum(r['status'] == 'pass' for r in results),
                       'recorded': sum(r['status'] == 'recorded' for r in results), 'failed': len(failed),
                       'seconds': round(time.time() - t0, 1)},
           'results': results}
    RES.mkdir(parents=True, exist_ok=True)
    (RES / 'run-results-0.14.json').write_text(json.dumps(out, indent=2, ensure_ascii=True, default=str) + '\n', encoding='utf-8')
    print(json.dumps(out['summary']))
    for f in failed:
        print('FAIL', json.dumps(f, ensure_ascii=True, default=str)[:400])
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
