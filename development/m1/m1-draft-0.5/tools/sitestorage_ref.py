"""SiteStorageRef and Navigator reference (R5-05, annex rows B4 and B9). Specification tooling only.

Transcribes FD:L1356-1368 (SiteStorage, D34/D96/D104), FD:L1193-1198 and
FD:L4925-4929 (Navigator, nav bucket, openExternal). Choices the baseline
leaves open are marked P in comments and listed in M1-SPEC-0.5-AMENDMENTS.md.
"""

SITE_STORE_MAX = 1048576          # FD:L1358
SITE_ENTRIES_MAX = 4096           # FD:L1365
NAV_CAP_MT, NAV_REFILL_MT_PER_MS, NAV_COST_MT = 6000, 1, 2000     # FD:L1196


def utf8len(s):
    return len(s.encode('utf-8'))


def entry(k, v):
    return utf8len(k) + utf8len(v)                     # FD:L1359


def storage_key(net_key, address):
    """FD:L1357 'site:'+netKey+':'+address. P: both as lowercase 0x-hex."""
    return 'site:' + net_key.lower() + ':' + address.lower()


class SiteStorage:
    """One (netKey, address) dictionary. disk(op) returns None (accept) or the 4300 data."""

    def __init__(self, disk=None):
        self.d, self.total, self.writes = {}, 0, 0
        self.disk = disk or (lambda op: None)
        self.fail_next_write = False

    def snapshot(self):
        return dict(self.d), self.total

    def _commit(self, new_d, new_total, op):
        rej = self.disk(op)                             # D104: every write, including delete and clear
        if rej:
            return {'code': 4300, 'data': rej}
        if self.fail_next_write:                        # the single chrome.storage.local.set failed
            self.fail_next_write = False
            return {'code': -32603, 'data': {'reason': 'transport'}}   # snapshot reloaded = unchanged
        self.d, self.total = new_d, new_total
        self.writes += 1                                # one set per accepted message (P: also for no-op deletes, U38)
        return {'result': None}

    def set(self, k, v):
        old = self.d.get(k)
        new_d = dict(self.d)
        if v is None:
            new_total = self.total - (entry(k, old) if old is not None else 0)
            new_d.pop(k, None)
            return self._commit(new_d, new_total, 'delete')            # never rejected by quota
        new_total = self.total - (entry(k, old) if old is not None else 0) + entry(k, v)
        if new_total > SITE_STORE_MAX:                                  # equality accepted
            return {'code': 4300, 'data': {'reason': 'quota', 'limit': SITE_STORE_MAX}}
        if old is None and len(self.d) + 1 > SITE_ENTRIES_MAX:          # P: quota checked before entries (U39)
            return {'code': 4300, 'data': {'reason': 'entries', 'limit': SITE_ENTRIES_MAX}}
        new_d[k] = v
        return self._commit(new_d, new_total, 'set')

    def clear(self):
        return self._commit({}, 0, 'clear')                             # never rejected by quota


class NavBucket:
    """P: the nav bucket uses the integer refill formula of the message bucket (FD:L1131)
    with the nav constants; created full at session creation; navigation does not refill it."""

    def __init__(self, created_ms):
        self.tokens_mt, self.last_ms = NAV_CAP_MT, created_ms

    def take(self, t):
        self.tokens_mt = min(NAV_CAP_MT, self.tokens_mt + NAV_REFILL_MT_PER_MS * max(0, t - self.last_ms))
        self.last_ms = t
        if self.tokens_mt >= NAV_COST_MT:
            self.tokens_mt -= NAV_COST_MT
            return True
        return False


def navigate(bucket, frame, path, t, manifest_paths, pending=None, lookup=None):
    """Navigator.navigate (FD:L4925-4929). `path` already passed the B3 pathStr schema.

    Steps: 1 nav bucket (deficit: -32005 {nav}, no teardown, the frame gets the reply);
    2 normalization (0.2 section 6.3); 3 orphan the frame's requests, cancel LoadJob,
    StoreQueue.cancelFrame, awaitActive; 4 tear down and build a new frame in the same
    session, or show the viewer 404 page. The torn-down frame gets no reply.
    """
    if not bucket.take(t):
        return {'reply': {'code': -32005, 'data': {'reason': 'nav'}}, 'teardown': False, 'steps': ['bucket']}
    r = lookup(path)
    if pending is not None:
        pending.teardown_frame(frame)
    steps = ['bucket', 'normalize', 'orphan', 'cancelLoadJob', 'storeQueue.cancelFrame', 'awaitActive', 'teardown']
    if r['kind'] == 'lookup':
        p = r['path'] or 'entry'
        if p == 'entry' or p in manifest_paths:
            return {'reply': None, 'teardown': True, 'page': p, 'steps': steps + ['newFrame']}
    return {'reply': None, 'teardown': True, 'page': '404', 'steps': steps + ['viewer404']}


class OpenExternal:
    """site_openExternal (FD:L1198, BR18e/f/h): one pending confirmation per frame."""

    def __init__(self):
        self.pending, self.tabs = set(), []

    def request(self, frame, url):
        if frame in self.pending:
            return {'code': -32005, 'data': {'reason': 'pending'}}
        self.pending.add(frame)
        return {'confirmShown': url}

    def user(self, frame, url, action):
        self.pending.discard(frame)
        if action == 'open':
            self.tabs.append({'url': url, 'opener': None})
            return {'result': None}
        return {'code': 4001}                      # cancel or close
