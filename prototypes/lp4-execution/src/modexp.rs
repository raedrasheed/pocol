//! MODEXP (address 5, EIP-198 with EIP-2565 gas) with two exact fast paths for the shapes the D-2 measurement
//! found slowest per gas (A13-DERIVATION §3b):
//! - a modulus that fits one 64-bit word (up to ~224 ns per gas in revm);
//! - an even modulus of more than one word whose power of two 2^t has t > 64 and whose odd part q is above 1
//!   (up to ~190 ns per gas in revm: aurora inverts q mod 2^t one bit at a time, which costs t·t/64 word
//!   operations for a price that does not grow with t).
//! Every input that does not take a fast path, and every input revm would refuse, is handed to revm's own
//! `berlin_run` unchanged, so the gas, the errors and the other results are revm's by construction. On a
//! fast path the input is parsed exactly as revm's `run_inner` parses it, the gas is revm's
//! `berlin_gas_calc`, and the output is the number left-padded to the modulus length, as revm pads aurora's.
//! The value is computed as base^exp mod m with m = 2^t·q, q odd:
//! - one word: mod q by Montgomery multiplication on one word; mod 2^t directly (an even base to an exponent of
//!   at least t is 0, an odd base's exponent is reduced mod 2^64: the multiplicative group mod 2^t has exponent
//!   dividing 2^(t-1));
//! - wide even: mod q and mod 2^t by revm's own MODEXP on those moduli (its odd and power-of-two paths, or the
//!   one-word path when q fits a word), q^-1 mod 2^t by Newton's iteration;
//! - both joined by the Chinese remainder theorem.
//! The tests compare it with revm on random and edge inputs.
use revm::precompile::{modexp::{berlin_gas_calc,berlin_run as revm_berlin_run},utilities::{left_pad,left_pad_vec,right_pad_vec,right_pad_with_offset},
    Precompile,PrecompileOutput,PrecompileResult,PrecompileWithAddress,u64_to_address};
use revm::primitives::{Bytes,U256};

/// The precompile pocol's EVM runs at address 5 (in place of revm's BERLIN entry).
pub const BERLIN:PrecompileWithAddress=PrecompileWithAddress(u64_to_address(5),Precompile::Standard(berlin_run));

pub fn berlin_run(input:&Bytes,gas_limit:u64)->PrecompileResult{fast(input,gas_limit).map_or_else(||revm_berlin_run(input,gas_limit),Ok)}

/// The fast paths' result, or None for every input they do not take (revm then runs it). They take only inputs
/// revm accepts: min gas and the gas cost within `gas_limit`, lengths that fit usize, and a modulus value of
/// 2^64 - 1 or less (0 and 1 included) or a wide even one (`wide_even_split`).
fn fast(input:&[u8],gas_limit:u64)->Option<PrecompileOutput>{
    const MIN_GAS:u64=200;const HEADER:usize=96;
    if MIN_GAS>gas_limit{return None}
    let base_len=usize::try_from(U256::from_be_bytes(right_pad_with_offset::<32>(input,0).into_owned())).ok()?;
    let exp_len=usize::try_from(U256::from_be_bytes(right_pad_with_offset::<32>(input,32).into_owned())).ok()?;
    let mod_len=usize::try_from(U256::from_be_bytes(right_pad_with_offset::<32>(input,64).into_owned())).ok()?;
    if base_len==0&&mod_len==0{return None}
    let input=input.get(HEADER..).unwrap_or_default();
    let exp_highp={let p=right_pad_with_offset::<32>(input,base_len);U256::from_be_bytes(left_pad::<32>(&p[..exp_len.min(32)]).into_owned())};
    let gas=berlin_gas_calc(base_len as u64,exp_len as u64,mod_len as u64,&exp_highp);
    if gas>gas_limit{return None}
    // The modulus starts at base_len + exp_len in the (right-padded) data; only a value of one word is taken.
    let mod_start=base_len.checked_add(exp_len)?;
    let modulus=right_pad_vec(input.get(mod_start.min(input.len())..).unwrap_or_default(),mod_len);
    let one=word(&modulus);
    let wide=if one.is_none(){Some(wide_even_split(&modulus)?)}else{None};
    let total=mod_start.checked_add(mod_len)?;
    let data=right_pad_vec(input,total);
    let (base,rest)=data.split_at(base_len);let (exp,_)=rest.split_at(exp_len);
    let out:Vec<u8>=match (one,wide){
        (Some(m),_)=>{
            let r=modpow_word(base,exp,m);
            let bytes=r.to_be_bytes();let lead=bytes.iter().take_while(|b|**b==0).count();
            // m = 0: aurora returns no bytes; otherwise the value's big-endian bytes ([0] for zero). Both pad to mod_len.
            if m==0{vec![]}else if r==0{vec![0]}else{bytes[lead..].to_vec()}
        }
        (None,Some(_)) if revm_is_quick(base,exp,&modulus)=>return None,
        (None,Some((q,t)))=>wide_even(base,exp,&q,t)?,
        (None,None)=>return None,
    };
    Some(PrecompileOutput::new(gas,left_pad_vec(&out,mod_len).into_owned().into()))
}
/// The inputs aurora answers without modular arithmetic, which the wide path would not speed up (the 2026-10-27
/// review: up to ~10^4 times revm's time there, a few ns per gas): a zero exponent (aurora returns 1), and a
/// power base^e of fewer words than the modulus, e of at most 8 bytes (aurora computes it directly when
/// base words · e < modulus words). The base's and the modulus's words are counted without leading zeros, as
/// aurora's `from_big_endian` keeps them, so this is aurora's condition or a wider one; either way revm runs it.
fn revm_is_quick(base:&[u8],exp:&[u8],m:&[u8])->bool{
    let words=|b:&[u8]|b.iter().position(|x|*x!=0).map_or(1,|i|(b.len()-i).div_ceil(8));
    let e=&exp[exp.iter().position(|x|*x!=0).unwrap_or(exp.len())..];
    if e.is_empty(){return true}
    e.len()<=8&&{let v=e.iter().fold(0u64,|a,x|(a<<8)|u64::from(*x));u64::try_from(words(base)).ok().and_then(|w|w.checked_mul(v)).is_some_and(|x|x<u64::try_from(words(m)).unwrap_or(u64::MAX))}
}
/// For a big-endian modulus m = q·2^t of more than one word with t > 64 and odd q > 1: q (little-endian words)
/// and t. None for every other modulus: revm runs those (the 2026-10-27 sweep measured its paths there at up to
/// about 85 ns per gas of a call, against up to about 190 for these).
fn wide_even_split(m:&[u8])->Option<(Vec<u64>,usize)>{
    let m=limbs(m);let z=m.iter().take_while(|x|**x==0).count();
    let t=z*64+usize::try_from(m.get(z)?.trailing_zeros()).ok()?;
    if t<=64{return None}
    let q=shr(&m,t);
    if q.len()==1&&q[0]==1{return None}
    Some((q,t))
}
/// base^exp mod q·2^t (q odd above 1, t above 64), big-endian without leading zero bytes ([0] for zero): x1 mod q
/// and x2 mod 2^t by revm's MODEXP (or `modpow_word` when q fits a word), joined by the Chinese remainder
/// theorem: x = x1 + q·((x2 - x1)·q^-1 mod 2^t), and 0 <= x <= (q - 1) + q·(2^t - 1) = m - 1. None if revm
/// refuses an inner call (it does not: the gas limit is u64::MAX and the lengths are the caller's).
fn wide_even(base:&[u8],exp:&[u8],q:&[u64],t:usize)->Option<Vec<u8>>{
    let x1=if q.len()==1{vec![modpow_word(base,exp,q[0])]}else{limbs(&revm_modexp(base,exp,&be(q))?)};
    let mut p2=vec![0u64;t/64+1];p2[t/64]=1u64<<(t%64);
    let x2=limbs(&revm_modexp(base,exp,&be(&p2))?);
    let n=t.div_ceil(64);
    let inv=inv_pow2(q,n);
    let y=mask(mul_trunc(&mask(sub_trunc(&x2,&x1,n),t),&inv,n),t);
    Some(be(&add(&x1,&mul(q,&y))))
}
/// revm's MODEXP of (base, exp, m) with no gas limit: its output, left-padded to m's length.
fn revm_modexp(base:&[u8],exp:&[u8],m:&[u8])->Option<Vec<u8>>{
    let len=|n:usize|U256::from(n).to_be_bytes::<32>();
    let input:Vec<u8>=[&len(base.len())[..],&len(exp.len()),&len(m.len()),base,exp,m].concat();
    revm_berlin_run(&input.into(),u64::MAX).ok().map(|o|o.bytes.to_vec())
}
/// Big-endian bytes as little-endian words, without high zero words (at least one word).
fn limbs(b:&[u8])->Vec<u64>{
    let mut v:Vec<u64>=b.rchunks(8).map(|c|c.iter().fold(0u64,|a,x|(a<<8)|u64::from(*x))).collect();
    while v.len()>1&&v[v.len()-1]==0{v.pop();}if v.is_empty(){v.push(0)}v
}
/// Little-endian words as big-endian bytes without leading zeros ([0] for zero).
fn be(v:&[u64])->Vec<u8>{
    let b:Vec<u8>=v.iter().rev().flat_map(|x|x.to_be_bytes()).collect();
    let lead=b.iter().take_while(|x|**x==0).count().min(b.len().saturating_sub(1));b[lead..].to_vec()
}
/// a >> s.
fn shr(a:&[u64],s:usize)->Vec<u64>{
    let (w,r)=(s/64,s%64);let mut v:Vec<u64>=(w..a.len()).map(|i|if r==0{a[i]}else{(a[i]>>r)|a.get(i+1).map_or(0,|x|x<<(64-r))}).collect();
    while v.len()>1&&v[v.len()-1]==0{v.pop();}if v.is_empty(){v.push(0)}v
}
/// a mod 2^t (a's words above t dropped, the top word masked).
fn mask(mut a:Vec<u64>,t:usize)->Vec<u64>{a.truncate(t.div_ceil(64));if t%64!=0{if let Some(x)=a.get_mut(t/64){*x&=(1u64<<(t%64))-1}}a}
/// a·b mod 2^(64n), n words.
fn mul_trunc(a:&[u64],b:&[u64],n:usize)->Vec<u64>{
    let mut r=vec![0u64;n];
    for (i,&x) in a.iter().enumerate().take(n){
        let mut c=0u128;
        for (j,&y) in b.iter().enumerate().take(n-i){let s=u128::from(r[i+j])+u128::from(x)*u128::from(y)+c;r[i+j]=s as u64;c=s>>64;}
        // The row's carry goes on above b's last word, up to word n.
        let mut k=i+b.len().min(n-i);
        while c!=0&&k<n{let s=u128::from(r[k])+c;r[k]=s as u64;c=s>>64;k+=1;}
    }
    r
}
/// a - b mod 2^(64n), n words.
fn sub_trunc(a:&[u64],b:&[u64],n:usize)->Vec<u64>{
    let mut r=vec![0u64;n];let mut borrow=false;
    for (i,x) in r.iter_mut().enumerate(){let (d,b1)=a.get(i).copied().unwrap_or(0).overflowing_sub(b.get(i).copied().unwrap_or(0));let (d,b2)=d.overflowing_sub(u64::from(borrow));*x=d;borrow=b1||b2;}
    r
}
/// a·b.
fn mul(a:&[u64],b:&[u64])->Vec<u64>{let mut r=mul_trunc(a,b,a.len()+b.len());while r.len()>1&&r[r.len()-1]==0{r.pop();}r}
/// a + b.
fn add(a:&[u64],b:&[u64])->Vec<u64>{
    let n=a.len().max(b.len());let mut r=Vec::with_capacity(n+1);let mut c=false;
    for i in 0..n{let (s,c1)=a.get(i).copied().unwrap_or(0).overflowing_add(b.get(i).copied().unwrap_or(0));let (s,c2)=s.overflowing_add(u64::from(c));r.push(s);c=c1||c2;}
    if c{r.push(1)}while r.len()>1&&r[r.len()-1]==0{r.pop();}r
}
/// q^-1 mod 2^(64n) for odd q, n words: Newton's iteration x' = x·(2 - q·x), which doubles the words that are
/// right, from q[0]'s inverse (one word).
fn inv_pow2(q:&[u64],n:usize)->Vec<u64>{
    let mut x=vec![inv_word(q[0])];
    while x.len()<n{
        let k=(2*x.len()).min(n);
        let qx=mul_trunc(q,&x,k);
        let two_minus=sub_trunc(&[2],&qx,k);
        x=mul_trunc(&x,&two_minus,k);
    }
    x
}
/// The value of big-endian `b` if it fits one word.
fn word(b:&[u8])->Option<u64>{
    let s=b.iter().position(|x|*x!=0).map_or(&b[b.len()..],|i|&b[i..]);
    if s.len()>8{return None}
    Some(s.iter().fold(0u64,|a,x|(a<<8)|u64::from(*x)))
}
/// base^exp mod m, all big-endian; m = 0 gives 0 (the caller returns no bytes, as aurora does).
pub fn modpow_word(base:&[u8],exp:&[u8],m:u64)->u64{
    if m<=1{return 0}
    let exp=&exp[exp.iter().position(|x|*x!=0).unwrap_or(exp.len())..];
    if exp.is_empty(){return 1}
    let b=base.iter().fold(0u128,|a,x|((a<<8)|u128::from(*x))%u128::from(m)) as u64;
    let t=m.trailing_zeros();let q=m>>t;
    let x1=if q==1{0}else{Mont::new(q).pow(b%q,exp)};
    if t==0{return x1}
    let mask=if t==64{u64::MAX}else{(1u64<<t)-1};
    let b2=b&mask;
    let x2=if b2&1==0{
        // An even base: b2^e has at least e factors of 2, so it is 0 mod 2^t once e >= t (t <= 63 here).
        let small=exp.len()<=8&&{let e=exp.iter().fold(0u64,|a,x|(a<<8)|u64::from(*x));e<u64::from(t)};
        if small{let e=exp.iter().fold(0u64,|a,x|(a<<8)|u64::from(*x));pow_wrapping(b2,e)&mask}else{0}
    }else{
        // An odd base: b2^(2^(t-1)) = 1 mod 2^t, and 2^(t-1) divides 2^64, so only exp mod 2^64 counts.
        let tail=&exp[exp.len().saturating_sub(8)..];
        pow_wrapping(b2,tail.iter().fold(0u64,|a,x|(a<<8)|u64::from(*x)))&mask
    };
    if q==1{return x2}
    // CRT: x = x1 + q·((x2 - x1)·q^-1 mod 2^t); q·y < q·2^t = m, so nothing overflows.
    let y=x2.wrapping_sub(x1).wrapping_mul(inv_word(q))&mask;
    x1+q*y
}
/// x^e mod 2^64.
fn pow_wrapping(mut x:u64,mut e:u64)->u64{let mut r=1u64;while e>0{if e&1==1{r=r.wrapping_mul(x)}x=x.wrapping_mul(x);e>>=1;}r}
/// q^-1 mod 2^64 for odd q (Newton: each step doubles the correct low bits; 3 bits to start, 6 steps).
fn inv_word(q:u64)->u64{let mut x=q;for _ in 0..6{x=x.wrapping_mul(2u64.wrapping_sub(q.wrapping_mul(x)));}x}
/// Montgomery arithmetic mod an odd q > 1 with R = 2^64.
struct Mont{q:u64,neg_inv:u64,r2:u64}
impl Mont{
    fn new(q:u64)->Self{let r=(1u128<<64)%u128::from(q);Self{q,neg_inv:inv_word(q).wrapping_neg(),r2:((r*r)%u128::from(q)) as u64}}
    /// a·b·R^-1 mod q, for a, b < q.
    fn mul(&self,a:u64,b:u64)->u64{
        let t=u128::from(a)*u128::from(b);let m=(t as u64).wrapping_mul(self.neg_inv);
        let (s,c)=t.overflowing_add(u128::from(m)*u128::from(self.q));
        let u=((s>>64) as u64,c);
        // u = (t + m·q)/2^64 < 2q; with the carry it is u.0 + 2^64.
        if u.1||u.0>=self.q{u.0.wrapping_sub(self.q)}else{u.0}
    }
    /// x^e mod q, x < q, e big-endian without leading zeros (non-empty).
    fn pow(&self,x:u64,e:&[u8])->u64{
        let xm=self.mul(x,self.r2);let mut acc=self.mul(1,self.r2);
        for byte in e{for i in (0..8).rev(){acc=self.mul(acc,acc);if (byte>>i)&1==1{acc=self.mul(acc,xm);}}}
        self.mul(acc,1)
    }
}

#[cfg(test)] mod tests;
