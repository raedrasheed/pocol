//! FN-5 (C10, D-V3-23 restricted to the cap fields): a cap raise scheduled at fork 3 switches ACCT_CAP, SLOT_CAP,
//! CODE_CAP and SYS_SLOT_MAX at its height (`ProfileIdentity::cp_at`), and only there; ParamGate evaluates R1-R26 at
//! the genesis CP and at the raised CP and refuses a raise that lowers a cap, raises none, is unscheduled, is on a
//! spec-1 genesis or breaks a rule. Disposable fixture profiles only; no production values.
use pocol_lp4_execution::{admission::{self,Admitted,NodeLocal,Timing},apply::{self,BlockInput,ParentResult,Proposer},devnet::{self,DevnetConfig},
    native::NativeBlockContext,profile::CapRaise,receipts,state_summary::Summary,txsign::Tx,user_phase::{self,Halt,UserContext}};
use revm::{InMemoryDB,primitives::{AccountInfo,Address,B256,Bytecode,Bytes,U256}};

const CHAIN:u64=777961;
/// The fork-3 (cap raise) height of the fixtures, with fork 2 (a nonce-rule no-op under nonceMode 1) before it.
const H:u64=10;
/// The cap headroom the admission's timing workload needs; a test fills it in its own copy of the genesis state.
const ROOM:u64=64;
fn cfg(tune:impl FnOnce(&mut DevnetConfig))->DevnetConfig{
    let mut c=DevnetConfig::new(CHAIN,1_800_000_000,&["a"]);tune(&mut c);c
}
fn counts(d:&devnet::Devnet)->Summary{Summary::scan(d.genesis.state(),d.profile.cp().B_code_max)}
fn timing(d:&devnet::Devnet)->Timing{admission::measure(&d.spec_bytes,&d.alloc,1).unwrap()}
/// The candidate admitted with the schedule [(1,0),(2,H-5),(3,H)] and `raise`.
fn admit(d:&mut devnet::Devnet,t:&Timing,local:&NodeLocal,raise:Option<CapRaise>)->Result<Admitted,admission::Rejection>{
    d.forks=if raise.is_some(){vec![(1,0),(2,H-5),(3,H)]}else{vec![(1,0),(2,H-5)]};
    admission::admit_with(&d.request(local,Some(t)),raise.as_ref())
}
fn base_raise(d:&devnet::Devnet)->CapRaise{let cp=d.profile.cp();CapRaise{code_cap:cp.CODE_CAP,slot_cap:cp.SLOT_CAP,acct_cap:cp.ACCT_CAP,sys_slot_max:cp.SYS_SLOT_MAX}}
fn ctx(a:&Admitted,h:u64)->NativeBlockContext{NativeBlockContext{height:U256::from(h),timestamp:U256::from(1_800_000_000+4*h),gas_limit:U256::from(a.profile().cp().GAS_LIMIT),base_fee:U256::from(100),prevrandao:B256::ZERO}}
fn tx(nonce:u64,to:Option<Address>,data:Vec<u8>,gas:u64)->Vec<u8>{
    Tx{chain_id:CHAIN,nonce,max_priority:U256::from(7),max_fee:U256::from(200),gas,to,value:U256::from(u64::from(to.is_some())),data:Bytes::from(data)}.sign_1559(&devnet::account_key("alice"))
}
fn run(a:&Admitted,db:&InMemoryDB,raw:&[Vec<u8>],h:u64)->user_phase::UserCandidate{
    user_phase::execute_for_block(db,raw,&ctx(a,h),2_000_000,1000,a.profile(),UserContext::default()).unwrap()
}
/// Init code that returns `runtime` (at most 255 bytes).
fn init(runtime:&[u8])->Vec<u8>{let len=runtime.len() as u8;let mut b=vec![0x60,len,0x60,12,0x60,0,0x39,0x60,len,0x60,0,0xf3];b.extend_from_slice(runtime);b}

#[test]
fn admission_is_unchanged_without_a_raise_and_evaluates_both_cps_with_one(){
    let mut d=devnet::build(&cfg(|_|{})).unwrap();let t=timing(&d);let local=NodeLocal::single_host();
    // Dormant: admit_with(None) is admit, field for field of the report, the genesis and the profile.
    d.forks=vec![(1,0),(2,H-5)];
    let (a,b)=(admission::admit(&d.request(&local,Some(&t))).unwrap(),admission::admit_with(&d.request(&local,Some(&t)),None).unwrap());
    assert_eq!(a.profile(),b.profile());assert_eq!(a.genesis().root(),b.genesis().root());
    assert_eq!((a.report().binding,&a.report().notes,a.report().historical_bytes),(b.report().binding,&b.report().notes,b.report().historical_bytes));
    for h in [0,H,u64::MAX]{assert_eq!(a.profile().cp_at(h),a.profile().cp());}assert_eq!(a.profile().cap_raise(),None);
    // A raise of ACCT_CAP alone: admitted, the genesis and its hash unchanged, the four caps switched at H.
    let r=CapRaise{acct_cap:d.profile.cp().ACCT_CAP*2,..base_raise(&d)};
    let up=admit(&mut d,&t,&local,Some(r)).unwrap();
    assert_eq!(up.genesis_hash(),a.genesis_hash());assert_eq!(up.genesis().root(),a.genesis().root());assert_eq!(up.profile().cp(),a.profile().cp());
    assert_eq!(up.profile().cp_at(H-1),up.profile().cp());assert_eq!(up.profile().cp_at(H).ACCT_CAP,r.acct_cap);assert_eq!(up.profile().cap_raise(),Some((H,r)));
    assert!(up.report().notes.iter().any(|n|n.starts_with("cap raise at fork 3: ACCT_CAP")&&n.ends_with("R1-R26 evaluated at both CPs")),"{:?}",up.report().notes);
    assert!(up.report().historical_bytes>a.report().historical_bytes,"R21 is reported at the raised caps");
    println!("R21 retained-state bound: {} B at the genesis CP, {} B with ACCT_CAP x2",a.report().historical_bytes,up.report().historical_bytes);
}

#[test]
fn admission_refuses_a_raise_that_is_unscheduled_lowering_empty_or_breaks_a_rule(){
    let mut d=devnet::build(&cfg(|_|{})).unwrap();let t=timing(&d);let local=NodeLocal::single_host();let b=base_raise(&d);
    let r24=|e:admission::Rejection,why:&str|{assert_eq!(e.rules,vec!["R24".to_string()]);assert!(e.notes.iter().any(|n|n.contains(why)),"{why}: {:?}",e.notes);};
    // Fork 3 without its raise, and a raise without fork 3.
    d.forks=vec![(1,0),(2,H-5),(3,H)];r24(admission::admit(&d.request(&local,Some(&t))).err().unwrap(),"fork version 3 is a cap raise");
    d.forks=vec![(1,0),(2,H-5)];r24(admission::admit_with(&d.request(&local,Some(&t)),Some(&CapRaise{acct_cap:b.acct_cap+1,..b})).err().unwrap(),"without a fork version 3");
    // Lowering any cap, or raising none.
    for low in [CapRaise{acct_cap:b.acct_cap-1,slot_cap:b.slot_cap+1,..b},CapRaise{slot_cap:b.slot_cap-1,acct_cap:b.acct_cap+1,..b},
        CapRaise{code_cap:b.code_cap-1,acct_cap:b.acct_cap+1,..b},CapRaise{sys_slot_max:b.sys_slot_max-1,acct_cap:b.acct_cap+1,..b}]{
        r24(admit(&mut d,&t,&local,Some(low)).err().unwrap(),"may not lower a cap");
    }
    r24(admit(&mut d,&t,&local,Some(b)).err().unwrap(),"must raise at least one cap");
    // A rule broken only at the raised CP: without the simnet retentionStress exception, R21's retained state at an
    // ACCT_CAP of 2^40 is over HIST_TRIE_BUDGET while the genesis CP's is not.
    let strict=NodeLocal{retention_stress:false,..NodeLocal::single_host()};
    d.forks=vec![(1,0),(2,H-5)];let ok=admission::admit(&d.request(&strict,Some(&t))).unwrap();assert!(ok.report().historical_bytes<=admission::HIST_TRIE_BUDGET);
    let e=admit(&mut d,&t,&strict,Some(CapRaise{acct_cap:1<<40,..b})).err().expect("R21 fails at the raised CP");
    assert_eq!(e.rules,vec!["R21".to_string()]);assert!(e.notes.iter().any(|n|n=="cap raise: the raised CP violates [R21]"),"{:?}",e.notes);
    // The same raise with the exception is admitted, with the exception's condition.
    let x=admit(&mut d,&t,&local,Some(CapRaise{acct_cap:1<<40,..b})).unwrap();assert!(x.report().r21_exception);
}

#[test]
fn a_full_account_cap_halts_below_the_raise_height_and_creates_from_it(){
    // Headroom for the admission's timing workload; the genesis state is then filled to the cap exactly.
    let base=devnet::build(&cfg(|_|{})).unwrap();let accts=counts(&base).user_accounts+ROOM;
    let mut d=devnet::build(&cfg(|c|c.acct_cap=accts)).unwrap();let t=timing(&d);
    let r=CapRaise{acct_cap:accts+1,..base_raise(&d)};let a=admit(&mut d,&t,&NodeLocal::single_host(),Some(r)).unwrap();
    let mut db=a.genesis().state().clone();for i in 0..ROOM{db.insert_account_info(Address::from([0xe0,i as u8,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,1]),AccountInfo::from_balance(U256::from(1)));}
    assert_eq!(Summary::scan(&db,32768).user_accounts,accts,"positive control: ACCT_CAP reached");let db=&db;
    let to=Address::from([0x91;20]);let raw=[tx(0,Some(to),vec![],50_000)];
    for h in [1,H-1]{let o=run(&a,db,&raw,h);assert_eq!(o.halts(),&[(0,Halt::AcctCap)],"h={h}");assert!(o.state().accounts.get(&to).map_or(true,|x|x.info().is_none()));}
    for h in [H,H+1]{let o=run(&a,db,&raw,h);assert!(o.halts().is_empty(),"h={h}");assert_eq!(Summary::scan(o.state(),32768).user_accounts,accts+1);
        assert_eq!(o.receipts()[0],receipts::encode(2,true,21000,&[]).unwrap());}
    // The next new account halts again at the raised cap.
    let o=run(&a,db,&[raw[0].clone(),tx(1,Some(Address::from([0x92;20])),vec![],50_000)],H);assert_eq!(o.halts(),&[(1,Halt::AcctCap)]);
}

#[test]
fn a_full_slot_cap_halts_below_the_raise_height_and_stores_from_it(){
    let base=devnet::build(&cfg(|_|{})).unwrap();let slots=counts(&base).user_slots+ROOM;
    let mut d=devnet::build(&cfg(|c|c.slot_cap=slots)).unwrap();let t=timing(&d);
    let r=CapRaise{slot_cap:slots+1,..base_raise(&d)};let a=admit(&mut d,&t,&NodeLocal::single_host(),Some(r)).unwrap();
    let mut db=a.genesis().state().clone();let bob=devnet::account_key("bob").address();
    for k in 0..ROOM{db.insert_account_storage(bob,U256::from(k),U256::from(1)).unwrap();}
    assert_eq!(Summary::scan(&db,32768).user_slots,slots,"positive control: SLOT_CAP reached");let db=&db;
    // A deployment whose constructor stores one nonzero slot (SSTORE(0, 1)) and returns an empty runtime.
    let raw=[tx(0,None,vec![0x60,1,0x60,0,0x55,0x60,0,0x60,0,0xf3],200_000)];
    for h in [1,H-1]{assert_eq!(run(&a,db,&raw,h).halts(),&[(0,Halt::SlotCap)],"h={h}");}
    for h in [H,H+5]{let o=run(&a,db,&raw,h);assert!(o.halts().is_empty(),"h={h}");assert_eq!(Summary::scan(o.state(),32768).user_slots,slots+1);}
}

#[test]
fn a_full_code_cap_fails_a_create_below_the_raise_height_and_deploys_from_it(){
    let base=devnet::build(&cfg(|_|{})).unwrap();let total=counts(&base).code_total as u64+ROOM;
    let mut d=devnet::build(&cfg(|c|c.code_cap=total)).unwrap();let t=timing(&d);
    let r=CapRaise{code_cap:total+32,..base_raise(&d)};let a=admit(&mut d,&t,&NodeLocal::single_host(),Some(r)).unwrap();
    let mut db=a.genesis().state().clone();let filler=Bytecode::new_raw(Bytes::from(vec![0u8;ROOM as usize]));
    db.insert_account_info(Address::from([0xe1;20]),AccountInfo{nonce:1,code_hash:filler.hash_slow(),code:Some(filler),..Default::default()});
    assert_eq!(Summary::scan(&db,32768).code_total as u64,total,"positive control: CODE_CAP reached");let db=&db;
    let raw=[tx(0,None,init(&[0;32]),200_000)];let created=devnet::account_key("alice").address().create(0);
    let code=|o:&user_phase::UserCandidate|o.state().accounts.get(&created).and_then(|x|x.info()).and_then(|i|i.code).map_or(0,|c|c.original_bytes().len());
    for h in [1,H-1]{let o=run(&a,db,&raw,h);assert_eq!(code(&o),0,"h={h}: CODE_CAP fails the CREATE");assert_eq!(Summary::scan(o.state(),32768).code_total as u64,total);}
    for h in [H,H+1]{let o=run(&a,db,&raw,h);assert_eq!(code(&o),32,"h={h}");assert_eq!(Summary::scan(o.state(),32768).code_total as u64,total+32);}
}

/// SYS_SLOT_MAX is checked on every block's parent and candidate (the SYSTEM calls write system storage): a parent
/// at exactly the genesis SYS_SLOT_MAX makes the next block invalid below the raise height and valid from it.
#[test]
fn a_full_system_storage_invalidates_the_block_below_the_raise_height_and_not_from_it(){
    let mut d=devnet::build(&cfg(|_|{})).unwrap();let t=timing(&d);let b=base_raise(&d);let ids:Vec<Address>=d.miners.iter().map(|m|m.id).collect();
    let local=NodeLocal::single_host();
    let raise=CapRaise{sys_slot_max:b.sys_slot_max+100,..b};
    // Two profiles that differ in the raise height only: at 2 and at 3.
    let at=|d:&mut devnet::Devnet,h3:u64|{d.forks=vec![(1,0),(2,1),(3,h3)];admission::admit_with(&d.request(&local,Some(&t)),Some(&raise)).unwrap()};
    let (early,late)=(at(&mut d,2),at(&mut d,3));
    let cp=early.profile().cp().clone();
    let input=|h:u64,prev:Option<ParentResult>|BlockInput{height:h,timestamp:1_800_000_000+4*h,base_fee:U256::from(1_000_000_000u64),gas_limit:cp.GAS_LIMIT,prevrandao:B256::ZERO,
        proposer:Proposer::Fallback,parent_result:prev,evidence:vec![],evidence_raw:&[],transactions:&[],ancestors:vec![(0,d.genesis_hash)],cum_log_bytes:0};
    let pr=|h:u64|Some(ParentResult{block_hash:B256::from(U256::from(h)),winner:ids[0],shares:vec![(10,ids[0])]});
    let o1=apply::apply_block(early.genesis().state(),early.genesis(),early.profile(),&input(1,None)).unwrap();
    let o1b=apply::apply_block(late.genesis().state(),late.genesis(),late.profile(),&input(1,None)).unwrap();
    assert_eq!(o1.state_root,o1b.state_root,"below both heights the profiles agree");
    let mut full=o1.state.clone();let sys=apply::program_address(4);let now=Summary::scan(&full,cp.B_code_max).sys_slots;
    for k in 0..u64::from(cp.SYS_SLOT_MAX)-now{full.insert_account_storage(sys,U256::from(k)+U256::from(1u64<<40),U256::from(1)).unwrap();}
    assert_eq!(Summary::scan(&full,cp.B_code_max).sys_slots,u64::from(cp.SYS_SLOT_MAX),"positive control: the parent is at the genesis bound");
    let refused=apply::apply_block(&full,late.genesis(),late.profile(),&input(2,pr(1)));
    assert!(matches!(refused,Err(apply::ApplyError::Genesis(ref m)) if m.starts_with("candidate")),"below the raise height the child is over the bound: {:?}",refused.as_ref().err());
    let sum=Summary::scan(&full,cp.B_code_max);
    assert!(apply::apply_block_on(&full,&sum,late.genesis(),late.profile(),&input(2,pr(1))).is_err(),"the overlay path refuses it too");
    let ok=apply::apply_block(&full,early.genesis(),early.profile(),&input(2,pr(1))).unwrap();
    assert!(Summary::scan(&ok.state,cp.B_code_max).sys_slots>u64::from(cp.SYS_SLOT_MAX),"from the raise height the child passes the genesis bound");
    let on=apply::apply_block_with(&full,None,Some(&sum),early.genesis(),early.profile(),&input(2,pr(1))).unwrap();assert_eq!(on.state_root,ok.state_root);
}
