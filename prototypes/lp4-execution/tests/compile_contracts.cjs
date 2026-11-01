// Invoke with NODE_PATH pointing to the isolated, locked solc@0.8.28 installation.
const fs=require('fs'); const path=require('path'); const solc=require('solc');
if(solc.version()!=='0.8.28+commit.7893614a.Emscripten.clang') throw Error('compiler mismatch');
const source=fs.readFileSync(path.join(__dirname,'../contracts/PayBoundaryHarness.sol'),'utf8');
const input={language:'Solidity',sources:{'PayBoundaryHarness.sol':{content:source}},settings:{optimizer:{enabled:true,runs:200},evmVersion:'cancun',metadata:{bytecodeHash:'none',appendCBOR:false},outputSelection:{'*':{'*':['abi','evm.deployedBytecode.object','evm.bytecode.object','storageLayout']}}}};
const output=JSON.parse(solc.compile(JSON.stringify(input)));
if((output.errors||[]).some(x=>x.severity==='error')) throw Error(JSON.stringify(output.errors));
fs.writeFileSync(process.argv[2],JSON.stringify({compiler:solc.version(),input,output},null,2)+'\n');
