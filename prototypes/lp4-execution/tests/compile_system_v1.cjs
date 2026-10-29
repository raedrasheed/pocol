// Invoke with NODE_PATH pointing to the isolated, locked solc@0.8.28 installation.
// Compiles the five v1 system programs (C0C001..C0C005) with fixed settings.
const fs=require('fs'),path=require('path'),solc=require('solc');
if(solc.version()!=='0.8.28+commit.7893614a.Emscripten.clang')throw Error('compiler mismatch');
const read=f=>fs.readFileSync(path.join(__dirname,'../contracts',f),'utf8');
const input={language:'Solidity',sources:{
  'system/PoColSystemV1.sol':{content:read('system/PoColSystemV1.sol')},
  'ChunkFactory.sol':{content:read('ChunkFactory.sol')}},
  settings:{optimizer:{enabled:true,runs:200},evmVersion:'cancun',metadata:{bytecodeHash:'none',appendCBOR:false},
  outputSelection:{'*':{'*':['abi','evm.deployedBytecode.object','storageLayout']}}}};
const output=JSON.parse(solc.compile(JSON.stringify(input)));
if((output.errors||[]).some(x=>x.severity==='error'))throw Error(JSON.stringify(output.errors,null,1));
const pick=(f,c)=>output.contracts[f][c].evm.deployedBytecode.object;
const programs=[['MinerRegistry','system/PoColSystemV1.sol'],['RewardEscrow','system/PoColSystemV1.sol'],['ShareLedger','system/PoColSystemV1.sol'],['ChainCaps','system/PoColSystemV1.sol'],['ChunkFactory','ChunkFactory.sol']]
  .map(([c,f])=>({name:c,runtime:'0x'+pick(f,c),abi:output.contracts[f][c].abi}));
const warnings=(output.errors||[]).map(e=>e.formattedMessage);
fs.writeFileSync(process.argv[2],JSON.stringify({compiler:solc.version(),settings:input.settings,programs,warnings},null,1)+'\n');
