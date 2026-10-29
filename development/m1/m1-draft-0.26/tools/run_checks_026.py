"""Reference-only acceptance runner for M1 draft 0.26 (author turn 024): V1 HeaderNetCheck vectors and reference
checker; consolidated R3-08 criteria under the recorded 0.25 reviewer decisions; N26 normative representation
fixtures; hash-freeze extension. Python standard library only; no network, browser, EVM or chain.
NOT executed by the author.

Usage (any of):
  coordination\\runtime\\python311\\python.exe m1-draft-0.26\\tools\\run_checks_026.py
  ...\\run_checks_026.py --root D:\\PoCol-Development --out <dir>
Environment alternatives: POCOL_M1_ROOT, POCOL_M1_OUT.
  root: the tree holding reference/, coordination/ and m1-draft-0.2 ... 0.25 (default: the nearest ancestor of this
        file that holds both reference/ and m1-draft-0.2/).
  out:  where every generated file goes (default: <this package>/results). Every generated artifact is read back and
        validated from THIS directory, never from a fixed tree (fixes the 0.25 copy-run output-path failure).
Writes only under out (never overwriting an earlier run):
  run-results-0.26.json (a later run: run-results-0.26-rerun-<n>.json), v1-transcripts-0.26.json, hash-freeze-v1-0.26.json.
Exits 1 on any FAIL.
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
INPUTS_OWN = ['tools/run_checks_026.py', 'tools/v1_ref.py', 'vectors/v1-network.json', 'vectors/v1-chains.json', 'vectors/v1-window-cases.json',
              'vectors/v1-units.json', 'vectors/n26-representation-cases.json', 'supplements/s26-v1-experiments.json', 'annex/V1-HEADERNETCHECK.md',
              'normative/N26-REPRESENTATION.md', 'owner/U08-CHANGE-REQUEST.md', 'audit/consolidation-0.26.json', 'audit/findings-trace-0.26.json',
              'hash/hash-freeze-plan-0.26.json']
INPUTS_ROOT = ['coordination/task-024.md', 'coordination/review-001/REVIEW-0.25.md', 'coordination/review-001/REVIEWER-DECISIONS-0.25.json',
               'coordination/issue-ledger.json', 'coordination/review-001/m1-draft-0.25/results/run-results-0.25.json',
               'coordination/review-001/m1-draft-0.25/results/hash-freeze-export-0.25.json', 'coordination/review-001/e05-new-three-library-0.25.json',
               'coordination/review-001/run-results-0.25-copy-output-path-failure.json', 'reference/browser.md', 'reference/consensus.md',
               'reference/validation.md', 'reference/network.md', 'reference/implementation.md']
PRESERVED = ['coordination/review-001/m1-draft-0.25/results/run-results-0.25.json', 'coordination/review-001/run-results-0.25-copy-output-path-failure.json',
             'coordination/review-001/REVIEWER-DECISIONS-0.25.json', 'm1-draft-0.25/audit/row-inventory.json', 'm1-draft-0.25/tools/supplements_ref.py',
             'm1-draft-0.21/vectors/v3-gsv1.json', 'm1-draft-0.13/tools/fmt2_codec.py', 'm1-draft-0.4/tools/bridge_ref.py', 'm1-draft-0.5/annex/br-messages.json']

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
    """int, decimal string, '2^k', '2^k-1', '2^k+1', 'm*2^k'."""
    if type(v) is int:
        return v
    s = str(v).replace(' ', '')
    m = re.fullmatch(r'(?:(\d+)\*)?2\^(\d+)([+-]\d+)?', s)
    if m:
        return int(m.group(1) or 1) * 2 ** int(m.group(2)) + int(m.group(3) or 0)
    return int(s)


sys.path.insert(0, str(HERE))
import v1_ref as V                    # noqa: E402  (this package only)

KEC = load_module(ROOT / 'm1-draft-0.2/tools/keccak.py', 'keccak_026')
RLP3 = load_module(ROOT / 'm1-draft-0.3/tools/rlp_strict.py', 'rlp_strict_03_026')
V.bind(KEC, RLP3)
kh = KEC.keccak256


# ------------------------------------------------------------------ evidence reuse and decisions

INV25 = rj('m1-draft-0.25/audit/row-inventory.json')
CONS = own('audit/consolidation-0.26.json')
RESULT_FILES = dict(INV25['results'])
RESULT_FILES.update(CONS['results'])
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
                 passed=st.get('pass', 0), recorded=st.get('recorded', 0), unexplainedFailures=unexplained, sample=[r['check'] for r in hit[:3]])


def review_copies():
    for n in range(2, 26):
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
ACCEPTED = {d['id'] for d in DEC['decisions'] if d['status'] == CONS['decisions']['acceptedStatus']}
OWNER_OPEN = {o['id'] for o in CONS['decisions']['owner']}
NOT_ADOPTED = {'ALT-E'}
PARAM_TOKEN = {'P-ABI': ['P-0.2', 'U22'], 'P13': ['P-0.2', 'U06'], 'P-X1': ['P-X1'], 'TK-1': ['TK-1']}


def decisions():
    check('decision.notOwnerApproved', DEC['ownerApproved'] is False)
    for d in DEC['decisions']:
        if d['status'] == CONS['decisions']['acceptedStatus']:
            check('decision.source.' + d['id'], contains(d['source']['file'], d['source']['find']), file=d['source']['file'])
    u08 = next((d for d in DEC['decisions'] if d['id'] == 'U08'), None)
    check('decision.U08isOwnerChangeRequest', u08 is not None and u08['status'] == 'pending-owner-change-request' and 'U08' not in ACCEPTED)
    check('decision.noOwnerItemAccepted', not (ACCEPTED & {'U01', 'U02', 'U10', 'U14', 'CR-M1-01', 'RF-E6-1', 'U08', 'U05', 'U25'}), accepted=sorted(ACCEPTED))
    led = rj('coordination/issue-ledger.json')
    st = {i['id']: i for i in led['issues']}
    check('decision.ledger.ownerItemsOpen', st['C07']['status'] == 'open' and st['C06']['status'] == 'open'
          and st['U08-CR']['status'] == 'pending-owner-change-request' and 'unapproved' in st['RF-E6-1']['status']
          and st['V1-DEFINITIONS']['status'] == 'open-authoring',
          statuses={k: st[k]['status'] for k in ('C07', 'C06', 'U08-CR', 'RF-E6-1', 'V1-DEFINITIONS')})
    check('decision.U07conditionalOnly', 'U07' in DEC['qualifications'] and 'not a measured gas result' in DEC['qualifications']['U07'])
    check('decision.P02qualified', 'U08/P22' in DEC['qualifications']['P-0.2'] and 'U01/P28' in DEC['qualifications']['P-0.2'])
    cr = (PKG / 'owner/U08-CHANGE-REQUEST.md').read_text(encoding='utf-8')
    check('decision.U08routed', all(s in cr for s in ('| A |', '| B |', '| C |', '| D |', '## Recommendation', 'exactly 6 keys'))
          and 'U08' in st['U08-CR']['id'])


def binding_025():
    r = rj('coordination/review-001/m1-draft-0.25/results/run-results-0.25.json')
    s = r['summary']
    check('evidence.run025.canonical', s['checks'] == 702 and s['passed'] == 617 and s['recorded'] == 85 and s['failed'] == 0, summary=s)
    f = rj('coordination/review-001/run-results-0.25-copy-output-path-failure.json')
    fails = sorted(x['check'] for x in f['results'] if x['status'] == 'FAIL')
    check('evidence.run025.retainedFailureUnchanged', fails == ['S25.expspec.inputsExist', 'coverage025.required'], failures=fails)


# ------------------------------------------------------------------ 0.25 hash entries after decisions and root verification

def hash_groups_025():
    exp = (ROOT / 'coordination/review-001/m1-draft-0.25/results/hash-freeze-export-0.25.json').read_text(encoding='utf-8')
    entries = [json.loads(line.rstrip(',')) for line in exp.split('\n')[1:] if line.startswith('{')]
    e05 = rj('coordination/review-001/e05-new-three-library-0.25.json')
    rootv = {x['id']: x for x in e05['results']}
    bad, verified = [], 0
    groups = {}
    for e in entries:
        st = {'owner': list(e['ownerParameters']), 'reviewerOpen': [], 'notAdopted': [], 'pending': False}
        for p in e['reviewerParameters']:
            if p in NOT_ADOPTED:
                st['notAdopted'].append(p)
            elif not all(t in ACCEPTED for t in PARAM_TOKEN.get(p, [p])):
                st['reviewerOpen'].append(p)
        if e['verification'] == 'pendingRootTriad':
            rv = rootv.get(e['id'])
            ok = (rv is not None and rv['pass'] is True and rv['inputSha256'] == hashlib.sha256(bytes.fromhex(e['preimageHex'])).hexdigest()
                  and rv['pythonKeccak'] == e['referenceDigest'] == rv['nobleKeccak'] == rv['jsSha3Keccak'])
            if ok:
                verified += 1
            else:
                bad.append(e['id'])
                st['pending'] = True
        g = groups.setdefault(e['group'], {'blockedBy': [], 'pending': False})
        for p in st['owner']:
            if {'type': 'owner', 'id': p} not in g['blockedBy']:
                g['blockedBy'].append({'type': 'owner', 'id': p})
        for p in st['reviewerOpen']:
            if {'type': 'reviewer', 'id': p} not in g['blockedBy']:
                g['blockedBy'].append({'type': 'reviewer', 'id': p})
        for p in st['notAdopted']:
            if {'type': 'notAdopted', 'id': p} not in g['blockedBy']:
                g['blockedBy'].append({'type': 'notAdopted', 'id': p})
        g['pending'] = g['pending'] or st['pending']
    check('hash025.newPreimagesRootVerified', verified == 56 and not bad and e05['passed'] == 56 and e05['failed'] == 0, verified=verified, unverified=bad)
    for a, t in (('alias.versionRecords', 'triad.manifests'), ('alias.t1_02.manifestHash', 'triad.manifests'), ('alias.X1.netKey', 'triad.netKey')):
        groups[a] = groups.get(t, {'blockedBy': [], 'pending': True})
    for s in ('syn.lc.branchHashes', 'privateProvenance', 'ph.v3.networkProfiles'):
        groups[s] = {'blockedBy': [], 'pending': False}
    record('hash025.groupsAfterDecisions', 'recorded', groups=groups)
    return groups


# ------------------------------------------------------------------ V1

def v1_network():
    net = own('vectors/v1-network.json')
    provenance('V1.network.source', net['source'])
    provenance('V1.network.constants', net['clientConstants']['source'])
    provenance('V1.network.template', net['templateDefaults']['source'])
    provenance('V1.network.wire', net['wire']['source'])
    gp = net['network']['genesisPre']
    segs = copy.deepcopy(rj('m1-draft-0.21/vectors/v3-gsv1.json')['flatSegments'])
    ok_seg = segs[gp['segmentIndex']] == gp['replace']
    segs[gp['segmentIndex']] = gp['with']
    pre = b''.join(bytes.fromhex(s) if isinstance(s, str) else bytes.fromhex(s['repeat']) * s['count'] for s in segs)
    NP = load_module(ROOT / 'm1-draft-0.21/tools/netprofile_ref.py', 'netprofile_021_026')
    spec = NP.decode_genesis(pre)
    cp_lit = {k: num(v) for k, v in net['network']['cp'].items()}
    cp_dec = {k: spec['CP'][k] for k in cp_lit if k in spec['CP']}
    check('V1.network.genesisPre', ok_seg and len(pre) == gp['bytes'] and spec['chainId'] == net['network']['chainId'] == 777002 and cp_dec == cp_lit,
          decodedCp=cp_dec, missingKeys=[k for k in cp_lit if k not in spec['CP']])
    cfg = {'chainId': spec['chainId'], 'genesisHash': kh(pre), 'forkSchedule': [tuple(x) for x in net['network']['forkSchedule']], 'cp': dict(spec['CP'])}
    for k, v in cp_lit.items():
        cfg['cp'].setdefault(k, v)
    ceil = V.ceil_target(cfg['cp']['target_g'])
    check('V1.network.constants', ceil == num(net['clientConstants']['ceilTarget']) and V.min_work(ceil) == net['clientConstants']['minWorkRP'] == 4095)
    return net, cfg, pre


def ec_selftests():
    br = rj('m1-draft-0.5/annex/br-messages.json')['txA']
    bad = []
    pubs = {}
    for k in br['kats']:
        q = V.ec_mul(int(k['priv'], 16), V.G)
        pubs['kat%d' % k['index']] = (q, k['address'])
        if '0x' + V.address_of(q).hex() != k['address']:
            bad.append(k['index'])
    one = V.G
    pubs['privKeyOne'] = (one, br['privKeyOneAddress'])
    if '0x' + V.address_of(one).hex() != br['privKeyOneAddress']:
        bad.append('privKeyOne')
    check('V1.ec.addressesFromFixtureKeys', not bad, bad=bad)
    txa = next((r for r in results_of('R05') or [] if r.get('check') == 'txA'), None)
    ok = False
    if txa:
        raw = bytes.fromhex(txa['raw'][2:])
        items = RLP3.decode(raw[1:])
        sighash = kh(b'\x02' + RLP3.encode(items[:9]))
        y, r, s = items[9], items[10], items[11]
        sig = r.rjust(32, b'\x00') + s.rjust(32, b'\x00') + bytes([int.from_bytes(y, 'big') if y else 0])
        q = V.recover(sighash, sig)
        ok = raw[0] == 2 and q is not None and '0x' + V.address_of(q).hex() == txa['fromAddress'] and kh(raw).hex() == txa['txHash'][2:]
        if q is not None:
            pubs['txA.signer'] = (q, txa['fromAddress'])
    check('V1.ec.txaSignerRecoveredWithoutPrivateKey', ok)
    x = int(br['kats'][0]['priv'], 16)
    z = kh(b'V1 self-test')
    sig = V.sign(x, z)
    q = V.recover(z, sig)
    check('V1.ec.signRecoverRoundTrip', q is not None and V.address_of(q) == V.address_of(V.ec_mul(x, V.G)) and int.from_bytes(sig[32:64], 'big') <= V.N_ORDER // 2
          and V.sign(x, z) == sig)
    return x, pubs


def units(cfg):
    u = own('vectors/v1-units.json')
    provenance('V1.rw', u['rw']['source'])
    bad = []
    for r in u['rw']['rows']:
        p = V.plan(num(r['h']))
        got = {'b': str(p['b']), 'from': str(p['from']), 'count': p['count'], 'n': p['n'], 'anchoredGenesis': p['anchoredGenesis'],
               'params': [hex(p['from']), hex(p['count'])]}
        want = {k: r[k] for k in got}
        if got != want:
            bad.append([r['id'], got])
    check('V1.rw.rows', not bad, bad=bad)
    first = None
    for h in range(1, 10 ** 6 + 1):
        p = V.plan(h)
        if not (p['count'] <= 14 and p['n'] <= 13 and p['count'] == p['n'] + (1 if p['b'] > 1 else 0) and p['from'] + p['count'] - 1 == h):
            first = h
            break
    check('V1.rw.exhaustive', first is None, firstBad=first)
    provenance('V1.hc', u['hc']['source'])
    provenance('V1.hc.messages', u['hc']['messageSource'])
    provenance('V1.hc.noBlocks', u['hc']['noBlocksSource'])
    bad = []
    for r in u['hc']['rows']:
        tg = num(r['target_g'])
        c = V.ceil_target(tg)
        ok = c == num(r['ceilTarget'])
        if 'target' in r:
            ok = ok and ((num(r['target']) <= c) == (r['check'] == 'accepted'))
        if 'minWork' in r:
            mw = V.min_work(c)
            ok = ok and mw == num(r['minWork'])
            if 'message' in r:
                msg = V.message(13, False, mw)
                ok = ok and msg == r['message'] and all(s in msg for s in r.get('contains', [])) and not any(s in msg for s in r.get('excludes', []))
        if not ok:
            bad.append(r['id'])
    check('V1.hc.rows', not bad, bad=bad)
    check('V1.hc.countPhrases', [V.count_phrase(n) for n in (1, 2, 3, 10, 11, 13)] == ['رأس واحد', 'رأسين', '3 رؤوس', '10 رؤوس', '11 رأسًا', '13 رأسًا'])
    rnd = u['hc']['random']
    rng = random.Random(rnd['seed'])
    viol = None
    for _ in range(rnd['samples']):
        tg = rng.randrange(1, V.TOP)
        c = V.ceil_target(tg)
        mw = V.min_work(c)
        t = rng.randrange(1, c + 1)
        passes = lambda target: target <= c                                      # noqa: E731  (the viewTargetCeil test)
        if V.work(t) < mw or V.work(c) != mw or not passes(c) or (c < V.U256_MAX and passes(c + 1)):
            viol = [str(tg), str(t)]
            break
    check('V1.hc.random', viol is None, samples=rnd['samples'], firstViolation=viol)
    provenance('V1.asert', u['asert']['source'])
    bad = []
    for r in u['asert']['rows']:
        ctr = V.Counters()
        got = V.asert(cfg['cp'], r['p_ts'], r['p_h'], ctr)
        dt = (r['p_ts'] - cfg['cp']['g_ts']) - cfg['cp']['T_blk'] * r['p_h']
        if got != num(r['target']) or ('dt' in r and dt != r['dt']) or ctr.maxAsertBits > 512:
            bad.append([r['id'], str(got), ctr.maxAsertBits])
    check('V1.asert.rows', not bad, bad=bad)
    fr = []
    for f in (0, 1, 32768, 65535):
        F = 65536 + (195766423245049 * f + 971821376 * f * f + 5127 * f ** 3 + 2 ** 47) // 2 ** 48
        fr.append(F)
    check('V1.asert.Frange', all(65536 <= F <= 131071 for F in fr) and fr[0] == 65536 and fr[2] == 92674, F=fr)


def build_all(net, cfg, x):
    ch = own('vectors/v1-chains.json')
    d = net['templateDefaults']
    signer = V.address_of(V.ec_mul(x, V.G))
    check('V1.signer.address', '0x' + signer.hex() == d['signer']['address'])
    k3 = kh(b'\x80')
    kother = kh(b'other')

    def base(h, ts, parent, target):
        return {'tag': V.TAG, 'chainId': d['chainId'], 'genesisHash': cfg['genesisHash'], 'parentHash': parent, 'h': h, 'a': d['a'],
                'protocolVersion': d['protocolVersion'], 'ts': ts, 'target': target, 'stateRoot': bytes.fromhex(d['stateRoot'][2:]),
                'txRoot': k3, 'receiptsRoot': k3, 'logsBloom': bytes(256), 'gasLimit': d['gasLimit'], 'gasUsed': d['gasUsed'],
                'baseFee': d['baseFee'], 'evidenceRoot': k3, 'proposer': signer}

    def nrule(s, base_nonce=None):
        if s == 'pow' or s == 'fail':
            return (s, None)
        v = s.split(':')[1]
        return ('fixed', base_nonce if v == 'base' else int(v))

    chains = {}
    fields = {}
    H = {}
    parent = cfg['genesisHash']
    for row in ch['chains']['H']['rows']:
        f = base(row['h'], row['ts'], parent, num(ch['chains']['H']['target']))
        hd = V.build_header(f, [], x, nrule(ch['chains']['H']['nonce']))
        H[row['h']], fields['H:%d' % row['h']] = hd, f
        parent = hd['blockHash']
    chains['H'] = H
    for name in ('G', 'F'):
        c = ch['chains'][name]
        r = c['reference']
        f = base(r['h'], r['ts'], bytes.fromhex(r['parentHash'][2:]), num(r['target']))
        ref = V.build_header(f, [], x, nrule(r['nonce']))
        out = {r['h']: ref}
        fields['%s:%d' % (name, r['h'])] = f
        parent = ref['blockHash']
        for row in c['rows']:
            f = base(row['h'], row['ts'], parent, num(c['target']))
            hd = V.build_header(f, [], x, nrule(c['nonce']))
            out[row['h']], fields['%s:%d' % (name, row['h'])] = hd, f
            parent = hd['blockHash']
        chains[name] = out
    f7 = dict(fields['H:7'])
    ref = V.build_header(f7, [], x, ('fail', None))
    Hc = {7: ref}
    fields['Hc:7'] = f7
    parent = ref['blockHash']
    for h in range(8, 21):
        f = base(h, fields['H:%d' % h]['ts'], parent, num(ch['chains']['Hc']['target']))
        hd = V.build_header(f, [], x, ('pow', None))
        Hc[h], fields['Hc:%d' % h] = hd, f
        parent = hd['blockHash']
    chains['Hc'] = Hc
    variants = {}
    for vid, v in ch['variants'].items():
        bchain, bh = v['copyOf'].split(':')
        f = dict(fields[v['copyOf']])
        for k, val in v['set'].items():
            if k == 'parentHash' or k == 'genesisHash':
                f[k] = kother if val == '@KOTHER' else bytes.fromhex(val[2:])
            else:
                f[k] = val
        variants[vid] = V.build_header(f, [], x, nrule(v['nonce'], chains[bchain][int(bh)]['nonce']))
        fields[vid] = f
    ok_pow = all(int.from_bytes(hd['powHash'], 'big') <= fields['H:%d' % h]['target'] for h, hd in H.items())
    ok_fail = int.from_bytes(Hc[7]['powHash'], 'big') > fields['Hc:7']['target']
    check('V1.build.chains', ok_pow and ok_fail and len(H) == 20 and len(chains['G']) == 14 and len(chains['F']) == 14 and len(Hc) == 14 and len(variants) == 6,
          nonces={k: [hd['nonce'] for _, hd in sorted(v.items())] for k, v in chains.items()})
    check('V1.build.asertConsistent', all(V.asert(cfg['cp'], (fields['H:%d' % (h - 1)]['ts'] if h > 1 else cfg['cp']['g_ts']), h - 1) == fields['H:%d' % h]['target']
                                          for h in range(1, 21))
          and all(V.asert(cfg['cp'], fields['G:%d' % (h - 1)]['ts'], h - 1) == fields['G:%d' % h]['target'] for h in range(8, 21))
          and all(V.asert(cfg['cp'], fields['F:%d' % (h - 1)]['ts'], h - 1) == fields['F:%d' % h]['target'] for h in range(8, 21)))
    return chains, variants, fields, kother


def resolve_reply(tokens, chains, variants):
    out = []
    for t in tokens:
        if t in variants:
            out.append(variants[t])
            continue
        name, rng = t.split(':')
        if '..' in rng:
            a, b = (int(x) for x in rng.split('..'))
            out += [chains[name][h] for h in range(a, b + 1)]
        else:
            out.append(chains[name][int(rng)])
    return out


def v1_cases(cfg, chains, variants, fields):
    doc = own('vectors/v1-window-cases.json')
    for k, src in doc['sources'].items():
        provenance('V1.cases.' + k, src)
    transcripts, bounds = [], []
    for case in doc['cases']:
        cid = case['id']
        calls, outcomes, ctr = [], [], None
        for i, ph in enumerate(case['phases']):
            replies = [{'jsonrpc': '2.0', 'id': 1, 'result': ph['blockNumber']}]
            if ph.get('reply'):
                hdrs = resolve_reply(ph['reply'], chains, variants)
                replies.append({'jsonrpc': '2.0', 'id': 2, 'result': '0x' + RLP3.encode([hd['item'] for hd in hdrs]).hex()})
            for err in ph.get('replyError', []):
                replies.append({'jsonrpc': '2.0', 'id': 2, 'error': err})
            rpc = V.ScriptedRpc(replies, ph.get('t', 0))
            ctr = V.Counters()
            out = V.check_window(cfg, rpc, ph['clock'], ctr)
            calls += rpc.calls
            outcomes.append(out)
            transcripts.append({'case': cid, 'phase': i, 'clock': ph['clock'], 'requests': rpc.calls, 'replies': replies, 'outcome': out,
                                'counters': ctr.as_dict(), 'events': ctr.events})
        final = outcomes[-1]
        cells = {}
        cells['requests'] = [[c['method'], c['params']] for c in calls] == case['requests']
        if 'requestTimes' in case:
            cells['requestTimes'] = [c['t'] for c in calls] == case['requestTimes']
        cells['plan'] = V.plan(int(case['phases'][-1]['blockNumber'], 16)) == case['plan']
        cells['outcome'] = all(final.get(k) == v for k, v in case['expect'].items())
        if 'expectPhase1' in case:
            cells['phase1'] = all(outcomes[0].get(k) == v for k, v in case['expectPhase1'].items())
        cd = ctr.as_dict()
        cells['counters'] = all(cd[k] == v for k, v in case['counters'].items())
        if not final['ok'] and final.get('at') is not None and ctr.events:
            cells['earlyExit'] = ctr.events[-1][1] == final['at']
        if 'mustNotHaveEvent' in case:
            cells['refNotPowChecked'] = case['mustNotHaveEvent'] not in ctr.events
        if case.get('referencePowHashAboveTarget'):
            ref = chains['Hc'][7]
            cells['refPowAboveTarget'] = int.from_bytes(ref['powHash'], 'big') > fields['Hc:7']['target']
        if case.get('reference'):
            hdr = resolve_reply(case['phases'][-1]['reply'], chains, variants)[0]
            pr = V.parse_header(RLP3.decode(hdr['encoded']))
            want = dict(case['reference'])
            want['genesisHash'] = cfg['genesisHash']
            cells['referenceIdentity'] = all(pr[k] == v for k, v in want.items())
        bad = {k: v for k, v in cells.items() if not v}
        check('V1.case.' + cid, not bad, failingCells=sorted(bad), outcome={k: final.get(k) for k in ('ok', 'rule', 'at', 'n', 'minWork', 'message')},
              counters=cd)
        bounds.append(cd)
    check('V1.costBounds', all(c['headerDecodes'] <= 14 and c['asert'] <= 13 and c['powHash'] <= 13 and c['templateIds'] <= 14 and c['maxAsertBits'] <= 512
                               for c in bounds), maxima={k: max(c[k] for c in bounds) for k in bounds[0]})
    return transcripts


def v1_experiments():
    doc = own('supplements/s26-v1-experiments.json')
    for e in doc['experiments']:
        provenance('V1.experiment.' + e['id'], e['source'])
    check('V1.experiments.defined', [e['id'] for e in doc['experiments']] == ['S26-NXV3E-PING', 'S26-NX-D', 'S26-NXV4-LIVE'] and all(e['pass'] for e in doc['experiments']))


def v1_hash_entries(cfg, pre, chains, variants, fields, kother, pubs):
    out = []

    def add(eid, group, alg, preimage, digest, form='full'):
        out.append({'id': eid, 'group': group, 'algorithm': alg, 'preimageHex': preimage.hex(), 'digest': digest.hex(), 'assertedForm': form,
                    'verification': 'pendingRootVerification'})

    add('v1.genesis:V1NET', 'v1.genesis', 'keccak256', pre, cfg['genesisHash'])
    add('v1.other', 'v1.other', 'keccak256', b'other', kother)
    add('v1.shareRoot', 'v1.shareRoot', 'keccak256', V.RLP.encode([]), kh(V.RLP.encode([])))
    named = {}
    for cname, hs in chains.items():
        for h, hd in hs.items():
            named['%s:%d' % (cname, h)] = hd
    named.update(variants)
    for name, hd in sorted(named.items()):
        add('v1.templateId:' + name, 'v1.templateId', 'sha256', hd['utRaw'], hd['tid'])
        add('v1.powHash:' + name, 'v1.powHash', 'sha256', hd['tid'] + V.be64(hd['nonce']), hd['powHash'])
        add('v1.blockHash:' + name, 'v1.blockHash', 'keccak256', hd['tid'] + V.be64(hd['nonce']) + hd['shareRoot'], hd['blockHash'])
        add('v1.sigMsg:' + name, 'v1.sigMsg', 'keccak256', b'\x19' + b'PoCol template' + b'\x0a' + hd['tid'], hd['sigMsg'])
        add('v1.winMsg:' + name, 'v1.winMsg', 'keccak256', b'\x19' + b'PoCol winner' + b'\x0a' + hd['tid'] + V.be64(hd['nonce']) + hd['shareRoot'], hd['winMsg'])
    for name, (q, addr) in sorted(pubs.items()):
        add('keys.public:' + name, 'keys.public', 'keccak256', V.pub_bytes(q), kh(V.pub_bytes(q)), 'last20')
        out[-1]['expectedLast20'] = addr[2:].lower()
    bad = [e['id'] for e in out if e['assertedForm'] == 'last20' and e['digest'][-40:] != e['expectedLast20']]
    bad += [e['id'] for e in out if e['algorithm'] == 'sha256' and hashlib.sha256(bytes.fromhex(e['preimageHex'])).hexdigest() != e['digest']]
    check('V1.hash.entriesSelfConsistent', not bad and len(out) > 300, entries=len(out), bad=bad[:5])
    out.sort(key=lambda e: e['id'])
    return out


# ------------------------------------------------------------------ N26 normative fixtures

def n26():
    doc = own('vectors/n26-representation-cases.json')
    S25 = load_module(ROOT / 'm1-draft-0.25/tools/supplements_ref.py', 'supplements_ref_025_026')
    C = load_module(ROOT / 'm1-draft-0.13/tools/fmt2_codec.py', 'fmt2_codec_026')
    for c in doc['n26_1_valueTexts']:
        v = S25.js_like_loads(c['text'])
        try:
            d, tomb = S25.decode_value_hex(C, (int(c['ver'][0]), c['ver'][1]), v)
            got = {'ok': True, 'dict': d, 'tomb': tomb}
        except S25.ReprError as e:
            got = {'ok': False, 'reason': e.reason}
        check('N26.value.' + c['id'], got == c['expect'], got=got)
    B = load_module(ROOT / 'm1-draft-0.4/tools/bridge_ref.py', 'bridge_ref_04_026')
    sur = rj('m1-draft-0.25/representation/cr-e4-02-surrogates.json')
    for c in sur['bridgeCases']['cases']:
        r = B.check(c['raw'], 1000, B.Bucket(1000))
        check('N26.bridge.' + c['id'], all(r.get(k) == v for k, v in c['expected'].items()), got=[r.get('stage'), r.get('data')])
    cid = rj('m1-draft-0.25/representation/cid-1-profile-chainid.json')
    for f in cid['fixtures']:
        got = S25.chainid_exact(f['text'])
        w = f['exact']
        check('N26.chainId.' + f['id'], got['ok'] == w['ok'] and ('value' not in w or str(got.get('value')) == w['value'])
              and ('reason' not in w or got.get('reason') == w['reason']), got=got)


# ------------------------------------------------------------------ consolidation

def consolidate(groups):
    have = {r['check']: r['status'] for r in results}
    fx_status = {}
    supp_pattern = {'S25-SWEEPMAX': '^S25\\.sweepMax\\.', 'S25-E07': '^S25\\.e07\\.', 'S25-LCLIT': '^S25\\.lclit\\.', 'S25-RG3B': '^S25\\.rg3b\\.'}
    changes = {}
    for k, v in CONS['rowChanges'].items():
        changes.setdefault(v.get('row', k), []).append(v)
    computed = {}
    for row in INV25['rows']:
        rid = row['id']
        ch = changes.get(rid, [])
        owner = list(row['owner']) + [t for c in ch for t in c.get('addOwner', [])]
        reviewer = [t for t in row['reviewer'] if not any(t in c.get('removeReviewer', []) for c in ch)] + [t for c in ch for t in c.get('addReviewer', [])]
        spec_missing = [s['file'] for s in row['spec'] if not (ROOT / s['file']).exists()]
        anchors = [s['file'] for s in row['spec'] if s.get('contains') and (ROOT / s['file']).exists() and not contains(s['file'], s['contains'])]
        files_missing = [f for f in row['fixtures'] + row['review'] if not (ROOT / f).exists()]
        new_files = [f for c in ch for f in c.get('addSpec', []) + c.get('addFixtures', [])]
        files_missing += [f for f in new_files if not (PKG / f.replace('m1-draft-0.26/', '')).exists()]
        ev = [evidence('row.%s.evidence.%s:%s' % (rid, k, p), k, p) for k, p in row['evidence']]
        ev += [evidence('row.%s.evidence.%s:%s' % (rid, k, p), k, p) for c in ch for k, p in c.get('addEvidence', [])]
        cur = [p for c in ch for p in c.get('currentRunEvidence', [])]
        cur_ok = True
        for p in cur:
            mine = [s for k, s in have.items() if re.search(p, k)]
            cur_ok = cur_ok and bool(mine) and all(s != 'FAIL' for s in mine)
            check('row.%s.currentRun:%s' % (rid, p), bool(mine) and all(s != 'FAIL' for s in mine), checks=len(mine))
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
        hash_block, hash_pending = [], False
        for g in row['hashGroups']:
            s = groups.get(g)
            if s is None:
                continue
            hash_block += [dict(b, group=g) for b in s['blockedBy'] if b['type'] in ('owner', 'reviewer')]
            hash_pending = hash_pending or s['pending']
        c1 = 'blocked' if (spec_missing or anchors or files_missing or unresolved) else ('pendingRootReview' if pending else 'satisfied')
        c2 = 'blocked' if not (all(ev) and cur_ok) else ('pendingRootReview' if pending else 'satisfied')
        c3 = 'pendingRootReview' if pending else 'satisfied'
        c4 = 'blocked' if own_open else ('pendingReviewerDecision' if rev_open else 'satisfied')
        c5 = 'blocked' if (own_open or unresolved or any(b['type'] == 'owner' for b in hash_block)) else \
            ('pendingRootReview' if (pending or hash_pending or hash_block) else 'satisfied')
        crit = {'c1': c1, 'c2': c2, 'c3': c3, 'c4': c4, 'c5': c5}
        status = 'CompleteCandidate' if all(v == 'satisfied' for v in crit.values()) else 'Partial'
        computed[rid] = status
        record('row.%s.criteria' % rid, 'recorded', criteria=crit, status=status, ownerOpen=own_open, reviewerOpen=rev_open, unresolved=unresolved,
               hashBlockers=hash_block, missingFiles=spec_missing + files_missing, anchors=anchors)
    prop = CONS['proposedStatus']
    want = {r: 'CompleteCandidate' for r in prop['CompleteCandidate']}
    want.update({r: 'Partial' for r in prop['Partial']})
    check('consolidation.rowsMatchProposal', computed == want, differing={r: [computed.get(r), want.get(r)] for r in set(computed) | set(want) if computed.get(r) != want.get(r)})
    record('consolidation.summary', 'recorded', counts=dict(Counter(computed.values())), rows=computed)
    ft = own('audit/findings-trace-0.26.json')
    bad = []
    for f in ft['findings']:
        owner_dep = [t for t in f['dependsOn'] if t in OWNER_OPEN or t in ('U02', 'U01', 'U10', 'U14')]
        open_rev = [t for t in f['dependsOn'] if t not in OWNER_OPEN and t not in ACCEPTED]
        want_d = 'openOwner' if owner_dep else ('closableAtSpecScope' if not open_rev else 'pendingReviewer')
        if f['disposition'] != want_d:
            bad.append([f['id'], f['disposition'], want_d, open_rev])
    check('findings.dispositionsFromDecisions', len(ft['findings']) == 26 and not bad, bad=bad,
          counts=dict(Counter(f['disposition'] for f in ft['findings'])))
    return computed


# ------------------------------------------------------------------ boundary, output, main

def boundary_static(texts):
    bad = {}
    for f in (HERE / 'v1_ref.py', HERE / 'run_checks_026.py'):
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
    deps = [str(f.relative_to(PKG)) for f in sorted(PKG.rglob('*'))
            if f.suffix.lower() in ('.exe', '.dll', '.node', '.so', '.pyd', '.whl', '.zip', '.lock', '.wasm') or 'node_modules' in f.parts]
    check('boundary.noDependencyOrBinary', not deps, files=deps)
    keys = [v['priv'][2:].lower() for v in rj('coordination/review-001/m1-draft-0.5/results/txa-node-check.json')['keys'].values()]
    leaked = [name for name, t in texts.items() if any(k in t.lower() for k in keys)]
    check('boundary.noPrivateKeyInOutputs', not leaked, files=leaked, keysChecked=len(keys))


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
    back = p.read_bytes()                                       # validated from the output directory itself
    check(label + '.writtenAndReadBack', back == data, file=str(name), sha256=hashlib.sha256(data).hexdigest(), bytes=len(data), outDir='<out>')


def coverage():
    have = {r['check']: r['status'] for r in results}
    cases = [c['id'] for c in own('vectors/v1-window-cases.json')['cases']]
    need = ['V1.case.' + c for c in cases] + ['V1.rw.rows', 'V1.rw.exhaustive', 'V1.hc.rows', 'V1.hc.random', 'V1.hc.countPhrases', 'V1.asert.rows',
                                              'V1.asert.Frange', 'V1.costBounds', 'V1.network.genesisPre', 'V1.network.constants', 'V1.build.chains',
                                              'V1.build.asertConsistent', 'V1.ec.addressesFromFixtureKeys', 'V1.ec.txaSignerRecoveredWithoutPrivateKey',
                                              'V1.ec.signRecoverRoundTrip', 'V1.hash.entriesSelfConsistent', 'V1.experiments.defined',
                                              'V1.transcripts.writtenAndReadBack', 'V1.hashExport.writtenAndReadBack', 'hash025.newPreimagesRootVerified',
                                              'consolidation.rowsMatchProposal', 'findings.dispositionsFromDecisions', 'decision.U08isOwnerChangeRequest',
                                              'decision.noOwnerItemAccepted', 'decision.ledger.ownerItemsOpen', 'evidence.run025.canonical',
                                              'evidence.run025.retainedFailureUnchanged', 'boundary.noPrivateKeyInOutputs']
    need += ['N26.value.' + c['id'] for c in own('vectors/n26-representation-cases.json')['n26_1_valueTexts']]
    missing = [x for x in need if have.get(x) != 'pass']
    check('coverage026.required', not missing, missing=missing, required=len(need))
    check('coverage026.noStepAborted', not [k for k in have if 'step.completed' in k])


def out_path():
    p = OUT / 'run-results-0.26.json'
    k = 1
    while p.exists():
        p = OUT / ('run-results-0.26-rerun-%d.json' % k)
        k += 1
    return p


def main():
    t0 = time.time()
    before = {rel: sha(ROOT / rel) for rel in PRESERVED}
    for rel in INPUTS_OWN:
        record('input.sha256 <package>/' + rel, 'recorded', exists=(PKG / rel).exists(), sha256=sha(PKG / rel))
    for rel in INPUTS_ROOT:
        record('input.sha256 ' + rel, 'recorded', exists=(ROOT / rel).exists(), sha256=sha(ROOT / rel))
    record('environment.paths', 'recorded', rootIsAncestorOfRunner=str(HERE).startswith(str(ROOT)), outUnderPackage=str(OUT).startswith(str(PKG)))
    check('boundary.positionalOnly', record.__code__.co_posonlyargcount == 2 and check.__code__.co_posonlyargcount == 2 and gap.__code__.co_posonlyargcount == 1)
    probe = {'x': [1]}
    record('boundary.probe', 'recorded', name='n', status='s', obj=probe)
    probe['x'].append(2)
    e = results[-1]
    check('boundary.renamedAndCopied', e['diagName'] == 'n' and e['proposalStatus'] == 's' and e['status'] == 'recorded' and e['obj'] == {'x': [1]})
    calls = [n.lineno for n in ast.walk(ast.parse(Path(__file__).read_text(encoding='utf-8')))
             if isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute) and n.func.attr == 'main']
    check('boundary.noOldMainCalled', not calls, attributeMainCallsAtLines=calls)
    texts, groups = {}, {}
    for name, fn in (('superseded', superseded_failures), ('reviewCopies', review_copies), ('decisions', decisions), ('binding025', binding_025)):
        try:
            fn()
        except Exception as ex:                                              # record, never hide
            check('step.completed ' + name, False, exception='%s: %s' % (type(ex).__name__, ex))
    try:
        groups = hash_groups_025()
    except Exception as ex:
        check('step.completed hash025', False, exception='%s: %s' % (type(ex).__name__, ex))
    try:
        net, cfg, pre = v1_network()
        x, pubs = ec_selftests()
        units(cfg)
        chains, variants, fields, kother = build_all(net, cfg, x)
        tr = v1_cases(cfg, chains, variants, fields)
        v1_experiments()
        texts['v1-transcripts-0.26.json'] = json.dumps({'schema': 'pocol-m1-v1-transcripts/0.26', 'network': 'V1NET', 'genesisHash': '0x' + cfg['genesisHash'].hex(),
                                                        'transcripts': tr}, sort_keys=True, separators=(',', ':'), ensure_ascii=True, default=lambda b: '0x' + b.hex()) + '\n'
        hx = v1_hash_entries(cfg, pre, chains, variants, fields, kother, pubs)
        texts['hash-freeze-v1-0.26.json'] = json.dumps({'schema': 'pocol-m1-hash-freeze-v1/0.26', 'plan': 'hash/hash-freeze-plan-0.26.json',
                                                        'note': 'not frozen; root verifies keccak256 entries with three libraries and sha256 entries independently',
                                                        'counts': dict(Counter(e['group'] for e in hx))}, sort_keys=True, separators=(',', ':'))[:-1] \
            + ',"list":[\n' + ',\n'.join(json.dumps(e, sort_keys=True, separators=(',', ':')) for e in hx) + '\n]}\n'
    except Exception as ex:
        check('step.completed v1', False, exception='%s: %s' % (type(ex).__name__, ex))
    try:
        boundary_static(texts)
        if 'v1-transcripts-0.26.json' in texts:
            write_asset('v1-transcripts-0.26.json', texts['v1-transcripts-0.26.json'], 'V1.transcripts')
            write_asset('hash-freeze-v1-0.26.json', texts['hash-freeze-v1-0.26.json'], 'V1.hashExport')
    except Exception as ex:
        check('step.completed outputs', False, exception='%s: %s' % (type(ex).__name__, ex))
    try:
        n26()
    except Exception as ex:
        check('step.completed n26', False, exception='%s: %s' % (type(ex).__name__, ex))
    computed = {}
    try:
        computed = consolidate(groups)
    except Exception as ex:
        check('step.completed consolidation', False, exception='%s: %s' % (type(ex).__name__, ex))
    gap('gap.ownerDecisions', text='U01, U02, U10, U14, CR-M1-01, RF-E6-1 and the U08 change request have no owner answer in the ledger')
    gap('gap.reviewerDecisionsV1', text='P-V1-1..10 await a root decision; V1 material awaits root review')
    gap('gap.pendingRootVerificationV1', text='hash-freeze-v1-0.26.json entries need independent verification (keccak256 with three libraries; sha256)')
    coverage()
    after = {rel: sha(ROOT / rel) for rel in PRESERVED}
    check('preserved.olderFilesUnchanged', before == after, before=before, after=after)
    failed = [r for r in results if r['status'] == 'FAIL']
    out = {'package': 'M1 draft 0.26 (V1 HeaderNetCheck vectors; consolidated criteria under recorded decisions; N26; hash extension)',
           'executedAtUtc': datetime.now(timezone.utc).isoformat(timespec='seconds'),
           'environment': {'python': sys.version.split()[0], 'platform': platform.platform(), 'thirdPartyPackages': 'none'},
           'statusSchema': {'coreStatus': list(CORE_STATUS), 'diagnosticRenames': RENAMED, 'partialGap': 'recorded entries with partialGap=true'},
           'evidenceKind': 'reference models of the specification; saved reviewed evidence re-bound; no browser, network, EVM or chain',
           'notExecuted': ['S26-NXV3E-PING, S26-NX-D, S26-NXV4-LIVE (Phase A)', 'E01-E04, E06, E07 (Phase A)', 'any owner decision'],
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
