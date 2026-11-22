<?php
declare(strict_types=1);

// Public, read-only summary. The browser never supplies RPC methods or an upstream URL.
const POCOL_SUMMARY_SCHEMA = 'pocol-network-summary-v1';
const POCOL_SUMMARY_CHAIN = 2656089850;
const POCOL_SUMMARY_GENESIS = '0x664dc53a2fa5e9e7c94ae3d735b1eefabb8630fcbabf5e8b0d34f5db0b8cc64e';
const POCOL_SUMMARY_SOURCE = 'https://explorer.pocol.net';
const POCOL_SUMMARY_RPC = 'https://explorer.pocol.net/api/rpc.php';
const POCOL_SUMMARY_SAFE_INTEGER = 9007199254740991;
const POCOL_SUMMARY_MAX_REPLY = 4194304;
const POCOL_SUMMARY_CACHE_TTL = 10;
const POCOL_SUMMARY_STALE_TTL = 300;
// SITE-05 and the 2026-11-22 review: the eligible count comes from the gateway's pocol_getMemberCounts (the ActList's
// counts at one head block, read by the gateway from MinerRegistry; deploy/rewards/gateway.py), not from the unpaged
// pocol_getMiners, whose reply grows with every record, nor from pages of pocol_getMembers: those cost 5 of the
// gateway's 80 shared request tokens each (refilled at 20 a second), 200 tokens at 10,000 members, more than the whole
// budget that explorer visitors share. A refresh now makes two requests: [status, params, counts] (22 tokens) and the
// 20 blocks (20 tokens), at most once per POCOL_SUMMARY_CACHE_TTL whatever the membership size.
const POCOL_SUMMARY_MAX_MEMBERS = 10000;
const POCOL_SUMMARY_BOND_STATES = ['NONE', 'HELD', 'BURNED', 'PAID', 'ACCRUING'];
// The time one refresh may take, every request included (each request's own limit is at most 12 s and never more than
// what is left of this).
const POCOL_SUMMARY_DEADLINE = 15;

function pocol_summary_integer($value): int {
    if (!is_int($value) || $value < 0 || $value > POCOL_SUMMARY_SAFE_INTEGER) {
        throw new RuntimeException('Invalid integer.');
    }
    return $value;
}

function pocol_summary_quantity($value): int {
    if (is_int($value)) return pocol_summary_integer($value);
    if (!is_string($value) || preg_match('/^(?:0x[0-9a-fA-F]{1,16}|0|[1-9][0-9]{0,15})$/D', $value) !== 1) {
        throw new RuntimeException('Invalid quantity.');
    }
    $number = str_starts_with(strtolower($value), '0x') ? hexdec(substr($value, 2)) : (int)$value;
    if (!is_int($number) || $number < 0 || $number > POCOL_SUMMARY_SAFE_INTEGER) {
        throw new RuntimeException('Quantity exceeds supported range.');
    }
    return $number;
}

function pocol_summary_hash($value): string {
    if (!is_string($value) || preg_match('/^0x[0-9a-fA-F]{64}$/D', $value) !== 1) {
        throw new RuntimeException('Invalid hash.');
    }
    return strtolower($value);
}

function pocol_summary_identity($value): void {
    if (!is_object($value) || ($value->chainId ?? null) !== POCOL_SUMMARY_CHAIN
        || pocol_summary_hash($value->genesisHash ?? null) !== POCOL_SUMMARY_GENESIS) {
        throw new RuntimeException('Unexpected network.');
    }
}

/** Validate complete batch envelopes and return results in request order. */
function pocol_summary_results(array $requests, $response): array {
    if (!is_array($response) || !array_is_list($response) || count($response) !== count($requests)) {
        throw new RuntimeException('Invalid RPC batch.');
    }
    $wanted = [];
    foreach ($requests as $request) $wanted[$request['id']] = true;
    $results = [];
    foreach ($response as $entry) {
        if (!is_object($entry) || ($entry->jsonrpc ?? null) !== '2.0'
            || !isset($entry->id) || !is_int($entry->id) || !isset($wanted[$entry->id])
            || array_key_exists($entry->id, $results)
            || !property_exists($entry, 'result') || property_exists($entry, 'error')) {
            throw new RuntimeException('Invalid or failed RPC response.');
        }
        $results[$entry->id] = $entry->result;
    }
    return array_map(static fn(array $request) => $results[$request['id']], $requests);
}

function pocol_summary_rpc(array $requests, float $timeout = 12.0): array {
    if (!extension_loaded('curl')) throw new RuntimeException('Data transport unavailable.');
    $milliseconds = max(1, (int)floor(min(12.0, $timeout) * 1000));
    $body = json_encode($requests, JSON_THROW_ON_ERROR);
    $reply = '';
    $handle = curl_init(POCOL_SUMMARY_RPC);
    if ($handle === false) throw new RuntimeException('Data transport unavailable.');
    try {
        curl_setopt_array($handle, [
            CURLOPT_POST => true,
            CURLOPT_POSTFIELDS => $body,
            CURLOPT_HTTPHEADER => ['Content-Type: application/json', 'Accept: application/json'],
            CURLOPT_CONNECTTIMEOUT_MS => min(10000, $milliseconds),
            CURLOPT_TIMEOUT_MS => $milliseconds,
            CURLOPT_FOLLOWLOCATION => false,
            CURLOPT_PROTOCOLS => CURLPROTO_HTTPS,
            CURLOPT_SSL_VERIFYPEER => true,
            CURLOPT_SSL_VERIFYHOST => 2,
            CURLOPT_WRITEFUNCTION => static function ($unused, string $chunk) use (&$reply): int {
                if (strlen($reply) + strlen($chunk) > POCOL_SUMMARY_MAX_REPLY) return 0;
                $reply .= $chunk;
                return strlen($chunk);
            },
        ]);
        if (curl_exec($handle) === false || curl_getinfo($handle, CURLINFO_HTTP_CODE) !== 200) {
            throw new RuntimeException('Data request failed.');
        }
        $decoded = json_decode($reply, false, 64, JSON_THROW_ON_ERROR | JSON_BIGINT_AS_STRING);
        return pocol_summary_results($requests, $decoded);
    } finally {
        curl_close($handle);
    }
}

function pocol_summary_call(int $id, string $method, array $params = []): array {
    return ['jsonrpc' => '2.0', 'id' => $id, 'method' => $method, 'params' => $params];
}

/** The eligible-miner count of a pocol_getMemberCounts answer, all at one block: every ActList member is live or a
 * tomb, the eligible ones (live and HELD (1) or ACCRUING (4), as the node counts them, prototypes/pocold/src/wallet.rs
 * status) are live members in one of those bond states, and the bond states add up to the membership. */
function pocol_summary_eligible($value): int {
    if (!is_object($value) || !is_object($value->bondStates ?? null)) throw new RuntimeException('Invalid member counts.');
    pocol_summary_integer($value->height ?? null);
    pocol_summary_hash($value->blockHash ?? null);
    $active = pocol_summary_integer($value->active ?? null);
    $live = pocol_summary_integer($value->live ?? null);
    $eligible = pocol_summary_integer($value->eligible ?? null);
    $tomb = pocol_summary_integer($value->tomb ?? null);
    $states = get_object_vars($value->bondStates);
    $names = array_keys($states); $known = POCOL_SUMMARY_BOND_STATES; sort($names); sort($known);
    if ($names !== $known) throw new RuntimeException('Invalid member counts.');
    $sum = 0;
    foreach ($states as $count) $sum += pocol_summary_integer($count);
    if ($active > POCOL_SUMMARY_MAX_MEMBERS) throw new RuntimeException('Unsupported membership size.');
    if ($live + $tomb !== $active || $sum !== $active || $eligible > $live
        || $eligible > $states['HELD'] + $states['ACCRUING']) throw new RuntimeException('Inconsistent member counts.');
    return $eligible;
}

/** A transport whose requests together keep to `$seconds`: each request is given what is left (at most 12 s), and none
 * starts once the time is spent. */
function pocol_summary_deadline(callable $rpc, float $seconds = POCOL_SUMMARY_DEADLINE, ?callable $clock = null): callable {
    $clock ??= static fn(): float => microtime(true);
    $end = $clock() + $seconds;
    return static function (array $requests) use ($rpc, $end, $clock) {
        $left = $end - $clock();
        if ($left <= 0) throw new RuntimeException('Summary time limit reached.');
        $values = $rpc($requests, min(12.0, $left));
        if ($clock() > $end) throw new RuntimeException('Summary time limit reached.');
        return $values;
    };
}

/** The injectable transport is only a PHP function argument, never a request parameter. */
function pocol_network_summary(callable $rpc, ?int $now = null): array {
    $initial = $rpc([
        pocol_summary_call(1, 'pocol_status'),
        pocol_summary_call(2, 'pocol_getParams'),
        pocol_summary_call(3, 'pocol_getMemberCounts'),
    ]);
    if (!is_array($initial) || count($initial) !== 3) throw new RuntimeException('Incomplete snapshot.');
    [$status, $params, $counts] = $initial;
    pocol_summary_identity($status);
    pocol_summary_identity($params);
    $eligible = pocol_summary_eligible($counts);
    if (!isset($status->head) || !is_object($status->head)) throw new RuntimeException('Missing head.');
    $height = pocol_summary_integer($status->head->height ?? null);
    $headHash = pocol_summary_hash($status->head->hash ?? null);
    $pending = pocol_summary_integer($status->pool ?? null);
    $count = min(20, $height + 1);
    $requests = [];
    for ($i = 0; $i < $count; $i++) {
        $requests[] = pocol_summary_call($i + 4, 'eth_getBlockByNumber', ['0x' . dechex($height - $i), false]);
    }
    $blocks = $rpc($requests);
    if (!is_array($blocks) || !array_is_list($blocks) || count($blocks) !== $count) {
        throw new RuntimeException('Incomplete blocks.');
    }
    $timestamps = [];
    $expectedHash = $headHash;
    foreach ($blocks as $i => $block) {
        if (!is_object($block) || pocol_summary_quantity($block->number ?? null) !== $height - $i
            || pocol_summary_hash($block->hash ?? null) !== $expectedHash) {
            throw new RuntimeException('Inconsistent chain snapshot.');
        }
        $timestamps[] = pocol_summary_quantity($block->timestamp ?? null);
        $expectedHash = pocol_summary_hash($block->parentHash ?? null);
    }
    $total = 0;
    for ($i = 1; $i < count($timestamps); $i++) $total += max(0, $timestamps[$i - 1] - $timestamps[$i]);
    return [
        'schema' => POCOL_SUMMARY_SCHEMA,
        'chainId' => POCOL_SUMMARY_CHAIN,
        'genesisHash' => POCOL_SUMMARY_GENESIS,
        'source' => POCOL_SUMMARY_SOURCE,
        'fetchedAt' => gmdate('Y-m-d\TH:i:s\Z', $now ?? time()),
        'stale' => false,
        'metrics' => [
            'blockHeight' => $height,
            'eligibleMiners' => $eligible,
            'averageBlockTime' => count($timestamps) > 1 ? $total / (count($timestamps) - 1) : null,
            'pendingTransactions' => $pending,
        ],
        'headTimestamp' => $timestamps[0],
    ];
}

function pocol_summary_valid_cache($data, int $now): bool {
    try {
        if (!is_array($data) || ($data['schema'] ?? null) !== POCOL_SUMMARY_SCHEMA
            || ($data['chainId'] ?? null) !== POCOL_SUMMARY_CHAIN
            || ($data['genesisHash'] ?? null) !== POCOL_SUMMARY_GENESIS
            || ($data['source'] ?? null) !== POCOL_SUMMARY_SOURCE || ($data['stale'] ?? null) !== false
            || !is_string($data['fetchedAt'] ?? null) || !is_array($data['metrics'] ?? null)) return false;
        $timestamp = strtotime($data['fetchedAt']);
        if ($timestamp === false || $timestamp > $now || $timestamp < $now - POCOL_SUMMARY_STALE_TTL) return false;
        foreach (['blockHeight', 'eligibleMiners', 'pendingTransactions'] as $key) pocol_summary_integer($data['metrics'][$key] ?? null);
        pocol_summary_integer($data['headTimestamp'] ?? null);
        if (!array_key_exists('averageBlockTime', $data['metrics'])) return false;
        $average = $data['metrics']['averageBlockTime'];
        return $average === null || ((is_int($average) || is_float($average)) && is_finite((float)$average) && $average >= 0);
    } catch (Throwable $error) {
        return false;
    }
}

function pocol_summary_read_cache(string $file, int $now): ?array {
    if (!is_file($file) || is_link($file)) return null;
    $raw = @file_get_contents($file, false, null, 0, 8193);
    if ($raw === false || strlen($raw) > 8192) return null;
    $data = json_decode($raw, true);
    return pocol_summary_valid_cache($data, $now) ? $data : null;
}

function pocol_network_cached(callable $loader, ?int $now = null, ?string $directory = null): array {
    $clock = static fn(): int => $now ?? time();
    $directory ??= sys_get_temp_dir() . '/pocol-summary-' . substr(hash('sha256', __FILE__), 0, 24);
    $previousMask = umask(0077);
    try {
        if (!is_dir($directory) && !@mkdir($directory, 0700) && !is_dir($directory)) {
            throw new RuntimeException('Summary cache unavailable.');
        }
        if (is_link($directory)) throw new RuntimeException('Summary cache unavailable.');
        // F16: as F15 — refuse a shared-temp cache directory that is not a real directory with no group or
        // other access (and, where POSIX is present, owned by this euid), so another local account cannot
        // pre-create it world-writable and control the cached figures or swap files inside it.
        $dirInfo = @lstat($directory);
        if ($dirInfo === false || ($dirInfo['mode'] & 0170000) !== 0040000 || ($dirInfo['mode'] & 0077) !== 0
            || (function_exists('posix_geteuid') && $dirInfo['uid'] !== posix_geteuid())) {
            throw new RuntimeException('Summary cache unavailable.');
        }
        $file = $directory . '/summary.json';
        $cached = pocol_summary_read_cache($file, $clock());
        $failureFile = $directory . '/failure.txt';
        $lastFailure = !is_link($failureFile) && is_file($failureFile)
            ? @file_get_contents($failureFile, false, null, 0, 32) : false;
        if (is_string($lastFailure) && preg_match('/^[0-9]{1,16}$/D', $lastFailure) === 1
            && (int)$lastFailure <= $clock() && $clock() - (int)$lastFailure < POCOL_SUMMARY_CACHE_TTL) {
            if ($cached !== null) return array_replace($cached, ['stale' => true]);
            throw new RuntimeException('Summary refresh deferred.');
        }
        if ($cached !== null && $clock() - strtotime($cached['fetchedAt']) < POCOL_SUMMARY_CACHE_TTL) return $cached;
        $lockFile = $directory . '/refresh.lock';
        if (is_link($lockFile)) throw new RuntimeException('Summary cache unavailable.');
        $lock = @fopen($lockFile, 'c');
        if ($lock === false) throw new RuntimeException('Summary cache unavailable.');
        try {
            if (!flock($lock, LOCK_EX | LOCK_NB)) {
                if ($cached !== null) return array_replace($cached, ['stale' => true]);
                throw new RuntimeException('Summary refresh in progress.');
            }
            // A request may have refreshed between our first read and taking the lock.
            $cached = pocol_summary_read_cache($file, $clock());
            if ($cached !== null && $clock() - strtotime($cached['fetchedAt']) < POCOL_SUMMARY_CACHE_TTL) return $cached;
            try {
                $snapshot = $loader();
                if (!pocol_summary_valid_cache($snapshot, $clock())) throw new RuntimeException('Invalid summary.');
                if (is_file($failureFile) && !is_link($failureFile)) @unlink($failureFile);
                $temporary = tempnam($directory, '.summary-');
                if ($temporary !== false) {
                    try {
                        if (file_put_contents($temporary, json_encode($snapshot, JSON_THROW_ON_ERROR)) !== false) @rename($temporary, $file);
                    } finally {
                        if (is_file($temporary)) @unlink($temporary);
                    }
                }
                return $snapshot;
            } catch (Throwable $error) {
                if (!is_link($failureFile)) @file_put_contents($failureFile, (string)$clock(), LOCK_EX);
                $cached = pocol_summary_read_cache($file, $clock());
                if ($cached !== null) return array_replace($cached, ['stale' => true]);
                throw $error;
            }
        } finally {
            flock($lock, LOCK_UN);
            fclose($lock);
        }
    } finally {
        umask($previousMask);
    }
}

function pocol_network_main(): void {
    ini_set('display_errors', '0');
    header('Content-Type: application/json; charset=utf-8');
    header('Cache-Control: no-store');
    header('X-Content-Type-Options: nosniff');
    if (($_SERVER['REQUEST_METHOD'] ?? '') !== 'GET') {
        header('Allow: GET');
        http_response_code(405);
        echo json_encode(['schema' => POCOL_SUMMARY_SCHEMA, 'error' => 'Use GET for the network summary.']);
        return;
    }
    try {
        $data = pocol_network_cached(static fn(): array => pocol_network_summary(pocol_summary_deadline('pocol_summary_rpc')));
        echo json_encode($data, JSON_THROW_ON_ERROR | JSON_UNESCAPED_SLASHES);
    } catch (Throwable $error) {
        http_response_code(503);
        header('Retry-After: 15');
        echo json_encode(['schema' => POCOL_SUMMARY_SCHEMA, 'error' => 'Network data is temporarily unavailable.']);
    }
}

if (!defined('POCOL_NETWORK_LIBRARY_ONLY')) pocol_network_main();
