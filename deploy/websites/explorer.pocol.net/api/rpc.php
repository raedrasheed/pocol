<?php
declare(strict_types=1);
ini_set('display_errors', '0');
require __DIR__ . '/lib.php';
$config = require __DIR__ . '/config.php';

if (($_SERVER['REQUEST_METHOD'] ?? '') !== 'POST') {
    header('Allow: POST');
    pocol_json(['error' => 'Use POST for read-only explorer queries.'], 405);
    exit;
}
$origin = $_SERVER['HTTP_ORIGIN'] ?? null;
if ($origin !== null && $origin !== $config['site_origin']) {
    pocol_json(['error' => 'Cross-origin requests are not enabled.'], 403);
    exit;
}
if (strtolower(trim(explode(';', $_SERVER['CONTENT_TYPE'] ?? '')[0])) !== 'application/json') {
    pocol_json(['error' => 'Content-Type must be application/json.'], 415);
    exit;
}
$body = file_get_contents('php://input', false, null, 0, POCOL_MAX_BODY + 1);
if ($body === false || strlen($body) > POCOL_MAX_BODY) {
    pocol_json(['error' => 'Request size limit exceeded.'], 413);
    exit;
}
$request = json_decode($body, false, 32, JSON_BIGINT_AS_STRING);
if (json_last_error() !== JSON_ERROR_NONE || !pocol_valid_payload($request)) {
    pocol_json(['error' => 'Only supported, bounded explorer reads are accepted.'], 400);
    exit;
}
[$status, $response, $error] = pocol_forward($body, $request, $config);
if ($status !== 200) {
    if ($status === 429) header('Retry-After: 3');
    pocol_json(['error' => $error], $status);
    exit;
}
header('Content-Type: application/json; charset=utf-8');
header('Cache-Control: no-store');
header('X-Content-Type-Options: nosniff');
echo $response;
