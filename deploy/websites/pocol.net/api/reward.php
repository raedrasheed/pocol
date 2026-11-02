<?php
declare(strict_types=1);

// Fixed read-only gateway operation; requests cannot supply RPC input or an upstream.
const POCOL_REWARD_SCHEMA = 'pocol-block-reward-v1';
const POCOL_REWARD_GENESIS = '0xbe137bfdb4fdf332f2e1500ee1a67e0b22ce6bdf57efa46fb4ea7b0debe509ca';
const POCOL_REWARD_UPSTREAM = 'https://gateway.pocol.net/rpc';
const POCOL_REWARD_MAX_REPLY = 262144;
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

function pocol_reward_validate($v, ?int $now = null): array {
    $now ??= time();
    if (!is_object($v) || ($v->schema ?? null) !== POCOL_REWARD_SCHEMA
        || ($v->chainId ?? null) !== 1599846715 || ($v->genesisHash ?? null) !== POCOL_REWARD_GENESIS
        || ($v->source ?? null) !== 'https://explorer.pocol.net' || ($v->status ?? null) !== 'pending'
        || !pocol_reward_integer($v->blockHeight ?? null) || $v->blockHeight < 1
        || !is_string($v->blockHash ?? null) || preg_match('/^0x[0-9a-f]{64}$/D', $v->blockHash) !== 1
        || !pocol_reward_integer($v->settlementHeight ?? null) || $v->settlementHeight <= $v->blockHeight
        || !pocol_reward_integer($v->shareCount ?? null) || $v->shareCount > 256
        || !pocol_reward_integer($v->beneficiaryCount ?? null) || $v->beneficiaryCount > 258
        || !is_array($v->beneficiaries ?? null) || !array_is_list($v->beneficiaries)
        || count($v->beneficiaries) !== $v->beneficiaryCount || !is_string($v->fetchedAt ?? null)) pocol_reward_fail();
    $stamp = strtotime($v->fetchedAt);
    if ($stamp === false || $stamp > $now + 10 || $now - $stamp > 60) pocol_reward_fail();
    foreach (['subsidyWei','grossRewardWei','projectedPaidWei','projectedBurnedWei'] as $key) {
        if (!pocol_reward_amount($v->$key ?? null)) pocol_reward_fail();
    }
    if (pocol_reward_compare($v->subsidyWei, $v->grossRewardWei) > 0
        || pocol_reward_add($v->projectedPaidWei, $v->projectedBurnedWei) !== $v->grossRewardWei) pocol_reward_fail();
    $sum = '0'; $ids = []; $beneficiaries = [];
    foreach ($v->beneficiaries as $b) {
        if (!is_object($b) || !pocol_reward_address($b->minerId ?? null)
            || !pocol_reward_address($b->rewardAddress ?? null) || !pocol_reward_amount($b->amountWei ?? null)
            || $b->amountWei === '0' || isset($ids[$b->minerId])) pocol_reward_fail();
        $ids[$b->minerId] = true; $sum = pocol_reward_add($sum, $b->amountWei);
        $beneficiaries[] = ['minerId'=>$b->minerId, 'rewardAddress'=>$b->rewardAddress, 'amountWei'=>$b->amountWei];
    }
    if ($sum !== $v->projectedPaidWei) pocol_reward_fail();
    $result = [];
    foreach (['schema','chainId','genesisHash','source','fetchedAt','blockHeight','blockHash','settlementHeight',
        'subsidyWei','grossRewardWei','projectedPaidWei','projectedBurnedWei','beneficiaryCount','status','shareCount'] as $key) {
        $result[$key] = $v->$key;
    }
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
