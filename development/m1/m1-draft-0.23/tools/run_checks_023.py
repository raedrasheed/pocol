"""Reference-only runner for M1 draft 0.23: V4 X8 fixture pages, configuration table and offline verdict rules.
Python standard library only; no network. NOT executed by the author.

Usage, from D:\\PoCol-Development:
  coordination\\runtime\\python311\\python.exe m1-draft-0.23\\tools\\run_checks_023.py
Writes only m1-draft-0.23/results/run-results-0.23.json and exits 1 on any FAIL.

Checks only genuine offline properties: literal asset hashes (recorded), static page properties (only the
canary token host, only the channel's own scheme/port, every declared attempt present, C7's five paths),
configuration/schema consistency with the canonical clauses, sanitized historical provenance (message index,
round and hash only; the private record is read, never copied), and the evaluator on synthetic logs with
hand-derived verdicts. Chrome/CDP experiments are recorded as not executed.
"""

import ast
import copy
import hashlib
import json
import platform
import re
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

HERE = Path(__file__).resolve().parent
PKG, ROOT = HERE.parent, HERE.parent.parent
RES = PKG / 'results'
OUT = RES / 'run-results-0.23.json'
REL = 'm1-draft-0.23/'
CORE_STATUS = ('pass', 'recorded', 'FAIL')
RENAMED = {'name': 'diagName', 'check': 'diagCheck', 'status': 'proposalStatus', 'ok': 'diagOk',
           'label': 'diagLabel', 'condition': 'diagCondition'}
PRIVATE = 'coordination/review-001/historical-x8-full-source-private.json'
INPUTS = ['coordination/task-021.md', 'coordination/review-001/REVIEW-0.22.md', 'coordination/issue-ledger.json', 'reference/validation.md',
          'reference/browser.md', 'reference/implementation.md', 'reference/governance.md', 'reference/decisions.json', 'reference/FINAL_DESIGN.md',
          REL + 'vectors/x8-config.json', REL + 'vectors/x8-synthetic-runs.json', REL + 'tools/x8_eval.py', REL + 'tools/run_checks_023.py']
PRESERVED = ['m1-draft-0.22/results/run-results-0.22.json', 'coordination/review-001/m1-draft-0.22/results/run-results-0.22.json',
             'coordination/issue-ledger.json', PRIVATE]
PORTS_B = {('tcp', 18080), ('tcp', 18443), ('tcp', 13478), ('udp', 13478), ('udp', 19000)}       # validation.md:131
SCHEME_PROTO = {'http': 'tcp', 'ws': 'tcp', 'stun': 'udp', 'turn': 'tcp'}

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
import x8_eval as EV                  # noqa: E402


# ------------------------------------------------------------------ assets: hashes and static properties

def assets(cfg):
    manifest = []
    for ch in cfg['channels']:
        cid = ch['id']
        html, js = [PKG / f[len(''):] if not f.startswith(REL) else ROOT / f for f in ch['files']]
        ok_files = html.exists() and js.exists()
        check('asset.%s.present' % cid, ok_files, files=ch['files'])
        if not ok_files:
            continue
        for p in (html, js):
            manifest.append([str(p.relative_to(PKG)).replace('\\', '/'), sha(p)])
        h = html.read_text(encoding='utf-8')
        s = js.read_text(encoding='utf-8')
        check('asset.%s.htmlOnlyMainJs' % cid, re.findall(r'\s(?:src|href)\s*=\s*"([^"]*)"', h) == ['main.js']
              and not re.search(r'\son\w+\s*=', h, re.I) and not re.search(r'<(iframe|link|img|object|embed|base|meta\s+http-equiv)', h, re.I),
              refs=re.findall(r'\s(?:src|href)\s*=\s*"([^"]*)"', h))
        check('asset.%s.tokens' % cid, s.count("var H = '__CANARY_HOST__', RUN = '__RUN__';") == 1)
        bad_sep = s.replace("scheme + '://' + H", '').replace("'http://www.w3.org/1999/xhtml'", '').count('://')
        check('asset.%s.noOtherUrlLiteral' % cid, bad_sep == 0 and not re.search(r'\b\d{1,3}(?:\.\d{1,3}){3}\b', s)
              and 'stun:' not in s.replace("ice('stun'", '') and 'turn:' not in s.replace("ice('turn'", ''), extraSchemeSeparators=bad_sep)
        targets = [(SCHEME_PROTO[m[0]], int(m[1])) for m in re.findall(r"url\('(http|ws)',\s*(\d+),", s)]
        targets += [(SCHEME_PROTO[m[0]], int(m[1])) for m in re.findall(r"ice\('(stun|turn)',\s*(\d+)", s)]
        want = tuple(ch['expected'])
        check('asset.%s.onlyOwnPort' % cid, bool(targets) and all(t == want for t in targets) and want in PORTS_B,
              targets=sorted(set('%s/%d' % t for t in targets)), expected='%s/%d' % want)
        rec_targets = re.findall(r"rec\('([^']+)',\s*'[^']+',\s*'([^']+)'", s)
        declared = [a for a, _ in rec_targets if a != 'X.apiProbe']
        check('asset.%s.attemptsDeclared' % cid, declared == ch['attempts'] and 'X.apiProbe' in [a for a, _ in rec_targets]
              and all(t == '%s/%d' % want for a, t in rec_targets if a != 'X.apiProbe'), declared=declared)
        if cid == 'C7':
            paths = ["document.createElement('iframe')", "d.innerHTML = '<iframe></iframe>'", "document.createElementNS('http://www.w3.org/1999/xhtml', 'iframe')",
                     "document.importNode(t.content, true)", "document.write('<iframe id=\"x8c7w\"></iframe>')"]
            check('asset.C7.fivePaths', all(p in s for p in paths) and s.count('rtcFrom(') == 6, missing=[p for p in paths if p not in s])
        sub = s.replace('__CANARY_HOST__', '192.0.2.10').replace('__RUN__', 'r1')
        check('asset.%s.substitutionOnlyTokens' % cid, '__CANARY_HOST__' not in sub and '__RUN__' not in sub
              and set(re.findall(r'\b\d{1,3}(?:\.\d{1,3}){3}\b', sub)) == {'192.0.2.10'})
    agg = hashlib.sha256(json.dumps(sorted(manifest), separators=(',', ':')).encode()).hexdigest()
    record('asset.manifest', 'recorded', files=sorted(manifest), aggregateSha256=agg, note='literal asset hashes for root to freeze')
    check('asset.count', len(manifest) == 16, count=len(manifest))


# ------------------------------------------------------------------ configuration and provenance

def config_checks(cfg):
    for c in cfg['canonical']:
        provenance('canonical.' + c['id'], c)
    check('config.ports', {(p['proto'], p['port']) for p in cfg['canary']['ports']} == PORTS_B)
    check('config.channels', [c['id'] for c in cfg['channels']] == ['C%d' % i for i in range(1, 9)])
    check('config.configurations', [k['id'] for k in cfg['configurations']] == ['K%d' % i for i in range(0, 6)])
    kinds, exp_reach = EV.cell_kinds(cfg)
    flags_ok = (all(kinds.get((k, c)) == 'gatingZero' for k in ('K0', 'K1', 'K2') for c in EV.CHANNELS)
                and all(kinds.get(('K3', c)) == 'informational' for c in EV.CHANNELS)
                and all(kinds.get(('K4', c)) == 'requiredReach' for c in EV.CHANNELS)
                and all(kinds.get(('K5', c)) == 'gatingZero' for c in ('C5', 'C6', 'C7'))
                and all(kinds.get(('K5', c)) == 'informational' for c in ('C1', 'C2', 'C3', 'C4', 'C8'))
                and len(kinds) == 48)
    check('config.flagsMatchCanonical', flags_ok, note='K0-K2 zero reach, K3 informational, K4 control (required reach), K5 zero for C5-C7')
    layers = {k['id']: k['layers'] for k in cfg['configurations']}
    check('config.layerSets', layers == {'K0': {'csp': 'on', 'dnr': 'on', 'rtc': 'on'}, 'K1': {'csp': 'on', 'dnr': 'off', 'rtc': 'on'},
                                         'K2': {'csp': 'relaxed', 'dnr': 'on', 'rtc': 'on'}, 'K3': {'csp': 'on', 'dnr': 'on', 'rtc': 'off'},
                                         'K4': {'csp': 'off', 'dnr': 'off', 'rtc': 'off'}, 'K5': {'csp': 'off', 'dnr': 'off', 'rtc': 'on'}})
    line366 = (ROOT / 'reference/browser.md').read_text(encoding='utf-8').splitlines()[365]
    check('config.cspOnIsBaselineText', cfg['layerSpecs']['cspOn']['text'] in line366)
    relaxed = cfg['layerSpecs']['cspRelaxed']['text']
    check('config.cspRelaxedOnlyAddsCanary', relaxed.startswith('sandbox allow-scripts allow-forms;') and "connect-src 'none'" not in relaxed
          and all(h == '__CANARY_HOST__' for h in re.findall(r'://([^:/\s;]+)', relaxed)))
    rules = cfg['layerSpecs']['dnrOn']['rules']
    check('config.dnrBaselineFields', [r['id'] for r in rules] == ['2*tabId', '2*tabId+1'] and [r['priority'] for r in rules] == [1, 2]
          and rules[1]['condition']['urlFilter'] == '|chrome-extension://<ID>/')
    exps = [e['id'] for e in cfg['experimentPlan']['experiments']]
    check('config.experimentIds', exps == ['X8-SELFTEST-<K>', 'X8-<K>-<C>', 'X8-SEC-K0', 'X8-F1', 'X8-FULL']
          and cfg['experimentPlan']['status'].startswith('notExecuted'))
    check('config.eventSchema', set(cfg['eventSchema']) == {'canaryEvent', 'secondaryEvent', 'dnsEvent', 'pageLog'}
          and 'evidenceSource' in cfg['eventSchema']['canaryEvent']['fields'] and 'srcIp' in cfg['eventSchema']['canaryEvent']['fields'])
    check('config.precisionBlocks', [p['id'] for p in cfg['precisionBlocks']] == ['PB%d' % i for i in range(1, 13)])


def historical_checks(cfg):
    p = ROOT / PRIVATE
    if not check('historical.privateRecordAvailable', p.exists()):
        return
    doc = json.loads(p.read_text(encoding='utf-8'))
    h44, h34 = cfg['historical']['H44'], cfg['historical']['H34']
    e34 = doc.get('earlierDetailedSource', {})
    check('historical.H44.indexRoundHash', doc.get('messageIndex') == h44['messageIndex'] and doc.get('round') == h44['round']
          and doc.get('messageSha256') == h44['messageSha256'])
    check('historical.H34.indexRoundHash', e34.get('messageIndex') == h34['messageIndex'] and e34.get('round') == h34['round']
          and e34.get('messageSha256') == h34['messageSha256'])
    b44, b34 = doc.get('body', ''), e34.get('body', '')
    t44 = ['TCP 18080 و18443 و13478، وUDP 13478 و19000', 'C1 fetch/XHR', 'C2 img/link/font/@import', 'C3 prefetch/preconnect', 'C4 WebSocket',
           'C5 RTCPeerConnection مع STUN', 'C6 TURN-TCP', 'C7 realm جديد (خمسة مسارات)', 'C8 sendBeacon/form', 'K0 (الكل) صفر وصول',
           'K1 (CSP+Rtc) صفر', 'K2 (DNR+Rtc) صفر', 'K3 (دون Rtc) معلوماتي', 'K4 (لا شيء) ضابط إلزامي', 'K5 (Rtc وحده) صفر لـC5–C7']
    t34 = ['TCP 18080 (HTTP)', 'TCP 18443 (WebSocket)', 'TCP 13478 (TURN-TCP)', 'UDP 13478 (STUN)', 'UDP 19000 (عام)',
           "createElement('iframe')", 'innerHTML', 'createElementNS', 'template مع importNode', 'document.write',
           'K0** (CSP + DNR + RtcLockdown)', 'K1** (CSP + RtcLockdown، وDNR معطل)', 'K2** (DNR + RtcLockdown، وCSP مرخّاة تسمح بالكناري)',
           'K3** (CSP + DNR، وRtcLockdown معطل)', 'K4** (كل الطبقات معطلة)', 'K5** (RtcLockdown وحده)']
    check('historical.H44.mappingTraceable', all(t in b44 for t in t44), missingFragments=[i for i, t in enumerate(t44) if t not in b44])
    check('historical.H34.mappingTraceable', all(t in b34 for t in t34), missingFragments=[i for i, t in enumerate(t34) if t not in b34])
    leaks = []
    for f in sorted(PKG.rglob('*')):
        if f.is_file() and f.suffix in ('.json', '.md', '.js', '.html', '.py') and 'results' not in f.parts:
            text = f.read_text(encoding='utf-8')
            markers = ('PoCol_' + 'Dialogue', 'state' + '.json', 'source' + 'State')       # built so this file does not match itself
            if any(x in text for x in markers) or (b44 and b44[:60] in text) or (b34 and b34[:60] in text):
                leaks.append(str(f.relative_to(PKG)))
    check('historical.noPrivatePathOrTranscriptInPackage', not leaks, files=leaks)


# ------------------------------------------------------------------ evaluator on synthetic logs

def build_base(cfg, base):
    exp = {c['id']: c['expected'] for c in cfg['channels']}
    run = {'selftests': {k: [{'proto': p['proto'], 'port': p['port']} for p in cfg['canary']['ports']] for k in EV.CONFIGS}, 'windows': []}
    reach = {(k, c) for k, spec in base['reach'].items() for c in EV.expand(spec)}
    for k in EV.CONFIGS:
        for c in EV.CHANNELS:
            ev = [{'proto': exp[c][0], 'port': exp[c][1]}] if (k, c) in reach else []
            run['windows'].append({'K': k, 'C': c, 'events': ev, 'secondary': [], 'dns': []})
    return run


def apply_mods(run, mods):
    r = copy.deepcopy(run)
    win = {(w['K'], w['C']): w for w in r['windows']}
    for m in mods:
        if m['op'] == 'addEvent':
            win[(m['K'], m['C'])]['events'].append({'proto': m['proto'], 'port': m['port']})
        elif m['op'] == 'clearEvents':
            win[(m['K'], m['C'])]['events'] = []
        elif m['op'] == 'dropSelftestPort':
            r['selftests'][m['K']] = [e for e in r['selftests'][m['K']] if (e['proto'], e['port']) != (m['proto'], m['port'])]
        elif m['op'] == 'addSecondary':
            win[(m['K'], m['C'])]['secondary'].append({'kind': 'tcpConnection', 'remote': '203.0.113.5:443'})
        elif m['op'] == 'addDns':
            win[(m['K'], m['C'])]['dns'].append({'qname': 'example.invalid'})
        elif m['op'] == 'dropWindow':
            r['windows'] = [w for w in r['windows'] if (w['K'], w['C']) != (m['K'], m['C'])]
        else:
            raise ValueError(m)
    return r


def evaluator_checks(cfg, syn):
    base = build_base(cfg, syn['base'])
    for run in syn['runs']:
        got = EV.evaluate(cfg, apply_mods(base, run['mods']))
        ex = run['expect']
        cells = [('verdict', ex['verdict'], got['verdict'])]
        if 'fail' in ex:
            cells.append(('fail', sorted(ex['fail']), sorted(got['fail'])))
        if 'untestable' in ex:
            cells.append(('untestable', sorted(ex['untestable']), sorted(got['untestable'])))
        if 'informationalUnexplained' in ex:
            cells.append(('unexplained', sorted(ex['informationalUnexplained']), sorted(k for k, v in got['informational'].items() if v == 'unexplained')))
        bad = [{'cell': c, 'expected': w, 'actual': a} for c, w, a in cells if w != a]
        check('eval.' + run['id'], not bad, mismatches=bad)


def no_network_checks():
    bad = {}
    for f in (HERE / 'x8_eval.py', HERE / 'run_checks_023.py'):
        tree = ast.parse(f.read_text(encoding='utf-8'))
        mods = {a.name.split('.')[0] for n in ast.walk(tree) if isinstance(n, ast.Import) for a in n.names}
        mods |= {(n.module or '').split('.')[0] for n in ast.walk(tree) if isinstance(n, ast.ImportFrom)}
        hit = sorted(mods & {'socket', 'urllib', 'http', 'requests', 'ssl', 'asyncio', 'subprocess'})
        if hit:
            bad[f.name] = hit
    check('boundary.noNetworkModules', not bad, imports=bad)


GAPS = [
    ('V4-notExecuted', 'Chrome/CDP experiments X8-SELFTEST, X8-<K>-<C> (48), X8-SEC-K0, X8-F1 are specified, not executed; no browser result is claimed.'),
    ('V4-X8FULL', 'X8-full packet capture is historical (H34), deferred beyond M1, required before any public release.'),
    ('V4-RtcLimits', 'RtcLockdown argument unproven (B); X8 covers counted channels only; WebRTC exposure limits are declared, not proved.'),
] + [('PB%d' % i, 'precision block, see vectors/x8-config.json') for i in range(1, 13)]


def coverage(cfg, syn):
    have = {r['check']: r['status'] for r in results}
    need = ['asset.%s.%s' % (c, p) for c in EV.CHANNELS for p in ('present', 'htmlOnlyMainJs', 'tokens', 'noOtherUrlLiteral', 'onlyOwnPort',
                                                                    'attemptsDeclared', 'substitutionOnlyTokens')]
    need += ['asset.C7.fivePaths', 'asset.count'] + ['eval.' + r['id'] for r in syn['runs']]
    need += ['canonical.%s.provenance' % c['id'] for c in cfg['canonical']]
    need += ['config.ports', 'config.channels', 'config.configurations', 'config.flagsMatchCanonical', 'config.layerSets', 'config.cspOnIsBaselineText',
             'config.dnrBaselineFields', 'config.experimentIds', 'config.eventSchema', 'config.precisionBlocks',
             'historical.H44.indexRoundHash', 'historical.H34.indexRoundHash', 'historical.H44.mappingTraceable', 'historical.H34.mappingTraceable',
             'historical.noPrivatePathOrTranscriptInPackage', 'boundary.noNetworkModules']
    missing = [x for x in need if have.get(x) != 'pass']
    check('coverage023.required', not missing, missing=missing, required=len(need))
    check('coverage023.noStepAborted', not [k for k in have if 'step.completed' in k])


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
    cfg = load_json(REL + 'vectors/x8-config.json')
    syn = load_json(REL + 'vectors/x8-synthetic-runs.json')
    for name, fn in (('assets', lambda: assets(cfg)), ('config', lambda: config_checks(cfg)), ('historical', lambda: historical_checks(cfg)),
                     ('evaluator', lambda: evaluator_checks(cfg, syn)), ('network', no_network_checks)):
        try:
            fn()
        except Exception as ex:                                              # record, never hide
            check('step.completed ' + name, False, exception='%s: %s' % (type(ex).__name__, ex))
    for gid, text in GAPS:
        gap('gap.' + gid, text=text)
    coverage(cfg, syn)
    after = {rel: sha(ROOT / rel) for rel in PRESERVED}
    check('preserved.olderFilesUnchanged', before == after, before=before, after=after)
    failed = [r for r in results if r['status'] == 'FAIL']
    out = {'package': 'M1 draft 0.23 (V4 X8: eight literal fixture pages, configuration table, experiment plan, offline verdict rules)',
           'executedAtUtc': datetime.now(timezone.utc).isoformat(timespec='seconds'),
           'environment': {'python': sys.version.split()[0], 'executable': sys.executable, 'platform': platform.platform(),
                           'thirdPartyPackages': 'none'},
           'statusSchema': {'coreStatus': list(CORE_STATUS), 'diagnosticRenames': RENAMED, 'partialGap': 'recorded entries with partialGap=true'},
           'evidenceKind': 'static fixture analysis and an offline evaluator on synthetic logs; no browser, no network',
           'notExecuted': ['Chrome/CDP X8 matrix', 'CanarySink', 'secondary monitor', 'X8-F1', 'X8-FULL'],
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
