"""Reference-only runner for M1 draft 0.22: C30 repair (non-recursive GenesisSpec framing parser) and the
E05 GSV1 hash supplement. Python standard library only. NOT executed by the author.

Usage, from D:\\PoCol-Development:
  coordination\\runtime\\python311\\python.exe m1-draft-0.22\\tools\\run_checks_022.py
Writes only m1-draft-0.22/results/run-results-0.22.json and exits 1 on any FAIL.

Steps:
  1. Load m1-draft-0.21/tools/run_checks_021.py as a module (its main() is not called, its result is not
     written); its model module R21.NP is the unchanged 0.21 netprofile_ref.
  2. With the ORIGINAL parse: every deep fixture reproduces RecursionError (the root probe included).
     Shallow fixtures, GSV1 and the 32 old negatives give identical results with both parsers.
  3. Install tools/iterative_parse.make_parse(NP.GsError) as NP.parse (the global decode_genesis uses).
  4. C30 fixtures: deep and shallow twins, decode and validate paths, repeated calls, GSV1 afterwards.
  5. E05 supplement: GSV1 input SHA-256 and the three-library Keccak-256.
  6. Replay of run_checks_021.main() body with the repaired parser. The one harness check that compared
     absolute paths (reuse.keccakFrom0.2) is replaced by a content-hash plus origin-role check; the
     preserved review failure caused by that path comparison is recorded as a harness mismatch.
"""

import ast
import copy
import hashlib
import importlib.util
import json
import platform
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
PKG, ROOT = HERE.parent, HERE.parent.parent
RES = PKG / 'results'
OUT = RES / 'run-results-0.22.json'
VEC = 'm1-draft-0.22/vectors/'
CORE_STATUS = ('pass', 'recorded', 'FAIL')
RENAMED = {'name': 'diagName', 'check': 'diagCheck', 'status': 'proposalStatus', 'ok': 'diagOk',
           'label': 'diagLabel', 'condition': 'diagCondition'}
COPY_FAILURE = 'coordination/review-001/run-results-0.21-copy-path-failure.json'
INPUTS = ['coordination/task-020.md', 'coordination/review-001/REVIEW-0.21.md', 'coordination/review-001/genesis-depth-independent-probe.py',
          'coordination/review-001/genesis-depth-independent-probe.json', 'coordination/review-001/e05-keccak-three-library.json',
          'coordination/review-001/e05-hash-inputs.json', COPY_FAILURE, 'reference/consensus.md', 'reference/validation.md',
          'm1-draft-0.21/tools/netprofile_ref.py', 'm1-draft-0.21/tools/run_checks_021.py', 'm1-draft-0.21/vectors/v3-gsv1.json',
          'm1-draft-0.21/vectors/v3-profiles.json', 'm1-draft-0.2/tools/keccak.py', 'm1-draft-0.2/tools/rlp_strict.py',
          'm1-draft-0.22/tools/iterative_parse.py', 'm1-draft-0.22/tools/run_checks_022.py', VEC + 'c30-depth.json', VEC + 'v3-e05-supplement.json']
PRESERVED = ['m1-draft-0.21/results/run-results-0.21.json', 'coordination/review-001/m1-draft-0.21/results/run-results-0.21.json',
             COPY_FAILURE, 'coordination/review-001/genesis-depth-independent-probe.json', 'coordination/issue-ledger.json',
             'm1-draft-0.2/tools/keccak.py', 'm1-draft-0.2/tools/rlp_strict.py', 'm1-draft-0.21/tools/netprofile_ref.py']

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


def sha(p):
    return hashlib.sha256(p.read_bytes()).hexdigest() if p.exists() else None


def load_module(rel, modname):
    spec = importlib.util.spec_from_file_location(modname, str(ROOT / rel))
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def load_json(rel):
    return json.loads((ROOT / rel).read_text(encoding='utf-8'))


def provenance(label, src):
    lines = (ROOT / src['file']).read_text(encoding='utf-8').splitlines()
    a, b = src['lines']
    text = '\n'.join(lines[a - 1:b])
    missing = [lit for lit in src.get('literals', []) if lit not in text]
    check(label + '.provenance', not missing, file=src['file'], lines=src['lines'], missingLiterals=missing)


sys.path.insert(0, str(HERE))
import iterative_parse as IP          # noqa: E402


# ------------------------------------------------------------------ iterative fixture construction

def list_header(n):
    if n < 56:
        return bytes([0xc0 + n])
    lb = n.to_bytes((n.bit_length() + 7) // 8, 'big')
    return bytes([0xf7 + len(lb)]) + lb


def wrap(inner, depth):
    b = bytes(inner)
    for _ in range(depth):                                       # loop, never recursion
        b = list_header(len(b)) + b
    return b


def build(R21, gtree, case, depth):
    inner = bytes.fromhex(case['inner'])
    if case['place'] == 'top':
        b = wrap(inner, depth)
    else:
        t = copy.deepcopy(gtree)
        nested = wrap(inner, depth).hex()
        if case['place'] == 'gsv1Append':
            t.append(nested)
        else:
            node = t
            for i in case['path'][:-1]:
                node = node[i]
            node[case['path'][-1]] = nested
        b = R21.apply_edits(t, case.get('edits', []))
    if case.get('dropLast'):
        b = b[:-1]
    if 'suffix' in case:
        b = b + bytes.fromhex(case['suffix'])
    return b


def outcome(fn, b):
    try:
        r = fn(b)
        return {'ok': True, 'value': r}
    except RecursionError:
        return {'exception': 'RecursionError'}
    except Exception as e:                                       # GsError carries code/detail; others are reported
        if hasattr(e, 'code'):
            return {'code': e.code, 'detail': str(e.detail)}
        return {'exception': type(e).__name__}


# ------------------------------------------------------------------ C30 steps

def reproduce_and_equivalence(R21, NP, orig_parse, new_parse, cdoc, gdoc):
    provenance('C30', cdoc['source'])
    probe = load_json(cdoc['rootProbe']['file'])
    check('C30.rootProbeRecord', probe['depth'] == cdoc['rootProbe']['depth'] and probe['bytes'] == cdoc['rootProbe']['bytes']
          and probe['actual'].get('exception') == cdoc['rootProbe']['oldActual'], probe=probe)
    for case in cdoc['cases']:
        b = build(R21, gdoc['tree'], case, case['deep']['depth'])
        old = outcome(orig_parse, b)
        want_old = case.get('oldDeep', {'exception': 'RecursionError'})
        check('C30.old.%s.deep' % case['id'], old == want_old, old=old, expectedOld=want_old, length=len(b),
              note='the published 0.21 parser on this input')
        s = build(R21, gdoc['tree'], case, case['shallow']['depth'])
        o1, o2 = outcome(orig_parse, s), outcome(new_parse, s)
        check('C30.equivalent.%s.shallow' % case['id'], o1 == o2, original=o1 if 'value' not in o1 else 'tree', repaired=o2 if 'value' not in o2 else 'tree')
    others = [('GSV1', R21.flat(gdoc['flatSegments']))] + [('neg.' + n['id'], R21.apply_edits(gdoc['tree'], n['edits'])) for n in gdoc['negatives']]
    diff = [name for name, b in others if outcome(orig_parse, b) != outcome(new_parse, b)]
    check('C30.equivalent.gsv1AndAll32Negatives', not diff and len(others) == 33, differing=diff, compared=len(others))


def c30_cases(R21, NP, cdoc, gdoc, pdoc):
    gsv1 = R21.flat(gdoc['flatSegments'])
    by_case = {}
    for case in cdoc['cases']:
        want = case['expect']
        for twin in ('deep', 'shallow'):
            b = build(R21, gdoc['tree'], case, case[twin]['depth'])
            first, second = outcome(NP.decode_genesis, b), outcome(NP.decode_genesis, b)
            ok = (first == second == {'code': want['code'], 'detail': want['detail']} and len(b) == case[twin]['length'])
            check('C30.%s.%s' % (case['id'], twin), ok, expected=want, first=first, second=second, length=len(b), expectedLength=case[twin]['length'],
                  why=case.get('why'))
            if twin == 'deep':
                by_case[case['id']] = b
        after = outcome(NP.decode_genesis, gsv1)
        check('C30.%s.gsv1StillDecodes' % case['id'], after.get('ok') is True and NP.encode_genesis(after['value']) == gsv1)
    base = pdoc['baseProfile']
    for p in cdoc['profiles']:
        b = by_case[p['case']]
        prof = copy.deepcopy(base)
        prof.update(profileId=p['id'], genesisPre='0x' + b.hex(), genesisHash='0x' + NP.keccak256(b).hex())
        r1 = NP.validate(copy.deepcopy(prof), {'signed': True})
        r2 = NP.validate(copy.deepcopy(prof), {'signed': True})
        got = {'ok': r1['ok'], 'error': {'code': (r1['error'] or {}).get('code')}, 'trace': ['%s:%s' % (t[0], t[1]) for t in r1['trace']]}
        check('C30.profile.' + p['id'], got == p['expect'] and r1['error'] == r2['error'], expected=p['expect'], actual=got)
    gp = copy.deepcopy(base)
    gp.update(genesisPre='0x' + gsv1.hex(), genesisHash='0x' + NP.keccak256(gsv1).hex())
    check('C30.profile.gsv1AfterRejections', NP.validate(gp, {'signed': True})['ok'] is True)


def e05(NP, gdoc, R21):
    doc = load_json(VEC + 'v3-e05-supplement.json')
    provenance('E05', doc['source'])
    gsv1 = R21.flat(gdoc['flatSegments'])
    g = doc['GSV1']
    check('E05.gsv1InputSha256', len(gsv1) == g['bytes'] and hashlib.sha256(gsv1).hexdigest() == g['inputSha256'])
    check('E05.gsv1PythonKeccak', NP.keccak256(gsv1).hex() == g['keccak256'], computed=NP.keccak256(gsv1).hex())
    ev = load_json(doc['evidence']['file'])
    rows = {r['id']: r for r in ev['results']}
    agree = rows['GSV1']['pythonKeccak'] == rows['GSV1']['nobleKeccak'] == rows['GSV1']['jsSha3Keccak'] == g['keccak256']
    k_ok = all(rows[k]['pythonKeccak'] == rows[k]['nobleKeccak'] == rows[k]['jsSha3Keccak'] == v for k, v in doc['K'].items())
    check('E05.threeLibraryAgreement', agree and k_ok and ev['failed'] == 0 and rows['GSV1']['inputSha256'] == g['inputSha256'],
          libraries=[l['name'] + ' ' + l['version'] for l in ev['libraries']])
    check('E05.pythonLibraryIsProtectedSource', ev['pythonLibrarySha256'] == sha(ROOT / 'm1-draft-0.2/tools/keccak.py'))
    inp = load_json(doc['evidence']['inputs'])
    check('E05.inputHexIsFixtureBytes', next(x for x in inp['inputs'] if x['id'] == 'GSV1')['hex'] == gsv1.hex())
    record('DG-V3-3.resolvedByE05', 'recorded', H_GSV1='0x' + g['keccak256'], note=doc['supersedes'])


# ------------------------------------------------------------------ replay of the 0.21 suite

def role_content_ok(mod, rel, expected_sha=None):
    """Harness repair: the module must have the protected content and origin role, wherever the copy lives."""
    p = Path(mod.__file__).resolve()
    canon = ROOT / rel
    same = sha(p) == sha(canon) and (expected_sha is None or sha(p) == expected_sha)
    role = p.name == canon.name and p.parent.name == 'tools' and p.parent.parent.name == 'm1-draft-0.2'
    return same and role, {'loaded': str(p.relative_to(ROOT)) if ROOT in p.parents else p.name, 'sha256': sha(p), 'canonicalSha256': sha(canon)}


def replay_021(R21, e05_python_sha):
    """run_checks_021.main() body (lines 380-424) without its result-file write; reuse.keccakFrom0.2 replaced."""
    before = {rel: R21.sha(ROOT / rel) for rel in R21.PRESERVED}
    for rel in R21.INPUTS:
        R21.record('input.sha256 ' + rel, 'recorded', exists=(ROOT / rel).exists(), sha256=R21.sha(ROOT / rel))
    R21.check('boundary.positionalOnly', R21.record.__code__.co_posonlyargcount == 2 and R21.check.__code__.co_posonlyargcount == 2
              and R21.gap.__code__.co_posonlyargcount == 1)
    calls = [n.lineno for n in ast.walk(ast.parse((ROOT / 'm1-draft-0.21/tools/run_checks_021.py').read_text(encoding='utf-8')))
             if isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute) and n.func.attr == 'main']
    R21.check('boundary.noOldMainCalled', not calls, attributeMainCallsAtLines=calls)
    k_ok, k_diag = role_content_ok(R21.NP.K, 'm1-draft-0.2/tools/keccak.py', e05_python_sha)
    r_ok, r_diag = role_content_ok(R21.NP.RLP, 'm1-draft-0.2/tools/rlp_strict.py')
    R21.check('reuse.keccakFrom0.2', k_ok and r_ok, keccak=k_diag, rlp=r_diag,
              note='0.22 harness: content hash + origin role instead of one absolute directory (C30 harness note)')
    gdoc = R21.load_json(R21.VEC + 'v3-gsv1.json')
    pdoc = R21.load_json(R21.VEC + 'v3-profiles.json')
    try:
        gsv1, h = R21.gsv1_suite(gdoc)
        sys.path.insert(0, str(ROOT / 'm1-draft-0.2' / 'tools'))
        spec = importlib.util.spec_from_file_location('eth_keys_ref_v3r', str(ROOT / 'm1-draft-0.5/tools/eth_keys_ref.py'))
        keys_mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(keys_mod)
        builder = R21.Builder(gsv1, gdoc, pdoc, keys_mod)
        for name, fn in (('profiles', lambda: R21.profile_suite(builder, pdoc, {h})), ('transport', lambda: R21.transport_suite(builder, pdoc)),
                         ('reuse', R21.reuse_checks)):
            try:
                fn()
            except Exception as ex:                                          # record, never hide
                R21.check('step.completed ' + name, False, exception='%s: %s' % (type(ex).__name__, ex))
    except Exception as ex:                                                  # record, never hide
        R21.check('step.completed gsv1', False, exception='%s: %s' % (type(ex).__name__, ex))
    for gid, text in R21.GAPS:
        R21.gap('gap.' + gid, text=text)
    R21.coverage()
    after = {rel: R21.sha(ROOT / rel) for rel in R21.PRESERVED}
    R21.check('preserved.olderFilesUnchanged', before == after, before=before, after=after)
    for r in R21.results:
        r = dict(r)
        r['check'] = 'suite021.' + r['check']
        results.append(r)


def harness_note():
    doc = load_json(COPY_FAILURE)
    rows = {r['check']: r['status'] for r in doc['results']}
    failed = [k for k, v in rows.items() if v == 'FAIL']
    check('C30.harness.copyPathFailurePreserved', failed == ['reuse.keccakFrom0.2'] and doc['summary']['failed'] == 1,
          failed=failed, sha256=sha(ROOT / COPY_FAILURE))
    record('C30.harness.classification', 'recorded',
           text='The first 0.21 review run imported the preserved review copy of m1-draft-0.2 tools; the 0.21 check compared absolute paths '
                'and failed although the content was identical. It is a harness mismatch, not a model semantic failure. The preserved '
                'failure result is kept unchanged; the 0.22 replay compares content hashes and origin roles instead.')


def coverage(cdoc):
    have = {r['check']: r['status'] for r in results}
    need = []
    for c in cdoc['cases']:
        need += ['C30.%s.deep' % c['id'], 'C30.%s.shallow' % c['id'], 'C30.old.%s.deep' % c['id'],
                 'C30.equivalent.%s.shallow' % c['id'], 'C30.%s.gsv1StillDecodes' % c['id']]
    need += ['C30.profile.' + p['id'] for p in cdoc['profiles']] + ['C30.equivalent.gsv1AndAll32Negatives', 'C30.install.parseInModelGlobals',
             'E05.threeLibraryAgreement', 'suite021.reuse.keccakFrom0.2', 'suite021.coverage021.required', 'suite021.coverage021.rfVariants',
             'suite021.coverage021.decodeNegatives', 'suite021.preserved.olderFilesUnchanged']
    bad = [x for x in need if have.get(x) != 'pass']
    check('coverage022.required', not bad, missing=bad, required=len(need))
    s21 = [k for k in have if k.startswith('suite021.')]
    check('coverage022.suite021NoFail', s21 and all(have[k] != 'FAIL' for k in s21), entries=len(s21))
    check('coverage022.noStepAborted', not [k for k in have if 'step.completed' in k])


def main():
    t0 = time.time()
    before = {rel: sha(ROOT / rel) for rel in PRESERVED}
    for rel in INPUTS:
        record('input.sha256 ' + rel, 'recorded', exists=(ROOT / rel).exists(), sha256=sha(ROOT / rel))
    check('boundary.positionalOnly', record.__code__.co_posonlyargcount == 2 and check.__code__.co_posonlyargcount == 2)
    calls = [n.lineno for n in ast.walk(ast.parse(Path(__file__).read_text(encoding='utf-8')))
             if isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute) and n.func.attr == 'main']
    check('boundary.noOldMainCalled', not calls, attributeMainCallsAtLines=calls)
    cdoc = load_json(VEC + 'c30-depth.json')
    try:
        R21 = load_module('m1-draft-0.21/tools/run_checks_021.py', 'run_checks_021_suite022')       # main() not called
        NP = R21.NP
        gdoc = load_json('m1-draft-0.21/vectors/v3-gsv1.json')
        pdoc = load_json('m1-draft-0.21/vectors/v3-profiles.json')
        orig_parse = NP.parse
        new_parse = IP.make_parse(NP.GsError)
        reproduce_and_equivalence(R21, NP, orig_parse, new_parse, cdoc, gdoc)
        NP.parse = new_parse                                                                       # in-memory install
        check('C30.install.parseInModelGlobals', NP.decode_genesis.__globals__['parse'] is new_parse and orig_parse is not new_parse
              and getattr(new_parse, 'c30_iterative', False) and Path(NP.__file__).resolve() == (ROOT / 'm1-draft-0.21/tools/netprofile_ref.py').resolve())
        c30_cases(R21, NP, cdoc, gdoc, pdoc)
        e05_doc = load_json(VEC + 'v3-e05-supplement.json')
        e05(NP, gdoc, R21)
        harness_note()
        replay_021(R21, '99aa7770d2966ccbc804eb913cd97d54e15bba627c4ee7f71dc3f248a2f91a1d')
        record('E05.supplementRecorded', 'recorded', GSV1=e05_doc['GSV1'])
    except Exception as ex:                                                  # record, never hide
        check('step.completed suite', False, exception='%s: %s' % (type(ex).__name__, ex))
    record('gap.open', 'recorded', partialGap=True,
           items=['DG-V3-1, 2, 4..9 and P-V3-1..7 from 0.21 remain open (replayed unchanged)', 'V3-scope: model only, no TS/Chrome/Rust/real transport'])
    coverage(cdoc)
    after = {rel: sha(ROOT / rel) for rel in PRESERVED}
    check('preserved.olderFilesUnchanged', before == after, before=before, after=after)
    failed = [r for r in results if r['status'] == 'FAIL']
    out = {'package': 'M1 draft 0.22 (C30 non-recursive GenesisSpec framing parser; E05 GSV1 hash supplement; full 0.21 replay)',
           'executedAtUtc': datetime.now(timezone.utc).isoformat(timespec='seconds'),
           'environment': {'python': sys.version.split()[0], 'executable': sys.executable, 'platform': platform.platform(),
                           'thirdPartyPackages': 'none', 'recursionLimit': sys.getrecursionlimit()},
           'statusSchema': {'coreStatus': list(CORE_STATUS), 'diagnosticRenames': RENAMED, 'partialGap': 'recorded entries with partialGap=true'},
           'evidenceKind': 'reference model of the specification text; no network, no node',
           'notExecuted': ['TypeScript/Chrome options page', 'Rust GenesisSpec decoder', 'real HttpTransport'],
           'summary': {'checks': len(results), 'passed': sum(r['status'] == 'pass' for r in results),
                       'recorded': sum(r['status'] == 'recorded' for r in results), 'failed': len(failed),
                       'partialGaps': sum(1 for r in results if r.get('partialGap')),
                       'suite021Entries': sum(1 for r in results if r['check'].startswith('suite021.')),
                       'seconds': round(time.time() - t0, 1)},
           'results': results}
    RES.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(out, indent=2, ensure_ascii=True, default=str) + '\n', encoding='utf-8')
    print(json.dumps(out['summary']))
    for f in failed:
        print('FAIL', json.dumps(f, ensure_ascii=True, default=str)[:400])
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
