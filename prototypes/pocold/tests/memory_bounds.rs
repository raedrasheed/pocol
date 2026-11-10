//! PR7 (C-1 S5): resident memory and per-block work that stay O(window), not O(height). The pieces the
//! eviction is built from, each checked on its own: the canonical window type and the checkpoint record
//! codec it shares with the node's canonical index, and that index (canon.redb).
use pocold::chain::Canon;

fn hash(i:u64)->[u8;32]{let mut h=[0u8;32];h[..8].copy_from_slice(&(i+1).to_be_bytes());h[31]=0xc5;h}
/// Genesis and heights 1..=n.
fn window(n:u64)->Canon{let mut c=Canon::new(hash(0));c.extend((1..=n).map(hash));c}

#[test] fn a_window_moved_down_and_back_up_equals_the_one_never_moved(){
    let mut c=window(10);let all=c.clone();
    assert_eq!((c.base(),c.len()),(0,11));assert_eq!(c.iter().copied().collect::<Vec<_>>(),(0..=10).map(hash).collect::<Vec<_>>());
    // From base 0 the genesis slot goes with height 1: the base is never 1.
    let mut bases=vec![];for _ in 0..3{let h=c.pop_low().unwrap();bases.push((c.base(),h));}
    assert_eq!(bases,vec![(2,hash(1)),(3,hash(2)),(4,hash(3))]);
    assert_eq!(c.len(),11,"the heights below the window still count");
    assert_eq!((c.get(0),c.get(1),c.get(3),c.get(4),c.get(10),c.get(11)),(Some(&hash(0)),None,None,Some(&hash(4)),Some(&hash(10)),None));
    assert_eq!(c[0],hash(0));assert_eq!(c[7],hash(7));
    assert_eq!(c.iter().copied().collect::<Vec<_>>(),[0,4,5,6,7,8,9,10].map(hash).to_vec(),"genesis, then the window");
    assert_eq!(c.iter_from(2).copied().collect::<Vec<_>>(),(4..=10).map(hash).collect::<Vec<_>>(),"a start below the window starts at its base");
    assert_eq!(c.iter_from(6).copied().collect::<Vec<_>>(),(6..=10).map(hash).collect::<Vec<_>>());
    assert!(c.iter_from(11).next().is_none());
    assert_ne!(c,all);
    for h in (1..=3).rev(){c.push_low(hash(h));}
    assert_eq!(c.base(),0,"back at height 1 the window is base 0 again");assert_eq!(c,all,"and equal to the window never moved");
    assert_eq!(Canon::at_base(hash(0),1),Canon::new(hash(0)),"a window from height 1 is base 0");
    let mut r=Canon::at_base(hash(0),4);r.extend((4..=10).map(hash));let mut d=window(10);for _ in 0..3{d.pop_low();}
    assert_eq!(r,d,"a window restored at its base equals one evicted down to it");
}
#[test] fn a_drain_keeps_genesis_and_the_heights_below_the_window(){
    let mut c=window(6);assert_eq!(c.drain_from(4),(4..=6).map(hash).collect::<Vec<_>>());assert_eq!(c.len(),4);
    assert_eq!(c.drain_from(9),Vec::<[u8;32]>::new(),"nothing above the head");
    let mut w=window(8);w.pop_low();w.pop_low();assert_eq!(w.base(),3);
    assert_eq!(w.drain_from(3),(3..=8).map(hash).collect::<Vec<_>>(),"the whole window from its base");
    assert_eq!((w.len(),w.get(0)),(3,Some(&hash(0))));w.extend([hash(30)]);assert_eq!(w[3],hash(30));
    assert!(window(0).pop_low().is_none(),"genesis never leaves");
    let mut e=Canon::at_base(hash(0),5);assert!(e.pop_low().is_none());assert_eq!(e.base(),5);
}
#[test] #[should_panic(expected="canonical height 2 is not held")]
fn indexing_a_height_below_the_window_panics(){let mut c=window(5);c.pop_low();c.pop_low();let _=c[2];}

/// The record codec a snapshot uses, on its own (the node's canonical index stores the same bytes): an
/// evicted entry's record (slim, pruned, offloaded: no summary, no root) is the smallest, REC_MIN bytes; one
/// with a summary and a root is REC_MAX; each reads back equal, and a cut or a longer record is refused.
#[test] fn a_record_reads_back_from_its_bytes_and_a_cut_or_longer_one_is_refused(){
    use pocold::checkpoint::{flags,rec_bytes,rec_from,Rec,REC_MAX,REC_MIN};
    use revm::primitives::{alloy_primitives::U512,Address,U256};
    let min=Rec{hash:[3;32],body_off:Some(77),import_seq:5,total_work:U256::from(9u64),flags:flags::PRUNED|flags::SLIM|flags::OFFLOADED|flags::OFF,cum_txs:1,cum_tx_bytes:2,
        cum_log_bytes:3,drawn:Address::repeat_byte(4),summary:None,root:None};
    let b=rec_bytes(&min);assert_eq!(b.len(),REC_MIN);assert_eq!(rec_from(&b).unwrap(),min);
    let sum=pocol_lp4_execution::state_summary::Summary{runtime:1,code_total:2,code_missing:3,code_mismatch:4,over_runtime:5,user_accounts:6,user_slots:7,sys_slots:8,balance:U512::from(9u64),contracts_clean:true};
    let full=Rec{flags:flags::STORED|flags::WORLD|flags::SUMMARY|flags::OFF,summary:Some(sum),root:Some([8;32]),..min.clone()};
    let b=rec_bytes(&full);assert_eq!(b.len(),REC_MAX);assert_eq!(rec_from(&b).unwrap(),full);
    assert!(rec_from(&b[..b.len()-1]).is_err(),"a cut record");let mut x=b.clone();x.push(0);assert!(rec_from(&x).is_err(),"a record with a byte more");
    assert!(rec_from(&rec_bytes(&Rec{flags:flags::WORLD,..full.clone()})).is_err(),"a root without a stored version");
    let none=Rec{body_off:None,flags:0,..min.clone()};assert_eq!(rec_from(&rec_bytes(&none)).unwrap(),none,"no journal record: the offset reads back as None");
}

/// The node's canonical index (canon.redb) on its own: rows read back by height, by hash and as a range; a row
/// overwritten by another block's drops that block's hash; the rows survive a reopen; and a file of another
/// genesis, one marked damaged and one that is not a redb file are each set aside and a new index is created.
#[test] fn the_canonical_index_reads_back_its_rows_and_sets_aside_a_foreign_or_damaged_file(){
    use pocold::checkpoint::{flags,Rec};use pocold::node::canon::{Index,DAMAGED,FILE};
    use revm::primitives::{Address,U256};
    let dir=std::env::temp_dir().join(format!("pocold-pr7-canon-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);std::fs::create_dir_all(&dir).unwrap();
    let rec=|h:u64,tag:u8|Rec{hash:{let mut x=hash(h);x[30]=tag;x},body_off:Some(40+h),import_seq:h,total_work:U256::from(h),flags:flags::PRUNED|flags::SLIM|flags::OFFLOADED|flags::OFF,
        cum_txs:0,cum_tx_bytes:0,cum_log_bytes:0,drawn:Address::ZERO,summary:None,root:None};
    let g=[9u8;32];
    {let ix=Index::open(&dir,&g).unwrap();assert!(!ix.rebuilt,"a new file is not a rebuilt one");
        ix.put(&(1..=5).map(|h|(h,rec(h,0))).collect::<Vec<_>>()).unwrap();
        assert_eq!(ix.at(3).unwrap(),Some(rec(3,0)));assert_eq!(ix.at(6).unwrap(),None);
        assert_eq!(ix.height_of(&rec(4,0).hash).unwrap(),Some(4));
        assert_eq!(ix.range(2,3).unwrap(),(2..=4).map(|h|(h,rec(h,0))).collect::<Vec<_>>());
        ix.put(&[(4,rec(4,1))]).unwrap();
        assert_eq!((ix.height_of(&rec(4,0).hash).unwrap(),ix.height_of(&rec(4,1).hash).unwrap()),(None,Some(4)),"an overwritten row's block is no longer found");
        let before=ix.reads();ix.at(1).unwrap();ix.height_of(&[1;32]).unwrap();ix.range(1,5).unwrap();assert_eq!(ix.reads()-before,3,"one lookup each");
        ix.sync().unwrap();}
    {let ix=Index::open(&dir,&g).unwrap();assert!(!ix.rebuilt);assert_eq!(ix.at(5).unwrap(),Some(rec(5,0)),"the rows survive a reopen");}
    {let ix=Index::open(&dir,&[8u8;32]).unwrap();assert!(ix.rebuilt,"a file of another genesis is set aside");assert_eq!(ix.at(5).unwrap(),None);
        ix.mark_damaged("a test");assert!(ix.put(&[(1,rec(1,0))]).is_err(),"a damaged index refuses writes");}
    assert!(dir.join(DAMAGED).exists());
    {let ix=Index::open(&dir,&[8u8;32]).unwrap();assert!(ix.rebuilt,"a file marked damaged is set aside");assert!(!dir.join(DAMAGED).exists());}
    std::fs::write(dir.join(FILE),b"not a redb file").unwrap();
    {let ix=Index::open(&dir,&g).unwrap();assert!(ix.rebuilt,"a file that is not a redb file is set aside");ix.put(&[(1,rec(1,0))]).unwrap();}
    let _=std::fs::remove_dir_all(&dir);
}
