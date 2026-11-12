//! V3-05 template-latency telemetry (node-local; network.md "Template latency"). For each attempt-0 slot this node
//! waits in (a parent it produces on, whose Draw(h,0) is another member), it records when the drawn proposer's
//! signed template first arrived and when it locked, relative to the parent's first receipt b_j and to the attempt
//! window's start L(0), and whether it arrived after the local deadline T_j. Samples are kept per proposer in
//! fixed-bucket histograms, and in one aggregate over all proposers, so the WAN soak (V3-06) can read Δ and φ.
//! Nothing here feeds a production, import or validity decision.
//!
//! Bounds: at most PROPOSERS_MAX proposers (least recently drawn evicted first; the aggregate keeps their samples)
//! and SLOTS_MAX recent slots (oldest first). `BOUND_BYTES` is the resident bound; it is below 160 KiB, so it is
//! not an A13 term (resources.rs CONDITIONS `telemetry-inventory`).
use std::collections::VecDeque;

/// Proposers tracked at once: the v3 M_max (V3-07), so every member of a full v3 set fits.
pub const PROPOSERS_MAX:usize=256;
/// Recent slots kept to match a template and its lock to the slot's timing, and to count each slot once.
pub const SLOTS_MAX:usize=64;
/// Upper bounds of the histogram buckets, in milliseconds; one more bucket holds every larger sample.
pub const BOUNDS_MS:[u32;24]=[50,100,200,300,500,750,1000,1500,2000,2500,3000,4000,5000,6000,8000,10_000,13_000,16_000,20_000,25_000,30_000,45_000,60_000,120_000];
pub const BUCKETS:usize=BOUNDS_MS.len()+1;

/// Samples in fixed buckets: counts, total, sum and exact maximum, all saturating.
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub struct Histogram{counts:[u32;BUCKETS],n:u64,sum_ms:u64,max_ms:u32}
impl Histogram{
    /// The bucket of a sample: the first whose upper bound is at least `ms`, else the open last one.
    pub fn bucket(ms:u32)->usize{BOUNDS_MS.iter().position(|b|ms<=*b).unwrap_or(BOUNDS_MS.len())}
    pub fn record(&mut self,ms:u128){
        let ms=u32::try_from(ms).unwrap_or(u32::MAX);
        let c=&mut self.counts[Self::bucket(ms)];*c=c.saturating_add(1);
        self.n=self.n.saturating_add(1);self.sum_ms=self.sum_ms.saturating_add(u64::from(ms));self.max_ms=self.max_ms.max(ms);
    }
    pub fn count(&self)->u64{self.n}
    pub fn max(&self)->Option<u32>{(self.n>0).then_some(self.max_ms)}
    /// The `pct` percentile (1..=100): the upper bound of the bucket holding the sample of rank ceil(pct·n/100) in
    /// increasing order, but never above the exact maximum. So it is at least the exact sample percentile and at most
    /// one bucket width above it. None without samples.
    pub fn quantile(&self,pct:u32)->Option<u32>{
        if self.n==0{return None}
        let total:u64=self.counts.iter().map(|c|u64::from(*c)).sum();
        let rank=(u128::from(total)*u128::from(pct.clamp(1,100))).div_ceil(100).max(1) as u64;
        let mut seen=0u64;
        for (i,c) in self.counts.iter().enumerate(){
            seen+=u64::from(*c);
            if seen>=rank{return Some(BOUNDS_MS.get(i).copied().unwrap_or(u32::MAX).min(self.max_ms))}
        }
        Some(self.max_ms)
    }
    pub fn json(&self)->serde_json::Value{
        serde_json::json!({"count":self.n,"p50":self.quantile(50),"p90":self.quantile(90),"p99":self.quantile(99),"max":self.max(),
            "mean":(self.n>0).then(||self.sum_ms/self.n)})
    }
}

/// One proposer's counters (or the aggregate's).
#[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
pub struct Stats{
    /// Attempt-0 slots this node waited in with this proposer drawn.
    pub slots:u64,
    /// Templates that arrived after T_j, and templates that arrived before this node first received their parent.
    pub late:u64,pub early:u64,
    /// First arrival − b_j (0 when early), first arrival − L(0), and lock − b_j.
    pub since_parent:Histogram,pub since_window:Histogram,pub lock:Histogram,
    /// The tracker sequence of the last slot drawn to this proposer (least recently drawn is evicted first).
    last:u64,
}
impl Stats{
    pub fn json(&self)->serde_json::Value{
        let received=self.since_parent.count();
        serde_json::json!({"slots":self.slots,"received":received,"missing":self.slots.saturating_sub(received),"late":self.late,"early":self.early,
            "locked":self.lock.count(),"sinceParentMs":self.since_parent.json(),"sinceWindowMs":self.since_window.json(),"lockMs":self.lock.json()})
    }
}

/// A slot this node waits in: the parent, Draw(h,0), b_j, L(0) and T_j in milliseconds, and what was recorded.
#[derive(Clone,Copy,Debug)]
struct Slot{parent:[u8;32],proposer:[u8;20],parent_ms:u128,window_ms:u128,deadline_ms:u128,arrived:bool,locked:bool}

#[derive(Debug,Default)]
pub struct Tracker{proposers:Vec<([u8;20],Stats)>,slots:VecDeque<Slot>,all:Stats,seq:u64,evicted:u64}
/// The resident bound: the proposer table and the slot ring at their capacity, and the tracker itself.
pub const BOUND_BYTES:usize=PROPOSERS_MAX*std::mem::size_of::<([u8;20],Stats)>()+SLOTS_MAX*std::mem::size_of::<Slot>()+std::mem::size_of::<Tracker>();
const _:()=assert!(BOUND_BYTES<=160*1024,"the template-latency telemetry exceeds its 160 KiB inventory");

impl Tracker{
    fn stats(&mut self,proposer:&[u8;20])->&mut Stats{
        let i=match self.proposers.iter().position(|(p,_)|p==proposer){
            Some(i)=>i,
            None=>{
                if self.proposers.len()>=PROPOSERS_MAX{
                    let oldest=self.proposers.iter().enumerate().min_by_key(|(_,(_,s))|s.last).map(|(i,_)|i).unwrap();
                    self.proposers.swap_remove(oldest);self.evicted+=1;
                }
                if self.proposers.capacity()==0{self.proposers.reserve_exact(PROPOSERS_MAX);}
                self.proposers.push((*proposer,Stats::default()));self.proposers.len()-1
            }
        };
        &mut self.proposers[i].1
    }
    fn slot_mut(&mut self,parent:&[u8;32])->Option<&mut Slot>{self.slots.iter_mut().find(|s|s.parent==*parent)}
    /// This node waits in the attempt-0 slot on `parent` for Draw(h,0) = `proposer`'s template, with b_j, L(0) and
    /// T_j in milliseconds. A slot already recorded (production restarted on the same parent) counts once.
    pub fn slot(&mut self,parent:[u8;32],proposer:[u8;20],parent_ms:u128,window_ms:u128,deadline_ms:u128){
        if self.slot_mut(&parent).is_some(){return}
        if self.slots.len()>=SLOTS_MAX{self.slots.pop_front();}
        if self.slots.capacity()==0{self.slots.reserve_exact(SLOTS_MAX);}
        self.slots.push_back(Slot{parent,proposer,parent_ms,window_ms,deadline_ms,arrived:false,locked:false});
        self.seq+=1;let seq=self.seq;
        let s=self.stats(&proposer);s.slots+=1;s.last=seq;self.all.slots+=1;
    }
    /// Draw(h,0) of a recorded slot on `parent` whose template has not arrived yet, for the caller's signature check.
    pub fn waiting_for(&self,parent:&[u8;32])->Option<[u8;20]>{self.slots.iter().find(|s|s.parent==*parent&&!s.arrived).map(|s|s.proposer)}
    /// The drawn proposer's signed template for `parent` first arrived at `arrival_ms` (the caller checked its
    /// signature). Later copies, and a template for a slot not recorded, are ignored.
    pub fn arrival(&mut self,parent:&[u8;32],arrival_ms:u128){
        let Some(s)=self.slot_mut(parent).filter(|s|!s.arrived) else{return};
        s.arrived=true;let s=*s;
        let late=arrival_ms>s.deadline_ms;let early=arrival_ms<s.parent_ms;
        let (p,w)=(arrival_ms.saturating_sub(s.parent_ms),arrival_ms.saturating_sub(s.window_ms));
        let note=|st:&mut Stats|{st.late+=u64::from(late);st.early+=u64::from(early);st.since_parent.record(p);st.since_window.record(w);};
        note(&mut self.all);note(self.stats(&s.proposer));
    }
    /// The drawn proposer's template for `parent` locked here at `lock_ms`. Counted once per slot.
    pub fn locked(&mut self,parent:&[u8;32],lock_ms:u128){
        let Some(s)=self.slot_mut(parent).filter(|s|!s.locked) else{return};
        s.locked=true;let s=*s;
        let d=lock_ms.saturating_sub(s.parent_ms);
        self.all.lock.record(d);self.stats(&s.proposer).lock.record(d);
    }
    pub fn tracked(&self)->usize{self.proposers.len()}
    pub fn all(&self)->&Stats{&self.all}
    pub fn get(&self,proposer:&[u8;20])->Option<&Stats>{self.proposers.iter().find(|(p,_)|p==proposer).map(|(_,s)|s)}
    /// `pocol_templateLatency` and pocol_status `templateLatency`: proposers in address order.
    pub fn json(&self,local:serde_json::Value)->serde_json::Value{
        let mut ps:Vec<&([u8;20],Stats)>=self.proposers.iter().collect();ps.sort_by_key(|(p,_)|*p);
        serde_json::json!({"local":local,"tracked":self.proposers.len(),"trackedMax":PROPOSERS_MAX,"evicted":self.evicted,"bucketsMs":BOUNDS_MS,
            "all":self.all.json(),
            "proposers":ps.iter().map(|(p,s)|{let mut v=s.json();v["proposer"]=serde_json::json!(crate::types::hex(p));v}).collect::<Vec<_>>()})
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    fn addr(i:u32)->[u8;20]{let mut a=[0u8;20];a[16..].copy_from_slice(&i.to_be_bytes());a}
    fn parent(i:u64)->[u8;32]{let mut p=[0u8;32];p[24..].copy_from_slice(&i.to_be_bytes());p}
    /// A small deterministic generator (no external crate).
    struct Lcg(u64);
    impl Lcg{fn next(&mut self)->u64{self.0=self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);self.0>>33}}

    #[test] fn quantiles_are_the_bucket_bound_of_the_exact_rank_sample_capped_by_the_max(){
        let mut r=Lcg(7);
        for round in 0..200{
            let n=1+(r.next()%300) as usize;
            let spread=[60u64,2_000,40_000,200_000][round%4];
            let mut xs:Vec<u32>=(0..n).map(|_|(r.next()%spread) as u32).collect();
            let mut h=Histogram::default();for x in &xs{h.record(u128::from(*x));}
            xs.sort();
            assert_eq!(h.count(),n as u64);assert_eq!(h.max(),Some(*xs.last().unwrap()));
            for pct in [1,50,90,99,100]{
                let exact=xs[((n*pct as usize).div_ceil(100)).max(1)-1];
                let want=BOUNDS_MS.get(Histogram::bucket(exact)).copied().unwrap_or(u32::MAX).min(*xs.last().unwrap());
                let got=h.quantile(pct).unwrap();
                assert_eq!(got,want,"n {n} pct {pct}");
                assert!(exact<=got,"never below the exact percentile");
                let b=Histogram::bucket(exact);let lower=if b==0{0}else{BOUNDS_MS[b-1]};
                assert!(got-lower<=BOUNDS_MS.get(b).copied().unwrap_or(u32::MAX)-lower,"within one bucket");
            }
            assert_eq!(h.json()["mean"],serde_json::json!(xs.iter().map(|x|u64::from(*x)).sum::<u64>()/n as u64));
        }
        // Exact at the bounds; empty is null; a sample past u32 saturates into the open bucket.
        let mut h=Histogram::default();assert_eq!(h.quantile(50),None);assert!(h.json()["p50"].is_null());
        for x in [50u128,100,100,1000]{h.record(x);}
        assert_eq!((h.quantile(25),h.quantile(50),h.quantile(75),h.quantile(100)),(Some(50),Some(100),Some(100),Some(1000)));
        h.record(u128::from(u64::MAX));assert_eq!(h.max(),Some(u32::MAX));assert_eq!(h.quantile(100),Some(u32::MAX));
    }

    #[test] fn slots_arrivals_and_locks_are_counted_once_per_slot(){
        let mut t=Tracker::default();let (a,b)=(addr(1),addr(2));
        // b_j 10 s, L(0) 9 s, T_j 15 s.
        t.slot(parent(1),a,10_000,9_000,15_000);t.slot(parent(1),a,10_000,9_000,15_000);
        assert_eq!(t.waiting_for(&parent(1)),Some(a));
        t.arrival(&parent(1),11_200);t.arrival(&parent(1),11_300);t.locked(&parent(1),11_250);t.locked(&parent(1),11_400);
        assert_eq!(t.waiting_for(&parent(1)),None);
        let s=*t.get(&a).unwrap();
        assert_eq!((s.slots,s.late,s.early,s.since_parent.max(),s.since_window.max(),s.lock.max(),s.lock.count()),(1,0,0,Some(1200),Some(2200),Some(1250),1));
        // A late template (after T_j), an early one (before b_j: 0 since the parent) and a slot without a template.
        t.slot(parent(2),a,20_000,19_000,25_000);t.arrival(&parent(2),25_001);
        t.slot(parent(3),b,30_000,29_000,35_000);t.arrival(&parent(3),29_500);
        t.slot(parent(4),b,40_000,39_000,45_000);
        // Unknown slots are ignored.
        t.arrival(&parent(9),1);t.locked(&parent(9),1);
        let (sa,sb)=(*t.get(&a).unwrap(),*t.get(&b).unwrap());
        assert_eq!((sa.slots,sa.late,sa.since_parent.count()),(2,1,2));
        assert_eq!((sb.slots,sb.early,sb.since_parent.max(),sb.since_window.max()),(2,1,Some(0),Some(500)));
        assert_eq!(sb.json()["missing"],1);assert_eq!(sa.json()["missing"],0);
        let all=*t.all();assert_eq!((all.slots,all.late,all.early,all.since_parent.count(),all.lock.count()),(4,1,1,3,1));
        let j=t.json(serde_json::json!({"deltaRel":5}));
        assert_eq!(j["tracked"],2);assert_eq!(j["proposers"][0]["proposer"],crate::types::hex(&a));
        assert_eq!(j["all"]["received"],3);assert_eq!(j["local"]["deltaRel"],5);assert_eq!(j["proposers"][1]["sinceParentMs"]["p50"],0);
    }

    #[test] fn proposers_and_slots_stay_bounded_and_evict_the_least_recent(){
        let mut t=Tracker::default();
        for i in 0..(3*PROPOSERS_MAX as u64){
            let p=addr(i as u32);t.slot(parent(i),p,1000,0,2000);t.arrival(&parent(i),1500);t.locked(&parent(i),1600);
            assert!(t.tracked()<=PROPOSERS_MAX&&t.slots.len()<=SLOTS_MAX);
            assert!(t.proposers.capacity()<=PROPOSERS_MAX&&t.slots.capacity()<=SLOTS_MAX.next_power_of_two());
        }
        assert_eq!(t.tracked(),PROPOSERS_MAX);assert_eq!(t.evicted,2*PROPOSERS_MAX as u64);
        // The survivors are the most recently drawn; the aggregate kept every sample.
        assert!(t.get(&addr(0)).is_none());assert!(t.get(&addr(3*PROPOSERS_MAX as u32-1)).is_some());
        assert_eq!((t.all().slots,t.all().since_parent.count(),t.all().lock.count()),(3*PROPOSERS_MAX as u64,3*PROPOSERS_MAX as u64,3*PROPOSERS_MAX as u64));
        // A proposer drawn again is refreshed and survives the next evictions.
        let keep=addr(2*PROPOSERS_MAX as u32);t.slot(parent(1<<40),keep,0,0,0);
        for i in 0..(PROPOSERS_MAX as u64-1){t.slot(parent((1<<41)+i),addr(1_000_000+i as u32),0,0,0);}
        assert!(t.get(&keep).is_some());
        // An evicted slot can no longer take a late copy of its template.
        let first=parent(0);assert!(t.slot_mut(&first).is_none());
        // The resident bound covers the table and the ring at capacity.
        let held=t.proposers.capacity()*std::mem::size_of::<([u8;20],Stats)>()+t.slots.capacity()*std::mem::size_of::<Slot>()+std::mem::size_of::<Tracker>();
        assert!(held<=BOUND_BYTES,"{held} > {BOUND_BYTES}");println!("TEMPLATE-LATENCY BOUND_BYTES {BOUND_BYTES} (held {held})");
        assert!(BOUND_BYTES<=160*1024);
    }
}
