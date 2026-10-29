"""Reference-only acceptance runner for M1 draft 0.27 (author turn 025): C32 defensive JSON-RPC error envelope,
V1-SHA-COST accounting (all SHA-256 evaluations, cached/uncached/unique, share-bearing worst cases), carried-forward
0.26 evidence and consolidated criteria. Python standard library only; no network, browser, EVM or chain.
NOT executed by the author. Results are written only by root, in a preserved review copy.

Usage (any of):
  coordination\\runtime\\python311\\python.exe m1-draft-0.27\\tools\\run_checks_027.py
  ...\\run_checks_027.py --root D:\\PoCol-Development --out <dir>
Environment alternatives: POCOL_M1_ROOT, POCOL_M1_OUT.
  root: the tree holding reference/, coordination/ and m1-draft-0.2 ... 0.26 (default: nearest ancestor of this file holding
        both reference/ and m1-draft-0.2/).
  out:  where every generated file goes (default: <this package>/results); every generated artifact is validated there.
Writes only under out, never overwriting an earlier run:
  run-results-0.27.json (later runs: run-results-0.27-rerun-<n>.json), v1-transcripts-0.27.json, hash-freeze-v1-0.27.json.
Exits 1 on any FAIL. Expected run time: a few minutes (pure-Python share and PoW searches for 13 x 256 shares).
"""

import ast
import copy
import hashlib
import importlib.util
import json
import os
import platform
import random
import re
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
INPUTS_OWN = ['tools/run_checks_027.py', 'tools/v1_ref_027.py', 'vectors/c32-error-cases.json', 'vectors/v1-sha-accounting.json',
              'annex/C32-RPC-ERROR-ENVELOPE.md', 'annex/V1-SHA-COST.md', 'owner/V1-SHA-COST-CHANGE-REQUEST.md', 'audit/consolidation-0.27.json']
INPUTS_ROOT = ['coordination/task-025.md', 'coordination/review-001/REVIEW-0.26.md', 'coordination/issue-ledger.json',
               'coordination/review-001/v1-malformed-error-probes-0.26.json', 'coordination/review-001/v1-share-work-probe-0.26.json',
               'coordination/review-001/m1-draft-0.26/results/run-results-0.26.json', 'coordination/review-001/m1-draft-0.26/results/v1-transcripts-0.26.json',
               'coordination/review-001/REVIEWER-DECISIONS-0.25.json', 'reference/browser.md', 'reference/consensus.md', 'reference/network.md',
               'reference/validation.md', 'm1-draft-0.26/tools/v1_ref.py', 'm1-draft-0.26/vectors/v1-window-cases.json']
PRESERVED = ['coordination/review-001/m1-draft-0.26/results/run-results-0.26.json', 'coordination/review-001/m1-draft-0.26/results/v1-transcripts-0.26.json',
             'coordination/review-001/v1-malformed-error-probes-0.26.json', 'coordination/review-001/v1-share-work-probe-0.26.json',
             'm1-draft-0.26/tools/v1_ref.py', 'm1-draft-0.26/tools/run_checks_026.py', 'm1-draft-0.26/vectors/v1-window-cases.json',
             'm1-draft-0.26/vectors/v1-chains.json', 'm1-draft-0.25/audit/row-inventory.json', 'm1-draft-0.5/annex/br-messages.json']

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
    results.append(make_entry(label, core_status, _snap(diag)))      # diagnostics copied at call time


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


def load_module(path, modname):
    spec = importlib.util.spec_from_file_location(modname, str(path))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


_lines = {}


def provenance(label, src):
    p = ROOT / src['file']
    if p not in _lines:
        _lines[p] = p.read_text(encoding='utf-8').splitlines() if p.exists() else []
    a, b = src['lines']
    seg = '\n'.join(_lines[p][a - 1:b])
    missing = [x for x in src.get('literals', []) if x not in seg]
    check(label + '.provenance', p.exists() and bool(seg) and not missing, file=src['file'], lines=[a, b], missingLiterals=missing)


def contains(rel, text):
    p = ROOT / rel
    return p.exists() and text in p.read_text(encoding='utf-8')


def num(v):
    if type(v) is int:
        return v
    s = str(v).replace(' ', '')
    m = re.fullmatch(r'(?:(\d+)\*)?2\^(\d+)([+-]\d+)?', s)
    if m:
        return int(m.group(1) or 1) * 2 ** int(m.group(2)) + int(m.group(3) or 0)
    return int(s)


sys.path.insert(0, str(HERE))
import v1_ref_027 as V                # noqa: E402  (this package only)

KEC = load_module(ROOT / 'm1-draft-0.2/tools/keccak.py', 'keccak_027')
RLP3 = load_module(ROOT / 'm1-draft-0.3/tools/rlp_strict.py', 'rlp_strict_03_027')
V.bind(KEC, RLP3)
kh = KEC.keccak256
V26DIR = 'm1-draft-0.26/'


# ------------------------------------------------------------------ evidence, decisions

INV25 = rj('m1-draft-0.25/audit/row-inventory.json')
CONS26 = rj(V26DIR + 'audit/consolidation-0.26.json')
CONS27 = own('audit/consolidation-0.27.json')
RESULT_FILES = dict(INV25['results'])
RESULT_FILES.update(CONS26['results'])
RESULT_FILES.update(CONS27['results'])
_RES = {}


def results_of(key):
    if key not in _RES:
        p = ROOT / RESULT_FILES[key]
        _RES[key] = json.loads(p.read_text(encoding='utf-8'))['results'] if p.exists() else None
    return _RES[key]


def match(key, pattern):
    res = results_of(key)
    if res is None:
        return None
    rx = re.compile(pattern)
    return [r for r in res if type(r.get('check')) is str and rx.search(r['check'])]


SUPERSEDED_OK = set()


def superseded_failures():
    for s in INV25['supersededFailures']:
        hit = match(s['results'], '^' + re.escape(s['check']) + '$') or []
        closure = match(*s['closure']) or []
        ok = len(hit) == 1 and hit[0].get('status') == 'FAIL' and any(r.get('status') == 'pass' for r in closure) \
            and not any(r.get('status') == 'FAIL' for r in closure)
        if ok:
            SUPERSEDED_OK.add((s['results'], s['check']))
        check('evidence.supersededFailure.%s.%s' % (s['results'], s['check']), ok, issue=s['issue'])


def evidence(label, key, pattern):
    hit = match(key, pattern)
    if hit is None:
        return check(label, False, results=RESULT_FILES[key], missingFile=True)
    st = Counter(r.get('status') for r in hit)
    fails = [r['check'] for r in hit if r.get('status') == 'FAIL']
    unexplained = [c for c in fails if (key, c) not in SUPERSEDED_OK]
    return check(label, st.get('pass', 0) >= 1 and not unexplained, results=RESULT_FILES[key], pattern=pattern, matched=len(hit),
                 passed=st.get('pass', 0), unexplainedFailures=unexplained)


def review_copies():
    for n in range(2, 27):
        pkg = 'm1-draft-0.%d' % n
        cp = ROOT / 'coordination' / 'review-001' / pkg
        if not cp.exists():
            record('evidence.reviewCopy.' + pkg, 'recorded', present=False)
            continue
        mism, only, compared = [], [], 0
        for f in sorted(cp.rglob('*')):
            if not f.is_file():
                continue
            rel = f.relative_to(cp)
            if rel.parts[0] == 'results' or '__pycache__' in rel.parts:
                continue
            main_file = ROOT / pkg / rel
            if not main_file.exists():
                only.append(rel.as_posix())
                continue
            compared += 1
            if f.read_bytes() != main_file.read_bytes():
                mism.append(rel.as_posix())
        check('evidence.reviewCopy.' + pkg, compared > 0 and not mism, compared=compared, mismatches=mism, copyOnly=only[:20])


DEC = rj('coordination/review-001/REVIEWER-DECISIONS-0.25.json')
ACCEPTED = {d['id'] for d in DEC['decisions'] if d['status'] == 'accepted-technical-qualified'}
OWNER_OPEN = {o['id'] for o in CONS26['decisions']['owner']} | {o['id'] for o in CONS27['ownerAdded']}
NOT_ADOPTED = {'ALT-E'}
PARAM_TOKEN = {'P-ABI': ['P-0.2', 'U22'], 'P13': ['P-0.2', 'U06'], 'P-X1': ['P-X1'], 'TK-1': ['TK-1']}


def decisions():
    check('decision.notOwnerApproved', DEC['ownerApproved'] is False)
    check('decision.noOwnerItemAccepted', not (ACCEPTED & {'U01', 'U02', 'U10', 'U14', 'CR-M1-01', 'RF-E6-1', 'U08', 'U05', 'U25', 'V1-SHA-COST'}))
    led = rj('coordination/issue-ledger.json')
    st = {i['id']: i for i in led['issues']}
    check('decision.ledger', st['C07']['status'] == 'open' and st['C06']['status'] == 'open' and st['U08-CR']['status'] == 'pending-owner-change-request'
          and 'unapproved' in st['RF-E6-1']['status'] and st['C32']['status'] == 'open-reference-defect'
          and st['V1-SHA-COST']['status'] == 'open-source-clarification',
          statuses={k: st[k]['status'] for k in ('C07', 'C06', 'U08-CR', 'RF-E6-1', 'C32', 'V1-SHA-COST', 'V1-DEFINITIONS')})
    cr = (PKG / 'owner/V1-SHA-COST-CHANGE-REQUEST.md').read_text(encoding='utf-8')
    check('decision.shaCostRouted', all(s in cr for s in ('| A |', '| B |', '| C |', '| D |', '| E |', '## Recommendation', '3355', 'Nothing here is adopted')))
    check('decision.P-V1-3withdrawn', any(w['id'] == 'P-V1-3' for w in CONS27['withdrawn']))


def binding_026():
    r = rj('coordination/review-001/m1-draft-0.26/results/run-results-0.26.json')
    s = r['summary']
    check('evidence.run026.preparedSuite', s['checks'] == 377 and s['passed'] == 302 and s['recorded'] == 75 and s['failed'] == 0, summary=s)
    statuses = {}
    for e in r['results']:
        if re.fullmatch(r'row\.[A-Z]\d+\.criteria', e['check']):
            statuses[e['check'][4:-9]] = e.get('proposalStatus')          # 0.26 recorded status=... -> renamed proposalStatus
    record('evidence.run026.rowStatuses', 'recorded', statuses=statuses, entries=len(statuses))
    mal = rj('coordination/review-001/v1-malformed-error-probes-0.26.json')
    check('evidence.rootProbes.malformed', mal['failed'] == 3 and sorted(x['check'] for x in mal['results']) == ['busyDataNull', 'missingBusyData', 'missingRetryDelay'])
    shp = rj('coordination/review-001/v1-share-work-probe-0.26.json')
    check('evidence.rootProbes.share', shp['totalSha256'] == 258 and shp['counters']['shareHash'] == 256 and shp['pass'] is True)
    return statuses


def hash_groups_025():
    exp = (ROOT / 'coordination/review-001/m1-draft-0.25/results/hash-freeze-export-0.25.json').read_text(encoding='utf-8')
    entries = [json.loads(line.rstrip(',')) for line in exp.split('\n')[1:] if line.startswith('{')]
    e05 = rj('coordination/review-001/e05-new-three-library-0.25.json')
    rootv = {x['id']: x for x in e05['results']}
    groups, bad = {}, []
    for e in entries:
        g = groups.setdefault(e['group'], {'blockedBy': [], 'pending': False})
        for p in e['ownerParameters']:
            if {'type': 'owner', 'id': p} not in g['blockedBy']:
                g['blockedBy'].append({'type': 'owner', 'id': p})
        for p in e['reviewerParameters']:
            if p in NOT_ADOPTED:
                b = {'type': 'notAdopted', 'id': p}
            elif not all(t in ACCEPTED for t in PARAM_TOKEN.get(p, [p])):
                b = {'type': 'reviewer', 'id': p}
            else:
                continue
            if b not in g['blockedBy']:
                g['blockedBy'].append(b)
        if e['verification'] == 'pendingRootTriad':
            rv = rootv.get(e['id'])
            if not (rv and rv['pass'] is True and rv['pythonKeccak'] == e['referenceDigest']):
                bad.append(e['id'])
                g['pending'] = True
    check('hash025.rootVerified', not bad, unverified=bad)
    for a, t in (('alias.versionRecords', 'triad.manifests'), ('alias.t1_02.manifestHash', 'triad.manifests'), ('alias.X1.netKey', 'triad.netKey')):
        groups[a] = groups.get(t, {'blockedBy': [], 'pending': True})
    for s in ('syn.lc.branchHashes', 'privateProvenance', 'ph.v3.networkProfiles'):
        groups[s] = {'blockedBy': [], 'pending': False}
    return groups


# ------------------------------------------------------------------ V1 build (0.26 fixtures, 0.27 builder)

def v1_network():
    net = rj(V26DIR + 'vectors/v1-network.json')
    gp = net['network']['genesisPre']
    segs = copy.deepcopy(rj('m1-draft-0.21/vectors/v3-gsv1.json')['flatSegments'])
    ok_seg = segs[gp['segmentIndex']] == gp['replace']
    segs[gp['segmentIndex']] = gp['with']
    pre = b''.join(bytes.fromhex(s) if isinstance(s, str) else bytes.fromhex(s['repeat']) * s['count'] for s in segs)
    NP = load_module(ROOT / 'm1-draft-0.21/tools/netprofile_ref.py', 'netprofile_021_027')
    spec = NP.decode_genesis(pre)
    cp_lit = {k: num(v) for k, v in net['network']['cp'].items()}
    check('V1.network.genesisPre', ok_seg and len(pre) == 341 and spec['chainId'] == 777002 and all(spec['CP'][k] == v for k, v in cp_lit.items()))
    cfg = {'chainId': spec['chainId'], 'genesisHash': kh(pre), 'forkSchedule': [tuple(x) for x in net['network']['forkSchedule']], 'cp': dict(spec['CP'])}
    return net, cfg, pre


def ec_selftests():
    br = rj('m1-draft-0.5/annex/br-messages.json')['txA']
    bad = [k['index'] for k in br['kats'] if '0x' + V.address_of(V.ec_mul(int(k['priv'], 16), V.G)).hex() != k['address']]
    check('V1.ec.addressesFromFixtureKeys', not bad, bad=bad)
    x = int(br['kats'][0]['priv'], 16)
    z = kh(b'V1 self-test')
    sig = V.sign(x, z)
    q = V.recover(z, sig)
    check('V1.ec.signRecoverRoundTrip', q is not None and V.address_of(q) == V.address_of(V.ec_mul(x, V.G)))
    return x


def units(cfg):
    u = rj(V26DIR + 'vectors/v1-units.json')
    bad = []
    for r in u['rw']['rows']:
        p = V.plan(num(r['h']))
        if [str(p['b']), str(p['from']), p['count'], p['n'], p['anchoredGenesis']] != [r['b'], r['from'], r['count'], r['n'], r['anchoredGenesis']]:
            bad.append(r['id'])
    for r in u['hc']['rows']:
        c = V.ceil_target(num(r['target_g']))
        if c != num(r['ceilTarget']) or ('minWork' in r and V.min_work(c) != num(r['minWork'])) \
                or ('message' in r and V.message(13, False, V.min_work(c)) != r['message']):
            bad.append(r['id'])
    for r in u['asert']['rows']:
        ctr = V.Counters()
        if V.asert(cfg['cp'], r['p_ts'], r['p_h'], ctr) != num(r['target']) or ctr.maxAsertBits > 512:
            bad.append(r['id'])
    check('V1.units.carried', not bad, bad=bad)
    rng = random.Random(u['hc']['random']['seed'])
    viol = None
    for _ in range(u['hc']['random']['samples']):
        tg = rng.randrange(1, V.TOP)
        c = V.ceil_target(tg)
        t = rng.randrange(1, c + 1)
        if V.work(t) < V.min_work(c):
            viol = [str(tg), str(t)]
            break
    check('V1.units.hcRandom', viol is None, firstViolation=viol)


def build_all(net, cfg, x):
    ch = rj(V26DIR + 'vectors/v1-chains.json')
    sa = own('vectors/v1-sha-accounting.json')
    d = net['templateDefaults']
    signer = V.address_of(V.ec_mul(x, V.G))
    k3 = kh(b'\x80')
    kother = kh(b'other')
    m = cfg['cp']['m']

    def base(h, ts, parent, target):
        return {'tag': V.TAG, 'chainId': d['chainId'], 'genesisHash': cfg['genesisHash'], 'parentHash': parent, 'h': h, 'a': d['a'],
                'protocolVersion': d['protocolVersion'], 'ts': ts, 'target': target, 'stateRoot': bytes.fromhex(d['stateRoot'][2:]),
                'txRoot': k3, 'receiptsRoot': k3, 'logsBloom': bytes(256), 'gasLimit': d['gasLimit'], 'gasUsed': d['gasUsed'],
                'baseFee': d['baseFee'], 'evidenceRoot': k3, 'proposer': signer}

    def nrule(s, base_nonce=None):
        if s in ('pow', 'fail'):
            return (s, None)
        v = s.split(':')[1]
        return ('fixed', base_nonce if v == 'base' else int(v))

    chains, fields = {}, {}
    H, parent = {}, cfg['genesisHash']
    for row in ch['chains']['H']['rows']:
        f = base(row['h'], row['ts'], parent, num(ch['chains']['H']['target']))
        H[row['h']], fields['H:%d' % row['h']] = V.build_header(f, x, nrule('pow'), 0, m), f
        parent = H[row['h']]['blockHash']
    chains['H'] = H
    for name in ('G', 'F'):
        c = ch['chains'][name]
        r = c['reference']
        f = base(r['h'], r['ts'], bytes.fromhex(r['parentHash'][2:]), num(r['target']))
        out = {r['h']: V.build_header(f, x, nrule(r['nonce']), 0, m)}
        fields['%s:%d' % (name, r['h'])] = f
        parent = out[r['h']]['blockHash']
        for row in c['rows']:
            f = base(row['h'], row['ts'], parent, num(c['target']))
            out[row['h']], fields['%s:%d' % (name, row['h'])] = V.build_header(f, x, nrule(c['nonce']), 0, m), f
            parent = out[row['h']]['blockHash']
        chains[name] = out
    Hc = {7: V.build_header(dict(fields['H:7']), x, ('fail', None), 0, m)}
    fields['Hc:7'] = dict(fields['H:7'])
    parent = Hc[7]['blockHash']
    for h in range(8, 21):
        f = base(h, fields['H:%d' % h]['ts'], parent, num(ch['chains']['Hc']['target']))
        Hc[h], fields['Hc:%d' % h] = V.build_header(f, x, ('pow', None), 0, m), f
        parent = Hc[h]['blockHash']
    chains['Hc'] = Hc
    variants = {}
    for vid, v in ch['variants'].items():
        bchain, bh = v['copyOf'].split(':')
        f = dict(fields[v['copyOf']])
        for k, val in v['set'].items():
            f[k] = (kother if val == '@KOTHER' else bytes.fromhex(val[2:])) if k in ('parentHash', 'genesisHash') else val
        variants[vid] = V.build_header(f, x, nrule(v['nonce'], chains[bchain][int(bh)]['nonce']), 0, m)
    # share-bearing fixtures (0.27)
    sc = sa['shareChains']
    variants['S1'] = V.build_header(dict(fields['H:1']), x, ('pow', None), sc['S1']['shares'], m)
    variants['S1bad'] = V.build_header(dict(fields['H:1']), x, ('pow', None), sc['S1bad']['shares'], m, invalid_after=True)
    W = {7: H[7]}
    parent = H[7]['blockHash']
    for h in range(8, 21):
        f = base(h, fields['H:%d' % h]['ts'], parent, num(ch['chains']['H']['target']))
        W[h], fields['W:%d' % h] = V.build_header(f, x, ('pow', None), sc['W']['shares'], m), f
        parent = W[h]['blockHash']
    chains['W'] = W
    sizes = [len(hd['encoded']) for hd in list(W.values())[1:]]
    check('SHA.build.shareFixtures', len(variants['S1']['shares']) == 256 and len(variants['S1bad']['shares']) == 3
          and all(len(hd['shares']) == 256 for hd in list(W.values())[1:]) and max(sizes) <= V.HDR_MAX,
          headerBytes=sizes, s1First=variants['S1']['shares'][:4])
    return chains, variants, fields


def resolve_reply(tokens, chains, variants):
    out = []
    for t in tokens:
        if t in variants:
            out.append(variants[t])
            continue
        name, rng = t.split(':')
        if '..' in rng:
            a, b = (int(v) for v in rng.split('..'))
            out += [chains[name][h] for h in range(a, b + 1)]
        else:
            out.append(chains[name][int(rng)])
    return out


def headers_text(hdrs):
    return json.dumps({'jsonrpc': '2.0', 'id': 2, 'result': '0x' + RLP3.encode([hd['item'] for hd in hdrs]).hex()}, separators=(',', ':'))


def run(cfg, replies, clock, t0=0, cache=True):
    rpc = V.ScriptedRpc(replies, t0)
    ctr = V.Counters()
    try:
        out = V.check_window(cfg, rpc, clock, ctr, cache)
        exc = None
    except Exception as e:                                           # must never happen (C32)
        out, exc = {'ok': None}, '%s: %s' % (type(e).__name__, e)
    return out, ctr, rpc, exc


# ------------------------------------------------------------------ V1 cases carried from 0.26 with the 0.27 checker

def carried_cases(cfg, chains, variants):
    doc = rj(V26DIR + 'vectors/v1-window-cases.json')
    acc = {c['case']: c for c in own('vectors/v1-sha-accounting.json')['inheritedCases']}
    t26 = rj('coordination/review-001/m1-draft-0.26/results/v1-transcripts-0.26.json')['transcripts']
    t26 = {(t['case'], t['phase']): t for t in t26}
    transcripts, totals = [], {}
    for case in doc['cases']:
        cid = case['id']
        calls, outcomes, last = [], [], None
        byte_equal = True
        for i, ph in enumerate(case['phases']):
            replies26 = [{'jsonrpc': '2.0', 'id': 1, 'result': ph['blockNumber']}]
            if ph.get('reply'):
                replies26.append({'jsonrpc': '2.0', 'id': 2, 'result': '0x' + RLP3.encode([hd['item'] for hd in resolve_reply(ph['reply'], chains, variants)]).hex()})
            for err in ph.get('replyError', []):
                replies26.append({'jsonrpc': '2.0', 'id': 2, 'error': err})
            old = t26.get((cid, i))
            byte_equal = byte_equal and old is not None and old['replies'] == replies26
            texts = [json.dumps(r, separators=(',', ':')) for r in replies26]
            out, ctr, rpc, exc = run(cfg, texts, ph['clock'], ph.get('t', 0), True)
            out_u, ctr_u, _, exc_u = run(cfg, texts, ph['clock'], ph.get('t', 0), False)
            calls += rpc.calls
            outcomes.append(out)
            last = (out, ctr, out_u, ctr_u, exc or exc_u)
            transcripts.append({'case': cid, 'phase': i, 'replies': texts, 'requests': rpc.calls, 'outcome': out, 'counters': ctr.as_dict(),
                                'countersUncached': ctr_u.as_dict()})
        out, ctr, out_u, ctr_u, exc = last
        cd, cu = ctr.as_dict(), ctr_u.as_dict()
        cells = {'noException': exc is None,
                 'replyBytesEqual0_26': byte_equal,
                 'requests': [[c['method'], c['params']] for c in calls] == case['requests'],
                 'outcome': all(out.get(k) == v for k, v in case['expect'].items()),
                 'counters': all(cd[k] == v for k, v in case['counters'].items()),
                 'uncachedSameOutcome': out_u == out}
        if 'requestTimes' in case:
            cells['requestTimes'] = [c['t'] for c in calls] == case['requestTimes']
        if 'expectPhase1' in case:
            cells['phase1'] = all(outcomes[0].get(k) == v for k, v in case['expectPhase1'].items())
        if not out.get('ok') and out.get('at') is not None and ctr.events:
            cells['earlyExit'] = ctr.events[-1][1] == out['at']
        a = acc[cid]
        cells['sha'] = (cd['sha256ByCategory'] == {'templateId': a['templateId'], 'powHash': a['powHash'], 'shareHash': a['shareHash']}
                        and cd['sha256Total'] == a['total'] and cu['sha256Total'] == a['uncachedTotal'] and cd['sha256UniquePreimages'] == a['unique']
                        and cu['sha256UniquePreimages'] == a['unique'])
        cells['literal13Column'] = (cd['sha256Total'] <= 13) == a['literal13']
        bad = sorted(k for k, v in cells.items() if not v)
        check('V1.case.' + cid, not bad, failingCells=bad, outcome={k: out.get(k) for k in ('ok', 'rule', 'at', 'n')}, counters=cd,
              uncachedSha=cu['sha256Total'], exception=exc)
        totals[cid] = cd['sha256Total']
    return transcripts, totals


# ------------------------------------------------------------------ C32

def c32(cfg, chains):
    doc = own('vectors/c32-error-cases.json')
    for k, src in doc['sources'].items():
        provenance('C32.source.' + k, src)
    ok_text = headers_text([chains['H'][h] for h in range(7, 21)])
    transcripts, raised = [], []
    for case in doc['cases']:
        replies = [case.get('blockNumberReply', doc['context']['blockNumberReply'])] + [ok_text if r == '@HEADERS' else r for r in case['replies']]
        out, ctr, rpc, exc = run(cfg, replies, 1700000205)
        if exc:
            raised.append(case['id'])
        want_req = case.get('requests') or ([['eth_blockNumber', []]] + [['pocol_getHeaders', ['0x7', '0xe']]] * (len(case['times']) - 1))
        cells = {'noException': exc is None,
                 'requests': [[c['method'], c['params']] for c in rpc.calls] == want_req,
                 'times': [c['t'] for c in rpc.calls] == case['times'],
                 'slept': ctr.sleptMs == case['slept']}
        if case['expect'] == 'ok':
            cells['outcome'] = out.get('ok') is True and out.get('n') == 13 and out.get('minWork') == 4095
        else:
            cells['outcome'] = out.get('ok') is False and out.get('rule') == 'viewIncomplete' and out.get('frame') is False and out.get('cancel') == 4901
            cells['noHeaderWork'] = ctr.decodes == 0 and sum(ctr.sha.values()) == 0
        bad = sorted(k for k, v in cells.items() if not v)
        check('C32.case.' + case['id'], not bad, failingCells=bad, outcome={k: out.get(k) for k in ('ok', 'rule', 'detail')}, times=[c['t'] for c in rpc.calls],
              exception=exc)
        transcripts.append({'case': case['id'], 'replies': replies, 'requests': rpc.calls, 'outcome': out, 'sleptMs': ctr.sleptMs})
    check('C32.rootProbesCovered', all(any(c.get('rootProbe') == p for c in doc['cases']) for p in ('missingBusyData', 'busyDataNull', 'missingRetryDelay')))
    check('C32.noCaseRaises', not raised, raised=raised)
    check('C32.maxTotalWait', max(c['slept'] for c in doc['cases']) == 3 * 2000, note='3 retries x 2000 ms; inside the 10 s RP protection')
    return transcripts


# ------------------------------------------------------------------ SHA accounting, new share cases

def sha_cases(cfg, chains, variants):
    doc = own('vectors/v1-sha-accounting.json')
    for k, src in doc['sources'].items():
        provenance('SHA.source.' + k, src)
    transcripts = []
    for case in doc['newCases']:
        texts = [json.dumps({'jsonrpc': '2.0', 'id': 1, 'result': case['blockNumber']}, separators=(',', ':')),
                 headers_text(resolve_reply(case['reply'], chains, variants))]
        out, ctr, rpc, exc = run(cfg, texts, case['clock'], 0, True)
        out_u, ctr_u, _, exc_u = run(cfg, texts, case['clock'], 0, False)
        cd, cu, s = ctr.as_dict(), ctr_u.as_dict(), case['sha']
        cells = {'noException': exc is None and exc_u is None,
                 'requests': [[c['method'], c['params']] for c in rpc.calls] == case['requests'],
                 'outcome': all(out.get(k) == v for k, v in case['expect'].items()),
                 'counters': all(cd[k] == v for k, v in case['counters'].items()),
                 'shaCached': cd['sha256ByCategory'] == {'templateId': s['templateId'], 'powHash': s['powHash'], 'shareHash': s['shareHash']} and cd['sha256Total'] == s['total'],
                 'shaUncached': cu['sha256Total'] == s['uncachedTotal'] and out_u == out,
                 'unique': cd['sha256UniquePreimages'] == s['unique'] == cu['sha256UniquePreimages'],
                 'literal13Column': (cd['sha256Total'] <= 13) == s['literal13'],
                 'boundsOtherThanSha': cd['headerDecodes'] <= 14 and cd['asert'] <= 13 and cd['maxAsertBits'] <= 512}
        bad = sorted(k for k, v in cells.items() if not v)
        check('SHA.case.' + case['id'], not bad, failingCells=bad, counters=cd, uncached=cu['sha256Total'], outcome={k: out.get(k) for k in ('ok', 'rule', 'at', 'n')})
        record('SHA.sourceBound13.' + case['id'], 'recorded', literalBound=13, cachedTotal=cd['sha256Total'], satisfied=cd['sha256Total'] <= 13,
               note='recorded, never passed: the source bound is an open owner change request')
        transcripts.append({'case': case['id'], 'replies': texts, 'requests': rpc.calls, 'outcome': out, 'counters': cd, 'countersUncached': cu})
    sb = doc['structuralBound']
    check('SHA.structuralBound', 14 + 13 + 13 * 256 == 3355 and (13 + 13 * (3 + 256)) + 13 + 3328 == 6721 and '3355' in sb['cached'] and '6721' in sb['uncached'])
    probe = rj('coordination/review-001/v1-share-work-probe-0.26.json')
    s1 = next(t for t in transcripts if t['case'] == 'SHA-S1')
    check('SHA.matchesRootProbe', s1['counters']['sha256Total'] == probe['totalSha256'] and s1['counters']['shareHash'] == probe['counters']['shareHash'])
    return transcripts


def hash_entries(cfg, chains, variants):
    out = []

    def add(eid, group, alg, pre, dig, extra=None):
        e = {'id': eid, 'group': group, 'algorithm': alg, 'preimageHex': pre.hex(), 'digest': dig.hex(), 'verification': 'pendingRootVerification'}
        if extra:
            e.update(extra)
        out.append(e)

    named = {'S1': variants['S1'], 'S1bad': variants['S1bad']}
    named.update({'W:%d' % h: hd for h, hd in chains['W'].items() if h > 7})
    t_share = {name: min(V.U256_MAX, cfg['cp']['target_g'] * cfg['cp']['m']) for name in named}
    for name, hd in sorted(named.items()):
        add('v1.templateId:' + name, 'v1.templateId', 'sha256', hd['utRaw'], hd['tid'])
        add('v1.powHash:' + name, 'v1.powHash', 'sha256', hd['tid'] + V.be64(hd['nonce']), hd['powHash'])
        add('v1.blockHash:' + name, 'v1.blockHash', 'keccak256', hd['tid'] + V.be64(hd['nonce']) + hd['shareRoot'], hd['blockHash'])
        add('v1.shareRoot:' + name, 'v1.shareRoot', 'keccak256', RLP3.encode([V.be64(n) for n in hd['shares']]), hd['shareRoot'])
        add('v1.sigMsg:' + name, 'v1.sigMsg', 'keccak256', b'\x19' + b'PoCol template' + b'\x0a' + hd['tid'], hd['sigMsg'])
        add('v1.winMsg:' + name, 'v1.winMsg', 'keccak256', b'\x19' + b'PoCol winner' + b'\x0a' + hd['tid'] + V.be64(hd['nonce']) + hd['shareRoot'], hd['winMsg'])
        for i, n in enumerate(hd['shares']):
            pre = hd['tid'] + V.be64(n)
            dig = hashlib.sha256(pre).digest()
            add('v1.shareHash:%s:%03d' % (name, i), 'v1.shareHash', 'sha256', pre, dig,
                {'nonce': n, 'withinTShare': int.from_bytes(dig, 'big') <= t_share[name]})
    bad = [e['id'] for e in out if e['algorithm'] == 'sha256' and hashlib.sha256(bytes.fromhex(e['preimageHex'])).hexdigest() != e['digest']]
    within = Counter(e.get('withinTShare') for e in out if e['group'] == 'v1.shareHash')
    check('SHA.hashEntries', not bad and within[False] == 1 and within[True] == 256 + 2 + 13 * 256, entries=len(out), within=dict(within))
    out.sort(key=lambda e: e['id'])
    return out


# ------------------------------------------------------------------ consolidation

def consolidate(groups, statuses026):
    have = {r['check']: r['status'] for r in results}
    supp_pattern = {'S25-SWEEPMAX': '^S25\\.sweepMax\\.', 'S25-E07': '^S25\\.e07\\.', 'S25-LCLIT': '^S25\\.lclit\\.', 'S25-RG3B': '^S25\\.rg3b\\.'}
    changes = {}
    for src in (CONS26['rowChanges'], CONS27['rowChanges']):
        for k, v in src.items():
            changes.setdefault(v.get('row', k), []).append(v)
    computed = {}
    for row in INV25['rows']:
        rid = row['id']
        ch = changes.get(rid, [])
        owner = list(row['owner']) + [t for c in ch for t in c.get('addOwner', [])]
        reviewer = [t for t in row['reviewer'] if not any(t in c.get('removeReviewer', []) for c in ch)] + [t for c in ch for t in c.get('addReviewer', [])]
        missing = [s['file'] for s in row['spec'] if not (ROOT / s['file']).exists()]
        missing += [s['file'] for s in row['spec'] if s.get('contains') and (ROOT / s['file']).exists() and not contains(s['file'], s['contains'])]
        missing += [f for f in row['fixtures'] + row['review'] if not (ROOT / f).exists()]
        for c in ch:
            for f in c.get('addSpec', []) + c.get('addFixtures', []):
                p = PKG / f.replace('m1-draft-0.27/', '') if f.startswith('m1-draft-0.27/') else ROOT / f
                if not p.exists():
                    missing.append(f)
        ev = [evidence('row.%s.evidence.%s:%s' % (rid, k, p), k, p) for k, p in row['evidence']]
        ev += [evidence('row.%s.evidence.%s:%s' % (rid, k, p), k, p) for c in ch for k, p in c.get('addEvidence', [])]
        cur_ok = True
        for p in [p for c in ch for p in c.get('currentRunEvidence', [])]:
            mine = [s for k, s in have.items() if re.search(p, k)]
            cur_ok = cur_ok and bool(mine) and all(s != 'FAIL' for s in mine)
        pending = any(c.get('pendingRootReview') for c in ch)
        resolved_now = [m for c in ch for m in c.get('resolved', [])]
        unresolved = []
        for m in row['missing']:
            if m.get('resolvedBy') in supp_pattern:
                if not evidence('row.%s.supplement.%s' % (rid, m['resolvedBy']), 'R25', supp_pattern[m['resolvedBy']]):
                    unresolved.append(m['id'])
            elif m['id'] not in resolved_now:
                unresolved.append(m['id'])
        rev_open = [t for t in reviewer if t not in ACCEPTED]
        own_open = [t for t in owner if t in OWNER_OPEN]
        hash_owner = [g for g in row['hashGroups'] if any(b['type'] == 'owner' for b in groups.get(g, {}).get('blockedBy', []))]
        hash_pending = any(groups.get(g, {}).get('pending') for g in row['hashGroups'])
        c1 = 'blocked' if (missing or unresolved) else ('pendingRootReview' if pending else 'satisfied')
        c2 = 'blocked' if not (all(ev) and cur_ok) else ('pendingRootReview' if pending else 'satisfied')
        c3 = 'pendingRootReview' if pending else 'satisfied'
        c4 = 'blocked' if own_open else ('pendingReviewerDecision' if rev_open else 'satisfied')
        c5 = 'blocked' if (own_open or unresolved or hash_owner) else ('pendingRootReview' if (pending or hash_pending) else 'satisfied')
        crit = {'c1': c1, 'c2': c2, 'c3': c3, 'c4': c4, 'c5': c5}
        computed[rid] = 'CompleteCandidate' if all(v == 'satisfied' for v in crit.values()) else 'Partial'
        record('row.%s.criteria' % rid, 'recorded', criteria=crit, status=computed[rid], ownerOpen=own_open, reviewerOpen=rev_open, unresolved=unresolved,
               hashOwnerBlocked=hash_owner, missing=missing)
    prop = CONS27['proposedStatus']
    want = {r: 'CompleteCandidate' for r in prop['CompleteCandidate']}
    want.update({r: 'Partial' for r in prop['Partial']})
    check('consolidation.rowsMatchProposal', computed == want, differing={r: [computed.get(r), want.get(r)] for r in set(computed) | set(want) if computed.get(r) != want.get(r)})
    check('consolidation.carriedFrom0_26', bool(statuses026) and all(statuses026.get(r) == s for r, s in computed.items()),
          differing={r: [statuses026.get(r), s] for r, s in computed.items() if statuses026.get(r) != s})
    record('consolidation.summary', 'recorded', counts=dict(Counter(computed.values())), rows=computed)
    return computed


# ------------------------------------------------------------------ boundary, outputs, main

def boundary_static(texts):
    bad = {}
    for f in (HERE / 'v1_ref_027.py', HERE / 'run_checks_027.py'):
        tree = ast.parse(f.read_text(encoding='utf-8'))
        mods = {a.name.split('.')[0] for n in ast.walk(tree) if isinstance(n, ast.Import) for a in n.names}
        mods |= {(n.module or '').split('.')[0] for n in ast.walk(tree) if isinstance(n, ast.ImportFrom)}
        hit = sorted(mods & {'socket', 'urllib', 'http', 'requests', 'ssl', 'asyncio', 'subprocess'})
        if hit:
            bad[f.name] = hit
    check('boundary.noNetworkModules', not bad, imports=bad)
    markers = ('PoCol_' + 'Dialogue', 'state' + '.json', 'source' + 'State', 'historical-x8-full' + '-source-private', 'transcript' + '-private')
    leaks = [str(f.relative_to(PKG)) for f in sorted(PKG.rglob('*')) if f.is_file() and 'results' not in f.parts
             and f.suffix in ('.json', '.md', '.py') and any(m in f.read_text(encoding='utf-8') for m in markers)]
    check('boundary.noPrivatePathInPackage', not leaks, files=leaks)
    keys = [v['priv'][2:].lower() for v in rj('coordination/review-001/m1-draft-0.5/results/txa-node-check.json')['keys'].values()]
    leaked = [name for name, t in texts.items() if any(k in t.lower() for k in keys)]
    check('boundary.noPrivateKeyInOutputs', not leaked, files=leaked)


def write_asset(name, text, label):
    OUT.mkdir(parents=True, exist_ok=True)
    p = OUT / name
    data = text.encode('utf-8')
    if p.exists() and p.read_bytes() != data:
        k = 1
        while (OUT / ('%s-rerun-%d%s' % (p.stem, k, p.suffix))).exists():
            k += 1
        alt = OUT / ('%s-rerun-%d%s' % (p.stem, k, p.suffix))
        alt.write_bytes(data)
        check(label + '.matchesExisting', False, kept=name, written=alt.name)
        return
    if not p.exists():
        p.write_bytes(data)
    check(label + '.writtenAndReadBack', p.read_bytes() == data, file=name, sha256=hashlib.sha256(data).hexdigest(), bytes=len(data))


def coverage():
    have = {r['check']: r['status'] for r in results}
    need = ['V1.case.' + c['id'] for c in rj(V26DIR + 'vectors/v1-window-cases.json')['cases']]
    need += ['C32.case.' + c['id'] for c in own('vectors/c32-error-cases.json')['cases']]
    need += ['SHA.case.' + c['id'] for c in own('vectors/v1-sha-accounting.json')['newCases']]
    need += ['C32.rootProbesCovered', 'C32.noCaseRaises', 'C32.maxTotalWait', 'SHA.structuralBound', 'SHA.matchesRootProbe', 'SHA.hashEntries',
             'SHA.build.shareFixtures', 'V1.network.genesisPre', 'V1.units.carried', 'V1.units.hcRandom', 'V1.ec.addressesFromFixtureKeys',
             'V1.ec.signRecoverRoundTrip', 'V1.transcripts.writtenAndReadBack', 'V1.hashExport.writtenAndReadBack', 'consolidation.rowsMatchProposal',
             'consolidation.carriedFrom0_26', 'decision.ledger', 'decision.shaCostRouted', 'decision.P-V1-3withdrawn', 'decision.noOwnerItemAccepted',
             'evidence.run026.preparedSuite', 'evidence.rootProbes.malformed', 'evidence.rootProbes.share', 'hash025.rootVerified', 'boundary.noPrivateKeyInOutputs']
    missing = [x for x in need if have.get(x) != 'pass']
    check('coverage027.required', not missing, missing=missing, required=len(need))
    check('coverage027.noStepAborted', not [k for k in have if 'step.completed' in k])


def out_path():
    p = OUT / 'run-results-0.27.json'
    k = 1
    while p.exists():
        p = OUT / ('run-results-0.27-rerun-%d.json' % k)
        k += 1
    return p


def main():
    t0 = time.time()
    before = {rel: sha(ROOT / rel) for rel in PRESERVED}
    for rel in INPUTS_OWN:
        record('input.sha256 <package>/' + rel, 'recorded', exists=(PKG / rel).exists(), sha256=sha(PKG / rel))
    for rel in INPUTS_ROOT:
        record('input.sha256 ' + rel, 'recorded', exists=(ROOT / rel).exists(), sha256=sha(ROOT / rel))
    check('boundary.positionalOnly', record.__code__.co_posonlyargcount == 2 and check.__code__.co_posonlyargcount == 2 and gap.__code__.co_posonlyargcount == 1)
    probe = {'x': [1]}
    record('boundary.probe', 'recorded', name='n', status='s', obj=probe)
    probe['x'].append(2)
    e = results[-1]
    check('boundary.renamedAndCopied', e['diagName'] == 'n' and e['proposalStatus'] == 's' and e['status'] == 'recorded' and e['obj'] == {'x': [1]})
    calls = [n.lineno for n in ast.walk(ast.parse(Path(__file__).read_text(encoding='utf-8')))
             if isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute) and n.func.attr == 'main']
    check('boundary.noOldMainCalled', not calls, attributeMainCallsAtLines=calls)
    texts, groups, statuses026 = {}, {}, {}
    for name, fn in (('superseded', superseded_failures), ('reviewCopies', review_copies), ('decisions', decisions)):
        try:
            fn()
        except Exception as ex:                                              # record, never hide
            check('step.completed ' + name, False, exception='%s: %s' % (type(ex).__name__, ex))
    try:
        statuses026 = binding_026()
        groups = hash_groups_025()
    except Exception as ex:
        check('step.completed binding', False, exception='%s: %s' % (type(ex).__name__, ex))
    try:
        net, cfg, pre = v1_network()
        x = ec_selftests()
        units(cfg)
        chains, variants, fields = build_all(net, cfg, x)
        tr1, totals = carried_cases(cfg, chains, variants)
        tr2 = c32(cfg, chains)
        tr3 = sha_cases(cfg, chains, variants)
        texts['v1-transcripts-0.27.json'] = json.dumps({'schema': 'pocol-m1-v1-transcripts/0.27', 'carried': tr1, 'c32': tr2, 'sha': tr3},
                                                       sort_keys=True, separators=(',', ':'), ensure_ascii=True, default=str) + '\n'
        hx = hash_entries(cfg, chains, variants)
        texts['hash-freeze-v1-0.27.json'] = json.dumps({'schema': 'pocol-m1-hash-freeze-v1/0.27', 'note': 'not frozen; root verifies sha256 and keccak256 entries independently',
                                                        'counts': dict(Counter(e['group'] for e in hx))}, sort_keys=True, separators=(',', ':'))[:-1] \
            + ',"list":[\n' + ',\n'.join(json.dumps(e, sort_keys=True, separators=(',', ':')) for e in hx) + '\n]}\n'
    except Exception as ex:
        check('step.completed v1', False, exception='%s: %s' % (type(ex).__name__, ex))
    try:
        boundary_static(texts)
        if 'v1-transcripts-0.27.json' in texts:
            write_asset('v1-transcripts-0.27.json', texts['v1-transcripts-0.27.json'], 'V1.transcripts')
            write_asset('hash-freeze-v1-0.27.json', texts['hash-freeze-v1-0.27.json'], 'V1.hashExport')
    except Exception as ex:
        check('step.completed outputs', False, exception='%s: %s' % (type(ex).__name__, ex))
    computed = {}
    try:
        computed = consolidate(groups, statuses026)
    except Exception as ex:
        check('step.completed consolidation', False, exception='%s: %s' % (type(ex).__name__, ex))
    gap('gap.V1-SHA-COST', text='browser.md:58 "<= 13 SHA-256" cannot be met by a full window (27 without shares, 3355 with 256 shares per header); owner change request open')
    gap('gap.ownerDecisions', text='U01, U02, U10, U14, CR-M1-01, U08, RF-E6-1 and V1-SHA-COST have no owner answer')
    gap('gap.reviewerDecisions', text='P-V1 (minus the withdrawn P-V1-3) and P-C32-1..4 await root decisions')
    coverage()
    after = {rel: sha(ROOT / rel) for rel in PRESERVED}
    check('preserved.olderFilesUnchanged', before == after, before=before, after=after)
    failed = [r for r in results if r['status'] == 'FAIL']
    out = {'package': 'M1 draft 0.27 (C32 error envelope; V1 SHA-256 accounting; carried 0.26 evidence)',
           'executedAtUtc': datetime.now(timezone.utc).isoformat(timespec='seconds'),
           'environment': {'python': sys.version.split()[0], 'platform': platform.platform(), 'thirdPartyPackages': 'none'},
           'statusSchema': {'coreStatus': list(CORE_STATUS), 'diagnosticRenames': RENAMED, 'partialGap': 'recorded entries with partialGap=true'},
           'evidenceKind': 'reference models of the specification; saved reviewed evidence re-bound; no browser, network, EVM or chain',
           'notExecuted': ['Phase A experiments', 'any owner decision'],
           'summary': {'checks': len(results), 'passed': sum(r['status'] == 'pass' for r in results),
                       'recorded': sum(r['status'] == 'recorded' for r in results), 'failed': len(failed),
                       'partialGaps': sum(1 for r in results if r.get('partialGap')),
                       'rows': dict(Counter(computed.values())) if computed else None, 'seconds': round(time.time() - t0, 1)},
           'results': results}
    OUT.mkdir(parents=True, exist_ok=True)
    target = out_path()
    target.write_text(json.dumps(out, indent=2, ensure_ascii=True, default=str) + '\n', encoding='utf-8')
    print(json.dumps(out['summary']), '->', target.name)
    for f in failed:
        print('FAIL', json.dumps(f, ensure_ascii=True, default=str)[:400])
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
