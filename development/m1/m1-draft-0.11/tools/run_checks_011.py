"""Reference-only runner for M1 draft 0.11 (C24 repair of 0.10). Python standard library only.
NOT executed by the author.

Usage, from D:\\PoCol-Development:
  coordination\\runtime\\python311\\python.exe m1-draft-0.11\\tools\\run_checks_011.py
Writes only m1-draft-0.11/results/run-results-0.11.json. Nothing in 0.10 is written.

Steps:
1. Input SHA-256 values (0.10 package, review evidence, baseline). Recorded.
2. Wiring: the repaired module is the one the 0.10 suite and World actually use.
3. Preserved evidence: the 24 independent C24 probes run against an UNPATCHED copy of 0.10.
   Exactly the 7 recorded failures must reproduce, and the 17 passes must still pass.
4. The same 24 probes against the repaired module: all must pass.
5. The ENTIRE 0.10 suite, re-run in this process with the repaired module bound as `sr_ref`:
   E1 vectors; BR22h h1-h6 including 2 x 729 h5 assignments; BR22b, retry and sweep;
   all 520 BR22a combinations; every legacy negative control. Results are copied with the
   prefix 'inherited010.'. The 0.10 runner's main() is never called, so it writes nothing.
6. New C24 regressions: strict names, generator types, value types and ranges, and
   World-level scenarios (fence over malformed names, bool bootMs in the window, recovery
   over corrupt numeric fields and malformed record names). The pristine outcomes are
   recorded and checked against their hand-written defect expectations.
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
sys.path.insert(0, str(HERE))

import sr_ref as SR                      # noqa: E402  (0.11 repaired module, bound as 'sr_ref')

results = []
INPUTS = [
    'reference/browser.md', 'reference/validation.md', 'reference/threat.md',
    'm1-draft-0.10/tools/sr_ref.py', 'm1-draft-0.10/tools/run_checks_010.py',
    'm1-draft-0.10/vectors/e1-units.json', 'm1-draft-0.10/vectors/e2-fence-cases.json', 'm1-draft-0.10/vectors/e3-br22a.json',
    'm1-draft-0.10/M1-SPEC-0.10-AMENDMENT.md', 'm1-draft-0.10/M1-STATUS-0.10.md',
    'coordination/review-001/REVIEW-0.10.md', 'coordination/review-001/epoch-independent-probes-0.10.py',
    'coordination/review-001/epoch-independent-probes-0.10.json', 'coordination/review-001/epoch-name-oracle.json',
    'coordination/review-001/epoch-name-oracle.mjs',
    'm1-draft-0.11/tools/sr_ref.py', 'm1-draft-0.11/tools/run_checks_011.py', 'm1-draft-0.11/vectors/c24-regressions.json',
]


def record(name, status, **kw):
    r = {'check': name, 'status': status}
    r.update(kw)
    results.append(r)


def check(name, ok, **kw):
    record(name, 'pass' if ok else 'FAIL', **kw)
    return ok


def input_hashes():
    for rel in INPUTS:
        p = ROOT / rel
        record('input.sha256 ' + rel, 'recorded', exists=p.exists(),
               sha256=hashlib.sha256(p.read_bytes()).hexdigest() if p.exists() else None)
    prior = ROOT / 'coordination' / 'review-001' / 'epoch-independent-probes-0.10.json'
    if prior.exists():
        j = json.loads(prior.read_text(encoding='utf-8'))
        record('preserved.independentProbes0.10', 'recorded', summary=j.get('summary'),
               failed=[r['check'] for r in j.get('results', []) if not r.get('passed')])


def wiring():
    sc = SR.self_check()
    check('wiring.repairedHelpersInstalled', all(v for k, v in sc.items() if k != 'sourceFile'), selfCheck=sc)
    check('wiring.importedAsSrRef', sys.modules.get('sr_ref') is SR)


# ------------------------------------------------------------------ the 24 independent probes

def probes(S, oracle_cases, tails):
    rows = []

    def add(name, got, want):
        rows.append({'check': name, 'passed': got == want, 'actual': got, 'expected': want})

    for c in oracle_cases:
        try:
            got = S.epoch_name(int(c['epoch'])) if 'seq' not in c else S.record_name('N', 'A', int(c['epoch']), int(c['seq']))
        except Exception as e:                                            # record, never hide
            got = 'raised %s' % type(e).__name__
        add('Native Node BigInt ' + c['expected'], got, c['expected'])
    en, rn = S.epoch_name(6), S.record_name('N', 'A', 6, 0)
    for tail in tails:
        add('Strict epoch tail ' + repr(tail), S.parse_epoch_name(en + tail), None)
        got = S.parse_record_name(rn + tail, 'N', 'A')
        add('Strict record tail ' + repr(tail), list(got) if got else None, None)
    for name, fn in [('bool epoch', lambda: S.epoch_name(True)), ('bool sequence', lambda: S.record_name('N', 'A', 1, True))]:
        try:
            fn()
            got = 'accepted'
        except (ValueError, TypeError):
            got = 'refused'
        add(name, got, 'refused')
    add('bool bootMs invalid', S.epoch_value_ok({'bootMs': True}), False)
    v = S.record_value(1, 0, {'k': 'v'})
    v['epoch'] = True
    add('bool epoch value invalid', S.valid_record((1, 0), v), False)
    v = S.record_value(1, 1, {'k': 'v'})
    v['seq'] = True
    add('bool seq value invalid', S.valid_record((1, 1), v), False)
    return rows


def probe_checks(reg, pristine):
    ip = reg['independentProbes']
    oracle_file = ROOT / ip['source']['oracle']
    file_cases = json.loads(oracle_file.read_text(encoding='utf-8'))['cases'] if oracle_file.exists() else None
    check('c24.nodeOracleCasesMatchReviewFile', file_cases == ip['nodeOracleCases'], file=ip['source']['oracle'])
    rows_p = probes(pristine, ip['nodeOracleCases'], ip['tails'])
    failed_p = [r['check'] for r in rows_p if not r['passed']]
    check('c24.preserved.pristineReproducesExactly7Failures', len(rows_p) == ip['checks'] and sorted(failed_p) == sorted(ip['pristineFailing']),
          failed=failed_p, total=len(rows_p))
    record('c24.preserved.pristineProbeRows', 'recorded', rows=rows_p)
    rows_r = probes(SR, ip['nodeOracleCases'], ip['tails'])
    failed_r = [r['check'] for r in rows_r if not r['passed']]
    check('c24.repaired.all24ProbesPass', len(rows_r) == ip['checks'] and not failed_r, failed=failed_r)
    for r in rows_r:
        check('c24.repaired.probe ' + r['check'], r['passed'], actual=r['actual'], expected=r['expected'])


# ------------------------------------------------------------------ new regressions

def _lit(expr):
    """Tiny literal parser for the generator vectors (no eval)."""
    table = {'True': True, 'False': False}
    if expr in table:
        return table[expr]
    if expr.startswith("'") and expr.endswith("'"):
        return expr[1:-1]
    if expr.startswith('2**'):
        base, _, rest = expr[3:].partition('-')
        return 2 ** int(base) - (int(rest) if rest else 0)
    if '.' in expr:
        return float(expr)
    return int(expr)


def name_checks(reg):
    n = reg['names']
    for s in n['epochReject']:
        check('c24.names.epochReject %r' % s, SR.parse_epoch_name(s) is None, got=SR.parse_epoch_name(s))
    for c in n['epochAccept']:
        check('c24.names.epochAccept ' + c['name'], SR.parse_epoch_name(c['name']) == c['E'])
    for s in n['recordReject']:
        got = SR.parse_record_name(s, 'N', 'A')
        check('c24.names.recordReject %r' % s, got is None, got=got)
    for c in n['recordAccept']:
        got = SR.parse_record_name(c['name'], 'N', 'A')
        check('c24.names.recordAccept ' + c['name'], list(got or []) == c['version'])
    g = reg['generators']
    for c in g['epochNameReject']:
        try:
            SR.epoch_name(_lit(c['E']))
            ok = False
        except (ValueError, TypeError):
            ok = True
        check('c24.gen.epochNameReject %s' % c['E'], ok)
    for c in g['epochNameAccept']:
        check('c24.gen.epochNameAccept %s' % c['E'], SR.epoch_name(_lit(c['E'])) == c['name'])
    for c in g['recordNameReject']:
        try:
            SR.record_name('N', 'A', _lit(c['E']), _lit(c['seq']))
            ok = False
        except (ValueError, TypeError):
            ok = True
        check('c24.gen.recordNameReject %s/%s' % (c['E'], c['seq']), ok)
    for c in g['recordNameAccept']:
        check('c24.gen.recordNameAccept %s/%s' % (c['E'], c['seq']), SR.record_name('N', 'A', _lit(c['E']), _lit(c['seq'])) == c['name'])


def value_checks(reg, pristine):
    for c in reg['values']['epochValue']:
        check('c24.values.epochValue %s' % json.dumps(c['v']), SR.epoch_value_ok(c['v']) == c['ok'], got=SR.epoch_value_ok(c['v']))
    for c in reg['values']['record']:
        ver = tuple(c['ver'])
        check('c24.values.record ' + c['id'], SR.valid_record(ver, c['v']) == c['ok'])
        if 'pristine' in c:
            got = pristine.valid_record(ver, c['v'])
            check('c24.preserved.pristineRecord ' + c['id'], got == c['pristine'], got=got)


def world_run(S, w_case):
    w = S.World(items=w_case['items'])
    w.load(w_case['events'])
    w.run(w_case['runUntil'])
    return w


def world_observe(w, want):
    gs = w.gen_summary()
    got = {}
    for f, v in want.items():
        if f.startswith('gen'):
            g = gs.get(int(f[3:]), {})
            got[f] = {k: g.get(k) for k in v}
        elif f == 'epochGateBlocks':
            got[f] = w.stats['epochGateBlocks']
        elif f == 'snapshots':
            got[f] = [{'t': s['t'], 'dict': s['dict']} for s in w.snapshots]
        elif f == 'readVersion':
            r = [x for x in w.trace if x['ev'] == 'read']
            got[f] = r[0]['version'] if r else None
        elif f == 'violations':
            got[f] = [x['kind'] for x in w.violations]
        else:
            got[f] = '(unknown field)'
    return got


def world_checks(reg, pristine):
    for c in reg['world']:
        if 'lookup' in c:
            for S, key, label in ((SR, 'expect', 'repaired'), (pristine, 'pristineExpect', 'pristine')):
                r = S.lookup(c['items'], 'n1', '0xa1')
                want = c['lookup'][key]
                got = {k: (list(r[k]) if isinstance(r.get(k), tuple) else r.get(k)) for k in want}
                check('c24.world.lookup.%s %s' % (label, c['id']), got == want, got=got, want=want)
        w = world_run(SR, c)
        got = world_observe(w, c['expect'])
        check('c24.world.repaired ' + c['id'], got == c['expect'], got=got, want=c['expect'])
        wp = world_run(pristine, c)
        gotp = world_observe(wp, c['pristineExpect'])
        check('c24.preserved.world.pristine ' + c['id'], gotp == c['pristineExpect'], got=gotp, want=c['pristineExpect'])


# ------------------------------------------------------------------ the full 0.10 suite, re-run

def inherited_suite():
    path = ROOT / 'm1-draft-0.10' / 'tools' / 'run_checks_010.py'
    spec = importlib.util.spec_from_file_location('run_checks_010_rerun', str(path))
    R10 = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(R10)          # its 'import sr_ref' resolves to sys.modules['sr_ref'] = this repaired module
    check('inherited.usesRepairedModule', R10.SR is SR and R10.SR.World.epochs is SR.BASE_MODULE.World.epochs
          and SR.self_check()['worldEpochsIsStrict'])
    t0 = time.time()
    for step in (R10.input_hashes, R10.e1_checks, R10.e2_checks, R10.e3_checks):
        try:
            step()
        except Exception as e:                                            # record, never hide
            R10.check('step.completed ' + step.__name__, False, exception='%s: %s' % (type(e).__name__, e))
    for r in R10.results:
        r = dict(r)
        r['check'] = 'inherited010.' + r['check']
        results.append(r)
    inh = [r for r in R10.results]
    record('inherited010.summary', 'recorded', checks=len(inh), passed=sum(r['status'] == 'pass' for r in inh),
           recorded=sum(r['status'] == 'recorded' for r in inh), failed=sum(r['status'] == 'FAIL' for r in inh),
           seconds=round(time.time() - t0, 1), note='0.10 author-run reference: 113 entries, 86 pass, 27 recorded, 0 fail (REVIEW-0.10.md)')


def main():
    t0 = time.time()
    reg = json.loads((PKG / 'vectors' / 'c24-regressions.json').read_text(encoding='utf-8'))
    pristine = SR.load_pristine_010()
    steps = [input_hashes, wiring, lambda: probe_checks(reg, pristine), lambda: name_checks(reg),
             lambda: value_checks(reg, pristine), lambda: world_checks(reg, pristine), inherited_suite]
    names = ['input_hashes', 'wiring', 'probe_checks', 'name_checks', 'value_checks', 'world_checks', 'inherited_suite']
    for name, step in zip(names, steps):
        try:
            step()
        except Exception as e:                                            # record, never hide
            check('step.completed ' + name, False, exception='%s: %s' % (type(e).__name__, e))
    failed = [r for r in results if r['status'] == 'FAIL']
    out = {'package': 'M1 draft 0.11 (C24 repair of 0.10: strict names and integer fields)',
           'executedAtUtc': datetime.now(timezone.utc).isoformat(timespec='seconds'),
           'environment': {'python': sys.version.split()[0], 'executable': sys.executable, 'platform': platform.platform(),
                           'thirdPartyPackages': 'none'},
           'evidenceKind': 'reference model of the specification text; the full 0.10 suite re-run with the repaired helpers, plus C24 regressions',
           'notExecuted': ['BR22c (Chrome + CDP; A15c/A15d)', 'TypeScript implementation', 'chrome.storage.local', 'fmt 2 wire codec (E4)',
                           'D104 disk bounds (E4/E6)', 'browser / MV3 extension'],
           'summary': {'checks': len(results), 'passed': sum(r['status'] == 'pass' for r in results),
                       'recorded': sum(r['status'] == 'recorded' for r in results), 'failed': len(failed),
                       'seconds': round(time.time() - t0, 1)},
           'results': results}
    RES.mkdir(parents=True, exist_ok=True)
    (RES / 'run-results-0.11.json').write_text(json.dumps(out, indent=2, ensure_ascii=True, default=str) + '\n', encoding='utf-8')
    print(json.dumps(out['summary']))
    for f in failed:
        print('FAIL', json.dumps(f, ensure_ascii=True, default=str)[:400])
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
