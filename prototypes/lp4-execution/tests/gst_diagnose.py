"""Independent py-evm diagnosis of upstream control mismatches. No signing keys."""
import json,pathlib,sys,subprocess
from eth.db.atomic import AtomicDB
from eth.vm.chain_context import ChainContext
from eth.vm.forks.cancun import CancunVM
from eth.vm.forks.cancun.blocks import CancunBlockHeader
from eth.vm.spoof import SpoofTransaction

def raw(s):return bytes.fromhex(s.removeprefix('0x'))
def num(x):return int(x,16)
corpus,binary,receipts,out=sys.argv[1:]
process=subprocess.Popen([binary],stdin=subprocess.PIPE,stdout=subprocess.PIPE,text=True)
results=[]
for row in map(json.loads,open(receipts)):
 if row['controlPass']:continue
 t=json.load(open(pathlib.Path(corpus)/row['file']))[row['name']];env=t['env'];tx=t['transaction'];i=row['indexes']
 payload=dict(pre=t['pre'],env=env,transaction={k:v for k,v in tx.items() if k!='secretKey'},indexes=i)
 process.stdin.write(json.dumps(payload)+'\n');process.stdin.flush();rust=json.loads(process.stdout.readline())
 header=CancunBlockHeader(difficulty=0,block_number=num(env['currentNumber']),gas_limit=num(env['currentGasLimit']),timestamp=num(env['currentTimestamp']),base_fee_per_gas=num(env.get('currentBaseFee','0x0')),coinbase=raw(env['currentCoinbase']),mix_hash=raw(env['currentRandom']),excess_blob_gas=num(env.get('currentExcessBlobGas','0x0')))
 state=CancunVM.build_state(AtomicDB(),header,ChainContext(1))
 for a,info in t['pre'].items():
  a=raw(a);state.set_balance(a,num(info['balance']));state.set_nonce(a,num(info['nonce']));state.set_code(a,raw(info['code']))
  for k,v in info['storage'].items():state.set_storage(a,num(k),num(v))
 args=dict(nonce=num(tx['nonce']),gas_price=num(tx['gasPrice']),gas=num(tx['gasLimit'][i['gas']]),to=raw(tx['to']),value=num(tx['value'][i['value']]),data=raw(tx['data'][i['data']]))
 record=dict(file=row['file'],name=row['name'],variant=row['variant'],expectedRoot=row.get('expectedRoot'),rustRoot=row.get('actualRoot'))
 try:
  c=state.apply_transaction(SpoofTransaction(CancunVM.create_unsigned_transaction(**args),from_=raw(tx['sender'])))
  record['pythonRoot']='0x'+state.make_state_root().hex();record['pythonGas']=args['gas']-c.get_gas_remaining()-min(c.get_gas_refund(),(args['gas']-c.get_gas_remaining())//5)
  diffs=[]
  for a,account in rust.get('state',{}).items():
   addr=raw(a)
   for field,pv,rv in [('balance',state.get_balance(addr),num(account['balance'])),('nonce',state.get_nonce(addr),account['nonce']),('code',state.get_code(addr).hex(),account['code'][2:])]:
    if pv!=rv:diffs.append(dict(address=a,field=field,python=pv,rust=rv))
   for k,v in account['storage'].items():
    if state.get_storage(addr,num(k))!=num(v):diffs.append(dict(address=a,field='storage',key=k,python=state.get_storage(addr,num(k)),rust=num(v)))
  record['diffs']=diffs
 except Exception as error:record['pythonError']=type(error).__name__+': '+str(error)
 results.append(record)
process.stdin.close();process.wait();json.dump(results,open(out,'w'),indent=2);print(json.dumps(results,indent=2))
