#!/usr/bin/env python3
"""Rehearsal measure collector (development/public-network/REHEARSAL.md, section 8): T1, T2, T5 and T6.

Python standard library only, so the file alone can be copied to a founder server. It reads the node's loopback
JSON-RPC and the host's clock status; it signs nothing, sends no transaction and reads no key.

  sample  appends one JSON line per sample (every --every seconds) to DIR/measures-LABEL-<startUTC>.jsonl: the
          service's PID and restart count, the node's RSS, the host's clock offset, pocol_finality,
          pocol_templateLatency and a whitelisted part of pocol_status. The status's `peers`, `discovery` and
          `lastIncompatibleReason` are never stored (they name peers' IP addresses, the PC's home address among them).
  blocks  writes one JSON line per canonical block (height, hash, timestamp, difficulty, attempt, signed, proposer,
          shares, gas used, transaction count) after a header line naming the chain.
  merge   reads sample files (and the PC's clock files from Collect-PoColClock.ps1), one blocks file, the drill
          windows, and writes report.json with each measure's value, threshold and pass, miss or not measured. It is
          mechanical: it closes no gate (REHEARSAL.md, sections 6 and 8).
"""
import argparse
import bisect
import datetime
import http.client
import json
import os
import re
import signal
import subprocess
import sys
import time
import urllib.parse
import urllib.request

VERSION = 1
LABEL = re.compile(r'^[A-Za-z0-9_-]{1,32}$')
ADDRESS = re.compile(r'^0x[0-9a-f]{40}$')
RESPONSE_MAX = 4 * 1024 * 1024
# pocol_status fields a sample keeps (public data only). Anything else, `peers` and `discovery` included, is dropped.
STATUS_FIELDS = ('chainId', 'genesisHash', 'consensusIdentity', 'consensusRulesVersion', 'protocolVersion',
                 'compatibilitySha256', 'head', 'miner', 'production', 'compatiblePeers', 'incompatiblePeers',
                 'inboundEstablished', 'outboundPeers', 'stats', 'retention', 'diskObserved', 'resourceDiskBytes',
                 'evictStalls', 'evictDepth', 'canonicalBase', 'journalRecords', 'networkHashrateEst')
IDENTITY = ('chainId', 'genesisHash', 'consensusIdentity')


class Refused(Exception):
    """A refusal before anything is written (exit 3)."""


class RpcError(Exception):
    def __init__(self, code, message):
        super().__init__(f'RPC error {code}: {message}')
        self.code = code


def check_url(url):
    """Only http://127.0.0.1:PORT or http://[::1]:PORT, with no credentials, path, query or fragment."""
    p = urllib.parse.urlsplit(url)
    if p.scheme != 'http' or p.hostname not in ('127.0.0.1', '::1') or p.username or p.password or p.query \
            or p.fragment or p.path not in ('', '/'):
        raise Refused(f'--rpc must be http://127.0.0.1:PORT (the node\'s loopback RPC), not {url!r}')
    try:
        port = p.port
    except ValueError:
        port = None
    if not port:
        raise Refused(f'--rpc needs a port: {url!r}')
    return url


class Rpc:
    """JSON-RPC over loopback. No proxy (an HTTP_PROXY in the environment cannot reroute it) and no redirect. The
    node's busy answer (-32021) is retried after its retryAfterMs."""

    def __init__(self, url, sleep=time.sleep, timeout=10):
        self.url, self.sleep, self.timeout = check_url(url), sleep, timeout

        class NoRedirect(urllib.request.HTTPRedirectHandler):
            def redirect_request(self, *args, **kwargs):
                raise ValueError('RPC redirect refused')
        self.opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())

    def call(self, method, params=()):
        body = json.dumps({'jsonrpc': '2.0', 'id': 1, 'method': method, 'params': list(params)}).encode()
        for _ in range(40):
            request = urllib.request.Request(self.url, body, {'Content-Type': 'application/json'})
            with self.opener.open(request, timeout=self.timeout) as response:
                raw = response.read(RESPONSE_MAX + 1)
            if len(raw) > RESPONSE_MAX:
                raise ValueError('oversized RPC response')
            value = json.loads(raw)
            if 'error' in value:
                e = value['error'] or {}
                if e.get('code') == -32021:
                    self.sleep(((e.get('data') or {}).get('retryAfterMs', 250)) / 1000)
                    continue
                raise RpcError(e.get('code'), e.get('message', ''))
            if 'result' not in value:
                raise ValueError('RPC answer without a result')
            return value['result']
        raise RpcError(-32021, f'{method}: the node stayed busy')


def utc(ms):
    return datetime.datetime.fromtimestamp(ms / 1000, datetime.timezone.utc).strftime('%Y-%m-%dT%H:%M:%S.%f')[:-3] + 'Z'


def run(cmd):
    """A local command's stdout, or None when it is missing or fails."""
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=10)
    except (OSError, subprocess.SubprocessError):
        return None
    return r.stdout if r.returncode == 0 else None


DURATION = re.compile(r'^([+-]?\d+(?:\.\d+)?)(ns|us|µs|ms|s|min)$')


def parse_duration_ms(text):
    """A systemd duration such as +1.123ms, -512us or 2.5s, in milliseconds."""
    m = DURATION.match(text.strip())
    if not m:
        return None
    scale = {'ns': 1e-6, 'us': 1e-3, 'µs': 1e-3, 'ms': 1.0, 's': 1000.0, 'min': 60000.0}[m.group(2)]
    return float(m.group(1)) * scale


def parse_chrony(text):
    """`chronyc -c tracking`: reference name (field 2), stratum (3) and the system time offset (5, seconds)."""
    line = (text or '').strip().splitlines()
    if not line:
        return None
    f = line[0].split(',')
    if len(f) < 6:
        return None
    try:
        return {'source': 'chrony', 'reference': f[1], 'stratum': int(f[2]), 'offsetMs': float(f[4]) * 1000}
    except ValueError:
        return None


def parse_timesyncd(text):
    """`timedatectl timesync-status`: its Server, Stratum and Offset lines."""
    fields = {}
    for line in (text or '').splitlines():
        k, sep, v = line.partition(':')
        if sep:
            fields[k.strip()] = v.strip()
    offset = parse_duration_ms(fields.get('Offset', ''))
    if offset is None:
        return None
    stratum = fields.get('Stratum', '')
    return {'source': 'timesyncd', 'reference': fields.get('Server'), 'stratum': int(stratum) if stratum.isdigit() else None,
            'offsetMs': offset}


def read_clock(runner=run):
    """This host's offset from its own NTP source: chrony, else systemd-timesyncd, else none. Each host can only measure
    itself against its own source, so two hosts' offsets bound their difference only when their sources agree."""
    for cmd, parse in ((['chronyc', '-c', 'tracking'], parse_chrony), (['timedatectl', 'timesync-status'], parse_timesyncd)):
        c = parse(runner(cmd))
        if c is not None:
            return c
    return {'source': 'none', 'reference': None, 'stratum': None, 'offsetMs': None}


def read_service(unit, runner=run):
    out = runner(['systemctl', 'show', '-p', 'MainPID', '-p', 'NRestarts', '-p', 'ExecMainStartTimestamp', unit])
    if out is None:
        return None
    kv = dict(line.split('=', 1) for line in out.splitlines() if '=' in line)
    as_int = lambda v: int(v) if (v or '').isdigit() else None
    return {'mainPid': as_int(kv.get('MainPID')), 'nRestarts': as_int(kv.get('NRestarts')), 'startTs': kv.get('ExecMainStartTimestamp')}


def read_rss_kb(pid, proc='/proc'):
    if not pid:
        return None
    try:
        with open(os.path.join(proc, str(pid), 'status'), encoding='utf-8') as f:
            for line in f:
                if line.startswith('VmRSS:'):
                    return int(line.split()[1])
    except (OSError, ValueError, IndexError):
        return None
    return None


def status_subset(status):
    out = {k: status[k] for k in STATUS_FIELDS if k in status}
    a13 = status.get('a13')
    if isinstance(a13, dict):
        out['a13'] = {k: a13[k] for k in ('boundBytes', 'budgetBytes') if k in a13}
    mining = status.get('mining')
    if isinstance(mining, dict):
        out['mining'] = {k: mining[k] for k in ('threads', 'running', 'cores') if k in mining}
    return out


def take_sample(rpc, label, unit, runner=run, now_ms=lambda: int(time.time() * 1000), proc='/proc'):
    t = now_ms()
    line = {'v': VERSION, 'kind': 'sample', 'label': label, 'utc': utc(t), 'unixMs': t}
    service = read_service(unit, runner)
    line['service'] = service
    line['rssKb'] = read_rss_kb(service and service.get('mainPid'), proc)
    line['clock'] = read_clock(runner)
    try:
        line['status'] = status_subset(rpc.call('pocol_status'))
        line['finality'] = rpc.call('pocol_finality')
        line['latency'] = rpc.call('pocol_templateLatency')
    except (OSError, ValueError, RpcError, http.client.HTTPException) as e:
        line = {'v': VERSION, 'kind': 'error', 'label': label, 'utc': line['utc'], 'unixMs': t,
                'service': service, 'clock': line['clock'], 'error': f'{type(e).__name__}: {e}'}
    return line


def append(f, line):
    f.write(json.dumps(line, sort_keys=True) + '\n')
    f.flush()
    os.fsync(f.fileno())


STOP = []


def cmd_sample(a, runner=run, sleep=time.sleep):
    if not LABEL.match(a.label):
        raise Refused('--label is 1 to 32 letters, digits, _ or -')
    if a.every < 1:
        raise Refused('--every is at least 1 second')
    rpc = Rpc(a.rpc)
    if a.expect_chain_id is not None:
        chain = rpc.call('pocol_status').get('chainId')
        if chain != a.expect_chain_id:
            raise Refused(f'the node runs chain id {chain}, not --expect-chain-id {a.expect_chain_id}')
    os.makedirs(a.out, exist_ok=True)
    start = int(time.time() * 1000)
    stamp = datetime.datetime.fromtimestamp(start / 1000, datetime.timezone.utc).strftime('%Y%m%dT%H%M%SZ')
    path = os.path.join(a.out, f'measures-{a.label}-{stamp}.jsonl')
    fd = os.open(path, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_APPEND, 0o644)
    signal.signal(signal.SIGTERM, lambda *_: STOP.append(1))
    n = 0
    with os.fdopen(fd, 'w', encoding='utf-8') as f:
        print(path, flush=True)
        try:
            while not STOP:
                t0 = time.monotonic()
                append(f, take_sample(rpc, a.label, a.unit, runner))
                n += 1
                if a.once or (a.count and n >= a.count):
                    break
                while not STOP and time.monotonic() - t0 < a.every:
                    sleep(min(1.0, a.every - (time.monotonic() - t0)))
        except KeyboardInterrupt:
            pass
    return 0


def block_line(b):
    p = b.get('pocol') or {}
    return {'height': int(b['number'], 16), 'hash': b['hash'], 'parentHash': b['parentHash'], 'timestamp': int(b['timestamp'], 16),
            'difficulty': str(int(b['difficulty'], 16)), 'attempt': p.get('attempt'), 'signed': p.get('signed'),
            'proposer': p.get('proposer'), 'shares': p.get('shares'), 'gasUsed': int(b['gasUsed'], 16),
            'txCount': len(b.get('transactions') or [])}


def cmd_blocks(a):
    rpc = Rpc(a.rpc)
    status = rpc.call('pocol_status')
    if a.expect_chain_id is not None and status.get('chainId') != a.expect_chain_id:
        raise Refused(f'the node runs chain id {status.get("chainId")}, not --expect-chain-id {a.expect_chain_id}')
    head = status['head']['height']
    to = head if a.to == 'latest' else int(a.to)
    if not 0 <= a.from_ <= to <= head:
        raise Refused(f'--from {a.from_} --to {a.to}: the head is {head}')
    fd = os.open(a.out, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
    with os.fdopen(fd, 'w', encoding='utf-8') as f:
        f.write(json.dumps({'v': VERSION, 'kind': 'chain', **{k: status.get(k) for k in IDENTITY}, 'fromHeight': a.from_,
                            'toHeight': to}, sort_keys=True) + '\n')
        for h in range(a.from_, to + 1):
            b = rpc.call('eth_getBlockByNumber', [hex(h), False])
            if b is None:
                raise Refused(f'block {h} is not served')
            f.write(json.dumps(block_line(b), sort_keys=True) + '\n')
    return 0


# ---- merge ---------------------------------------------------------------------------------------------------------

def read_jsonl(path):
    with open(path, encoding='utf-8') as f:
        return [json.loads(line) for line in f if line.strip()]


def parse_time(v):
    """Unix seconds (a number) or an ISO 8601 UTC time ending in Z, in milliseconds."""
    if isinstance(v, (int, float)) and not isinstance(v, bool):
        return int(v * 1000)
    if isinstance(v, str) and v.endswith('Z'):
        return int(datetime.datetime.fromisoformat(v[:-1]).replace(tzinfo=datetime.timezone.utc).timestamp() * 1000)
    raise Refused(f'a window time is unix seconds or an ISO 8601 UTC time ending in Z, not {v!r}')


def read_windows(path):
    if not path:
        return []
    with open(path, encoding='utf-8') as f:
        data = json.load(f)
    out = []
    for w in data.get('windows', []):
        s, e = parse_time(w['start']), parse_time(w['end'])
        if e < s:
            raise Refused(f'window {w.get("name")!r} ends before it starts')
        out.append({'name': w.get('name', ''), 'startMs': s, 'endMs': e})
    return out


def dig(d, *path):
    for k in path:
        if not isinstance(d, dict) or k not in d:
            return None
        d = d[k]
    return d


COUNTERS = (('finality', 'reorgs'), ('finality', 'reorgsGeKAlarm'), ('latency', 'all', 'slots'), ('status', 'stats', 'imported'))


def segments(samples):
    """A server's samples, in time order, cut wherever the service's PID or restart count changes or a counter that
    only grows since the node's start goes down (a restart the service did not see)."""
    out, cur = [], []
    for s in sorted(samples, key=lambda s: s['unixMs']):
        if cur:
            p = cur[-1]
            pid = lambda x: (dig(x, 'service', 'mainPid'), dig(x, 'service', 'nRestarts'))
            dropped = any((dig(s, *c) or 0) < (dig(p, *c) or 0) for c in COUNTERS)
            if (pid(s) != pid(p) and None not in pid(s) + pid(p)) or dropped:
                out.append(cur)
                cur = []
        cur.append(s)
    if cur:
        out.append(cur)
    return out


def verdict(ok):
    return 'pass' if ok else 'miss'


def t1(servers, names, declared):
    """T1 per ordered pair (receiver server <- proposer): late 0, sinceParentMs p99 <= 1,000 ms (Δ1) or 2,000 ms (Δ2),
    max < Δ_rel - 2 s. Counters are cumulative since the node's start, so each segment's last sample is its total."""
    p99_limit = 1000 if declared == 'delta1' else 2000
    pairs = []
    for label, segs in sorted(servers.items()):
        fallbacks = sum((dig(seg[-1], 'status', 'stats', 'fallbackTemplates') or 0) - (dig(seg[0], 'status', 'stats', 'fallbackTemplates') or 0) for seg in segs)
        found = {}
        for seg in segs:
            last = seg[-1]['latency']
            delta_rel = dig(last, 'local', 'deltaRel')
            for p in last.get('proposers', []):
                a = found.setdefault(p['proposer'], {'late': 0, 'received': 0, 'missing': 0, 'early': 0, 'slots': 0, 'p99': None, 'max': None,
                                                     'deltaRel': delta_rel, 'segments': 0})
                for k in ('late', 'received', 'missing', 'early', 'slots'):
                    a[k] += p.get(k, 0)
                a['segments'] += 1
                for k, src in (('p99', 'p99'), ('max', 'max')):
                    v = dig(p, 'sinceParentMs', src)
                    if v is not None:
                        a[k] = v if a[k] is None else max(a[k], v)
        for proposer, a in sorted(found.items()):
            max_limit = None if a['deltaRel'] is None else (a['deltaRel'] - 2) * 1000
            row = {'receiver': label, 'proposer': names.get(proposer, proposer), 'proposerAddress': proposer, **a,
                   'fallbackTemplatesAtReceiver': fallbacks,
                   'thresholds': {'late': 0, 'p99MsAtMost': p99_limit, 'maxMsBelow': max_limit}}
            if a['received'] == 0:
                row['verdict'] = 'not measured'
            else:
                row['verdict'] = verdict(a['late'] == 0 and a['p99'] is not None and a['p99'] <= p99_limit
                                         and max_limit is not None and a['max'] is not None and a['max'] < max_limit)
            pairs.append(row)
    observed = set(servers)
    not_observable = [{'receiver': name, 'reason': 'no RPC on that device (the PC runs the wallet engine over stdio)'}
                      for name in sorted(set(names.values()) - observed)]
    return {'declared': declared, 'pairs': pairs, 'notObservable': not_observable,
            'note': 'p99 is the upper bound of its bucket (never below the exact value), so a pass is sound and a miss may be '
                    'up to one bucket above the true value. Drill windows cannot be left out of these cumulative counters.'}


def t2(servers, clocks, align_ms):
    """T2: pairwise clock offsets <= 500 ms, graded on the sign-free bound |o_i| + |o_j| of samples taken within
    `align_ms` of each other; and no timed template rejected as future (beforeStampMs.max <= Φ·1000, Φ = 2φ)."""
    labels = sorted(clocks)
    pairs = []
    for i, a in enumerate(labels):
        for b in labels[i + 1:]:
            worst, n = None, 0
            sb = sorted(clocks[b])
            times = [x[0] for x in sb]
            for ta, oa in clocks[a]:
                i = bisect.bisect_left(times, ta)
                near = min(sb[max(i - 1, 0):i + 1], key=lambda x: abs(x[0] - ta), default=None)
                if near is None or abs(near[0] - ta) > align_ms:
                    continue
                bound = abs(oa) + abs(near[1])
                worst = bound if worst is None else max(worst, bound)
                n += 1
            pairs.append({'pair': [a, b], 'alignedSamples': n, 'worstBoundMs': worst, 'thresholdMs': 500,
                          'verdict': 'not measured' if worst is None else verdict(worst <= 500)})
    future = []
    for label, segs in sorted(servers.items()):
        for k, seg in enumerate(segs):
            last = seg[-1]['latency']
            phi = dig(last, 'local', 'phi')
            mx = dig(last, 'all', 'beforeStampMs', 'max')
            limit = None if phi is None else 2 * phi * 1000
            if mx is None:
                v = 'not measured'
            elif limit is not None and mx <= limit:
                v = 'no timed template rejected as future'
            else:
                v = 'cannot exclude'
            future.append({'receiver': label, 'segment': k, 'beforeStampMaxMs': mx, 'limitMs': limit, 'verdict': v})
    return {'clockPairs': pairs, 'futureTemplates': future,
            'notMeasured': ['future-queued blocks (not exposed by the node)', 'templates that were never timed'],
            'note': 'Each host measures its offset against its own NTP source; the bound |o_i| + |o_j| bounds the hosts\' '
                    'difference only when their sources agree. Hosts without clock data are left out.'}


def overlaps(a, b, windows):
    return any(w['startMs'] <= b and a <= w['endMs'] for w in windows)


def t5(servers, windows, every_ms):
    """T5 in intervals (REHEARSAL.md section 2): outside the drill windows, an interval passes when its reorgsGeKAlarm
    increase is 0 and, if reorgs rose, maxReorgDepth at its end is at most 2 (below kSafe)."""
    intervals, gaps = [], []
    for label, segs in sorted(servers.items()):
        for k, seg in enumerate(segs):
            if k:
                prev = segs[k - 1][-1]
                planned = overlaps(prev['unixMs'], seg[0]['unixMs'], windows)
                gaps.append({'node': label, 'fromUtc': prev['utc'], 'toUtc': seg[0]['utc'], 'seconds': (seg[0]['unixMs'] - prev['unixMs']) / 1000,
                             'restart': 'in a drill window' if planned else 'unplanned (outside every window)',
                             'note': 'reorganizations between the last sample before the restart and the restart are not measured '
                                     f'(up to one sampling interval, about {every_ms // 1000} s)'})
            run_ = [seg[0]]
            for s in seg[1:]:
                if overlaps(run_[-1]['unixMs'], s['unixMs'], windows):
                    if len(run_) > 1:
                        intervals.append((label, k, run_))
                    run_ = [s]
                else:
                    run_.append(s)
            if len(run_) > 1:
                intervals.append((label, k, run_))
    rows = []
    for label, k, r in intervals:
        a, b = r[0], r[-1]
        d = lambda *p: (dig(b, *p) or 0) - (dig(a, *p) or 0)
        alarm, reorgs, depth, k_safe = d('finality', 'reorgsGeKAlarm'), d('finality', 'reorgs'), dig(b, 'finality', 'maxReorgDepth'), dig(b, 'finality', 'kSafe')
        ok = alarm == 0 and (reorgs == 0 or (depth is not None and depth <= 2))
        heads = d('status', 'head', 'height')
        siblings = None if heads <= 0 else (d('status', 'stats', 'imported') - d('status', 'stats', 'refetched') - heads) / heads
        rows.append({'node': label, 'segment': k, 'fromUtc': a['utc'], 'toUtc': b['utc'], 'reorgsGeKAlarmIncrease': alarm, 'reorgsIncrease': reorgs,
                     'maxReorgDepthAtEnd': depth, 'kSafe': k_safe, 'verdict': verdict(ok),
                     'siblingEstimate': siblings,
                     'siblingVerdict': None if siblings is None else ('estimate pass' if siblings <= 0.02 else 'estimate miss')})
    return {'intervals': rows, 'gaps': gaps, 'thresholds': {'reorgsGeKAlarmIncrease': 0, 'maxReorgDepthAtMost': 2, 'siblingsAtMost': 0.02},
            'note': 'siblings are estimated as (Δimported - Δrefetched - Δhead) / Δhead per interval; the node has no sibling '
                    'counter, so the estimate is reported, not graded as T5.'}


def t6(blocks, windows, step_height):
    """T6 from the blocks file: the 24 h mean interval (10 s ± 5 %) per full day from genesis, the first 100 blocks'
    mean (5-20 s), and the difficulty against its trailing 60-block mean (τ / T_blk), whose grading is to confirm."""
    by_h = {b['height']: b for b in blocks}
    out = {}
    if 0 in by_h and 100 in by_h:
        m = (by_h[100]['timestamp'] - by_h[0]['timestamp']) / 100
        out['first100MeanSeconds'] = {'value': m, 'threshold': [5, 20], 'verdict': verdict(5 <= m <= 20)}
    else:
        out['first100MeanSeconds'] = {'verdict': 'not measured'}
    days = []
    if 0 in by_h:
        g = by_h[0]['timestamp']
        ordered = sorted(blocks, key=lambda b: b['height'])
        last_ts = ordered[-1]['timestamp']
        day = 0
        while g + (day + 1) * 86400 <= last_ts:
            lo, hi = g + day * 86400, g + (day + 1) * 86400
            inside = [b for b in ordered if lo <= b['timestamp'] < hi]
            if len(inside) > 1:
                m = (inside[-1]['timestamp'] - inside[0]['timestamp']) / (len(inside) - 1)
                days.append({'day': day, 'blocks': len(inside), 'meanSeconds': m, 'threshold': [9.5, 10.5], 'verdict': verdict(9.5 <= m <= 10.5)})
            day += 1
    out['dayMeans'] = days or [{'verdict': 'not measured (less than 24 h of blocks)'}]
    ordered = sorted(blocks, key=lambda b: b['height'])
    diffs = [int(b['difficulty']) for b in ordered]
    noise = None
    for i in range(60, len(diffs)):
        mean = sum(diffs[i - 60:i]) / 60
        if mean:
            dev = abs(diffs[i] / mean - 1)
            noise = dev if noise is None else max(noise, dev)
    reconv = None
    if step_height is not None:
        for b_i in range(len(ordered)):
            h = ordered[b_i]['height']
            if h <= step_height or b_i < 60:
                continue
            mean = sum(diffs[b_i - 60:b_i]) / 60
            if mean and all(abs(diffs[j] / mean - 1) <= 0.05 for j in range(b_i, min(b_i + 10, len(diffs)))):
                reconv = h - step_height
                break
    out['difficulty'] = {'maxDeviationFromTrailing60Mean': noise, 'blocksToWithin5PercentAfterStep': reconv,
                         'verdict': 'definition to confirm',
                         'note': '"difficulty noise" and "re-converges" are not defined in this repository (design §4b is kept '
                                 'outside it); reported, not graded.'}
    outside = [b for b in ordered if b['height'] > 0 and not any(w['startMs'] <= b['timestamp'] * 1000 <= w['endMs'] for w in windows)]
    out['t4Raw'] = {'blocksOutsideWindows': len(outside), 'attempt0Empty': sum(1 for b in outside if b['attempt'] == 0 and b['txCount'] == 0),
                    'attemptAtLeast1': sum(1 for b in outside if (b['attempt'] or 0) >= 1), 'unsigned': sum(1 for b in outside if b['signed'] is False),
                    'verdict': 'raw counts, definition to confirm'}
    return out


def merge(paths, blocks_path, windows_path, declared, addresses, step_height, align_ms=120_000):
    samples, clocks, servers = {}, {}, {}
    identity = None

    def same(src, where):
        nonlocal identity
        got = {k: src.get(k) for k in IDENTITY}
        if None in got.values():
            return
        if identity is None:
            identity = got
        elif got != identity:
            raise Refused(f'{where} names another chain: {got} != {identity}')

    every_ms = 0
    for path in paths:
        for line in read_jsonl(path):
            label = line.get('label')
            if not isinstance(label, str) or not LABEL.match(label):
                raise Refused(f'{path}: a line without a valid label')
            off = dig(line, 'clock', 'offsetMs')
            if off is not None:
                clocks.setdefault(label, []).append((line['unixMs'], off))
            if line.get('kind') == 'sample':
                same(line['status'], path)
                samples.setdefault(label, []).append(line)
    for label, s in samples.items():
        servers[label] = segments(s)
        times = sorted(x['unixMs'] for x in s)
        steps = sorted(b - a for a, b in zip(times, times[1:]))
        if steps:
            every_ms = max(every_ms, steps[len(steps) // 2])
    names = {}
    for spec in addresses:
        label, sep, addr = spec.partition('=')
        if not sep or not LABEL.match(label) or not ADDRESS.match(addr):
            raise Refused(f'--address LABEL=0x... (40 lower-case hex digits), not {spec!r}')
        names[addr] = label
    for label, segs in servers.items():
        miner = dig(segs[-1][-1], 'status', 'miner')
        if miner:
            names.setdefault(miner, label)
    blocks = []
    if blocks_path:
        lines = read_jsonl(blocks_path)
        if not lines or lines[0].get('kind') != 'chain':
            raise Refused(f'{blocks_path}: not a blocks file (no chain header)')
        same(lines[0], blocks_path)
        blocks = lines[1:]
    windows = read_windows(windows_path)
    return {'v': VERSION, 'identity': identity, 'inputs': {'samples': list(paths), 'blocks': blocks_path, 'windows': windows},
            'labels': {'servers': sorted(servers), 'clocks': sorted(clocks), 'addresses': {v: k for k, v in names.items()}},
            'T1': t1(servers, names, declared), 'T2': t2(servers, clocks, align_ms), 'T5': t5(servers, windows, every_ms),
            'T6': t6(blocks, windows, step_height) if blocks else {'verdict': 'not measured (no --blocks file)'},
            'note': 'Mechanical: each measure with its value, threshold and verdict. It closes no gate (REHEARSAL.md, sections 6 '
                    'and 8); a reviewer reads it against section 2.'}


def cmd_merge(a):
    report = merge(a.inputs, a.blocks, a.windows, a.declared, a.address or [], a.step_height)
    fd = os.open(a.out, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
    with os.fdopen(fd, 'w', encoding='utf-8') as f:
        json.dump(report, f, indent=1, sort_keys=True)
        f.write('\n')
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest='cmd', required=True)
    s = sub.add_parser('sample', help='append samples of one node to a JSON-lines file')
    s.add_argument('--rpc', required=True)
    s.add_argument('--label', required=True)
    s.add_argument('--out', required=True, help='directory for measures-LABEL-<startUTC>.jsonl')
    s.add_argument('--every', type=int, default=60, help='seconds between samples (default 60)')
    s.add_argument('--expect-chain-id', type=int)
    s.add_argument('--unit', default='pocol-standalone', help='the node\'s systemd unit (default pocol-standalone)')
    s.add_argument('--once', action='store_true', help='one sample, then exit')
    s.add_argument('--count', type=int, default=0, help=argparse.SUPPRESS)
    b = sub.add_parser('blocks', help='write the canonical blocks of one node')
    b.add_argument('--rpc', required=True)
    b.add_argument('--out', required=True, help='a new file')
    b.add_argument('--from', dest='from_', type=int, default=0)
    b.add_argument('--to', default='latest')
    b.add_argument('--expect-chain-id', type=int)
    m = sub.add_parser('merge', help='grade T1, T2, T5 and T6 from the collected files')
    m.add_argument('--in', dest='inputs', nargs='+', required=True, help='sample files and the PC\'s clock files')
    m.add_argument('--blocks', help='the blocks file')
    m.add_argument('--windows', help='windows.json: {"windows":[{"name":..,"start":..,"end":..}]} (unix seconds or ISO UTC with Z)')
    m.add_argument('--declared', choices=('delta1', 'delta2'), default='delta1')
    m.add_argument('--address', action='append', help='LABEL=0x... names a founder\'s miner address (the PC\'s, for example)')
    m.add_argument('--step-height', type=int)
    m.add_argument('--out', required=True, help='a new report file')
    a = ap.parse_args(argv)
    try:
        return {'sample': cmd_sample, 'blocks': cmd_blocks, 'merge': cmd_merge}[a.cmd](a)
    except Refused as e:
        print(f'refused: {e}', file=sys.stderr)
        return 3
    except FileExistsError as e:
        print(f'refused: {e.filename} exists; an existing file is never replaced', file=sys.stderr)
        return 3
    except (OSError, ValueError, RpcError) as e:
        print(f'error: {type(e).__name__}: {e}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
