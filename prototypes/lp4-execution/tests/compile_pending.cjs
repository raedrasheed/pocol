const fs=require('fs'),path=require('path'),solc=require('solc');
if(solc.version()!=='0.8.28+commit.7893614a.Emscripten.clang')throw Error('compiler mismatch');
const sources={};for(const name of ['RegistryPayment.sol','PendingEscrow.sol','FinalFundingWitness.sol'])sources[name]={content:fs.readFileSync(path.join(__dirname,'../contracts',name),'utf8')};
const input={language:'Solidity',sources,settings:{optimizer:{enabled:true,runs:200},evmVersion:'cancun',metadata:{bytecodeHash:'none',appendCBOR:false},outputSelection:{'*':{'*':['abi','evm.deployedBytecode.object']}}}};
const output=JSON.parse(solc.compile(JSON.stringify(input)));if((output.errors||[]).some(x=>x.severity==='error'))throw Error(JSON.stringify(output.errors));fs.writeFileSync(process.argv[2],JSON.stringify({compiler:solc.version(),input,output},null,2)+'\n');
