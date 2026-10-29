//! Range miner: scans ONLY this identity's assigned nonce range [start,end] for
//! the locked TemplateID. Every hash <= T_share is reported as a share; the first
//! hash <= target is a solution. After the range is exhausted the miner stops
//! hashing (the thesis energy-saving post-range policy) until a new template.
use std::sync::{Arc,atomic::{AtomicBool,AtomicUsize,Ordering}};
use std::sync::mpsc::Sender;
use crate::types::{Fixed,pow_hash,le_target};

#[derive(Debug,Clone)]
pub enum MinerEvent{Share{tid:[u8;32],nonce:u64},Solution{tid:[u8;32],nonce:u64},Exhausted{tid:[u8;32],hashes:u64}}

/// Dropping the handle stops the miner and joins its thread, so at most one miner thread per
/// node exists (A13 thread inventory).
pub struct MinerHandle{stop:Arc<AtomicBool>,pub tid:[u8;32],thread:Option<std::thread::JoinHandle<()>>}
impl MinerHandle{pub fn stop(&self){self.stop.store(true,Ordering::SeqCst);}}
impl Drop for MinerHandle{fn drop(&mut self){self.stop();if let Some(t)=self.thread.take(){let _=t.join();}LIVE.fetch_sub(1,Ordering::SeqCst);}}
/// Miner handles alive in the process and their high-water mark. A miner thread exists only
/// between its spawn and the join in its handle's Drop, so the handles bound the threads: a peak
/// above one per mining node means a miner was started before the previous one was joined.
pub static LIVE:AtomicUsize=AtomicUsize::new(0);pub static PEAK:AtomicUsize=AtomicUsize::new(0);
/// Miners started in the process (each new template starts one).
pub static STARTS:AtomicUsize=AtomicUsize::new(0);

pub fn start(tid:[u8;32],start:u64,end:u64,target:Fixed,t_share:Fixed,max_shares:usize,events:Sender<MinerEvent>)->MinerHandle{
    let stop=Arc::new(AtomicBool::new(false));let flag=stop.clone();
    PEAK.fetch_max(LIVE.fetch_add(1,Ordering::SeqCst)+1,Ordering::SeqCst);STARTS.fetch_add(1,Ordering::SeqCst);
    let thread=crate::threads::spawn("miner",crate::threads::IO_STACK,move||{
        let mut n=start;let mut hashes=0u64;let mut shares=0usize;
        loop{
            if hashes%4096==0&&flag.load(Ordering::Relaxed){return}
            let h=pow_hash(&tid,n);hashes+=1;
            if le_target(&h,&t_share){
                if le_target(&h,&target){let _=events.send(MinerEvent::Solution{tid,nonce:n});return}
                if shares<max_shares{shares+=1;let _=events.send(MinerEvent::Share{tid,nonce:n});}
            }
            if n==end{let _=events.send(MinerEvent::Exhausted{tid,hashes});return}
            n+=1;
        }
    }).expect("spawn miner");
    MinerHandle{stop,tid,thread:Some(thread)}
}
