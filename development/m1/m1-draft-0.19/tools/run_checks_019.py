"""Reference-only runner for M1 draft 0.19: V2 LogClient (D89, D91), LC1-LC18 and variants.
Python standard library only. NOT executed by the author.

Usage, from D:\\PoCol-Development:
  coordination\\runtime\\python311\\python.exe m1-draft-0.19\\tools\\run_checks_019.py
Writes only m1-draft-0.19/results/run-results-0.19.json and exits 1 on any FAIL.

Standalone: it imports only tools/logclient_ref.py of this package. Older packages and results are
hashed (read-only) before and after; no older main() is called and no older result is written.
Expected values come from the hand-written fixtures in ../vectors, never from the model.
"""

import ast
import copy
import hashlib
import itertools
import json
import platform
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
PKG, ROOT = HERE.parent, HERE.parent.parent
RES = PKG / 'results'
OUT = RES / 'run-results-0.19.json'
CORE_STATUS = ('pass', 'recorded', 'FAIL')
RENAMED = {'name': 'diagName', 'check': 'diagCheck', 'status': 'proposalStatus', 'ok': 'diagOk',
           'label': 'diagLabel', 'condition': 'diagCondition'}
VEC = 'm1-draft-0.19/vectors/'
INPUTS = ['coordination/task-017.md', 'coordination/review-001/REVIEW-0.18.md', 'reference/browser.md', 'reference/validation.md',
          'reference/implementation.md', 'reference/FINAL_DESIGN.md',
          'm1-draft-0.19/tools/logclient_ref.py', 'm1-draft-0.19/tools/run_checks_019.py',
          VEC + 'lc-data.json', VEC + 'lc-step-table.json', VEC + 'lc-cases.json', VEC + 'lc-sink-controls.json']
PRESERVED = ['m1-draft-0.15/results/run-results-0.15.json', 'm1-draft-0.16/results/run-results-0.16.json',
             'm1-draft-0.17/results/run-results-0.17.json', 'm1-draft-0.18/results/run-results-0.18.json',
             'coordination/review-001/m1-draft-0.18/results/run-results-0.18.json', 'coordination/issue-ledger.json']
REQUIRED = (['LC%d' % i for i in range(1, 19) if i not in (3, 7, 9, 15)]
            + ['LC3', 'LC3-rate', 'LC3-singleton', 'LC4-edge-ok', 'LC4-edge-over', 'LC7a', 'LC7b', 'LC7c', 'LC7d', 'LC8-beyondAnchor',
               'LC9a', 'LC9b', 'LC9c', 'LC9d', 'LC9e', 'LC9f', 'LC14-variant', 'LC14-inherit', 'LC15a', 'LC15b', 'LC17-variant',
               'X-beyondHead', 'X-unsupported', 'X-otherError', 'X-maxBeforeBegin', 'X-fetchAll-timeout-ok', 'X-fetchAll-timeout-over',
               'X-fetchEach-timeout-ok', 'X-fetchEach-timeout-over', 'X-bridge-range', 'X-bridge-ok'])
TOTALS = {'LC1': 5, 'LC5': 5, 'LC8': 7, 'LC10': 4, 'LC16': 11, 'LC17': 12, 'LC17-variant': 8, 'LC18': 10}   # task-017 literal table

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
    results.append(make_entry(label, core_status, _snap(diag)))      # diagnostics copied at call time (C28 lesson)


def check(label, condition, /, **diag):
    record(label, 'pass' if condition else 'FAIL', **diag)
    return condition


def gap(label, /, **diag):
    record(label, 'recorded', partialGap=True, **diag)


def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest() if p.exists() else None


def load_json(rel):
    return json.loads((ROOT / rel).read_text(encoding='utf-8'))


def compact(v):
    return json.dumps(v, separators=(',', ':'), sort_keys=True, ensure_ascii=False)


def provenance(label, src):
    lines = (ROOT / src['file']).read_text(encoding='utf-8').splitlines()
    a, b = src['lines']
    text = '\n'.join(lines[a - 1:b])
    missing = [lit for lit in src.get('literals', []) if lit not in text]
    check(label + '.provenance', not missing, file=src['file'], lines=src['lines'], missingLiterals=missing)


sys.path.insert(0, str(HERE))
import logclient_ref as M             # noqa: E402  (this package only)


# ------------------------------------------------------------------ fixture resolution (independent of the model)

class Fixture:
    def __init__(self, data):
        self.data = data
        self.hashes = data['hashes']
        self.refs = {k: [self.resolve(x) for x in v] for k, v in data['refs'].items()}

    def resolve(self, v):
        if isinstance(v, str) and v.startswith('@'):
            return self.hashes[v[1:]]
        if isinstance(v, list):
            return [self.resolve(x) for x in v]
        if isinstance(v, dict):
            if 'repeat' in v and 'times' in v:
                return v['prefix'] + v['repeat'] * v['times']
            if 'ref' in v and 'from' in v:
                return [lg for lg in self.refs[v['ref']] if v['from'] <= int(lg['blockNumber'], 16) <= v['to']]
            return {k: self.resolve(x) for k, x in v.items()}
        return v

    def resolve_script_value(self, v):
        """Script replies keep 'gen' and 'repeat' templates for the mock; only '@' literals are substituted."""
        if isinstance(v, str) and v.startswith('@'):
            return self.hashes[v[1:]]
        if isinstance(v, list):
            return [self.resolve_script_value(x) for x in v]
        if isinstance(v, dict):
            return {k: self.resolve_script_value(x) for k, x in v.items()}
        return v


def data_checks(fx, mock):
    provenance('LC-data', fx.data['source'])
    for name, h in fx.hashes.items():
        n = 42 if name == 'ADDR' else 66
        check('LC-data.literal.' + name, isinstance(h, str) and len(h) == n and h.startswith('0x')
              and all(c in '0123456789abcdef' for c in h[2:]), value=h)
    for name, br, h in fx.data['hashRuleChecks']:
        check('LC-data.hashRule.%s@%s' % (name, br), mock.hash_at(br, h) == fx.hashes[name], literal=fx.hashes[name], rule=mock.hash_at(br, h))
    for br, ref in fx.refs.items():
        gen = [mock.make_log(br, blk, i) for blk, _ in fx.data['branches'][br]['logs'] for i in (0, 1)]
        check('LC-data.refEqualsBranchTable.' + br, gen == ref, refCount=len(ref))
        counts = [len(fx.resolve({'ref': br, 'from': a, 'to': b})) for a, b in ((1, 1024), (1025, 2048), (2049, 3000))]
        check('LC-data.windows.' + br, counts == fx.data['windows'][br] == [8, 0, 2], counts=counts)
        check('LC-data.total10.' + br, len(ref) == 10)
    same = [fx.resolve({'ref': x, 'from': 1, 'to': 999}) for x in ('A', 'B', 'Ap')]
    check('LC-data.sharedPrefix1to999', same[0] == same[1] == same[2] and len(same[0]) == 6)
    diff = [fx.resolve({'ref': x, 'from': 1000, 'to': 3000}) for x in ('A', 'B', 'Ap')]
    check('LC-data.divergeFrom1000', diff[0] != diff[1] and diff[0] != diff[2] and diff[1] != diff[2])


# ------------------------------------------------------------------ step table

def concrete(pos, a, b):
    mid = a + (b - a) // 2
    return {'a-2': a - 2, 'a-1': a - 1, 'a': a, 'mid': mid, 'b-1': b - 1, 'b': b, 'b+1': b + 1, 'a+1': a + 1}[pos], mid


def sym(s, a, b, lc, mid):
    return {'a': a, 'b': b, 'lc': lc, 'lc+1': lc + 1, 'mid': mid, 'mid+1': mid + 1}[s]


def step_table(fx, tbl):
    provenance('LC-step', tbl['source'])
    reasons = {'P1': ['resultBytes', 'resultCount', 'scanBytes'], 'RES': ['deadline', 'scratch'], 'UNKNOWN': ['bogus', 'range']}
    n = bad = 0
    fails = []
    for cls, singleton, mark, pos, retries, exp in tbl['limitRows']:
        ranges = [(500, 500), (1, 1)] if singleton else [(1, 1024), (2049, 3000)]
        for (a, b), r in itertools.product(ranges, reasons[cls]):
            lc, mid = concrete(pos, a, b)
            reply = {'error': {'code': -32020, 'data': {'reason': r, 'fromBlock': a, 'lastCompleteBlock': lc, 'nextFromBlock': lc + 1}}}
            got = M.step({'a': a, 'b': b, 'withinLimit': mark, 'retries': retries}, reply)
            want = {'act': exp['act']}
            if exp['act'] in ('replace', 'split'):
                for part in ('first', 'second'):
                    x, y, m = exp[part]
                    want[part] = [sym(x, a, b, lc, mid), sym(y, a, b, lc, mid), mark if m == 'parent' else m]
            elif exp['act'] == 'retry':
                want.update(delayMs=exp['delayMs'], retries=exp['retries'])
            elif exp['act'] == 'error':
                want.update(reason=r, block=a)
            actual = {k: got.get(k) for k in want}
            n += 1
            if actual != want:
                bad += 1
                fails.append({'row': [cls, singleton, mark, pos, retries], 'range': [a, b], 'reason': r, 'want': want, 'got': got})
    check('LC-step.limitTable', bad == 0 and n == 2 * sum(len(reasons[r[0]]) for r in tbl['limitRows']), cells=n, failures=fails[:8])
    record('LC-step.limitTable.size', 'recorded', rows=len(tbl['limitRows']), concreteCells=n)
    mock = M.MockLogServer(fx.data['branches'])
    for row in tbl['otherRows']:
        a, b, mark, retries = row['range']
        reply = mock.expand(fx.resolve_script_value(row['reply']))
        last = tuple(row['last']) if 'last' in row else None
        got = M.step({'a': a, 'b': b, 'withinLimit': mark, 'retries': retries}, reply, last, {})
        want = row['expect']
        check('LC-step.' + row['id'], {k: got.get(k) for k in want} == want, want=want, got={k: v for k, v in got.items() if k != 'logs'})


# ------------------------------------------------------------------ cases

class Tee:
    def __init__(self, *cs):
        self.cs = cs

    def on(self, ev):
        for c in self.cs:
            c.on(ev)


def run_case(fx, data, case, faults=()):
    tl = M.Timeline()
    script = fx.resolve_script_value(case.get('script', {}))
    mock = M.MockLogServer(data['branches'], 'A', data['head'], script, tl)
    clock = M.Clock()
    ref, faulty = M.RefConsumer(), M.FaultyConsumer()
    out = {'mock': mock, 'timeline': tl, 'ref': ref, 'faulty': faulty, 'client': None, 'sink': None}
    if case['mode'] == 'bridge':
        out['bridgeReply'] = M.Bridge(mock, clock).eth_getLogs(case['from'], case['to'])
        return out
    if case['mode'] == 'fetchAll':
        cl, sink, res = M.fetch_all(mock, clock, case['from'], case['to'], faults=faults)
    else:
        params = dict(M.FETCH_EACH)
        params.update(case.get('params', {}))
        cl, sink, res = M.run_client(mock, clock, params, case['from'], case['to'], Tee(ref, faulty), faults)
        res = {'ok': True, 'logs': ref.result} if res['ok'] else {'ok': False, 'error': res['error']}
    out.update(client=cl, sink=sink, result=res)
    return out


def requests_of(mock):
    return [[e['t']] + list(e['req']) for e in mock.log]


def case_cells(fx, case, run):
    cells = [('requests', fx.resolve(case['requests']), requests_of(run['mock']))]
    if case['mode'] == 'bridge':
        cells.append(('bridgeReply', fx.resolve(case['bridgeReply']), run['bridgeReply']))
        return cells
    cells.append(('events', fx.resolve(case['events']), run['sink'].events))
    cells.append(('result', fx.resolve(case['result']), run['result']))
    if 'steps' in case:
        cells.append(('steps', case['steps'], run['client'].steps))
    return cells


def case_suite(fx, data, doc):
    by_id = {}
    for case in doc['cases']:
        cid = case['id']
        by_id[cid] = case
        provenance(cid, case['source'])
        run = run_case(fx, data, case)
        for cell, want, act in case_cells(fx, case, run):
            check('%s.%s' % (cid, cell), want == act, expected=want, actual=act)
        if case['mode'] == 'bridge':
            continue
        viol = M.SinkChecker().check(run['timeline'].entries, case['from'], case['to'])
        check(cid + '.sinkChecker', viol == [], violations=viol)
        res = run['result']
        if res.get('ok'):
            want_logs = fx.resolve(case['result']['logs'])
            check(cid + '.finalLogsSha256', hashlib.sha256(compact(want_logs).encode()).hexdigest()
                  == hashlib.sha256(compact(res['logs']).encode()).hexdigest(),
                  expected=hashlib.sha256(compact(want_logs).encode()).hexdigest(), count=len(res['logs']))
        else:
            check(cid + '.noPartialResult', 'logs' not in res, result=res)
        if cid in TOTALS:
            check(cid + '.literalTotalRequests', len(run['mock'].log) == TOTALS[cid], sent=len(run['mock'].log), literal=TOTALS[cid])
        commits = [e for e in run['sink'].events if e[0] == 'commit']
        if 'pieceCounts' in case:
            got = [len(e[4]) for e in run['sink'].events if e[0] == 'piece' and commits and e[1] == commits[-1][1]]
            check(cid + '.pieceCounts', got == case['pieceCounts'], actual=got)
        if 'pieceBytes' in case:
            got = [len(compact(e[4]).encode('utf-8')) for e in run['sink'].events if e[0] == 'piece']
            check(cid + '.pieceBytes', got == case['pieceBytes'], actual=got)
        if 'perAttempt' in case:
            per = per_attempt(run['timeline'].entries)
            check(cid + '.perAttempt', per == case['perAttempt'], actual=per)
        cons = case.get('consumer')
        if cons:
            check(cid + '.consumer.reference', run['ref'].result == fx.resolve(cons['reference']), count=len(run['ref'].result or []))
            want_disc = [[i, fx.resolve(x)] for i, x in cons['discarded']]
            check(cid + '.consumer.discardedAttempts', run['ref'].discarded == want_disc,
                  discarded=[[i, len(x)] for i, x in run['ref'].discarded])
            if 'faulty' in cons:
                f = cons['faulty']
                want_all = sum((fx.resolve(x) for x in f['logs']), [])
                anchor_ref = fx.refs[f['staleVs']]
                stale = [lg for lg in run['faulty'].all if lg not in anchor_ref]
                check(cid + '.consumer.faultyExposed', len(run['faulty'].all) == f['count'] and run['faulty'].all == want_all
                      and stale == fx.resolve(f['stale']), count=len(run['faulty'].all), staleCount=len(stale),
                      staleHashes=sorted({lg['blockHash'] for lg in stale}))
                check(cid + '.consumer.faultyDiffersFromReference', run['faulty'].all != run['ref'].result)
        # termination bound (browser.md:113): log requests per attempt <= 5*(to-from+1) + busy retries
        bound_ok = all(v['logRequests'] <= 5 * (case['to'] - case['from'] + 1) + v['busy'] for v in attempt_counts(run).values())
        check(cid + '.terminationBound', bound_ok)
    return by_id


def per_attempt(entries):
    out, cur, n = {}, None, {}
    for e in entries:
        if e['kind'] == 'req':
            n.setdefault('requests', 0)
            n['requests'] += 1
            k = 'logRequests' if e['req'][0] == 'L' else 'headRequests'
            n[k] = n.get(k, 0) + 1
        elif e['ev'][0] == 'begin':
            cur = e['ev'][1]
        elif e['ev'][0] in ('abort', 'commit'):
            out[str(e['ev'][1])] = {'requests': n.get('requests', 0), 'logRequests': n.get('logRequests', 0), 'headRequests': n.get('headRequests', 0)}
            n = {}
    return out


def attempt_counts(run):
    per = {}
    attempt = 0
    for e in run['mock'].log:
        if e['req'][0] == 'H' and e['req'][1] == 'latest':
            attempt += 1
        d = per.setdefault(attempt, {'logRequests': 0, 'busy': 0})
        if e['req'][0] == 'L':
            d['logRequests'] += 1
    for s in run['client'].steps if run['client'] else []:
        if s[4] == 'retry':
            per.setdefault(s[0], {'logRequests': 0, 'busy': 0})['busy'] += 1
    return per


def control_suite(fx, data, doc, by_id):
    for c in doc['controls']:
        case = by_id[c['case']]
        good = {cell: (w, a) for cell, w, a in case_cells(fx, case, run_case(fx, data, case))}
        bad = {cell: (w, a) for cell, w, a in case_cells(fx, case, run_case(fx, data, case, faults=(c['fault'],)))}
        mism = sorted(cell for cell, (w, a) in bad.items() if w != a)
        check('control.%s.%s' % (c['fault'], c['case']), all(w == a for w, a in good.values()) and set(c['witness']) <= set(mism),
              mismatchingCells=mism, witness=c['witness'])


# ------------------------------------------------------------------ SinkChecker controls

def mutate(entries, mid):
    es = copy.deepcopy(entries)
    sinks = [e for e in es if e['kind'] == 'sink']

    def find(kind, i=None, seq=None):
        for e in sinks:
            ev = e['ev']
            if ev[0] == kind and (i is None or ev[1] == i) and (seq is None or ev[2] == seq):
                return e
        raise KeyError((kind, i, seq))

    if mid == 'dupBegin':
        b = find('begin', 1)
        es.insert(es.index(b) + 1, dict(copy.deepcopy(b), seq=b['seq'] + 0.5))
    elif mid == 'noBegin':
        es.remove(find('begin', 1))
    elif mid == 'pieceSeqGap':
        find('piece', 1, 2)['ev'][2] = 3
        find('piece', 1, 1)['ev'][2] = 2
    elif mid == 'dropTerminal':
        es.remove(find('abort', 1))
    elif mid == 'pieceAfterCommit':
        cm = find('commit', 1)
        es.append({'seq': cm['seq'] + 0.5, 'kind': 'sink', 'ev': ['piece', 1, 3, [3001, 3001], []]})
    elif mid == 'skipId':
        for e in sinks:
            if e['ev'][1] == 2:
                e['ev'][1] = 3
    elif mid == 'twoCommits':
        ab = find('abort', 1)
        ab['ev'] = ['commit', 1, copy.deepcopy(find('commit', 2)['ev'][2])]
    elif mid == 'commitNotLast':
        cm = find('commit', 1)
        anchor = copy.deepcopy(find('begin', 1)['ev'][2])
        es.append({'seq': cm['seq'] + 0.25, 'kind': 'sink', 'ev': ['begin', 2, anchor]})
        es.append({'seq': cm['seq'] + 0.5, 'kind': 'sink', 'ev': ['abort', 2, 'reorg']})
    elif mid == 'gap':
        find('piece', 1, 1)['ev'][3] = [1026, 2048]
    elif mid == 'overlap':
        find('piece', 1, 1)['ev'][3] = [1024, 2048]
    elif mid == 'summaryLogs':
        find('commit', 1)['ev'][2]['logs'] = 9
    elif mid == 'headCount':
        find('commit', 1)['ev'][2]['headRequests'] = 1
    elif mid == 'totalExcludesAborted':
        find('commit', 2)['ev'][2]['totalRequests'] = 5
    elif mid != 'baseline':
        raise ValueError(mid)
    return es


def sink_controls(fx, data, doc, by_id):
    provenance('LC-sink', doc['source'])
    for c in doc['controls']:
        base = by_id[c['base']]
        run = run_case(fx, data, base)
        mid = 'baseline' if c['mutation'] == 'none' else c['id']
        viol = M.SinkChecker().check(mutate(run['timeline'].entries, mid), base['from'], base['to'])
        rules = sorted({v['rule'] for v in viol})
        check('sinkControl.' + c['id'], rules == sorted(c['rules']), expected=c['rules'], actual=rules, violations=viol[:6])


# ------------------------------------------------------------------ constants, exposure, gaps

def constants_and_exposure():
    src = {'file': 'reference/browser.md', 'lines': [134, 139], 'literals': ['4 MiB', '30s', 'maxRequests = 4096', '600s', 'maxRequests = 65536', 'ولا يتاح للصفحات']}
    provenance('LC-const', src)
    check('LC-const.fetchAll', M.FETCH_ALL == {'maxBytes': 4 * 1024 * 1024, 'timeoutMs': 30000, 'maxRequests': 4096}, actual=M.FETCH_ALL)
    check('LC-const.fetchEach', M.FETCH_EACH == {'maxBytes': None, 'timeoutMs': 600000, 'maxRequests': 65536}, actual=M.FETCH_EACH)
    check('LC-const.windowRestartsRetries', M.WINDOW == 1024 and M.MAX_RESTARTS == 3 and M.SINGLETON_RETRIES == 3 and M.SINGLETON_RETRY_MS == 500)
    public = sorted(n for n in dir(M.Bridge) if not n.startswith('_'))
    check('LC-exposure.bridgeOnlyEthGetLogs', public == ['eth_getLogs'], publicMethods=public)
    tree = ast.parse((HERE / 'logclient_ref.py').read_text(encoding='utf-8'))
    bridge = next(n for n in tree.body if isinstance(n, ast.ClassDef) and n.name == 'Bridge')
    names = {x.id for x in ast.walk(bridge) if isinstance(x, ast.Name)} | {x.attr for x in ast.walk(bridge) if isinstance(x, ast.Attribute)}
    check('LC-exposure.bridgeNeverReachesFetchEach', 'fetch_each' not in names and 'run_client' not in names and 'fetch_all' in names)


GAPS = [
    ('P-V2-1', 'maxRequests exhausted before a new attempt H(latest): no attempt id is open, so the model returns Error{tooManyRequests} without abort/begin (browser.md:107 names abort(id, ...)). Proposed supplement; case X-maxBeforeBegin.'),
    ('P-V2-2', 'Client-side consistency of -32020 fields (fromBlock = a, nextFromBlock = lc+1) treated as serverViolation; not written in step text (FINAL_DESIGN.md:2050-2052 states the server shape). Proposed supplement.'),
    ('P-V2-3', 'Malformed -32021 (no integer retryAfterMs / unknown reason) and -32022 with an unknown reason -> serverViolation. Proposed supplement.'),
    ('P-V2-4', 'Timeouts are inclusive and checked at each send: a request due at elapsed = timeout is sent; elapsed > timeout -> abort(timeout). Proposed convention.'),
    ('P-V2-5', 'maxBytes measured as UTF-8 bytes of the compact key-sorted JSON array per piece, cumulative per attempt; real wire bytes may differ. Proposed convention.'),
    ('P-V2-6', 'A malformed head reply before begin -> Error{headViolation} with no sink call. Proposed convention.'),
    ('P-V2-7', 'Branch block hashes, address and data tags are author-chosen literals; the source fixes structure, not values. Proposal.'),
    ('P-V2-8', 'The model waits exactly retryAfterMs / 500 ms; the source requires >= . Exact timing is a fixture choice.'),
    ('V2-scope', 'Model only: no TypeScript LogClient, no Chrome, no pocold/RPC server (RG3, RG3b, RG9 not executed), no Node fetchEach tool, no BridgeAuth schema/tag resolution.'),
]


def coverage(by_id):
    have = {r['check']: r['status'] for r in results}
    for cid in REQUIRED:
        own = [s for k, s in have.items() if k.startswith(cid + '.') and not k.endswith('.provenance')]
        check('coverage.' + cid, cid in by_id and own and all(s == 'pass' for s in own), executedChecks=len(own))
    check('coverage.allTotalsAsserted', all(('%s.literalTotalRequests' % k) in have for k in TOTALS))
    ctl = [k for k in have if k.startswith('control.')]
    check('coverage.controls', len(ctl) == 9, controls=sorted(ctl))
    sc = [k for k in have if k.startswith('sinkControl.')]
    check('coverage.sinkControls', len(sc) == 15, sinkControls=len(sc))
    check('coverage.noStepAborted', not [k for k in have if 'step.completed' in k])


def main():
    t0 = time.time()
    before = {rel: sha(ROOT / rel) for rel in PRESERVED}
    for rel in INPUTS:
        record('input.sha256 ' + rel, 'recorded', exists=(ROOT / rel).exists(), sha256=sha(ROOT / rel))
    check('boundary.positionalOnly', record.__code__.co_posonlyargcount == 2 and check.__code__.co_posonlyargcount == 2
          and gap.__code__.co_posonlyargcount == 1)
    probe = {'x': [1]}
    record('boundary.probe', 'recorded', name='n', status='s', obj=probe)
    probe['x'].append(2)
    e = results[-1]
    check('boundary.renamedAndCopied', e['diagName'] == 'n' and e['proposalStatus'] == 's' and e['status'] == 'recorded' and e['obj'] == {'x': [1]})
    calls = [n.lineno for n in ast.walk(ast.parse(Path(__file__).read_text(encoding='utf-8')))
             if isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute) and n.func.attr == 'main']
    check('boundary.noOldMainCalled', not calls, attributeMainCallsAtLines=calls)
    by_id = {}
    data = load_json(VEC + 'lc-data.json')
    fx = Fixture(data)
    steps = [('data', lambda: data_checks(fx, M.MockLogServer(data['branches']))),
             ('stepTable', lambda: step_table(fx, load_json(VEC + 'lc-step-table.json'))),
             ('constants', constants_and_exposure)]
    for name, fn in steps:
        try:
            fn()
        except Exception as ex:                                              # record, never hide
            check('step.completed ' + name, False, exception='%s: %s' % (type(ex).__name__, ex))
    try:
        cases = load_json(VEC + 'lc-cases.json')
        by_id = case_suite(fx, data, cases)
        control_suite(fx, data, cases, by_id)
        sink_controls(fx, data, load_json(VEC + 'lc-sink-controls.json'), by_id)
    except Exception as ex:                                                  # record, never hide
        check('step.completed cases', False, exception='%s: %s' % (type(ex).__name__, ex))
    for gid, text in GAPS:
        gap('gap.' + gid, text=text)
    coverage(by_id)
    after = {rel: sha(ROOT / rel) for rel in PRESERVED}
    check('preserved.olderFilesUnchanged', before == after, before=before, after=after)
    failed = [r for r in results if r['status'] == 'FAIL']
    out = {'package': 'M1 draft 0.19 (V2 LogClient: step table, LC1-LC18 and variants, SinkChecker, collectors)',
           'executedAtUtc': datetime.now(timezone.utc).isoformat(timespec='seconds'),
           'environment': {'python': sys.version.split()[0], 'executable': sys.executable, 'platform': platform.platform(),
                           'thirdPartyPackages': 'none'},
           'statusSchema': {'coreStatus': list(CORE_STATUS), 'diagnosticRenames': RENAMED, 'partialGap': 'recorded entries with partialGap=true'},
           'evidenceKind': 'reference model of the specification text (manual clock, MockLogServer); not TypeScript, not Chrome, not pocold',
           'notExecuted': ['TypeScript LogClient', 'Chrome bridge', 'pocold pocol_getLogs (RG3, RG3b, RG9)', 'Node fetchEach tool'],
           'summary': {'checks': len(results), 'passed': sum(r['status'] == 'pass' for r in results),
                       'recorded': sum(r['status'] == 'recorded' for r in results), 'failed': len(failed),
                       'partialGaps': sum(1 for r in results if r.get('partialGap')), 'seconds': round(time.time() - t0, 1)},
           'results': results}
    RES.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(out, indent=2, ensure_ascii=True, default=str) + '\n', encoding='utf-8')
    print(json.dumps(out['summary']))
    for f in failed:
        print('FAIL', json.dumps(f, ensure_ascii=True, default=str)[:400])
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
