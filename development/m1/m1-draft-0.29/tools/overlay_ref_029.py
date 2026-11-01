"""Reference models for the M1 draft 0.29 delegated overlay (author turn 027). SPECIFICATION FIXTURE TOOLING ONLY.
Python standard library only. No real clock, network, browser, EVM or chain. NOT executed by the author.

  Budget, DeadlineRpc   U14 whole-load deadline in VIRTUAL time (M1-SPEC-0.29-OVERLAY.md section 3): 10000 ms from the
                        first request through the final verdict; request timeout = min(10000, remaining budget); an event
                        at elapsed >= 10000 is expiry; a valid -32021 delay must end before the deadline (P-D29-1).
  header_net_check      the unchanged 0.27 checker (m1-draft-0.27/tools/v1_ref_027.py check_window) inside one Budget.
  run_steps, run_ops    abstract virtual-time engine: content loads (CR-M1-01 rev 2 with the deadline amendment) and
                        abstract HeaderNetCheck timings.
  canonical_split, contract_args, proof2_keys, proof_bytes   U08 alternative A (contract side and proof budget); the
                        viewer side is the unchanged 0.2 rule manifest.split (m1-draft-0.2/tools/m1model.py).
  view                  U02 interstitial on top of the unchanged 0.25 select_version.
  Website               U10 keep-with-warning: authorization rows of m1-draft-0.2 section 5.2. Not a contract.
"""

DEADLINE_MS = 10000                 # U14 (delegated): whole load through the verdict
REQUEST_CAP_MS = 10000              # CR-M1-01 rev 2 section 3.1 per-request deadline, now min(10000, remaining)
RETRY_RANGE = {'busy': (250, 2000), 'rate': (0, 2000)}      # network.md:234 (busy); P-C32-2 alternative A (rate)
MAX_RETRIES = 3                     # browser.md:41; CR-M1-01 rev 2 section 6
MAX_ATTEMPTS = 4                    # m1-draft-0.2 section 4.2 step 4 (3 restarts)
PARALLEL_MAX = 8                    # FD:L905
CHUNK_MAX = 24575
MANIFEST_MAX = 65536
UNSCRIPTED = '{"jsonrpc":"2.0","id":0,"error":{"code":-32603,"message":"unscripted"}}'   # as 0.27 ScriptedRpc


class Expired(Exception):
    def __init__(self, at, where):
        super().__init__(where)
        self.at, self.where = at, where


class DoesNotFit(Exception):
    """A valid -32021 delay that cannot end before the deadline: never slept (P-D29-1)."""

    def __init__(self, at, delay):
        super().__init__('retry delay does not fit')
        self.at, self.delay = at, delay


class Budget:
    """One covered load. Absolute virtual ms; the budget starts at the first request (t0)."""

    def __init__(self, t0=0, deadline_ms=DEADLINE_MS):
        self.t0, self.t, self.end = t0, t0, t0 + deadline_ms
        self.slept = self.sends = 0
        self.log = []

    def elapsed(self):
        return self.t - self.t0

    def timeout(self):
        return min(REQUEST_CAP_MS, self.end - self.t)

    def reply(self, latency, what):
        """latency None = no reply. A reply arriving at or after the request timeout never counts; since the
        remaining budget is <= 10000 the timeout always ends exactly at the whole-load deadline."""
        self.sends += 1
        lim = self.timeout()
        if latency is None or latency >= lim:
            raise Expired(self.t + lim, what)
        self.t += latency
        self.log.append(['reply', what, self.t - self.t0])

    def sleep(self, ms, what):
        if self.t + ms >= self.end:
            raise DoesNotFit(self.t, ms)
        self.t += ms
        self.slept += ms
        self.log.append(['slept', what, ms])

    def compute(self, ms, what):
        if self.t + ms >= self.end:
            raise Expired(self.end, what)
        self.t += ms
        self.log.append(['computed', what, self.t - self.t0])


class DeadlineRpc:
    """The 0.27 ScriptedRpc with a Budget: script = [(latency_ms | None, reply_text)]; records method, params, send time."""

    def __init__(self, budget, script):
        self.b, self.script, self.calls = budget, list(script), []

    def call(self, method, params):
        self.calls.append({'t': self.b.t, 'method': method, 'params': list(params)})
        lat, text = self.script.pop(0) if self.script else (0, UNSCRIPTED)
        self.b.reply(lat, method)
        return text

    def sleep(self, ms):
        self.b.sleep(ms, 'retryAfterMs')


def _vi(detail):
    return {'ok': False, 'rule': 'viewIncomplete', 'at': None, 'detail': detail, 'frame': False, 'cancel': 4901,
            'log': {'code': -32019, 'data': {'rule': 'viewIncomplete'}}, 'rootReliance': False}


def header_net_check(V, cfg, script, clock_s, compute_ms, t0=0, cache=True, ctr=None):
    """browser.md:35-56 under U14: the unchanged 0.27 check_window, then compute_ms of verdict computation not
    represented by the reference (a hypothetical input, never a measurement)."""
    b = Budget(t0)
    rpc = DeadlineRpc(b, script)
    ctr = ctr if ctr is not None else V.Counters()
    why = None
    try:
        verdict = V.check_window(cfg, rpc, clock_s, ctr, cache)
        b.compute(compute_ms, 'verdict')
        at = b.elapsed()
    except Expired as e:
        verdict, at, why = _vi({'deadline': 'expired', 'where': e.where}), e.at - t0, 'expired'
    except DoesNotFit as e:
        verdict, at, why = _vi({'deadline': 'retryDelayDoesNotFit', 'delayMs': e.delay}), e.at - t0, 'retryDelayDoesNotFit'
    return {'verdict': verdict, 'atMs': at, 'why': why, 'sleptMs': b.slept, 'requests': rpc.calls, 'counters': ctr}


# ------------------------------------------------------------------ abstract virtual-time engine

def _parse(rep):
    if ':' in rep:
        k, v = rep.split(':', 1)
        if k in RETRY_RANGE:
            return k, int(v)
    return rep, None


def request(b, replies, what, retry=True):
    """One request with its -32021 retries. replies = [[latency | None, 'ok' | 'busy:<ms>' | 'rate:<ms>' | other]].
    Returns 'ok' or a failure kind; raises Expired / DoesNotFit."""
    retries = 0
    for lat, rep in replies:
        b.reply(lat, what)
        kind, ms = _parse(rep)
        if kind == 'ok':
            return 'ok'
        if kind in RETRY_RANGE:
            if not retry:
                return 'noRetry'
            lo, hi = RETRY_RANGE[kind]
            if not lo <= ms <= hi:
                return 'malformed'                        # C32: out-of-range delay, no sleep
            if retries >= MAX_RETRIES:
                return 'exhausted'                        # the 4th valid -32021, no 4th sleep
            retries += 1
            b.sleep(ms, what)
            continue
        return kind
    return 'unscripted'


VERDICTS = ('noWebsite', 'stateInvariant')


def run_steps(steps, b, kind):
    """kind 'content': CR-M1-01 rev 2 attempts (a failed attempt restarts; 4 attempts at most -> 'inconsistent');
    expiry -> 'unavailable' with zero frames (P-D29-2). kind 'hnc': any failure -> 'viewIncomplete'.
    Steps: {'r': name, 'replies': [...], 'retry': bool} | {'b': name, 'lat': [...]} (parallel) | {'c': ms}."""
    expired_outcome = 'unavailable' if kind == 'content' else 'viewIncomplete'
    attempts = 1
    res = {'outcome': None, 'atMs': None, 'why': None}
    try:
        for st in steps:
            if 'c' in st:
                b.compute(st['c'], 'compute')
                continue
            if 'b' in st:
                lats = st['lat']
                if len(lats) > PARALLEL_MAX:
                    raise ValueError('parallelLimit')
                b.sends += len(lats) - 1
                b.reply(None if any(x is None for x in lats) else max(lats), st['b'])
                continue
            try:
                r = request(b, st['replies'], st['r'], st.get('retry', True))
            except DoesNotFit as e:
                if kind == 'hnc':
                    raise
                r = 'retryDelayDoesNotFit'
                b.log.append(['doesNotFit', st['r'], e.delay])
            if r == 'ok':
                continue
            if r in VERDICTS:
                res.update(outcome=r, atMs=b.elapsed())
                break
            if kind == 'hnc':
                res.update(outcome='viewIncomplete', atMs=b.elapsed(), why=r)
                break
            if attempts >= MAX_ATTEMPTS:
                res.update(outcome='inconsistent', atMs=b.elapsed(), why='attemptsExhausted')
                break
            attempts += 1
        else:
            res.update(outcome='render' if kind == 'content' else 'ok', atMs=b.elapsed())
    except Expired as e:
        res.update(outcome=expired_outcome, atMs=e.at - b.t0, why='expired')
    except DoesNotFit as e:
        res.update(outcome='viewIncomplete', atMs=e.at - b.t0, why='retryDelayDoesNotFit')
    res.update(sleptMs=b.slept, sends=b.sends, attempts=attempts)
    return res


def run_ops(ops, mode):
    """RP: HeaderNetCheck then content load. mode 'separate': each operation has its own budget from its first request;
    'shared': one budget from the first request of the first operation (P-D29-3 open; both represented)."""
    out, t, shared = [], 0, None
    for op in ops:
        if mode == 'shared':
            shared = shared or Budget(0)
            b = shared
            start = b.t
        else:
            b = Budget(t)
            start = t
        r = run_steps(op['steps'], b, op['kind'])
        r['absoluteAtMs'] = r['atMs'] + b.t0
        r['startMs'] = start
        out.append(r)
        t = r['absoluteAtMs']
        if r['outcome'] not in ('ok', 'render'):
            break
    return out


# ------------------------------------------------------------------ U08 alternative A

def canonical_split(n):
    """m1-draft-0.2 P22: exactly ceil(n/24575) chunks, all 24575 bytes except the last."""
    if n < 1:
        return []
    full = (n - 1) // CHUNK_MAX
    return [CHUNK_MAX] * full + [n - CHUNK_MAX * full]


def contract_args(n, n_chunks, lens):
    """createVersion argument checks of m1-draft-0.2 section 5.2 after authorization and VersionLimit; the U07 chunk and
    hash checks follow. ManifestSplit(i): i = first position where lens and the canonical split differ, a missing element
    counting as a difference (P-U08-1)."""
    if not 1 <= n <= MANIFEST_MAX:
        return 'ManifestLength(%d)' % n
    if n_chunks != len(lens):
        return 'ChunkArrays(%d,%d)' % (n_chunks, len(lens))
    canon = canonical_split(n)
    if lens != canon:
        i = next(k for k in range(max(len(lens), len(canon))) if k >= len(lens) or k >= len(canon) or lens[k] != canon[k])
        return 'ManifestSplit(%d)' % i
    return None


def proof2_keys(n):
    """CR-M1-01 proof 2: always base, base+1, base+2 and e0..e2; unused element slots prove the value 0."""
    k = len(canonical_split(n))
    return {'chunks': k, 'keys': 6, 'elementSlotsUsed': k, 'zeroProvedElementSlots': 3 - k}


def proof_bytes(keys, nodes=65, node_bytes=564, fixed=4096):
    """m1-draft-0.4/annex/recv-limits.json proofBoundDerivation: (k + 1) paths of 65 nodes, 1133 JSON bytes per node."""
    per_node = 2 * node_bytes + 2 + 2 + 1
    return (keys + 1) * nodes * per_node + fixed


# ------------------------------------------------------------------ U02 interstitial

def view(S, state, request_, actions=(), policy='interstitial'):
    """S = the 0.25 supplements_ref module (select_version unchanged). actions: 'click' (deliberate proceed), 'dismiss',
    'navigate' (in-session, P30: no new anchor), 'reload' (user reload: section 4.2 runs again; consent is per load,
    P-U02-1). A click starts a new covered load (P-U02-2)."""
    trace = []
    cur = {}

    def load_once():
        r = S.select_version(state, request_, policy)
        cur['r'] = r
        if r['result'] == 'interstitialRevoked':
            trace.append({'shown': 'interstitial', 'frame': False, 'chunkFetches': 0})
            cur['loaded'], cur['banner'] = False, None
        else:
            trace.append({'shown': r['result'], 'version': r.get('version'), 'frame': r['frame'], 'banner': r.get('banner'),
                          'chunkFetches': 'manifest+files' if r['frame'] else 0})
            cur['loaded'], cur['banner'] = bool(r['frame']), r.get('banner')

    load_once()
    for a in actions:
        last = trace[-1]['shown']
        if a == 'click' and last == 'interstitial':
            after = cur['r']['afterExplicitClick']
            trace.append({'shown': 'load', 'version': after['version'], 'frame': True, 'banner': after['banner'],
                          'chunkFetches': 'manifest+files', 'newCoveredLoad': True})
            cur['loaded'], cur['banner'] = True, after['banner']
        elif a == 'dismiss' and last == 'interstitial':
            trace.append({'shown': 'declinedRevoked', 'frame': False, 'chunkFetches': 0})
        elif a == 'navigate' and cur['loaded']:
            trace.append({'shown': 'page', 'frame': True, 'banner': cur['banner'], 'newAnchor': False})
        elif a == 'reload':
            load_once()
        else:
            trace.append({'shown': 'ignored', 'action': a})
    return trace


# ------------------------------------------------------------------ U10 keep-with-warning

PUBLISHER_CALLS = ['createVersion', 'publish', 'setCurrent']


class Website:
    """Owner/publisher authorization of m1-draft-0.2 sections 5.1-5.2 under U10 keep-with-warning (P-U10-1 warning
    points). Addresses are labels; '0' is the zero address. Versions: {id: 'draft' | 'published' | 'revoked'}.
    auto_revoke=True models the REJECTED U10 alternative (previous owner loses publishing on transfer), for comparison."""

    def __init__(self, owner, versions=None, current=0, auto_revoke=False):
        self.auto_revoke = auto_revoke
        self.owner, self.pending = owner, None
        self.publishers = [owner]
        self.status = {int(k): v for k, v in (versions or {}).items()}
        self.current = current
        self.events, self.warnings = [], []
        self.active = None

    def _warn(self, stage, prev):
        w = {'stage': stage, 'previousOwner': prev, 'retainedCalls': list(PUBLISHER_CALLS),
             'removal': 'setPublisher(%s,false) by the new owner' % prev}
        self.warnings.append(w)
        self.active = w

    def call(self, caller, fn, *args):
        ev0 = len(self.events)
        r = self._call(caller, fn, *args)
        if r is None:
            return {'ok': True, 'events': self.events[ev0:]}
        return {'revert': r}

    def _call(self, caller, fn, *args):
        if fn == 'setCurrent':
            if caller not in self.publishers:
                return 'NotPublisher(%s)' % caller
            n = args[0]
            if n not in self.status:
                return 'UnknownVersion(%d)' % n
            if self.status[n] != 'published':
                return 'WrongStatus(%d)' % n
            if n == self.current:
                return 'AlreadyCurrent(%d)' % n
            prev, self.current = self.current, n
            self.events.append('CurrentVersionChanged(%d,%d)' % (prev, n))
            return None
        if fn == 'revoke':
            if caller != self.owner:
                return 'NotOwner(%s)' % caller
            n = args[0]
            if n not in self.status:
                return 'UnknownVersion(%d)' % n
            if n == self.current:
                return 'CurrentVersion(%d)' % n
            if self.status[n] != 'published':
                return 'WrongStatus(%d)' % n
            self.status[n] = 'revoked'
            self.events.append('VersionRevoked(%d)' % n)
            return None
        if fn == 'setPublisher':
            if caller != self.owner:
                return 'NotOwner(%s)' % caller
            p, enable = args
            if p == '0':
                return 'ZeroAddress()'
            if enable:
                if p in self.publishers:
                    return None
                if len(self.publishers) == 16:
                    return 'PublisherLimit()'
                self.publishers.append(p)
            else:
                if p not in self.publishers:
                    return None
                self.publishers.remove(p)
                if self.active and self.active['previousOwner'] == p:
                    self.active = None
            self.events.append('PublisherChanged(%s,%s)' % (p, 'true' if enable else 'false'))
            return None
        if fn == 'transferOwnership':
            if caller != self.owner:
                return 'NotOwner(%s)' % caller
            c = args[0]
            if c == self.owner:
                return 'InvalidCandidate(%s)' % c
            self.pending = None if c == '0' else c
            self.events.append('OwnershipNominated(%s,%s)' % (self.owner, c))
            if c == '0':
                if self.active and self.active['stage'] == 'nomination':
                    self.active = None
            elif self.owner in self.publishers and not self.auto_revoke:
                self._warn('nomination', self.owner)
            return None
        if fn == 'acceptOwnership':
            if caller != self.pending:
                return 'NotPendingOwner(%s)' % caller
            prev, self.owner, self.pending = self.owner, caller, None
            self.events.append('OwnershipTransferred(%s,%s)' % (prev, caller))
            if prev in self.publishers:
                if self.auto_revoke:
                    self.publishers.remove(prev)
                    self.events.append('PublisherChanged(%s,false)' % prev)
                else:
                    self._warn('accepted', prev)
            return None
        raise ValueError(fn)

    def state(self):
        return {'owner': self.owner, 'pendingOwner': self.pending, 'publishers': sorted(self.publishers),
                'current': self.current, 'activeWarning': self.active}
