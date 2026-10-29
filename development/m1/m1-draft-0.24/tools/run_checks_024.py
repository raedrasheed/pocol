"""Reference-only runner for M1 draft 0.24: C31 repair (X8 evidence-evaluator integrity) and the supplemental
per-path C7 positive-control plan. Python standard library only; no network, no browser. NOT executed by the author.

Usage, from D:\\PoCol-Development:
  coordination\\runtime\\python311\\python.exe m1-draft-0.24\\tools\\run_checks_024.py
Writes only m1-draft-0.24/results/run-results-0.24.json and exits 1 on any FAIL.

Steps:
  1. Load m1-draft-0.23/tools/run_checks_023.py as a module (main() not called, result not written). Keep the published
     evaluator as OLD, then set evaluate = x8_eval_v2.evaluate on the 0.23 module object the runner uses (R23.EV).
  2. Replay the 0.23 main body: assets, configuration, provenance, historical, 14 synthetic verdicts, no-network,
     coverage, preservation. Entries are copied as 'suite023.'.
  3. C31 controls (new and old), the root probe replicated (37 cases), and both duplicate orders for every gating cell.
  4. Supplemental C7 per-path plan and pages: static properties only.
"""

import ast
import copy
import hashlib
import importlib.util
import json
import platform
import re
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
PKG, ROOT = HERE.parent, HERE.parent.parent
RES = PKG / 'results'
OUT = RES / 'run-results-0.24.json'
REL = 'm1-draft-0.24/'
CORE_STATUS = ('pass', 'recorded', 'FAIL')
RENAMED = {'name': 'diagName', 'check': 'diagCheck', 'status': 'proposalStatus', 'ok': 'diagOk',
           'label': 'diagLabel', 'condition': 'diagCondition'}
REVIEW_023 = 'coordination/review-001/m1-draft-0.23/results/run-results-0.23.json'
INPUTS = ['coordination/task-022.md', 'coordination/review-001/REVIEW-0.23.md', 'coordination/review-001/x8-independent-probes-0.23.json',
          'coordination/review-001/x8-independent-probes-prepared.py', 'm1-draft-0.23/tools/x8_eval.py', 'm1-draft-0.23/tools/run_checks_023.py',
          'm1-draft-0.23/vectors/x8-config.json', 'm1-draft-0.23/vectors/x8-synthetic-runs.json',
          REL + 'tools/x8_eval_v2.py', REL + 'tools/run_checks_024.py', REL + 'vectors/c31-controls.json', REL + 'vectors/c7-path-plan.json']
PRESERVED = ['m1-draft-0.23/results/run-results-0.23.json', REVIEW_023, 'coordination/review-001/x8-independent-probes-0.23.json',
             'coordination/issue-ledger.json', 'm1-draft-0.23/tools/x8_eval.py', 'm1-draft-0.23/vectors/x8-config.json']

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


def _snap(v):
    try:
        return copy.deepcopy(v)
    except Exception:
        return json.loads(json.dumps(v, default=str))


def record(label, core_status, /, **diag):
    results.append(make_entry(label, core_status, _snap(diag)))      # diagnostics copied at call time


def check(label, condition, /, **diag):
    record(label, 'pass' if condition else 'FAIL', **diag)
    return condition


def gap(label, /, **diag):
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


sys.path.insert(0, str(HERE))
import x8_eval_v2 as V2               # noqa: E402


# ------------------------------------------------------------------ replay of the 0.23 suite

def replay_023(R23):
    """run_checks_023.main() body (lines 325-346) without its result-file write."""
    before = {rel: R23.sha(ROOT / rel) for rel in R23.PRESERVED}
    for rel in R23.INPUTS:
        R23.record('input.sha256 ' + rel, 'recorded', exists=(ROOT / rel).exists(), sha256=R23.sha(ROOT / rel))
    R23.check('boundary.positionalOnly', R23.record.__code__.co_posonlyargcount == 2 and R23.check.__code__.co_posonlyargcount == 2
              and R23.gap.__code__.co_posonlyargcount == 1)
    calls = [n.lineno for n in ast.walk(ast.parse((ROOT / 'm1-draft-0.23/tools/run_checks_023.py').read_text(encoding='utf-8')))
             if isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute) and n.func.attr == 'main']
    R23.check('boundary.noOldMainCalled', not calls, attributeMainCallsAtLines=calls)
    cfg = R23.load_json(R23.REL + 'vectors/x8-config.json')
    syn = R23.load_json(R23.REL + 'vectors/x8-synthetic-runs.json')
    for name, fn in (('assets', lambda: R23.assets(cfg)), ('config', lambda: R23.config_checks(cfg)), ('historical', lambda: R23.historical_checks(cfg)),
                     ('evaluator', lambda: R23.evaluator_checks(cfg, syn)), ('network', R23.no_network_checks)):
        try:
            fn()
        except Exception as ex:                                              # record, never hide
            R23.check('step.completed ' + name, False, exception='%s: %s' % (type(ex).__name__, ex))
    for gid, text in R23.GAPS:
        R23.gap('gap.' + gid, text=text)
    R23.coverage(cfg, syn)
    after = {rel: R23.sha(ROOT / rel) for rel in R23.PRESERVED}
    R23.check('preserved.olderFilesUnchanged', before == after, before=before, after=after)
    for r in R23.results:
        r = dict(r)
        r['check'] = 'suite023.' + r['check']
        results.append(r)
    return cfg, syn


def asset_hashes_unchanged():
    mine = next((r for r in results if r['check'] == 'suite023.asset.manifest'), None)
    if not (ROOT / REVIEW_023).exists():
        record('C31.assetsUnchangedVsReview', 'recorded', note='review copy of the 0.23 results not present here', aggregate=mine and mine.get('aggregateSha256'))
        return
    rev = next((r for r in load_json(REVIEW_023)['results'] if r['check'] == 'asset.manifest'), None)
    check('C31.assetsUnchangedVsReview', mine is not None and rev is not None and mine.get('aggregateSha256') == rev.get('aggregateSha256'),
          now=mine and mine.get('aggregateSha256'), review=rev and rev.get('aggregateSha256'))


# ------------------------------------------------------------------ C31 controls

def root_base(cfg):
    ports = [{'proto': x['proto'], 'port': x['port']} for x in cfg['canary']['ports']]
    chans = {x['id']: x['expected'] for x in cfg['channels']}
    run = {'selftests': {'K%d' % k: copy.deepcopy(ports) for k in range(6)}, 'windows': []}
    for k in range(6):
        for c, (pr, po) in chans.items():
            run['windows'].append({'K': 'K%d' % k, 'C': c, 'events': [{'proto': pr, 'port': po}] if k == 4 else [], 'secondary': [], 'dns': []})
    return run


def _find(r, k, c):
    return next(i for i, w in enumerate(r['windows']) if type(w) is dict and w.get('K') == k and w.get('C') == c)


def apply_ops(run, ops):
    r = copy.deepcopy(run)
    for o in ops:
        op = o['op']
        if op == 'replaceRun':
            r = copy.deepcopy(o['value'])
        elif op == 'setRunKey':
            r[o['key']] = copy.deepcopy(o['value'])
        elif op == 'deleteRunKey':
            del r[o['key']]
        elif op == 'appendRaw':
            r['windows'].append(copy.deepcopy(o['value']))
        elif op == 'appendWindow':
            r['windows'].append({'K': o['K'], 'C': o['C'], 'events': [], 'secondary': [], 'dns': []})
        elif op == 'insertWindowBefore':
            r['windows'].insert(_find(r, o['K'], o['C']), {'K': o['K'], 'C': o['C'], 'events': [], 'secondary': [], 'dns': []})
        elif op == 'setEvents':
            r['windows'][_find(r, o['K'], o['C'])]['events'] = copy.deepcopy(o['events'])
        elif op == 'setWindowField':
            r['windows'][_find(r, o['K'], o['C'])][o['key']] = copy.deepcopy(o['value'])
        elif op == 'deleteWindowKey':
            del r['windows'][_find(r, o['K'], o['C'])][o['key']]
        elif op == 'dropWindow':
            del r['windows'][_find(r, o['K'], o['C'])]
        elif op == 'setSelftest':
            r['selftests'][o['K']] = copy.deepcopy(r['selftests'][o['copyOf']])
        elif op == 'deleteSelftest':
            del r['selftests'][o['K']]
        elif op == 'setSelftestPort':
            r['selftests'][o['K']][o['index']]['port'] = o['value']
        else:
            raise ValueError(op)
    return r


def old_outcome(old_eval, cfg, run):
    try:
        return old_eval(cfg, copy.deepcopy(run))['verdict']
    except Exception as e:                                       # the defect may crash the old evaluator
        return 'crash:' + type(e).__name__


def c31_controls(R23, old_eval, cfg, syn):
    doc = load_json(REL + 'vectors/c31-controls.json')
    probe = load_json(doc['rootProbe']['file'])
    failed = [r for r in probe['results'] if not r['pass']]
    check('C31.rootProbeRecord', len(failed) == 1 and failed[0]['check'] == doc['rootProbe']['failedCheck'] and failed[0]['actual'] == doc['rootProbe']['oldActual'])
    bases = {'syn': R23.build_base(cfg, syn['base']), 'root': root_base(cfg)}
    for c in doc['controls']:
        run = apply_ops(bases[c['base']], c['ops'])
        got = V2.evaluate(cfg, copy.deepcopy(run))
        again = V2.evaluate(cfg, copy.deepcopy(run))
        ex = c['new']
        cells = [('verdict', ex['verdict'], got['verdict']), ('fail', sorted(ex['fail']), sorted(got['fail'])),
                 ('codes', sorted(ex['codes']), sorted({f['code'] for f in got['integrity']}))]
        if 'invalidK' in ex:
            cells.append(('invalidK', ex['invalidK'], [x['K'] for x in got['invalid']]))
        bad = [{'cell': n, 'expected': w, 'actual': a} for n, w, a in cells if w != a]
        check('C31.new.' + c['id'], not bad and got == again, mismatches=bad, integrity=got['integrity'][:6], deterministic=got == again)
        old = old_outcome(old_eval, cfg, run)
        want_old = c['old']
        check('C31.old.' + c['id'], old == want_old or (want_old == 'crash' and old.startswith('crash:')), old=old, expectedOld=want_old,
              note='published 0.23 evaluator, recorded to show the defect')


def root_probe_replica(cfg):
    base = root_base(cfg)
    chans = {x['id']: x['expected'] for x in cfg['channels']}
    rows = [('all_required_positive_controls_and_zero_cells', base, ['PASS'])]
    for k in (0, 1, 2, 5):
        for c in chans:
            if k == 5 and c not in ('C5', 'C6', 'C7'):
                continue
            q = copy.deepcopy(base)
            q['windows'][_find(q, 'K%d' % k, c)]['events'] = [{'proto': chans[c][0], 'port': chans[c][1]}]
            rows.append(('gating_reach_K%d_%s' % (k, c), q, ['FAIL']))
    for c in chans:
        q = copy.deepcopy(base)
        q['windows'][_find(q, 'K4', c)]['events'] = []
        rows.append(('missing_positive_' + c, q, ['PENDING']))
    q = copy.deepcopy(base)
    q['windows'][_find(q, 'K0', 'C1')]['events'] = [{'proto': 'tcp', 'port': 18080}]
    q['windows'].append({'K': 'K0', 'C': 'C1', 'events': [], 'secondary': [], 'dns': []})
    rows.append(('duplicate_window_cannot_erase_recorded_K0_reach', q, ['FAIL', 'INVALID']))
    bad = [(n, V2.evaluate(cfg, r)['verdict']) for n, r, want in rows if V2.evaluate(cfg, r)['verdict'] not in want]
    check('C31.rootProbeReplica', not bad and len(rows) == 37, failing=bad, cases=len(rows))


def duplicate_sweep(cfg):
    """Every gating-zero cell, both duplicate orders: never PASS, the reach always stays in 'fail'."""
    base = root_base(cfg)
    chans = {x['id']: x['expected'] for x in cfg['channels']}
    cells = [('K%d' % k, c) for k in (0, 1, 2) for c in chans] + [('K5', c) for c in ('C5', 'C6', 'C7')]
    bad = []
    for k, c in cells:
        for order in ('after', 'before'):
            q = copy.deepcopy(base)
            i = _find(q, k, c)
            q['windows'][i]['events'] = [{'proto': chans[c][0], 'port': chans[c][1]}]
            empty = {'K': k, 'C': c, 'events': [], 'secondary': [], 'dns': []}
            if order == 'after':
                q['windows'].append(empty)
            else:
                q['windows'].insert(i, empty)
            got = V2.evaluate(cfg, q)
            if got['verdict'] != 'INVALID' or [k, c] not in got['fail']:
                bad.append({'cell': [k, c], 'order': order, 'verdict': got['verdict']})
    check('C31.duplicateSweepBothOrders', not bad and len(cells) == 27, failures=bad, combinations=2 * len(cells))


# ------------------------------------------------------------------ C7 supplemental plan (static only)

def c7_plan():
    plan = load_json(REL + 'vectors/c7-path-plan.json')
    ids = [p['id'] for p in plan['paths']]
    check('C7plan.fivePaths', ids == ['P1', 'P2', 'P3', 'P4', 'P5'] and [p['attempt'] for p in plan['paths']]
          == ['C7.createElement', 'C7.innerHTML', 'C7.createElementNS', 'C7.templateImportNode', 'C7.documentWrite'])
    for p in plan['paths']:
        html = (PKG / p['files'][0]).read_text(encoding='utf-8')
        js = (PKG / p['files'][1]).read_text(encoding='utf-8')
        recs = re.findall(r"rec\('([^']+)',\s*'[^']+',\s*'([^']+)'", js)
        ports = re.findall(r"ice\('(stun|turn)',\s*(\d+)", js)
        seps = js.replace("'http://www.w3.org/1999/xhtml'", '').count('://')
        check('C7plan.page.' + p['id'], recs == [(p['attempt'], 'udp/13478')] and ports == [('stun', '13478')] and seps == 0
              and "var H = '__CANARY_HOST__', RUN = '__RUN__';" in js and ("path: '%s'" % p['id']) in js
              and 'p.close()' in js and '}, 12000);' in js and 'log.realm = true' in js and 'log.pc = true' in js
              and re.findall(r'\s(?:src|href)\s*=\s*"([^"]*)"', html) == ['main.js'] and 'id="x8host"' in html
              and not re.search(r'\b\d{1,3}(?:\.\d{1,3}){3}\b', js), recs=recs, ports=ports)
        record('C7plan.asset.' + p['id'], 'recorded', sha256={f: sha(PKG / f) for f in p['files']})
    w = plan['windows']
    check('C7plan.windows', w['count'] == len(w['configurations']) * len(ids) == 30 and w['windowMs'] == 15000 and w['gapMs'] == 3000
          and w['pageClosesPeersAtMs'] == 12000)
    k4 = plan['positiveControlK4']
    check('C7plan.positiveControlPerPath', k4['perPath'] is True and [e[:3] for e in k4['requiredEvidence']] == ['E1:', 'E2:', 'E3:']
          and set(k4['outcomes']) == {'testable', 'untestable:noRealm', 'untestable:noTraffic', 'ambiguous'})
    check('C7plan.zeroCriteria', plan['zeroCriteria']['configurations'] == ['K0', 'K1', 'K2', 'K5'] and 'K0' in plan['zeroCriteria']['rule'])
    check('C7plan.attributionAndCleanup', 'gapRule' in plan['attribution'] and len(plan['cleanup']) == 3)
    check('C7plan.pb6pb8StillOpen', all('pending root review' in plan['status'][k] for k in ('PB6', 'PB8')))


def boundary_static():
    bad = {}
    for f in (HERE / 'x8_eval_v2.py', HERE / 'run_checks_024.py'):
        tree = ast.parse(f.read_text(encoding='utf-8'))
        mods = {a.name.split('.')[0] for n in ast.walk(tree) if isinstance(n, ast.Import) for a in n.names}
        mods |= {(n.module or '').split('.')[0] for n in ast.walk(tree) if isinstance(n, ast.ImportFrom)}
        hit = sorted(mods & {'socket', 'urllib', 'http', 'requests', 'ssl', 'asyncio', 'subprocess'})
        if hit:
            bad[f.name] = hit
    check('boundary.noNetworkModules', not bad, imports=bad)
    markers = ('PoCol_' + 'Dialogue', 'state' + '.json', 'source' + 'State', 'historical-x8-full' + '-source-private')
    leaks = [str(f.relative_to(PKG)) for f in sorted(PKG.rglob('*')) if f.is_file() and 'results' not in f.parts
             and f.suffix in ('.json', '.md', '.js', '.html', '.py') and any(m in f.read_text(encoding='utf-8') for m in markers)]
    check('boundary.noPrivatePathInPackage', not leaks, files=leaks)


def coverage(doc_ids):
    have = {r['check']: r['status'] for r in results}
    need = ['C31.new.' + i for i in doc_ids] + ['C31.old.' + i for i in doc_ids]
    need += ['C31.install.evaluatorInModuleGlobals', 'C31.rootProbeRecord', 'C31.rootProbeReplica', 'C31.duplicateSweepBothOrders',
             'C7plan.fivePaths', 'C7plan.windows', 'C7plan.positiveControlPerPath', 'C7plan.zeroCriteria', 'C7plan.attributionAndCleanup',
             'C7plan.pb6pb8StillOpen'] + ['C7plan.page.P%d' % i for i in range(1, 6)]
    need += ['suite023.eval.R%d-%s' % (i, s) for i, s in enumerate(['allGood', 'K0leak', 'K4c7silent', 'selftestMissing', 'K0secondary',
                                                                    'K3unexplained', 'K5c6reach', 'K4wrongPort', 'missingWindow', 'K0udp19000',
                                                                    'K2leak', 'K5httpOnlyPass', 'K0dns', 'invalidBeatsFail'], start=1)]
    need += ['suite023.coverage023.required', 'suite023.preserved.olderFilesUnchanged']
    missing = [x for x in need if have.get(x) != 'pass']
    check('coverage024.required', not missing, missing=missing, required=len(need))
    s = [k for k in have if k.startswith('suite023.')]
    check('coverage024.suite023NoFail', bool(s) and all(have[k] != 'FAIL' for k in s), entries=len(s))
    check('coverage024.noStepAborted', not [k for k in have if 'step.completed' in k])


def main():
    t0 = time.time()
    before = {rel: sha(ROOT / rel) for rel in PRESERVED}
    for rel in INPUTS:
        record('input.sha256 ' + rel, 'recorded', exists=(ROOT / rel).exists(), sha256=sha(ROOT / rel))
    check('boundary.positionalOnly', record.__code__.co_posonlyargcount == 2 and check.__code__.co_posonlyargcount == 2)
    calls = [n.lineno for n in ast.walk(ast.parse(Path(__file__).read_text(encoding='utf-8')))
             if isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute) and n.func.attr == 'main']
    check('boundary.noOldMainCalled', not calls, attributeMainCallsAtLines=calls)
    doc_ids = [c['id'] for c in load_json(REL + 'vectors/c31-controls.json')['controls']]
    try:
        R23 = load_module('m1-draft-0.23/tools/run_checks_023.py', 'run_checks_023_suite024')        # main() not called
        old_eval = R23.EV.evaluate
        R23.EV.evaluate = V2.evaluate                                                                 # in-memory patch
        check('C31.install.evaluatorInModuleGlobals', R23.evaluator_checks.__globals__['EV'].evaluate is V2.evaluate and old_eval is not V2.evaluate
              and Path(R23.EV.__file__).resolve() == (ROOT / 'm1-draft-0.23/tools/x8_eval.py').resolve())
        cfg, syn = replay_023(R23)
        asset_hashes_unchanged()
        c31_controls(R23, old_eval, cfg, syn)
        root_probe_replica(cfg)
        duplicate_sweep(cfg)
    except Exception as ex:                                                  # record, never hide
        check('step.completed suite', False, exception='%s: %s' % (type(ex).__name__, ex))
    for name, fn in (('c7plan', c7_plan), ('static', boundary_static)):
        try:
            fn()
        except Exception as ex:                                              # record, never hide
            check('step.completed ' + name, False, exception='%s: %s' % (type(ex).__name__, ex))
    for gid, text in (('PB6', 'document.write testability: per-path plan proposed; the source decision is still open'),
                      ('PB8', 'attribution schedule for path-less traffic: proposed; the source fixes none'),
                      ('V4-notExecuted', 'the X8 matrix and the C7 per-path windows are specified, not executed; no browser result is claimed')):
        gap('gap.' + gid, text=text)
    coverage(doc_ids)
    after = {rel: sha(ROOT / rel) for rel in PRESERVED}
    check('preserved.olderFilesUnchanged', before == after, before=before, after=after)
    failed = [r for r in results if r['status'] == 'FAIL']
    out = {'package': 'M1 draft 0.24 (C31 X8 evaluator integrity; supplemental per-path C7 plan; full 0.23 replay)',
           'executedAtUtc': datetime.now(timezone.utc).isoformat(timespec='seconds'),
           'environment': {'python': sys.version.split()[0], 'executable': sys.executable, 'platform': platform.platform(),
                           'thirdPartyPackages': 'none'},
           'statusSchema': {'coreStatus': list(CORE_STATUS), 'diagnosticRenames': RENAMED, 'partialGap': 'recorded entries with partialGap=true'},
           'evidenceKind': 'offline evaluator and static fixture analysis on synthetic data; no browser, no network',
           'notExecuted': ['Chrome/CDP X8 matrix', 'C7 per-path windows', 'CanarySink'],
           'summary': {'checks': len(results), 'passed': sum(r['status'] == 'pass' for r in results),
                       'recorded': sum(r['status'] == 'recorded' for r in results), 'failed': len(failed),
                       'partialGaps': sum(1 for r in results if r.get('partialGap')),
                       'suite023Entries': sum(1 for r in results if r['check'].startswith('suite023.')),
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
