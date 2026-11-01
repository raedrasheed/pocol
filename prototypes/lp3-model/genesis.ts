/** Bounded iterative RLP framing and complete GenesisSpec v1 identity, never boot. */
import { CP_SCHEMA } from './schema.ts';
import { MAX256,be,concat,hex,unhex } from './model.ts';
const MASK=(1n<<64n)-1n;
const ROT=[0,1,62,28,27,36,44,6,55,20,3,10,43,25,39,41,45,15,21,8,18,2,61,56,14];
const RC=[1n,0x8082n,0x800000000000808an,0x8000000080008000n,0x808bn,0x80000001n,0x8000000080008081n,0x8000000000008009n,0x8an,0x88n,0x80008009n,0x8000000an,0x8000808bn,0x800000000000008bn,0x8000000000008089n,0x8000000000008003n,0x8000000000008002n,0x8000000000000080n,0x800an,0x800000008000000an,0x8000000080008081n,0x8000000000008080n,0x80000001n,0x8000000080008008n];
function rot(n:bigint,k:number):bigint {return k===0?n:((n<<BigInt(k))|(n>>BigInt(64-k)))&MASK;}
export function keccak256(input:Uint8Array):Uint8Array {
 const pad=new Uint8Array(136-input.length%136);pad[0]=1;pad[pad.length-1]|=128;const message=concat(input,pad),a=Array<bigint>(25).fill(0n);
 for(let start=0;start<message.length;start+=136){for(let i=0;i<136;i++)a[Math.floor(i/8)]^=BigInt(message[start+i])<<BigInt(8*(i%8));
  for(const rc of RC){const c=Array<bigint>(5).fill(0n),d=Array<bigint>(5).fill(0n),b=Array<bigint>(25).fill(0n);
   for(let x=0;x<5;x++)for(let y=0;y<5;y++)c[x]^=a[x+5*y];
   for(let x=0;x<5;x++)d[x]=c[(x+4)%5]^rot(c[(x+1)%5],1);
   for(let x=0;x<5;x++)for(let y=0;y<5;y++)a[x+5*y]^=d[x];
   for(let x=0;x<5;x++)for(let y=0;y<5;y++)b[y+5*((2*x+3*y)%5)]=rot(a[x+5*y],ROT[x+5*y]);
   for(let x=0;x<5;x++)for(let y=0;y<5;y++)a[x+5*y]=b[x+5*y]^((~b[(x+1)%5+5*y])&b[(x+2)%5+5*y]);a[0]^=rc;
  }
 }
 return Uint8Array.from({length:32},(_,i)=>Number((a[Math.floor(i/8)]>>BigInt(8*(i%8)))&255n));
}
export class GenesisError extends Error {readonly code:string;readonly detail:string;constructor(code:string,detail=''){super(code+':'+detail);this.code=code;this.detail=detail;}}
type Rlp={bytes:Uint8Array;wrapped:boolean}|{list:Rlp[]};
function fail(code:string,detail=''):never {throw new GenesisError(code,detail);}
export const MAX_SPEC_BYTES=2818474;
function framing(b:Uint8Array):Rlp {
 if(b.length>MAX_SPEC_BYTES)fail('L0','inputLimit');
 const roots:Rlp[]=[],stack:{end:number;items:Rlp[]}[]=[];let pos=0;
 while(pos<b.length){while(stack.length&&pos===stack[stack.length-1].end)stack.pop();const parent=stack.length?stack[stack.length-1]:null,limit=parent?.end??b.length,items=parent?.items??roots;
  const prefix=b[pos++];let list=false,length=0,wrapped=false;
  if(prefix<128){items.push({bytes:Uint8Array.of(prefix),wrapped:false});continue;}
  list=prefix>=192;const offset=list?192:128,short=prefix-offset;
  if(short<=55)length=short;else {const ll=short-55;if(pos+ll>limit||b[pos]===0)fail('L0','length');for(let j=0;j<ll;j++){length=length*256+b[pos++];if(!Number.isSafeInteger(length)||length>MAX_SPEC_BYTES)fail('L0','length');}if(length<56)fail('L0','longShort');}
  const end=pos+length;if(end>limit)fail('L0','truncated');
  if(list){const node:{list:Rlp[]}={list:[]};items.push(node);if(length)stack.push({end,items:node.list});}
  else {wrapped=length===1&&b[pos]<128;items.push({bytes:b.subarray(pos,end),wrapped});pos=end;}
 }
 while(stack.length&&pos===stack[stack.length-1].end)stack.pop();if(stack.length||roots.length!==1)fail('L0','framing');return roots[0];
}
const uint=(b:Uint8Array):bigint=>b.length?BigInt('0x'+hex(b)):0n;
const bytes=(r:Rlp):Uint8Array=>'bytes'in r?r.bytes:fail('gsStructure');
export interface GenesisSpec {chainId:bigint;cp:bigint[];allocRoot:string;members:{id:string;reward:string}[];sysCodeHash:string}
export function decodeGenesis(b:Uint8Array):GenesisSpec {
 const top=framing(b);if(!('list'in top))fail('gsStructure','top not a list');const fields=top.list;
 const kinds=[false,false,true,false,true,false];fields.slice(0,6).forEach((f,i)=>{if(('list'in f)!==kinds[i])fail('gsStructure','top['+i+']');});
 const cp=fields[2]&&'list'in fields[2]?fields[2].list:[],members=fields[4]&&'list'in fields[4]?fields[4].list:[];
 if(cp.some(f=>'list'in f))fail('gsStructure','CP item is a list');
 for(const m of members)if(!('list'in m)||m.list.some(n=>'list'in n))fail('gsStructure','M_0List entry');
 if(fields[0]&&uint(bytes(fields[0]))!==1n)fail('gsVersion',uint(bytes(fields[0])).toString());
 if(fields.length!==6)fail('gsCount','top '+fields.length);if(cp.length!==52)fail('gsCount','CP '+cp.length);if(!members.length)fail('gsCount','M_0List empty');
 for(const m of members)if('list'in m&&m.list.length!==2)fail('gsCount','M_0List entry '+m.list.length);
 for(const [name,item] of [['specVersion',fields[0]],['chainId',fields[1]],...cp.map((f,i)=>[CP_SCHEMA[i][0],f])] as [string,Rlp][]){if('bytes'in item&&(item.wrapped||item.bytes[0]===0))fail('gsInt',name);}
 if(bytes(fields[3]).length!==32||bytes(fields[5]).length!==32)fail('gsLen','root');
 const pairs=members.map(m=>{if(!('list'in m))return fail('gsStructure');if(m.list.some(b=>bytes(b).length!==20))fail('gsLen','M_0List address');return {id:hex(bytes(m.list[0])),reward:hex(bytes(m.list[1]))};});
 const chain=uint(bytes(fields[1]));if(chain<1n||chain>=(1n<<64n))fail('gsRange','chainId');const values=cp.map(f=>uint(bytes(f)));
 CP_SCHEMA.forEach(([name,width,lo,hi],i)=>{const high=hi==='gamma'?10000n-values[13]:hi===null?MAX256:BigInt(hi);if(values[i]>>BigInt(width)||values[i]<BigInt(lo)||values[i]>high)fail('gsRange',name);});
 for(let i=1;i<pairs.length;i++)if(pairs[i-1].id>=pairs[i].id)fail('gsOrder','ids');
 for(const p of pairs){const n=BigInt('0x'+p.id);if(n===0n||n===(1n<<160n)-2n||(n>=0xc0c001n&&n<=0xc0c0ffn))fail('gsSys','0x'+p.id);}
 return {chainId:chain,cp:values,allocRoot:hex(bytes(fields[3])),members:pairs,sysCodeHash:hex(bytes(fields[5]))};
}
function minBytes(n:bigint):Uint8Array {if(n<0n)throw Error('uint');if(!n)return new Uint8Array();return be(n,Math.ceil(n.toString(16).length/2));}
function prefix(length:number,offset:number):Uint8Array {if(length<=55)return Uint8Array.of(offset+length);const b=minBytes(BigInt(length));return concat(Uint8Array.of(offset+55+b.length),b);}
export function rlpBytes(b:Uint8Array):Uint8Array {return b.length===1&&b[0]<128?b:concat(prefix(b.length,128),b);}
export function rlpList(items:Uint8Array[]):Uint8Array {const b=Buffer.concat(items);return concat(prefix(b.length,192),b);}
export function encodeGenesis(s:GenesisSpec):Uint8Array {
 return rlpList([rlpBytes(Uint8Array.of(1)),rlpBytes(minBytes(s.chainId)),rlpList(s.cp.map(n=>rlpBytes(minBytes(n)))),rlpBytes(unhex(s.allocRoot)),rlpList(s.members.map(m=>rlpList([rlpBytes(unhex(m.id)),rlpBytes(unhex(m.reward))]))),rlpBytes(unhex(s.sysCodeHash))]);
}
export function genesisIdentity(b:Uint8Array):{spec:GenesisSpec;hash:string;bootable:false} {return{spec:decodeGenesis(b),hash:hex(keccak256(b)),bootable:false};}
