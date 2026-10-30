"""BridgeRef, RpcReadClient model and BridgeTrace format (R6-05 B10, R6-06 B11). Specification tooling.

- ReadClient: RpcReadClient/LogClient pending behaviour with a FakeTransport and
  FakeClock (BR19b), on top of the handle-based Pending of draft 0.5.
- bridge_ref(): BridgeRef (FD:L2659, FD:L4848): given messages with arrivalMs and
  recorded reply times, compute the expected BridgeTrace line of every message:
  Bpre (size/rate), B0-B3 stage and code (SchemaRef = annex bridge-matrix schemas),
  B4 pending for routed reads, and SiteStorageRef decisions for storage_set.
- TRACE_FIELDS / validate_trace_line(): the BridgeTrace line format of FD:L1221.
"""

import bridge_ref as B04
import bridge_ref_06 as B6
import sitestorage_ref as SS

READ_TIMEOUT_MS, LOGS_TIMEOUT_MS = 10000, 30000            # FD:L1149

TRACE_FIELDS = ['frame', 'seq', 'id', 'arrivalMs', 'tokensBefore_mt', 'bpre', 'stage', 'code', 'route',
                'pendingBefore', 'storeTotalBefore', 'storeTotalAfter', 'forwardedSeq', 'replyMs']


def validate_trace_line(line):
    """FD:L1221 format. P: stage is null for Bpre rejections; B4 for routed rejections."""
    if list(line) != TRACE_FIELDS:
        return 'fields'
    if line['bpre'] not in ('pass', 'size', 'rate'):
        return 'bpre'
    if line['bpre'] != 'pass' and line['stage'] is not None:
        return 'stageAfterBpre'
    if line['bpre'] == 'pass' and line['stage'] not in ('B0', 'B1', 'B2', 'B3', 'B4', 'ok'):
        return 'stage'
    if line['storeTotalBefore'] is not None and line['route'] != 'siteStorage':
        return 'storeOnlyForStorage'
    if line['forwardedSeq'] is not None and line['route'] not in ('readClient', 'logClient'):
        return 'forwardOnlyForReads'
    return None


class ReadClient:
    """RpcReadClient + LogClient pending accounting with FakeTransport/FakeClock (BR19b)."""

    def __init__(self):
        self.pending, self.now = B6.Pending(), 0
        self.live = {}                          # handle -> deadline
        self.replies = []                       # (frame, id, code|None)
        self.transport = []                     # handles received by the transport

    def send(self, session, frame, rid, logs=False):
        o = self.pending.route(session, frame, rid)
        if not o.get('accepted'):
            self.replies.append((frame, rid, -32005))
            return o
        h = o['handle']
        self.live[h] = self.now + (LOGS_TIMEOUT_MS if logs else READ_TIMEOUT_MS)
        self.transport.append(h)
        return o

    def release(self, h):
        """The transport delivers the final reply (or settles an orphan)."""
        self.live.pop(h, None)
        r = self.pending.req[h]
        if r['state'] == 'live':
            rep = self.pending.final_reply(h)
            self.replies.append((rep['toFrame'], rep['id'], None))
        else:
            self.pending.settle_orphan(h)

    def advance(self, ms):
        self.now += ms
        for h, deadline in sorted(self.live.items()):
            if deadline <= self.now:
                del self.live[h]
                r = self.pending.req[h]
                if r['state'] == 'live':
                    rep = self.pending.final_reply(h)
                    self.replies.append((rep['toFrame'], rep['id'], -32603))
                else:
                    self.pending.settle_orphan(h)

    def teardown(self, frame):
        self.pending.teardown_frame(frame)


def bridge_ref(messages, reply_delay_ms, created_ms, session='S'):
    """messages: [{'seq', 'frame', 'raw', 'arrivalMs'}] in arrival order.
    reply_delay_ms(seq, method, forwardedAt) -> replyMs, or None (no reply: timeout).
    Replies due at a time t are processed before arrivals at t (P)."""
    bucket = B6.Bucket(created_ms)
    pend = B6.Pending()
    storage = SS.SiteStorage()
    due = []                                    # (replyMs, handle)
    lines = []
    for m in messages:
        t = m['arrivalMs']
        for item in sorted(d for d in due if d[0] <= t):
            due.remove(item)
            if pend.req[item[1]]['state'] == 'live':
                pend.final_reply(item[1])
        before = bucket.tokens_mt
        r = B6.check(m['raw'], t, bucket)
        line = dict.fromkeys(TRACE_FIELDS)
        line.update(frame=m['frame'], seq=m['seq'], id=r['id'], arrivalMs=t, tokensBefore_mt=before, bpre=r['bpre'],
                    stage=r['stage'], code=r['code'], route=r['route'])
        if r['stage'] == 'ok' and r['route'] in ('readClient', 'logClient'):
            line['pendingBefore'] = pend.counts(session)['session']
            o = pend.route(session, m['frame'], r['id'])
            if o.get('accepted'):
                line['forwardedSeq'] = m['seq']
                method = B04.json.loads(m['raw'])['payload']['method']
                rm = reply_delay_ms(m['seq'], method, t)
                rm = t + (LOGS_TIMEOUT_MS if r['route'] == 'logClient' else READ_TIMEOUT_MS) if rm is None else rm
                line['replyMs'] = rm
                due.append((rm, o['handle']))
            else:
                line.update(stage='B4', code=-32005)
        elif r['stage'] == 'ok' and r['route'] == 'siteStorage':
            p = B04.json.loads(m['raw'])['payload']
            line['storeTotalBefore'] = storage.total
            if p['method'] == 'site_storageClear':
                rep = storage.clear()
            else:
                rep = storage.set(p['params'][0], p['params'][1])
            line['storeTotalAfter'] = storage.total
            if 'code' in rep:
                line.update(stage='B4', code=rep['code'])
        lines.append(line)
    return lines


def decisive(lines60, lines5):
    """BR19c decision rule (FD:L2759)."""
    return any(l['bpre'] == 'rate' for l in lines60) and any(l['stage'] == 'B4' and l['code'] == -32005 for l in lines5)
