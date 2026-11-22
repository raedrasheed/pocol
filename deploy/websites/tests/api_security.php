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
// SITE-05 and the 2026-11-22 review: the eligible count is the gateway's pocol_getMemberCounts (one call, at one block),
// not pocol_getMiners and not pages of pocol_getMembers (5 shared gateway tokens a page).
$counts = static fn(int $active, int $tomb, int $eligible, array $states = []): object => (object)['height'=>3,
    'blockHash'=>'0x'.str_repeat('33',32),'active'=>$active,'live'=>$active-$tomb,'eligible'=>$eligible,'tomb'=>$tomb,
    'bondStates'=>(object)array_replace(['NONE'=>0,'HELD'=>$active,'BURNED'=>0,'PAID'=>0,'ACCRUING'=>0], $states)];
$summaryCalls = [];
$rpcWith = static function(object $memberCounts, ?callable $tamper = null) use ($hash, &$summaryCalls): callable {
  return static function(array $requests) use ($hash, $memberCounts, $tamper, &$summaryCalls): array {
    $summaryCalls[] = array_column($requests, 'method');
    if ($requests[0]['method'] === 'pocol_status') {
        if ($requests[2]['method'] !== 'pocol_getMemberCounts' || $requests[2]['params'] !== []) throw new RuntimeException('FAIL counts request');
        $c = clone $memberCounts; $c->bondStates = clone $memberCounts->bondStates;
        return [(object)['chainId'=>POCOL_SUMMARY_CHAIN,'genesisHash'=>POCOL_SUMMARY_GENESIS,
            'head'=>(object)['height'=>3,'hash'=>$hash(3)],'pool'=>2],
            (object)['chainId'=>POCOL_SUMMARY_CHAIN,'genesisHash'=>POCOL_SUMMARY_GENESIS], $tamper ? $tamper($c) : $c];
    }
    if ($requests[0]['method'] !== 'eth_getBlockByNumber') throw new RuntimeException('FAIL unexpected summary request');
    return array_map(static function(array $r) use ($hash): object {
        $h=hexdec($r['params'][0]);
        return (object)['number'=>'0x'.dechex($h),'hash'=>$hash($h),'parentHash'=>$hash(max(0,$h-1)),'timestamp'=>'0x'.dechex(100+$h*4)];
    },$requests);
  };
};
// Five members: HELD, ACCRUING (4, mines on probation), NONE, a HELD tomb and BURNED: two eligible.
$five = $counts(5, 1, 2, ['NONE'=>1,'HELD'=>2,'BURNED'=>1,'ACCRUING'=>1]);
$rpc = $rpcWith($five);
$summaryCalls = [];
$summary = pocol_network_summary($rpc,$now);
check('summary identity and branch-consistent metrics', $summary['metrics']['eligibleMiners']===2 && $summary['metrics']['blockHeight']===3 && $summary['metrics']['averageBlockTime']==4);
check('summary makes two requests: status, params and counts, then the blocks',
    $summaryCalls === [['pocol_status','pocol_getParams','pocol_getMemberCounts'], array_fill(0, 4, 'eth_getBlockByNumber')]);
$mixed = static function(array $requests) use ($rpc): array {
    $values=$rpc($requests);
    if($requests[0]['method']==='eth_getBlockByNumber')$values[1]->hash='0x'.str_repeat('ff',32);
    return $values;
};
rejected('summary mixed branch refused', static fn()=>pocol_network_summary($mixed,$now));
// 10,000 members cost the same two requests as five (deploy/websites/tests/network_budget.py replays them through the
// gateway's own token bucket).
$summaryCalls = [];
check('summary reads 10000 members in one counts call',
    pocol_network_summary($rpcWith($counts(10000, 0, 9000, ['HELD'=>9000,'PAID'=>1000])),$now)['metrics']['eligibleMiners']===9000
    && count($summaryCalls) === 2 && !in_array('pocol_getMembers', array_merge(...$summaryCalls), true));
check('summary of an empty membership', pocol_network_summary($rpcWith($counts(0, 0, 0)),$now)['metrics']['eligibleMiners']===0);
foreach ([
    'membership over 10000' => static function(object $c){$c->active=10001;$c->live=10001;$c->bondStates->HELD=10001;return $c;},
    'live and tombs that are not the membership' => static function(object $c){$c->live++;return $c;},
    'bond states that are not the membership' => static function(object $c){$c->bondStates->PAID++;return $c;},
    'more eligible than live' => static function(object $c){$c->eligible=$c->live+1;return $c;},
    'more eligible than HELD and ACCRUING' => static function(object $c){$c->bondStates->HELD--;$c->bondStates->NONE++;$c->eligible=3;return $c;},
    'an unknown bond state' => static function(object $c){$c->bondStates->SLASHED=0;return $c;},
    'a missing bond state' => static function(object $c){unset($c->bondStates->PAID);return $c;},
    'a string count' => static function(object $c){$c->eligible='2';return $c;},
    'a negative count' => static function(object $c){$c->tomb=-1;$c->live=6;return $c;},
    'no block hash' => static function(object $c){unset($c->blockHash);return $c;},
    'no counts' => static fn(object $c) => null,
] as $name => $tamper) rejected('summary member counts refused: '.$name, static fn()=>pocol_network_summary($rpcWith($five, $tamper),$now));
// One refresh keeps to POCOL_SUMMARY_DEADLINE across its requests: each request gets what is left (at most 12 s), and a
// request that would start, or that ended, past it fails the refresh.
$clock = 0.0; $given = [];
$slow = static function(float $seconds) use ($rpc, &$clock, &$given): callable {
    return static function(array $requests, float $timeout) use ($rpc, $seconds, &$clock, &$given): array {
        $given[] = $timeout; $clock += $seconds; return $rpc($requests);
    };
};
$tick = static function() use (&$clock): float { return $clock; };
check('summary within its deadline', pocol_network_summary(pocol_summary_deadline($slow(4.0), POCOL_SUMMARY_DEADLINE, $tick),$now)['metrics']['eligibleMiners']===2
    && $given === [12.0, 11.0]);
$clock = 0.0; $given = [];
rejected('summary past its deadline refused', static fn()=>pocol_network_summary(pocol_summary_deadline($slow(8.0), POCOL_SUMMARY_DEADLINE, $tick),$now));
check('the second request was given only the 7 s left', $given === [12.0, 7.0]);
$clock = 0.0; $given = [];
rejected('no request starts once the deadline has passed', static fn()=>pocol_network_summary(pocol_summary_deadline($slow(15.0), POCOL_SUMMARY_DEADLINE, $tick),$now));
check('only the first request was sent', $given === [12.0]);
check('the deadline is 15 s', POCOL_SUMMARY_DEADLINE === 15);
// Integration (2026-11-22): the explorer gateway's per-client budget (40 tokens burst) refuses the refresh's second
// request (20 tokens after 22) as busy, HTTP 429 with Retry-After: 3; the refresh sends it again after that wait, at most
// twice, within the deadline, instead of failing every refresh that follows its first request closely.
$clock = 0.0; $given = []; $slept = [];
$busyFirst = static function(int $refusals, float $retryAfter) use ($rpc, &$clock, &$given): callable {
    $left = [2 => $refusals];
    return static function(array $requests, float $timeout) use ($rpc, &$left, $retryAfter, &$clock, &$given): array {
        $given[] = $timeout; $clock += 0.1;
        if ($requests[0]['method'] === 'eth_getBlockByNumber' && $left[2]-- > 0) throw new PocolSummaryBusy($retryAfter);
        return $rpc($requests);
    };
};
$nap = static function(float $s) use (&$clock, &$slept): void { $slept[] = $s; $clock += $s; };
check('summary after one busy refusal of its block read', pocol_network_summary(pocol_summary_deadline($busyFirst(1, 3.0), POCOL_SUMMARY_DEADLINE, $tick, $nap),$now)['metrics']['eligibleMiners']===2
    && $slept === [3.0] && count($given) === 3);
$clock = 0.0; $given = []; $slept = [];
check('the wait follows Retry-After, at least 1 s and at most 3 s', pocol_network_summary(pocol_summary_deadline($busyFirst(2, 60.0), POCOL_SUMMARY_DEADLINE, $tick, $nap),$now)['metrics']['eligibleMiners']===2
    && $slept === [3.0, 3.0]);
$clock = 0.0; $given = []; $slept = [];
pocol_network_summary(pocol_summary_deadline($busyFirst(1, 0.0), POCOL_SUMMARY_DEADLINE, $tick, $nap),$now);
check('a refusal without Retry-After waits 1 s', $slept === [1.0]);
$clock = 0.0; $given = []; $slept = [];
rejected('a request still busy after two retries fails the refresh', static fn()=>pocol_network_summary(pocol_summary_deadline($busyFirst(3, 3.0), POCOL_SUMMARY_DEADLINE, $tick, $nap),$now));
check('it was sent three times and waited twice', count($given) === 4 && $slept === [3.0, 3.0]);
// A refresh that started 12 s ago: a 3 s wait would leave no time for the request, so the refusal is final.
$clock = 0.0; $given = []; $slept = [];
$late = pocol_summary_deadline($busyFirst(1, 3.0), 15.0, $tick, $nap); $clock = 12.0;
rejected('no retry that the deadline cannot hold', static fn()=>pocol_network_summary($late,$now));
check('no wait past the deadline', $slept === []);

// F-74: the gateway answers schema pocol-block-reward-v3 (deploy/rewards/pocol_rewards.py snapshot_v3).
$miner = '0x'.str_repeat('11',20); $payee = '0x'.str_repeat('22',20);
$reward = (object)['schema'=>POCOL_REWARD_SCHEMA,'chainId'=>2656089850,'genesisHash'=>POCOL_REWARD_GENESIS,
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
// 2026-11-22 review: the two vectors above are also refused by the paid-sum check. Each vector below balances (paid 9,
// bond 3, the beneficiary count matches), so only the check it names refuses it; tests/README.md lists the removals that
// were run to confirm that each one fails without its check.
$wrong = clone $reward; $wrong->beneficiaryCount=3;
$wrong->beneficiaries=[(object)['minerId'=>$miner,'rewardAddress'=>$payee,'amountWei'=>'4'],
    (object)['minerId'=>$miner,'rewardAddress'=>$payee,'amountWei'=>'2'],$reward->beneficiaries[1]];
rejected('balanced duplicate beneficiary refused', static fn()=>pocol_reward_validate($wrong,$now));
$wrong->beneficiaries[1]=(object)['minerId'=>$miner,'rewardAddress'=>'0x'.str_repeat('55',20),'amountWei'=>'2'];
check('the same miner with another reward address is not a duplicate', pocol_reward_validate($wrong,$now)['beneficiaryCount']===3);
$wrong = clone $reward; $wrong->beneficiaryCount=3;
$wrong->beneficiaries=[...$reward->beneficiaries,(object)['minerId'=>'0x'.str_repeat('55',20),'rewardAddress'=>'0x'.str_repeat('66',20),'amountWei'=>'0']];
rejected('balanced empty beneficiary refused', static fn()=>pocol_reward_validate($wrong,$now));
$wrong = clone $reward;
$wrong->beneficiaries=[(object)['minerId'=>$miner,'rewardAddress'=>$payee,'amountWei'=>'6','bondAccruedWei'=>'0'],$reward->beneficiaries[1]];
rejected('explicit zero bond fill refused', static fn()=>pocol_reward_validate($wrong,$now));
$wrong = clone $reward; unset($wrong->closing);
rejected('reward without the closing key refused', static fn()=>pocol_reward_validate($wrong,$now));
foreach (['no', 0, null] as $value) {
    $wrong = clone $reward; $wrong->winnerAccruing=$value;
    rejected('winnerAccruing that is not a boolean refused', static fn()=>pocol_reward_validate($wrong,$now));
}
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
$wrong = clone $final; $wrong->settlementHeight=7;
rejected('final block with a settlement height refused', static fn()=>pocol_reward_validate($wrong,$now));
$wrong = clone $final; unset($wrong->settlementHeight);
rejected('final block without the settlementHeight key refused', static fn()=>pocol_reward_validate($wrong,$now));
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
