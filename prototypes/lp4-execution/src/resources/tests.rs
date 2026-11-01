use super::*;
use crate::{apply::{self,BlockInput,EvidenceEffect,ParentResult,Proposer},devnet::{self,DevnetConfig}};
use revm::primitives::{Address,B256,U256};

fn network_like(miners:usize,h_close:u64,h_end:u64)->DevnetConfig{
    let labels:Vec<String>=(0..miners).map(|i|format!("m{i}")).collect();let refs:Vec<&str>=labels.iter().map(|s|s.as_str()).collect();
    let mut c=DevnetConfig::new(777920,1_800_000_000,&refs);
    c.d=16;c.w_w=64;c.s_max=256;c.m_max=256;c.h_close=h_close;c.h_end=h_end;c.ret=128;c.e_win=2048;c.u=2200;c.w_act=128;c.theta_inact=1024;
    // Network-scale caps (governance.md), independent of the single-host devnet defaults.
    c.r_max=2;c.sys_slot_max=131072;c.slot_cap=4_000_000;c.acct_cap=1_000_000;c.code_cap=1<<30;c.txbytes_cap=1<<33;c.tx_cap=10_000_000;c
}

#[test] fn r18_network_values_and_caps64_minimum_boundary(){
    let net=devnet::build(&network_like(4,1000,2000)).unwrap();let cp=net.profile.cp().clone();
    let b=r18(&cp).unwrap();
    // Network-like CP: S=256, M=256, RRM=8192 (devnet), K=64, D=16, E=4, RET=128.
    assert_eq!(b.slot_min,5+5*8192+256+64*(2*256+2)+2+3*17+52);
    assert!(b.keys_max()<=2048&&b.accts_max()<=512,"{b:?}");
    // caps64 minimum: SYS_SLOT_MAX = slot_min passes, slot_min-1 fails R18.
    let mut m=cp.clone();m.SYS_SLOT_MAX=b.slot_min as u32;assert!(r18(&m).is_ok());
    m.SYS_SLOT_MAX=b.slot_min as u32-1;assert!(r18(&m).unwrap_err()[0].starts_with("SYS_SLOT_MAX"));
    let mut m=cp.clone();m.SYS_KEYS_BLOCK_MAX=b.keys_max() as u32-1;assert!(r18(&m).is_err());
    let mut m=cp.clone();m.SYS_ACCT_BLOCK_MAX=b.accts_max() as u32-1;assert!(r18(&m).is_err());
    println!("R18-DERIVED {}",serde_json::json!({"slotMin":b.slot_min,"keys":[b.keys_normal,b.keys_closing,b.keys_final],"accounts":[b.accts_normal,b.accts_closing,b.accts_final]}));
}

/// Worst-case stress: 256 identities (M_max), every height carries S_max shares
/// so 256 distinct owners per height, four evidence punishments in one block,
/// closing cursor over 256 records with RET=128, finalization at H_END.
#[test] fn measured_worst_case_stays_within_derived_bounds_and_caps(){
    let net=devnet::build(&network_like(256,60,80)).unwrap();let cp=net.profile.cp().clone();let b=derive(&cp);
    let ids:Vec<Address>=net.miners.iter().map(|m|m.id).collect();
    let mut state=net.genesis.state().clone();let mut prev:Option<ParentResult>=None;let mut base=U256::from(1_000_000_000u64);
    let (mut kn,mut kc,mut kf,mut an,mut ac,mut af,mut gas,mut call)=(0usize,0usize,0usize,0usize,0usize,0usize,0u64,0u64);let mut final_seen=false;
    for h in 1..=80u64{
        let evidence=if h==30{(0..4).map(|i|EvidenceEffect::Punish{accused:ids[200+i]}).collect()}else{vec![]};
        let input=BlockInput{height:h,timestamp:1_800_000_000+60*h,base_fee:base,gas_limit:8_000_000,prevrandao:B256::ZERO,
            proposer:if h%2==0{Proposer::Signed(ids[(h as usize*7)%256])}else{Proposer::Fallback},parent_result:prev.clone(),evidence,evidence_raw:&[],transactions:&[],ancestors:vec![],cum_log_bytes:0};
        let o=apply::apply_block(&state,&net.genesis,&net.profile,&input).unwrap_or_else(|e|panic!("h={h} {e:?}"));
        base=crate::fees::next_base_fee(base,o.gas_used,8_000_000).unwrap();
        let (k,a)=(o.meter.keys.len(),o.meter.accounts.len());
        if o.is_final{kf=kf.max(k);af=af.max(a);final_seen=true}else if o.facts.closing{kc=kc.max(k);ac=ac.max(a)}else{kn=kn.max(k);an=an.max(a)}
        gas=gas.max(o.system_gas);call=call.max(o.meter.max_call_gas);if h%10==0||o.facts.closing{println!("GAS h={h} sys={} maxcall={} calls={} keys={k} accts={a}",o.system_gas,o.meter.max_call_gas,o.meter.calls);}
        assert!(sys_slots(&o.state)<=b.slot_min,"slots {} > {}",sys_slots(&o.state),b.slot_min);
        state=o.state.clone();
        let w=ids[h as usize%256];let shares:Vec<(u64,Address)>=(0..256u64).map(|i|(i*3+1,ids[((h+i) as usize)%256])).collect();
        prev=Some(ParentResult{block_hash:B256::from(U256::from(h)),winner:w,shares});
        if o.is_final{break}
    }
    assert!(final_seen,"final block reached");
    assert!(kn as u64<=b.keys_normal&&kc as u64<=b.keys_closing&&kf as u64<=b.keys_final,"keys {kn}/{kc}/{kf} vs {b:?}");
    assert!(an as u64<=b.accts_normal&&ac as u64<=b.accts_closing&&af as u64<=b.accts_final,"accounts {an}/{ac}/{af} vs {b:?}");
    assert!(call<=apply::PER_CALL_GAS&&gas<=cp.G_SYS,"gas call={call} block={gas}");
    println!("R18-MEASURED {}",serde_json::json!({"keysMax":[kn,kc,kf],"accountsMax":[an,ac,af],"maxBlockSysGas":gas,"maxCallGas":call,"G_SYS":cp.G_SYS}));
}

#[test] fn r21_retention_formula(){assert_eq!(r21_retained_bytes(32,2,100,1000),34*(100*ACCOUNT_BYTES+1000*SLOT_BYTES));}

/// A02/A13 SYS gas envelope fixture at a gate-valid REG_RECORDS_MAX (R16). Scenario 1 is
/// the P7 worst case plus evidence that invalidates pending pots. Scenario 2 makes the
/// beginBlock sweep delete records (76 tombstones, short E_win) before closing.
/// Records the largest gas of every SYSTEM call name in any phase class;
/// `admission::GAS_ENVELOPE` must dominate every value.
fn gas_scenario(c:&DevnetConfig,evidence_at:&dyn Fn(u64,&[Address])->Vec<EvidenceEffect>,owners:usize,seen:&mut std::collections::BTreeMap<&'static str,u64>)->([u64;3],usize){
    let net=devnet::build(c).unwrap();
    let ids:Vec<Address>=net.miners.iter().map(|m|m.id).collect();
    let mut state=net.genesis.state().clone();let mut prev:Option<ParentResult>=None;let mut base=U256::from(1_000_000_000u64);let mut block_max=[0u64;3];let mut fewest=usize::MAX;
    for h in 1..=c.h_end{
        let input=BlockInput{height:h,timestamp:1_800_000_000+60*h,base_fee:base,gas_limit:8_000_000,prevrandao:B256::ZERO,
            proposer:if h%2==0{Proposer::Signed(ids[(h as usize*7)%owners])}else{Proposer::Fallback},parent_result:prev.clone(),evidence:evidence_at(h,&ids),evidence_raw:&[],transactions:&[],ancestors:vec![],cum_log_bytes:0};
        let o=apply::apply_block(&state,&net.genesis,&net.profile,&input).unwrap_or_else(|e|panic!("h={h} {e:?}"));
        base=crate::fees::next_base_fee(base,o.gas_used,8_000_000).unwrap();
        for (name,g) in &o.meter.call_gas{let e=seen.entry(*name).or_default();*e=(*e).max(*g);}
        let class=if o.is_final{2}else if o.facts.closing{1}else{0};block_max[class]=block_max[class].max(o.system_gas);
        state=o.state.clone();fewest=fewest.min(apply::record_list(&state).len());
        let shares:Vec<(u64,Address)>=(0..256u64).map(|i|(i*3+1,ids[((h+i) as usize)%owners])).collect();
        prev=Some(ParentResult{block_hash:B256::from(U256::from(h)),winner:ids[h as usize%owners],shares});
        if o.is_final{return (block_max,fewest)}
    }
    panic!("final block not reached")
}
#[test] fn sys_gas_envelope_fixture(){
    let mut seen:std::collections::BTreeMap<&'static str,u64>=Default::default();
    let mut c=network_like(256,60,80);c.reg_records_max=16384;
    let (worst,_)=gas_scenario(&c,&|h,ids|match h{30=>(0..4).map(|i|EvidenceEffect::Punish{accused:ids[200+i]}).collect(),
        31=>(0..4u64).map(|i|EvidenceEffect::PunishAndInvalidate{accused:ids[100+i as usize],j:30-i}).collect(),_=>vec![]},256,&mut seen);
    let mut c2=c.clone();c2.e_win=16;c2.u=40;
    let (sweep,fewest)=gas_scenario(&c2,&|h,ids|if (2..=20).contains(&h){(0..4).map(|i|EvidenceEffect::Punish{accused:ids[100+4*(h as usize-2)+i]}).collect()}else{vec![]},100,&mut seen);
    assert!(fewest<=256-76,"the sweep scenario must delete the 76 tombstoned records, fewest={fewest}");
    println!("GAS-ENVELOPE {}",serde_json::json!({"perCall":seen,"blockMax":{"worst":worst,"sweep":sweep},"G_SYS":c.cp()[44].to_u64()}));
    for (name,g) in &seen{
        let bound=crate::admission::GAS_ENVELOPE.iter().find(|(n,_)|n==name).map(|x|x.1);
        assert!(bound.map_or(false,|b|*g<=b),"envelope misses {name}={g} bound={bound:?}");
    }
}

/// Thread-local allocation meter (only allocations of the measuring thread are counted,
/// so parallel tests do not disturb it).
mod alloc_meter{
    use std::alloc::{GlobalAlloc,Layout,System};use std::cell::Cell;
    thread_local!{static ON:Cell<bool>=const{Cell::new(false)};static BYTES:Cell<isize>=const{Cell::new(0)};static PEAK:Cell<isize>=const{Cell::new(0)};}
    fn add(d:isize){let _=ON.try_with(|on|if on.get(){let _=BYTES.try_with(|b|{b.set(b.get()+d);let _=PEAK.try_with(|p|p.set(p.get().max(b.get())));});});}
    pub struct Meter;
    unsafe impl GlobalAlloc for Meter{
        unsafe fn alloc(&self,l:Layout)->*mut u8{let p=unsafe{System.alloc(l)};if !p.is_null(){add(l.size() as isize)}p}
        unsafe fn dealloc(&self,p:*mut u8,l:Layout){unsafe{System.dealloc(p,l)};add(-(l.size() as isize))}
        unsafe fn realloc(&self,p:*mut u8,l:Layout,n:usize)->*mut u8{let q=unsafe{System.realloc(p,l,n)};if !q.is_null(){add(n as isize-l.size() as isize)}q}
    }
    #[global_allocator] static A:Meter=Meter;
    /// Net heap bytes `f` leaves allocated (what it returns keeps alive).
    pub fn measure<T>(f:impl FnOnce()->T)->(T,isize){ON.with(|o|o.set(true));BYTES.with(|b|b.set(0));let r=f();let n=BYTES.with(|b|b.get());ON.with(|o|o.set(false));(r,n)}
    /// Highest heap bytes alive at any moment while `f` runs (relative to its start).
    pub fn peak<T>(f:impl FnOnce()->T)->(T,isize){ON.with(|o|o.set(true));BYTES.with(|b|b.set(0));PEAK.with(|p|p.set(0));let r=f();let n=PEAK.with(|p|p.get());ON.with(|o|o.set(false));(r,n)}
}
/// A13: the snapshot model bounds the bytes a real clone allocates, for spread slots,
/// concentrated slots at power-of-two boundaries, grown-then-cleared tables (normalized)
/// and a state produced by actual block execution.
#[test] fn snapshot_model_bounds_measured_clone_bytes(){
    use revm::{InMemoryDB,primitives::{AccountInfo,U256 as RU}};
    let check=|db:&InMemoryDB,label:&str|{
        let accounts=db.accounts.len() as u128;let slots=db.accounts.values().map(|a|a.storage.len()).sum::<usize>() as u128;let codes=db.contracts.len() as u128;
        let (copy,bytes)=alloc_meter::measure(||db.clone());drop(copy);
        let bound=memory::snapshot(accounts,slots,codes).unwrap();
        assert!(bytes>0&&(bytes as u128)<=bound,"{label}: clone allocated {bytes} B > model {bound} B (accounts {accounts}, slots {slots}, codes {codes})");
        (bytes,bound)
    };
    let addr=|i:u64|revm::primitives::Address::from_word(RU::from(0x5000_0000u64+i).into());
    for n in [1u64,7,8,100,1000,3500]{
        let mut db=InMemoryDB::default();for i in 0..n{db.insert_account_info(addr(i),AccountInfo{nonce:1,..Default::default()});db.insert_account_storage(addr(i),RU::from(1),RU::from(1)).unwrap();}
        check(&db,&format!("{n} accounts x 1 slot"));
    }
    for s in [7u64,8,14,15,448,449,3584,3585]{
        let mut db=InMemoryDB::default();db.insert_account_info(addr(0),AccountInfo{nonce:1,..Default::default()});
        for k in 0..s{db.insert_account_storage(addr(0),RU::from(k+1),RU::from(1)).unwrap();}
        check(&db,&format!("1 account x {s} slots"));
    }
    // 3000 slots written then cleared to one: normalization keeps the table proportional.
    let mut db=InMemoryDB::default();db.insert_account_info(addr(0),AccountInfo{nonce:1,..Default::default()});
    for k in 0..3000u64{db.insert_account_storage(addr(0),RU::from(k+1),RU::from(1)).unwrap();}
    for k in 1..3000u64{db.insert_account_storage(addr(0),RU::from(k+1),RU::ZERO).unwrap();}
    crate::user_phase::normalize(&mut db,&[addr(0)]);assert_eq!(db.accounts[&addr(0)].storage.len(),1);
    let (b,_)=check(&db,"grown then cleared");assert!(b<8192,"the cleared table was shrunk: {b} B");
    // A state from real execution (genesis plus blocks).
    let net=devnet::build(&network_like(16,60,80)).unwrap();check(net.genesis.state(),"genesis");
    println!("MEMORY-MODEL {}",serde_json::json!({"accountEntry":memory::ACCOUNT_ENTRY,"slotEntry":memory::SLOT_ENTRY,"codeEntry":memory::CODE_ENTRY,"db":memory::DB}));
}

/// A13 obligation 4: the execution model bounds the peak heap of adversarial executions at the
/// largest per-execution gas (30M): TSTORE, cold SLOAD and LOG floods, the largest single-frame
/// memory returned whole, memory-carrying recursion to the depth the 63/64 rule allows, and a
/// large identity-precompile output held as return data. Each runs on its own engine copy.
#[test] fn execution_model_bounds_measured_adversarial_peaks(){
    use revm::{InMemoryDB,primitives::{AccountInfo,Address,Bytecode,Bytes,TxEnv,TxKind,U256 as RU}};
    let gas=memory::execution_gas(8_000_000);assert_eq!(gas,30_000_000);let bound=memory::execution(gas,24_577).unwrap();
    assert!(std::mem::size_of::<revm::CallFrame>().max(std::mem::size_of::<revm::CreateFrame>()) as u128<=memory::FRAME_BYTES);
    let p1=|x:u8|vec![0x60,x];let p2=|x:u16|{let b=x.to_be_bytes();vec![0x61,b[0],b[1]]};let p4=|x:u32|{let b=x.to_be_bytes();vec![0x63,b[0],b[1],b[2],b[3]]};
    let cat=|v:Vec<Vec<u8>>|v.concat();
    let w=memory::words(gas-60_000) as u32;
    let programs:Vec<(&str,Vec<u8>)>=vec![
        ("tstore flood",cat(vec![p1(1),vec![0x5b],p1(1),vec![0x81,0x5d],p1(1),vec![0x01],p1(2),vec![0x56]])),
        ("cold sload flood",cat(vec![p1(1),vec![0x5b,0x80,0x54,0x50],p1(1),vec![0x01],p1(2),vec![0x56]])),
        ("log flood",cat(vec![vec![0x5b],p1(32),p1(0),vec![0xa0],p1(0),vec![0x56]])),
        ("largest memory returned",cat(vec![p1(1),p4((w-1)*32),vec![0x52],p4(w*32),p1(0),vec![0xf3]])),
        ("recursion with memory 1 KiB",cat(vec![p1(1),p2(32*32-32),vec![0x52],p1(0),p1(0),p2(32*32),p1(0),p1(0),vec![0x30,0x5a,0xf1,0x50],p2(32*32),p1(0),vec![0xf3]])),
        ("recursion with memory 16 KiB",cat(vec![p1(1),p2(512*32-32),vec![0x52],p1(0),p1(0),p2(512*32),p1(0),p1(0),vec![0x30,0x5a,0xf1,0x50],p2(512*32),p1(0),vec![0xf3]])),
        ("identity output held",cat(vec![p1(1),p4(((w/2)-1)*32),vec![0x52],p1(0),p1(0),p4((w/2)*32),p1(0),p1(0),p1(4),vec![0x5a,0xf1,0x50,0x3d],p1(0),vec![0xf3]])),
    ];
    let caller=Address::with_last_byte(0xca);let target=Address::with_last_byte(0xee);let mut out=Vec::new();
    for (label,code) in programs{
        let mut db=InMemoryDB::default();db.insert_account_info(caller,AccountInfo{balance:RU::from(10u64).pow(RU::from(24u64)),..Default::default()});
        let c=Bytecode::new_raw(Bytes::from(code));db.insert_account_info(target,AccountInfo{code_hash:c.hash_slow(),code:Some(c),..Default::default()});
        let (_,clone)=alloc_meter::measure(||db.clone());
        let tx=TxEnv{caller,transact_to:TxKind::Call(target),gas_limit:gas as u64,gas_price:RU::ZERO,value:RU::ZERO,data:Bytes::new(),nonce:None,chain_id:None,..Default::default()};
        let (r,peak)=alloc_meter::peak(||crate::native::execute(&db,tx).expect(label));
        let used=r.result.gas_used();
        assert!(peak>0&&(peak as u128)<=bound+(clone.max(0) as u128),"{label}: peak {peak} B > model {bound} B");
        out.push(serde_json::json!({"case":label,"gasUsed":used,"peakBytes":peak,"success":r.result.is_success()}));
    }
    println!("EXECUTION-MODEL {}",serde_json::json!({"gas":gas as u64,"boundBytes":bound as u64,"cases":out}));
}

/// A13 obligation 4 (execution classes): a loaded account's storage maps start empty and
/// allocate nothing, so a map's own share belongs to the account's first slot.
#[test] fn loaded_accounts_start_with_unallocated_storage_maps(){
    use revm::primitives::{Account,AccountInfo,EvmStorageSlot,HashMap,U256 as RU};
    // The account's code is shared (a refcounted clone); only the storage maps are measured.
    let info=AccountInfo::default();
    let (a,d)=(Account::from(info.clone()),revm::db::DbAccount{info:info.clone(),..Default::default()});
    assert_eq!(a.storage.capacity(),0);assert_eq!(d.storage.capacity(),0);
    let (maps,bytes)=alloc_meter::measure(||(HashMap::<RU,EvmStorageSlot>::default(),HashMap::<RU,RU>::default()));
    assert_eq!(bytes,0,"empty storage maps allocated {bytes} B");drop(maps);
    let c=memory::classes();let rate=|i:usize|c[i].bytes as f64/c[i].gas as f64;
    assert!(c[2].bytes==c[0].bytes+c[1].bytes+memory::table_bytes(memory::STATE_SLOT)+memory::table_bytes(memory::SLOT_ENTRY)&&c[2].gas==c[0].gas+c[1].gas);
    println!("EXECUTION-CLASSES {}",serde_json::json!({"accountPerGas":rate(0),"slotPerGas":rate(1),"accountWithFirstSlotPerGas":rate(2)}));
}
