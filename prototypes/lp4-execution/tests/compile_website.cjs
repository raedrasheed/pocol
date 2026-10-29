// Invoke with NODE_PATH pointing to the isolated, locked solc@0.8.28 installation.
const fs=require('fs'),path=require('path'),solc=require('solc');
if(solc.version()!=='0.8.28+commit.7893614a.Emscripten.clang')throw Error('compiler mismatch');
const input={language:'Solidity',sources:{'web/Website.sol':{content:fs.readFileSync(path.join(__dirname,'../contracts/web/Website.sol'),'utf8')}},
  settings:{optimizer:{enabled:true,runs:200},evmVersion:'cancun',metadata:{bytecodeHash:'none',appendCBOR:false},outputSelection:{'*':{'*':['abi','evm.bytecode.object','evm.deployedBytecode.object','evm.methodIdentifiers','storageLayout']}}}};
const output=JSON.parse(solc.compile(JSON.stringify(input)));
if((output.errors||[]).some(x=>x.severity==='error'))throw Error(JSON.stringify(output.errors,null,1));
const c=output.contracts['web/Website.sol'].Website;
fs.writeFileSync(process.argv[2],JSON.stringify({compiler:solc.version(),settings:input.settings,initcode:'0x'+c.evm.bytecode.object,runtime:'0x'+c.evm.deployedBytecode.object,abi:c.abi,methodIdentifiers:c.evm.methodIdentifiers,storageLayout:c.storageLayout},null,1)+'\n');
