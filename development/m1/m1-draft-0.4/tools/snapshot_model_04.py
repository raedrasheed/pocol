"""Abstract model of Website-state snapshot reads, draft 0.4 (R4-01, C10).

Specification tooling only. Proofs are ABSTRACT, as in draft 0.3
(m1-draft-0.3/tools/snapshot_model.py): a proof built from state X verifies
against root R exactly when it is unforged, covers the requested keys and X's
root is R. Not an MPT verifier; no evidence about pocold (E03).

Change from 0.3 (C10): after a proof verifies, the decoded Website words are
validated in a fixed order before any selection outcome. Every violation is
'stateInvariant' with zero frames and no restart (the state is consistently
read but impossible for a conforming contract). Only status 1 (published) may
render; status 2 (revoked) may only reach the interstitial; any other status
can never render.

State format (abstract decoded words, fields default to 0 when absent):
  website = {versionCount, currentVersion, slot2HighBits,
             versions: {"<id>": {status, manifestHash, manifestLen,
                                 publishedBlock, chunkCount, word1HighBits}}}
A version id that is <= versionCount but missing from `versions` is an
all-zero record ("absent").
"""

PUBLISHED, DRAFT, REVOKED = 1, 0, 2
STATUS_DOMAIN = (DRAFT, PUBLISHED, REVOKED)
MAX_ATTEMPTS = 4                      # first attempt + at most 3 restarts
VERSION_LIMIT = 1024                  # FD:L904
ZERO_HASH = '0x' + '00' * 32


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


def _slot2(st):
    w = st['website']
    return {'versionCount': w.get('versionCount', 0), 'currentVersion': w.get('currentVersion', 0),
            'slot2HighBits': w.get('slot2HighBits', 0)}


def _record(st, n):
    v = st['website'].get('versions', {}).get(str(n))
    rec = {'status': 0, 'manifestHash': ZERO_HASH, 'manifestLen': 0, 'publishedBlock': 0,
           'chunkCount': 0, 'word1HighBits': 0}
    if v:
        rec.update(v)
    return rec


def _proof(node, step, root, keys):
    if 'error' in step:
        return None
    st = node.states[step['from']]
    if step.get('forged') or st['root'] != root:
        return None
    if step.get('keys', keys) != keys:
        return None
    if keys == 'slot2':
        return _slot2(st)
    return _record(st, int(keys.split(':')[1]))


def slot2_invariant(s2):
    """R4-01 checks S1-S3, in order. Returns an invariant rule ID or None."""
    if s2['slot2HighBits']:
        return 'state.slot2Bits'
    if s2['versionCount'] > VERSION_LIMIT:
        return 'state.countRange'
    if s2['currentVersion'] > s2['versionCount']:
        return 'state.currentRange'
    return None


def record_invariant(rec, anchor_number):
    """R4-01 checks V1-V4, in order. Returns an invariant rule ID or None."""
    if rec['word1HighBits']:
        return 'state.recordBits'
    if rec['manifestLen'] == 0 and rec['manifestHash'] == ZERO_HASH and rec['chunkCount'] == 0:
        return 'state.recordAbsent'
    if rec['status'] not in STATUS_DOMAIN:
        return 'state.statusDomain'
    pb = rec['publishedBlock']
    if rec['status'] == DRAFT and pb != 0:
        return 'state.publishedBlock'
    if rec['status'] != DRAFT and not 1 <= pb <= anchor_number:
        return 'state.publishedBlock'
    return None


def load_p20(states, script, request):
    """request: {'kind': 'default'} or {'kind': 'explicit', 'version': n}."""
    node = Node(states, script)
    restarts = -1
    for _ in range(MAX_ATTEMPTS):
        restarts += 1
        a = node.take('anchor')
        hdr = dict(states[a['from']], label=a['from'])
        s2 = _proof(node, node.take('proof1'), hdr['root'], 'slot2')
        if s2 is None:
            continue
        inv = slot2_invariant(s2)
        if inv:
            return _done(node, 'stateInvariant', restarts, hdr, invariant=inv)
        default = request['kind'] == 'default'
        n = s2['currentVersion'] if default else request['version']
        if default and n == 0:
            return _done(node, 'noSite', restarts, hdr)
        if n > s2['versionCount']:
            return _done(node, 'versionNotFound', restarts, hdr)
        rec = _proof(node, node.take('proof2'), hdr['root'], 'version:%d' % n)
        if rec is None:
            continue
        inv = record_invariant(rec, hdr['number'])
        if inv:
            return _done(node, 'stateInvariant', restarts, hdr, invariant=inv)
        if default:
            if rec['status'] != PUBLISHED:
                return _done(node, 'stateInvariant', restarts, hdr, invariant='state.currentNotPublished')
            return _done(node, 'render', restarts, hdr, n, rec)
        if rec['status'] == DRAFT:
            return _done(node, 'refuseDraft', restarts, hdr)
        if rec['status'] == REVOKED:
            return _done(node, 'revokedInterstitial', restarts, hdr, n, rec)
        if rec['status'] == PUBLISHED:
            banner = None if n == s2['currentVersion'] else 'notCurrent'
            return _done(node, 'render', restarts, hdr, n, rec, banner=banner)
        raise AssertionError('unreachable: status domain checked')     # never render by default
    return _done(node, 'inconsistent', restarts, None)


def _done(node, result, restarts, hdr, n=None, rec=None, banner=None, invariant=None):
    if node.pos != len(node.script):
        raise ScriptError('%d unused script steps' % (len(node.script) - node.pos))
    out = {'result': result, 'restarts': restarts, 'calls': list(node.log),
           'frames': 1 if result == 'render' else 0, 'mixed': False}
    if hdr:
        out['anchorState'] = hdr['label']
    if rec is not None:
        out['version'] = n
        out['manifestHash'] = rec['manifestHash']
    if banner:
        out['banner'] = banner
    if invariant:
        out['invariant'] = invariant
    return out


def upgrade_03_states(states):
    """Explicit inheritance transform for the 0.3 scenario states (R4-01).

    0.3 states carry only status and manifestHash. A conforming record also has
    manifestLen >= 1, one canonical chunk and, if published or revoked, a
    publishedBlock in 1..N. The transform fills manifestLen 79, chunkCount 1,
    publishedBlock 1 (status 1 or 2) or 0 (status 0). Nothing else changes.
    """
    out = {}
    for name, st in states.items():
        st = {k: v for k, v in st.items()}
        w = dict(st['website'])
        vs = {}
        for vid, v in w.get('versions', {}).items():
            v = dict(v)
            v.setdefault('manifestLen', 79)
            v.setdefault('chunkCount', 1)
            v.setdefault('publishedBlock', 0 if v['status'] == DRAFT else 1)
            vs[vid] = v
        w['versions'] = vs
        st['website'] = w
        out[name] = st
    return out
