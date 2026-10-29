"""Bridge reference harness, draft 0.5 (R5-02 C15, R5-03 C16). Specification tooling only.

Extends m1-draft-0.4/tools/bridge_ref.py without copying it:
- httpsUrl is decided ONLY by the Node WHATWG oracle output
  (results/url-oracle-0.5.json), keyed by the exact string. A value the
  oracle has not evaluated raises OracleMissing; the 0.4 Python
  approximation is never consulted.
- Pending replaces the 0.4 counter model: each accepted request gets an
  internal opaque handle, independent of the frame-supplied id.
"""

import json
from pathlib import Path

import bridge_ref as B04                      # 0.4 module, resolved by the runner's sys.path

ORACLE_FILE = Path(__file__).resolve().parent.parent / 'results' / 'url-oracle-0.5.json'


class OracleMissing(Exception):
    pass


def load_oracle(path=ORACLE_FILE):
    if not Path(path).exists():
        return None
    doc = json.loads(Path(path).read_text(encoding='utf-8'))
    return {r['input']: bool(r['accept']) for r in doc['results']}, doc.get('node')


_ORACLE = {}


def install_oracle(table):
    """Route B04's httpsUrl primitive to the oracle table (exact-string lookup)."""
    _ORACLE.clear()
    _ORACLE.update(table or {})

    def https_url(v):
        if not isinstance(v, str):
            return False                      # type failure needs no URL parsing
        if v not in _ORACLE:
            raise OracleMissing(v[:80])
        return _ORACLE[v]
    B04._https_url = https_url


check = B04.check
Bucket = B04.Bucket
KINDS = B04.KINDS


class Pending:
    """pendingReads with internal handles (R5-03).

    route(session, frame, frame_id) -> {'accepted': True, 'handle': h} or the
    -32005 {pending} error. h is allocated from a worker-wide counter and never
    derived from frame_id, so equal frame ids (repeated in one frame, or in
    different frames or sessions) never alias. The reply envelope uses the
    frame-supplied id unchanged (baseline response-id semantics, FD:L1113).
    """

    def __init__(self):
        self.next_handle, self.global_ = 1, 0
        self.session = {}
        self.req = {}                 # handle -> {session, frame, frameId, state}

    def route(self, session, frame, frame_id):
        if self.session.get(session, 0) >= B04.PENDING_SESSION_MAX or self.global_ >= B04.PENDING_GLOBAL_MAX:
            return {'code': -32005, 'data': {'reason': 'pending'}}
        h = 'h%d' % self.next_handle
        self.next_handle += 1
        self.session[session] = self.session.get(session, 0) + 1
        self.global_ += 1
        self.req[h] = {'session': session, 'frame': frame, 'frameId': frame_id, 'state': 'live'}
        return {'accepted': True, 'handle': h}

    def _release(self, h):
        r = self.req.pop(h)
        self.session[r['session']] -= 1
        self.global_ -= 1
        return r

    def final_reply(self, h):
        if self.req[h]['state'] != 'live':
            raise AssertionError('an orphan gets no reply')
        r = self._release(h)
        return {'toFrame': r['frame'], 'id': r['frameId']}

    def teardown_frame(self, frame):
        for r in self.req.values():
            if r['frame'] == frame and r['state'] == 'live':
                r['state'] = 'orphan'

    def settle_orphan(self, h):
        if self.req[h]['state'] != 'orphan':
            raise AssertionError('only orphans settle without reply')
        self._release(h)
        return None

    def counts(self, session):
        return {'session': self.session.get(session, 0), 'global': self.global_}
