"""Reference-only acceptance runner for M1 draft 0.25 (author turn 023): consolidated M1 specification audit,
supplements, hash-fixture freeze export and representation alternatives. Python standard library only; no
network, no browser, no EVM, no chain. NOT executed by the author.

Usage, from D:\\PoCol-Development:
  coordination\\runtime\\python311\\python.exe m1-draft-0.25\\tools\\run_checks_025.py
Writes only under m1-draft-0.25/results/ and exits 1 on any FAIL:
  run-results-0.25.json          (never overwritten: a later run writes run-results-0.25-rerun-<n>.json)
  hash-freeze-export-0.25.json   (deterministic; an existing different file is kept and the run FAILs)
  lc-literal-replies-0.25.json   (deterministic; same rule)

Steps:
  1. Boundary: safe logger, no old main() call, no network module, no private path, no dependency or binary.
  2. Evidence reuse: each cited saved review result exists; each reviewed package copy under coordination/review-001
     is byte-equal to the current package, so the saved results still describe the current files; superseded
     failures are closed by named later evidence. Earlier runners are not re-run.
  3. Inventory: 41 rows, files and anchors, saved-evidence patterns, decision tokens, gap registry, R3-08 criteria.
  4. Findings F01-F26 and the eight Codex corrections.
  5. Hash freeze: triad binding of all 611 preimages to the current fixtures, E05 binding, new preimage groups,
     aliases, classification of every 64-hex token in the earlier packages, and the export.
  6. Supplements: sweepMax, LogClient literal replies, RG3b model runs, E07 table, experiment specs.
  7. Representation: CR-E4-01, CR-E4-02 (bridge cases on the 0.4 model; WTF-8 alternative), CID-1.
"""

import ast
import copy
import hashlib
import importlib.util
import itertools
import json
import platform
import random
import re
import sys
import time
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
PKG, ROOT = HERE.parent, HERE.parent.parent
RES = PKG / 'results'
REL = 'm1-draft-0.25/'
CORE_STATUS = ('pass', 'recorded', 'FAIL')
RENAMED = {'name': 'diagName', 'check': 'diagCheck', 'status': 'proposalStatus', 'ok': 'diagOk',
           'label': 'diagLabel', 'condition': 'diagCondition'}
INPUTS = ['coordination/task-023.md', 'coordination/review-001/REVIEW-0.24.md', 'coordination/issue-ledger.json',
          'M1-CLAUDE-REVIEW.md', 'M1-CODEX-RESPONSE.md', 'm1-draft-0.2/M1-DISPOSITIONS-0.2.md', 'm1-draft-0.3/M1-ANNEX-INVENTORY-0.3.md',
          'coordination/hash-triad/inputs-expanded.tsv', 'coordination/hash-triad/generate_inputs.py',
          'coordination/review-001/e05-historical-evidence-inventory.json', 'coordination/review-001/e05-keccak-three-library.json',
          REL + 'tools/run_checks_025.py', REL + 'tools/supplements_ref.py', REL + 'tools/js_number_probe.cjs',
          REL + 'audit/row-inventory.json', REL + 'audit/findings-trace.json', REL + 'audit/decision-register.json', REL + 'audit/gap-registry.json',
          REL + 'hash/hash-freeze-plan.json', REL + 'supplements/s25-sweepmax.json', REL + 'supplements/s25-lc-literal.json',
          REL + 'supplements/s25-rg3b.json', REL + 'supplements/s25-e07-viewer-table.json', REL + 'supplements/s25-experiment-specs.json',
          REL + 'representation/cr-e4-01-u64-epoch.json', REL + 'representation/cr-e4-02-surrogates.json', REL + 'representation/cid-1-profile-chainid.json']
PRESERVED = ['coordination/issue-ledger.json', 'coordination/hash-triad/inputs-expanded.tsv', 'coordination/hash-triad/python-expanded-result.json',
             'coordination/hash-triad/noble-expanded-result.json', 'coordination/hash-triad/rust-expanded-result.json',
             'coordination/review-001/m1-draft-0.24/results/run-results-0.24.json', 'm1-draft-0.24/results/run-results-0.24.json',
             'm1-draft-0.2/vectors/code-table.json', 'm1-draft-0.2/vectors/abi-and-slots.json', 'm1-draft-0.19/vectors/lc-cases.json',
             'm1-draft-0.10/vectors/e3-br22a.json', 'm1-draft-0.13/vectors/e4-units.json']
ROW_IDS = (['C1', 'C2', 'C3', 'C4', 'X1', 'X2', 'X3'] + ['B%d' % i for i in range(1, 12)] + ['R1', 'R2', 'R3', 'R4', 'S1', 'S2', 'S3', 'S4',
           'Q1', 'Q2', 'Q3', 'Q4'] + ['E%d' % i for i in range(1, 8)] + ['V1', 'V2', 'V3', 'V4'])
SUPPLEMENT_PREFIX = {'S25-SWEEPMAX': 'S25.sweepMax.', 'S25-E07': 'S25.e07.', 'S25-LCLIT': 'S25.lclit.', 'S25-RG3B': 'S25.rg3b.'}

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


def load_json(rel):
    return json.loads((ROOT / rel).read_text(encoding='utf-8'))


def load_module(rel, modname):
    spec = importlib.util.spec_from_file_location(modname, str(ROOT / rel))
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


sys.path.insert(0, str(HERE))
import supplements_ref as S           # noqa: E402  (this package only)


# ------------------------------------------------------------------ 2. evidence reuse

INV = load_json(REL + 'audit/row-inventory.json')
_RES = {}


def results_of(key):
    if key not in _RES:
        p = ROOT / INV['results'][key]
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
    for s in INV['supersededFailures']:
        hit = match(s['results'], '^' + re.escape(s['check']) + '$') or []
        closure = match(*s['closure']) or []
        ok = (len(hit) == 1 and hit[0].get('status') == 'FAIL' and any(r.get('status') == 'pass' for r in closure)
              and not any(r.get('status') == 'FAIL' for r in closure))
        if ok:
            SUPERSEDED_OK.add((s['results'], s['check']))
        check('evidence.supersededFailure.%s.%s' % (s['results'], s['check']), ok, issue=s['issue'], closure=s['closure'],
              closurePassed=sum(r.get('status') == 'pass' for r in closure))


def evidence(label, key, pattern):
    hit = match(key, pattern)
    if hit is None:
        return check(label, False, results=INV['results'][key], missingFile=True)
    st = Counter(r.get('status') for r in hit)
    fails = [r['check'] for r in hit if r.get('status') == 'FAIL']
    unexplained = [c for c in fails if (key, c) not in SUPERSEDED_OK]
    return check(label, st.get('pass', 0) >= 1 and not unexplained, results=INV['results'][key], pattern=pattern, matched=len(hit),
                 passed=st.get('pass', 0), recorded=st.get('recorded', 0), failedSuperseded=[c for c in fails if c not in unexplained],
                 unexplainedFailures=unexplained, sample=[r['check'] for r in hit[:4]])


def review_copies():
    for n in range(2, 25):
        pkg = 'm1-draft-0.%d' % n
        cp = ROOT / 'coordination' / 'review-001' / pkg
        if not cp.exists():
            record('evidence.reviewCopy.' + pkg, 'recorded', present=False, note='no review copy of this package; its saved result, if any, is not cited')
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


# ------------------------------------------------------------------ 3. inventory, decisions, gaps

REG = load_json(REL + 'audit/decision-register.json')
OWNER_IDS = {x['id'] for x in REG['owner']}
REVIEWER_IDS = {x['id'] for x in REG['reviewer']}


def decision_register():
    for x in REG['owner'] + REG['reviewer']:
        src = x['source']
        check('decision.source.' + x['id'], contains(src['file'], src['find']), file=src['file'], find=src['find'])
    for x in REG['notOwnerDecisions']:
        if x['source']:
            check('decision.notOwner.' + x['id'], contains(x['source']['file'], x['source']['find']), why=x['why'])
    led = load_json('coordination/issue-ledger.json')
    st = {i['id']: i for i in led['issues']}
    check('decision.ledger.ownerItemsStillOpen', st['C07']['status'] == 'open' and st['C06']['status'] == 'open' and st['C01']['status'] == 'open'
          and all(u in st['C07']['evidence'] for u in ('U01', 'U02', 'U10', 'U14')), C07=st['C07'], C06=st['C06']['status'])
    check('decision.ledger.proposalsUnapproved', st['CR-E4-01']['status'].startswith('proposed') and st['CR-E4-02']['status'].startswith('proposed')
          and 'unapproved' in st['RF-E6-1']['status'] and st['F01-F26']['status'] == 'open',
          statuses={k: st[k]['status'] for k in ('CR-E4-01', 'CR-E4-02', 'RF-E6-1', 'F01-F26')})
    check('decision.ownerSetExact', OWNER_IDS == {'U01', 'U02', 'U10', 'U14', 'CR-M1-01', 'RF-E6-1'}, owner=sorted(OWNER_IDS))


def collect_gaps():
    out = []
    for key in load_json(REL + 'audit/gap-registry.json')['scanned']:
        for r in results_of(key) or []:
            if r.get('partialGap') is True:
                out.append((key, r['check']))
    return out


def gap_registry():
    reg = load_json(REL + 'audit/gap-registry.json')
    gaps = collect_gaps()
    unclassified, by_class = [], Counter()
    for key, name in gaps:
        hits = [e for e in reg['entries'] if re.search(e['pattern'], name)]
        if not hits:
            unclassified.append([key, name])
        else:
            by_class[hits[0]['class']] += 1
    check('gaps.allClassified', bool(gaps) and not unclassified, gaps=len(gaps), unclassified=unclassified, byClass=dict(by_class))
    for e in reg['entries']:
        if e['class'] == 'coveredBySavedEvidence' and isinstance(e['by'], list) and e['by'][0] in INV['results']:
            evidence('gaps.coveredBy.' + e['pattern'], e['by'][0], e['by'][1])
    return gaps


def files_and_anchors(row):
    missing, anchors = [], []
    for s in row['spec']:
        if not (ROOT / s['file']).exists():
            missing.append(s['file'])
        elif s.get('contains') and not contains(s['file'], s['contains']):
            anchors.append([s['file'], s['contains']])
    for f in row['fixtures'] + row['review']:
        if not (ROOT / f).exists():
            missing.append(f)
    return missing, anchors


def inventory(hash_status):
    rows = INV['rows']
    check('inventory.rows', [r['id'] for r in rows] == ROW_IDS, count=len(rows))
    provenance('inventory.rowsSource', {'file': INV['rowsSource']['file'], 'lines': [16, 95], 'literals': ['C1', 'X3', 'B11', 'R4', 'S4', 'Q4', 'E7', 'V4']})
    check('inventory.acceptanceRule', contains(INV['acceptanceRule']['file'], INV['acceptanceRule']['anchor'])
          and contains(INV['acceptanceRule']['phases']['file'], INV['acceptanceRule']['phases']['anchor']))
    computed = {}
    have = {r['check']: r['status'] for r in results}
    for row in rows:
        rid = row['id']
        missing, anchors = files_and_anchors(row)
        check('row.%s.filesAndAnchors' % rid, not missing and not anchors, missingFiles=missing, missingAnchors=anchors)
        ev_ok = all([evidence('row.%s.evidence.%s:%s' % (rid, k, p), k, p) for k, p in row['evidence']]) if row['evidence'] else False
        unknown = [t for t in row['owner'] if t not in OWNER_IDS] + [t for t in row['reviewer'] if t not in REVIEWER_IDS]
        check('row.%s.decisionTokensRegistered' % rid, not unknown, unknown=unknown)
        supp = [m['resolvedBy'] for m in row['missing'] if m.get('resolvedBy')]
        unresolved = [m['id'] for m in row['missing'] if not m.get('resolvedBy')]
        supp_ok = all(any(k.startswith(SUPPLEMENT_PREFIX[s]) for k in have) and
                      all(v != 'FAIL' for k, v in have.items() if k.startswith(SUPPLEMENT_PREFIX[s])) for s in supp)
        new_files = [f for f in row['fixtures'] if f.startswith(REL)]
        pending = bool(supp or new_files)
        hb = [hash_status.get(g) for g in row['hashGroups']]
        blockers = ([{'type': 'owner', 'id': t} for t in row['owner']] + [{'type': 'reviewer', 'id': t} for t in row['reviewer']]
                    + [{'type': 'definition', 'id': m} for m in unresolved])
        for g, s in zip(row['hashGroups'], hb):
            if s and s['blockedBy']:
                blockers += [{'type': 'hash:' + b['type'], 'group': g, 'id': b['id']} for b in s['blockedBy']]
        hash_pending = any(s and s['pendingRootTriad'] for s in hb)
        c1 = 'blocked' if (missing or anchors or unresolved or (supp and not supp_ok)) else ('pendingRootReview' if pending else 'satisfied')
        c2 = 'blocked' if not ev_ok or (supp and not supp_ok) else ('pendingRootReview' if pending else 'satisfied')
        c3 = 'pendingRootReview' if pending else 'satisfied'
        c4 = 'blocked' if (row['owner'] or row['reviewer']) else 'satisfied'
        hard = [b for b in blockers if b['type'] in ('owner', 'definition', 'hash:owner', 'hash:reviewer')]
        c5 = 'blocked' if hard else ('pendingRootReview' if (pending or hash_pending) else 'satisfied')
        crit = {'c1': c1, 'c2': c2, 'c3': c3, 'c4': c4, 'c5': c5}
        status = 'Complete' if all(v == 'satisfied' for v in crit.values()) else 'Partial'
        only_reviewer = all(b['type'] in ('reviewer', 'hash:reviewer', 'hash:phaseA') for b in blockers) and c1 != 'blocked' and c2 != 'blocked'
        computed[rid] = {'status': status, 'criteria': crit, 'blockers': blockers, 'onlyReviewerBlocked': only_reviewer,
                         'phaseA': row.get('phaseA', []), 'experiments': row['experiments']}
        record('row.%s.criteria' % rid, 'recorded', criteria=crit, computedStatus=status, blockers=blockers, onlyReviewerBlocked=only_reviewer)
        check('row.%s.statusMatchesProposal' % rid, status == row['proposedStatus'], computed=status, proposed=row['proposedStatus'])
    tally = Counter(v['status'] for v in computed.values())
    record('inventory.summary', 'recorded', statuses=dict(tally), criteria={c: dict(Counter(v['criteria'][c] for v in computed.values())) for c in ('c1', 'c2', 'c3', 'c4', 'c5')},
           ownerBlockedRows=sorted(r for r, v in computed.items() if any(b['type'] in ('owner', 'hash:owner') for b in v['blockers'])),
           definitionBlockedRows=sorted(r for r, v in computed.items() if any(b['type'] == 'definition' for b in v['blockers'])),
           onlyReviewerBlockedRows=sorted(r for r, v in computed.items() if v['onlyReviewerBlocked']))
    return computed


# ------------------------------------------------------------------ 4. findings

def findings():
    doc = load_json(REL + 'audit/findings-trace.json')
    ids = [f['id'] for f in doc['findings']]
    check('findings.all26', ids == ['F%02d' % i for i in range(1, 27)])
    claude = (ROOT / 'M1-CLAUDE-REVIEW.md').read_text(encoding='utf-8')
    codex = (ROOT / 'M1-CODEX-RESPONSE.md').read_text(encoding='utf-8')
    matrix = (ROOT / 'm1-draft-0.2/M1-DISPOSITIONS-0.2.md').read_text(encoding='utf-8')
    for f in doc['findings']:
        fid = f['id']
        check('findings.%s.inBothReports' % fid, fid in claude and fid in codex and ('| %s |' % fid) in matrix)
        for k, p in f['evidence']:
            evidence('findings.%s.evidence.%s:%s' % (fid, k, p), k, p)
        if not f['evidence']:
            record('findings.%s.noReferenceEvidence' % fid, 'recorded', why=f.get('note', 'contract or procedure item; its outcome is an implementation experiment'),
                   experiments=f['experiments'])
        unknown = [t for t in f['owner'] if t not in OWNER_IDS] + [t for t in f['reviewer'] if t not in REVIEWER_IDS]
        want = 'openOwner' if f['owner'] else ('proposeCloseSpecAfterReviewerDecision' if f['reviewer'] else 'proposeCloseSpec')
        check('findings.%s.dispositionConsistent' % fid, not unknown and f['proposedDisposition'] == want,
              proposed=f['proposedDisposition'], derived=want, unknownTokens=unknown)
    for c in doc['codexCorrectionsReconciled']:
        for k, p in c['evidence']:
            evidence('findings.codexCorrection.%s.%s:%s' % (c['finding'], k, p), k, p)
    check('findings.codexCorrections8', len(doc['codexCorrectionsReconciled']) == 8 and all('errata' in c['adoptedIn'] for c in doc['codexCorrectionsReconciled']))
    record('findings.summary', 'recorded', dispositions=dict(Counter(f['proposedDisposition'] for f in doc['findings'])))


# ------------------------------------------------------------------ 5. hash freeze

def be(n, w):
    return n.to_bytes(w, 'big')


def rebuild_triad(K, R3, tsv):
    """generate_inputs.py rules over the CURRENT fixtures. CREATE2 uses the triad digests of .data/.initcode."""
    samples = {'K1': b'', 'K2': b'\xc0', 'K3': b'\x80'}
    factory = bytes.fromhex('0000000000000000000000000000000000c0c005')
    table = load_json('m1-draft-0.2/vectors/code-table.json')['codes']
    for addr, code in table.items():
        rt = bytes.fromhex(code[2:])
        data = rt[1:]
        init = b'\x61' + len(rt).to_bytes(2, 'big') + bytes.fromhex('80600a5f395ff3') + rt
        samples[addr + '.data'] = data
        samples[addr + '.runtime'] = rt
        samples[addr + '.initcode'] = init
        dd, di = tsv.get(addr + '.data'), tsv.get(addr + '.initcode')
        samples[addr + '.create2'] = b'\xff' + factory + bytes.fromhex(dd[1] if dd else '00' * 32) + bytes.fromhex(di[1] if di else '00' * 32)
    for fn in ('manifest-positive.json', 'manifest-negative.json'):
        for fx in load_json('m1-draft-0.2/vectors/' + fn)['fixtures']:
            samples[fx['id'] + '.manifest'] = bytes.fromhex(fx['manifest'][2:])
    net = load_json('coordination/review-001/independent-hashes-corrected.json')
    samples['netKey'] = bytes.fromhex(net['netKeyPreimage'][2:])
    for fx in load_json('m1-draft-0.2/vectors/manifest-positive.json')['fixtures']:
        decoded = R3.decode(bytes.fromhex(fx['manifest'][2:]))
        for path, mime, size, declared, chunks in decoded[2]:
            samples[fx['id'] + '.file.' + path.hex()] = b''.join(bytes.fromhex(table['0x' + a.hex()][4:]) for a, ln in chunks)
    return samples, table


def hash_freeze():
    plan = load_json(REL + 'hash/hash-freeze-plan.json')
    K = load_module('m1-draft-0.2/tools/keccak.py', 'keccak_025')
    R3 = load_module('m1-draft-0.3/tools/rlp_strict.py', 'rlp_strict_03_s25')
    try:
        check('HF.referenceKeccak.selftest', K.selftest() is True)
    except AssertionError as e:
        check('HF.referenceKeccak.selftest', False, error=str(e))
    kh = lambda b: K.keccak256(b).hex()                                          # noqa: E731
    inv = load_json('coordination/review-001/e05-historical-evidence-inventory.json')
    tsv_path = ROOT / inv['inputs']['file']
    check('HF.triad.inputsSha256', sha(tsv_path) == inv['inputs']['sha256'], now=sha(tsv_path), rootRecorded=inv['inputs']['sha256'])
    for s in inv['saved']:
        check('HF.triad.resultSha256 ' + s['library'], sha(ROOT / s['file']) == s['sha256'], file=s['file'])
    py = load_json('coordination/hash-triad/python-expanded-result.json')
    noble = load_json('coordination/hash-triad/noble-expanded-result.json')
    rust = json.loads((ROOT / 'coordination/hash-triad/rust-expanded-result.json').read_bytes().decode('utf-16'))   # UTF-16LE with BOM
    check('HF.triad.threeLibrariesAgree', py['samples'] == 611 and py['failed'] == 0 and py['declaredPositiveFileHashesVerified'] == 486
          and noble['checks'] == 611 and noble['failed'] == 0 and rust['checks'] == 611 and rust['failed'] == 0,
          python=[py['samples'], py['failed']], noble=[noble['checks'], noble['failed']], rust=rust)
    rows = []
    for line in tsv_path.read_text(encoding='ascii').splitlines():
        label, hx, dg = line.split('\t')
        rows.append((label, hx, dg))
    tsv = {l: (h, d) for l, h, d in rows}
    check('HF.triad.rows', len(rows) == 611 and len(tsv) == 611, rows=len(rows))
    samples, table = rebuild_triad(K, R3, tsv)
    order_ok = list(samples) == [r[0] for r in rows]
    mism = [l for l, b in samples.items() if l not in tsv or tsv[l][0] != b.hex()]
    check('HF.triad.bindingCurrentFixtures', order_ok and not mism and len(samples) == 611, rebuilt=len(samples), orderEqual=order_ok, mismatches=mism[:10])
    check('HF.triad.declaredFileChecks', py['declaredChecks'] == [l for l in samples if '.file.' in l], declared=len(py['declaredChecks']))
    bad_addr = [a for a in table if tsv.get(a + '.create2', ('', ''))[1][-40:] != a[2:].lower()]
    check('HF.triad.create2AddressesBound', not bad_addr and len(table) == 12, addresses=len(table), bad=bad_addr)
    small = [(l, h, d) for l, h, d in rows if len(h) <= 4096]
    bad_ref = [l for l, h, d in small if kh(bytes.fromhex(h)) != d]
    check('HF.referenceKeccakMatchesTriad.small', not bad_ref and len(small) > 100, compared=len(small), bad=bad_ref[:5])
    # fixture-asserted digests equal the triad
    bad = []
    for fn in ('manifest-positive.json', 'manifest-negative.json'):
        for fx in load_json('m1-draft-0.2/vectors/' + fn)['fixtures']:
            if 'manifestKeccak256' in fx and fx['manifestKeccak256'][2:] != tsv[fx['id'] + '.manifest'][1]:
                bad.append(fx['id'])
    check('HF.triad.manifestDigestsAsserted', not bad, bad=bad)
    vr = load_json('m1-draft-0.2/vectors/version-records.json')['fixtures']
    want = {'ver-minimal': 'pos-minimal', 'ver-manifest-chunk-missing': 'pos-minimal', 'ver-manifest-noncanonical-split': 'pos-minimal',
            'ver-manifest-length-0': 'pos-minimal', 'ver-manifest-65536': 'pos-manifest-65536'}
    bad = [f['id'] for f in vr if f['id'] in want and f['record']['manifestHash'][2:] != tsv[want[f['id']] + '.manifest'][1]]
    zero = [f['id'] for f in vr if f['record']['manifestHash'] == '0x' + '00' * 32]
    cat = [f['id'] for f in vr if f['id'] in ('ver-minimal', 'ver-manifest-65536', 'ver-manifest-noncanonical-split')
           and b''.join(bytes.fromhex(table[a][4:])[:n] for a, n in f['record']['chunks']).hex() != tsv[want[f['id']] + '.manifest'][0]]
    check('HF.alias.versionRecords', not bad and zero == ['ver-manifest-hash'] and not cat, bad=bad, zeroHash=zero, chunkConcatMismatch=cat)
    # E05: K1-K3 and GSV1
    e05 = load_json('coordination/review-001/e05-keccak-three-library.json')
    supp = load_json('m1-draft-0.22/vectors/v3-e05-supplement.json')
    segs = load_json('m1-draft-0.21/vectors/v3-gsv1.json')['flatSegments']
    gsv1 = b''.join(bytes.fromhex(s) if isinstance(s, str) else bytes.fromhex(s['repeat']) * s['count'] for s in segs)
    g = next(r for r in e05['results'] if r['id'] == 'GSV1')
    check('HF.e05.GSV1', len(gsv1) == 341 and hashlib.sha256(gsv1).hexdigest() == supp['GSV1']['inputSha256'] == g['inputSha256']
          and kh(gsv1) == supp['GSV1']['keccak256'] == g['pythonKeccak'] == g['nobleKeccak'] == g['jsSha3Keccak'] and g['pass'] is True,
          bytes=len(gsv1))
    check('HF.e05.K', all(next(r for r in e05['results'] if r['id'] == k)['pass'] and supp['K'][k] == tsv[k][1] for k in ('K1', 'K2', 'K3')))
    check('HF.fileSha256.keccakPy', sha(ROOT / 'm1-draft-0.2/tools/keccak.py') == e05['pythonLibrarySha256'] == plan['fileSha256'][0]['value'])
    # new preimage groups
    abi = load_json('m1-draft-0.2/vectors/abi-and-slots.json')
    new = []

    def add(eid, group, pre, expected, form, source, reviewer, phase_a=(), owner=()):
        ref = kh(pre)
        got = {'full': ref, 'prefix4': ref[:8], 'last20': ref[-40:]}[form]
        ok = expected is None or got == expected.lower().replace('0x', '')
        new.append({'id': eid, 'group': group, 'class': 'pendingRootTriad', 'preimageHex': pre.hex(), 'expectedDigest': expected and expected.lower().replace('0x', ''),
                    'assertedForm': form, 'referenceDigest': ref, 'source': source, 'verification': 'pendingRootTriad',
                    'ownerParameters': list(owner), 'reviewerParameters': list(reviewer), 'phaseAConfirmation': list(phase_a), 'referenceAgrees': ok})
        return ok

    bad = []
    for part, grp in (('factory', 'functions'), ('factory', 'errors'), ('website', 'functions'), ('website', 'errors')):
        for sig, sel in abi[part][grp].items():
            if not add('abi.selector:' + sig, 'abi.selector', sig.encode('ascii'), sel, 'prefix4', 'abi-and-slots.json %s.%s' % (part, grp), ['P-ABI'], ['E04']):
                bad.append(sig)
    for sig, topic in abi['website']['eventTopic0'].items():
        if not add('abi.topic0:' + sig, 'abi.topic0', sig.encode('ascii'), topic, 'full', 'abi-and-slots.json website.eventTopic0', ['P-ABI'], ['E04']):
            bad.append(sig)
    check('HF.new.abi', not bad, bad=bad)
    bad, derived = [], []
    for vid, v in abi['slots']['version'].items():
        base_hex = v['manifestHash']
        if not add('slot.versionBase:%s' % vid, 'slot.versionBase', be(int(vid), 32) + be(3, 32), base_hex, 'full', 'abi-and-slots.json slots.version.%s.manifestHash' % vid, ['P13'], ['E04']):
            bad.append(vid)
        base = int(base_hex, 16)
        if not add('slot.chunks0:%s' % vid, 'slot.chunks0', be((base + 2) % 2 ** 256, 32), v['chunks[i]'][0], 'full', 'abi-and-slots.json slots.version.%s.chunks[i][0]' % vid, ['P13'], ['E04']):
            bad.append(vid + '.chunks0')
        c0 = int(v['chunks[i]'][0], 16)
        if not (int(v['packed(manifestLen|status|publishedBlock)'], 16) == base + 1 and int(v['chunks.length'], 16) == base + 2
                and [int(x, 16) for x in v['chunks[i]']] == [c0 + i for i in range(len(v['chunks[i]']))]):
            derived.append(vid)
    pubs = abi['slots']['examplePublisherSlots(placeholder addresses, not test accounts)']
    for k, v in pubs.items():
        addr = k.split('=')[1]
        if not add('slot.publisher:' + addr.lower(), 'slot.publisher', bytes(12) + bytes.fromhex(addr[2:]) + be(4, 32), v, 'full', 'abi-and-slots.json examplePublisherSlots', ['P13'], ['E04']):
            bad.append(addr)
    t = abi['t1_02_expectedState']
    v1 = abi['slots']['version']['1']
    alias = (t['Version[1].base'] == v1['manifestHash'] and t['value@Version[1].base'][2:] == tsv['pos-minimal.manifest'][1]
             and t['slot of chunks[0]'] == v1['chunks[i]'][0]
             and int(t['value@chunks[0]'], 16) == (79 << 160) | int('bfed2132e3e3f8a0dbfac65cb3eaf871f28d138f', 16))
    check('HF.new.slots', not bad and not derived, bad=bad, derivedArithmeticBad=derived)
    check('HF.alias.t1_02', alias)
    pr = load_json('m1-draft-0.5/vectors/proof-response-cases.json')
    prc = json.dumps(pr)
    bad = [i for i in range(3) if not add('proof.storageSlot:%d' % i, 'proof.storageSlot', be(i, 32), _find_key(pr, 'storageSlot%d' % i),
                                         'full', 'proof-response-cases.json pathKats.storageSlot%d' % i, [])]
    check('HF.new.proofSlots', not bad, bad=bad)
    check('HF.alias.proofK', ('= 0x' + tsv['K3'][1]) in prc and ('= 0x' + tsv['K1'][1]) in prc, note='emptyTrieRoot = K3, keccakEmpty = K1')
    c14 = load_json('m1-draft-0.6/vectors/c14-head.json')['rb2848']
    check('HF.new.c14', add('c14.contentHash', 'c14.contentHash', b''.join(be(i, 2) for i in range(2848)), c14['contentHash'], 'full', 'c14-head.json rb2848.contentHash', []))
    txa = next((r for r in results_of('R05') or [] if r.get('check') == 'txA'), None)
    if txa:
        check('HF.new.txA', add('txA.hash', 'txA.hash', bytes.fromhex(txa['raw'][2:]), txa['txHash'], 'full', 'saved 0.5 check txA', ['TK-1']))
    else:
        check('HF.new.txA', False, missing='saved 0.5 txA entry')
    sel = next((r for r in match('R08', '^snapshotE\\.selector$') or []), None)
    se_sig = 'websiteSnapshot(uint32)'
    if sel is None:
        record('HF.new.snapshotE', 'recorded', missing='saved 0.8 entry snapshotE.selector', note='alternative E was not adopted; nothing to freeze')
    else:
        check('HF.new.snapshotE', add('snapshotE.selector', 'snapshotE.selector', se_sig.encode('ascii'), sel.get('selector'), 'prefix4',
                                      'saved 0.8 check snapshotE.selector', ['ALT-E']) and sel.get('signature') == se_sig, saved=sel)
    ab = load_json('m1-draft-0.3/vectors/annex-batch1.json')['X1_netKey']['vector']
    ss = load_json('m1-draft-0.5/vectors/sitestorage-nav-cases.json')['storageKey']
    check('HF.alias.X1.netKey', ab['rlpPreimage'][2:] == tsv['netKey'][0] and ss['netKey'][2:] == tsv['netKey'][1] and ss['netKey'] in ss['expected'])
    vw = (ROOT / 'm1-draft-0.3/vectors/version-width.json').read_text(encoding='utf-8')
    check('HF.alias.versionWidth.fileHash', tsv['pos-minimal.file.2f696e6465782e68746d6c'][1] in vw)
    gv = load_json('m1-draft-0.21/vectors/v3-gsv1.json')['keccakVectors']
    check('HF.alias.gsv1.vectors', all(gv[k][1] == tsv[k][1] and gv[k][0] == tsv[k][0] for k in ('K1', 'K2', 'K3')))
    # export
    entries = []
    addr_bytes = [bytes.fromhex(a[2:]) for a in table]
    for label, hx, dg in rows:
        owner, reviewer, group = [], [], 'triad.files'
        if label in ('K1', 'K2', 'K3'):
            group = 'triad.K'
        elif label.startswith('0x'):
            group = 'triad.codeTable'
            if label.endswith('.create2'):
                owner = ['U01']
        elif label.endswith('.manifest'):
            group = 'triad.manifests'
            if any(a in bytes.fromhex(hx) for a in addr_bytes):
                owner = ['U01']
        elif label == 'netKey':
            group, reviewer = 'triad.netKey', ['P-X1']
        entries.append({'id': 'triad:' + label, 'group': group, 'class': 'triad', 'preimageHex': hx, 'expectedDigest': dg, 'assertedForm': 'full',
                        'referenceDigest': None, 'source': 'coordination/hash-triad/inputs-expanded.tsv ' + label, 'verification': 'triad',
                        'ownerParameters': owner, 'reviewerParameters': reviewer, 'phaseAConfirmation': []})
    entries.append({'id': 'e05:GSV1', 'group': 'e05.GSV1', 'class': 'e05', 'preimageHex': gsv1.hex(), 'expectedDigest': supp['GSV1']['keccak256'], 'assertedForm': 'full',
                    'referenceDigest': kh(gsv1), 'source': 'm1-draft-0.21/vectors/v3-gsv1.json flatSegments', 'verification': 'e05',
                    'ownerParameters': [], 'reviewerParameters': [], 'phaseAConfirmation': []})
    entries += new
    for e in entries:
        bl = [{'type': 'owner', 'id': p} for p in e['ownerParameters']] + [{'type': 'reviewer', 'id': p} for p in e['reviewerParameters']]
        if e['class'] == 'pendingRootTriad' and e['reviewerParameters'] == ['ALT-E']:
            bl = [{'type': 'reviewer', 'id': 'ALT-E (alternative E not adopted)'}]
        e['blockers'] = bl + [{'type': 'phaseA', 'id': p} for p in e.get('phaseAConfirmation', [])]
        e['status'] = 'blocked' if bl else ('pendingRootTriad' if e['verification'] == 'pendingRootTriad' else 'freezable')
    entries.sort(key=lambda e: e['id'])
    head = {'schema': 'pocol-m1-hash-freeze-export/0.25', 'plan': REL + 'hash/hash-freeze-plan.json',
            'note': 'Not a frozen list. status freezable = three-library verified and free of unapproved parameters; blocked = see blockers; pendingRootTriad = never verified by three libraries.',
            'counts': dict(Counter(e['status'] for e in entries)), 'entries': len(entries)}
    text = json.dumps(head, sort_keys=True, separators=(',', ':'))[:-1] + ',"list":[\n' + ',\n'.join(
        json.dumps(e, sort_keys=True, separators=(',', ':')) for e in entries) + '\n]}\n'
    keys = [k['priv'][2:] for k in load_json('m1-draft-0.5/annex/br-messages.json')['txA']['kats']]
    check('HF.export.noKeyMaterial', not any(k in text for k in keys) and all(p['value'] not in text for p in plan['privateProvenance']))
    write_asset('hash-freeze-export-0.25.json', text, 'HF.export')
    record('HF.export.counts', 'recorded', counts=head['counts'], byGroup={g: dict(Counter(e['status'] for e in entries if e['group'] == g))
                                                                         for g in sorted({e['group'] for e in entries})})
    status = {}
    for gname in sorted({e['group'] for e in entries}):
        es = [e for e in entries if e['group'] == gname]
        status[gname] = {'blockedBy': _uniq([{'type': b['type'], 'id': b['id']} for e in es for b in e['blockers'] if b['type'] != 'phaseA'])
                         + _uniq([{'type': 'phaseA', 'id': b['id']} for e in es for b in e['blockers'] if b['type'] == 'phaseA']),
                         'pendingRootTriad': any(e['status'] == 'pendingRootTriad' for e in es)}
    for a, target in (('alias.versionRecords', 'triad.manifests'), ('alias.t1_02.manifestHash', 'triad.manifests'), ('alias.X1.netKey', 'triad.netKey')):
        status[a] = status[target]
    for s_ in ('syn.lc.branchHashes', 'privateProvenance', 'ph.v3.networkProfiles'):
        status[s_] = {'blockedBy': [], 'pendingRootTriad': False}
    coverage_scan(plan, tsv, rows, new, supp, keys)
    return status


def _find_key(o, k):
    if isinstance(o, dict):
        if k in o:
            return o[k]
        for v in o.values():
            r = _find_key(v, k)
            if r is not None:
                return r
    if isinstance(o, list):
        for v in o:
            r = _find_key(v, k)
            if r is not None:
                return r
    return None


def _uniq(xs):
    out = []
    for x in xs:
        if x not in out:
            out.append(x)
    return out


TOKEN = re.compile(r'(?<![0-9a-fA-F])(?:0x)?([0-9a-fA-F]{64})(?![0-9a-fA-F])')
LC_HASH = re.compile(r'(5a|a1|b2|a3)[0-9a-f]{8}0{54}')


def coverage_scan(plan, tsv, rows, new, supp, keys):
    """Every 64-hex token of every JSON file under the vectors/ and annex/ folders of 0.2-0.24 is classified."""
    digests = {d for _, _, d in rows}
    pre64 = {h for _, h, _ in rows if len(h) == 64}
    computed = {e['expectedDigest'] for e in new if e['expectedDigest'] and len(e['expectedDigest']) == 64}
    abi = load_json('m1-draft-0.2/vectors/abi-and-slots.json')
    for v in abi['slots']['version'].values():
        computed |= {v['packed(manifestLen|status|publishedBlock)'][2:], v['chunks.length'][2:]} | {x[2:] for x in v['chunks[i]']}
    private = {p['value'] for p in plan['privateProvenance']}
    filesha = {p['value'] for p in plan['fileSha256']}
    e05 = {supp['GSV1']['keccak256']}
    packed = {abi['t1_02_expectedState']['value@chunks[0]'][2:]}
    unclassified, tally, files = [], Counter(), 0
    for n in range(2, 25):
        for sub in ('vectors', 'annex'):
            d = ROOT / ('m1-draft-0.%d' % n) / sub
            if not d.exists():
                continue
            for f in sorted(d.rglob('*.json')):
                files += 1
                for m in TOKEN.finditer(f.read_text(encoding='utf-8')):
                    t = m.group(1).lower()
                    b = bytes.fromhex(t)
                    if t in digests:
                        c = 'triad'
                    elif t in e05:
                        c = 'e05'
                    elif t in pre64:
                        c = 'triadPreimage'
                    elif t in computed:
                        c = 'pendingRootTriad'
                    elif t in private:
                        c = 'privateProvenance'
                    elif t in filesha:
                        c = 'fileSha256'
                    elif t in keys:
                        c = 'fixtureKey(notExported)'
                    elif t in packed:
                        c = 'packedValue'
                    elif len(set(b)) == 1:
                        c = 'synthetic(uniform)'
                    elif LC_HASH.fullmatch(t):
                        c = 'synthetic(lc)'
                    elif int(t, 16) < 2 ** 128:
                        c = 'smallInteger'
                    else:
                        c = 'unclassified'
                        unclassified.append([f.relative_to(ROOT).as_posix(), t])
                    tally[c] += 1
    check('HF.coverage.every64HexClassified', files > 0 and not unclassified, files=files, byClass=dict(tally), unclassified=unclassified[:20])


def write_asset(name, text, label):
    RES.mkdir(parents=True, exist_ok=True)
    p = RES / name
    data = text.encode('utf-8')
    if p.exists() and p.read_bytes() != data:
        k = 1
        while (RES / ('%s-rerun-%d%s' % (p.stem, k, p.suffix))).exists():
            k += 1
        alt = RES / ('%s-rerun-%d%s' % (p.stem, k, p.suffix))
        alt.write_bytes(data)
        check(label + '.matchesExisting', False, kept=name, written=alt.name, sha256=hashlib.sha256(data).hexdigest())
        return
    if not p.exists():
        p.write_bytes(data)
    check(label + '.written', True, file=REL + 'results/' + name, sha256=hashlib.sha256(data).hexdigest(), bytes=len(data))


# ------------------------------------------------------------------ 6. supplements

def load_sr12_and_r10():
    SR = load_module('m1-draft-0.12/tools/sr_ref.py', 'sr_ref_012_s25')
    saved = sys.modules.get('sr_ref')
    sys.modules['sr_ref'] = SR                                     # run_checks_010's 'import sr_ref' resolves to the 0.12 World
    try:
        R10 = load_module('m1-draft-0.10/tools/run_checks_010.py', 'run_checks_010_s25')   # main() not called
    finally:
        if saved is None:
            sys.modules.pop('sr_ref', None)
        else:
            sys.modules['sr_ref'] = saved
    return SR, R10


def sweepmax(SR, R10):
    doc = load_json(REL + 'supplements/s25-sweepmax.json')
    provenance('S25.sweepMax.source', doc['source'])
    provenance('S25.sweepMax.criterion', doc['criterionSource'])
    check('S25.sweepMax.usesAcceptedWorld', R10.SR is SR and SR.World is SR.BASE_MODULE.World)
    base = load_json('m1-draft-0.10/vectors/e3-br22a.json')
    SM = S.sweepmax_world(SR)
    diff, n, pending_rm = [], 0, []
    for combo in R10.combos():
        n += 1
        w0, ev0, _, _ = R10.br22a_world(base, combo)
        w1, ev1, _, _ = S.br22a_run(R10, SM, base, combo, sweep_max=False)
        if S.world_fingerprint(w0) != S.world_fingerprint(w1) or ev0 != ev1:
            diff.append(combo)
        if any(op.gen == 1 and op.kind == 'remove' and op.state == 'pending' for op in w0.be.ops):
            pending_rm.append(combo)
        pos = S.br22a_positions(combo)
        la = combo['L'].get('A', 'L0')
        if la != 'L0' and not any(e['do'] == 'place' and e['cat'] == 'record' and e['gen'] == 1 and e['t'] == pos[la] for e in ev0):
            diff.append({'positions': combo})
    check('S25.sweepMax.equivalence', n == 520 and not diff, combinations=n, differing=diff[:5])
    check('S25.sweepMax.noGen1RemoveWhenCorrect', not pending_rm, combos=pending_rm[:5])
    c = doc['control']
    w, ev, R, W = S.br22a_run(R10, SM, base, c['combo'], sweep_max=True)
    res = R10.criteria(w, c['combo'], R, W)
    failed = sorted(k for k, v in res.items() if not v)
    kinds = [v['kind'] for v in w.violations]
    check('S25.sweepMax.control', all(x in failed for x in c['mustFail']) and all(k in kinds for k in c['mustViolate']),
          failedCriteria=failed, violations=kinds, trace=[x for x in w.trace if x['ev'] in ('sweepAtIssue', 'lateApply', 'death')][:8])
    w0, ev0, R0, W0 = S.br22a_run(R10, SM, base, doc['baselineSameCell']['combo'], sweep_max=False)
    res0 = R10.criteria(w0, doc['baselineSameCell']['combo'], R0, W0)
    gs = w0.gen_summary()
    e_ok = all(gs.get(g, {}).get('E') == v for g, v in R10.e_rule(doc['baselineSameCell']['combo'], R0, W0).items() if v is not None)
    check('S25.sweepMax.baselineSameCellPasses', all(res0.values()) and not w0.violations and e_ok, criteria=res0)
    grid, fails = 0, Counter()
    for first in ('P1', 'P3', 'fail'):
        for second in ('none', 'P4', 'P5'):
            for extra in ('no', 'yes'):
                labels = ['A'] + (['F2'] if second == 'P4' else []) + (['CP2'] if second == 'P5' else []) + ['RM']
                for ps in itertools.product(['L0', 'L1', 'L2', 'L3', 'L4', 'L5'], repeat=len(labels)):
                    combo = {'first': first, 'second': second, 'extraDeath': extra, 'L': dict(zip(labels, ps))}
                    w, ev, R, W = S.br22a_run(R10, SM, base, combo, sweep_max=True)
                    grid += 1
                    bad = tuple(sorted(k for k, v in R10.criteria(w, combo, R, W).items() if not v))
                    if bad:
                        fails[(first, combo['L']['RM'], bad)] += 1
    record('S25.sweepMax.grid', 'recorded', runs=grid, failing=sum(fails.values()),
           distribution=[[f, rm, list(b), k] for (f, rm, b), k in sorted(fails.items())])


def lc_literal():
    doc = load_json(REL + 'supplements/s25-lc-literal.json')
    provenance('S25.lclit.source', doc['source'])
    R19 = load_module('m1-draft-0.19/tools/run_checks_019.py', 'run_checks_019_s25')     # main() not called
    M = R19.M
    data = load_json('m1-draft-0.19/vectors/lc-data.json')
    cases = load_json('m1-draft-0.19/vectors/lc-cases.json')
    fx = R19.Fixture(data)

    def generate():
        runs, infid = [], []
        for case in cases['cases']:
            script = fx.resolve_script_value(case.get('script', {}))
            rec = S.RecordingServer(M.MockLogServer(data['branches'], 'A', data['head'], script, M.Timeline()))
            out = S.run_lc(M, R19.Tee, case, rec)
            if S.run_outcome(out) != S.run_outcome(R19.run_case(fx, data, case)):
                infid.append(case['id'])
            runs.append({'id': case['id'], 'mode': case['mode'], 'from': case['from'], 'to': case['to'], 'params': case.get('params', {}),
                         'fault': None, 'requests': rec.transcript})
        for c in cases['controls']:
            case = next(x for x in cases['cases'] if x['id'] == c['case'])
            script = fx.resolve_script_value(case.get('script', {}))
            rec = S.RecordingServer(M.MockLogServer(data['branches'], 'A', data['head'], script, M.Timeline()))
            S.run_lc(M, R19.Tee, case, rec, faults=(c['fault'],))
            runs.append({'id': '%s@%s' % (case['id'], c['fault']), 'mode': case['mode'], 'from': case['from'], 'to': case['to'],
                         'params': case.get('params', {}), 'fault': c['fault'], 'requests': rec.transcript})
        return runs, infid

    runs, infid = generate()
    check('S25.lclit.generatorFidelity', not infid and len(runs) == len(cases['cases']) + len(cases['controls']), unfaithful=infid, runs=len(runs))
    body = {'schema': 'pocol-m1-lc-literal-replies/0.25', 'supplement': 'S25-LCLIT', 'generator': 'm1-draft-0.19 MockLogServer over m1-draft-0.19/vectors/lc-cases.json',
            'replayRule': doc['form']['replayRule'], 'runs': runs}
    text = json.dumps(body, sort_keys=True, separators=(',', ':'), ensure_ascii=True) + '\n'
    runs2, _ = generate()
    check('S25.lclit.exportDeterministic', json.dumps(runs2, sort_keys=True, separators=(',', ':'), ensure_ascii=True)
          == json.dumps(runs, sort_keys=True, separators=(',', ':'), ensure_ascii=True))
    portable = json.loads(text)['runs']
    by_id = {c['id']: c for c in cases['cases']}
    for r in portable:
        if r['fault'] is None:
            case = by_id[r['id']]
            lit = S.LiteralServer(r['requests'], M.Timeline())
            run = S.run_lc(M, R19.Tee, case, lit)
            bad = [cell for cell, want, act in R19.case_cells(fx, case, run) if want != act]
            check('S25.lclit.replay.' + r['id'], not bad and not lit.mismatches and len(lit.log) == len(r['requests']),
                  badCells=bad, mismatches=lit.mismatches[:3], served=len(lit.log), transcript=len(r['requests']))
        else:
            cid = r['id'].split('@')[0]
            case = by_id[cid]
            lit = S.LiteralServer(r['requests'], M.Timeline())
            run = S.run_lc(M, R19.Tee, case, lit, faults=(r['fault'],))
            script = fx.resolve_script_value(case.get('script', {}))
            gen = S.run_lc(M, R19.Tee, case, M.MockLogServer(data['branches'], 'A', data['head'], script, M.Timeline()), faults=(r['fault'],))
            check('S25.lclit.replayControl.' + r['id'], S.run_outcome(run) == S.run_outcome(gen) and not lit.mismatches, mismatches=lit.mismatches[:3])
    write_asset('lc-literal-replies-0.25.json', text, 'S25.lclit.export')
    return R19


def rg3b(R19):
    doc = load_json(REL + 'supplements/s25-rg3b.json')
    provenance('S25.rg3b.source', doc['source'])
    provenance('S25.rg3b.fixtureSource', doc['fixtureSource'])
    STRICT = load_module('m1-draft-0.20/tools/sink_checker_strict.py', 'sink_checker_strict_s25')
    M = R19.M
    data = load_json('m1-draft-0.19/vectors/lc-data.json')
    fx = R19.Fixture(data)
    for run in doc['modelAnalogue']['runs']:
        frm, to = run['range']
        tl = M.Timeline()
        mock = M.MockLogServer(data['branches'], 'A', data['head'], None, tl)
        rec = S.RecordingServer(mock)
        ref = M.RefConsumer()
        hc = S.HookConsumer(ref, mock, run['hooks'])
        cl, sink, res = M.run_client(rec, M.Clock(), dict(M.FETCH_EACH), frm, to, hc)
        skel = S.event_skeleton(sink.events)
        reqs = [list(e['req']) for e in mock.log]
        anchors = [e[2]['hash'] for e in sink.events if e[0] == 'begin']
        commits = [e for e in sink.events if e[0] == 'commit']
        viol = STRICT.StrictSinkChecker().check(tl.entries, frm, to)
        cells = {'skeleton': (skel == run['skeleton'], skel),
                 'requests': (reqs == fx.resolve(run['requests']), reqs),
                 'beginAnchors': (anchors == fx.resolve(run['beginAnchors']), anchors),
                 'result': (res.get('ok') is True and ref.result == fx.resolve(run['result']), len(ref.result or [])),
                 'discarded': (ref.discarded == [[i, fx.resolve(x)] for i, x in run['discarded']], [[i, len(x)] for i, x in ref.discarded]),
                 'totalRequests': (bool(commits) and commits[-1][2]['totalRequests'] == len(mock.log), len(mock.log)),
                 'strictSinkChecker': (viol == [], viol[:4]),
                 'hooksFired': (all(h['fired'] for h in hc.hooks), [h['fired'] for h in hc.hooks])}
        if 'anchorNotCanonicalAtRequest' in run:
            k = run['anchorNotCanonicalAtRequest']
            rp = rec.transcript[k - 1]['reply'] if len(rec.transcript) >= k else {}
            cells['anchorNotCanonical'] = (rp.get('error', {}).get('code') == -32022 and rp['error']['data']['reason'] == 'anchorNotCanonical', rp)
        bad = {n: v[1] for n, v in cells.items() if not v[0]}
        check('S25.rg3b.%s' % run['id'], not bad, failingCells=bad)
    check('S25.rg3b.scriptsComplete', [s['id'] for s in doc['scripts']] == ['RG3b-main', 'RG3b-ABA', 'RG3b-beforeAnchor']
          and all(s['pass'] for s in doc['scripts']) and all(k in doc['hookProtocol'] for k in ('hookPoint', 'submit', 'adoptionSignal', 'timeLimit', 'preconditions')))


def e07():
    doc = load_json(REL + 'supplements/s25-e07-viewer-table.json')
    provenance('S25.e07.source', doc['source'])
    provenance('S25.e07.baseline', doc['baselineSource'])
    check('S25.e07.rows', [r['id'] for r in doc['rows']] == ['T43-%d' % i for i in range(1, 10)])
    for r in doc['rows']:
        for pol in ('interstitial', 'refuse'):
            want = r['expect'] if 'expect' in r else r['expectByU02'][pol]
            got = S.select_version(r['state'], r['request'], pol)
            check('S25.e07.%s.%s' % (r['id'], pol), got == want, got=got, want=want)
    sel = load_json('m1-draft-0.3/vectors/selector-cases.json')
    case = next((c for c in sel['cases'] if c['id'] == 'sel-u32-max-plus-1'), None)
    check('S25.e07.malformedSelectorFromFixture', case is not None and case['input'] == doc['rows'][3]['request']['text']
          and case['expected']['kind'] == 'malformedSelector')


def experiment_specs():
    doc = load_json(REL + 'supplements/s25-experiment-specs.json')
    provenance('S25.expspec.list', doc['sources']['list'])
    check('S25.expspec.ids', [e['id'] for e in doc['experiments']] == ['E01', 'E02', 'E03', 'E04', 'E05', 'E06', 'E07'])
    missing = []
    for e in doc['experiments']:
        for i in e['inputs']:
            path = i.split(' ')[0].rstrip(',;')
            if '/' in path and not (ROOT / path).exists():
                missing.append([e['id'], path])
        if not e['pass'] or 'definitionComplete' not in e:
            missing.append([e['id'], 'pass or definitionComplete'])
    for r in doc['rowExperimentsDefinedElsewhere']:
        if not (ROOT / r['file']).exists():
            missing.append([r['id'], r['file']])
    check('S25.expspec.inputsExist', not missing, missing=missing)


# ------------------------------------------------------------------ 7. representation

def cr_e4_01(SR):
    doc = load_json(REL + 'representation/cr-e4-01-u64-epoch.json')
    for k, src in doc['sources'].items():
        provenance('CR-E4-01.source.' + k, src)
    C = load_module('m1-draft-0.13/tools/fmt2_codec.py', 'fmt2_codec_s25')
    fx = doc['fixtures']
    bad = []
    for r in fx['roundTrip']:
        E = int(r['epoch'])
        v = S.encode_value_hex(C, E, fx['seq'], fx['dict'])
        back = S.decode_value_hex(C, (E, fx['seq']), json.loads(C.compact_json(v)))
        js = S.js_like_loads('{"epoch":%s}' % r['epoch'])['epoch']
        if v['epoch'] != r['hex16'] or back != (fx['dict'], False) or js != int(r['jsonNumberReadAs']):
            bad.append(r['epoch'])
    check('CR-E4-01.roundTrip', not bad, bad=bad)
    bad = []
    for r in fx['decodeRejects']:
        v = S.encode_value_hex(C, 1, 0, fx['dict'])
        v['epoch'] = r['epoch']
        try:
            S.decode_value_hex(C, (int(r['ver'][0]), r['ver'][1]), v)
            got = None
        except S.ReprError as e:
            got = e.reason
        if got != r['reason']:
            bad.append([r['id'], got])
    check('CR-E4-01.decodeRejects', not bad, bad=bad)
    m = fx['maxRecord']
    d = {'K%04x' % i: 'a' * 251 for i in range(4096)}
    v = S.encode_value_hex(C, 2 ** 64 - 1, 2 ** 32 - 1, d)
    name = SR.record_name('0x' + '1' * 64, '0x' + '2' * 40, 2 ** 64 - 1, 2 ** 32 - 1)
    enc = C.enc_size(name, v)
    check('CR-E4-01.maxRecord', len(name) == 142 and len(C.compact_json(v)) == m['valueJsonLength'] and enc == m['encSize']
          and (enc <= C.RECORD_BYTES_MAX) == m['withinRecordBytesMax'] and 66 + (C.RECORD_BYTES_MAX - enc) == m['netKeyBound'],
          name=len(name), json=len(C.compact_json(v)), enc=enc)
    try:
        C.encode_value(int(fx['oldCodecPreserved']['epoch']), 0, {})
        old = None
    except C.CodecError as e:
        old = e.reason
    check('CR-E4-01.oldCodecUnchanged', old == fx['oldCodecPreserved']['oldEncodeReason'], got=old)


def cr_e4_02():
    doc = load_json(REL + 'representation/cr-e4-02-surrogates.json')
    for k, src in doc['sources'].items():
        provenance('CR-E4-02.source.' + k, src)
    B = load_module('m1-draft-0.4/tools/bridge_ref.py', 'bridge_ref_04_s25')
    C = load_module('m1-draft-0.13/tools/fmt2_codec.py', 'fmt2_codec_s25b')
    old = next(c for c in load_json('m1-draft-0.4/vectors/bridge-check-cases.json')['cases'] if c['id'] == 'BR17d-lone-surrogate')
    r = B.check(old['raw'], 1000, B.Bucket(1000))
    check('CR-E4-02.existingCaseBR17d', r['stage'] == old['expected']['stage'] == 'B3' and r['data'] == old['expected']['data'], got=[r['stage'], r['data']])
    for c in doc['bridgeCases']['cases']:
        r = B.check(c['raw'], 1000, B.Bucket(1000))
        bad = {k: [v, r.get(k)] for k, v in c['expected'].items() if r.get(k) != v}
        check('CR-E4-02.bridge.' + c['id'], not bad, mismatches=bad)
    for c in doc['scalarCodecCases']['cases']:
        k = bytes.fromhex(c['keyUnits']).decode('utf-16-be')
        v = bytes.fromhex(c['valueUnits']).decode('utf-16-be')
        pairs = C.serialize_pairs({k: v})
        ok = C.parse_pairs(pairs) == {k: v} and S.textencoder_len(S.units_from_hex(c['keyUnits'])) == len(k.encode('utf-8'))
        if 'utf8KeyHex' in c:
            ok = ok and k.encode('utf-8').hex() == c['utf8KeyHex']
        if 'utf8ValueHex' in c:
            ok = ok and v.encode('utf-8').hex() == c['utf8ValueHex']
        check('CR-E4-02.scalarCodec.' + c['id'], ok)
    w = doc['wtf8Alternative']
    bad = []
    for e in w['encode']:
        u = S.units_from_hex(e['units'])
        b = S.wtf8_encode(u)
        ok = b.hex() == e['wtf8'] and S.textencoder_len(u) == e['textEncoderLen'] == len(b) and S.wtf8_decode(b) == u \
            and (not S.has_lone_surrogate(u)) == e['scalar']
        if e['scalar']:
            ok = ok and b == bytes.fromhex(e['units']).decode('utf-16-be').encode('utf-8')
        if not ok:
            bad.append(e['id'])
    check('CR-E4-02.wtf8.encode', not bad, bad=bad)
    bad = []
    for e in w['decodeRejects']:
        try:
            S.wtf8_decode(bytes.fromhex(e['hex']))
            got = None
        except S.ReprError as x:
            got = x.reason
        if got != e['reason']:
            bad.append([e['id'], got])
    check('CR-E4-02.wtf8.decodeRejects', not bad, bad=bad)
    cr = w['collisionResolved']
    pairs = [[S.units_from_hex(k), S.units_from_hex(v)] for k, v in cr['pairs']]
    pb = S.wtf8_pairs(pairs)
    e4 = load_json('m1-draft-0.13/vectors/e4-units.json')['surrogateCollision']
    lossy = C.serialize_pairs_textencoder({'\ud800': '1', '\ud801': '2'})
    check('CR-E4-02.wtf8.collisionResolved', pb.hex() == cr['pairsHex'] and S.wtf8_parse_pairs(pb) == pairs
          and lossy.hex() == cr['textEncoderPairsHex'] == e4['textEncoderPairsHex'])
    bo = w['byteOrder']
    us = {h: S.units_from_hex(h) for h in bo['units']}
    check('CR-E4-02.wtf8.byteOrder', sorted(bo['units'], key=lambda h: S.wtf8_encode(us[h])) == bo['wtf8Order']
          and sorted(bo['units'], key=lambda h: us[h]) == bo['utf16Order'])
    p = w['property']
    rng = random.Random(p['seed'])
    pools = [(0x0000, 0x007F), (0x0080, 0x07FF), (0x0800, 0xD7FF), (0xD800, 0xDBFF), (0xDC00, 0xDFFF), (0xE000, 0xFFFF)]
    bad = []
    for _ in range(p['samples']):
        u = [rng.randint(*pools[rng.randrange(6)]) for _ in range(rng.randint(0, p['maxUnits']))]
        b = S.wtf8_encode(u)
        ok = S.textencoder_len(u) == len(b) and S.wtf8_decode(b) == u
        if ok and not S.has_lone_surrogate(u):
            ok = b == b''.join(x.to_bytes(2, 'big') for x in u).decode('utf-16-be').encode('utf-8')
        if not ok:
            bad.append(u)
            break
    check('CR-E4-02.wtf8.property', not bad, samples=p['samples'], firstBad=bad)


def cid_1():
    doc = load_json(REL + 'representation/cid-1-profile-chainid.json')
    for k, src in doc['sources'].items():
        provenance('CID-1.source.' + k, src)
    for f in doc['fixtures']:
        for mode, fn in (('exact', S.chainid_exact), ('double', S.chainid_double)):
            got = fn(f['text'])
            want = f[mode]
            ok = got['ok'] == want['ok'] and (('value' not in want) or str(got.get('value')) == want['value']) \
                and (('reason' not in want) or got.get('reason') == want['reason'])
            check('CID-1.%s.%s' % (f['id'], mode), ok, got=got, want=want)
    K = load_module('m1-draft-0.2/tools/keccak.py', 'keccak_025b')
    RL = load_module('m1-draft-0.2/tools/rlp_strict.py', 'rlp_strict_02_s25')
    gh = bytes.fromhex(doc['collision']['genesisHash'][2:])
    a, b = (int(x) for x in doc['collision']['pair'])
    pa, pb = (RL.encode([b'PoCol-net-v1', RL.uint(x), gh]) for x in (a, b))
    da, db = (S.js_number_int(str(x)) for x in (a, b))
    check('CID-1.collision', pa != pb and K.keccak256(pa) != K.keccak256(pb) and da == db,
          netKeyExact=[K.keccak256(pa).hex(), K.keccak256(pb).hex()], doubleRead=[da, db])
    v1 = load_json('m1-draft-0.21/vectors/v3-profiles.json')
    ids = sorted({int(m) for m in re.findall(r'"chainId": (\d+)', json.dumps(v1))})
    check('CID-1.existingFixturesJsSafe', bool(ids) and max(ids) <= 2 ** 53 - 1, chainIds=ids)


# ------------------------------------------------------------------ 1. boundary, coverage, main

def boundary_static():
    bad = {}
    for f in (HERE / 'supplements_ref.py', HERE / 'run_checks_025.py'):
        tree = ast.parse(f.read_text(encoding='utf-8'))
        mods = {a.name.split('.')[0] for n in ast.walk(tree) if isinstance(n, ast.Import) for a in n.names}
        mods |= {(n.module or '').split('.')[0] for n in ast.walk(tree) if isinstance(n, ast.ImportFrom)}
        hit = sorted(mods & {'socket', 'urllib', 'http', 'requests', 'ssl', 'asyncio', 'subprocess'})
        if hit:
            bad[f.name] = hit
    js = (HERE / 'js_number_probe.cjs').read_text(encoding='utf-8')
    if any(x in js for x in ('require(', 'fetch(', 'http', 'writeFile')):
        bad['js_number_probe.cjs'] = 'require/fetch/http/writeFile present'
    check('boundary.noNetworkModules', not bad, imports=bad)
    markers = ('PoCol_' + 'Dialogue', 'state' + '.json', 'source' + 'State', 'historical-x8-full' + '-source-private', 'transcript' + '-private')
    leaks = [str(f.relative_to(PKG)) for f in sorted(PKG.rglob('*')) if f.is_file() and 'results' not in f.parts
             and f.suffix in ('.json', '.md', '.js', '.cjs', '.html', '.py') and any(m in f.read_text(encoding='utf-8') for m in markers)]
    check('boundary.noPrivatePathInPackage', not leaks, files=leaks)
    deps = [str(f.relative_to(PKG)) for f in sorted(PKG.rglob('*'))
            if f.suffix.lower() in ('.exe', '.dll', '.node', '.so', '.pyd', '.whl', '.zip', '.lock', '.wasm') or f.name in ('package.json', 'Cargo.toml')
            or 'node_modules' in f.parts or 'runtime' in f.parts]
    check('boundary.noDependencyOrBinary', not deps, files=deps)


def coverage(computed):
    have = {r['check']: r['status'] for r in results}
    need = ['S25.sweepMax.equivalence', 'S25.sweepMax.control', 'S25.sweepMax.baselineSameCellPasses', 'S25.sweepMax.noGen1RemoveWhenCorrect',
            'S25.lclit.generatorFidelity', 'S25.lclit.exportDeterministic', 'S25.rg3b.M-main', 'S25.rg3b.M-midAttempt', 'S25.rg3b.M-ABA',
            'S25.rg3b.M-beforeAnchor', 'S25.rg3b.scriptsComplete', 'S25.e07.rows', 'S25.expspec.inputsExist',
            'HF.triad.bindingCurrentFixtures', 'HF.triad.threeLibrariesAgree', 'HF.e05.GSV1', 'HF.coverage.every64HexClassified', 'HF.export.noKeyMaterial',
            'CR-E4-01.roundTrip', 'CR-E4-01.maxRecord', 'CR-E4-02.wtf8.property', 'CID-1.collision', 'gaps.allClassified', 'inventory.rows',
            'findings.all26', 'decision.ledger.ownerItemsStillOpen', 'decision.ownerSetExact']
    need += ['row.%s.statusMatchesProposal' % r for r in ROW_IDS] + ['findings.F%02d.dispositionConsistent' % i for i in range(1, 27)]
    missing = [x for x in need if have.get(x) != 'pass']
    lclit = [k for k in have if k.startswith('S25.lclit.replay.')]
    missing += [] if len(lclit) == 45 else ['S25.lclit.replay.* (%d of 45)' % len(lclit)]
    check('coverage025.required', not missing, missing=missing, required=len(need) + 1)
    check('coverage025.noStepAborted', not [k for k in have if 'step.completed' in k])


def out_path():
    p = RES / 'run-results-0.25.json'
    k = 1
    while p.exists():
        p = RES / ('run-results-0.25-rerun-%d.json' % k)
        k += 1
    return p


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
    hash_status, computed, SR, R10, R19 = {}, {}, None, None, None
    steps = [('boundary', boundary_static), ('superseded', superseded_failures), ('reviewCopies', review_copies), ('decisions', decision_register)]
    for name, fn in steps:
        try:
            fn()
        except Exception as ex:                                              # record, never hide
            check('step.completed ' + name, False, exception='%s: %s' % (type(ex).__name__, ex))
    try:
        hash_status = hash_freeze()
    except Exception as ex:
        check('step.completed hashFreeze', False, exception='%s: %s' % (type(ex).__name__, ex))
    try:
        SR, R10 = load_sr12_and_r10()
        sweepmax(SR, R10)
    except Exception as ex:
        check('step.completed sweepMax', False, exception='%s: %s' % (type(ex).__name__, ex))
    try:
        R19 = lc_literal()
        rg3b(R19)
    except Exception as ex:
        check('step.completed logClient', False, exception='%s: %s' % (type(ex).__name__, ex))
    for name, fn in (('e07', e07), ('expspec', experiment_specs), ('crE402', cr_e4_02), ('cid1', cid_1), ('findings', findings), ('gaps', gap_registry)):
        try:
            fn()
        except Exception as ex:
            check('step.completed ' + name, False, exception='%s: %s' % (type(ex).__name__, ex))
    try:
        cr_e4_01(SR if SR is not None else load_sr12_and_r10()[0])
    except Exception as ex:
        check('step.completed crE401', False, exception='%s: %s' % (type(ex).__name__, ex))
    try:
        computed = inventory(hash_status)
    except Exception as ex:
        check('step.completed inventory', False, exception='%s: %s' % (type(ex).__name__, ex))
    gap('gap.V1-authoring', text='V1 window vectors, HC message texts and NX-V3e/f/g are not authored (V1-WINDOW, V1-TEXTS, V1-NX); NX-V3* need the M3-TS/ASERT model')
    gap('gap.ownerDecisions', text='U01, U02, U10, U14, CR-M1-01 and RF-E6-1 have no named owner answer in the ledger')
    gap('gap.reviewerDecisions', text='U/P conventions in audit/decision-register.json await recorded root decisions')
    gap('gap.pendingRootTriad', text='new preimage groups of hash-freeze-export-0.25.json need three-library verification by root')
    coverage(computed)
    after = {rel: sha(ROOT / rel) for rel in PRESERVED}
    check('preserved.olderFilesUnchanged', before == after, before=before, after=after)
    failed = [r for r in results if r['status'] == 'FAIL']
    out = {'package': 'M1 draft 0.25 (consolidated M1 specification audit, supplements, hash-freeze export, representation alternatives)',
           'executedAtUtc': datetime.now(timezone.utc).isoformat(timespec='seconds'),
           'environment': {'python': sys.version.split()[0], 'executable': sys.executable, 'platform': platform.platform(),
                           'thirdPartyPackages': 'none'},
           'statusSchema': {'coreStatus': list(CORE_STATUS), 'diagnosticRenames': RENAMED, 'partialGap': 'recorded entries with partialGap=true'},
           'evidenceKind': 'reference models and static audit over saved, reviewed evidence; no browser, network, EVM or chain',
           'notExecuted': ['E01-E04, E06, E07 (Phase A)', 'three-library verification of pendingRootTriad entries (root)', 'any owner decision'],
           'summary': {'checks': len(results), 'passed': sum(r['status'] == 'pass' for r in results),
                       'recorded': sum(r['status'] == 'recorded' for r in results), 'failed': len(failed),
                       'partialGaps': sum(1 for r in results if r.get('partialGap')),
                       'rows': dict(Counter(v['status'] for v in computed.values())) if computed else None,
                       'seconds': round(time.time() - t0, 1)},
           'results': results}
    RES.mkdir(parents=True, exist_ok=True)
    target = out_path()
    target.write_text(json.dumps(out, indent=2, ensure_ascii=True, default=str) + '\n', encoding='utf-8')
    print(json.dumps(out['summary']), '->', target.name)
    for f in failed:
        print('FAIL', json.dumps(f, ensure_ascii=True, default=str)[:400])
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
