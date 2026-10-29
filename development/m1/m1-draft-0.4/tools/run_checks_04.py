"""Reference-only check runner for M1 draft 0.4.

Usage (from any directory; suitable for the embedded interpreter, whose ._pth
file keeps the script directory off sys.path):
    <python> m1-draft-0.4/tools/run_checks_04.py
Writes m1-draft-0.4/results/run-results-0.4.json and generated-0.4.json only.
Exit status is non-zero if any check fails.

Module resolution: this directory (snapshot_model_04, bridge_ref), then
m1-draft-0.3/tools (rlp_strict, m1model, m1paths: unchanged in 0.4), then
m1-draft-0.2/tools (keccak, gen_fixtures). Nothing outside m1-draft-0.4 is
written. Python standard library only.
"""

import json
import platform
import re
import struct
import sys
import time
import zlib
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
PKG = HERE.parent
ROOT = PKG.parent
VEC = PKG / 'vectors'
ANNEX = PKG / 'annex'
VEC03 = ROOT / 'm1-draft-0.3' / 'vectors'
sys.path[:0] = [str(HERE), str(ROOT / 'm1-draft-0.3' / 'tools'), str(ROOT / 'm1-draft-0.2' / 'tools')]

import keccak                                    # noqa: E402  (0.2)
from keccak import keccak256                     # noqa: E402
import rlp_strict as R                           # noqa: E402  (0.3)
import m1model as M                              # noqa: E402  (0.3)
from gen_fixtures import HTML                    # noqa: E402  (0.2)
import snapshot_model_04 as S4                   # noqa: E402
import bridge_ref as B                           # noqa: E402

results, generated = [], {}


def check(name, ok, **detail):
    results.append({'check': name, 'status': 'pass' if ok else 'FAIL', **detail})
    return ok


def record(name, **detail):
    results.append({'check': name, 'status': 'recorded', **detail})


def load(path):
    return json.loads(Path(path).read_text(encoding='utf-8'))


def hx(b):
    return '0x' + bytes(b).hex()


def subset(expected, got):
    bad = {k: [v, got.get(k)] for k, v in expected.items() if got.get(k) != v}
    return not bad, bad


# --- R4-01 snapshot invariants (C10) -----------------------------------------
def snapshot_checks():
    old = load(VEC03 / 'snapshot-mock-scenarios.json')
    states03 = S4.upgrade_03_states(old['states'])
    for sc in old['scenarios']:
        if sc['mechanism'] != 'P20':
            continue                                   # detector scenario: 0.3 model unchanged
        try:
            got = S4.load_p20(states03, sc['script'], sc['request'])
        except S4.ScriptError as e:
            got = {'scriptError': str(e)}
        ok, bad = subset(sc['expected'], got)
        check('snapshot.inherited03 ' + sc['id'], ok, mismatches=bad)
    doc = load(VEC / 'snapshot-invariants.json')
    for sc in doc['scenarios']:
        try:
            got = S4.load_p20(doc['states'], sc['script'], sc['request'])
        except S4.ScriptError as e:
            got = {'scriptError': str(e)}
        ok, bad = subset(sc['expected'], got)
        check('snapshot.invariant ' + sc['id'], ok, mismatches=bad)
    # Property: no state outside the conforming domain ever renders.
    rendered_bad = []
    for status in range(0, 8):
        for req in ({'kind': 'default'}, {'kind': 'explicit', 'version': 1}):
            st = {'X': {'number': 100, 'blockHash': 'H', 'root': 'R',
                        'website': {'versionCount': 1, 'currentVersion': 1,
                                    'versions': {'1': {'status': status, 'manifestHash': 'M', 'manifestLen': 79,
                                                       'publishedBlock': 0 if status == 0 else 1, 'chunkCount': 1}}}}}
            script = [{'call': 'anchor', 'from': 'X'}, {'call': 'proof1', 'from': 'X'}, {'call': 'proof2', 'from': 'X'}]
            r = S4.load_p20(st, script, req)
            if r['frames'] and status != S4.PUBLISHED:
                rendered_bad.append([status, req['kind']])
    check('snapshot.property onlyStatus1Renders (status 0..7, default and explicit)', not rendered_bad,
          violations=rendered_bad)


# --- R4-03 request bound (C12) ---------------------------------------------
def build_from_construction(c):
    items, codes = [], {}
    for f in c['files']:
        if 'chunksOf' in f:
            n = int(re.search(r'range\((\d+)\)', f['chunksOf']).group(1))
            parts = [i.to_bytes(2, 'big') for i in range(n)]
        else:
            data = HTML if f['content'].startswith('draft 0.1 HTML') else f['content'].encode()
            sp = f.get('split')
            if sp is None:
                parts = M.split(data)
            elif sp == '111 x 1':
                parts = [data[i:i + 1] for i in range(len(data))]
            else:
                parts, o = [], 0
                for ln in sp:
                    parts.append(data[o:o + ln])
                    o += ln
        data = b''.join(parts)
        refs = []
        for p in parts:
            a = M.chunk_address(p)
            codes[a] = M.runtime(p)
            refs.append([a, R.uint(len(p))])
        items.append([f['path'].encode(), R.uint(f['mime']), R.uint(len(data)), keccak256(data), refs])
    m = R.encode([R.uint(1), R.uint(c['entryIndex']), items])
    return m, codes


def request_bound_checks():
    doc = load(VEC / 'request-bound.json')
    check('bound.arithmetic', (65536 - 54) // 23 == 2847 and 54 + 23 * 2848 == 65558)
    for fx in doc['fixtures']:
        m, codes = build_from_construction(fx['construction'])
        e = fx['expected']
        if 'manifestLength' in e:
            check('bound.length ' + fx['id'], len(m) == e['manifestLength'], got=len(m))
        if 'manifestHeadHex' in e:
            check('bound.head ' + fx['id'], hx(m[:18]) == e['manifestHeadHex'], got=hx(m[:18]))
        r = M.validate_manifest(m, codes)
        want = {k: e[k] for k in ('result', 'requests', 'uniqueKeys', 'references') if k in e}
        if e['result'] == 'reject':
            want.update(rule=e['firstRule'], stage=e['stage'])
        ok, bad = subset(want, r)
        if 'logicalBytes' in e:
            ok = ok and sum(f['size'] for f in r.get('files', [])) == e['logicalBytes']
        if e.get('requestsEqualsDistinctBytesOfHtml'):
            ok = ok and r['requests'] == len(set(HTML)) and r['requests'] <= e['requestsAtMost']
        check('bound.validate ' + fx['id'], ok, mismatches=bad, got=[r.get('requests'), r.get('rule')])
        if 'validateVersion' in e:
            chunks, mcodes = M.manifest_chunks(m)
            rec = {'manifestHash': keccak256(m), 'manifestLen': len(m), 'chunks': chunks}
            rv = M.validate_version(rec, {**codes, **mcodes})
            vv = e['validateVersion']
            check('bound.validateVersion ' + fx['id'], rv['result'] == vv['result'] and rv['requests'] == vv['requests']
                  and [ln for _, ln in chunks] == vv['manifestChunks'], got=[rv['result'], rv['requests']])
        generated.setdefault('requestBound', {})[fx['id']] = {'length': len(m), 'keccak256': hx(keccak256(m))}


# --- R4-06 bridge (B1, B2, B3, B5) -------------------------------------------
def build_raw(case):
    if 'raw' in case:
        return case['raw']
    if 'message' in case:
        return json.dumps(case['message'], separators=(',', ':'), ensure_ascii=False)
    if 'methodConstruction' in case:
        k = case['methodConstruction']
        method = k['prefix'] + k['repeat'] * k['count'] + k.get('suffix', '')
        return json.dumps({'id': 1, 'kind': case['kind'], 'payload': {'method': method, 'params': []}},
                          separators=(',', ':'), ensure_ascii=False)
    c = case['construction']
    t = c['template']
    if 'padToUtf8Bytes' in c:
        n = c['padToUtf8Bytes'] - B.utf8_len(t.replace('{PAD}', ''))
    else:
        n = c['padLength']
    return t.replace('{PAD}', 'a' * n)


def bridge_checks():
    doc = load(VEC / 'bridge-check-cases.json')
    by_id = {c['id']: c for c in doc['cases']}
    for c in doc['cases']:
        raw = build_raw(c)
        if 'padToUtf8Bytes' in c.get('construction', {}):
            check('bridge.size ' + c['id'], B.utf8_len(raw) == c['construction']['padToUtf8Bytes'])
        r = B.check(raw, 1000, B.Bucket(1000))
        exp = dict(c['expected'])
        if 'expectedDataMethodConstruction' in c:
            k = c['expectedDataMethodConstruction']
            exp['data'] = {'method': k['prefix'] + k['repeat'] * k['count'] + k.get('suffix', '')}
        ok, bad = subset(exp, r)
        check('bridge.case ' + c['id'], ok, mismatches={k: [repr(v[0])[:80], repr(v[1])[:80]] for k, v in bad.items()})
    # every matrix method accepts a generated sample (self-consistency of the schema file)
    sample = {'addr': '0x' + '11' * 20, 'hash32': '0x' + '22' * 32, 'slot32': '0x' + '33' * 32, 'tag': 'latest',
              'falseLiteral': False, 'callObj': {'to': '0x' + '11' * 20}, 'txObj': {'from': '0x' + '11' * 20},
              'filter': {'fromBlock': '0x1', 'toBlock': '0x2'}, 'feeBlocks': '0x1', 'percentiles': [50],
              'pathStr': '/', 'httpsUrl': 'https://example.org/', 'strOrNull': None}
    for kind, methods in B.MATRIX.items():
        for name, spec in methods.items():
            alt = spec['paramsOneOf'][0] if 'paramsOneOf' in spec else spec['params']
            params = [sample[t] if isinstance(t, str) else 'k' for t in alt]
            raw = json.dumps({'id': 1, 'kind': kind, 'payload': {'method': name, 'params': params}})
            r = B.check(raw, 1000, B.Bucket(1000))
            check('bridge.matrixSample %s/%s' % (kind, name), r['stage'] == 'ok', got=[r['stage'], r['data']])
    # B5 deny list: every listed (kind, name) gives 4200 at B2
    dl = load(ANNEX / 'bridge-matrix.json')['denyList']
    pairs = [(k, n) for n in dl['everyKind'] + dl['malformedNames'] for k in B.KINDS]
    for group in dl['kindCrossing'].values():
        pairs += [tuple(p) for p in group]
    bad = []
    for k, n in pairs:
        raw = json.dumps({'id': 1, 'kind': k, 'payload': {'method': n, 'params': []}}, ensure_ascii=False)
        r = B.check(raw, 1000, B.Bucket(1000))
        if not (r['stage'] == 'B2' and r['code'] == 4200 and r['data'] == {'method': B.utf16_prefix(n)}):
            bad.append([k, n, r['stage']])
    check('bridge.denyList %d pairs' % len(pairs), not bad, failures=bad[:10])
    # B2 bucket sequences
    seq = {s['id']: s for s in doc['bucketSequences']}
    ok_raw = build_raw(by_id['ok-eth_chainId'])
    bk = B.Bucket(1000)
    rs = [B.check(ok_raw, 1000, bk) for _ in range(51)]
    want_after = [50000 - 1000 * k for k in range(1, 51)] + [0]
    check('bucket.BK1-burst', [r['bpre'] for r in rs] == ['pass'] * 50 + ['rate']
          and [r['tokensAfter'] for r in rs] == want_after)
    for sid in ('BK2-partial-refill', 'BK3-cap'):
        for a, e in zip(seq[sid]['arrivals'], seq[sid]['expected']):
            r = B.check(build_raw(by_id[a['message']]), a['ms'], bk)
            ok, bad = subset(e, r)
            check('bucket.%s @%d' % (sid, a['ms']), ok, mismatches=bad)
    for sid in ('BK4-size-does-not-touch-bucket', 'BK5-B0-reject-consumes'):
        s = seq[sid]
        bk2 = B.Bucket(s['createdMs'])
        for a, e in zip(s['arrivals'], s['expected']):
            r = B.check(build_raw(by_id[a['message']]), a['ms'], bk2)
            e = dict(e)
            after = e.pop('bucketAfter', None)
            ok, bad = subset(e, r)
            if after:
                ok = ok and after == {'tokens_mt': bk2.tokens_mt, 'last_ms': bk2.last_ms}
            check('bucket.%s @%d' % (sid, a['ms']), ok, mismatches=bad)
    # pendingReads sequences
    pend, last_sess, outcomes = None, None, {}
    for s in doc['pendingSequences']:
        if 'after' not in s:
            pend, outcomes = B.Pending(), {}
        for st in s['steps']:
            if st[0] == 'route':
                outcomes[st[2]] = pend.route(st[1], st[2])
                last_sess = st[1]
            elif st[0] == 'final':
                pend.final_reply(st[1])
            elif st[0] == 'teardown':
                pend.teardown_frame(st[1])
            elif st[0] == 'settle':
                pend.settle_orphan(st[1])
            elif st[0] == 'routeMany':
                for i in range(st[1]):
                    for j in range(st[2]):
                        pend.route('m%d' % i, 'm%d-%d' % (i, j))
        e = dict(s['expected'])
        counts = e.pop('counts')
        ok = all(outcomes.get(k) == v for k, v in e.items()) and pend.counts(last_sess) == counts
        check('pending.' + s['id'], ok, got=pend.counts(last_sess))


# --- R4-06 dependency rules (B7) ---------------------------------------------
def dependency_checks():
    rules = load(ANNEX / 'dependency-rules.json')['dependencyCruiserForbidden']
    doc = load(VEC / 'dependency-graphs.json')

    def violated(edges):
        names = set()
        for frm, to in edges:
            for rule in rules:
                f, t = rule['from'], rule['to']
                if 'path' in f and not re.search(f['path'], frm):
                    continue
                if 'pathNot' in f and re.search(f['pathNot'], frm):
                    continue
                if re.search(t['path'], to):
                    names.add(rule['name'])
        return names
    for g in doc['graphs']:
        got = violated(doc['baseEdges'] + g['add'])
        check('dependency.' + g['id'], got == set(g['expected']), got=sorted(got))


# --- R4-07 receive limits (R1) and the CR-M1-01 proof bound -------------------
def fee_history(n, p):
    return 325 + 2 * (n + 1) * 69 + 2 * n * 32 + n * (69 * p + 3)


def worst(cp):
    return {'eth_getCode': 2 * cp['B_code_max'] + 1024,
            'pocol_getHeaders(count<=14)': 14 * (2 * 3067 + 16) + 1024,
            'pocol_getParams': 2 * cp['genesisPreMax'] + 4096,
            'eth_getBlockByHash': 2048 + (cp['BODY_MAX'] // 85) * 70,
            'eth_feeHistory(n<=128,p<=16)': fee_history(128, 16),
            'eth_getTransactionByHash': 2 * cp['TX_MAX'] + 2048,
            'eth_getTransactionReceipt': 2 * cp['LOG_BLOCK_MAX'] + 546 * 512 + 2048}


CAPPED_CLOSED = {'eth_call', 'eth_estimateGas'}                      # FD:L1255-1259


def table_errors(rows, cp):
    w, errs = worst(cp), []
    for row in rows:
        name = row['methods'][0]
        if row['class'] == 'capped' and name not in CAPPED_CLOSED:
            errs.append('capped without row: ' + name)
        if row['class'].startswith('exact') and name in w and row['limit'] < w[name]:
            errs.append('%s %d < %d' % (name, row['limit'], w[name]))
    return errs


def recv_checks():
    doc = load(ANNEX / 'recv-limits.json')
    cp = doc['cpNetwork']
    w = worst(cp)
    for row in doc['rows']:
        name = row['methods'][0]
        if name in w and 'baselineValue' in row:
            check('recv.worst ' + name, w[name] == row['baselineValue'], got=w[name])
    check('recv.4096 class', 1024 + 70 <= 4096)
    check('recv.table network passes', not table_errors(doc['rows'], cp), errors=table_errors(doc['rows'], cp))
    rows = doc['rows']

    def mutated(name, **chg):
        return [dict(r, **chg) if r['methods'][0] == name else r for r in rows]
    check('recv.D100-1 getCode 65536 fails', bool(table_errors(mutated('eth_getCode', limit=65536), cp)))
    check('recv.D100-2 getBlock 163840 fails', bool(table_errors(mutated('eth_getBlockByHash', limit=163840), cp)))
    check('recv.D100-3 feeHistory n=256 worst', fee_history(256, 16) == 335567 and fee_history(256, 16) > 180224)
    check('recv.D100-4 capped getCode fails', bool(table_errors(mutated('eth_getCode', **{'class': 'capped'}), cp)))
    check('recv.RF3 genesisPre 47105', 2 * 47105 + 4096 > 98304)
    check('recv.RF4 BODY_MAX 262144', 2048 + (262144 // 85) * 70 == 217928 > 167936)
    # CR-M1-01 proof bound under premises PR-1, PR-2
    pb = doc['proofBoundDerivation']
    per_path = 65 * (2 * 564 + 5)
    check('recv.proof perPath', per_path == 73645)
    check('recv.proof k1/k6', 2 * per_path + 4096 == pb['values']['k1'] and 7 * per_path + 4096 == pb['values']['k6']
          and pb['values']['k6'] <= pb['proposedLimit'])
    branch = R.encode([b'\xaa' * 32] * 16 + [b'\xbb' * 32])           # 16 refs + 32-byte value slot
    check('recv.proof branchNodeMax', len(branch) == 3 + 16 * 33 + 33 == 564, got=len(branch))
    for k in (1, 6):
        node = '0x' + 'ab' * 564
        proof = [node] * 65
        res = {'address': '0x' + '11' * 20, 'accountProof': proof, 'balance': '0x' + 'f' * 64,
               'codeHash': '0x' + '22' * 32, 'nonce': '0x' + 'f' * 16, 'storageHash': '0x' + '33' * 32,
               'storageProof': [{'key': '0x' + '44' * 32, 'value': '0x' + 'f' * 64, 'proof': proof} for _ in range(k)]}
        n = len(json.dumps({'jsonrpc': '2.0', 'id': 4294967295, 'result': res}, separators=(',', ':')))
        bound = pb['values']['k%d' % k]
        check('recv.proof constructedWorstJson k=%d' % k, n <= bound, bytes=n, bound=bound)


# --- R4-04 addendum proposal (C13) -------------------------------------------
def png_1x1():
    def chunk(t, d):
        return struct.pack('>I', len(d)) + t + d + struct.pack('>I', zlib.crc32(t + d) & 0xffffffff)
    raw = b'\x00' + bytes([0xff, 0x00, 0x00, 0xff])
    return (b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', 1, 1, 8, 6, 0, 0, 0))
            + chunk(b'IDAT', zlib.compress(raw, 0)) + chunk(b'IEND', b''))


def addendum_checks():
    doc = load(ANNEX / 'addendum-x3-proposal.json')
    ids = [i['id'] for i in doc['items']]
    want = ['Q8', 'Q9', 'Q10', 'Q11', 'Q12'] + ['X%d' % i for i in range(1, 8)] + ['د%d' % i for i in range(1, 6)]
    check('addendum.count 17', len(ids) == 17 and ids == want, got=ids)
    check('addendum.status unsigned', doc['status'].startswith('UNSIGNED PROPOSAL'))
    check('addendum.every item has inputs and pass criterion',
          all(i.get('inputs') and i.get('pass') for i in doc['items']))
    p = png_1x1()
    check('addendum.Q9 png structure', p[:8] == b'\x89PNG\r\n\x1a\n' and p[12:16] == b'IHDR')
    record('addendum.Q9 png bytes', hex=hx(p), sha256=__import__('hashlib').sha256(p).hexdigest())


def main():
    t0 = time.time()
    check('keccak.selftest', keccak.selftest())
    for step in (snapshot_checks, request_bound_checks, bridge_checks, dependency_checks, recv_checks,
                 addendum_checks):
        try:
            step()
        except Exception as e:                                   # record, never hide
            check('step.completed ' + step.__name__, False, exception='%s: %s' % (type(e).__name__, e))
    failed = [r for r in results if r['status'] == 'FAIL']
    out = {
        'package': 'M1 draft 0.4',
        'executedAtUtc': datetime.now(timezone.utc).isoformat(timespec='seconds'),
        'environment': {'python': sys.version.split()[0], 'executable': sys.executable,
                        'platform': platform.platform(), 'thirdPartyPackages': 'none'},
        'notExecuted': ['EVM / anvil', 'Solidity compilation', 'gas', 'browser / extension (bridge model is a Python reading of the text)',
                        'dependency-cruiser and ESLint themselves (rule semantics are emulated)', 'pocold', 'MPT proofs',
                        'M0 K1-K3 triad'],
        'summary': {'checks': len(results), 'passed': sum(r['status'] == 'pass' for r in results),
                    'recorded': sum(r['status'] == 'recorded' for r in results), 'failed': len(failed),
                    'seconds': round(time.time() - t0, 1)},
        'results': results,
    }
    res = PKG / 'results'
    res.mkdir(exist_ok=True)
    (res / 'run-results-0.4.json').write_text(json.dumps(out, indent=2, ensure_ascii=True) + '\n', encoding='utf-8')
    (res / 'generated-0.4.json').write_text(json.dumps(generated, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(out['summary']))
    for f in failed:
        print('FAIL', json.dumps(f, ensure_ascii=True)[:300])
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
