"""Reference-only runner for M1 draft 0.20: C29 repair (closed SinkChecker grammar).
Python standard library only. NOT executed by the author.

Usage, from D:\\PoCol-Development:
  coordination\\runtime\\python311\\python.exe m1-draft-0.20\\tools\\run_checks_020.py
Writes only m1-draft-0.20/results/run-results-0.20.json and exits 1 on any FAIL.

Steps:
  1. Load m1-draft-0.19/tools/run_checks_019.py as a module (its main() is not called; its result file
     is not written). Keep the 0.19 SinkChecker class as PRISTINE, then install
     tools/sink_checker_strict.StrictSinkChecker as SinkChecker in the loaded model module object that
     the 0.19 runner uses (R19.M). No file of 0.19 is edited.
  2. Replay of run_checks_019.main() body without its file write: every LC case and variant, literal
     requests/results/SHA-256, step table, client controls, the 15 sink controls with their EXACT rule
     sets (now judged by the strict checker), gaps, coverage, input SHAs. Entries copied as 'suite019.'.
  3. C29 controls: the root minimal counterexample and the adversarial grammar/type controls, judged by
     the strict checker (exact rules and reasons) and by the pristine checker (recorded old behaviour,
     including the false greens and crashes).
  4. Deferred experiment specifications and definition gaps (recorded, not executed); coverage; preserved files.
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
OUT = RES / 'run-results-0.20.json'
CORE_STATUS = ('pass', 'recorded', 'FAIL')
RENAMED = {'name': 'diagName', 'check': 'diagCheck', 'status': 'proposalStatus', 'ok': 'diagOk',
           'label': 'diagLabel', 'condition': 'diagCondition'}
INPUTS = ['coordination/task-018.md', 'coordination/review-001/REVIEW-0.19.md', 'coordination/review-001/sink-unknown-terminal-probe.py',
          'coordination/review-001/sink-unknown-terminal-probe.json', 'reference/browser.md', 'reference/validation.md', 'reference/implementation.md',
          'm1-draft-0.19/tools/logclient_ref.py', 'm1-draft-0.19/tools/run_checks_019.py', 'm1-draft-0.19/vectors/lc-cases.json',
          'm1-draft-0.19/vectors/lc-sink-controls.json', 'm1-draft-0.20/tools/sink_checker_strict.py', 'm1-draft-0.20/tools/run_checks_020.py',
          'm1-draft-0.20/vectors/c29-controls.json', 'm1-draft-0.20/vectors/v2-deferred-experiments.json']
PRESERVED = ['m1-draft-0.18/results/run-results-0.18.json', 'm1-draft-0.19/results/run-results-0.19.json',
             'coordination/review-001/m1-draft-0.19/results/run-results-0.19.json', 'coordination/review-001/sink-unknown-terminal-probe.json',
             'coordination/issue-ledger.json']
SINK_CONTROLS_0_19 = 15
# 45 cases in m1-draft-0.19/vectors/lc-cases.json minus the 2 bridge cases (no sink timeline) = 43 SinkChecker runs

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
    results.append(make_entry(label, core_status, _snap(diag)))


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


def provenance(label, src):
    lines = (ROOT / src['file']).read_text(encoding='utf-8').splitlines()
    a, b = src['lines']
    text = '\n'.join(lines[a - 1:b])
    missing = [lit for lit in src.get('literals', []) if lit not in text]
    check(label + '.provenance', not missing, file=src['file'], lines=src['lines'], missingLiterals=missing)


sys.path.insert(0, str(HERE))
import sink_checker_strict as SC      # noqa: E402


# ------------------------------------------------------------------ step 1-2: the 0.19 suite under the strict checker

def replay_019(R19):
    """run_checks_019.main() lines 410-448 without its result-file write (lines 449-466)."""
    before = {rel: R19.sha(ROOT / rel) for rel in R19.PRESERVED}
    for rel in R19.INPUTS:
        R19.record('input.sha256 ' + rel, 'recorded', exists=(ROOT / rel).exists(), sha256=R19.sha(ROOT / rel))
    R19.check('boundary.positionalOnly', R19.record.__code__.co_posonlyargcount == 2 and R19.check.__code__.co_posonlyargcount == 2
              and R19.gap.__code__.co_posonlyargcount == 1)
    probe = {'x': [1]}
    R19.record('boundary.probe', 'recorded', name='n', status='s', obj=probe)
    probe['x'].append(2)
    e = R19.results[-1]
    R19.check('boundary.renamedAndCopied', e['diagName'] == 'n' and e['proposalStatus'] == 's' and e['status'] == 'recorded' and e['obj'] == {'x': [1]})
    calls = [n.lineno for n in ast.walk(ast.parse((ROOT / 'm1-draft-0.19/tools/run_checks_019.py').read_text(encoding='utf-8')))
             if isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute) and n.func.attr == 'main']
    R19.check('boundary.noOldMainCalled', not calls, attributeMainCallsAtLines=calls)
    by_id = {}
    data = R19.load_json(R19.VEC + 'lc-data.json')
    fx = R19.Fixture(data)
    steps = [('data', lambda: R19.data_checks(fx, R19.M.MockLogServer(data['branches']))),
             ('stepTable', lambda: R19.step_table(fx, R19.load_json(R19.VEC + 'lc-step-table.json'))),
             ('constants', R19.constants_and_exposure)]
    for name, fn in steps:
        try:
            fn()
        except Exception as ex:                                              # record, never hide
            R19.check('step.completed ' + name, False, exception='%s: %s' % (type(ex).__name__, ex))
    try:
        cases = R19.load_json(R19.VEC + 'lc-cases.json')
        by_id = R19.case_suite(fx, data, cases)
        R19.control_suite(fx, data, cases, by_id)
        R19.sink_controls(fx, data, R19.load_json(R19.VEC + 'lc-sink-controls.json'), by_id)
    except Exception as ex:                                                  # record, never hide
        R19.check('step.completed cases', False, exception='%s: %s' % (type(ex).__name__, ex))
    for gid, text in R19.GAPS:
        R19.gap('gap.' + gid, text=text)
    R19.coverage(by_id)
    after = {rel: R19.sha(ROOT / rel) for rel in R19.PRESERVED}
    R19.check('preserved.olderFilesUnchanged', before == after, before=before, after=after)
    for r in R19.results:
        r = dict(r)
        r['check'] = 'suite019.' + r['check']
        results.append(r)
    return fx, data, by_id


# ------------------------------------------------------------------ step 3: C29 controls

def timeline_for(R19, fx, data, by_id, ctl, anchor):
    if 'inline' in ctl:
        ents = []
        for i, ev in enumerate(ctl['inline']):
            ev = copy.deepcopy(ev)
            if isinstance(ev, list) and len(ev) > 2 and ev[2] == 'ANC':
                ev[2] = dict(anchor)
            ents.append({'seq': i, 'kind': 'sink', 'ev': ev})
        return ents, ctl['from'], ctl['to']
    base = by_id[ctl['base']]
    es = copy.deepcopy(R19.run_case(fx, data, base)['timeline'].entries)
    sinks = [e for e in es if e['kind'] == 'sink']
    commit = next(e for e in sinks if e['ev'][0] == 'commit')
    piece = lambda s: next(e for e in sinks if e['ev'][0] == 'piece' and e['ev'][2] == s)   # noqa: E731
    cid = ctl['id']
    if cid == 'C29-unknownAfterCommit':
        es.append({'seq': commit['seq'] + 0.5, 'kind': 'sink', 'ev': ['unknownTerminal', 1, 'x']})
    elif cid == 'C29-boolPieceSeq':
        piece(0)['ev'][2] = False
    elif cid == 'C29-floatPieceSeq':
        piece(0)['ev'][2] = 0.0
    elif cid == 'C29-pieceRange':
        piece(1)['ev'][3] = [1025]
    elif cid == 'C29-boolCounter':
        commit['ev'][2]['pieces'] = True
    elif cid == 'C29-floatCounter':
        commit['ev'][2]['pieces'] = 1.0
    elif cid == 'C29-negativeCounter':
        commit['ev'][2]['totalRequests'] = -1
    elif cid == 'C29-summaryNotDict':
        commit['ev'][2] = 'ok'
    elif cid == 'C29-summaryMissingKey':
        del commit['ev'][2]['logs']
    elif cid == 'C29-anchorMismatch':
        commit['ev'][2]['anchor'] = {'number': 3000, 'hash': fx.hashes['B3000']}
    elif cid == 'C29-badEntryKind':
        es.append({'seq': 99, 'kind': 'other'})
    elif cid == 'C29-boolEntrySeq':
        es.append({'seq': True, 'kind': 'req', 'req': ['H', 'latest']})
    elif cid == 'C29-malformedRequest':
        es.append({'seq': 100, 'kind': 'req', 'req': ['X']})
    elif ctl['mutation'] != 'none':
        raise ValueError(cid)
    return es, base['from'], base['to']


def c29_controls(R19, fx, data, by_id, pristine_cls):
    doc = load_json('m1-draft-0.20/vectors/c29-controls.json')
    provenance('C29', doc['source'])
    probe = load_json(doc['rootProbe']['file'])
    check('C29.rootProbeIsTheReviewedCounterexample', probe['actual'] == doc['rootProbe']['observedOldResult'] and probe['expectedRule'] == 'S-a'
          and [e['ev'] for e in probe['entries']] == [['begin', 1, doc['anchor']], ['unknownTerminal', 1, 'fake']])
    false_greens = []
    for ctl in doc['controls']:
        ents, frm, to = timeline_for(R19, fx, data, by_id, ctl, doc['anchor'])
        got = SC.StrictSinkChecker().check(copy.deepcopy(ents), frm, to)
        rules, whys = sorted({v['rule'] for v in got}), sorted({v['why'] for v in got})
        check('C29.new.' + ctl['id'], rules == ctl['new']['rules'] and whys == ctl['new']['whys'],
              expectedRules=ctl['new']['rules'], expectedWhys=ctl['new']['whys'], rules=rules, whys=whys)
        try:
            old = pristine_cls().check(copy.deepcopy(ents), frm, to)
            old_out = {'rules': sorted({v['rule'] for v in old})}
        except Exception as ex:                                              # the defect may crash the old checker
            old_out = {'crash': True, 'exception': type(ex).__name__}
        want_old = ctl['old']
        same = (old_out.get('crash') is True) if want_old.get('crash') else old_out.get('rules') == want_old['rules']
        record('C29.old.' + ctl['id'], 'recorded', oldChecker=old_out, expectedOld=want_old, matchesExpectedOldBehaviour=same)
        check('C29.oldBehaviourAsDerived.' + ctl['id'], same, oldChecker=old_out, expectedOld=want_old)
        if ctl['new']['rules'] and old_out.get('rules') == []:
            false_greens.append(ctl['id'])
    check('C29.rootFalseGreenReproducedOnOldChecker', 'C29-root' in false_greens, falseGreens=false_greens)
    record('C29.oldFalseGreens', 'recorded', controls=false_greens)
    return doc


def install_checks(R19, pristine_cls):
    check('C29.install.strictInModelGlobals', R19.M.SinkChecker is SC.StrictSinkChecker and pristine_cls is not SC.StrictSinkChecker)
    check('C29.install.oldFileUntouchedInMemoryOnly', Path(R19.M.__file__).resolve() == (ROOT / 'm1-draft-0.19/tools/logclient_ref.py').resolve())
    check('C29.install.runnerResolvesAtCallTime', R19.case_suite.__globals__['M'] is R19.M and R19.sink_controls.__globals__['M'] is R19.M)


# ------------------------------------------------------------------ step 4: deferred specs, coverage

def deferred():
    doc = load_json('m1-draft-0.20/vectors/v2-deferred-experiments.json')
    for x in doc['experiments']:
        provenance('V2-deferred.' + x['id'], x['source'])
        missing = [p for p in x['inputs'] if p.startswith('m1-draft-') and not (ROOT / p).exists()]
        check('V2-deferred.' + x['id'] + '.inputsExist', not missing, missing=missing)
        record('V2-deferred.' + x['id'], 'recorded', specification=True, executed=False, what=x['what'], inputs=x['inputs'], passCriteria=x['pass'])
    for g in doc['definitionGaps']:
        gap('V2-definitionGap.' + g['id'], experiment=g['experiment'], gap=g['gap'])
    for pid, text in (('P-V2-9', 'closed sink event grammar (tools/sink_checker_strict.py) as a contract supplement'),
                      ('P-V2-10', 'publish expanded literal replies for non-Python harnesses')):
        gap('proposal.' + pid, text=text, approved=False)


def coverage(c29doc):
    have = {r['check']: r for r in results}
    missing = [c['id'] for c in c29doc['controls'] if have.get('C29.new.' + c['id'], {}).get('status') != 'pass']
    check('coverage020.allC29ControlsPass', not missing, missing=missing)
    sc = [r for k, r in have.items() if k.startswith('suite019.sinkControl.')]
    check('coverage020.sinkControls15ExactUnderStrict', len(sc) == SINK_CONTROLS_0_19 and all(r['status'] == 'pass' for r in sc),
          count=len(sc), notPassing=[r['check'] for r in sc if r['status'] != 'pass'])
    lc = [r for k, r in have.items() if k.startswith('suite019.coverage.')]
    check('coverage020.all019CoverageGuardsPass', bool(lc) and all(r['status'] == 'pass' for r in lc), count=len(lc))
    sinkchk = [r for k, r in have.items() if k.startswith('suite019.') and k.endswith('.sinkChecker')]
    check('coverage020.everyCaseSinkCheckedStrict', len(sinkchk) == 43 and all(r['status'] == 'pass' for r in sinkchk), count=len(sinkchk))
    pv2 = [k for k, r in have.items() if k.startswith('suite019.gap.P-V2-') and r.get('partialGap')]
    check('coverage020.pV2ProposalsStillOpen', len(pv2) == 8, proposals=sorted(pv2))
    check('coverage020.noStepAborted', not [k for k in have if 'step.completed' in k])


def main():
    t0 = time.time()
    before = {rel: sha(ROOT / rel) for rel in PRESERVED}
    for rel in INPUTS:
        record('input.sha256 ' + rel, 'recorded', exists=(ROOT / rel).exists(), sha256=sha(ROOT / rel))
    check('boundary.positionalOnly', record.__code__.co_posonlyargcount == 2 and check.__code__.co_posonlyargcount == 2)
    calls = [n.lineno for n in ast.walk(ast.parse(Path(__file__).read_text(encoding='utf-8')))
             if isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute) and n.func.attr == 'main']
    check('boundary.noOldMainCalled', not calls, attributeMainCallsAtLines=calls)
    c29doc = {'controls': []}
    try:
        R19 = load_module('m1-draft-0.19/tools/run_checks_019.py', 'run_checks_019_suite020')       # main() not called
        pristine_cls = R19.M.SinkChecker
        R19.M.SinkChecker = SC.StrictSinkChecker                                                     # in-memory install
        install_checks(R19, pristine_cls)
        fx, data, by_id = replay_019(R19)
        c29doc = c29_controls(R19, fx, data, by_id, pristine_cls)
    except Exception as ex:                                                  # record, never hide
        check('step.completed suite019', False, exception='%s: %s' % (type(ex).__name__, ex))
    try:
        deferred()
    except Exception as ex:                                                  # record, never hide
        check('step.completed deferred', False, exception='%s: %s' % (type(ex).__name__, ex))
    coverage(c29doc)
    after = {rel: sha(ROOT / rel) for rel in PRESERVED}
    check('preserved.olderFilesUnchanged', before == after, before=before, after=after)
    failed = [r for r in results if r['status'] == 'FAIL']
    out = {'package': 'M1 draft 0.20 (C29 strict SinkChecker grammar; full 0.19 V2 suite replayed under it)',
           'executedAtUtc': datetime.now(timezone.utc).isoformat(timespec='seconds'),
           'environment': {'python': sys.version.split()[0], 'executable': sys.executable, 'platform': platform.platform(),
                           'thirdPartyPackages': 'none'},
           'statusSchema': {'coreStatus': list(CORE_STATUS), 'diagnosticRenames': RENAMED, 'partialGap': 'recorded entries with partialGap=true'},
           'evidenceKind': 'reference model of the specification text; deferred experiments are specifications only',
           'notExecuted': ['TS-LC', 'BR10', 'RG3', 'RG3-bridge', 'RG3b', 'RG9'],
           'summary': {'checks': len(results), 'passed': sum(r['status'] == 'pass' for r in results),
                       'recorded': sum(r['status'] == 'recorded' for r in results), 'failed': len(failed),
                       'partialGaps': sum(1 for r in results if r.get('partialGap')),
                       'suite019Entries': sum(1 for r in results if r['check'].startswith('suite019.')),
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
