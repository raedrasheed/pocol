"""Abstract model of Website-state snapshot reads (M1-SPEC-0.3-AMENDMENTS R3-04, C06).

Specification tooling only. Proofs are ABSTRACT: a proof built from state X
"verifies" against root R exactly when it is unforged, covers the requested
keys, and X's root is R. This models the binding property that proof-bound
reads rely on. It is not a Merkle-Patricia verifier and is no evidence that
pocold serves eth_getProof (experiment E03).

Two mechanisms:
- 'P20': proof-bound reads against one anchored stateRoot, at most 3 restarts.
- 'detectorB': alternative (B), unverified reads pinned by number with a
  before/after block-hash comparison. Included to demonstrate, executably,
  that it is a detector only (A -> B -> A evades it).
"""

PUBLISHED, DRAFT, REVOKED = 1, 0, 2
MAX_ATTEMPTS = 4                       # first attempt + at most 3 restarts (P20 step 4)


class ScriptError(Exception):
    """The fixture script does not match the calls the algorithm makes."""


class Node:
    def __init__(self, states, script):
        self.states, self.script, self.pos, self.log = states, script, 0, []

    def take(self, call):
        if self.pos >= len(self.script):
            raise ScriptError('script exhausted at call %r' % call)
        step = self.script[self.pos]
        self.pos += 1
        if step['call'] != call:
            raise ScriptError('expected %r, script has %r at %d' % (call, step['call'], self.pos - 1))
        self.log.append(call)
        return step

    def header(self, step):
        s = self.states[step['from']]
        return {'number': s['number'], 'hash': s['blockHash'], 'root': s['root'], 'label': step['from']}


def _slot2(st):
    w = st['website']
    return {'versionCount': w['versionCount'], 'currentVersion': w['currentVersion']}


def _version(st, n):
    v = st['website']['versions'].get(str(n))
    return dict(v) if v else {'status': 0, 'manifestHash': '0x' + '00' * 32, 'absent': True}


def _proof(node, step, root, keys):
    """Return proof values if the abstract proof verifies against `root`, else None."""
    if 'error' in step:
        return None
    st = node.states[step['from']]
    if step.get('forged') or st['root'] != root:
        return None
    served = step.get('keys', keys)
    if served != keys:
        return None
    if keys == 'slot2':
        return _slot2(st)
    return _version(st, int(keys.split(':')[1]))


def load_p20(states, script, request):
    """request: {'kind': 'default'} or {'kind': 'explicit', 'version': n}."""
    node = Node(states, script)
    restarts = -1
    for _ in range(MAX_ATTEMPTS):
        restarts += 1
        hdr = node.header(node.take('anchor'))
        s2 = _proof(node, node.take('proof1'), hdr['root'], 'slot2')
        if s2 is None:
            continue
        n = s2['currentVersion'] if request['kind'] == 'default' else request['version']
        if request['kind'] == 'default' and n == 0:
            return _done(node, 'noSite', restarts, hdr)
        if n > s2['versionCount']:
            return _done(node, 'versionNotFound', restarts, hdr)
        v = _proof(node, node.take('proof2'), hdr['root'], 'version:%d' % n)
        if v is None:
            continue
        if request['kind'] == 'default':
            if v['status'] != PUBLISHED:
                return _done(node, 'stateInvariant', restarts, hdr)
            return _done(node, 'render', restarts, hdr, n, v, banner=None)
        if v['status'] == DRAFT:
            return _done(node, 'refuseDraft', restarts, hdr)
        if v['status'] == REVOKED:
            return _done(node, 'revokedInterstitial', restarts, hdr, n, v)
        banner = None if n == s2['currentVersion'] else 'notCurrent'
        return _done(node, 'render', restarts, hdr, n, v, banner=banner)
    return _done(node, 'inconsistent', restarts, None)


def _done(node, result, restarts, hdr, n=None, v=None, banner=None):
    if node.pos != len(node.script):
        raise ScriptError('%d unused script steps' % (len(node.script) - node.pos))
    out = {'result': result, 'restarts': restarts, 'calls': list(node.log),
           'frames': 1 if result == 'render' else 0, 'mixed': False}
    if hdr:
        out['anchorState'] = hdr['label']
    if v is not None:
        out['version'] = n
        out['manifestHash'] = v['manifestHash']
    if banner:
        out['banner'] = banner
    return out


def load_detector_b(states, script, request):
    """Alternative (B): unverified reads, before/after block-hash comparison."""
    node = Node(states, script)
    restarts = -1
    for _ in range(MAX_ATTEMPTS):
        restarts += 1
        before = node.take('hashAt')
        r1 = node.take('read1')
        s2 = _slot2(states[r1['from']])
        n = s2['currentVersion'] if request['kind'] == 'default' else request['version']
        r2 = node.take('read2')
        v = _version(states[r2['from']], n)
        after = node.take('hashAt')
        if states[before['from']]['blockHash'] != states[after['from']]['blockHash']:
            continue
        if node.pos != len(node.script):
            raise ScriptError('unused script steps')
        labels = {before['from'], r1['from'], r2['from'], after['from']}
        return {'result': 'render' if v['status'] == PUBLISHED else 'stateInvariant',
                'restarts': restarts, 'calls': list(node.log),
                'frames': 1 if v['status'] == PUBLISHED else 0,
                'mixed': len(labels) > 1, 'version': n, 'manifestHash': v['manifestHash'],
                'servedFrom': {'slot2': r1['from'], 'version': r2['from']}}
    return {'result': 'inconsistent', 'restarts': restarts, 'calls': list(node.log),
            'frames': 0, 'mixed': False}


def never_current(states, n, manifest_hash):
    """True if no state has currentVersion n with that manifestHash (used for detectorB)."""
    for st in states.values():
        w = st['website']
        v = w['versions'].get(str(n))
        if w['currentVersion'] == n and v and v['manifestHash'] == manifest_hash:
            return False
    return True
