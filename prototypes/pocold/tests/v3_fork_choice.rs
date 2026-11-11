//! V3-04: timing is local fork-choice metadata, never header validity.
//! Private fixture chains only; no network listener or production keys.
use std::{collections::HashMap,path::{Path,PathBuf},sync::atomic::{AtomicU64,Ordering}};
use pocol_lp4_execution::{devnet::{self,DevnetConfig},txsign::Key};
use pocold::{chain::{Chain,Spec},node::{Config,Limits,Node},store::{self,FirstImport},types::*};
use revm::primitives::{Address,B256};

struct Net{chain:Chain,keys:HashMap<Address,Key>,cfg:DevnetConfig}
fn spec(c:&DevnetConfig,extra_rel:u32)->Spec{
    let mut local=pocold::single_host_local();local.delta_rel+=extra_rel;
    Spec::new(devnet::build(c).unwrap().admit(&local).unwrap().0)
}
fn net(v3:bool)->Net{
    let mut cfg=DevnetConfig::new(777937,1_700_000_000,&["fork-miner"]);
    // R7 has room for the restart test's alternate, still gate-valid local
    // deadline. The original 73-second fixture sat exactly on that gate.
    cfg.hash_rate=16;cfg.c=8;cfg.d_att=91;cfg.r_max=1;
    cfg.spec_version=if v3{2}else{1};cfg.nonce_mode=1;
    let d=devnet::build(&cfg).unwrap();let keys=d.miners.iter().map(|m|(m.id,Key::from_secret(&m.key.secret()).unwrap())).collect();
    Net{chain:Chain::new(spec(&cfg,0)),keys,cfg}
}
fn block(n:&Net,parent:[u8;32],signed:bool,attempt:u32,start:u64)->Block{
    let p=n.chain.full_entry(&parent).unwrap();let ctx=n.chain.child_ctx(&p,attempt).unwrap();
    let (mut h,_,_)=if signed{
        n.chain.build_signed(&p,&n.keys[&ctx.proposer],ctx.window.fallback_timestamp(n.chain.spec.cp().dFbWait).unwrap(),Body::default(),vec![]).unwrap()
    }else{n.chain.build_fallback(&p,attempt).unwrap()};
    let tid=template_id(&h.ut_raw);let mut shares=Vec::new();
    for nonce in start..start+1_000_000{
        let ph=pow_hash(&tid,nonce);
        if le_target(&ph,&ctx.target){
            h.nonce=nonce;h.shares=shares;reseal(&mut h);
            let owner=addr(&ctx.assignment.owner(nonce).unwrap());
            h.winner_sig=n.keys[&owner].sign_hash(&B256::from(pocold::hashes::win_msg(&tid,nonce,&share_root(&h))));
            reseal(&mut h);return Block{header:h,body:Body::default()}
        }
        if shares.len()<usize::from(n.chain.spec.cp().S_max)&&le_target(&ph,&ctx.t_share){shares.push(nonce);}
    }panic!("fixture solution not found")
}
fn accept(n:&mut Net,b:&Block,timely:bool){
    let (mut e,_)=n.chain.validate(b,None,0).unwrap();
    e.first_import=Some(FirstImport{received_ms:123,parent_received_ms:100,timely});n.chain.insert(e);
}
fn dir(label:&str)->PathBuf{
    static NEXT:AtomicU64=AtomicU64::new(0);
    std::env::temp_dir().join(format!("pocol-v3-fork-{label}-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)))
}
fn config(d:&Path,store:bool)->Config{Config{name:"fork-test".into(),dir:d.to_path_buf(),listen:None,peers:vec![],miner:None,stop_at:None,log:false,rpc:None,
    limits:Limits{r_keep:4,side_cap:2,state_store:store,checkpoint_every:2,slim_depth:Some(4),history_window:Some(2),..Default::default()}}}
#[cfg(feature="v3-preview")]
fn deadline(n:&Node,parent:[u8;32])->u128{
    let p=n.chain.get(&parent).unwrap();let local=n.chain.spec.local();
    pocold::timing::AttemptWindow::new(p.ts,0,n.chain.spec.cp().D_att).unwrap().deadline_ms(p.first_import.unwrap().received_ms,u64::from(local.delta_rel),n.chain.spec.cp().dFbWait).unwrap()
}
fn at(n:&mut Node,b:&Block,ms:u128)->bool{n.import_clock=Some(ms);n.import(b.clone(),None).unwrap()}

/// Independent whole-path oracle: total work first, then the children of the
/// first divergence. It does not call Chain::better/key or inspect a tip's flag.
fn best(c:&Chain)->[u8;32]{
    let path=|mut h:[u8;32]|{let mut p=vec![h];while let Some(parent)=c.blocks[&h].parent{p.push(parent);h=parent}p.reverse();p};
    let mut best=c.spec.genesis_hash();
    for e in c.blocks.values(){
        let b=&c.blocks[&best];let better=if e.total_work!=b.total_work{e.total_work>b.total_work}else if e.hash==best{false}else{
            let (x,y)=(path(e.hash),path(best));let i=x.iter().zip(&y).take_while(|(a,b)|a==b).count();
            let key=|h:&[u8;32]|{let e=&c.blocks[h];(u8::from(c.spec.v3_active(e.height)&&!e.first_import.unwrap().timely),u8::from(!e.signed),e.import_seq,e.hash)};
            key(&x[i])<key(&y[i])
        };if better{best=e.hash}
    }best
}
fn twenty_orders(v3:bool){
    let mut build=net(v3);let g=build.chain.head;let mut dag=Vec::new();
    let fb=block(&build,g,false,0,0);let a=block(&build,g,true,0,0);let b=block(&build,g,true,0,a.header.nonce+1);
    assert_ne!(a.hash(),b.hash());
    for (root,timely) in [(fb,true),(a,false),(b,true)]{
        let mut p=root.hash();accept(&mut build,&root,timely);dag.push((root,timely));
        for depth in 0..2{let child=block(&build,p,depth==1,0,0);p=child.hash();accept(&mut build,&child,!timely);dag.push((child,!timely));}
    }
    for order in 0..20{
        let mut n=net(v3);let mut remaining=dag.clone();let mut seed=0x4352_0001u64+order;
        while !remaining.is_empty(){
            let ready:Vec<_>=remaining.iter().enumerate().filter(|(_,x)|n.chain.get(&x.0.header.parent_hash).is_some()).map(|(i,_)|i).collect();
            seed^=seed<<13;seed^=seed>>7;seed^=seed<<17;
            let (b,timely)=remaining.remove(ready[seed as usize%ready.len()]);accept(&mut n,&b,timely);
            assert_eq!(n.chain.head,best(&n.chain),"order {order}, height {}",b.header.h);
        }
    }
}
#[test]fn twenty_import_orders_keep_the_legacy_d133_order(){twenty_orders(false)}

#[test]fn legacy_node_keeps_three_field_journals_and_signed_precedence(){
    let n=net(false);let g=n.chain.head;let fb=block(&n,g,false,0,0);let sg=block(&n,g,true,0,0);let d=dir("legacy");
    {let mut node=Node::open(config(&d,false),spec(&n.cfg,0)).unwrap();
        assert!(at(&mut node,&fb,u128::MAX));assert!(at(&mut node,&sg,u128::MAX));assert_eq!(node.chain.head,sg.hash());
        assert!(node.chain.blocks.values().all(|e|e.first_import.is_none()));}
    let r=store::open(&d,&g).unwrap();for p in r.payloads(){assert!(store::decode_record_ref(&p).unwrap().first_import.is_none());assert_eq!(pocold::rlp::decode(&p).unwrap().list().unwrap().len(),3);}drop(r);
    let node=Node::open(config(&d,false),spec(&n.cfg,0)).unwrap();assert_eq!(node.chain.head,sg.hash());drop(node);std::fs::remove_dir_all(d).unwrap();
}

#[cfg(feature="v3-preview")]
mod preview{
    use super::*;
    #[test]fn twenty_import_orders_use_the_timely_key_at_the_first_divergence(){twenty_orders(true)}

    #[test]fn timely_fallback_beats_late_signed_and_early_fallback_loses(){
        for early in [false,true]{let n=net(true);let g=n.chain.head;let fb=block(&n,g,false,0,0);let sg=block(&n,g,true,0,0);let d=dir("boundaries");
            let mut node=Node::open(config(&d,false),spec(&n.cfg,0)).unwrap();let tj=deadline(&node,g);
            let local=node.chain.spec.local();let low=tj-u128::from(local.delta+local.capital_phi())*1000;let high=tj+u128::from(local.capital_phi())*1000;
            assert!(at(&mut node,&fb,if early{low-1}else{low}));assert_eq!(node.chain.head,fb.hash());
            let changed=at(&mut node,&sg,if early{high}else{high+1});assert_eq!(changed,early);
            assert_eq!(node.chain.head,if early{sg.hash()}else{fb.hash()});
            assert_eq!(node.chain.get(&fb.hash()).unwrap().first_import.unwrap().timely,!early);
            assert_eq!(node.chain.get(&sg.hash()).unwrap().first_import.unwrap().timely,early);
            drop(node);std::fs::remove_dir_all(d).unwrap();
        }
    }

    #[test]fn replay_checkpoint_and_changed_local_deadline_keep_the_original_flags(){
        for store in [false,true]{let n=net(true);let g=n.chain.head;let fb=block(&n,g,false,0,0);let sg=block(&n,g,true,0,0);let d=dir("restart");
            let (first,head)={let mut node=Node::open(config(&d,store),spec(&n.cfg,0)).unwrap();let tj=deadline(&node,g);
                at(&mut node,&fb,tj);at(&mut node,&sg,tj+1);assert_eq!(node.chain.head,fb.hash());
                if store{node.checkpoint_now().unwrap();}
                (node.chain.blocks.iter().map(|(h,e)|(*h,e.first_import)).collect::<HashMap<_,_>>(),node.chain.head)};
            // A larger local deadline would call that signed import timely if
            // replay recalculated instead of using the original persisted flag.
            for lost_index in [false,true]{if lost_index{std::fs::remove_file(d.join(pocold::node::canon::FILE)).unwrap();}
                let node=Node::open(config(&d,store),spec(&n.cfg,1)).unwrap();assert_eq!(node.chain.head,head);
                for (h,want) in &first{assert_eq!(node.chain.get(h).unwrap().first_import,*want,"original stamp {h:?}");}
                if store&&!lost_index{assert_eq!(node.stats.resumed_from,Some(2));}
            }
            std::fs::remove_dir_all(d).unwrap();
        }
    }

    #[test]fn refetch_keeps_original_timing_but_receives_a_new_import_sequence(){
        for store in [false,true]{let n=net(true);let g=n.chain.head;
            let fb=block(&n,g,false,0,0);let late=block(&n,g,true,0,0);let timely=block(&n,g,true,0,late.header.nonce+1);
            let d=dir("refetch");let mut cfg=config(&d,store);cfg.limits.side_cap=0;
            let (stamp,seq,head)={let mut node=Node::open(cfg,spec(&n.cfg,0)).unwrap();let tj=deadline(&node,g);
                at(&mut node,&fb,tj);at(&mut node,&late,tj+1);assert_eq!(node.chain.head,fb.hash());
                // side_cap=0 evicts the late signed sibling immediately.
                assert!(node.chain.get(&late.hash()).is_none());
                at(&mut node,&timely,tj);assert_eq!(node.chain.head,timely.hash());
                node.import_clock=Some(tj);assert!(!node.import_with(late.clone(),None,true).unwrap());
                let e=node.chain.get(&late.hash()).unwrap();assert_eq!(e.import_seq,4);
                assert_eq!(e.first_import.unwrap().received_ms,tj+1);assert!(!e.first_import.unwrap().timely);
                let out=(e.first_import,e.import_seq,node.chain.head);
                if store{node.checkpoint_now().unwrap();}out};
            let mut cfg=config(&d,store);cfg.limits.side_cap=0;
            let mut node=Node::open(cfg,spec(&n.cfg,1)).unwrap();assert_eq!(node.chain.head,head);
            assert!(node.chain.get(&late.hash()).is_none(),"the old sync protection ends at restart");
            node.import_clock=Some(stamp.unwrap().received_ms-1);assert!(!node.import_with(late.clone(),None,true).unwrap());
            let e=node.chain.get(&late.hash()).unwrap();assert_eq!((e.first_import,e.import_seq),(stamp,seq+1));
            drop(node);std::fs::remove_dir_all(d).unwrap();
        }
    }

    #[test]fn old_preview_journal_without_timing_refuses_to_invent_history(){
        let n=net(true);let g=n.chain.head;let b=block(&n,g,true,0,0);let d=dir("old-preview");let src=dir("timed-source");
        {let mut node=Node::open(config(&src,false),spec(&n.cfg,0)).unwrap();let tj=deadline(&node,g);at(&mut node,&b,tj);}
        let legacy={let r=store::open(&src,&g).unwrap();let p=r.payload(0).unwrap();let rec=store::decode_record_ref(&p).unwrap();store::encode_record(rec.header,rec.body,rec.result)};
        std::fs::remove_dir_all(src).unwrap();
        {let mut r=store::open(&d,&g).unwrap();r.journal.append(&legacy).unwrap();}
        let original=std::fs::read(d.join("blocks.journal")).unwrap();
        let error=Node::open(config(&d,false),spec(&n.cfg,0)).err().unwrap();assert!(error.contains("no first-import timing"),"{error}");
        assert_eq!(std::fs::read(d.join("blocks.journal")).unwrap(),original);std::fs::remove_dir_all(d).unwrap();
    }

    #[test]fn slimmed_offloaded_and_canonically_evicted_entries_keep_the_stamp(){
        let mut n=net(true);let d=dir("canonical-eviction");let mut cfg=config(&d,true);cfg.limits.checkpoint_every=64;
        let (root,stamp,head,base)={let mut node=Node::open(cfg,spec(&n.cfg,0)).unwrap();
            let count=node.chain.evict_depth.unwrap()+3;assert!(count<500,"keep this private fixture bounded");
            let mut root=[0;32];let mut stamp=None;
            for h in 1..=count{let p=n.chain.head;let b=block(&n,p,false,0,0);let tj=deadline(&node,p);assert!(at(&mut node,&b,tj));accept(&mut n,&b,true);
                if h==1{root=b.hash();stamp=node.chain.get(&root).unwrap().first_import;}
                if h==50{let e=node.chain.get(&root).unwrap();assert!(e.slim&&e.offloaded);assert_eq!(e.first_import,stamp);}
            }
            assert!(node.chain.get(&root).is_none(),"root left the canonical RAM window");
            let restored=node.canonical_view(1,pocold::node::Want{receipts:false}).unwrap().unwrap();assert_eq!(restored.first_import,stamp);drop(restored);
            node.checkpoint_now().unwrap();(root,stamp,node.chain.head,node.chain.canonical.base())};
        assert!(base>1);
        let mut cfg=config(&d,true);cfg.limits.checkpoint_every=64;
        let node=Node::open(cfg,spec(&n.cfg,1)).unwrap();assert_eq!(node.chain.head,head);assert!(node.stats.resumed_from.is_some());
        let e=node.canonical_view(1,pocold::node::Want{receipts:false}).unwrap().unwrap();assert_eq!((e.hash,e.first_import),(root,stamp));drop(e);drop(node);
        std::fs::remove_dir_all(d).unwrap();
    }

    #[test]fn later_attempts_are_timely_even_outside_the_first_attempt_bounds(){
        let n=net(true);let g=n.chain.head;let a=block(&n,g,false,1,0);let b=block(&n,g,false,1,a.header.nonce+1);let d=dir("later-attempt");
        let mut node=Node::open(config(&d,false),spec(&n.cfg,0)).unwrap();
        at(&mut node,&a,0);at(&mut node,&b,u128::MAX);
        for h in [a.hash(),b.hash()]{assert!(node.chain.get(&h).unwrap().first_import.unwrap().timely);}
        drop(node);std::fs::remove_dir_all(d).unwrap();
    }

    #[test]fn an_invalid_copy_does_not_stamp_or_poison_the_first_valid_import(){
        let n=net(true);let g=n.chain.head;let b=block(&n,g,true,0,0);let mut bad=b.clone();bad.header.winner_sig=[0;65];reseal(&mut bad.header);
        assert_eq!(bad.hash(),b.hash());let d=dir("invalid-copy");let mut node=Node::open(config(&d,false),spec(&n.cfg,0)).unwrap();let tj=deadline(&node,g);
        node.import_clock=Some(tj);assert!(matches!(node.import(bad,None),Err(pocold::chain::Reject::Envelope(_))));
        assert_eq!(node.journal_records(),0);assert!(node.chain.get(&b.hash()).is_none());
        at(&mut node,&b,tj+1);let first=node.chain.get(&b.hash()).unwrap().first_import.unwrap();assert_eq!(first.received_ms,tj+1);assert!(!first.timely);
        node.import_clock=Some(tj);assert!(matches!(node.import(b.clone(),None),Err(pocold::chain::Reject::Duplicate)));
        assert_eq!(node.chain.get(&b.hash()).unwrap().first_import,Some(first));drop(node);std::fs::remove_dir_all(d).unwrap();
    }
}
