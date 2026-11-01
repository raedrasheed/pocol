"""Reference-only check runner for M1 draft 0.7 (Python standard library only; no Node step needed).

Sequence, from D:\\PoCol-Development:
  coordination\\runtime\\python311\\python.exe m1-draft-0.7\\tools\\run_checks_07.py
Writes only m1-draft-0.7/results/. Imports 0.2-0.6 tools by path without modifying them.
Every check here is MODEL evidence; browser, socket, node and EVM behaviour is listed
under 'notExecuted'.
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
sys.path[:0] = [str(HERE)] + [str(ROOT / ('m1-draft-0.%d' % v) / 'tools') for v in (6, 5, 4, 3, 2)]

import keccak                                    # noqa: E402
from keccak import keccak256                     # noqa: E402
import rlp_strict as R                           # noqa: E402
import m1model_07 as MM                          # noqa: E402  (patches m1model in place)
import bridge_ref as B04                         # noqa: E402
import bridge_ref_06 as B6                       # noqa: E402
import profile_race_ref_07 as PRR                # noqa: E402
import recvguard_ref as RG                       # noqa: E402
import session_ref as SR                         # noqa: E402
import snapshot_model_04 as S4                   # noqa: E402
import snapshot_e_ref as SE                      # noqa: E402

M = MM.model
results = []


def check(name, ok, **detail):
    results.append({'check': name, 'status': 'pass' if ok else 'FAIL', **detail})
    return ok


def record(name, **detail):
    results.append({'check': name, 'status': 'recorded', **detail})


def load(p):
    return json.loads(Path(p).read_text(encoding='utf-8'))


# --- R7-02 C20 ---------------------------------------------------------------
def one_file(path):
    a = M.chunk_address(b'a')
    m = R.encode([R.uint(1), R.uint(0), [[path, R.uint(1), R.uint(1), keccak256(b'a'), [[a, R.uint(1)]]]]])
    return m, {a: M.runtime(b'a')}


def c20_checks():
    doc = load(VEC / 'c20-cases.json')
    for c in doc['pathCases']:
        path = (b'/' + b'a' * 256 + b'/..') if c.get('lengthOver') else c['path'].encode()
        check('c20.violations %r' % c['path'][:20], MM.path_violations(path) == c['violations'], got=MM.path_violations(path))
        m, codes = one_file(path)
        n = M.validate_manifest(m, codes)
        mu = M.validate_manifest(m, codes, disabled=frozenset([c['disable']]))
        got_mu = mu['result'] if mu['result'] == 'accept' else mu['rule']
        check('c20.normal %r' % c['path'][:20], n['rule'] == c['normal'], got=n.get('rule'))
        check('c20.mutant %r -%s' % (c['path'][:20], c['disable']), got_mu == c['mutant'], got=got_mu)
        if 'v03' in c:
            M._semantic = MM.ORIGINAL_SEMANTIC
            try:
                old = M.validate_manifest(m, codes, disabled=frozenset([c['disable']]))
            finally:
                M._semantic = MM._semantic
            check('c20.v03demonstrated %r' % c['path'][:20], old['result'] == c['v03'], got=old['result'])
    # regression of every 0.2 fixture under the corrected instrumentation
    v02 = ROOT / 'm1-draft-0.2' / 'vectors'
    table = {bytes.fromhex(a[2:]): bytes.fromhex(x[2:]) for a, x in load(v02 / 'code-table.json')['codes'].items()}

    def prov(fx):
        codes = {bytes.fromhex(a[2:]): table[bytes.fromhex(a[2:])] for a in fx.get('provider', [])}
        codes.update({bytes.fromhex(a[2:]): bytes.fromhex(x[2:]) for a, x in fx.get('providerOverrides', {}).items()})
        return codes
    corr = {x['fixture'].split()[-1]: x['now'] for x in doc['fixtureIsolationCorrections']}
    for fx in load(v02 / 'manifest-positive.json')['fixtures']:
        r = M.validate_manifest(bytes.fromhex(fx['manifest'][2:]), prov(fx))
        check('c20.regression.positive ' + fx['id'], r['result'] == 'accept')
    for fx in load(v02 / 'manifest-negative.json')['fixtures']:
        mb, p = bytes.fromhex(fx['manifest'][2:]), prov(fx)
        r = M.validate_manifest(mb, p)
        e = fx['expected']
        ok = r['result'] == 'reject' and r['rule'] == e['firstRule'] and r['stage'] == e['stage']
        if e.get('fetchCalls') is not None:
            ok = ok and r['requests'] == e['fetchCalls']
        check('c20.regression.negative ' + fx['id'], ok, got=r.get('rule'))
        iso = dict(isolation=fx['isolation'], nextRuleWhenDisabled=fx.get('nextRuleWhenDisabled'))
        iso.update(corr.get(fx['id'], {}))
        if iso['isolation'] == 'none':
            continue
        d = M.validate_manifest(mb, p, disabled=frozenset([fx['rule']]))
        want = 'accept' if iso['isolation'] == 'full' else iso['nextRuleWhenDisabled']
        got = d['result'] if iso['isolation'] == 'full' else d.get('rule')
        check('c20.regression.isolation ' + fx['id'], got == want, expected=want, got=got)
    for fx in load(v02 / 'version-records.json')['fixtures']:
        rec = dict(fx['record'])
        rec['manifestHash'] = bytes.fromhex(rec['manifestHash'][2:])
        rec['chunks'] = [(bytes.fromhex(a[2:]), ln) for a, ln in rec['chunks']]
        r = M.validate_version(rec, prov(fx))
        e = fx['expected']
        check('c20.regression.version ' + fx['id'], r['result'] == e['result'] and (e['result'] == 'accept' or r['rule'] == e['firstRule']))
    # cache cases
    honest_addr = M.chunk_address(b'honest')
    for c in doc['cacheCases']:
        code = {'CACHE-1-cross-stage-mutant': b'\x00forged!', 'CACHE-2-normal-sharing-unchanged': b'\x00honest',
                'CACHE-3-content-mutant-not-cached': b'\x01honest'}[c['id']]
        ln = len(code) - 1
        for impl, label in ((MM._fetch, 'v07'), (MM.ORIGINAL_FETCH, 'v03')):
            M.Fetcher.fetch = impl
            fx = M.Fetcher(M.Provider({honest_addr: code}))
            outs = []
            for _, stage, how in c['steps']:
                disabled = frozenset() if how == 'all rules enabled' else frozenset([how.split()[-1]])
                try:
                    outs.append(('ok', fx.fetch(honest_addr, ln, M.FACTORY, disabled, stage)))
                except M.Reject as e:
                    outs.append(('reject', e.rule))
            M.Fetcher.fetch = MM._fetch
            if label == 'v07':
                e = c['expected']
                ok = len(fx.prov.requests) == e['requests']
                if isinstance(e.get('step2'), dict):
                    ok = ok and outs[1] == ('reject', e['step2']['reject'])
                else:
                    ok = ok and outs[1][0] == 'ok'
                check('c20.cache ' + c['id'], ok, outs=[[o[0], o[1] if isinstance(o[1], str) else o[1].hex()] for o in outs])
            elif 'v03' in c:
                check('c20.cache.v03demonstrated ' + c['id'], outs[1] == ('ok', b'forged!'))


# --- R7-03 C21 ---------------------------------------------------------------
def epoch_checks():
    prof_doc = load(ROOT / 'm1-draft-0.6' / 'vectors' / 'profile-race-cases.json')
    edoc = load(VEC / 'epoch-cases.json')
    profs, site = prof_doc['profiles'], prof_doc['site']
    nk = {k: PRR.net_key(v['chainId'], v['genesisHash']) for k, v in profs.items()}

    def world(sc):
        pid = sc.get('setupProfile', 'A')
        w = PRR.World({pid: profs[pid]}, site, rp=sc.get('rp'), node_genesis=sc.get('nodeGenesis'))
        w.connections.add((nk[pid], site))
        w.request('r1', pid)
        return w

    def op(o):
        o = list(o)
        if o[0] == 'addProfile':
            return ['addProfile', o[1], profs[o[1]]]
        if len(o) > 1 and isinstance(o[1], str) and o[1].startswith('@netKey:'):
            o[1] = nk[o[1].split(':')[1]]
        return o

    def run(w, steps):
        for st in steps:
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
                raw = dict(st[1])
                if 'profile' in raw:
                    raw['profile'] = profs[raw['profile']]
                w.raw_write(**raw)
            elif st[0] == 'request':
                w.request(st[1], st[2])

    def verdict(w, e):
        ok = True
        for rid in ('r1', 'r2'):
            if rid in e:
                ok = ok and all(w.pending.get(rid, {}).get(k) == v for k, v in e[rid].items())
        for k in ('signs', 'sends', 'epoch'):
            if k in e:
                ok = ok and getattr(w, k) == e[k]
        if 'conflicting' in e:
            ok = ok and PRR.conflicting(w) == e['conflicting']
        for item in e.get('logHas', []):
            ok = ok and tuple(item) in [tuple(x) for x in w.log]
        return ok
    replaced = edoc['replaced']
    for sc in prof_doc['scenarios']:
        steps, e = sc['steps'], sc['expected']
        if sc['id'] in replaced:
            steps, e = replaced[sc['id']]['now']['steps'], replaced[sc['id']]['now']['expected']
        w = world(sc)
        run(w, steps)
        check('epoch.rerun06 ' + sc['id'], verdict(w, e), state={k: [w.pending[k]['state'], w.pending[k]['code'], w.pending[k]['epoch']] for k in w.pending},
              sends=w.sends, epoch=w.epoch)
    # purity
    w = world({})
    w.epoch = 1
    before = json.dumps(w.pending['r1'], sort_keys=True)
    c1, c2 = w.sign_eligible(w.pending['r1']), w.sign_eligible(w.pending['r1'])
    check('epoch.PURE-1-stale-not-rewritten', c1 == c2 == 4901 and json.dumps(w.pending['r1'], sort_keys=True) == before and w.pending['r1']['epoch'] == 0)
    w = world({})
    before = json.dumps(w.pending['r1'], sort_keys=True)
    c1, c2 = w.sign_eligible(w.pending['r1']), w.sign_eligible(w.pending['r1'])
    check('epoch.PURE-2-eligible-not-rewritten', c1 is None and c2 is None and json.dumps(w.pending['r1'], sort_keys=True) == before)
    c3 = w.sign_eligible(w.pending['r1'], view=w.storage)
    check('epoch.PURE-3-storage-view-not-rewritten', c3 is None and json.dumps(w.pending['r1'], sort_keys=True) == before)
    for sc in edoc['stale']:
        w = world({})
        run(w, sc['steps'])
        check('epoch.' + sc['id'], verdict(w, sc['expected']), state={k: [w.pending[k]['state'], w.pending[k]['code'], w.pending[k]['epoch']] for k in w.pending})


# --- R7-01 BR19 checker fix --------------------------------------------------
def br19_checks():
    bk = B6.Bucket(1000)
    snaps = []

    def send(raw, t):
        snapshot = bk.tokens_mt                     # independent snapshot; never overwritten by r
        r = B6.check(raw, t, bk)
        snaps.append({'snapshotTokensBefore': snapshot, 'bpre': r['bpre']})
        return r

    def m(i):
        return '{"id":%d,"kind":"rpc_read","payload":{"method":"eth_blockNumber","params":[]}}' % i

    def oversize(i):
        head = '{"id":%d,"kind":"rpc_read","payload":{"method":"eth_blockNumber","params":["' % i
        return head + 'a' * (65537 - len(head) - 4) + '"]}}'
    for i in range(1, 61):
        send(m(i), 1000)
    send(m(61), 1019)
    send(m(62), 1020)
    for i in range(63, 114):
        send(m(i), 3000)
    for i in range(114, 164):
        send('{"id":%d,"kind":"rpc_read",' % i, 5000)
    send(m(164), 5000)
    for i in range(165, 215):
        send(oversize(i), 7000)
    for i in range(215, 266):
        send(m(i), 7000)
    ok = all(snaps[k + 1]['snapshotTokensBefore'] == snaps[k]['snapshotTokensBefore']
             for k in range(len(snaps) - 1) if snaps[k]['bpre'] == 'size')
    check('br19a.a7 sizeLinesKeepTokensBefore (checker fixed)', ok and len(snaps) == 265
          and [s['bpre'] for s in snaps[164:214]] == ['size'] * 50)


# --- R7-04 D98 ---------------------------------------------------------------
def head_body(prefix, suffix, total, fill='a', unit=2):
    """Pick id 1 or 10 so the fill length is a multiple of `unit`, then pad to `total` bytes."""
    for rid in (1, 10, 100):
        p = prefix.replace('ID', str(rid))
        n = total - len(p) - len(suffix)
        if n >= 0 and n % unit == 0:
            return p + fill * n + suffix
    raise ValueError('no parity')


def fee_history(total, n, p):
    q = '0x' + 'f' * 64
    base = ['"%s"' % q] * (n + 1)
    ratios = ['0.5'] * n
    reward = ['[' + ','.join(['"%s"' % q] * p) + ']'] * n

    def build():
        return ('{"jsonrpc":"2.0","id":1,"result":{"oldestBlock":"0x1","baseFeePerGas":[' + ','.join(base) + '],"gasUsedRatio":[' +
                ','.join(ratios) + '],"reward":[' + ','.join(reward) + ']}}')
    t = build()
    k = 0
    while len(t) > total:                       # shorten quantities one hex digit at a time
        i = k % (n + 1)
        s = base[i]
        if len(s) > 5:
            base[i] = s[:-2] + '"'
        k += 1
        t = build()
    if len(t) < total:                          # lengthen the first ratio's fraction
        ratios[0] = '0.5' + '1' * (total - len(t))
        t = build()
    return t


def mr_checks():
    doc = load(ANNEX / 'malicioushttp-scripts.json')
    for s in doc['scripts']:
        if 'construct' in s:
            c = s['construct']
            if c['kind'] == 'hexResult':
                body = head_body('{"jsonrpc":"2.0","id":ID,"result":"0x', '"}', c['total'])
            elif c['kind'] == 'blockResult':
                body = head_body('{"jsonrpc":"2.0","id":ID,"result":{"number":"0x1","extraData":"0x', '"}}', c['total'])
            else:
                body = fee_history(c['total'], c['n'], c['p'])
            script = {'body': [{'text': body}]}
            check('mr.%s length' % s['id'], len(body.encode()) == c['total'], got=len(body.encode()))
            if c['kind'] == 'feeHistory':
                fh = json.loads(body)['result']
                check('mr.%s shape' % s['id'], len(fh['baseFeePerGas']) == c['n'] + 1 and len(fh['reward']) == c['n']
                      and all(len(x) == c['p'] for x in fh['reward']))
        else:
            script = {'body': s.get('body', []), 'gzip': s.get('gzip'),
                      'headers': [tuple(h.split(': ', 1)) for h in s.get('headersExtra', [])]}
        if 'atMs' in s['expected']:
            check('mr.%s deadline' % s['id'], s['expected']['atMs'][0] <= 10000 <= s['expected']['atMs'][1])
            continue
        r = RG.read(script, s['recvLimit'])
        e = s['expected']
        ok = r['outcome'] == e['outcome']
        if 'bytesCounted' in e:
            ok = ok and r['bytesCounted'] == e['bytesCounted']
        if 'bytesCountedAtMost' in e:
            ok = ok and r['bytesCounted'] <= e['bytesCountedAtMost']
        if 'step' in e:
            ok = ok and r.get('step') == e['step']
        if 'bodyBytes' in e:
            ok = ok and sum(len(x) for x in RG.segments(s['body'])) == e['bodyBytes']
        if 'messageUnits' in e:
            ok = ok and len(r['error']['message']) == e['messageUnits'] and ('data' in r['error']) == e['dataPresent']
        check('mr.' + s['id'], ok, got={k: v for k, v in r.items() if k != 'result'} if r['outcome'] != 'ok' or 'error' not in r
              else {'outcome': r['outcome'], 'messageLen': len(r['error']['message'])})
    # MR-neg: without a limit MR1 and MR4 read past 4096 bytes
    for sid in ('MR1', 'MR4'):
        s = next(x for x in doc['scripts'] if x['id'] == sid)
        past = RG.counts_past({'body': s.get('body', []), 'gzip': s.get('gzip')}, 4096)
        check('mr.neg recvNoLimit fails ' + sid, past, note='a reader without the limit counts more than 4096 bytes, so MR1/MR4 would not end in recvLimit')
    # MR4 compressed stream evidence (full 1 GiB, lazily)
    h, n = hashlib.sha256(), 0
    for part in RG.gzip_zero_stream(1 << 30):
        h.update(part)
        n += len(part)
    record('mr.MR4 gzip stream', compressedBytes=n, sha256=h.hexdigest(), zlib=RG.zlib.ZLIB_RUNTIME_VERSION)
    check('mr.MR4 about 1 MiB compressed', 900 * 1024 <= n <= 1200 * 1024, compressedBytes=n)
    # MR10 pools
    for c in load(VEC / 'mr10-cases.json')['cases']:
        P = RG.Pools()
        snap = {}
        forwarded = rejected = 0
        for st in c['steps']:
            t = st[0]
            P.advance(t)
            if st[1] == 'reserveMany':
                for k in range(1, st[6] + 1):
                    P.reserve('%s%d' % (st[2], k), st[3], st[4], st[5])
            elif st[1] == 'reserveSessions':
                for s_ in range(1, st[4] + 1):
                    for k in range(st[5]):
                        P.reserve('%s%d_%d' % (st[2], s_, k), st[3], 'S%d' % s_, st[6])
            elif st[1] == 'reserveForwarded':
                for s_ in range(1, st[4] + 1):
                    for k in range(st[5]):
                        if forwarded < st[7]:
                            P.reserve('%s%d_%d' % (st[2], s_, k), st[3], 'S%d' % s_, st[6])
                            forwarded += 1
                        else:
                            rejected += 1
            elif st[1] == 'reserve':
                P.reserve(st[2], st[3], st[4], st[5])
            elif st[1] == 'release':
                P.release(st[2])
            snap[t] = {'site': P.used['site'], 'S': P.session_bytes('S'), 'S1': P.session_bytes('S1'), 'inflight': P.inflight}
        e = c['expected']
        ok = True
        for req, want in e.items():
            if isinstance(want, dict) and 'busyAt' in want:
                ok = ok and P.busy.get(req) == want['busyAt']
            elif isinstance(want, dict) and 'grantedAt' in want:
                ok = ok and req in P.granted and P.granted[req][3] == want['grantedAt']
        if 'S1bytesAt0' in e:
            ok = ok and snap[0]['S1'] == e['S1bytesAt0']
        if 'siteUsed' in e:
            ok = ok and P.used['site'] == e['siteUsed']
        if 'inflightAt0' in e:
            ok = ok and snap[0]['inflight'] == e['inflightAt0']
        if 'sessionAt20' in e:
            ok = ok and snap[20]['S'] == e['sessionAt20'] and snap[2100]['S'] == e['sessionAt2100'] and snap[2200]['S'] == e['sessionAt2200']
        if 'forwarded' in e:
            ok = ok and forwarded == e['forwarded'] and rejected == e['pendingRejected']
        check('mr10.' + c['id'], ok, busy=P.busy, granted={k: v[3] for k, v in P.granted.items() if k in e})


# --- R7-05 D99 ---------------------------------------------------------------
def session_checks():
    doc = load(VEC / 'br20-cases.json')
    call = '[{"to":"0x976ea74026e726554db657fa54763abd0c3a0aa9"},"latest"]'

    def run_table(fault=False, stop_after=None, late_old_frame=False):
        s = SR.Session(1000, frame_budget_fault=fault)
        out = []
        for row in doc['BR20a']['rows']:
            if 'settle' in row:
                s.settle_orphan(row['settle'])
                out.append((row, None, s.counts()))
                continue
            a, b = row['msgs']
            res = None
            for i in range(a, b + 1):
                params = '["/"]' if row['method'] == 'site_navigate' else (call if row['method'] == 'eth_call' else '[]')
                res = s.message(row['t'], row['frame'], SR.msg(i, row['method'], params))
            out.append((row, res, s.counts()))
            if late_old_frame and a == 51:
                out.append(('late', s.message(1002, 'F1', SR.msg(999, 'eth_blockNumber')), s.counts()))
            if stop_after and b >= stop_after:
                break
        return s, out
    s, out = run_table(late_old_frame=True)
    for row, res, cnt in out:
        if row == 'late':
            check('br20a.lateOldFrameDropped', res is None and cnt['tokens_mt'] == 50)
            continue
        ok = True
        if res is not None:
            ok = res == (row['check'], row['route'])
        for k, v in row['after'].items():
            ok = ok and cnt.get(k) == v
        check('br20a.%s %s' % (row['t'], row.get('msgs', row.get('settle'))), ok, got=[res, cnt])
    rate_line = next(l for l in s.lines if l['seq'] == 51)
    check('br20a.m51 rate id null', rate_line['bpre'] == 'rate' and rate_line['id'] is None)
    fs, fout = run_table(fault=True, stop_after=51)
    check('br20a.frameBudget negative control differs at m51', fout[-1][1] == ('pass', 'forwarded'), got=fout[-1][1])
    # BR20b
    s = SR.Session(0)
    got = []
    for i, (t, _) in enumerate(doc['BR20b']['steps']):
        res = s.message(t, s.frame, SR.msg(i + 1, 'site_navigate', '["/"]'))
        got.append(res[1])
    check('br20b', got == doc['BR20b']['expected'], got=got)
    # BR20c model run
    sim = SR.br20c_sim()
    acc_ok = all(sum(1 for x in sim['acceptedTimes'] if x <= t) <= 50 + 50 * t / 1000 for t in sim['acceptedTimes'])
    nav_ok = all(sum(1 for x in sim['navTimes'] if x <= t) <= 3 + (t / 1000) / 2 for t in sim['navTimes'])
    check('br20c.model overlap <= 4', sim['overlapMax'] <= 4, got=sim['overlapMax'])
    check('br20c.model accepted <= 50+50t', acc_ok)
    check('br20c.model navigations <= 3+t/2', nav_ok, navs=len(sim['navTimes']))
    check('br20c.model getCode once per chunk', sim['getCode'] == sim['siteChunks'])
    check('br20c.model session reservation <= 10 MiB', sim['sessionReservationMax'] <= 10 * 1048576)
    record('br20c.model summary', navigations=len(sim['navTimes']), accepted=len(sim['acceptedTimes']), overlapMax=sim['overlapMax'])


# --- R7-06 E -----------------------------------------------------------------
def e_checks():
    states = S4.upgrade_03_states(load(ROOT / 'm1-draft-0.3' / 'vectors' / 'snapshot-mock-scenarios.json')['states'])
    states['U3'] = load(ROOT / 'm1-draft-0.6' / 'vectors' / 'snapshot-invariants.json')['states']['U3']
    record('snapshotE.selector', signature=SE.GETTER_SIGNATURE, selector='0x' + keccak256(SE.GETTER_SIGNATURE.encode())[:4].hex())
    for c in load(VEC / 'snapshot-e-cases.json')['cases']:
        if 'sequence' in c:
            outs = [SE.load_e(states, {'from': st}, c['request']) for st in c['sequence']]
            ok = [o['manifestHash'] for o in outs] == c['expected']['renderedHashes'] and not any(o['mixed'] for o in outs)
        else:
            o = SE.load_e(states, c['response'], c['request'])
            ok = all(o.get(k) == v for k, v in c['expected'].items())
        check('snapshotE.' + c['id'], ok)


def fetch_rule_checks():
    doc = load(ANNEX / 'fetch-rule-criterion.json')
    c1 = doc['criterion']['C1-static']
    must = ['fetch', 'globalThis', 'self', 'window', 'Reflect', 'Function', 'eval', 'navigator', 'importScripts']
    check('fetchRule.C1 names the global object and constructors', all(n in c1 for n in must))
    check('fetchRule.no case labelled as an accepted bypass', not any('NOT flagged' in x['expected'] for x in doc['fixtures']['cases']))
    check('fetchRule.alias case flagged', any("g['fe' + 'tch']" in x['source'] and x['expected'].startswith('violation') for x in doc['fixtures']['cases']))


def main():
    t0 = time.time()
    check('keccak.selftest', keccak.selftest())
    for step in (c20_checks, epoch_checks, br19_checks, mr_checks, session_checks, e_checks, fetch_rule_checks):
        try:
            step()
        except Exception as e:                                      # record, never hide
            check('step.completed ' + step.__name__, False, exception='%s: %s' % (type(e).__name__, e))
    failed = [r for r in results if r['status'] == 'FAIL']
    out = {'package': 'M1 draft 0.7', 'executedAtUtc': datetime.now(timezone.utc).isoformat(timespec='seconds'),
           'environment': {'python': sys.version.split()[0], 'executable': sys.executable, 'platform': platform.platform(),
                           'thirdPartyPackages': 'none'},
           'evidenceKind': 'reference models of the specification text only',
           'notExecuted': ['MaliciousHttp server, real sockets, Chrome stream reader, TestHooks (MR1-MR12 future parts)',
                           'Chrome BR20c/BR20d runs and C measurements', 'browser/MV3 extension, DNR', 'anvil/pocold, eth_call getter, MPT',
                           'ESLint/dependency-cruiser and the C2/C3 runtime tests', 'real transactions'],
           'summary': {'checks': len(results), 'passed': sum(r['status'] == 'pass' for r in results),
                       'recorded': sum(r['status'] == 'recorded' for r in results), 'failed': len(failed),
                       'seconds': round(time.time() - t0, 1)},
           'results': results}
    RES.mkdir(exist_ok=True)
    (RES / 'run-results-0.7.json').write_text(json.dumps(out, indent=2, ensure_ascii=True, default=str) + '\n', encoding='utf-8')
    print(json.dumps(out['summary']))
    for f in failed:
        print('FAIL', json.dumps(f, ensure_ascii=True, default=str)[:300])
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
