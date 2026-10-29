"""RecvGuard reference model (R7-04: annex rows R1-R4, D98). Specification tooling only.

- read(): read steps 2-6 of FD:L1236-1251 over a scripted response (status,
  headers, body segments), counting bytes after decompression, without ever
  materialising more than recvLimit + one segment.
- Pools: reserve/release with FIFO queues and the 2 s busy timeout of step 1,
  SITE_POOL, SESSION_RECV_MAX, TRUST_POOL, CONTENT_POOL and RECV_INFLIGHT_MAX
  (FD:L1278-1283), on a fake clock (MR10a-c, MR10f, BR20d).
Model evidence only: real sockets, Chrome's stream reader and TestHooks are future tests.
"""

import json
import zlib

import bridge_ref as B04

MiB, KiB = 1 << 20, 1 << 10
SITE_POOL, SESSION_RECV_MAX, TRUST_POOL = 32 * MiB, 10 * MiB, 2 * MiB      # FD:L1279-1281
CONTENT_POOL = 8 * 68 * KiB                                                  # FD:L1282
RECV_INFLIGHT_MAX = 48                                                       # FD:L1283
RESERVE_WAIT_MS = 2000                                                       # FD:L1237
MESSAGE_MAX_UNITS, DATA_MAX_BYTES = 256, 4096                                # FD:L1247-1248
PARSE_DEPTH_MAX = 16                                                         # FD:L1244


# --- read steps 2-6 ---------------------------------------------------------
def segments(spec):
    """Yield body bytes from a script body spec without building it whole."""
    for seg in spec:
        if 'hex' in seg:
            yield bytes.fromhex(seg['hex'])
        elif 'text' in seg:
            yield seg['text'].encode('utf-8')
        elif 'repeat' in seg:
            unit = seg['repeat'].encode('latin-1')
            total, chunk = seg['count'], seg.get('chunk', 65536)
            while total > 0:
                n = min(total, chunk)
                yield unit * n
                total -= n


def gzip_zero_stream(uncompressed, level=9, piece=MiB):
    """P: deterministic gzip-framed stream of `uncompressed` zero bytes (MR4)."""
    c = zlib.compressobj(level, zlib.DEFLATED, 31)
    left, zero = uncompressed, bytes(piece)
    while left > 0:
        n = min(left, piece)
        out = c.compress(zero[:n])
        if out:
            yield out
        left -= n
    yield c.flush()


def stream(script):
    """Body pieces as the client counts them: after decompression (FD:L1240)."""
    if script.get('gzip'):
        d = zlib.decompressobj(31)
        for comp in gzip_zero_stream(script['gzip']['uncompressed']):
            out = d.decompress(comp)
            if out:
                yield out
    else:
        yield from segments(script['body'])


def counts_past(script, n):
    """MR-neg probe: True if a reader WITHOUT a limit would count more than n bytes."""
    total = 0
    for piece in stream(script):
        total += len(piece)
        if total > n:
            return True
    return False


def read(script, recv_limit):
    """Returns {'outcome': 'ok'|'recvLimit'|'recvParse'|'recvDepth', 'bytesCounted', 'result'/'error'}."""
    cl = dict((k.lower(), v) for k, v in script.get('headers', [])).get('content-length')
    if cl is not None and int(cl) > recv_limit:
        return {'outcome': 'recvLimit', 'bytesCounted': 0, 'step': 2}
    it = stream(script)
    buf, counted = [], 0
    for piece in it:
        if counted + len(piece) > recv_limit:                    # step 3: the piece that crosses is dropped
            return {'outcome': 'recvLimit', 'bytesCounted': counted, 'step': 3}
        counted += len(piece)
        buf.append(piece)
    raw = b''.join(buf)
    try:
        text = raw.decode('utf-8')                               # step 4: fatal decoding
    except UnicodeDecodeError:
        return {'outcome': 'recvParse', 'bytesCounted': counted, 'step': 4}
    if B04.json_depth(text) > PARSE_DEPTH_MAX:
        return {'outcome': 'recvDepth', 'bytesCounted': counted, 'step': 4}
    try:
        msg = json.loads(text, object_pairs_hook=B04._pairs, parse_constant=B04._no_constants)
    except (B04._Reject, ValueError):
        return {'outcome': 'recvParse', 'bytesCounted': counted, 'step': 4}
    if isinstance(msg, dict) and isinstance(msg.get('error'), dict):   # step 5
        e = dict(msg['error'])
        m = e.get('message', '')
        if isinstance(m, str):
            e['message'] = B04.utf16_prefix(m, MESSAGE_MAX_UNITS)
        if 'data' in e and len(json.dumps(e['data'], separators=(',', ':')).encode()) > DATA_MAX_BYTES:
            del e['data']
        return {'outcome': 'ok', 'bytesCounted': counted, 'error': e}
    return {'outcome': 'ok', 'bytesCounted': counted, 'result': msg.get('result') if isinstance(msg, dict) else None}


# --- pools (step 1) ---------------------------------------------------------
class Pools:
    """FIFO reservation per pool; SESSION_RECV_MAX and RECV_INFLIGHT_MAX apply to
    SITE_POOL and CONTENT_POOL as stated in R7-04 scopes; TRUST_POOL is outside the
    in-flight count (FD:L1283)."""

    def __init__(self):
        self.cap = {'site': SITE_POOL, 'trust': TRUST_POOL, 'content': CONTENT_POOL}
        self.used = {'site': 0, 'trust': 0, 'content': 0}
        self.session = {}
        self.inflight = 0
        self.queue = []                      # [(req, pool, session, bytes, t_submit)]
        self.granted, self.busy, self.now = {}, {}, 0

    def _fits(self, pool, session, n):
        if self.used[pool] + n > self.cap[pool]:
            return False
        if pool == 'site' and self.session.get(session, 0) + n > SESSION_RECV_MAX:
            return False
        if pool in ('site', 'content') and self.inflight + 1 > RECV_INFLIGHT_MAX:
            return False
        return True

    def _grant(self, req, pool, session, n):
        self.used[pool] += n
        if pool == 'site':
            self.session[session] = self.session.get(session, 0) + n
        if pool in ('site', 'content'):
            self.inflight += 1
        self.granted[req] = (pool, session, n, self.now)

    def reserve(self, req, pool, session, n):
        if not any(q[1] == pool for q in self.queue) and self._fits(pool, session, n):
            self._grant(req, pool, session, n)
            return 'reserved'
        self.queue.append((req, pool, session, n, self.now))
        return 'waiting'

    def release(self, req):
        pool, session, n, _ = self.granted.pop(req)
        self.used[pool] -= n
        if pool == 'site':
            self.session[session] -= n
        if pool in ('site', 'content'):
            self.inflight -= 1
        self._pump()

    def _pump(self):
        progressed = True
        while progressed:
            progressed = False
            for pool in ('site', 'trust', 'content'):
                head = next((q for q in self.queue if q[1] == pool), None)   # strict FIFO per pool (P)
                if head and self._fits(head[1], head[2], head[3]):
                    self.queue.remove(head)
                    self._grant(head[0], head[1], head[2], head[3])
                    progressed = True

    def advance(self, t):
        """Move the clock to t; waiting requests older than 2 s become busy."""
        while True:
            pending_deadlines = [q[4] + RESERVE_WAIT_MS for q in self.queue if q[4] + RESERVE_WAIT_MS <= t]
            if not pending_deadlines:
                break
            dl = min(pending_deadlines)
            self.now = dl
            for q in [q for q in self.queue if q[4] + RESERVE_WAIT_MS == dl]:
                self.queue.remove(q)
                self.busy[q[0]] = dl
            self._pump()
        self.now = t
        self._pump()

    def session_bytes(self, s):
        return self.session.get(s, 0)
