"""Reference-only runner for M1 draft 0.9 (annex rows Q1-Q4, StoreQueue D101/D102).
Python standard library only. NOT executed by the author.

Usage, from D:\\PoCol-Development:
  coordination\\runtime\\python311\\python.exe m1-draft-0.9\\tools\\run_checks_09.py
Writes only m1-draft-0.9/results/run-results-0.9.json.

What it checks:
1. Input SHA-256 values of the baseline sources and of this package (recorded, for
   reproducibility; the author did not compute them).
2. Provenance: every golden block cites a source file and line range; the literal
   numbers/words it relies on must appear in that range of the file as it is now.
3. Unit vectors: UTF-8 length (TextEncoder rules), qbytes, the four admission counters
   with the literal violated set, SiteStorageRef on the BR17e table and UTF-8 quota
   boundaries, BG1 configuration (release rejects TQ), immutable defaults.
4. Schedules: the model (tools/storequeue_ref.py) is driven through every case of
   vectors/storequeue-cases.json and compared with the HAND-WRITTEN golden checkpoints.
   Criterion G invariants, the replyState x ownState table and an independent
   SiteStorageRef replay of the backend are checked on every run.
5. Fault controls: each fault must be detected by its literal golden case no later
   than the stated time, and where the baseline describes the faulty behaviour
   (e.g. (7, 184793) at 5000) the faulty model must show exactly that.
"""

import hashlib
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

import storequeue_ref as SQ      # noqa: E402

results = []

INPUTS = [
    'reference/browser.md', 'reference/validation.md', 'reference/implementation.md', 'reference/governance.md',
    'm1-draft-0.2/M1-ANNEX-CHECKLIST-0.2.md', 'm1-draft-0.7/M1-STATUS-0.7.md',
    'm1-draft-0.8/README.md', 'm1-draft-0.8/M1-SPEC-0.8-AMENDMENT.md', 'm1-draft-0.8/tools/run_checks_08.py',
    'm1-draft-0.9/tools/storequeue_ref.py', 'm1-draft-0.9/tools/run_checks_09.py',
    'm1-draft-0.9/vectors/storequeue-units.json', 'm1-draft-0.9/vectors/storequeue-cases.json',
    'm1-draft-0.9/vectors/storequeue-transitions.json',
]


def record(name, status, **kw):
    r = {'check': name, 'status': status}
    r.update(kw)
    results.append(r)


def check(name, ok, **kw):
    record(name, 'pass' if ok else 'FAIL', **kw)
    return ok


def load(rel):
    return json.loads((PKG / rel).read_text(encoding='utf-8'))


def val(v):
    if isinstance(v, dict):
        return v['rep'] * v['n']
    return v


def params_of(spec):
    if spec == 'DEFAULTS':
        return SQ.DEFAULTS
    if spec == 'TQ':
        return SQ.TQ
    p = dict(params_of(spec['base']))
    p.update(spec['set'])
    return p


# ------------------------------------------------------------------ 1-2 hashes, provenance

def input_hashes():
    for rel in INPUTS:
        p = ROOT / rel
        record('input.sha256 ' + rel, 'recorded', sha256=hashlib.sha256(p.read_bytes()).hexdigest() if p.exists() else None,
               exists=p.exists())


_lines_cache = {}


def provenance(name, src):
    path = ROOT / src['file']
    if path not in _lines_cache:
        _lines_cache[path] = path.read_text(encoding='utf-8').splitlines()
    a, b = src['lines']
    seg = '\n'.join(_lines_cache[path][a - 1:b])
    missing = [lit for lit in src['literals'] if lit not in seg]
    check('provenance ' + name, not missing, file=src['file'], lines=[a, b], missing=missing)


# ------------------------------------------------------------------ 3 unit vectors

def unit_checks():
    u = load('vectors/storequeue-units.json')
    for block in ('utf8', 'qbytes', 'admission', 'quotaBR17e', 'quotaUtf8Boundary', 'config'):
        provenance('units.' + block, u[block]['source'])

    for c in u['utf8']['cases']:
        got = SQ.utf8len(c['s'])
        check('utf8.' + c['id'], got == c['len'], expected=c['len'], got=got)
        if 'jsStringLength' in c:
            check('utf8.notStringLength.' + c['id'], len(c['s'].encode('utf-16-le', 'surrogatepass')) // 2 == c['jsStringLength'] != c['len'])

    for c in u['qbytes']['cases']:
        op = dict(c['op'])
        if op['op'] == 'set':
            op['k'], op['v'] = val(op['k']), val(op['v'])
        got = SQ.qbytes(op)
        check('qbytes.' + c['id'], got == c['qbytes'], expected=c['qbytes'], got=got)

    for c in u['admission']['cases']:
        got = SQ.admission(params_of(c['params']), c['sess'], c['glob'], c['q'])
        check('admission.' + c['id'], got == c['violated'], expected=c['violated'], got=got)

    q = u['quotaBR17e']
    d = {}
    for s in q['steps']:
        k, v = s['op']
        r = SQ.SiteStorageRef.apply(d, {'op': 'set', 'k': k, 'v': val(v)})
        ok = r['decision'] == s['decision'] and r['totalAfter'] == s['after']
        if 'attempted' in s:
            ok = ok and r.get('attempted') == s['attempted']
        if s['decision'] == 4300:
            ok = ok and r['newDict'] == d                      # byte-identical dictionary (atomicity)
        check('quota.BR17e.' + s['id'], ok, expected=[s['decision'], s['after'], s.get('attempted')],
              got=[r['decision'], r['totalAfter'], r.get('attempted')])
        d = r['newDict']
    check('quota.BR17e.e9', sorted(d) == q['finalKeys'] and SQ.total_of(d) == q['finalTotal'] and d['k00'] == 'b'
          and len(d['k17']) == 4042, total=SQ.total_of(d))

    b = u['quotaUtf8Boundary']
    start = {k: val(b['start']['series']['v']) for k in b['start']['series']['keys']}
    start.update({k: val(v) for k, v in b['start']['extra'].items()})
    check('quota.utf8.startTotal', SQ.total_of(start) == b['startTotal'], got=SQ.total_of(start))
    for s in b['steps']:
        op = {'op': 'clear'} if s['op'] == 'clear' else {'op': 'set', 'k': s['op'][0], 'v': val(s['op'][1])}
        r = SQ.SiteStorageRef.apply(dict(start), op)
        ok = r['decision'] == s['decision'] and r['totalAfter'] == s['after'] and r.get('attempted') == s.get('attempted')
        check('quota.utf8.' + s['id'], ok, expected=[s['decision'], s['after'], s.get('attempted')],
              got=[r['decision'], r['totalAfter'], r.get('attempted')])

    for c in u['config']['cases']:
        got = SQ.validate_config(params_of(c['params']), c['build'])
        check('config.' + c['id'], got['ok'] == c['ok'] and got['violations'] == c['violations'], expected=c, got=got)
    try:
        SQ.DEFAULTS['active'] = 3
        immutable = False
    except TypeError:
        immutable = True
    check('config.defaultsImmutable', immutable and SQ.DEFAULTS['active'] == 2)
    try:
        SQ.StoreQueue(SQ.TQ, 'release')
        rejected = False
    except SQ.ConfigRejected:
        rejected = True
    check('config.tqReleaseConstructionRejected', rejected)


# ------------------------------------------------------------------ 4 schedules

def compact(r):
    if r['code'] is None:
        return [r['id'], None]
    out = [r['id'], r['code'], r['reason']]
    if 'violated' in r:
        out.append(r['violated'])
    return out


def expand(by_id, case):
    if 'prefix' not in case:
        return dict(case)
    base = expand(by_id, by_id[case['prefix']])
    merged = dict(base)
    merged.update({k: v for k, v in case.items() if k != 'events'})
    merged['events'] = list(base['events']) + list(case['events'])
    merged['final'] = case.get('final')
    return merged


def expand_events(events):
    out = []
    for ev in events:
        if ev['do'] != 'series':
            out.append(ev)
            continue
        a, b = ev['t']
        for j in range(ev['j'][0], ev['j'][1] + 1):
            for i in range(ev['i'][0], ev['i'][1] + 1):
                out.append({'t': a * (j - 1) + b * i, 'do': 'set', 'f': ev['f'].format(j=j, i=i), 'id': ev['id'].format(j=j, i=i),
                            'k': ev['k'].format(j=j, i=i), 'v': ev['v']})
    return out


def build(doc, case, faults):
    spec = case.get('params', 'DEFAULTS')
    p = params_of(spec)
    bld = case.get('build', 'release' if spec == 'DEFAULTS' else 'test')
    sites = dict(doc['sites'])
    sessions = {}
    if 'sessionSeries' in case:
        ss = case['sessionSeries']
        for j in range(ss['j'][0], ss['j'][1] + 1):
            lab = ss['site'].format(j=j)
            sites[lab] = ['n' + lab.lower(), '0x' + lab.lower()]
            sessions[ss['s'].format(j=j)] = {'site': lab, 'frames': [ss['f'].format(j=j)]}
    sessions.update(case.get('sessions', {}))
    skey = {lab: 'site:%s:%s' % (v[0], v[1]) for lab, v in sites.items()}
    dicts = {}
    for lab, d in case.get('initDicts', {}).items():
        d = doc['init'][d] if isinstance(d, str) else d
        dicts[skey[lab]] = {k: val(v) for k, v in d.items()}
    sq = SQ.StoreQueue(p, bld, faults, dicts)
    for sid, s in sessions.items():
        net, addr = sites[s['site']]
        sq.open_session(sid, net, addr)
        for f in s['frames']:
            sq.attach(f, sid)
    return sq, skey, dicts


def observe_field(sq, skey, field, want):
    if field == 'replies':
        return [compact(r) for r in sq.event_replies]
    if field == 'sess':
        return {sid: ([sq.sessions[sid]['n'], sq.sessions[sid]['bytes']] if sid in sq.sessions else None) for sid in want}
    if field == 'glob':
        return list(sq.glob)
    if field == 'sets':
        return len(sq.calls)
    if field == 'own':
        return {i: (sq.msgs[i].own if i in sq.msgs else ('notAdmitted' if i in sq.rejected else None)) for i in want}
    if field == 'reply':
        return {i: (sq.msgs[i].reply if i in sq.msgs else None) for i in want}
    if field == 'total':
        return {lab: SQ.total_of(sq._dict(skey[lab])) for lab in want}
    if field == 'snapshot':
        snaps = [[s['frame'], s['keys'], s['badge']] for s in sq.event_snapshots]
        return None if not snaps else (snaps[0] if len(snaps) == 1 else snaps)
    if field == 'sessions':
        return sorted(sq.sessions)
    raise KeyError(field)


def final_field(sq, skey, field, want):
    if field == 'setOrder':
        return [c['msg'] for c in sq.calls]
    if field == 'setDictKeys':
        return [sorted(c['dict']) for c in sq.calls]
    if field == 'dicts':
        return {lab: sq.backend.get(skey[lab], {}) for lab in want}
    if field == 'totals':
        return {lab: SQ.total_of(sq.backend.get(skey[lab], {})) for lab in want}
    if field == 'replyLog':
        return [compact(r) for r in sq.replies]
    if field == 'replyCounts':
        return {i: sq.msgs[i].replyCount for i in want}
    if field == 'stats':
        return {k: sq.stats[k] for k in want}
    if field == 'glob':
        return list(sq.glob)
    if field == 'keyInSetCalls':
        return {k: [k in c['dict'] for c in sq.calls] for k in want}
    if field == 'framesWithReplies':
        return sorted({r['frame'] for r in sq.replies if r.get('delivered')})
    raise KeyError(field)


def expand_want(field, want):
    if field == 'dicts':
        return {lab: {k: val(v) for k, v in d.items()} for lab, d in want.items()}
    return want


def run_case(doc, case, faults=(), table=None):
    sq, skey, init = build(doc, case, faults)
    mism, inv, bad_boundary, obs = [], [], [], []
    stable = {tuple(x) for x in table['stable']} if table else set()
    for ev in expand_events(case['events']):
        sq.event_replies, sq.event_snapshots = [], []
        t = ev.get('t', ev.get('from'))
        try:
            do = ev['do']
            if do == 'set':
                op = {'op': 'set', 'k': ev['k'], 'v': val(ev['v'])}
                if 'site' in ev:
                    op['site'] = ev['site']
                sq.submit(t, ev['f'], ev['id'], op)
            elif do == 'clear':
                sq.submit(t, ev['f'], ev['id'], {'op': 'clear'})
            elif do == 'tick':
                sq.tick(t)
            elif do == 'settle':
                sq.settle(t, ev['call'], ev['ok'])
            elif do == 'navigate':
                sq.navigate(t, ev['f'], ev['to'])
            elif do == 'close':
                sq.close_session(t, ev['s'])
            elif do == 'settleAll':
                tt = ev['from']
                while sq.oldest_unsettled_call() is not None:
                    sq.settle(tt, sq.oldest_unsettled_call(), True)
                    tt += 1
            else:
                raise ValueError('unknown event ' + do)
        except Exception as e:                                         # record, never hide
            mism.append({'t': t, 'field': 'exception', 'got': '%s: %s' % (type(e).__name__, e)})
            break
        obs.append({'t': t, 'do': ev['do'], 'id': ev.get('id'),
                    'sess': {sid: [s['n'], s['bytes']] for sid, s in sq.sessions.items()}, 'glob': list(sq.glob),
                    'sets': len(sq.calls), 'replies': [compact(r) for r in sq.event_replies]})
        v = sq.invariant_violations()
        if v:
            inv.append({'t': t, 'violations': v})
        for m in sq.msgs.values():
            if table and (m.reply, m.own) not in stable:
                bad_boundary.append({'t': t, 'id': m.id, 'pair': [m.reply, m.own]})
        for field, want in ev.get('x', {}).items():
            got = observe_field(sq, skey, field, want)
            if got != want:
                mism.append({'t': t, 'field': field, 'want': want, 'got': got})
    for field, want in (case.get('final') or {}).items():
        if any(m['field'] == 'exception' for m in mism):
            break
        got = final_field(sq, skey, field, want)
        if got != expand_want(field, want):
            mism.append({'t': 'final', 'field': field, 'want': want if field != 'dicts' else '(dict)', 'got': got if field != 'dicts' else
                         {lab: {k: (len(x) if isinstance(x, str) else x) for k, x in d.items()} for lab, d in got.items()}})
    return {'sq': sq, 'skey': skey, 'init': init, 'mismatches': mism, 'invariants': inv, 'badBoundary': bad_boundary, 'observations': obs}


def replay_consistent(run):
    """SiteStorageRef replay of the successful set calls, per key, from the initial
    dictionaries, must equal the fake backend. Consistency only, not proof."""
    sq = run['sq']
    want = {k: dict(v) for k, v in run['init'].items()}
    for c in sq.calls:
        if c['settled'] == 'ok':
            m = sq.msgs[c['msg']]
            want[c['skey']] = SQ.SiteStorageRef.apply(want.get(c['skey'], {}), m.op)['newDict']
    keys = set(want) | set(sq.backend)
    return all(want.get(k, {}) == sq.backend.get(k, {}) for k in keys)


def schedule_checks():
    doc = load('vectors/storequeue-cases.json')
    table = load('vectors/storequeue-transitions.json')
    provenance('transitions', table['source'])
    micro = {(tuple(x['from']), tuple(x['to'])) for x in table['micro']}
    by_id = {c['id']: c for c in doc['cases']}
    seen = set()
    for c in doc['cases']:
        provenance('case.' + c['id'], c['source'])
        case = expand(by_id, c)
        run = run_case(doc, case, table=table)
        sq = run['sq']
        trans = {(tuple(x['from']), tuple(x['to'])) for x in sq.transitions}
        seen |= trans
        illegal = sorted([list(f), list(t)] for f, t in trans - micro)
        ok = not run['mismatches'] and not run['invariants'] and not run['badBoundary'] and not illegal
        check('case ' + c['id'], ok, row=c.get('row'), mismatches=run['mismatches'][:8], invariantViolations=run['invariants'][:5],
              badBoundary=run['badBoundary'][:5], illegalTransitions=illegal)
        check('case.replayConsistent ' + c['id'], replay_consistent(run))
        check('case.noDoubleRelease ' + c['id'], sq.stats['doubleReleaseCount'] == 0 and sq.stats['quotaRecheckMismatch'] == 0,
              stats=sq.stats)
        record('case.recorded ' + c['id'], 'recorded', stats=dict(sq.stats),
               backendDicts={sk: {'keys': sorted(d), 'total': SQ.total_of(d)} for sk, d in sq.backend.items()},
               counters=run['observations'] if len(run['observations']) <= 40 else run['observations'][-12:],
               snapshots=sq.snapshots)
    missing = sorted([list(f), list(t)] for f, t in micro - seen)
    check('transitions.allMicroExercised', not missing, missing=missing)

    for fc in doc['faultControls']:
        case = expand(by_id, by_id[fc['case']])
        run = run_case(doc, case, faults=(fc['fault'],))
        mism = run['mismatches']
        if fc['detectBy'] == 'final':
            detected = bool(mism)
        else:
            detected = any(m['t'] != 'final' and m['t'] <= fc['detectBy'] for m in mism)
        first = mism[0] if mism else None
        observed_ok, observed = True, []
        for o in fc.get('observed', []):
            at = [x for x in run['observations'] if x['t'] == o['t']]
            got = {sid: at[-1]['sess'].get(sid) for sid in o['sess']} if at else None
            observed.append({'t': o['t'], 'want': o['sess'], 'got': got})
            observed_ok = observed_ok and got == o['sess']
        if 'observedFinal' in fc:
            got = {k: run['sq'].stats[k] for k in fc['observedFinal']}
            observed.append({'final': fc['observedFinal'], 'got': got})
            observed_ok = observed_ok and got == fc['observedFinal']
        check('fault %s detected by %s' % (fc['fault'], fc['case']), detected and observed_ok, detectBy=fc['detectBy'],
              firstMismatch=first, observed=observed)


def main():
    t0 = time.time()
    for step in (input_hashes, unit_checks, schedule_checks):
        try:
            step()
        except Exception as e:                                          # record, never hide
            check('step.completed ' + step.__name__, False, exception='%s: %s' % (type(e).__name__, e))
    failed = [r for r in results if r['status'] == 'FAIL']
    out = {'package': 'M1 draft 0.9 (annex Q1-Q4: StoreQueue D101/D102)',
           'executedAtUtc': datetime.now(timezone.utc).isoformat(timespec='seconds'),
           'environment': {'python': sys.version.split()[0], 'executable': sys.executable, 'platform': platform.platform(),
                           'thirdPartyPackages': 'none'},
           'evidenceKind': 'reference model of the specification text against hand-written golden data; fake clock and fake backend only',
           'notExecuted': ['BR21c (Chrome on anvil-br, C measurements)', 'BR21d (real loader in Chrome)', 'TypeScript StoreQueue/SiteStorage',
                           'browser / MV3 extension, DNR', 'chrome.storage.local atomicity', 'real transactions'],
           'summary': {'checks': len(results), 'passed': sum(r['status'] == 'pass' for r in results),
                       'recorded': sum(r['status'] == 'recorded' for r in results), 'failed': len(failed),
                       'seconds': round(time.time() - t0, 1)},
           'results': results}
    RES.mkdir(parents=True, exist_ok=True)
    (RES / 'run-results-0.9.json').write_text(json.dumps(out, indent=2, ensure_ascii=True, default=str) + '\n', encoding='utf-8')
    print(json.dumps(out['summary']))
    for f in failed:
        print('FAIL', json.dumps(f, ensure_ascii=True, default=str)[:400])
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
