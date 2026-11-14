// Invoke with NODE_PATH pointing to the isolated, locked solc@0.8.28 installation.
// Compiles the five v3 system programs (C0C001..C0C005) with the v1/earn-in settings.
//   node compile_system_v3.cjs OUT.json            the v3 program set (tests/data/system-v3.json)
//   node compile_system_v3.cjs --mutants OUT.json  planted mutants (tests/v3/system-v3-mutants.json): four of
//     EARN-IN-SPEC.md §9 (its P_MAX and T_PROB mutants went with the probation pool and deadline, FN-2b) and seven of the
//     V3-08/V3-09 registry budgets (V3-PR-E1), MinerRegistry; five of the V3-16/V3-17 evidence penalties (V3-PR-E2b),
//     MinerRegistry or RewardEscrow; five of the V3-02 late share credit (V3-PR-E3), MinerRegistry or ShareLedger; eight of
//     the V3-19/V3-15/V3-20 issuance, carry and tip split (V3-PR-E2a), RewardEscrow; eight of the FN-2b removal at a full
//     set (C4: removeInactive's gate and registerReplacing), MinerRegistry.
//     Each entry names the program it replaces.
const fs=require('fs'),path=require('path'),solc=require('solc');
if(solc.version()!=='0.8.28+commit.7893614a.Emscripten.clang')throw Error('compiler mismatch');
const read=f=>fs.readFileSync(path.join(__dirname,'../contracts',f),'utf8');
// The v3 source is PoColSystemEarnIn.sol with the V3-xx changes (registry budgets; evidence penalties; late share credit;
// issuance, carry and tip split) and FN-2b's (no probation pool or deadline; removal only at a full set, registerReplacing).
const source=read('system/PoColSystemV3.sol');
const settings={optimizer:{enabled:true,runs:200},evmVersion:'cancun',metadata:{bytecodeHash:'none',appendCBOR:false},
  outputSelection:{'*':{'*':['abi','evm.deployedBytecode.object','storageLayout']}}};
function compile(registry){
  const input={language:'Solidity',sources:{
    'system/PoColSystemV3.sol':{content:registry},
    'ChunkFactory.sol':{content:read('ChunkFactory.sol')}},settings};
  const output=JSON.parse(solc.compile(JSON.stringify(input)));
  if((output.errors||[]).some(x=>x.severity==='error'))throw Error(JSON.stringify(output.errors,null,1));
  return output;
}
const pick=(o,f,c)=>o.contracts[f][c].evm.deployedBytecode.object;
const names=[['MinerRegistry','system/PoColSystemV3.sol'],['RewardEscrow','system/PoColSystemV3.sol'],['ShareLedger','system/PoColSystemV3.sol'],['ChainCaps','system/PoColSystemV3.sol'],['ChunkFactory','ChunkFactory.sol']];
// EARN-IN-SPEC.md §9's planted mutants that outlive FN-2b, restated on the v3 source: each must fail tests/v3_contracts.rs.
const MUTANTS=[
  ['r9-removed','if(!(arg==msg.sender && msg.sender==tx.origin) && !Sys.pay(arg,0))revert RewardNotDurable();','if(!Sys.pay(arg,0))revert RewardNotDurable();'],
  ['no-promotion-at-b','if(bond==cfg(16))put(r,(w&~STATE_MASK)|(uint256(1)<<16));','if(bond>cfg(16))put(r,(w&~STATE_MASK)|(uint256(1)<<16));'],
  ['over-fill-bad-value','uint256 add=cfg(16)-bond;if(msg.value<add)add=msg.value;','uint256 add=cfg(16)-bond;if(msg.value>add)revert BadValue();if(msg.value<add)add=msg.value;'],
  ['excess-kept','if(msg.value>add && !Sys.pay(msg.sender,msg.value-add))revert PaymentFailed();',''],
  // V3-PR-E1 (V3-08, V3-09): each must fail tests/v3_contracts.rs on tests/v3/registry-budget-vectors.json.
  ['maint-counter-shared','bool maintaining=rewarding||topping; // V3-08','bool maintaining=false; // V3-08'],
  ['maint-twice-per-block','if((x>>224)==(height&MAINT_MOD))revert RegOpsLimit();',''],
  ['react-budget-ignored','(reactivating?((used>>8)&255)>=cfg(CP_R_REACT):','(reactivating?false:'],
  ['react-cooldown-off-by-one','if(last!=0 && height<last+cfg(CP_C_REACT))revert BadState();','if(last!=0 && height<=last+cfg(CP_C_REACT))revert BadState();'],
  ['react-window-at-sweep-height','||height>=((clocks>>64)&LOW64))revert BadState();','||height>((clocks>>64)&LOW64))revert BadState();'],
  // The budgets are independent: maintenance and reactivate must not also be bounded by REG_OPS (D-V3-08/09).
  ['maint-bounded-by-reg-ops','maintaining?(used&255)>=REG_MAINT_MAX:','maintaining?((used&255)>=REG_MAINT_MAX||(c&65535)+(used&255)>=cfg(23)):'],
  ['react-counts-reg-ops','(reactivating?((used>>8)&255)>=cfg(CP_R_REACT):','(reactivating?(((used>>8)&255)>=cfg(CP_R_REACT)||(c&65535)+(used&255)>=cfg(23)):'],
  // V3-PR-E2b (V3-16, V3-17): each must fail tests/v3_contracts.rs on tests/v3/evidence-penalty-vectors.json.
  ['reporter-paid-on-fallback','if(reporter!=tx.origin) revert BadCalldata(); // V3-17: no reporter on a fallback block','','MinerRegistry'],
  ['whole-pot-for-bonded-accused','if(address(uint160(w))==accused && w&WIN_ACCRUING!=0) put(b+1,meta|INVALID);','if(address(uint160(w))==accused) put(b+1,meta|INVALID);','RewardEscrow'],
  ['accruing-read-at-evidence','if(address(uint160(w))==accused && w&WIN_ACCRUING!=0)','if(address(uint160(w))==accused && accruingNow(accused))','RewardEscrow'],
  ['accruing-read-at-settle','if(meta&INVALID!=0){ burned=amount; }','if(meta&INVALID!=0&&accruingNow(winner)){ burned=amount; }','RewardEscrow'],
  ['rho-on-pot-not-bond','uint256 cut=reward==address(0)?0:amount*Sys.cfg(58)/10000;','uint256 cut=reward==address(0)?0:Sys.cfg(15)*Sys.cfg(58)/10000;','RewardEscrow'],
  // V3-PR-E3 (V3-02): each must fail tests/v3_contracts.rs on tests/v3/late-share-vectors.json.
  ['late-last-share-reverts','if(((w>>88)&LOW64)<uint256(j)) put(r,(w&~LAST_MASK)|(uint256(j)<<88)); // V3-02 lastShare = max(lastShare, j)','if(((w>>88)&LOW64)>uint256(j)) revert BadState(); put(r,(w&~LAST_MASK)|(uint256(j)<<88));','MinerRegistry'],
  ['late-last-share-overwrites','if(((w>>88)&LOW64)<uint256(j)) put(r,(w&~LAST_MASK)|(uint256(j)<<88)); // V3-02 lastShare = max(lastShare, j)','put(r,(w&~LAST_MASK)|(uint256(j)<<88));','MinerRegistry'],
  ['late-cap-removed','if(t==0 || t+n>Sys.cfg(8)*Sys.cfg(9)) revert BadState();','if(t==0) revert BadState();','ShareLedger'],
  ['late-window-one-wider','uint256(j)>=height || uint256(j)+Sys.cfg(11)<height','uint256(j)>=height || uint256(j)+Sys.cfg(11)+1<height','ShareLedger'],
  ['late-total-not-raised','        total[j]=t+n;\n','','ShareLedger'],
  // V3-PR-E2a (V3-19, V3-15, V3-20): each must fail tests/v3_contracts.rs on tests/v3/issuance-carry-vectors.json.
  ['carry-not-added','uint256 amount=issued+(tips-share)+c;','uint256 amount=issued+(tips-share);','RewardEscrow'],
  ['tip-share-to-pot','uint256 share=tx.origin==address(0)?0:tips*Sys.cfg(54)/10000;','uint256 share=0;','RewardEscrow'],
  ['base-burn-still-burned','if(r!=a){ if(get(1)!=0) burned+=r-a; else {uint256 k=carrySlot(block.number+1); put(k,get(k)+(r-a));} } // V3-15','burned+=r-a;','RewardEscrow'],
  ['issued-ignored-flat-s0','uint256 amount=issued+(tips-share)+c;','uint256 amount=Sys.cfg(15)+(tips-share)+c;','RewardEscrow'],
  // The final block keeps the carry of its own settle(F-D) instead of burning it.
  ['carry-kept-at-final','        if(c!=0) put(4,0);\n        if(n!=0) put(5,0);\n        put(0,get(0)+tips+c+n); put(1,h);','        uint256 f=get(carrySlot(h)); if(f!=0) put(carrySlot(h),0); put(0,get(0)+tips+f); put(1,h);','RewardEscrow'],
  // The design V3-15 rejects (safety finding 8): half of settle(j)'s base burn goes into pot j+1, closed by an earlier
  // block; it must fail tests/v3_contracts.rs pot_j_plus_1_is_fixed_by_block_j_plus_1.
  ['base-burn-into-closed-pot','if(r!=a){ if(get(1)!=0) burned+=r-a; else {uint256 k=carrySlot(block.number+1); put(k,get(k)+(r-a));} } // V3-15','if(r!=a){ if(get(1)!=0) burned+=r-a; else {uint256 x=(r-a)/2; uint256 n=base(j+1); put(n,get(n)+x); put(carrySlot(block.number+1),get(carrySlot(block.number+1))+(r-a)-x);} }','RewardEscrow'],
  // The previous, unlagged rule (D-V3-15 incentive finding): settle(h-D)'s base burn enters pot h, the pot of the last
  // carrier of h-D; it must fail tests/v3_contracts.rs last_carrier_gains_nothing_by_omitting_late_shares.
  ['carry-not-lagged','uint256 k=carrySlot(block.number+1); put(k,get(k)+(r-a));','uint256 k=carrySlot(block.number); put(k,get(k)+(r-a));','RewardEscrow'],
  // The final block burns only the carry of its own settle(F-D) and keeps the carry into pot F that block F-1 left.
  ['lagged-carry-kept-at-final','        if(c!=0) put(4,0);\n        if(n!=0) put(5,0);\n        put(0,get(0)+tips+c+n); put(1,h);','        uint256 f=get(carrySlot(h+1)); if(f!=0) put(carrySlot(h+1),0); put(0,get(0)+tips+f); put(1,h);','RewardEscrow'],
  // FN-2b C4 (owner 10b, Q2(a)): each must fail tests/v3_contracts.rs on tests/v3/registry-budget-vectors.json (the
  // removal scenarios of tests/v3_model.py).
  // removeInactive (and registerReplacing) below a full set: the ungated v1 rule.
  ['removal-below-full-set','if(active<cfg(18))return 2;',''],
  ['removal-theta-off-by-one','if(height<(last>grace?last:grace)+cfg(25))return 4;','if(height<=(last>grace?last:grace)+cfg(25))return 4;'],
  ['removal-ignores-grace','if(height<(last>grace?last:grace)+cfg(25))return 4;','if(height<last+cfg(25))return 4;'],
  ['removal-at-m-min','if(active<=cfg(19))return 3;','if(active<cfg(19))return 3;'],
  ['replacing-one-reg-ops-unit','checkBudgets(c,true,false,false,1);','checkBudgets(c,true,false,false,0);'],
  ['replacing-no-r-max-entry','put(3,c+2+(uint256(1)<<16));','put(3,c+2);'],
  // The victim refunded and erased at once: its parts of pending pots burn and later evidence finds no record.
  ['replacing-victim-erased-at-once','exitAt(vr,vw,get(vr+3),height);\n        enroll(','{uint256 w=removeActive(vr,vw);if(!refund(address(uint160(v)),vr))revert PaymentFailed();erase(address(uint160(v)),vr,w);}\n        enroll('],
  // An exited (or tombed) victim accepted: the newcomer takes a seat nobody freed.
  ['replacing-any-victim','if(!live(vw)||(vw&255)!=1)return 1;','if(!live(vw))return 1;'],
  // A sender with a record (live or exited) accepted: enroll(..., w = 0) writes a new record over it, resetting its bond,
  // state and positions (enroll's own w != 0 check is not reached with the literal 0).
  ['replacing-sender-with-record','uint256 r=rec(msg.sender);if(get(r)!=0)revert BadState();\n        uint256 vr=rec(address(uint160(v)));','uint256 r=rec(msg.sender);\n        uint256 vr=rec(address(uint160(v)));'],
];
const args=process.argv.slice(2);const mutants=args[0]==='--mutants';const out=mutants?args[1]:args[0];
if(!out)throw Error('usage: compile_system_v3.cjs [--mutants] OUT.json');
if(mutants){
  const list=MUTANTS.map(([name,from,to,program='MinerRegistry'])=>{
    if(source.split(from).length!==2)throw Error(`mutant ${name}: source patch no longer matches exactly once`);
    const o=compile(source.replace(from,to));
    return {name,program,from,to,runtime:'0x'+pick(o,'system/PoColSystemV3.sol',program)};
  });
  fs.writeFileSync(out,JSON.stringify({compiler:solc.version(),settings,source:'system/PoColSystemV3.sol',mutants:list},null,1)+'\n');
}else{
  const output=compile(source);
  const programs=names.map(([c,f])=>({name:c,runtime:'0x'+pick(output,f,c),abi:output.contracts[f][c].abi}));
  const warnings=(output.errors||[]).map(e=>e.formattedMessage);
  fs.writeFileSync(out,JSON.stringify({compiler:solc.version(),settings,programs,warnings},null,1)+'\n');
}
