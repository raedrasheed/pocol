//! Range miner: scans ONLY this identity's assigned nonce range [start,end] for
//! the locked TemplateID. Every hash <= T_share is reported as a share; the first
//! hash <= target is a solution. After the range is exhausted the miner stops
//! hashing (the thesis energy-saving post-range policy) until a new template.
//!
//! V3-30 S1 (D-V3-30, node-local): `--mine-threads N` splits the own range into N disjoint, deterministic slices
//! (`slices`), one thread each. Every hash stays inside the member's own range (P4/P5); the slices only change which
//! own nonce is tried first, never which nonces are valid. N = 1 (the default) is the single miner thread of before.
use std::sync::{Arc,atomic::{AtomicBool,AtomicU64,AtomicUsize,Ordering}};
use std::sync::mpsc::Sender;
use crate::types::{Fixed,pow_hash,le_target};

#[derive(Debug,Clone)]
pub enum MinerEvent{Share{tid:[u8;32],nonce:u64},Solution{tid:[u8;32],nonce:u64},Exhausted{tid:[u8;32],hashes:u64}}

/// The most mining threads one node runs (`--mine-threads`, 1..=MINE_THREADS_MAX). A13 counts each at a full
/// IO_STACK (`resources::io_threads`), so the bound and the task inventory grow with the setting.
pub const MINE_THREADS_MAX:usize=64;
/// `--mine-threads N`: a decimal integer in 1..=MINE_THREADS_MAX, surrounding white space ignored.
pub fn parse_threads(v:&str)->Result<usize,String>{
    let n:usize=v.trim().parse().map_err(|e|format!("{e}"))?;
    if n==0||n>MINE_THREADS_MAX{return Err(format!("must be 1..={MINE_THREADS_MAX}"))}
    Ok(n)
}
/// V3-30 S1: the inclusive range [start,end] cut into `n` disjoint slices that cover it, in order: with
/// L = end − start + 1 (in u128, so the full 2^64 domain fits), slice i is [start + ⌊i·L/n⌋, start + ⌊(i+1)·L/n⌋ − 1].
/// Deterministic (a function of the range and n only); a slice that would be empty (n > L) is left out, so no two
/// slices share a nonce and their union is the range. An empty range (start > end) or n = 0 has no slice.
pub fn slices(start:u64,end:u64,n:usize)->Vec<(u64,u64)>{(0..n).filter_map(|i|slice(start,end,n as u64,i as u64)).collect()}
/// Slice `i` of `slices(start, end, n)` alone, without the others: [start + ⌊i·L/n⌋, start + ⌊(i+1)·L/n⌋ − 1], None
/// when it is empty (n > L), i ≥ n, n = 0 or start > end. pocol_getWork computes the one slice a worker asks for.
pub fn slice(start:u64,end:u64,n:u64,i:u64)->Option<(u64,u64)>{
    if start>end||i>=n{return None}
    let len=u128::from(end-start)+1;let (n,i)=(u128::from(n),u128::from(i));
    let (a,b)=(i*len/n,(i+1)*len/n);
    (b>a).then(||(start+a as u64,start+(b-1) as u64))
}

/// Dropping the handle stops the miner and joins all its threads, so at most one miner (one handle, with its
/// `--mine-threads` threads) per node exists (A13 thread inventory).
pub struct MinerHandle{stop:Arc<AtomicBool>,pub tid:[u8;32],threads:Vec<std::thread::JoinHandle<()>>,exited:Arc<AtomicUsize>}
impl MinerHandle{
    pub fn stop(&self){self.stop.store(true,Ordering::SeqCst);}
    /// The threads this miner runs (its slices).
    pub fn threads(&self)->usize{self.threads.len()}
    /// How many of this miner's threads have returned (each counts its own exit as its last action): all of them once
    /// the handle is dropped, since Drop joins every thread.
    pub fn exited(&self)->Arc<AtomicUsize>{self.exited.clone()}
}
/// Counts its thread's exit when the thread's scan returns, whichever way (cancelled, solved, exhausted). In the
/// crate's unit tests the count waits 10 ms first, so a handle that returned without joining is seen before it.
struct ExitCount(Arc<AtomicUsize>);
impl Drop for ExitCount{fn drop(&mut self){#[cfg(test)]std::thread::sleep(std::time::Duration::from_millis(10));self.0.fetch_add(1,Ordering::SeqCst);}}
impl Drop for MinerHandle{fn drop(&mut self){self.stop();let n=self.threads.len();for t in self.threads.drain(..){let _=t.join();}LIVE.fetch_sub(1,Ordering::SeqCst);THREADS_LIVE.fetch_sub(n,Ordering::SeqCst);}}
/// Miner handles alive in the process and their high-water mark. A miner thread exists only
/// between its spawn and the join in its handle's Drop, so the handles bound the threads: a peak
/// above one per mining node means a miner was started before the previous one was joined.
pub static LIVE:AtomicUsize=AtomicUsize::new(0);pub static PEAK:AtomicUsize=AtomicUsize::new(0);
/// Miner threads alive in the process (every handle's slices) and their high-water mark: at most
/// `--mine-threads` per mining node.
pub static THREADS_LIVE:AtomicUsize=AtomicUsize::new(0);pub static THREADS_PEAK:AtomicUsize=AtomicUsize::new(0);
/// Miners started in the process (each new template starts one).
pub static STARTS:AtomicUsize=AtomicUsize::new(0);

fn advance_progress(hashes:&mut u64,stop_check:&mut u16){
    *hashes=hashes.saturating_add(1);
    *stop_check=(*stop_check+1)&4095;
}
/// What one template's threads share: the shares reported (at most `max_shares` in all), the slices not yet
/// exhausted, the hashes of the exhausted ones and whether a solution was reported (at most one).
struct Shared{shares:AtomicUsize,left:AtomicUsize,hashes:AtomicU64,solved:AtomicBool}

/// Starts the miner on one thread (`start_threads` with 1), the miner of before V3-30.
pub fn start(tid:[u8;32],start:u64,end:u64,target:Fixed,t_share:Fixed,max_shares:usize,events:Sender<MinerEvent>)->Option<MinerHandle>{
    start_threads(tid,start,end,target,t_share,max_shares,1,events)
}
/// Starts the miner on `threads` threads (`slices` of [start,end]; 1 is the single miner of before). None, logged,
/// when a thread cannot be spawned (the service's task limit reached, R2): the threads already started are stopped
/// and joined, and the node stays up without a miner for this template (it called this on the consensus thread,
/// which a panic here would stop). Exactly one Solution is reported (the first found, which then stops every slice),
/// at most `max_shares` shares in all, and one Exhausted, with every slice's hashes, once every slice is scanned.
#[allow(clippy::too_many_arguments)]
pub fn start_threads(tid:[u8;32],start:u64,end:u64,target:Fixed,t_share:Fixed,max_shares:usize,threads:usize,events:Sender<MinerEvent>)->Option<MinerHandle>{
    let parts=slices(start,end,threads.max(1));
    let stop=Arc::new(AtomicBool::new(false));
    let sh=Arc::new(Shared{shares:AtomicUsize::new(0),left:AtomicUsize::new(parts.len()),hashes:AtomicU64::new(0),solved:AtomicBool::new(false)});
    let mut handles=Vec::with_capacity(parts.len());let exited=Arc::new(AtomicUsize::new(0));
    for (i,&(s,e)) in parts.iter().enumerate(){
        let (flag,sh,events,done)=(stop.clone(),sh.clone(),events.clone(),ExitCount(exited.clone()));
        let name=if parts.len()==1{"miner".to_string()}else{format!("miner-{i}")};
        let t=crate::threads::spawn(name,crate::threads::IO_STACK,move||{let _done=done;scan(tid,s,e,target,t_share,max_shares,&flag,&sh,&events)});
        match t{
            Ok(t)=>handles.push(t),
            Err(err)=>{
                eprintln!("miner: spawn failed: {err}; no miner for this template");
                stop.store(true,Ordering::SeqCst);for t in handles{let _=t.join();}return None
            }
        }
    }
    // Counted once the threads exist: only a handle's Drop gives LIVE back.
    PEAK.fetch_max(LIVE.fetch_add(1,Ordering::SeqCst)+1,Ordering::SeqCst);STARTS.fetch_add(1,Ordering::SeqCst);
    THREADS_PEAK.fetch_max(THREADS_LIVE.fetch_add(handles.len(),Ordering::SeqCst)+handles.len(),Ordering::SeqCst);
    Some(MinerHandle{stop,tid,threads:handles,exited})
}
/// Takes one of the template's `max` share places; false once they are all taken.
fn take_share(count:&AtomicUsize,max:usize)->bool{
    let mut c=count.load(Ordering::SeqCst);
    loop{
        if c>=max{return false}
        match count.compare_exchange_weak(c,c+1,Ordering::SeqCst,Ordering::SeqCst){Ok(_)=>return true,Err(x)=>c=x}
    }
}
/// One slice [s,e]: the single-thread loop of before, with the share count, the solution and the exhaustion shared.
#[allow(clippy::too_many_arguments)]
fn scan(tid:[u8;32],s:u64,e:u64,target:Fixed,t_share:Fixed,max_shares:usize,flag:&AtomicBool,sh:&Shared,events:&Sender<MinerEvent>){
    let mut n=s;let mut hashes=0u64;let mut stop_check=0u16;
    loop{
        // Cancellation must keep running after the diagnostic count saturates.
        if stop_check==0&&flag.load(Ordering::Relaxed){return}
        let h=pow_hash(&tid,n);advance_progress(&mut hashes,&mut stop_check);
        if le_target(&h,&t_share){
            if le_target(&h,&target){
                // The first solution of the template stops every slice; a second one found meanwhile is dropped.
                if !sh.solved.swap(true,Ordering::SeqCst){let _=events.send(MinerEvent::Solution{tid,nonce:n});}
                flag.store(true,Ordering::SeqCst);return
            }
            if take_share(&sh.shares,max_shares){let _=events.send(MinerEvent::Share{tid,nonce:n});}
        }
        if n==e{break}
        n+=1;
    }
    // This slice is exhausted: its hashes join the others' before it counts down, so the last slice reports them all.
    let mut t=sh.hashes.load(Ordering::SeqCst);
    while let Err(x)=sh.hashes.compare_exchange_weak(t,t.saturating_add(hashes),Ordering::SeqCst,Ordering::SeqCst){t=x;}
    if sh.left.fetch_sub(1,Ordering::SeqCst)==1&&!sh.solved.load(Ordering::SeqCst){
        let _=events.send(MinerEvent::Exhausted{tid,hashes:sh.hashes.load(Ordering::SeqCst)});
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]
    fn saturated_hash_count_does_not_disable_cancellation_checkpoints(){
        let mut hashes=u64::MAX-1;let mut stop_check=4095u16;
        super::advance_progress(&mut hashes,&mut stop_check);
        assert_eq!(hashes,u64::MAX);assert_eq!(stop_check,0);
        // A complete second cancellation interval after the count saturated.
        for _ in 0..4096{super::advance_progress(&mut hashes,&mut stop_check);}
        assert_eq!(hashes,u64::MAX);assert_eq!(stop_check,0);
        super::advance_progress(&mut hashes,&mut stop_check);
        assert_eq!(hashes,u64::MAX);assert_eq!(stop_check,1);
    }
    /// V3-30: the slices are disjoint, ordered, cover the range exactly and are a function of (range, n) only.
    fn check(start:u64,end:u64,n:usize){
        let p=slices(start,end,n);let len=u128::from(end-start)+1;
        assert_eq!(p.len() as u128,(n as u128).min(len),"{start}..={end} n={n}: one slice per thread unless the range is shorter");
        assert_eq!(p.first().unwrap().0,start);assert_eq!(p.last().unwrap().1,end);
        let mut covered=0u128;
        for (i,&(a,b)) in p.iter().enumerate(){
            assert!(a<=b,"slice {i} is not empty");covered+=u128::from(b-a)+1;
            if i>0{assert_eq!(u128::from(a),u128::from(p[i-1].1)+1,"slice {i} starts right after slice {}: no gap, no overlap",i-1);}
            // Balanced: every slice is ⌊L/n⌋ or ⌈L/n⌉ long.
            let w=u128::from(b-a)+1;let k=(n as u128).min(len);assert!(w==len/k||w==len/k+1,"slice {i} width {w}");
        }
        assert_eq!(covered,len,"the union is the range");assert_eq!(slices(start,end,n),p,"deterministic");
    }
    #[test] fn slices_are_disjoint_and_cover_the_own_range(){
        for &(s,e) in &[(0u64,0u64),(5,5),(0,9),(3,1000),(17,17+63),(1u64<<40,(1u64<<41)-1),(0,u64::MAX),(u64::MAX-6,u64::MAX),(1,u64::MAX)]{
            for n in [1,2,3,4,7,8,13,64]{check(s,e,n);}
        }
        assert_eq!(slices(0,u64::MAX,4),vec![(0,(1u64<<62)-1),(1u64<<62,(1u64<<63)-1),(1u64<<63,(3u64<<62)-1),(3u64<<62,u64::MAX)],"the full 64-bit domain in quarters");
        assert_eq!(slices(10,12,8),vec![(10,10),(11,11),(12,12)],"more threads than nonces: one nonce each, no empty slice");
        assert!(slices(5,4,2).is_empty()&&slices(0,9,0).is_empty());
        assert_eq!(slices(100,199,1),vec![(100,199)],"one thread scans the whole range, as before");
    }
    #[test] fn thread_count_parses_within_its_bounds(){
        assert_eq!(parse_threads("1"),Ok(1));assert_eq!(parse_threads(" 8 "),Ok(8));assert_eq!(parse_threads(&MINE_THREADS_MAX.to_string()),Ok(MINE_THREADS_MAX));
        for bad in ["0","65","-1","x",""]{assert!(parse_threads(bad).is_err(),"{bad}");}
    }
    fn events(rx:&std::sync::mpsc::Receiver<MinerEvent>,until:fn(&MinerEvent)->bool)->Vec<MinerEvent>{
        let mut out=vec![];
        while let Ok(e)=rx.recv_timeout(std::time::Duration::from_secs(60)){let stop=until(&e);out.push(e);if stop{break}}
        out
    }
    /// Every slice is scanned once: an impossible target and share target exhaust the range with exactly one
    /// Exhausted event counting every nonce of the range, on N threads as on one.
    #[test] fn threads_scan_the_whole_range_once_and_report_one_exhaustion(){
        for n in [1usize,3,4]{
            let (tx,rx)=std::sync::mpsc::channel();let tid=[7u8;32];
            let h=start_threads(tid,1000,1000+4999,Fixed::ZERO,Fixed::ZERO,8,n,tx).unwrap();
            assert_eq!(h.threads(),n,"the thread count is respected");
            let ev=events(&rx,|e|matches!(e,MinerEvent::Exhausted{..}));
            assert!(matches!(ev.as_slice(),[MinerEvent::Exhausted{hashes:5000,..}]),"n={n}: {ev:?}");
            drop(h);assert!(rx.try_recv().is_err(),"n={n}: nothing after the one exhaustion");
        }
    }
    /// Shares are capped for the template across threads, distinct and inside the range.
    #[test] fn threads_share_one_share_cap_and_stay_in_range(){
        let (tx,rx)=std::sync::mpsc::channel();
        let h=start_threads([9u8;32],50,50+999,Fixed::ZERO,Fixed::MAX,16,4,tx).unwrap();
        let ev=events(&rx,|e|matches!(e,MinerEvent::Exhausted{..}));drop(h);
        let shares:Vec<u64>=ev.iter().filter_map(|e|match e{MinerEvent::Share{nonce,..}=>Some(*nonce),_=>None}).collect();
        assert_eq!(shares.len(),16,"{ev:?}");let set:std::collections::BTreeSet<u64>=shares.iter().copied().collect();assert_eq!(set.len(),16,"distinct");
        assert!(shares.iter().all(|n|(50..=1049).contains(n)),"every share is inside the own range");
        assert!(matches!(ev.last(),Some(MinerEvent::Exhausted{hashes:1000,..})));
    }
    /// Exactly one solution per template however many threads find one; the others stop without an exhaustion.
    #[test] fn one_solution_reported_when_every_thread_finds_one(){
        let (tx,rx)=std::sync::mpsc::channel();
        let h=start_threads([1u8;32],0,u64::MAX,Fixed::MAX,Fixed::MAX,4,8,tx).unwrap();
        let ev=events(&rx,|e|matches!(e,MinerEvent::Solution{..}));drop(h);
        assert!(matches!(ev.as_slice(),[MinerEvent::Solution{..}]),"{ev:?}");
        let rest:Vec<_>=rx.try_iter().collect();assert!(rest.iter().all(|e|!matches!(e,MinerEvent::Solution{..}|MinerEvent::Exhausted{..})),"{rest:?}");
    }
    /// A nonce of tid [1; 32] whose hash has 28 leading zero bits (found off line): with the target set to that hash,
    /// only about one hash in 2^28 meets it, so in practice only the slice that starts at this nonce finds a solution.
    const LOW_NONCE:u64=374_770_398;
    /// One thread's solution stops the others, which would otherwise scan for ever: every thread exits while the
    /// handle is still held (nobody is joining them), and only the one solution is reported.
    #[test] fn one_solution_stops_every_thread(){
        let tid=[1u8;32];let target=Fixed::from_be_slice(&pow_hash(&tid,LOW_NONCE)).unwrap();
        assert!(target.bits()<=228,"the fixed nonce's hash has 28 leading zero bits");
        let (tx,rx)=std::sync::mpsc::channel();
        // Slice 0 starts at the solving nonce; the seven others cover the rest of the 64-bit domain above it.
        // The share target is the target (a solution is checked inside the share check): no share event either.
        let h=start_threads(tid,LOW_NONCE,u64::MAX,target,target,4,8,tx).unwrap();assert_eq!(h.threads(),8);
        let ev=events(&rx,|e|matches!(e,MinerEvent::Solution{..}));
        assert!(matches!(ev.as_slice(),[MinerEvent::Solution{nonce:LOW_NONCE,..}]),"{ev:?}");
        let exited=h.exited();let t=std::time::Instant::now();
        while exited.load(Ordering::SeqCst)<8&&t.elapsed()<std::time::Duration::from_secs(10){std::thread::sleep(std::time::Duration::from_millis(5));}
        assert_eq!(exited.load(Ordering::SeqCst),8,"the solution stopped all eight threads, not only its own");
        drop(h);assert!(rx.try_iter().all(|e|!matches!(e,MinerEvent::Solution{..}|MinerEvent::Exhausted{..})));
    }
    /// Dropping the handle stops and joins every thread of a miner that would otherwise run for ever: once drop
    /// returns, all six threads have returned (each counts its exit 10 ms after it stops in this build, so a Drop
    /// that only detached them would return before any count).
    #[test] fn cancellation_joins_all_threads(){
        let (tx,_rx)=std::sync::mpsc::channel();
        let h=start_threads([2u8;32],0,u64::MAX,Fixed::ZERO,Fixed::ZERO,4,6,tx).unwrap();
        assert_eq!(h.threads(),6);assert!(THREADS_PEAK.load(Ordering::SeqCst)>=6);
        let exited=h.exited();std::thread::sleep(std::time::Duration::from_millis(20));
        assert_eq!(exited.load(Ordering::SeqCst),0,"nothing stops the threads before the drop");
        let t=std::time::Instant::now();drop(h);
        assert_eq!(exited.load(Ordering::SeqCst),6,"every thread was joined before drop returned");
        assert!(t.elapsed()<std::time::Duration::from_secs(30),"joined at the next cancellation checkpoint");
    }
    /// The one slice pocol_getWork computes is the same slice as the full cut's, for every case `check` covers.
    #[test] fn one_slice_equals_the_cut(){
        for &(s,e) in &[(0u64,0u64),(5,5),(0,9),(3,1000),(17,17+63),(1u64<<40,(1u64<<41)-1),(0,u64::MAX),(u64::MAX-6,u64::MAX),(1,u64::MAX)]{
            for n in [1u64,2,3,4,7,8,13,64,65536]{
                let all=slices(s,e,n as usize);let one:Vec<_>=(0..n).filter_map(|i|slice(s,e,n,i)).collect();
                assert_eq!(one,all,"{s}..={e} n={n}");
                // With at least as many nonces as slices every slice is non-empty: slice i is the cut's i-th.
                if u128::from(n)<=u128::from(e-s)+1{for i in 0..n.min(64){assert_eq!(slice(s,e,n,i),Some(all[i as usize]),"{s}..={e} n={n} i={i}");}}
            }
        }
        assert_eq!(slice(0,9,4,4),None,"index past the count");assert_eq!(slice(0,9,0,0),None);assert_eq!(slice(5,4,2,0),None);
        assert_eq!((slice(10,12,8,0),slice(10,12,8,1)),(None,None),"empty slices of a short range");
        assert_eq!([2,5,7].map(|i|slice(10,12,8,i)),[Some((10,10)),Some((11,11)),Some((12,12))],"the non-empty ones, one nonce each");
    }
}
