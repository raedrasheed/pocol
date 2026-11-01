use super::*;
use revm::primitives::{AccountInfo,Bytecode,Bytes};

struct Rng(u64);
impl Rng{fn next(&mut self)->u64{self.0^=self.0<<13;self.0^=self.0>>7;self.0^=self.0<<17;self.0}fn below(&mut self,n:u64)->u64{self.next()%n}}

/// Codes: none, two runtimes (4 and 1 byte, against a counting limit of 3 bytes: one is over), and two
/// broken accounts (a hash with no bytes; bytes under another hash).
fn info(r:&mut Rng)->AccountInfo{
    let mut i=AccountInfo{balance:U256::from(r.below(1<<40))*U256::from(1u64<<40),nonce:r.below(5),code_hash:KECCAK_EMPTY,code:None};
    match r.below(8){
        0=>{let c=Bytecode::new_raw(Bytes::from_static(&[0x60,1,0x60,0]));i.code_hash=c.hash_slow();i.code=Some(c);}
        1=>{let c=Bytecode::new_raw(Bytes::from_static(&[0x00]));i.code_hash=c.hash_slow();i.code=Some(c);}
        2=>{i.code_hash=B256::from(U256::from(r.next()));}
        3=>{let c=Bytecode::new_raw(Bytes::from_static(&[0x5b]));i.code_hash=B256::from(U256::from(7u64));i.code=Some(c);}
        _=>{}
    }
    i
}
fn addr(r:&mut Rng)->Address{if r.below(6)==0{crate::apply::program_address(r.below(5) as usize)}else{Address::from_word(B256::from(U256::from(r.below(400))))}}
fn mutate(r:&mut Rng,db:&InMemoryDB)->InMemoryDB{
    let mut d=db.clone();
    for _ in 0..1+r.below(25){
        let a=addr(r);
        match r.below(8){
            0=>{d.accounts.remove(&a);}
            1=>{if let Some(x)=d.accounts.get_mut(&a){x.account_state=AccountState::NotExisting}}
            2=>{if let Some(x)=d.accounts.get_mut(&a){x.storage.clear();x.account_state=AccountState::StorageCleared}}
            3=>d.insert_account_info(a,info(r)),
            4=>{if let Some(x)=d.accounts.get_mut(&a){x.info.balance=U256::from(r.next())}}
            _=>{if !d.accounts.contains_key(&a){d.insert_account_info(a,info(r))}
                for _ in 0..1+r.below(5){d.insert_account_storage(a,U256::from(r.below(40)),U256::from(r.below(3))).unwrap();}}
        }
    }
    d
}

#[test]
fn moved_summaries_equal_scans_through_random_states(){
    let mut r=Rng(0x51a7e5u64.wrapping_mul(0x9e3779b97f4a7c15));let mut nonzero=Summary::scan(&InMemoryDB::default(),3);
    for case in 0..40{
        let mut db=InMemoryDB::default();for _ in 0..r.below(200){db=mutate(&mut r,&db);}
        let mut s=Summary::scan(&db,3);
        for step in 0..25{
            let next=mutate(&mut r,&db);let t=Touched::between(&db,&next);
            let m=s.moved(&db,&next,&t);let full=Summary::scan(&next,3);
            assert_eq!(Summary{contracts_clean:full.contracts_clean,..m},full,"case {case} step {step}");
            s=full;db=next;
        }
        if s.code_missing>0&&s.code_mismatch>0&&s.over_runtime>0&&s.sys_slots>0&&s.user_slots>0{nonzero=s;}
    }
    // Positive control: some state had every kind of count.
    assert!(nonzero.code_missing>0&&nonzero.code_mismatch>0&&nonzero.over_runtime>0&&nonzero.sys_slots>0&&nonzero.user_slots>0,"{nonzero:?}");
}

#[test]
fn the_code_check_follows_validate_snapshot(){
    let lim=CodeLimits{runtime:24577,logical:10};
    let mut db=InMemoryDB::default();let c=Bytecode::new_raw(Bytes::from(vec![0u8;8]));
    db.insert_account_info(Address::from([1;20]),AccountInfo{code_hash:c.hash_slow(),code:Some(c.clone()),..Default::default()});
    let s=Summary::scan(&db,lim.runtime);assert_eq!(s.code_check(lim).is_ok(),code_caps::validate_snapshot(&db,lim).is_ok());assert!(s.code_check(lim).is_ok());
    db.insert_account_info(Address::from([2;20]),AccountInfo{code_hash:c.hash_slow(),code:Some(c.clone()),..Default::default()});
    let s=Summary::scan(&db,lim.runtime);assert_eq!(s.code_check(lim),code_caps::validate_snapshot(&db,lim));assert!(s.code_check(lim).is_err());
    db.insert_account_info(Address::from([3;20]),AccountInfo{code_hash:B256::from([9;32]),code:None,..Default::default()});
    let s=Summary::scan(&db,lim.runtime);assert!(s.code_check(lim).is_err()&&code_caps::validate_snapshot(&db,lim).is_err());
    assert!(Summary::scan(&db,100).code_check(CodeLimits{runtime:100,logical:10}).is_err());
}

#[test]
fn the_drop_is_skipped_only_when_nothing_could_be_dropped(){
    let mut r=Rng(99);
    for _ in 0..200{
        let mut db=InMemoryDB::default();for _ in 0..r.below(60){db=mutate(&mut r,&db);}
        crate::user_phase::drop_unreferenced_code(&mut db);let s=Summary::scan(&db,3);assert!(s.contracts_clean);
        let mut next=mutate(&mut r,&db);let t=Touched::between(&db,&next);
        let mut reference=next.clone();crate::user_phase::drop_unreferenced_code(&mut reference);
        assert!(drop_unreferenced_code(&db,&s,&mut next,&t));
        let mut a:Vec<_>=next.contracts.keys().copied().collect();let mut b:Vec<_>=reference.contracts.keys().copied().collect();a.sort();b.sort();
        assert_eq!(a,b);assert!(Summary::scan(&next,3).contracts_clean);
    }
}

#[test]
fn a_contracts_map_that_grew_with_unreferenced_code_is_dropped(){
    // Not produced by execution (revm's commit skips a destroyed account's code), but the skip must not
    // rely on that: a grown map is dropped whole.
    let mut db=InMemoryDB::default();db.insert_account_info(Address::from([1;20]),AccountInfo::from_balance(U256::from(1)));
    crate::user_phase::drop_unreferenced_code(&mut db);let s=Summary::scan(&db,3);assert!(s.contracts_clean);
    let mut next=db.clone();let c=Bytecode::new_raw(Bytes::from_static(&[0x60,1]));next.contracts.insert(c.hash_slow(),c);
    let t=Touched::between(&db,&next);assert!(t.is_empty());
    assert!(drop_unreferenced_code(&db,&s,&mut next,&t));assert_eq!(next.contracts.len(),db.contracts.len());
}
