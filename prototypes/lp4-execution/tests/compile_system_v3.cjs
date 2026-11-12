// Invoke with NODE_PATH pointing to the isolated, locked solc@0.8.28 installation.
// Compiles the five v3 system programs (C0C001..C0C005) with the v1/earn-in settings.
//   node compile_system_v3.cjs OUT.json            the v3 program set (tests/data/system-v3.json)
//   node compile_system_v3.cjs --mutants OUT.json  planted mutants (tests/v3/system-v3-mutants.json): the six of
//     EARN-IN-SPEC.md §9 and seven of the V3-08/V3-09 registry budgets (V3-PR-E1), MinerRegistry; five of the V3-16/V3-17
//     evidence penalties (V3-PR-E2b), MinerRegistry or RewardEscrow. Each entry names the program it replaces.
const fs=require('fs'),path=require('path'),solc=require('solc');
if(solc.version()!=='0.8.28+commit.7893614a.Emscripten.clang')throw Error('compiler mismatch');
const read=f=>fs.readFileSync(path.join(__dirname,'../contracts',f),'utf8');
// The v3 source is PoColSystemEarnIn.sol with the V3-xx changes (P_MAX/T_PROB from CP schema v2; registry budgets;
// evidence penalties).
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
// EARN-IN-SPEC.md §9's six planted mutants, restated on the v3 source: each must fail tests/v3_contracts.rs.
const MUTANTS=[
  ['p-max-plus-one','if(probation && get(ACC_COUNT)>=cfg(CP_P_MAX))revert Capacity();','if(probation && get(ACC_COUNT)>cfg(CP_P_MAX))revert Capacity();'],
  ['r9-removed','if(!(arg==msg.sender && msg.sender==tx.origin) && !Sys.pay(arg,0))revert RewardNotDurable();','if(!Sys.pay(arg,0))revert RewardNotDurable();'],
  ['no-promotion-at-b','if(bond==cfg(16)){put(r,(w&~STATE_MASK)|(uint256(1)<<16));','if(bond>cfg(16)){put(r,(w&~STATE_MASK)|(uint256(1)<<16));'],
  ['t-prob-plus-one','return height>=registeredAt(w)+cfg(CP_T_PROB);','return height>=registeredAt(w)+cfg(CP_T_PROB)+1;'],
  ['over-fill-bad-value','uint256 add=cfg(16)-bond;if(msg.value<add)add=msg.value;','uint256 add=cfg(16)-bond;if(msg.value>add)revert BadValue();if(msg.value<add)add=msg.value;'],
  ['excess-kept','if(msg.value>add && !Sys.pay(msg.sender,msg.value-add))revert PaymentFailed();',''],
  // V3-PR-E1 (V3-08, V3-09): each must fail tests/v3_contracts.rs on tests/v3/registry-budget-vectors.json.
  ['maint-counter-shared','bool maintaining=rewarding||topping; // V3-08','bool maintaining=false; // V3-08'],
  ['maint-twice-per-block','if((w>>216)==(height&MAINT_MOD))revert RegOpsLimit();',''],
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
  ['rho-on-pot-not-bond','uint256 cut=reward==address(0)?0:amount*Sys.cfg(66)/10000;','uint256 cut=reward==address(0)?0:Sys.cfg(15)*Sys.cfg(66)/10000;','RewardEscrow'],
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
