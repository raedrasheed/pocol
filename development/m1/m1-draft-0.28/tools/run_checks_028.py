"""Small reference-only runner for M1 draft 0.28 (author turn 026): policy/status corrections. It binds the saved 0.27
results and root's independent evidence by hash and summary, proves that the reused 0.27 tools and vectors are
byte-identical to the reviewed copy, checks the explicit owner routing of P-C32-2 and V1-SHA-COST, and evaluates the
conditional U14 timing arithmetic. It does not rebuild any cryptographic fixture. Python standard library only.
NOT executed by the author.

Usage: run_checks_028.py [--root <tree>] [--out <dir>]   (or POCOL_M1_ROOT / POCOL_M1_OUT)
Writes only <out>/run-results-0.28.json (a later run: run-results-0.28-rerun-<n>.json). Exits 1 on any FAIL.
"""

import ast
import copy
import hashlib
import json
import os
import platform
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
OWN = ['tools/run_checks_028.py', 'annex/C32-POLICY-CORRECTIONS.md', 'owner/P-C32-2-CHANGE-REQUEST.md', 'vectors/u14-timing-cases.json',
       'audit/carry-forward-0.28.json']
results = []


def make_entry(label, core_status, diag):
    if core_status not in CORE_STATUS:
        raise ValueError(core_status)
    entry = {}
    for k, v in diag.items():
        entry[RENAMED.get(k, k)] = v
    entry['check'] = label
    entry['status'] = core_status
    return entry


def record(label, core_status, /, **diag):
    results.append(make_entry(label, core_status, copy.deepcopy(diag)))


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


def text(rel):
    p = ROOT / rel
    return p.read_text(encoding='utf-8') if p.exists() else ''


CF = own('audit/carry-forward-0.28.json')


def reused_identical():
    man = rj(CF['reused']['manifest'])
    want = {e['path'].replace('\\', '/'): e['sha256'] for e in man}
    bad = []
    for rel in CF['reused']['mustBeByteIdentical']:
        a = sha(ROOT / CF['reused']['package'] / rel)
        b = sha(ROOT / CF['reused']['reviewCopy'] / rel)
        if not (a and a == b == want.get(rel)):
            bad.append({'file': rel, 'package': a, 'reviewCopy': b, 'manifest': want.get(rel)})
        record('reuse.sha256 ' + rel, 'recorded', sha256=a)
    check('reuse.byteIdenticalToReviewedCopy', not bad and len(want) >= len(CF['reused']['mustBeByteIdentical']), mismatches=bad)
    allbad = [p for p, h in want.items() if sha(ROOT / CF['reused']['package'] / p) != h]
    check('reuse.wholeManifestUnchanged', not allbad, files=len(want), changed=allbad)


def saved_and_independent():
    sr = CF['savedResults']
    r = rj(sr['file'])
    s = r['summary']
    record('saved.sha256', 'recorded', file=sr['file'], sha256=sha(ROOT / sr['file']))
    check('saved.summary', [s['checks'], s['passed'], s['recorded'], s['failed']] == [sr['checks'], sr['passed'], sr['recorded'], sr['failed']], summary=s)
    rows = {}
    for e in r['results']:
        c = e.get('check', '')
        if c.startswith('row.') and c.endswith('.criteria'):
            rows[c[4:-9]] = e.get('proposalStatus')
    cnt = Counter(rows.values())
    check('saved.rows', dict(cnt) == sr['rows'] and all(rows.get(x) == 'CompleteCandidate' for x in CF['rows']['CompleteCandidate'])
          and all(rows.get(x) == 'Partial' for x in CF['rows']['ownerBlocked']), counts=dict(cnt))
    for ev in CF['independent']:
        d = rj(ev['file'])
        ok = d.get('checks') == ev['checks'] and d.get('failed') == ev['failed'] and ('passed' not in ev or d.get('passed') == ev['passed'])
        if 'flag' in ev:
            ok = ok and d.get(ev['flag']) is True
        check('independent.' + Path(ev['file']).stem, ok, sha256=sha(ROOT / ev['file']), checks=d.get('checks'), failed=d.get('failed'))


def source_unchanged():
    su = CF['sourceUnchanged']
    rec = {e['check'][len('input.sha256 '):]: e.get('sha256') for e in rj(su['results'])['results'] if e.get('check', '').startswith('input.sha256 ')}
    bad = [f for f in su['files'] if rec.get(f) is None or rec.get(f) != sha(ROOT / f)]
    check('source.unchangedSince0_27', not bad, changed=bad)
    lit = su['literal']
    line = text(lit['file']).splitlines()[lit['lines'][0] - 1]
    check('source.literalSha13Untouched', lit['text'] in line, line=line)


def ledger_and_routing():
    led = rj('coordination/issue-ledger.json')
    st = {i['id']: i['status'] for i in led['issues']}
    bad = {}
    for k, v in CF['ledgerExpect'].items():
        key, sub = (k[:-1], True) if k.endswith('~') else (k, False)
        if not ((v in st.get(key, '')) if sub else st.get(key) == v):
            bad[key] = st.get(key)
    check('ledger.statuses', not bad, mismatches=bad)
    cr = (PKG / 'owner/P-C32-2-CHANGE-REQUEST.md').read_text(encoding='utf-8')
    check('routing.P-C32-2', all(s in cr for s in ('| A |', '| B |', '| C |', '| D |', '| E |', '## Recommendation', 'Nothing is adopted', 'U14')))
    sha_cr = text('m1-draft-0.27/owner/V1-SHA-COST-CHANGE-REQUEST.md')
    check('routing.V1-SHA-COST', '3355' in sha_cr and 'Nothing here is adopted' in sha_cr)
    ann = (PKG / 'annex/C32-POLICY-CORRECTIONS.md').read_text(encoding='utf-8')
    check('corrections.statusTable', all(s in ann for s in ('Unapproved owner client policy', 'retry sleep only', 'Withdrawn for rate', 'Neither branch is selected')))
    missing = [s for s in CF['superseded'] if s['text'] not in text(s['file'])]
    check('corrections.supersededTextsExist', not missing, missing=missing)


def simulate(case, branch):
    """Conditional arithmetic of annex section 3. Returns (outcome, atMs, sleepMs)."""
    lat = list(case['latency'])
    t, slept, i = 0, 0, 0
    wl = branch in ('WL_a', 'WL_b')

    def send():
        nonlocal t, i
        d = lat[i]
        i += 1
        if branch == 'PR' and d > 10000:
            return ('viewIncomplete', t + 10000)
        if wl and t + d > 10000:
            return ('viewIncomplete', 10000)
        t += d
        return None
    r = send()                                                    # eth_blockNumber
    if r:
        return r + (slept,)
    retries = 0
    for rep in case['replies']:
        r = send()
        if r:
            return r + (slept,)
        if rep == 'ok':
            if branch == 'WL_a' and t + case['computeMs'] > 10000:
                return ('viewIncomplete', 10000, slept)
            return ('ok', t + case['computeMs'], slept)
        reason, ms = rep.split(':')
        ms = int(ms)
        lo, hi = (250, 2000) if reason == 'busy' else ((0, 2000) if case['policy'] == 'A' else (0, None))
        if ms < lo or (hi is not None and ms > hi) or retries >= 3:
            return ('viewIncomplete', t, slept)
        retries += 1
        if wl and t + ms > 10000:
            return ('viewIncomplete', 10000, slept + (10000 - t))
        t += ms
        slept += ms
    return ('viewIncomplete', t, slept)


def timing():
    doc = own('vectors/u14-timing-cases.json')
    for k, src in doc['sources'].items():
        lines = text(src['file']).splitlines()
        seg = '\n'.join(lines[src['lines'][0] - 1:src['lines'][1]])
        check('timing.source.' + k, all(x in seg for x in src['literals']), file=src['file'])
    for c in doc['cases']:
        bad = {}
        for br in ('PR', 'WL_a', 'WL_b'):
            o, at, s = simulate(c, br)
            if [o, at] != [c['expect'][br]['outcome'], c['expect'][br]['atMs']]:
                bad[br] = [o, at]
            if br == 'PR' and s != c['expect']['sleepMs']:
                bad['sleepMs'] = s
        check('timing.' + c['id'], not bad, mismatches=bad, conditional='hypothetical latencies; no branch selected')
    sleep_only = [c['id'] for c in doc['cases'] if c['expect']['sleepMs'] <= 6000 and c['expect']['PR']['atMs'] > 10000 and c['expect']['PR']['outcome'] == 'ok']
    check('timing.sleepBoundDoesNotBoundElapsed', bool(sleep_only), cases=sleep_only)
    derived = {'F1_single': -(-1000 // 20), 'F1_batch20': -(-20 * 1000 // 20), 'F2_fill': 1000}
    record('rate.conditionalDerivations', 'recorded', values=derived, insideProposedCap={k: v <= 2000 for k, v in derived.items()},
           note='conditional on assumed server formulas F1/F2; the source gives no rate-delay formula; F3/F4 unbounded')


def boundary():
    calls = [n.lineno for n in ast.walk(ast.parse(Path(__file__).read_text(encoding='utf-8')))
             if isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute) and n.func.attr == 'main']
    check('boundary.noOldMainCalled', not calls, lines=calls)
    tree = ast.parse(Path(__file__).read_text(encoding='utf-8'))
    mods = {a.name.split('.')[0] for n in ast.walk(tree) if isinstance(n, ast.Import) for a in n.names}
    check('boundary.noNetworkOrCryptoRebuild', not (mods & {'socket', 'urllib', 'http', 'subprocess', 'v1_ref_027', 'v1_ref'}), imports=sorted(mods))
    check('boundary.positionalOnly', record.__code__.co_posonlyargcount == 2 and check.__code__.co_posonlyargcount == 2)


def main():
    t0 = time.time()
    for rel in OWN:
        record('input.sha256 <package>/' + rel, 'recorded', sha256=sha(PKG / rel))
    for name, fn in (('boundary', boundary), ('reuse', reused_identical), ('saved', saved_and_independent), ('source', source_unchanged),
                     ('ledger', ledger_and_routing), ('timing', timing)):
        try:
            fn()
        except Exception as ex:                                              # record, never hide
            check('step.completed ' + name, False, exception='%s: %s' % (type(ex).__name__, ex))
    gap('gap.ownerDecisions', text='U01, U02, U10, U14, CR-M1-01, U08, RF-E6-1, V1-SHA-COST and P-C32-2 unanswered; seven rows owner-blocked; M1 not complete')
    have = {r['check']: r['status'] for r in results}
    need = ['reuse.byteIdenticalToReviewedCopy', 'reuse.wholeManifestUnchanged', 'saved.summary', 'saved.rows', 'source.unchangedSince0_27',
            'source.literalSha13Untouched', 'ledger.statuses', 'routing.P-C32-2', 'routing.V1-SHA-COST', 'corrections.statusTable',
            'corrections.supersededTextsExist', 'timing.sleepBoundDoesNotBoundElapsed'] + ['independent.' + Path(e['file']).stem for e in CF['independent']] \
        + ['timing.' + c['id'] for c in own('vectors/u14-timing-cases.json')['cases']]
    missing = [x for x in need if have.get(x) != 'pass']
    check('coverage028.required', not missing, missing=missing)
    failed = [r for r in results if r['status'] == 'FAIL']
    out = {'package': 'M1 draft 0.28 (retry-policy and status corrections; carried 0.27 evidence)',
           'executedAtUtc': datetime.now(timezone.utc).isoformat(timespec='seconds'),
           'environment': {'python': sys.version.split()[0], 'platform': platform.platform()},
           'summary': {'checks': len(results), 'passed': sum(r['status'] == 'pass' for r in results),
                       'recorded': sum(r['status'] == 'recorded' for r in results), 'failed': len(failed), 'seconds': round(time.time() - t0, 1)},
           'results': results}
    OUT.mkdir(parents=True, exist_ok=True)
    p, k = OUT / 'run-results-0.28.json', 1
    while p.exists():
        p, k = OUT / ('run-results-0.28-rerun-%d.json' % k), k + 1
    p.write_text(json.dumps(out, indent=2, ensure_ascii=True, default=str) + '\n', encoding='utf-8')
    print(json.dumps(out['summary']), '->', p.name)
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
