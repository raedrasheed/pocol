"""Reference-only runner for M1 draft 0.10 (annex rows E1-E3: D103/D134 worker-generation
recovery). Python standard library only. NOT executed by the author.

Usage, from D:\\PoCol-Development:
  coordination\\runtime\\python311\\python.exe m1-draft-0.10\\tools\\run_checks_010.py
Writes only m1-draft-0.10/results/run-results-0.10.json.

Steps:
1. SHA-256 of every input (recorded; the author did not compute them).
2. Provenance: each golden block's literal anchors must appear in its cited source lines.
3. E1 unit vectors (names, ranges, strict parsing, largest valid record, epoch value).
4. E2 fence cases (BR22h h1-h4, h6, BR22b, retry, sweep) against hand-written expectations;
   h5: all 3^6 assignments for both spacings, invariants on every run, literal rows,
   distinct-schedule count recorded.
5. E3 BR22a: every combination of the axes, criteria (1)-(7), the hand-derived k1/E rules;
   distinct-schedule count recorded.
6. Negative controls: each must make its named vector fail as the baseline states.
"""

import hashlib
import itertools
import json
import platform
import random
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
PKG, ROOT = HERE.parent, HERE.parent.parent
RES = PKG / 'results'
sys.path.insert(0, str(HERE))

import sr_ref as SR     # noqa: E402

results = []
INPUTS = [
    'reference/browser.md', 'reference/validation.md', 'reference/implementation.md', 'reference/governance.md',
    'reference/threat.md', 'reference/FINAL_DESIGN.md', 'm1-draft-0.2/M1-ANNEX-CHECKLIST-0.2.md', 'm1-draft-0.9/M1-STATUS-0.9.md',
    'm1-draft-0.9/tools/storequeue_ref.py', 'm1-draft-0.10/tools/sr_ref.py', 'm1-draft-0.10/tools/run_checks_010.py',
    'm1-draft-0.10/vectors/e1-units.json', 'm1-draft-0.10/vectors/e2-fence-cases.json', 'm1-draft-0.10/vectors/e3-br22a.json',
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


_lines = {}


def provenance(name, src):
    p = ROOT / src['file']
    if p not in _lines:
        _lines[p] = p.read_text(encoding='utf-8').splitlines()
    a, b = src['lines']
    seg = '\n'.join(_lines[p][a - 1:b])
    missing = [x for x in src['literals'] if x not in seg]
    check('provenance ' + name, not missing, file=src['file'], lines=[a, b], missing=missing)


def input_hashes():
    for rel in INPUTS:
        p = ROOT / rel
        record('input.sha256 ' + rel, 'recorded', exists=p.exists(),
               sha256=hashlib.sha256(p.read_bytes()).hexdigest() if p.exists() else None)


# ------------------------------------------------------------------ E1

def e1_checks():
    u = load('vectors/e1-units.json')
    for block in ('epochName', 'recordName', 'lookup', 'epochValue'):
        provenance('e1.' + block, u[block]['source'])
    for c in u['epochName']['cases']:
        try:
            got, err = SR.epoch_name(c['E']), False
        except ValueError:
            got, err = None, True
        check('e1.epochName %d' % c['E'], err if c.get('error') else got == c['name'], got=got)
    for c in u['epochName']['parse']:
        got = SR.parse_epoch_name(c['name'])
        check('e1.parseEpoch ' + c['name'], got == c['E'], got=got)
    for c in u['recordName']['cases']:
        try:
            got, err = SR.record_name(c['net'], c['addr'], c['E'], c['seq']), False
        except ValueError:
            got, err = None, True
        check('e1.recordName %s/%s' % (c['E'], c['seq']), err if c.get('error') else got == c['name'], got=got)
    for c in u['recordName']['parse']:
        got = SR.parse_record_name(c['name'], 'n1', '0xa1')
        check('e1.parseRecord ' + c['name'], (list(got) if got else None) == c['version'], got=got)
    for c in u['lookup']['cases']:
        r = SR.lookup(c['items'], 'n1', '0xa1')
        want = c['expect']
        got = {k: (list(r[k]) if isinstance(r.get(k), tuple) else r.get(k)) for k in want}
        check('e1.lookup ' + c['id'], got == want, got=got, want=want)
    for c in u['epochValue']['cases']:
        check('e1.epochValue %s' % json.dumps(c['v']), SR.epoch_value_ok(c['v']) == c['ok'])


# ------------------------------------------------------------------ E2 builders

def h1_events():
    ev = []
    for k in range(1, 41):
        ev += [{'t': 1000 * k, 'do': 'boot', 'gen': k, 'cfg': {'settleFence': False}}, {'t': 1000 * k + 10, 'do': 'death', 'gen': k}]
    order = list(range(1, 41))
    random.Random(2201).shuffle(order)
    ev += [{'t': 40001 + i, 'do': 'place', 'gen': g, 'cat': 'fence', 'action': 'apply'} for i, g in enumerate(order)]
    return ev


def h2_events():
    ev = []
    for k in range(1, 41):
        b = 70000 * k
        ev += [{'t': b, 'do': 'boot', 'gen': k, 'cfg': {'settleFence': False}},
               {'t': b + 50, 'do': 'place', 'gen': k, 'cat': 'fence', 'action': 'apply'},
               {'t': b + 100, 'do': 'death', 'gen': k}]
    return ev


def h5_events(spacing, assign):
    d = 10 if spacing == 1000 else 100
    ev = []
    for k in range(1, 7):
        b, nxt = k * spacing, (k + 1) * spacing
        ev += [{'t': b, 'do': 'boot', 'gen': k, 'cfg': {'settleFence': False}}, {'t': b + d, 'do': 'death', 'gen': k}]
        a = assign[k - 1]
        if a == 'B':
            ev.append({'t': nxt - 1, 'do': 'place', 'gen': k, 'cat': 'fence', 'action': 'apply'})
        elif a == 'A':
            ev.append({'t': nxt + SR.T_LATE - 1, 'do': 'place', 'gen': k, 'cat': 'fence', 'action': 'apply'})
        else:
            ev.append({'t': nxt, 'do': 'place', 'gen': k, 'cat': 'fence', 'action': 'drop'})
    s7 = 7 * spacing
    ev += [{'t': s7, 'do': 'boot', 'gen': 7, 'cfg': {}}, {'t': s7 + 10, 'do': 'set', 'gen': 7, 'id': 'kv', 'k': 'k', 'v': 'v'}]
    return ev, s7 + 61000


def run_world(items, events, until, faults=(), sites=None):
    w = SR.World(items=items, sites=sites, faults=faults)
    w.load(events)
    w.run(until)
    return w


def case_events(doc, case):
    if case['id'] == 'h1':
        return h1_events(), case['runUntil']
    if case['id'] == 'h2':
        return h2_events(), case['runUntil']
    if case.get('extends') == 'h2':
        return h2_events() + case['extra'], case['runUntil']
    return case['events'], case['runUntil']


def reply_tuples(w):
    out = []
    for r in w.replies:
        t = [r['id'], r['code'], r['reason'], r['t']]
        if r['detail'] is not None:
            t.append(r['detail'])
        out.append(t)
    return out


def record_ops(w, gid):
    out = []
    for op in w.be.ops:
        if op.gen == gid and op.kind == 'set' and op.cat == 'record':
            v = op.value
            out.append([v['epoch'], v['seq'], op.issued])
    return out


def sweep_trace(w, gid):
    return [x for x in w.trace if x['ev'] == 'sweep' and x['gen'] == gid]


def eval_expect(w, exp):
    """Compare each expectation field; returns list of mismatches."""
    mism = []
    gs = w.gen_summary()

    def cmp(field, got, want):
        if got != want:
            mism.append({'field': field, 'want': want, 'got': got})

    for f, want in exp.items():
        if f == 'allE':
            cmp(f, sorted({g['E'] for g in gs.values()}), [want])
        elif f in ('E', 'EN'):
            cmp(f, {k: gs[int(k)][f] for k in want}, want)
        elif f == 'confirmedAt':
            cmp(f, {k: gs[int(k)]['confirmedAt'] for k in want}, want)
        elif f == 'noFence':
            cmp(f, [g for g in range(want[0], want[1] + 1) if gs[g]['fenceIssues']], [])
        elif f in ('epochGateBlocks', 'action25'):
            cmp(f, w.stats[f], want)
        elif f == 'maxEpochNames':
            cmp(f, w.max_names, want)
        elif f == 'finalEpochs':
            cmp(f, sorted(e for e, _, _ in w.epochs()), want)
        elif f == 'violations':
            cmp(f, [v['kind'] for v in w.violations], want)
        elif f.startswith('gen') and f[3:].isdigit():
            g = gs[int(f[3:])]
            cmp(f, {k: g[k] for k in want}, want)
        elif f.startswith('sweepRemovedBy'):
            sw = sweep_trace(w, int(f[len('sweepRemovedBy'):]))
            cmp(f, sw[0]['removed'] if sw else None, want)
        elif f.startswith('sweepAt'):
            gid = int(f[len('sweepAt'):]) if f[len('sweepAt'):] else 41
            sw = sweep_trace(w, gid)
            cmp(f, sw[0]['t'] if sw else None, want)
        elif f.startswith('recordOpsBy'):
            cmp(f, record_ops(w, int(f[len('recordOpsBy'):])), want)
        elif f == 'firstRecordOpNotBefore':
            ts = [op.issued for op in w.be.ops if op.cat == 'record' and op.kind == 'set' and op.gen == 41]
            cmp(f, bool(ts) and min(ts) >= want, True)
        elif f == 'replies':
            cmp(f, reply_tuples(w), want)
        elif f == 'noReply':
            cmp(f, [i for i in want if any(r['id'] == i for r in w.replies)], [])
        elif f == 'finalDict':
            cmp(f, w.final_dict(), want)
        elif f == 'recordPresent':
            cmp(f, want in w.be.items, True)
        elif f == 'probeAfterOldApply':
            p = [x for x in w.probes if x['label'] == 'afterOldApply'][0]
            cmp(f, {'F6bootMs': dict((e, b) for e, b in p['epochs']).get(6)}, want)
        elif f in ('siteB', 'afterLateA'):
            p = [x for x in w.probes if x['label'] == ('B' if f == 'siteB' else 'afterLateA')][0]
            got = {'lateGens': p['lateGens'], 'LATE_SITES': p['LATE_SITES'], 'need': p['decision']['need'],
                   'decision': p['decision']['decision'], 'reason': p['decision'].get('reason'), 'limit': p['decision'].get('limit'),
                   'diskKeys': p['diskKeys']}
            cmp(f, {k: got[k] for k in want}, want)
        elif f == 'lateAWithinCoverage':
            f7 = [b for e, _, b in w.epochs() if e == 7]
            cover = (f7[0] + SR.T_LATE) if f7 else None
            cmp(f, {'applyAt': want['applyAt'], 'coverUntil': cover}, want)
            if cover is not None and want['applyAt'] > cover:
                mism.append({'field': f, 'error': 'late apply outside coverage'})
        elif f == 'recordsWrittenBy':
            cmp(f, {k: [r[:2] for r in record_ops(w, int(k))] for k in want}, want)
        elif f == 'fenceNamesIssued':
            cmp(f, sorted({op.name for op in w.be.ops if op.cat == 'fence'}), want)
        elif f == 'snapshots':
            cmp(f, len(w.snapshots), want)
        else:
            mism.append({'field': f, 'error': 'unknown expectation field'})
    return mism


def e2_checks():
    doc = load('vectors/e2-fence-cases.json')
    items = doc['initialBR22h']['items']
    by_id = {c['id']: c for c in doc['cases']}
    for c in doc['cases']:
        provenance('e2.' + c['id'], c['source'])
        if c['id'] == 'h5':
            h5_checks(c, items)
            continue
        ev, until = case_events(doc, c)
        w = run_world(items, ev, until, sites=c.get('sites'))
        mism = eval_expect(w, c['expect'])
        check('e2.case ' + c['id'], not mism, row=c['row'], mismatches=mism[:8], violations=w.violations[:5])
        record('e2.recorded ' + c['id'], 'recorded', gens=w.gen_summary(), stats=w.stats, maxEpochNames=w.max_names,
               finalEpochs=sorted(e for e, _, _ in w.epochs()), probes=w.probes, fakeBackendMax={'items': w.max_items, 'bytes': w.max_bytes})

    for ctl in doc['controls']:
        c = by_id[ctl['case']]
        if c['id'] == 'h5':
            h5_control(c, items, ctl)
            continue
        ev, until = case_events(doc, c)
        w = run_world(items, ev, until, faults=(ctl['fault'],), sites=c.get('sites'))
        kinds = [v['kind'] for v in w.violations]
        ok = all(k in kinds for k in ctl['expect'].get('violationKinds', []))
        if 'maxEpochNames' in ctl['expect']:
            ok = ok and w.max_names == ctl['expect']['maxEpochNames']
        check('e2.control %s on %s' % (ctl['fault'], ctl['case']), ok, violations=kinds, maxEpochNames=w.max_names)


def h5_signature(w):
    return hashlib.sha256(json.dumps([[x['ev'], x.get('gen'), x.get('E'), x.get('name'), x['t']] for x in w.trace]
                                     + [sorted(w.be.items)], default=str).encode()).hexdigest()


def h5_checks(c, items):
    letters = 'BAD'
    exp = c['expect']
    rows = {(r['spacing'], r['assign']): r for r in exp['rows']}
    for spacing in c['generator']['spacings']:
        count, bad, sigs = 0, [], set()
        for assign in itertools.product(letters, repeat=6):
            a = ''.join(assign)
            ev, until = h5_events(spacing, a)
            w = run_world(items, ev, until)
            count += 1
            sigs.add(h5_signature(w))
            g7 = w.gens.get(7)
            rep = [r for r in w.replies if r['id'] == 'kv']
            fails = []
            if w.violations:
                fails.append([v['kind'] for v in w.violations])
            if not (g7 and g7.state == 'confirmed'):
                fails.append('survivor not confirmed')
            if not (len(rep) == 1 and rep[0]['code'] is None):
                fails.append('survivor reply')
            if w.max_names > SR.EPOCH_NAMES_MAX:
                fails.append('names')
            if fails and len(bad) < 5:
                bad.append({'assign': a, 'fails': fails})
            elif fails:
                bad.append(None)
            row = rows.get((spacing, a))
            if row:
                gs = w.gen_summary()
                got = {'E': [gs[k]['E'] for k in range(1, 7)], 'E7': gs[7]['E'], 'finalEpochs': sorted(e for e, _, _ in w.epochs())}
                want = {'E': row['E'], 'E7': row['E7'], 'finalEpochs': row['finalEpochs']}
                if 'confirmed7' in row:
                    got['confirmed7'], want['confirmed7'] = gs[7]['confirmedAt'], row['confirmed7']
                check('e2.h5.row %d %s' % (spacing, a), got == want, got=got, want=want)
        check('e2.h5.allAssignments spacing=%d' % spacing, count == exp['assignmentsPerSpacing'] and not bad,
              assignments=count, failing=len(bad), examples=[b for b in bad if b][:5])
        record('e2.h5.distinctSchedules spacing=%d' % spacing, 'recorded', distinct=len(sigs), assignments=count)


def h5_control(c, items, ctl):
    letters = 'BAD'
    for spacing in c['generator']['spacings']:
        detected = []
        for assign in itertools.product(letters, repeat=6):
            a = ''.join(assign)
            ev, until = h5_events(spacing, a)
            w = run_world(items, ev, until, faults=(ctl['fault'],))
            if any(v['kind'] == 'epochResurrected' for v in w.violations):
                detected.append(a)
        must = [m['assign'] for m in ctl['expect'].get('mustDetect', []) if m['spacing'] == spacing]
        ok = all(m in detected for m in must) and (spacing != 1000 or len(detected) >= ctl['expect']['detectedInAtLeast'])
        check('e2.control %s on h5 spacing=%d' % (ctl['fault'], spacing), ok, detectedCount=len(detected), mustDetect=must,
              examples=detected[:5])


# ------------------------------------------------------------------ E3 BR22a

def br22a_events(first, second, extra, L):
    ev = [{'t': 10, 'do': 'set', 'tab': 'T1', 'id': 'A', 'k': 'k1', 'v': 'x'}]
    if first == 'P0':
        ev.append({'t': 10, 'do': 'death', 'gen': 1})
        td = 10
    elif first == 'P1':
        ev.append({'t': 20, 'do': 'death', 'gen': 1})
        td = 20
    elif first == 'P2':
        ev += [{'t': 15, 'do': 'place', 'gen': 1, 'cat': 'record', 'action': 'apply'}, {'t': 20, 'do': 'death', 'gen': 1}]
        td = 20
    elif first == 'P3':
        ev.append({'t': 5020, 'do': 'death', 'gen': 1})
        td = 5020
    else:  # fail
        ev += [{'t': 15, 'do': 'settle', 'gen': 1, 'cat': 'record', 'ok': False},
               {'t': 16, 'do': 'set', 'tab': 'T1', 'id': 'Aq', 'k': 'k1', 'v': 'q'},
               {'t': 17, 'do': 'settle', 'gen': 1, 'cat': 'record', 'ok': True},
               {'t': 20, 'do': 'death', 'gen': 1}]
        td = 20
    r = td + 50
    g = 2
    pending = []
    if first in ('P1', 'P3', 'fail'):
        pending.append(('A', 1, 'record'))
    if second == 'P4':
        ev += [{'t': r, 'do': 'boot', 'gen': 2, 'cfg': {'settleFence': False}},
               {'t': r + 3, 'do': 'click', 'tab': 'T1'}, {'t': r + 3, 'do': 'click', 'tab': 'T2'}, {'t': r + 9, 'do': 'death', 'gen': 2}]
        pending.append(('F2', 2, 'fence'))
        r, g = r + 9 + 50, 3
    elif second == 'P5':
        ev += [{'t': r, 'do': 'boot', 'gen': 2, 'cfg': {}},
               {'t': r + 3, 'do': 'click', 'tab': 'T1'}, {'t': r + 3, 'do': 'click', 'tab': 'T2'}, {'t': r + 7, 'do': 'death', 'gen': 2}]
        pending.append(('CP2', 2, 'record'))
        r, g = r + 7 + 50, 3
    R, rR = g, r
    ev += [{'t': rR, 'do': 'boot', 'gen': R, 'cfg': {}}, {'t': rR + 3, 'do': 'click', 'tab': 'T1'}, {'t': rR + 3, 'do': 'click', 'tab': 'T2'}]
    if extra == 'yes':
        ev.append({'t': rR + 9, 'do': 'death', 'gen': R})
        W, rW = R + 1, rR + 9 + 50
        ev += [{'t': rW, 'do': 'boot', 'gen': W, 'cfg': {}}, {'t': rW + 3, 'do': 'click', 'tab': 'T1'}, {'t': rW + 3, 'do': 'click', 'tab': 'T2'}]
    else:
        W, rW = R, rR
    ev += [{'t': rW + 10, 'do': 'set', 'tab': 'T2', 'id': 'B', 'k': 'k2', 'v': 'y'},
           {'t': rW + 13, 'do': 'set', 'tab': 'T1', 'id': 'C', 'k': 'k1', 'v': 'z'}]
    pos_t = {'L1': rR - 1, 'L2': rR + 2, 'L3': rR + 5, 'L4': rR + 8, 'L5': rW + 15}
    for label, gid, cat in pending:
        p = L[label]
        if p != 'L0':
            ev.append({'t': pos_t[p], 'do': 'place', 'gen': gid, 'cat': cat, 'action': 'apply'})
    return ev, rW + 200, R, W, [p[0] for p in pending]


def br22a_world(doc, combo, faults=()):
    init = doc['initial']
    items = dict(init['items'])
    if 'singleItem' in faults:
        items = {k: v for k, v in items.items() if not k.startswith('site:')}
        items[SR.single_name('n1', '0xa1')] = {'fmt': 2, 'epoch': 1, 'seq': 1, 'tomb': False, 'dict': dict(init['gen1']['dict'])}
    ev, until, R, W, pend = br22a_events(combo['first'], combo['second'], combo['extraDeath'], combo['L'])
    w = SR.World(items=items, faults=faults)
    g1 = init['gen1']
    w.preload_gen(1, g1['boot'], g1['E'], {'settleRecords': False}, 'SA', g1['dict'], tuple(g1['version']), g1['seqNext'], g1['tabs'])
    w.load(ev)
    w.run(until)
    return w, ev, R, W


def k1_rule(combo):
    f, s, L = combo['first'], combo['second'], combo['L']
    if f == 'fail':
        return 'q'
    if f == 'P0':
        return None
    if f == 'P2':
        return 'x'
    if L.get('A') in ('L1', 'L2') and not (s == 'P5' and L.get('CP2') in ('L1', 'L2')):
        return 'x'
    return None


def e_rule(combo, R, W):
    out = {2: 2}
    if combo['second'] == 'P4':
        out[3] = 3 if combo['L'].get('F2') == 'L1' else 2
    elif combo['second'] == 'P5':
        out[3] = 3
    if combo['extraDeath'] == 'yes' and R in out:
        out[W] = out[R] + 1                      # W sees R's confirmed F(E_R) and takes E_R + 1
    return out


def criteria(w, combo, R, W):
    res = {}
    snaps = w.snapshots
    fin = w.final_dict() or {}
    res['c1'] = all(s['dict'].get('k0') == 'a' for s in snaps) and fin.get('k0') == 'a'
    res['c2'] = fin.get('k1') == 'z' and fin.get('k2') == 'y'
    c_t = [r['t'] for r in w.replies if r['id'] == 'C' and r['code'] is None]
    new = [s for s in snaps if s['gen'] >= 2 and (not c_t or s['t'] < c_t[0])]
    vals = {s['dict'].get('k1') for s in new}
    res['c3'] = bool(new) and vals == {k1_rule(combo)}
    res['c4'] = all(m['replies'] <= 1 for m in w.msgs.values()) and all(r['emitterAlive'] for r in w.replies)
    writers = {}
    for x in w.record_writers:
        writers.setdefault(x['gen'], set()).add(x['E'])
    seq = [writers[g] for g in sorted(writers)]
    res['c5'] = all(len(s) == 1 for s in seq) and all(min(seq[i + 1]) > max(seq[i]) for i in range(len(seq) - 1)) \
        and 'recordNameReused' not in [v['kind'] for v in w.violations]
    res['c6'] = all(s['afterCheckpoint'] for s in snaps)
    kinds = [v['kind'] for v in w.violations]
    res['c7a'] = 'greatestRecordLost' not in kinds and 'removedWithoutGreater' not in kinds
    res['c7b'] = 'fakeBackendBoundExceeded' not in kinds
    res['epochInvariants'] = not ({'epochNamesOverMax', 'maxVisibleDecreased', 'epochResurrected'} & set(kinds))
    return res


def combos():
    for first in ('P0', 'P1', 'P2', 'P3', 'fail'):
        for second in ('none', 'P4', 'P5'):
            for extra in ('no', 'yes'):
                labels = (['A'] if first in ('P1', 'P3', 'fail') else []) + (['F2'] if second == 'P4' else []) + (['CP2'] if second == 'P5' else [])
                for ps in itertools.product(['L0', 'L1', 'L2', 'L3', 'L4', 'L5'], repeat=len(labels)):
                    yield {'first': first, 'second': second, 'extraDeath': extra, 'L': dict(zip(labels, ps))}


def e3_checks():
    doc = load('vectors/e3-br22a.json')
    provenance('e3.br22a', doc['source'])
    n, failing, sigs, coverage = 0, [], set(), {}
    for combo in combos():
        n += 1
        w, ev, R, W = br22a_world(doc, combo)
        res = criteria(w, combo, R, W)
        gs = w.gen_summary()
        e_want = e_rule(combo, R, W)
        e_ok = all(gs.get(g, {}).get('E') == v for g, v in e_want.items() if v is not None)
        viewer_ok = w.clicks == sum(1 for e in ev if e['do'] == 'click') and all(b['text'] for b in w.banners)
        ok = all(res.values()) and e_ok and not w.violations and viewer_ok
        if not ok:
            failing.append({'combo': combo, 'criteria': res, 'E': {g: gs[g]['E'] for g in gs}, 'eWant': e_want,
                            'violations': w.violations[:3]})
        sigs.add(hashlib.sha256(json.dumps([[x['ev'], x.get('gen'), x.get('name'), x.get('version'), x['t']] for x in w.trace]
                                           + [sorted(w.be.items), [s['dict'] for s in w.snapshots]], default=str).encode()).hexdigest())
        k = '%s/%s/%s' % (combo['first'], combo['second'], combo['extraDeath'])
        coverage[k] = coverage.get(k, 0) + 1
    check('e3.br22a.allCombinations', not failing, combinations=n, failing=len(failing), examples=failing[:5])
    record('e3.br22a.coverage', 'recorded', combinations=n, distinctSchedules=len(sigs), perProfile=coverage)

    for ctl in doc['controls']:
        combo = ctl['combo']
        w, ev, R, W = br22a_world(doc, combo, faults=(ctl['fault'],))
        res = criteria(w, combo, R, W)
        failed = sorted(k for k, v in res.items() if not v)
        check('e3.control %s' % ctl['fault'], all(c in failed for c in ctl['mustFail']), failedCriteria=failed, mustFail=ctl['mustFail'],
              violations=[v['kind'] for v in w.violations])
        # the same combination must pass without the fault (the control is meaningful)
        w0, _, R0, W0 = br22a_world(doc, combo)
        res0 = criteria(w0, combo, R0, W0)
        check('e3.control.baselinePasses %s' % ctl['fault'], all(res0.values()), criteria=res0)


def main():
    t0 = time.time()
    for step in (input_hashes, e1_checks, e2_checks, e3_checks):
        try:
            step()
        except Exception as e:                                            # record, never hide
            check('step.completed ' + step.__name__, False, exception='%s: %s' % (type(e).__name__, e))
    failed = [r for r in results if r['status'] == 'FAIL']
    out = {'package': 'M1 draft 0.10 (annex E1-E3: D103/D134 worker-generation recovery)',
           'executedAtUtc': datetime.now(timezone.utc).isoformat(timespec='seconds'),
           'environment': {'python': sys.version.split()[0], 'executable': sys.executable, 'platform': platform.platform(),
                           'thirdPartyPackages': 'none'},
           'evidenceKind': 'SR reference model of the specification text against hand-written expectations; manual clock and fake backend only',
           'notExecuted': ['BR22c (Chrome + CDP; A15c/A15d measurements)', 'TypeScript StoreQueue/StoreRecovery', 'chrome.storage.local behaviour',
                           'fmt 2 codec, RECORD_BYTES_MAX, DiskLedger (E4/E6)', 'browser / MV3 extension'],
           'summary': {'checks': len(results), 'passed': sum(r['status'] == 'pass' for r in results),
                       'recorded': sum(r['status'] == 'recorded' for r in results), 'failed': len(failed),
                       'seconds': round(time.time() - t0, 1)},
           'results': results}
    RES.mkdir(parents=True, exist_ok=True)
    (RES / 'run-results-0.10.json').write_text(json.dumps(out, indent=2, ensure_ascii=True, default=str) + '\n', encoding='utf-8')
    print(json.dumps(out['summary']))
    for f in failed:
        print('FAIL', json.dumps(f, ensure_ascii=True, default=str)[:400])
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
