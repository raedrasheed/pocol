import {asert,MAX256} from '../../../../prototypes/lp3-model/model.ts';
import {createHash} from 'node:crypto';
import assert from 'node:assert/strict';
let error=0,arg=0,previous=0n,prevTarget=0n;const digest=createHash('sha256');
for(let f=0;f<65536;f++){const r=asert(1n<<240n,0n,10n,65536n,BigInt(f),0n);assert(r.F!==null);assert(r.F>=previous);assert(r.target>=prevTarget);assert(r.maxBits<=512);previous=r.F;prevTarget=r.target;digest.update(r.F+'\n');const relative=Math.abs(Number(r.F)/65536/2**(f/65536)-1);if(relative>error){error=relative;arg=f;}}
let boundaries=0;for(const target of [1n,1n<<240n,MAX256])for(let s=-256;s<256;s++){const a=asert(target,1700000000n,10n,65536n,1700000000n+BigInt(s)*65536n+65535n,0n),b=asert(target,1700000000n,10n,65536n,1700000000n+BigInt(s+1)*65536n,0n);assert(a.target<=b.target);boundaries++;}
assert(error<=2e-4);console.log(JSON.stringify({fractionCases:65536,exponentBoundaries:boundaries,maxRelativeError:error,atF:arg,Fsha256:digest.digest('hex'),monotonic:true,passed:true}));
