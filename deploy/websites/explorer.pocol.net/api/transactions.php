<?php
declare(strict_types=1);
ini_set('display_errors', '0');
require __DIR__.'/lib.php';
$config=require __DIR__.'/config.php';
$method=$_SERVER['REQUEST_METHOD']??'GET';
if (!in_array($method,['GET','HEAD'],true)) {header('Allow: GET, HEAD');pocol_json(['error'=>'Use GET to read transaction history.'],405);exit;}
try {
    $page=$_GET['page']??'1'; $size=$_GET['size']??'25'; $order=$_GET['order']??'newest'; $q=$_GET['q']??'';
    if (!is_string($page)||preg_match('/^[0-9]+$/D',$page)!==1||(int)$page<1||(int)$page>1000000000 ||
        !is_string($size)||!in_array($size,['25','50','100'],true)||!is_string($order)||!in_array($order,['newest','oldest','value_desc','value_asc'],true)||
        !is_string($q)||($q!==''&&preg_match('/^0x[0-9a-fA-F]{1,64}$/D',$q)!==1)) throw new RuntimeException();
    $options=['page'=>(int)$page,'size'=>(int)$size,'order'=>$order,'q'=>strtolower($q)];
    if(isset($_GET['height'])||isset($_GET['hash'])) {
        $h=$_GET['height']??null;$hash=$_GET['hash']??null;
        if(!is_string($h)||preg_match('/^[0-9]+$/D',$h)!==1||(float)$h>9007199254740991||!pocol_hash($hash))throw new RuntimeException();
        $options['snapshot']=['height'=>(int)$h,'hash'=>strtolower($hash)];
    }
} catch(Throwable $e) {pocol_json(['error'=>'Invalid transaction query.'],400);exit;}
$request=(object)['jsonrpc'=>'2.0','id'=>1,'method'=>'pocol_getTransactions','params'=>[(object)$options]];
[$status,$raw,$error]=pocol_forward(json_encode($request),$request,$config);
$reply=$raw?json_decode($raw):null;
if (($reply->error->code??null)===-32022) {pocol_json(['error'=>'The chain changed. Refreshing transaction history.','reset'=>true],409);exit;}
$data=$reply->result??null;
if($status!==200||!is_object($data)||($data->schema??'')!=='pocol-transactions-v1'||($data->chainId??null)!==$config['chain_id']||($data->genesisHash??null)!==$config['genesis_hash']||!is_array($data->rows??null)||count($data->rows)>(int)$size) {
    pocol_json(['error'=>'Transaction history is unavailable. Install or check the Transactions gateway update on the Linux node.'],503);exit;
}
if($method==='HEAD') {header('Content-Type: application/json; charset=utf-8');header('Cache-Control: no-store');exit;}
pocol_json(['data'=>$data]);
