<?php
declare(strict_types=1);
ini_set('display_errors', '0');
require __DIR__ . '/lib.php';
$config = require __DIR__ . '/config.php';
if (($_SERVER['REQUEST_METHOD'] ?? '') !== 'GET') {
    header('Allow: GET');
    pocol_json(['error' => 'Use GET for the connection check.'], 405);
    exit;
}
$request = (object)['jsonrpc' => '2.0', 'id' => 1, 'method' => 'pocol_getParams', 'params' => []];
[$status, $raw, $error] = pocol_forward(json_encode($request), $request, $config);
$params = $raw ? (json_decode($raw)->result ?? null) : null;
$match = is_object($params) && ($params->chainId ?? null) === $config['chain_id']
    && strtolower($params->genesisHash ?? '') === $config['genesis_hash'];
pocol_json([
    'service' => 'PoCol Explorer',
    'phpReady' => PHP_VERSION_ID >= 80100,
    'curlReady' => extension_loaded('curl'),
    'gatewayReachable' => $status === 200,
    'networkMatch' => $match,
    'result' => $match ? 'PASS' : 'NOT_CONNECTED',
    'message' => $match ? 'Expected PoCol network reached. Check head freshness in Explorer separately.'
        : ($error ?? 'The gateway does not report the expected PoCol network.'),
], $match ? 200 : ($status === 429 ? 429 : 503));
