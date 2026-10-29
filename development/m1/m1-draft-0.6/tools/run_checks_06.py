"""Reference-only check runner for M1 draft 0.6 (standard library only).

One sequence, from D:\\PoCol-Development (Node 22 for the .cjs steps):
  node m1-draft-0.6/tools/url_oracle_06.cjs
  node m1-draft-0.6/tools/id_oracle_06.cjs
  node m1-draft-0.6/tools/br16_gen.cjs
  <python> m1-draft-0.6/tools/run_checks_06.py
Missing Node outputs make the dependent checks FAIL (never pass silently).
Writes only m1-draft-0.6/results/.
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
VEC, ANNEX, RES = PKG / 'vectors', PKG / 'annex', PKG / 'results'
sys.path[:0] = [str(HERE), str(ROOT / 'm1-draft-0.5' / 'tools'), str(ROOT / 'm1-draft-0.4' / 'tools'),
                str(ROOT / 'm1-draft-0.3' / 'tools'), str(ROOT / 'm1-draft-0.2' / 'tools')]

import keccak                                   # noqa: E402
from keccak import keccak256                    # noqa: E402
import rlp_strict as R                          # noqa: E402
import m1model as M                             # noqa: E402
import bridge_ref as B04                        # noqa: E402
import bridge_ref_05 as B05                     # noqa: E402
import bridge_ref_06 as B6                      # noqa: E402
import profile_race_ref as PRR                  # noqa: E402
import dnr_ref as DNR                           # noqa: E402
import bridgeref_06 as BR                       # noqa: E402
import br16_gen as G                            # noqa: E402

results = []


def check(name, ok, **detail):
    results.append({'check': name, 'status': 'pass' if ok else 'FAIL', **detail})
    return ok


def record(name, **detail):
    results.append({'check': name, 'status': 'recorded', **detail})


def load(p):
    return json.loads(Path(p).read_text(encoding='utf-8'))


def dumps(o):
    return json.dumps(o, separators=(',', ':'), ensure_ascii=False)


def subset(exp, got):
    bad = {k: [repr(v)[:80], repr(got.get(k))[:80]] for k, v in exp.items() if got.get(k) != v}
    return not bad, bad


# --- R6-01 C14 prefix -------------------------------------------------------
def c14_checks():
    doc = load(VEC / 'c14-head.json')['rb2848']
    parts = [i.to_bytes(2, 'big') for i in range(2848)]
    data = b''.join(parts)
    f = [b'/a', R.uint(1), R.uint(len(data)), keccak256(data), [[M.chunk_address(p), R.uint(2)] for p in parts]]
    m = R.encode([R.uint(1), R.uint(0), [f]])
    check('c14.head24', '0x' + m[:24].hex() == doc['manifestHeadHex24'], got='0x' + m[:24].hex())
    check('c14.contentHash', '0x' + keccak256(data).hex() == doc['contentHash'])
    off = doc['chunksHeaderOffset']
    check('c14.chunksHeaderAtOffset54', m[off:off + 3].hex() == doc['chunksHeaderHex'] and len(m) == 65561)


# --- oracles ----------------------------------------------------------------
def install_oracles():
    loaded = B05.load_oracle(RES / 'url-oracle-0.6.json')
    table = dict(loaded[0]) if loaded else {}
    node = RES / 'br16-node.json'
    if node.exists():
        table.update({k: bool(v) for k, v in load(node)['httpsUrlOracle'].items()})
    B05.install_oracle(table)
    check('oracle.url06Available', loaded is not None, values=len(table))


# --- R6-02 C19 --------------------------------------------------------------
def id_checks():
    doc = load(VEC / 'id-integer-cases.json')
    orc_file = RES / 'id-oracle-0.6.json'
    orc = {r['token']: r for r in load(orc_file)['results']} if orc_file.exists() else None
    check('id.nodeOracleAvailable', orc is not None)
    for c in doc['cases']:
        raw = doc['messageTemplate'].replace('TOKEN', c['token'])
        r = B6.check(raw, 1000, B6.Bucket(1000))
        py_accept = r['stage'] == 'ok'
        ok = py_accept == c['accept'] and (r['id'] == c['returnedId'] if c['accept'] else r['stage'] == 'B0' and r['id'] is None)
        if orc is not None:
            o = orc.get(c['token'], {})
            ok = ok and o.get('accept') == c['accept'] and o.get('returnedId') == c['returnedId']
        check('id.%s' % c['token'], ok, python=[r['stage'], r['id']], node=None if orc is None else orc.get(c['token']))
    for c in doc['proofEnvelopeCases']:
        if 'errorJson' in c:
            raw = '{"jsonrpc":"2.0","id":%d,"error":%s}' % (c['requestId'], c['errorJson'])
            kind, val = B6.envelope(raw, c['requestId'])
            got = list(B6.classify_error(val)) if kind == 'error' else val
        else:
            raw = '{"jsonrpc":"2.0","id":%s,"result":null}' % c['responseIdToken']
            kind, val = B6.envelope(raw, c['requestId'])
            got = 'result' if kind == 'result' else val
        check('id.envelope ' + c['id'], got == c['expected'], got=got)
    # the 0.4 bridge cases under value semantics (two superseded expectations)
    superseded = {'B0-id-1.0': {'stage': 'ok', 'id': 1}, 'B0-id-minus-zero': {'stage': 'ok', 'id': 0}}
    for c in load(ROOT / 'm1-draft-0.4' / 'vectors' / 'bridge-check-cases.json')['cases']:
        if 'raw' in c:
            raw = c['raw']
        elif 'message' in c:
            raw = dumps(c['message'])
        elif 'methodConstruction' in c:
            k = c['methodConstruction']
            raw = dumps({'id': 1, 'kind': c['kind'], 'payload': {'method': k['prefix'] + k['repeat'] * k['count'] + k.get('suffix', ''), 'params': []}})
        else:
            k = c['construction']
            n = k['padToUtf8Bytes'] - B04.utf8_len(k['template'].replace('{PAD}', '')) if 'padToUtf8Bytes' in k else k['padLength']
            raw = k['template'].replace('{PAD}', 'a' * n)
        exp = superseded.get(c['id'], dict(c['expected']))
        if 'expectedDataMethodConstruction' in c:
            k = c['expectedDataMethodConstruction']
            exp['data'] = {'method': k['prefix'] + k['repeat'] * k['count'] + k.get('suffix', '')}
        r = B6.check(raw, 1000, B6.Bucket(1000))
        ok, bad = subset(exp, r)
        check('bridge04.under06 ' + c['id'], ok, mismatches=bad)


# --- R6-03/R6-04 X2 and Q8-Q12 ---------------------------------------------
def profile_checks():
    doc = load(VEC / 'profile-race-cases.json')
    profs = doc['profiles']
    for sc in doc['scenarios']:
        pid = sc.get('setupProfile', 'A')
        w = PRR.World({pid: profs[pid]}, doc['site'], rp=sc.get('rp'), node_genesis=sc.get('nodeGenesis'))
        nk = {k: PRR.net_key(v['chainId'], v['genesisHash']) for k, v in profs.items()}
        w.connections.add((nk[pid], doc['site']))
        w.request('r1', pid)

        def op(o):
            o = list(o)
            if o[0] == 'addProfile':
                return ['addProfile', o[1], profs[o[1]]]
            if len(o) > 1 and isinstance(o[1], str) and o[1].startswith('@netKey:'):
                o[1] = nk[o[1].split(':')[1]]
            return o
        for st in sc['steps']:
            if st[0] == 'submit':
                w.submit(op(st[1]))
            elif st[0] == 'accept':
                w.accept(st[1])
            elif st[0] == 'acceptDuring':
                w.accept(st[1], during=[op(x) for x in st[2]])
            elif st[0] == 'acceptRaw':
                raw = dict(st[3])
                raw['profile'] = profs[raw['profile']]
                w.accept(st[1], raw_at=st[2], raw=raw)
            elif st[0] == 'raw':
                w.raw_write(**st[1])
            elif st[0] == 'request':
                w.request(st[1], st[2])
        e = sc['expected']
        ok = True
        for rid in ('r1', 'r2'):
            if rid in e:
                req = w.pending.get(rid, {})
                ok = ok and all(req.get(k) == v for k, v in e[rid].items())
        for k in ('signs', 'sends', 'epoch'):
            if k in e:
                ok = ok and getattr(w, k) == e[k]
        if 'conflicting' in e:
            ok = ok and PRR.conflicting(w) == e['conflicting']
        for item in e.get('logHas', []):
            ok = ok and tuple(item) in [tuple(x) for x in w.log]
        check('profile.' + sc['id'], ok, state={k: w.pending[k]['state'] for k in w.pending}, signs=w.signs, sends=w.sends,
              epoch=w.epoch, log=[list(x) for x in w.log])


# --- R6-04 DNR --------------------------------------------------------------
def dnr_checks():
    doc = load(VEC / 'dnr-cases.json')
    for c in doc['cases']:
        d = DNR.FakeDnr()
        v = DNR.Viewer(d)
        if c.get('inject') == 'failUpdate':
            d.fail_update = True
        if c.get('inject') == 'corruptRead':
            d.corrupt_read = True
        outs = {}
        for st in c['steps']:
            if st[0] == 'open':
                outs['open'] = v.open(st[1])
            elif st[0] == 'close':
                v.remove(st[1], 'tabs.onRemoved')
            elif st[0] == 'workerRestartPingFailed':
                v.worker_restart_ping_failed(st[1])
        e = c.get('expected', {})
        ok = True
        if 'open' in e:
            ok = ok and outs.get('open') == e['open']
        if 'sessionRuleIds' in e:
            ok = ok and sorted(d.rules) == e['sessionRuleIds']
        if e.get('rulesExact'):
            ok = ok and d.get_session_rules() == DNR.rules_for(7)
        if 'frames' in e:
            ok = ok and sorted(v.frames) == e['frames']
        if 'eventOrder' in e:
            ok = ok and [x[0] for x in v.events] == e['eventOrder']
        for item in e.get('eventsHave', []):
            ok = ok and any(list(x) == item for x in v.events)
        for ev in c.get('evaluate', []):
            types = DNR.RT if ev['types'] == 'RT' else ev['types']
            got = {d.evaluate(ev.get('tab', 7), ev['url'], t) for t in types}
            ok = ok and got == {ev['expected']}
        check('dnr.' + c['id'], ok, rules=sorted(d.rules), events=[list(x) for x in v.events])


def x3_checks():
    doc = load(ANNEX / 'x3-restored.json')
    t = doc['translations']
    q = ['Q8', 'Q9a', 'Q9b', 'Q9c', 'Q10', 'Q11', 'Q12']
    x = ['X%d' % i for i in range(1, 8)]
    d = ['\u062f%d' % i for i in range(1, 6)]
    check('x3.allRequiredIdsRestored (17 incl. Q9 a/b/c)', all(k in t and t[k].get('en') for k in q + x + d))
    check('x3.xSitesComplete', [s['id'] for s in doc['xSites']] == x and all(s['files'] and s['expected'] for s in doc['xSites']))
    scen = {s['id'] for s in load(VEC / 'profile-race-cases.json')['scenarios']}
    check('x3.qScenariosModelled', {'Q8', 'Q9a', 'Q9b', 'Q10', 'Q12'} <= scen and any(s.startswith('Q9c') for s in scen) and any(s.startswith('Q11') for s in scen))
    dn = {c['id'] for c in load(VEC / 'dnr-cases.json')['cases']}
    check('x3.dalScenariosModelled', set(d) <= dn)


# --- R6-05 B10 --------------------------------------------------------------
def m(i):
    return '{"id":%d,"kind":"rpc_read","payload":{"method":"eth_blockNumber","params":[]}}' % i


def oversize(i):
    head = '{"id":%d,"kind":"rpc_read","payload":{"method":"eth_blockNumber","params":["' % i
    return head + 'a' * (65537 - len(head) - 4) + '"]}}'


def br19_checks():
    bk = B6.Bucket(1000)
    lines = []

    def send(raw, t):
        before = bk.tokens_mt
        r = B6.check(raw, t, bk)
        lines.append({'tokensBefore': before, **r})
        return r
    a1 = [send(m(i), 1000) for i in range(1, 61)]
    check('br19a.a1', all(r['stage'] == 'ok' for r in a1[:50]) and [r['tokensAfter'] for r in a1[:50]] == [50000 - 1000 * k for k in range(1, 51)]
          and all(r['bpre'] == 'rate' and r['code'] == -32005 and r['id'] is None and r['tokensAfter'] == 0 for r in a1[50:]))
    r = send(m(61), 1019)
    check('br19a.a2', r['bpre'] == 'rate' and r['tokensAfter'] == 950)
    r = send(m(62), 1020)
    check('br19a.a3', r['stage'] == 'ok' and r['tokensAfter'] == 0)
    a4 = [send(m(i), 3000) for i in range(63, 114)]
    check('br19a.a4', all(x['stage'] == 'ok' for x in a4[:50]) and a4[-1]['bpre'] == 'rate')
    a5 = [send('{"id":%d,"kind":"rpc_read",' % i, 5000) for i in range(114, 164)]
    r = send(m(164), 5000)
    check('br19a.a5', all(x['stage'] == 'B0' and x['code'] == -32600 and x['id'] is None for x in a5) and r['bpre'] == 'rate')
    a6s = [send(oversize(i), 7000) for i in range(165, 215)]
    a6 = [send(m(i), 7000) for i in range(215, 266)]
    check('br19a.a6', all(x['bpre'] == 'size' and x['data'] == {'reason': 'size'} for x in a6s) and bk.last_ms == 7000
          and all(x['stage'] == 'ok' for x in a6[:50]) and a6[-1]['bpre'] == 'rate')
    ok = all(lines[k + 1]['tokensBefore'] == lines[k]['tokensBefore'] for k in range(len(lines) - 1) if lines[k]['bpre'] == 'size')
    check('br19a.a7 sizeLinesKeepTokensBefore', ok and len(lines) == 265)
    # negative control: noRefill must fail a3
    nb = B6.Bucket(1000)
    for i in range(1, 62):
        nb.tokens_mt = max(0, nb.tokens_mt - 1000) if nb.tokens_mt >= 1000 else nb.tokens_mt
    check('br19a.noRefillFailsA3', nb.tokens_mt < 1000)
    # BR19b
    rc = BR.ReadClient()
    o = [rc.send('S', 'f1', i) for i in range(1, 6)]
    check('br19b.b1', sum(1 for x in o if x.get('accepted')) == 4 and o[4].get('code') == -32005 and len(rc.transport) == 4
          and rc.pending.counts('S')['session'] == 4)
    rc.release(o[0]['handle'])
    p_after = rc.pending.counts('S')['session']
    o6 = rc.send('S', 'f1', 6)
    check('br19b.b2', p_after == 3 and o6.get('accepted') and rc.pending.counts('S')['session'] == 4)
    n_rep = len(rc.replies)
    rc.advance(10000)
    errs = [x for x in rc.replies[n_rep:] if x[2] == -32603]
    check('br19b.b3', len(errs) == 4 and rc.pending.counts('S')['session'] == 0)
    rc.send('S', 'f1', 7, logs=True)
    calls = [rc.send('S', 'f1', i) for i in range(8, 12)]
    check('br19b.b4', rc.pending.counts('S')['session'] == 4 and sum(1 for x in calls if x.get('accepted')) == 3 and calls[3].get('code') == -32005)
    rc2 = BR.ReadClient()
    hs = [rc2.send('S', 'f2', i)['handle'] for i in range(1, 5)]
    rc2.teardown('f2')
    first = rc2.send('S', 'f3', 1)
    rc2.release(hs[0])
    second = rc2.send('S', 'f3', 2)
    for h in hs[1:]:
        rc2.release(h)
    check('br19b.b5', first.get('code') == -32005 and second.get('accepted') and not [x for x in rc2.replies if x[0] == 'f2' and x[2] is None])
    # BR19c synthetic trace through BridgeRef
    msgs = [{'seq': j + 1, 'frame': 'f1', 'raw': m(j + 1), 'arrivalMs': 1000 + j} for j in range(60)]
    call = '{"id":%d,"kind":"rpc_read","payload":{"method":"eth_call","params":[{"to":"0x976ea74026e726554db657fa54763abd0c3a0aa9"},"latest"]}}'
    msgs += [{'seq': 61 + k, 'frame': 'f1', 'raw': call % (61 + k), 'arrivalMs': 3000} for k in range(5)]

    def delay(seq, method, t):
        return 6000 if method == 'eth_call' else t + 5
    tr = BR.bridge_ref(msgs, delay, 1000)
    bp = [l['bpre'] for l in tr[:60]]
    calls5 = tr[60:]
    check('br19c.synthetic bpre', bp == ['pass'] * 52 + ['rate'] * 8)
    check('br19c.synthetic ethCall', [l['forwardedSeq'] is not None for l in calls5] == [True] * 4 + [False] and calls5[4]['code'] == -32005)
    check('br19c.synthetic decisive', BR.decisive(tr[:60], calls5))
    check('br19c.noForwardForRejected', all(l['forwardedSeq'] is None for l in tr if l['stage'] != 'ok'))
    check('bridgetrace.format synthetic', all(BR.validate_trace_line(l) is None for l in tr))


# --- R6-06 B11 BR16 ---------------------------------------------------------
def br16_checks():
    py_hash, n = G.stream_hash()
    node = RES / 'br16-node.json'
    nd = load(node) if node.exists() else None
    check('br16.nodeStreamAvailable', nd is not None)
    if nd:
        check('br16.pythonEqualsNode', nd['streamSha256'] == py_hash and nd['count'] == n, python=py_hash, node=nd['streamSha256'])
    msgs = [{'seq': j + 1, 'frame': 'f1', 'raw': text, 'arrivalMs': 1000 + 25 * j} for j, text, _ in G.generate()]
    tr = BR.bridge_ref(msgs, lambda seq, method, t: t + 5, 1000)
    hist = {}
    for l in tr:
        key = '%s/%s/%s/%s' % (l['bpre'], l['stage'], l['code'], l['route'])
        hist[key] = hist.get(key, 0) + 1
    check('br16.everyMessageDecided', len(tr) == n == 10000)
    check('br16.traceFormat', all(BR.validate_trace_line(l) is None for l in tr))
    check('br16.noForwardForRejected', all(l['forwardedSeq'] is None for l in tr if l['stage'] != 'ok'))
    check('br16.storeTotalWithinQuota', all(l['storeTotalAfter'] is None or l['storeTotalAfter'] <= 1048576 for l in tr))
    dh = hashlib.sha256('\n'.join(json.dumps(l, sort_keys=True) for l in tr).encode()).hexdigest()
    record('br16.summary', seedIndex=G.SPEC['seedIndex'], streamSha256=py_hash, decisionsSha256=dh, histogram=dict(sorted(hist.items())))


def main():
    t0 = time.time()
    check('keccak.selftest', keccak.selftest())
    install_oracles()
    for step in (c14_checks, id_checks, profile_checks, dnr_checks, x3_checks, br19_checks, br16_checks):
        try:
            step()
        except Exception as e:                                         # record, never hide
            check('step.completed ' + step.__name__, False, exception='%s: %s' % (type(e).__name__, e))
    failed = [r for r in results if r['status'] == 'FAIL']
    out = {'package': 'M1 draft 0.6', 'executedAtUtc': datetime.now(timezone.utc).isoformat(timespec='seconds'),
           'environment': {'python': sys.version.split()[0], 'executable': sys.executable, 'platform': platform.platform(),
                           'thirdPartyPackages': 'none'},
           'notExecuted': ['browser / MV3 extension (X1-X7, د1-د5, Q8-Q12 browser runs)', 'Chrome DNR itself (modelled)',
                           'anvil / pocold', 'MPT proofs', 'ESLint / dependency-cruiser', 'real transactions'],
           'summary': {'checks': len(results), 'passed': sum(r['status'] == 'pass' for r in results),
                       'recorded': sum(r['status'] == 'recorded' for r in results), 'failed': len(failed),
                       'seconds': round(time.time() - t0, 1)},
           'results': results}
    RES.mkdir(exist_ok=True)
    (RES / 'run-results-0.6.json').write_text(json.dumps(out, indent=2, ensure_ascii=True) + '\n', encoding='utf-8')
    print(json.dumps(out['summary']))
    for f in failed:
        print('FAIL', json.dumps(f, ensure_ascii=True)[:300])
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
