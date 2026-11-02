<?php
declare(strict_types=1);

const POCOL_MAX_BODY = 16384;
const POCOL_MAX_REPLY = 4194304;
const POCOL_MAX_ID = 9007199254740991;

function pocol_json($value, int $status = 200): void {
    http_response_code($status);
    header('Content-Type: application/json; charset=utf-8');
    header('Cache-Control: no-store');
    header('X-Content-Type-Options: nosniff');
    echo json_encode($value, JSON_UNESCAPED_SLASHES | JSON_INVALID_UTF8_SUBSTITUTE);
}

function pocol_hash($s): bool {
    return is_string($s) && preg_match('/^0x[0-9a-fA-F]{64}$/D', $s) === 1;
}

function pocol_tag($s): bool {
    return $s === 'latest' || (is_string($s) && preg_match('/^0x[0-9a-fA-F]{1,16}$/D', $s) === 1);
}

function pocol_valid_call($r): bool {
    if (!is_object($r) || ($r->jsonrpc ?? null) !== '2.0' || !isset($r->id)
        || !is_int($r->id) || abs($r->id) > POCOL_MAX_ID
        || !isset($r->method) || !is_string($r->method)
        || !isset($r->params) || !is_array($r->params)) return false;
    $p = $r->params;
    switch ($r->method) {
        case 'pocol_status': case 'pocol_getParams': case 'pocol_getMiners':
        case 'eth_blockNumber': case 'net_peerCount': case 'eth_gasPrice':
            return count($p) === 0;
        case 'eth_getTransactionByHash': case 'eth_getTransactionReceipt':
            return count($p) === 1 && pocol_hash($p[0]);
        case 'eth_getBalance': case 'eth_getTransactionCount': case 'eth_getCode':
            return count($p) === 2 && is_string($p[0])
                && preg_match('/^0x[0-9a-fA-F]{40}$/D', $p[0]) === 1 && pocol_tag($p[1]);
        case 'eth_getBlockByNumber':
            return count($p) === 2 && pocol_tag($p[0]) && is_bool($p[1]);
        case 'eth_getBlockByHash':
            return count($p) === 2 && pocol_hash($p[0]) && is_bool($p[1]);
    }
    return false;
}

function pocol_valid_payload($v): bool {
    $calls = is_array($v) ? $v : [$v];
    if (count($calls) < 1 || count($calls) > 20) return false;
    $ids = [];
    foreach ($calls as $r) {
        if (!pocol_valid_call($r) || isset($ids[(string)$r->id])) return false;
        $ids[(string)$r->id] = true;
    }
    return true;
}

function pocol_valid_response($v, $request): bool {
    if (is_array($request) !== is_array($v)) return false;
    $calls = is_array($request) ? $request : [$request];
    $results = is_array($v) ? $v : [$v];
    if (count($calls) !== count($results)) return false;
    $ids = [];
    foreach ($calls as $call) $ids[(string)$call->id] = true;
    foreach ($results as $r) {
        if (!is_object($r) || ($r->jsonrpc ?? null) !== '2.0' || !isset($r->id)
            || !is_int($r->id) || !isset($ids[(string)$r->id])) return false;
        $result = property_exists($r, 'result');
        $error = property_exists($r, 'error');
        if ($result === $error) return false;
        if ($error && (!is_object($r->error) || !is_int($r->error->code ?? null)
            || !is_string($r->error->message ?? null))) return false;
        unset($ids[(string)$r->id]);
    }
    return !$ids;
}

// Four local outbound slots. The Linux gateway independently applies its rate,
// concurrency, cache and response budgets. No state is stored in the web root.
function pocol_slot() {
    $scope = substr(hash('sha256', __DIR__), 0, 24);
    for ($i = 0; $i < 4; $i++) {
        $path = sys_get_temp_dir() . '/pocol-explorer-' . $scope . '-' . $i . '.lock';
        $old = umask(0077);
        $handle = @fopen($path, 'c');
        umask($old);
        if (!$handle) continue;
        if (flock($handle, LOCK_EX | LOCK_NB)) return $handle;
        fclose($handle);
    }
    return false;
}

function pocol_forward(string $body, $request, array $config): array {
    if (!extension_loaded('curl')) return [503, null, 'Enable the PHP cURL extension for this domain.'];
    $url = $config['gateway_url'] ?? '';
    $parts = parse_url($url);
    if (!$parts || !in_array($parts['scheme'] ?? '', ['http', 'https'], true)
        || empty($parts['host']) || isset($parts['user']) || isset($parts['pass'])
        || isset($parts['fragment'])) return [503, null, 'The operator gateway configuration is invalid.'];
    $slot = pocol_slot();
    if (!$slot) return [429, null, 'The data service is busy. Please retry shortly.'];
    $ch = curl_init($url);
    $response = '';
    try {
        curl_setopt_array($ch, [
            CURLOPT_POST => true,
            CURLOPT_POSTFIELDS => $body,
            CURLOPT_HTTPHEADER => ['Content-Type: application/json', 'Accept: application/json'],
            CURLOPT_CONNECTTIMEOUT => 3,
            CURLOPT_TIMEOUT => 9,
            CURLOPT_FOLLOWLOCATION => false,
            CURLOPT_PROTOCOLS => CURLPROTO_HTTP | CURLPROTO_HTTPS,
            CURLOPT_SSL_VERIFYPEER => true,
            CURLOPT_SSL_VERIFYHOST => 2,
            CURLOPT_WRITEFUNCTION => static function ($handle, string $chunk) use (&$response): int {
                if (strlen($response) + strlen($chunk) > POCOL_MAX_REPLY) return 0;
                $response .= $chunk;
                return strlen($chunk);
            },
        ]);
        $ok = curl_exec($ch);
        $status = curl_getinfo($ch, CURLINFO_HTTP_CODE);
        if ($ok === false || $status < 200 || $status >= 300) {
            return [503, null, 'The PoCol gateway request failed. Check gateway_url in api/config.php, the gateway service, and hosting access to the configured endpoint.'];
        }
        $decoded = json_decode($response, false, 64, JSON_BIGINT_AS_STRING);
        if (json_last_error() !== JSON_ERROR_NONE || !pocol_valid_response($decoded, $request)) {
            return [502, null, 'The PoCol gateway returned an invalid response.'];
        }
        // Forward the original bytes to preserve all exact on-chain values.
        return [200, $response, null];
    } finally {
        curl_close($ch);
        flock($slot, LOCK_UN);
        fclose($slot);
    }
}
