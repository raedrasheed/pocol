"""Separate comparator Python mutation checks, NOT native compiled controls.
Usage: python gst_admission_mutations.py OUTPUT
Each modified comparator must import and fail a meaningful boundary assertion.
"""
import pathlib,subprocess,sys,json,hashlib
root=pathlib.Path(__file__).resolve().parent;out=pathlib.Path(sys.argv[1]);out.mkdir(parents=True,exist_ok=True)
source=(root/'gst_independent_vm.py').read_text();test=(root/'gst_admission_checks.py').read_text();results=[]
mutations=[('payment-value','payment=transaction.gas*transaction.max_fee_per_gas+transaction.value','payment=transaction.gas*transaction.max_fee_per_gas'),('initcode-word-gas','intrinsic+=2*((len(transaction.data)+31)//32)','intrinsic+=0*((len(transaction.data)+31)//32)'),('sender-code','if self.get_code(transaction.sender):','if False:'),('priority-fee','if transaction.max_priority_fee_per_gas>transaction.max_fee_per_gas:','if False:'),('blob-count','if len(blobs)>6:','if False:'),('block-gas','if transaction.gas>self.execution_context.gas_limit:','if False:'),('nonce','if transaction.nonce>=2**64-1 or self.get_nonce(transaction.sender)>=2**64-1:','if False:'),('initcode-size','if len(transaction.data)>49152:','if False:')]
for name,old,new in mutations:
    assert source.count(old)==1;clone=out/name;clone.mkdir();(clone/'gst_independent_vm.py').write_text(source.replace(old,new));(clone/'gst_admission_checks.py').write_text(test)
    p=subprocess.run([sys.executable,str(clone/'gst_admission_checks.py')],capture_output=True,text=True);(out/(name+'.log')).write_text(p.stdout+p.stderr)
    killed=p.returncode!=0 and 'AssertionError' in p.stderr and 'admission boundary' in p.stderr
    assert killed,(name,'mutant not killed by meaningful assertion')
    results.append(dict(name=name,exit=p.returncode,assertionKilled=killed,classification='Python comparator control, not a native compiled mutant',baselineSHA256=hashlib.sha256(source.encode()).hexdigest()))
(out/'results.json').write_text(json.dumps(results,indent=2)+'\n');print(f'PASS {len(results)} comparator mutations rejected by boundary assertions; native compiled-control evidence separate')
