//! A13 §2b: resident memory of full-cap snapshot clones against the snapshot model, so the
//! allocator overhead and fragmentation of the largest term are measured rather than assumed.
//! Linux only (/proc/self/statm). Its own test binary, so no other test allocates meanwhile.
use revm::{InMemoryDB,primitives::{AccountInfo,Address,Bytecode,Bytes,U256}};
use pocol_lp4_execution::resources::memory;

#[cfg(target_os="linux")]
fn rss()->u128{let s=std::fs::read_to_string("/proc/self/statm").unwrap();s.split_whitespace().nth(1).unwrap().parse::<u128>().unwrap()*4096}

/// The devnet caps (DevnetConfig::new): ACCT_CAP 20,000 + 5 programs, SLOT_CAP 50,000 +
/// SYS_SLOT_MAX 65,536, one code entry per account. Slots are spread over all accounts (a storage
/// table per account, the layout with the most table overhead). Ten clones are kept alive and the
/// resident growth is compared with ten times `memory::snapshot`.
#[test] fn full_cap_snapshot_clones_stay_within_the_model_in_rss(){
    #[cfg(not(target_os="linux"))]{eprintln!("RSS-NOT-MEASURED");return}
    #[cfg(target_os="linux")]{
    let (accounts,slots)=(20_005u64,50_000u64+65_536);
    let addr=|i:u64|Address::from_word(U256::from(0x7000_0000u64+i).into());
    let mut db=InMemoryDB::default();
    for i in 0..accounts{let c=Bytecode::new_raw(Bytes::from(vec![0x60,(i%251) as u8,0x00]));db.insert_account_info(addr(i),AccountInfo{nonce:1,code_hash:c.hash_slow(),code:Some(c),..Default::default()});}
    for k in 0..slots{db.insert_account_storage(addr(k%accounts),U256::from(k+1),U256::from(1)).unwrap();}
    let model=memory::snapshot(u128::from(accounts),u128::from(slots),u128::from(accounts)+2).unwrap();
    const K:u128=10;let before=rss();
    let clones:Vec<InMemoryDB>=(0..K).map(|_|db.clone()).collect();
    let grown=rss().saturating_sub(before);let ratio=grown as f64/(K*model) as f64;
    println!("RSS-SNAPSHOT {}",serde_json::json!({"accounts":accounts,"slots":slots,"clones":K as u64,"modelPerSnapshotBytes":model as u64,
        "rssGrowthBytes":grown as u64,"rssOverModel":(ratio*10000.0).round()/10000.0}));
    assert!(grown<=K*model,"resident growth {grown} B exceeds {K} x the snapshot model {model} B");
    drop(clones);
    }
}
