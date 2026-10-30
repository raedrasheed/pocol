const fs=require('fs'),path=require('path'),solc=require('solc');
if(solc.version()!=='0.8.28+commit.7893614a.Emscripten.clang')throw Error('compiler mismatch');
const input={language:'Solidity',sources:{'SystemEpochWitness.sol':{content:fs.readFileSync(path.join(__dirname,'../contracts/SystemEpochWitness.sol'),'utf8')}},settings:{optimizer:{enabled:true,runs:200},evmVersion:'cancun',metadata:{bytecodeHash:'none',appendCBOR:false},outputSelection:{'*':{'*':['abi','evm.deployedBytecode.object']}}}};
const output=JSON.parse(solc.compile(JSON.stringify(input)));
if((output.errors||[]).some(x=>x.severity==='error'))throw Error(JSON.stringify(output.errors));
fs.writeFileSync(process.argv[2],JSON.stringify({compiler:solc.version(),input,output},null,2)+'\n');
