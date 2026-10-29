"""Reference-only acceptance runner for M1 draft 0.31 (author turn 029): C35 native error-data codec and C36 evidence
bindings, with the recomputed 41-row gate. NOT executed by the author; root executes it in a preserved copy.

Runtime (stated plainly, not "pure Python"): Python standard library PLUS the locally installed Node.js (expected v22.13.1,
the version of root's native oracles) running the fixed codec tools/json_codec_031.mjs through tools/guard_ref_031.py
(subprocess, shell=False, stdin only, bounded time and output). Node is found from --node <path>, POCOL_NODE, or PATH.
If Node or the codec is missing or behaves unexpectedly, the dependent checks FAIL; there is no Python fallback.

Usage: run_checks_031.py [--root <tree>] [--out <dir>] [--node <node executable>]
Writes only under out, never overwriting an earlier run (a differing later run gets a -rerun-<n> name):
  run-results-0.31.json, acceptance-matrix-0.31.json, bindings-0.31.json, status-0.31.json, dashboard-0.31-ar.json.
Reuses unchanged earlier tools by path (0.30 runner helpers and guard, 0.29 overlay models, 0.27 checker, 0.4 parser).
Rebuilds no header and recomputes no freeze hash: those are bound to root's 0.30 results by status and sha256.
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
import shutil
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
NODE = _arg('--node', 'POCOL_NODE') or shutil.which('node')
CODEC = HERE / 'json_codec_031.mjs'
OWN = ['tools/run_checks_031.py', 'tools/guard_ref_031.py', 'tools/json_codec_031.mjs', 'vectors/c35-native-codec-cases.json',
       'vectors/c36-binding-repairs.json', 'audit/acceptance-inventory-0.31.json', 'M1-SPEC-0.31-AMENDMENT.md', 'M1-STATUS-0.31.md',
       'M1-DASHBOARD-0.31-AR.md', 'README.md']
INPUTS_ROOT = ['coordination/task-029.md', 'coordination/review-001/REVIEW-0.30.md', 'coordination/review-001/REVIEW-DECISIONS-0.30.json',
               'coordination/review-001/m1-draft-0.30/results/run-results-0.30.json', 'coordination/review-001/m1-draft-0.30/results/acceptance-matrix-0.30.json',
               'coordination/review-001/m1-draft-0.30/results/hash-freeze-0.30.json', 'coordination/review-001/m1-draft-0.30/results/bindings-0.30.json',
               'coordination/review-001/error-data-native-oracle-030.json', 'coordination/review-001/error-data-native-expanded-031.json',
               'coordination/review-001/error-data-native-comparison-030.json', 'coordination/review-001/error-data-native-comparison-utf8-030.json',
               'coordination/review-001/guard-independent-030.json', 'coordination/review-001/guard-corpus-independent-030.json',
               'coordination/review-001/v1-envelope-scope-probes-029.json', 'coordination/DELEGATED-M1-CONVENTIONS-030.json',
               'coordination/issue-ledger.json', 'm1-draft-0.30/tools/guard_ref_030.py', 'm1-draft-0.30/tools/run_checks_030.py',
               'm1-draft-0.30/vectors/c33-guard-cases.json', 'm1-draft-0.30/decisions/conventions-applied-0.30.json']
PRESERVED = INPUTS_ROOT[1:] + ['coordination/review-001/m1-draft-0.27/results/v1-transcripts-0.27.json']

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
    return json.loads((ROOT / rel).read_text(encoding='utf-8'))             # explicit UTF-8 reader everywhere


def own(rel):
    return json.loads((PKG / rel).read_text(encoding='utf-8'))


def path_of(rel):
    return PKG / rel[len('m1-draft-0.31/'):] if rel.startswith('m1-draft-0.31/') else ROOT / rel


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


# ------------------------------------------------------------------ modules: the unchanged 0.30 runner supplies every earlier tool

R30m = load_module(ROOT / 'm1-draft-0.30/tools/run_checks_030.py', 'run_checks_030_031')
V, O, B04, G30, KEC = R30m.V, R30m.O, R30m.B04, R30m.G, R30m.KEC
sys.path.insert(0, str(HERE))
import guard_ref_031 as G31           # noqa: E402  (this package only)

INV = own('audit/acceptance-inventory-0.31.json')
RES = {k: json.loads((ROOT / v).read_text(encoding='utf-8'))['results'] for k, v in INV['results'].items()}
CONV = rj('coordination/DELEGATED-M1-CONVENTIONS-030.json')
APPL30 = rj(INV['base']['conventions'])
DEC25 = rj('coordination/review-001/REVIEWER-DECISIONS-0.25.json')
ACCEPTED = {d['id'] for d in DEC25['decisions'] if d['status'] == 'accepted-technical-qualified'} | {'P-C32-1', 'P-C32-3', 'P-C32-4'}
M30 = rj(INV['base']['matrix0_30'])
APPLIED, DECIDED = set(), set()
CODECOBJ = None


def ev_ok(key, pattern):
    rx = re.compile(pattern)
    hit = [r for r in (results if key == 'run' else RES[key]) if type(r.get('check')) is str and rx.search(r['check'])]
    return bool(hit) and any(r.get('status') == 'pass' for r in hit) and not any(r.get('status') == 'FAIL' for r in hit), len(hit)


# ------------------------------------------------------------------ boundary and codec provenance

def boundary():
    check('boundary.positionalOnly', record.__code__.co_posonlyargcount == 2 and check.__code__.co_posonlyargcount == 2 and gap.__code__.co_posonlyargcount == 1)
    probe = {'x': [1]}
    record('boundary.probe', 'recorded', name='n', status='s', obj=probe)
    probe['x'].append(2)
    e = results[-1]
    check('boundary.renamedAndCopied', e['diagName'] == 'n' and e['proposalStatus'] == 's' and e['obj'] == {'x': [1]})
    bad, calls = {}, []
    for f in (HERE / 'run_checks_031.py', HERE / 'guard_ref_031.py'):
        tree = ast.parse(f.read_text(encoding='utf-8'))
        mods = {a.name.split('.')[0] for n in ast.walk(tree) if isinstance(n, ast.Import) for a in n.names}
        mods |= {(n.module or '').split('.')[0] for n in ast.walk(tree) if isinstance(n, ast.ImportFrom)}
        allowed_sub = f.name == 'guard_ref_031.py'
        hit = sorted(mods & ({'socket', 'urllib', 'http', 'requests', 'ssl', 'asyncio', 'threading'} | (set() if allowed_sub else {'subprocess'})))
        if hit:
            bad[f.name] = hit
        calls += [n.lineno for n in ast.walk(tree) if isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute) and n.func.attr == 'main']
    check('boundary.noNetworkModules', not bad, imports=bad)
    check('boundary.noOldMainCalled', not calls, lines=calls)
    tree = ast.parse((HERE / 'guard_ref_031.py').read_text(encoding='utf-8'))
    sub = []
    for n in ast.walk(tree):
        if isinstance(n, ast.Attribute) and isinstance(n.value, ast.Name) and n.value.id == 'subprocess':
            sub.append(n.attr)
    runs = [n for n in ast.walk(tree) if isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute) and n.func.attr == 'run'
            and isinstance(n.func.value, ast.Name) and n.func.value.id == 'subprocess']
    shape = all(isinstance(c.args[0], ast.List) and any(k.arg == 'shell' and isinstance(k.value, ast.Constant) and k.value.value is False for k in c.keywords)
                and any(k.arg == 'timeout' for k in c.keywords) and any(k.arg == 'input' for k in c.keywords) for c in runs)
    check('boundary.subprocessShape', len(runs) == 2 and shape and set(sub) <= {'run', 'TimeoutExpired'}, attributes=sorted(set(sub)), runCalls=len(runs),
          rule='fixed argv list, shell=False, timeout, stdin input; nothing else from subprocess')
    js = CODEC.read_text(encoding='utf-8')
    imports = re.findall(r"^import .* from '([^']+)';", js, flags=re.M)
    forbidden = [w for w in ('eval(', 'Function(', 'require(', 'child_process', 'node:fs', "'fs'", 'node:net', 'node:http', 'node:https', 'fetch(',
                             'process.env', 'process.argv', 'import(', 'WebAssembly', 'node:worker') if w in js]
    check('codec.staticShape', sorted(imports) == ['node:buffer', 'node:process'] and not forbidden, imports=imports, forbidden=forbidden, sha256=sha(CODEC))


def codec_setup():
    global CODECOBJ
    oracle_node = {rj(o['file'])['node'] for o in own('vectors/c35-native-codec-cases.json')['oracles']}
    if not NODE:
        check('codec.nodeAvailable', False, node=None, note='Node.js not found (--node / POCOL_NODE / PATH); dependent checks fail, no fallback')
        return
    c = G31.NodeCodec(NODE, CODEC)
    try:
        ver = c.version()
    except Exception as e:                                                   # recorded, never hidden
        ver = None
        record('codec.versionError', 'recorded', error='%s: %s' % (type(e).__name__, e))
    check('codec.nodeAvailable', ver is not None, node=NODE, version=ver)
    check('codec.versionMatchesOracle', ver is not None and {ver} == oracle_node, version=ver, oracle=sorted(oracle_node))
    try:
        rep = c.run(b'{"jsonrpc":"2.0","id":2,"error":{"code":-32021,"message":"busy","data":{"reason":"busy","retryAfterMs":250}}}')
        ok = rep['dataBytes'] == len('{"reason":"busy","retryAfterMs":250}') and rep['changed'] is False and rep['node'] == ver
    except G31.CodecError as e:
        rep, ok = {'error': str(e)}, False
    check('codec.smoke', ok, report=rep)
    try:
        c.run(b'{"a":1}' + b' ' * 98304)
        refused = False
    except G31.CodecError:
        refused = True
    check('codec.refusesOversizedInput', refused)
    CODECOBJ = c


# ------------------------------------------------------------------ 0.30 binding and C36

def bind030():
    r30 = rj(INV['results']['R30'])
    s = r30['summary']
    c36 = own('vectors/c36-binding-repairs.json')
    w = c36['results030']
    fails = sorted(r['check'] for r in r30['results'] if r['status'] == 'FAIL')
    check('bind030.summaryAndFailures', [s['checks'], s['passed'], s['recorded'], s['failed']] == [w['checks'], w['passed'], w['recorded'], w['failed']]
          and fails == sorted(f['old'] for f in c36['failures']), summary=s, failures=fails)
    cp, mism, compared = ROOT / 'coordination/review-001/m1-draft-0.30', [], 0
    for f in sorted(cp.rglob('*')) if cp.exists() else []:
        rel = f.relative_to(cp)
        if not f.is_file() or rel.parts[0] == 'results' or '__pycache__' in rel.parts or not (ROOT / 'm1-draft-0.30' / rel).exists():
            continue
        compared += 1
        if f.read_bytes() != (ROOT / 'm1-draft-0.30' / rel).read_bytes():
            mism.append(rel.as_posix())
    check('bind030.reviewCopyByteIdentical', compared > 0 and not mism, compared=compared, mismatches=mism)
    for p in INV['boundFrom030']['patterns']:
        ok, n = ev_ok('R30', p)
        check('bind030.pattern.' + p, ok, entries=n)
    have30 = {r['check']: r for r in RES['R30']}
    for rel in INV['boundFrom030']['freezeOutputs']:
        name = Path(rel).name
        e = have30.get('asset.%s.writtenAndReadBack' % name, {})
        check('bind030.output.' + name, e.get('status') == 'pass' and e.get('sha256') == sha(ROOT / rel), recorded=e.get('sha256'), current=sha(ROOT / rel))
    for ev in INV['boundFrom030']['rootIndependent']:
        d = rj(ev['file'])
        check('bind030.root.' + Path(ev['file']).stem, all(d.get(k) == v for k, v in ev.items() if k != 'file'), sha256=sha(ROOT / ev['file']))
    for d in ('U01', 'U02', 'U10', 'U14', 'CR-M1-01', 'RF-E6-1', 'V1-SHA-COST', 'P-C32-2'):
        if ev_ok('R30', '^bind029\\.decisionApplied\\.%s$' % re.escape(d))[0]:
            APPLIED.add(d)
    if ev_ok('R30', '^decision\\.U08\\.applied$')[0]:
        APPLIED.add('U08')
    check('bind030.delegatedNineApplied', len(APPLIED) == 9, applied=sorted(APPLIED))
    led = {i['id']: i.get('status') for i in rj('coordination/issue-ledger.json').get('issues', [])}
    check('bind030.ledgerNotReopened', all(led.get(k, '').startswith('closed') for k in c36['ledger']['closedNotReopened'])
          and all(led.get(k, '').startswith('open') for k in c36['ledger']['openRepairedHere']), statuses={k: led.get(k) for k in ('C33', 'C34', 'C35', 'C36')})


def c36():
    rd = rj(INV['base']['reviewDecisions0_30'])
    check('C36.reviewDecisions030.acceptedUnaffected', rd.get('revision') == '0.30' and rd.get('acceptedUnaffectedRows') == ['C1', 'C2', 'R1', 'E6']
          and rd.get('personalOwnerAnswersRecorded') == 0 and 'not personalownerapproval' in rd.get('authority', ''),
          accepted=rd.get('acceptedUnaffectedRows'), conditions=rd.get('conditions'))
    doc = own('vectors/c36-binding-repairs.json')
    cases = {c['id']: c for c in rj('m1-draft-0.30/vectors/c33-guard-cases.json')['cases']}
    bad = []
    for wtn in doc['scopeProbeWitnesses']:
        d = rj(wtn['file'])
        lit = d['results'][wtn['index']]['literal'] if 'index' in wtn else next(c['literal'] for c in d['cases'] if c['id'] == wtn['caseId'])
        fixture = cases[wtn['c33Case']]['replies'][1].get('text')
        if not (wtn['contains'] in lit and lit == fixture):
            bad.append({'witness': wtn, 'literal': lit[:120], 'fixture': (fixture or '')[:120]})
    check('C36.scopeProbes.parsedLiterals', not bad, mismatches=bad, note='parsed JSON fields compared; the raw container text is not searched')
    rd_src = INV['base']['reviewDecisions0_30']
    record('C36.reviewDecisions030.sha256', 'recorded', file=rd_src, sha256=sha(ROOT / rd_src))


# ------------------------------------------------------------------ guarded check helper

def tokens():
    c32 = {t['case']: t for t in R30m.T27['c32']}
    shas = {t['case']: t for t in R30m.T27['sha']}
    base, s1 = c32['C32-ok-busy250'], shas['SHA-S1']
    tok = {'BN20': base['replies'][0], 'HDR20': base['replies'][-1], 'S1BN': s1['replies'][0], 'S1HDR': s1['replies'][1]}
    tok['@RS1'] = json.dumps(json.loads(tok['S1HDR'])['result'])
    return tok, s1


def ghnc(cfg, script, clock, compute=0, t0=0):
    return G31.guarded_header_net_check(V, O, B04, G30, CODECOBJ, cfg, script, clock, compute, t0)


def kind_of(v):
    return 'ok' if v.get('ok') is True else v.get('rule')


def v1cfg():
    net = rj('m1-draft-0.26/vectors/v1-network.json')
    gp = net['network']['genesisPre']
    segs = copy.deepcopy(rj('m1-draft-0.21/vectors/v3-gsv1.json')['flatSegments'])
    segs[gp['segmentIndex']] = gp['with']
    pre = b''.join(bytes.fromhex(s) if isinstance(s, str) else bytes.fromhex(s['repeat']) * s['count'] for s in segs)
    spec = load_module(ROOT / 'm1-draft-0.21/tools/netprofile_ref.py', 'netprofile_021_031').decode_genesis(pre)
    return {'chainId': spec['chainId'], 'genesisHash': KEC.keccak256(pre), 'forkSchedule': [tuple(x) for x in net['network']['forkSchedule']], 'cp': dict(spec['CP'])}


BASE_DATA = '{"reason":"busy","retryAfterMs":250,"pad":"%s"}'
BUSY_DATA = '{"jsonrpc":"2.0","id":2,"error":{"code":-32021,"message":"busy","data":%s}}'


def busy_with(raw_data):
    return (BUSY_DATA % raw_data).encode('utf-8')


def integrated(cfg, tok, reply, drop):
    r = ghnc(cfg, [(0, tok['S1BN']), (0, reply), (0, tok['S1HDR'])], 1700000015)
    if drop:
        ok = kind_of(r['verdict']) == 'viewIncomplete' and r['guard'] is None and r['sleptMs'] == 0 and len(r['requests']) == 2 \
            and r['counters'].decodes == 0 and r['verdict'].get('frame') is False and r['verdict'].get('cancel') == 4901
    else:
        ok = kind_of(r['verdict']) == 'ok' and r['sleptMs'] == 250 and len(r['requests']) == 3 and r['counters'].decodes == 1 and r['verdict'].get('frame') is True
    return ok, r


# ------------------------------------------------------------------ C35

def c35(cfg):
    doc = own('vectors/c35-native-codec-cases.json')
    provenance('C35.source.bound', doc['sources']['bound'])
    provenance('C35.source.review', doc['sources']['review'])
    dec = rj(doc['sources']['decision']['file'])[doc['sources']['decision']['json']]
    check('C35.source.decision', all(x in dec for x in doc['sources']['decision']['contains']), decision=dec)
    hist = doc['loaderHistory']
    a, b = rj(hist['initial']['file']), rj(hist['corrected']['file'])
    check('C35.oracle.utf8Readers', a['failed'] == hist['initial']['failed'] and b['failed'] == hist['corrected']['failed']
          and [r['id'] for r in b['results'] if not r['pass']] == ['loneSurrogateData'] and 'initialLoaderFailurePreserved' in b,
          files={o['file']: sha(ROOT / o['file']) for o in doc['oracles']})
    tok, _ = tokens()
    rows = []
    for o in doc['oracles']:
        d = rj(o['file'])
        check('C35.oracle.rows.' + Path(o['file']).stem, len(d[o['rowsField']]) == o['rows'] and d['node'] == o['node'], rows=len(d[o['rowsField']]))
        tag = 'oracle030' if 'dataField' in o else 'expanded031'
        for row in d[o['rowsField']]:
            raw = json.dumps(row[o['dataField']], ensure_ascii=True) if 'dataField' in o else row[o['rawField']]
            rows.append((tag, row['id'], raw, row['expectedJsonUtf8Bytes'], row['shouldDrop']))
    for tag, rid, raw, want, drop in rows:
        reply = busy_with(raw)
        try:
            rep = CODECOBJ.run(reply)
            direct = rep['dataBytes'] == want and rep['dataDropped'] == drop
        except Exception as e:                                               # a codec problem is a FAIL, never a pass
            rep, direct = {'error': '%s: %s' % (type(e).__name__, e)}, False
        try:
            prev = G30.compact_bytes(json.loads(raw, parse_int=B04._parse_int, parse_float=B04.FloatTok))
        except Exception as e:
            prev = 'error: %s' % type(e).__name__
        check('C35.direct.%s.%s' % (tag, rid), direct, nativeBytes=rep.get('dataBytes'), expected=want, shouldDrop=drop, previousModelBytes=prev)
        try:
            ok, r = integrated(cfg, tok, reply, drop)
            diag = {'outcome': kind_of(r['verdict']), 'sleptMs': r['sleptMs'], 'requests': len(r['requests']), 'guard': r['guard']}
        except Exception as e:
            ok, diag = False, {'exception': '%s: %s' % (type(e).__name__, e)}
        check('C35.integrated.%s.%s' % (tag, rid), ok, shouldDrop=drop, **diag)
    prev_ls = [r for r in results if r['check'] == 'C35.direct.oracle030.loneSurrogateData']
    check('C35.defectWitness.loneSurrogate', bool(prev_ls) and isinstance(prev_ls[0].get('previousModelBytes'), int)
          and prev_ls[0]['previousModelBytes'] <= 4096 < prev_ls[0].get('nativeBytes', 0),
          note='the 0.30 Python measurement would keep this data; the native codec drops it')
    for bnd in doc['boundaries']:
        p = bnd['pad']
        pad = '\\ud800' * p.get('lone', 0) + '\U0001F600' * p.get('emoji', 0) + 'x' * p.get('ascii', 0)
        reply = busy_with(BASE_DATA % pad)
        try:
            rep = CODECOBJ.run(reply)
            direct = rep['dataBytes'] == bnd['expectedDataBytes'] and rep['dataDropped'] == bnd['drop']
            ok, r = integrated(cfg, tok, reply, bnd['drop'])
        except Exception as e:
            rep, direct, ok = {'error': '%s: %s' % (type(e).__name__, e)}, False, False
        check('C35.boundary.' + bnd['id'], direct and ok, nativeBytes=rep.get('dataBytes'), expected=bnd['expectedDataBytes'], drop=bnd['drop'])
    for cl in doc['clip']:
        m = cl['message']
        msg = 'm' * m.get('ascii', 0) + '\U0001F600' * m.get('emoji', 0) + 'z' * m.get('tail', 0)
        reply = ('{"jsonrpc":"2.0","id":2,"error":{"code":-32021,"message":%s,"data":{"reason":"busy","retryAfterMs":250}}}'
                 % json.dumps(msg, ensure_ascii=False)).encode('utf-8')
        e = cl['expect']
        try:
            rep = CODECOBJ.run(reply)
            r = ghnc(cfg, [(0, tok['S1BN']), (0, reply), (0, tok['S1HDR'])], 1700000015)
            cells = {'units': rep['messageUnits'] == e['messageUnits'], 'clipped': rep['messageClipped'] == e['clipped'],
                     'outcome': kind_of(r['verdict']) == e['outcome'] and r['sleptMs'] == e['sleptMs']}
            if 'textContains' in e:
                cells['safeReserialization'] = e['textContains'] in (rep['textForChecker'] or '') and isinstance(json.loads(rep['textForChecker']), dict)
            if 'passedOriginal' in e:
                cells['passedOriginal'] = r['guardInfo'][1].get('passedOriginal') is e['passedOriginal']
        except Exception as ex:
            cells = {'exception': False}
            rep = {'error': '%s: %s' % (type(ex).__name__, ex)}
        bad = sorted(k for k, v in cells.items() if not v)
        check('C35.clip.' + cl['id'], not bad, failingCells=bad, report={k: rep.get(k) for k in ('messageUnits', 'messageClipped', 'changed', 'error')})
    ordering(cfg, doc, tok)


def ordering(cfg, doc, tok):
    small = b'{"jsonrpc":"2.0","id":2,"error":{"code":-32021,"message":"busy","data":{"reason":"busy","retryAfterMs":250}}}'
    for c in doc['ordering']['cases']:
        spec = c['reply']
        declared = None
        if 'oversizeBusy' in spec:
            n = spec['oversizeBusy']
            base = busy_with(BASE_DATA % '')
            raw = busy_with(BASE_DATA % ('x' * (n - len(base))))
            if len(raw) != n:
                raise ValueError('oversize builder')
        elif 'declaredBusy' in spec:
            raw, declared = small, spec['declaredBusy']
        else:
            raw, declared = R30m.build(spec, tok)
        item = (0, raw, declared) if declared is not None else (0, raw)
        before = CODECOBJ.calls
        r = ghnc(cfg, [(0, tok['S1BN']), item, (0, tok['S1HDR'])], 1700000015)
        got = [r['guard']['reason'], r['guard']['stage']] if r['guard'] else None
        check('C35.ordering.' + c['id'], got == c['guard'] and CODECOBJ.calls == before and r['sleptMs'] == 0 and len(r['requests']) == 2
              and r['verdict'].get('frame') is False, guard=got, codecCallsDuringCase=CODECOBJ.calls - before)
    for c in doc['ordering']['positive']:
        before = CODECOBJ.calls
        script = []
        for t in c['replies']:
            script.append((0, tok[t] if t in tok else R30m.build(t, tok)[0]))
        r = ghnc(cfg, script, 1700000015)
        originals = [x if isinstance(x, str) else x.decode('utf-8') for _, x in script]
        same = r['texts'] == originals[:len(r['texts'])]
        want_calls = sum(1 for t in c['replies'] if t not in tok)
        check('C35.ordering.' + c['id'], kind_of(r['verdict']) == 'ok' and same and CODECOBJ.calls - before == want_calls,
              codecCalls=CODECOBJ.calls - before, byteIdenticalTexts=same)


# ------------------------------------------------------------------ C33 rerun through the 0.31 guard

def c33(cfg):
    doc = rj('m1-draft-0.30/vectors/c33-guard-cases.json')
    for k, src in doc['sources'].items():
        if k == 'scopeProbes':
            continue                                                         # bound by parsed fields in C36.scopeProbes.parsedLiterals
        provenance('C33.source.' + k, src)
    tok, s1 = tokens()
    for c in doc['cases']:
        try:
            script = []
            for spec in c['replies']:
                raw, declared = R30m.build(spec, tok)
                script.append((0, raw, declared) if declared is not None else (0, raw))
            r = ghnc(cfg, script, c['clock'])
        except Exception as e:
            check('C33.case.' + c['id'], False, exception='%s: %s' % (type(e).__name__, e))
            continue
        e = c['expect']
        got = {'outcome': kind_of(r['verdict']), 'guard': [r['guard']['reason'], r['guard']['stage']] if r['guard'] else None,
               'requests': len(r['requests']), 'sleptMs': r['sleptMs'], 'decodes': r['counters'].decodes}
        cells = {k: got[k] == e[k] for k in got}
        cells['frame'] = r['verdict'].get('frame') is (e['outcome'] == 'ok')
        if 'ids' in e:
            cells['ids'] = r['ids'] == e['ids']
        if 'clippedMessageUnits' in e:
            cells['clipped'] = any(i.get('clippedMessageUnits') == e['clippedMessageUnits'] for i in r['guardInfo'])
        if 'dataDropped' in e:
            cells['dataDropped'] = any(i.get('dataDropped') for i in r['guardInfo']) == e['dataDropped']
        if 'sameAsSaved' in e:
            cells['sameAsSaved'] = norm(r['verdict']) == norm(s1['outcome']) and norm(r['requests']) == norm(s1['requests'])
        bad = sorted(k for k, v in cells.items() if not v)
        check('C33.case.' + c['id'], not bad, failingCells=bad, got=got)
    d29 = rj('m1-draft-0.29/vectors/deadline-cases-0.29.json')
    ttok = d29['hnc']['tokens']
    c32 = {t['case']: t for t in R30m.T27['c32']}
    bn20, hdr20 = c32['C32-ok-busy250']['replies'][0], c32['C32-ok-busy250']['replies'][-1]

    def text(t):
        if t in ('BN20', 'HDR20'):
            return bn20 if t == 'BN20' else hdr20
        if ':' in t:
            k, ms = t.split(':')
            return ttok[k + ':<ms>'].replace('<ms>', ms)
        return ttok[t]
    ov = doc['hncReplay']['overrides']
    for c in d29['hnc']['cases']:
        r = ghnc(cfg, list(zip(c['lat'], [text(x) for x in c['replies']])), d29['hnc']['clock'], c['computeMs'])
        want = dict(c['expect'])
        if c['id'] in ov:
            want['why'] = ov[c['id']]['why']
        got = {'outcome': kind_of(r['verdict']), 'atMs': r['atMs'], 'why': r['why'], 'sleptMs': r['sleptMs'], 'requests': len(r['requests'])}
        check('C33.hncReplay.' + c['id'], got == want, got=got)
    cnt = Counter()
    for label, tr, clock, t0 in R30m.replay_set():
        r = ghnc(cfg, [(0, x) for x in tr['replies']], clock, 0, t0)
        sk = kind_of(tr['outcome'])
        cells = {'kind': kind_of(r['verdict']) == sk, 'requests': norm(r['requests']) == norm(tr['requests'])}
        if r['guard'] is None:
            cells['verdict'] = norm(r['verdict']) == norm(tr['outcome'])
        else:
            cells['noValidReplyRejected'] = sk != 'ok'
        if label.startswith('sha.'):
            cd = r['counters'].as_dict()
            cells['shaCounters'] = all(cd[k] == tr['counters'][k] for k in ('sha256ByCategory', 'sha256Total', 'sha256UniquePreimages'))
        bad = sorted(k for k, v in cells.items() if not v)
        check('C33.replay.' + label, not bad, failingCells=bad, guard=r['guard'])
        cnt['guardRejected' if r['guard'] else 'guardPassed'] += 1
    record('C33.replaySummary', 'recorded', counts=dict(cnt))


def rp_shared():
    d29 = rj('m1-draft-0.29/vectors/deadline-cases-0.29.json')
    c = next(x for x in d29['rp']['cases'] if x['id'] == 'RP-COMB')
    eff = [{'outcome': o['outcome'], 'absoluteAtMs': o['absoluteAtMs']} for o in O.run_ops(c['ops'], 'shared')]
    rej = [{'outcome': o['outcome'], 'absoluteAtMs': o['absoluteAtMs']} for o in O.run_ops(c['ops'], 'separate')]
    check('U14.rp.RP-COMB.effectiveShared', eff == c['expect']['shared'] == [{'outcome': 'ok', 'absoluteAtMs': 6900}, {'outcome': 'unavailable', 'absoluteAtMs': 10000}], got=eff)
    check('U14.rp.RP-COMB.rejectedSeparateComparison', rej == c['expect']['separate'] and rej != eff, got=rej)


# ------------------------------------------------------------------ conventions, consolidation

def conventions():
    want = {d['id']: d for d in CONV['decisions']}
    ovr = INV['conventionEvidenceOverride']
    for cv in APPL30['conventions']:
        exact = cv['selection'] == want.get(cv['id'], {}).get('selection') and want[cv['id']]['status'] == 'delegated-technical-decision'
        oks = []
        for ev in ovr.get(cv['id'], cv['evidence']):
            if ev[0] == 'file':
                d = rj(ev[1])
                oks.append(all(d.get(k) == v for k, v in ev[2].items()))
            else:
                oks.append(ev_ok(ev[0], ev[1])[0])
        if check('convention.%s.decided' % cv['id'], exact and bool(oks) and all(oks), evidence=ovr.get(cv['id'], cv['evidence']), evidenceOk=oks):
            DECIDED.add(cv['id'])
    check('conventions.all17', len(DECIDED) == 17 and sorted(want) == sorted(c['id'] for c in APPL30['conventions']), decided=sorted(DECIDED))


def consolidate():
    have = {r['check']: r['status'] for r in results}
    matrix = {}
    for rid, m in M30['rows'].items():
        basis = m.get('basis', {})
        owners = list(m.get('ownerOpen', []))
        if basis.get('c4'):
            owners += [t.strip() for t in basis['c4'].split(':', 1)[1].split(',')]
        owner_open = [t for t in owners if t not in APPLIED]
        reviewer = list(m.get('reviewerOpen', [])) + list(basis.get('conventions', []))
        rev_open = [t for t in reviewer if t not in DECIDED and t not in ACCEPTED]
        ev29 = [r for r in RES['R29'] if re.match('^row\\.%s\\.(evidence|supplement)\\.' % re.escape(rid), r['check'])]
        ev_pass = bool(ev29) and all(r['status'] == 'pass' for r in ev29)
        missing = list(m.get('missing', []))
        changed = rid in INV['changedRows']
        cur_ok = True
        if changed:
            for p in INV['changedRows'][rid]['currentRunEvidence']:
                mine = [s for k, s in have.items() if re.search(p, k)]
                cur_ok = cur_ok and bool(mine) and all(s != 'FAIL' for s in mine)
            missing += [f for f in INV['addFiles'] if not path_of(f).exists()]
        hash_blocked = [g for g, s in m.get('hashGroups', {}).items() if s == 'blocked']
        c1 = 'blocked' if missing else ('pendingRootReview' if changed else 'satisfied')
        c2 = 'blocked' if not (ev_pass and cur_ok) else ('pendingRootReview' if changed else 'satisfied')
        c3 = 'pendingRootReview' if changed else 'satisfied'
        c4 = 'blocked' if owner_open else ('pendingReviewerDecision' if rev_open else 'satisfied')
        c5 = 'blocked' if (owner_open or missing or hash_blocked) else ('pendingRootReview' if changed else 'satisfied')
        crit = {'c1': c1, 'c2': c2, 'c3': c3, 'c4': c4, 'c5': c5}
        st = 'CompleteCandidate' if all(v == 'satisfied' for v in crit.values()) else \
            ('PendingRootReview' if all(v in ('satisfied', 'pendingRootReview') for v in crit.values()) else 'Partial')
        matrix[rid] = {'title': m['title'], 'source': m['source'], 'status': st, 'criteria': crit,
                       'basis': {'c3': ('pending root review of 0.31: ' + INV['changedRows'][rid]['why']) if changed else
                                 ('REVIEW-DECISIONS-0.30 acceptedUnaffectedRows' if rid in ('C1', 'C2', 'R1', 'E6') else 'earlier root reviews (bound in 0.29/0.30)'),
                                 'c4': ('delegated: ' + ', '.join(t for t in owners if t in APPLIED)) if owners else None,
                                 'conventions': [t for t in reviewer if t in DECIDED]},
                       'ownerOpen': owner_open, 'reviewerOpen': rev_open, 'missing': missing, 'hashGroups': m.get('hashGroups', {}),
                       'phaseA': m.get('phaseA', []), 'experiments': m.get('experiments', [])}
        record('row.%s.criteria' % rid, 'recorded', **{k: v for k, v in matrix[rid].items() if k not in ('title', 'source')})
    want = {r: s for s, rows in INV['proposedStatus'].items() for r in rows}
    got = {r: v['status'] for r, v in matrix.items()}
    check('consolidation.rowsMatchProposal', got == want and len(got) == 41, differing={r: [got.get(r), want.get(r)] for r in set(got) | set(want) if got.get(r) != want.get(r)})
    return matrix


def findings():
    out, rep = {}, set(INV['repairedTokens'])
    for fid, f in M30['findings'].items():
        dep = f['dependsOn']
        und = [t for t in dep if not (t in ACCEPTED or t in APPLIED or t in DECIDED)]
        disp = 'open' if und else ('closableAfterRootReviewOf031' if any(t in rep for t in dep) else 'closableAtSpecScope')
        out[fid] = {'disposition': disp, 'dependsOn': dep, 'undecided': und, 'phaseA': f.get('phaseA', [])}
        record('finding.' + fid, 'recorded', **out[fid])
    want = {f: d for d, fs in INV['proposedFindings'].items() for f in fs}
    got = {f: v['disposition'] for f, v in out.items()}
    check('consolidation.findingsMatchProposal', got == want and len(got) == 26, differing={f: [got.get(f), want.get(f)] for f in set(got) | set(want) if got.get(f) != want.get(f)})
    return out


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


def dashboard(matrix, failed):
    cnt = Counter(v['status'] for v in matrix.values())
    pend = [r for r, v in matrix.items() if v['status'] != 'CompleteCandidate']
    return {'schema': 'pocol-m1-dashboard-ar/0.31', 'lang': 'ar', 'pendingRows': pend, 'lines': [
        'M1، المسودة 0.31: إصلاح C35 (قياس حجم بيانات الخطأ بترميز JSON الأصلي في Node) وC36 (ربط الأدلة بملفات الجذر المقروءة آليًا).',
        'يقيس المحوِّل الأصلي حجم data كما يفعل المتصفح: البديل المنفرد يُكتب بستة بايتات، و1.0 يصبح 1، و1e400 يصبح null. ما زاد على 4096 بايتًا يُحذف، فيصبح رد busy غير صالح ويتوقف العرض دون انتظار.',
        'الفحوص الصارمة (الحجم وUTF-8 والعمق والمفاتيح المكررة ورقم الطلب) تسبق المحوِّل دائمًا.',
        'وقت التشغيل: مكتبة Python القياسية مع Node المثبت محليًا، وليس Python وحده. إذا غاب Node تفشل الفحوص ولا يوجد بديل صامت.',
        'نتيجة التشغيل: %d فحصًا، فشل منها %d.' % (len(results), len(failed)),
        'الصفوف: %d مرشح للاكتمال، و%d بانتظار مراجعة الجذر، و%d جزئي.' % (cnt['CompleteCandidate'], cnt['PendingRootReview'], cnt['Partial']),
        'بانتظار مراجعة الجذر: %s.' % ('، '.join(pend) if pend else 'لا شيء'),
        'الإخفاقات الستة في 0.30 محفوظة، وكل منها مربوط بفحص إصلاحه.',
        'لا يكتمل أي صف قبل مراجعة الجذر. لا كود إنتاجي ولا نشر ولا معاملات.']}


def coverage():
    have = {r['check']: r['status'] for r in results}
    doc = own('vectors/c35-native-codec-cases.json')
    need = ['boundary.subprocessShape', 'codec.staticShape', 'codec.nodeAvailable', 'codec.versionMatchesOracle', 'codec.smoke', 'codec.refusesOversizedInput',
            'bind030.summaryAndFailures', 'bind030.reviewCopyByteIdentical', 'bind030.delegatedNineApplied', 'bind030.ledgerNotReopened',
            'C36.reviewDecisions030.acceptedUnaffected', 'C36.scopeProbes.parsedLiterals', 'C35.source.decision', 'C35.oracle.utf8Readers',
            'C35.defectWitness.loneSurrogate', 'U14.rp.RP-COMB.effectiveShared', 'U14.rp.RP-COMB.rejectedSeparateComparison', 'conventions.all17',
            'consolidation.rowsMatchProposal', 'consolidation.findingsMatchProposal', 'boundary.noPrivatePath']
    need += ['bind030.pattern.' + p for p in INV['boundFrom030']['patterns']]
    need += ['C35.boundary.' + b['id'] for b in doc['boundaries']] + ['C35.clip.' + c['id'] for c in doc['clip']]
    need += ['C35.ordering.' + c['id'] for c in doc['ordering']['cases'] + doc['ordering']['positive']]
    need += [k for k in have if k.startswith(('C35.direct.', 'C35.integrated.', 'C33.case.', 'C33.hncReplay.', 'C33.replay.', 'convention.'))]
    need += ['C33.case.' + c['id'] for c in rj('m1-draft-0.30/vectors/c33-guard-cases.json')['cases']]
    need = list(dict.fromkeys(need))
    missing = [x for x in need if have.get(x) != 'pass']
    check('coverage031.required', not missing and len([k for k in have if k.startswith('C35.direct.')]) == 17, missing=missing, required=len(need))
    check('coverage031.noStepAborted', not [k for k in have if k.startswith('step.completed')])


def history_final():
    have = {r['check']: r['status'] for r in results}
    r29 = {r['check']: r['status'] for r in RES['R29']}
    r30 = {r['check']: r['status'] for r in RES['R30']}
    hf8_old = ['U08.noncanonical.N05', 'decision.U08.applied', 'consolidation.findingsMatchProposal', 'coverage029.required']
    check('history.HF-8', all(r29.get(k) == 'FAIL' for k in hf8_old) and all(r30.get(k) == 'pass' for k in hf8_old[:3])
          and have.get('coverage031.required') == 'pass', note='coverage029.required is now covered by coverage031.required')
    c36 = own('vectors/c36-binding-repairs.json')
    rebound = {f['old']: {x: have.get(x) for x in f['repair']} for f in c36['failures']}
    check('history.HF-12', all(r30.get(k) == 'FAIL' for k in rebound) and all(s == 'pass' for v in rebound.values() for s in v.values()), rebound=rebound)
    for h in INV['history'][1:]:
        d = rj(h['file'])
        check('history.' + h['id'], d.get('failed') == h['failed'] and all(have.get(x) == 'pass' for x in h['reboundTo']), preserved=h['file'], repairedBy=h['reboundTo'])


def leaks(texts):
    markers = ('PoCol_' + 'Dialogue', 'state' + '.json', 'source' + 'State', 'historical-x8-full' + '-source-private', 'transcript' + '-private')
    found = [str(f.relative_to(PKG)) for f in sorted(PKG.rglob('*')) if f.is_file() and 'results' not in f.parts
             and f.suffix in ('.json', '.md', '.py', '.mjs') and any(m in f.read_text(encoding='utf-8') for m in markers)]
    found += [n for n, t in texts.items() if any(m in t for m in markers)]
    check('boundary.noPrivatePath', not found, files=found)


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
    step('codec', codec_setup)
    step('bind030', bind030)
    step('c36', c36)
    cfg = step('v1cfg', v1cfg)
    if cfg is not None and CODECOBJ is not None:
        step('c35', c35, cfg)
        step('c33', c33, cfg)
    step('rp', rp_shared)
    step('conventions', conventions)
    matrix = step('consolidation', consolidate) or {}
    finds = step('findings', findings) or {}
    gap('gap.rootReviewOf031', text='C35 and C36 stay open until root executes this run (with Node) in a preserved copy and reviews it; no row is Complete')
    gap('gap.phaseA', text='X-C33, X-U14, E01-E04, E06, E07 outcomes are later implementation results, not M1-spec gate conditions')
    texts = {'acceptance-matrix-0.31.json': dumps({'schema': 'pocol-m1-acceptance-matrix/0.31', 'rows': matrix, 'findings': finds,
                                                   'counts': dict(Counter(v['status'] for v in matrix.values()))}),
             'bindings-0.31.json': dumps({'schema': 'pocol-m1-bindings/0.31', 'runtime': {'python': sys.version.split()[0], 'node': NODE,
                                          'codecSha256': sha(CODEC), 'nodeVersion': next((r.get('version') for r in results if r['check'] == 'codec.nodeAvailable'), None)},
                                          'files': {rel: sha(ROOT / rel) for rel in INPUTS_ROOT + PRESERVED},
                                          'freezeBoundFrom030': {rel: sha(ROOT / rel) for rel in INV['boundFrom030']['freezeOutputs']},
                                          'checks': {r['check']: r['status'] for r in results if r['check'].startswith(('bind030.', 'C36.', 'history.'))}})}
    step('leaks', leaks, texts)
    coverage()
    history_final()
    after = {rel: sha(ROOT / rel) for rel in PRESERVED}
    check('preserved.olderFilesUnchanged', before == after, changed=[k for k in before if before[k] != after[k]])
    failed = [r for r in results if r['status'] == 'FAIL']
    texts['status-0.31.json'] = dumps({'schema': 'pocol-m1-status/0.31', 'rows': {r: v['status'] for r, v in matrix.items()},
                                       'failed': [r['check'] for r in failed], 'missingRequired': next((r.get('missing') for r in results if r['check'] == 'coverage031.required'), None),
                                       'remainingRoot': ['execute in a preserved copy with Node v22.13.1', 'independently review C35 and C36', 'ledger final verification'],
                                       'complete': False})
    texts['dashboard-0.31-ar.json'] = dumps(dashboard(matrix, failed))
    for name in sorted(texts):
        step('write ' + name, write_asset, name, texts[name], 'asset.' + name)
    failed = [r for r in results if r['status'] == 'FAIL']
    out = {'package': 'M1 draft 0.31 (C35 native error-data codec; C36 evidence bindings; 41-row gate)',
           'executedAtUtc': datetime.now(timezone.utc).isoformat(timespec='seconds'),
           'environment': {'python': sys.version.split()[0], 'platform': platform.platform(), 'node': NODE,
                           'runtimeNote': 'Python standard library plus the installed Node.js reference codec; not pure Python'},
           'summary': {'checks': len(results), 'passed': sum(r['status'] == 'pass' for r in results), 'recorded': sum(r['status'] == 'recorded' for r in results),
                       'failed': len(failed), 'rows': dict(Counter(v['status'] for v in matrix.values())) if matrix else None,
                       'codecCalls': CODECOBJ.calls if CODECOBJ else 0, 'seconds': round(time.time() - t0, 1)},
           'results': results}
    OUT.mkdir(parents=True, exist_ok=True)
    p, k = OUT / 'run-results-0.31.json', 1
    while p.exists():
        p, k = OUT / ('run-results-0.31-rerun-%d.json' % k), k + 1
    p.write_text(json.dumps(out, indent=2, ensure_ascii=True, default=str) + '\n', encoding='utf-8')
    print(json.dumps(out['summary']), '->', p.name)
    return 1 if failed else 0


if __name__ == '__main__':
    sys.exit(main())
