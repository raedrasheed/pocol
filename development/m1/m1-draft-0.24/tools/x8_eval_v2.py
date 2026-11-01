"""Repaired offline X8 verdict evaluator for M1 draft 0.24 (C31). Python standard library only; no network.
REFERENCE / SPECIFICATION FIXTURE TOOLING ONLY. NOT executed by the author.

C31 (coordination/review-001/REVIEW-0.23.md): the 0.23 evaluator indexed windows by (K, C) in a dict, so a later
empty duplicate window erased a recorded K0/C1 reach and the run passed.

Contract (same signature and output keys as 0.23, plus 'integrity' and 'gatingReaches'):
  1. Integrity is validated over the WHOLE input before any verdict. Any finding makes the verdict INVALID:
       run shape (exactly 'selftests' and 'windows'), selftest map (known K, list of events), window list, window
       shape (exactly K, C, events, secondary, dns: the secondary and DNS collections are required evidence),
       known K and C, event shape (proto 'tcp'|'udp', port an int that is not bool, on a canary port),
       secondary/dns entries are objects, and AT MOST ONE window per (K, C) (duplicates are ambiguous).
  2. Every window whose K and C are known is inspected. Evidence is aggregated per (K, C) over ALL its windows, so no
     observation is ever dropped: any element of an events list in a gating-zero cell is an observed reach
     (malformed elements included), and is reported in 'fail' and 'gatingReaches' even when the verdict is INVALID.
  3. Verdict precedence: INVALID (integrity findings or a self-test missing a port) > FAIL > PENDING > PASS.
Diagnostics are produced in input order, so equal inputs give equal outputs.
"""

CHANNELS = ['C%d' % i for i in range(1, 9)]
CONFIGS = ['K%d' % i for i in range(0, 6)]
WINDOW_KEYS = {'K', 'C', 'events', 'secondary', 'dns'}


def expand(spec):
    out = []
    for part in spec.split(','):
        if '-' in part:
            a, b = part.split('-')
            out += ['C%d' % i for i in range(int(a[1:]), int(b[1:]) + 1)]
        else:
            out.append(part)
    return out


def cell_kinds(config):
    r = config['cellRules']
    kinds = {}
    for kind in ('informational', 'gatingZero', 'requiredReach'):
        for k, spec in r[kind].items():
            for c in expand(spec):
                kinds[(k, c)] = kind
    exp_reach = {(k, c) for k, spec in r['informationalExpectedReach'].items() for c in expand(spec)}
    return kinds, exp_reach


def _event_issue(e, ports):
    if type(e) is not dict:
        return 'eventNotObject'
    if e.get('proto') not in ('tcp', 'udp'):
        return 'eventProto'
    if type(e.get('port')) is not int:
        return 'eventPortType'
    if (e['proto'], e['port']) not in ports:
        return 'eventPortUnknown'
    return None


def evaluate(config, run):
    ports = {(p['proto'], p['port']) for p in config['canary']['ports']}
    expected = {c['id']: tuple(c['expected']) for c in config['channels']}
    kinds, exp_reach = cell_kinds(config)
    out = {'verdict': None, 'invalid': [], 'fail': [], 'untestable': [], 'missing': [], 'informational': {}, 'secondarySignals': [],
           'integrity': [], 'gatingReaches': []}
    integ = out['integrity']

    def finding(code, where, **kw):
        d = {'code': code, 'where': where}
        d.update(kw)
        integ.append(d)

    if type(run) is not dict:
        finding('runNotObject', 'run')
        out['verdict'] = 'INVALID'
        return out
    if set(run) != {'selftests', 'windows'}:
        finding('runKeys', 'run', missing=sorted({'selftests', 'windows'} - set(run)), unknown=sorted(set(run) - {'selftests', 'windows'}))
    # ---- self-tests (canary sensitivity)
    selftests = run.get('selftests')
    seen_by_k = {k: set() for k in CONFIGS}
    if type(selftests) is not dict:
        finding('selftestsNotObject', 'selftests')
    else:
        for k, evs in selftests.items():
            if k not in CONFIGS:
                finding('selftestUnknownK', 'selftests', K=k)
                continue
            if type(evs) is not list:
                finding('selftestNotList', 'selftests.' + k)
                continue
            for i, e in enumerate(evs):
                issue = _event_issue(e, ports)
                if issue:
                    finding(issue, 'selftests.%s[%d]' % (k, i))
                else:
                    seen_by_k[k].add((e['proto'], e['port']))
    for k in CONFIGS:
        if not ports <= seen_by_k[k]:
            out['invalid'].append({'K': k, 'missingPorts': sorted('%s/%d' % p for p in ports - seen_by_k[k])})
    # ---- windows: inspect every one before any classification
    windows = run.get('windows')
    cells = {}                                                     # (K, C) -> aggregated evidence
    if type(windows) is not list:
        finding('windowsNotList', 'windows')
        windows = []
    for i, w in enumerate(windows):
        where = 'windows[%d]' % i
        if type(w) is not dict:
            finding('windowNotObject', where)
            continue
        if set(w) != WINDOW_KEYS:
            finding('windowKeys', where, missing=sorted(WINDOW_KEYS - set(w)), unknown=sorted(set(w) - WINDOW_KEYS))
        k, c = w.get('K'), w.get('C')
        known = True
        if k not in CONFIGS:
            finding('unknownK', where, K=k)
            known = False
        if c not in CHANNELS:
            finding('unknownC', where, C=c)
            known = False
        evs = w.get('events', [])
        if type(evs) is not list:
            finding('eventsNotList', where)
            evs = []
        for j, e in enumerate(evs):
            issue = _event_issue(e, ports)
            if issue:
                finding(issue, '%s.events[%d]' % (where, j))
        sig = []
        for coll in ('secondary', 'dns'):
            items = w.get(coll, [])
            if type(items) is not list:
                finding(coll + 'NotList', where)
                items = []
            for j, x in enumerate(items):
                if type(x) is not dict:
                    finding(coll + 'EntryNotObject', '%s.%s[%d]' % (where, coll, j))
            sig += items
        if not known:
            continue
        cell = cells.setdefault((k, c), {'windows': [], 'events': [], 'signals': []})
        cell['windows'].append(i)
        cell['events'] += evs
        cell['signals'] += sig
    for (k, c), cell in cells.items():
        if len(cell['windows']) > 1:
            finding('duplicateWindow', 'windows', K=k, C=c, indices=cell['windows'])
    # ---- classification over aggregated evidence
    for k in CONFIGS:
        for c in CHANNELS:
            cell = cells.get((k, c))
            if cell is None:
                out['missing'].append([k, c])
                continue
            events = cell['events']
            kind = kinds.get((k, c))
            if kind == 'gatingZero' and events:
                out['fail'].append([k, c])
                out['gatingReaches'].append({'K': k, 'C': c, 'windows': cell['windows'], 'observations': len(events)})
            elif kind == 'requiredReach':
                if not any(type(e) is dict and type(e.get('port')) is int and (e.get('proto'), e['port']) == expected[c] for e in events):
                    out['untestable'].append([k, c])
            elif kind == 'informational':
                if events:
                    status = 'reach'
                elif (k, c) in exp_reach:
                    status = 'unexplained'
                else:
                    status = 'noReach'
                out['informational']['%s/%s' % (k, c)] = status
            if k == 'K0' and cell['signals']:
                out['secondarySignals'].append([k, c])
    if integ or out['invalid']:
        out['verdict'] = 'INVALID'
    elif out['fail'] or out['secondarySignals']:
        out['verdict'] = 'FAIL'
    elif out['untestable'] or out['missing']:
        out['verdict'] = 'PENDING'
    else:
        out['verdict'] = 'PASS'
    return out
