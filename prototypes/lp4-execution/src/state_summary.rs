//! C-1 E2: the per-block whole-state checks from a state summary.
//!
//! `apply_block` checks every candidate (and its parent) against the code limits, the user-state caps, the
//! system slot bound and the supply bound, and drops unreferenced code. Each of those walked the whole
//! state. A `Summary` keeps the numbers they compute, and `moved` carries it from one state to another
//! through the accounts and slots that differ between them (`state_trie::Touched::between`, complete by
//! construction), so a check reads a few numbers instead of every account.
//!
//! `scan` computes the summary from the whole state exactly as the walks count. Tests (and the
//! `root-check` feature) compare every moved summary with `scan`, and every outcome with the old walks.
use revm::{db::{AccountState,DbAccount},primitives::{alloy_primitives::U512,Address,B256,U256,KECCAK_EMPTY},InMemoryDB};
use crate::{code_caps::{self,CodeLimits},state_trie::Touched,view::{Acc,View}};

/// What the whole-state checks read, for one state.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Summary{
    /// B_code_max the code counts below were taken against.
    pub runtime:u32,
    /// Logical code of every existing account (`code_caps::inventory`, without its errors).
    pub code_total:u128,
    /// Existing accounts whose runtime bytes are missing, or do not hash to their code hash.
    pub code_missing:u64,pub code_mismatch:u64,
    /// Existing accounts whose runtime is longer than `runtime`.
    pub over_runtime:u64,
    /// Existing accounts outside the five system programs, and their nonzero slots (`check_caps`).
    pub user_accounts:u64,pub user_slots:u64,
    /// Nonzero slots of the existing system programs.
    pub sys_slots:u64,
    /// The sum of every existing account's balance (no intermediate sum can overflow 512 bits).
    pub balance:U512,
    /// The contracts map holds only referenced code (and the two defaults), within its capacity rule:
    /// `drop_unreferenced_code` would leave it as it is.
    pub contracts_clean:bool,
}

fn exists(a:&DbAccount)->bool{!matches!(a.account_state,AccountState::NotExisting)}
fn is_system(a:&Address)->bool{(0..5).any(|i|*a==crate::apply::program_address(i))}
fn nonzero(db:&dyn View,addr:&Address)->u64{let mut n=0u64;db.each_slot(addr,&mut |_,v|{if *v!=U256::ZERO{n+=1}});n}
fn clean(db:&InMemoryDB)->bool{
    let used:std::collections::HashSet<B256>=db.accounts.values().filter(|a|exists(a)).map(|a|a.info.code_hash).collect();
    db.contracts.keys().all(|h|used.contains(h)||*h==KECCAK_EMPTY||*h==B256::ZERO)&&db.contracts.capacity()<=2*db.contracts.len()+16
}

/// One account's part of a summary, with its storage counted whole or not at all.
#[derive(Default)]
struct Part{code:u128,missing:u64,mismatch:u64,over:u64,user:u64,slots:u64,balance:U512}
fn part(db:&dyn View,addr:&Address,a:&Acc,runtime:u32,with_slots:bool)->Part{
    let mut p=Part::default();if !a.exists(){return p}
    match code_caps::code_len(db,&a.info){
        Ok(n)=>{p.code=u128::from(n);if n>u64::from(runtime){p.over=1}}
        Err(e) if e=="missing runtime bytes"=>p.missing=1,
        Err(_)=>p.mismatch=1,
    }
    if !is_system(addr){p.user=1}
    if with_slots{p.slots=nonzero(db,addr)}
    p.balance=U512::from(a.info.balance);p
}

impl Summary{
    /// From the whole state.
    pub fn scan(db:&InMemoryDB,runtime:u32)->Summary{Summary{contracts_clean:clean(db),..Summary::scan_view(db,runtime)}}
    /// `scan` of any state view, with `contracts_clean` left true (C-1 E3: an overlay's intermediate state).
    pub fn scan_view(db:&dyn View,runtime:u32)->Summary{
        let mut s=Summary{runtime,code_total:0,code_missing:0,code_mismatch:0,over_runtime:0,user_accounts:0,user_slots:0,sys_slots:0,balance:U512::ZERO,contracts_clean:true};
        // C-1 E4.5: summed as the accounts are read (a stored version is read once, holding nothing per
        // account; the sums do not depend on the order).
        db.each_account(&mut |addr,a|{let p=part(db,addr,&a,runtime,true);s.add(addr,&p);});
        s
    }
    fn add(&mut self,addr:&Address,p:&Part){
        self.code_total+=p.code;self.code_missing+=p.missing;self.code_mismatch+=p.mismatch;self.over_runtime+=p.over;self.user_accounts+=p.user;self.balance+=p.balance;
        if is_system(addr){self.sys_slots+=p.slots}else{self.user_slots+=p.slots}
    }
    fn sub(&mut self,addr:&Address,p:&Part){
        self.code_total-=p.code;self.code_missing-=p.missing;self.code_mismatch-=p.mismatch;self.over_runtime-=p.over;self.user_accounts-=p.user;self.balance-=p.balance;
        if is_system(addr){self.sys_slots-=p.slots}else{self.user_slots-=p.slots}
    }
    /// The summary of `to`, given that this is `from`'s and that only the `touched` accounts and slots
    /// differ between them. `contracts_clean` is left as it is: the caller decides it with the drop.
    pub fn moved(&self,from:&dyn View,to:&dyn View,touched:&Touched)->Summary{
        let mut s=*self;
        for (addr,slots) in touched.iter(){
            let (old,new)=(from.account(addr),to.account(addr));
            let both=old.as_ref().map_or(false,|a|a.exists())&&new.as_ref().map_or(false,|a|a.exists());
            // Slots one by one only while the account exists on both sides; otherwise its storage whole.
            let whole=!both||slots.is_none();
            if let Some(a)=&old{s.sub(addr,&part(from,addr,a,self.runtime,whole));}
            if let Some(a)=&new{s.add(addr,&part(to,addr,a,self.runtime,whole));}
            if let (false,Some(list),Some(o),Some(n))=(whole,slots,old,new){
                let mut list=list.to_vec();list.sort_unstable();list.dedup();
                let _=(o,n);let (mut gone,mut came)=(0u64,0u64);for k in &list{if from.slot(addr,k)!=U256::ZERO{gone+=1}if to.slot(addr,k)!=U256::ZERO{came+=1}}
                if is_system(addr){s.sys_slots=s.sys_slots-gone+came}else{s.user_slots=s.user_slots-gone+came}
            }
        }
        s
    }
    /// `code_caps::validate_snapshot`'s outcome for the summarized state (Ok/Err; on several failures a
    /// code failure is named first, then B_code_max, then CODE_CAP).
    pub fn code_check(&self,limits:CodeLimits)->Result<(),String>{
        if limits.runtime<24577{return Err("R9: B_code_max below 24577".into())}
        if limits.runtime!=self.runtime{return Err("summary taken against another B_code_max".into())}
        if self.code_missing>0{return Err("missing runtime bytes".into())}
        if self.code_mismatch>0{return Err("runtime code hash mismatch".into())}
        if self.over_runtime>0{return Err("snapshot exceeds B_code_max".into())}
        if self.code_total>u128::from(limits.logical){return Err("snapshot exceeds CODE_CAP".into())}
        Ok(())
    }
    /// The inventory of a state that passed `code_check` (it is at most CODE_CAP).
    pub fn inventory(&self)->u64{u64::try_from(self.code_total).unwrap_or(u64::MAX)}
}

/// `user_phase::drop_unreferenced_code` on `db`, a state reached from `from` (whose summary is
/// `from_summary`) through `touched`. During a block the contracts map only grows (revm's commit inserts;
/// only this drop removes), so when its size is unchanged and no touched account lost its code, the map
/// is `from`'s, which was clean: nothing to drop. Otherwise the drop runs whole. Returns whether `db`'s
/// map is clean after it.
pub fn drop_unreferenced_code(from:&InMemoryDB,from_summary:&Summary,db:&mut InMemoryDB,touched:&Touched)->bool{
    let lost=touched.iter().any(|(a,_)|from.accounts.get(a).filter(|x|exists(x)&&x.info.code_hash!=KECCAK_EMPTY)
        .map_or(false,|x|db.accounts.get(a).filter(|y|exists(y)).map_or(true,|y|y.info.code_hash!=x.info.code_hash)));
    if from_summary.contracts_clean&&!lost&&db.contracts.len()==from.contracts.len(){return true}
    crate::user_phase::drop_unreferenced_code(db);true
}

#[cfg(test)] mod tests;
