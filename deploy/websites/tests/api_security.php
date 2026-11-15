<?php
declare(strict_types=1);
// Local-only tests: injectable transports and private temporary cache directories.
define('POCOL_NETWORK_LIBRARY_ONLY', true);
define('POCOL_REWARD_LIBRARY_ONLY', true);
require __DIR__.'/../explorer.pocol.net/api/lib.php';
require __DIR__.'/../pocol.net/api/network.php';
require __DIR__.'/../pocol.net/api/reward.php';

$checks = 0;
function check(string $name, bool $value): void {
    global $checks;
    if (!$value) throw new RuntimeException('FAIL '.$name);
    $checks++; echo 'PASS '.$name."\n";
}
function rejected(string $name, callable $operation): void {
    $failed = false;
    try { $operation(); } catch (Throwable $error) { $failed = true; }
    check($name, $failed);
}
function call(string $method = 'eth_blockNumber', array $params = [], int $id = 1): object {
    return (object)['jsonrpc'=>'2.0','id'=>$id,'method'=>$method,'params'=>$params];
}

check('ordinary explorer read accepted', pocol_valid_call(call()));
foreach (['eth_sendRawTransaction','eth_sendTransaction','personal_unlockAccount','admin_peers','debug_traceTransaction','pocol_submitShare'] as $method) {
    check('privileged method refused: '.$method, !pocol_valid_call(call($method)));
}
// Sites W6: v3 membership and finality reads, with the gateway's bounds.
foreach (['pocol_getRemovable','pocol_finality'] as $method) {
    check('parameterless read accepted: '.$method, pocol_valid_call(call($method)));
    check('parameters refused: '.$method, !pocol_valid_call(call($method,['latest'])));
}
check('members page accepted', pocol_valid_call(call('pocol_getMembers',[0,256])) && pocol_valid_call(call('pocol_getMembers',[9999,1])));
foreach ([[0,257],[0,0],[-1,1],[10000,1],[true,1],[0,1.0],['0',1],['0x0',1],[0],[0,1,2],[null,1]] as $params) {
    check('members page bounds refused: '.json_encode($params), !pocol_valid_call(call('pocol_getMembers',$params)));
}
$big = json_decode('{"jsonrpc":"2.0","id":1,"method":"pocol_getMembers","params":[0,99999999999999999999]}', false, 32, JSON_BIGINT_AS_STRING);
check('members page oversized integer refused', !pocol_valid_call($big));
$member = '0x'.str_repeat('ab',20);
check('member record accepted', pocol_valid_call(call('pocol_getMember',[$member])));
check('member record 0X prefix refused', !pocol_valid_call(call('pocol_getMember',[strtoupper($member)])));
check('member record mixed-case address accepted', pocol_valid_call(call('pocol_getMember',['0x'.str_repeat('aB',20)])));
foreach ([[],[$member,'latest'],['0x11'],[$member.'00'],[1],[(object)[]]] as $params) {
    check('member record shape refused: '.json_encode($params), !pocol_valid_call(call('pocol_getMember',$params)));
}
check('duplicate request IDs refused', !pocol_valid_payload([call(),call()]));
check('empty batch refused', !pocol_valid_payload([]));
check('batch above twenty refused', !pocol_valid_payload(array_map(static fn($i)=>call(id:$i),range(1,21))));
check('account shape refused', !pocol_valid_call(call('eth_getBalance',['0x1','latest'])));
check('state overrides refused', !pocol_valid_call(call('eth_call',[(object)[],'latest',(object)[]])));
$requests = [call(id:1),call(id:2)];
$responses = [(object)['jsonrpc'=>'2.0','id'=>2,'result'=>null],(object)['jsonrpc'=>'2.0','id'=>1,'result'=>'0x1']];
check('response IDs may be reordered', pocol_valid_response($responses,$requests));
check('duplicate response IDs refused', !pocol_valid_response([$responses[0],$responses[0]],$requests));
check('foreign response ID refused', !pocol_valid_response((object)['jsonrpc'=>'2.0','id'=>2,'result'=>'0x1'],call()));
check('response cannot contain both result and error', !pocol_valid_response((object)['jsonrpc'=>'2.0','id'=>1,'result'=>null,'error'=>(object)['code'=>1,'message'=>'x']],call()));
check('untyped error refused', !pocol_valid_response((object)['jsonrpc'=>'2.0','id'=>1,'error'=>(object)['code'=>'1','message'=>'x']],call()));
check('batch results mapped to request order', pocol_summary_results([['id'=>1],['id'=>2]],$responses) === ['0x1',null]);
rejected('summary wrong protocol refused', static fn()=>pocol_summary_results([['id'=>1]],[(object)['jsonrpc'=>'1.0','id'=>1,'result'=>1]]));
rejected('summary unsafe quantity refused', static fn()=>pocol_summary_quantity('9007199254740992'));

$now = 1700000000;
$hash = static fn(int $h): string => $h === 0 ? POCOL_SUMMARY_GENESIS : '0x'.str_pad(dechex($h),64,'0',STR_PAD_LEFT);
$rpc = static function(array $requests) use ($hash): array {
    if ($requests[0]['method'] === 'pocol_status') {
        return [(object)['chainId'=>POCOL_SUMMARY_CHAIN,'genesisHash'=>POCOL_SUMMARY_GENESIS,
            'head'=>(object)['height'=>3,'hash'=>$hash(3)],'pool'=>2],
            (object)['chainId'=>POCOL_SUMMARY_CHAIN,'genesisHash'=>POCOL_SUMMARY_GENESIS],
            [(object)['status'=>1,'tomb'=>false,'bondState'=>1],(object)['status'=>2,'tomb'=>false,'bondState'=>1]]];
    }
    return array_map(static function(array $r) use ($hash): object {
        $h=hexdec($r['params'][0]);
        return (object)['number'=>'0x'.dechex($h),'hash'=>$hash($h),'parentHash'=>$hash(max(0,$h-1)),'timestamp'=>'0x'.dechex(100+$h*4)];
    },$requests);
};
$summary = pocol_network_summary($rpc,$now);
check('summary identity and branch-consistent metrics', $summary['metrics']['eligibleMiners']===1 && $summary['metrics']['blockHeight']===3 && $summary['metrics']['averageBlockTime']==4);
$mixed = static function(array $requests) use ($rpc): array {
    $values=$rpc($requests);
    if($requests[0]['method']==='eth_getBlockByNumber')$values[1]->hash='0x'.str_repeat('ff',32);
    return $values;
};
rejected('summary mixed branch refused', static fn()=>pocol_network_summary($mixed,$now));

$reward = (object)['schema'=>POCOL_REWARD_SCHEMA,'chainId'=>1599846715,'genesisHash'=>POCOL_REWARD_GENESIS,
    'source'=>'https://explorer.pocol.net','status'=>'pending','blockHeight'=>3,'blockHash'=>$hash(3),
    'settlementHeight'=>5,'shareCount'=>0,'beneficiaryCount'=>1,'fetchedAt'=>gmdate('Y-m-d\TH:i:s\Z',$now),
    'subsidyWei'=>'10','grossRewardWei'=>'11','projectedPaidWei'=>'10','projectedBurnedWei'=>'1',
    'beneficiaries'=>[(object)['minerId'=>'0x'.str_repeat('11',20),'rewardAddress'=>'0x'.str_repeat('22',20),'amountWei'=>'10']]];
check('valid exact reward arithmetic', pocol_reward_validate($reward,$now)['grossRewardWei']==='11');
check('uint256 maximum accepted as decimal string', pocol_reward_amount(POCOL_REWARD_UINT256_MAX));
check('uint256 overflow refused', !pocol_reward_amount(pocol_reward_add(POCOL_REWARD_UINT256_MAX,'1')));
foreach (['0e1','-1','01',1,1.0] as $amount) check('noncanonical reward amount refused',!pocol_reward_amount($amount));
$wrong = clone $reward; $wrong->chainId=1;
rejected('wrong reward chain refused', static fn()=>pocol_reward_validate($wrong,$now));
$wrong = clone $reward; $wrong->projectedPaidWei='9';
rejected('unbalanced reward accounting refused', static fn()=>pocol_reward_validate($wrong,$now));
$wrong = clone $reward; $wrong->beneficiaries=[$reward->beneficiaries[0],$reward->beneficiaries[0]];$wrong->beneficiaryCount=2;
rejected('duplicate beneficiary refused', static fn()=>pocol_reward_validate($wrong,$now));
rejected('stale reward refused', static fn()=>pocol_reward_validate($reward,$now+61));

$base=sys_get_temp_dir().'/pocol-php-audit-'.bin2hex(random_bytes(8));
mkdir($base,0700);
function clean(string $path): void {
    if(is_link($path)||is_file($path)){unlink($path);return;}
    foreach(new FilesystemIterator($path) as $p)clean($p->getPathname());
    rmdir($path);
}
try {
    $loads=0;
    $loader=static function()use(&$loads,$summary):array{$loads++;return $summary;};
    check('private summary cache loads', pocol_network_cached($loader,$now,$base.'/summary')===$summary);
    check('fresh summary cache reuses snapshot', pocol_network_cached($loader,$now+1,$base.'/summary')===$summary && $loads===1);
    mkdir($base.'/shared',0777);chmod($base.'/shared',0777);
    rejected('shared-writable summary cache refused',static fn()=>pocol_network_cached($loader,$now,$base.'/shared'));
    rejected('shared-writable reward cache refused',static fn()=>pocol_reward_cached(static fn()=>(array)$reward,$now,$base.'/shared'));
    symlink($base.'/summary',$base.'/link');
    rejected('symlink cache directory refused',static fn()=>pocol_network_cached($loader,$now,$base.'/link'));
    symlink($base.'/outside.json',$base.'/summary/refresh.lock.swap');
    unlink($base.'/summary/refresh.lock');rename($base.'/summary/refresh.lock.swap',$base.'/summary/refresh.lock');
    rejected('symlink refresh lock refused',static fn()=>pocol_network_cached($loader,$now+20,$base.'/summary'));
    check('symlink target was not created',!file_exists($base.'/outside.json'));
} finally {clean($base);}
echo json_encode(['result'=>'PASS','checks'=>$checks,'networkRequests'=>0])."\n";
