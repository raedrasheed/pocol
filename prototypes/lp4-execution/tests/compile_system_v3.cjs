// Invoke with NODE_PATH pointing to the isolated, locked solc@0.8.28 installation.
// Compiles the five v3 system programs (C0C001..C0C005) with the v1/earn-in settings.
//   node compile_system_v3.cjs OUT.json            the v3 program set (tests/data/system-v3.json)
//   node compile_system_v3.cjs --mutants OUT.json  six planted MinerRegistry mutants (tests/v3/system-v3-mutants.json)
const fs=require('fs'),path=require('path'),solc=require('solc');
if(solc.version()!=='0.8.28+commit.7893614a.Emscripten.clang')throw Error('compiler mismatch');
const read=f=>fs.readFileSync(path.join(__dirname,'../contracts',f),'utf8');
// The v3 source is PoColSystemEarnIn.sol with the V3-xx changes (P_MAX/T_PROB from CP schema v2).
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
];
const args=process.argv.slice(2);const mutants=args[0]==='--mutants';const out=mutants?args[1]:args[0];
if(!out)throw Error('usage: compile_system_v3.cjs [--mutants] OUT.json');
if(mutants){
  const list=MUTANTS.map(([name,from,to])=>{
    if(source.split(from).length!==2)throw Error(`mutant ${name}: source patch no longer matches exactly once`);
    const o=compile(source.replace(from,to));
    return {name,from,to,runtime:'0x'+pick(o,'system/PoColSystemV3.sol','MinerRegistry')};
  });
  fs.writeFileSync(out,JSON.stringify({compiler:solc.version(),settings,source:'system/PoColSystemV3.sol',program:'MinerRegistry',mutants:list},null,1)+'\n');
}else{
  const output=compile(source);
  const programs=names.map(([c,f])=>({name:c,runtime:'0x'+pick(output,f,c),abi:output.contracts[f][c].abi}));
  const warnings=(output.errors||[]).map(e=>e.formattedMessage);
  fs.writeFileSync(out,JSON.stringify({compiler:solc.version(),settings,programs,warnings},null,1)+'\n');
}
