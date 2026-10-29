"""Execute every check that M1 draft 0.3 claims, and record machine-readable results.

Usage (any directory; works with the embedded interpreter whose ._pth file
keeps the script directory off sys.path):
    <python> m1-draft-0.3/tools/run_checks_03.py
Writes ../results/run-results-0.3.json and ../results/generated-0.3.json.
Never writes into m1-draft-0.2. Exit status is non-zero if any check fails.

Module resolution: this directory first (rlp_strict, m1model, m1paths,
snapshot_model of draft 0.3), then m1-draft-0.2/tools (keccak, m1abi,
gen_fixtures, which are unchanged). The draft 0.2 decoder is loaded under
a separate name only to record the 0.2 behaviour of each new fixture.
"""

import importlib.util
import json
import platform
import random
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
PKG = HERE.parent
ROOT = PKG.parent
TOOLS02 = ROOT / 'm1-draft-0.2' / 'tools'
VEC02 = ROOT / 'm1-draft-0.2' / 'vectors'
VEC = PKG / 'vectors'
sys.path[:0] = [str(HERE)]
sys.path.append(str(TOOLS02))

import keccak                                   # noqa: E402  (draft 0.2, unchanged)
from keccak import keccak256                    # noqa: E402
import rlp_strict as R                          # noqa: E402  (draft 0.3)
import m1model as M                             # noqa: E402  (draft 0.3)
import m1paths                                  # noqa: E402  (draft 0.3)
import snapshot_model as S                      # noqa: E402
from gen_fixtures import HTML, file_item, manifest   # noqa: E402  (draft 0.2 helpers)

_spec = importlib.util.spec_from_file_location('rlp_strict_02', TOOLS02 / 'rlp_strict.py')
rlp02 = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(rlp02)

results, generated = [], {}


def check(name, ok, **detail):
    results.append({'check': name, 'status': 'pass' if ok else 'FAIL', **detail})
    return ok


def load(path):
    return json.loads(Path(path).read_text(encoding='utf-8'))


def hx(b):
    return '0x' + bytes(b).hex()


def unhex(s):
    return bytes.fromhex(s[2:])


CODE02 = {unhex(a): unhex(c) for a, c in load(VEC02 / 'code-table.json')['codes'].items()}


def provider_for(fx):
    codes = {unhex(a): CODE02[unhex(a)] for a in fx.get('provider', [])}
    for a, c in fx.get('providerOverrides', {}).items():
        codes[unhex(a)] = unhex(c)
    return codes


def subset_equal(expected, got, skip=()):
    bad = {k: (v, got.get(k)) for k, v in expected.items() if k not in skip and got.get(k) != v}
    return not bad, bad


# --- R3-01: RLP adversarial (C03) -----------------------------------------
def build_rlp(c):
    if 'hex' in c:
        return unhex(c['hex'])
    if 'listOf' in c:
        body = b''.join(build_rlp(p) for p in c['listOf'])
        return R._len_prefix(len(body), 0xc0, 0xf7) + body
    b = R.nest(unhex(c['inner']), c['depth'])
    if 'append' in c:
        b += unhex(c['append'])
    if 'dropLast' in c:
        b = b[:-c['dropLast']]
    return b


def draft02_outcome(b):
    if len(b) > M.MANIFEST_MAX:
        return {'rule': 'decode.oversize'}           # 0.2 checked size before decoding too
    try:
        rlp02.decode(b)
        return {'decoded': True}
    except RecursionError:
        return {'exception': 'RecursionError'}
    except rlp02.RlpError as e:
        return {'rule': e.rule}


def rlp_checks():
    doc = load(VEC / 'rlp-adversarial.json')
    for fx in doc['fixtures']:
        b = unhex(fx['hex']) if 'hex' in fx else build_rlp(fx['construction'])
        fid = fx['id']
        if 'length' in fx:
            check('rlp.length ' + fid, len(b) == fx['length'], got=len(b))
        if 'headHex' in fx:
            check('rlp.head ' + fid, b.startswith(unhex(fx['headHex'])), got=hx(b[:8]))
        if 'tailHex' in fx:
            check('rlp.tail ' + fid, b.endswith(unhex(fx['tailHex'])), got=hx(b[-8:]))
        if 'maxDepth' in fx:
            check('rlp.maxDepth ' + fid, R.max_depth(b) == fx['maxDepth'], got=R.max_depth(b))
        exp = fx['expected']
        try:
            r = M.validate_manifest(b, {})
        except RecursionError:
            r = {'result': 'uncaught RecursionError'}
        ok = (r.get('result') == exp['result'] and r.get('rule') == exp['firstRule']
              and r.get('stage') == exp['stage'] and r.get('requests') == exp['requests'])
        if 'detailContains' in exp:
            ok = ok and exp['detailContains'] in r.get('detail', '')
        check('rlp.expected ' + fid, ok, expected=exp, got={k: r.get(k) for k in
                                                           ('result', 'rule', 'stage', 'detail', 'requests')})
        if 'alsoRunWithRecursionLimit' in fx:
            old = sys.getrecursionlimit()
            sys.setrecursionlimit(fx['alsoRunWithRecursionLimit'])
            try:
                r2 = M.validate_manifest(b, {})
            except RecursionError:
                r2 = {'result': 'uncaught RecursionError'}
            finally:
                sys.setrecursionlimit(old)
            check('rlp.lowRecursionLimit ' + fid, r2.get('rule') == exp['firstRule'],
                  limit=fx['alsoRunWithRecursionLimit'], got=r2.get('rule', r2.get('result')))
        e02 = fx.get('expected02')
        if e02:
            got02 = draft02_outcome(b)
            want = {k: v for k, v in e02.items() if k in ('rule', 'exception')}
            check('rlp.draft02Behaviour ' + fid, got02 == want, expected=want, got=got02)
        if len(b) <= 4096:
            generated.setdefault('rlp', {})[fid] = hx(b)
        else:
            generated.setdefault('rlp', {})[fid] = {'length': len(b), 'keccak256': hx(keccak256(b))}


# --- R3-02: selector (C04) ------------------------------------------------
def selector_checks():
    for c in load(VEC / 'selector-cases.json')['cases']:
        if 'construction' in c:
            k = c['construction']
            s = k['prefix'] + k['repeat'] * k['count'] + k.get('suffix', '')
        else:
            s = c['input']
        try:
            got = m1paths.omnibox(s)
        except Exception as e:                  # an exception is a failure, never an outcome
            got = {'exception': type(e).__name__}
        check('selector ' + c['id'], got == c['expected'], expected=c['expected'], got=got)
    pc = load(VEC02 / 'path-cases.json')       # draft 0.2 cases stay in force
    for c in pc['omnibox']:
        got = m1paths.omnibox(c['input'])
        check('regression02.omnibox %r' % c['input'][:40], got == c['expected'], got=got)
    for c in pc['navigate']:
        got = m1paths.navigate(c['input'])
        check('regression02.navigate %r' % c['input'][:40], got == c['expected'], got=got)
    for c in pc['reference']:
        got = m1paths.reference(c['input'], c['base'])
        check('regression02.reference %r @ %s' % (c['input'][:40], c['base']),
              got == c['expected'], got=got)


# --- R3-05: version width (C09) -------------------------------------------
def version_width_checks():
    doc = load(VEC / 'version-width.json')
    html_addr = M.chunk_address(HTML)
    codes = {html_addr: CODE02[html_addr]}
    for fx in doc['fixtures']:
        m = unhex(fx['manifest'])
        check('versionWidth.length ' + fx['id'], len(m) == fx['manifestLength'], got=len(m))
        check('versionWidth.layout ' + fx['id'], m.endswith(unhex(doc['filesListF'])))
        r = M.validate_manifest(m, codes)
        exp = fx['expected']
        check('versionWidth ' + fx['id'], r['result'] == 'reject' and r['rule'] == exp['firstRule']
              and r['stage'] == exp['stage'] and r['requests'] == exp['requests'],
              got={k: r.get(k) for k in ('rule', 'stage', 'requests')})
        d = M.validate_manifest(m, codes, disabled=frozenset([fx['rule']]))
        if fx['isolation'] == 'full':
            check('versionWidth.isolation.full ' + fx['id'], d['result'] == 'accept', got=d.get('rule'))
        else:
            check('versionWidth.isolation.next ' + fx['id'], d.get('rule') == fx['nextRuleWhenDisabled'],
                  got=d.get('rule'))


# --- R3-03: actual requests and the shared cache (C05) ----------------------
def regression02_and_requests():
    exp = load(VEC / 'fetch-requests.json')
    pexp = exp['draft02Positives']['expected']
    for fx in load(VEC02 / 'manifest-positive.json')['fixtures']:
        m = unhex(fx['manifest'])
        r = M.validate_manifest(m, provider_for(fx))
        check('regression02.positive ' + fx['id'], r['result'] == 'accept', got=r.get('rule'))
        e = pexp.get(fx['id'])
        if e is None:
            check('requests.positiveHasExpectation ' + fx['id'], False)
            continue
        want = {k: e[k] for k in ('requests', 'uniqueKeys', 'references') if k in e}
        if e.get('referencesEqualsFileCount'):
            want['references'] = len(r.get('files', []))
        ok, bad = subset_equal(want, r)
        check('requests.positive ' + fx['id'], ok, mismatches=bad)
        rn = M.validate_manifest(m, provider_for(fx), cache_mode='none')
        check('requests.mutantNoCache ' + fx['id'], rn['requests'] == rn['references'] == r['references'],
              got=[rn['requests'], rn['references']])
    for fx in load(VEC02 / 'manifest-negative.json')['fixtures']:
        m = unhex(fx['manifest'])
        prov = provider_for(fx)
        r = M.validate_manifest(m, prov)
        e = fx['expected']
        ok = r['result'] == 'reject' and r['rule'] == e['firstRule'] and r['stage'] == e['stage']
        if e.get('fetchCalls') is not None:
            ok = ok and r['requests'] == e['fetchCalls']
        check('regression02.negative ' + fx['id'], ok, got=[r.get('rule'), r.get('stage'), r['requests']])
        if fx['isolation'] == 'none':
            continue
        d = M.validate_manifest(m, prov, disabled=frozenset([fx['rule']]))
        want = 'accept' if fx['isolation'] == 'full' else fx['nextRuleWhenDisabled']
        got = d['result'] if fx['isolation'] == 'full' else d.get('rule')
        check('regression02.isolation ' + fx['id'], got == want, expected=want, got=got)
    vexp = exp['draft02VersionRecords']['expected']
    for fx in load(VEC02 / 'version-records.json')['fixtures']:
        rec = dict(fx['record'])
        rec['manifestHash'] = unhex(rec['manifestHash'])
        rec['chunks'] = [(unhex(a), ln) for a, ln in rec['chunks']]
        r = M.validate_version(rec, provider_for(fx))
        e = fx['expected']
        ok = r['result'] == e['result'] and (e['result'] == 'accept' or r.get('rule') == e['firstRule'])
        check('regression02.version ' + fx['id'], ok, got=r.get('rule', r['result']))
        if fx['id'] in vexp:
            want = {k: v for k, v in vexp[fx['id']].items() if k in ('result', 'requests', 'uniqueKeys', 'references')}
            ok, bad = subset_equal(want, r)
            check('requests.version ' + fx['id'], ok, mismatches=bad)
            if fx['id'] == 'ver-minimal':
                fetcher = M.Fetcher(M.Provider(provider_for(fx)))
                r1 = M.validate_version(rec, fetcher)
                n1 = r1['requests']
                r2 = M.validate_version(rec, fetcher)
                check('requests.fr-session-reload', r1['result'] == r2['result'] == 'accept'
                      and n1 == 2 and r2['requests'] == n1, got=[n1, r2['requests']])
    constructed_requests()


def constructed_requests():
    fillers = [file_item('/p%03d-' % i + 'x' * 240, 10, b'a') for i in range(100)]
    index = file_item('/index.html', 1, HTML)

    def build(content):
        return manifest([file_item('/a-shared.txt', 10, content), index] + fillers, 1)
    probe = build(b'\x00' * 1000)
    l1 = len(probe) - M.CHUNK_MAX
    m1 = build(b'\x00' * l1)
    chunk1 = m1[M.CHUNK_MAX:]
    m = build(chunk1)
    first_entry_end = len(R.encode(file_item('/a-shared.txt', 10, chunk1))) + 8
    check('fr-shared.construction', len(probe) == len(m1) == len(m) and M.CHUNK_MAX < len(m) <= 2 * M.CHUNK_MAX
          and m[M.CHUNK_MAX:] == chunk1 and first_entry_end < M.CHUNK_MAX and 256 <= l1 <= 65535,
          manifestLength=len(m), chunk1Length=len(chunk1))
    chunks, codes = M.manifest_chunks(m)
    for data in (HTML, b'a'):
        codes[M.chunk_address(data)] = M.runtime(data)
    rec = {'manifestHash': keccak256(m), 'manifestLen': len(m), 'chunks': chunks}
    want = {'session': {'requests': 4, 'uniqueKeys': 4, 'references': 104},
            'separate': {'requests': 5}, 'none': {'requests': 104}}
    for mode, w in want.items():
        r = M.validate_version(rec, M.Provider(codes), cache_mode=mode)
        ok, bad = subset_equal({'result': 'accept', **w}, r)
        check('requests.fr-shared-manifest-content ' + mode, ok, mismatches=bad)
    generated['fr-shared-manifest-content'] = {'manifest': hx(m), 'manifestKeccak256': hx(keccak256(m)),
                                               'chunks': [[hx(a), ln] for a, ln in chunks]}
    a = M.chunk_address(HTML)
    e1 = [b'/a.html', R.uint(1), R.uint(len(HTML)), keccak256(HTML), [[a, R.uint(len(HTML))]]]
    e2 = [b'/b.html', R.uint(1), R.uint(len(HTML) + 1), keccak256(HTML), [[a, R.uint(len(HTML) + 1)]]]
    m2 = manifest([e1, e2], 0)
    r = M.validate_manifest(m2, {a: M.runtime(HTML)})
    ok, bad = subset_equal({'result': 'reject', 'rule': 'fetch.length', 'stage': 'fetch',
                            'requests': 2, 'uniqueKeys': 2, 'references': 2}, r)
    check('requests.fr-same-address-different-length', ok and len(HTML) == 111, mismatches=bad)
    generated['fr-same-address-different-length'] = hx(m2)


# --- R3-04: snapshot mock scenarios (C06) ----------------------------------
def snapshot_checks():
    doc = load(VEC / 'snapshot-mock-scenarios.json')
    for sc in doc['scenarios']:
        fn = S.load_p20 if sc['mechanism'] == 'P20' else S.load_detector_b
        try:
            got = fn(doc['states'], sc['script'], sc['request'])
        except S.ScriptError as e:
            got = {'scriptError': str(e)}
        exp = dict(sc['expected'])
        ok, bad = subset_equal(exp, got, skip=('neverCurrentInAnyState',))
        if exp.get('neverCurrentInAnyState'):
            ok = ok and S.never_current(doc['states'], got.get('version'), got.get('manifestHash'))
        if sc['mechanism'] == 'P20':
            ok = ok and got.get('calls', []).count('anchor') <= 4 and \
                got.get('calls', []).count('proof1') + got.get('calls', []).count('proof2') <= 8
        check('snapshot ' + sc['id'], ok, mismatches=bad)


# --- R3-07: annex batch 1 (partial) ----------------------------------------
def annex_checks():
    doc = load(VEC / 'annex-batch1.json')
    v = doc['X1_netKey']['vector']
    pre = R.encode([b'PoCol-net-v1', R.uint(v['chainId']), unhex(v['genesisHash'])])
    check('annex.X1.preimage', pre == unhex(v['rlpPreimage']) and len(pre) == v['rlpPreimageLength'],
          got=hx(pre))
    results.append({'check': 'annex.X1.netKey', 'status': 'recorded', 'value': hx(keccak256(pre)),
                    'note': 'computed with tools keccak only; not asserted until K1-K3 agree (E05)'})
    for row in doc['V1_RW_unit']['rows']:
        h = int(row['h'])
        b = max(1, h - 12)
        frm = max(1, b - 1)
        got = {'b': str(b), 'from': str(frm), 'count': h - frm + 1, 'n': h - b + 1, 'anchoredGenesis': b == 1}
        want = {k: row[k] for k in got}
        check('annex.V1.' + row['id'], got == want, got=got)
    bad = []
    for h in range(1, 10 ** 6 + 1):
        b = max(1, h - 12)
        frm = max(1, b - 1)
        cnt, n = h - frm + 1, h - b + 1
        if not (cnt <= 14 and n <= 13 and cnt == n + (1 if b > 1 else 0)):
            bad.append(h)
            break
    check('annex.V1.RW.exhaustive 1..10^6', not bad, firstBad=bad)
    top = 2 ** 256

    def ceil_target(tg):
        return min(top - 1, tg * 16)

    def min_work(tg):
        return top // (ceil_target(tg) + 1)
    check('annex.V1.HC1', 2 ** 244 <= ceil_target(2 ** 240))
    check('annex.V1.HC2', 2 ** 244 + 1 > ceil_target(2 ** 240))
    check('annex.V1.HC3', ceil_target(top - 1) == top - 1)
    check('annex.V1.HC4 minWork', min_work(2 ** 240) == 4095, got=min_work(2 ** 240))
    check('annex.V1.HC5 minWork', min_work(top - 1) == 1)
    check('annex.V1.HC6', ceil_target(2 ** 252) == top - 1 and min_work(2 ** 252) == 1)
    check('annex.V1.HC7 ceilTarget', ceil_target(1) == 16)
    results.append({'check': 'annex.V1.HC7', 'status': 'recorded', 'minWorkDecimal': str(min_work(1)),
                    'expression': 'floor(2^256/17)'})
    rng = random.Random(20261007)
    viol = None
    for _ in range(10 ** 4):
        tg = rng.randrange(1, top)
        mw = min_work(tg)
        t = rng.randrange(1, ceil_target(tg) + 1)
        if top // (t + 1) < mw:
            viol = (tg, t)
            break
    check('annex.V1.HC.random 10^4 (seed 20261007)', viol is None, firstViolation=str(viol))


def main():
    t0 = time.time()
    check('keccak.selftest', keccak.selftest())
    for step in (rlp_checks, selector_checks, version_width_checks, regression02_and_requests,
                 snapshot_checks, annex_checks):
        try:
            step()
        except Exception as e:                                 # record, never hide
            check('step.completed ' + step.__name__, False, exception='%s: %s' % (type(e).__name__, e))
    failed = [r for r in results if r['status'] == 'FAIL']
    out = {
        'package': 'M1 draft 0.3',
        'executedAtUtc': datetime.now(timezone.utc).isoformat(timespec='seconds'),
        'environment': {'python': sys.version.split()[0], 'executable': sys.executable,
                        'platform': platform.platform(), 'thirdPartyPackages': 'none'},
        'notExecuted': ['EVM / anvil', 'Solidity compilation', 'gas measurement', 'browser / extension',
                        'pocold', 'eth_getProof / MPT verification (snapshot model is abstract)',
                        'M0 K1-K3 hash libraries'],
        'summary': {'checks': len(results), 'passed': sum(r['status'] == 'pass' for r in results),
                    'recorded': sum(r['status'] == 'recorded' for r in results),
                    'failed': len(failed), 'seconds': round(time.time() - t0, 1)},
        'results': results,
    }
    res = PKG / 'results'
    res.mkdir(exist_ok=True)
    (res / 'run-results-0.3.json').write_text(json.dumps(out, indent=2) + '\n', encoding='utf-8')
    (res / 'generated-0.3.json').write_text(json.dumps(generated, indent=2) + '\n', encoding='utf-8')
    print(json.dumps(out['summary']))
    for f in failed:
        print('FAIL', json.dumps(f)[:300])
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
