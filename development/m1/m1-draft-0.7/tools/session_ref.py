"""SiteSession reference model and BridgeRef session extension (R7-05: annex rows S1-S4, D99).

Specification tooling only. One SiteSession, keyed (tabId, netKey, siteAddress),
owns the message bucket, pendingReads, the nav bucket, the SESSION_RECV_MAX
reservation and the ChunkCache (FD:L1290-1317). Frames created by site_navigate
or reload join the same session and renew no budget (FD:L1303). Messages from a
torn-down frame's port are dropped after it closes and consume no token
(FD:L1306). Every line is produced in the BridgeTrace format of 0.6
(tools/bridgeref_06.TRACE_FIELDS).
"""

import json

import bridge_ref as B04
import bridge_ref_06 as B6
import sitestorage_ref as SS
import bridgeref_06 as BR


class Session:
    def __init__(self, created_ms, key=('tab1', 'netKey', 'site'), frame_budget_fault=False):
        self.key = key
        self.bucket = B6.Bucket(created_ms)
        self.nav = SS.NavBucket(created_ms)
        self.pending = B6.Pending()
        self.frame = 'F1'
        self.frames_seen = ['F1']
        self.fault = frame_budget_fault           # bridge-fault:frameBudget (negative control)
        self.lines = []
        self.handles = []                          # forwarded handles, in order
        self.seq = 0

    def _new_frame(self, created_ms):
        n = len(self.frames_seen) + 1
        self.frame = 'F%d' % n
        self.frames_seen.append(self.frame)
        if self.fault:                             # the fault gives the frame its own budgets
            self.bucket = B6.Bucket(created_ms)
            self.pending = B6.Pending()

    def message(self, t, frame, raw):
        """Returns (check, route) as in the BR20a columns; None if the port is closed."""
        if frame != self.frame:
            return None                            # dropped, no token (FD:L1306)
        self.seq += 1
        before = self.bucket.tokens_mt
        r = B6.check(raw, t, self.bucket)
        line = dict.fromkeys(BR.TRACE_FIELDS)
        line.update(frame=frame, seq=self.seq, id=r['id'], arrivalMs=t, tokensBefore_mt=before, bpre=r['bpre'],
                    stage=r['stage'], code=r['code'], route=r['route'])
        check = 'pass' if r['bpre'] == 'pass' else r['bpre']
        route = '-'
        if r['stage'] == 'ok' and r['route'] in ('readClient', 'logClient'):
            line['pendingBefore'] = self.pending.counts('S')['session']
            o = self.pending.route('S', frame, r['id'])
            if o.get('accepted'):
                route = 'forwarded'
                line['forwardedSeq'] = self.seq
                self.handles.append(o['handle'])
            else:
                route = 'pending'
                line.update(stage='B4', code=-32005)
        elif r['stage'] == 'ok' and r['route'] == 'navigator':
            if self.nav.take(t):
                route = 'nav'
                self.pending.teardown_frame(frame)           # orphans; no reset (FD:L1150)
                self._new_frame(t)
            else:
                route = 'navRejected'
                line.update(stage='B4', code=-32005)
        elif r['stage'] != 'ok':
            route = '-'
        self.lines.append(line)
        return check, route

    def settle_orphan(self, index):
        """FakeTransport.settle for the index-th forwarded request (1-based)."""
        h = self.handles[index - 1]
        self.pending.settle_orphan(h)

    def counts(self):
        return {'tokens_mt': self.bucket.tokens_mt, 'pending': self.pending.counts('S')['session'], 'nav': self.nav.tokens_mt}


def msg(i, method, params='[]'):
    return '{"id":%d,"kind":"%s","payload":{"method":"%s","params":%s}}' % (
        i, 'nav' if method.startswith('site_') else 'rpc_read', method, params)


def br20c_sim(duration_ms=60000, hold_ms=3000, retry_ms=100, site_chunks=7):
    """P: BadSite's start page sends 4 eth_call then site_navigate('/') on load;
    a rejected navigation is retried every 100 ms; a new frame loads at the moment
    the navigation is accepted. eth_call replies are held 3000 ms. Each load asks
    ChunkFetcher for the site's chunks; the session ChunkCache serves repeats."""
    s = Session(0)
    events, accepted_msgs, navs, overlap_max = [], [], [], 0
    replies = []                                    # (t, handle)
    cache, getcode = set(), 0
    t, i = 0, 0

    def load(t0):
        nonlocal getcode, i
        for c in range(site_chunks):
            if c not in cache:
                cache.add(c)
                getcode += 1
        for _ in range(4):
            i += 1
            res = s.message(t0, s.frame, msg(i, 'eth_call', '[{"to":"0x976ea74026e726554db657fa54763abd0c3a0aa9"},"latest"]'))
            if res and res[0] == 'pass':
                accepted_msgs.append(t0)
            if res and res[1] == 'forwarded':
                replies.append((t0 + hold_ms, s.handles[-1]))
        return nav_attempt(t0)

    def nav_attempt(t0):
        nonlocal i
        i += 1
        res = s.message(t0, s.frame, msg(i, 'site_navigate', '["/"]'))
        if res and res[0] == 'pass':
            accepted_msgs.append(t0)
        if res and res[1] == 'nav':
            navs.append(t0)
            return ('load', t0)
        return ('retry', t0 + retry_ms)

    nxt = load(0)
    overlap_max = len(s.pending.req)
    while True:
        t = nxt[1]
        if t > duration_ms:
            break
        for rt, h in sorted(r for r in replies if r[0] <= t):
            replies.remove((rt, h))
            req = s.pending.req.get(h)
            if req and req['state'] == 'live':
                s.pending.final_reply(h)
            elif req:
                s.pending.settle_orphan(h)
        nxt = load(t) if nxt[0] == 'load' else nav_attempt(t)
        overlap_max = max(overlap_max, len(s.pending.req))      # live or orphan reads after this step
    return {'acceptedTimes': accepted_msgs, 'navTimes': navs, 'overlapMax': overlap_max, 'getCode': getcode,
            'siteChunks': site_chunks, 'sessionReservationMax': overlap_max * 1048576}
