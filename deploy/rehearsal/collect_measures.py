#!/usr/bin/env python3
"""Launch-evidence measure collector (development/public-network/REHEARSAL.md, section 8): T1, T2, T5 and T6, on PoCol
Mainnet after the launch or on a devnet.

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
import fractions
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


def cmd_blocks(a, now_ms=lambda: int(time.time() * 1000)):
    rpc = Rpc(a.rpc)
    collected = now_ms()  # taken before the read, so no block after the head below was made before it
    status = rpc.call('pocol_status')
    if a.expect_chain_id is not None and status.get('chainId') != a.expect_chain_id:
        raise Refused(f'the node runs chain id {status.get("chainId")}, not --expect-chain-id {a.expect_chain_id}')
    head = status['head']['height']
    to = head if a.to == 'latest' else int(a.to)
    if not 0 <= a.from_ <= to <= head:
        raise Refused(f'--from {a.from_} --to {a.to}: the head is {head}')
    header = {'v': VERSION, 'kind': 'chain', **{k: status.get(k) for k in IDENTITY}, 'fromHeight': a.from_,
              'toHeight': to, 'headHeight': head, 'headHash': status['head'].get('hash'), 'collectedUnixMs': collected}
    fd = os.open(a.out, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o644)
    try:
        with os.fdopen(fd, 'w', encoding='utf-8') as f:
            f.write(json.dumps(header, sort_keys=True) + '\n')
            prev = None
            for h in range(a.from_, to + 1):
                b = rpc.call('eth_getBlockByNumber', [hex(h), False])
                if b is None:
                    raise Refused(f'block {h} is not served')
                line = block_line(b)
                if prev is not None and line['parentHash'] != prev['hash']:
                    raise Refused(f'block {h} does not extend block {h - 1} read before it (the chain reorganized during '
                                  'the export); run it again')
                f.write(json.dumps(line, sort_keys=True) + '\n')
                prev = line
            check_chain(header, [], prev)
    except BaseException:
        os.unlink(a.out)
        raise
    return 0


def check_chain(header, blocks, last=None):
    """Refuses a blocks file that is not one chain: heights from fromHeight to toHeight with no gap, each block's
    parentHash the hash of the block before it, and, when the file was written up to the head, its last block the head
    the header recorded. A reorganization during an export breaks one of these."""
    if blocks:
        want = list(range(header.get('fromHeight', blocks[0]['height']), header.get('toHeight', blocks[-1]['height']) + 1))
        if [b['height'] for b in blocks] != want:
            raise Refused('the blocks file does not hold each height from fromHeight to toHeight once, in order')
        for p, b in zip(blocks, blocks[1:]):
            if b.get('parentHash') is None or b['parentHash'] != p.get('hash'):
                raise Refused(f'block {b["height"]} does not extend block {p["height"]} (the chain reorganized during '
                              'the export); export the blocks again')
        last = blocks[-1]
    if (last is not None and header.get('headHash') is not None and header.get('headHeight') == last['height']
            and last.get('hash') != header['headHash']):
        raise Refused(f'block {last["height"]} is not the head {header["headHash"]} the header recorded (the chain '
                      'reorganized during the export); export the blocks again')


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


# A proposer's counters that only grow while the node keeps its entry (node/latency.rs, Stats). `missing` is not one:
# a template can arrive after its slot was counted. A decrease means the entry was evicted and created again.
GROWING = (('slots',), ('received',), ('late',), ('early',), ('stampAtWindow',), ('locked',), ('sinceParentMs', 'count'),
           ('sinceStampMs', 'count'), ('beforeStampMs', 'count'), ('lockMs', 'count'), ('sinceParentMs', 'max'),
           ('sinceStampMs', 'max'), ('beforeStampMs', 'max'), ('lockMs', 'max'))
TRACKED_MAX = 256  # latency.rs PROPOSERS_MAX, when the answer has no trackedMax


def proposer_epochs(seg):
    """Each proposer's entries in one segment's samples, cut into epochs. The node keeps at most trackedMax proposers
    and evicts the least recently drawn; one drawn (or timed) again starts from zero. So an epoch ends when the proposer
    is missing from a sample, or when a counter that only grows went down; its last sample is that epoch's total.

    An entry can also be evicted and created again between two samples with every counter at least as high as before.
    Slots are numbered by the aggregate's `slots`, which counts every slot and is never evicted. That needs:
    - an eviction no visible departure accounts for: each eviction removes one entry, and every proposer listed in the
      earlier sample but not in the later one was removed at least once, so `evicted` must have risen by more than
      their number (any rise counts when either listing is incomplete; an unknown count, an older node, always does);
    - trackedMax - 1 + n slots after the proposer's last known draw, where n is its slots in the later sample: the
      proposer is evicted only as the least recently drawn of trackedMax entries (each other entry drawn later), and its
      new entry starts from zero, so its n draws all came after the eviction. The last known draw is at least the
      earlier sample's total plus the draws added since it (or, in a segment's first sample, the entry's own draws).
      An entry never drawn (made by a template or lock alone) is evicted first, at any eviction.
    Then the counts are read as one epoch (late, received and max are right either way: a reset with no decrease leaves
    the new epoch at least as high), and the earlier sample's p99 is kept too, since a p99 is not additive. Returns
    {proposer: {'epochs': [final entries], 'prefixes': [entries before a possible reset], 'max', 'possibleResets'}}."""
    tracks = {}
    prev, prev_present, prev_complete = None, {}, False
    for s in seg:
        lat = s.get('latency') or {}
        total = dig(lat, 'all', 'slots') or 0
        prev_total = (dig(prev, 'all', 'slots') or 0) if prev is not None else 0
        cap = lat.get('trackedMax') or TRACKED_MAX
        listed = lat.get('proposers') or []
        present = {p['proposer']: p for p in listed if isinstance(p, dict) and isinstance(p.get('proposer'), str)}
        complete = isinstance(lat.get('tracked'), int) and lat['tracked'] == len(listed) == len(present)
        evicted, prev_evicted = lat.get('evicted'), (prev or {}).get('evicted')
        if evicted is None or prev_evicted is None:
            eviction = True
        else:
            departed = sum(1 for a in prev_present if a not in present) if complete and prev_complete else 0
            eviction = evicted - prev_evicted > departed
        for addr, t in tracks.items():
            if t['cur'] is not None and addr not in present:
                t['epochs'].append(t['cur'])
                t['cur'] = None
        for addr, p in present.items():
            t = tracks.setdefault(addr, {'epochs': [], 'cur': None, 'drawn': 0, 'prefixes': [], 'max': None, 'possibleResets': 0})
            slots = p.get('slots') or 0
            cur = t['cur']
            grows = lambda path: (dig(p, *path) if dig(p, *path) is not None else -1) >= (dig(cur, *path) if dig(cur, *path) is not None else -1)
            if cur is not None and not all(grows(path) for path in GROWING):
                t['epochs'].append(cur)
                cur = None
            if cur is None:
                # A new entry: its draws (if any) came after the previous sample, each on its own slot.
                t['drawn'] = prev_total + slots if slots > 0 else 0
            else:
                if eviction and (t['drawn'] == 0 or total - t['drawn'] >= cap - 1 + slots):
                    t['prefixes'].append(cur)
                    t['possibleResets'] += 1
                added = slots - (cur.get('slots') or 0)
                if added > 0:
                    t['drawn'] = max(t['drawn'], prev_total + added)
            t['cur'] = p
            mx = dig(p, 'sinceParentMs', 'max')
            if mx is not None:
                t['max'] = mx if t['max'] is None else max(t['max'], mx)
        prev, prev_present, prev_complete = lat, present, complete
    out = {}
    for addr, t in tracks.items():
        epochs = t['epochs'] + ([t['cur']] if t['cur'] is not None else [])
        out[addr] = {'epochs': epochs, 'prefixes': t['prefixes'], 'max': t['max'], 'possibleResets': t['possibleResets']}
    return out


def p99_verdict(definite, possible, limit):
    """A pair's p99 from its pieces: each definite epoch's last entry, and each entry seen just before a possible reset
    (whose templates the later entries then do not hold). Its templates are the definite entries' and some of the
    possible ones', disjoint. `limit` is a bucket bound (latency.rs BOUNDS_MS holds 1,000 and 2,000), so a piece whose
    p99 is above it has more than floor(n/100) of its n templates above it, and none at or below it has more.
    Returns (the highest p99, 'pass' | 'miss' | 'unknown'): pass when no piece's p99 is above the limit; miss when the
    definite pieces' templates above it are more than 1 % of every possible total; else unknown (the epochs' p99s do
    not add up to the whole's)."""
    def piece(e):
        n = dig(e, 'sinceParentMs', 'count')
        return dig(e, 'sinceParentMs', 'p99'), n if isinstance(n, int) else e.get('received', 0)
    defs = [piece(e) for e in definite]
    poss = [piece(e) for e in possible]
    p99s = [v for v, _ in defs + poss if v is not None]
    if not p99s:
        return None, 'unknown'
    top = max(p99s)
    if top <= limit:
        return top, 'pass'
    above = sum(n // 100 + 1 for v, n in defs if v is not None and v > limit and n > 0)
    most = sum(n for _, n in defs + poss)
    return top, ('miss' if above >= most // 100 + 1 else 'unknown')


def t1(servers, names, declared):
    """T1 per ordered pair (receiver server <- proposer): late 0, sinceParentMs p99 <= 1,000 ms (Δ1) or 2,000 ms (Δ2),
    max < Δ_rel - 2 s. Counters are cumulative since the node's start, so a segment is cut at each restart, and within
    it a proposer's entry into epochs at each eviction (proposer_epochs); the epochs' totals are added, the worst max
    kept, and the p99 graded by p99_verdict. The aggregate (`all`, never evicted) checks that no template was left
    unattributed."""
    p99_limit = 1000 if declared == 'delta1' else 2000
    pairs, receivers = [], []
    for label, segs in sorted(servers.items()):
        fallbacks = sum((dig(seg[-1], 'status', 'stats', 'fallbackTemplates') or 0) - (dig(seg[0], 'status', 'stats', 'fallbackTemplates') or 0) for seg in segs)
        found, parts = {}, {}
        agg = {'late': 0, 'received': 0, 'max': None}
        delta_rel = None
        for seg in segs:
            last = seg[-1]['latency']
            delta_rel = dig(last, 'local', 'deltaRel')
            for k in ('late', 'received'):
                agg[k] += dig(last, 'all', k) or 0
            for s in seg:
                v = dig(s, 'latency', 'all', 'sinceParentMs', 'max')
                if v is not None:
                    agg['max'] = v if agg['max'] is None else max(agg['max'], v)
            for proposer, e in proposer_epochs(seg).items():
                a = found.setdefault(proposer, {'late': 0, 'received': 0, 'missing': 0, 'early': 0, 'slots': 0, 'p99': None, 'max': None,
                                                'deltaRel': delta_rel, 'segments': 0, 'epochs': 0, 'possibleResets': 0})
                for p in e['epochs']:
                    for k in ('late', 'received', 'missing', 'early', 'slots'):
                        a[k] += p.get(k, 0)
                a['segments'] += 1
                a['epochs'] += len(e['epochs'])
                a['possibleResets'] += e['possibleResets']
                if e['max'] is not None:
                    a['max'] = e['max'] if a['max'] is None else max(a['max'], e['max'])
                pp = parts.setdefault(proposer, {'definite': [], 'possible': []})
                pp['definite'] += e['epochs']
                pp['possible'] += e['prefixes']
        attributed_max = None
        for proposer, a in sorted(found.items()):
            max_limit = None if a['deltaRel'] is None else (a['deltaRel'] - 2) * 1000
            a['p99'], p99_state = p99_verdict(parts[proposer]['definite'], parts[proposer]['possible'], p99_limit)
            row = {'receiver': label, 'proposer': names.get(proposer, proposer), 'proposerAddress': proposer, **a,
                   'fallbackTemplatesAtReceiver': fallbacks,
                   'thresholds': {'late': 0, 'p99MsAtMost': p99_limit, 'maxMsBelow': max_limit}}
            if a['received'] == 0:
                row['verdict'] = 'not measured'
            elif a['late'] != 0 or max_limit is None or a['max'] is None or a['max'] >= max_limit or p99_state == 'miss':
                row['verdict'] = 'miss'
            else:
                row['verdict'] = 'pass' if p99_state == 'pass' else 'cannot exclude a miss'
            if a['max'] is not None:
                attributed_max = a['max'] if attributed_max is None else max(attributed_max, a['max'])
            pairs.append(row)
        late_left = agg['late'] - sum(a['late'] for a in found.values())
        received_left = agg['received'] - sum(a['received'] for a in found.values())
        max_limit = None if delta_rel is None else (delta_rel - 2) * 1000
        slower = agg['max'] is not None and (attributed_max is None or agg['max'] > attributed_max)
        if late_left > 0 or (received_left > 0 and slower and max_limit is not None and agg['max'] >= max_limit):
            v = 'miss'
        elif received_left > 0 and (agg['max'] is None or agg['max'] > p99_limit or max_limit is None):
            v = 'cannot exclude a miss'
        elif received_left > 0:
            v = 'pass'
        else:
            v = 'all attributed'
        receivers.append({'receiver': label, 'aggregateLate': agg['late'], 'aggregateReceived': agg['received'], 'aggregateMaxMs': agg['max'],
                          'unattributedLate': late_left, 'unattributedReceived': received_left, 'verdict': v})
    observed = set(servers)
    not_observable = [{'receiver': name, 'reason': 'no RPC on that device (the PC runs the wallet engine over stdio)'}
                      for name in sorted(set(names.values()) - observed)]
    return {'declared': declared, 'pairs': pairs, 'receivers': receivers, 'notObservable': not_observable,
            'note': 'p99 is the upper bound of its bucket (never below the exact value), and the limits are bucket bounds. '
                    'A pair\'s p99 is the highest of its pieces (each epoch, and the entry before each possible eviction, '
                    'counted in possibleResets): a pass when none is above the limit, a miss when the templates certainly above '
                    'it are more than 1 % of the most the pair can hold, else "cannot exclude a miss" (epochs\' p99s do not '
                    'add up). A pair\'s counts may miss a possibly evicted entry\'s (never more than the true ones), which '
                    'changes no verdict: late and max cannot drop without a visible reset. A receiver\'s '
                    'unattributed templates were timed for proposers evicted between two samples (their pair is unknown): '
                    'a late one, or one at or above the max limit, is a miss; above p99\'s limit their p99 is unknown. '
                    'Drill windows cannot be left out of these cumulative counters.'}


def t2(servers, clocks, align_ms):
    """T2: pairwise clock offsets <= 500 ms, graded on the sign-free bound |o_i| + |o_j| of samples taken within
    `align_ms` of each other; and no timed template rejected as future (beforeStampMs.max <= Φ·1000, Φ = 2φ)."""
    labels = sorted(clocks)
    pairs = []
    for i, a in enumerate(labels):
        for b in labels[i + 1:]:
            # Every sample of either host is matched with the other host's nearest sample within align_ms, in both
            # directions, so neither a sampling gap on one host nor the labels' order can hide an observed offset.
            worst, matched = None, set()
            for xs, ys, flip in ((clocks[a], clocks[b], False), (clocks[b], clocks[a], True)):
                sy = sorted(ys)
                times = [y[0] for y in sy]
                for tx, ox in xs:
                    k = bisect.bisect_left(times, tx)
                    near = min(sy[max(k - 1, 0):k + 1], key=lambda y: abs(y[0] - tx), default=None)
                    if near is None or abs(near[0] - tx) > align_ms:
                        continue
                    matched.add((near[0], tx) if flip else (tx, near[0]))
                    bound = abs(ox) + abs(near[1])
                    worst = bound if worst is None else max(worst, bound)
            n = len(matched)
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


def t6(blocks, windows, step_height, end=None):
    """T6 from the blocks file: the 24 h mean interval (10 s ± 5 %) per full day from genesis, the first 100 blocks'
    mean (5-20 s), and the difficulty against its trailing 60-block mean (τ / T_blk), whose grading is to confirm.

    A day (lo, hi] is 86,400 s divided by the blocks it holds, where an interval (ts[h-1], ts[h]] that crosses a day's
    boundary counts in each day by the share of its length that lies there. So a stall counts against the day it falls
    in, whichever day its next block lands in, and is never counted twice; every full day is listed, one without a block
    as stalled. `end` ({'unixMs', 'source'}, from merge) is a time at which the last block was still the head: the time
    from the last block to it holds no block, so the days up to it are listed too, and it is reported."""
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
        horizon = last_ts
        if end is not None:
            horizon = max(last_ts, fractions.Fraction(end['unixMs'], 1000))
            out['afterLastBlock'] = {'lastBlockHeight': ordered[-1]['height'], 'lastBlockTimestamp': last_ts, 'endUnixMs': end['unixMs'],
                                     'endSource': end['source'], 'secondsWithoutBlock': float(horizon - last_ts),
                                     'partialDaySeconds': float((horizon - g) % 86400)}
        else:
            out['afterLastBlock'] = {'lastBlockHeight': ordered[-1]['height'], 'lastBlockTimestamp': last_ts, 'endUnixMs': None,
                                     'note': 'not known: the blocks file records no collection time at the head and no sample shows '
                                             'its last block as the head, so a stall after the last block is in no day'}
        n_days = max(0, int((horizon - g) // 86400))
        share, longest, inside = [0] * n_days, [0] * n_days, [0] * n_days
        day_of = lambda ts: (ts - g - 1) // 86400  # the day (lo, hi] holding ts
        for x, y in zip(ordered, ordered[1:]):
            a, b, n = x['timestamp'], y['timestamp'], y['height'] - x['height']  # a gap in the heights is n blocks
            if 0 <= day_of(b) < n_days:
                inside[day_of(b)] += 1
            if b <= a:  # no length: its blocks count in the day of b
                if 0 <= day_of(b) < n_days:
                    share[day_of(b)] += n
                continue
            for d in range(max(0, (a - g) // 86400), min(n_days - 1, day_of(b)) + 1):
                lo, hi = g + d * 86400, g + (d + 1) * 86400
                part = min(b, hi) - max(a, lo)
                if part > 0:
                    # Exact, so a day exactly at a threshold is graded as such.
                    share[d] += n if part == b - a else fractions.Fraction(n * part, b - a)
                    longest[d] = max(longest[d], b - a)
        for d in range(max(0, (last_ts - g) // 86400), n_days):  # the open time after the last block, up to the end
            if horizon > max(last_ts, g + d * 86400):
                longest[d] = max(longest[d], float(horizon - last_ts))
        for d in range(n_days):
            m = fractions.Fraction(86400) / share[d] if share[d] else None
            days.append({'day': d, 'blocks': inside[d], 'stalled': inside[d] == 0, 'meanSeconds': None if m is None else float(m),
                         'longestIntervalSeconds': longest[d], 'threshold': [9.5, 10.5],
                         'verdict': verdict(m is not None and fractions.Fraction(19, 2) <= m <= fractions.Fraction(21, 2))})
    out['dayMeans'] = days or [{'verdict': 'not measured (less than 24 h from genesis to the end)'}]
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


def blocks_end(header, blocks, samples):
    """The latest time at which the blocks file's last block is known to have been the head: the file's collection time
    when it was written up to the head, or a sample whose head is that block (same height and hash). None if neither."""
    last = max(blocks, key=lambda b: b['height'])
    found = []
    if (isinstance(header.get('collectedUnixMs'), int) and header.get('headHeight') == header.get('toHeight') == last['height']
            and header.get('headHash') in (None, last.get('hash'))):
        found.append((header['collectedUnixMs'], 'blocks file collected at its last block'))
    for label, s in sorted(samples.items()):
        for x in s:
            head = dig(x, 'status', 'head') or {}
            if head.get('height') == last['height'] and head.get('hash') is not None and head.get('hash') == last.get('hash'):
                found.append((x['unixMs'], f'sample {label} {x["utc"]} with the last block as its head'))
    if not found:
        return None
    ms, source = max(found)
    return {'unixMs': ms, 'source': source}


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
        check_chain(lines[0], blocks)
    end = blocks_end(lines[0], blocks, samples) if blocks else None
    windows = read_windows(windows_path)
    return {'v': VERSION, 'identity': identity, 'inputs': {'samples': list(paths), 'blocks': blocks_path, 'windows': windows},
            'labels': {'servers': sorted(servers), 'clocks': sorted(clocks), 'addresses': {v: k for k, v in names.items()}},
            'T1': t1(servers, names, declared), 'T2': t2(servers, clocks, align_ms), 'T5': t5(servers, windows, every_ms),
            'T6': t6(blocks, windows, step_height, end) if blocks else {'verdict': 'not measured (no --blocks file)'},
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
