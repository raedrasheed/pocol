#!/usr/bin/env python3
"""pocol.net's network summary (api/network.php) against the explorer gateway's own request budget (2026-11-22 review).

The summary's reads go through explorer.pocol.net/api/rpc.php, which charges the gateway's single token bucket
(deploy/rewards/gateway.py Gateway.charge: 80 tokens, refilled at 20 a second, shared by every explorer visitor; the
cost of each call is gateway.COSTS, as explorer_api.Relay is built with). This script runs pocol_network_summary in PHP
at the largest supported membership (10,000 members), records the batches it sends, and replays them through the real
Gateway.charge on a virtual clock: one refresh every POCOL_SUMMARY_CACHE_TTL seconds (the cache allows no more) while
explorer visitors read at half the refill rate. Every summary request must be accepted, no visitor read refused, and
a refresh must cost the same at 10,000 members as at 5.

Needs php-cli (CI runs it in the PHP step). Exit status 0 only when every check holds."""
import json, shutil, subprocess, sys, tempfile
from pathlib import Path
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'deploy' / 'rewards'))
import gateway  # noqa: E402

NETWORK = ROOT / 'deploy' / 'websites' / 'pocol.net' / 'api' / 'network.php'
DRIVER = r"""<?php
declare(strict_types=1);
define('POCOL_NETWORK_LIBRARY_ONLY', true);
require $argv[1];
$active = (int)$argv[2];
$trace = [];
$hash = static fn(int $h): string => '0x'.str_pad(dechex($h + 1), 64, '0', STR_PAD_LEFT);
// A members page as the gateway answers pocol_getMembers, so that a summary that pages the membership (the code before
// the 2026-11-22 review) runs too and is measured, instead of failing on an unknown call.
$page = static fn(array $r): object => (object)['height'=>500,'blockHash'=>$hash(500),'active'=>$active,'start'=>$r['params'][0],
    'count'=>max(0, min($r['params'][1], $active - $r['params'][0])),
    'members'=>array_fill(0, max(0, min($r['params'][1], $active - $r['params'][0])), (object)['status'=>1,'tomb'=>false,'bondState'=>1])];
$rpc = static function(array $requests) use (&$trace, $hash, $active, $page): array {
    $trace[] = array_column($requests, 'method');
    if ($requests[0]['method'] === 'pocol_getMembers') return array_map($page, $requests);
    if ($requests[0]['method'] === 'pocol_status' && $requests[2]['method'] === 'pocol_getMembers') {
        return [(object)['chainId'=>POCOL_SUMMARY_CHAIN,'genesisHash'=>POCOL_SUMMARY_GENESIS,'head'=>(object)['height'=>500,'hash'=>$hash(500)],'pool'=>0],
            (object)['chainId'=>POCOL_SUMMARY_CHAIN,'genesisHash'=>POCOL_SUMMARY_GENESIS], $page($requests[2])];
    }
    if ($requests[0]['method'] === 'pocol_status') {
        $states = (object)['NONE'=>0,'HELD'=>$active,'BURNED'=>0,'PAID'=>0,'ACCRUING'=>0];
        return [(object)['chainId'=>POCOL_SUMMARY_CHAIN,'genesisHash'=>POCOL_SUMMARY_GENESIS,'head'=>(object)['height'=>500,'hash'=>$hash(500)],'pool'=>0],
            (object)['chainId'=>POCOL_SUMMARY_CHAIN,'genesisHash'=>POCOL_SUMMARY_GENESIS],
            (object)['height'=>500,'blockHash'=>$hash(500),'active'=>$active,'live'=>$active,'eligible'=>$active,'tomb'=>0,'bondStates'=>$states]];
    }
    return array_map(static function(array $r) use ($hash): object {
        $h = hexdec($r['params'][0]);
        return (object)['number'=>'0x'.dechex($h),'hash'=>$hash($h),'parentHash'=>$hash($h - 1),'timestamp'=>'0x'.dechex(1000 + 10 * $h)];
    }, $requests);
};
$summary = pocol_network_summary($rpc, 1700000000);
echo json_encode(['trace'=>$trace, 'eligible'=>$summary['metrics']['eligibleMiners'], 'ttl'=>POCOL_SUMMARY_CACHE_TTL]);
"""
NET = {'chainId': 777963, 'genesisHash': '0x' + '5a' * 32, 'specVersion': 2, 'v3': True}

def summary_trace(active):
    with tempfile.TemporaryDirectory() as tmp:
        driver = Path(tmp) / 'driver.php'
        driver.write_text(DRIVER, encoding='utf-8')
        run = subprocess.run(['php', str(driver), str(NETWORK), str(active)], capture_output=True, text=True, timeout=120)
    if run.returncode != 0:
        raise SystemExit(f'FAIL the summary did not run: {run.stderr[-400:]}')
    return json.loads(run.stdout)

def cost(methods):
    return sum(gateway.COSTS.get(m, 1) for m in methods)

def replay(trace, ttl, seconds=300.0, visitor_rate=10.0, round_trip=0.2):
    """(refused summary requests, refused visitor reads, lowest bucket level) of `seconds` of traffic through one
    real Gateway's charge(): the summary's batches every `ttl` s, `round_trip` apart, and visitor reads of 1 token at
    `visitor_rate` a second."""
    service = gateway.Gateway(network=NET)
    clock = [1000.0]
    events = []
    t = 0.0
    while t < seconds:
        for i, methods in enumerate(trace):
            events.append((t + i * round_trip, 'summary', cost(methods)))
        t += ttl
    step = 1.0 / visitor_rate
    events += [(k * step + step / 2, 'visitor', 1) for k in range(int(seconds * visitor_rate))]
    events.sort()
    refused = {'summary': 0, 'visitor': 0}
    low = 80.0
    with patch.object(gateway.time, 'monotonic', side_effect=lambda: clock[0]):
        service.tokens, service.token_time = 80.0, clock[0]
        for at, who, amount in events:
            clock[0] = 1000.0 + at
            if not service.charge(amount):
                refused[who] += 1
            low = min(low, service.tokens)
    return refused['summary'], refused['visitor'], low

def main():
    if shutil.which('php') is None:
        print('FAIL php-cli is required', file=sys.stderr)
        return 2
    ok = True
    def check(name, value):
        nonlocal ok
        ok &= bool(value)
        print(('PASS ' if value else 'FAIL ') + name)
    large, small = summary_trace(10000), summary_trace(5)
    check('the summary counts 10,000 members', large['eligible'] == 10000)
    check('two requests a refresh: status, params and the member counts, then 20 blocks',
          large['trace'] == [['pocol_status', 'pocol_getParams', 'pocol_getMemberCounts'], ['eth_getBlockByNumber'] * 20])
    check('no members page is read', 'pocol_getMembers' not in sum(large['trace'], []))
    per_refresh = sum(cost(m) for m in large['trace'])
    check(f'a refresh costs {per_refresh} gateway tokens at 10,000 members, as at 5', per_refresh == sum(cost(m) for m in small['trace']) == 42)
    check('every request of a refresh fits the 80-token burst', all(cost(m) <= 80 for m in large['trace']))
    summary_refused, visitors_refused, low = replay(large['trace'], large['ttl'])
    check('every summary request is accepted, refreshed every %d s for 5 minutes' % large['ttl'], summary_refused == 0)
    check('no explorer visitor read (10 a second, half the refill) is refused meanwhile', visitors_refused == 0)
    check(f'the bucket never falls below 30 tokens (lowest {low:.1f})', low >= 30)
    print('PASS' if ok else 'FAIL', 'network summary budget')
    return 0 if ok else 1

if __name__ == '__main__':
    sys.exit(main())
