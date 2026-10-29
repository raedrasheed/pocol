"""Offline X8 verdict evaluator for M1 draft 0.23. Python standard library only; no network.
REFERENCE / SPECIFICATION FIXTURE TOOLING ONLY. NOT executed by the author.

evaluate(config, run) applies the X8 pass criterion (vectors/x8-config.json: cellRules, experimentPlan.verdict)
to recorded logs. It never observes traffic itself: Chrome/CDP runs are future experiments; here it is
exercised only on synthetic logs with hand-derived verdicts.

run = {'selftests': {K: [canaryEvent...]}, 'windows': [{'K', 'C', 'events': [canaryEvent...], 'secondary': [...], 'dns': [...]}]}
"""

CHANNELS = ['C%d' % i for i in range(1, 9)]
CONFIGS = ['K%d' % i for i in range(0, 6)]


def expand(spec):
    """'C1-C8' / 'C5-C7' / 'C1-C4,C8' -> list of channel ids."""
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


def evaluate(config, run):
    ports = {(p['proto'], p['port']) for p in config['canary']['ports']}
    expected = {c['id']: tuple(c['expected']) for c in config['channels']}
    kinds, exp_reach = cell_kinds(config)
    out = {'verdict': None, 'invalid': [], 'fail': [], 'untestable': [], 'missing': [], 'informational': {}, 'secondarySignals': []}
    for k in CONFIGS:
        seen = {(e['proto'], e['port']) for e in run.get('selftests', {}).get(k, [])}
        if not ports <= seen:
            out['invalid'].append({'K': k, 'missingPorts': sorted('%s/%d' % p for p in ports - seen)})
    windows = {(w['K'], w['C']): w for w in run.get('windows', [])}
    for k in CONFIGS:
        for c in CHANNELS:
            w = windows.get((k, c))
            if w is None:
                out['missing'].append([k, c])
                continue
            events = w.get('events', [])
            reach = bool(events)
            kind = kinds.get((k, c))
            if kind == 'gatingZero' and reach:
                out['fail'].append([k, c])
            elif kind == 'requiredReach':
                if not any((e['proto'], e['port']) == expected[c] for e in events):
                    out['untestable'].append([k, c])
            elif kind == 'informational':
                if reach:
                    status = 'reach'
                elif (k, c) in exp_reach:
                    status = 'unexplained'
                else:
                    status = 'noReach'
                out['informational']['%s/%s' % (k, c)] = status
            if k == 'K0' and (w.get('secondary') or w.get('dns')):
                out['secondarySignals'].append([k, c])
    if out['invalid']:
        out['verdict'] = 'INVALID'
    elif out['fail'] or out['secondarySignals']:
        out['verdict'] = 'FAIL'
    elif out['untestable'] or out['missing']:
        out['verdict'] = 'PENDING'
    else:
        out['verdict'] = 'PASS'
    return out
