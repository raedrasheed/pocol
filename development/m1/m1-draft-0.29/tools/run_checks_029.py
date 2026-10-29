"""Reference-only acceptance runner for M1 draft 0.29 (author turn 027): the delegated-decision overlay and the full
41-row gate. Python standard library only; no network, browser, EVM, chain, real clock or subprocess.
NOT executed by the author. Results are written only by root, in a preserved review copy.

Usage (any of):
  coordination\\runtime\\python311\\python.exe m1-draft-0.29\\tools\\run_checks_029.py
  ...\\run_checks_029.py --root D:\\PoCol-Development --out <dir>
Environment alternatives: POCOL_M1_ROOT, POCOL_M1_OUT.
  root: the tree holding reference/, coordination/ and m1-draft-0.2 ... 0.28 (default: nearest ancestor of this file holding
        both reference/ and m1-draft-0.2/). The package's own files are always read next to this script.
  out:  where every generated file goes (default: <this package>/results).
Writes only under out, never overwriting an earlier run (a differing later run gets a -rerun-<n> name):
  run-results-0.29.json, acceptance-matrix-0.29.json, decision-register-0.29.json, hash-bindings-0.29.json,
  deadline-rerun-0.29.json, dashboard-0.29-ar.json.
Rebuilds no header, nonce or share: the reviewed 0.27 V1 transcripts are replayed through the unchanged 0.27 checker
inside the 0.29 virtual deadline. Expected run time: a few minutes (pure-Python signature recovery and hashing).
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
OWN = ['tools/run_checks_029.py', 'tools/overlay_ref_029.py', 'decisions/adopted-decisions-0.29.json', 'audit/acceptance-inventory-0.29.json',
       'vectors/deadline-cases-0.29.json', 'vectors/u08-canonical-split-0.29.json', 'vectors/u02-revoked-viewer-0.29.json',
       'vectors/u10-publisher-transfer-0.29.json', 'vectors/rfe61-x12b-binding-0.29.json', 'vectors/experiment-amendments-0.29.json',
       'M1-SPEC-0.29-OVERLAY.md', 'M1-STATUS-0.29.md', 'M1-DASHBOARD-0.29-AR.md', 'README.md']
INPUTS_ROOT = ['coordination/task-027.md', 'coordination/DELEGATED-M1-DECISIONS-029.json', 'coordination/issue-ledger.json',
               'coordination/review-001/REVIEW-0.28.md', 'coordination/review-001/u14-independent-0.28.json',
               'coordination/review-001/m1-draft-0.27/results/v1-transcripts-0.27.json', 'coordination/review-001/m1-draft-0.27/results/hash-freeze-v1-0.27.json',
               'coordination/review-001/m1-draft-0.26/results/hash-freeze-v1-0.26.json', 'm1-draft-0.27/tools/v1_ref_027.py',
               'm1-draft-0.25/tools/supplements_ref.py', 'm1-draft-0.2/tools/m1model.py', 'reference/browser.md', 'reference/network.md',
               'reference/validation.md', 'reference/FINAL_DESIGN.md']
PRESERVED = ['coordination/review-001/m1-draft-0.27/results/run-results-0.27.json', 'coordination/review-001/m1-draft-0.27/results/v1-transcripts-0.27.json',
             'coordination/review-001/m1-draft-0.28/results/run-results-0.28.json', 'coordination/review-001/m1-draft-0.18/results/run-results-0.18.json',
             'coordination/review-001/u14-independent-0.28.json', 'coordination/DELEGATED-M1-DECISIONS-029.json',
             'm1-draft-0.27/tools/v1_ref_027.py', 'm1-draft-0.28/vectors/u14-timing-cases.json', 'm1-draft-0.25/audit/row-inventory.json',
             'm1-draft-0.17/vectors/rf-e6-1-x12-timelines.json', 'm1-draft-0.5/vectors/proof-response-cases.json', 'reference/browser.md']

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
    """Package-relative paths of this revision resolve next to this script (preserved copies keep working)."""
    return PKG / rel[len('m1-draft-0.29/'):] if rel.startswith('m1-draft-0.29/') else ROOT / rel


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


def contains(rel, text):
    p = path_of(rel)
    return p.exists() and text in p.read_text(encoding='utf-8')


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


# ------------------------------------------------------------------ modules (all unchanged earlier tools, loaded by path)

KEC = load_module(ROOT / 'm1-draft-0.2/tools/keccak.py', 'keccak_029')
RLP3 = load_module(ROOT / 'm1-draft-0.3/tools/rlp_strict.py', 'rlp_strict_03_029')
V = load_module(ROOT / 'm1-draft-0.27/tools/v1_ref_027.py', 'v1_ref_027_029')
V.bind(KEC, RLP3)
S = load_module(ROOT / 'm1-draft-0.25/tools/supplements_ref.py', 'supplements_ref_025_029')
sys.path.insert(0, str(HERE))
import overlay_ref_029 as O           # noqa: E402  (this package only)
sys.path.insert(0, str(ROOT / 'm1-draft-0.2/tools'))                  # m1model imports its siblings keccak and rlp_strict
M = load_module(ROOT / 'm1-draft-0.2/tools/m1model.py', 'm1model_02_029')

DEL = rj('coordination/DELEGATED-M1-DECISIONS-029.json')
REG = own('decisions/adopted-decisions-0.29.json')
INV = own('audit/acceptance-inventory-0.29.json')
INV25 = rj('m1-draft-0.25/audit/row-inventory.json')
CONS26 = rj('m1-draft-0.26/audit/consolidation-0.26.json')
CONS27 = rj('m1-draft-0.27/audit/consolidation-0.27.json')
CF28 = rj('m1-draft-0.28/audit/carry-forward-0.28.json')
DEC25 = rj('coordination/review-001/REVIEWER-DECISIONS-0.25.json')
TOKEN = INV['tokenMap']
REGD = {d['id']: d for d in REG['delegated']}
RESULT_FILES = dict(INV25['results'])
RESULT_FILES.update(CONS26['results'])
RESULT_FILES.update(CONS27['results'])
RESULT_FILES.update(INV['results'])
NOT_ADOPTED = {'ALT-E'}
PARAM_TOKEN = {'P-ABI': ['P-0.2', 'U22'], 'P13': ['P-0.2', 'U06'], 'P-X1': ['P-X1'], 'TK-1': ['TK-1']}
ACCEPTED = {d['id'] for d in DEC25['decisions'] if d['status'] == 'accepted-technical-qualified'}
APPLIED = set()
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


def evidence(label, key, pattern):
    hit = match(key, pattern)
    if hit is None:
        return check(label, False, results=RESULT_FILES[key], missingFile=True)
    st = Counter(r.get('status') for r in hit)
    fails = [r['check'] for r in hit if r.get('status') == 'FAIL']
    unexplained = [c for c in fails if (key, c) not in SUPERSEDED_OK]
    return check(label, st.get('pass', 0) >= 1 and not unexplained, results=RESULT_FILES[key], pattern=pattern, matched=len(hit),
                 passed=st.get('pass', 0), unexplainedFailures=unexplained)


# ------------------------------------------------------------------ boundary

def boundary():
    check('boundary.positionalOnly', record.__code__.co_posonlyargcount == 2 and check.__code__.co_posonlyargcount == 2 and gap.__code__.co_posonlyargcount == 1)
    probe = {'x': [1]}
    record('boundary.probe', 'recorded', name='n', status='s', obj=probe)
    probe['x'].append(2)
    e = results[-1]
    check('boundary.renamedAndCopied', e['diagName'] == 'n' and e['proposalStatus'] == 's' and e['status'] == 'recorded' and e['obj'] == {'x': [1]})
    bad, calls = {}, []
    for f in (HERE / 'run_checks_029.py', HERE / 'overlay_ref_029.py'):
        tree = ast.parse(f.read_text(encoding='utf-8'))
        mods = {a.name.split('.')[0] for n in ast.walk(tree) if isinstance(n, ast.Import) for a in n.names}
        mods |= {(n.module or '').split('.')[0] for n in ast.walk(tree) if isinstance(n, ast.ImportFrom)}
        hit = sorted(mods & {'socket', 'urllib', 'http', 'requests', 'ssl', 'asyncio', 'subprocess', 'threading'})
        if hit:
            bad[f.name] = hit
        calls += [n.lineno for n in ast.walk(tree) if isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute) and n.func.attr == 'main']
    check('boundary.noNetworkOrProcessModules', not bad, imports=bad)
    check('boundary.noOldMainCalled', not calls, attributeMainCallsAtLines=calls)
    tree = ast.parse((HERE / 'overlay_ref_029.py').read_text(encoding='utf-8'))
    mods = {a.name.split('.')[0] for n in ast.walk(tree) if isinstance(n, ast.Import) for a in n.names}
    mods |= {(n.module or '').split('.')[0] for n in ast.walk(tree) if isinstance(n, ast.ImportFrom)}
    check('boundary.noRealClockInModels', not (mods & {'time', 'datetime', 'random'}), imports=sorted(mods))


# ------------------------------------------------------------------ authority and exact application

def authority():
    led = rj('coordination/issue-ledger.json')
    sa, dd = led.get('standingAuthorization', {}), led.get('delegatedDecisions', {})
    want = REG['authority']['ledger']['standingAuthorization']
    check('authority.ledgerStandingAuthorization', all(sa.get(k) == v for k, v in want.items()), standingAuthorization=sa)
    ids_del = [d['id'] for d in DEL['decisions']]
    check('authority.registerIds', dd.get('register') == REG['authority']['register'] and sorted(dd.get('ids', [])) == sorted(ids_del) == sorted(REGD),
          ledgerIds=dd.get('ids'), registerIds=ids_del, overlayIds=sorted(REGD))
    record('authority.ledgerDelegatedStatus', 'recorded', status=dd.get('status'))
    a = DEL['authority']
    check('authority.notPersonalApproval', a.get('personalApproval') is False and a.get('noProduction') is True and a.get('noPermissionBypass') is True
          and a.get('kind') == 'user-standing-authorization')
    check('authority.ownerAnswersUntouched', REG['authority']['ownerAnswers'].startswith('unanswered')
          and not any(k in d for d in REG['delegated'] for k in ('ownerAnswer', 'ownerApproved', 'approvedBy')))
    check('authority.reviewer025NotOwner', DEC25['ownerApproved'] is False
          and not (ACCEPTED & {'U01', 'U02', 'U10', 'U14', 'CR-M1-01', 'RF-E6-1', 'U08', 'V1-SHA-COST', 'P-C32-2'}))
    fields = {'U01': ['option', 'value'], 'U02': ['option'], 'U10': ['option'], 'U14': ['option', 'deadlineMs', 'tieRule', 'coveredOperations'],
              'CR-M1-01': ['option'], 'U08': ['option'], 'RF-E6-1': ['option'], 'V1-SHA-COST': ['option', 'limits'],
              'P-C32-2': ['option', 'rateDelayMs', 'busyDelayMs', 'maxRetries']}
    for d in DEL['decisions']:
        r = REGD.get(d['id'], {})
        diff = {f: [d.get(f), r.get(f)] for f in fields.get(d['id'], ['option']) if d.get(f) != r.get(f)}
        check('authority.exact.' + d['id'], d['id'] in fields and not diff, differences=diff)
        for i, src in enumerate(r.get('sources', [])):
            provenance('source.%s.%d' % (d['id'], i), src)
    for x in REG['reviewer']['acceptedAfter0_25']:
        if provenance('reviewer.acceptedAfter0_25.' + x['id'], x['source']):
            ACCEPTED.add(x['id'])
    overlay = (PKG / 'M1-SPEC-0.29-OVERLAY.md').read_text(encoding='utf-8')
    missing = [d['id'] for d in REG['delegated'] if d['effective'] not in overlay]
    check('authority.effectiveTextsInOverlay', not missing, missing=missing)
    pend = [p['id'] for p in REG['reviewer']['pendingRootDecision']]
    check('authority.pendingConventionsListed', all(p in overlay for p in pend) and not (set(pend) & ACCEPTED), pending=pend)


# ------------------------------------------------------------------ U01

def u01():
    v = REGD['U01']['value']
    check('U01.factoryConstantOf02Model', '0x' + M.FACTORY.hex() == v, model='0x' + M.FACTORY.hex(), value=v)
    check('U01.notADeployment', REGD['U01']['deploymentConfirmed'] is False and 'NOT a deployment address' in REGD['U01']['effective'])
    evidence('U01.codeTableVectors', 'R02', '^codeTable\\.')
    evidence('U01.draft01Addresses', 'R02', '^draft01\\.')
    exp = (ROOT / 'coordination/review-001/m1-draft-0.25/results/hash-freeze-export-0.25.json').read_text(encoding='utf-8')
    entries = [json.loads(line.rstrip(',')) for line in exp.split('\n')[1:] if line.startswith('{')]
    dep = Counter(e['group'] for e in entries if 'U01' in e.get('ownerParameters', []))
    check('U01.hashEntriesUsingTheAddress', sum(dep.values()) > 0, groups=dict(dep))


# ------------------------------------------------------------------ U02

def u02():
    doc = own('vectors/u02-revoked-viewer-0.29.json')
    for k, src in doc['sources'].items():
        provenance('U02.source.' + k, src)
    for row in doc['rows']:
        tr = O.view(S, doc['states'][row['state']], row['request'], row['actions'], row.get('policy', 'interstitial'))
        check('U02.row.' + row['id'], norm(tr) == norm(row['expect']), trace=tr, alternative=row.get('alternative', False))
    adopted = [r for r in doc['rows'] if not r.get('alternative')]
    traces = [O.view(S, doc['states'][r['state']], r['request'], r['actions']) for r in adopted]
    revoked_default = [r['id'] for r, t in zip(adopted, traces) if r['request']['kind'] == 'default'
                       and any(x.get('banner') == 'revoked' or doc['states'][r['state']]['status'].get(str(x.get('version'))) == 'revoked' for x in t)]
    check('U02.defaultNeverShowsRevoked', not revoked_default, rows=revoked_default)
    early = [r['id'] for r, t in zip(adopted, traces) for x in t if x['shown'] == 'interstitial' and x['chunkFetches'] != 0]
    check('U02.noChunkFetchBeforeClick', not early, rows=early)
    tm = doc['timing']
    first = O.run_steps(tm['firstLoad'], O.Budget(0), 'content')
    post = O.run_steps(tm['postClickLoad'], O.Budget(tm['clickAtMs']), 'content')
    e = tm['expect']
    check('U02.timing.P-U02-2', first['atMs'] == e['firstVerdictAtMs'] and post['outcome'] == e['postClickOutcome'] and post['atMs'] == e['postClickElapsedMs']
          and post['atMs'] + tm['clickAtMs'] == e['postClickAbsoluteMs'] and tm['clickAtMs'] >= O.DEADLINE_MS,
          first=first, postClick=post, conditional='P-U02-2 pending root decision')


# ------------------------------------------------------------------ U10

def u10():
    doc = own('vectors/u10-publisher-transfer-0.29.json')
    for k, src in doc['sources'].items():
        provenance('U10.source.' + k, src)
    for case in doc['cases']:
        w = O.Website(case['owner'], case['versions'], case['current'])
        bad = []
        for i, st in enumerate(case['steps']):
            r = w.call(*st['call'])
            if r != st['expect']:
                bad.append({'step': i, 'got': r, 'want': st['expect']})
            if 'activeStage' in st and (w.active['stage'] if w.active else None) != st['activeStage']:
                bad.append({'step': i, 'activeStage': w.active})
        fin = w.state()
        for k, v in case['final'].items():
            got = fin[k]
            if isinstance(v, dict):
                if not (isinstance(got, dict) and all(got.get(kk) == vv for kk, vv in v.items())):
                    bad.append({'final': k, 'got': got})
            elif got != v:
                bad.append({'final': k, 'got': got})
        stages = [x['stage'] for x in w.warnings]
        if stages != case['warningStages']:
            bad.append({'warningStages': stages})
        content_ok = all(x['retainedCalls'] == doc['warning']['retainedCalls'] and x['previousOwner'] in x['removal'] for x in w.warnings)
        if not content_ok:
            bad.append({'warningContent': w.warnings})
        check('U10.case.' + case['id'], not bad, mismatches=bad, events=w.events)
        alt = case.get('alternativeAutoRevoke')
        if alt:
            a = O.Website(case['owner'], case['versions'], case['current'], auto_revoke=True)
            got = None
            for i, st in enumerate(case['steps']):
                got = a.call(*st['call'])
                if i == alt['step']:
                    break
            check('U10.rejectedAlternative.' + case['id'], got == alt['expect'] and not a.warnings, got=got, note='comparison only; not adopted')


# ------------------------------------------------------------------ U08

def u08():
    doc = own('vectors/u08-canonical-split-0.29.json')
    for k, src in doc['sources'].items():
        provenance('U08.source.' + k, src)
    for b in doc['boundaries']:
        L, canon = b['len'], b['canonical']
        same02 = [len(p) for p in M.split(b'\x00' * L)] == canon
        rec = {'manifestHash': b'\x00' * 32, 'manifestLen': L, 'chunks': [(bytes(20), x) for x in canon]}
        r = M.validate_version(rec, {})
        data = bytes(i % 251 for i in range(L))
        chunks, prov = M.manifest_chunks(data)
        full = M.validate_version({'manifestHash': M.keccak256(data), 'manifestLen': L, 'chunks': chunks}, prov)
        cells = {'canonical': O.canonical_split(L) == canon and same02,
                 'contractAccepts': O.contract_args(L, len(canon), canon) is None,
                 'viewerSplitPasses': r.get('rule') == 'mfetch.missing' and r.get('fetchCalls') == 1,
                 'viewerManifestStagePasses': full.get('stage') != 'manifest' and full.get('fetchCalls') == len(canon) and len(chunks) == len(canon),
                 'sixKeys': O.proof2_keys(L) == {'chunks': len(canon), 'keys': 6, 'elementSlotsUsed': len(canon), 'zeroProvedElementSlots': 3 - len(canon)}}
        bad = sorted(k for k, v in cells.items() if not v)
        check('U08.boundary.%d' % L, not bad, failingCells=bad, viewer=r, full={k: full.get(k) for k in ('result', 'rule', 'stage', 'fetchCalls')})
    for c in doc['invalidLength']:
        r = M.validate_version({'manifestHash': b'\x00' * 32, 'manifestLen': c['len'], 'chunks': [(bytes(20), x) for x in c['lens']]}, {})
        got = O.contract_args(c['len'], len(c['lens']), c['lens'])
        check('U08.invalidLength.' + c['id'], got == c['contract'] and r.get('rule') == c['viewer'] and r.get('fetchCalls') == 0, contract=got, viewer=r)
    for c in doc['arrays']:
        got = O.contract_args(c['len'], c['nChunks'], c['lens'])
        check('U08.arrays.' + c['id'], got == c['contract'], contract=got)
    for c in doc['noncanonical']:
        lens = c['lens'] if isinstance(c['lens'], list) else [c['lens']['repeat'][0]] * c['lens']['repeat'][1]
        got = O.contract_args(c['len'], len(lens), lens)
        r = M.validate_version({'manifestHash': b'\x00' * 32, 'manifestLen': c['len'], 'chunks': [(bytes(20), x) for x in lens]}, {})
        check('U08.noncanonical.' + c['id'], got == c['contract'] and r.get('rule') == 'manifest.split' and r.get('stage') == 'manifest' and r.get('fetchCalls') == 0
              and sum(lens) == c['len'], contract=got, viewer={k: r.get(k) for k in ('rule', 'stage', 'fetchCalls')}, chunks=len(lens))
    pk = doc['proofKeys']
    ranges, keys_bad = {}, []
    for L in range(1, 65537):
        k = O.proof2_keys(L)
        if k['keys'] != 6 or not 1 <= k['chunks'] <= 3:
            keys_bad.append(L)
        rg = ranges.setdefault('k%d' % k['chunks'], [L, L])
        rg[1] = L
    check('U08.proofKeys.allLengths', not keys_bad and ranges == pk['counts'], ranges=ranges, bad=keys_bad[:5])
    rl = rj('m1-draft-0.4/annex/recv-limits.json')['proofBoundDerivation']
    by = pk['bytes']
    check('U08.proofKeys.bytes', O.proof_bytes(1) == by['k1'] == rl['values']['k1'] and O.proof_bytes(6) == by['k6'] == rl['values']['k6']
          and by['k6'] <= by['limit'] == rl['proposedLimit'] and by['statePool'] == 2 * by['limit'] and rl['round2Keys'] == 6,
          k1=O.proof_bytes(1), k6=O.proof_bytes(6))
    w = pk['withoutU08']
    check('U08.withoutU08Recorded', w['keys'] == 3 + w['chunks'] and w['proofRequestsAt6Keys'] == -(-w['keys'] // 6), note='alternative C budget; recorded only')


# ------------------------------------------------------------------ V1 network and saved transcripts

def v1_network():
    net = rj('m1-draft-0.26/vectors/v1-network.json')
    gp = net['network']['genesisPre']
    segs = copy.deepcopy(rj('m1-draft-0.21/vectors/v3-gsv1.json')['flatSegments'])
    ok_seg = segs[gp['segmentIndex']] == gp['replace']
    segs[gp['segmentIndex']] = gp['with']
    pre = b''.join(bytes.fromhex(s) if isinstance(s, str) else bytes.fromhex(s['repeat']) * s['count'] for s in segs)
    NP = load_module(ROOT / 'm1-draft-0.21/tools/netprofile_ref.py', 'netprofile_021_029')
    spec = NP.decode_genesis(pre)
    cp_lit = {k: num(v) for k, v in net['network']['cp'].items()}
    check('V1rerun.network.genesisPre', ok_seg and len(pre) == 341 and spec['chainId'] == 777002 and all(spec['CP'][k] == v for k, v in cp_lit.items()))
    return {'chainId': spec['chainId'], 'genesisHash': KEC.keccak256(pre), 'forkSchedule': [tuple(x) for x in net['network']['forkSchedule']], 'cp': dict(spec['CP'])}


T27 = None


def transcripts():
    global T27
    if T27 is None:
        T27 = rj('coordination/review-001/m1-draft-0.27/results/v1-transcripts-0.27.json')
    return T27


def replay_set():
    """(label, transcript, clock, t0) for every reviewed 0.27 transcript."""
    t = transcripts()
    cases26 = {c['id']: c for c in rj('m1-draft-0.26/vectors/v1-window-cases.json')['cases']}
    sha27 = {c['id']: c for c in rj('m1-draft-0.27/vectors/v1-sha-accounting.json')['newCases']}
    out = []
    for tr in t['carried']:
        ph = cases26[tr['case']]['phases'][tr['phase']]
        out.append(('carried.%s#%d' % (tr['case'], tr['phase']), tr, ph['clock'], ph.get('t', 0)))
    for tr in t['c32']:
        out.append(('c32.' + tr['case'], tr, 1700000205, 0))
    for tr in t['sha']:
        out.append(('sha.' + tr['case'], tr, sha27[tr['case']]['clock'], 0))
    return out


def kind_of(verdict):
    return 'ok' if verdict.get('ok') is True else verdict.get('rule')


def expected_profile(times, t0, L, C, saved_kind):
    """Independent arithmetic over the saved send times (sleeps only, latency 0): the whole-load rule with latency L per
    request and C ms of computation. Does not use overlay_ref_029."""
    rel = [x - t0 for x in times]
    t, D = 0, 10000
    for i in range(len(rel)):
        if t + L >= D:
            return ('viewIncomplete', D, 'expired')
        t += L
        if i + 1 < len(rel):
            s = rel[i + 1] - rel[i]
            if s > 0 and t + s >= D:
                return ('viewIncomplete', t, 'retryDelayDoesNotFit')
            t += s
    if t + C >= D:
        return ('viewIncomplete', D, 'expired')
    return (saved_kind, t + C, None)


# ------------------------------------------------------------------ U14 / CR-M1-01 / P-C32-2 virtual deadline

def deadline(cfg):
    doc = own('vectors/deadline-cases-0.29.json')
    for k, src in doc['sources'].items():
        provenance('U14.source.' + k, src)
    c32 = {t['case']: t for t in transcripts()['c32']}
    base = c32['C32-ok-busy250']
    bn20, hdr20 = base['replies'][0], base['replies'][-1]
    check('U14.tokens.fromReviewedTranscript', base['outcome'].get('ok') is True and base['outcome'].get('n') == 13
          and json.loads(bn20).get('result') == '0x14', saved={k: base['outcome'].get(k) for k in ('ok', 'n')})
    tok = doc['hnc']['tokens']

    def text(t):
        if t == 'BN20':
            return bn20
        if t == 'HDR20':
            return hdr20
        if ':' in t:
            k, ms = t.split(':')
            return tok[k + ':<ms>'].replace('<ms>', ms)
        return tok[t]

    for c in doc['hnc']['cases']:
        script = list(zip(c['lat'], [text(x) for x in c['replies']]))
        r = O.header_net_check(V, cfg, script, doc['hnc']['clock'], c['computeMs'])
        got = {'outcome': kind_of(r['verdict']), 'atMs': r['atMs'], 'why': r['why'], 'sleptMs': r['sleptMs'], 'requests': len(r['requests'])}
        extra = (r['verdict'].get('n') == 13 and r['verdict'].get('frame') is True) if got['outcome'] == 'ok' else (r['verdict'].get('frame') is False)
        check('U14.hnc.' + c['id'], got == c['expect'] and extra, got=got, tag=c['tag'], virtualTime=True)
    for c in doc['content']['cases']:
        if 'expectError' in c:
            try:
                O.run_steps(c['steps'], O.Budget(0), 'content')
                err = None
            except ValueError as e:
                err = str(e)
            check('U14.content.' + c['id'], err == c['expectError'], error=err)
            continue
        r = O.run_steps(c['steps'], O.Budget(0), 'content')
        got = {k: r[k] for k in c['expect']}
        check('U14.content.' + c['id'], got == c['expect'], got=got, tag=c['tag'], virtualTime=True)
    for c in doc['rp']['cases']:
        for mode, want in c['expect'].items():
            out = O.run_ops(c['ops'], mode)
            got = [{'outcome': o['outcome'], 'absoluteAtMs': o['absoluteAtMs']} for o in out]
            check('U14.rp.%s.%s' % (c['id'], mode), got == want, got=got, convention='P-D29-3 open: both readings are fixtures')
    u28 = rj('m1-draft-0.28/vectors/u14-timing-cases.json')
    root = {(x['case'], x['mode']): x['result'] for x in rj('coordination/review-001/u14-independent-0.28.json')['results']}
    b = doc['bind028']
    for c in u28['cases']:
        steps = [{'r': 'eth_blockNumber', 'replies': [[c['latency'][0], 'ok']], 'retry': False},
                 {'r': 'pocol_getHeaders', 'replies': [[c['latency'][i + 1], rep] for i, rep in enumerate(c['replies'])]}, {'c': c['computeMs']}]
        r = O.run_steps(steps, O.Budget(0), 'hnc')
        if c['id'] in b['effectiveEqualsWLa']:
            wl = root[(c['id'], 'WL_a')]
            check('U14.bind028.' + c['id'], [r['outcome'], r['atMs']] == [wl['outcome'], wl['atMs']], effective=r, rootWLa=wl)
        else:
            want = b['supersededPolicyC'][c['id']]
            wl = root[(c['id'], 'WL_a')]
            check('U14.bind028.' + c['id'], [r['outcome'], r['atMs']] == [want['outcome'], want['atMs']] and [wl['outcome'], wl['atMs']] != [want['outcome'], want['atMs']],
                  effective=r, rootWLaUnderPolicyC=wl, note='policy C not adopted (P-C32-2 A)')
    check('U14.sleepBoundIsNotElapsedBound', any(c['expect']['sleptMs'] <= 6000 and c['expect']['why'] == 'expired' for c in doc['hnc']['cases']))
    bud(doc)


def bud(doc):
    base = {c['id']: c['expected'] for c in rj('m1-draft-0.5/vectors/proof-response-cases.json')['budget']['cases']}
    for c in doc['bud']['cases']:
        steps = c['steps']
        if steps == '@worst':
            steps = []
            for _ in range(4):
                steps += [{'r': 'anchor', 'replies': [[0, 'rate:0']] * 3 + [[0, 'ok']]}, {'r': 'proof1', 'replies': [[0, 'rate:0']] * 3 + [[0, 'ok']]},
                          {'r': 'proof2', 'replies': [[0, 'rate:0']] * 4}]
        r = O.run_steps(steps, O.Budget(0), 'content')
        got = {k: r[k] for k in c['expect']}
        old = base[c['id']]
        mapped = {'outcome': old['result'], 'sends': old['sends'], 'sleptMs': old.get('delayMs'), 'attempts': old['restarts'] + 1}
        same = all(mapped[k] == c['expect'][k] for k in mapped if mapped[k] is not None)
        check('CRM101.bud.' + c['id'], got == c['expect'] and same != c['changed'], got=got, baseline0_5=old, changedByOverlay=c['changed'])


def crm101():
    d = REGD['CR-M1-01']
    rl = rj('m1-draft-0.4/annex/recv-limits.json')['proofBoundDerivation']
    check('CRM101.replyLimitUnchanged', O.proof_bytes(6) == rl['values']['k6'] <= rl['proposedLimit'] == 524288 and '524288' in d['effective'])
    check('CRM101.statePoolUnchanged', 2 * 524288 == 1048576 and 'STATE_POOL 1 MiB' in d['effective'])
    check('CRM101.sitesStillForbidden', 'sites still forbidden' in d['effective'])
    evidence('CRM101.siteBanEvidence', 'R04', '^bridge\\.denyList')
    check('CRM101.sendsPerLoad', 4 * 3 * (1 + 3) == 48 and contains('m1-draft-0.5/CR-M1-01-REV2.md', '**48**'))
    check('CRM101.worstBytesBound', 16 * 167936 + 32 * 524288 == 19464192 and contains('m1-draft-0.5/CR-M1-01-REV2.md', '19464192'))
    ex = own('vectors/experiment-amendments-0.29.json')
    e03 = next((e for e in ex['amended'] if e['id'] == 'E03'), {})
    check('CRM101.E03Amended', e03.get('definitionComplete') is True and 'MPT-15' in e03.get('replacedPass', {}) and e03.get('claims', '').startswith('none'))
    check('CRM101.supersededTextsPreserved', contains('m1-draft-0.5/CR-M1-01-REV2.md', '| Cumulative wait per load | ≤ 20000 ms (P)')
          and contains('m1-draft-0.5/CR-M1-01-REV2.md', '| Single wait | 0–10000 ms (P) |')
          and 'the 20000 ms cumulative wait are superseded' in d['effective'] and 'E03 stays a later experiment' in d['effective'])
    have = {r['check']: r['status'] for r in results}
    content = [k for k in have if k.startswith('U14.content.') or k.startswith('CRM101.bud.')]
    check('CRM101.virtualTimeFixtures', bool(content) and all(have[k] == 'pass' for k in content), fixtures=len(content))


# ------------------------------------------------------------------ V1 rerun under the whole deadline

def v1_rerun(cfg):
    doc = own('vectors/deadline-cases-0.29.json')['rerunProfiles']
    summary = {}
    for p in doc['profiles']:
        cnt = Counter()
        for label, tr, clock, t0 in replay_set():
            script = [(p['latencyMs'], x) for x in tr['replies']]
            try:
                r = O.header_net_check(V, cfg, script, clock, p['computeMs'], t0)
                exc = None
            except Exception as e:                                        # must never happen
                r, exc = None, '%s: %s' % (type(e).__name__, e)
            name = 'V1rerun.%s.%s' % (p['id'], label)
            if r is None:
                check(name, False, exception=exc)
                continue
            saved_kind = kind_of(tr['outcome'])
            if p['id'] == 'zero':
                cells = {'verdict': norm(r['verdict']) == norm(tr['outcome']),
                         'requests': norm(r['requests']) == norm(tr['requests']),
                         'atNoDeadline': r['why'] is None}
                if 'counters' in tr and label.startswith('sha.'):
                    cd = r['counters'].as_dict()
                    cells['shaCounters'] = all(cd[k] == tr['counters'][k] for k in ('sha256ByCategory', 'sha256Total', 'sha256UniquePreimages', 'powHash', 'shareHash'))
                bad = sorted(k for k, v in cells.items() if not v)
                check(name, not bad, failingCells=bad, outcome=saved_kind)
            else:
                want = expected_profile([q['t'] for q in tr['requests']], t0, p['latencyMs'], p['computeMs'], saved_kind)
                got = (kind_of(r['verdict']), r['atMs'], r['why'])
                check(name, got == want, got=list(got), want=list(want))
            cnt[(kind_of(r['verdict']), r['why'])] += 1
        summary[p['id']] = {'%s/%s' % k: v for k, v in sorted(cnt.items(), key=str)}
    record('V1rerun.summary', 'recorded', profiles=summary)
    keep = all(k.split('/')[1] == 'None' for k in summary.get('u700', {}))
    check('V1rerun.u700KeepsEverySavedVerdict', keep and summary.get('u700') is not None, summary=summary.get('u700'))
    check('V1rerun.u1600HasExpiries', any(k.endswith('/expired') for k in summary.get('u1600', {})), summary=summary.get('u1600'))
    return summary


# ------------------------------------------------------------------ V1-SHA-COST and P-C32-2

class RecordingCounters(V.Counters):
    def __init__(self):
        super().__init__()
        self.calls = []

    def sha256(self, category, data):
        d = super().sha256(category, data)
        self.calls.append((category, data, d))
        return d


def sha29(cfg):
    lim = REGD['V1-SHA-COST']['limits']
    tr = next(t for t in transcripts()['sha'] if t['case'] == 'SHA-W')
    script = [(0, x) for x in tr['replies']]
    rc = RecordingCounters()
    r = O.header_net_check(V, cfg, script, 1700000205, 0, ctr=rc)
    cd = rc.as_dict()
    check('SHA29.worstCase.cachedByCategory', cd['sha256ByCategory'] == {'templateId': lim['templateId'], 'powHash': lim['powHash'], 'shareHash': lim['shareHash']}
          and cd['sha256Total'] == lim['sha256Total'] == 3355 and cd['sha256UniquePreimages'] == 3355, counters=cd)
    check('SHA29.worstCase.verdict', r['verdict'].get('ok') is True and r['verdict'].get('n') == 13 and r['why'] is None)
    sc = tr['counters']
    check('SHA29.worstCase.otherBounds', cd['headerDecodes'] == sc['headerDecodes'] <= 14 and cd['asert'] == sc['asert'] <= 13 and cd['maxAsertBits'] <= 512
          and cd['ecrecover'] == sc['ecrecover'], counters=cd, saved={k: sc.get(k) for k in ('headerDecodes', 'asert', 'ecrecover')})
    steps = ['netid', 'viewFuture', 'item2', 'item3', 'item4', 'asert', 'viewTargetCeil', 'item6', 'powHash', 'item8', 'item9']
    ev = Counter((e[0], e[1]) for e in rc.events)
    missing = [[s, h] for h in range(8, 21) for s in steps if ev[(s, h)] < 1]
    check('SHA29.everyCheckRetained', not missing, missing=missing[:10])
    ru = V.Counters()
    r2 = O.header_net_check(V, cfg, script, 1700000205, 0, cache=False, ctr=ru)
    cu = ru.as_dict()
    check('SHA29.uncachedIs6721', cu['sha256Total'] == 6721 and cu['sha256UniquePreimages'] == 3355 and norm(r2['verdict']) == norm(r['verdict']), uncached=cu['sha256Total'])
    check('SHA29.cachingRequiredForBudget', cu['sha256Total'] > lim['sha256Total'] >= cd['sha256Total'])
    f27 = rj('coordination/review-001/m1-draft-0.27/results/hash-freeze-v1-0.27.json')['list']
    f26 = rj('coordination/review-001/m1-draft-0.26/results/hash-freeze-v1-0.26.json')['list']
    v27 = {x['id'] for x in rj('coordination/review-001/v1-hash-independent-0.27.json')['results'] if x.get('pass') is True}
    v26 = {x['id'] for x in rj('coordination/review-001/v1-independent-hashes-0.26.json')['results'] if x.get('pass') is True}
    idx = {}
    for lst, ver, tag in ((f26, v26, '0.26'), (f27, v27, '0.27')):
        for e in lst:
            if e.get('algorithm') == 'sha256':
                idx[e['preimageHex']] = (e['id'], e['digest'], e['id'] in ver, tag)
    bound, unbound = Counter(), []
    seen = set()
    for cat, data, dig in rc.calls:
        if data in seen:
            continue
        seen.add(data)
        hit = idx.get(data.hex())
        if hit and hit[1] == dig.hex() and hit[2]:
            bound[(cat, hit[3])] += 1
        else:
            unbound.append([cat, data.hex()[:32], hit[0] if hit else None])
    check('SHA29.preimageBinding', not unbound and len(seen) == 3355, bound={'%s@%s' % k: v for k, v in sorted(bound.items())}, unbound=unbound[:10],
          note='every distinct SHA-256 preimage of the worst case equals an exported freeze entry whose id root verified independently')
    t_sha = sha(ROOT / 'coordination/review-001/m1-draft-0.27/results/v1-transcripts-0.27.json')
    record('SHA29.worstCaseFixtureHash', 'recorded', file='coordination/review-001/m1-draft-0.27/results/v1-transcripts-0.27.json', sha256=t_sha,
           replySha256=[hashlib.sha256(x.encode('utf-8')).hexdigest() for x in tr['replies']])
    roots = [('v1-sha-independent-0.27', {'checks': 18, 'passed': 18, 'failed': 0}), ('v1-hash-independent-0.27', {'checks': 3677, 'passed': 3677, 'failed': 0}),
             ('v1-signatures-independent-0.27', {'checks': 31, 'failed': 0, 'noSigningOrSubmission': True}),
             ('asert-native-comparison-0.27', {'checks': 1027, 'passed': 1027, 'failed': 0}), ('v1-independent-hashes-0.26', {'checks': 347, 'passed': 347, 'failed': 0})]
    for name, want in roots:
        rel = 'coordination/review-001/%s.json' % name
        d = rj(rel)
        check('SHA29.rootEvidence.' + name, all(d.get(k) == v for k, v in want.items()), sha256=sha(ROOT / rel), summary={k: d.get(k) for k in want})
    probe = rj('coordination/review-001/v1-share-work-probe-0.26.json')
    s1 = next(t for t in transcripts()['sha'] if t['case'] == 'SHA-S1')
    check('SHA29.singleHeaderMatchesRootProbe', s1['counters']['sha256Total'] == probe['totalSha256'] == 258)
    line = (ROOT / 'reference/browser.md').read_text(encoding='utf-8').splitlines()[57]
    check('SHA29.sourceLiteralPreservedAsHistory', '≤ 13 SHA-256' in line and 'reference/browser.md:58' in REGD['V1-SHA-COST']['superseded'], line=line)
    check('SHA29.effectiveSentence', '14 TemplateID + 13 PoW + 3328 share hashes' in REGD['V1-SHA-COST']['effective']
          and 'Every check of items 1-9 is retained' in REGD['V1-SHA-COST']['effective'])


def pc322():
    d = REGD['P-C32-2']
    want = {'busy': tuple(d['busyDelayMs']), 'rate': tuple(d['rateDelayMs'])}
    check('PC322.rangesInCheckerAndModel', V.RETRY_RANGE == want == O.RETRY_RANGE and V.MAX_RETRIES == d['maxRetries'] == O.MAX_RETRIES,
          checker=V.RETRY_RANGE, model=O.RETRY_RANGE)
    cases = [('rate', 0, 'retry'), ('rate', 2000, 'retry'), ('rate', 2001, 'malformed'), ('rate', -1, 'malformed'),
             ('busy', 249, 'malformed'), ('busy', 250, 'retry'), ('busy', 2000, 'retry'), ('busy', 2001, 'malformed')]
    bad = []
    for reason, ms, kind in cases:
        t = json.dumps({'jsonrpc': '2.0', 'id': 2, 'error': {'code': -32021, 'message': reason, 'data': {'reason': reason, 'retryAfterMs': ms}}})
        if V.classify_reply(t)[0] != kind:
            bad.append([reason, ms])
    check('PC322.classifyBounds', not bad, bad=bad)
    have = {r['check']: r['status'] for r in results}
    c32 = [k for k in have if k.startswith('V1rerun.zero.c32.')]
    check('PC322.c32CasesUnderDeadline', len(c32) == 41 and all(have[k] == 'pass' for k in c32), cases=len(c32))
    check('PC322.noSourceRangeClaim', 'not a source range' in d['effective'] and 'no claim is made that every honest server' in d['effective'])


# ------------------------------------------------------------------ RF-E6-1

def rfe61():
    doc = own('vectors/rfe61-x12b-binding-0.29.json')
    provenance('RFE61.source', doc['source'])
    vec = rj(doc['vector']['file'])
    x12b = next(m for m in vec['modes'] if m['id'] == doc['vector']['mode'])
    check('RFE61.vector', x12b['adminStaleDropped'] == doc['vector']['adminStaleDropped'] and x12b['staleDroppedByOp'] == doc['vector']['staleDroppedByOp']
          and any(c['t'] == doc['vector']['derivedCell'] and c['tag'] == 'derived' for c in vec['cells']))
    res = {r['check']: r for r in rj(doc['results']['file'])['results']}
    inp = res.get(doc['results']['inputRecord'], {})
    check('RFE61.executedVectorIsCurrentFile', inp.get('sha256') is not None and inp.get('sha256') == sha(ROOT / doc['vector']['file']), recorded=inp.get('sha256'))
    for b in doc['bindings']:
        e = res.get(b['check'])
        ok = e is not None and e.get('status') == b['status']
        if ok and 'actual' in b:
            ok = e.get('actual') == b['actual']
        if ok and 'fifo' in b:
            ok = e.get('fifo') == b['fifo'] and e.get('trace') == b['trace']
        if ok and 'contains' in b:
            ok = all(x in e.get('actual', []) for x in b['contains'])
        check('RFE61.bound.' + b['check'], ok, saved={k: e.get(k) for k in ('status', 'actual', 'fifo', 'trace')} if e else None)
    for g in doc['historicalGaps']:
        e = res.get(g['check'])
        check('RFE61.historicalGapRebound.' + g['check'], e is not None and e.get('status') == g['status'] and ('model' not in g or e.get('model') == g['model']),
              reboundTo=g['reboundTo'])
    rv = (ROOT / doc['results']['review']['file']).read_text(encoding='utf-8')
    check('RFE61.rootReview0_18', all(x in rv for x in doc['results']['review']['literals']))
    check('RFE61.effectiveText', 'adminStaleDropped = 2 (op1: 1, op2: 1)' in doc['effectiveText'] and '5001' in doc['effectiveText'])


# ------------------------------------------------------------------ evidence and history

def history():
    for s in INV25['supersededFailures']:
        hit = match(s['results'], '^' + re.escape(s['check']) + '$') or []
        closure = match(*s['closure']) or []
        ok = len(hit) == 1 and hit[0].get('status') == 'FAIL' and any(r.get('status') == 'pass' for r in closure) and not any(r.get('status') == 'FAIL' for r in closure)
        if ok:
            SUPERSEDED_OK.add((s['results'], s['check']))
    for h in INV['historicalFailures']['entries']:
        if 'rootProbe' in h:
            d = rj(h['rootProbe'])
            still = d.get('failed') == h['failed'] and sorted(x.get('check') for x in d.get('results', [])) == sorted(h['checks'])
            cp = rj(h['closureProbe']['file'])
            probe_ok = cp.get('checks') == h['closureProbe']['checks'] and cp.get('failed') == h['closureProbe']['failed']
        else:
            res = results_of(h['results']) or []
            st = {r['check']: r.get('status') for r in res if type(r.get('check')) is str}
            still = all(st.get(c) == 'FAIL' for c in h['checks'])
            probe_ok = True
        clos = [match(k, p) or [] for k, p in h['closure']]
        closed = all(any(r.get('status') == 'pass' for r in c) and not any(r.get('status') == 'FAIL' for r in c) for c in clos)
        check('history.%s' % h['id'], still and closed and probe_ok, stillRecordedAsFail=still, closedBy=h['closure'], issue=h['issue'])
    for dc in INV['historicalFailures']['documentCorrections']:
        clos = [match(k, p) or [] for k, p in dc['closure']]
        check('history.%s' % dc['id'], all(c and all(r.get('status') == 'pass' for r in c) for c in clos), what=dc['what'])
    for s in INV['savedSummaries']:
        d = json.loads((ROOT / RESULT_FILES[s['key']]).read_text(encoding='utf-8'))['summary']
        check('evidence.savedSummary.' + s['key'], [d['checks'], d['passed'], d['recorded'], d['failed']] == [s['checks'], s['passed'], s['recorded'], s['failed']], summary=d)
    for rv in INV['reviewed']['reviews']:
        p = ROOT / rv['file']
        check('evidence.review.' + Path(rv['file']).stem, p.exists() and ('literal' not in rv or rv['literal'] in p.read_text(encoding='utf-8')))
    for ev in CF28['independent']:
        d = rj(ev['file'])
        ok = d.get('checks') == ev['checks'] and d.get('failed') == ev['failed'] and ('passed' not in ev or d.get('passed') == ev['passed'])
        check('evidence.independent.' + Path(ev['file']).stem, ok, sha256=sha(ROOT / ev['file']))
    u = rj('coordination/review-001/u14-independent-0.28.json')
    check('evidence.independent.u14-independent-0.28', u['checks'] == 21 and u['passed'] == 21 and u['failed'] == 0)
    for n in range(2, 29):
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
    reg = rj('m1-draft-0.25/audit/gap-registry.json')
    for g in INV['gapReclassification']:
        check('evidence.gapReclassified.' + g['pattern'], any(e['pattern'] == g['pattern'] and e['class'] == g['was'] for e in reg['entries']), now=g['now'])


# ------------------------------------------------------------------ decisions applied, hash groups

def decisions_applied():
    have = [(r['check'], r['status']) for r in results]
    out = {}
    for d in REG['delegated']:
        pats = d['checks'] + ['^authority\\.exact\\.%s$' % re.escape(d['id']), '^source\\.%s\\.' % re.escape(d['id'])]
        mine = [(c, s) for c, s in have if any(re.search(p, c) for p in pats)]
        own_checks = [(c, s) for c, s in mine if any(re.search(p, c) for p in d['checks'])]
        ok = bool(own_checks) and any(s == 'pass' for _, s in own_checks) and all(s != 'FAIL' for _, s in mine)
        if ok:
            APPLIED.add(d['id'])
        out[d['id']] = ok
        check('decision.%s.applied' % d['id'], ok, checks=len(mine), failed=[c for c, s in mine if s == 'FAIL'][:10],
              basis='delegated technical decision (user standing authorization + coordinator decision); not an owner answer')
    return out


def hash_groups():
    exp = (ROOT / 'coordination/review-001/m1-draft-0.25/results/hash-freeze-export-0.25.json').read_text(encoding='utf-8')
    entries = [json.loads(line.rstrip(',')) for line in exp.split('\n')[1:] if line.startswith('{')]
    rootv = {x['id']: x for x in rj('coordination/review-001/e05-new-three-library-0.25.json')['results']}
    groups, bad = {}, []
    for e in entries:
        g = groups.setdefault(e['group'], {'entries': 0, 'blockedBy': [], 'resolvedByDelegation': [], 'pending': False})
        g['entries'] += 1
        for p in e.get('ownerParameters', []):
            tgt = 'resolvedByDelegation' if TOKEN.get(p, p) in APPLIED else 'blockedBy'
            item = p if tgt == 'resolvedByDelegation' else {'type': 'owner', 'id': p}
            if item not in g[tgt]:
                g[tgt].append(item)
        for p in e.get('reviewerParameters', []):
            if TOKEN.get(p, p) in APPLIED:
                if p not in g['resolvedByDelegation']:
                    g['resolvedByDelegation'].append(p)
                continue
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
    check('hash29.rootVerified025', not bad, unverified=bad)
    for a, t in (('alias.versionRecords', 'triad.manifests'), ('alias.t1_02.manifestHash', 'triad.manifests'), ('alias.X1.netKey', 'triad.netKey')):
        groups[a] = dict(groups.get(t, {'blockedBy': [], 'resolvedByDelegation': [], 'pending': True}), aliasOf=t)
    for s_ in ('syn.lc.branchHashes', 'privateProvenance', 'ph.v3.networkProfiles'):
        groups[s_] = {'blockedBy': [], 'resolvedByDelegation': [], 'pending': False, 'class': 'synthetic/private/placeholder'}
    v1 = {}
    for tag, rel, ind in (('0.26', 'coordination/review-001/m1-draft-0.26/results/hash-freeze-v1-0.26.json', 'coordination/review-001/v1-independent-hashes-0.26.json'),
                          ('0.27', 'coordination/review-001/m1-draft-0.27/results/hash-freeze-v1-0.27.json', 'coordination/review-001/v1-hash-independent-0.27.json')):
        lst = rj(rel)['list']
        ver = {x['id'] for x in rj(ind)['results'] if x.get('pass') is True}
        unv = [e['id'] for e in lst if e['id'] not in ver]
        check('hash29.v1Freeze' + tag.replace('.', '_') + 'RootVerified', not unv, entries=len(lst), unverified=unv[:10], file=rel, sha256=sha(ROOT / rel))
        for e in lst:
            gg = v1.setdefault(e['group'], {'entries': 0, 'rootVerified': 0, 'freezes': []})
            gg['entries'] += 1
            gg['rootVerified'] += e['id'] in ver
            if tag not in gg['freezes']:
                gg['freezes'].append(tag)
    owner_left = sorted(g for g, v in groups.items() if any(b['type'] == 'owner' for b in v['blockedBy']))
    check('hash29.noOwnerBlockedGroup', not owner_left, ownerBlocked=owner_left)
    return groups, v1


# ------------------------------------------------------------------ consolidation

def consolidate(groups):
    have = {r['check']: r['status'] for r in results}
    supp_pattern = {'S25-SWEEPMAX': '^S25\\.sweepMax\\.', 'S25-E07': '^S25\\.e07\\.', 'S25-LCLIT': '^S25\\.lclit\\.', 'S25-RG3B': '^S25\\.rg3b\\.'}
    old = {}
    for src in (CONS26['rowChanges'], CONS27['rowChanges']):
        for k, v in src.items():
            old.setdefault(v.get('row', k), []).append(v)
    matrix = {}
    for row in INV25['rows']:
        rid = row['id']
        ch, new = old.get(rid, []), INV['rowChanges'].get(rid, {})
        owner = []
        for t in list(row['owner']) + [t for c in ch for t in c.get('addOwner', [])] + CF28['rows']['ownerBlocked'].get(rid, []):
            t = TOKEN.get(t, t)
            if t not in owner:
                owner.append(t)
        resolved = [t for t in owner if t in APPLIED and t in [TOKEN.get(x, x) for x in new.get('resolveOwner', [])]]
        own_open = [t for t in owner if t not in resolved]
        removed = set(new.get('removeReviewer', [])) | {t for c in ch for t in c.get('removeReviewer', [])}
        reviewer = [t for t in list(row['reviewer']) + [t for c in ch for t in c.get('addReviewer', [])] if t not in removed] + new.get('addReviewer', [])
        reviewer = list(dict.fromkeys(reviewer))
        missing = [s['file'] for s in row['spec'] if not path_of(s['file']).exists()]
        missing += [s['file'] for s in row['spec'] if s.get('contains') and path_of(s['file']).exists() and not contains(s['file'], s['contains'])]
        missing += [f for f in row['fixtures'] + row['review'] if not path_of(f).exists()]
        missing += [f for c in ch + [new] for f in c.get('addSpec', []) + c.get('addFixtures', []) if not path_of(f).exists()]
        ev = [evidence('row.%s.evidence.%s:%s' % (rid, k, p), k, p) for k, p in row['evidence']]
        ev += [evidence('row.%s.evidence.%s:%s' % (rid, k, p), k, p) for c in ch for k, p in c.get('addEvidence', [])]
        ev += [evidence('row.%s.evidence.%s:%s' % (rid, k, p), k, p) for k, p in INV['reviewed']['reboundEvidence'].get(rid, [])]
        cur_ok = True
        for p in new.get('currentRunEvidence', []):
            mine = [s for k, s in have.items() if re.search(p, k)]
            cur_ok = cur_ok and bool(mine) and all(s != 'FAIL' for s in mine)
        pending = bool(new.get('pendingRootReview'))
        resolved_now = [m for c in ch for m in c.get('resolved', [])]
        unresolved = []
        for m in row['missing']:
            if m.get('resolvedBy') in supp_pattern:
                if not evidence('row.%s.supplement.%s' % (rid, m['resolvedBy']), 'R25', supp_pattern[m['resolvedBy']]):
                    unresolved.append(m['id'])
            elif m['id'] not in resolved_now:
                unresolved.append(m['id'])
        rev_open = [t for t in reviewer if t not in ACCEPTED]
        hash_owner = [g for g in row['hashGroups'] if any(b['type'] == 'owner' for b in groups.get(g, {}).get('blockedBy', []))]
        hash_pending = any(groups.get(g, {}).get('pending') for g in row['hashGroups'])
        c1 = 'blocked' if (missing or unresolved) else ('pendingRootReview' if pending else 'satisfied')
        c2 = 'blocked' if not (all(ev) and cur_ok) else ('pendingRootReview' if pending else 'satisfied')
        c3 = 'pendingRootReview' if pending else 'satisfied'
        c4 = 'blocked' if own_open else ('pendingReviewerDecision' if rev_open else 'satisfied')
        c5 = 'blocked' if (own_open or unresolved or hash_owner) else ('pendingRootReview' if (pending or hash_pending) else 'satisfied')
        crit = {'c1': c1, 'c2': c2, 'c3': c3, 'c4': c4, 'c5': c5}
        if all(v == 'satisfied' for v in crit.values()):
            st = 'CompleteCandidate'
        elif all(v in ('satisfied', 'pendingRootReview') for v in crit.values()):
            st = 'PendingRootReview'
        else:
            st = 'Partial'
        matrix[rid] = {'title': row['title'], 'source': row['source'], 'status': st, 'criteria': crit,
                       'c4Basis': ('delegated: ' + ', '.join(resolved)) if resolved else None,
                       'ownerOpen': own_open, 'reviewerOpen': rev_open, 'unresolved': unresolved, 'hashOwnerBlocked': hash_owner, 'missing': missing,
                       'phaseA': row.get('phaseA', []), 'experiments': row.get('experiments', [])}
        record('row.%s.criteria' % rid, 'recorded', **{k: v for k, v in matrix[rid].items() if k not in ('title', 'source')})
    prop = INV['proposedStatus']
    want = {r: s for s, rows in prop.items() for r in rows}
    computed = {r: v['status'] for r, v in matrix.items()}
    check('consolidation.rowsMatchProposal', computed == want and len(computed) == 41,
          differing={r: [computed.get(r), want.get(r)] for r in set(computed) | set(want) if computed.get(r) != want.get(r)})
    record('consolidation.summary', 'recorded', counts=dict(Counter(computed.values())), rows=computed)
    return matrix


def findings():
    base = rj('m1-draft-0.26/audit/findings-trace-0.26.json')['findings']
    out = {}
    for f in base:
        dep = [TOKEN.get(t, t) for t in f['dependsOn']] + INV['findingChanges'].get(f['id'], {}).get('addDepends', [])
        delegated = [t for t in dep if t in REGD]
        open_owner = [t for t in delegated if t not in APPLIED]
        pend = [t for t in dep if t not in REGD and t not in ACCEPTED]
        if open_owner:
            disp = 'openOwner'
        elif pend:
            disp = 'pendingReviewerDecision'
        elif delegated:
            disp = 'closableAfterRootReviewOfOverlay'
        else:
            disp = 'closableAtSpecScope'
        out[f['id']] = {'disposition': disp, 'dependsOn': dep, 'delegated': delegated, 'pendingReviewer': pend, 'phaseA': f.get('phaseA', [])}
        record('finding.' + f['id'], 'recorded', **out[f['id']])
    want = {f: d for d, fs in INV['proposedFindings'].items() for f in fs}
    got = {f: v['disposition'] for f, v in out.items()}
    check('consolidation.findingsMatchProposal', got == want and len(got) == 26, differing={f: [got.get(f), want.get(f)] for f in set(got) | set(want) if got.get(f) != want.get(f)})
    return out


def experiment_definitions():
    s25 = {e['id']: e for e in rj('m1-draft-0.25/supplements/s25-experiment-specs.json')['experiments']}
    ex = own('vectors/experiment-amendments-0.29.json')
    am = {e['id']: e for e in ex['amended'] + ex['new']}
    missing = []
    for eid in INV['experimentDefinitions']:
        a = am.get(eid)
        b = s25.get(eid, {}).get('definitionComplete')
        ok = (a is not None and a.get('definitionComplete') is True) or b is True \
            or (isinstance(b, str) and b.startswith('complete for') and 'except' not in b and 'conditional' not in b)
        if not ok:
            missing.append(eid)
    need = sorted({e for r in INV25['rows'] for e in r.get('experiments', [])} | {'X-U14'})
    check('experiments.definitionsComplete', not missing and all(e in INV['experimentDefinitions'] for e in need), missing=missing, referenced=need)
    return missing


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


ROW_AR = {'CompleteCandidate': 'مرشح للاكتمال', 'PendingRootReview': 'بانتظار مراجعة الجذر فقط', 'Partial': 'جزئي'}


def dashboard(matrix, finds, missing_exp, failed):
    cnt = Counter(v['status'] for v in matrix.values())
    pend = [p['id'] for p in REG['reviewer']['pendingRootDecision']]
    lines = [
        'M1، المسودة 0.29: تطبيق تسعة قرارات تقنية مفوضة في طبقة مواصفات جديدة.',
        'هذه القرارات اتخذها المنسق بتفويض دائم من المستخدم. ليست موافقة شخصية من المالك، وأسئلة المالك المحفوظة ما تزال بلا جواب.',
        'نتيجة هذا التشغيل: %d فحصًا، فشل منها %d.' % (len(results), len(failed)),
        'الصفوف الواحد والأربعون: %d مرشح للاكتمال، و%d بانتظار مراجعة الجذر فقط، و%d جزئي.' % (cnt['CompleteCandidate'], cnt['PendingRootReview'], cnt['Partial']),
        'لا يكتمل أي صف قبل مراجعة الجذر. «مرشح للاكتمال» لا يعني «مكتمل».',
        'أعراف تنتظر قرار الجذر (%d): %s.' % (len(pend), '، '.join(pend)),
        'تعريفات تجارب ناقصة: %s.' % ('لا يوجد' if not missing_exp else '، '.join(missing_exp)),
        'المهلة: عشر ثوانٍ من أول طلب حتى الحكم النهائي، وعند بلوغ 10000 ملي ثانية تنتهي المهلة. النتائج هنا زمن افتراضي محسوب، وليست قياسًا في المتصفح.',
        'كلفة SHA-256 الفعالة: 14 لـTemplateID و13 لـPoW و3328 للحصص، والمجموع 3355، مع إبقاء كل الفحوص.',
        'عنوان المصنع 0x…c0c005 عنوان مرجعي لاختبارات M1 فقط، ولا يؤكد أي نشر.',
        'لا تنفيذ إنتاجي ولا نشر ولا معاملات.'
    ]
    rows = [{'id': r, 'statusAr': ROW_AR[v['status']], 'status': v['status'],
             'waitingFor': v['reviewerOpen'] + v['ownerOpen'] + (['مراجعة الجذر'] if 'pendingRootReview' in v['criteria'].values() else [])}
            for r, v in matrix.items() if v['status'] != 'CompleteCandidate']
    return {'schema': 'pocol-m1-dashboard-ar/0.29', 'lang': 'ar', 'lines': lines, 'rowsNotYetCandidate': rows,
            'findings': dict(Counter(v['disposition'] for v in finds.values()))}


def outputs(matrix, finds, groups, v1groups, rerun, missing_exp):
    reg = {'schema': 'pocol-m1-decision-register/0.29', 'authority': REG['authority'],
           'delegated': [{'id': d['id'], 'option': d['option'], 'applied': d['id'] in APPLIED, 'effective': d['effective'], 'rows': d['rows'],
                          'conventions': d.get('conventions', []), 'ownerAnswer': 'unanswered'} for d in REG['delegated']],
           'reviewerAccepted': sorted(ACCEPTED), 'reviewerPendingRootDecision': REG['reviewer']['pendingRootDecision'], 'withdrawn': REG['reviewer']['withdrawn']}
    hb = {'schema': 'pocol-m1-hash-bindings/0.29', 'groups025': groups, 'v1Groups': v1groups,
          'worstCase': {k: v for r in results if r['check'] in ('SHA29.preimageBinding', 'SHA29.worstCaseFixtureHash') for k, v in [(r['check'], r)]},
          'evidenceFiles': {rel: sha(ROOT / rel) for rel in INPUTS_ROOT + PRESERVED}}
    am = {'schema': 'pocol-m1-acceptance-matrix/0.29', 'rule': INV['acceptanceRule'], 'rows': matrix, 'findings': finds,
          'missingExperimentDefinitions': missing_exp, 'counts': dict(Counter(v['status'] for v in matrix.values())),
          'note': 'CompleteCandidate is not Complete; PendingRootReview needs root review of this overlay; Phase A outcomes are not M1 gate conditions'}
    texts = {'acceptance-matrix-0.29.json': dumps(am), 'decision-register-0.29.json': dumps(reg), 'hash-bindings-0.29.json': dumps(hb),
             'deadline-rerun-0.29.json': dumps({'schema': 'pocol-m1-deadline-rerun/0.29', 'profiles': rerun,
                                                'checks': {r['check']: r['status'] for r in results if r['check'].startswith(('U14.', 'V1rerun.', 'CRM101.bud.'))}})}
    return texts


def leak_and_keys(texts):
    markers = ('PoCol_' + 'Dialogue', 'state' + '.json', 'source' + 'State', 'historical-x8-full' + '-source-private', 'transcript' + '-private')
    leaks = [str(f.relative_to(PKG)) for f in sorted(PKG.rglob('*')) if f.is_file() and 'results' not in f.parts
             and f.suffix in ('.json', '.md', '.py') and any(m in f.read_text(encoding='utf-8') for m in markers)]
    leaks += [n for n, t in texts.items() if any(m in t for m in markers)]
    check('boundary.noPrivatePath', not leaks, files=leaks)
    keys = [v['priv'][2:].lower() for v in rj('coordination/review-001/m1-draft-0.5/results/txa-node-check.json')['keys'].values()]
    leaked = [n for n, t in texts.items() if any(k in t.lower() for k in keys)]
    check('boundary.noPrivateKeyInOutputs', not leaked, files=leaked)


def coverage():
    have = {r['check']: r['status'] for r in results}
    need = ['authority.ledgerStandingAuthorization', 'authority.registerIds', 'authority.notPersonalApproval', 'authority.ownerAnswersUntouched',
            'authority.effectiveTextsInOverlay', 'authority.pendingConventionsListed', 'U01.factoryConstantOf02Model', 'U01.notADeployment',
            'U02.defaultNeverShowsRevoked', 'U02.noChunkFetchBeforeClick', 'U02.timing.P-U02-2', 'U08.proofKeys.allLengths', 'U08.proofKeys.bytes',
            'U14.tokens.fromReviewedTranscript', 'U14.sleepBoundIsNotElapsedBound', 'CRM101.replyLimitUnchanged', 'CRM101.E03Amended',
            'CRM101.supersededTextsPreserved', 'CRM101.virtualTimeFixtures', 'SHA29.worstCase.cachedByCategory', 'SHA29.everyCheckRetained',
            'SHA29.uncachedIs6721', 'SHA29.preimageBinding', 'SHA29.sourceLiteralPreservedAsHistory', 'PC322.rangesInCheckerAndModel',
            'PC322.classifyBounds', 'PC322.c32CasesUnderDeadline', 'RFE61.executedVectorIsCurrentFile', 'V1rerun.u700KeepsEverySavedVerdict',
            'V1rerun.u1600HasExpiries', 'hash29.noOwnerBlockedGroup', 'hash29.rootVerified025', 'consolidation.rowsMatchProposal',
            'consolidation.findingsMatchProposal', 'experiments.definitionsComplete', 'boundary.noPrivatePath', 'boundary.noPrivateKeyInOutputs']
    need += ['authority.exact.' + d['id'] for d in DEL['decisions']] + ['decision.%s.applied' % d['id'] for d in DEL['decisions']]
    need += ['U14.hnc.' + c['id'] for c in own('vectors/deadline-cases-0.29.json')['hnc']['cases']]
    need += ['U14.content.' + c['id'] for c in own('vectors/deadline-cases-0.29.json')['content']['cases']]
    need += ['CRM101.bud.' + c['id'] for c in own('vectors/deadline-cases-0.29.json')['bud']['cases']]
    need += ['U02.row.' + r['id'] for r in own('vectors/u02-revoked-viewer-0.29.json')['rows']]
    need += ['U10.case.' + c['id'] for c in own('vectors/u10-publisher-transfer-0.29.json')['cases']]
    need += ['U08.noncanonical.' + c['id'] for c in own('vectors/u08-canonical-split-0.29.json')['noncanonical']]
    need += ['U08.boundary.%d' % b['len'] for b in own('vectors/u08-canonical-split-0.29.json')['boundaries']]
    need += ['history.' + h['id'] for h in INV['historicalFailures']['entries']]
    missing = [x for x in need if have.get(x) != 'pass']
    check('coverage029.required', not missing, missing=missing, required=len(need))
    check('coverage029.noStepAborted', not [k for k in have if k.startswith('step.completed')])


def out_path():
    p = OUT / 'run-results-0.29.json'
    k = 1
    while p.exists():
        p = OUT / ('run-results-0.29-rerun-%d.json' % k)
        k += 1
    return p


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
    step('history', history)
    step('authority', authority)
    step('u01', u01)
    step('u02', u02)
    step('u10', u10)
    step('u08', u08)
    cfg = step('v1network', v1_network)
    rerun = {}
    if cfg is not None:
        step('deadline', deadline, cfg)
        rerun = step('v1rerun', v1_rerun, cfg) or {}
        step('sha29', sha29, cfg)
    step('crm101', crm101)
    step('pc322', pc322)
    step('rfe61', rfe61)
    step('decisions', decisions_applied)
    hg = step('hash', hash_groups) or ({}, {})
    matrix = step('consolidation', consolidate, hg[0]) or {}
    finds = step('findings', findings) or {}
    missing_exp = step('experiments', experiment_definitions)
    texts = step('outputs', outputs, matrix, finds, hg[0], hg[1], rerun, missing_exp) or {}
    step('leaks', leak_and_keys, texts)
    for p in REG['reviewer']['pendingRootDecision']:
        gap('gap.reviewerDecision.' + p['id'], text=p['text'], rows=p['rows'])
    gap('gap.rootReviewOfOverlay', text='root review of the 0.29 effective profile is pending; no row is Complete')
    gap('gap.phaseA', text='Phase A outcomes (E01-E04, E06, E07, X-U14: EVM, Chrome, node) are implementation results, not M1-spec gate conditions')
    coverage()
    after = {rel: sha(ROOT / rel) for rel in PRESERVED}
    check('preserved.olderFilesUnchanged', before == after, before=before, after=after)
    failed = [r for r in results if r['status'] == 'FAIL']
    texts['dashboard-0.29-ar.json'] = dumps(dashboard(matrix, finds, missing_exp or [], failed))
    for name in sorted(texts):
        step('write ' + name, write_asset, name, texts[name], 'asset.' + name)
    failed = [r for r in results if r['status'] == 'FAIL']
    out = {'package': 'M1 draft 0.29 (delegated technical decisions overlay; full 41-row gate)',
           'executedAtUtc': datetime.now(timezone.utc).isoformat(timespec='seconds'),
           'environment': {'python': sys.version.split()[0], 'platform': platform.platform(), 'thirdPartyPackages': 'none'},
           'statusSchema': {'coreStatus': list(CORE_STATUS), 'diagnosticRenames': RENAMED, 'partialGap': 'recorded entries with partialGap=true'},
           'evidenceKind': 'reference models in virtual time; saved reviewed evidence re-bound; no browser, network, EVM, chain or real clock',
           'notExecuted': ['Phase A experiments', 'any owner decision', 'any production code'],
           'summary': {'checks': len(results), 'passed': sum(r['status'] == 'pass' for r in results),
                       'recorded': sum(r['status'] == 'recorded' for r in results), 'failed': len(failed),
                       'partialGaps': sum(1 for r in results if r.get('partialGap')),
                       'rows': dict(Counter(v['status'] for v in matrix.values())) if matrix else None, 'seconds': round(time.time() - t0, 1)},
           'results': results}
    OUT.mkdir(parents=True, exist_ok=True)
    target = out_path()
    target.write_text(json.dumps(out, indent=2, ensure_ascii=True, default=str) + '\n', encoding='utf-8')
    print(json.dumps(out['summary']), '->', target.name)
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
