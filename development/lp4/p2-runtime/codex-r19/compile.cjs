const fs=require('fs'),solc=require('solc');
if(solc.version()!=='0.8.28+commit.7893614a.Emscripten.clang')throw Error('compiler mismatch');
const source=fs.readFileSync(process.argv[2],'utf8');
const input={language:'Solidity',sources:{'ForceSurplus.sol':{content:source}},settings:{optimizer:{enabled:true,runs:200},evmVersion:'cancun',metadata:{bytecodeHash:'none'},outputSelection:{'*':{'*':['abi','evm.bytecode.object','evm.deployedBytecode.object']}}}};
const output=JSON.parse(solc.compile(JSON.stringify(input)));
if((output.errors||[]).some(e=>e.severity==='error'))throw Error('witness compilation failed');
fs.writeFileSync(process.argv[3],JSON.stringify({compiler:solc.version(),input,output},null,2)+'\n');
