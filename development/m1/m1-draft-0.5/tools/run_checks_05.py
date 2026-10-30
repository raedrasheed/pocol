"""Reference-only check runner for M1 draft 0.5 (standard library only).

Sequence (from D:\\PoCol-Development; Node 22 for the two .cjs steps):
  1. node m1-draft-0.5/tools/url_oracle.cjs            -> results/url-oracle-0.5.json
  2. <python> m1-draft-0.5/tools/run_checks_05.py      -> results/run-results-0.5.json,
                                                          keys-txa-0.5.json, *-expanded.json
  3. node m1-draft-0.5/tools/txa_check.cjs             -> results/txa-node-check.json
  4. <python> m1-draft-0.5/tools/run_checks_05.py      (second pass consumes step 3)
Without step 1 the httpsUrl checks FAIL (oracle missing). Without step 3 the txA
cross-check is recorded as pending, not passed. Writes only m1-draft-0.5/results.
"""

import hashlib
import json
import platform
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
PKG = HERE.parent
ROOT = PKG.parent
VEC, ANNEX, RES = PKG / 'vectors', PKG / 'annex', PKG / 'results'
VEC04 = ROOT / 'm1-draft-0.4' / 'vectors'
sys.path[:0] = [str(HERE), str(ROOT / 'm1-draft-0.4' / 'tools'), str(ROOT / 'm1-draft-0.3' / 'tools'),
                str(ROOT / 'm1-draft-0.2' / 'tools')]

import keccak                                       # noqa: E402  0.2
from keccak import keccak256                        # noqa: E402
import rlp_strict as R                              # noqa: E402  0.3
import m1model as M                                 # noqa: E402  0.3
import m1paths                                      # noqa: E402  0.3
import bridge_ref as B04                            # noqa: E402  0.4
import bridge_ref_05 as B5                          # noqa: E402
import proof_response_ref as PR                     # noqa: E402
import sitestorage_ref as SS                        # noqa: E402
import eth_keys_ref as K                            # noqa: E402

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


# --- R5-01 (C14) ------------------------------------------------------------
def manifest_n(n):
    parts = [i.to_bytes(2, 'big') for i in range(n)]
    data = b''.join(parts)
    refs, codes = [], {}
    for p in parts:
        a = M.chunk_address(p)
        refs.append([a, R.uint(2)])
        codes[a] = M.runtime(p)
    f = [b'/a', R.uint(1), R.uint(len(data)), keccak256(data), refs]
    return R.encode([R.uint(1), R.uint(0), [f]]), f, codes


def c14_checks():
    doc = load(VEC / 'request-bound-c14.json')
    for n, key in ((2847, 'rb-2847'), (2848, 'rb-2848')):
        m, f, codes = manifest_n(n)
        chunks = R.encode(f[4])
        fil = R.encode(f)
        files = R.encode([f])
        levels = [('chunks list', chunks), ('file', fil), ('files list', files), ('top level', m)]
        want = doc['levels'][key]
        rows = []
        for (name, enc), w in zip(levels, want):
            hl = 1 + (enc[0] - 0xf7 if enc[0] > 0xf7 else 0)
            rows.append({'level': name, 'body': len(enc) - hl, 'header': enc[:hl].hex(), 'total': len(enc)})
        check('c14.levels ' + key, rows == want, got=rows)
        if n == 2848:
            fx = doc['fixture']['expected']
            r = M.validate_manifest(m, codes)
            check('c14.fixture rb-2848', len(m) == fx['manifestLength'] and '0x' + m[:24].hex() == fx['manifestHeadHex']
                  and r['rule'] == fx['firstRule'] and r['requests'] == 0, got=[len(m), r.get('rule')])
            check('c14.lowerBound', len(m) >= 54 + 23 * n and len(m) - (54 + 23 * n) == 3)


# --- R5-02 (C15) ------------------------------------------------------------
def build_raw04(case):
    if 'raw' in case:
        return case['raw']
    if 'message' in case:
        return dumps(case['message'])
    if 'methodConstruction' in case:
        k = case['methodConstruction']
        return dumps({'id': 1, 'kind': case['kind'],
                      'payload': {'method': k['prefix'] + k['repeat'] * k['count'] + k.get('suffix', ''), 'params': []}})
    c = case['construction']
    n = c['padToUtf8Bytes'] - B04.utf8_len(c['template'].replace('{PAD}', '')) if 'padToUtf8Bytes' in c else c['padLength']
    return c['template'].replace('{PAD}', 'a' * n)


def url_checks():
    loaded = B5.load_oracle()
    if not check('url.oracleAvailable (run tools/url_oracle.cjs first)', loaded is not None):
        B5.install_oracle({})
        return
    table, node = loaded
    record('url.oracleNode', node=node)
    B5.install_oracle(table)
    doc = load(VEC / 'url-cases.json')
    items = [(c['input'], c['expected'], c['source']) for c in doc['cases']]
    for c in doc['constructed']:
        k = c['construction']
        items.append((k['prefix'] + k['repeat'] * (k['totalLength'] - len(k['prefix'])), c['expected'], c['source']))
    for s, exp, src in items:
        got = table.get(s)
        check('url.%s %r' % (src, s[:60]), got == exp, oracle=got, expected=exp)
    # unknown values are never accepted by the old approximation
    raw = dumps({'id': 1, 'kind': 'nav', 'payload': {'method': 'site_openExternal', 'params': ['https://not-in-oracle.example/']}})
    try:
        B5.check(raw, 1000, B5.Bucket(1000))
        check('url.unknownRaises', False)
    except B5.OracleMissing:
        check('url.unknownRaises', True)
    # the 0.4 bridge cases re-run with the oracle
    for c in load(VEC04 / 'bridge-check-cases.json')['cases']:
        r = B5.check(build_raw04(c), 1000, B5.Bucket(1000))
        exp = dict(c['expected'])
        if 'expectedDataMethodConstruction' in c:
            k = c['expectedDataMethodConstruction']
            exp['data'] = {'method': k['prefix'] + k['repeat'] * k['count'] + k.get('suffix', '')}
        ok, bad = subset(exp, r)
        check('bridge04.withOracle ' + c['id'], ok, mismatches=bad)


# --- R5-03 (C16) ------------------------------------------------------------
def pending_checks():
    seqs = load(VEC / 'pending-handles.json')['sequences']
    for s in seqs:
        p, labels, outcomes, replies, order = B5.Pending(), {}, {}, {}, []
        for st in s['steps']:
            if st[0] == 'route':
                o = p.route(st[1], st[2], st[3])
                outcomes[st[4]] = o
                if o.get('accepted'):
                    labels[st[4]] = o['handle']
            elif st[0] == 'final':
                rep = p.final_reply(labels[st[1]])
                replies[st[1]] = rep
                order.append(rep)
            elif st[0] == 'teardown':
                p.teardown_frame(st[1])
            elif st[0] == 'settle':
                p.settle_orphan(labels[st[1]])
        e = s['expected']
        ok = True
        for k, v in e.get('replies', {}).items():
            ok = ok and replies.get(k) == v
        for k in e.get('noReply', []):
            ok = ok and k not in replies
        if 'handlesDistinct' in e:
            hs = [labels[x] for x in e['handlesDistinct']]
            ok = ok and len(set(hs)) == len(hs)
        if 'replyOrder' in e:
            ok = ok and order == e['replyOrder']
        for k, v in e.get('outcomes', {}).items():
            ok = ok and outcomes.get(k) == v
        for sess, v in e.get('counts', {}).items():
            ok = ok and p.counts(sess) == v
        check('pending.' + s['id'], ok, replies=replies)


# --- R5-04 (C17, C18) -------------------------------------------------------
def proof_checks():
    doc = load(VEC / 'proof-response-cases.json')
    k = doc['pathKats']
    for slot in (0, 1, 2):
        check('proof.storagePathKat slot%d' % slot, PR.storage_path(slot) == k['storageSlot%d' % slot])
    check('proof.emptyTrieRoot', '0x' + keccak256(b'\x80').hex() == PR.EMPTY_TRIE_ROOT)
    check('proof.keccakEmpty', '0x' + keccak256(b'').hex() == PR.KECCAK_EMPTY)
    W = doc['W']
    check('proof.accountPathIs20Bytes', PR.account_path(W) != '0x' + keccak256(bytes(12) + bytes.fromhex(W[2:])).hex())
    rid = doc['requestId']
    env = {'jsonrpc': '2.0', 'id': rid, 'result': doc['baseResult']}
    for c in doc['envelopeCases']:
        if c.get('raw') == '@base':
            raw = dumps(env)
        elif c.get('raw') == '[@base]':
            raw = '[' + dumps(env) + ']'
        elif 'rawText' in c:
            raw = c['rawText']
        elif 'nestedResultArrays' in c:
            n = c['nestedResultArrays']
            raw = '{"jsonrpc":"2.0","id":%d,"result":%s%s}' % (rid, '[' * n, ']' * n)
        else:
            e2 = dict(env)
            for f, v in c['envelopePatch'].items():
                if v is None:
                    e2.pop(f, None)
                else:
                    e2[f] = v
            raw = dumps(e2)
        kind, val = PR.envelope(raw, rid)
        got = [kind] if kind == 'result' else [kind, val]
        check('proof.envelope ' + c['id'], got == c['expected'], got=got)
    for c in doc['errorCases']:
        raw = dumps({'jsonrpc': '2.0', 'id': rid, 'error': c['error']})
        kind, e = PR.envelope(raw, rid)
        got = list(PR.classify_error(e)) if kind == 'error' else [kind, e]
        check('proof.error ' + c['id'], got == c['expected'], got=got)
    for c in doc['resultCases']:
        res = json.loads(json.dumps(doc['baseResult']))
        ver = json.loads(json.dumps(doc['baseVerifier']))
        for f, v in c.get('patch', {}).items():
            if v is None:
                res.pop(f, None)
            elif v == ['@node565']:
                res[f] = ['0x' + 'aa' * 565]
            elif v == ['@nodes66']:
                res[f] = ['0xc180'] * 66
            else:
                res[f] = v
        if 'storageProof' in c:
            res['storageProof'] = c['storageProof']
        if res['storageProof']:
            sp = res['storageProof'][0]
            if 'storageKey' in c:
                sp['key'] = c['storageKey']
            if 'storageValue' in c:
                sp['value'] = c['storageValue']
            if 'storageProofNodes' in c:
                sp['proof'] = c['storageProofNodes']
        if 'verifierAccount' in c:
            ver['account'] = c['verifierAccount']
        ver['account'].setdefault('leaf', {})
        ver['account']['leaf'].update(c.get('leafPatch', {}))
        if 'verifierStorage' in c:
            ver['storage'] = c['verifierStorage']
        # the envelope is parsed first, so result values are IntTok/str exactly as on the wire
        kind, parsed = PR.envelope(dumps({'jsonrpc': '2.0', 'id': rid, 'result': res}), rid)
        got = list(PR.decide(parsed, W, doc['keys'], c.get('stateRoot', doc['stateRoot']), ver))
        check('proof.result ' + c['id'], got == c['expected'], got=got)
    for c in doc['budget']['cases']:
        script = c['script']
        if script == '@worst':
            script = []
            for _ in range(4):
                script += [{'retry': 0}] * 3 + [{'ok': True}] + [{'retry': 0}] * 3 + [{'ok': True}] + [{'retry': 0}] * 4
        got = PR.simulate(script)
        ok, bad = subset(c['expected'], got)
        check('proof.budget ' + c['id'], ok, mismatches=bad)
    check('proof.budget worstCaseSends', PR.worst_case_sends() == 48)


# --- R5-09 (U34) ------------------------------------------------------------
def guard_checks():
    doc = load(VEC / 'bridge-guard-cases.json')
    lb = doc['bg1']['lowerBound']
    check('guard.bg1 lowerBound', lb['value'] + lb['key'] + lb['envelopeReserve'] == lb['sum'] == 61824)
    for v in doc['bg1']['vectors']:
        check('guard.bg1 %d' % v['BRIDGE_MSG_MAX'], (v['BRIDGE_MSG_MAX'] >= lb['sum']) == (v['expected'] == 'pass'))
    for c in doc['cases']:
        k = c['construction']
        raw = dumps({'id': k['id'], 'kind': 'storage_set',
                     'payload': {'method': 'site_storageSet', 'params': ['k' * k['keyLen'], k['valueChar'] * k['valueLen']]}})
        r = B5.check(raw, 1000, B5.Bucket(1000))
        exp = dict(c['expected'])
        n = exp.pop('utf8Length', None)
        ok, bad = subset(exp, r)
        if n is not None:
            ok = ok and B04.utf8_len(raw) == n
        check('guard.' + c['id'], ok, mismatches=bad, length=B04.utf8_len(raw))


# --- R5-05 (B4) -------------------------------------------------------------
def sitestorage_checks():
    doc = load(VEC / 'sitestorage-nav-cases.json')
    for case in doc['storage']:
        s = SS.SiteStorage()
        disk = [None]
        s.disk = lambda op: disk[0]
        ok, got_all = True, []
        for op, exp in zip(case['ops'], case['expected']):
            if op[0] == 'set':
                rep = s.set(op[1], op[2])
            elif op[0] == 'clear':
                rep = s.clear()
            elif op[0] == 'bulk':
                for i in range(op[2]):
                    s.set('%s%04d' % (op[1], i), op[3] * op[4])
                rep = None
            elif op[0] == 'disk':
                disk[0] = op[1]
                rep = None
            else:
                s.fail_next_write = True
                rep = None
            got = {'reply': rep, 'total': s.total, 'entries': len(s.d), 'writes': s.writes}
            got_all.append(got)
            for k2, v in exp.items():
                ok = ok and got[k2] == v
        check('sitestorage.' + case['id'], ok, got=got_all[-1])
    sk = doc['storageKey']
    check('sitestorage.key', SS.storage_key(sk['netKey'], sk['address']) == sk['expected'])
    nav = doc['navigation']

    def lookup(p):
        return m1paths.lookup_path(m1paths.split_query_fragment(p))
    for case in nav['cases']:
        b = SS.NavBucket(0)
        pend = B5.Pending()
        for st in case.get('pendingBefore', []):
            pend.route(*st)
        ok, outs = True, []
        for st, exp in zip(case['steps'], case['expected']):
            o = SS.navigate(b, 'f1', st[1], st[2], set(nav['manifestPaths']), pend, lookup)
            outs.append(o)
            ok = ok and all(o.get(k2) == v for k2, v in exp.items())
        if 'expectedPending' in case:
            ep = case['expectedPending']
            ok = ok and pend.counts('A') == ep['A'] and sum(r['state'] == 'orphan' for r in pend.req.values()) == ep['orphans']
        check('navigator.' + case['id'], ok, got=outs[-1])
    for case in doc['openExternal']:
        oe = SS.OpenExternal()
        ok = True
        for st, exp in zip(case['steps'], case['expected']):
            o = oe.request(st[1], st[2]) if st[0] == 'request' else oe.user(st[1], st[2], st[3])
            ok = ok and all(o.get(k2) == v for k2, v in exp.items())
        ok = ok and oe.tabs == case['expectedTabs']
        check('openExternal.' + case['id'], ok)


# --- R5-06 (B9) BR17e -------------------------------------------------------
def br17e_checks():
    doc = load(VEC / 'br17e-table.json')
    s, bucket, expanded = SS.SiteStorage(), B5.Bucket(1000), []
    snapshots_ok = True
    for row in doc['rows']:
        v = row['value']
        val = None if v is None else v['char'] * v['len']
        text = dumps({'id': row['msg'], 'kind': 'storage_set', 'payload': {'method': 'site_storageSet', 'params': [row['key'], val]}})
        n = B04.utf8_len(text)
        envelope = n - (len(val) if val else 0) - len(row['key'])
        chk = B5.check(text, 1000 + 100 * row['msg'], bucket)
        before = (dict(s.d), s.total, s.writes)
        new_total = s.total - (SS.entry(row['key'], s.d[row['key']]) if row['key'] in s.d else 0) + \
            (SS.entry(row['key'], val) if val is not None else 0)
        rep = s.set(row['key'], val)
        want_reply = {'result': None} if row['reply'] is None else row['reply']
        if 'code' in rep:
            snapshots_ok = snapshots_ok and (dict(s.d), s.total, s.writes) == before
        ok = (chk['stage'] == 'ok' and before[1] == row['totalBefore'] and new_total == row['newTotal']
              and rep == want_reply and s.total == row['totalAfter'] and n <= 61571 and envelope <= 128)
        check('br17e.msg%02d (%s)' % (row['msg'], row['group']), ok,
              got={'newTotal': new_total, 'reply': rep, 'total': s.total, 'bytes': n, 'envelope': envelope})
        expanded.append({'msg': row['msg'], 'utf8Length': n, 'sha256': hashlib.sha256(text.encode()).hexdigest(), 'text': text})
    check('br17e.rejectsLeaveSnapshotAndWritesUnchanged', snapshots_ok)
    e9 = doc['e9']
    want = {'k00': 'b', 'k17': 'a' * 4042}
    want.update({'k%02d' % i: 'a' * 61440 for i in range(2, 17)})
    check('br17e.e9', s.d == want and s.total == e9['total'] == sum(SS.entry(k2, v2) for k2, v2 in s.d.items()))
    # mutants must fail somewhere
    def run_mutant(kind):
        st = SS.SiteStorage()
        outs = []
        for row in doc['rows']:
            v = row['value']
            val = None if v is None else v['char'] * v['len']
            old = st.d.get(row['key'])
            if val is None:
                outs.append(st.set(row['key'], None))
                continue
            nt = st.total - (SS.entry(row['key'], old) if old is not None and kind != 'quotaNoSubtract' else 0) + SS.entry(row['key'], val)
            reject = nt >= SS.SITE_STORE_MAX if kind == 'strictLess' else nt > SS.SITE_STORE_MAX
            if reject and kind != 'nonAtomic':
                outs.append({'code': 4300})
                continue
            st.d[row['key']] = val
            st.total = sum(SS.entry(a, b) for a, b in st.d.items())
            outs.append({'code': 4300} if reject else {'result': None})
        return [o.get('code') for o in outs], st.total
    good = [r['reply']['code'] if r['reply'] else None for r in doc['rows']]
    for kind in ('strictLess', 'quotaNoSubtract', 'nonAtomic'):
        codes, total = run_mutant(kind)
        check('br17e.mutantDetected ' + kind, codes != good or total != e9['total'])
    RES.mkdir(exist_ok=True)
    (RES / 'br17e-expanded.json').write_text(json.dumps({'messages': expanded}, ensure_ascii=True) + '\n', encoding='utf-8')


# --- R5-07 (B6) -------------------------------------------------------------
def write_path_checks():
    doc = load(ANNEX / 'write-path.json')
    order = ['Wallet', 'Eligibility1', 'Approval', 'Identity', 'Eligibility2', 'Sign', 'WalletSubmit']
    send = 'Node.eth_sendRawTransaction'

    def props(edges, sources):
        adj = {}
        for a, b in edges:
            adj.setdefault(a, []).append(b)
        paths, stack = [], [['Frame']]
        while stack:
            p = stack.pop()
            for nxt in adj.get(p[-1], []):
                if nxt in p:
                    continue
                if nxt == send:
                    paths.append(p + [nxt])
                elif nxt == 'HttpTransport':
                    if p[-1] in sources:
                        paths.append(p + [nxt, send])
                    stack.append(p + [nxt])
                else:
                    stack.append(p + [nxt])

        def in_order(p):
            idx = [p.index(n) if n in p else -1 for n in order]
            return -1 not in idx and idx == sorted(idx)
        v = set()
        if not paths or not all(in_order(p) for p in paths):
            v.add('W1')
        if sources != ['WalletSubmit']:
            v.add('W2')
        if any(a == 'BridgeAuth' and b in ('WalletSubmit', 'HttpTransport') for a, b in edges):
            v.add('W3')
        if any(b.startswith('Node.') and a != 'HttpTransport' for a, b in edges):
            v.add('W4')
        return v
    base_sources = doc['methodEdges']['HttpTransport -> Node.eth_sendRawTransaction']
    check('writePath.base', props(doc['edges'], base_sources) == set())
    for m in doc['mutants']:
        srcs = list(base_sources) + m.get('addMethodSources', {}).get('HttpTransport -> Node.eth_sendRawTransaction', [])
        got = props(doc['edges'] + m.get('addEdges', []), srcs)
        check('writePath.mutant ' + m['id'], set(m['mustViolate']) <= got, got=sorted(got))


# --- R5-08 (B8) -------------------------------------------------------------
def keys_and_br_checks():
    doc = load(ANNEX / 'br-messages.json')
    t = doc['txA']
    check('keys.privKey1Address', K.address(1) == t['privKeyOneAddress'])
    seed = K.bip39_seed()
    keys = []
    for i in range(1, 11):
        d = K.fixture_key(i, seed)
        keys.append({'index': i, 'priv': '0x%064x' % d, 'address': K.address(d)})
    for kat in t['kats']:
        k = keys[kat['index'] - 1]
        check('keys.anvilKat %d' % kat['index'], k['address'] == kat['address'], got=k)
    sk = int(keys[t['fromKey'] - 1]['priv'], 16)
    to = keys[t['toKey'] - 1]['address']
    tx = K.type2_tx(sk, t['chainId'], t['nonce'], t['maxPriorityFeePerGas'], t['maxFeePerGas'], t['gas'], to, t['value'])
    check('txA.recoverSigner', K.recover(bytes.fromhex(tx['signingHash'][2:]), int(tx['r'], 16), int(tx['s'], 16),
                                         tx['yParity']) == keys[t['fromKey'] - 1]['address'])
    check('txA.lowS', int(tx['s'], 16) <= K.N // 2)
    txa = dict(t, to=to, **tx)
    RES.mkdir(exist_ok=True)
    (RES / 'keys-txa-0.5.json').write_text(json.dumps({'policy': 'TK-1 (P)', 'mnemonic': K.MNEMONIC, 'keys': keys, 'txA': txa},
                                                      indent=2) + '\n', encoding='utf-8')
    record('txA', raw=tx['raw'], txHash=tx['txHash'], fromAddress=keys[t['fromKey'] - 1]['address'], to=to)
    node = RES / 'txa-node-check.json'
    if node.exists():
        nc = load(node)
        if nc.get('status') == 'notRun':
            record('txA.nodeCrossCheck', pending=nc.get('reason'))
        else:
            same = nc.get('keys', {}).get(str(t['fromKey']), {}).get('priv') == keys[t['fromKey'] - 1]['priv']
            check('txA.nodeCrossCheck', nc.get('status') == 'pass' and same, failures=nc.get('failures'))
    else:
        record('txA.nodeCrossCheck', pending='run tools/txa_check.cjs, then this runner again')
    s1, s2 = keys[6]['address'], keys[7]['address']
    subst = {'@txA': tx['raw'], '@s1': s1, '@s2': s2, '@name65': 'eth_' + 'x' * 61,
             '@granted': '0x000000000000000000000000000000000000beef', '@17addresses': [s1] * 17}
    b04 = {c['id']: c for c in load(VEC04 / 'bridge-check-cases.json')['cases']}
    rawsubst = {'@depth9': b04['BR13-depth-9']['raw'], '@size65537': build_raw04(b04['BR13-size-65537'])}

    def sub(x):
        if isinstance(x, str):
            return subst.get(x, x)
        if isinstance(x, list):
            return [sub(y) for y in x]
        if isinstance(x, dict):
            return {sub(k2): sub(v) for k2, v in x.items()}
        return x
    expanded = []
    for case in doc['cases']:
        texts = []
        if 'messages' in case:
            texts = [dumps(sub(m)) for m in case['messages']]
        elif 'raw' in case:
            texts = [rawsubst.get(r, r) for r in case['raw']]
        elif 'names' in case:
            i = 0
            for kind in case['kinds']:
                for name in case['names']:
                    i += 1
                    texts.append(dumps({'id': i, 'kind': kind, 'payload': {'method': name, 'params': []}}))
        for q in case.get('schemaOnly', []):
            texts.append(dumps(sub({k2: v for k2, v in q.items() if k2 != 'reply'})))
        replies = case.get('reply') if isinstance(case.get('reply'), list) else None
        replies = (replies or []) + [q['reply'] for q in case.get('schemaOnly', [])]
        bucket = B5.Bucket(1000)
        for j, text in enumerate(texts):
            r = B5.check(text, 1000 + j, bucket)
            expanded.append({'case': case['id'], 'n': j + 1, 'utf8Length': B04.utf8_len(text),
                             'sha256': hashlib.sha256(text.encode('utf-8', 'surrogatepass')).hexdigest(),
                             'text': text if B04.utf8_len(text) <= 4096 else None, 'check': {k2: r[k2] for k2 in ('bpre', 'stage', 'code', 'data', 'id')}})
            if case['id'] == 'BR11':
                check('br.BR11 #%d' % (j + 1), r['code'] == 4200)
                continue
            if j >= len(replies) or not isinstance(replies[j], dict):
                continue
            e = replies[j]
            if e.get('code') in (-32600, 4200, -32602):
                want = {'code': e['code'], 'id': e['id']}
                if 'data' in e:
                    want['data'] = e['data']
                if 'stage' in e:
                    want['stage'] = e['stage']
                ok, bad = subset(want, r)
            else:
                ok, bad = r['stage'] == 'ok', {'stage': r['stage']}
            check('br.%s #%d' % (case['id'], j + 1), ok, mismatches=bad)
    (RES / 'br-messages-expanded.json').write_text(json.dumps({'txA': txa, 'messages': expanded}, ensure_ascii=True) + '\n',
                                                   encoding='utf-8')


def main():
    t0 = time.time()
    check('keccak.selftest', keccak.selftest())
    for step in (c14_checks, url_checks, pending_checks, proof_checks, guard_checks, sitestorage_checks,
                 br17e_checks, write_path_checks, keys_and_br_checks):
        try:
            step()
        except Exception as e:                                      # record, never hide
            check('step.completed ' + step.__name__, False, exception='%s: %s' % (type(e).__name__, e))
    failed = [r for r in results if r['status'] == 'FAIL']
    out = {'package': 'M1 draft 0.5', 'executedAtUtc': datetime.now(timezone.utc).isoformat(timespec='seconds'),
           'environment': {'python': sys.version.split()[0], 'executable': sys.executable, 'platform': platform.platform(),
                           'thirdPartyPackages': 'none'},
           'notExecuted': ['EVM / anvil', 'browser / extension', 'ESLint and dependency-cruiser', 'pocold',
                           'Merkle-Patricia proof verification (decision tables assume verifier outcomes)', 'real transactions'],
           'summary': {'checks': len(results), 'passed': sum(r['status'] == 'pass' for r in results),
                       'recorded': sum(r['status'] == 'recorded' for r in results), 'failed': len(failed),
                       'seconds': round(time.time() - t0, 1)},
           'results': results}
    RES.mkdir(exist_ok=True)
    (RES / 'run-results-0.5.json').write_text(json.dumps(out, indent=2, ensure_ascii=True) + '\n', encoding='utf-8')
    print(json.dumps(out['summary']))
    for f in failed:
        print('FAIL', json.dumps(f, ensure_ascii=True)[:300])
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
