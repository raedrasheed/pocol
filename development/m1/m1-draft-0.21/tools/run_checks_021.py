"""Reference-only runner for M1 draft 0.21: V3 NetworkProfiles.validate (GSV1, L4n, RF1-RF4).
Python standard library only. NOT executed by the author.

Usage, from D:\\PoCol-Development:
  coordination\\runtime\\python311\\python.exe m1-draft-0.21\\tools\\run_checks_021.py
Writes only m1-draft-0.21/results/run-results-0.21.json and exits 1 on any FAIL.

Reuses read-only: m1-draft-0.2/tools/keccak.py and rlp_strict.py (via tools/netprofile_ref.py),
m1-draft-0.5/tools/eth_keys_ref.py (fixture keys 3-6, proposal TK-1). No older main() is called and
no older result is written. Expected values are the hand-written fixtures in ../vectors.
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
OUT = RES / 'run-results-0.21.json'
VEC = 'm1-draft-0.21/vectors/'
CORE_STATUS = ('pass', 'recorded', 'FAIL')
RENAMED = {'name': 'diagName', 'check': 'diagCheck', 'status': 'proposalStatus', 'ok': 'diagOk',
           'label': 'diagLabel', 'condition': 'diagCondition'}
INPUTS = ['coordination/task-019.md', 'coordination/review-001/REVIEW-0.20.md', 'coordination/issue-ledger.json',
          'reference/validation.md', 'reference/browser.md', 'reference/consensus.md', 'reference/governance.md', 'reference/implementation.md',
          'm1-draft-0.2/tools/keccak.py', 'm1-draft-0.2/tools/rlp_strict.py', 'm1-draft-0.5/tools/eth_keys_ref.py', 'm1-draft-0.4/annex/recv-limits.json',
          'm1-draft-0.21/tools/netprofile_ref.py', 'm1-draft-0.21/tools/run_checks_021.py', VEC + 'v3-gsv1.json', VEC + 'v3-profiles.json']
PRESERVED = ['m1-draft-0.20/results/run-results-0.20.json', 'coordination/review-001/m1-draft-0.20/results/run-results-0.20.json',
             'coordination/issue-ledger.json', 'm1-draft-0.2/tools/keccak.py', 'm1-draft-0.2/tools/rlp_strict.py']
REQUIRED = ['GSV1.flatEqualsTree', 'GSV1.length', 'GSV1.decode', 'GSV1.reencode', 'GSV1.hashRecorded', 'K1', 'K2', 'K3',
            'neg.N1', 'neg.N6', 'profile.N1', 'profile.N6', 'profile.N11', 'profile.RF1', 'profile.RF2', 'profile.RF3', 'profile.RF4',
            'profile.RF3-edge-equal', 'profile.RF3-edge-over', 'profile.RF4-edge-ok', 'profile.RF4-edge-over',
            'profile.RFC-edge-equal', 'profile.RFC-edge-over', 'profile.RFT-edge-equal', 'profile.RFT-edge-over',
            'profile.GSV1-signed', 'profile.GSV1-confirmed', 'profile.GSV1-unconfirmed'] + ['neg.N%d' % i for i in range(2, 11)]
GAPS = [
    ('DG-V3-1', 'The network profile genesisPre is not literal in the source: g_ts, target_g, the four HELD M_0 identities, alloc (allocRoot) and sysCodeHash are absent. RF1 uses network CP values with placeholders; its decision depends only on field widths and |genesisPre| (439 B).'),
    ('DG-V3-2', 'end/full allocRoot and sysCodeHash need node-side alloc and system code; placeholders used. w..z come from TK-1 (0.5 proposal); the well-known addresses are an author recollection to be confirmed by an independent root oracle.'),
    ('DG-V3-3', 'H_GSV1 must be agreed by three Keccak libraries after K1-K3 (validation.md:75 iii). Only the accepted pure-Python Keccak runs here: H_GSV1 is recorded, not certified.'),
    ('DG-V3-4', 'The decode error name for the first stage (structure) is not given; this model reports gsStructure (P-V3-2).'),
    ('DG-V3-5', 'consensus.md:27 puts field relations in ParamGate, yet gamma_bp <= 10^4 - alpha_bp is written in the CP field list and |M_0List| bounds in the M_0List rules. Model: gamma relation and |M_0List| >= 1 at decode; M_min/M_max bounds left to ParamGate (R12). Proposal P-V3-3.'),
    ('DG-V3-6', 'networks.json fetch: response cap, deadline and pool reservation semantics are not specified; fixtures use proposals P-V3-6/P-V3-7 (1048576 B, 10000 ms inclusive, reserve recvLimit from TRUST_POOL).'),
    ('DG-V3-7', 'forkSchedule (R24 b-d) is not among the acceptance steps of browser.md:11-16; validate checks its JSON shape only. Step 3 "chainId match" is read as profile.chainId = GenesisSpec.chainId; the endpoint chainId is the netKey check at open (browser.md:19).'),
    ('DG-V3-8', 'No string/URL rules are given for profile fields; validate rejects non-canonical hex and wrong types without normalizing (P-V3-1); endpoint URL syntax is not validated.'),
    ('DG-V3-9', 'implementation.md:292 says NetworkProfiles.validate imports asert and the M3-TS model; no acceptance step uses ASERT. This model does not evaluate ASERT; the dependency is recorded, not faked.'),
    ('P-V3-4', 'Identity line format "<name> · chainId <n> · genesisHash <full 0x hash>" is a proposal for the step-4 confirmation (the source requires the full hash only).'),
    ('P-V3-5', 'A validated profile never starts a node in this model: node start needs node-side R24(a) on alloc; GSV1 is additionally on a known-non-bootable list.'),
    ('V3-scope', 'Model only: no TypeScript/Chrome options page, no Rust decoder, no real HttpTransport, no signed networks.json signature scheme (signature verification is an input flag).'),
]

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


def provenance(label, src):
    lines = (ROOT / src['file']).read_text(encoding='utf-8').splitlines()
    a, b = src['lines']
    text = '\n'.join(lines[a - 1:b])
    missing = [lit for lit in src.get('literals', []) if lit not in text]
    check(label + '.provenance', not missing, file=src['file'], lines=src['lines'], missingLiterals=missing)


sys.path.insert(0, str(HERE))
import netprofile_ref as NP           # noqa: E402


# ------------------------------------------------------------------ literal byte construction (independent of the model)

def leaf(v):
    if isinstance(v, str):
        return bytes.fromhex(v)
    if isinstance(v, dict) and 'repeat' in v:
        return bytes.fromhex(v['repeat']) * v['count']
    if isinstance(v, dict) and 'concat' in v:
        return b''.join(leaf(x) for x in v['concat'])
    raise ValueError(v)


def list_header(n):
    if n < 56:
        return bytes([0xc0 + n])
    lb = n.to_bytes((n.bit_length() + 7) // 8, 'big')
    return bytes([0xf7 + len(lb)]) + lb


def serialize(t):
    if isinstance(t, list):
        body = b''.join(serialize(x) for x in t)
        return list_header(len(body)) + body
    return leaf(t)


def flat(segs):
    return b''.join(leaf(s) for s in segs)


def at(tree, path):
    node = tree
    for i in path[:-1]:
        node = node[i]
    return node, path[-1]


def apply_edits(tree, edits):
    t = copy.deepcopy(tree)
    post = []
    for e in edits:
        if e['op'] == 'delete':
            p, i = at(t, e['path'])
            del p[i]
        elif e['op'] == 'set':
            p, i = at(t, e['path'])
            p[i] = copy.deepcopy(e['value'])
        elif e['op'] == 'swap':
            pa, ia = at(t, e['a'])
            pb, ib = at(t, e['b'])
            pa[ia], pb[ib] = pb[ib], pa[ia]
        elif e['op'] == 'copy':
            pf, i_f = at(t, e['from'])
            pt, it = at(t, e['to'])
            pt[it] = copy.deepcopy(pf[i_f])
        else:
            post.append(e)
    b = serialize(t)
    for e in post:
        if e['op'] == 'appendBytes':
            b = b + bytes.fromhex(e['hex'])
        elif e['op'] == 'setOuterHeader':
            h = bytes.fromhex(e['hex'])
            b = h + b[len(h):]
        else:
            raise ValueError(e)
    return b


def num(v):
    return 2 ** v['pow2'] if isinstance(v, dict) and 'pow2' in v else v


# ------------------------------------------------------------------ GSV1 and L4n

def gsv1_suite(doc):
    provenance('GSV1', doc['source'])
    provenance('GSV1.l4n', doc['l4nSource'])
    provenance('GSV1.order', doc['orderSource'])
    provenance('GSV1.svFix', doc['expected']['svFixCrossCheck']['source'])
    provenance('K', doc['keccakVectors']['source'])
    for k in ('K1', 'K2', 'K3'):
        inp, want = doc['keccakVectors'][k]
        check(k, NP.keccak256(bytes.fromhex(inp)).hex() == want, input=inp)
    b = flat(doc['flatSegments'])
    ex = doc['expected']
    check('GSV1.flatEqualsTree', b == serialize(doc['tree']), flatLen=len(b))
    check('GSV1.length', len(b) == ex['length'] and len(serialize(doc['tree'][2])) == ex['cpEncodedLength']
          and len(serialize(doc['tree'][4])) == ex['m0EncodedLength'] and len(b) - 3 == ex['payloadLength'],
          length=len(b), cp=len(serialize(doc['tree'][2])), m0=len(serialize(doc['tree'][4])))
    spec = NP.decode_genesis(b)
    want_cp = [num(v) for v in ex['cp']]
    got_cp = [spec['CP'][n] for n, _, _, _ in NP.CP_FIELDS]
    check('GSV1.decode', spec['specVersion'] == ex['specVersion'] and spec['chainId'] == ex['chainId'] and got_cp == want_cp
          and spec['allocRoot'] == leaf(ex['allocRoot']) and spec['sysCodeHash'] == leaf(ex['sysCodeHash'])
          and spec['M_0List'] == [[leaf(i), leaf(r)] for i, r in ex['m0']],
          mismatchedFields=[NP.CP_FIELDS[k][0] for k in range(52) if got_cp[k] != want_cp[k]])
    check('GSV1.reencode', NP.encode_genesis(spec) == b)
    h = '0x' + NP.keccak256(b).hex()
    record('GSV1.hashRecorded', 'recorded', H_GSV1=h, libraries=['m1-draft-0.2/tools/keccak.py (pure Python)'], certified=False,
           note='three-library agreement (validation.md:75 iii) is outstanding: DG-V3-3')
    for n in doc['negatives']:
        nb = apply_edits(doc['tree'], n['edits'])
        try:
            NP.decode_genesis(nb)
            got = 'ok'
        except NP.GsError as e:
            got = e.code
        diag = {'expected': n['expect'], 'actual': got, 'length': len(nb)}
        ok = got == n['expect']
        if 'lengths' in n:
            ok = ok and len(nb) == n['lengths']['length']
        check('neg.' + n['id'], ok, **diag)
    return b, h


# ------------------------------------------------------------------ profiles and RF

def m0_entries(rule, n, gsv1_spec, keys_mod, endfull_known):
    if rule == 'gsv1':
        return copy.deepcopy(gsv1_spec['M_0List'])
    if rule in ('seq', 'network4'):
        lead = 0x10 if rule == 'seq' else 0x20
        ids = [bytes([lead]) + (i + 1).to_bytes(19, 'big') for i in range(n)]
        return [[i, i] for i in ids]
    if rule == 'endfull':
        seed = keys_mod.bip39_seed()
        named = [bytes.fromhex(keys_mod.address(keys_mod.fixture_key(i, seed))[2:]) for i in (3, 4, 5, 6)]
        check('RF2.fixtureKeys3to6MatchKnownAddresses', ['0x' + a.hex() for a in named] == endfull_known,
              derived=['0x' + a.hex() for a in named], known=endfull_known)
        gen = [NP.keccak256(b'PoColEnd' + i.to_bytes(4, 'big'))[12:] for i in range(1020)]
        ids = sorted(named + gen)
        bad = len(set(ids)) != len(ids) or any(i == bytes(20) or i == NP.SYSTEM_ADDRESS or
                                                (i[:17] == bytes(17) and 0xC0C001 <= int.from_bytes(i[17:], 'big') <= 0xC0C0FF) for i in ids)
        check('RF2.fixtureSetupIds', not bad and len(ids) == n, count=len(ids))
        return [[i, i] for i in ids]
    raise ValueError(rule)


class Builder:
    def __init__(self, gsv1_bytes, gdoc, pdoc, keys_mod):
        self.gsv1 = gsv1_bytes
        self.gdoc, self.pdoc = gdoc, pdoc
        self.spec = NP.decode_genesis(gsv1_bytes)
        self.keys = keys_mod
        self.cache = {}

    def genesis(self, build):
        if build['kind'] == 'gsv1':
            return self.gsv1
        if build['kind'] == 'negative':
            n = next(x for x in self.gdoc['negatives'] if x['id'] == build['id'])
            return apply_edits(self.gdoc['tree'], n['edits'])
        spec = copy.deepcopy(self.spec)
        spec['chainId'] = build['chainId']
        for k, v in build.get('cp', {}).items():
            spec['CP'][k] = num(v)
        m = build['m0']
        spec['M_0List'] = m0_entries(m['rule'], m.get('n'), self.spec, self.keys, self.pdoc['knownTestAddresses']['addresses'])
        return NP.encode_genesis(spec)

    def profile(self, case):
        if case['id'] in self.cache:
            return copy.deepcopy(self.cache[case['id']])
        gp = self.genesis(case['build'])
        prof = copy.deepcopy(self.pdoc['baseProfile'])
        prof['profileId'] = case['id']
        prof['genesisPre'] = '0x' + gp.hex()
        prof['genesisHash'] = '0x' + NP.keccak256(gp).hex()
        prof.update(copy.deepcopy(case.get('profilePatch', {})))
        tr = case.get('transform')
        if tr == 'upperGenesisPre':
            prof['genesisPre'] = '0x' + prof['genesisPre'][2:].upper()
        elif tr == 'dropLastNibble':
            prof['genesisPre'] = prof['genesisPre'][:-1]
        self.cache[case['id']] = (prof, gp)
        return copy.deepcopy((prof, gp))


def resolve(v, gh):
    if isinstance(v, str):
        return v.replace('@GH', gh)
    if isinstance(v, dict):
        return {k: resolve(x, gh) for k, x in v.items()}
    if isinstance(v, list):
        return [resolve(x, gh) for x in v]
    return v


def profile_suite(builder, pdoc, nonbootable):
    for k in ('acceptance', 'limits', 'rf', 'network', 'endfull', 'trustPool'):
        provenance('V3.' + k, pdoc['sources'][k])
    for case in pdoc['cases']:
        prof, gp = builder.profile(case)
        gh = '0x' + NP.keccak256(gp).hex()
        ex = resolve(case['expect'], gh)
        trust = resolve(case['trust'], gh)
        res = NP.validate(copy.deepcopy(prof), trust)
        lbl = 'profile.' + case['id']
        cells = [('ok', ex['ok'], res['ok'])]
        if 'error' in ex:
            got = {k: res['error'].get(k) for k in ex['error']} if res['error'] else None
            cells.append(('error', ex['error'], got))
        if 'trace' in ex:
            cells.append(('trace', ex['trace'], ['%s:%s' % (t[0], t[1]) for t in res['trace']]))
        if 'length' in ex:
            cells.append(('length', ex['length'], len(gp)))
        if 'worst' in ex:
            rf = next((t for t in res['trace'] if t[0] == 'recvFit'), None)
            cells.append(('worst', ex['worst'], [r['worst'] for r in rf[2]] if rf else None))
        if 'identityLine' in ex:
            tr = next((t for t in res['trace'] if t[0] == 'trust'), None)
            cells.append(('identityLine', ex['identityLine'], tr[2] if tr and len(tr) > 2 else None))
        if 'startNode' in ex:
            cells.append(('startNode', ex['startNode'], list(NP.may_start_node(prof, res, nonbootable))))
        bad = [{'cell': c, 'expected': w, 'actual': a} for c, w, a in cells if w != a]
        check(lbl, not bad, mismatches=bad, derivation=case.get('derivation'), error=res['error'])
        if res['ok']:
            check(lbl + '.netKeyShape', isinstance(res.get('netKey'), str) and len(res['netKey']) == 66)


def transport_suite(builder, pdoc):
    by_id = {c['id']: c for c in pdoc['cases']}
    for c in pdoc['transport']['cases']:
        script = []
        for s in c['script']:
            s2 = {k: v for k, v in s.items() if k != 'bodyProfiles'}
            if 'bodyProfiles' in s:
                s2['body'] = json.dumps([builder.profile(by_id[p])[0] for p in s['bodyProfiles']], separators=(',', ':'))
            script.append(s2)
        tp = NP.FakeTransport(script, reserved=c['reserved'])
        res = NP.fetch_and_validate(tp, 'https://example.invalid/networks.json', {'signed': True})
        ex = c['expect']
        cells = [('ok', ex['ok'], res['ok'])]
        if 'error' in ex:
            cells.append(('error', ex['error'], res['error']))
        if 'profilesOk' in ex:
            cells.append(('profilesOk', ex['profilesOk'], [p['ok'] for p in res['profiles']]))
        if 'log' in ex:
            cells.append(('log', ex['log'], tp.log[-1][1] if tp.log else None))
        cells.append(('poolReleased', c['reserved'], tp.reserved))
        bad = [{'cell': x, 'expected': w, 'actual': a} for x, w, a in cells if w != a]
        check('transport.' + c['id'], not bad, mismatches=bad)


def reuse_checks():
    acc = load_json('m1-draft-0.4/annex/recv-limits.json')
    rows = {tuple(r['methods']): r for r in acc['rows']}
    want = {('eth_getCode',): 69632, ('pocol_getParams',): 98304, ('eth_getBlockByHash', 'eth_getBlockByNumber'): 167936, ('eth_getTransactionByHash',): 266240}
    check('reuse.acceptedRecvTableLimits', all(rows[k]['limit'] == v for k, v in want.items()),
          limits={'/'.join(k): rows[k]['limit'] for k in want})
    check('reuse.modelRowsMatchAccepted', [r[1] for r in NP.RECV_FIT_ROWS] == [69632, 98304, 167936, 266240])
    check('reuse.rf3rf4AcceptedArithmetic', 2 * 47105 + 4096 == 98306 and 2048 + (262144 // 85) * 70 == 217928)


def coverage():
    have = {r['check']: r['status'] for r in results}
    missing = [x for x in REQUIRED if have.get(x) not in ('pass', 'recorded')]
    check('coverage021.required', not missing, missing=missing)
    rf = [k for k in have if k.startswith('profile.RF') and not k.endswith('.netKeyShape')]
    check('coverage021.rfVariants', len(rf) == 15 and all(have[k] == 'pass' for k in rf), rfChecks=sorted(rf))
    neg = [k for k in have if k.startswith('neg.')]
    check('coverage021.decodeNegatives', len(neg) == 32 and all(have[k] == 'pass' for k in neg), count=len(neg))
    check('coverage021.noStepAborted', not [k for k in have if 'step.completed' in k])


def main():
    t0 = time.time()
    before = {rel: sha(ROOT / rel) for rel in PRESERVED}
    for rel in INPUTS:
        record('input.sha256 ' + rel, 'recorded', exists=(ROOT / rel).exists(), sha256=sha(ROOT / rel))
    check('boundary.positionalOnly', record.__code__.co_posonlyargcount == 2 and check.__code__.co_posonlyargcount == 2
          and gap.__code__.co_posonlyargcount == 1)
    calls = [n.lineno for n in ast.walk(ast.parse(Path(__file__).read_text(encoding='utf-8')))
             if isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute) and n.func.attr == 'main']
    check('boundary.noOldMainCalled', not calls, attributeMainCallsAtLines=calls)
    check('reuse.keccakFrom0.2', Path(NP.K.__file__).resolve() == (ROOT / 'm1-draft-0.2/tools/keccak.py').resolve()
          and Path(NP.RLP.__file__).resolve() == (ROOT / 'm1-draft-0.2/tools/rlp_strict.py').resolve())
    gdoc = load_json(VEC + 'v3-gsv1.json')
    pdoc = load_json(VEC + 'v3-profiles.json')
    try:
        gsv1, h = gsv1_suite(gdoc)
        sys.path.insert(0, str(ROOT / 'm1-draft-0.2' / 'tools'))                    # eth_keys_ref imports keccak, rlp_strict
        spec = importlib.util.spec_from_file_location('eth_keys_ref_v3', str(ROOT / 'm1-draft-0.5/tools/eth_keys_ref.py'))
        keys_mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(keys_mod)
        builder = Builder(gsv1, gdoc, pdoc, keys_mod)
        for name, fn in (('profiles', lambda: profile_suite(builder, pdoc, {h})), ('transport', lambda: transport_suite(builder, pdoc)),
                         ('reuse', reuse_checks)):
            try:
                fn()
            except Exception as ex:                                          # record, never hide
                check('step.completed ' + name, False, exception='%s: %s' % (type(ex).__name__, ex))
    except Exception as ex:                                                  # record, never hide
        check('step.completed gsv1', False, exception='%s: %s' % (type(ex).__name__, ex))
    for gid, text in GAPS:
        gap('gap.' + gid, text=text)
    coverage()
    after = {rel: sha(ROOT / rel) for rel in PRESERVED}
    check('preserved.olderFilesUnchanged', before == after, before=before, after=after)
    failed = [r for r in results if r['status'] == 'FAIL']
    out = {'package': 'M1 draft 0.21 (V3 NetworkProfiles.validate: GSV1, L4n, RF1-RF4, trust-pool fetch)',
           'executedAtUtc': datetime.now(timezone.utc).isoformat(timespec='seconds'),
           'environment': {'python': sys.version.split()[0], 'executable': sys.executable, 'platform': platform.platform(),
                           'thirdPartyPackages': 'none'},
           'statusSchema': {'coreStatus': list(CORE_STATUS), 'diagnosticRenames': RENAMED, 'partialGap': 'recorded entries with partialGap=true'},
           'evidenceKind': 'reference model of the specification text; FakeTransport only; no network, no node',
           'notExecuted': ['TypeScript/Chrome options page', 'Rust GenesisSpec decoder', 'three-library H_GSV1 agreement (M0)', 'real HttpTransport'],
           'summary': {'checks': len(results), 'passed': sum(r['status'] == 'pass' for r in results),
                       'recorded': sum(r['status'] == 'recorded' for r in results), 'failed': len(failed),
                       'partialGaps': sum(1 for r in results if r.get('partialGap')), 'seconds': round(time.time() - t0, 1)},
           'results': results}
    RES.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(out, indent=2, ensure_ascii=True, default=str) + '\n', encoding='utf-8')
    print(json.dumps(out['summary']))
    for f in failed:
        print('FAIL', json.dumps(f, ensure_ascii=True, default=str)[:400])
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
