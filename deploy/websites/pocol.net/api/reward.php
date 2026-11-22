<?php
declare(strict_types=1);

// Fixed read-only gateway operation; requests cannot supply RPC input or an upstream.
// F-74 (2026-11-22): the gateway's pocol_getBlockReward answers the v3 projection (deploy/rewards/pocol_rewards.py
// snapshot_v3, schema pocol-block-reward-v3); this proxy validates it and passes on only the summary the page shows.
const POCOL_REWARD_SCHEMA = 'pocol-block-reward-v3';
const POCOL_REWARD_GENESIS = '0x664dc53a2fa5e9e7c94ae3d735b1eefabb8630fcbabf5e8b0d34f5db0b8cc64e';
const POCOL_REWARD_UPSTREAM = 'https://explorer.pocol.net/rpc';
// A v3 answer carries the block's own flows and, for a final block, every pot it settles: the gateway's reply cap.
const POCOL_REWARD_MAX_REPLY = 4194304;
const POCOL_REWARD_UINT256_MAX = '115792089237316195423570985008687907853269984665640564039457584007913129639935';

function pocol_reward_fail(): never {
    throw new RuntimeException('Reward data is temporarily unavailable.');
}

function pocol_reward_compare(string $a, string $b): int {
    return strlen($a) === strlen($b) ? strcmp($a, $b) : strlen($a) <=> strlen($b);
}

/** Exact nonnegative decimal addition; no floating point or optional PHP extension. */
function pocol_reward_add(string $a, string $b): string {
    $i = strlen($a) - 1; $j = strlen($b) - 1; $carry = 0; $sum = '';
    while ($i >= 0 || $j >= 0 || $carry) {
        $n = ($i >= 0 ? ord($a[$i--]) - 48 : 0) + ($j >= 0 ? ord($b[$j--]) - 48 : 0) + $carry;
        $sum .= chr(48 + $n % 10); $carry = intdiv($n, 10);
    }
    return strrev($sum);
}

function pocol_reward_amount($v): bool {
    return is_string($v) && preg_match('/^(0|[1-9][0-9]{0,77})$/D', $v) === 1
        && pocol_reward_compare($v, POCOL_REWARD_UINT256_MAX) <= 0;
}

function pocol_reward_integer($v): bool {
    return is_int($v) && $v >= 0 && $v <= 9007199254740991;
}

function pocol_reward_address($v): bool {
    return is_string($v) && preg_match('/^0x[0-9a-f]{40}$/D', $v) === 1
        && $v !== '0x0000000000000000000000000000000000000000';
}

function pocol_reward_sum(array $values): string {
    $sum = '0';
    foreach ($values as $value) $sum = pocol_reward_add($sum, $value);
    return $sum;
}

/** The v3 summary the page shows: identity, the captured block, pot h and its projected split, and the projected
 * beneficiaries. Every amount is a canonical uint256 decimal and every identity the gateway's: the pot is issuance plus
 * tips less the proposer's tip share plus the carry in, and splits exactly into paid (including bond fills), burned and
 * carried. A final block opens no pot and projects nothing. Anything else is refused (fail closed). */
function pocol_reward_validate($v, ?int $now = null): array {
    $now ??= time();
    if (!is_object($v) || ($v->schema ?? null) !== POCOL_REWARD_SCHEMA
        || ($v->chainId ?? null) !== 2656089850 || ($v->genesisHash ?? null) !== POCOL_REWARD_GENESIS
        || ($v->specVersion ?? null) !== 2
        || (property_exists($v, 'source') && $v->source !== 'https://explorer.pocol.net') || ($v->status ?? null) !== 'pending'
        || !pocol_reward_integer($v->blockHeight ?? null) || $v->blockHeight < 1
        || !is_string($v->blockHash ?? null) || preg_match('/^0x[0-9a-f]{64}$/D', $v->blockHash) !== 1
        || !is_bool($v->final ?? null) || !is_bool($v->winnerAccruing ?? null)
        || ($v->final ? !property_exists($v, 'settlementHeight') || $v->settlementHeight !== null
            : !pocol_reward_integer($v->settlementHeight ?? null) || $v->settlementHeight <= $v->blockHeight)
        || !pocol_reward_integer($v->shareCount ?? null) || $v->shareCount > 256
        || !pocol_reward_integer($v->beneficiaryCount ?? null) || $v->beneficiaryCount > 258
        || !is_array($v->beneficiaries ?? null) || !array_is_list($v->beneficiaries)
        || count($v->beneficiaries) !== $v->beneficiaryCount || !is_string($v->fetchedAt ?? null)
        || !property_exists($v, 'closing')) pocol_reward_fail();
    $stamp = strtotime($v->fetchedAt);
    if ($stamp === false || $stamp > $now + 10 || $now - $stamp > 60) pocol_reward_fail();
    $keys = ['issuedWei','tipsWei','tipShareWei','carryInWei','grossRewardWei','projectedPaidWei','projectedBurnedWei',
        'projectedBondWei','projectedCarryWei'];
    foreach ($keys as $key) if (!pocol_reward_amount($v->$key ?? null)) pocol_reward_fail();
    if (pocol_reward_compare($v->tipShareWei, $v->tipsWei) > 0
        || pocol_reward_sum([$v->projectedPaidWei, $v->projectedBurnedWei, $v->projectedCarryWei]) !== $v->grossRewardWei
        || pocol_reward_compare($v->projectedBondWei, $v->projectedPaidWei) > 0) pocol_reward_fail();
    // The final block opens no pot and takes no tip share: its tips and carries burn in finalize (block flows).
    if ($v->final ? $v->grossRewardWei !== '0' || $v->issuedWei !== '0' || $v->tipShareWei !== '0' || $v->beneficiaryCount !== 0
        : pocol_reward_add($v->grossRewardWei, $v->tipShareWei) !== pocol_reward_sum([$v->issuedWei, $v->tipsWei, $v->carryInWei])) pocol_reward_fail();
    $closing = null;
    if ($v->closing !== null) {
        $c = $v->closing;
        if (!is_object($c) || !pocol_reward_integer($c->closeHeight ?? null) || !is_bool($c->cursorDone ?? null)) pocol_reward_fail();
        $closing = ['closeHeight'=>$c->closeHeight, 'cursorDone'=>$c->cursorDone];
        if (property_exists($c, 'earliestFinalHeight') || property_exists($c, 'settlementMayBeFinal')) {
            if (!pocol_reward_integer($c->earliestFinalHeight ?? null) || !is_bool($c->settlementMayBeFinal ?? null)) pocol_reward_fail();
            $closing += ['earliestFinalHeight'=>$c->earliestFinalHeight, 'settlementMayBeFinal'=>$c->settlementMayBeFinal];
        }
    }
    $paid = '0'; $bond = '0'; $ids = []; $beneficiaries = [];
    foreach ($v->beneficiaries as $b) {
        if (!is_object($b) || !pocol_reward_address($b->minerId ?? null)
            || !pocol_reward_address($b->rewardAddress ?? null) || !pocol_reward_amount($b->amountWei ?? null)) pocol_reward_fail();
        $filled = property_exists($b, 'bondAccruedWei') ? $b->bondAccruedWei : '0';
        $pair = $b->minerId . $b->rewardAddress;
        if (!pocol_reward_amount($filled) || (property_exists($b, 'bondAccruedWei') && $filled === '0')
            || ($b->amountWei === '0' && $filled === '0') || isset($ids[$pair])) pocol_reward_fail();
        $ids[$pair] = true; $paid = pocol_reward_sum([$paid, $b->amountWei, $filled]); $bond = pocol_reward_add($bond, $filled);
        $beneficiaries[] = ['minerId'=>$b->minerId, 'rewardAddress'=>$b->rewardAddress, 'amountWei'=>$b->amountWei]
            + ($filled === '0' ? [] : ['bondAccruedWei'=>$filled]);
    }
    if ($paid !== $v->projectedPaidWei || $bond !== $v->projectedBondWei) pocol_reward_fail();
    $result = [];
    foreach (['schema','chainId','genesisHash','specVersion','fetchedAt','blockHeight','blockHash','final','settlementHeight',
        ...$keys, 'winnerAccruing','beneficiaryCount','status','shareCount'] as $key) {
        $result[$key] = $v->$key;
    }
    if (property_exists($v, 'source')) $result['source'] = $v->source;
    $result['closing'] = $closing;
    $result['beneficiaries'] = $beneficiaries;
    return $result;
}

function pocol_reward_envelope(string $raw, ?int $now = null): array {
    if (strlen($raw) > POCOL_REWARD_MAX_REPLY) pocol_reward_fail();
    $reply = json_decode($raw, false, 32, JSON_THROW_ON_ERROR | JSON_BIGINT_AS_STRING);
    if (!is_object($reply) || ($reply->jsonrpc ?? null) !== '2.0' || ($reply->id ?? null) !== 1
        || property_exists($reply, 'error') || !property_exists($reply, 'result')) pocol_reward_fail();
    return pocol_reward_validate($reply->result, $now);
}

function pocol_reward_fetch(): array {
    if (!extension_loaded('curl')) pocol_reward_fail();
    $handle = curl_init(POCOL_REWARD_UPSTREAM);
    if ($handle === false) pocol_reward_fail();
    $raw = '';
    try {
        curl_setopt_array($handle, [
            CURLOPT_POST=>true,
            CURLOPT_POSTFIELDS=>' {"jsonrpc":"2.0","id":1,"method":"pocol_getBlockReward","params":[]}',
            CURLOPT_HTTPHEADER=>['Content-Type: application/json','Accept: application/json'],
            CURLOPT_CONNECTTIMEOUT=>5, CURLOPT_TIMEOUT=>30,
            CURLOPT_FOLLOWLOCATION=>false, CURLOPT_PROTOCOLS=>CURLPROTO_HTTPS,
            CURLOPT_SSL_VERIFYPEER=>true, CURLOPT_SSL_VERIFYHOST=>2,
            CURLOPT_WRITEFUNCTION=>static function ($unused, string $chunk) use (&$raw): int {
                if (strlen($raw) + strlen($chunk) > POCOL_REWARD_MAX_REPLY) return 0;
                $raw .= $chunk; return strlen($chunk);
            },
        ]);
        if (curl_exec($handle) === false || curl_getinfo($handle, CURLINFO_HTTP_CODE) !== 200) pocol_reward_fail();
        return pocol_reward_envelope($raw);
    } finally { curl_close($handle); }
}

/** Three-second cache and nonblocking refresh lock. Expired data is never served. */
function pocol_reward_cached(callable $loader, ?int $now = null, ?string $directory = null): array {
    $clock = static fn(): int => $now ?? time();
    $directory ??= sys_get_temp_dir() . '/pocol-reward-' . substr(hash('sha256', __FILE__), 0, 24);
    $mask = umask(0077);
    try {
        if (is_link($directory) || (!is_dir($directory) && !@mkdir($directory, 0700) && !is_dir($directory))) pocol_reward_fail();
        // F15: a cache directory in shared temp may have been pre-created by another local account. Require
        // a real directory with no group/other access (and, where POSIX is present, owned by this euid), so
        // no other account can seed a snapshot there or swap a file inside it for a symlink; fail closed.
        $dirInfo = @lstat($directory);
        if ($dirInfo === false || ($dirInfo['mode'] & 0170000) !== 0040000 || ($dirInfo['mode'] & 0077) !== 0
            || (function_exists('posix_geteuid') && $dirInfo['uid'] !== posix_geteuid())) pocol_reward_fail();
        $file = $directory . '/snapshot.json'; $lockFile = $directory . '/refresh.lock'; $failure = $directory . '/failure.txt';
        foreach ([$file,$lockFile,$failure] as $path) if (is_link($path)) pocol_reward_fail();
        $readCache = static function () use ($file, $clock): ?array {
            if (!is_file($file)) return null;
            $raw = @file_get_contents($file, false, null, 0, POCOL_REWARD_MAX_REPLY + 1);
            if ($raw === false || strlen($raw) > POCOL_REWARD_MAX_REPLY) return null;
            try {
                $saved = json_decode($raw, false, 32, JSON_THROW_ON_ERROR | JSON_BIGINT_AS_STRING);
                if (!is_object($saved) || !is_int($saved->cachedAt ?? null) || $saved->cachedAt > $clock()
                    || $clock() - $saved->cachedAt >= 3) return null;
                return pocol_reward_validate($saved->snapshot ?? null, $clock());
            } catch (Throwable $error) { return null; }
        };
        $cached = $readCache(); if ($cached !== null) return $cached;
        $lastFailure = is_file($failure) ? @file_get_contents($failure, false, null, 0, 32) : false;
        if (is_string($lastFailure) && preg_match('/^[0-9]{1,16}$/D', $lastFailure) === 1
            && (int)$lastFailure <= $clock() && $clock() - (int)$lastFailure < 3) pocol_reward_fail();
        $lock = @fopen($lockFile, 'c'); if ($lock === false) pocol_reward_fail();
        try {
            if (!flock($lock, LOCK_EX | LOCK_NB)) pocol_reward_fail();
            $cached = $readCache(); if ($cached !== null) return $cached;
            try {
                $loaded = $loader();
                $snapshot = pocol_reward_validate(json_decode(json_encode($loaded, JSON_THROW_ON_ERROR)), $clock());
                $temp = tempnam($directory, '.snapshot-');
                if ($temp !== false) {
                    try {
                        if (file_put_contents($temp, json_encode(['cachedAt'=>$clock(),'snapshot'=>$snapshot], JSON_THROW_ON_ERROR)) !== false) @rename($temp, $file);
                    } finally { if (is_file($temp)) @unlink($temp); }
                }
                if (is_file($failure)) @unlink($failure);
                return $snapshot;
            } catch (Throwable $error) {
                @file_put_contents($failure, (string)$clock(), LOCK_EX); throw $error;
            }
        } finally { flock($lock, LOCK_UN); fclose($lock); }
    } finally { umask($mask); }
}

function pocol_reward_main(?callable $loader = null): void {
    ini_set('display_errors', '0');
    header('Content-Type: application/json; charset=utf-8');
    header('Cache-Control: no-store'); header('X-Content-Type-Options: nosniff');
    if (($_SERVER['REQUEST_METHOD'] ?? '') !== 'GET') {
        http_response_code(405); header('Allow: GET');
        echo json_encode(['schema'=>POCOL_REWARD_SCHEMA,'error'=>'Use GET.']); return;
    }
    try {
        $data = $loader !== null ? $loader() : pocol_reward_cached('pocol_reward_fetch');
        $data = pocol_reward_validate(json_decode(json_encode($data, JSON_THROW_ON_ERROR)));
        http_response_code(200); echo json_encode($data, JSON_THROW_ON_ERROR | JSON_UNESCAPED_SLASHES);
    } catch (Throwable $error) {
        http_response_code(503); header('Retry-After: 3');
        echo json_encode(['schema'=>POCOL_REWARD_SCHEMA,'error'=>'Reward data is temporarily unavailable.','code'=>'reward_unavailable']);
    }
}

if (!defined('POCOL_REWARD_LIBRARY_ONLY')) pocol_reward_main();
