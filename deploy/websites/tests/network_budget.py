#!/usr/bin/env python3
"""pocol.net's network summary (api/network.php) against the explorer gateway's own request budget (2026-11-22 review).

The summary's reads go through explorer.pocol.net/api/rpc.php, which charges the gateway's single token bucket
(deploy/rewards/gateway.py Gateway.charge: 80 tokens, refilled at 20 a second, shared by every explorer visitor; the
cost of each call is gateway.COSTS, as explorer_api.Relay is built with). This script runs pocol_network_summary in PHP
at the largest supported membership (10,000 members), records the batches it sends, and replays them through the real
Gateway.charge on a virtual clock: one refresh every POCOL_SUMMARY_CACHE_TTL seconds (the cache allows no more) while
explorer visitors read at half the refill rate. Every summary request must be accepted, no visitor read refused, and
a refresh must cost the same at 10,000 members as at 5. The gateway also budgets each client (EXP-02: CLIENT_BURST 40,
CLIENT_RATE 10 a second) and pocol.net's server is one client, so the replay charges the summary's requests to that
client too: a request refused there is resent after its Retry-After (3 s), at most twice, as api/network.php's
pocol_summary_deadline does, and every refresh must complete (integration, 2026-11-22).

Final review (2026-11-22): pocol.net's reward page (api/reward.php) reads pocol_getBlockReward (20 tokens) from /rpc
through the same client, once per its cache lifetime under steady visitors. The replay now charges those reads too, at
the cache lifetime and busy resend that reward.php defines (POCOL_REWARD_CACHE_TTL, POCOL_REWARD_BUSY_RETRIES), at every
phase between the two pages' refreshes, and requires that no reward read is refused. Before the review (3-second
cache, no resend) about a fifth of the reward reads were refused.

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
REWARD = ROOT / 'deploy' / 'websites' / 'pocol.net' / 'api' / 'reward.php'
# reward.php's cache lifetime and busy resend; a reward.php without the constants (before the final review) had a
# 3-second cache, a 3-second pause after a failure, and no resend.
REWARD_DRIVER = r"""<?php
declare(strict_types=1);
define('POCOL_REWARD_LIBRARY_ONLY', true);
require $argv[1];
$c = static fn(string $name, $default) => defined($name) ? constant($name) : $default;
echo json_encode(['ttl'=>$c('POCOL_REWARD_CACHE_TTL', 3), 'pause'=>$c('POCOL_REWARD_FAILURE_PAUSE', 3), 'retries'=>$c('POCOL_REWARD_BUSY_RETRIES', 0),
    'wait'=>(float)$c('POCOL_REWARD_BUSY_WAIT_MAX', 3.0), 'method'=>'pocol_getBlockReward']);
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

def reward_settings():
    with tempfile.TemporaryDirectory() as tmp:
        driver = Path(tmp) / 'reward.php'
        driver.write_text(REWARD_DRIVER, encoding='utf-8')
        run = subprocess.run(['php', str(driver), str(REWARD)], capture_output=True, text=True, timeout=120)
    if run.returncode != 0:
        raise SystemExit(f'FAIL reward.php did not load: {run.stderr[-400:]}')
    return json.loads(run.stdout)

def cost(methods):
    return sum(gateway.COSTS.get(m, 1) for m in methods)

SERVER = 'pocol.net server'   # pocol.net's PHP reaches /api/rpc.php from one address: one client of the gateway
BUSY_WAIT, BUSY_RETRIES = 3.0, 2  # the gateway's Retry-After on a budget refusal; api/network.php's retries

def replay(trace, ttl, seconds=300.0, visitor_rate=10.0, round_trip=0.2, retry=True):
    """(refused summary requests, refused visitor reads, lowest bucket level, resent summary requests) of `seconds` of
    traffic through one real Gateway's charge(): the summary's batches every `ttl` s, each `round_trip` after the one
    before, charged to the SERVER client as well, and visitor reads of 1 token at `visitor_rate` a second (each from its
    own address). A summary request refused is resent BUSY_WAIT later, at most BUSY_RETRIES times, when `retry`; one
    still refused then counts as refused."""
    service = gateway.Gateway(network=NET)
    clock = [1000.0]
    events = []
    t = 0.0
    while t < seconds:
        for i, methods in enumerate(trace):
            events.append((t + i * round_trip, 'summary', cost(methods), 0))
        t += ttl
    step = 1.0 / visitor_rate
    events += [(k * step + step / 2, 'visitor', 1, 0) for k in range(int(seconds * visitor_rate))]
    refused = {'summary': 0, 'visitor': 0}
    resent = 0
    low = 80.0
    with patch.object(gateway.time, 'monotonic', side_effect=lambda: clock[0]):
        service.tokens, service.token_time = 80.0, clock[0]
        visitor = 0
        while events:
            events.sort()
            at, who, amount, tries = events.pop(0)
            clock[0] = 1000.0 + at
            if who == 'summary':
                accepted = service.charge(amount, client=SERVER)
            else:
                visitor += 1
                accepted = service.charge(amount, client=f'visitor {visitor % 50}')
            if not accepted:
                if who == 'summary' and retry and tries < BUSY_RETRIES:
                    events.append((at + BUSY_WAIT, who, amount, tries + 1)); resent += 1
                else:
                    refused[who] += 1
            low = min(low, service.tokens)
    return refused['summary'], refused['visitor'], low, resent

def replay_both(trace, ttl, reward, offset, seconds=300.0, visitor_rate=10.0, round_trip=0.2):
    """(refused reward reads, reward reads, refused summary requests, refused visitor reads) of `seconds` of both pages'
    traffic through one real Gateway: the summary as replay() sends it (from t=0, resent after a busy answer), and
    reward.php's read from t=`offset`: under steady visitors the cache is refreshed as soon as it expires, so the next
    read follows `reward['ttl']` s after an accepted one; a busy read is resent after its wait, at most
    `reward['retries']` times; a refused read pauses reads for `reward['pause']` s (reward.php's failure file). The
    reward read goes to /rpc, so it may not take the node's bucket below gateway.API_RESERVE."""
    service = gateway.Gateway(network=NET)
    clock = [1000.0]
    events, t = [], 0.0
    while t < seconds:
        for i, methods in enumerate(trace):
            events.append((t + i * round_trip, 'summary', cost(methods), 0))
        t += ttl
    events.append((offset, 'reward', gateway.COSTS[reward['method']], 0))
    step = 1.0 / visitor_rate
    events += [(k * step + step / 2, 'visitor', 1, 0) for k in range(int(seconds * visitor_rate))]
    refused = {'summary': 0, 'visitor': 0, 'reward': 0}
    reads = 0
    with patch.object(gateway.time, 'monotonic', side_effect=lambda: clock[0]):
        service.tokens, service.token_time = 80.0, clock[0]
        visitor = 0
        while events:
            events.sort()
            at, who, amount, tries = events.pop(0)
            if at >= seconds and who == 'reward':
                continue
            clock[0] = 1000.0 + at
            if who == 'summary':
                accepted = service.charge(amount, client=SERVER)
                if not accepted:
                    if tries < BUSY_RETRIES: events.append((at + BUSY_WAIT, who, amount, tries + 1))
                    else: refused['summary'] += 1
            elif who == 'reward':
                accepted = service.charge(amount, client=SERVER, reserve=gateway.API_RESERVE)
                if tries == 0: reads += 1
                if accepted:
                    events.append((at + round_trip + reward['ttl'], 'reward', amount, 0))
                elif tries < reward['retries']:
                    events.append((at + min(reward['wait'], max(1.0, BUSY_WAIT)), 'reward', amount, tries + 1))
                else:
                    refused['reward'] += 1
                    events.append((at + reward['pause'], 'reward', amount, 0))
            else:
                visitor += 1
                if not service.charge(amount, client=f'visitor {visitor % 50}'): refused['visitor'] += 1
    return refused['reward'], reads, refused['summary'], refused['visitor']

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
    summary_refused, visitors_refused, low, resent = replay(large['trace'], large['ttl'])
    check('every summary request is accepted, refreshed every %d s for 5 minutes (%d resent after a busy answer)'
          % (large['ttl'], resent), summary_refused == 0)
    # The detector: on a fast link the second request follows the first too closely for the server's client bucket
    # (40 - 22 = 18 < 20 tokens), so without the resend every refresh would fail.
    fast_refused, _, _, _ = replay(large['trace'], large['ttl'], round_trip=0.05, retry=False)
    check(f'without the resend the per-client budget refuses refreshes ({fast_refused} refused at a 50 ms round trip)', fast_refused > 0)
    fast_retried, _, _, _ = replay(large['trace'], large['ttl'], round_trip=0.05)
    check('with the resend every refresh completes at a 50 ms round trip too', fast_retried == 0)
    check('no explorer visitor read (10 a second, half the refill) is refused meanwhile', visitors_refused == 0)
    check(f'the bucket never falls below 30 tokens (lowest {low:.1f})', low >= 30)
    # Final review: both pages of pocol.net's server, at 50 phases between the reward read and the summary refresh.
    reward = reward_settings()
    worst = (0, 0, 0, 0)
    totals = [0, 0, 0, 0]
    for k in range(50):
        r = replay_both(large['trace'], large['ttl'], reward, offset=k * large['ttl'] / 50)
        totals = [a + b for a, b in zip(totals, r)]
        if r[0] > worst[0]: worst = r
    check(f"no reward read is refused with the summary alongside ({totals[0]} of {totals[1]} refused over 50 phases; "
          f"cache {reward['ttl']} s, {reward['retries']} busy resend)", totals[0] == 0)
    check('every summary request is still accepted alongside the reward reads', totals[2] == 0)
    check('no explorer visitor read is refused alongside both pages', totals[3] == 0)
    old_refused, old_reads, _, _ = (lambda rs: [sum(x) for x in zip(*rs)])([replay_both(large['trace'], large['ttl'], {**reward, 'ttl': 3, 'retries': 0, 'pause': 3}, offset=k * large['ttl'] / 50) for k in range(50)])
    check(f'the detector: a 3-second reward cache without a resend has refusals ({old_refused} of {old_reads})', old_refused > 0)
    print('PASS' if ok else 'FAIL', 'network summary budget')
    return 0 if ok else 1

if __name__ == '__main__':
    sys.exit(main())
