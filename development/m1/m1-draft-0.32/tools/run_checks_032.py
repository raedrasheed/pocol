"""Supplemental runner for M1 draft 0.32 (author turn 030): C37 transitive history closure and the 41-row acceptance
candidate. Python standard library only. It READS preserved artifacts; it rebuilds no model, fixture, hash or signature and
starts no Node process (the 0.31 native-codec executions are bound from root's completed 0.31 results).
NOT executed by the author. Root executes it in a preserved copy.

C37: run_checks_031.history_final() built its status map BEFORE recording history.HF-8, so history.HF-12 read None for a
check the completed run contains as pass. Here one closure method, close(), is used for everything: the fresh HF-12
recompute, the explicit stale-before-HF-8 versus fresh-after-HF-8 demonstration, every mutation, and the global closure
of all historical failures. Status maps are built from complete result lists at the moment of use.

Usage: run_checks_032.py [--root <tree>] [--out <dir>]   (or POCOL_M1_ROOT / POCOL_M1_OUT)
Writes only under out, never overwriting: run-results-0.32.json, history-closure-0.32.json, acceptance-candidate-0.32.json,
status-0.32.json, dashboard-0.32-ar.json. Exits 1 on any FAIL.
"""

import ast
import copy
import hashlib
import json
import os
import platform
import re
import sys
import time
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
PKG = HERE.parent
CORE_STATUS = ('pass', 'recorded', 'FAIL')
RENAMED = {'name': 'diagName', 'check': 'diagCheck', 'status': 'proposalStatus', 'ok': 'diagOk',
           'label': 'diagLabel', 'condition': 'diagCondition'}


def _arg(flag, env):
    if flag in sys.argv:
        i = sys.argv.index(flag)
        if i + 1 < len(sys.argv):
            return sys.argv[i + 1]
    return os.environ.get(env)


def _find_root():
    given = _arg('--root', 'POCOL_M1_ROOT')
    if given:
        return Path(given).resolve()
    for p in [HERE] + list(HERE.parents):
        if (p / 'reference').is_dir() and (p / 'm1-draft-0.2').is_dir():
            return p
    return HERE.parent.parent


ROOT = _find_root()
OUT = Path(_arg('--out', 'POCOL_M1_OUT') or (PKG / 'results')).resolve()
REVIEW = ROOT / 'coordination' / 'review-001'
OWN = ['tools/run_checks_032.py', 'audit/history-registry-0.32.json', 'audit/acceptance-inventory-0.32.json', 'M1-SPEC-0.32-SUPPLEMENT.md',
       'M1-STATUS-0.32.md', 'M1-DASHBOARD-0.32-AR.md', 'README.md']
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


def rj(rel):
    return json.loads((ROOT / rel).read_text(encoding='utf-8'))


def own(rel):
    return json.loads((PKG / rel).read_text(encoding='utf-8'))


REG = own('audit/history-registry-0.32.json')
INV = own('audit/acceptance-inventory-0.32.json')
KEY_OF = {v: k for k, v in REG['results'].items()}


# ------------------------------------------------------------------ the single closure method

def index(result_list):
    """label -> list of statuses of the TOP-LEVEL result entries (complete at the moment it is called)."""
    m = {}
    for r in result_list:
        c = r.get('check')
        if type(c) is str:
            m.setdefault(c, []).append(r.get('status'))
    return m


def failed_count(d):
    if not isinstance(d, dict):
        return None
    if isinstance(d.get('failed'), int):
        return d['failed']
    s = d.get('summary')
    return s.get('failed') if isinstance(s, dict) and isinstance(s.get('failed'), int) else None


def run_failures(lists):
    """lists: key -> result list. Every FAIL entry becomes '<key>::<label>' (a label failing twice appears twice)."""
    out = []
    for key, lst in lists.items():
        for r in lst:
            if r.get('status') == 'FAIL' and type(r.get('check')) is str:
                out.append('%s::%s' % (key, r['check']))
    return out


def close(failure_ids, entries, maps, selfmap):
    """Returns (allClosed, report). A failure closes only with EXACTLY one registry entry whose every witness is proved;
    'via' witnesses recurse with a cycle guard. maps: key -> label index; selfmap: label index of the current run."""
    count = Counter(e['id'] for e in entries)
    by_id = {e['id']: e for e in entries}
    memo = {}

    def witness(w, seen):
        if 'via' in w:
            ok, why = closed(w['via'], seen)
            return ok, {'via': w['via'], 'closed': ok, 'why': why}
        if 'file' in w:
            p = ROOT / w['file']
            n = failed_count(json.loads(p.read_text(encoding='utf-8'))) if p.exists() else None
            return n == w['failed'], {'file': w['file'], 'failed': n}
        m = selfmap if w.get('self') else maps.get(w['r'])
        if m is None:
            return False, {'missingResults': w.get('r')}
        if 'check' in w:
            st = m.get(w['check'], [])
            return st == ['pass'], {'check': w['check'], 'statuses': st}
        rx = re.compile(w['pattern'])
        st = [s for k, v in m.items() if rx.search(k) for s in v]
        if w.get('mustBeAbsent'):
            return not st, {'pattern': w['pattern'], 'absent': not st}
        return 'pass' in st and 'FAIL' not in st, {'pattern': w['pattern'], 'counts': dict(Counter(st))}

    def closed(fid, seen):
        if fid in memo:
            return memo[fid][0], memo[fid][1]
        if count[fid] != 1:
            res = (False, 'unmapped' if count[fid] == 0 else 'ambiguousRegistry')
        elif fid in seen:
            res = (False, 'cycle')
        else:
            ws = by_id[fid].get('witness') or []
            proofs = [witness(w, seen | {fid}) for w in ws]
            res = (bool(ws) and all(p[0] for p in proofs), [p[1] for p in proofs])
        memo[fid] = res
        return res

    report = {}
    for fid in failure_ids:
        ok, why = closed(fid, frozenset())
        report.setdefault(fid, {'closed': ok, 'proof': why, 'occurrences': 0})
        report[fid]['occurrences'] += 1
    return all(v['closed'] for v in report.values()) and bool(report), report


# ------------------------------------------------------------------ loading

def run_lists():
    out = {}
    for p in sorted(REVIEW.rglob('run-results*.json')):
        rel = p.relative_to(ROOT).as_posix()
        if '/m1-draft-0.32/' in rel:
            continue                                                         # this supplement's own outputs are not history

        out[KEY_OF.get(rel, rel)] = json.loads(p.read_text(encoding='utf-8'))['results']
    return out


def root_failures():
    out = []
    for p in sorted(REVIEW.glob('*.json')):
        if p.name.startswith('run-results') or '-private' in p.name:
            continue
        try:
            n = failed_count(json.loads(p.read_text(encoding='utf-8')))
        except ValueError:
            continue
        if n:
            out.append({'file': p.relative_to(ROOT).as_posix(), 'failed': n})
    return out


# ------------------------------------------------------------------ checks

def boundary():
    check('boundary.positionalOnly', record.__code__.co_posonlyargcount == 2 and check.__code__.co_posonlyargcount == 2 and gap.__code__.co_posonlyargcount == 1)
    tree = ast.parse((HERE / 'run_checks_032.py').read_text(encoding='utf-8'))
    mods = {a.name.split('.')[0] for n in ast.walk(tree) if isinstance(n, ast.Import) for a in n.names}
    mods |= {(n.module or '').split('.')[0] for n in ast.walk(tree) if isinstance(n, ast.ImportFrom)}
    check('boundary.readOnlyStdlib', not (mods & {'subprocess', 'socket', 'urllib', 'http', 'importlib', 'threading', 'asyncio'}), imports=sorted(mods),
          note='no subprocess: Node is not started; no importlib: no earlier runner is executed')


def bind031(lists):
    b = INV['base']
    r31 = rj(b['results0_31'])
    s, w = r31['summary'], INV['r31Expected']
    check('bind031.summary', [s['checks'], s['passed'], s['recorded'], s['failed'], s.get('codecCalls'), s.get('rows')]
          == [w['checks'], w['passed'], w['recorded'], w['failed'], w['codecCalls'], w['rows']], summary=s)
    env = r31.get('environment', {})
    bd = rj(b['bindings0_31']).get('runtime', {})
    man = {e['path'].replace('\\', '/'): e['sha256'] for e in rj(b['manifest0_31'])}
    check('bind031.runtimeFromArtifacts', env.get('python') == INV['runtime']['python'] and bd.get('nodeVersion') == INV['runtime']['node']
          and 'not pure Python' in env.get('runtimeNote', '') and bd.get('codecSha256') == man.get(INV['runtime']['codecFile']),
          python=env.get('python'), nodeVersion=bd.get('nodeVersion'), codecSha256=bd.get('codecSha256'))
    bad, recs = [], {r['check'][len('input.sha256 <package>/'):]: r.get('sha256') for r in r31['results'] if r.get('check', '').startswith('input.sha256 <package>/')}
    for rel, h in man.items():
        main, copy_ = ROOT / 'm1-draft-0.31' / rel, REVIEW / 'm1-draft-0.31' / rel
        if not (sha(main) == sha(copy_) == h == recs.get(rel)):
            bad.append({'file': rel, 'manifest': h, 'main': sha(main), 'copy': sha(copy_), 'recordedByRun': recs.get(rel)})
    check('bind031.unchangedPackage', len(man) == 10 and not bad, files=len(man), mismatches=bad)
    for ev in INV['boundEvidence']:
        d = rj(ev['file'])
        check('bind031.evidence.' + Path(ev['file']).stem, all(d.get(k) == v for k, v in ev.items() if k != 'file'), sha256=sha(ROOT / ev['file']))
    fz = INV['freeze']
    d = rj(fz['file'])
    st31 = index(lists['R31'])
    check('bind031.freeze', len(d['legacy']) == fz['legacy'] and len(d['v1']) == fz['v1'] and not [e['id'] for e in d['legacy'] + d['v1'] if e['status'] == 'blocked']
          and all(d['labelled'].get(g, {}).get('status') == 'labelledNotFrozen' for g in fz['labelled']) and st31.get(fz['r31Binding']) == ['pass'],
          legacy=len(d['legacy']), v1=len(d['v1']), sha256=sha(ROOT / fz['file']))
    ex = rj(INV['experiments']['amendments'])
    st30 = index(lists['R30'])
    check('bind031.experimentDefinitions', st30.get(INV['experiments']['r30']) == ['pass'] and all(e.get('definitionComplete') is True for e in ex['amended'] + ex['new'])
          and all('notInM1Gate' in e for e in ex['new']), note='definitions only; no Phase A measurement is claimed')
    rd31, rd30 = rj(b['reviewDecisions0_31']), rj(b['reviewDecisions0_30'])
    check('bind031.reviewDecisions', rd31.get('acceptedSpecScopeRows') == INV['acceptedRows']['r0_31'] and rd31.get('globalAcceptance') is False
          and 'C37' in rd31.get('remainingBlocker', '') and sorted(rd31.get('criteriaReviewed', [])) == ['c1', 'c2', 'c3', 'c4', 'c5']
          and rd30.get('acceptedUnaffectedRows') == INV['acceptedRows']['r0_30'] and rd30.get('personalOwnerAnswersRecorded') == 0,
          sha256={'0.31': sha(ROOT / b['reviewDecisions0_31']), '0.30': sha(ROOT / b['reviewDecisions0_30'])})
    led = {i['id']: i.get('status') for i in rj('coordination/issue-ledger.json').get('issues', [])}
    record('bind031.ledger', 'recorded', statuses={k: led.get(k) for k in ('C33', 'C34', 'C35', 'C36', 'C37')},
           note='C37 is closed only by root after executing this supplement')


def history(lists):
    maps = {k: index(v) for k, v in lists.items()}
    entries = REG['runFailures']
    exp = REG['expected']
    sets = {k: sorted(r['check'] for r in lists[k] if r.get('status') == 'FAIL') for k in ('R29', 'R30', 'R31')}
    check('history.oldFailedSetsExact', sets == {'R29': sorted(exp['r29']), 'R30': sorted(exp['r30']), 'R31': sorted(exp['r31'])}, sets=sets)
    exact = {x: maps['R31'].get(w['check']) for x in exp['r30'] for w in next(e for e in entries if e['id'] == 'R30::' + x)['witness'] if 'check' in w}
    check('history.r30RepairsPassExactlyOnceInR31', all(v == ['pass'] for v in exact.values()) and len(exact) == 6, statuses=exact)
    six = ['R30::' + x for x in exp['r30']]
    ok, rep = close(six, entries, {'R31': maps['R31']}, {})
    check('history.HF-12.freshClosure', ok, report=rep, method='close() over the complete 0.31 result list')
    cut = next(i for i, r in enumerate(lists['R31']) if r.get('check') == 'history.HF-8')
    stale_ok, stale_rep = close(six, entries, {'R31': index(lists['R31'][:cut])}, {})
    check('history.liveOrder.staleBeforeHF8DoesNotClose', not stale_ok and stale_rep['R30::history.HF-8']['closed'] is False, report=stale_rep['R30::history.HF-8'],
          note='the 0.31 snapshot point: same close() method, status map taken before history.HF-8 was recorded')
    check('history.liveOrder.freshAfterHF8Closes', ok and rep['R30::history.HF-8']['closed'] is True)
    hf8 = ['R29::' + x for x in exp['r29']]
    ok8, rep8 = close(hf8, entries, maps, {})
    check('history.HF-8.chainToFour029Failures', ok8 and rep8['R29::coverage029.required']['proof'][0].get('via') == 'R30::coverage030.required', report=rep8)
    runf = run_failures(lists)
    check('history.enumeratedRunFailures', len(runf) == exp['runFailures'], count=len(runf), failures=runf)
    rootf = root_failures()
    reg_root = {(e['file'], e['failed']) for e in REG['rootFailures']}
    check('history.enumeratedRootFailures', len(rootf) == exp['rootFailureFiles'] and {(f['file'], f['failed']) for f in rootf} == reg_root, found=rootf)
    selfmap = index(results)
    root_entries = [{'id': 'ROOT::%s::%d' % (e['file'], e['failed']), 'witness': e['witness']} for e in REG['rootFailures']]
    root_ids = ['ROOT::%s::%d' % (f['file'], f['failed']) for f in rootf]
    allok, allrep = close(runf + root_ids, entries + root_entries, maps, selfmap)
    check('history.globalClosure', allok, open=[k for k, v in allrep.items() if not v['closed']], failures=len(allrep))
    mutations(lists, entries, root_entries, root_ids, selfmap)
    return allok, allrep


def mutations(lists, entries, root_entries, root_ids, selfmap):
    def run(mlists=None, mentries=None, mself=None):
        L = mlists or lists
        maps = {k: index(v) for k, v in L.items()}
        ok, _ = close(run_failures(L) + root_ids, (mentries or entries) + root_entries, maps, selfmap if mself is None else mself)
        return ok

    def edit(key, fn):
        L = dict(lists)
        L[key] = fn(copy.deepcopy(lists[key]))
        return L

    def setst(lst, label, st):
        for r in lst:
            if r.get('check') == label:
                r['status'] = st
        return lst
    cases = {
        'M1-removeHF8': run(edit('R31', lambda l: [r for r in l if r.get('check') != 'history.HF-8'])),
        'M2-HF8fail': run(edit('R31', lambda l: setst(l, 'history.HF-8', 'FAIL'))),
        'M3-C36repairFail': run(edit('R31', lambda l: setst(l, 'C36.scopeProbes.parsedLiterals', 'FAIL'))),
        'M4-removeRepairLabel': run(edit('R31', lambda l: [r for r in l if r.get('check') != 'C36.reviewDecisions030.acceptedUnaffected'])),
        'M5-unmappedOldFail': run(edit('R30', lambda l: l + [{'check': 'phantom.unmappedOldFailure', 'status': 'FAIL'}])),
        'M6-duplicateHF8withFail': run(edit('R31', lambda l: l + [{'check': 'history.HF-8', 'status': 'FAIL'}])),
        'M7-duplicateHF8bothPass': run(edit('R31', lambda l: l + [{'check': 'history.HF-8', 'status': 'pass'}])),
        'M8-removeRegistryEntry': run(mentries=[e for e in entries if e['id'] != 'R29::coverage029.required']),
        'M9-transitiveBreak': run(edit('R31', lambda l: setst(l, 'coverage031.required', 'FAIL'))),
        'M10-ambiguousRegistry': run(mentries=entries + [next(e for e in entries if e['id'] == 'R30::history.HF-8')]),
        'M11-noFreshSelfCheck': run(mself={}),
    }
    for k, closed_ in cases.items():
        check('mutation.' + k, closed_ is False, closedUnderMutation=closed_)


def candidate(closure_ok):
    b = INV['base']
    m31 = rj(b['matrix0_31'])
    rd31 = rj(b['reviewDecisions0_31'])
    r31 = {r['check']: r for r in rj(b['results0_31'])['results']}
    accepted = set(rd31['acceptedSpecScopeRows'])
    rows, bad = {}, []
    for rid, m in m31['rows'].items():
        e31 = r31.get('row.%s.criteria' % rid, {})
        if e31.get('criteria') != m['criteria'] or e31.get('proposalStatus') != m['status']:
            bad.append(rid)
        crit, basis = {}, {}
        for ck, v in m['criteria'].items():
            if v == 'satisfied':
                crit[ck], basis[ck] = 'satisfied', 'R31 row.%s.criteria' % rid
            elif v == 'pendingRootReview' and rid in accepted and ck in rd31['criteriaReviewed']:
                crit[ck], basis[ck] = 'satisfied', 'REVIEW-DECISIONS-0.31 acceptedSpecScopeRows'
            else:
                crit[ck], basis[ck] = v, 'unchanged from R31'
        blockers = list(m.get('ownerOpen', [])) + list(m.get('reviewerOpen', [])) + list(m.get('missing', [])) \
            + [g for g, s in m.get('hashGroups', {}).items() if s == 'blocked']
        if not closure_ok:
            crit['c5'], basis['c5'] = 'blocked', 'C37: history.globalClosure did not pass'
        elif blockers:
            crit['c5'], basis['c5'] = 'blocked', 'row blockers: %s' % blockers
        elif crit['c5'] == 'satisfied':
            basis['c5'] = basis['c5'] + ' + history.globalClosure (C33-C36 verified closed; C37 closure computed in this run)'
        st = 'CompleteCandidate' if all(v == 'satisfied' for v in crit.values()) else 'Partial'
        rows[rid] = {'title': m['title'], 'status': st, 'criteria': crit, 'basis': basis, 'genuineBlockers': blockers,
                     'links': {'r31RowCheck': 'row.%s.criteria' % rid, 'r31Sha256': sha(ROOT / b['results0_31']),
                               'review': 'REVIEW-DECISIONS-0.31.json' if rid in accepted else ('REVIEW-DECISIONS-0.30.json' if rid in INV['acceptedRows']['r0_30'] else INV['acceptedRows']['earlier'])},
                     'phaseA': m.get('phaseA', [])}
        record('row.%s.candidate' % rid, 'recorded', status=st, criteria=crit, basis=basis, genuineBlockers=blockers)
    check('candidate.matrixMatchesR31Entries', not bad and len(m31['rows']) == 41, differing=bad)
    check('candidate.all41', len(rows) == 41 and all(v['status'] == 'CompleteCandidate' for v in rows.values()),
          notCandidate={r: v['criteria'] for r, v in rows.items() if v['status'] != 'CompleteCandidate'})
    finds = {}
    for fid, f in m31['findings'].items():
        if fid == 'F26':
            d = 'pendingRootRecordAll41'
        elif f['disposition'] == 'closableAtSpecScope' or (f['disposition'] == 'closableAfterRootReviewOf031' and closure_ok):
            d = 'closableAtSpecScope'
        else:
            d = f['disposition']
        finds[fid] = {'disposition': d, 'from031': f['disposition'], 'dependsOn': f['dependsOn']}
    want = {f: d for d, fs in INV['proposedFindings'].items() for f in fs}
    check('candidate.findings', {f: v['disposition'] for f, v in finds.items()} == want and len(finds) == 26,
          differing={f: [finds.get(f, {}).get('disposition'), want.get(f)] for f in set(finds) | set(want) if finds.get(f, {}).get('disposition') != want.get(f)})
    return rows, finds


# ------------------------------------------------------------------ outputs

def write_asset(name, data_text, label):
    OUT.mkdir(parents=True, exist_ok=True)
    p = OUT / name
    data = data_text.encode('utf-8')
    if p.exists() and p.read_bytes() != data:
        k = 1
        while (OUT / ('%s-rerun-%d%s' % (p.stem, k, p.suffix))).exists():
            k += 1
        (OUT / ('%s-rerun-%d%s' % (p.stem, k, p.suffix))).write_bytes(data)
        record(label + '.differsFromEarlierRun', 'recorded', kept=name, sha256=hashlib.sha256(data).hexdigest())
        return
    if not p.exists():
        p.write_bytes(data)
    check(label + '.writtenAndReadBack', p.read_bytes() == data, file=name, sha256=hashlib.sha256(data).hexdigest())


def dumps(x):
    return json.dumps(x, sort_keys=True, indent=1, ensure_ascii=False, default=str) + '\n'


def leaks():
    markers = ('PoCol_' + 'Dialogue', 'state' + '.json', 'source' + 'State', 'historical-x8-full' + '-source-private', 'transcript' + '-private')
    found = [str(f.relative_to(PKG)) for f in sorted(PKG.rglob('*')) if f.is_file() and 'results' not in f.parts
             and f.suffix in ('.json', '.md', '.py') and any(m in f.read_text(encoding='utf-8') for m in markers)]
    check('boundary.noPrivatePath', not found, files=found)


def step(name, fn, *a):
    try:
        return fn(*a)
    except Exception as ex:                                                  # record, never hide
        check('step.completed ' + name, False, exception='%s: %s' % (type(ex).__name__, ex))
        return None


def main():
    t0 = time.time()
    for rel in OWN:
        record('input.sha256 <package>/' + rel, 'recorded', exists=(PKG / rel).exists(), sha256=sha(PKG / rel))
    for rel in REG['results'].values():
        record('input.sha256 ' + rel, 'recorded', sha256=sha(ROOT / rel))
    step('boundary', boundary)
    lists = step('load', run_lists) or {}
    step('bind031', bind031, lists)
    hist = step('history', history, lists) or (False, {})
    rows_finds = step('candidate', candidate, hist[0]) or ({}, {})
    step('leaks', leaks)
    need = ['boundary.readOnlyStdlib', 'bind031.summary', 'bind031.runtimeFromArtifacts', 'bind031.unchangedPackage', 'bind031.freeze', 'bind031.experimentDefinitions',
            'bind031.reviewDecisions', 'history.oldFailedSetsExact', 'history.r30RepairsPassExactlyOnceInR31', 'history.HF-12.freshClosure',
            'history.liveOrder.staleBeforeHF8DoesNotClose', 'history.liveOrder.freshAfterHF8Closes', 'history.HF-8.chainToFour029Failures',
            'history.enumeratedRunFailures', 'history.enumeratedRootFailures', 'history.globalClosure', 'candidate.matrixMatchesR31Entries',
            'candidate.all41', 'candidate.findings', 'boundary.noPrivatePath']
    have = {r['check']: r['status'] for r in results}
    need += [k for k in have if k.startswith(('mutation.', 'bind031.evidence.'))]
    missing = [x for x in need if have.get(x) != 'pass']
    check('coverage032.required', not missing and sum(k.startswith('mutation.') for k in have) == 11, missing=missing, required=len(need))
    check('coverage032.noStepAborted', not [k for k in have if k.startswith('step.completed')])
    failed = [r for r in results if r['status'] == 'FAIL']
    rows, finds = rows_finds
    cnt = Counter(v['status'] for v in rows.values())
    texts = {
        'history-closure-0.32.json': dumps({'schema': 'pocol-m1-history-closure/0.32', 'globalClosure': hist[0], 'failures': hist[1],
                                            'method': 'close(): exactly one registry entry per failure, every witness proved; via recursion; fresh complete status maps'}),
        'acceptance-candidate-0.32.json': dumps({'schema': 'pocol-m1-acceptance-candidate/0.32', 'rows': rows, 'findings': finds, 'counts': dict(cnt),
                                                 'globalAcceptance': False, 'm1Complete': False,
                                                 'note': 'a candidate matrix computed from executed evidence; root records Complete and F26 only after reviewing this run'}),
        'status-0.32.json': dumps({'schema': 'pocol-m1-status/0.32', 'failed': [r['check'] for r in failed], 'rows': dict(cnt), 'missingRequired': missing,
                                   'remainingRoot': ['execute run_checks_032.py in a preserved copy and require zero FAIL', 'review the closure report and mutations',
                                                     'close C37 in the ledger, then record the 41 rows and F26 if accepted'],
                                   'complete': False}),
        'dashboard-0.32-ar.json': dumps({'schema': 'pocol-m1-dashboard-ar/0.32', 'lang': 'ar', 'lines': [
            'M1، المسودة 0.32: ملحق صغير يصلح C37، أي خطأ ترتيب في فحص التاريخ، دون إعادة أي حساب.',
            'في 0.31 أُخذت لقطة الحالات قبل تسجيل history.HF-8، فرأى فحص HF-12 قيمة فارغة مع أن HF-8 نجح.',
            'يحسب هذا الملحق إغلاق كل الإخفاقات التاريخية من النتائج الكاملة، بطريقة واحدة، ويبيّن الفرق بين اللقطة القديمة والحالة الكاملة.',
            'أحد عشر تعديلًا متعمدًا يجب أن يمنع كل منها الإغلاق، ومنها حذف HF-8 وجعله فاشلًا وتكرار اسمه.',
            'نتيجة التشغيل: %d فحصًا، فشل منها %d.' % (len(results), len(failed)),
            'الصفوف: %d مرشحًا للاكتمال من 41.' % cnt.get('CompleteCandidate', 0),
            'القبول الكامل لـM1 وتسجيل F26 للجذر وحده بعد مراجعة هذا التشغيل. لا كود إنتاجي ولا نشر ولا معاملات.']})}
    for name in sorted(texts):
        step('write ' + name, write_asset, name, texts[name], 'asset.' + name)
    failed = [r for r in results if r['status'] == 'FAIL']
    out = {'package': 'M1 draft 0.32 (C37 transitive history closure; 41-row acceptance candidate)',
           'executedAtUtc': datetime.now(timezone.utc).isoformat(timespec='seconds'),
           'environment': {'python': sys.version.split()[0], 'platform': platform.platform(), 'node': 'not used by this supplement'},
           'summary': {'checks': len(results), 'passed': sum(r['status'] == 'pass' for r in results), 'recorded': sum(r['status'] == 'recorded' for r in results),
                       'failed': len(failed), 'rows': dict(cnt), 'seconds': round(time.time() - t0, 1)},
           'results': results}
    OUT.mkdir(parents=True, exist_ok=True)
    p, k = OUT / 'run-results-0.32.json', 1
    while p.exists():
        p, k = OUT / ('run-results-0.32-rerun-%d.json' % k), k + 1
    p.write_text(json.dumps(out, indent=2, ensure_ascii=True, default=str) + '\n', encoding='utf-8')
    print(json.dumps(out['summary']), '->', p.name)
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
