"""Reference-only acceptance runner for M1 draft 0.30 (author turn 028): C33 guarded HeaderNetCheck receive boundary,
C34 per-case U08 sum invariants, the 17 delegated conventions, the hash freeze and the recomputed 41-row gate.
Python standard library only; no network, browser, EVM, chain, real clock or subprocess. NOT executed by the author.

Usage: run_checks_030.py [--root <tree>] [--out <dir>]   (or POCOL_M1_ROOT / POCOL_M1_OUT)
Writes only under out, never overwriting an earlier run (a differing later run gets a -rerun-<n> name):
  run-results-0.30.json, acceptance-matrix-0.30.json, decision-register-0.30.json, hash-freeze-0.30.json,
  bindings-0.30.json, dashboard-0.30-ar.json.
Reuses unchanged earlier tools by path (0.27 checker, 0.29 overlay models, 0.25 viewer model, 0.4 strict parser, 0.2 validator).
Rebuilds no header, nonce or share. Expect a few minutes (pure-Python Keccak over the freeze and signature recovery).
Exits 1 on any FAIL.
"""

import ast
import copy
import hashlib
import importlib.util
import json
import os
import platform
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
OWN = ['tools/run_checks_030.py', 'tools/guard_ref_030.py', 'decisions/conventions-applied-0.30.json', 'audit/acceptance-inventory-0.30.json',
       'vectors/c33-guard-cases.json', 'vectors/c34-u08-invariants.json', 'vectors/experiment-amendments-0.30.json',
       'M1-SPEC-0.30-AMENDMENT.md', 'M1-STATUS-0.30.md', 'M1-DASHBOARD-0.30-AR.md', 'README.md']
INPUTS_ROOT = ['coordination/task-028.md', 'coordination/DELEGATED-M1-CONVENTIONS-030.json', 'coordination/DELEGATED-M1-DECISIONS-029.json',
               'coordination/issue-ledger.json', 'coordination/review-001/REVIEW-0.29.md', 'coordination/review-001/m1-draft-0.29/results/run-results-0.29.json',
               'coordination/review-001/m1-draft-0.29/results/acceptance-matrix-0.29.json', 'coordination/review-001/v1-guard-integration-probes-029.json',
               'coordination/review-001/v1-envelope-scope-probes-029.json', 'coordination/review-001/deadline-independent-029.json',
               'coordination/review-001/full-hash-triad-029.json', 'coordination/review-001/full-hash-triad-executed-029.json',
               'coordination/review-001/full-hash-audit-029.json', 'coordination/review-001/m1-draft-0.25/results/hash-freeze-export-0.25.json',
               'm1-draft-0.29/tools/overlay_ref_029.py', 'm1-draft-0.29/vectors/u08-canonical-split-0.29.json', 'm1-draft-0.29/vectors/deadline-cases-0.29.json',
               'm1-draft-0.27/tools/v1_ref_027.py', 'm1-draft-0.4/tools/bridge_ref.py', 'reference/browser.md']
PRESERVED = INPUTS_ROOT[1:] + ['coordination/review-001/m1-draft-0.27/results/v1-transcripts-0.27.json', 'm1-draft-0.25/audit/row-inventory.json']

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


def path_of(rel):
    return PKG / rel[len('m1-draft-0.30/'):] if rel.startswith('m1-draft-0.30/') else ROOT / rel


def load_module(path, modname):
    spec = importlib.util.spec_from_file_location(modname, str(path))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


_lines = {}


def provenance(label, src):
    p = path_of(src['file'])
    if p not in _lines:
        _lines[p] = p.read_text(encoding='utf-8').splitlines() if p.exists() else []
    a, b = src['lines']
    seg = '\n'.join(_lines[p][a - 1:b])
    missing = [x for x in src.get('literals', []) if x not in seg]
    return check(label + '.provenance', p.exists() and bool(seg) and not missing, file=src['file'], lines=[a, b], missingLiterals=missing)


def norm(x):
    return json.loads(json.dumps(x, sort_keys=True, default=str))


def num(v):
    if type(v) is int:
        return v
    s = str(v).replace(' ', '')
    m = re.fullmatch(r'(?:(\d+)\*)?2\^(\d+)([+-]\d+)?', s)
    if m:
        return int(m.group(1) or 1) * 2 ** int(m.group(2)) + int(m.group(3) or 0)
    return int(s)


# ------------------------------------------------------------------ modules (unchanged earlier tools, loaded by path)

KEC = load_module(ROOT / 'm1-draft-0.2/tools/keccak.py', 'keccak_030')
RLP3 = load_module(ROOT / 'm1-draft-0.3/tools/rlp_strict.py', 'rlp_strict_03_030')
V = load_module(ROOT / 'm1-draft-0.27/tools/v1_ref_027.py', 'v1_ref_027_030')
V.bind(KEC, RLP3)
S = load_module(ROOT / 'm1-draft-0.25/tools/supplements_ref.py', 'supplements_ref_025_030')
O = load_module(ROOT / 'm1-draft-0.29/tools/overlay_ref_029.py', 'overlay_ref_029_030')
B04 = load_module(ROOT / 'm1-draft-0.4/tools/bridge_ref.py', 'bridge_ref_04_030')
sys.path.insert(0, str(HERE))
import guard_ref_030 as G             # noqa: E402  (this package only)
sys.path.insert(0, str(ROOT / 'm1-draft-0.2/tools'))                  # m1model imports its siblings keccak and rlp_strict
M = load_module(ROOT / 'm1-draft-0.2/tools/m1model.py', 'm1model_02_030')

CONV = rj('coordination/DELEGATED-M1-CONVENTIONS-030.json')
DEL29 = rj('coordination/DELEGATED-M1-DECISIONS-029.json')
APPL = own('decisions/conventions-applied-0.30.json')
INV = own('audit/acceptance-inventory-0.30.json')
INV25 = rj('m1-draft-0.25/audit/row-inventory.json')
DEC25 = rj('coordination/review-001/REVIEWER-DECISIONS-0.25.json')
M29 = rj(INV['base']['matrix0_29'])
RES = {k: json.loads((ROOT / v).read_text(encoding='utf-8'))['results'] for k, v in INV['results'].items()}
PARAM_TOKEN = {'P-ABI': ['P-0.2', 'U22'], 'P13': ['P-0.2', 'U06'], 'P-X1': ['P-X1'], 'TK-1': ['TK-1']}
ACCEPTED = {d['id'] for d in DEC25['decisions'] if d['status'] == 'accepted-technical-qualified'} | {'P-C32-1', 'P-C32-3', 'P-C32-4'}
NINE = [d['id'] for d in DEL29['decisions']]
APPLIED = set()           # delegated decisions proven applied (0.29 root run, or this run for U08)
DECIDED = set()           # conventions whose selection is copied exactly and whose evidence passes


def saved_status(key, pattern):
    rx = re.compile(pattern)
    hit = [r for r in RES[key] if type(r.get('check')) is str and rx.search(r['check'])]
    return hit


def ev_ok(key, pattern):
    if key == 'run':
        rx = re.compile(pattern)
        hit = [r for r in results if rx.search(r['check'])]
    else:
        hit = saved_status(key, pattern)
    return bool(hit) and any(r.get('status') == 'pass' for r in hit) and not any(r.get('status') == 'FAIL' for r in hit), len(hit)


# ------------------------------------------------------------------ boundary

def boundary():
    check('boundary.positionalOnly', record.__code__.co_posonlyargcount == 2 and check.__code__.co_posonlyargcount == 2 and gap.__code__.co_posonlyargcount == 1)
    probe = {'x': [1]}
    record('boundary.probe', 'recorded', name='n', status='s', obj=probe)
    probe['x'].append(2)
    e = results[-1]
    check('boundary.renamedAndCopied', e['diagName'] == 'n' and e['proposalStatus'] == 's' and e['obj'] == {'x': [1]})
    bad, calls = {}, []
    for f in (HERE / 'run_checks_030.py', HERE / 'guard_ref_030.py'):
        tree = ast.parse(f.read_text(encoding='utf-8'))
        mods = {a.name.split('.')[0] for n in ast.walk(tree) if isinstance(n, ast.Import) for a in n.names}
        mods |= {(n.module or '').split('.')[0] for n in ast.walk(tree) if isinstance(n, ast.ImportFrom)}
        hit = sorted(mods & {'socket', 'urllib', 'http', 'requests', 'ssl', 'asyncio', 'subprocess', 'threading'})
        if hit:
            bad[f.name] = hit
        if f.name == 'guard_ref_030.py' and mods & {'time', 'datetime', 'random'}:
            bad[f.name + ':clock'] = sorted(mods & {'time', 'datetime', 'random'})
        calls += [n.lineno for n in ast.walk(tree) if isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute) and n.func.attr == 'main']
    check('boundary.noNetworkProcessOrClock', not bad, imports=bad)
    check('boundary.noOldMainCalled', not calls, lines=calls)


# ------------------------------------------------------------------ authority and the 0.29 binding

def authority():
    led = rj('coordination/issue-ledger.json')
    sa = led.get('standingAuthorization', {})
    check('authority.standingAuthorization', sa.get('delegatedTechnicalDecisions') is True and sa.get('personalApproval') is False and sa.get('noProduction') is True,
          standingAuthorization=sa)
    st = {i['id']: i.get('status') for i in led.get('issues', [])}
    record('authority.ledgerPendingFinalVerification', 'recorded', statuses={k: st.get(k) for k in INV['ledgerPendingFinalVerification']},
           note='pending final verification by root; not unanswered-owner blockers')
    want = {d['id']: d for d in CONV['decisions']}
    got = {d['id']: d for d in APPL['conventions']}
    check('authority.conventionsSet', sorted(want) == sorted(got) and len(got) == 17, missing=sorted(set(want) - set(got)), extra=sorted(set(got) - set(want)))
    for cid, d in want.items():
        g = got.get(cid, {})
        check('authority.convention.' + cid, g.get('selection') == d['selection'] and g.get('status') == d['status'] == 'delegated-technical-decision')
        if 'source' in g:
            provenance('authority.conventionSource.' + cid, g['source'])
    provenance('authority.outsideCovered', APPL['outsideCoveredOperations']['source'])
    check('authority.noOwnerAnswerWritten', 'personalApproval' not in json.dumps(APPL['conventions'])
          and CONV['authority'].startswith('User standing delegation') and 'not personalownerapproval' in CONV['authority'])


def bind029():
    r29 = rj(INV['results']['R29'])
    s = r29['summary']
    w = INV['r29Summary']
    fails = sorted(r['check'] for r in r29['results'] if r['status'] == 'FAIL')
    check('bind029.summaryAndFailures', [s['checks'], s['passed'], s['recorded'], s['failed']] == [w['checks'], w['passed'], w['recorded'], w['failed']]
          and fails == sorted(w['failures']), summary=s, failures=fails)
    cp, mism, compared = ROOT / 'coordination/review-001/m1-draft-0.29', [], 0
    for f in sorted(cp.rglob('*')) if cp.exists() else []:
        rel = f.relative_to(cp)
        if not f.is_file() or rel.parts[0] == 'results' or '__pycache__' in rel.parts or not (ROOT / 'm1-draft-0.29' / rel).exists():
            continue
        compared += 1
        if f.read_bytes() != (ROOT / 'm1-draft-0.29' / rel).read_bytes():
            mism.append(rel.as_posix())
    check('bind029.reviewCopyByteIdentical', compared > 0 and not mism, compared=compared, mismatches=mism)
    rv = (ROOT / INV['base']['review0_29']).read_text(encoding='utf-8')
    check('bind029.reviewAcceptsUnaffected', INV['reviewedBy029']['literal'] in rv)
    for d in NINE:
        if d == 'U08':
            continue
        ok, n = ev_ok('R29', '^decision\\.%s\\.applied$' % re.escape(d))
        if check('bind029.decisionApplied.' + d, ok and d in INV['reviewedBy029']['delegatedReviewed'], entries=n):
            APPLIED.add(d)
    for h in INV['historical']['boundIn029']:
        ok, n = ev_ok('R29', '^history\\.%s$' % re.escape(h))
        check('bind029.history.' + h, ok, entries=n)
    roots = [('deadline-independent-029', {'checks': 1120, 'passed': 1120, 'failed': 0}),
             ('full-hash-triad-executed-029', {'checks': 668, 'passed': 668, 'failed': 0}),
             ('full-hash-audit-029', {'checks': 668, 'passed': 668, 'failed': 0, 'expectedAssertionFailures': 0}),
             ('v1-hash-independent-0.27', {'checks': 3677, 'passed': 3677, 'failed': 0}),
             ('v1-signatures-independent-0.27', {'checks': 31, 'failed': 0, 'noSigningOrSubmission': True}),
             ('asert-native-comparison-0.27', {'checks': 1027, 'passed': 1027, 'failed': 0}),
             ('v1-independent-hashes-0.26', {'checks': 347, 'passed': 347, 'failed': 0})]
    for name, want in roots:
        rel = 'coordination/review-001/%s.json' % name
        d = rj(rel)
        check('bind029.rootEvidence.' + name, all(d.get(k) == v for k, v in want.items()), sha256=sha(ROOT / rel), summary={k: d.get(k) for k in want})
    au = rj('coordination/review-001/full-hash-audit-029.json')
    t0 = rj('coordination/review-001/full-hash-triad-029.json')
    check('bind029.harnessFailurePreservedAndExplained', t0.get('failed') == au['initialHarnessFailure']['failed'] == 611
          and au['correctedEvidence'] == 'full-hash-triad-executed-029.json' and 'null' in au['initialHarnessFailure']['cause'], initialFailed=t0.get('failed'))
    gp = rj('coordination/review-001/v1-guard-integration-probes-029.json')
    check('bind029.guardProbesPreservedAsFailing', gp['checks'] == 3 and gp['failed'] == 3 and sorted(r['case'] for r in gp['results']) == ['duplicateResult', 'missingId', 'wrongId']
          and all(r['actual']['verdict']['frame'] is True for r in gp['results']))


# ------------------------------------------------------------------ C34: U08 with per-case invariants

def _lens(c):
    return c['lens'] if isinstance(c['lens'], list) else [c['lens']['repeat'][0]] * c['lens']['repeat'][1]


def eval_negative(c, inv, viewer=True):
    """One evaluator for real fixtures and mutants: contract error, viewer rule, and the case's own sum invariant."""
    lens = _lens(c)
    s = sum(lens)
    cells = {'invariantPresent': all(k in inv for k in ('expectedSum', 'delta', 'sumMatches')),
             'sum': s == inv.get('expectedSum'),
             'delta': inv.get('delta') == inv.get('expectedSum', 0) - c['len'],
             'sumMatches': inv.get('sumMatches') == (inv.get('expectedSum') == c['len'])}
    if 'contract' in c:
        cells['contract'] = O.contract_args(c['len'], c.get('nChunks', len(lens)), lens) == c['contract']
    if viewer and 'nChunks' not in c:
        r = M.validate_version({'manifestHash': b'\x00' * 32, 'manifestLen': c['len'], 'chunks': [(bytes(20), x) for x in lens]}, {})
        want = c.get('viewer', 'manifest.split')
        cells['viewer'] = r.get('rule') == want and r.get('stage') == 'manifest' and r.get('fetchCalls') == 0
    return cells


def c34():
    doc = own('vectors/c34-u08-invariants.json')
    provenance('C34.review', doc['review'])
    rec = [r for r in RES['R29'] if r.get('check') == doc['base']['record']]
    cur = sha(ROOT / doc['base']['file'])
    check('C34.vectorIsReviewedFile', len(rec) == 1 and rec[0].get('sha256') == cur, recorded=rec[0].get('sha256') if rec else None, current=cur)
    u = rj(doc['base']['file'])
    inv = doc['invariants']
    sets = {'boundaries': [str(b['len']) for b in u['boundaries']], 'invalidLength': [c['id'] for c in u['invalidLength']],
            'arrays': [c['id'] for c in u['arrays']], 'noncanonical': [c['id'] for c in u['noncanonical']]}
    check('C34.everyCaseHasInvariant', all(sorted(sets[k]) == sorted(inv[k]) for k in sets), sets=sets)
    for k, src in u['sources'].items():
        provenance('U08.source.' + k, src)
    for b in u['boundaries']:
        L, canon = b['len'], b['canonical']
        iv = inv['boundaries'][str(L)]
        rec_ = {'manifestHash': b'\x00' * 32, 'manifestLen': L, 'chunks': [(bytes(20), x) for x in canon]}
        r = M.validate_version(rec_, {})
        data = bytes(i % 251 for i in range(L))
        chunks, prov = M.manifest_chunks(data)
        full = M.validate_version({'manifestHash': M.keccak256(data), 'manifestLen': L, 'chunks': chunks}, prov)
        cells = {'canonical': O.canonical_split(L) == canon and [len(p) for p in M.split(b'\x00' * L)] == canon,
                 'sumInvariant': sum(canon) == iv['expectedSum'] and iv['delta'] == iv['expectedSum'] - L and iv['sumMatches'] is True,
                 'contractAccepts': O.contract_args(L, len(canon), canon) is None,
                 'viewerSplitPasses': r.get('rule') == 'mfetch.missing' and r.get('fetchCalls') == 1,
                 'viewerManifestStagePasses': full.get('stage') != 'manifest' and full.get('fetchCalls') == len(canon),
                 'sixKeys': O.proof2_keys(L)['keys'] == 6}
        bad = sorted(k for k, v in cells.items() if not v)
        check('U08.boundary.%d' % L, not bad, failingCells=bad)
    for c in u['invalidLength']:
        cells = eval_negative(dict(c), inv['invalidLength'][c['id']])
        bad = sorted(k for k, v in cells.items() if not v)
        check('U08.invalidLength.' + c['id'], not bad, failingCells=bad)
    for c in u['arrays']:
        cells = eval_negative(dict(c), inv['arrays'][c['id']])
        bad = sorted(k for k, v in cells.items() if not v)
        check('U08.arrays.' + c['id'], not bad, failingCells=bad)
    for c in u['noncanonical']:
        iv = inv['noncanonical'][c['id']]
        cells = eval_negative(dict(c), iv)
        bad = sorted(k for k, v in cells.items() if not v)
        check('U08.noncanonical.' + c['id'], not bad, failingCells=bad, sum=sum(_lens(c)), declared=c['len'], invariant=iv)
    n05 = next(c for c in u['noncanonical'] if c['id'] == 'N05')
    check('C34.N05.deliberateMismatchRejected', sum(_lens(n05)) == 24575 and n05['len'] == 24576 and n05['contract'] == 'ManifestSplit(1)'
          and all(eval_negative(dict(n05), inv['noncanonical']['N05']).values()))
    pk, ranges = u['proofKeys'], {}
    for L in range(1, 65537):
        k = O.proof2_keys(L)
        rg = ranges.setdefault('k%d' % k['chunks'], [L, L])
        rg[1] = L
    check('U08.proofKeys.allLengths', ranges == pk['counts'], ranges=ranges)
    check('U08.proofKeys.bytes', O.proof_bytes(1) == pk['bytes']['k1'] and O.proof_bytes(6) == pk['bytes']['k6'] <= pk['bytes']['limit'] == 524288)
    by_id = {c['id']: c for c in u['noncanonical']}
    for m in doc['mutants']['list']:
        if 'drop' in m:
            ids = [i for i in sets['noncanonical'] if i != m['drop']]
            killed = sorted(ids) != sorted(inv['noncanonical'])
        else:
            c = dict(by_id[m['case']])
            c.update(m.get('set', {}))
            iv = dict(inv['noncanonical'][m['case']])
            iv.update(m.get('setInvariant', {}))
            killed = not all(eval_negative(c, iv).values())
        check('C34.mutantKilled.' + m['id'], killed, mutant=m)


# ------------------------------------------------------------------ V1 network, transcripts, tokens

def v1_network():
    net = rj('m1-draft-0.26/vectors/v1-network.json')
    gp = net['network']['genesisPre']
    segs = copy.deepcopy(rj('m1-draft-0.21/vectors/v3-gsv1.json')['flatSegments'])
    ok_seg = segs[gp['segmentIndex']] == gp['replace']
    segs[gp['segmentIndex']] = gp['with']
    pre = b''.join(bytes.fromhex(s) if isinstance(s, str) else bytes.fromhex(s['repeat']) * s['count'] for s in segs)
    NP = load_module(ROOT / 'm1-draft-0.21/tools/netprofile_ref.py', 'netprofile_021_030')
    spec = NP.decode_genesis(pre)
    cp_lit = {k: num(v) for k, v in net['network']['cp'].items()}
    check('C33.network.genesisPre', ok_seg and len(pre) == 341 and spec['chainId'] == 777002 and all(spec['CP'][k] == v for k, v in cp_lit.items()),
          label='V1NET is a labelled placeholder network (P-V1-8); no actual network claim')
    return {'chainId': spec['chainId'], 'genesisHash': KEC.keccak256(pre), 'forkSchedule': [tuple(x) for x in net['network']['forkSchedule']], 'cp': dict(spec['CP'])}


T27 = rj('coordination/review-001/m1-draft-0.27/results/v1-transcripts-0.27.json')


def replay_set():
    cases26 = {c['id']: c for c in rj('m1-draft-0.26/vectors/v1-window-cases.json')['cases']}
    sha27 = {c['id']: c for c in rj('m1-draft-0.27/vectors/v1-sha-accounting.json')['newCases']}
    out = []
    for tr in T27['carried']:
        ph = cases26[tr['case']]['phases'][tr['phase']]
        out.append(('carried.%s#%d' % (tr['case'], tr['phase']), tr, ph['clock'], ph.get('t', 0)))
    for tr in T27['c32']:
        out.append(('c32.' + tr['case'], tr, 1700000205, 0))
    for tr in T27['sha']:
        out.append(('sha.' + tr['case'], tr, sha27[tr['case']]['clock'], 0))
    return out


def kind_of(verdict):
    return 'ok' if verdict.get('ok') is True else verdict.get('rule')


def tokens():
    c32 = {t['case']: t for t in T27['c32']}
    shas = {t['case']: t for t in T27['sha']}
    base, s1 = c32['C32-ok-busy250'], shas['SHA-S1']
    tok = {'BN20': base['replies'][0], 'HDR20': base['replies'][-1], 'S1BN': s1['replies'][0], 'S1HDR': s1['replies'][1]}
    check('C33.tokens.fromReviewedTranscripts', base['outcome'].get('ok') is True and s1['outcome'].get('ok') is True and s1['outcome'].get('n') == 1
          and json.loads(tok['S1BN']).get('id') == 1 and json.loads(tok['S1HDR']).get('id') == 2 and json.loads(tok['BN20']).get('id') == 1)
    tok['@RS1'] = json.dumps(json.loads(tok['S1HDR'])['result'])
    return tok, s1


BUSY = '{"jsonrpc":"2.0","id":2,"error":{"code":-32021,"message":"%s","data":%s}}'


def build(spec, tok):
    """Returns (raw bytes, declaredLength | None) and asserts every builder's exact size/depth target."""
    if isinstance(spec, str):
        if spec in tok:
            return tok[spec].encode('utf-8'), None
        k, ms = spec.split(':')
        return (BUSY % (k, json.dumps({'reason': k, 'retryAfterMs': int(ms)}, separators=(',', ':')))).encode('utf-8'), None
    if 'token' in spec:
        return tok[spec['token']].encode('utf-8'), spec.get('declaredLength')
    if 'text' in spec:
        return spec['text'].encode('utf-8'), None
    if 'pairs' in spec:
        body = ','.join('"%s":%s' % (k, tok['@RS1'] if v == '@RS1' else v) for k, v in spec['pairs'])
        return ('{' + body + '}').encode('utf-8'), None
    if 'padTo' in spec:
        t, n = spec['padTo']
        raw = tok[t].encode('utf-8')
        if len(raw) > n:
            raise ValueError('padTo below the token length')
        return raw + b' ' * (n - len(raw)), None
    if 'depthBusy' in spec:
        k = spec['depthBusy'] - 3
        data = '{"reason":"busy","retryAfterMs":250,"pad":' + '[' * k + ']' * k + '}'
        text = BUSY % ('busy', data)
        if B04.json_depth(text) != spec['depthBusy']:
            raise ValueError('depth builder')
        return text.encode('utf-8'), None
    if 'busyDataBytes' in spec:
        n = spec['busyDataBytes']
        base = {'reason': 'busy', 'retryAfterMs': 250, 'pad': ''}
        base['pad'] = 'x' * (n - G.compact_bytes(base))
        if G.compact_bytes(base) != n:
            raise ValueError('data builder')
        return (BUSY % ('busy', json.dumps(base, separators=(',', ':')))).encode('utf-8'), None
    if 'busyMessageUnits' in spec:
        return (BUSY % ('m' * spec['busyMessageUnits'], '{"reason":"busy","retryAfterMs":250}')).encode('utf-8'), None
    if 'seq' in spec:
        return b''.join(s['t'].encode('utf-8') if 't' in s else bytes.fromhex(s['h']) for s in spec['seq']), None
    if 'bom' in spec:
        return b'\xef\xbb\xbf' + tok[spec['bom']].encode('utf-8'), None
    raise ValueError('unknown builder %r' % spec)


# ------------------------------------------------------------------ C33

def c33(cfg):
    doc = own('vectors/c33-guard-cases.json')
    for k, src in doc['sources'].items():
        provenance('C33.source.' + k, src)
    check('C33.limitsAndIds', G.RECV_LIMIT == {'eth_blockNumber': 4096, 'pocol_getHeaders': 98304} and G.DEPTH_MAX == 16
          and G.REQUEST_ID == {'eth_blockNumber': 1, 'pocol_getHeaders': 2} and G.MESSAGE_UNITS_MAX == 256 and G.DATA_BYTES_MAX == 4096)
    tok, s1 = tokens()
    for c in doc['cases']:
        try:
            script = []
            for spec in c['replies']:
                raw, declared = build(spec, tok)
                script.append((0, raw, declared) if declared is not None else (0, raw))
            r = G.guarded_header_net_check(V, O, B04, cfg, script, c['clock'], 0)
            exc = None
        except Exception as e:                                            # a builder or model error is a FAIL, never hidden
            r, exc = None, '%s: %s' % (type(e).__name__, e)
        if r is None:
            check('C33.case.' + c['id'], False, exception=exc)
            continue
        e = c['expect']
        info = r['guardInfo'][-1] if r['guardInfo'] else {}
        got = {'outcome': kind_of(r['verdict']), 'guard': [r['guard']['reason'], r['guard']['stage']] if r['guard'] else None,
               'requests': len(r['requests']), 'sleptMs': r['sleptMs'], 'decodes': r['counters'].decodes}
        cells = {k: got[k] == e[k] for k in got}
        cells['frame'] = r['verdict'].get('frame') is (e['outcome'] == 'ok')
        if e['outcome'] != 'ok':
            cells['controlled'] = r['verdict'].get('cancel') == 4901 and r['verdict'].get('ok') is False
        if 'ids' in e:
            cells['ids'] = r['ids'] == e['ids']
        if 'clippedMessageUnits' in e:
            cells['clipped'] = any(i.get('clippedMessageUnits') == e['clippedMessageUnits'] for i in r['guardInfo'])
        if 'dataDropped' in e:
            cells['dataDropped'] = any(i.get('dataDropped') for i in r['guardInfo']) == e['dataDropped']
        if 'sameAsSaved' in e:
            cells['sameAsSaved'] = norm(r['verdict']) == norm(s1['outcome']) and norm(r['requests']) == norm(s1['requests'])
        if e['guard'] is not None and e['outcome'] != 'ok':
            cells['noShaWork'] = sum(r['counters'].sha.values()) == 0
        bad = sorted(k for k, v in cells.items() if not v)
        check('C33.case.' + c['id'], not bad, failingCells=bad, got=got, tag=c['tag'], lastGuardInfo=info)
    hnc_replay(cfg, doc)
    saved_replay(cfg)


def hnc_replay(cfg, doc):
    d29 = rj('m1-draft-0.29/vectors/deadline-cases-0.29.json')
    tok = d29['hnc']['tokens']
    c32 = {t['case']: t for t in T27['c32']}
    bn20, hdr20 = c32['C32-ok-busy250']['replies'][0], c32['C32-ok-busy250']['replies'][-1]

    def text(t):
        if t == 'BN20':
            return bn20
        if t == 'HDR20':
            return hdr20
        if ':' in t:
            k, ms = t.split(':')
            return tok[k + ':<ms>'].replace('<ms>', ms)
        return tok[t]

    ov = doc['hncReplay']['overrides']
    for c in d29['hnc']['cases']:
        r = G.guarded_header_net_check(V, O, B04, cfg, list(zip(c['lat'], [text(x) for x in c['replies']])), d29['hnc']['clock'], c['computeMs'])
        want = dict(c['expect'])
        if c['id'] in ov:
            want['why'] = ov[c['id']]['why']
        got = {'outcome': kind_of(r['verdict']), 'atMs': r['atMs'], 'why': r['why'], 'sleptMs': r['sleptMs'], 'requests': len(r['requests'])}
        check('C33.hncReplay.' + c['id'], got == want, got=got, override=c['id'] in ov)


def saved_replay(cfg):
    rejected, cnt = {}, Counter()
    for label, tr, clock, t0 in replay_set():
        r = G.guarded_header_net_check(V, O, B04, cfg, [(0, x) for x in tr['replies']], clock, 0, t0)
        saved_kind = kind_of(tr['outcome'])
        cells = {'kind': kind_of(r['verdict']) == saved_kind, 'requests': norm(r['requests']) == norm(tr['requests'])}
        if r['guard'] is None:
            cells['verdict'] = norm(r['verdict']) == norm(tr['outcome'])
        else:
            rejected[label] = r['guard']
            cells['noValidReplyRejected'] = saved_kind != 'ok'
        if label.startswith('sha.'):
            cd = r['counters'].as_dict()
            cells['shaCounters'] = all(cd[k] == tr['counters'][k] for k in ('sha256ByCategory', 'sha256Total', 'sha256UniquePreimages'))
        bad = sorted(k for k, v in cells.items() if not v)
        check('C33.replay.' + label, not bad, failingCells=bad, guard=r['guard'])
        cnt['guardRejected' if r['guard'] else 'guardPassed'] += 1
    record('C33.replay.summary', 'recorded', counts=dict(cnt), rejected=rejected,
           note='rejections are only on replies the 0.27 checker already refused; detail now names the receive guard')
    w = next(t for t in T27['sha'] if t['case'] == 'SHA-W')
    r = G.guarded_header_net_check(V, O, B04, cfg, [(0, x) for x in w['replies']], 1700000205, 0)
    cd = r['counters'].as_dict()
    check('C33.shaW.boundsPreserved', r['guard'] is None and r['verdict'].get('ok') is True and cd['sha256Total'] == 3355
          and cd['sha256ByCategory'] == {'templateId': 14, 'powHash': 13, 'shareHash': 3328} and len(w['replies'][1].encode('utf-8')) <= 98304,
          headersReplyBytes=len(w['replies'][1].encode('utf-8')), counters=cd)


def rp_shared():
    d29 = rj('m1-draft-0.29/vectors/deadline-cases-0.29.json')
    c = next(x for x in d29['rp']['cases'] if x['id'] == 'RP-COMB')
    eff = [{'outcome': o['outcome'], 'absoluteAtMs': o['absoluteAtMs']} for o in O.run_ops(c['ops'], 'shared')]
    rej = [{'outcome': o['outcome'], 'absoluteAtMs': o['absoluteAtMs']} for o in O.run_ops(c['ops'], 'separate')]
    check('U14.rp.RP-COMB.effectiveShared', eff == c['expect']['shared'] == [{'outcome': 'ok', 'absoluteAtMs': 6900}, {'outcome': 'unavailable', 'absoluteAtMs': 10000}], got=eff,
          convention='P-D29-3 shared budget')
    check('U14.rp.RP-COMB.rejectedSeparateComparison', rej == c['expect']['separate'] and rej != eff, got=rej, note='comparison only; not adopted')
    tm = rj('m1-draft-0.29/vectors/u02-revoked-viewer-0.29.json')['timing']
    post = O.run_steps(tm['postClickLoad'], O.Budget(tm['clickAtMs']), 'content')
    check('U14.rp.clickStartsFreshBudget', post['outcome'] == 'render' and post['atMs'] == 500 and tm['clickAtMs'] >= O.DEADLINE_MS, convention='P-U02-2')


# ------------------------------------------------------------------ decisions and conventions

def decision_u08():
    pats = ['^U08\\.', '^C34\\.']
    mine = [r for r in results if any(re.search(p, r['check']) for p in pats)]
    ok29 = all(ev_ok('R29', p)[0] for p in ('^authority\\.exact\\.U08$', '^source\\.U08\\.'))
    ok = bool(mine) and all(r['status'] != 'FAIL' for r in mine) and ok29
    if ok:
        APPLIED.add('U08')
    check('decision.U08.applied', ok, checks=len(mine), failed=[r['check'] for r in mine if r['status'] == 'FAIL'][:10], authority029=ok29,
          basis='delegated technical decision (user standing authorization + coordinator decision); not an owner answer')


def conventions():
    for cv in APPL['conventions']:
        oks = []
        for ev in cv['evidence']:
            if ev[0] == 'file':
                d = rj(ev[1])
                oks.append(all(d.get(k) == v for k, v in ev[2].items()))
            else:
                oks.append(ev_ok(ev[0], ev[1])[0])
        exact = any(r['check'] == 'authority.convention.' + cv['id'] and r['status'] == 'pass' for r in results)
        if check('convention.%s.decided' % cv['id'], exact and all(oks), evidence=cv['evidence'], evidenceOk=oks):
            DECIDED.add(cv['id'])


# ------------------------------------------------------------------ hash freeze

def freeze():
    exp = (ROOT / 'coordination/review-001/m1-draft-0.25/results/hash-freeze-export-0.25.json').read_text(encoding='utf-8')
    entries = [json.loads(line.rstrip(',')) for line in exp.split('\n')[1:] if line.startswith('{')]
    tri = {x['id']: x for x in rj('coordination/review-001/full-hash-triad-executed-029.json')['results']}
    out, bad, groups = [], [], {}
    for e in entries:
        pre = bytes.fromhex(e['preimageHex'])
        d = KEC.keccak256(pre).hex()
        t = tri.get(e['id'], {})
        three = t.get('pass') is True and t.get('pythonKeccak') == t.get('nobleKeccak') == t.get('jsSha3Keccak') == d \
            and t.get('inputSha256') == hashlib.sha256(pre).hexdigest()
        exp_d = (e.get('expectedDigest') or '').lower().replace('0x', '')
        form = e.get('assertedForm', 'full')
        asserted = not exp_d or (d == exp_d if form == 'full' else d.startswith(exp_d) if form == 'prefix4' else d.endswith(exp_d))
        decisions, unresolved, not_adopted = [], [], []
        for p in e.get('ownerParameters', []):
            (decisions if p in APPLIED else unresolved).append(p)
        for p in e.get('reviewerParameters', []):
            if p == 'ALT-E':
                not_adopted.append(p)
            elif p in APPLIED or p in DECIDED:
                decisions.append(p)
            elif all(x in ACCEPTED for x in PARAM_TOKEN.get(p, [p])):
                decisions += PARAM_TOKEN.get(p, [p])
            else:
                unresolved.append(p)
        status = 'notAdopted' if not_adopted else ('frozen' if three and asserted and not unresolved else 'blocked')
        if status == 'blocked':
            bad.append({'id': e['id'], 'three': three, 'asserted': asserted, 'unresolved': unresolved})
        g = groups.setdefault(e['group'], Counter())
        g[status] += 1
        out.append({'id': e['id'], 'group': e['group'], 'algorithm': 'keccak256', 'preimageHex': e['preimageHex'], 'digest': d, 'assertedForm': form,
                    'expectedDigest': exp_d or None, 'decisionIds': sorted(set(decisions)), 'phaseAConfirmation': e.get('phaseAConfirmation', []),
                    'status': status, 'evidence': 'full-hash-triad-executed-029.json'})
    check('freeze.legacy', len(out) == 668 and not bad, entries=len(out), blocked=bad[:10], groups={k: dict(v) for k, v in groups.items()})
    v1, vbad = [], []
    for tag, rel, ind in (('0.26', 'coordination/review-001/m1-draft-0.26/results/hash-freeze-v1-0.26.json', 'coordination/review-001/v1-independent-hashes-0.26.json'),
                          ('0.27', 'coordination/review-001/m1-draft-0.27/results/hash-freeze-v1-0.27.json', 'coordination/review-001/v1-hash-independent-0.27.json')):
        ver = {x['id'] for x in rj(ind)['results'] if x.get('pass') is True}
        for e in rj(rel)['list']:
            pre = bytes.fromhex(e['preimageHex'])
            d = hashlib.sha256(pre).hexdigest() if e['algorithm'] == 'sha256' else KEC.keccak256(pre).hex()
            ok = d == e['digest'] and e['id'] in ver
            if not ok:
                vbad.append(e['id'])
            dec = INV['v1FreezeDecisions']['keys.public' if e['group'] == 'keys.public' else 'v1.']
            v1.append({'id': '%s/%s' % (tag, e['id']), 'group': e['group'], 'algorithm': e['algorithm'], 'preimageHex': e['preimageHex'], 'digest': d,
                       'decisionIds': dec, 'status': 'frozen' if ok else 'blocked', 'evidence': Path(ind).name})
            g = groups.setdefault(e['group'], Counter())
            g['frozen' if ok else 'blocked'] += 1
    check('freeze.v1', len(v1) == 347 + 3677 and not vbad, entries=len(v1), blocked=vbad[:10])
    for a, t in (('alias.versionRecords', 'triad.manifests'), ('alias.t1_02.manifestHash', 'triad.manifests'), ('alias.X1.netKey', 'triad.netKey')):
        groups[a] = groups.get(t, Counter({'missing': 1}))
    labelled = {g: {'status': 'labelledNotFrozen', 'label': why} for g, why in INV['syntheticGroups'].items()}
    gstat = {g: ('frozen' if set(c) <= {'frozen'} else 'blocked' if c.get('blocked') or c.get('missing') else 'notAdopted') for g, c in groups.items()}
    gstat.update({g: v['status'] for g, v in labelled.items()})
    check('freeze.syntheticLabelled', all(v['status'] == 'labelledNotFrozen' for v in labelled.values()), labelled=labelled)
    return out, v1, gstat, labelled


# ------------------------------------------------------------------ consolidation

def consolidate(gstat):
    have = {r['check']: r['status'] for r in results}
    rows29 = M29['rows']
    hg = {r['id']: r['hashGroups'] for r in INV25['rows']}
    matrix = {}
    for rid, m in rows29.items():
        owners = list(m['ownerOpen'])
        if m.get('c4Basis'):
            owners += [t.strip() for t in m['c4Basis'].split(':', 1)[1].split(',')]
        owner_open = [t for t in owners if t not in APPLIED]
        rev_open = [t for t in m['reviewerOpen'] if t not in DECIDED and t not in ACCEPTED]
        ev = saved_status('R29', '^row\\.%s\\.(evidence|supplement)\\.' % re.escape(rid))
        ev_pass = bool(ev) and all(r['status'] == 'pass' for r in ev)
        missing = list(m['missing']) + list(m['unresolved'])
        changed = rid in INV['changedRows']
        cur_ok = True
        if changed:
            for p in INV['changedRows'][rid]['currentRunEvidence']:
                mine = [s for k, s in have.items() if re.search(p, k)]
                cur_ok = cur_ok and bool(mine) and all(s != 'FAIL' for s in mine)
            missing += [f for f in INV['addFiles'] if not path_of(f).exists()]
        hash_blocked = [g for g in hg.get(rid, []) if gstat.get(g) in ('blocked',)]
        c1 = 'blocked' if missing else ('pendingRootReview' if changed else 'satisfied')
        c2 = 'blocked' if not (ev_pass and cur_ok) else ('pendingRootReview' if changed else 'satisfied')
        c3 = 'pendingRootReview' if changed else 'satisfied'
        c4 = 'blocked' if owner_open else ('pendingReviewerDecision' if rev_open else 'satisfied')
        c5 = 'blocked' if (owner_open or missing or hash_blocked) else ('pendingRootReview' if changed else 'satisfied')
        crit = {'c1': c1, 'c2': c2, 'c3': c3, 'c4': c4, 'c5': c5}
        st = 'CompleteCandidate' if all(v == 'satisfied' for v in crit.values()) else \
            ('PendingRootReview' if all(v in ('satisfied', 'pendingRootReview') for v in crit.values()) else 'Partial')
        basis = {'c3': ('pending root review of 0.30: ' + INV['changedRows'][rid]['why']) if changed else
                 ('REVIEW-0.29' if rid in INV['reviewedBy029']['rows'] else 'earlier root reviews (bound in 0.29)'),
                 'c4': ('delegated: ' + ', '.join(t for t in owners if t in APPLIED)) if owners else None,
                 'conventions': [t for t in m['reviewerOpen'] if t in DECIDED]}
        matrix[rid] = {'title': m['title'], 'source': m['source'], 'status': st, 'criteria': crit, 'basis': basis, 'ownerOpen': owner_open,
                       'reviewerOpen': rev_open, 'missing': missing, 'hashGroups': {g: gstat.get(g, 'notInFreeze') for g in hg.get(rid, [])},
                       'rowEvidence029': len(ev), 'phaseA': m.get('phaseA', []), 'experiments': m.get('experiments', [])}
        record('row.%s.criteria' % rid, 'recorded', **{k: v for k, v in matrix[rid].items() if k not in ('title', 'source')})
    want = {r: s for s, rows in INV['proposedStatus'].items() for r in rows}
    got = {r: v['status'] for r, v in matrix.items()}
    check('consolidation.rowsMatchProposal', got == want and len(got) == 41, differing={r: [got.get(r), want.get(r)] for r in set(got) | set(want) if got.get(r) != want.get(r)})
    record('consolidation.summary', 'recorded', counts=dict(Counter(got.values())))
    return matrix


def findings():
    out = {}
    repaired = set(INV['repairedTokens'])
    for fid, f in M29['findings'].items():
        dep = f['dependsOn']
        undecided = [t for t in dep if not (t in ACCEPTED or t in APPLIED or t in DECIDED)]
        if undecided:
            disp = 'open'
        elif any(t in repaired for t in dep):
            disp = 'closableAfterRootReviewOf030'
        else:
            disp = 'closableAtSpecScope'
        out[fid] = {'disposition': disp, 'dependsOn': dep, 'undecided': undecided, 'phaseA': f.get('phaseA', [])}
        record('finding.' + fid, 'recorded', **out[fid])
    want = {f: d for d, fs in INV['proposedFindings'].items() for f in fs}
    got = {f: v['disposition'] for f, v in out.items()}
    check('consolidation.findingsMatchProposal', got == want and len(got) == 26, differing={f: [got.get(f), want.get(f)] for f in set(got) | set(want) if got.get(f) != want.get(f)})
    return out


def experiments():
    ok29, n = ev_ok('R29', '^experiments\\.definitionsComplete$')
    ex = own('vectors/experiment-amendments-0.30.json')
    new_ok = all(e.get('definitionComplete') is True for e in ex['amended'] + ex['new'])
    check('experiments.definitionsComplete', ok29 and new_ok and any(e['id'] == 'X-C33' for e in ex['new']), r29=ok29)


# ------------------------------------------------------------------ outputs

def write_asset(name, data_text, label):
    OUT.mkdir(parents=True, exist_ok=True)
    p = OUT / name
    data = data_text.encode('utf-8')
    if p.exists() and p.read_bytes() != data:
        k = 1
        while (OUT / ('%s-rerun-%d%s' % (p.stem, k, p.suffix))).exists():
            k += 1
        alt = OUT / ('%s-rerun-%d%s' % (p.stem, k, p.suffix))
        alt.write_bytes(data)
        record(label + '.differsFromEarlierRun', 'recorded', kept=name, written=alt.name, sha256=hashlib.sha256(data).hexdigest())
        return
    if not p.exists():
        p.write_bytes(data)
    check(label + '.writtenAndReadBack', p.read_bytes() == data, file=name, sha256=hashlib.sha256(data).hexdigest(), bytes=len(data))


def dumps(x):
    return json.dumps(x, sort_keys=True, indent=1, ensure_ascii=False, default=str) + '\n'


def dashboard(matrix, finds, failed):
    cnt = Counter(v['status'] for v in matrix.values())
    fc = Counter(v['disposition'] for v in finds.values())
    pend = [r for r, v in matrix.items() if v['status'] != 'CompleteCandidate']
    lines = [
        'M1، المسودة 0.30: إصلاحان مركزان (C33 وC34) وتطبيق 17 عرفًا اختارها المنسق بتفويض من المستخدم.',
        'هذه اختيارات تقنية مفوضة، وليست موافقة شخصية من المالك.',
        'C33: كل رد يصل إلى فحص الرؤوس يمر أولًا بحارس الاستقبال: حد الحجم، وUTF-8 الصارم، والعمق 16، ورفض المفاتيح المكررة، وربط رقم الطلب. أي فشل يوقف العرض دون إعادة أو انتظار.',
        'C34: الحالة N05 مقصودة؛ مجموع قطعها 24575 والطول المعلن 24576. صار لكل حالة شرط مجموعها الخاص، وأُبقيت الحالة.',
        'الميزانية في التصفح الأول في RP واحدة: عشر ثوانٍ من أول طلب لفحص الرؤوس حتى حكم المحتوى.',
        'نتيجة التشغيل: %d فحصًا، فشل منها %d.' % (len(results), len(failed)),
        'الصفوف: %d مرشح للاكتمال، و%d بانتظار مراجعة الجذر، و%d جزئي.' % (cnt['CompleteCandidate'], cnt['PendingRootReview'], cnt['Partial']),
        'بانتظار مراجعة الجذر: %s.' % ('، '.join(pend) if pend else 'لا شيء'),
        'الملاحظات: %d قابلة للإغلاق، و%d بعد مراجعة 0.30.' % (fc['closableAtSpecScope'], fc['closableAfterRootReviewOf030']),
        '«مرشح للاكتمال» لا يعني «مكتمل». تجارب المتصفح والعقدة والعقد لاحقة، وليست شرطًا للبوابة.',
        'لا كود إنتاجي ولا نشر ولا معاملات.'
    ]
    return {'schema': 'pocol-m1-dashboard-ar/0.30', 'lang': 'ar', 'lines': lines, 'pendingRows': pend}


def leak_and_keys(texts):
    markers = ('PoCol_' + 'Dialogue', 'state' + '.json', 'source' + 'State', 'historical-x8-full' + '-source-private', 'transcript' + '-private')
    leaks = [str(f.relative_to(PKG)) for f in sorted(PKG.rglob('*')) if f.is_file() and 'results' not in f.parts
             and f.suffix in ('.json', '.md', '.py') and any(m in f.read_text(encoding='utf-8') for m in markers)]
    leaks += [n for n, t in texts.items() if any(m in t for m in markers)]
    check('boundary.noPrivatePath', not leaks, files=leaks)
    keys = [v['priv'][2:].lower() for v in rj('coordination/review-001/m1-draft-0.5/results/txa-node-check.json')['keys'].values()]
    check('boundary.noPrivateKeyInOutputs', not [n for n, t in texts.items() if any(k in t.lower() for k in keys)])


def coverage():
    have = {r['check']: r['status'] for r in results}
    need = ['authority.standingAuthorization', 'authority.conventionsSet', 'bind029.summaryAndFailures', 'bind029.reviewCopyByteIdentical',
            'bind029.harnessFailurePreservedAndExplained', 'bind029.guardProbesPreservedAsFailing', 'C34.vectorIsReviewedFile', 'C34.everyCaseHasInvariant',
            'C34.N05.deliberateMismatchRejected', 'U08.proofKeys.allLengths', 'decision.U08.applied', 'C33.limitsAndIds', 'C33.tokens.fromReviewedTranscripts',
            'C33.shaW.boundsPreserved', 'U14.rp.RP-COMB.effectiveShared', 'U14.rp.RP-COMB.rejectedSeparateComparison', 'U14.rp.clickStartsFreshBudget',
            'freeze.legacy', 'freeze.v1', 'freeze.syntheticLabelled', 'consolidation.rowsMatchProposal', 'consolidation.findingsMatchProposal',
            'experiments.definitionsComplete', 'boundary.noPrivatePath', 'boundary.noPrivateKeyInOutputs']
    need += ['authority.convention.' + d['id'] for d in CONV['decisions']] + ['convention.%s.decided' % d['id'] for d in CONV['decisions']]
    need += ['bind029.decisionApplied.' + d for d in NINE if d != 'U08']
    need += ['C33.case.' + c['id'] for c in own('vectors/c33-guard-cases.json')['cases']]
    need += ['C33.hncReplay.' + c['id'] for c in rj('m1-draft-0.29/vectors/deadline-cases-0.29.json')['hnc']['cases']]
    need += ['C34.mutantKilled.' + m['id'] for m in own('vectors/c34-u08-invariants.json')['mutants']['list']]
    u = rj('m1-draft-0.29/vectors/u08-canonical-split-0.29.json')
    need += ['U08.noncanonical.' + c['id'] for c in u['noncanonical']] + ['U08.boundary.%d' % b['len'] for b in u['boundaries']]
    need += ['U08.invalidLength.' + c['id'] for c in u['invalidLength']] + ['U08.arrays.' + c['id'] for c in u['arrays']]
    need += [k for k in have if k.startswith('C33.replay.') and k != 'C33.replay.summary']
    missing = [x for x in need if have.get(x) != 'pass']
    check('coverage030.required', not missing, missing=missing, required=len(need))
    check('coverage030.noStepAborted', not [k for k in have if k.startswith('step.completed')])


def history_final():
    have = {r['check']: r['status'] for r in results}
    rb = own('vectors/c34-u08-invariants.json')['reboundFailures']['failures']
    r29 = {r['check']: r['status'] for r in RES['R29']}
    check('history.HF-8', all(r29.get(k) == 'FAIL' for k in rb) and all(have.get(v) == 'pass' for v in rb.values()),
          stillFailIn029={k: r29.get(k) for k in rb}, repairedNow={v: have.get(v) for v in rb.values()})
    for h in INV['historical']['new'][1:]:
        d = rj(h['file'])
        repaired = [x for x in h['reboundTo'] if x.startswith(('C33.', 'freeze.'))]
        preserved = (d.get('failed') == h['failed']) if 'failed' in h else True
        check('history.' + h['id'], preserved and bool(repaired) and all(have.get(x) == 'pass' for x in repaired),
              repairedBy=h['reboundTo'], preservedFile=h['file'], stillRecordedAsFailing=preserved)


def step(name, fn, *a):
    try:
        return fn(*a)
    except Exception as ex:                                                  # record, never hide
        check('step.completed ' + name, False, exception='%s: %s' % (type(ex).__name__, ex))
        return None


def main():
    t0 = time.time()
    before = {rel: sha(ROOT / rel) for rel in PRESERVED}
    for rel in OWN:
        record('input.sha256 <package>/' + rel, 'recorded', exists=(PKG / rel).exists(), sha256=sha(PKG / rel))
    for rel in INPUTS_ROOT:
        record('input.sha256 ' + rel, 'recorded', exists=(ROOT / rel).exists(), sha256=sha(ROOT / rel))
    step('boundary', boundary)
    step('authority', authority)
    step('bind029', bind029)
    step('c34', c34)
    step('decisionU08', decision_u08)
    cfg = step('v1network', v1_network)
    if cfg is not None:
        step('c33', c33, cfg)
    step('rp', rp_shared)
    step('conventions', conventions)
    fz = step('freeze', freeze) or ([], [], {}, {})
    matrix = step('consolidation', consolidate, fz[2]) or {}
    finds = step('findings', findings) or {}
    step('experiments', experiments)
    gap('gap.rootReviewOf030', text='C33 and C34 stay open until root executes this run in a preserved copy and reviews it; no row is Complete')
    gap('gap.phaseA', text='X-C33, X-U14, E01-E04, E06, E07 outcomes (Chrome, node, EVM) are later implementation results, not M1-spec gate conditions')
    gap('gap.ledgerFinalVerification', text='C06, C07, U08-CR, RF-E6-1, V1-SHA-COST, P-C32-2: ledger final verification is root\'s action after this review')
    failed = [r for r in results if r['status'] == 'FAIL']
    texts = {'acceptance-matrix-0.30.json': dumps({'schema': 'pocol-m1-acceptance-matrix/0.30', 'rows': matrix, 'findings': finds,
                                                   'counts': dict(Counter(v['status'] for v in matrix.values())),
                                                   'note': 'CompleteCandidate is not Complete; PendingRootReview awaits root review of the 0.30 repairs'}),
             'decision-register-0.30.json': dumps({'schema': 'pocol-m1-decision-register/0.30', 'delegatedApplied': sorted(APPLIED), 'conventionsDecided': sorted(DECIDED),
                                                   'conventions': APPL['conventions'], 'ownerAnswers': 'unanswered; not written or inferred'}),
             'hash-freeze-0.30.json': dumps({'schema': 'pocol-m1-hash-freeze/0.30', 'legacy': fz[0], 'v1': fz[1], 'groups': fz[2], 'labelled': fz[3]}),
             'bindings-0.30.json': dumps({'schema': 'pocol-m1-bindings/0.30', 'files': {rel: sha(ROOT / rel) for rel in INPUTS_ROOT + PRESERVED},
                                          'checks': {r['check']: r['status'] for r in results if r['check'].startswith(('bind029.', 'history.', 'C33.replay.'))}})}
    step('leaks', leak_and_keys, texts)
    coverage()
    history_final()
    after = {rel: sha(ROOT / rel) for rel in PRESERVED}
    check('preserved.olderFilesUnchanged', before == after, changed=[k for k in before if before[k] != after[k]])
    failed = [r for r in results if r['status'] == 'FAIL']
    texts['dashboard-0.30-ar.json'] = dumps(dashboard(matrix, finds, failed))
    for name in sorted(texts):
        step('write ' + name, write_asset, name, texts[name], 'asset.' + name)
    failed = [r for r in results if r['status'] == 'FAIL']
    out = {'package': 'M1 draft 0.30 (C33 guarded HeaderNetCheck receive boundary; C34 U08 invariants; 17 conventions; freeze; 41-row gate)',
           'executedAtUtc': datetime.now(timezone.utc).isoformat(timespec='seconds'),
           'environment': {'python': sys.version.split()[0], 'platform': platform.platform(), 'thirdPartyPackages': 'none'},
           'statusSchema': {'coreStatus': list(CORE_STATUS), 'diagnosticRenames': RENAMED, 'partialGap': 'recorded entries with partialGap=true'},
           'summary': {'checks': len(results), 'passed': sum(r['status'] == 'pass' for r in results), 'recorded': sum(r['status'] == 'recorded' for r in results),
                       'failed': len(failed), 'rows': dict(Counter(v['status'] for v in matrix.values())) if matrix else None, 'seconds': round(time.time() - t0, 1)},
           'results': results}
    OUT.mkdir(parents=True, exist_ok=True)
    p, k = OUT / 'run-results-0.30.json', 1
    while p.exists():
        p, k = OUT / ('run-results-0.30-rerun-%d.json' % k), k + 1
    p.write_text(json.dumps(out, indent=2, ensure_ascii=True, default=str) + '\n', encoding='utf-8')
    print(json.dumps(out['summary']), '->', p.name)
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
