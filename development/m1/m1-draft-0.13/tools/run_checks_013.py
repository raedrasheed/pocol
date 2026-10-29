"""Reference-only runner for M1 draft 0.13 (annex rows E4-E5). Python standard library only.
NOT executed by the author.

Usage, from D:\\PoCol-Development:
  coordination\\runtime\\python311\\python.exe m1-draft-0.13\\tools\\run_checks_013.py
Writes only m1-draft-0.13/results/run-results-0.13.json. Earlier runners are imported as
modules; their main() is never called.

Steps:
1. Input SHA-256 values (recorded) and provenance of every golden block.
2. Wiring: the engine uses the 0.12 strict helpers (identity, and behaviourally: an LF-tailed
   record name is invisible to recovery) and the 0.12 C25 flags are live.
3. E4 units: fmt-2 known answers (hex and base64, hand computed), rejections, the surrogate
   collision, value checks, the maximum record, DiskLedger boundaries and ticket lifecycle.
4. E5 schedules: every row of every case against the engine; full raw dictionaries; controls
   (each must be detected no later than its stated time).
5. Regression: the full 0.12 suite re-run in memory (and through it 0.11 and 0.10), prefixed
   'inherited012.'.
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
sys.path.insert(0, str(ROOT / 'm1-draft-0.12' / 'tools'))
import sr_ref as SR12                    # noqa: E402  (0.12, bound as 'sr_ref')
sys.path.insert(0, str(HERE))
import fmt2_codec as C                   # noqa: E402
import recovery_ref as R                 # noqa: E402

results = []
INPUTS = [
    'reference/browser.md', 'reference/validation.md', 'reference/governance.md', 'reference/implementation.md',
    'm1-draft-0.2/M1-ANNEX-CHECKLIST-0.2.md', 'm1-draft-0.12/tools/sr_ref.py', 'm1-draft-0.12/tools/run_checks_012.py',
    'm1-draft-0.12/vectors/c25-namespace.json', 'coordination/review-001/REVIEW-0.11.md',
    'm1-draft-0.13/tools/fmt2_codec.py', 'm1-draft-0.13/tools/recovery_ref.py', 'm1-draft-0.13/tools/run_checks_013.py',
    'm1-draft-0.13/vectors/e4-units.json', 'm1-draft-0.13/vectors/e5-schedules.json',
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


def wiring():
    check('wiring.engineUsesStrict012Helpers', R.SR12 is SR12 and sys.modules.get('sr_ref') is SR12)
    sc = SR12.self_check()
    check('wiring.c25FlagsLive', sc['attemptUsesRawNamespace'] and sc['observeUsesRawNamespace'] and sc['worldEpochsIsStrict'])
    # behavioural: a valid-looking but LF-tailed record name must be invisible to recovery
    good = SR12.record_name('nA', '0xa', 1, 3)
    items = {good: {'fmt': 2, 'epoch': 1, 'seq': 3, 'tomb': False, 'b64': 'AAAAAmswAAAAAWE='},
             SR12.record_name('nA', '0xa', 9, 0) + '\n': {'fmt': 2, 'epoch': 9, 'seq': 0, 'tomb': False, 'b64': ''}}
    e = R.Engine({'K': ['nA', '0xa']}, records=items)
    e.load([{'t': 0, 'do': 'snap', 'key': 'K', 'frame': 'F'}, {'t': 10, 'do': 'settle', 'set': 1, 'ok': True}])
    e.run(20)
    check('wiring.recoveryIgnoresLfTailedName', [list(o.version) for o in e.sets] == [[2, 0]] and e.snapshots
          and e.snapshots[0]['dict'] == {'k0': 'a'}, versions=[list(o.version) for o in e.sets])


# ------------------------------------------------------------------ E4 units

def _rule_dict(rule):
    if 'entries' in rule:
        return {'K%04x' % i: '' for i in range(rule['entries'])}
    if rule.get('maxPlusOne'):
        d = max_dict()
        d['K0000'] = 'a' * 252
        return d
    def expand(s):
        if '*' in s:
            ch, n = s.split('*')
            return ch * int(n)
        return s
    return {expand(rule['key']): expand(rule['value'])}


def max_dict():
    return {'K%04x' % i: 'a' * 251 for i in range(4096)}


def _lit(x):
    if isinstance(x, str) and x.startswith('2**'):
        return 2 ** int(x[3:])
    return int(x) if isinstance(x, str) else x


def e4_checks():
    u = load('vectors/e4-units.json')
    for i, s in enumerate(u['sources']):
        provenance('e4.source%d' % i, s)
    provenance('e4.disk', u['disk']['source'])
    k = u['constants']
    check('e4.constants', C.RECORD_BYTES_MAX == k['RECORD_BYTES_MAX'] and R.LATE_RESERVE == k['LATE_RESERVE'] and R.LATE_NAMES == k['LATE_NAMES']
          and R.EPOCH_NAMES_MAX == k['EPOCH_NAMES_MAX'] and R.META_RESERVE == k['META_RESERVE'] and R.GATE_BOUND == k['GATE_BOUND']
          and R.ADMIN_BOUND == k['ADMIN_BOUND'],
          got=[C.RECORD_BYTES_MAX, R.LATE_RESERVE, R.LATE_NAMES, R.GATE_BOUND, R.ADMIN_BOUND])
    for ka in u['knownAnswers']:
        b = C.serialize_pairs(ka['dict'])
        ok = b.hex() == ka['pairsHex']
        if 'b64' in ka:
            ok = ok and C.b64_encode(b) == ka['b64'] and C.parse_pairs(C.b64_decode_strict(ka['b64'])) == ka['dict']
        check('e4.knownAnswer ' + ka['id'], ok, gotHex=b.hex(), gotB64=C.b64_encode(b))
    for r in u['serializeRejects']:
        d = r['dict'] if 'dict' in r else _rule_dict(r['rule'])
        try:
            C.serialize_pairs(d)
            got = None
        except C.CodecError as e:
            got = e.reason
        check('e4.serializeReject ' + r['id'], got == r['reason'], got=got)
    sc = u['surrogateCollision']
    try:
        C.serialize_pairs(sc['dict'])
        strict = None
    except C.CodecError as e:
        strict = e.reason
    lossy = C.serialize_pairs_textencoder(sc['dict'])
    try:
        C.parse_pairs(lossy)
        dec = None
    except C.CodecError as e:
        dec = e.reason
    check('e4.surrogateCollision', strict == sc['strictReason'] and lossy.hex() == sc['textEncoderPairsHex'] and dec == sc['decodeOfTextEncoderBytes'],
          strict=strict, lossyHex=lossy.hex(), decode=dec)
    record('e4.surrogateCollision.status', 'recorded', status=sc['status'])
    for r in u['parseRejects']:
        try:
            C.parse_pairs(bytes.fromhex(r['pairsHex']))
            got = None
        except C.CodecError as e:
            got = e.reason
        check('e4.parseReject ' + r['id'], got == r['reason'], got=got)
    for r in u['base64Rejects']:
        try:
            C.b64_decode_strict(r['b64'])
            got = None
        except C.CodecError as e:
            got = e.reason
        check('e4.base64Reject ' + r['id'], got == r['reason'], got=got)
    for r in u['valueChecks']:
        try:
            d, _ = C.decode_value(tuple(r['ver']), r['value'])
            got = ('ok', d)
        except C.CodecError as e:
            got = ('err', e.reason)
        want = ('ok', r['dict']) if 'dict' in r else ('err', r['reason'])
        check('e4.value ' + r['id'], got == want, got=list(got))
    for r in u['encodeRejects']:
        try:
            C.encode_value(_lit(r['E']), r['seq'], r.get('dict', {}), r.get('tomb', False))
            got = None
        except C.CodecError as e:
            got = e.reason
        check('e4.encodeReject ' + r['id'], got == r['reason'], got=got)
    m = u['maxRecord']
    d = max_dict()
    v = C.encode_value(m['epoch'], m['seq'], d)
    name = SR12.record_name('0x' + '1' * 64, '0x' + '2' * 40, m['epoch'], m['seq'])
    enc = C.enc_size(name, v)
    check('e4.maxRecord', len(v['b64']) == m['b64Length'] and len(C.compact_json(v)) == m['valueJsonLength'] and len(name) == 142
          and enc == m['encSize'] and (enc <= C.RECORD_BYTES_MAX) == m['withinRecordBytesMax'],
          b64=len(v['b64']), json=len(C.compact_json(v)), name=len(name), enc=enc)
    for a in u['disk']['accept']:
        led = R.DiskLedger(in_use=a['inUse'], names=a['names'])
        t, bad = led.reserve(a['enc'], a['kind'])
        ok = (bad is None) == a['ok'] and (a['ok'] or bad.get('dim') == a.get('dim'))
        check('e4.disk ' + a['id'], ok, bad=bad)
    led = R.DiskLedger()
    tickets, lc_ok, notes = [], True, []
    for step in u['disk']['lifecycle']:
        ex = step['expect']
        if step['do'] == 'reserve':
            t, _ = led.reserve(step['enc'])
            tickets.append(t)
            got = {'rBytes': led.r_bytes, 'rNames': led.r_names}
        elif step['do'] == 'refreshIssue':
            led.refresh_issue()
            got = {'refreshing': led.refreshing is not None}
        elif step['do'] == 'refreshIssueWhileBusy':
            rv = led.refresh_issue()
            got = {'returned': rv, 'pending': led.pending_refresh}
        elif step['do'] == 'settle':
            led.settled(tickets[step['ticket'] - 1])
            got = {'settleSeq1': tickets[0]['settleSeq']}
        else:
            nxt = led.refresh_complete(step['inUse'], step['names'])
            got = {'rBytes': led.r_bytes, 'rNames': led.r_names}
            if 'nextRefreshIssued' in ex:
                got['nextRefreshIssued'] = nxt is not None
        want = {k: v for k, v in ex.items() if k != 'why'}
        if got != want:
            lc_ok = False
            notes.append({'step': step['do'], 'got': got, 'want': want})
    check('e4.disk.ticketLifecycle', lc_ok, mismatches=notes)


# ------------------------------------------------------------------ E5 schedules

def build(doc, case, faults=()):
    items = {}
    for rid in case.get('records', []):
        r = doc['records'][rid]
        items[SR12.record_name(*r['name'])] = copy.deepcopy(r['value'])
    for r in case.get('rawRecords', []):
        items[SR12.record_name(*r['name'])] = copy.deepcopy(r['value'])
    cfg = case.get('config', {})
    e = R.Engine(case['sites'], records=items, faults=faults, hold_reads=cfg.get('holdReads', False),
                 hold_removes=cfg.get('holdRemoves', False), seq_start=cfg.get('seqStart', 0))
    e.load(case['events'])
    return e


def observe(e, field, want):
    if field == 'sets':
        return len(e.sets)
    if field == 'versions':
        return [list(o.version) for o in e.sets]
    if field in ('status', 'outcome', 'gate', 'resolvedAt', 'seqNext'):
        src = {'status': 'status', 'outcome': 'outcome', 'gate': 'gate', 'resolvedAt': 'resolvedAt', 'seqNext': 'seq'}[field]
        return {k: e.keys[k][src] for k in want}
    if field == 'confirmed':
        return {k: (list(e.keys[k]['confirmed']) if e.keys[k]['confirmed'] else None) for k in want}
    if field == 'held':
        return len(e.rec_held)
    if field == 'unsettled':
        return e.unsettled_recovery()
    if field == 'readHeld':
        return e.read_held
    if field == 'reads':
        return len(e.reads)
    if field == 'replies':
        return [[r['id'], r['code'], r['reason'], r['sub'], r['t']] for r in e.replies]
    if field == 'snapshots':
        return [[s['t'], s['key'], s['frame'], s['dict']] for s in e.snapshots]
    if field == 'records':
        return {k: e.backend_records(k) for k in want}
    if field == 'dict':
        return {k: C.lookup_fmt2(e.items, *e.sites[k], SR12.parse_record_name).get('dict') for k in want}
    if field in ('lateRecordsSeen', 'recoveryBusy', 'readTimeouts', 'lateReadsDropped', 'recoveryCorrupt'):
        return e.stats[field]
    if field == 'unsettledMax':
        return e.stats['recoveryUnsettledMax']
    if field == 'tickets':
        return len(e.disk.tickets)
    if field == 'tombs':
        return [list(o.version) for o in e.sets if o.purpose == 'tomb']
    raise KeyError(field)


def final_obs(e, field, want):
    if field == 'violations':
        return [v['kind'] for v in e.violations]
    if field == 'distinctSetNames':
        return len({o.name for o in e.sets}) == len(e.sets)
    if field == 'itemsDecoded':
        out = {}
        for n, v in e.items.items():
            for k, (net, addr) in e.sites.items():
                ver = SR12.parse_record_name(n, net, addr)
                if ver:
                    out[n] = C.decode_value(ver, v)[0]
        return out
    if field == 'largestRecord':
        return {k: list(C.lookup_fmt2(e.items, *e.sites[k], SR12.parse_record_name)['version']) for k in want}
    if field == 'setNames':
        return [o.name for o in e.sets]
    if field in ('dict', 'records'):
        return observe(e, field, want)
    if field == 'resolvedWithin':
        return {k: (e.keys[k]['resolvedAt'] - e.keys[k]['firstReqAt'] <= b) and b for k, b in want.items()}
    if field == 'nextGenLookup':
        out = {}
        for k in want:
            r = C.lookup_fmt2(e.items, *e.sites[k], SR12.parse_record_name)
            out[k] = {'version': list(r['version']) if r.get('version') else None, 'dict': r.get('dict')}
        return out
    if field == 'corruptReasons':
        return [x['reason'] for x in e.trace if x['ev'] == 'corrupt']
    if field == 'readSlotsHeldMax':
        return e.stats['readSlotsHeldMax']
    if field == 'deleteResolvedBy':
        ts = [x['t'] for x in e.trace if x['ev'] == 'tombSettled']
        return ts[0] if ts else None
    if field == 'adminBound':
        req = [x['t'] for x in e.trace if x['ev'] == 'deleteRequested']
        ts = [x['t'] for x in e.trace if x['ev'] == 'tombSettled']
        return want if (req and ts and ts[0] - req[0] <= want) else None
    raise KeyError(field)


def run_case(doc, case, faults=()):
    e = build(doc, case, faults)
    mism = []
    for row in sorted(case['rows'], key=lambda r: r['t']):
        try:
            e.run(row['t'])
        except Exception as ex:                                            # record, never hide
            mism.append({'t': row['t'], 'field': 'exception', 'got': '%s: %s' % (type(ex).__name__, ex)})
            return e, mism
        for f, want in row.items():
            if f == 't':
                continue
            got = observe(e, f, want)
            if got != want:
                mism.append({'t': row['t'], 'field': f, 'want': want, 'got': got})
    end = max([ev['t'] for ev in case['events']] + [r['t'] for r in case['rows']])
    try:
        e.run(end)
        for f, want in (case.get('final') or {}).items():
            got = final_obs(e, f, want)
            if got != want:
                mism.append({'t': 'final', 'field': f, 'want': want, 'got': got})
    except Exception as ex:                                                # record, never hide
        mism.append({'t': 'final', 'field': 'exception', 'got': '%s: %s' % (type(ex).__name__, ex)})
    return e, mism


def e5_checks():
    doc = load('vectors/e5-schedules.json')
    by_id = {c['id']: c for c in doc['cases']}
    for c in doc['cases']:
        provenance('e5.' + c['id'], c['source'])
        e, mism = run_case(doc, c)
        check('e5.case ' + c['id'], not mism, row=c['row'], mismatches=mism[:8])
        record('e5.recorded ' + c['id'], 'recorded', stats=e.stats, backendSha256=e.backend_hash(),
               sets=[[o.n, o.purpose, list(o.version), o.state, o.settled] for o in e.sets], trace=e.trace[:80])
    for ctl in doc['controls']:
        c = by_id[ctl['case']]
        e, mism = run_case(doc, c, faults=(ctl['fault'],))
        early = [m for m in mism if m['t'] != 'final' and m['t'] <= ctl['detectBy']]
        kinds = [v['kind'] for v in e.violations]
        ok = bool(early) and (('violation' not in ctl) or ctl['violation'] in kinds)
        check('e5.control %s on %s' % (ctl['fault'], ctl['case']), ok, firstMismatch=mism[0] if mism else None, violations=kinds)


# ------------------------------------------------------------------ inherited 0.12 suite

def inherited_012():
    path = ROOT / 'm1-draft-0.12' / 'tools' / 'run_checks_012.py'
    spec = importlib.util.spec_from_file_location('run_checks_012_rerun', str(path))
    R12 = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(R12)            # its 'import sr_ref' resolves to the cached 0.12 module
    check('inherited012.sameModule', R12.SR is SR12)
    doc = json.loads((ROOT / 'm1-draft-0.12' / 'vectors' / 'c25-namespace.json').read_text(encoding='utf-8'))
    pristine011 = SR12.load_pristine_011()
    t0 = time.time()
    for name, step in [('input_hashes', R12.input_hashes), ('provenance', lambda: R12.provenance(doc['source'])), ('wiring', R12.wiring),
                       ('c25_repro', lambda: R12.c25_repro(doc, pristine011)), ('namespace_cases', lambda: R12.namespace_cases(doc, pristine011)),
                       ('inherited_011', lambda: R12.inherited_011(doc))]:
        try:
            step()
        except Exception as ex:                                            # record, never hide
            R12.check('step.completed ' + name, False, exception='%s: %s' % (type(ex).__name__, ex))
    for r in R12.results:
        r = dict(r)
        r['check'] = 'inherited012.' + r['check']
        results.append(r)
    st = [r['status'] for r in R12.results]
    record('inherited012.summary', 'recorded', checks=len(st), passed=st.count('pass'), recorded=st.count('recorded'),
           failed=st.count('FAIL'), seconds=round(time.time() - t0, 1), note='0.12 accepted as tooling: 237 pass, 64 recorded, 0 fail (task-011)')


def main():
    t0 = time.time()
    for name, step in [('input_hashes', input_hashes), ('wiring', wiring), ('e4_checks', e4_checks), ('e5_checks', e5_checks),
                       ('inherited_012', inherited_012)]:
        try:
            step()
        except Exception as ex:                                            # record, never hide
            check('step.completed ' + name, False, exception='%s: %s' % (type(ex).__name__, ex))
    failed = [r for r in results if r['status'] == 'FAIL']
    out = {'package': 'M1 draft 0.13 (annex E4-E5: RecoveryGate, slots, SeqAlloc, fmt 2, DiskLedger; BR22d-g)',
           'executedAtUtc': datetime.now(timezone.utc).isoformat(timespec='seconds'),
           'environment': {'python': sys.version.split()[0], 'executable': sys.executable, 'platform': platform.platform(),
                           'thirdPartyPackages': 'none'},
           'evidenceKind': 'reference model of the specification text against hand-written expectations; manual clock and FakeBackend; model byte metrics are not Chrome bytes',
           'notExecuted': ['BR22c (Chrome + CDP)', 'TypeScript implementation', 'chrome.storage getBytesInUse/getKeys', 'JS Number handling of u64 fields',
                           'TextEncoder/TextDecoder behaviour (only the root Node observation is cited)', 'AdminDelete/AdminSlots/TombReaper/sites (E6/E7)'],
           'summary': {'checks': len(results), 'passed': sum(r['status'] == 'pass' for r in results),
                       'recorded': sum(r['status'] == 'recorded' for r in results), 'failed': len(failed),
                       'seconds': round(time.time() - t0, 1)},
           'results': results}
    RES.mkdir(parents=True, exist_ok=True)
    (RES / 'run-results-0.13.json').write_text(json.dumps(out, indent=2, ensure_ascii=True, default=str) + '\n', encoding='utf-8')
    print(json.dumps(out['summary']))
    for f in failed:
        print('FAIL', json.dumps(f, ensure_ascii=True, default=str)[:400])
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
