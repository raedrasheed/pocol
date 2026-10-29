"""Reference-only runner for M1 draft 0.12 (C25 repair: physical epoch-namespace accounting).
Python standard library only. NOT executed by the author.

Usage, from D:\\PoCol-Development:
  coordination\\runtime\\python311\\python.exe m1-draft-0.12\\tools\\run_checks_012.py
Writes only m1-draft-0.12/results/run-results-0.12.json. Earlier runners are imported as
modules; their main() is never called, so they write nothing.

Steps:
1. Input SHA-256 values (recorded).
2. Wiring: the repaired World really uses the raw-namespace gate and invariant.
3. C25 reproduction: the review probe against an unpatched 0.11 must reproduce 9 physical
   names, a confirmed state and no violation; the repaired model must block before any set.
4. New namespace cases N1-N5b, judged against the repaired expectations and, separately, the
   pristine-0.11 defect expectations. The raw namespace count is measured by this runner's
   own instrumentation (a subclass hook), not taken from the model's statistic.
5. The full 0.11 suite re-run with the repaired module bound as `sr_ref`. That is: the 24 C24
   probes; preserved pristine-0.10 failures; names, generators and values; the World
   regressions, with ONE amended expectation (W-A2, see vectors/c25-namespace.json); and,
   through it, the entire 0.10 suite (E1, BR22h h1-h6 with 2 x 729 h5, BR22b, retry/sweep,
   520 BR22a combinations, all legacy controls). Results are prefixed 'inherited011.'.
"""

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
sys.path.insert(0, str(HERE))

import sr_ref as SR                      # noqa: E402  (0.12 repaired module, bound as 'sr_ref')

results = []
INPUTS = [
    'reference/browser.md', 'reference/validation.md',
    'm1-draft-0.10/tools/sr_ref.py', 'm1-draft-0.10/tools/run_checks_010.py',
    'm1-draft-0.11/tools/sr_ref.py', 'm1-draft-0.11/tools/run_checks_011.py', 'm1-draft-0.11/vectors/c24-regressions.json',
    'coordination/review-001/REVIEW-0.11.md', 'coordination/review-001/namespace-independent-probe-0.11.py',
    'coordination/review-001/namespace-independent-probe-0.11.json',
    'm1-draft-0.12/tools/sr_ref.py', 'm1-draft-0.12/tools/run_checks_012.py', 'm1-draft-0.12/vectors/c25-namespace.json',
]
PREFIX = 'pocol:epoch:'


def record(name, status, **kw):
    r = {'check': name, 'status': status}
    r.update(kw)
    results.append(r)


def check(name, ok, **kw):
    record(name, 'pass' if ok else 'FAIL', **kw)
    return ok


def raw_names(items):
    return [n for n in items if type(n) is str and n.startswith(PREFIX)]


def instrumented(S):
    """A World subclass that measures the raw namespace after every event, independently of
    the model's own statistic."""
    class W(S.World):
        def __init__(self, *a, **kw):
            super().__init__(*a, **kw)
            self.raw_seen_max = len(raw_names(self.be.items))

        def _observe(self):
            super()._observe()
            self.raw_seen_max = max(self.raw_seen_max, len(raw_names(self.be.items)))
    return W


def run(S, items, events, until):
    w = instrumented(S)(items=copy.deepcopy(items))
    w.load(events)
    w.run(until)
    return w


def input_hashes():
    for rel in INPUTS:
        p = ROOT / rel
        record('input.sha256 ' + rel, 'recorded', exists=p.exists(),
               sha256=hashlib.sha256(p.read_bytes()).hexdigest() if p.exists() else None)
    prior = ROOT / 'coordination' / 'review-001' / 'namespace-independent-probe-0.11.json'
    if prior.exists():
        record('preserved.namespaceProbe0.11', 'recorded', result=json.loads(prior.read_text(encoding='utf-8')))


def provenance(src):
    lines = (ROOT / src['file']).read_text(encoding='utf-8').splitlines()
    a, b = src['lines']
    seg = '\n'.join(lines[a - 1:b])
    missing = [x for x in src['literals'] if x not in seg]
    check('provenance c25 ' + src['file'], not missing, lines=[a, b], missing=missing)


def wiring():
    sc = SR.self_check()
    check('wiring.c25', sc['attemptUsesRawNamespace'] and sc['observeUsesRawNamespace'] and sc['initRecordsInitialRaw']
          and sc['worldEpochsIsStrict'] and sc['lookupGlobalsPatched'], selfCheck=sc)
    check('wiring.importedAsSrRef', sys.modules.get('sr_ref') is SR)


def sets_by(w, gid, cat):
    return [op for op in w.be.ops if op.gen == gid and op.kind == 'set' and op.cat == cat]


def c25_repro(doc, pristine011):
    p = doc['reviewProbe']
    items = {SR.epoch_name(5): {'bootMs': -200000}}
    items.update({SR.epoch_name(100 + i) + '\n': {'bootMs': -200000} for i in range(7)})
    ev = [{'t': 0, 'do': 'boot', 'gen': 1, 'cfg': {}}]
    wp = run(pristine011, items, ev, p['runUntil'])
    got_p = {'physical': len(raw_names(wp.be.items)), 'state': wp.gens[1].state, 'violations': wp.violations}
    check('c25.preserved.pristine011ReproducesC25', got_p == p['pristine011Expect'], got=got_p)
    w = run(SR, items, ev, p['runUntil'])
    got = {'physicalAtMost': w.raw_seen_max <= 8, 'state': w.gens[1].state, 'fenceSets': len(sets_by(w, 1, 'fence'))}
    check('c25.repaired.reviewProbeBlocksBeforeSet', got == {'physicalAtMost': True, 'state': p['expect']['state'], 'fenceSets': 0},
          got=got, rawSeenMax=w.raw_seen_max)


def observe(w, want, items0):
    gs = w.gen_summary()
    got = {}
    for f, v in want.items():
        if f.startswith('gen'):
            g = gs.get(int(f[3:]), {})
            got[f] = {k: g.get(k) for k in v}
        elif f in ('epochGateBlocks', 'action25'):
            got[f] = w.stats[f]
        elif f == 'fenceSets':
            got[f] = len(sets_by(w, 1, 'fence'))
        elif f == 'recordSets':
            got[f] = len(sets_by(w, 1, 'record'))
        elif f == 'sweepRemoved':
            sw = [x for x in w.trace if x['ev'] == 'sweep' and x['gen'] == 1]
            got[f] = sw[0]['removed'] if sw else None
        elif f == 'rawMax':
            got[f] = w.raw_seen_max
        elif f == 'rawFinal':
            got[f] = len(raw_names(w.be.items))
        elif f == 'malformedPreserved':
            got[f] = sum(1 for n in raw_names(items0) if SR.parse_epoch_name(n) is None and n in w.be.items)
        elif f == 'lookalikesPreserved':
            got[f] = sum(1 for n in items0 if not n.startswith(PREFIX) and 'epoch' in n.lower() and n in w.be.items)
        elif f == 'replies':
            out = []
            for r in w.replies:
                t = [r['id'], r['code'], r['reason'], r['t']]
                if r['detail'] is not None:
                    t.append(r['detail'])
                out.append(t)
            got[f] = out
        elif f == 'snapshots':
            got[f] = len(w.snapshots)
        elif f == 'violations':
            got[f] = [x['kind'] for x in w.violations]
        elif f == 'reports':
            got[f] = [x['kind'] for x in getattr(w, 'reports', [])]
        else:
            got[f] = '(unknown field)'
    return got


def namespace_cases(doc, pristine011):
    for c in doc['cases']:
        w = run(SR, c['items'], c['events'], c['runUntil'])
        got = observe(w, c['expect'], c['items'])
        check('c25.case ' + c['id'], got == c['expect'], got=got, want=c['expect'])
        check('c25.case.rawNeverAboveMaxUnlessInitial ' + c['id'],
              w.raw_seen_max <= max(8, len(raw_names(c['items']))), rawSeenMax=w.raw_seen_max)
        wp = run(pristine011, c['items'], c['events'], c['runUntil'])
        gotp = observe(wp, c['pristine011Expect'], c['items'])
        check('c25.preserved.pristine011 ' + c['id'], gotp == c['pristine011Expect'], got=gotp, want=c['pristine011Expect'])


def inherited_011(doc):
    path = ROOT / 'm1-draft-0.11' / 'tools' / 'run_checks_011.py'
    spec = importlib.util.spec_from_file_location('run_checks_011_rerun', str(path))
    R11 = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(R11)          # its 'import sr_ref' resolves to this 0.12 module
    check('inherited011.usesRepairedModule', R11.SR is SR)
    reg = json.loads((ROOT / 'm1-draft-0.11' / 'vectors' / 'c24-regressions.json').read_text(encoding='utf-8'))
    amend = doc['amendedInherited011']
    reg_amended = copy.deepcopy(reg)
    hits = [w for w in reg_amended['world'] if w['id'].startswith('W-A2')]
    ok = len(hits) == 1 and hits[0]['expect'] == amend['oldExpect']
    check('inherited011.amendment.oldExpectationMatchesFile', ok, old=hits[0]['expect'] if hits else None)
    if hits:
        hits[0]['expect'] = amend['newExpect']
    record('inherited011.amendment', 'recorded', fixture=amend['fixture'], old=amend['oldExpect'], new=amend['newExpect'], why=amend['why'])
    # The old expectation must now FAIL on the repaired model (demonstrates the change is real).
    w_old = [w for w in reg['world'] if w['id'].startswith('W-A2')][0]
    w = run(SR, w_old['items'], w_old['events'], w_old['runUntil'])
    g = w.gen_summary()[1]
    check('inherited011.amendment.oldExpectationNowRejected', {'E': g['E'], 'EN': g['EN'], 'state': g['state']} != amend['oldExpect']['gen1'],
          got={'E': g['E'], 'EN': g['EN'], 'state': g['state']})
    pristine010 = SR.load_pristine_010()
    t0 = time.time()
    steps = [('input_hashes', R11.input_hashes), ('wiring', R11.wiring),
             ('probe_checks', lambda: R11.probe_checks(reg, pristine010)), ('name_checks', lambda: R11.name_checks(reg)),
             ('value_checks', lambda: R11.value_checks(reg, pristine010)), ('world_checks', lambda: R11.world_checks(reg_amended, pristine010)),
             ('inherited_suite', R11.inherited_suite)]
    for name, step in steps:
        try:
            step()
        except Exception as e:                                            # record, never hide
            R11.check('step.completed ' + name, False, exception='%s: %s' % (type(e).__name__, e))
    for r in R11.results:
        r = dict(r)
        r['check'] = 'inherited011.' + r['check']
        results.append(r)
    st = [r['status'] for r in R11.results]
    record('inherited011.summary', 'recorded', checks=len(st), passed=st.count('pass'), recorded=st.count('recorded'), failed=st.count('FAIL'),
           seconds=round(time.time() - t0, 1), note='0.11 independent reference: 259 entries, 211 pass, 48 recorded, 0 fail (REVIEW-0.11.md)')


def main():
    t0 = time.time()
    doc = json.loads((PKG / 'vectors' / 'c25-namespace.json').read_text(encoding='utf-8'))
    pristine011 = SR.load_pristine_011()
    steps = [('input_hashes', input_hashes), ('provenance', lambda: provenance(doc['source'])), ('wiring', wiring),
             ('c25_repro', lambda: c25_repro(doc, pristine011)), ('namespace_cases', lambda: namespace_cases(doc, pristine011)),
             ('inherited_011', lambda: inherited_011(doc))]
    for name, step in steps:
        try:
            step()
        except Exception as e:                                            # record, never hide
            check('step.completed ' + name, False, exception='%s: %s' % (type(e).__name__, e))
    failed = [r for r in results if r['status'] == 'FAIL']
    out = {'package': 'M1 draft 0.12 (C25 repair: physical epoch-namespace accounting, on top of the C24 repair)',
           'executedAtUtc': datetime.now(timezone.utc).isoformat(timespec='seconds'),
           'environment': {'python': sys.version.split()[0], 'executable': sys.executable, 'platform': platform.platform(),
                           'thirdPartyPackages': 'none'},
           'evidenceKind': 'reference model of the specification text; full 0.11 (and so 0.10) suite re-run in memory with one documented amendment',
           'notExecuted': ['BR22c (Chrome + CDP; A15c/A15d)', 'TypeScript implementation', 'chrome.storage.local', 'fmt 2 wire codec (E4)',
                           'D104 disk bounds (E4/E6)', 'browser / MV3 extension'],
           'summary': {'checks': len(results), 'passed': sum(r['status'] == 'pass' for r in results),
                       'recorded': sum(r['status'] == 'recorded' for r in results), 'failed': len(failed),
                       'seconds': round(time.time() - t0, 1)},
           'results': results}
    RES.mkdir(parents=True, exist_ok=True)
    (RES / 'run-results-0.12.json').write_text(json.dumps(out, indent=2, ensure_ascii=True, default=str) + '\n', encoding='utf-8')
    print(json.dumps(out['summary']))
    for f in failed:
        print('FAIL', json.dumps(f, ensure_ascii=True, default=str)[:400])
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
