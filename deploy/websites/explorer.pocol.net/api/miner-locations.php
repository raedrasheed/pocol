<?php
declare(strict_types=1);
ini_set('display_errors', '0');
require __DIR__ . '/lib.php';
$config = require __DIR__ . '/config.php';
$method = $_SERVER['REQUEST_METHOD'] ?? 'GET';
if (!in_array($method, ['GET','HEAD'], true)) {
    header('Allow: GET, HEAD');
    pocol_json(['error'=>'Use GET to read automatic network geography.'],405);
    exit;
}
$request = (object)['jsonrpc'=>'2.0','id'=>1,'method'=>'pocol_getNetworkGeography','params'=>[]];
[$status,$raw,$error] = pocol_forward(json_encode($request),$request,$config);
$value = $raw ? (json_decode($raw)->result ?? null) : null;
try {
    if ($status !== 200 || !is_object($value)) throw new RuntimeException();
    if (($value->schema ?? '') !== 'pocol-network-geography-v1' || ($value->chainId ?? null) !== $config['chain_id'] ||
        ($value->genesisHash ?? '') !== $config['genesis_hash'] || ($value->source ?? '') !== 'observed-peer-ip' ||
        ($value->scope ?? '') !== 'one-node' || !is_array($value->nodes ?? null) || !is_object($value->database ?? null)) throw new RuntimeException();
    $public = [];
    foreach (['schema','chainId','genesisHash','generatedAt','source','scope','peerConnections','excludedConnections'] as $key) $public[$key]=$value->$key ?? null;
    $public['database']=array_intersect_key(get_object_vars($value->database),array_flip(['provider','release','updatedAt']));
    $public['nodes']=[];
    foreach ($value->nodes as $row) {
        if (!is_object($row)) throw new RuntimeException();
        $public['nodes'][]=array_intersect_key(get_object_vars($row),array_flip(['id','country','connections','observer','miner']));
    }
    if ($method==='HEAD') {
        header('Content-Type: application/json; charset=utf-8');header('Cache-Control: no-store');
    } else pocol_json(['data'=>$public]);
} catch (Throwable $e) {
    if ($method==='HEAD') {http_response_code(503);header('Cache-Control: no-store');}
    else pocol_json(['error'=>'Automatic GeoIP is unavailable. Install or check the GeoIP gateway update on the Linux node.'],$status===429?429:503);
}
