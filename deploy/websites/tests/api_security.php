<?php
declare(strict_types=1);
// Local-only tests: injectable transports and private temporary cache directories.
define('POCOL_NETWORK_LIBRARY_ONLY', true);
define('POCOL_REWARD_LIBRARY_ONLY', true);
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

// Owner decision 9 (2026-11-16): the explorer gateway is served at explorer.pocol.net.
check('reward upstream is the explorer gateway host', POCOL_REWARD_UPSTREAM === 'https://explorer.pocol.net/rpc');
check('network summary reads the explorer host', str_starts_with(POCOL_SUMMARY_RPC, 'https://explorer.pocol.net/'));
// Sites W8: the explorer's PHP relays (explorer.pocol.net/api/lib.php) are retired; their 43 checks are in
// deploy/rewards/test_explorer_api.py against the gateway's /api handlers, which pocol.net's summary below reads.
$requests = [call(id:1),call(id:2)];
$responses = [(object)['jsonrpc'=>'2.0','id'=>2,'result'=>null],(object)['jsonrpc'=>'2.0','id'=>1,'result'=>'0x1']];
check('batch results mapped to request order', pocol_summary_results([['id'=>1],['id'=>2]],$responses) === ['0x1',null]);
rejected('summary wrong protocol refused', static fn()=>pocol_summary_results([['id'=>1]],[(object)['jsonrpc'=>'1.0','id'=>1,'result'=>1]]));
rejected('summary unsafe quantity refused', static fn()=>pocol_summary_quantity('9007199254740992'));

$now = 1700000000;
$hash = static fn(int $h): string => $h === 0 ? POCOL_SUMMARY_GENESIS : '0x'.str_pad(dechex($h),64,'0',STR_PAD_LEFT);
// SITE-05: the members come in pocol_getMembers pages of 256 at one block (the gateway's v3 read), not pocol_getMiners.
$page = static function(array $all, int $start, string $at = ''): object {
    $members = array_slice($all, $start, 256);
    return (object)['height'=>3,'blockHash'=>$at ?: '0x'.str_repeat('33',32),'active'=>count($all),'start'=>$start,
        'count'=>count($members),'members'=>$members];
};
$members = [(object)['status'=>1,'tomb'=>false,'bondState'=>1],(object)['status'=>2,'tomb'=>false,'bondState'=>1],
    // ACCRUING (4) mines on probation; NONE (0) and a tomb do not.
    (object)['status'=>1,'tomb'=>false,'bondState'=>4],(object)['status'=>1,'tomb'=>false,'bondState'=>0],
    (object)['status'=>1,'tomb'=>true,'bondState'=>1]];
$pageCalls = [];
$rpcWith = static function(array $all, ?callable $tamper = null) use ($hash, $page, &$pageCalls): callable {
  return static function(array $requests) use ($hash, $page, $all, $tamper, &$pageCalls): array {
    if ($requests[0]['method'] === 'pocol_status') {
        if ($requests[2]['method'] !== 'pocol_getMembers' || $requests[2]['params'] !== [0,256]) throw new RuntimeException('FAIL first page request');
        $pageCalls[] = 1;
        return [(object)['chainId'=>POCOL_SUMMARY_CHAIN,'genesisHash'=>POCOL_SUMMARY_GENESIS,
            'head'=>(object)['height'=>3,'hash'=>$hash(3)],'pool'=>2],
            (object)['chainId'=>POCOL_SUMMARY_CHAIN,'genesisHash'=>POCOL_SUMMARY_GENESIS], $page($all, 0)];
    }
    if ($requests[0]['method'] === 'pocol_getMembers') {
        if (count($requests) > 8) throw new RuntimeException('FAIL batch over 8 pages (40 gateway tokens)');
        $pageCalls[] = count($requests);
        return array_map(static function(array $r) use ($page, $all, $tamper) {
            $p = $page($all, $r['params'][0]); return $tamper ? $tamper($p) : $p;
        }, $requests);
    }
    return array_map(static function(array $r) use ($hash): object {
        $h=hexdec($r['params'][0]);
        return (object)['number'=>'0x'.dechex($h),'hash'=>$hash($h),'parentHash'=>$hash(max(0,$h-1)),'timestamp'=>'0x'.dechex(100+$h*4)];
    },$requests);
  };
};
$rpc = $rpcWith($members);
$summary = pocol_network_summary($rpc,$now);
check('summary identity and branch-consistent metrics', $summary['metrics']['eligibleMiners']===2 && $summary['metrics']['blockHeight']===3 && $summary['metrics']['averageBlockTime']==4);
$mixed = static function(array $requests) use ($rpc): array {
    $values=$rpc($requests);
    if($requests[0]['method']==='eth_getBlockByNumber')$values[1]->hash='0x'.str_repeat('ff',32);
    return $values;
};
rejected('summary mixed branch refused', static fn()=>pocol_network_summary($mixed,$now));
// 600 members: three pages (256 + 256 + 88), the two later ones in one batch; 120 of them eligible.
$many = [];
for ($i = 0; $i < 600; $i++) $many[] = (object)['status'=>$i % 5 === 0 ? 1 : 2,'tomb'=>false,'bondState'=>$i % 10 === 0 ? 4 : 1];
$pageCalls = [];
check('summary counts every page of a large membership', pocol_network_summary($rpcWith($many),$now)['metrics']['eligibleMiners']===120
    && $pageCalls===[1,2]);
$wide = array_fill(0, 10000, (object)['status'=>1,'tomb'=>false,'bondState'=>1]);
$pageCalls = [];
check('summary reads 10000 members in bounded batches', pocol_network_summary($rpcWith($wide),$now)['metrics']['eligibleMiners']===10000
    && $pageCalls===[1,8,8,8,8,7]);
rejected('summary page at another block refused', static fn()=>pocol_network_summary($rpcWith($many, static function(object $p){$p->blockHash='0x'.str_repeat('44',32);return $p;}),$now));
rejected('summary short page refused', static fn()=>pocol_network_summary($rpcWith($many, static function(object $p){array_pop($p->members);$p->count--;return $p;}),$now));
rejected('summary membership that grew while read refused', static fn()=>pocol_network_summary($rpcWith($many, static function(object $p){$p->active++;return $p;}),$now));
rejected('summary membership over 10000 refused', static fn()=>pocol_network_summary($rpcWith(array_fill(0, 10001, $members[0])),$now));

// F-74: the gateway answers schema pocol-block-reward-v3 (deploy/rewards/pocol_rewards.py snapshot_v3).
$miner = '0x'.str_repeat('11',20); $payee = '0x'.str_repeat('22',20);
$reward = (object)['schema'=>POCOL_REWARD_SCHEMA,'chainId'=>1599846715,'genesisHash'=>POCOL_REWARD_GENESIS,
    'specVersion'=>2,'source'=>'https://explorer.pocol.net','status'=>'pending','blockHeight'=>3,'blockHash'=>$hash(3),
    'final'=>false,'settlementHeight'=>7,'closing'=>null,'shareCount'=>2,'beneficiaryCount'=>2,'winnerAccruing'=>false,
    'fetchedAt'=>gmdate('Y-m-d\TH:i:s\Z',$now),
    // pot = issued 10 + tips 4 - tip share 1 + carry in 2 = 15 = paid 9 (3 of them into a bond) + burned 1 + carried 5
    'issuedWei'=>'10','tipsWei'=>'4','tipShareWei'=>'1','carryInWei'=>'2','grossRewardWei'=>'15',
    'projectedPaidWei'=>'9','projectedBurnedWei'=>'1','projectedBondWei'=>'3','projectedCarryWei'=>'5',
    'owners'=>[(object)['minerId'=>$miner,'shares'=>2]],'block'=>(object)['evidence'=>[]],
    'beneficiaries'=>[(object)['minerId'=>$miner,'rewardAddress'=>$payee,'amountWei'=>'6'],
        (object)['minerId'=>'0x'.str_repeat('33',20),'rewardAddress'=>'0x'.str_repeat('44',20),'amountWei'=>'0','bondAccruedWei'=>'3']]];
$shown = pocol_reward_validate($reward,$now);
check('valid exact v3 reward arithmetic', $shown['grossRewardWei']==='15' && $shown['projectedCarryWei']==='5' && $shown['projectedBondWei']==='3');
check('v3 summary passes no block flows or owners', !isset($shown['block']) && !isset($shown['owners']) && count($shown['beneficiaries'])===2);
check('v3 summary validates again unchanged', pocol_reward_validate(json_decode(json_encode($shown)),$now)===$shown);
check('uint256 maximum accepted as decimal string', pocol_reward_amount(POCOL_REWARD_UINT256_MAX));
check('uint256 overflow refused', !pocol_reward_amount(pocol_reward_add(POCOL_REWARD_UINT256_MAX,'1')));
foreach (['0e1','-1','01',1,1.0] as $amount) check('noncanonical reward amount refused',!pocol_reward_amount($amount));
$wrong = clone $reward; $wrong->chainId=1;
rejected('wrong reward chain refused', static fn()=>pocol_reward_validate($wrong,$now));
$wrong = clone $reward; $wrong->schema='pocol-block-reward-v1';
rejected('retired v1 reward schema refused', static fn()=>pocol_reward_validate($wrong,$now));
$wrong = clone $reward; $wrong->specVersion=1;
rejected('spec-1 reward refused', static fn()=>pocol_reward_validate($wrong,$now));
$wrong = clone $reward; $wrong->source='https://example.org';
rejected('foreign reward source refused', static fn()=>pocol_reward_validate($wrong,$now));
$wrong = clone $reward; unset($wrong->source);
check('reward without a source accepted (the gateway names one only when configured)', pocol_reward_validate($wrong,$now)['grossRewardWei']==='15');
$wrong = clone $reward; $wrong->projectedPaidWei='8';
rejected('unbalanced reward accounting refused', static fn()=>pocol_reward_validate($wrong,$now));
$wrong = clone $reward; $wrong->projectedCarryWei='4'; $wrong->projectedBurnedWei='2';
check('carry and burn move together', pocol_reward_validate($wrong,$now)['projectedBurnedWei']==='2');
$wrong = clone $reward; $wrong->carryInWei='3';
rejected('pot that is not issuance plus tips less the tip share plus the carry refused', static fn()=>pocol_reward_validate($wrong,$now));
$wrong = clone $reward; $wrong->tipShareWei='5'; $wrong->issuedWei='14';
rejected('tip share above the tips refused', static fn()=>pocol_reward_validate($wrong,$now));
$wrong = clone $reward; $wrong->projectedBondWei='2';
rejected('bond total that is not the beneficiaries bond fills refused', static fn()=>pocol_reward_validate($wrong,$now));
$wrong = clone $reward; $wrong->beneficiaries=[$reward->beneficiaries[0],$reward->beneficiaries[0]];
rejected('duplicate beneficiary refused', static fn()=>pocol_reward_validate($wrong,$now));
$wrong = clone $reward; $wrong->beneficiaries=[$reward->beneficiaries[0],(object)['minerId'=>$miner,'rewardAddress'=>$payee,'amountWei'=>'0']];
rejected('empty beneficiary refused', static fn()=>pocol_reward_validate($wrong,$now));
$wrong = clone $reward; $wrong->settlementHeight=3;
rejected('settlement at or before the block refused', static fn()=>pocol_reward_validate($wrong,$now));
$wrong = clone $reward; $wrong->final=true; $wrong->settlementHeight=null;
rejected('final block with a pot refused', static fn()=>pocol_reward_validate($wrong,$now));
$final = clone $reward; $final->final=true; $final->settlementHeight=null; $final->issuedWei='0'; $final->tipsWei='0';
$final->tipShareWei='0'; $final->carryInWei='0'; $final->grossRewardWei='0'; $final->projectedPaidWei='0';
$final->projectedBurnedWei='0'; $final->projectedBondWei='0'; $final->projectedCarryWei='0'; $final->beneficiaries=[];
$final->beneficiaryCount=0; $final->closing=(object)['closeHeight'=>2,'cursorDone'=>true];
$final->tipsWei='3'; $final->carryInWei='4'; // burned by finalize, not a pot
check('final block without a pot accepted', pocol_reward_validate($final,$now)['final']===true);
$closing = clone $reward; $closing->closing=(object)['closeHeight'=>2,'cursorDone'=>false,'earliestFinalHeight'=>6,'settlementMayBeFinal'=>true];
check('closing summary passed', pocol_reward_validate($closing,$now)['closing']['settlementMayBeFinal']===true);
$closing->closing=(object)['closeHeight'=>2,'cursorDone'=>false,'earliestFinalHeight'=>6];
rejected('half a closing projection refused', static fn()=>pocol_reward_validate($closing,$now));
rejected('stale reward refused', static fn()=>pocol_reward_validate($reward,$now+61));
// The recorded v3 devnet snapshots (deploy/rewards/testdata/v3-devnet-tape.json, the gateway's own answers),
// under this page's pinned identity: every one is accepted, and its summary is exactly its figures.
$tape = json_decode(file_get_contents(__DIR__.'/../../rewards/testdata/v3-devnet-tape.json'), false, 64, JSON_THROW_ON_ERROR | JSON_BIGINT_AS_STRING);
$recorded = 0;
foreach ($tape->snapshots as $snap) {
    $snap = is_object($snap) && property_exists($snap, 'snapshot') ? $snap->snapshot : $snap;
    if (!is_object($snap) || ($snap->schema ?? null) !== POCOL_REWARD_SCHEMA) continue;
    $snap->chainId = $reward->chainId; $snap->genesisHash = POCOL_REWARD_GENESIS; $snap->fetchedAt = gmdate('Y-m-d\TH:i:s\Z',$now);
    $s = pocol_reward_validate($snap,$now);
    if ($s['grossRewardWei'] !== $snap->grossRewardWei || $s['beneficiaryCount'] !== count($snap->beneficiaries)) throw new RuntimeException('FAIL recorded snapshot');
    $recorded++;
}
check('recorded v3 devnet snapshots accepted ('.$recorded.')', $recorded >= 1);
$envelope = json_encode(['jsonrpc'=>'2.0','id'=>1,'result'=>$reward]);
check('v3 gateway envelope accepted', pocol_reward_envelope($envelope,$now)['blockHeight']===3);
rejected('gateway error envelope refused', static fn()=>pocol_reward_envelope(json_encode(['jsonrpc'=>'2.0','id'=>1,'error'=>['code'=>-32021,'message'=>'busy']]),$now));
rejected('reply over the gateway cap refused', static fn()=>pocol_reward_envelope(str_repeat(' ',POCOL_REWARD_MAX_REPLY+1),$now));

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
