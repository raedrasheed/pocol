//! PR7 (C-1 S5): resident memory and per-block work that stay O(window), not O(height). The pieces the
//! eviction is built from, each checked on its own: the canonical window type and the checkpoint record
//! codec it shares with the node's canonical index, and that index (canon.redb).
use pocold::chain::{Canon,Chain,Spec};
use pocold::types::*;
use pocol_lp4_execution::{devnet::{self,DevnetConfig},txsign::Key};
use revm::primitives::{Address,B256};
use std::collections::HashMap;

// ---------------- chain helpers (as tests/consensus.rs) ----------------
struct Net{chain:Chain,keys:HashMap<Address,Key>}
fn cfg()->DevnetConfig{let mut c=DevnetConfig::new(777910,1_700_000_000,&["a","b","c"]);c.hash_rate=1024;c.d_att=73;c.tau=40;c.c=8;c}
fn admitted()->Spec{let d=devnet::build(&cfg()).unwrap();Spec::new(d.admit(&pocold::single_host_local()).unwrap_or_else(|e|panic!("{e}")).0)}
fn net()->Net{let d=devnet::build(&cfg()).unwrap();let keys=d.miners.iter().map(|m|(m.id,Key::from_secret(&m.key.secret()).unwrap())).collect();Net{chain:Chain::new(admitted()),keys}}
fn try_seal(n:&Net,mut h:pocold::header::Header,body:Body,attempt:u32)->Option<Block>{
    let p=n.chain.full_entry(&h.parent_hash).unwrap();let ctx=n.chain.child_ctx(&p,attempt).unwrap();
    let tid=template_id(&h.ut_raw);let mut shares=vec![];let mut x=0u64;
    loop{if (x as u128)>=ctx.assignment.nonce_max(){return None}let ph=pow_hash(&tid,x);if le_target(&ph,&ctx.t_share){if le_target(&ph,&ctx.target){break}shares.push(x);}x+=1;}
    let owner=addr(&ctx.assignment.owner(x).unwrap());
    h.nonce=x;shares.truncate(256);h.shares=shares;reseal(&mut h);
    h.winner_sig=n.keys[&owner].sign_hash(&B256::from(pocold::hashes::win_msg(&tid,x,&share_root(&h))));reseal(&mut h);
    Some(Block{header:h,body})
}
/// A fallback block on `parent` at attempt `a` (or the next attempt with a solution).
fn fallback(n:&Net,parent:[u8;32],a:u32)->Block{let p=n.chain.full_entry(&parent).unwrap();
    for a in a..a+16{let (h,_,_)=n.chain.build_fallback(&p,a).unwrap();if let Some(b)=try_seal(n,h,Body::default(),a){return b}}panic!("no fallback solution")}
fn import(n:&mut Net,b:&Block){let (e,_)=n.chain.validate(b,None,0).unwrap_or_else(|e|panic!("h={} {e:?}",b.header.h));n.chain.insert(e);n.chain.retain();}
/// `count` fallback blocks on the head.
fn extend(n:&mut Net,count:usize)->Vec<Block>{(0..count).map(|_|{let p=n.chain.head;let b=fallback(n,p,0);import(n,&b);b}).collect()}
fn temp(tag:&str)->std::path::PathBuf{let d=std::env::temp_dir().join(format!("pocold-pr7-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&d);d}

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

/// A test-only format-1 encoder: the snapshot layout before C-1 S5 (no eviction or anchor parameters, no window
/// fields), sealed as before.
fn encode_v1(s:&pocold::checkpoint::Snapshot)->Vec<u8>{
    let mut w=Vec::new();let u=|w:&mut Vec<u8>,x:u64|w.extend_from_slice(&x.to_be_bytes());let o=|w:&mut Vec<u8>,x:Option<u64>|match x{Some(v)=>{w.push(1);w.extend_from_slice(&v.to_be_bytes())}None=>w.push(0)};
    let hs=|w:&mut Vec<u8>,v:&[[u8;32]]|{w.extend_from_slice(&(v.len() as u64).to_be_bytes());for h in v{w.extend_from_slice(h)}};
    w.extend_from_slice(b"POCKPT01");w.extend_from_slice(&s.genesis);let p=&s.params;
    for x in [p.r_keep,p.side_cap,p.k_hist,p.side_max]{u(&mut w,x)}o(&mut w,p.history_window);o(&mut w,p.slim_depth);w.push(u8::from(p.keep_full));
    u(&mut w,s.binding.records);u(&mut w,s.binding.end);w.extend_from_slice(&s.binding.hash);
    w.extend_from_slice(&s.head);for x in [s.next_seq,s.slim_cursor,s.history_cursor,s.evicted_total]{u(&mut w,x)}
    hs(&mut w,&s.unslimmed);hs(&mut w,&s.evicted_order);match &s.protect{Some(v)=>{w.push(1);hs(&mut w,v)}None=>w.push(0)}u(&mut w,s.entries.len() as u64);
    for r in &s.entries{w.extend_from_slice(&pocold::checkpoint::rec_bytes(r))}
    let seal=revm::primitives::keccak256(&w);w.extend_from_slice(&seal.0);w
}
/// One row of a canonical index (what the node's would hold below a window base).
struct OneRow(u64,pocold::checkpoint::Rec);
impl pocold::checkpoint::CanonIndex for OneRow{fn at(&self,h:u64)->Result<Option<pocold::checkpoint::Rec>,String>{Ok((h==self.0).then(||self.1.clone()))}}

/// Checkpoint format 2 (C-1 S5): a snapshot of a store-mode chain with its entries below height `base` left out (as
/// they are once evicted) restores at that window base, from the block below it that the canonical index holds; the
/// restored chain's snapshot is the one restored. An index without that block, or with another one, and entries
/// that do not start at the base are refused, leaving the chain as it was. A format-1 snapshot (every entry, no
/// window fields) still reads back and restores under the new parameters (the one-time migration).
#[test] fn a_snapshot_restores_at_its_window_base_from_the_block_the_index_holds_below_it(){
    use pocold::checkpoint::*;use pocold::node::{Config,Limits,Node};
    // The base is below the K_hist cut (38), as an eviction base always is: no stored version at or near it.
    let mut h=net();let blocks=extend(&mut h,70);let dir=temp("v2");let base=20u64;
    let conf=Config{dir:dir.clone(),name:"v2".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Limits{state_store:true,checkpoint_every:0,..Default::default()}};
    let mut n=Node::open(conf,admitted()).unwrap();for b in &blocks{n.import(b.clone(),None).unwrap();}
    let full=fingerprint(&n.chain).unwrap();let below=rec_of_meta(n.chain.canonical_at(base-1).unwrap());
    let canon:Vec<[u8;32]>=n.chain.canonical.iter().copied().collect();let height:HashMap<[u8;32],u64>=n.chain.blocks.values().map(|e|(e.hash,e.height)).collect();
    drop(n);
    let mut s=full.clone();s.entries.retain(|r|height[&r.hash]==0||height[&r.hash]>=base);(s.canon_base,s.base_parent,s.base_parent_work)=(base,below.hash,below.total_work);
    assert_eq!(decode(&encode_snapshot(&s)).unwrap(),s,"format 2 reads back");
    let v1=decode(&encode_v1(&full)).unwrap();
    assert!(full.anchor_cursor>0,"positive control: the forward prune moved the anchor cursor");
    assert_eq!(v1,Snapshot{v1:true,params:Params{evict_depth:None,anchor_step:None,..full.params},anchor_cursor:0,..full.clone()},"format 1 reads back, without the new fields");
    let spec=admitted();let gh=spec.genesis_hash();
    let rec=pocold::store::open(&dir,&gh).unwrap();let history=pocold::history::History::open(&dir).unwrap();
    let st:std::sync::Arc<dyn pocol_lp4_execution::node_trie::TrieStore>=std::sync::Arc::new(pocold::state_store::StateStore::open(&dir).unwrap());
    let fresh=||{let mut c=Chain::new(admitted());let p=&full.params;(c.r_keep,c.side_cap,c.history_window,c.slim_depth,c.keep_full_states,c.evict_depth,c.anchor_step)=(p.r_keep,p.side_cap as usize,p.history_window,p.slim_depth,p.keep_full,p.evict_depth,p.anchor_step);
        c.enable_store(st.clone()).unwrap();c};
    let go=|c:&mut Chain,s:&Snapshot,ix:&dyn CanonIndex|restore(c,s,&JournalSource{offsets:&rec.offsets,journal:&rec.journal,records:rec.offsets.len(),history:&history,canon:ix},&st);
    let mut c=fresh();
    for (ix,want) in [(&NoIndex as &dyn CanonIndex,"does not reach"),(&OneRow(base-1,Rec{total_work:below.total_work+revm::primitives::U256::from(1u64),..below.clone()}),"does not reach")]{
        let e=go(&mut c,&s,ix).unwrap_err();assert!(e.contains(want),"{e}");assert_eq!((c.blocks.len(),c.canonical.len()),(1,1),"refused: the chain is as it was");}
    let mut cut=s.clone();cut.entries.retain(|r|height[&r.hash]!=base);
    let e=go(&mut c,&cut,&OneRow(base-1,below.clone())).unwrap_err();assert!(e.contains("parent not restored"),"{e}");
    go(&mut c,&s,&OneRow(base-1,below.clone())).unwrap();
    assert_eq!((c.canonical.base(),c.head,c.canonical.len()),(base,full.head,canon.len()));
    assert!((1..base as usize).all(|h|c.canonical.get(h).is_none())&&(base as usize..canon.len()).all(|h|c.canonical[h]==canon[h]),"the window from its base, the heights below it not held");
    assert_eq!(fingerprint(&c).unwrap(),s,"the restored chain's snapshot is the one restored");
    drop(c);let mut m=fresh();m.evict_depth=Some(1000);m.anchor_step=Some(7);
    go(&mut m,&v1,&NoIndex).unwrap();assert_eq!((m.canonical.base(),m.head,m.blocks.len()),(0,full.head,full.entries.len()),"a format-1 snapshot restores whole under the new parameters");
    drop(m);drop(rec);let _=std::fs::remove_dir_all(&dir);
}
