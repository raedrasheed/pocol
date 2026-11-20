#!/usr/bin/env python3
"""collect_measures.py (REHEARSAL.md section 8): sampling against a fake RPC, the URL and privacy rules, clock parsing,
the segments, the T1, T2, T5 and T6 rules at their boundaries, and the merge's refusals. Opt-in under
POCOL_REHEARSAL_E2E=1: `sample --once` and `blocks` against a real `pocol start` node, to catch renamed fields.
"""
import http.server
import json
import os
import re
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import time
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
sys.path.insert(0, str(HERE))
import collect_measures as cm  # noqa: E402

PC_IP, PEER_IP = '203.0.113.77', '198.51.100.23'
A = {'s1': '0x' + '11' * 20, 's2': '0x' + '22' * 20, 's3': '0x' + '33' * 20, 'pc': '0x' + '44' * 20}
IDENT = {'chainId': 1656550731, 'genesisHash': '0x' + 'ab' * 32, 'consensusIdentity': '0x' + 'cd' * 32}


def hist(p99=None, mx=None, count=0):
    return {'count': count, 'p50': p99, 'p90': p99, 'p99': p99, 'max': mx, 'mean': p99}


def proposer(addr, received=10, late=0, p99=500, mx=900, before_max=100, slots=None):
    return {'proposer': addr, 'slots': received if slots is None else slots, 'received': received, 'missing': 0, 'late': late,
            'early': 0, 'stampAtWindow': 0, 'locked': received, 'sinceParentMs': hist(p99, mx, received),
            'sinceStampMs': hist(), 'beforeStampMs': hist(before_max, before_max, received), 'lockMs': hist()}


def latency(proposers, before_max=100, slots=10, delta_rel=7, phi=1):
    return {'local': {'delta': 1, 'phi': phi, 'tExec': 1, 'deltaRel': delta_rel, 'gamma': 5, 'dFbWait': 8}, 'tracked': len(proposers),
            'all': {**proposer('all', slots=slots, before_max=before_max), 'slots': slots}, 'proposers': proposers}


def status(height=10, imported=10, refetched=0, miner=None, fallback=0):
    return {**IDENT, 'head': {'height': height, 'hash': '0x' + '01' * 32, 'ts': 1}, 'miner': miner,
            'peers': [f'{PC_IP}:30300', f'{PEER_IP}:30300'], 'discovery': {'book': [PC_IP]},
            'lastIncompatibleReason': f'network from {PEER_IP}', 'a13': {'boundBytes': '5512153289', 'budgetBytes': 6 << 30, 'conditions': []},
            'mining': {'threads': 1, 'running': 1, 'cores': 4, 'workRpc': '127.0.0.1:1'},
            'stats': {'imported': imported, 'refetched': refetched, 'fallbackTemplates': fallback}}


def sample(label, t, pid=100, restarts=0, reorgs=0, alarm=0, depth=0, height=10, imported=10, refetched=0, lat=None, offset=0.0, miner=None):
    return {'v': 1, 'kind': 'sample', 'label': label, 'utc': cm.utc(t * 1000), 'unixMs': t * 1000,
            'service': {'mainPid': pid, 'nRestarts': restarts, 'startTs': 'x'}, 'rssKb': 1,
            'clock': {'source': 'chrony', 'reference': 'ntp.example', 'stratum': 2, 'offsetMs': offset},
            'status': cm.status_subset(status(height, imported, refetched, miner)),
            'finality': {'reorgs': reorgs, 'reorgsGeKAlarm': alarm, 'maxReorgDepth': depth, 'kSafe': 6, 'kAlarm': 60},
            'latency': lat if lat is not None else latency([])}


class FakeNode:
    """A loopback JSON-RPC server answering from `answers` (method -> list of results or errors, the last repeated)."""

    def __init__(self, answers):
        self.answers, self.seen = answers, []
        outer = self

        class Handler(http.server.BaseHTTPRequestHandler):
            protocol_version = 'HTTP/1.1'

            def do_POST(self):
                req = json.loads(self.rfile.read(int(self.headers['Content-Length'])))
                outer.seen.append(req['method'])
                q = outer.answers[req['method']]
                a = q.pop(0) if len(q) > 1 else q[0]
                body = json.dumps({'jsonrpc': '2.0', 'id': 1, **a}).encode()
                self.send_response(200)
                self.send_header('Content-Length', str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, *a):
                pass
        self.server = http.server.ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        threading.Thread(target=self.server.serve_forever, daemon=True).start()
        self.url = f'http://127.0.0.1:{self.server.server_port}'

    def close(self):
        self.server.shutdown()
        self.server.server_close()


def no_commands(cmd):
    return None


class Sampling(unittest.TestCase):
    def setUp(self):
        self.dir = Path(tempfile.mkdtemp(prefix='pocol-measures-'))
        self.addCleanup(shutil.rmtree, self.dir)

    def node(self, answers):
        n = FakeNode(answers)
        self.addCleanup(n.close)
        return n

    def args(self, url, *extra):
        return ['sample', '--rpc', url, '--label', 's1', '--out', str(self.dir), *extra]

    def lines(self):
        files = list(self.dir.glob('measures-s1-*.jsonl'))
        self.assertEqual(len(files), 1, files)
        return files[0], cm.read_jsonl(files[0])

    def test_one_sample_keeps_the_whitelisted_fields_and_no_peer_address(self):
        n = self.node({'pocol_status': [{'result': status(miner=A['s1'])}], 'pocol_finality': [{'result': {'reorgs': 0, 'kSafe': 6}}],
                       'pocol_templateLatency': [{'result': latency([proposer(A['pc'])])}]})
        self.assertEqual(cm.main(self.args(n.url, '--once', '--expect-chain-id', '1656550731')), 0)
        path, lines = self.lines()
        self.assertEqual(len(lines), 1)
        s = lines[0]
        self.assertEqual((s['v'], s['kind'], s['label']), (1, 'sample', 's1'))
        self.assertEqual({k: s['status'][k] for k in cm.IDENTITY}, IDENT)
        self.assertEqual(s['status']['a13'], {'boundBytes': '5512153289', 'budgetBytes': 6 << 30})
        self.assertEqual(s['status']['mining'], {'threads': 1, 'running': 1, 'cores': 4})
        self.assertEqual(s['latency']['proposers'][0]['proposer'], A['pc'])
        for gone in ('peers', 'discovery', 'lastIncompatibleReason'):
            self.assertNotIn(gone, s['status'])
        raw = path.read_text(encoding='utf-8')
        for ip in (PC_IP, PEER_IP, '127.0.0.1:1'):
            self.assertNotIn(ip, raw, 'no peer or local address reaches the evidence')
        self.assertEqual(n.seen, ['pocol_status', 'pocol_status', 'pocol_finality', 'pocol_templateLatency'])

    def test_a_wrong_chain_is_refused_before_any_file(self):
        n = self.node({'pocol_status': [{'result': status()}]})
        self.assertEqual(cm.main(self.args(n.url, '--once', '--expect-chain-id', '2656089850')), 3)
        self.assertEqual(list(self.dir.iterdir()), [])

    def test_only_a_loopback_http_url(self):
        for url in ('http://192.0.2.1:8545', 'https://127.0.0.1:8545', 'http://user:pw@127.0.0.1:8545', 'http://127.0.0.1:8545/rpc',
                    'http://127.0.0.1:8545/?a=1', 'http://127.0.0.1:8545#x', 'http://localhost:8545', 'http://127.0.0.1'):
            with self.assertRaises(cm.Refused, msg=url):
                cm.check_url(url)
        self.assertEqual(cm.check_url('http://[::1]:8545'), 'http://[::1]:8545')
        self.assertEqual(cm.main(self.args('http://192.0.2.1:8545', '--once')), 3)

    def test_a_proxy_variable_cannot_reroute_the_call(self):
        n = self.node({'pocol_status': [{'result': status()}]})
        with socket.socket() as s:
            s.bind(('127.0.0.1', 0))
            dead = s.getsockname()[1]
        old = {k: os.environ.get(k) for k in ('HTTP_PROXY', 'http_proxy', 'NO_PROXY', 'no_proxy')}
        os.environ.update({'HTTP_PROXY': f'http://127.0.0.1:{dead}', 'http_proxy': f'http://127.0.0.1:{dead}'})
        os.environ.pop('NO_PROXY', None)
        os.environ.pop('no_proxy', None)
        try:
            self.assertEqual(cm.Rpc(n.url).call('pocol_status')['chainId'], IDENT['chainId'])
        finally:
            for k, v in old.items():
                if v is None:
                    os.environ.pop(k, None)
                else:
                    os.environ[k] = v

    def test_busy_is_retried_and_an_rpc_failure_is_an_error_line_then_sampling_goes_on(self):
        busy = {'error': {'code': -32021, 'message': 'busy', 'data': {'retryAfterMs': 1}}}
        n = self.node({'pocol_status': [busy, {'result': status()}, {'error': {'code': -32603, 'message': 'restarting'}}, {'result': status()}],
                       'pocol_finality': [{'result': {'reorgs': 0}}], 'pocol_templateLatency': [{'result': latency([])}]})
        self.assertEqual(cm.main(self.args(n.url, '--every', '1', '--count', '3')), 0)
        _, lines = self.lines()
        self.assertEqual([x['kind'] for x in lines], ['sample', 'error', 'sample'])
        self.assertIn('restarting', lines[1]['error'])
        self.assertEqual(n.seen.count('pocol_status'), 4, 'the busy answer was asked again')

    def test_a_refused_connection_is_an_error_line(self):
        with socket.socket() as s:
            s.bind(('127.0.0.1', 0))
            dead = s.getsockname()[1]
        line = cm.take_sample(cm.Rpc(f'http://127.0.0.1:{dead}', timeout=2), 's1', 'pocol-standalone', no_commands)
        self.assertEqual(line['kind'], 'error')
        self.assertIn('error', line)

    def test_an_existing_output_file_is_never_replaced(self):
        good = self.dir / 's1.jsonl'
        good.write_text(json.dumps(sample('s1', 0)) + '\n', encoding='utf-8')
        out = self.dir / 'report.json'
        out.write_text('{}', encoding='utf-8')
        self.assertEqual(cm.main(['merge', '--in', str(good), '--out', str(out)]), 3)
        self.assertEqual(out.read_text(encoding='utf-8'), '{}')
        self.assertEqual(cm.main(['merge', '--in', str(out), '--out', str(self.dir / 'new.json')]), 3, 'a line without a label')


class Host(unittest.TestCase):
    def test_clock_sources(self):
        chrony = 'A29FC87B,ntp.example,2,1700000000.123,-0.000412345,0.000100,0.000200,-12.3,0.1,0.01,0.02,0.003,64.0,Normal\n'
        timesyncd = ('       Server: 192.0.2.5 (ntp.example)\nPoll interval: 34min 8s\n      Stratum: 2\n       Offset: +1.123ms\n'
                     '        Delay: 52.313ms\n')
        answers = {('chronyc', '-c', 'tracking'): chrony, ('timedatectl', 'timesync-status'): timesyncd}
        c = cm.read_clock(lambda cmd: answers.get(tuple(cmd)))
        self.assertEqual((c['source'], c['reference'], c['stratum']), ('chrony', 'ntp.example', 2))
        self.assertAlmostEqual(c['offsetMs'], -0.412345)
        c = cm.read_clock(lambda cmd: None if cmd[0] == 'chronyc' else answers.get(tuple(cmd)))
        self.assertEqual((c['source'], c['stratum']), ('timesyncd', 2))
        self.assertAlmostEqual(c['offsetMs'], 1.123)
        self.assertEqual(cm.read_clock(no_commands), {'source': 'none', 'reference': None, 'stratum': None, 'offsetMs': None})
        for text, ms in (('-512us', -0.512), ('+2.5s', 2500.0), ('7ms', 7.0), ('+40ns', 0.00004)):
            self.assertAlmostEqual(cm.parse_duration_ms(text), ms)
        self.assertIsNone(cm.parse_chrony('garbage'))
        self.assertIsNone(cm.parse_timesyncd('Offset: n/a'))

    def test_service_and_rss(self):
        out = 'MainPID=4242\nNRestarts=3\nExecMainStartTimestamp=Fri 2026-11-20 10:00:00 UTC\n'
        self.assertEqual(cm.read_service('pocol-standalone', lambda cmd: out if cmd[0] == 'systemctl' and cmd[-1] == 'pocol-standalone' else None),
                         {'mainPid': 4242, 'nRestarts': 3, 'startTs': 'Fri 2026-11-20 10:00:00 UTC'})
        self.assertIsNone(cm.read_service('pocol-standalone', no_commands))
        with tempfile.TemporaryDirectory() as d:
            os.makedirs(os.path.join(d, '4242'))
            Path(d, '4242', 'status').write_text('Name:\tpocol\nVmRSS:\t  123456 kB\n', encoding='utf-8')
            self.assertEqual(cm.read_rss_kb(4242, d), 123456)
            self.assertIsNone(cm.read_rss_kb(4243, d))
        self.assertIsNone(cm.read_rss_kb(0))


class Rules(unittest.TestCase):
    def test_segments_split_at_a_new_pid_or_a_counter_reset(self):
        s = [sample('s1', 0, reorgs=1), sample('s1', 60, reorgs=2), sample('s1', 120, pid=200, restarts=1, reorgs=0),
             sample('s1', 180, pid=200, restarts=1, reorgs=1), sample('s1', 240, pid=200, restarts=1, reorgs=0)]
        self.assertEqual([[x['unixMs'] // 1000 for x in seg] for seg in cm.segments(s)], [[0, 60], [120, 180], [240]])

    def t5(self, samples, windows=()):
        return cm.t5({'s1': cm.segments(samples)}, list(windows), 60_000)

    def test_t5_intervals_outside_the_drill_windows(self):
        ok = self.t5([sample('s1', 0), sample('s1', 60, reorgs=3, depth=2)])
        self.assertEqual([r['verdict'] for r in ok['intervals']], ['pass'], 'depth exactly 2 passes')
        deep = self.t5([sample('s1', 0), sample('s1', 60, reorgs=1, depth=3)])
        self.assertEqual(deep['intervals'][0]['verdict'], 'miss')
        alarm = self.t5([sample('s1', 0), sample('s1', 60, alarm=1)])
        self.assertEqual(alarm['intervals'][0]['verdict'], 'miss')
        # A drill window between the samples at 60 and 120 s: its deep reorganization is left out.
        w = [{'name': 'partition', 'startMs': 70_000, 'endMs': 110_000}]
        drill = self.t5([sample('s1', 0), sample('s1', 60), sample('s1', 120, reorgs=1, depth=9), sample('s1', 180, reorgs=1, depth=9)], w)
        self.assertEqual([(r['fromUtc'][17:19], r['toUtc'][17:19], r['verdict']) for r in drill['intervals']], [('00', '00', 'pass'), ('00', '00', 'pass')])
        self.assertEqual(len(drill['intervals']), 2)
        # Restarts are gaps, planned only inside a window.
        r = self.t5([sample('s1', 0), sample('s1', 60), sample('s1', 120, pid=7, restarts=1)])
        self.assertEqual([g['restart'] for g in r['gaps']], ['unplanned (outside every window)'])
        # Siblings: (Δimported - Δrefetched - Δhead) / Δhead.
        sib = self.t5([sample('s1', 0, height=0, imported=0), sample('s1', 60, height=100, imported=103, refetched=1)])
        self.assertAlmostEqual(sib['intervals'][0]['siblingEstimate'], 0.02)
        self.assertEqual(sib['intervals'][0]['siblingVerdict'], 'estimate pass')

    def t1(self, prop, declared='delta1', delta_rel=7):
        servers = {'s1': [[sample('s1', 0, lat=latency([prop], delta_rel=delta_rel), miner=A['s1'])]]}
        return cm.t1(servers, {A['s1']: 's1', A['pc']: 'pc'}, declared)

    def test_t1_boundaries(self):
        v = lambda **k: self.t1(proposer(A['pc'], **k))['pairs'][0]['verdict']
        self.assertEqual(v(p99=1000, mx=4999), 'pass')
        self.assertEqual(v(p99=1500, mx=1500), 'miss', 'Δ1: p99 above 1,000 ms')
        self.assertEqual(self.t1(proposer(A['pc'], p99=2000, mx=2000), 'delta2')['pairs'][0]['verdict'], 'pass')
        self.assertEqual(self.t1(proposer(A['pc'], p99=2001, mx=2001), 'delta2')['pairs'][0]['verdict'], 'miss')
        self.assertEqual(v(p99=500, mx=5000), 'miss', 'max < Δ_rel - 2 s is strict')
        self.assertEqual(self.t1(proposer(A['pc'], p99=500, mx=5999), delta_rel=8)['pairs'][0]['verdict'], 'pass')
        self.assertEqual(v(late=1), 'miss')
        self.assertEqual(v(received=0, p99=None, mx=None, slots=3), 'not measured')
        r = self.t1(proposer('0x' + '99' * 20))
        self.assertEqual(r['pairs'][0]['proposer'], '0x' + '99' * 20, 'an unlabelled proposer keeps its address')
        self.assertEqual(self.t1(proposer(A['pc']))['pairs'][0]['proposer'], 'pc')
        self.assertEqual(r['notObservable'], [{'receiver': 'pc', 'reason': 'no RPC on that device (the PC runs the wallet engine over stdio)'}])

    def test_t1_adds_the_segments_and_keeps_the_worst(self):
        segs = [[sample('s1', 0, lat=latency([proposer(A['pc'], received=5, p99=300, mx=400)]))],
                [sample('s1', 600, pid=9, lat=latency([proposer(A['pc'], received=7, late=1, p99=750, mx=800)]))]]
        row = cm.t1({'s1': segs}, {A['pc']: 'pc'}, 'delta1')['pairs'][0]
        self.assertEqual((row['received'], row['late'], row['p99'], row['max'], row['segments'], row['verdict']), (12, 1, 750, 800, 2, 'miss'))

    def test_t2_clock_bound_and_future_templates(self):
        clocks = {'s1': [(0, 250.0)], 's2': [(30_000, -260.0)], 'pc': [(500_000, 0.0)]}
        r = cm.t2({}, clocks, 120_000)
        by = {tuple(p['pair']): p for p in r['clockPairs']}
        self.assertEqual((by[('s1', 's2')]['worstBoundMs'], by[('s1', 's2')]['verdict']), (510.0, 'miss'))
        self.assertEqual(by[('pc', 's1')]['verdict'], 'not measured', 'no samples within the alignment')
        ok = cm.t2({}, {'s1': [(0, 250.0)], 's2': [(0, -250.0)]}, 120_000)
        self.assertEqual(ok['clockPairs'][0]['verdict'], 'pass')
        fut = lambda m: cm.t2({'s1': [[sample('s1', 0, lat=latency([], before_max=m))]]}, {}, 1)['futureTemplates'][0]['verdict']
        self.assertEqual(fut(2000), 'no timed template rejected as future')
        self.assertEqual(fut(2001), 'cannot exclude')

    def test_t6_intervals(self):
        def chain(n, step, start=1_800_000_000, diff=1000):
            return [{'height': h, 'timestamp': start + int(round(h * step)), 'difficulty': str(diff), 'attempt': 0, 'signed': True,
                     'txCount': 0} for h in range(n)]
        first = lambda step: cm.t6(chain(101, step), [], None)['first100MeanSeconds']['verdict']
        self.assertEqual(first(5), 'pass')
        self.assertEqual(first(20), 'pass')
        self.assertEqual(cm.t6(chain(101, 1) + [], [], None)['first100MeanSeconds']['verdict'], 'miss')
        b = chain(101, 5)
        b[100]['timestamp'] = b[0]['timestamp'] + 499
        self.assertEqual(cm.t6(b, [], None)['first100MeanSeconds']['verdict'], 'miss', '4.99 s')
        day = lambda step: cm.t6(chain(int(86400 / step) + 20, step), [], None)['dayMeans'][0]
        self.assertEqual(day(10.5)['verdict'], 'pass')
        self.assertEqual(day(10.52)['verdict'], 'miss')
        self.assertEqual(day(9.5)['verdict'], 'pass')
        self.assertEqual(cm.t6(chain(200, 10), [], None)['dayMeans'], [{'verdict': 'not measured (less than 24 h of blocks)'}])
        d = cm.t6(chain(200, 10), [], 100)['difficulty']
        self.assertEqual((d['verdict'], d['maxDeviationFromTrailing60Mean'], d['blocksToWithin5PercentAfterStep']), ('definition to confirm', 0.0, 1))


class Merge(unittest.TestCase):
    def setUp(self):
        self.dir = Path(tempfile.mkdtemp(prefix='pocol-merge-'))
        self.addCleanup(shutil.rmtree, self.dir)

    def write(self, name, lines):
        p = self.dir / name
        p.write_text(''.join(json.dumps(x) + '\n' for x in lines), encoding='utf-8')
        return str(p)

    def test_a_full_merge_and_its_refusals(self):
        s1 = self.write('s1.jsonl', [sample('s1', 0, miner=A['s1'], lat=latency([proposer(A['pc'])])), sample('s1', 60, miner=A['s1'], lat=latency([proposer(A['pc'])]))])
        pc = self.write('pc.jsonl', [{'v': 1, 'kind': 'clock', 'label': 'PC', 'utc': cm.utc(0), 'unixMs': 0, 'clock': {'source': 'w32tm', 'offsetMs': 12.5}}])
        blocks = self.write('blocks.jsonl', [{'kind': 'chain', **IDENT}] + [{'height': h, 'timestamp': 1_800_000_000 + 10 * h, 'difficulty': '100', 'attempt': 0,
                                                                              'signed': True, 'txCount': 0} for h in range(101)])
        windows = self.dir / 'windows.json'
        windows.write_text(json.dumps({'windows': [{'name': 'pc off/on', 'start': '2026-12-01T10:00:00Z', 'end': 1796121000}]}), encoding='utf-8')
        out = self.dir / 'report.json'
        self.assertEqual(cm.main(['merge', '--in', s1, pc, '--blocks', blocks, '--windows', str(windows), '--address', f'pc={A["pc"]}', '--out', str(out)]), 0)
        r = json.loads(out.read_text(encoding='utf-8'))
        self.assertEqual(r['identity'], IDENT)
        self.assertEqual(r['T1']['pairs'][0]['receiver'], 's1')
        self.assertEqual(r['T1']['pairs'][0]['proposer'], 'pc')
        self.assertEqual(r['T6']['first100MeanSeconds']['verdict'], 'pass')
        self.assertEqual(r['inputs']['windows'][0]['endMs'] - r['inputs']['windows'][0]['startMs'], 1_800_000)
        self.assertIn('closes no gate', r['note'])
        # Files of another chain are refused, and nothing is written.
        other = self.write('other.jsonl', [dict(sample('s2', 0), status={**sample('s2', 0)['status'], 'genesisHash': '0x' + 'ee' * 32})])
        out2 = self.dir / 'report2.json'
        self.assertEqual(cm.main(['merge', '--in', s1, other, '--out', str(out2)]), 3)
        bad_blocks = self.write('bad.jsonl', [{'kind': 'chain', **IDENT, 'chainId': 2656089850}])
        self.assertEqual(cm.main(['merge', '--in', s1, '--blocks', bad_blocks, '--out', str(out2)]), 3)
        self.assertEqual(cm.main(['merge', '--in', s1, '--address', 'pc=0xABC', '--out', str(out2)]), 3)
        self.assertFalse(out2.exists())


class Scripts(unittest.TestCase):
    def test_the_pc_clock_logger_measures_as_the_ceremony_does(self):
        clock = (HERE / 'Collect-PoColClock.ps1').read_text(encoding='utf-8')
        (HERE / 'Collect-PoColClock.ps1').read_bytes().decode('ascii')
        (HERE / 'test_collect_clock.ps1').read_bytes().decode('ascii')
        ceremony = (ROOT / 'deploy/standalone/Initialize-PoColNetwork.ps1').read_text(encoding='utf-8')
        for same in ("[regex]::Match($Chart, '(?m),\\s*([+-]\\d+[.,]\\d+)s\\s*$')", "-match 'Local CMOS Clock|Free-running System Clock'",
                     "($TimeSource -split ',')[0].Trim()", "/stripchart \"/computer:$TimePeer\" /samples:1 /dataonly"):
            self.assertIn(same, clock)
            self.assertIn(same, ceremony)
        workflow = (ROOT / '.github/workflows/full-nonce-upgrade.yml').read_text(encoding='utf-8')
        self.assertIn('./deploy/rehearsal/test_collect_clock.ps1', workflow)
        self.assertIn("'deploy/rehearsal/**'", workflow)
        # This file's tests read REHEARSAL.md, so an edit to it alone runs them; and REHEARSAL.md section 5's store-mode
        # figure is pinned by network_budget.rs, which this workflow runs.
        filters = re.findall(r'^    paths: \[(.*)\]$', workflow, re.M)
        self.assertEqual(len(filters), 2)
        for paths in filters:
            self.assertIn("'development/public-network/REHEARSAL.md'", paths)
        self.assertIn('--test fn4_profiles --test launch_delay_asert --test network_budget', workflow)

    def test_the_collector_runs_on_the_standard_library_alone(self):
        src = (HERE / 'collect_measures.py').read_text(encoding='utf-8')
        import ast
        mods = {n.names[0].name.split('.')[0] for n in ast.walk(ast.parse(src)) if isinstance(n, ast.Import)}
        mods |= {n.module.split('.')[0] for n in ast.walk(ast.parse(src)) if isinstance(n, ast.ImportFrom) and n.module}
        self.assertLessEqual(mods, set(sys.stdlib_module_names), mods - set(sys.stdlib_module_names))
        rehearsal = (ROOT / 'development/public-network/REHEARSAL.md').read_text(encoding='utf-8')
        self.assertIn('## 8. The measure collector (T1, T2, T5, T6)', rehearsal)
        for name in ('collect_measures.py sample', 'collect_measures.py blocks', 'collect_measures.py merge', 'Collect-PoColClock.ps1'):
            self.assertIn(name, rehearsal)

    @unittest.skipUnless(shutil.which('pwsh'), 'pwsh is not installed (the Windows CI job runs the PowerShell test)')
    def test_the_powershell_test_passes_under_pwsh(self):
        p = subprocess.run(['pwsh', '-NoProfile', '-File', str(HERE / 'test_collect_clock.ps1')], capture_output=True, text=True, timeout=300)
        self.assertEqual(p.returncode, 0, p.stdout + p.stderr)


ENABLED = os.environ.get('POCOL_REHEARSAL_E2E') == '1'
BIN = Path(os.environ.get('POCOLD_BIN_DIR', ROOT / 'prototypes/pocold/target/release'))
EXE = '.exe' if os.name == 'nt' else ''


@unittest.skipUnless(ENABLED, 'opt-in: POCOL_REHEARSAL_E2E=1 (a real local node)')
class RealNode(unittest.TestCase):
    """`sample --once` and `blocks` against a real `pocol start` node: with one founder `proposers` is empty, so this
    checks the shape and the field names, not the measures."""

    def test_sample_and_blocks_against_pocol_start(self):
        tmp = Path(tempfile.mkdtemp(prefix='pocol-measures-e2e-'))
        self.addCleanup(shutil.rmtree, tmp)

        def tool(name, *args):
            p = subprocess.run([str(BIN / (name + EXE)), *map(str, args)], capture_output=True, text=True, timeout=120)
            self.assertEqual(p.returncode, 0, p.stderr)
            return p.stdout

        def port():
            with socket.socket() as s:
                s.bind(('127.0.0.1', 0))
                return s.getsockname()[1]
        profile = json.loads((ROOT / 'deploy/mainnet/v3-rehearsal-profile.json').read_text(encoding='utf-8'))
        founder = json.loads(tool('pocol', 'identity', '--home', tmp / 'home'))['address']
        # A funded account (init-testnet's calibration needs one); its key is a throwaway in the temporary folder.
        account = tool('pocol-key', 'generate', '--out', tmp / 'account.pocol-key').strip()
        config = {'chainId': 777963, 'genesisTime': int(time.time()) + 3, 'registrationPerBlock': profile['registrationPerBlock'],
                  'localTiming': profile['localTiming'], 'cp': profile['cp'], 'miners': [{'id': founder, 'reward': founder}],
                  'accounts': [{'address': account, 'wei': str(10**24)}]}
        (tmp / 'config.json').write_text(json.dumps(config), encoding='utf-8')
        tool('pocold', 'init-testnet', '--config', tmp / 'config.json', '--out', tmp / 'genesis')
        p2p, api = port(), port()
        tool('pocol', 'network', '--from', tmp / 'genesis', '--seed', f'127.0.0.1:{p2p}', '--out', tmp / 'network.pocol.json')
        log = open(tmp / 'node.log', 'w', encoding='utf-8')
        self.addCleanup(log.close)
        node = subprocess.Popen([str(BIN / ('pocol' + EXE)), 'start', '--home', str(tmp / 'home'), '--network', str(tmp / 'network.pocol.json'),
                                 '--listen', f'127.0.0.1:{p2p}', '--rpc', f'127.0.0.1:{api}'], stdout=log, stderr=log)

        def stop():
            node.terminate()
            try:
                node.wait(timeout=20)
            except subprocess.TimeoutExpired:
                node.kill()
        self.addCleanup(stop)
        url = f'http://127.0.0.1:{api}'
        deadline = time.time() + 180
        while True:
            try:
                if cm.Rpc(url).call('pocol_status')['head']['height'] >= 2:
                    break
            except (OSError, ValueError, cm.RpcError):
                pass
            self.assertLess(time.time(), deadline, 'the node did not reach height 2')
            time.sleep(0.5)
        out = tmp / 'measures'
        self.assertEqual(cm.main(['sample', '--rpc', url, '--label', 'local', '--out', str(out), '--once', '--unit', 'pocol-e2e-none']), 0)
        line = cm.read_jsonl(next(out.glob('measures-local-*.jsonl')))[0]
        self.assertEqual(line['kind'], 'sample', line)
        self.assertEqual(line['status']['chainId'], 777963)
        self.assertEqual(line['status']['miner'], founder)
        for path in (('status', 'genesisHash'), ('status', 'consensusIdentity'), ('status', 'head', 'height'), ('status', 'stats', 'imported'),
                     ('status', 'stats', 'refetched'), ('status', 'stats', 'fallbackTemplates'), ('status', 'a13', 'boundBytes'),
                     ('finality', 'reorgs'), ('finality', 'maxReorgDepth'), ('finality', 'reorgsGeKAlarm'), ('finality', 'kSafe'),
                     ('latency', 'local', 'deltaRel'), ('latency', 'local', 'phi'), ('latency', 'all', 'slots'), ('latency', 'all', 'beforeStampMs'),
                     ('latency', 'proposers')):
            self.assertIsNotNone(cm.dig(line, *path), path)
        for gone in ('peers', 'discovery'):
            self.assertNotIn(gone, line['status'])
        blocks = tmp / 'blocks.jsonl'
        self.assertEqual(cm.main(['blocks', '--rpc', url, '--out', str(blocks), '--to', '2']), 0)
        lines = cm.read_jsonl(blocks)
        self.assertEqual(lines[0]['kind'], 'chain')
        self.assertEqual([b['height'] for b in lines[1:]], [0, 1, 2])
        for b in lines[2:]:
            self.assertEqual(set(b), {'height', 'hash', 'parentHash', 'timestamp', 'difficulty', 'attempt', 'signed', 'proposer', 'shares', 'gasUsed', 'txCount'})
            self.assertIsInstance(b['attempt'], int)
            self.assertGreater(int(b['difficulty']), 0)
        self.assertEqual(lines[2]['parentHash'], lines[1]['hash'])
        report = tmp / 'report.json'
        self.assertEqual(cm.main(['merge', '--in', *map(str, out.glob('*.jsonl')), '--blocks', str(blocks), '--out', str(report)]), 0)


if __name__ == '__main__':
    unittest.main()
