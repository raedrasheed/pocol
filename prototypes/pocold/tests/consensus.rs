//! LP5 consensus tests: fresh-genesis production, replay determinism, D133 fork
//! choice (heavier / tied / signed-first), per-item H-pre/H-full rejections,
//! envelope-vs-block rejection, evidence, finality.
use std::collections::HashMap;
use pocold::{chain::{Chain,Reject,Spec},types::*,hashes,rlp};
use pocol_lp4_execution::{apply,devnet::{self,DevnetConfig},txsign::{Key,Tx}};
use revm::primitives::{Address,B256,U256};
#[path="support/auth_peer.rs"] mod auth_peer;

struct Net{chain:Chain,keys:HashMap<Address,Key>}
/// Gate-valid test profile (A02): about 2^12 hashes per block (target 2^244-1 at T_blk=4),
/// c=8, and D_att=73 so that R7 holds with single-host timing: 73+5 >= 2(7+8*4).
/// Genesis is in the past: honest single-host Φ=0 tolerates no future timestamps.
fn cfg(miners:&[&str])->DevnetConfig{let mut c=DevnetConfig::new(777910,1_700_000_000,miners);c.hash_rate=1024;c.d_att=73;c.tau=40;c.c=8;c}
/// The admitted spec (full ParamGate with measured timing); a Devnet alone cannot boot.
fn admitted(c:&DevnetConfig)->Spec{let d=devnet::build(c).unwrap();Spec::new(d.admit(&pocold::single_host_local()).unwrap_or_else(|e|panic!("{e}")).0)}
fn net_with(c:&DevnetConfig)->Net{
    let d=devnet::build(c).unwrap();let mut keys=HashMap::new();
    for m in &d.miners{keys.insert(m.id,Key::from_secret(&m.key.secret()).unwrap());}
    Net{chain:Chain::new(admitted(c)),keys}
}
fn net()->Net{net_with(&cfg(&["a","b","c"]))}

/// Mines a nonce for the header's template; fills shares (<= T_share) and winnerSig.
fn seal_block(n:&Net,h:lp1hdr::Header,body:Body,attempt:u32,start:u64)->Block{try_seal(n,h,body,attempt,start).expect("no solution in nonce space")}
/// None when the bounded nonce space holds no solution (probability ~e^-c).
fn try_seal(n:&Net,mut h:lp1hdr::Header,body:Body,attempt:u32,start:u64)->Option<Block>{
    let p=n.chain.full_entry(&h.parent_hash).unwrap();let ctx=n.chain.child_ctx(&p,attempt).unwrap();
    let tid=template_id(&h.ut_raw);let mut shares=vec![];let mut x=start;
    loop{if (x as u128)>=ctx.assignment.nonce_max(){return None}let ph=pow_hash(&tid,x);if le_target(&ph,&ctx.t_share){if le_target(&ph,&ctx.target){break}shares.push(x);}x+=1;}
    let owner=addr(&ctx.assignment.owner(x).unwrap());
    h.nonce=x;shares.truncate(256);h.shares=shares;reseal(&mut h);
    h.winner_sig=n.keys[&owner].sign_hash(&B256::from(hashes::win_msg(&tid,x,&share_root(&h))));reseal(&mut h);
    Some(Block{header:h,body})
}
mod lp1hdr{pub use pocold::header::Header;}
/// Fallback at attempt a; if its nonce space has no solution, the next attempt (consensus §4).
fn fallback(n:&Net,parent:[u8;32],a:u32)->Block{let p=n.chain.full_entry(&parent).unwrap();
    for a in a..a+16{let (h,_,_)=n.chain.build_fallback(&p,a).unwrap();if let Some(b)=try_seal(n,h,Body::default(),a,0){return b}}panic!("no fallback solution")}
/// Signed template by Draw(h,0); a different in-window timestamp is a new TemplateID.
fn signed(n:&Net,parent:[u8;32],body:Body,ts_offset:u64)->Block{
    let p=n.chain.full_entry(&parent).unwrap();let ctx=n.chain.child_ctx(&p,0).unwrap();
    for off in ts_offset..u64::from(n.chain.spec.cp().D_att){let (h,_,_)=n.chain.build_signed(&p,&n.keys[&ctx.proposer],ctx.window.lower()+off,body.clone(),vec![]).unwrap();
        if let Some(b)=try_seal(n,h,body.clone(),0,0){return b}}panic!("no signed solution")
}
fn import(n:&mut Net,b:&Block)->bool{let (e,_)=n.chain.validate(b,None,0).unwrap_or_else(|e|panic!("h={} {e:?}",b.header.h));n.chain.insert(e)}
fn reject(n:&Net,b:&Block)->Reject{n.chain.validate(b,None,0).err().expect("must reject")}
fn remine(n:&Net,mut h:lp1hdr::Header,body:Body)->Block{h.nonce=0;h.shares=vec![];reseal(&mut h);let a=h.a;
    // A mutated template may hold no solution; mutate the timestamp minimally only if needed is not allowed, so search the full space.
    seal_block(n,h,body,a,0)}

#[test] fn thirty_five_blocks_from_fresh_genesis_replay_identically(){
    let mut n=net();let mut blocks=vec![];
    let alice=devnet::account_key("alice");let bob=devnet::account_key("bob").address();
    let (mut nonce,mut sent)=(0u64,0u64);
    for i in 0..35u64{
        let head=n.chain.head;
        let b=if i%3==2{fallback(&n,head,0)}else{
            let txs=if i%5==1{nonce+=1;sent+=i;vec![Tx{chain_id:777910,nonce:nonce-1,max_priority:U256::from(1_000_000_000u64),max_fee:U256::from(100_000_000_000u64),gas:21000,to:Some(bob),value:U256::from(i),data:Default::default()}.sign_1559(&alice)]}else{vec![]};
            signed(&n,head,Body{txs,evidence:vec![]},0)};
        assert!(import(&mut n,&b),"new head at {i}");blocks.push(b);
    }
    assert_eq!(n.chain.head().height,35);
    assert_eq!(n.chain.head().state.accounts[&bob].info.balance,U256::from(10u128.pow(24))+U256::from(sent));assert_eq!(nonce,5);
    // Every miner earned rewards; ledger and pending invariants hold at the head.
    let s=&n.chain.head().state;for id in apply::record_list(s){assert!(s.accounts[&apply::miner_record(s,id).unwrap().reward].info.balance>U256::ZERO);}
    apply::check_escrow_invariant(s,n.chain.spec.profile(),35,false).unwrap();
    // Replay into a fresh chain: identical hashes and state roots for every block.
    let mut m=net();for b in &blocks{import(&mut m,b);}
    assert_eq!(m.chain.head,n.chain.head);
    for b in &blocks{let (x,y)=(&n.chain.blocks[&b.hash()],&m.chain.blocks[&b.hash()]);assert_eq!(apply::state_root(&x.state).unwrap(),apply::state_root(&y.state).unwrap());assert_eq!(x.result,y.result);}
}

/// Independent reference for D133: W, then key of the LCA children.
fn reference_best(n:&Net)->[u8;32]{best_of(&n.chain)}
/// D133 over every stored block of a chain, computed from whole paths (not the incremental head).
fn best_of(chain:&Chain)->[u8;32]{
    let tips:Vec<[u8;32]>=chain.blocks.keys().copied().collect();
    let path=|h:[u8;32]|{let mut v=vec![];let mut c=Some(h);while let Some(x)=c{v.push(x);c=chain.blocks[&x].parent;}v.reverse();v};
    let key=|h:&[u8;32]|{let e=&chain.blocks[h];(if e.signed{0}else{1},e.import_seq,e.hash)};
    let mut best=tips[0];
    for t in tips{
        let (a,b)=(&chain.blocks[&t],&chain.blocks[&best]);
        let better=if a.total_work!=b.total_work{a.total_work>b.total_work}else if t==best{false}else{
            let (pa,pb)=(path(t),path(best));let i=pa.iter().zip(pb.iter()).take_while(|(x,y)|x==y).count();key(&pa[i])<key(&pb[i])};
        if better{best=t}
    }
    best
}
#[test] fn fork_choice_heavier_tied_signed_first_and_rollback(){
    let mut n=net();for _ in 0..5{let h=n.chain.head;let b=fallback(&n,h,0);import(&mut n,&b);}
    let base=n.chain.head;
    // Tie at h=6: fallback imported first, then signed sibling: signed-first wins.
    let fb=fallback(&n,base,0);let sg=signed(&n,base,Body::default(),0);
    assert_eq!(n.chain.blocks[&base].total_work+pocold::chain::work(&fb.header.target),n.chain.blocks[&base].total_work+pocold::chain::work(&sg.header.target));
    import(&mut n,&fb);assert_eq!(n.chain.head,fb.hash());
    assert!(import(&mut n,&sg),"signed sibling wins the tie");assert_eq!(n.chain.head,sg.hash());assert_eq!(n.chain.head,reference_best(&n));
    // Two fallback siblings at h=6 (different attempts): earlier import wins the tie
    let fb1=fallback(&n,base,fb.header.a+1);
    if n.chain.blocks[&base].total_work+pocold::chain::work(&fb1.header.target)==n.chain.blocks[&sg.hash()].total_work{import(&mut n,&fb1);assert_eq!(n.chain.head,sg.hash());}
    else{import(&mut n,&fb1);}
    assert_eq!(n.chain.head,reference_best(&n));
    // Heavier: extend the fallback branch by two blocks -> head moves (reorg).
    let a7=fallback(&n,fb.hash(),0);import(&mut n,&a7);let a8=fallback(&n,a7.hash(),0);import(&mut n,&a8);
    assert_eq!(n.chain.head,reference_best(&n));assert_eq!(n.chain.head,a8.hash());
    // Rollback determinism: each tip's stored state equals a fresh re-execution of its branch.
    let mut m=net();for b in [&fb,&a7,&a8]{let _=b;}
    let mut cur=n.chain.head;let mut path=vec![];while let Some(p)=n.chain.blocks[&cur].parent{path.push(cur);cur=p;}path.reverse();
    for h in &path{let e=n.chain.blocks[h].clone();let b=Block{header:e.header.clone(),body:e.body.clone()};import(&mut m,&b);}
    assert_eq!(apply::state_root(&m.chain.head().state).unwrap(),apply::state_root(&n.chain.head().state).unwrap());
    // Extend the signed branch to be heavier again -> reorg back.
    let b7=fallback(&n,sg.hash(),0);import(&mut n,&b7);let b8=fallback(&n,b7.hash(),0);import(&mut n,&b8);let b9=fallback(&n,b8.hash(),0);import(&mut n,&b9);
    assert_eq!(n.chain.head,reference_best(&n));assert_eq!(n.chain.head,b9.hash());
}

/// Review R4-1: every stored block's total work is its parent's plus work(target) of its OWN committed
/// target, computed here independently of `chain::work` as floor(2^256/(t+1)) = (!t)/(t+1)+1
/// (consensus.md:117, §9), on a chain of signed and fallback blocks and on a side branch.
#[test] fn every_block_total_work_is_its_parents_plus_the_work_of_its_own_target(){
    let mut n=net();let g=n.chain.head;let mut p=g;
    for i in 0..24{let b=if i%3==0{signed(&n,p,Body::default(),0)}else{fallback(&n,p,0)};import(&mut n,&b);n.chain.retain();p=b.hash();}
    let fork=n.chain.canonical[10];let s=signed(&n,fork,Body::default(),0);import(&mut n,&s);let mut q=s.hash();for _ in 0..3{let b=fallback(&n,q,0);import(&mut n,&b);q=b.hash();}
    assert!(!n.chain.is_canonical(&q),"a side branch is stored too");
    let mut checked=0;let mut distinct=std::collections::HashSet::new();
    for e in n.chain.blocks.values(){
        assert!(e.height>0||e.total_work==U256::ZERO,"genesis has total work 0");
        let Some(par)=e.parent else{continue};let t=U256::from_be_bytes(e.header.target.to_be_bytes());
        let w=(!t)/(t+U256::from(1u8))+U256::from(1u8);distinct.insert(t);
        assert_eq!(e.total_work,n.chain.blocks[&par].total_work+w,"h={}: total work = parent's + work(own target)",e.height);checked+=1;
    }
    assert!(checked>=28&&distinct.len()>=2,"positive control: {checked} blocks, {} distinct targets (a parent-target mutant differs)",distinct.len());
}
#[test] fn every_validity_item_rejects_its_mutation(){
    let mut n=net();for _ in 0..3{let h=n.chain.head;let b=fallback(&n,h,0);import(&mut n,&b);}
    let head=n.chain.head;
    let good=signed(&n,head,Body::default(),0);
    let block=|why:&str,r:Reject|match r{Reject::Block(s)=>assert!(s.contains(why),"expected {why}, got {s}"),o=>panic!("expected Block({why}), got {o:?}")};
    // 1b NetID.
    let mut h=good.header.clone();h.chain_id+=1;reseal(&mut h);block("netChain",reject(&n,&Block{header:h,body:Body::default()}));
    let mut h=good.header.clone();h.genesis_hash[0]^=1;reseal(&mut h);block("netGenesis",reject(&n,&Block{header:h,body:Body::default()}));
    // 2 height.
    let mut h=good.header.clone();h.h+=1;reseal(&mut h);block("2 height",reject(&n,&Block{header:h,body:Body::default()}));
    // 5 target (unsigned fallback copy so item 3 does not fire first).
    let fbg=fallback(&n,head,0);
    let mut h=fbg.header.clone();h.target=Fixed::from_be_slice(&(from_fixed(&h.target)>>1u32).to_be_bytes::<32>()).unwrap();reseal(&mut h);block("5 target",reject(&n,&Block{header:h,body:Body::default()}));
    // 6 nonce >= nMax, 7 PoW.
    let mut h=fbg.header.clone();h.nonce=u64::MAX;reseal(&mut h);block("6",reject(&n,&Block{header:h,body:Body::default()}));
    let ftid=template_id(&fbg.header.ut_raw);let target=fbg.header.target;
    let bad=(0..).find(|x|!le_target(&pow_hash(&ftid,*x),&target)).unwrap();
    let mut h=fbg.header.clone();h.nonce=bad;reseal(&mut h);block("7 pow",reject(&n,&Block{header:h,body:Body::default()}));
    let tid=template_id(&good.header.ut_raw);
    // 9 shares: unsorted, equal to nonce, above T_share.
    if good.header.shares.len()>=2{let mut h=good.header.clone();h.shares.swap(0,1);reseal(&mut h);block("9 shares not strictly ascending",reject(&n,&Block{header:h,body:Body::default()}));}
    let mut h=good.header.clone();h.shares=vec![h.nonce];reseal(&mut h);block("9 share range",reject(&n,&Block{header:h,body:Body::default()}));
    let p=n.chain.full_entry(&head).unwrap();let ctx=n.chain.child_ctx(&p,0).unwrap();
    let over=(0..).find(|x|*x!=good.header.nonce&&!le_target(&pow_hash(&tid,*x),&ctx.t_share)).unwrap();
    let mut h=good.header.clone();h.shares=vec![over];reseal(&mut h);block("9 share target",reject(&n,&Block{header:h,body:Body::default()}));
    // 11 gasLimit / baseFee (re-mined so PoW passes).
    let mut h=good.header.clone();h.gas_limit+=1;reseal(&mut h);let b=remine_signed(&n,h,ctx.proposer);block("11 gasLimit",reject(&n,&b));
    let mut h=good.header.clone();h.base_fee=to_fixed(from_fixed(&h.base_fee)+U256::from(1));reseal(&mut h);let b=remine_signed(&n,h,ctx.proposer);block("11 baseFee",reject(&n,&b));
    // 12 proposer not Draw(h,0): a validly signed template by another identity.
    let other=*n.keys.keys().find(|k|**k!=ctx.proposer).unwrap();
    let mut h=good.header.clone();h.proposer=other.to_vec();reseal(&mut h);let b=remine_signed(&n,h,other);block("12 proposer",reject(&n,&b));
    // 17 stateRoot.
    let mut h=good.header.clone();h.state_root[0]^=1;reseal(&mut h);let b=remine_signed(&n,h,ctx.proposer);block("17 stateRoot",reject(&n,&b));
    // Fallback: wrong timestamp (4) and non-empty body (12).
    let fb=fallback(&n,head,0);
    let mut h=fb.header.clone();h.ts+=1;reseal(&mut h);let b=remine(&n,h,Body::default());block("4 fallback timestamp",reject(&n,&b));
    let tx=Tx{chain_id:777910,nonce:0,max_priority:U256::from(1u64),max_fee:U256::from(10_000_000_000u64),gas:21000,to:Some(Address::ZERO),value:U256::ZERO,data:Default::default()}.sign_1559(&devnet::account_key("alice"));
    // A fallback header committing to the empty body plus a non-empty body is another copy (envelope);
    // a fallback header that commits to a non-empty body is invalid (12).
    let b=Block{header:fb.header.clone(),body:Body{txs:vec![tx.clone()],evidence:vec![]}};assert!(matches!(reject(&n,&b),Reject::Envelope(s) if s.contains("body does not match")));
    let body=Body{txs:vec![tx],evidence:vec![]};let mut h=fb.header.clone();h.tx_root=pocold::types::ordered_root(&body.txs).unwrap();reseal(&mut h);
    let b=remine(&n,h,body);block("12 fallback body",reject(&n,&b));
    // Envelope rejection: winnerSig from a non-owner rejects only this copy.
    let mut h=good.header.clone();h.winner_sig=n.keys[&other].sign_hash(&B256::from(hashes::win_msg(&tid,h.nonce,&share_root(&h))));reseal(&mut h);
    let copy=Block{header:h,body:Body::default()};
    if addr(&ctx.assignment.owner(good.header.nonce).unwrap())!=other{
        assert_eq!(copy.hash(),good.hash(),"winnerSig is not committed by blockHash");
        assert!(matches!(reject(&n,&copy),Reject::Envelope(_)));
    }
    assert!(import(&mut n,&good),"a valid copy of the same blockHash is still accepted");
    // Unknown parent.
    let mut h=good.header.clone();h.parent_hash=[7;32];reseal(&mut h);assert_eq!(reject(&n,&Block{header:h,body:Body::default()}),Reject::UnknownParent);
}
fn remine_signed(n:&Net,mut h:lp1hdr::Header,signer:Address)->Block{
    let tid=template_id(&h.ut_raw);h.sig=n.keys[&signer].sign_hash(&B256::from(hashes::sig_msg(&tid))).to_vec();reseal(&mut h);remine(n,h,Body::default())
}

#[test] fn evidence_type1_and_type2_punish_through_a_signed_block(){
    let mut n=net();for _ in 0..3{let h=n.chain.head;let b=fallback(&n,h,0);import(&mut n,&b);}
    // Type 1: the drawn proposer of h=4 signs two different templates.
    let head=n.chain.head;let p=n.chain.full_entry(&head).unwrap();let ctx=n.chain.child_ctx(&p,0).unwrap();let accused1=ctx.proposer;
    let (t1,_,_)=n.chain.build_signed(&p,&n.keys[&accused1],ctx.window.lower(),Body::default(),vec![]).unwrap();
    let (t2,_,_)=n.chain.build_signed(&p,&n.keys[&accused1],ctx.window.lower()+1,Body::default(),vec![]).unwrap();
    let st=|h:&lp1hdr::Header|{let mut p=h.ut_raw.clone();rlp::encode_bytes(&h.sig,&mut p);let mut o=Vec::new();rlp::encode_list_payload(&p,&mut o);o};
    let mut ev1=Vec::new();rlp::encode_bytes(&[1],&mut ev1);ev1.extend(st(&t1));ev1.extend(st(&t2));let mut e1=Vec::new();rlp::encode_list_payload(&ev1,&mut e1);
    let b4=seal_block(&n,t1.clone(),Body::default(),0,0);import(&mut n,&b4);
    // Type 2: the winner of h=4 signs a second share root for the same TemplateID+nonce.
    let winner=n.chain.head().result.clone().unwrap().winner;let tid=template_id(&b4.header.ut_raw);
    let mut alt=b4.header.clone();alt.shares=vec![];reseal(&mut alt);
    let root1=share_root(&b4.header);let root2=share_root(&alt);assert_ne!(root1,root2);
    let sig2=n.keys[&winner].sign_hash(&B256::from(hashes::win_msg(&tid,b4.header.nonce,&root2)));
    let mut ev2=Vec::new();rlp::encode_bytes(&[2],&mut ev2);rlp::encode_bytes(&tid,&mut ev2);rlp::encode_bytes(&b4.header.nonce.to_be_bytes(),&mut ev2);
    rlp::encode_bytes(&root1,&mut ev2);rlp::encode_bytes(&b4.header.winner_sig,&mut ev2);rlp::encode_bytes(&root2,&mut ev2);rlp::encode_bytes(&sig2,&mut ev2);
    let mut e2=Vec::new();rlp::encode_list_payload(&ev2,&mut e2);
    let mut evidence=vec![e1.clone()];if winner!=accused1{evidence.push(e2.clone());}
    // Include in a signed block at h=5 (the drawn proposer must not be one of the accused for this test to apply cleanly).
    let head=n.chain.head;let p=n.chain.full_entry(&head).unwrap();let ctx=n.chain.child_ctx(&p,0).unwrap();
    let effects=n.chain.evidence_effects(&p,&Body{txs:vec![],evidence:evidence.clone()},5).unwrap();
    assert_eq!(effects.len(),evidence.len());
    let (h,_,_)=n.chain.build_signed(&p,&n.keys[&ctx.proposer],ctx.window.lower(),Body{txs:vec![],evidence:evidence.clone()},effects).unwrap();
    let b5=seal_block(&n,h,Body{txs:vec![],evidence:evidence.clone()},0,0);import(&mut n,&b5);
    let s=&n.chain.head().state;
    let r=apply::miner_record(s,accused1).unwrap();assert!(r.tomb);assert_eq!(r.bond_state,2);
    if winner!=accused1{let r=apply::miner_record(s,winner).unwrap();assert!(r.tomb);assert_ne!(apply::pending(s,4).1>>168u32,U256::ZERO,"pot 4 invalidated");}
    // Re-using the same evidence later is invalid (accused already a tomb).
    let head=n.chain.head;let p=n.chain.full_entry(&head).unwrap();
    assert!(matches!(n.chain.evidence_effects(&p,&Body{txs:vec![],evidence:vec![e1]},6),Err(Reject::Block(_))));
}

#[test] fn final_block_has_no_child_and_h_end_is_enforced(){
    // Gate-valid short end (end/full style): R_max=0 and a two-record registry make
    // R20 need Z_close >= ceil(2/2)+P2+D+1 = 8, so H_CLOSE+Z_close <= H_END=9 (R13).
    let mut c=cfg(&["a","b"]);c.h_close=1;c.h_end=9;c.r_max=0;c.m_max=2;c.reg_records_max=2;c.ret=2;c.z_close=8;let mut n=net_with(&c);
    let mut final_at=None;
    for _ in 0..9{let h=n.chain.head;let b=fallback(&n,h,0);import(&mut n,&b);if n.chain.head().is_final{final_at=Some(n.chain.head().height);break}}
    assert_eq!(final_at,Some(9));
    let s=&n.chain.head().state;for j in 1..=9{assert_eq!(apply::pending(s,j).1,U256::ZERO);}
    let head=n.chain.head;let p=n.chain.full_entry(&head).unwrap();
    assert!(n.chain.build_fallback(&p,0).is_err(),"no valid child above H_END/final");
}

#[test] fn ordinary_blocks_then_closing_then_final_at_h_end(){
    // Review finding: the test above runs with H_CLOSE=1, so the ordinary-to-closing transition is
    // covered here with the audited H_CLOSE=4 and the gate-valid H_END = H_CLOSE + Z_close = 12 (R13).
    let mut c=cfg(&["a","b"]);c.h_close=4;c.h_end=12;c.r_max=0;c.m_max=2;c.reg_records_max=2;c.ret=2;c.z_close=8;let mut n=net_with(&c);
    let mut closing=vec![];let mut final_at=None;
    for _ in 0..12{let h=n.chain.head;let b=fallback(&n,h,0);import(&mut n,&b);let e=n.chain.head();
        closing.push((e.height,apply::chain_facts(&e.state).closing));if e.is_final{final_at=Some(e.height);break}}
    assert!(closing.iter().filter(|(h,_)|*h<4).all(|(_,c)|!c),"ordinary below H_CLOSE: {closing:?}");
    assert!(closing.iter().filter(|(h,_)|*h>=4).all(|(_,c)|*c),"closing from H_CLOSE on: {closing:?}");
    assert_eq!(final_at,Some(12));
    let head=n.chain.head;let p=n.chain.full_entry(&head).unwrap();assert!(n.chain.build_fallback(&p,0).is_err(),"no valid child above H_END/final");
}

#[test] fn k_hist_retention_and_trie_rebuild_for_deep_fork(){
    let mut n=net();n.chain.k_hist=4;let mut hashes=vec![n.chain.head];
    for _ in 0..12{let h=n.chain.head;let b=fallback(&n,h,0);import(&mut n,&b);n.chain.retain();hashes.push(n.chain.head);}
    let pruned:Vec<u64>=n.chain.blocks.values().filter(|e|e.pruned).map(|e|e.height).collect();
    assert!(pruned.iter().all(|h|*h>=1&&*h<=8)&&pruned.len()==8,"{pruned:?}");
    // A fork child of pruned height 3 needs TrieRebuild; the rebuilt state matches the committed root.
    let fork_parent=hashes[3];
    let mut m=net();for h in &hashes[1..=3]{let e=n.chain.blocks[h].clone();import(&mut m,&Block{header:e.header,body:e.body});}
    let child=fallback(&m,fork_parent,1);
    assert_eq!(n.chain.validate(&child,None,0).err(),Some(Reject::NeedsRebuild(fork_parent)));
    n.chain.ensure_state(&fork_parent).unwrap();assert!(n.chain.rebuilds>=3);
    let (e,_)=n.chain.validate(&child,None,0).unwrap();n.chain.insert(e);
    assert_eq!(apply::state_root(&n.chain.blocks[&fork_parent].state).unwrap().0,n.chain.blocks[&fork_parent].header.state_root);
}

#[test] fn template_form_classes_follow_d129(){
    let mut n=net();for _ in 0..2{let h=n.chain.head;let b=fallback(&n,h,0);import(&mut n,&b);}
    let head=n.chain.head;let good=signed(&n,head,Body::default(),0);
    // proposer present but sig removed: envelope (sig bytes are not committed).
    let mut h=good.header.clone();h.sig=vec![];reseal(&mut h);assert!(matches!(reject(&n,&Block{header:h,body:Body::default()}),Reject::Envelope(_)));
    // fallback-shaped header carrying a sig: envelope.
    let fb=fallback(&n,head,0);let mut h=fb.header.clone();h.sig=good.header.sig.clone();reseal(&mut h);assert!(matches!(reject(&n,&Block{header:h,body:Body::default()}),Reject::Envelope(_)));
    // proposer present with a != 0 is committed content: block reject.
    let mut h=good.header.clone();h.a=1;reseal(&mut h);assert!(matches!(reject(&n,&Block{header:h,body:Body::default()}),Reject::Block(s) if s.contains("a=0")));
}

#[test] fn node_detects_template_equivocation_and_produces_valid_evidence(){
    use pocold::{node::{Config,Event,Node},net::{Msg,NetEvent}};
    let c=cfg(&["a","b","c"]);let d=devnet::build(&c).unwrap();
    let keys:HashMap<Address,Key>=d.miners.iter().map(|m|(m.id,Key::from_secret(&m.key.secret()).unwrap())).collect();
    let spec=admitted(&c);
    let dir=std::env::temp_dir().join(format!("pocold-ev-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    let mut node=Node::open(Config{dir,name:"t".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Default::default()},spec).unwrap();
    node.handle(Event::Tick);
    let net2=Net{chain:Chain::new(admitted(&c)),keys};
    let g=net2.chain.head;let p=net2.chain.full_entry(&g).unwrap();let ctx=net2.chain.child_ctx(&p,0).unwrap();
    let mk=|off:u64|{let (h,_,_)=net2.chain.build_signed(&p,&net2.keys[&ctx.proposer],ctx.window.lower()+off,Body::default(),vec![]).unwrap();h};
    let (t1,t2)=(mk(0),mk(1));
    // Simulate the node's production loop at h=1 (it starts producing on run(); use the handler directly).
    let (peer,_socket)=auth_peer::connect(&mut node);
    for t in [&t1,&t2]{node.handle(Event::Net(NetEvent::Message(peer,Msg::Template{shell:t.encoded.clone(),body:Body::default().encode()})));}
    assert_eq!(node.evidence_pool.len(),1,"one type-1 evidence");
    // The evidence is valid in a block at h=2 on a branch whose h=1 drew the same proposer.
    let mut n2=net2;let b1=fallback(&n2,g,0);import(&mut n2,&b1);
    let head=n2.chain.head;let ph=n2.chain.full_entry(&head).unwrap();
    let eff=n2.chain.evidence_effects(&ph,&Body{txs:vec![],evidence:node.evidence_pool.clone()},2).unwrap();
    assert_eq!(eff,vec![apply::EvidenceEffect::Punish{accused:ctx.proposer}]);
}

/// Review R1-5, consensus.md:101: a template that the drawn proposer signed for this node's parent but
/// whose block would be invalid is as if absent, and the peer is punished (its connection is closed;
/// counted in templatePenalties). A valid template, and one for another parent, are not punished.
#[test] fn an_invalid_template_from_the_drawn_proposer_punishes_its_peer(){
    use pocold::{node::{Config,Event,Node},net::{Msg,NetEvent}};
    let c=cfg(&["a","b","c"]);let d=devnet::build(&c).unwrap();
    let keys:HashMap<Address,Key>=d.miners.iter().map(|m|(m.id,Key::from_secret(&m.key.secret()).unwrap())).collect();
    let node=|tag:&str|{let dir=std::env::temp_dir().join(format!("pocold-tplpen-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
        let mut n=Node::open(Config{dir,name:"t".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Default::default()},admitted(&c)).unwrap();n.start_production();n};
    let net2=Net{chain:Chain::new(admitted(&c)),keys};
    let g=net2.chain.head;let p=net2.chain.full_entry(&g).unwrap();let ctx=net2.chain.child_ctx(&p,0).unwrap();
    let (good,_,_)=net2.chain.build_signed(&p,&net2.keys[&ctx.proposer],ctx.window.lower(),Body::default(),vec![]).unwrap();
    // The drawn proposer signs a template whose stateRoot is not the parent's result: its rebuild differs.
    let mut bad=good.clone();bad.state_root[0]^=1;reseal(&mut bad);
    bad.sig=net2.keys[&ctx.proposer].sign_hash(&B256::from(hashes::sig_msg(&template_id(&bad.ut_raw)))).to_vec();reseal(&mut bad);
    let send=|n:&mut Node,h:&lp1hdr::Header|{let (peer,_socket)=auth_peer::connect(n);n.handle(Event::Net(NetEvent::Message(peer,Msg::Template{shell:h.encoded.clone(),body:Body::default().encode()})))};
    let mut a=node("bad");send(&mut a,&bad);assert_eq!(a.stats.template_penalties,1,"the invalid signed template punishes its peer");
    let mut b=node("good");send(&mut b,&good);assert_eq!(b.stats.template_penalties,0,"positive control: a valid template is not punished");
    let mut other=bad.clone();other.parent_hash[0]^=1;reseal(&mut other);
    let mut e=node("other");send(&mut e,&other);assert_eq!(e.stats.template_penalties,0,"a template for another parent does not apply here");
}

#[test] fn journal_replay_rebuilds_pruned_state_for_a_journaled_fork(){
    // LP10 finding: a node that imported a fork child of a block below the K_hist window
    // (live TrieRebuild) must replay that journal; replay applies the same TrieRebuild.
    use pocold::node::{Config,Node};
    let c=cfg(&["a","b","c"]);
    let spec=|| admitted(&c);
    let dir=std::env::temp_dir().join(format!("pocold-replay-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    let conf=||Config{dir:dir.clone(),name:"r".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Default::default()};
    let mut n=net();let mut hashes=vec![n.chain.head];
    let mut node=Node::open(conf(),spec()).unwrap();
    for _ in 0..70{let h=n.chain.head;let b=fallback(&n,h,0);import(&mut n,&b);node.import(b,None).unwrap();hashes.push(n.chain.head);}
    node.chain.retain();
    let fork_parent=hashes[3];
    let mut m=net();for h in &hashes[1..=3]{let e=n.chain.blocks[h].clone();import(&mut m,&Block{header:e.header,body:e.body});}
    let child=fallback(&m,fork_parent,1);let child_hash=child.hash();
    assert!(node.chain.blocks[&fork_parent].pruned,"fork parent below the K_hist window");
    node.import(child,None).expect("live import rebuilds the pruned parent");
    let head=node.chain.head;drop(node);
    let again=Node::open(conf(),spec()).expect("replay of a journal that contains a deep fork");
    assert_eq!(again.chain.head,head);assert!(again.chain.get(&child_hash).is_some());
    let _=std::fs::remove_dir_all(&dir);
}

#[test] fn a_valid_header_with_a_foreign_body_rejects_only_that_copy(){
    // Review finding (HIGH): a peer relaying a real header with a mismatched body must not get the
    // block marked invalid; the correct body is accepted afterwards.
    use pocold::node::{Config,Node};
    let c=cfg(&["a","b","c"]);
    let spec=admitted(&c);
    let dir=std::env::temp_dir().join(format!("pocold-copy-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    let mut node=Node::open(Config{dir:dir.clone(),name:"c".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Default::default()},spec).unwrap();
    let n=net();let g=n.chain.head;let good=fallback(&n,g,0);
    let junk=Block{header:good.header.clone(),body:Body{txs:vec![vec![0xc0;40]],evidence:vec![]}};
    assert!(matches!(node.import(junk,None),Err(Reject::Envelope(_))),"mismatched body is an envelope rejection");
    assert!(node.import(good.clone(),None).expect("the real block is still accepted"),"becomes head");
    assert_eq!(node.chain.head,good.hash());
    let _=std::fs::remove_dir_all(&dir);
}

// ---------------- audit A01 regressions: unbound bodies never poison a valid header ----------------
fn a01_node(tag:&str)->(pocold::node::Node,std::path::PathBuf){
    use pocold::node::{Config,Node};
    let c=cfg(&["a","b","c"]);
    let spec=admitted(&c);
    let dir=std::env::temp_dir().join(format!("pocold-a01-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    (Node::open(Config{dir:dir.clone(),name:"a01".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Default::default()},spec).unwrap(),dir)
}
#[test] fn a01_malformed_or_unbound_bodies_reject_only_the_copy(){
    let n=net();let g=n.chain.head;let good=fallback(&n,g,0);
    let tx=Tx{chain_id:777910,nonce:0,max_priority:U256::from(1u64),max_fee:U256::from(10_000_000_000u64),gas:21000,to:Some(Address::ZERO),value:U256::ZERO,data:Default::default()}.sign_1559(&devnet::account_key("alice"));
    let copies:Vec<(&str,Body)>=vec![
        ("empty tx item",Body{txs:vec![vec![]],evidence:vec![]}),
        ("empty evidence item",Body{txs:vec![],evidence:vec![vec![]]}),
        ("empty tx among valid ones",Body{txs:vec![tx.clone(),vec![]],evidence:vec![]}),
        ("undecodable tx bytes",Body{txs:vec![vec![0xff,0x01,0x02]],evidence:vec![]}),
        ("undecodable evidence bytes",Body{txs:vec![],evidence:vec![vec![0xc1]]}),
        ("valid tx, header commits to the empty body",Body{txs:vec![tx],evidence:vec![]}),
        ("oversized tx item",Body{txs:vec![vec![0x02;200_000]],evidence:vec![]}),
    ];
    let (mut node,dir)=a01_node("copies");
    for (what,body) in &copies{
        let r=node.import(Block{header:good.header.clone(),body:body.clone()},None);
        assert!(matches!(r,Err(Reject::Envelope(_))),"{what}: must be an envelope rejection, got {r:?}");
    }
    assert!(node.import(good.clone(),None).expect("legitimate body after every malformed copy"),"becomes head");
    assert_eq!(node.chain.head,good.hash());
    let _=std::fs::remove_dir_all(&dir);
}
#[test] fn a01_committed_invalid_body_is_still_a_block_rejection_and_cached(){
    // The header commits to a body whose content is invalid: that is a verdict on the block hash.
    let mut n=net();for _ in 0..2{let h=n.chain.head;let b=fallback(&n,h,0);import(&mut n,&b);}
    let head=n.chain.head;let p=n.chain.full_entry(&head).unwrap();let ctx=n.chain.child_ctx(&p,0).unwrap();
    let good=signed(&n,head,Body::default(),0);
    let garbage=Body{txs:vec![vec![0x02,0xc0]],evidence:vec![]};
    let mut h=good.header.clone();h.tx_root=pocold::types::ordered_root(&garbage.txs).unwrap();reseal(&mut h);
    let mut b=remine_signed(&n,h,ctx.proposer);b.body=garbage;
    let r=n.chain.validate(&b,None,0);
    assert!(matches!(r,Err(Reject::Block(_))),"committed invalid content is a block rejection, got {:?}",r.err());
    // Through the node: the hash is cached as invalid and a resubmission is refused.
    let (mut node,dir)=a01_node("committed");
    let parent=n.chain.blocks[&head].parent.unwrap();
    for h in [parent,head]{let e=n.chain.blocks[&h].clone();node.import(Block{header:e.header,body:e.body},None).unwrap();}
    assert!(matches!(node.import(b.clone(),None),Err(Reject::Block(_))));
    assert!(matches!(node.import(b,None),Err(Reject::Block(s)) if s.contains("known invalid")),"cached verdict for committed content");
    let _=std::fs::remove_dir_all(&dir);
}

// ---------------- audit A03 regression: an over-limit log is an exceptional halt ----------------
#[test] fn a03_over_limit_log_halts_the_transaction_and_the_block_is_valid(){
    // The audited case: CREATE whose initcode emits LOG0 with 32769 data bytes (36+32769 > 32768).
    let mut n=net();let h=n.chain.head;let b=fallback(&n,h,0);import(&mut n,&b);
    let alice=devnet::account_key("alice");let before=n.chain.head().state.accounts[&alice.address()].info.clone();
    let init=vec![0x61,0x80,0x01,0x5f,0xa0,0x00];
    let tx=Tx{chain_id:777910,nonce:0,max_priority:U256::from(1u64),max_fee:U256::from(100_000_000_000u64),gas:2_000_000,to:None,value:U256::ZERO,data:init.into()}.sign_1559(&alice);
    let head=n.chain.head;let blk=signed(&n,head,Body{txs:vec![tx],evidence:vec![]},0);
    let (e,_)=n.chain.validate(&blk,None,0).expect("the block stays valid");
    let it=rlp::decode(&e.receipts[0][1..]).unwrap();let l=it.list().unwrap();
    assert_eq!(l[0].bytes().unwrap(),&[] as &[u8],"status 0");assert_eq!(l[3].list().unwrap().len(),0,"no logs");
    assert_eq!(e.gas_used,2_000_000,"all gas charged");assert_eq!(e.cum_log_bytes,0,"a halted transaction charges no log bytes");
    let a=&e.state.accounts[&alice.address()].info;assert_eq!(a.nonce,before.nonce+1);assert!(a.balance<before.balance);
    assert!(n.chain.insert(e),"becomes head");
    // Within the limit the same pattern succeeds and the chain accumulates its log bytes.
    let init=vec![0x61,0x7f,0xdc,0x5f,0xa0,0x00];// LOG0 with 32732 bytes: 36+32732 = 32768
    let tx=Tx{chain_id:777910,nonce:1,max_priority:U256::from(1u64),max_fee:U256::from(100_000_000_000u64),gas:2_000_000,to:None,value:U256::ZERO,data:init.into()}.sign_1559(&alice);
    let head=n.chain.head;let blk=signed(&n,head,Body{txs:vec![tx],evidence:vec![]},0);
    let (e,_)=n.chain.validate(&blk,None,0).unwrap();let it=rlp::decode(&e.receipts[0][1..]).unwrap();
    assert_eq!(it.list().unwrap()[0].bytes().unwrap(),&[1u8]);assert_eq!(e.cum_log_bytes,32768);
}

// ---------------- audit A11 regressions: detached transactions return to the pool ----------------
fn a11_node(tag:&str)->(pocold::node::Node,std::path::PathBuf){
    use pocold::node::{Config,Node};
    let dir=std::env::temp_dir().join(format!("pocold-a11-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    (Node::open(Config{dir:dir.clone(),name:"a11".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Default::default()},admitted(&cfg(&["a","b","c"]))).unwrap(),dir)
}
fn pay(key:&Key,nonce:u64,value:u64)->Vec<u8>{Tx{chain_id:777910,nonce,max_priority:U256::from(1_000_000_000u64),max_fee:U256::from(100_000_000_000u64),gas:21000,to:Some(devnet::account_key("bob").address()),value:U256::from(value),data:Default::default()}.sign_1559(key)}
/// The pool after its revalidation backlog (decision D-7) is settled.
fn pool(n:&mut pocold::node::Node)->Vec<Vec<u8>>{n.pool_settle();n.pool.txs.iter().map(|(_,r)|r.clone()).collect()}
/// Builds `count` fallback blocks on `parent` in harness `h` (importing them there) and returns them.
fn branch(h:&mut Net,parent:[u8;32],count:usize,bodies:&[Body])->Vec<Block>{
    let mut out=vec![];let mut p=parent;
    for i in 0..count{let b=match bodies.get(i){Some(body) if !body.txs.is_empty()=>signed(h,p,body.clone(),0),_=>fallback(h,p,0)};import(h,&b);p=b.hash();out.push(b);}
    out
}
fn slice_node(tag:&str,slice:std::time::Duration)->(pocold::node::Node,std::path::PathBuf){
    use pocold::node::{Config,Limits,Node};
    let dir=std::env::temp_dir().join(format!("pocold-slice-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    (Node::open(Config{dir:dir.clone(),name:tag.into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Limits{pool_slice:slice,..Default::default()}},admitted(&cfg(&["a","b","c"]))).unwrap(),dir)
}
/// Decision D-7: pool revalidation after a new head runs in slices, one event between two slices. A
/// node whose slice admits one item at a time ends, once its backlog is settled, with exactly the pool
/// and pending state of a node that revalidates everything at once (the behaviour before D-7): the same
/// transactions in the same order, through a reorg that detaches a block. While its backlog lasts, a
/// template still sees every candidate, and an in-process submission waits for the backlog.
#[test] fn sliced_pool_revalidation_equals_the_whole_rebuild(){
    let (alice,bob)=(devnet::account_key("alice"),devnet::account_key("bob"));
    let (mut a,da)=slice_node("one",std::time::Duration::ZERO);let (mut b,db)=slice_node("all",std::time::Duration::from_secs(3600));
    let mut h=net();let g=h.chain.head;
    let mined=pay(&alice,0,7);let ours=branch(&mut h,g,1,&[Body{txs:vec![mined.clone()],evidence:vec![]}]);
    for n in [&mut a,&mut b]{assert!(n.import(ours[0].clone(),None).unwrap());}
    let pooled:Vec<Vec<u8>>=(1..13).map(|k|pay(&alice,k,k)).chain((0..8).map(|k|pay(&bob,k,100+k))).collect();
    for n in [&mut a,&mut b]{for t in &pooled{n.submit_tx(t.clone(),false).unwrap();}}
    let mut h2=net();let theirs=branch(&mut h2,g,2,&[]);
    for n in [&mut a,&mut b]{for x in &theirs{n.import(x.clone(),None).unwrap();}assert_eq!(n.chain.head,theirs[1].hash());}
    assert!(a.pool_pending()&&!b.pool_pending(),"positive control: the one-item slice left a backlog, the whole rebuild did not");
    let head=a.chain.head().meta_clone();assert_eq!(a.template_txs(&head),b.template_txs(&head),"a template during the backlog sees the same candidates");
    a.pool_settle();
    let expected:Vec<Vec<u8>>=std::iter::once(mined).chain(pooled.iter().cloned()).collect();
    assert_eq!(pool(&mut b),expected,"the whole rebuild: the detached transaction first, then the pool in order");
    assert_eq!(pool(&mut a),pool(&mut b),"sliced revalidation gives the same pool");
    assert_eq!(apply::state_root(&a.pool_state()).unwrap(),apply::state_root(&b.pool_state()).unwrap(),"and the same pending state");
    assert!(a.stats.pool_slices>1,"it ran in more than one slice: {}",a.stats.pool_slices);
    // An in-process submission during a backlog is executed after it, in pool order.
    let (mut c,dc)=slice_node("submit",std::time::Duration::ZERO);c.import(ours[0].clone(),None).unwrap();
    for t in &pooled{c.submit_tx(t.clone(),false).unwrap();}for x in &theirs{c.import(x.clone(),None).unwrap();}
    assert!(c.pool_pending());let late=pay(&alice,13,1);c.submit_tx(late.clone(),false).unwrap();
    assert!(!c.pool_pending());assert_eq!(pool(&mut c).last(),Some(&late),"after the backlog, never before it");
    for d in [da,db,dc]{let _=std::fs::remove_dir_all(&d);}
}
/// Decision D-7, measurement (release, --ignored): a pool of D7_K heavy transactions (BLAKE2F with
/// 7.8M rounds, about GAS_LIMIT each, which no stop hook can cut short) is revalidated after a new head.
/// D7_K defaults to 4, the most the default gas bound (4 × GAS_LIMIT) holds; a larger D7_K raises the bound
/// to hold them (the measurement before the bound used 12).
/// With POOL_SLICE the import, and then every slice, holds the consensus thread for at most the slice
/// plus one transaction; revalidating at once (the behaviour before D-7) holds it for the whole pool.
#[test] #[ignore="D-7 measurement: run in release with --ignored --nocapture"]
fn d7_pool_revalidation_hold_under_a_heavy_pool(){
    use std::time::{Duration,Instant};
    let k:u64=std::env::var("D7_K").ok().and_then(|s|s.parse().ok()).unwrap_or(4);let gas_max=(k*7_900_000).max(32_000_000);
    let alice=devnet::account_key("alice");
    let input=pocold::types::unhex(&format!("{:08x}{}01",7_800_000u32,"00".repeat(64+128+16))).unwrap();
    let heavy=|nonce:u64|Tx{chain_id:777910,nonce,max_priority:U256::from(1_000_000_000u64),max_fee:U256::from(100_000_000_000u64),gas:7_900_000,to:Some(Address::with_last_byte(9)),value:U256::ZERO,data:input.clone().into()}.sign_1559(&alice);
    let mut h=net();let g=h.chain.head;let blocks=branch(&mut h,g,2,&[]);
    let mut out=serde_json::Map::new();
    for (tag,slice) in [("sliced",pocold::node::POOL_SLICE),("whole",Duration::from_secs(3600))]{
        let (n,dir)=gas_node(&format!("d7-{tag}"),pocold::node::Limits{pool_slice:slice,pool_gas_max:Some(gas_max),..Default::default()});let mut n=n.unwrap();
        n.import(blocks[0].clone(),None).unwrap();
        let mut max_tx=0f64;for i in 0..k{let t=Instant::now();n.submit_tx(heavy(i),false).unwrap();max_tx=max_tx.max(t.elapsed().as_secs_f64()*1e3);}
        let t=Instant::now();n.import(blocks[1].clone(),None).unwrap();let import_ms=t.elapsed().as_secs_f64()*1e3;
        let (mut steps,mut step_max,t)=(0u64,0f64,Instant::now());
        while n.pool_pending(){let s0=Instant::now();n.pool_step();step_max=step_max.max(s0.elapsed().as_secs_f64()*1e3);steps+=1;}
        let rest_ms=t.elapsed().as_secs_f64()*1e3;
        assert_eq!(n.pool.txs.len() as u64,k,"{tag}: every heavy transaction is still valid and pooled");
        if tag=="sliced"{let bound=slice.as_secs_f64()*1e3+max_tx+25.0;
            assert!(step_max<=bound,"a slice held the thread {step_max:.0} ms, over the slice + one transaction ({bound:.0} ms)");}
        out.insert(tag.into(),serde_json::json!({"poolTxs":k,"maxTxMs":max_tx,"importMs":import_ms,"slicesAfterImport":steps,"sliceMaxMs":step_max,"restMs":rest_ms,"sliceMs":slice.as_millis() as u64,"poolGasMax":gas_max}));
        let _=std::fs::remove_dir_all(&dir);
    }
    println!("D7-MEASURE {}",serde_json::Value::Object(out));
}
/// A transfer to bob from `key` that declares `gas` (decision D-7).
fn pay_gas(key:&Key,nonce:u64,value:u64,gas:u64)->Vec<u8>{Tx{chain_id:777910,nonce,max_priority:U256::from(1_000_000_000u64),max_fee:U256::from(100_000_000_000u64),gas,to:Some(devnet::account_key("bob").address()),value:U256::from(value),data:Default::default()}.sign_1559(key)}
fn gas_node(tag:&str,limits:pocold::node::Limits)->(Result<pocold::node::Node,String>,std::path::PathBuf){
    use pocold::node::{Config,Node};
    let dir=std::env::temp_dir().join(format!("pocold-poolgas-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    (Node::open(Config{dir:dir.clone(),name:tag.into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits},admitted(&cfg(&["a","b","c"]))),dir)
}
/// Decision D-7 (approved 2026-10-25: POOL_GAS_BLOCKS × GAS_LIMIT by default), admission. A transaction whose
/// declared gas limit would take the pooled sum past the bound is refused with -32016 (gas) before it is
/// executed, and changes nothing; a duplicate is answered with its hash even at the bound; a new head recounts
/// the sum from what stays.
#[test] fn the_pool_gas_bound_refuses_past_it_and_is_recounted_on_a_new_head(){
    use pocold::node::{POOL_FULL_GAS,POOL_GAS_BLOCKS};
    let (n,dir)=gas_node("admit",Default::default());let mut n=n.unwrap();
    let gl=n.chain.spec.cp().GAS_LIMIT;assert_eq!((n.pool.gas_max,POOL_GAS_BLOCKS*gl),(32_000_000,32_000_000),"the default bound");
    let alice=devnet::account_key("alice");
    let txs:Vec<Vec<u8>>=(0..4).map(|k|pay_gas(&alice,k,1,gl)).collect();
    for t in &txs{n.submit_tx(t.clone(),false).unwrap();}
    assert_eq!((n.pool.txs.len(),n.pool.gas),(4,4*gl),"four transactions that declare GAS_LIMIT fill it exactly");n.pool_audit().unwrap();
    let root=apply::state_root(&n.pool_state()).unwrap();
    for t in [pay_gas(&alice,4,1,gl),pay(&alice,4,1)]{assert_eq!(n.submit_tx(t,false).unwrap_err(),POOL_FULL_GAS,"one gas past the bound is refused");}
    assert_eq!((n.pool.txs.len(),n.pool.gas),(4,4*gl),"refused, nothing pooled");
    assert_eq!(apply::state_root(&n.pool_state()).unwrap(),root,"and the pending state is unchanged");
    // A duplicate is not a new transaction: its hash, and nothing counted.
    assert_eq!(n.submit_tx(txs[0].clone(),false).unwrap(),n.pool.txs[0].0);assert_eq!(n.pool.gas,4*gl);n.pool_audit().unwrap();
    // A block that includes the first one (a block holds one transaction that declares GAS_LIMIT): the pool is
    // revalidated on it and its gas recounted, so a new one fits again.
    let mut h=net();let g=h.chain.head;let b=branch(&mut h,g,1,&[Body{txs:vec![txs[0].clone()],evidence:vec![]}]);
    n.import(b[0].clone(),None).unwrap();n.pool_settle();assert_eq!((n.pool.txs.len(),n.pool.gas),(3,3*gl));n.pool_audit().unwrap();
    n.submit_tx(pay(&alice,4,1),false).unwrap();assert_eq!(n.pool.gas,3*gl+21000);n.pool_audit().unwrap();
    drop(n);let _=std::fs::remove_dir_all(&dir);
}
/// D-7: the bound is a local setting (`Limits::pool_gas_max`, `pocold run --pool-gas-max`). Below GAS_LIMIT the
/// node refuses to open, since a transaction a block can hold must fit an empty pool; at GAS_LIMIT one such
/// transaction fills it; a larger value is taken as given (A13 counts the pool at it).
#[test] fn the_pool_gas_bound_is_configurable_and_refused_below_gas_limit(){
    use pocold::node::{Limits,POOL_FULL_GAS};
    let gl=8_000_000u64;let alice=devnet::account_key("alice");
    let (low,d0)=gas_node("low",Limits{pool_gas_max:Some(gl-1),..Default::default()});
    match low{Err(e)=>assert!(e.contains("below GAS_LIMIT"),"{e}"),Ok(_)=>panic!("a bound below GAS_LIMIT opened")}
    let (one,d1)=gas_node("one",Limits{pool_gas_max:Some(gl),..Default::default()});let mut one=one.unwrap();assert_eq!(one.pool.gas_max,gl);
    one.submit_tx(pay_gas(&alice,0,1,gl),false).unwrap();assert_eq!(one.submit_tx(pay(&alice,1,1),false).unwrap_err(),POOL_FULL_GAS);
    let (big,d2)=gas_node("big",Limits{pool_gas_max:Some(16*gl),..Default::default()});let big=big.unwrap();assert_eq!(big.pool.gas_max,16*gl);
    drop((one,big));for d in [d0,d1,d2]{let _=std::fs::remove_dir_all(&d);}
}
/// D-7, replacement: the pool keeps the first transaction of a sender and nonce. Another with the same nonce
/// (whatever its fee) is refused by its execution on the pending state (-32000, nonce) and changes no counter.
/// The pooled one leaves only when a new head makes it invalid, here by including the other, and its gas is
/// freed then.
#[test] fn a_same_nonce_transaction_does_not_replace_a_pooled_one(){
    let (n,dir)=gas_node("nonce",Default::default());let mut n=n.unwrap();let alice=devnet::account_key("alice");
    let a=pay_gas(&alice,0,1,1_000_000);
    let b=Tx{chain_id:777910,nonce:0,max_priority:U256::from(50_000_000_000u64),max_fee:U256::from(200_000_000_000u64),gas:21000,to:Some(devnet::account_key("bob").address()),value:U256::from(2u64),data:Default::default()}.sign_1559(&alice);
    n.submit_tx(a.clone(),false).unwrap();
    let e=n.submit_tx(b.clone(),false).unwrap_err();assert!(e.starts_with("-32000")&&e.contains("NonceTooLow"),"{e}");
    assert_eq!(pool(&mut n),vec![a.clone()]);assert_eq!(n.pool.gas,1_000_000,"the refused one is not counted");n.pool_audit().unwrap();
    let mut h=net();let g=h.chain.head;let x=branch(&mut h,g,1,&[]);n.import(x[0].clone(),None).unwrap();
    assert_eq!(pool(&mut n),vec![a.clone()],"a head without either keeps it");n.pool_audit().unwrap();
    let y=branch(&mut h,x[0].hash(),1,&[Body{txs:vec![b],evidence:vec![]}]);n.import(y[0].clone(),None).unwrap();
    assert!(pool(&mut n).is_empty(),"a head that includes the other drops it");assert_eq!(n.pool.gas,0);n.pool_audit().unwrap();
    drop(n);let _=std::fs::remove_dir_all(&dir);
}
/// D-7, re-insertion after a reorganization (consensus.md: excluded transactions return to the pool once checked
/// again). The abandoned block's transactions come first, in block order, then the previous pool in its order,
/// each admitted within every pool bound. One that no longer fits the gas bound is dropped and is not pooled
/// later; a later, smaller one is still admitted. The counters equal the contents throughout.
#[test] fn reorg_reinsertion_puts_detached_transactions_first_within_the_gas_bound(){
    let (n,dir)=gas_node("reorg",Default::default());let mut n=n.unwrap();
    let (alice,bob)=(devnet::account_key("alice"),devnet::account_key("bob"));let m=1_000_000u64;
    let (a0,a1)=(pay_gas(&alice,0,1,4*m),pay_gas(&alice,1,1,4*m));
    let mut h=net();let g=h.chain.head;let x=branch(&mut h,g,1,&[Body{txs:vec![a0.clone(),a1.clone()],evidence:vec![]}]);
    n.import(x[0].clone(),None).unwrap();assert_eq!(n.chain.head,x[0].hash());
    let (b0,b1,b2,b3)=(pay_gas(&bob,0,1,8*m),pay_gas(&bob,1,1,8*m),pay_gas(&bob,2,1,7_900_000),pay_gas(&bob,3,1,8*m));let a2=pay(&alice,2,1);
    for t in [&b0,&b1,&b2,&b3,&a2]{n.submit_tx(t.clone(),false).unwrap();}
    assert_eq!(n.pool.gas,31_921_000);n.pool_audit().unwrap();
    // A heavier branch without x: a0 and a1 (8M) return first; b0, b1 and b2 fill the bound to 31.9M; b3 (8M) no
    // longer fits and is dropped; a2 (21,000) still fits.
    let mut h2=net();let y=branch(&mut h2,g,2,&[]);
    for b in &y{n.import(b.clone(),None).unwrap();n.pool_audit().unwrap();}
    assert_eq!(n.chain.head,y[1].hash(),"reorganized");
    assert_eq!(pool(&mut n),vec![a0,a1,b0,b1,b2,a2],"detached first, then the pool in order, b3 dropped");
    assert_eq!(n.pool.gas,8*m+16*m+7_900_000+21000);n.pool_audit().unwrap();
    assert_eq!(n.submit_tx(b3,false).unwrap_err(),pocold::node::POOL_FULL_GAS,"b3 is not pooled later on its own");
    drop(n);let _=std::fs::remove_dir_all(&dir);
}
/// The frames a node sent on `s` within `ms`, decoded; the node handles its queued events (the peer's
/// connection and Hello) between reads.
fn frames_from(n:&mut pocold::node::Node,s:&mut std::net::TcpStream,buf:&mut Vec<u8>,ms:u64)->Vec<pocold::net::Msg>{
    use std::io::Read;let t=std::time::Instant::now();let mut out=Vec::new();let mut b=[0u8;65536];
    while t.elapsed()<std::time::Duration::from_millis(ms){
        n.drain_between_members();
        match s.read(&mut b){Ok(0)=>break,Ok(k)=>buf.extend_from_slice(&b[..k]),Err(_)=>{}}
        while buf.len()>=4{let len=u32::from_be_bytes(buf[..4].try_into().unwrap()) as usize;if buf.len()<4+len{break}
            out.push(pocold::net::Msg::decode(&buf[4..4+len]).unwrap());buf.drain(..4+len);}
    }
    out
}
/// D-7, re-insertion reaches the network. A transaction returned from an abandoned block may be held by this node
/// alone: in e2e_integrated a node was isolated, its blocks were abandoned at the heal, and it was not drawn on the
/// healed branch (Draw(h,0) is weighted by shares on that branch), so the transactions it returned to its pool
/// were never included. Each one admitted again is offered to the peers, and the peer is kept; a transaction of
/// the previous pool, relayed when it was first admitted, is not sent again.
#[test] fn transactions_returned_from_an_abandoned_block_are_offered_to_the_peers(){
    use pocold::{net::Msg,node::{Config,Node}};use std::io::Write;
    let port=std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let dir=std::env::temp_dir().join(format!("pocold-poolgas-offer-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    let spec=admitted(&cfg(&["a","b","c"]));let (chain_id,genesis)=(spec.chain_id(),spec.genesis_hash());
    let mut n=Node::open(Config{dir:dir.clone(),name:"offer".into(),miner:None,listen:Some(format!("127.0.0.1:{port}").parse().unwrap()),peers:vec![],rpc:None,stop_at:None,log:false,limits:Default::default()},spec).unwrap();
    n.start_network().unwrap();
    let mut s=std::net::TcpStream::connect(("127.0.0.1",port)).unwrap();s.set_read_timeout(Some(std::time::Duration::from_millis(50))).unwrap();
    let hello=Msg::Hello{chain_id,genesis,head:genesis,height:0,work:[0;32],node:"peer".into(),protocol:pocold::identity::PROTOCOL_VERSION,compatibility:pocold::identity::compatibility()}.encode();
    let mut f=(hello.len() as u32).to_be_bytes().to_vec();f.extend(hello);s.write_all(&f).unwrap();
    let mut buf=Vec::new();let mut sent=frames_from(&mut n,&mut s,&mut buf,1500);
    assert_eq!(n.status_json()["peers"].as_array().unwrap().len(),1,"the peer is connected");
    let (alice,bob)=(devnet::account_key("alice"),devnet::account_key("bob"));
    let (a0,a1,b0)=(pay(&alice,0,1),pay(&alice,1,1),pay(&bob,0,1));
    // a0 and a1 reach this node only in its block x; b0 is submitted here and relayed then.
    let mut h=net();let g=h.chain.head;let x=branch(&mut h,g,1,&[Body{txs:vec![a0.clone(),a1.clone()],evidence:vec![]}]);
    n.import(x[0].clone(),None).unwrap();n.submit_tx(b0.clone(),true).unwrap();
    let mut h2=net();let y=branch(&mut h2,g,2,&[]);for b in &y{n.import(b.clone(),None).unwrap();}
    assert_eq!(n.chain.head,y[1].hash(),"reorganized");assert_eq!(pool(&mut n),vec![a0.clone(),a1.clone(),b0.clone()]);
    sent.extend(frames_from(&mut n,&mut s,&mut buf,1500));
    let txs:Vec<Vec<u8>>=sent.into_iter().filter_map(|m|match m{Msg::Tx{raw}=>Some(raw),_=>None}).collect();
    let count=|t:&Vec<u8>|txs.iter().filter(|r|*r==t).count();
    assert_eq!((count(&a0),count(&a1)),(1,1),"each returned transaction is offered once");
    assert_eq!(count(&b0),1,"the previous pool's transaction is not sent again");
    assert_eq!(txs.len(),3);
    let st=n.status_json();assert_eq!((st["stats"]["reinsertedOffered"].as_u64(),st["stats"]["reinsertedUnoffered"].as_u64()),(Some(2),Some(0)));
    assert_eq!(st["peers"].as_array().unwrap().len(),1,"the peer is kept");
    drop(s);drop(n);let _=std::fs::remove_dir_all(&dir);
}
/// D-7, counters: a deterministic sequence of submissions (fitting, past the gas bound, duplicates, same nonce,
/// wrong nonce), relays during a revalidation backlog, one-item slices, new heads from the node's own template
/// and reorganizations. After every step the pool's counters equal its contents and every bound holds
/// (`Node::pool_audit`). The positive controls: each kind of step happened.
#[test] fn the_pool_counters_equal_the_contents_through_admission_revalidation_and_reorgs(){
    use pocold::{net::{Msg,NetEvent},node::{Event,Limits}};
    let (n,dir)=gas_node("audit",Limits{pool_slice:std::time::Duration::ZERO,..Default::default()});let mut n=n.unwrap();
    let (peer,_socket)=auth_peer::connect(&mut n);
    let keys=[devnet::account_key("alice"),devnet::account_key("bob")];let mut h=net();
    let mut seed=0x9e37_79b9_7f4a_7c15u64;let mut rnd=|k:u64|{seed^=seed<<13;seed^=seed>>7;seed^=seed<<17;seed%k};
    let (mut admitted_n,mut gas_refused,mut relayed_in_backlog,mut heads,mut reorgs)=(0,0,0,0,0);
    let gases=[21000u64,4_000_000,8_000_000,8_000_000];
    for step in 0..160{
        match rnd(10){
            0..=3=>{let k=&keys[rnd(2) as usize];
                // Mostly the sender's next nonce on the pending state, sometimes another one.
                let next=n.pool_state().accounts.get(&k.address()).map_or(0,|a|a.info.nonce);let nonce=if rnd(5)==0{rnd(12)}else{next};
                let raw=pay_gas(k,nonce,1+rnd(3),gases[rnd(4) as usize]);
                match n.submit_tx(raw,false){Ok(_)=>admitted_n+=1,Err(e) if e==pocold::node::POOL_FULL_GAS=>gas_refused+=1,Err(_)=>{}}}
            4=>if let Some((_,raw))=n.pool.txs.first().cloned(){n.submit_tx(raw,false).unwrap();},
            5=>{let k=&keys[rnd(2) as usize];let raw=pay_gas(k,rnd(12),1,gases[rnd(4) as usize]);if n.pool_pending(){relayed_in_backlog+=1;}
                n.handle(Event::Net(NetEvent::Message(peer,Msg::Tx{raw})));}
            6|7=>n.pool_step(),
            8=>{let head=n.chain.head().meta_clone();let txs=n.template_txs(&head);let p=n.chain.head;h.chain.ensure_state(&p).unwrap();
                let b=if txs.is_empty(){fallback(&h,p,0)}else{signed(&h,p,Body{txs,evidence:vec![]},0)};import(&mut h,&b);n.import(b,None).unwrap();heads+=1;}
            _=>{let head=n.chain.head();if let Some(p)=head.parent.filter(|_|head.height>1){let old=n.chain.head;
                // Two fallback blocks on the head's parent, at an attempt not used there before (a new block).
                let mut q=p;for _ in 0..2{h.chain.ensure_state(&q).unwrap();let mut a=0;let b=loop{let b=fallback(&h,q,a);if !h.chain.blocks.contains_key(&b.hash()){break b}a+=1;};
                    import(&mut h,&b);n.import(b.clone(),None).unwrap();q=b.hash();}
                if n.chain.head!=old{reorgs+=1;}}}
        }
        n.pool_audit().unwrap_or_else(|e|panic!("step {step}: {e}"));
    }
    n.pool_settle();n.pool_audit().unwrap();
    println!("D7-AUDIT admitted {admitted_n} refusedForGas {gas_refused} relayedInBacklog {relayed_in_backlog} heads {heads} reorgs {reorgs}");
    assert!(admitted_n>10&&gas_refused>0&&relayed_in_backlog>0&&heads>3&&reorgs>0,"every kind of step ran: admitted {admitted_n}, refused for gas {gas_refused}, relayed in a backlog {relayed_in_backlog}, heads {heads}, reorgs {reorgs}");
    drop(n);let _=std::fs::remove_dir_all(&dir);
}
/// D-7 over JSON-RPC: eth_sendRawTransaction past the gas bound is a -32016 error that names the gas bound, the
/// transaction is not pooled, and pocol_status reports the pooled gas and its bound.
#[test] fn rpc_answers_minus_32016_past_the_pool_gas_bound_and_reports_it(){
    use pocold::rpc::{answer,Request};use std::time::{Duration,Instant};
    let (n,dir)=gas_node("rpc",Default::default());let mut n=n.unwrap();let alice=devnet::account_key("alice");
    for k in 0..4{n.submit_tx(pay_gas(&alice,k,1,8_000_000),false).unwrap();}
    let call=|n:&mut pocold::node::Node,body:String|->serde_json::Value{let (t,_r)=std::sync::mpsc::channel();
        serde_json::from_str(&answer(n,&Request::local(body.into_bytes(),t,Instant::now()+Duration::from_secs(30),false)).body).unwrap()};
    let raw=pay(&alice,4,1);
    let v=call(&mut n,format!(r#"{{"jsonrpc":"2.0","id":7,"method":"eth_sendRawTransaction","params":["{}"]}}"#,pocold::types::hex(&raw)));
    assert_eq!((v["id"].clone(),v["error"]["code"].clone(),v["error"]["message"].clone()),(serde_json::json!(7),serde_json::json!(-32016),serde_json::json!(pocold::node::POOL_FULL_GAS)),"{v}");
    assert!(v.get("result").is_none(),"{v}");assert_eq!(n.pool.txs.len(),4,"not pooled");
    let s=call(&mut n,r#"{"jsonrpc":"2.0","id":8,"method":"pocol_status","params":[]}"#.into());
    assert_eq!((s["result"]["pool"].clone(),s["result"]["poolGas"].clone(),s["result"]["poolGasMax"].clone()),(serde_json::json!(4),serde_json::json!(32_000_000u64),serde_json::json!(32_000_000u64)),"{s}");
    drop(n);let _=std::fs::remove_dir_all(&dir);
}
/// D-7: pool revalidation does not hold up consensus. A node runs its event loop (`Node::run`) with the pool at
/// its gas bound: four BLAKE2F transactions of 7.9M gas each, which no stop hook can cut short. Two blocks arrive
/// back to back. The second is imported while the first head's revalidation backlog is still pending, and an
/// RPC request is answered between slices. The gas bound alone bounds no time: what bounds the hold is the slice
/// (POOL_SLICE) plus one transaction, whose time is measured here, not derived. Release and debug.
#[test] fn pool_revalidation_at_the_gas_bound_does_not_hold_up_block_import(){
    use pocold::{net::{Msg,NetEvent},node::{Config,Event,Limits,Node},rpc::Request};use std::time::{Duration,Instant};
    let mut h=net();let g=h.chain.head;let blocks=branch(&mut h,g,3,&[]);
    let dir=std::env::temp_dir().join(format!("pocold-poolgas-run-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    let (ready_tx,ready_rx)=std::sync::mpsc::channel();let d=dir.clone();
    let node=std::thread::Builder::new().stack_size(64<<20).spawn(move||{
        // One-item slices make the order of events and slices deterministic. The status request measures the hold,
        // so it waits for admission as long as its deadline: with QUEUE_WAIT's 2 s, a loaded host whose heavy
        // transaction takes 0.5 s answers it busy (-32021, network.md:232-234) before the bound below is reached
        // (seen in a suite run beside a scale measurement; admission has its own tests).
        let mut n=Node::open(Config{dir:d,name:"run".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:Some(3),log:false,
            limits:Limits{pool_slice:Duration::ZERO,rpc:pocold::rpc::Limits{queue_wait:Duration::from_secs(120),..Default::default()},..Default::default()}},admitted(&cfg(&["a","b","c"]))).unwrap();
        let alice=devnet::account_key("alice");
        let input=pocold::types::unhex(&format!("{:08x}{}01",7_800_000u32,"00".repeat(64+128+16))).unwrap();
        let mut max_tx=Duration::ZERO;
        for nonce in 0..4{let raw=Tx{chain_id:777910,nonce,max_priority:U256::from(1_000_000_000u64),max_fee:U256::from(100_000_000_000u64),gas:7_900_000,to:Some(Address::with_last_byte(9)),value:U256::ZERO,data:input.clone().into()}.sign_1559(&alice);
            let t=Instant::now();n.submit_tx(raw,false).unwrap();max_tx=max_tx.max(t.elapsed());}
        assert_eq!((n.pool.txs.len(),n.pool.gas),(4,31_600_000),"the pool is at its gas bound");
        let (peer,_socket)=auth_peer::connect(&mut n);
        ready_tx.send((n.tx.clone(),max_tx,peer)).unwrap();n.run();
        n.pool_settle();(n.pool.txs.len(),n.chain.head().height,n.pool_audit())
    }).unwrap();
    let (tx,max_tx,peer)=ready_rx.recv().unwrap();
    let send_block=|b:&Block|tx.send(Event::Net(NetEvent::Message(peer,Msg::Block{header:b.header.encoded.clone(),body:b.body.encode()}))).unwrap();
    let status=||{let (t,r)=std::sync::mpsc::channel();
        tx.send(Event::Rpc(Request::local(br#"{"jsonrpc":"2.0","id":1,"method":"pocol_status","params":[]}"#.to_vec(),t,Instant::now()+Duration::from_secs(120),false))).unwrap();
        let body=r.recv().unwrap().body;let v:serde_json::Value=serde_json::from_str(&body).unwrap();
        match (v["result"]["head"]["height"].as_u64(),v["result"]["stats"]["poolBacklog"].as_u64()){(Some(h),Some(b))=>(h,b),_=>panic!("the status request was not answered with a status: {body}")}};
    send_block(&blocks[0]);let t0=Instant::now();send_block(&blocks[1]);
    let (height,backlog)=status();let took=t0.elapsed();
    assert_eq!(height,2,"both blocks were imported before the status request");
    assert!(backlog>0,"the second block was imported while revalidation was still pending (backlog {backlog})");
    // Before the second block: at most the first block's own slice and one more (two transactions); after it,
    // its slice and one more before the status request is taken. Generous for a loaded machine.
    let bound=max_tx*6+Duration::from_secs(2);assert!(took<bound,"the second block and the answer took {took:?}, over {bound:?} (one transaction: {max_tx:?})");
    send_block(&blocks[2]);
    let (pooled,height,audit)=node.join().unwrap();
    assert_eq!((pooled,height),(4,3),"every heavy transaction is still valid and pooled at the last head");audit.unwrap();
    println!("D7-RUN maxTxMs {} secondBlockAndAnswerMs {}",max_tx.as_millis(),took.as_millis());
    let _=std::fs::remove_dir_all(&dir);
}
/// Decision D-7: an eth_sendRawTransaction that arrives during a pool backlog drives the backlog until
/// its compute deadline. If the backlog is not done by then, the answer is -32021 {busy, retryAfterMs}
/// and nothing is pooled; once the backlog is done the same request is admitted. In process.
#[test] fn a_submission_during_a_long_backlog_is_busy_at_its_deadline_and_not_pooled(){
    use pocold::{node::{Config,Limits,Node},rpc::{answer,Request}};use std::time::{Duration,Instant};
    let dir=std::env::temp_dir().join(format!("pocold-busy-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    let mut l=Limits{pool_slice:Duration::ZERO,..Default::default()};l.rpc.compute_light=Duration::from_millis(100);
    let mut n=Node::open(Config{dir:dir.clone(),name:"busy".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:l},admitted(&cfg(&["a","b","c"]))).unwrap();
    let alice=devnet::account_key("alice");
    let input=pocold::types::unhex(&format!("{:08x}{}01",7_800_000u32,"00".repeat(64+128+16))).unwrap();
    let heavy=|nonce:u64|Tx{chain_id:777910,nonce,max_priority:U256::from(1_000_000_000u64),max_fee:U256::from(100_000_000_000u64),gas:7_900_000,to:Some(Address::with_last_byte(9)),value:U256::ZERO,data:input.clone().into()}.sign_1559(&alice);
    for k in 0..4{n.submit_tx(heavy(k),false).unwrap();}
    let mut h=net();let g=h.chain.head;let b=branch(&mut h,g,1,&[]);n.import(b[0].clone(),None).unwrap();assert!(n.pool_pending(),"a backlog of heavy transactions");
    let body=format!(r#"{{"jsonrpc":"2.0","id":3,"method":"eth_sendRawTransaction","params":["0x{}"]}}"#,pocold::types::hex(&pay(&devnet::account_key("bob"),0,1)).trim_start_matches("0x"));
    let req=||{let (t,_r)=std::sync::mpsc::channel();Request::local(body.as_bytes().to_vec(),t,Instant::now()+Duration::from_secs(30),true)};
    let t=Instant::now();let v:serde_json::Value=serde_json::from_str(&answer(&mut n,&req()).body).unwrap();let took=t.elapsed();
    assert_eq!((v["error"]["code"].clone(),v["error"]["data"]["reason"].clone()),(serde_json::json!(-32021),serde_json::json!("busy")),"{v}");
    assert!(took<Duration::from_millis(100)+Duration::from_millis(400),"held about its deadline plus one transaction: {took:?}");
    assert!(n.pool.txs.iter().all(|(_,r)|*r!=pay(&devnet::account_key("bob"),0,1)),"not pooled");
    n.pool_settle();let v:serde_json::Value=serde_json::from_str(&answer(&mut n,&req()).body).unwrap();assert!(v["result"].is_string(),"positive control: after the backlog it is admitted: {v}");
    let _=std::fs::remove_dir_all(&dir);
}
#[test] fn a11_reorg_restores_detached_transactions_without_resubmission(){
    let alice=devnet::account_key("alice");let t0=pay(&alice,0,7);
    let (mut node,dir)=a11_node("restore");let mut h=net();let g=h.chain.head;
    node.submit_tx(t0.clone(),false).unwrap();assert_eq!(pool(&mut node),vec![t0.clone()]);
    // submit -> mine: the block carrying t0 becomes head and t0 leaves the pool.
    let old=branch(&mut h,g,1,&[Body{txs:vec![t0.clone()],evidence:vec![]}]);
    assert!(node.import(old[0].clone(),None).unwrap());assert!(pool(&mut node).is_empty());
    // A heavier competing branch without t0: t0 is detached and returns, still valid.
    let mut h2=net();let new=branch(&mut h2,g,2,&[]);
    for b in &new{node.import(b.clone(),None).unwrap();}
    assert_eq!(node.chain.head,new[1].hash(),"reorganized to the heavier branch");
    assert_eq!(pool(&mut node),vec![t0],"the detached transaction is back in the pool");
    let _=std::fs::remove_dir_all(&dir);
}
#[test] fn a11_excludes_included_and_conflicting_transactions_and_keeps_order(){
    let alice=devnet::account_key("alice");let bob=devnet::account_key("bob");
    let (t0,t1,t2)=(pay(&alice,0,1),pay(&alice,1,2),pay(&bob,0,3));
    let conflict=pay(&alice,0,99);// same nonce as t0, different transaction
    // Old branch: two blocks [t0], [t1] plus t2 waiting in the pool.
    let (mut node,dir)=a11_node("order");let mut h=net();let g=h.chain.head;
    let old=branch(&mut h,g,2,&[Body{txs:vec![t0.clone()],evidence:vec![]},Body{txs:vec![t1.clone()],evidence:vec![]}]);
    for b in &old{node.import(b.clone(),None).unwrap();}
    node.submit_tx(t2.clone(),false).unwrap();node.submit_tx(t2.clone(),false).unwrap();assert_eq!(pool(&mut node),vec![t2.clone()],"duplicates are kept once");
    // Multiple detached blocks, oldest first, then the previous pool.
    let mut h2=net();let new=branch(&mut h2,g,3,&[]);
    for b in &new{node.import(b.clone(),None).unwrap();}
    assert_eq!(node.chain.head,new[2].hash());assert_eq!(pool(&mut node),vec![t0.clone(),t1.clone(),t2.clone()]);
    // The new canonical branch includes a different transaction with alice's nonce 0: every
    // detached transaction is revalidated on the new head.
    let (mut node,dir2)=a11_node("conflict");let mut h=net();let g=h.chain.head;
    let old=branch(&mut h,g,2,&[Body{txs:vec![t0.clone()],evidence:vec![]},Body{txs:vec![t1.clone()],evidence:vec![]}]);
    for b in &old{node.import(b.clone(),None).unwrap();}
    let mut h2=net();let new=branch(&mut h2,g,3,&[Body{txs:vec![conflict.clone()],evidence:vec![]},Body::default(),Body::default()]);
    for b in &new{node.import(b.clone(),None).unwrap();}
    assert_eq!(node.chain.head,new[2].hash());
    assert_eq!(pool(&mut node),vec![t1.clone()],"t0 conflicts with the included nonce 0 and is dropped; t1 (nonce 1) is valid again");
    // A branch that includes t0 itself: t0 is not restored (already canonical).
    let (mut node,dir3)=a11_node("included");let mut h=net();let g=h.chain.head;
    let old=branch(&mut h,g,1,&[Body{txs:vec![t0.clone()],evidence:vec![]}]);node.import(old[0].clone(),None).unwrap();
    let mut h2=net();let new=branch(&mut h2,g,2,&[Body::default(),Body{txs:vec![t0.clone()],evidence:vec![]}]);
    for b in &new{node.import(b.clone(),None).unwrap();}
    assert_eq!(node.chain.head,new[1].hash());assert!(pool(&mut node).is_empty(),"included on the new branch: not restored");
    for d in [dir,dir2,dir3]{let _=std::fs::remove_dir_all(&d);}
}

/// A reorg at the spec depth whose two sides total more than 2·R_keep blocks (PR #27 review threads;
/// review R5-3): the walk back to the common ancestor stopped after 2·1024 steps, so only the newest
/// detached blocks were restored and a transaction in the oldest one was lost from the pool. Ours is
/// R_keep blocks deep, theirs is heavier and longer (side cap 2·R_keep, so it is held as side blocks
/// until it overtakes), so the walk is longer than 4096 steps and any step bound up to that fails.
/// Release only: about 4,100 blocks are validated twice. Run with
/// `cargo test --release --test consensus -- --ignored a11_deep_reorg`.
#[test] #[ignore="deep-reorg test: run in release with --ignored"]
fn a11_deep_reorg_restores_the_oldest_detached_transaction(){
    let rk=pocold::chain::R_KEEP;
    let alice=devnet::account_key("alice");let t0=pay(&alice,0,7);
    let mut a=net();let g=a.chain.head;let first=t0.clone();
    let ours=grow(&mut a,g,rk as usize,&move|n,p|signed(n,p,Body{txs:vec![first.clone()],evidence:vec![]},0));
    // Theirs also starts with a signed block (a signed block outweighs many fallback blocks), then
    // fallback blocks until its work exceeds ours, then one more, so it is longer than ours.
    let mut b=net();let s0=signed(&b,g,Body::default(),1);import(&mut b,&s0);
    let theirs:Vec<Block>=std::iter::once(s0.clone()).chain(heavier(&mut b,s0.hash(),a.chain.head().total_work,1)).collect();
    assert!(ours.len()+theirs.len()>2*rk as usize,"the two sides total {} blocks",ours.len()+theirs.len());
    let (mut node,dir,_)=rnode("a11-deep",rk,2*pocold::chain::SIDE_BLOCKS_MAX);
    for x in &ours{node.import(x.clone(),None).unwrap();}
    assert!(pool(&mut node).is_empty(),"t0 is included in the oldest block of our branch");
    for x in &theirs{node.import(x.clone(),None).unwrap_or_else(|e|panic!("h={}: {e:?}",x.header.h));}
    assert_eq!(node.chain.head,theirs.last().unwrap().hash(),"reorganized to the heavier branch at depth R_keep");
    assert_eq!(pool(&mut node),vec![t0],"the oldest detached block's transaction is back in the pool");
    let _=std::fs::remove_dir_all(&dir);
}
/// Review R5-7: the transactions of one detached block return in block order (a reversed walk inside
/// a block survived every earlier test).
#[test] fn a11_detached_transactions_keep_their_order_inside_a_block(){
    let alice=devnet::account_key("alice");let (t0,t1)=(pay(&alice,0,1),pay(&alice,1,2));
    let (mut node,dir)=a11_node("inblock");let mut h=net();let g=h.chain.head;
    let old=branch(&mut h,g,1,&[Body{txs:vec![t0.clone(),t1.clone()],evidence:vec![]}]);node.import(old[0].clone(),None).unwrap();
    let mut h2=net();let new=branch(&mut h2,g,2,&[]);for b in &new{node.import(b.clone(),None).unwrap();}
    assert_eq!(node.chain.head,new[1].hash());assert_eq!(pool(&mut node),vec![t0,t1],"both, in block order");
    let _=std::fs::remove_dir_all(&dir);
}
/// Review R5-7: the abandoned branch's oldest block is removed by retention (it is R_keep below the new
/// head) at the retain() that follows the adopting import, before the pool is rebuilt. Its
/// transactions still return: the rebuild reads them from the journal by the offsets taken before
/// retain(), not from the chain's entries.
#[test] fn a11_detached_transactions_return_after_retention_removed_their_block(){
    let rk=8u64;let alice=devnet::account_key("alice");let t0=pay(&alice,0,7);
    let mut a=net();let g=a.chain.head;let first=t0.clone();
    let ours=grow(&mut a,g,rk as usize,&move|n,p|signed(n,p,Body{txs:vec![first.clone()],evidence:vec![]},0));
    let mut b=net();let s0=signed(&b,g,Body::default(),1);import(&mut b,&s0);
    let mut theirs:Vec<Block>=std::iter::once(s0.clone()).chain(heavier(&mut b,s0.hash(),a.chain.head().total_work,0)).collect();
    while theirs.len()<=rk as usize{let p=theirs.last().unwrap().hash();let x=fallback(&b,p,0);import(&mut b,&x);theirs.push(x);}
    let (mut node,dir,_)=rnode("a11-evicted",rk,4*rk as usize);
    for x in &ours{node.import(x.clone(),None).unwrap();}
    for x in &theirs{node.import(x.clone(),None).unwrap_or_else(|e|panic!("h={}: {e:?}",x.header.h));}
    assert_eq!(node.chain.head,theirs.last().unwrap().hash());
    assert!(node.chain.get(&ours[0].hash()).is_none(),"positive control: the block carrying t0 was removed by retention");
    assert_eq!(pool(&mut node),vec![t0],"its transaction is back in the pool all the same");
    let _=std::fs::remove_dir_all(&dir);
}

// ---------------- audit A13 regressions: enforced resource assumptions ----------------
fn admitted_with(c:&DevnetConfig,local:pocol_lp4_execution::admission::NodeLocal)->Spec{Spec::new(devnet::build(c).unwrap().admit(&local).unwrap_or_else(|e|panic!("{e}")).0)}
#[test] fn a13_side_branch_states_are_capped_and_rebuilt_on_demand(){
    let local=pocol_lp4_execution::admission::NodeLocal{side_max:2,..pocold::single_host_local()};
    let c=cfg(&["a","b","c"]);let d=devnet::build(&c).unwrap();let mut keys=HashMap::new();
    for m in &d.miners{keys.insert(m.id,Key::from_secret(&m.key.secret()).unwrap());}
    let mut n=Net{chain:Chain::new(admitted_with(&c,local)),keys};let g=n.chain.head;
    // Five competing children of genesis (attempts 0..4) and a canonical extension.
    let kids:Vec<Block>=(0..5u32).map(|a|fallback(&n,g,a)).collect();
    for b in &kids{let (e,_)=n.chain.validate(b,None,0).unwrap();n.chain.insert(e);n.chain.retain();}
    let (side,with_state)=n.chain.side_counts();assert_eq!(side,4);assert!(with_state<=2,"at most side_max side states: {with_state}");
    // A pruned side block is rebuilt when a child extends it.
    let pruned=kids.iter().map(|b|b.hash()).find(|h|!n.chain.is_canonical(h)&&n.chain.blocks[h].pruned).expect("a pruned side block");
    let child=fallback_on_pruned(&mut n,pruned);
    let (e,_)=match n.chain.validate(&child,None,0){Err(Reject::NeedsRebuild(p))=>{n.chain.ensure_state(&p).unwrap();n.chain.validate(&child,None,0).unwrap()}r=>r.unwrap()};
    assert!(n.chain.insert(e),"the rebuilt branch is now the heaviest");
}
/// Builds a fallback child of `parent` after rebuilding its state in a scratch copy.
fn fallback_on_pruned(n:&mut Net,parent:[u8;32])->Block{n.chain.ensure_state(&parent).unwrap();fallback(n,parent,0)}
/// 2026-10-28: retain's R21 prune visits each canonical height once (`Chain::prune_cursor`, it walked the
/// whole chain at every call) and again only where a rebuild, a restore or a reorg could have given a block
/// below the cut a state back. After every retain each canonical block at or below the cut is pruned, as
/// the old walk left it: on a straight chain, after a rebuild of a pruned block, and after a reorg onto a
/// branch that forks below the cut.
#[test] fn retain_prunes_each_canonical_height_once_and_again_where_a_state_came_back(){
    let mut n=net();n.chain.k_hist=4;let mut hashes=vec![n.chain.head];
    let all_pruned=|n:&Net|{let cut=n.chain.head().height.saturating_sub(n.chain.k_hist);
        (1..=cut).all(|h|n.chain.canonical_at(h).map_or(false,|e|e.pruned&&e.stored.is_none()&&!e.full&&e.diff.is_none()))};
    for _ in 0..30{let h=n.chain.head;let b=fallback(&n,h,0);import(&mut n,&b);n.chain.retain();hashes.push(n.chain.head);
        assert_eq!(n.chain.prune_cursor,n.chain.head().height.saturating_sub(4)+1,"each height once");assert!(n.chain.prune_holds()&&all_pruned(&n));}
    // A rebuild gives a pruned canonical block its state back: the cursor goes back to it, and retain prunes it again.
    n.chain.ensure_state(&hashes[8]).unwrap();assert!(!n.chain.blocks[&hashes[8]].pruned,"positive control: rebuilt");
    assert!(n.chain.prune_cursor<=8,"the rebuild lowered the cursor: {}",n.chain.prune_cursor);
    n.chain.retain();assert!(n.chain.blocks[&hashes[8]].pruned&&n.chain.prune_holds()&&all_pruned(&n),"pruned again");
    // A heavier branch from height 10 (below the cut at 26): its blocks become canonical below the cut.
    let mut p=hashes[10];let mut side=vec![];
    while !side.last().map_or(false,|x|*x==n.chain.head){n.chain.ensure_state(&p).unwrap();let b=signed(&n,p,Body::default(),0);import(&mut n,&b);p=b.hash();side.push(p);if side.len()>40{panic!("not adopted: side {} at {}, head {} at {}",n.chain.get(&p).unwrap().total_work,n.chain.get(&p).unwrap().height,n.chain.head().total_work,n.chain.head().height)}}
    assert!(n.chain.is_canonical(&side[0])&&n.chain.get(&side[0]).unwrap().height==11,"positive control: the fork is canonical from 11");
    assert!(n.chain.prune_cursor<=11,"the reorg lowered the cursor: {}",n.chain.prune_cursor);
    n.chain.retain();assert!(n.chain.prune_holds()&&all_pruned(&n),"the adopted blocks below the cut are pruned");
    for _ in 0..5{let h=n.chain.head;let b=fallback(&n,h,0);import(&mut n,&b);n.chain.retain();assert!(n.chain.prune_holds()&&all_pruned(&n));}
}
/// 2026-10-28, with the test above: a reorg onto side blocks that kept their states (side_max) makes blocks
/// below the cut canonical with no rebuild among them; the canonical change itself lowers `prune_cursor`, and
/// retain prunes them. A side branch removed by R_keep takes its descendants with it, and none is left whose
/// parent is gone (the orphan pass runs after every removal).
#[test] fn retain_prunes_side_blocks_made_canonical_below_the_cut_and_leaves_no_orphan(){
    let mut n=net();n.chain.k_hist=4;let g=n.chain.head;
    let main=grow(&mut n,g,40,&|n,p|fallback(n,p,0));let target=n.chain.head().total_work;
    // A heavier branch of signed blocks from height 10, built on a second node.
    let mut b=net();for x in &main[..10]{import(&mut b,x);}
    let mut p=main[9].hash();let mut side=vec![];
    while b.chain.get(&p).unwrap().total_work<=target{let x=signed(&b,p,Body::default(),0);import(&mut b,&x);p=x.hash();side.push(x);}
    let (last,rest)=side.split_last().unwrap();assert!(rest.len()>=2&&b.chain.get(&p).unwrap().height<36,"positive control: the branch's tip is below the cut");
    n.chain.ensure_state(&main[9].hash()).unwrap();for x in rest{assert!(!import(&mut n,x),"positive control: lighter until its last block");assert!(side_matches(&n.chain));}
    n.chain.retain();let cut=n.chain.head().height-4;assert_eq!(n.chain.prune_cursor,cut+1);
    let tip=n.chain.get(&rest.last().unwrap().hash()).unwrap();assert!(!tip.pruned&&tip.height<cut,"positive control: the side tip below the cut kept its state (side_max)");
    // Its last block makes the branch canonical with no rebuild: only the canonical change lowers the cursor.
    assert!(import(&mut n,last),"positive control: adopted");assert!(n.chain.is_canonical(&rest[0].hash()));
    assert!(side_matches(&n.chain)&&n.chain.side_counts().0>=30,"the old main chain from 11 up is in the side set: {:?}",n.chain.side_counts());
    assert!(n.chain.prune_cursor<=11,"the canonical change lowered the cursor: {}",n.chain.prune_cursor);
    n.chain.retain();assert!(n.chain.prune_holds(),"retain pruned them");
    let cut=n.chain.head().height.saturating_sub(4);
    assert!((1..=cut).all(|h|n.chain.canonical_at(h).map_or(false,|e|e.pruned&&e.stored.is_none()&&!e.full&&e.diff.is_none())));
    // The old main chain from 11 up is now a side branch; R_keep removes its deep blocks, and their descendants go too.
    let before=n.chain.blocks.len();n.chain.r_keep=8;n.chain.retain();
    assert!(n.chain.blocks.len()<before,"positive control: blocks were removed");assert!(side_matches(&n.chain),"side set after the removals: {:?}",n.chain.side_counts());
    assert!(n.chain.blocks.values().all(|e|e.parent.map_or(true,|x|n.chain.blocks.contains_key(&x))),"no block is left whose parent is gone");
    assert!(main[10..].iter().all(|b|n.chain.get(&b.hash()).is_none()),"the old main chain went with its removed blocks");
}
/// 2026-10-28: settle sheds full canonical states from prune_cursor up (every one below it is pruned). A canonical
/// state made full at exactly the cursor's height (cut + 1, the lowest one kept as a diff) is shed again too.
#[test] fn settle_sheds_a_full_canonical_state_at_the_lowest_height_it_keeps(){
    let mut n=net();n.chain.k_hist=4;for _ in 0..20{let h=n.chain.head;let b=fallback(&n,h,0);import(&mut n,&b);n.chain.retain();}
    let cut=n.chain.head().height-4;assert_eq!(n.chain.prune_cursor,cut+1);
    let low=n.chain.canonical[(cut+1) as usize];assert!(!n.chain.blocks[&low].full&&!n.chain.blocks[&low].pruned,"positive control: held as a diff");
    let st=n.chain.state_of(&low).unwrap().into_owned();{let e=n.chain.blocks.get_mut(&low).unwrap();e.state=st.into();e.full=true;}
    n.chain.retain();assert!(!n.chain.blocks[&low].full&&!n.chain.blocks[&low].pruned,"shed again, kept as its diff");
    let full=n.chain.canonical.iter().filter(|h|n.chain.blocks[*h].full).count();assert_eq!(full,2,"genesis and the head");
}
#[test] fn a13_rebuild_of_a_deep_fork_keeps_only_the_target_state(){
    let mut n=net();n.chain.k_hist=4;let mut hashes=vec![n.chain.head];
    for _ in 0..14{let h=n.chain.head;let b=fallback(&n,h,0);import(&mut n,&b);n.chain.retain();hashes.push(n.chain.head);}
    let target=hashes[6];assert!(n.chain.blocks[&target].pruned);
    n.chain.ensure_state(&target).unwrap();
    assert!(!n.chain.blocks[&target].pruned,"target rebuilt");
    for h in &hashes[1..6]{assert!(n.chain.blocks[h].pruned,"intermediate states are not kept (rolling rebuild)");}
}
#[test] fn a13_boot_refuses_a_bound_above_the_memory_budget(){
    use pocold::node::{Config,Limits,Node};
    let dir=std::env::temp_dir().join(format!("pocold-a13-{}",std::process::id()));
    let conf=|limits:Limits,peers:usize|Config{dir:dir.clone(),name:"a13".into(),miner:None,listen:None,peers:(0..peers).map(|i|format!("127.0.0.1:{}",1+i).parse().unwrap()).collect(),rpc:None,stop_at:None,log:false,limits};
    let l=pocold::single_host_local();let default=admitted(&cfg(&["a","b","c"]));
    // The devnet's own configuration: default limits, two configured peers.
    let d=pocold::resources::derive(default.cp(),l.k_hist,l.side_max,&Limits::default(),2).unwrap();
    let need=pocold::resources::budget_required(d.total).unwrap();
    // 1. The approved default boots, with its 1/8 reserve inside the 6 GiB budget.
    assert!(need<=u128::from(pocold::node::MEM_BUDGET),"the default needs {need} B: {d:?}");
    let _=std::fs::remove_dir_all(&dir);
    Node::open(conf(Limits::default(),2),default).expect("the approved default devnet boots");
    // 2. A budget one byte below what that bound requires is refused.
    let _=std::fs::remove_dir_all(&dir);
    let err=Node::open(conf(Limits{mem_budget:(need-1) as u64,..Default::default()},2),admitted(&cfg(&["a","b","c"]))).err().expect("a budget below the requirement is refused");
    assert!(err.starts_with("A13 memory bound")&&err.contains("exceeds the node memory budget"),"{err}");
    // 3. The requirement itself boots (the boundary is exact).
    let _=std::fs::remove_dir_all(&dir);
    Node::open(conf(Limits{mem_budget:need as u64,..Default::default()},2),admitted(&cfg(&["a","b","c"]))).expect("exactly the required budget boots");
    // 4. Network-scale caps cannot be held by a full-snapshot node within the default budget.
    let mut big=cfg(&["a","b","c"]);big.slot_cap=4_000_000;big.acct_cap=1_000_000;
    let _=std::fs::remove_dir_all(&dir);
    let err=Node::open(conf(Limits::default(),2),admitted(&big)).err().expect("network-scale caps exceed the budget");assert!(err.contains("exceeds the node memory budget"),"{err}");
    let mib=|x:u128|format!("{:.1}",x as f64/1048576.0);
    println!("A13-BOUND {}",serde_json::json!({"totalBytes":d.total.to_string(),"totalGiB":format!("{:.3}",d.total as f64/1073741824.0),"requiredBudgetBytes":need.to_string(),"budgetGiB":pocold::node::MEM_BUDGET>>30,
        "snapshots":d.snapshots,"snapshotMiB":mib(d.snapshot_bytes),"codeMiB":mib(d.code),"historyMiB":mib(d.history),"sideBlocksMiB":mib(d.side_blocks),"queuesMiB":mib(d.queues),"rpcMiB":mib(d.rpc),"threadsMiB":mib(d.threads),"executionMiB":mib(d.execution)}));
    let _=std::fs::remove_dir_all(&dir);
}

// ---------------- review finding (A13): the pool's pending state follows the block caps ----------------
#[test] fn a13_pool_pending_state_is_capped_and_normalized(){
    use pocold::node::{Config,Node};
    let mut c=cfg(&["a","b","c"]);c.slot_cap=40;
    let dir=std::env::temp_dir().join(format!("pocold-a13-pool-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    let mut node=Node::open(Config{dir:dir.clone(),name:"pool".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Default::default()},admitted(&c)).unwrap();
    // Each CREATE stores 16 fresh slots in its constructor and leaves an account without code.
    let mut init=Vec::new();for i in 0..16u8{init.extend([0x60,0x01,0x60,i,0x55]);}init.push(0x00);
    let alice=devnet::account_key("alice");
    let create=|nonce:u64|Tx{chain_id:777910,nonce,max_priority:U256::from(1_000_000_000u64),max_fee:U256::from(100_000_000_000u64),gas:1_000_000,to:None,value:U256::ZERO,data:init.clone().into()}.sign_1559(&alice);
    let slots=|n:&Node|n.pool_state().accounts.values().filter(|a|a.info().is_some()).map(|a|a.storage.values().filter(|v|**v!=U256::ZERO).count()).sum::<usize>();
    let base=slots(&node);
    assert!(node.submit_tx(create(0),false).is_ok()&&node.submit_tx(create(1),false).is_ok(),"32 of 40 slots");
    let e=node.submit_tx(create(2),false).unwrap_err();assert!(e.contains("SlotCap"),"{e}");
    assert_eq!(node.pool.txs.len(),2);assert_eq!(slots(&node),base+32,"the refused transaction left no effects");
    // Reads and empty calls leave no residue in the pending state (normalize).
    let mut read=Vec::new();for i in 0..200u8{read.extend([0x60,i,0x54,0x50]);}read.push(0x00);
    let entries=|n:&Node|n.pool_state().accounts.values().map(|a|a.storage.len()).sum::<usize>();let e0=entries(&node);
    let tx=Tx{chain_id:777910,nonce:2,max_priority:U256::from(1_000_000_000u64),max_fee:U256::from(100_000_000_000u64),gas:1_000_000,to:None,value:U256::ZERO,data:read.into()}.sign_1559(&alice);
    node.submit_tx(tx,false).unwrap();assert_eq!(entries(&node),e0,"200 SLOADs add no storage entries");
    // Second review: zero-value calls touch empty accounts. They are removed as in a block (EIP-161),
    // so neither the pending state nor the meter drifts.
    let profile=admitted(&c);let recount=|n:&Node|pocol_lp4_execution::user_phase::PoolMeter::of(&n.pool_state(),profile.profile()).live();
    let accounts=|n:&Node|n.pool_state().accounts.len();let a0=accounts(&node);assert_eq!(node.pool_live(),recount(&node));
    for k in 0..3u64{let to=Address::from([0xee;20]);
        let tx=Tx{chain_id:777910,nonce:3+k,max_priority:U256::from(1_000_000_000u64),max_fee:U256::from(100_000_000_000u64),gas:21000,to:Some(to),value:U256::ZERO,data:Default::default()}.sign_1559(&alice);
        node.submit_tx(tx,false).unwrap();assert_eq!(node.pool_live(),recount(&node),"meter = recount after zero-value transfer {k}");}
    // A constructor making 300 zero-value CALLs to fresh addresses leaves none of them behind.
    let mut calls=Vec::new();for i in 0..300u16{calls.extend([0x60,0x00,0x60,0x00,0x60,0x00,0x60,0x00,0x60,0x00,0x61,(i>>8) as u8,i as u8,0x61,0xff,0xff,0xf1,0x50]);}calls.push(0x00);
    let tx=Tx{chain_id:777910,nonce:6,max_priority:U256::from(1_000_000_000u64),max_fee:U256::from(100_000_000_000u64),gas:8_000_000,to:None,value:U256::ZERO,data:calls.into()}.sign_1559(&alice);
    node.submit_tx(tx,false).unwrap();assert_eq!(node.pool_live(),recount(&node),"meter = recount after 300 empty calls");
    assert!(accounts(&node)<=a0+2,"touched empty accounts are not kept: {} -> {}",a0,accounts(&node));
    let _=std::fs::remove_dir_all(&dir);
}

/// C-1 E3: the RPC "pending" answers are read in place on the pool's layer over the head. Balance, nonce,
/// code and storage at "pending" equal the merged pending state for every account the pool wrote (and a
/// few it did not), "latest" still answers from the head, and eth_call funds its caller's value on an
/// overlay without changing the head.
#[test] fn e3_pending_answers_are_the_pool_layer(){
    let (mut n,dir)=a11_node("e3-pending");
    let (alice,bob)=(devnet::account_key("alice"),devnet::account_key("bob"));
    let mut init=Vec::new();for i in 0..4u8{init.extend([0x60,i+1,0x60,i,0x55]);}init.extend([0x60,0x01,0x60,0x00,0xf3]);
    let create=|nonce:u64|Tx{chain_id:777910,nonce,max_priority:U256::from(1_000_000_000u64),max_fee:U256::from(100_000_000_000u64),gas:1_000_000,to:None,value:U256::ZERO,data:init.clone().into()}.sign_1559(&alice);
    for t in [pay(&alice,0,7),create(1),pay(&alice,2,9),pay(&bob,0,11)]{n.submit_tx(t,false).unwrap();}
    assert_eq!(n.pool.txs.len(),4);assert!(n.pool_written()>=4,"positive control: the layer holds the pool's writes");
    let merged=n.pool_state();let head=n.chain.head().state.clone();
    let accounts:Vec<Address>=[alice.address(),bob.address(),alice.address().create(1),Address::ZERO,Address::repeat_byte(0x77)].into();
    let q=|v:serde_json::Value|v.as_str().map(|s|U256::from_str_radix(s.trim_start_matches("0x"),16).unwrap()).expect("an answer");
    for a in &accounts{let s=format!("{a:#x}");
        for (tag,st) in [("pending",&merged),("latest",&*head)]{
            let acc=st.accounts.get(a).filter(|x|x.info().is_some());
            assert_eq!(q(rpc_local(&mut n,"eth_getBalance",json!([s,tag]))),acc.map(|x|x.info.balance).unwrap_or_default(),"{tag} balance of {s}");
            assert_eq!(q(rpc_local(&mut n,"eth_getTransactionCount",json!([s,tag]))),U256::from(acc.map(|x|x.info.nonce).unwrap_or(0)),"{tag} nonce of {s}");
            let code=acc.and_then(|x|x.info.code.as_ref()).map(|c|c.original_bytes().to_vec()).unwrap_or_default();
            assert_eq!(rpc_local(&mut n,"eth_getCode",json!([s,tag])),json!(format!("0x{}",code.iter().map(|b|format!("{b:02x}")).collect::<String>())),"{tag} code of {s}");
            for k in 0..5u64{assert_eq!(q(rpc_local(&mut n,"eth_getStorageAt",json!([s,format!("{k:#x}"),tag]))),st.accounts.get(a).and_then(|x|x.storage.get(&U256::from(k)).copied()).unwrap_or_default(),"{tag} slot {k} of {s}");}
        }
    }
    // Positive controls: the pending state differs from the head where the pool wrote.
    let nonce=|tag:&str,n:&mut pocold::node::Node|q(rpc_local(n,"eth_getTransactionCount",json!([format!("{:#x}",alice.address()),tag])));
    assert_eq!((nonce("pending",&mut n),nonce("latest",&mut n)),(U256::from(3),U256::ZERO));
    assert_eq!(q(rpc_local(&mut n,"eth_getStorageAt",json!([format!("{:#x}",alice.address().create(1)),"0x3","pending"]))),U256::from(4),"the created contract's slot");
    // eth_call from bob with more value than bob holds: the caller is funded on the overlay only.
    let bal=q(rpc_local(&mut n,"eth_getBalance",json!([format!("{:#x}",bob.address()),"latest"])));
    let call=json!([{"from":format!("{:#x}",bob.address()),"to":format!("{:#x}",alice.address()),"value":format!("{:#x}",bal+U256::from(1))},"latest"]);
    assert_eq!(rpc_local(&mut n,"eth_call",call),json!("0x"),"a funded value call succeeds");
    assert_eq!(q(rpc_local(&mut n,"eth_getBalance",json!([format!("{:#x}",bob.address()),"latest"]))),bal,"the head is unchanged");
    let _=std::fs::remove_dir_all(&dir);
}

/// D-7 (A13 store mode): the pool's layer keeps no copy of the code of the accounts it touched. A store-mode
/// node reads an account's code from state.redb with the account; after a pool transaction calls a
/// contract of the head, the contract's entry holds its code hash only; "pending" still answers its code,
/// and later pool transactions run it. A contract the pool creates keeps its code in the layer's own code,
/// read back the same way.
#[test] fn store_mode_imports_on_the_head_execute_each_block_once(){
    // R14/R19 (2026-10-27): `restore_canonical` re-executed every canonical block above the K_hist cut that had
    // no S3a diff, so that its diff links the head to a retained state. A store-mode entry has no diff (its state
    // is a stored version, read in place), so each import that moved the head re-executed the old head and the
    // new one, and committed a rolling version only to release it. Whole-state mode keeps its diffs.
    use pocold::node::{Config,Limits,Node};
    let mut h=net();let g=h.chain.head;let blocks=branch(&mut h,g,6,&[]);
    for store in [true,false]{
        let dir=std::env::temp_dir().join(format!("pocold-once-{store}-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
        let mut n=Node::open(Config{dir:dir.clone(),name:"once".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Limits{state_store:store,..Default::default()}},admitted(&cfg(&["a","b","c"]))).unwrap();
        for b in &blocks{assert!(n.import(b.clone(),None).unwrap());}
        assert_eq!(n.chain.head,blocks[5].hash());
        assert_eq!(n.chain.restore_reexecs,0,"store {store}: no block re-executed on a chain that only grew");
        drop(n);let _=std::fs::remove_dir_all(&dir);
    }
}
#[test] fn d7_the_store_mode_pool_layer_keeps_no_code_copies(){
    use pocold::node::{Config,Limits,Node};
    let dir=std::env::temp_dir().join(format!("pocold-d7-code-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    let mut n=Node::open(Config{dir:dir.clone(),name:"d7c".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Limits{state_store:true,..Default::default()}},admitted(&cfg(&["a","b","c"]))).unwrap();
    // A counter: slot 0 += 1, and the new value returned.
    let runtime=[0x60,0x00,0x54,0x60,0x01,0x01,0x80,0x60,0x00,0x55,0x60,0x00,0x52,0x60,0x20,0x60,0x00,0xf3];
    let mut init=vec![0x71];init.extend(runtime);init.extend([0x60,0x00,0x52,0x60,0x12,0x60,0x0e,0xf3]);
    let alice=devnet::account_key("alice");
    let tx=|nonce:u64,to:Option<Address>,data:Vec<u8>|Tx{chain_id:777910,nonce,max_priority:U256::from(1_000_000_000u64),max_fee:U256::from(100_000_000_000u64),gas:200_000,to,value:U256::ZERO,data:data.into()}.sign_1559(&alice);
    let (head_c,pool_c)=(alice.address().create(0),alice.address().create(2));
    let mut h=net();let g=h.chain.head;let b=branch(&mut h,g,1,&[Body{txs:vec![tx(0,None,init.clone())],evidence:vec![]}]);
    assert!(n.import(b[0].clone(),None).unwrap());
    n.submit_tx(tx(1,Some(head_c),vec![]),false).unwrap();n.submit_tx(tx(2,None,init),false).unwrap();
    let hex=|b:&[u8]|format!("0x{}",b.iter().map(|x|format!("{x:02x}")).collect::<String>());let word=|v:u8|{let mut w=[0u8;32];w[31]=v;hex(&w)};
    let at=|n:&mut Node,m:&str,p:serde_json::Value|rpc_local(n,m,p);
    let s=|a:Address|format!("{a:#x}");
    // Positive controls: the layer wrote the head's contract (its slot) and holds the pool's creation.
    assert_eq!(at(&mut n,"eth_getStorageAt",json!([s(head_c),"0x0","pending"])),json!(word(1)),"the pool's call wrote the head's contract");
    assert_eq!(at(&mut n,"eth_getStorageAt",json!([s(head_c),"0x0","latest"])),json!(word(0)));
    assert!(n.pool_layer_entries().0>=3,"entries for alice and both contracts");
    assert_eq!(n.pool_view().layer.code_copies(),0,"no entry keeps a copy of its code");
    for c in [head_c,pool_c]{assert_eq!(at(&mut n,"eth_getCode",json!([s(c),"pending"])),json!(hex(&runtime)),"pending code of {c}");}
    // Pool transactions run both contracts from entries that hold their code hash only.
    n.submit_tx(tx(3,Some(head_c),vec![]),false).unwrap();n.submit_tx(tx(4,Some(pool_c),vec![]),false).unwrap();
    for (c,v) in [(head_c,2),(pool_c,1)]{assert_eq!(at(&mut n,"eth_getStorageAt",json!([s(c),"0x0","pending"])),json!(word(v)),"the pool ran the code of {c}");}
    assert_eq!(n.pool_view().layer.code_copies(),0);
    assert_eq!(at(&mut n,"eth_getCode",json!([s(head_c),"latest"])),json!(hex(&runtime)));
    assert_eq!(at(&mut n,"eth_getCode",json!([s(pool_c),"latest"])),json!("0x"),"the pool's creation is pending only");
    let _=std::fs::remove_dir_all(&dir);
}

// ---------------- review finding (A05): range sync continues after a truncated response ----------------
#[test] fn range_sync_reaches_the_peer_head_through_truncated_responses(){
    use pocold::node::{Config,Limits,Node};
    use std::io::{Read,Write};
    let port=||std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let c=cfg(&["a","b","c"]);let mut n=net();let mut blocks=vec![];
    for _ in 0..120{let h=n.chain.head;let b=fallback(&n,h,0);import(&mut n,&b);blocks.push(b);}
    // A small peer queue: every range response stops after about 16 KiB (half the queue).
    let mut l=Limits::default();l.net.queue_bytes_max=32*1024;
    let (pa,pb,rb)=(port(),port(),port());let tag=std::process::id();
    let dir=|x:&str|{let d=std::env::temp_dir().join(format!("pocold-sync-{x}-{tag}"));let _=std::fs::remove_dir_all(&d);d};
    let mut a=Node::open(Config{dir:dir("a"),name:"a".into(),miner:None,listen:Some(format!("127.0.0.1:{pa}").parse().unwrap()),peers:vec![],rpc:None,stop_at:None,log:false,limits:l.clone()},admitted(&c)).unwrap();
    for b in blocks{a.import(b,None).unwrap();}assert_eq!(a.chain.head().height,120);
    a.start_network().unwrap();std::thread::spawn(move||a.run());
    let mut b=Node::open(Config{dir:dir("b"),name:"b".into(),miner:None,listen:Some(format!("127.0.0.1:{pb}").parse().unwrap()),peers:vec![format!("127.0.0.1:{pa}").parse().unwrap()],
        rpc:Some(format!("127.0.0.1:{rb}").parse().unwrap()),stop_at:None,log:false,limits:l},admitted(&c)).unwrap();
    b.start_network().unwrap();std::thread::spawn(move||b.run());
    let height=||->u64{let Ok(mut s)=std::net::TcpStream::connect(("127.0.0.1",rb)) else{return 0};let body=r#"{"jsonrpc":"2.0","id":1,"method":"eth_blockNumber","params":[]}"#;
        let _=write!(s,"POST / HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\n\r\n{body}",body.len());let mut o=String::new();let _=s.read_to_string(&mut o);
        o.split("\"result\":\"0x").nth(1).and_then(|x|u64::from_str_radix(x.split('"').next()?,16).ok()).unwrap_or(0)};
    let t=std::time::Instant::now();while height()<120&&t.elapsed()<std::time::Duration::from_secs(60){std::thread::sleep(std::time::Duration::from_millis(200));}
    assert_eq!(height(),120,"B syncs all 120 blocks from A (one Hello, truncated range responses)");
}

// ---------------- review finding: BODY_MAX bounds the encoded body on every path ----------------
#[test] fn body_max_bounds_the_encoded_body_not_the_item_sum(){
    let mut n=net();for _ in 0..2{let h=n.chain.head;let b=fallback(&n,h,0);import(&mut n,&b);}
    let head=n.chain.head;let p=n.chain.full_entry(&head).unwrap();let ctx=n.chain.child_ctx(&p,0).unwrap();
    let max=n.chain.spec.cp().BODY_MAX as usize;let k=max/100;
    // k items of 100 bytes: the raw sum fits BODY_MAX, the encoding (2-byte item headers) does not.
    let body=Body{txs:vec![vec![0x02;100];k],evidence:vec![]};assert!(k*100<=max&&body.encode().len()>max);
    let good=signed(&n,head,Body::default(),0);
    let mut h=good.header.clone();h.tx_root=pocold::types::ordered_root(&body.txs).unwrap();reseal(&mut h);
    let mut b=remine_signed(&n,h,ctx.proposer);b.body=body.clone();
    assert!(matches!(n.chain.validate(&b,None,0),Err(Reject::Block(s)) if s.contains("BODY_MAX")),"the locally checked limit is the encoded one");
    assert!(Body::decode(&body.encode(),max).is_err(),"peers reject the same body at decode");
}

// ---------------- second review: eth_getLogs limits answer -32020 with resume data (network.md:298) ----------------
#[test] fn get_logs_limits_answer_32020_with_resume_data(){
    use pocold::node::{Config,Node};
    let mut n=net();let alice=devnet::account_key("alice");
    // Init code: 546 LOG0 of zero bytes (the per-block log count limit), then STOP.
    let mut init=vec![0x61,0x02,0x22,0x5b];// PUSH2 546; JUMPDEST(3)
    init.extend([0x5f,0x5f,0xa0,0x60,0x01,0x90,0x03,0x80,0x60,0x03,0x57,0x00]);// PUSH0 PUSH0 LOG0; PUSH1 1 SWAP1 SUB; DUP1 PUSH1 3 JUMPI; STOP
    let mut first=0;
    for k in 0..19u64{let head=n.chain.head;
        let tx=Tx{chain_id:777910,nonce:k,max_priority:U256::from(1u64),max_fee:U256::from(100_000_000_000u64),gas:2_000_000,to:None,value:U256::ZERO,data:init.clone().into()}.sign_1559(&alice);
        let b=signed(&n,head,Body{txs:vec![tx],evidence:vec![]},0);import(&mut n,&b);if k==0{first=n.chain.head().height;}}
    let dir=std::env::temp_dir().join(format!("pocold-logs-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    let mut node=Node::open(Config{dir:dir.clone(),name:"logs".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Default::default()},admitted(&cfg(&["a","b","c"]))).unwrap();
    let mut hs:Vec<_>=n.chain.blocks.values().filter(|e|e.height>0).map(|e|(e.height,e.hash)).collect();hs.sort();
    for (_,h) in hs{let e=n.chain.blocks[&h].clone();node.import(Block{header:e.header,body:e.body},None).unwrap();}
    let q=format!(r#"{{"jsonrpc":"2.0","id":5,"method":"eth_getLogs","params":[{{"fromBlock":"0x{:x}","toBlock":"latest"}}]}}"#,first);
    let v:serde_json::Value=serde_json::from_str(&pocold::rpc::handle(&mut node,q.as_bytes())).unwrap();
    let e=&v["error"];assert_eq!(e["code"],-32020,"{v}");assert_eq!(e["data"]["reason"],"resultCount");assert_eq!(v["id"],5);
    // 18 complete blocks hold 9828 logs; the 19th would pass LOGS_COUNT_MAX = 10000.
    let x=first+18;assert_eq!(e["data"]["nextFromBlock"],format!("0x{x:x}"));assert_eq!(e["data"]["lastCompleteBlock"],format!("0x{:x}",x-1));assert_eq!(e["data"]["fromBlock"],format!("0x{first:x}"));
    // Resuming from nextFromBlock succeeds.
    let q=format!(r#"{{"jsonrpc":"2.0","id":6,"method":"eth_getLogs","params":[{{"fromBlock":"0x{x:x}","toBlock":"latest"}}]}}"#);
    let v:serde_json::Value=serde_json::from_str(&pocold::rpc::handle(&mut node,q.as_bytes())).unwrap();assert_eq!(v["result"].as_array().map(|a|a.len()),Some(546),"{}",&v.to_string()[..200.min(v.to_string().len())]);
    let _=std::fs::remove_dir_all(&dir);
}
/// RpcGuard SCRATCH (network.md:220, (G)): eth_getLogs charges the working bytes it materializes
/// against the heavy SCRATCH budget; an overflow is -32020 {scratch} with the same resume data as a
/// per-block limit, and the partial result is dropped. A heavy request is run through `answer`, which
/// derives the budget from the node's running SCRATCH_HEAVY, so a small limit reaches the bound in
/// the first block (546 logs ≈ 130 KiB). With the default 16 MiB budget the same block succeeds.
#[test] fn get_logs_scratch_limit_is_32020_scratch_with_resume_data(){
    use pocold::node::{Config,Limits,Node};use pocold::rpc::{self,Request};use std::time::{Duration,Instant};
    let mut n=net();let alice=devnet::account_key("alice");
    let mut init=vec![0x61,0x02,0x22,0x5b];init.extend([0x5f,0x5f,0xa0,0x60,0x01,0x90,0x03,0x80,0x60,0x03,0x57,0x00]);// 546 LOG0 then STOP
    let mut first=0;
    for k in 0..3u64{let head=n.chain.head;
        let tx=Tx{chain_id:777910,nonce:k,max_priority:U256::from(1u64),max_fee:U256::from(100_000_000_000u64),gas:2_000_000,to:None,value:U256::ZERO,data:init.clone().into()}.sign_1559(&alice);
        let b=signed(&n,head,Body{txs:vec![tx],evidence:vec![]},0);import(&mut n,&b);if k==0{first=n.chain.head().height;}}
    let hs:Vec<_>={let mut v:Vec<_>=n.chain.blocks.values().filter(|e|e.height>0).map(|e|(e.height,e.hash)).collect();v.sort();v};
    let getlogs=|node:&mut Node|->serde_json::Value{
        let q=format!(r#"{{"jsonrpc":"2.0","id":7,"method":"eth_getLogs","params":[{{"fromBlock":"0x{first:x}","toBlock":"0x{first:x}"}}]}}"#);
        let (t,_r)=std::sync::mpsc::channel();
        let a=rpc::answer(node,&Request::local(q.into_bytes(),t,Instant::now()+Duration::from_secs(30),false));
        serde_json::from_str::<serde_json::Value>(&a.body).unwrap()
    };
    // Small SCRATCH_HEAVY: the first block's logs exceed it -> -32020 {scratch}, no result.
    let dir=std::env::temp_dir().join(format!("pocold-scratch-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    let tight=Limits{rpc:rpc::Limits{scratch_heavy:60_000,..Default::default()},..Default::default()};
    let mut node=Node::open(Config{dir:dir.clone(),name:"scratch".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:tight},admitted(&cfg(&["a","b","c"]))).unwrap();
    for (_,h) in &hs{let e=n.chain.blocks[h].clone();node.import(Block{header:e.header,body:e.body},None).unwrap();}
    let v=getlogs(&mut node);let e=&v["error"];
    assert_eq!(e["code"],-32020,"{v}");assert_eq!(e["data"]["reason"],"scratch");
    assert_eq!(e["data"]["fromBlock"],format!("0x{first:x}"));assert_eq!(e["data"]["nextFromBlock"],format!("0x{first:x}"));assert!(e["data"]["lastCompleteBlock"].is_null());assert!(v.get("result").is_none());
    // Default SCRATCH_HEAVY (16 MiB): the same block succeeds with all 546 logs.
    let dir2=std::env::temp_dir().join(format!("pocold-scratch2-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir2);
    let mut node2=Node::open(Config{dir:dir2.clone(),name:"scratch2".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Default::default()},admitted(&cfg(&["a","b","c"]))).unwrap();
    for (_,h) in &hs{let e=n.chain.blocks[h].clone();node2.import(Block{header:e.header,body:e.body},None).unwrap();}
    assert_eq!(getlogs(&mut node2)["result"].as_array().map(|a|a.len()),Some(546),"the default budget answers the block");
    // Review M3/V5: past its COMPUTE_DEADLINE (here already at entry) each log handler answers -32020
    // {deadline} with the resume data of no progress (network.md:300), and its scan stops at the first
    // block, before it materializes any log (the handler's charged working set stays 0).
    let fh=format!("0x{first:x}");let bh=pocold::types::hex(&hs.iter().find(|x|x.0==first).unwrap().1);
    for q in [format!(r#"{{"jsonrpc":"2.0","id":1,"method":"eth_getLogs","params":[{{"fromBlock":"{fh}","toBlock":"{fh}"}}]}}"#),
        format!(r#"{{"jsonrpc":"2.0","id":2,"method":"pocol_getLogs","params":[{{"fromBlock":"{fh}","toBlock":"{fh}"}}]}}"#),
        format!(r#"{{"jsonrpc":"2.0","id":3,"method":"pocol_getLogs","params":[{{"blockHash":"{bh}"}}]}}"#)]{
        let mut b=rpc::Budget{compute_by:Instant::now(),scratch_max:usize::MAX,scratch_used:0,walled:false};
        let v:serde_json::Value=serde_json::from_str(&rpc::handle_until_budgeted(&mut node2,q.as_bytes(),false,None,None,&mut b)).unwrap();let e=&v["error"];
        assert_eq!(e["code"],-32020,"{v}");assert_eq!(e["data"]["reason"],"deadline");assert_eq!(e["data"]["fromBlock"],fh);assert_eq!(e["data"]["nextFromBlock"],fh);assert!(e["data"]["lastCompleteBlock"].is_null());
        assert_eq!(b.scratch_used,0,"the scan stopped at its first block: {q}");
    }
    let _=std::fs::remove_dir_all(&dir);let _=std::fs::remove_dir_all(&dir2);
}

// ---------------- proposal vectors (development/audit-2026-10-22/proposals): D-BODY-MAX ----------------
/// The independent model's encoded lengths equal pocold's Body encoding, and the decode path
/// (BODY_MAX on the encoded body, the recommended alternative) accepts exactly the bodies the
/// model marks valid. Agreement is not an approval of the proposal.
#[test] fn d_body_max_vectors(){
    let v:serde_json::Value=serde_json::from_str(include_str!("../../../development/audit-2026-10-22/proposals/vectors/d-body-max.json")).unwrap();
    let rec=v["recommended"].as_str().unwrap();let max=v["constants"]["BODY_MAX"].as_u64().unwrap() as usize;
    for s in v["scenarios"].as_array().unwrap(){
        let items=|k:&str|s[k].as_array().unwrap().iter().map(|n|vec![0xabu8;n.as_u64().unwrap() as usize]).collect::<Vec<_>>();
        let body=pocold::types::Body{txs:items("txSizes"),evidence:items("evidenceSizes")};let enc=body.encode();
        assert_eq!(enc.len() as u64,s["encodedLen"].as_u64().unwrap(),"{}",s["id"]);
        assert_eq!(pocold::types::Body::decode(&enc,max).is_ok(),s["expected"][rec]["valid"]==true,"{}",s["id"]);
    }
}

// ---------------- A13: the itemized model bound of the default devnet ----------------
#[test] fn a13_bound_breakdown(){
    use pocold::node::Limits;
    // The values pocold boots with: its node-local parameters, default limits, two configured peers.
    let l=pocold::single_host_local();
    let d=pocold::resources::derive(admitted(&cfg(&["a","b","c"])).cp(),l.k_hist,l.side_max,&Limits::default(),2).unwrap();
    assert_eq!(d.items.iter().map(|x|x.1).sum::<u128>(),d.total);
    // RpcGuard review 2 (m2): the total is pinned, so a term cannot change unnoticed (round 3: the
    // small-answer term counts SMALL_REPLY_PEAK per connection, +4,082,688 B; C-1 S1: canonical history
    // below the resident window is off the heap, 5,487,223,178 -> 4,323,240,949 B; C-1 S3a: retained
    // canonical states as diffs, -> 3,601,500,638 B; C-1 S4: old canonical entries slimmed,
    // -> 3,322,680,105 B; C-1 E1: the head's state trie, one derived or built trie and a block's changed
    // paths, and the entry's trie handle, -> 3,486,909,657 B; C-1 E2: each entry's boxed state summary,
    // -> 3,489,051,177 B; C-1 E3: blocks, production, the pool and RPC on overlays, 10 transient working
    // copies -> 2, plus the pool's layer and the applied block's overlay, -> 3,186,963,844 B; C-1 E4.3: each entry's handle to a
    // stored version, 8 B, -> 3,187,591,348 B; C-1 E5: entries without a state share one empty state,
    // -> 3,154,465,633 B; D-7: the pool's layer from its gas bound in place of a whole state, its code from its
    // creations, the block overlay with the slots a created account keeps, one abandoned body in revalidation,
    // -> 3,287,955,262 B; 2026-10-27: the chain's entries boxed in their map, each counted as its slot at the
    // table's growth peak and the Entry once, -> 3,233,597,728 B; 2026-10-28: each side entry's slot in the chain's
    // side set, 32 + 16 B, -> 3,233,892,640 B; the independent review's F-C, the set as its own term: (side entries
    // + r_keep) × 76 B, -> 3,234,220,320 B). Some terms are type
    // sizes (Entry, Block), so the exact value is pinned for the target the evidence is produced on; every
    // target checks that it boots within the budget with its reserve.
    if cfg!(all(target_os="linux",target_arch="x86_64")){assert_eq!(d.total,3_241_940_480,"the A13 model total changed");}
    // R6 churn correction: eviction=3982768, protect delta=1967104,
    // side-set delta=1868592. Total increase from R5 = 7720160 bytes.
    assert_eq!(d.items.iter().find(|x|x.0.starts_with("side/evicted hash")).unwrap().1,3_982_768);
    // C-1 S1: a window of the whole chain is the model before S1, plus history.redb's cache and allocator.
    let all=pocold::resources::derive(admitted(&cfg(&["a","b","c"])).cp(),l.k_hist,l.side_max,&Limits{history_window:Some(u64::MAX),..Default::default()},2).unwrap();
    // (C-1 E5: and the one empty state entries share, which each entry counted before.)
    let redb:u128=all.items.iter().filter(|x|x.0.starts_with("history/history.redb")||x.0.starts_with("history/the one empty state")).map(|x|x.1).sum();
    // The one term that differs: the logs, now also bounded per block (LOG_BLOCK_MAX, enforced by the USER
    // phase), which is tighter than LOG_CAP for H_END+1 devnet blocks.
    let logs=all.items.iter().find(|x|x.0=="history/resident window: logs").unwrap().1;let pre_logs=u128::from(pocol_lp4_execution::user_phase::caps::LOG_CAP)/32*33;
    assert!(logs<pre_logs);
    // The pre-S1 history group, by its formula: (H_END+1) entries, TXBYTES_CAP, TX_CAP receipts and index
    // (380 B each), LOG_CAP·33/32 and the evidence identities (EVID_BYTES_MAX+24 each).
    let cp=admitted(&cfg(&["a","b","c"])).cp().clone();let entries=all.items.iter().find(|x|x.0=="history/resident window: block entries").unwrap().1;
    let identities=u128::from(cp.M_max)+u128::from(cp.R_max)*u128::from(cp.H_END);
    let pre_s1=entries+u128::from(cp.TXBYTES_CAP)+u128::from(cp.TX_CAP)*380+pre_logs+identities*(u128::from(cp.EVID_BYTES_MAX)+24);
    assert_eq!(entries%(u128::from(cp.H_END)+1),0);assert_eq!(all.history-redb+(pre_logs-logs),pre_s1,"the whole-chain window reproduces the pre-S1 history group");
    assert!(all.items.iter().any(|x|x.0.starts_with("history/offloaded")&&x.1==0));
    assert!(pocold::resources::budget_required(d.total).unwrap()<=u128::from(pocold::node::MEM_BUDGET));
    assert_eq!(d.items.iter().find(|x|x.0.starts_with("rpc/small answers")).map(|x|x.1),Some(32*pocold::rpc::SMALL_REPLY_PEAK as u128));
    println!("A13-BREAKDOWN {}",serde_json::json!({"total":d.total.to_string(),"items":d.items.iter().map(|(k,v)|serde_json::json!([k,v.to_string()])).collect::<Vec<_>>()}));
}

// ---------------- second round-2 review: side-branch retention, eviction and replay ----------------
fn retention_node(tag:&str)->(pocold::node::Node,std::path::PathBuf,impl Fn()->pocold::node::Config){
    use pocold::node::{Config,Node};
    let dir=std::env::temp_dir().join(format!("pocold-retain-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    let d=dir.clone();let conf=move||Config{dir:d.clone(),name:"retain".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Default::default()};
    (Node::open(conf(),admitted(&cfg(&["a","b","c"]))).unwrap(),dir,conf)
}
/// `n` blocks from `parent`: the first by `first`, then fallbacks at attempt 0.
fn grow(h:&mut Net,parent:[u8;32],n:usize,first:&dyn Fn(&Net,[u8;32])->Block)->Vec<Block>{
    let mut out=vec![];let mut p=parent;for i in 0..n{let b=if i==0{first(h,p)}else{fallback(h,p,0)};import(h,&b);h.chain.retain();p=b.hash();out.push(b);}out
}
/// A heavier competing branch whose fork is 150 blocks deep (more than the 64 a review showed
/// unadoptable) is adopted with the default limits: the node switches to its tip.
#[test] fn a_competing_branch_within_side_blocks_max_is_adopted(){
    let depth=150usize;let mut a=net();let g=a.chain.head;
    let ours=grow(&mut a,g,depth,&|n,p|signed(n,p,Body::default(),0));
    let mut b=net();let target=a.chain.head().total_work;let mut theirs=vec![];let mut p=g;
    while b.chain.head().total_work<=target{let x=fallback(&b,p,0);import(&mut b,&x);b.chain.retain();p=x.hash();theirs.push(x);}
    for _ in 0..2{let x=fallback(&b,p,0);import(&mut b,&x);p=x.hash();theirs.push(x);}
    let (mut n,dir,_)=retention_node("adopt");
    for x in &ours{n.import(x.clone(),None).unwrap();}
    for x in &theirs{n.import(x.clone(),None).unwrap_or_else(|e|panic!("h={}: {e:?}",x.header.h));assert!(n.chain.side_counts().0<=pocold::chain::SIDE_BLOCKS_MAX);}
    assert_eq!(n.chain.head,theirs.last().unwrap().hash(),"the heavier branch ({} blocks against {depth}) is adopted",theirs.len());
    let _=std::fs::remove_dir_all(&dir);
}
/// Second review: a valid, included transaction of TX_MAX bytes is fetched by hash in the light lane,
/// and its answer stays within network.md's 2·TX_MAX + 2 KiB (no `raw` beside `input`).
#[test] fn a_max_size_transaction_is_answered_in_the_light_lane(){
    let mut h=net();let g=h.chain.head;let txmax=h.chain.spec.cp().TX_MAX as usize;let alice=devnet::account_key("alice");
    let mk=|len:usize|Tx{chain_id:777910,nonce:0,max_priority:U256::from(1_000_000_000u64),max_fee:U256::from(100_000_000_000u64),gas:2_000_000,to:Some(devnet::account_key("bob").address()),value:U256::ZERO,data:vec![0u8;len].into()}.sign_1559(&alice);
    let mut len=txmax-200;while mk(len+1).len()<=txmax{len+=1;}
    let raw=mk(len);assert!(raw.len()<=txmax&&raw.len()+64>txmax);let hash=revm::primitives::keccak256(&raw);
    let b=signed(&h,g,Body{txs:vec![raw.clone()],evidence:vec![]},0);import(&mut h,&b);
    let (mut n,dir,_)=retention_node("txmax");n.import(b,None).unwrap();assert_eq!(n.chain.head().body.txs.len(),1);
    let body=format!(r#"{{"jsonrpc":"2.0","id":3,"method":"eth_getTransactionByHash","params":["{hash}"]}}"#);assert!(pocold::rpc::is_light(body.as_bytes()));
    let (t,_r)=std::sync::mpsc::channel();
    let a=pocold::rpc::answer(&mut n,&pocold::rpc::Request::local(body.into_bytes(),t,std::time::Instant::now()+std::time::Duration::from_secs(30),true));
    let v:serde_json::Value=serde_json::from_str(&a.body).unwrap();
    assert_eq!(v["result"]["hash"],format!("{hash}"),"answered, not -32020 {{resultBytes}}: {}",&a.body[..a.body.len().min(200)]);
    assert!(v["result"].get("raw").is_none());assert!(a.body.len()<=2*txmax+2048,"{} B",a.body.len());
    let _=std::fs::remove_dir_all(&dir);
}


// ---------------- D-REORG-REDB acceptance (owner-approved storage repair; D133 and validity unchanged) ----------------
// Retention (R_keep, the side cap) is local (consensus.md:29), so most tests lower it to reach the
// boundaries quickly; the spec-depth tests run the defaults in release (`--ignored`).
use serde_json::json;
fn rnode(tag:&str,r_keep:u64,side_cap:usize)->(pocold::node::Node,std::path::PathBuf,impl Fn()->pocold::node::Config){
    use pocold::node::{Config,Limits,Node};
    let dir=std::env::temp_dir().join(format!("pocold-redb-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    let (d,name)=(dir.clone(),tag.to_string());
    let conf=move||Config{dir:d.clone(),name:name.clone(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Limits{r_keep,side_cap,..Default::default()}};
    (Node::open(conf(),admitted(&cfg(&["a","b","c"]))).unwrap(),dir,conf)
}
fn reopen(conf:&dyn Fn()->pocold::node::Config)->pocold::node::Node{pocold::node::Node::open(conf(),admitted(&cfg(&["a","b","c"]))).expect("reopen")}
/// A fallback branch from `parent` whose cumulative work strictly exceeds `than`, then `extra` more blocks.
fn heavier(h:&mut Net,parent:[u8;32],than:U256,extra:usize)->Vec<Block>{
    let mut out=vec![];let mut p=parent;
    while h.chain.get(&p).unwrap().total_work<=than{let x=fallback(h,p,0);import(h,&x);h.chain.retain();p=x.hash();out.push(x);}
    for _ in 0..extra{let x=fallback(h,p,0);import(h,&x);h.chain.retain();p=x.hash();out.push(x);}
    out
}
/// Non-canonical blocks the side cap applies to (not protected by a session).
/// 2026-10-28: the chain keeps its side blocks as a set (`Chain::side`, read by `side_counts`); it must be what a
/// walk over every entry finds, in release builds too (debug builds also assert it inside retain).
fn side_matches(c:&Chain)->bool{let walk:Vec<&pocold::chain::Entry>=c.blocks.values().filter(|e|e.height>0&&!c.is_canonical(&e.hash)).map(|e|&**e).collect();
    c.side_counts()==(walk.len(),walk.iter().filter(|e|!e.pruned).count())}
fn unprotected(n:&pocold::node::Node)->usize{n.chain.blocks.keys().filter(|h|n.chain.blocks[*h].height>0&&!n.chain.is_canonical(h)&&!n.chain.protect.contains(*h)).count()}
fn rpc_local(n:&mut pocold::node::Node,method:&str,params:serde_json::Value)->serde_json::Value{
    let body=json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}).to_string();let (t,_r)=std::sync::mpsc::channel();
    let a=pocold::rpc::answer(n,&pocold::rpc::Request::local(body.into_bytes(),t,std::time::Instant::now()+std::time::Duration::from_secs(120),false));
    serde_json::from_str::<serde_json::Value>(&a.body).unwrap()["result"].clone()
}
/// Every canonical block (with full transactions) and every receipt, as the node answers them over RPC.
fn chain_view(n:&mut pocold::node::Node)->Vec<serde_json::Value>{
    let top=n.chain.head().height;let mut out=vec![];
    for h in 0..=top{let b=rpc_local(n,"eth_getBlockByNumber",json!([format!("{h:#x}"),true]));
        for t in b["transactions"].as_array().cloned().unwrap_or_default(){out.push(rpc_local(n,"eth_getTransactionReceipt",json!([t["hash"]])));}
        out.push(b);}
    out
}

/// network.md:144 (adoption at depth <= R_keep) and governance.md:200 (deeper: manual checkpoint).
/// Depth R_keep: the heavier branch is adopted, its offloaded blocks are restored, and RPC answers the
/// same chain after restart. Depth R_keep+1: NOT adopted and NOT invalid (no blockReject, no invalid
/// mark: retention is never a validity rule); the node stays on the valid lighter chain.
fn r_keep_boundary(rk:u64,side_cap:usize,tag:&str){
    let mut a=net();let g=a.chain.head;
    // Ours starts at attempt 1 (a later timestamp, so less work per block): theirs, delivered without a
    // session, overtakes within fewer blocks than the depth and fits the side cap unprotected.
    let ours=grow(&mut a,g,rk as usize+1,&|n,p|fallback(n,p,1));
    let mut b=net();let theirs=heavier(&mut b,g,a.chain.head().total_work,1);
    let (mut n,dir,conf)=rnode(&format!("{tag}-at"),rk,side_cap);
    for x in &ours[..rk as usize]{n.import(x.clone(),None).unwrap();}
    assert_eq!(n.chain.head().height,rk,"the fork point (genesis) is at depth R_keep");
    for x in &theirs{n.import(x.clone(),None).unwrap_or_else(|e|panic!("h={}: {e:?}",x.header.h));}
    let tip=theirs.last().unwrap().hash();
    assert_eq!(n.chain.head,tip,"a heavier branch forked at depth R_keep ({rk}) is adopted");
    assert!(n.stats.restored>0&&n.chain.canonical_resident(),"positive control: its blocks were offloaded as side blocks and restored on adoption");
    let view=chain_view(&mut n);drop(n);
    let mut r=reopen(&conf);assert_eq!(r.chain.head,tip,"restart replays to the adopted head");assert!(r.chain.canonical_resident());assert_eq!(r.stats.replay_skipped,0);
    assert_eq!(chain_view(&mut r),view,"RPC answers the same chain after restart");drop(r);
    let (mut n,dir2,conf2)=rnode(&format!("{tag}-beyond"),rk,side_cap);
    for x in &ours{n.import(x.clone(),None).unwrap();}
    let ours_tip=ours.last().unwrap().hash();let rec0=n.journal_records();
    for x in &theirs{if let Err(Reject::Block(w))=n.import(x.clone(),None){panic!("retention never yields a validity verdict: {w}")}}
    assert_eq!(n.chain.head,ours_tip,"depth R_keep+1 is not adopted (governance.md:200: manual checkpoint)");
    assert!(theirs.iter().all(|x|!n.chain.invalid.contains_key(&x.hash())),"and no block of it is marked invalid");
    assert!(matches!(n.import(theirs[0].clone(),None),Err(Reject::Evicted)),"a block at or below head-R_keep is refused by local retention");
    assert_eq!(n.journal_records(),rec0,"before validation or journaling (retention would remove it at once)");
    drop(n);let r=reopen(&conf2);assert_eq!(r.chain.head,ours_tip);assert_eq!(r.stats.replay_skipped,0);
    println!("R_KEEP-BOUNDARY {}",json!({"rKeep":rk,"ourBlocks":ours.len(),"theirBlocks":theirs.len(),"adoptedAtRKeep":true,"adoptedBeyond":false}));
    for d in [dir,dir2]{let _=std::fs::remove_dir_all(&d);}
}
#[test] fn r_keep_boundary_adopts_at_depth_r_keep_and_keeps_the_valid_chain_beyond(){r_keep_boundary(24,24,"rk")}
/// The spec depth with the default limits (R_keep = 2048, governance.md:15; side cap 2048). Release
/// only: about 4,000 blocks are mined and validated twice. Run with
/// `cargo test --release --test consensus -- --ignored adoption_at_the_spec_depth`.
#[test] #[ignore="spec-depth test: run in release with --ignored"]
fn adoption_at_the_spec_depth_2048_and_not_at_2049(){r_keep_boundary(pocold::chain::R_KEEP,pocold::chain::SIDE_BLOCKS_MAX,"spec")}

/// C-1 E4.3: the same boundary on a store-mode node beside a whole-state node: at depth R_keep the
/// branch is adopted, at R_keep + 1 it is not, by both; before the reorg, after it and after reopening
/// they answer the same blocks, receipts and state queries at every height, keep exactly the same states,
/// and the store node's pins are exactly its stored versions (a released state leaks nothing).
fn r_keep_boundary_store(rk:u64,side_cap:usize,tag:&str){
    use pocold::node::{Config,Limits,Node};
    let mk=|t:&str,store:bool|{let dir=std::env::temp_dir().join(format!("pocold-e4b-{t}-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
        let (d,name)=(dir.clone(),t.to_string());
        let conf=move||Config{dir:d.clone(),name:name.clone(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Limits{r_keep:rk,side_cap,state_store:store,..Default::default()}};
        (Node::open(conf(),admitted(&cfg(&["a","b","c"]))).unwrap(),dir,conf)};
    let mut a=net();let g=a.chain.head;
    let ours=grow(&mut a,g,rk as usize+1,&|n,p|fallback(n,p,1));
    let mut b=net();let theirs=heavier(&mut b,g,a.chain.head().total_work,1);
    let ids:Vec<Address>=devnet::build(&cfg(&["a","b","c"])).unwrap().miners.iter().map(|m|m.id).collect();
    let accounts:Vec<String>=ids.iter().chain([devnet::account_key("alice").address(),apply::escrow(),apply::registry()].iter()).map(|a|format!("{a:#x}")).collect();
    let states=|n:&mut Node|->Vec<serde_json::Value>{let top=n.chain.head().height;let mut v=vec![];
        for t in (0..=top).map(|x|json!(format!("{x:#x}"))).chain(["latest","pending"].map(|t|json!(t))){for a in &accounts{
            v.push(rpc_local(n,"eth_getBalance",json!([a,t])));v.push(rpc_local(n,"eth_getStorageAt",json!([a,"0x0",t])));}}
        v};
    let check=|x:&mut Node,y:&mut Node,what:&str|{
        assert_eq!((x.chain.head,x.chain.head().header.state_root),(y.chain.head,y.chain.head().header.state_root),"{what}: head");
        assert_eq!(chain_view(x),chain_view(y),"{what}: blocks and receipts");
        let (p,q)=(states(x),states(y));assert_eq!(p,q,"{what}: state answers");let answered=p.iter().filter(|r|!r.is_null()).count();assert!(answered>=4*accounts.len(),"positive control ({what}): {answered} answers");
        let mut s1:Vec<[u8;32]>=x.chain.blocks.values().filter(|e|e.stored.is_some()).map(|e|e.hash).collect();
        let mut s2:Vec<[u8;32]>=y.chain.blocks.values().filter(|e|y.chain.view(e).is_some()).map(|e|e.hash).collect();s1.sort();s2.sort();
        assert_eq!(s1,s2,"{what}: the retained states");
        assert!(x.chain.blocks.values().all(|e|e.state.accounts.is_empty()),"{what}: the store node holds no whole state");
        assert_eq!(x.chain.trie_store.as_ref().unwrap().pinned().unwrap() as usize,s1.len(),"{what}: pins are the stored versions");};
    let (mut x,dx,cx)=mk(&format!("{tag}-s"),true);let (mut y,dy,cy)=mk(&format!("{tag}-m"),false);
    for blk in &ours[..rk as usize]{x.import(blk.clone(),None).unwrap();y.import(blk.clone(),None).unwrap();}
    check(&mut x,&mut y,"before the reorg");
    for blk in &theirs{x.import(blk.clone(),None).unwrap();y.import(blk.clone(),None).unwrap();}
    assert_eq!(x.chain.head,theirs.last().unwrap().hash(),"adopted at depth R_keep");
    check(&mut x,&mut y,"after the reorg");
    drop(x);drop(y);let (mut x,mut y)=(reopen(&cx),reopen(&cy));check(&mut x,&mut y,"after reopen");drop(x);drop(y);
    let (mut x,dx2,_)=mk(&format!("{tag}-s2"),true);let (mut y,dy2,_)=mk(&format!("{tag}-m2"),false);
    for blk in &ours{x.import(blk.clone(),None).unwrap();y.import(blk.clone(),None).unwrap();}
    for blk in &theirs{let (r1,r2)=(x.import(blk.clone(),None).is_ok(),y.import(blk.clone(),None).is_ok());assert_eq!(r1,r2,"the same verdict at h={}",blk.header.h);}
    assert_eq!(x.chain.head,ours.last().unwrap().hash(),"not adopted at R_keep + 1");
    check(&mut x,&mut y,"beyond R_keep");
    for d in [dx,dy,dx2,dy2]{let _=std::fs::remove_dir_all(&d);}
}
#[test] fn e4_store_node_at_the_r_keep_boundary_answers_as_a_whole_state_node(){r_keep_boundary_store(24,24,"rk")}
/// C-1 E4.3: a fork deeper than K_hist on a store-mode node beside a whole-state node: the fork point's
/// state was released, so the first block of the branch makes the node rebuild it (TrieRebuild from the
/// nearest stored ancestor, here genesis) and the adoption restores the branch. Before, after and after
/// reopening both answer the same and keep the same states, and the store node's pins are its versions.
#[test] fn e4_store_node_rebuilds_a_fork_below_k_hist_as_a_whole_state_node(){
    use pocold::node::{Config,Limits,Node};
    let mk=|t:&str,store:bool|{let dir=std::env::temp_dir().join(format!("pocold-e4k-{t}-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
        let (d,name)=(dir.clone(),t.to_string());
        let conf=move||Config{dir:d.clone(),name:name.clone(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Limits{state_store:store,..Default::default()}};
        (Node::open(conf(),admitted(&cfg(&["a","b","c"]))).unwrap(),dir,conf)};
    let mut a=net();let g=a.chain.head;let alice=devnet::account_key("alice");
    let common=grow(&mut a,g,5,&|n,p|signed(n,p,Body{txs:vec![pay(&alice,0,9)],evidence:vec![]},0));let fork=common.last().unwrap().hash();
    let ours=grow(&mut a,fork,40,&|n,p|fallback(n,p,1));
    let mut b=net();for x in &common{import(&mut b,x);b.chain.retain();}
    let theirs=heavier(&mut b,fork,a.chain.head().total_work,1);
    let ids:Vec<Address>=devnet::build(&cfg(&["a","b","c"])).unwrap().miners.iter().map(|m|m.id).collect();
    let accounts:Vec<String>=ids.iter().chain([alice.address(),devnet::account_key("bob").address(),apply::escrow(),apply::registry()].iter()).map(|a|format!("{a:#x}")).collect();
    let states=|n:&mut Node|->Vec<serde_json::Value>{let top=n.chain.head().height;let mut v=vec![];
        for t in (0..=top).map(|x|json!(format!("{x:#x}"))).chain(["latest","pending"].map(|t|json!(t))){for a in &accounts{
            v.push(rpc_local(n,"eth_getBalance",json!([a,t])));v.push(rpc_local(n,"eth_getTransactionCount",json!([a,t])));v.push(rpc_local(n,"eth_getStorageAt",json!([a,"0x0",t])));}}
        v};
    let check=|x:&mut Node,y:&mut Node,what:&str|{
        assert_eq!((x.chain.head,x.chain.head().header.state_root),(y.chain.head,y.chain.head().header.state_root),"{what}: head");
        assert_eq!(chain_view(x),chain_view(y),"{what}: blocks and receipts");
        let (p,q)=(states(x),states(y));assert_eq!(p,q,"{what}: state answers");let answered=p.iter().filter(|r|!r.is_null()).count();assert!(answered>=4*accounts.len(),"positive control ({what}): {answered} answers");
        let mut s1:Vec<[u8;32]>=x.chain.blocks.values().filter(|e|e.stored.is_some()).map(|e|e.hash).collect();
        let mut s2:Vec<[u8;32]>=y.chain.blocks.values().filter(|e|y.chain.view(e).is_some()).map(|e|e.hash).collect();s1.sort();s2.sort();
        assert_eq!(s1,s2,"{what}: the retained states");
        assert!(x.chain.blocks.values().all(|e|e.state.accounts.is_empty()),"{what}: the store node holds no whole state");
        assert_eq!(x.chain.trie_store.as_ref().unwrap().pinned().unwrap() as usize,s1.len(),"{what}: pins are the stored versions");};
    let (mut x,dx,cx)=mk("s",true);let (mut y,dy,cy)=mk("m",false);
    for blk in common.iter().chain(&ours){x.import(blk.clone(),None).unwrap();y.import(blk.clone(),None).unwrap();}
    assert!(x.chain.get(&fork).unwrap().stored.is_none(),"positive control: the fork point's state was released");
    check(&mut x,&mut y,"before the fork");
    for blk in &theirs{x.import(blk.clone(),None).unwrap_or_else(|e|panic!("store node h={}: {e:?}",blk.header.h));y.import(blk.clone(),None).unwrap();}
    assert_eq!(x.chain.head,theirs.last().unwrap().hash(),"the heavier branch is adopted");
    assert!(x.chain.rebuilds>0&&y.chain.rebuilds>0,"positive control: a TrieRebuild ran ({} / {})",x.chain.rebuilds,y.chain.rebuilds);
    check(&mut x,&mut y,"after the deep fork");
    drop(x);drop(y);let (mut x,mut y)=(reopen(&cx),reopen(&cy));check(&mut x,&mut y,"after reopen");
    drop(x);drop(y);for d in [dx,dy]{let _=std::fs::remove_dir_all(&d);}
}
/// The spec depth (R_keep = 2048, side cap 2048) in store mode. Release only, about 4,000 blocks on two
/// nodes: `cargo test --release --test consensus -- --ignored e4_store_node_at_the_spec_depth`.
#[test] #[ignore="spec-depth test: run in release with --ignored"]
fn e4_store_node_at_the_spec_depth_2048_and_2049_answers_as_a_whole_state_node(){r_keep_boundary_store(pocold::chain::R_KEEP,pocold::chain::SIDE_BLOCKS_MAX,"spec")}

/// Lowest-work unprotected leaves leave first and the cap holds after a deep reorg. An evicted block
/// received unsolicited is refused before validation or journaling (churn guard); a sync session
/// (solicited) takes it again (consensus.md §10: a branch that becomes heavier is fetched by sync).
/// Replay reproduces the live head.
#[test] fn side_cap_holds_after_a_reorg_and_only_a_session_takes_an_evicted_block_again(){
    let (rk,cap)=(64u64,16usize);let depth=40usize;let mut a=net();let g=a.chain.head;
    // Ours starts at attempt 1 (a later timestamp, less work per block); theirs is heavier sooner.
    let ours=grow(&mut a,g,depth,&|n,p|fallback(n,p,1));
    let mut b=net();let theirs=heavier(&mut b,g,a.chain.head().total_work,0);
    let (mut n,dir,conf)=rnode("cap",rk,cap);
    for x in &ours{n.import(x.clone(),None).unwrap();}
    assert!(side_matches(&n.chain));
    for x in &theirs{let _=n.import(x.clone(),None);assert!(unprotected(&n)<=cap,"{} unprotected side blocks after h={}",unprotected(&n),x.header.h);assert!(side_matches(&n.chain),"side set after h={}",x.header.h);}
    let head=theirs.last().unwrap().hash();assert_eq!(n.chain.head,head);
    assert!(n.chain.evicted_total>=(depth-cap) as u64,"evicted {}",n.chain.evicted_total);
    let tip=ours.last().unwrap();assert!(!n.chain.blocks.contains_key(&tip.hash()),"the lowest-work leaf went first");
    let records=n.journal_records();
    assert!(matches!(n.import(tip.clone(),None),Err(Reject::Evicted)),"an evicted block received unsolicited is refused");
    assert_eq!(n.journal_records(),records,"and not journaled again");
    for x in &ours{let _=n.import_with(x.clone(),None,true);}
    assert!(n.stats.refetched>0&&n.chain.blocks.contains_key(&tip.hash()),"a session takes evicted blocks again (refetched {})",n.stats.refetched);assert!(side_matches(&n.chain));
    assert_eq!(n.chain.head,head,"the lighter branch stays a side branch");assert_eq!(n.chain.head,best_of(&n.chain));
    n.chain.protect.clear();n.chain.retain();assert!(unprotected(&n)<=cap);assert!(side_matches(&n.chain)&&n.chain.side_counts().0>0,"positive control: side blocks are left");
    drop(n);let r=reopen(&conf);assert_eq!(r.chain.head,head,"replay reproduces the live head");assert!(side_matches(&r.chain));assert_eq!(r.stats.replay_skipped,0);assert!(r.chain.canonical_resident());
    let _=std::fs::remove_dir_all(&dir);
}

/// The multi-branch case a metadata cap alone does not cover: while a sync session delivers a heavier
/// branch forked deep, competing side branches flood past the cap. Only unprotected leaves are evicted,
/// so the session branch D133 prefers is adopted; on restart replay protects every block a later record
/// builds on, so it reproduces the same head with no record skipped.
#[test] fn a_session_branch_is_adopted_through_a_flood_of_competing_side_branches(){
    // R_keep 16: protect_max 32. The flood is siblings of the head (equal work to the head, losing its
    // tie by importSeq), so each is heavier than any not-yet-adopted session block and the session tip
    // is always the lowest-work leaf. Flood plus session branch exceed cap + protect_max = 36, the
    // bound replay applies, so replay must evict, and only its child-record protection spares the tip.
    let (rk,cap)=(16u64,4usize);let mut a=net();let g=a.chain.head;
    let ours=grow(&mut a,g,12,&|n,p|fallback(n,p,1));let parent=ours[ours.len()-2].hash();
    let mut noise:Vec<Block>=vec![];for att in 1..44u32{let y=fallback(&a,parent,att);if !noise.iter().any(|z|z.hash()==y.hash()){noise.push(y);}}
    let mut b=net();let theirs=heavier(&mut b,g,a.chain.head().total_work,1);
    let (mut n,dir,conf)=rnode("flood",rk,cap);
    for x in &ours{n.import(x.clone(),None).unwrap();}
    let (mut noise_in,per)=(0,noise.len().div_ceil(theirs.len()-1));
    for (k,x) in theirs.iter().enumerate(){
        n.import_with(x.clone(),None,true).unwrap_or_else(|e|panic!("session block h={}: {e:?}",x.header.h));
        for y in noise.iter().skip(per*k).take(per){if n.import(y.clone(),None).is_ok(){noise_in+=1;}}
        assert!(unprotected(&n)<=cap);assert!(n.chain.protect.len()<=n.chain.protect_max());
    }
    assert!(noise_in+theirs.len()>cap+n.chain.protect_max()&&n.chain.evicted_total>0,"positive control: the flood ({noise_in} side blocks) exceeded cap + protect_max");
    let tip=theirs.last().unwrap().hash();assert_eq!(n.chain.head,tip,"the heavier session branch is adopted");assert_eq!(n.chain.head,best_of(&n.chain));
    assert!(n.chain.canonical_resident());
    n.chain.protect.clear();n.chain.retain();let view=chain_view(&mut n);drop(n);
    let mut r=reopen(&conf);assert_eq!(r.stats.replay_skipped,0,"every journal record's parent was retained on replay");
    assert_eq!(r.chain.head,tip);assert_eq!(chain_view(&mut r),view);
    let _=std::fs::remove_dir_all(&dir);
}

/// consensus.md:205: a block removed by retention and imported again takes a NEW importSeq. A second
/// journal record of a stored block is such a re-import, so replay gives it a new importSeq, and the
/// equal-work tie at the fork point follows the re-import order (mutation control: replay that keeps
/// the old importSeq picks the other branch).
#[test] fn replay_gives_a_reimported_block_a_new_import_seq(){
    let alice=devnet::account_key("alice");let mut h=net();let g=h.chain.head;
    let y=signed(&h,g,Body::default(),0);let x=signed(&h,g,Body{txs:vec![pay(&alice,0,7)],evidence:vec![]},0);
    assert_eq!(x.header.ts,y.header.ts,"same timestamp, so their children have the same target");
    import(&mut h,&y);import(&mut h,&x);
    let (x2,y2)=(fallback(&h,x.hash(),0),fallback(&h,y.hash(),0));
    assert_eq!(h.chain.blocks[&x.hash()].total_work+pocold::chain::work(&x2.header.target),h.chain.blocks[&y.hash()].total_work+pocold::chain::work(&y2.header.target),"equal work");
    let (mut n,dir,conf)=rnode("seq",64,16);n.import(y.clone(),None).unwrap();n.import(x.clone(),None).unwrap();
    assert_eq!(n.chain.head,y.hash(),"the first imported wins the signed tie");
    let binding=n.chain.spec.genesis_hash();drop(n);
    {let mut rec=pocold::store::open(&dir,&binding).unwrap();let again=rec.payload(0).unwrap();rec.journal.append(&again).unwrap();}
    let mut n=reopen(&conf);assert_eq!(n.chain.head,x.hash(),"y re-imported after x: x now has the smaller importSeq");
    n.import(x2.clone(),None).unwrap();n.import(y2.clone(),None).unwrap();assert_eq!(n.chain.head,x2.hash());assert_eq!(n.chain.head,best_of(&n.chain));
    drop(n);let r=reopen(&conf);assert_eq!(r.chain.head,x2.hash(),"replay keeps the re-import order");
    let _=std::fs::remove_dir_all(&dir);
}

/// Off-heap fidelity (frozen test 6): an offloaded side block is served byte-equal from the journal,
/// viewed over RPC with its transactions, and re-executed from its journaled body to its committed
/// state root; a wrong offset is an error, never an empty body.
#[test] fn an_offloaded_block_is_served_viewed_and_re_executed_from_the_journal(){
    let alice=devnet::account_key("alice");let mut h=net();let g=h.chain.head;
    let main=grow(&mut h,g,3,&|n,p|fallback(n,p,0));
    let side=signed(&h,g,Body{txs:vec![pay(&alice,0,9)],evidence:vec![]},0);
    let (mut n,dir,_)=rnode("offload",64,16);
    for x in &main{n.import(x.clone(),None).unwrap();}n.import(side.clone(),None).unwrap();
    let sh=side.hash();assert!(!n.chain.is_canonical(&sh),"the one-block branch is lighter");
    let e=&n.chain.blocks[&sh];assert!(e.offloaded&&e.body.txs.is_empty()&&e.header.encoded.is_empty()&&e.tx_hashes.is_empty(),"positive control: the side block is offloaded");
    match n.block_msg(&sh){Some(pocold::net::Msg::Block{header,body})=>{assert_eq!(header,side.header.encoded);assert_eq!(body,side.body.encode());}m=>panic!("{m:?}")}
    let v=rpc_local(&mut n,"eth_getBlockByHash",json!([hex(&sh),true]));
    assert_eq!(v["transactions"][0]["hash"],format!("{}",revm::primitives::keccak256(&side.body.txs[0])));assert_eq!(v["size"],format!("{:#x}",side.header.encoded.len()));
    {let x=n.chain.blocks.get_mut(&sh).unwrap();x.state=Default::default();x.pruned=true;}
    n.rebuild_state(&sh).expect("re-executed from the journaled body to its committed state root");
    assert_eq!(apply::state_root(&n.chain.blocks[&sh].state).unwrap().0,side.header.state_root);
    n.chain.blocks.get_mut(&sh).unwrap().body_off=Some(41);
    assert!(n.body_of(&sh).is_err(),"a wrong offset is an error, never an empty body");
    let _=std::fs::remove_dir_all(&dir);
}

/// Crash consistency (frozen test 5). The adopting block's record is fsynced before the commit file
/// counts it and before the in-memory adoption, so a crash leaves the old chain or the whole adopted
/// chain, never a mix: (a) record fsynced, commit not updated: reopens on the adopted chain; (b) record
/// torn mid-write: reopens on the old chain, and the block re-delivered is adopted. In each, head,
/// canonical chain, state, RPC (blocks, receipts, transaction index) and journal agree.
#[test] fn a_crash_around_an_adoption_reopens_on_one_whole_chain(){
    let alice=devnet::account_key("alice");let mut a=net();let g=a.chain.head;
    let ours=grow(&mut a,g,8,&|n,p|signed(n,p,Body::default(),0));
    let mut b=net();let first=signed(&b,g,Body{txs:vec![pay(&alice,0,5)],evidence:vec![]},0);import(&mut b,&first);
    let mut theirs=vec![first.clone()];theirs.extend(heavier(&mut b,first.hash(),a.chain.head().total_work,0));
    let (mut n,dir,conf)=rnode("crash",64,16);
    for x in &ours{n.import(x.clone(),None).unwrap();}
    let (last,prefix)=theirs.split_last().unwrap();
    for x in prefix{n.import(x.clone(),None).unwrap();}assert_eq!(n.chain.head,ours.last().unwrap().hash(),"the prefix is lighter");
    let old_view=chain_view(&mut n);
    let (jp,cp)=(dir.join("blocks.journal"),dir.join("blocks.commit"));let (j0,c0)=(std::fs::metadata(&jp).unwrap().len() as usize,std::fs::read(&cp).unwrap());
    n.import(last.clone(),None).unwrap();assert_eq!(n.chain.head,last.hash(),"adopted");let new_view=chain_view(&mut n);drop(n);
    let j1=std::fs::read(&jp).unwrap();assert!(j1.len()>j0);
    let tx=format!("{}",revm::primitives::keccak256(&first.body.txs[0]));
    // (a) the record is complete and fsynced; the crash came before the commit-file update.
    std::fs::write(&cp,&c0).unwrap();
    let mut r=reopen(&conf);assert_eq!(r.chain.head,last.hash());assert!(r.chain.canonical_resident());assert_eq!(r.stats.replay_skipped,0);
    assert_eq!(chain_view(&mut r),new_view,"the whole adopted chain");assert_eq!(rpc_local(&mut r,"eth_getTransactionReceipt",json!([tx]))["blockHash"],json!(hex(&first.hash())));
    drop(r);
    // (b) the record was torn mid-write (never committed).
    std::fs::write(&cp,&c0).unwrap();std::fs::write(&jp,&j1[..j0+(j1.len()-j0)/2]).unwrap();
    let mut r=reopen(&conf);assert_eq!(r.chain.head,ours.last().unwrap().hash(),"the old chain, whole");assert!(r.chain.canonical_resident());
    assert_eq!(chain_view(&mut r),old_view);assert!(rpc_local(&mut r,"eth_getTransactionReceipt",json!([tx])).is_null(),"no transaction index entry from the lighter branch");
    assert!(prefix.iter().all(|x|r.chain.blocks.get(&x.hash()).map_or(false,|e|e.offloaded)),"the lighter prefix is retained as offloaded side blocks");
    r.import(last.clone(),None).unwrap();assert_eq!(r.chain.head,last.hash());assert_eq!(chain_view(&mut r),new_view,"re-delivery adopts the same chain");
    drop(r);let mut r=reopen(&conf);assert_eq!(chain_view(&mut r),new_view);
    let _=std::fs::remove_dir_all(&dir);
}

/// Review R3-3: replay restores each adopted branch when it is adopted, as live import does. Before,
/// one restore at the end of open rebuilt the parent of the lowest offloaded canonical block (pruned:
/// from genesis) and re-executed every canonical block up to the highest offloaded one, so one early
/// and one late depth-1 reorg made the open re-execute nearly the whole chain again. The reopened node
/// must hold the same chain as live, whole, having done the same rebuild and restore work as live.
#[test] fn replay_restores_each_adopted_branch_with_the_work_live_did(){
    let (len,early,late)=(90u64,5u64,80u64);
    let mut a=net();let (mut n,dir,conf)=rnode("r33",64,16);
    let step=|a:&mut Net,n:&mut pocold::node::Node,b:Block|{import(a,&b);a.chain.retain();n.import(b,None).unwrap();};
    while n.chain.head().height<len{
        let h=n.chain.head().height+1;let p=n.chain.head;
        if h==early||h==late{
            // F, F2 on p; then a signed S on p (a side block, offloaded), extended until it is adopted.
            let f=fallback(&a,p,0);step(&mut a,&mut n,f.clone());let f2=fallback(&a,f.hash(),0);step(&mut a,&mut n,f2);
            let s=signed(&a,p,Body::default(),0);step(&mut a,&mut n,s.clone());assert!(!n.chain.is_canonical(&s.hash()),"S starts as a side block");
            let mut q=s.hash();while n.chain.head!=q{let b=fallback(&a,q,0);q=b.hash();step(&mut a,&mut n,b);}
            assert!(n.chain.is_canonical(&s.hash()));continue;
        }
        let b=fallback(&a,p,0);step(&mut a,&mut n,b);
    }
    assert!(n.stats.reorgs>=2&&n.stats.restored>=2,"positive control: two adoptions of offloaded branches: {:?}",(n.stats.reorgs,n.stats.restored));
    let live=(n.chain.head,n.chain.rebuilds,n.chain.restore_reexecs,n.stats.restored);let view=chain_view(&mut n);drop(n);
    for _ in 0..2{
        let mut r=reopen(&conf);
        assert_eq!(r.chain.head,live.0);assert!(r.chain.canonical_resident());assert_eq!(r.stats.replay_skipped,0);
        assert_eq!((r.chain.rebuilds,r.chain.restore_reexecs,r.stats.restored),(live.1,live.2,live.3),"open does the rebuild and restore work live did, no more");
        assert!(r.chain.rebuilds+r.chain.restore_reexecs<late-early,"and not a re-execution of the span between the two reorgs");
        assert_eq!(chain_view(&mut r),view);
    }
    let _=std::fs::remove_dir_all(&dir);
}

/// C-1 S1 acceptance (proposals/C1-STORAGE-DESIGN.md): canonical history below the resident window is off
/// the heap, its bodies and headers in the journal and its receipts and transaction index in history.redb.
/// A node that offloads answers every block, transaction, receipt, log, header and block-bytes query, and
/// serves every block to peers, exactly as a reference node that keeps everything resident. The chain has
/// an early reorg, so blocks first imported as side blocks are now canonical history. The same holds:
/// - after reopening, after history.redb is deleted, and after it is overwritten with junk (the store is
///   rebuilt from the journal, invariant I1);
/// - after a crash between the history write and the journal append (I3: an orphan entry nobody reads);
/// - for a third node that syncs the whole chain over P2P from the offloading node alone.
/// The transaction index is also checked against the blocks themselves (the reference uses it too), and
/// with S1_DUMP set the reference answers are written out for the cross-version check against the tree
/// before S1 (evidence/c1-s1/).
#[test] fn canonical_history_off_the_heap_answers_as_it_did_resident(){
    use pocold::node::{Config,Limits,Node};
    let alice=devnet::account_key("alice");
    let mk=|tag:&str,window:Option<u64>|{let dir=std::env::temp_dir().join(format!("pocold-s1-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
        let d=dir.clone();let t=tag.to_string();let conf=move||Config{dir:d.clone(),name:t.clone(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Limits{history_window:window,..Default::default()}};
        (Node::open(conf(),admitted(&cfg(&["a","b","c"]))).unwrap(),dir,conf)};
    let (mut a,da,ca)=mk("off",None);let (mut b,db,_)=mk("ref",Some(u64::MAX));
    let (mut h,blocks,p)=s1_chain();
    let mut txs=vec![];for x in &blocks{for t in &x.body.txs{txs.push(format!("{}",revm::primitives::keccak256(t)));}}
    for n in [&mut a,&mut b]{for x in &blocks{n.import(x.clone(),None).unwrap();}assert_eq!(n.chain.head,p);}
    let w=a.chain.history_window.unwrap();let top=a.chain.head().height;
    let off:Vec<u64>=(1..=top).filter(|h|a.chain.canonical_at(*h).unwrap().offloaded).collect();
    assert!(off.len()>=60&&a.chain.canonical_resident(),"positive control: {} canonical blocks offloaded, the window resident",off.len());
    assert_eq!(off,(1..top.saturating_sub(w)+1).collect::<Vec<_>>(),"exactly the canonical blocks below the window of {w} are offloaded");
    assert!((1..=b.chain.head().height).all(|h|!b.chain.canonical_at(h).unwrap().offloaded),"the reference keeps all resident");
    let views=|n:&mut Node,txs:&[String]|->Vec<serde_json::Value>{let top=n.chain.head().height;let mut v=chain_view(n);
        for t in txs{v.push(rpc_local(n,"eth_getTransactionByHash",json!([t])));v.push(rpc_local(n,"eth_getTransactionReceipt",json!([t])));}
        v.push(rpc_local(n,"eth_getLogs",json!([{"fromBlock":"0x0","toBlock":format!("{top:#x}")}])));
        v.push(rpc_local(n,"pocol_getHeaders",json!(["0x1",format!("{top:#x}")])));
        for x in 0..=top{v.push(rpc_local(n,"pocol_getBlockRlp",json!([format!("{x:#x}")])));
            let hash=n.chain.canonical_at(x).unwrap().hash;v.push(json!(n.block_msg(&hash).map(|m|format!("{m:?}"))));}
        v};
    let want=views(&mut b,&txs);let logs=want.iter().filter(|x|x.is_array()).map(|x|x.as_array().unwrap().len()).max().unwrap_or(0);
    assert!(logs>=24,"positive control: the log query returns the 24 transactions' logs: {logs}");
    if let Ok(path)=std::env::var("S1_DUMP"){std::fs::write(path,serde_json::to_string_pretty(&want).unwrap()).unwrap();}
    // The transaction index against the blocks themselves: each canonical transaction at its block and
    // index, the abandoned block's transaction unknown (both nodes use the index, so this is not A vs B).
    let canon:HashMap<String,(String,u64)>=(1..=top).flat_map(|x|{let e=b.chain.canonical_at(x).unwrap();let bh=hex(&e.hash);
        e.tx_hashes.iter().enumerate().map(move|(i,t)|(format!("{t}"),(bh.clone(),i as u64))).collect::<Vec<_>>()}).collect();
    assert_eq!(canon.len(),48,"positive control: 48 canonical transactions");
    for n in [&mut a,&mut b]{for t in &txs{let got=rpc_local(n,"eth_getTransactionByHash",json!([t]));
        match canon.get(t){Some((bh,i))=>{assert_eq!(got["blockHash"].as_str().map(|x|x.trim_start_matches("0x").to_string()),Some(bh.trim_start_matches("0x").to_string()),"{t}");assert_eq!(got["transactionIndex"],json!(format!("{i:#x}")),"{t}");}
            None=>assert!(got.is_null(),"the abandoned block's transaction is not found: {t}")}}}
    assert!(txs.iter().any(|t|!canon.contains_key(t)),"positive control: one transaction only in the abandoned block");
    assert_eq!(views(&mut a,&txs),want,"offloaded canonical history answers as resident");
    let receipts_a=a.history.receipts(&blocks[1].hash()).unwrap();assert!(receipts_a.is_some(),"the side block's receipts were stored when it was imported");
    let open=||Node::open(ca(),admitted(&cfg(&["a","b","c"]))).unwrap();
    drop(a);let mut r=open();assert_eq!(r.stats.history_refilled,0,"an intact store is not rewritten");assert_eq!(views(&mut r,&txs),want,"after reopen");drop(r);
    std::fs::remove_file(da.join(pocold::history::FILE)).unwrap();
    let mut r=open();assert_eq!(r.stats.history_refilled,blocks.len() as u64,"replay refilled every journaled block");
    assert_eq!(views(&mut r,&txs),want,"after history.redb was deleted: rebuilt by replay");drop(r);
    std::fs::write(da.join(pocold::history::FILE),vec![0x5a;65536]).unwrap();
    let mut r=open();assert!(r.history.rebuilt,"a damaged store is set aside and rebuilt");assert_eq!(r.stats.history_refilled,blocks.len() as u64);
    assert_eq!(views(&mut r,&txs),want,"after history.redb was overwritten");
    let len=std::fs::metadata(da.join(pocold::history::FILE)).unwrap().len();drop(r);
    let f=std::fs::OpenOptions::new().write(true).open(da.join(pocold::history::FILE)).unwrap();f.set_len(len/2).unwrap();drop(f);
    let mut r=open();assert!(r.history.rebuilt,"a truncated store is set aside and rebuilt");assert_eq!(views(&mut r,&txs),want,"after history.redb was truncated");drop(r);
    // I3: a crash after the history write and before the journal append. The next block X is validated by
    // the reference; its receipts and index go into A's store as import writes them, and A stops there.
    let x=branch(&mut h,p,1,&[Body{txs:vec![pay(&alice,48,9)],evidence:vec![]}]).remove(0);let xt=format!("{}",revm::primitives::keccak256(&x.body.txs[0]));
    b.import(x.clone(),None).unwrap();let ex=b.chain.get(&x.hash()).unwrap().clone();
    let r=open();r.history.put(&ex.hash,&ex.receipts,&ex.tx_hashes,&ex.ranges).unwrap();drop(r);
    let mut r=open();assert_eq!(r.chain.head,p,"the block was never journaled");assert!(r.history.has(&x.hash()).unwrap(),"positive control: the orphan entry is there");
    assert!(rpc_local(&mut r,"eth_getTransactionByHash",json!([xt])).is_null()&&rpc_local(&mut r,"eth_getTransactionReceipt",json!([xt])).is_null(),"an orphan entry is never read");
    assert_eq!(views(&mut r,&txs),want,"after the crash");
    r.import(x.clone(),None).unwrap();txs.push(xt);let want=views(&mut b,&txs);assert_eq!(views(&mut r,&txs),want,"the block imported after the crash");drop(r);
    // P2P: node C syncs the whole chain from A alone (GetFrom/GetRange serve offloaded canonical blocks).
    let port=||std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let addr=|p:u16|->std::net::SocketAddr{format!("127.0.0.1:{p}").parse().unwrap()};
    let (pa,pc,rc)=(port(),port(),port());let dc=std::env::temp_dir().join(format!("pocold-s1-sync-{}",std::process::id()));let _=std::fs::remove_dir_all(&dc);
    let mut na=Node::open(Config{listen:Some(addr(pa)),..ca()},admitted(&cfg(&["a","b","c"]))).unwrap();
    let served=(1..=na.chain.head().height).filter(|h|na.chain.canonical_at(*h).unwrap().offloaded).count();assert!(served>=60,"positive control: A serves {served} offloaded blocks");
    let target=x.hash();na.start_network().unwrap();std::thread::spawn(move||na.run());
    let mut nc=Node::open(Config{dir:dc.clone(),name:"c".into(),miner:None,listen:Some(addr(pc)),peers:vec![addr(pa)],rpc:Some(addr(rc)),stop_at:None,log:false,limits:Default::default()},admitted(&cfg(&["a","b","c"]))).unwrap();
    nc.start_network().unwrap();std::thread::spawn(move||nc.run());
    assert!(wait_for(300,&||rpc_tcp(rc,"eth_getBlockByNumber",json!(["latest",false]))["hash"]==json!(hex(&target))),"C syncs to A's head over P2P");
    let tcp_view=|p:u16|{let mut v=vec![];for h in 0..=b.chain.head().height{let blk=rpc_tcp(p,"eth_getBlockByNumber",json!([format!("{h:#x}"),true]));
        for t in blk["transactions"].as_array().cloned().unwrap_or_default(){v.push(rpc_tcp(p,"eth_getTransactionReceipt",json!([t["hash"]])));}v.push(blk);}v};
    assert_eq!(tcp_view(rc),chain_view(&mut b),"the synced node answers as the reference");
    for d in [da,db,dc]{let _=std::fs::remove_dir_all(&d);}
}
/// C-1 S1: a damaged history.redb is never served. Single bytes of the store are flipped: inside receipts
/// records and transaction index keys (found by their bytes) and at positions spread over the file. After
/// each flip the node opens and answers every block, transaction, receipt, log, header and block-bytes
/// query. Each answer equals the reference's, or is an error while the store is marked damaged; the
/// comparison would also admit a transaction reported not found, and the tally shows none occurs: open's
/// full integrity check catches every targeted flip. A wrong block, transaction, receipt or log is never
/// returned. Then wrong content that passes redb's checksums (written through redb) is caught by its
/// reader. The next open rebuilds a store marked damaged, and its answers equal the reference's.
#[test] fn a_damaged_history_store_is_never_served_and_is_rebuilt(){
    use pocold::node::{Config,Limits,Node};
    let dir=std::env::temp_dir().join(format!("pocold-s1-flip-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    let conf=||Config{dir:dir.clone(),name:"flip".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Limits::default()};
    let open=||Node::open(conf(),admitted(&cfg(&["a","b","c"]))).unwrap();
    let (_,blocks,_)=s1_chain();
    let mut a=open();for x in &blocks{a.import(x.clone(),None).unwrap();}
    let txs:Vec<String>=blocks.iter().flat_map(|x|x.body.txs.iter().map(|t|format!("{}",revm::primitives::keccak256(t)))).collect();
    let full=|n:&mut Node,m:&str,p:serde_json::Value|{let body=json!({"jsonrpc":"2.0","id":1,"method":m,"params":p}).to_string();let (t,_r)=std::sync::mpsc::channel();
        let r=pocold::rpc::answer(n,&pocold::rpc::Request::local(body.into_bytes(),t,std::time::Instant::now()+std::time::Duration::from_secs(120),false));
        let v:serde_json::Value=serde_json::from_str(&r.body).unwrap();if v.get("error").is_some(){json!({"error":v["error"]["code"]})}else{json!({"result":v["result"]})}};
    // (answer, whether it is a transaction lookup)
    let answers=|n:&mut Node|->Vec<(serde_json::Value,bool)>{let top=n.chain.head().height;let mut v=vec![];
        for t in &txs{v.push((full(n,"eth_getTransactionByHash",json!([t])),true));v.push((full(n,"eth_getTransactionReceipt",json!([t])),true));}
        v.push((full(n,"eth_getLogs",json!([{"fromBlock":"0x0","toBlock":format!("{top:#x}")}])),false));
        v.push((full(n,"pocol_getHeaders",json!(["0x1",format!("{top:#x}")])),false));
        for x in 0..=top{let hx=format!("{x:#x}");v.push((full(n,"eth_getBlockByNumber",json!([hx,true])),false));v.push((full(n,"pocol_getBlockRlp",json!([hx])),false));
            let hash=hex(&n.chain.canonical_at(x).unwrap().hash);v.push((full(n,"eth_getBlockByHash",json!([hash,true])),false));}
        v};
    let want=answers(&mut a);assert!(want.iter().all(|(x,_)|x.get("result").is_some()),"positive control: every reference answer is a result");
    let path=dir.join(pocold::history::FILE);
    // Targets: a byte inside each of a few receipts records and inside a few index keys, then spread.
    let mut receipts_at=vec![];let mut keys_at=vec![];
    for x in blocks.iter().filter(|x|!x.body.txs.is_empty()).step_by(6){let e=a.block_view(&x.hash()).unwrap();let r=&e.receipts[0];
        let mut k=e.tx_hashes[0].to_vec();k.extend_from_slice(&e.hash);receipts_at.push(r.clone());keys_at.push(k);}
    drop(a);let orig=std::fs::read(&path).unwrap();
    let find=|needle:&[u8]|orig.windows(needle.len()).position(|w|w==needle);
    let mut pos:Vec<(usize,&str)>=vec![];
    for r in &receipts_at{if let Some(i)=find(r){pos.push((i+r.len()/2,"receipts"));}}
    for k in &keys_at{if let Some(i)=find(k){pos.push((i+10,"key-tx"));pos.push((i+40,"key-block"));}}
    assert!(pos.iter().filter(|p|p.1=="receipts").count()>=3&&pos.iter().filter(|p|p.1=="key-tx").count()>=3,"positive control: records and keys found in the file: {pos:?}");
    for k in 0..24{pos.push(((orig.len()/24)*k+ (k*131)%4096,"spread"));}
    let mut tally:HashMap<String,usize>=HashMap::new();
    for (at,kind) in &pos{let at=(*at).min(orig.len()-1);
        let mut f=orig.clone();f[at]^=0xff;std::fs::write(&path,&f).unwrap();let _=std::fs::remove_file(dir.join(pocold::history::DAMAGED));
        let mut r=open();let rebuilt=r.history.rebuilt;let got=answers(&mut r);let damaged=r.history.is_damaged();
        assert_eq!(got.len(),want.len(),"{kind}@{at}");
        let mut not_found=0;let mut errors=0;
        for ((g,_),(w,is_tx)) in got.iter().zip(&want){if g==w{continue}
            if g.get("error")==Some(&json!(-32603)){assert!(damaged,"{kind}@{at}: an error only once the store is marked damaged");errors+=1;continue}
            assert!(*is_tx&&g.get("result")==Some(&serde_json::Value::Null),"{kind}@{at}: a wrong answer {g} for {w}");not_found+=1;}
        let outcome=if rebuilt{"rebuilt at open"}else if errors>0{"detected: errors, marked damaged"}else if damaged{"detected: marked damaged"}else if not_found>0{"undetected: not found"}else{"no effect"};
        *tally.entry(format!("{kind}: {outcome}")).or_default()+=1;
        drop(r);let mut r=open();if damaged{assert!(r.history.rebuilt,"{kind}@{at}: the store marked damaged is rebuilt");}
        if damaged||rebuilt{assert_eq!(answers(&mut r),want,"{kind}@{at}: answers after the rebuild");}drop(r);
    }
    let mut t:Vec<_>=tally.into_iter().collect();t.sort();println!("S1-FLIP {}",json!(t));
    // Open's full check (every page checksum) catches each targeted flip, index keys included.
    assert!(!t.iter().any(|(k,_)|k.contains("undetected")),"no flip goes undetected with a changed answer: {t:?}");
    for kind in ["receipts","key-tx","key-block"]{assert!(t.iter().filter(|(k,_)|k.starts_with(kind)).all(|(k,_)|k.ends_with("rebuilt at open")),"{kind}: {t:?}");}
    // Wrong content that redb's checksums cannot see (written through redb itself): the reader's checks
    // catch it. A receipts record under another block's key fails its seal; an index entry with the wrong
    // position does not name its transaction.
    let x=blocks.iter().find(|b|b.body.txs.len()==2).unwrap();let y=blocks.iter().rev().find(|b|b.body.txs.len()==2).unwrap();
    let tx0=revm::primitives::keccak256(&x.body.txs[0]);
    let receipts_t:redb::TableDefinition<&[u8;32],&[u8]>=redb::TableDefinition::new("receipts");
    let txidx_t:redb::TableDefinition<&[u8;64],u32>=redb::TableDefinition::new("txidx");
    use redb::ReadableTable;let edits:[(&str,Box<dyn Fn(&redb::WriteTransaction)>);2]=[
        ("receipts under another key",Box::new(|w|{let mut t=w.open_table(receipts_t).unwrap();let v=t.get(&y.hash()).unwrap().unwrap().value().to_vec();t.insert(&x.hash(),v.as_slice()).unwrap();})),
        ("index entry at the wrong position",Box::new(|w|{let mut t=w.open_table(txidx_t).unwrap();let mut k=[0u8;64];k[..32].copy_from_slice(tx0.as_slice());k[32..].copy_from_slice(&x.hash());
            assert_eq!(t.get(&k).unwrap().map(|v|v.value()),Some(0),"positive control: the entry exists");t.insert(&k,1u32).unwrap();}))];
    for (what,edit) in &edits{
        std::fs::write(&path,&orig).unwrap();
        {let db=redb::Database::create(&path).unwrap();let w=db.begin_write().unwrap();edit(&w);w.commit().unwrap();}
        let mut r=open();assert!(!r.history.rebuilt,"{what}: positive control: redb's check passes");let got=answers(&mut r);let damaged=r.history.is_damaged();
        let mut errors=0;
        for ((g,_),(w,_)) in got.iter().zip(&want){if g==w{continue}
            assert_eq!(g.get("error"),Some(&json!(-32603)),"{what}: a wrong answer {g} for {w}");errors+=1;}
        assert!(damaged&&errors>0,"{what}: detected by its reader ({errors} errors) and the store marked damaged");
        println!("S1-CONTENT {}",json!({"edit":what,"errors":errors}));
        drop(r);let mut r=open();assert!(r.history.rebuilt,"{what}: rebuilt at the next open");assert_eq!(answers(&mut r),want,"{what}: answers after the rebuild");drop(r);
    }
    let _=std::fs::remove_dir_all(&dir);
}
/// C-1 S3a acceptance (proposals/C1-STORAGE-DESIGN.md "S3 refinement"): only the head, genesis and the
/// side states within side_max are held full; every other retained state is a diff from its parent's and
/// is materialized on demand. The same import sequence goes into a chain with S3a and into a reference
/// that keeps every retained state full (the behaviour before S3a). The sequence has transactions and
/// contract creates, a 10-block reorg inside the K_hist window, a re-reorg back, and an adoption from a
/// fork below the cut (TrieRebuild). After every import:
/// - head, canonical chain and the pruned flag of every canonical block are the reference's;
/// - every state S3a still has equals the reference's full state field by field, and its stateRoot is the
///   block's header stateRoot;
/// - S3a keeps a state only where the reference does, and holds at most 2 + side_max full states.
#[test] fn s3a_every_retained_state_equals_the_full_state_it_replaces(){
    use pocold::statediff::states_equal;
    let alice=devnet::account_key("alice");let bob=devnet::account_key("bob");
    let log_tx=|nonce:u64|Tx{chain_id:777910,nonce,max_priority:U256::from(1u64),max_fee:U256::from(100_000_000_000u64),gas:200_000,to:None,value:U256::ZERO,data:vec![0x61,0x00,0x10,0x5f,0xa0,0x00].into()}.sign_1559(&alice);
    let mut h=net();let g=h.chain.head;
    let mut main=vec![];let (mut p,mut nonce)=(g,0u64);
    for i in 0..70u64{let body=if i%3==0{let t=vec![pay(&alice,nonce,1+i),log_tx(nonce+1)];nonce+=2;Body{txs:t,evidence:vec![]}}else{Body::default()};
        let x=branch(&mut h,p,1,&[body]);p=x[0].hash();main.extend(x);}
    let at=|v:&Vec<Block>,k:usize|v[k-1].hash();
    // A 10-deep heavier branch from height 60 (with a transaction), then the main branch extended past it.
    let mut h2=net();for b in &main[..60]{import(&mut h2,b);}
    let mut side=vec![signed(&h2,at(&main,60),Body{txs:vec![pay(&bob,0,7)],evidence:vec![]},0)];import(&mut h2,&side[0]);
    side.extend(heavier(&mut h2,side[0].hash(),h.chain.get(&main[69].hash()).unwrap().total_work,1));
    let mut h3=net();for b in &main{import(&mut h3,b);}
    let back=heavier(&mut h3,main[69].hash(),h2.chain.head().total_work,1);
    // A fork below the K_hist cut (height 20), adopted once heavier: its parent state was pruned.
    let mut h4=net();for b in &main[..20]{import(&mut h4,b);}
    // Its first block is the attempt-1 fallback, so it differs from the main block at that height.
    let d0=fallback(&h4,at(&main,20),1);import(&mut h4,&d0);
    let mut deep=vec![d0.clone()];deep.extend(heavier(&mut h4,d0.hash(),h3.chain.head().total_work,1));
    let seq:Vec<Block>=main.iter().chain(&side).chain(&back).chain(&deep).cloned().collect();
    let mut a=net();let mut b=net();b.chain.keep_full_states=true;
    // As the node imports: retain, the restore of an adopted branch, then settle (no journal: nothing here
    // is offloaded, so the restore re-executes only the adopted blocks without a diff).
    struct NoJournal;impl pocold::chain::BlockBytes for NoJournal{fn record_at(&self,_:u64)->Result<(Vec<u8>,Vec<u8>,Vec<u8>),Reject>{Err(Reject::Block("no journal".into()))}}
    // The deep branch arrives as a sync session's does: protected from the side cap while non-canonical.
    let session:std::collections::HashSet<[u8;32]>=deep.iter().map(|x|x.hash()).collect();let m63=main[63].hash();
    let step=|n:&mut Net,x:&Block|{let r=match n.chain.validate(x,None,0){Err(Reject::NeedsRebuild(q))=>{n.chain.ensure_state(&q).unwrap();n.chain.validate(x,None,0)}r=>r};
        let (e,_)=r.unwrap_or_else(|e|panic!("h={} {e:?}",x.header.h));let old=n.chain.head;n.chain.insert(e);
        if session.contains(&x.hash())&&!n.chain.is_canonical(&x.hash()){n.chain.protect.insert(x.hash());}
        // The moment the main branch is left, its block at height 64 is protected (as a sync session's
        // block is): its state is kept while its neighbours' are pruned.
        if !n.chain.is_canonical(&m63)&&n.chain.get(&m63).map_or(false,|e|!e.pruned){n.chain.protect.insert(m63);}
        let fork_h=if n.chain.head!=old{n.chain.lca_height(&old,&n.chain.head).unwrap_or(0)}else{0};let adopted=n.chain.head!=old;
        n.chain.retain_unsettled();if adopted{n.chain.restore_canonical(fork_h,&NoJournal).unwrap();}n.chain.settle();};
    let side_max=a.chain.spec.local().side_max as usize;let (mut compared,mut max_full,mut ref_max)=(0usize,0usize,0usize);let mut protected_answered=false;let mut was_side=false;
    for (i,x) in seq.iter().enumerate(){
        let before=a.chain.head;step(&mut a,x);step(&mut b,x);
        // The main branch is adopted again later: the protected block's height must then answer again.
        if !a.chain.is_canonical(&main[63].hash()){was_side=true;}
        ref_max=ref_max.max(b.chain.blocks.values().filter(|e|!e.pruned).count());
        let full=a.chain.blocks.values().filter(|e|e.full&&!e.pruned).count();max_full=max_full.max(full);
        assert!(full<=2+side_max,"step {i}: {full} full states");
        assert_eq!(a.chain.head,b.chain.head,"step {i}");assert_eq!(a.chain.canonical,b.chain.canonical,"step {i}");
        for (hh,eb) in &b.chain.blocks{assert_eq!(a.chain.blocks[hh].pruned,eb.pruned,"step {i}: pruned flag at h={} (canonical {})",eb.height,b.chain.is_canonical(hh));}
        // Every state in full at each reorg (the head did not just extend), every 4th step and at the end.
        let reorg=a.chain.get(&a.chain.head).and_then(|e|e.parent)!=Some(before);
        if !(reorg||i%4==0||i+1==seq.len()){continue}
        for (hh,eb) in &b.chain.blocks{let ea=&a.chain.blocks[hh];
            if ea.pruned{continue}
            let st=a.chain.state_of(hh).unwrap_or_else(||panic!("step {i}: h={} not reachable",eb.height));
            assert!(states_equal(&st,&eb.state),"step {i}: h={} materialized state differs",eb.height);
            if eb.height>0{assert_eq!(apply::state_root(&st).unwrap().0,eb.header.state_root,"step {i}: h={} stateRoot",eb.height);}
            if *hh==m63&&was_side&&a.chain.is_canonical(hh){protected_answered=true;}
            compared+=1;}
    }
    let mats=a.chain.materializations.load(std::sync::atomic::Ordering::Relaxed);
    assert_eq!(a.chain.head,deep.last().unwrap().hash(),"positive control: the deep fork is adopted");
    assert!(protected_answered,"positive control: the protected main block kept its state and was canonical again with it");
    assert!(a.chain.rebuilds>0&&mats>0,"positive control: a TrieRebuild ({}) and materializations ({mats})",a.chain.rebuilds);
    assert!(compared>1000&&ref_max>max_full*5,"positive control: {compared} states compared; the reference held up to {ref_max} full states, S3a at most {max_full}");
    println!("S3A-EQUIV {}",json!({"steps":seq.len(),"statesCompared":compared,"maxFull":max_full,"referenceMaxFull":ref_max,"materializations":mats,"rebuildsS3a":a.chain.rebuilds,"rebuildsRef":b.chain.rebuilds}));
}

/// C-1 E1: the incremental state root. The tests build with lp4's `root-check` feature, so every root a
/// block execution computes from a parent's trie is also computed over the whole state and compared
/// (apply_block_with panics on a difference). This test drives every way a parent's trie is obtained:
/// - the head's own trie (a chain that extends);
/// - derived from the head's trie through the diffs (a sibling, a side branch, the branch back, a fork
///   below the K_hist cut, the rebuild of a pruned state close to the head);
/// - built from the whole state (a parent more than TRIE_PATH_MAX diffs from the head: the rebuild of the
///   deep fork's pruned parent);
/// and checks that a node with the incremental root accepts exactly what one without it accepts, with the
/// same heads, canonical chains, pruned flags and state roots, and that no trie was ever refused.
#[test] fn e1_incremental_roots_are_the_full_roots_on_every_path(){
    let alice=devnet::account_key("alice");let bob=devnet::account_key("bob");
    let log_tx=|nonce:u64|Tx{chain_id:777910,nonce,max_priority:U256::from(1u64),max_fee:U256::from(100_000_000_000u64),gas:200_000,to:None,value:U256::ZERO,data:vec![0x61,0x00,0x10,0x5f,0xa0,0x00].into()}.sign_1559(&alice);
    let mut h=net();let g=h.chain.head;
    let mut main=vec![];let (mut p,mut nonce)=(g,0u64);
    for i in 0..90u64{let body=if i%3==0{let t=vec![pay(&alice,nonce,1+i),log_tx(nonce+1)];nonce+=2;Body{txs:t,evidence:vec![]}}else{Body::default()};
        let x=branch(&mut h,p,1,&[body]);p=x[0].hash();main.extend(x);}
    let at=|v:&Vec<Block>,k:usize|v[k-1].hash();
    let mut h2=net();for b in &main[..80]{import(&mut h2,b);}
    let mut side=vec![signed(&h2,at(&main,80),Body{txs:vec![pay(&bob,0,7)],evidence:vec![]},0)];import(&mut h2,&side[0]);
    side.extend(heavier(&mut h2,side[0].hash(),h.chain.get(&main[89].hash()).unwrap().total_work,1));
    let mut h3=net();for b in &main{import(&mut h3,b);}
    let back=heavier(&mut h3,main[89].hash(),h2.chain.head().total_work,1);
    // A fork at height 15: more than TRIE_PATH_MAX blocks below the head when it arrives.
    let mut h4=net();for b in &main[..15]{import(&mut h4,b);}
    let d0=fallback(&h4,at(&main,15),1);import(&mut h4,&d0);
    let mut deep=vec![d0.clone()];deep.extend(heavier(&mut h4,d0.hash(),h3.chain.head().total_work,1));
    let seq:Vec<Block>=main.iter().chain(&side).chain(&back).chain(&deep).cloned().collect();
    let mut a=net();let mut b=net();b.chain.incremental_root=false;
    struct NoJournal;impl pocold::chain::BlockBytes for NoJournal{fn record_at(&self,_:u64)->Result<(Vec<u8>,Vec<u8>,Vec<u8>),Reject>{Err(Reject::Block("no journal".into()))}}
    // The deep branch arrives as a sync session's does: protected from the side cap while non-canonical.
    let session:std::collections::HashSet<[u8;32]>=deep.iter().map(|x|x.hash()).collect();
    let step=|n:&mut Net,x:&Block|{let r=match n.chain.validate(x,None,0){Err(Reject::NeedsRebuild(q))=>{n.chain.ensure_state(&q).unwrap();n.chain.validate(x,None,0)}r=>r};
        let (e,_)=r.unwrap_or_else(|e|panic!("h={} {e:?}",x.header.h));let old=n.chain.head;n.chain.insert(e);
        if session.contains(&x.hash())&&!n.chain.is_canonical(&x.hash()){n.chain.protect.insert(x.hash());}
        let fork_h=if n.chain.head!=old{n.chain.lca_height(&old,&n.chain.head).unwrap_or(0)}else{0};let adopted=n.chain.head!=old;
        n.chain.retain_unsettled();if adopted{n.chain.restore_canonical(fork_h,&NoJournal).unwrap();}n.chain.settle();};
    let ld=|x:&std::sync::atomic::AtomicU64|x.load(std::sync::atomic::Ordering::Relaxed);
    let counts=|n:&Net|(ld(&n.chain.trie_derived),ld(&n.chain.trie_built),ld(&n.chain.trie_refused));
    let mut phases=vec![];
    for (i,x) in seq.iter().enumerate(){
        step(&mut a,x);step(&mut b,x);
        if [main.len(),main.len()+side.len(),main.len()+side.len()+back.len(),seq.len()].contains(&(i+1)){phases.push(counts(&a));}
        assert_eq!(a.chain.head,b.chain.head,"step {i}");assert_eq!(a.chain.canonical,b.chain.canonical,"step {i}");
        for (hh,eb) in &b.chain.blocks{let ea=&a.chain.blocks[hh];assert_eq!((ea.pruned,ea.header.state_root),(eb.pruned,eb.header.state_root),"step {i}");
            // Only the head keeps a trie after settle, and it is the head state's.
            assert_eq!(ea.trie.is_some(),*hh==a.chain.head,"step {i}: h={}",ea.height);}
        let head=a.chain.head();assert_eq!(head.trie.as_ref().unwrap().root(),apply::state_root(&head.state).unwrap(),"step {i}");
        // C-1 E2: every entry that is not slimmed carries its state's summary.
        assert_eq!(head.summary.as_deref(),Some(&pocol_lp4_execution::state_summary::Summary::scan(&head.state,a.chain.spec.cp().B_code_max)),"step {i}");
        assert!(a.chain.blocks.values().all(|e|e.slim||e.summary.is_some()),"step {i}");
    }
    // Production and a fallback child on the final head take the head's trie.
    let before=counts(&a);let fb=fallback(&a,a.chain.head,0);step(&mut a,&fb);step(&mut b,&fb);assert_eq!(a.chain.head,fb.hash());
    let (derived,built,refused)=counts(&a);
    println!("E1-PATHS {}",json!({"blocks":seq.len()+1,"phases":{"main":phases[0],"side":phases[1],"back":phases[2],"deep":phases[3]},"rebuilds":a.chain.rebuilds,"restoreReexecs":a.chain.restore_reexecs,"side":side.len(),"back":back.len(),"deep":deep.len()}));
    assert_eq!(refused,0,"no trie is refused when every trie belongs to its state");
    // The main chain extends the head: every root from the head's own trie.
    assert_eq!(phases[0],(0,0,0),"main: the head's own trie only");
    // The side branch: its first block's parent is 10 blocks below the head, and while the branch is no
    // deeper than side_max its parents keep their diffs, so their tries are derived from the head's. Past
    // that, a side block whose state was pruned has lost its diff, the path breaks, and the root is built
    // from the whole state, as before E1 (as for the branch back, and for the deep fork, whose parents are
    // also more than TRIE_PATH_MAX diffs from the head).
    let side_max=u64::from(a.chain.spec.local().side_max);
    assert!(phases[1].0>=side_max,"positive control: tries derived from the head's ({phases:?})");
    assert!(phases[1].1>=1&&phases[3].1>phases[2].1,"positive control: whole-state builds ({phases:?})");
    // A child of the final head (a fallback block validated and produced) takes the head's trie again.
    assert_eq!(counts(&a),before);let _=(derived,built);
    assert_eq!(ld(&b.chain.trie_derived)+ld(&b.chain.trie_built)+ld(&b.chain.trie_refused),0);
}

/// C-1 E1: a rolling rebuild carries each re-executed block's trie to the next block, so a rebuild of many
/// blocks builds one trie (for its root block) and refuses none.
#[test] fn e1_a_rolling_rebuild_carries_the_trie(){
    let mut h=net();let g=h.chain.head;let x=branch(&mut h,g,40,&[]);
    let mut n=net();for b in &x{import(&mut n,b);}n.chain.retain();
    let ld=|x:&std::sync::atomic::AtomicU64|x.load(std::sync::atomic::Ordering::Relaxed);
    let target=x[6].hash();assert!(n.chain.get(&target).unwrap().pruned,"positive control: the state is pruned");
    let (r0,b0,f0)=(n.chain.rebuilds,ld(&n.chain.trie_built),ld(&n.chain.trie_refused));
    n.chain.ensure_state(&target).unwrap();
    assert!(n.chain.rebuilds-r0>=5,"positive control: a rolling rebuild of {} blocks",n.chain.rebuilds-r0);
    assert_eq!((ld(&n.chain.trie_built)-b0,ld(&n.chain.trie_refused)-f0),(1,0),"one build for the rebuild's first block, none refused");
}

/// C-1 E1: a trie that is not the parent state's is never used. The head's trie is replaced by another
/// state's (genesis's); a child of the head and a sibling of the head (derived through the head's trie)
/// must both still validate with the right root, the wrong trie refused and the root computed from the
/// whole state instead.
#[test] fn e1_a_trie_that_is_not_the_parents_is_never_used(){
    let alice=devnet::account_key("alice");
    let mut h=net();let g=h.chain.head;let x=branch(&mut h,g,6,&[Body{txs:vec![pay(&alice,0,5)],evidence:vec![]}]);
    let mut n=net();for b in &x{import(&mut n,b);}
    let wrong=n.chain.blocks[&g].trie.clone().unwrap();
    let ld=|x:&std::sync::atomic::AtomicU64|x.load(std::sync::atomic::Ordering::Relaxed);
    let head=n.chain.head;let child=fallback(&n,head,0);let sib=fallback(&n,x[4].hash(),1);
    assert_eq!((ld(&n.chain.trie_refused),ld(&n.chain.trie_built),ld(&n.chain.trie_derived)),(0,0,0),"the blocks were built with the parents' own tries");
    n.chain.blocks.get_mut(&head).unwrap().trie=Some(wrong.clone());
    let (e,_)=n.chain.validate(&child,None,0).expect("child validates");assert_eq!(e.header.state_root,child.header.state_root);
    assert_eq!((ld(&n.chain.trie_refused),ld(&n.chain.trie_built)),(1,1),"the head's wrong trie is refused, the root built");
    // The sibling's parent (x[4]) has its own trie in this unsettled chain: drop it so that it is derived.
    n.chain.blocks.get_mut(&x[4].hash()).unwrap().trie=None;
    let (e,_)=n.chain.validate(&sib,None,0).expect("sibling validates");assert_eq!(e.header.state_root,sib.header.state_root);
    assert_eq!((ld(&n.chain.trie_refused),ld(&n.chain.trie_built),ld(&n.chain.trie_derived)),(2,2,0),"the derivation through the wrong trie is refused");
    // With the head's own trie back, the same sibling's parent trie is derived.
    n.chain.blocks.get_mut(&head).unwrap().trie=None;let t=pocol_lp4_execution::state_trie::StateTrie::build(&n.chain.head().state);n.chain.blocks.get_mut(&head).unwrap().trie=Some(t);
    n.chain.validate(&sib,None,0).expect("sibling validates");assert_eq!(ld(&n.chain.trie_derived),1);
}

/// C-1 E4.3: a node whose states are trie versions in state.redb answers every state query (balance,
/// nonce, code, storage, eth_call, "pending") at every height and tag exactly as a node that keeps whole
/// states, live, after a reorg and back, and after reopening, with the same blocks, heads and rebuild
/// work; it holds no whole state.
#[test] fn e4_a_store_node_answers_as_a_whole_state_node(){
    use pocold::node::{Config,Limits,Node};
    let mk=|tag:&str,store:bool|{let dir=std::env::temp_dir().join(format!("pocold-e4-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
        let d=dir.clone();let t=tag.to_string();let conf=move||Config{dir:d.clone(),name:t.clone(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Limits{state_store:store,..Default::default()}};
        (Node::open(conf(),admitted(&cfg(&["a","b","c"]))).unwrap(),dir,conf)};
    let (mut a,da,ca)=mk("store",true);let (mut b,db,cb)=mk("mem",false);
    let (mut h,blocks,p)=s1_chain();
    let fork=h.chain.get(&p).unwrap().parent.unwrap();let fork=h.chain.get(&fork).unwrap().parent.unwrap();
    let mut h2=net();for x in &blocks{import(&mut h2,x);}
    let bob=devnet::account_key("bob");
    let mut side=vec![signed(&h2,fork,Body{txs:vec![pay(&bob,0,3)],evidence:vec![]},0)];import(&mut h2,&side[0]);
    side.extend(heavier(&mut h2,side[0].hash(),h.chain.get(&p).unwrap().total_work,2));
    let back=heavier(&mut h,p,h2.chain.head().total_work,1);
    let alice=devnet::account_key("alice");let alice_a=alice.address();
    let accounts:Vec<Address>=std::iter::once(alice_a).chain([bob.address(),Address::ZERO]).chain((0..50u64).map(|k|alice_a.create(k))).collect();
    let full=|n:&mut Node,m:&str,p:serde_json::Value|{let body=json!({"jsonrpc":"2.0","id":1,"method":m,"params":p}).to_string();let (t,_r)=std::sync::mpsc::channel();
        let r=pocold::rpc::answer(n,&pocold::rpc::Request::local(body.into_bytes(),t,std::time::Instant::now()+std::time::Duration::from_secs(120),false));
        let v:serde_json::Value=serde_json::from_str(&r.body).unwrap();if v.get("error").is_some(){json!({"error":v["error"]["code"]})}else{json!({"result":v["result"]})}};
    let answers=|n:&mut Node|->Vec<serde_json::Value>{let top=n.chain.head().height;let mut v=vec![];
        let tags:Vec<serde_json::Value>=(0..=top).map(|x|json!(format!("{x:#x}"))).chain(["latest","safe","finalized","earliest","pending"].map(|t|json!(t))).collect();
        for t in &tags{for a in &accounts{let a=format!("{a:#x}");
            for m in ["eth_getBalance","eth_getTransactionCount","eth_getCode"]{v.push(full(n,m,json!([a,t])));}
            v.push(full(n,"eth_getStorageAt",json!([a,"0x0",t])));v.push(full(n,"eth_getStorageAt",json!([a,"0x1",t])));}
            v.push(full(n,"eth_call",json!([{"to":format!("{:#x}",alice_a.create(1)),"data":"0x"},t])));}
        v.push(full(n,"pocol_getMiners",json!([])));
        v};
    let compare=|a:&mut Node,b:&mut Node,what:&str|{let (x,y)=(answers(a),answers(b));assert_eq!(x.len(),y.len());
        let ok=x.iter().filter(|r|r.get("result").is_some()).count();
        for (i,(p,q)) in x.iter().zip(&y).enumerate(){if p!=q{
            let ht=i/266;let fl=|n:&Node|n.chain.canonical_at(ht as u64).map(|e|(e.height,e.pruned,e.full,e.stored.is_some(),e.diff.is_some()));
            panic!("{what}: answer {i} (height {ht}, head {} / {}): store {p} {:?}, memory {q} {:?}",a.chain.head().height,b.chain.head().height,fl(a),fl(b));}}
        assert!(ok>1000,"positive control ({what}): {ok} answers");
        assert_eq!((a.chain.head,a.chain.head().header.state_root),(b.chain.head,b.chain.head().header.state_root),"{what}: the same head");};
    let whole=|n:&Node|n.chain.blocks.values().filter(|e|!e.state.accounts.is_empty()).count();
    let stored=|n:&Node|n.chain.blocks.values().filter(|e|e.stored.is_some()).count();
    // Every pin the store holds is a stored version an entry holds (the pool's base shares the head's):
    // nothing leaks when retention drops a state.
    let pins=|n:&Node|n.chain.trie_store.as_ref().unwrap().pinned().unwrap() as usize;
    // The store node keeps a version of exactly the states the whole-state node can still read (retention
    // is unchanged): no state is kept longer, none dropped sooner.
    let same_states=|a:&Node,b:&Node,what:&str|{
        let mut x:Vec<[u8;32]>=a.chain.blocks.values().filter(|e|e.stored.is_some()).map(|e|e.hash).collect();
        let mut y:Vec<[u8;32]>=b.chain.blocks.values().filter(|e|b.chain.view(e).is_some()).map(|e|e.hash).collect();x.sort();y.sort();
        assert_eq!(x,y,"{what}: the retained states");};
    for x in &blocks{a.import(x.clone(),None).unwrap();b.import(x.clone(),None).unwrap();}
    // Pool transactions on both: the pending state is a layer over the stored head.
    for (k,t) in [pay(&alice,blocks.len() as u64*0+a.chain.head().cum_txs,1)].into_iter().enumerate(){let _=k;let ra=a.submit_tx(t.clone(),false);let rb=b.submit_tx(t,false);assert_eq!(ra.is_ok(),rb.is_ok());}
    assert_eq!(whole(&a),0,"the store node holds no whole state");assert!(stored(&a)>10,"positive control: {} stored versions",stored(&a));
    assert_eq!(pins(&a),stored(&a),"pins equal the stored versions held");same_states(&a,&b,"live");
    compare(&mut a,&mut b,"live");
    for x in side.iter().chain(&back){a.import(x.clone(),None).unwrap();b.import(x.clone(),None).unwrap();}
    assert_eq!(a.chain.head,back.last().unwrap().hash());assert!(a.stats.reorgs>=2,"positive control: two reorgs");
    compare(&mut a,&mut b,"after the reorgs");assert_eq!(whole(&a),0);assert_eq!(pins(&a),stored(&a),"after the reorgs: no pin leaked");same_states(&a,&b,"after the reorgs");
    assert_eq!((a.chain.rebuilds>0,a.stats.reorgs),(b.chain.rebuilds>0,b.stats.reorgs),"the same kind of work");
    drop(a);drop(b);let mut a=Node::open(ca(),admitted(&cfg(&["a","b","c"]))).unwrap();let mut b=Node::open(cb(),admitted(&cfg(&["a","b","c"]))).unwrap();
    compare(&mut a,&mut b,"after reopen");assert_eq!(whole(&a),0);assert_eq!(pins(&a),stored(&a),"after reopen");same_states(&a,&b,"after reopen");
    for d in [da,db]{let _=std::fs::remove_dir_all(&d);}
}

/// C-1 S3a at the node: every state query (balance, nonce, code, storage, eth_call) at every height and
/// tag answers as on a node that keeps every retained state full (the behaviour before S3a): the
/// retained heights from materialized states, the older ones -32017 on both. Live, after a reorg and after
/// reopening, with the same rebuild and restore work.
#[test] fn s3a_state_queries_answer_as_with_full_states(){
    use pocold::node::{Config,Limits,Node};
    let mk=|tag:&str,full:bool|{let dir=std::env::temp_dir().join(format!("pocold-s3a-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
        let d=dir.clone();let t=tag.to_string();let conf=move||Config{dir:d.clone(),name:t.clone(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Limits{full_states:full,..Default::default()}};
        (Node::open(conf(),admitted(&cfg(&["a","b","c"]))).unwrap(),dir,conf)};
    let (mut a,da,ca)=mk("diff",false);let (mut b,db,cb)=mk("full",true);
    let (mut h,blocks,p)=s1_chain();
    // A 6-block reorg near the tip, then the old branch again: both import paths run.
    let fork=h.chain.get(&p).unwrap().parent.unwrap();let fork=h.chain.get(&fork).unwrap().parent.unwrap();
    let mut h2=net();for x in &blocks{import(&mut h2,x);}
    let bob=devnet::account_key("bob");
    let mut side=vec![signed(&h2,fork,Body{txs:vec![pay(&bob,0,3)],evidence:vec![]},0)];import(&mut h2,&side[0]);
    side.extend(heavier(&mut h2,side[0].hash(),h.chain.get(&p).unwrap().total_work,2));
    let back=heavier(&mut h,p,h2.chain.head().total_work,1);
    let alice=devnet::account_key("alice").address();
    let accounts:Vec<Address>=std::iter::once(alice).chain([bob.address(),Address::ZERO]).chain((0..50u64).map(|k|alice.create(k))).collect();
    let full=|n:&mut Node,m:&str,p:serde_json::Value|{let body=json!({"jsonrpc":"2.0","id":1,"method":m,"params":p}).to_string();let (t,_r)=std::sync::mpsc::channel();
        let r=pocold::rpc::answer(n,&pocold::rpc::Request::local(body.into_bytes(),t,std::time::Instant::now()+std::time::Duration::from_secs(120),false));
        let v:serde_json::Value=serde_json::from_str(&r.body).unwrap();if v.get("error").is_some(){json!({"error":v["error"]["code"]})}else{json!({"result":v["result"]})}};
    let answers=|n:&mut Node|->Vec<serde_json::Value>{let top=n.chain.head().height;let mut v=vec![];
        let tags:Vec<serde_json::Value>=(0..=top).map(|x|json!(format!("{x:#x}"))).chain(["latest","safe","finalized","earliest"].map(|t|json!(t))).collect();
        for t in &tags{for a in &accounts{let a=format!("{a:#x}");
            for m in ["eth_getBalance","eth_getTransactionCount","eth_getCode"]{v.push(full(n,m,json!([a,t])));}
            v.push(full(n,"eth_getStorageAt",json!([a,"0x0",t])));}
            v.push(full(n,"eth_call",json!([{"to":format!("{:#x}",alice.create(1)),"data":"0x"},t])));}
        v};
    let compare=|a:&mut Node,b:&mut Node,what:&str|{let (x,y)=(answers(a),answers(b));assert_eq!(x.len(),y.len());
        let ok=x.iter().filter(|r|r.get("result").is_some()).count();let pruned=x.iter().filter(|r|r.get("error")==Some(&json!(-32017))).count();
        assert_eq!(x,y,"{what}");assert!(ok>1000&&pruned>100,"positive control ({what}): {ok} answers, {pruned} outside the window");};
    for x in &blocks{a.import(x.clone(),None).unwrap();b.import(x.clone(),None).unwrap();}
    let full_states=|n:&Node|n.chain.blocks.values().filter(|e|e.full&&!e.pruned).count();
    assert!(full_states(&a)<=4&&full_states(&b)>30,"positive control: S3a holds {} full states, the reference {}",full_states(&a),full_states(&b));
    compare(&mut a,&mut b,"live");
    for x in side.iter().chain(&back){a.import(x.clone(),None).unwrap();b.import(x.clone(),None).unwrap();}
    assert_eq!(a.chain.head,back.last().unwrap().hash());assert!(a.stats.reorgs>=2,"positive control: two reorgs");
    compare(&mut a,&mut b,"after the reorgs");
    assert_eq!((a.chain.rebuilds,a.chain.restore_reexecs,a.stats.restored),(b.chain.rebuilds,b.chain.restore_reexecs,b.stats.restored),"the same rebuild and restore work");
    drop(a);drop(b);let mut a=Node::open(ca(),admitted(&cfg(&["a","b","c"]))).unwrap();let mut b=Node::open(cb(),admitted(&cfg(&["a","b","c"]))).unwrap();
    compare(&mut a,&mut b,"after reopen");
    for d in [da,db]{let _=std::fs::remove_dir_all(&d);}
}

/// C-1 S4 acceptance: canonical entries deeper than `slim_depth` drop the header's heap fields, the
/// ParentResult and the nonce ranges, and are restored exactly from the journal record and history.redb.
/// A node slimming at depth 8 answers every block, transaction, receipt, log, header and block-bytes
/// query, and serves every P2P block, as a reference that never slims. Every slimmed entry restores to the
/// reference's header, ParentResult and ranges. A side block on a slimmed parent (a fork the depth would
/// not allow by default) is validated after the parent is restored, and the parent is slimmed again later.
/// The same holds after reopen.
#[test] fn s4_slimmed_entries_answer_as_full_ones_and_restore_exactly(){
    use pocold::node::{Config,Limits,Node};
    let mk=|tag:&str,depth:u64|{let dir=std::env::temp_dir().join(format!("pocold-s4-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
        let d=dir.clone();let t=tag.to_string();let conf=move||Config{dir:d.clone(),name:t.clone(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Limits{slim_depth:Some(depth),..Default::default()}};
        (Node::open(conf(),admitted(&cfg(&["a","b","c"]))).unwrap(),dir,conf)};
    let (mut a,da,ca)=mk("slim",8);let (mut b,db,_)=mk("ref",u64::MAX);
    let (_,blocks,p)=s1_chain();
    let txs:Vec<String>=blocks.iter().flat_map(|x|x.body.txs.iter().map(|t|format!("{}",revm::primitives::keccak256(t)))).collect();
    for n in [&mut a,&mut b]{for x in &blocks{n.import(x.clone(),None).unwrap();}assert_eq!(n.chain.head,p);}
    let views=|n:&mut Node|->Vec<serde_json::Value>{let top=n.chain.head().height;let mut v=chain_view(n);
        for t in &txs{v.push(rpc_local(n,"eth_getTransactionByHash",json!([t])));v.push(rpc_local(n,"eth_getTransactionReceipt",json!([t])));}
        v.push(rpc_local(n,"eth_getLogs",json!([{"fromBlock":"0x0","toBlock":format!("{top:#x}")}])));
        v.push(rpc_local(n,"pocol_getHeaders",json!(["0x1",format!("{top:#x}")])));
        for x in 0..=top{v.push(rpc_local(n,"pocol_getBlockRlp",json!([format!("{x:#x}")])));let hash=n.chain.canonical_at(x).unwrap().hash;
            v.push(rpc_local(n,"eth_getBlockByHash",json!([hex(&hash),true])));v.push(json!(n.block_msg(&hash).map(|m|format!("{m:?}"))));}
        v};
    let slims=|n:&Node|n.chain.blocks.values().filter(|e|e.slim).count();
    let exact=|a:&Node,b:&Node|{let mut k=0;for e in a.chain.blocks.values().filter(|e|e.slim){
        let r=a.restored_entry(&e.hash).unwrap();let f=b.chain.get(&e.hash).unwrap();let mut fh=f.header.clone();if r.offloaded{fh.encoded=Vec::new();}
        assert_eq!((&r.header,&r.result,&r.ranges),(&fh,&f.result,&f.ranges),"h={}: restored exactly",e.height);assert!(!r.ranges.is_empty()&&r.result.is_some());k+=1;}k};
    assert!(slims(&a)>80&&slims(&b)==0,"positive control: {} slimmed entries, the reference {}",slims(&a),slims(&b));
    let top=a.chain.head().height;let slim_h:Vec<u64>=(1..=top).filter(|h|a.chain.canonical_at(*h).unwrap().slim).collect();
    assert_eq!(slim_h,(1..top-8).collect::<Vec<_>>(),"exactly the canonical entries deeper than 8 are slimmed");
    let small=a.chain.blocks.values().filter(|e|e.slim).map(|e|e.header.ut_raw.len()+e.ranges.len()+e.result.as_ref().map_or(0,|r|r.shares.len())).sum::<usize>();assert_eq!(small,0);
    assert_eq!(exact(&a,&b),slims(&a));
    let want=views(&mut b);assert_eq!(views(&mut a),want,"slimmed entries answer as full ones");
    // A fork on a slimmed parent 20 blocks deep (allowed by R_keep, refused by the default depth): A
    // restores the parent to validate it, as B validates it without.
    let mut h=net();for x in &blocks{import(&mut h,x);}let fork=a.chain.canonical_at(a.chain.head().height-20).unwrap().hash;
    assert!(a.chain.get(&fork).unwrap().slim,"positive control: the fork point is slimmed");
    let side=fallback(&h,fork,1);
    a.import(side.clone(),None).unwrap();b.import(side.clone(),None).unwrap();
    assert!(a.stats.unslims>=1,"the slimmed parent was restored for the validation");
    assert_eq!(a.chain.get(&side.hash()).map(|e|e.hash),b.chain.get(&side.hash()).map(|e|e.hash));
    assert!(a.chain.get(&fork).unwrap().slim,"and slimmed again by the next retain");
    assert_eq!(views(&mut a),views(&mut b),"after the fork on a slimmed parent");
    // A fork 40 deep, below the K_hist cut: its parent's state is rebuilt by re-executing from genesis
    // through slimmed entries, each from its restored fields.
    let deep=a.chain.canonical_at(a.chain.head().height-40).unwrap().hash;assert!(a.chain.get(&deep).unwrap().slim&&a.chain.get(&deep).unwrap().pruned);
    let side2=fallback(&h,deep,1);let rb=a.chain.rebuilds;
    a.import(side2.clone(),None).unwrap();b.import(side2.clone(),None).unwrap();
    assert!(a.chain.rebuilds>rb,"positive control: a TrieRebuild through slimmed entries");assert!(a.chain.get(&side2.hash()).is_some());
    assert_eq!(views(&mut a),views(&mut b),"after the fork below the cut");
    drop(a);let mut r=Node::open(ca(),admitted(&cfg(&["a","b","c"]))).unwrap();
    assert!(slims(&r)>80);assert_eq!(exact(&r,&b),slims(&r));assert_eq!(views(&mut r),views(&mut b),"after reopen");drop(r);
    for d in [da,db]{let _=std::fs::remove_dir_all(&d);}
}

/// C-1 S4: type-2 evidence whose ancestor (the equivocating winner's block) is slimmed. A node slimming
/// at depth 3 restores that ancestor for the evidence walk and accepts the block with the same effect
/// (the winner tombstoned) as a node that never slims.
#[test] fn s4_evidence_against_a_slimmed_ancestor_restores_it_first(){
    use pocold::node::{Config,Limits,Node};
    let mut n=net();for _ in 0..3{let h=n.chain.head;let b=fallback(&n,h,0);import(&mut n,&b);}
    let mut seq:Vec<Block>=n.chain.canonical.iter().skip(1).map(|h|{let e=&n.chain.blocks[h];Block{header:e.header.clone(),body:e.body.clone()}}).collect();
    let head=n.chain.head;let b4=fallback(&n,head,0);import(&mut n,&b4);seq.push(b4.clone());
    let winner=n.chain.head().result.clone().unwrap().winner;let tid=template_id(&b4.header.ut_raw);
    let mut alt=b4.header.clone();alt.shares=vec![];reseal(&mut alt);
    let (root1,root2)=(share_root(&b4.header),share_root(&alt));assert_ne!(root1,root2);
    let sig2=n.keys[&winner].sign_hash(&B256::from(hashes::win_msg(&tid,b4.header.nonce,&root2)));
    let mut ev2=Vec::new();rlp::encode_bytes(&[2],&mut ev2);rlp::encode_bytes(&tid,&mut ev2);rlp::encode_bytes(&b4.header.nonce.to_be_bytes(),&mut ev2);
    rlp::encode_bytes(&root1,&mut ev2);rlp::encode_bytes(&b4.header.winner_sig,&mut ev2);rlp::encode_bytes(&root2,&mut ev2);rlp::encode_bytes(&sig2,&mut ev2);
    let mut e2=Vec::new();rlp::encode_list_payload(&ev2,&mut e2);
    // Blocks until b4 is 8 deep, then the first height whose drawn proposer is not the winner carries e2.
    loop{let h=n.chain.head;let p=n.chain.full_entry(&h).unwrap();let ctx=n.chain.child_ctx(&p,0).unwrap();
        if p.height>=12&&ctx.proposer!=winner{
            let body=Body{txs:vec![],evidence:vec![e2.clone()]};let effects=n.chain.evidence_effects(&p,&body,p.height+1).unwrap();
            let (hd,_,_)=n.chain.build_signed(&p,&n.keys[&ctx.proposer],ctx.window.lower(),body.clone(),effects).unwrap();
            let x=seal_block(&n,hd,body,0,0);import(&mut n,&x);seq.push(x);break}
        assert!(p.height<4+16,"no eligible proposer within E_win");let x=fallback(&n,h,0);import(&mut n,&x);seq.push(x);}
    let mk=|tag:&str,depth:u64|{let dir=std::env::temp_dir().join(format!("pocold-s4ev-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
        (Node::open(Config{dir:dir.clone(),name:tag.into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Limits{slim_depth:Some(depth),..Default::default()}},admitted(&cfg(&["a","b","c"]))).unwrap(),dir)};
    let (mut a,da)=mk("slim",3);let (mut b,db)=mk("ref",u64::MAX);
    for x in &seq[..seq.len()-1]{a.import(x.clone(),None).unwrap();b.import(x.clone(),None).unwrap();}
    assert!(a.chain.get(&b4.hash()).unwrap().slim,"positive control: the evidence's ancestor is slimmed");
    let before=a.stats.unslims;let last=seq.last().unwrap().clone();
    a.import(last.clone(),None).unwrap();b.import(last.clone(),None).unwrap();
    assert!(a.stats.unslims>before,"the slimmed ancestor was restored for the evidence walk");
    assert_eq!(a.chain.head,b.chain.head);assert_eq!(a.chain.head,last.hash());
    let r=apply::miner_record(&a.chain.head().state,winner).unwrap();assert!(r.tomb,"the equivocating winner is tombstoned");
    assert_eq!(apply::state_root(&a.chain.head().state).unwrap(),apply::state_root(&b.chain.head().state).unwrap());
    for d in [da,db]{let _=std::fs::remove_dir_all(&d);}
}

/// The S1 scenario chain: A1 (with a transaction) abandoned for the heavier B1, B2 from genesis, then 96
/// blocks with a payment and a LOG0-emitting create every 4th block. Returns the builder, every block in
/// import order and the tip.
fn s1_chain()->(Net,Vec<Block>,[u8;32]){
    let alice=devnet::account_key("alice");
    let log_tx=|nonce:u64|Tx{chain_id:777910,nonce,max_priority:U256::from(1u64),max_fee:U256::from(100_000_000_000u64),gas:200_000,to:None,value:U256::ZERO,data:vec![0x61,0x00,0x10,0x5f,0xa0,0x00].into()}.sign_1559(&alice);
    let mut h1=net();let g=h1.chain.head;let a1=branch(&mut h1,g,1,&[Body{txs:vec![pay(&alice,0,5)],evidence:vec![]}]);
    let mut h=net();let mut blocks=a1.clone();blocks.extend(branch(&mut h,g,2,&[]));
    let (mut nonce,mut p)=(0u64,blocks[2].hash());
    for i in 0..96{let body=if i%4==0{let t=vec![pay(&alice,nonce,1+i),log_tx(nonce+1)];nonce+=2;Body{txs:t,evidence:vec![]}}else{Body::default()};
        let x=branch(&mut h,p,1,&[body]);p=x[0].hash();blocks.extend(x);}
    (h,blocks,p)
}

/// Equal-work branches: at every height all of them use the same attempt, so their timestamps and
/// targets stay equal (a template without a solution moves all branches to the next attempt).
fn lockstep(nets:&mut [Net],firsts:&[Block],len:usize)->Vec<Vec<Block>>{
    let mut out:Vec<Vec<Block>>=firsts.iter().map(|b|vec![b.clone()]).collect();
    for (n,b) in nets.iter_mut().zip(firsts){import(n,b);}
    for _ in 1..len{
        let mut a=0u32;
        let next=loop{
            let got:Vec<Option<Block>>=nets.iter().zip(out.iter()).map(|(n,br)|{let p=n.chain.get(&br.last().unwrap().hash()).unwrap().clone();
                let (h,_,_)=n.chain.build_fallback(&p,a).unwrap();try_seal(n,h,Body::default(),a,0)}).collect();
            if got.iter().all(|x|x.is_some()){break got.into_iter().map(|x|x.unwrap()).collect::<Vec<_>>()}a+=1;
        };
        for ((n,br),b) in nets.iter_mut().zip(out.iter_mut()).zip(next){import(n,&b);n.chain.retain();br.push(b);}
    }
    out
}
/// D133 at depth (frozen test 3), through eviction and re-fetch (frozen test 4). Three equal-work
/// branches from genesis: X (fallback first), Y and Z (signed first, same timestamp). X is imported
/// first; Y, delivered by a session, wins the tie at its tip because a signed block precedes a fallback
/// at the fork point (D15/D133), a reorg `depth` deep; X is evicted down to the cap. Z ties Y with both
/// signed: Y keeps the head (smaller importSeq). X, extended by one block and re-fetched by a session,
/// becomes heavier and is adopted. The head equals the whole-path D133 reference after every step and
/// after restart.
fn d133_at_depth(depth:usize,rk:u64,cap:usize,tag:&str){
    let alice=devnet::account_key("alice");
    let mut nets=vec![net(),net(),net()];let g=nets[0].chain.head;
    let x0=fallback(&nets[0],g,0);
    let lower=nets[1].chain.child_ctx(&nets[1].chain.full_entry(&g).unwrap(),0).unwrap().window.lower();
    let off=x0.header.ts-lower;
    let y0=signed(&nets[1],g,Body::default(),off);let z0=signed(&nets[2],g,Body{txs:vec![pay(&alice,0,3)],evidence:vec![]},off);
    assert!(x0.header.ts==y0.header.ts&&y0.header.ts==z0.header.ts,"one timestamp at the fork point");
    let br=lockstep(&mut nets,&[x0,y0,z0],depth);let (x,y,z)=(&br[0],&br[1],&br[2]);
    let w=|n:&Net,b:&Block|n.chain.blocks[&b.hash()].total_work;
    assert!(w(&nets[0],x.last().unwrap())==w(&nets[1],y.last().unwrap())&&w(&nets[1],y.last().unwrap())==w(&nets[2],z.last().unwrap()),"equal work at the tips");
    let xe=fallback(&nets[0],x.last().unwrap().hash(),0);
    let (mut n,dir,conf)=rnode(tag,rk,cap);
    for b in x{n.import(b.clone(),None).unwrap();}assert_eq!(n.chain.head,x.last().unwrap().hash());
    for b in y{n.import_with(b.clone(),None,true).unwrap();}
    assert_eq!(n.chain.head,y.last().unwrap().hash(),"signed before fallback at the fork point: a reorg {depth} deep");assert_eq!(n.chain.head,best_of(&n.chain));
    n.chain.protect.clear();n.chain.retain();assert!(n.chain.evicted_total>=(depth-cap) as u64,"positive control: X was evicted ({})",n.chain.evicted_total);
    for b in z{n.import_with(b.clone(),None,true).unwrap();}
    assert_eq!(n.chain.head,y.last().unwrap().hash(),"both signed: the smaller importSeq (Y) keeps the head");assert_eq!(n.chain.head,best_of(&n.chain));
    n.chain.protect.clear();n.chain.retain();
    let before=n.stats.refetched;
    for b in x.iter().chain(std::iter::once(&xe)){n.import_with(b.clone(),None,true).unwrap_or_else(|e|panic!("re-fetch h={}: {e:?}",b.header.h));}
    assert!(n.stats.refetched>before,"positive control: evicted X blocks were re-fetched");
    assert_eq!(n.chain.head,xe.hash(),"X, now heavier, is adopted after re-fetch");assert_eq!(n.chain.head,best_of(&n.chain));assert!(n.chain.canonical_resident());
    n.chain.protect.clear();n.chain.retain();let view=chain_view(&mut n);drop(n);
    let mut r=reopen(&conf);assert_eq!(r.chain.head,xe.hash());assert_eq!(r.stats.replay_skipped,0);assert_eq!(r.chain.head,best_of(&r.chain));assert_eq!(chain_view(&mut r),view);
    println!("D133-DEPTH {}",json!({"depth":depth,"rKeep":rk,"cap":cap,"evicted":r.chain.evicted_total}));
    let _=std::fs::remove_dir_all(&dir);
}
#[test] fn d133_tie_break_at_depth_holds_through_eviction_and_refetch(){d133_at_depth(40,128,8,"d133")}
/// Deeper than the former 320-block cap, within R_keep (default). Release only; run with
/// `cargo test --release --test consensus -- --ignored d133_tie_break_at_depth_400`.
#[test] #[ignore="spec-depth test: run in release with --ignored"]
fn d133_tie_break_at_depth_400_holds_through_eviction_and_refetch(){d133_at_depth(400,pocold::chain::R_KEEP,16,"d133-400")}

fn rpc_tcp(port:u16,method:&str,params:serde_json::Value)->serde_json::Value{
    use std::io::{Read,Write};
    let Ok(mut s)=std::net::TcpStream::connect(("127.0.0.1",port)) else{return serde_json::Value::Null};
    let body=json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}).to_string();
    let _=write!(s,"POST / HTTP/1.1\r\nHost: x\r\nContent-Length: {}\r\n\r\n{body}",body.len());let mut o=String::new();let _=s.read_to_string(&mut o);
    o.split("\r\n\r\n").nth(1).and_then(|b|serde_json::from_str::<serde_json::Value>(b).ok()).map(|v|v["result"].clone()).unwrap_or(serde_json::Value::Null)
}
fn wait_for(secs:u64,f:&dyn Fn()->bool)->bool{let t=std::time::Instant::now();while t.elapsed()<std::time::Duration::from_secs(secs){if f(){return true}std::thread::sleep(std::time::Duration::from_millis(200));}f()}
/// consensus.md §10 / D118 re-fetch over the network (frozen tests 4 and 8). Node A held a lighter
/// branch and evicted it (side cap). Peer P serves that branch, now heavier, as its canonical chain.
/// A's sync session (Status work, locator) re-fetches the evicted blocks (solicited), adopts P's head
/// and restores the adopted blocks, so every canonical block and receipt A answers over RPC equals
/// P's. A third node C syncs the same chain from A alone, so A serves its restored history.
#[test] fn a_sync_session_refetches_an_evicted_branch_and_serves_it(){
    use pocold::node::{Config,Limits,Node};
    let port=||std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let alice=devnet::account_key("alice");let c=cfg(&["a","b","c"]);
    let mut a=net();let g=a.chain.head;let ours=grow(&mut a,g,20,&|n,p|signed(n,p,Body::default(),0));
    let noise:Vec<Block>=ours.iter().skip(6).take(10).map(|x|fallback(&a,x.hash(),1)).collect();
    let mut b=net();let first=signed(&b,g,Body{txs:vec![pay(&alice,0,5)],evidence:vec![]},0);import(&mut b,&first);
    let mut theirs=vec![first.clone()];theirs.extend(heavier(&mut b,first.hash(),a.chain.head().total_work,3));
    let lim=Limits{r_keep:64,side_cap:6,..Default::default()};
    let (pa,pp,pc,ra,rp,rc)=(port(),port(),port(),port(),port(),port());let tag=std::process::id();
    let dir=|x:&str|{let d=std::env::temp_dir().join(format!("pocold-refetch-{x}-{tag}"));let _=std::fs::remove_dir_all(&d);d};
    let addr=|p:u16|->std::net::SocketAddr{format!("127.0.0.1:{p}").parse().unwrap()};
    let conf=|name:&str,listen:u16,peers:Vec<std::net::SocketAddr>,rpc:u16|Config{dir:dir(name),name:name.into(),miner:None,listen:Some(addr(listen)),peers,rpc:Some(addr(rpc)),stop_at:None,log:false,limits:lim.clone()};
    let mut na=Node::open(conf("a",pa,vec![addr(pp)],ra),admitted(&c)).unwrap();
    for x in &ours{na.import(x.clone(),None).unwrap();}
    for x in &theirs[..8]{let _=na.import(x.clone(),None);}
    for x in &noise{let _=na.import(x.clone(),None);}
    assert!(theirs[..8].iter().any(|x|!na.chain.blocks.contains_key(&x.hash())),"positive control: A evicted part of the lighter branch (cap {})",lim.side_cap);
    assert_eq!(na.chain.head,ours.last().unwrap().hash());
    let mut np=Node::open(conf("p",pp,vec![],rp),admitted(&c)).unwrap();
    for x in &theirs{np.import(x.clone(),None).unwrap();}let target=json!(hex(&np.chain.head));
    np.start_network().unwrap();std::thread::spawn(move||np.run());
    na.start_network().unwrap();std::thread::spawn(move||na.run());
    let head=|p:u16|rpc_tcp(p,"eth_getBlockByNumber",json!(["latest",false]))["hash"].clone();
    assert!(wait_for(300,&||head(ra)==target),"A adopts P's head: {} vs {target}",head(ra));
    let st=rpc_tcp(ra,"pocol_status",json!([]));
    assert!(st["stats"]["refetched"].as_u64().unwrap_or(0)>0&&st["stats"]["restored"].as_u64().unwrap_or(0)>0&&st["stats"]["syncSessions"].as_u64().unwrap_or(0)>0,
        "positive control: a session re-fetched evicted blocks and the adopted ones were restored: {}",st["stats"]);
    let top=theirs.last().unwrap().header.h;
    let view=|p:u16|{let mut v=vec![];for h in 0..=top{let b=rpc_tcp(p,"eth_getBlockByNumber",json!([format!("{h:#x}"),true]));
        for t in b["transactions"].as_array().cloned().unwrap_or_default(){v.push(rpc_tcp(p,"eth_getTransactionReceipt",json!([t["hash"]])));}v.push(b);}v};
    let pv=view(rp);assert!(pv.iter().all(|x|!x.is_null()));
    assert_eq!(view(ra),pv,"A answers P's chain: blocks, transactions, receipts");
    let mut nc=Node::open(conf("c",pc,vec![addr(pa)],rc),admitted(&c)).unwrap();nc.start_network().unwrap();std::thread::spawn(move||nc.run());
    assert!(wait_for(300,&||head(rc)==target),"C syncs from A");assert_eq!(view(rc),pv,"C, served by A, answers the same chain");
    println!("REFETCH-NET {}",json!({"ourBlocks":ours.len(),"theirBlocks":theirs.len(),"sideCap":lim.side_cap,"stats":st["stats"],"retention":st["retention"]}));
}
/// eth_call / eth_estimateGas on a block whose state is pruned (outside the K_hist window, or a block
/// adopted after its state was pruned as a side block) answer -32017 like the state reads (consensus.md
/// §10), never a simulation on an empty state.
#[test] fn a_call_on_a_pruned_state_is_32017_not_an_empty_state_simulation(){
    let mut h=net();let g=h.chain.head;let blocks=grow(&mut h,g,40,&|n,p|fallback(n,p,0));
    let (mut n,dir,_)=rnode("call",64,16);for x in &blocks{n.import(x.clone(),None).unwrap();}
    assert!(n.chain.canonical_at(1).unwrap().pruned,"block 1 is outside the K_hist window");
    let body=json!({"jsonrpc":"2.0","id":1,"method":"eth_call","params":[{"to":format!("{:#x}",devnet::account_key("bob").address())},"0x1"]}).to_string();let (t,_r)=std::sync::mpsc::channel();
    let a=pocold::rpc::answer(&mut n,&pocold::rpc::Request::local(body.into_bytes(),t,std::time::Instant::now()+std::time::Duration::from_secs(60),false));
    let v:serde_json::Value=serde_json::from_str(&a.body).unwrap();assert_eq!(v["error"]["code"],-32017,"{v}");
    assert!(!rpc_local(&mut n,"eth_call",json!([{"to":format!("{:#x}",devnet::account_key("bob").address())},"latest"])).is_null(),"the head state answers");
    let _=std::fs::remove_dir_all(&dir);
}
/// A session's protection covers only its non-canonical blocks: a linear catch-up far longer than
/// protect_max (2·R_keep, here 16) completes through continuation rounds, since each block it
/// delivers extends the canonical chain and needs no protection. The catch-up is longer than one
/// GetFrom answer (at most SYNC_COUNT blocks), so it needs a second round whatever the timing; with
/// 60 blocks only the server's queue (offer stops at half of queue_bytes_max) could cut a round short,
/// which depends on how fast its writer drains, and once one round carried all 60 (syncRounds 1).
#[test] fn a_long_catch_up_is_not_bounded_by_the_protected_branch_size(){
    use pocold::node::{Config,Limits,Node};
    let port=||std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let len=pocold::node::SYNC_COUNT as usize+8;
    let c=cfg(&["a","b","c"]);let mut h=net();let g=h.chain.head;let blocks=grow(&mut h,g,len,&|n,p|fallback(n,p,0));
    let mut lim=Limits{r_keep:8,side_cap:8,..Default::default()};lim.net.queue_bytes_max=32*1024;
    let (pa,pb,ra)=(port(),port(),port());let tag=std::process::id();
    let dir=|x:&str|{let d=std::env::temp_dir().join(format!("pocold-catchup-{x}-{tag}"));let _=std::fs::remove_dir_all(&d);d};
    let addr=|p:u16|->std::net::SocketAddr{format!("127.0.0.1:{p}").parse().unwrap()};
    let mut np=Node::open(Config{dir:dir("p"),name:"p".into(),miner:None,listen:Some(addr(pb)),peers:vec![],rpc:None,stop_at:None,log:false,limits:lim.clone()},admitted(&c)).unwrap();
    for x in &blocks{np.import(x.clone(),None).unwrap();}let target=json!(hex(&np.chain.head));
    np.start_network().unwrap();std::thread::spawn(move||np.run());
    let mut na=Node::open(Config{dir:dir("a"),name:"a".into(),miner:None,listen:Some(addr(pa)),peers:vec![addr(pb)],rpc:Some(addr(ra)),stop_at:None,log:false,limits:lim},admitted(&c)).unwrap();
    na.start_network().unwrap();std::thread::spawn(move||na.run());
    let head=||rpc_tcp(ra,"eth_getBlockByNumber",json!(["latest",false]))["hash"].clone();
    assert!(wait_for(300,&||head()==target),"A catches up {len} blocks with protect_max 16: at {}",head());
    let st=rpc_tcp(ra,"pocol_status",json!([]));assert!(st["stats"]["syncRounds"].as_u64().unwrap_or(0)>1,"positive control: the catch-up was continued past its first round: {}",st["stats"]);
}
/// R21's K_hist window prunes canonical states only; non-canonical states are bounded by side_max.
/// A competing branch below the window therefore imports with its tip's state kept, so a deep
/// adoption needs no rebuild per block (pruning side states too re-executed each block's ancestry
/// from genesis: quadratic work at depth R_keep).
#[test] fn a_side_branch_below_the_k_hist_window_imports_without_rebuilding_each_parent(){
    let mut a=net();let g=a.chain.head;let ours=grow(&mut a,g,24,&|n,p|signed(n,p,Body::default(),0));
    let mut b=net();let theirs=grow(&mut b,g,16,&|n,p|fallback(n,p,1));
    let (mut n,dir,_)=rnode("kwin",64,64);n.chain.k_hist=4;
    for x in &ours{n.import(x.clone(),None).unwrap();}
    let before=n.chain.rebuilds;
    for x in &theirs{n.import(x.clone(),None).unwrap();}
    assert!(theirs.iter().all(|x|!n.chain.is_canonical(&x.hash())),"the branch stays lighter (side)");
    assert!(n.chain.rebuilds-before<=1,"linear import: {} re-executions for {} side blocks",n.chain.rebuilds-before,theirs.len());
    let (_,with_state)=n.chain.side_counts();assert!(with_state<=n.chain.spec.local().side_max as usize);
    let _=std::fs::remove_dir_all(&dir);
}

/// A peer cannot hold the sync slot with a false Status: one announcing enormous work that answers with
/// copies of our own blocks (and re-announces new heads) makes no progress after its first pass, so its
/// session ends after SESSION_STALL_MS and the round-robin reaches the honest heavier peer, whose branch
/// is adopted. Local test peers only.
#[test] fn a_peer_with_a_false_status_cannot_hold_the_sync_session(){
    use pocold::{node::{Config,Limits,Node},net::Msg};use std::io::{Read,Write};
    let port=||std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let c=cfg(&["a","b","c"]);let mut a=net();let g=a.chain.head;let ours=grow(&mut a,g,20,&|n,p|signed(n,p,Body::default(),0));
    let mut b=net();let theirs=heavier(&mut b,g,a.chain.head().total_work,2);
    let lim=Limits{r_keep:64,side_cap:16,..Default::default()};
    let (pa,pp,ra)=(port(),port(),port());let tag=std::process::id();
    let dir=|x:&str|{let d=std::env::temp_dir().join(format!("pocold-liar-{x}-{tag}"));let _=std::fs::remove_dir_all(&d);d};
    let addr=|p:u16|->std::net::SocketAddr{format!("127.0.0.1:{p}").parse().unwrap()};
    let mut na=Node::open(Config{dir:dir("a"),name:"a".into(),miner:None,listen:Some(addr(pa)),peers:vec![],rpc:Some(addr(ra)),stop_at:None,log:false,limits:lim.clone()},admitted(&c)).unwrap();
    for x in &ours{na.import(x.clone(),None).unwrap();}
    let (chain_id,genesis)=(na.chain.spec.chain_id(),na.chain.spec.genesis_hash());
    na.start_network().unwrap();std::thread::spawn(move||na.run());
    // The false peer: Hello with maximal work and an unknown head, then copies of A's own blocks forever.
    let frame=|m:&Msg|{let e=m.encode();let mut o=(e.len() as u32).to_be_bytes().to_vec();o.extend(e);o};
    let mut x=std::net::TcpStream::connect(addr(pa)).unwrap();let mut xr=x.try_clone().unwrap();
    std::thread::spawn(move||{let mut buf=[0u8;65536];while xr.read(&mut buf).map_or(false,|n|n>0){}});
    let copies:Vec<Vec<u8>>=ours.iter().map(|y|frame(&Msg::Block{header:y.header.encoded.clone(),body:y.body.encode()})).collect();
    std::thread::spawn(move||{let mut k=0u64;loop{
        if k%20==0{let head=[(k/20) as u8;32];if x.write_all(&frame(&Msg::Hello{chain_id,genesis,head,height:1_000_000,work:[0xff;32],node:"x".into(),protocol:pocold::identity::PROTOCOL_VERSION,compatibility:pocold::identity::compatibility()})).is_err(){break}}
        if x.write_all(&copies[(k as usize)%copies.len()]).is_err(){break}k+=1;std::thread::sleep(std::time::Duration::from_millis(50));}});
    let head=|p:u16|rpc_tcp(p,"eth_getBlockByNumber",json!(["latest",false]))["hash"].clone();
    assert!(wait_for(20,&||rpc_tcp(ra,"pocol_status",json!([]))["stats"]["syncSessions"].as_u64().unwrap_or(0)>=1),"the false peer's first Hello starts a session");
    let mut np=Node::open(Config{dir:dir("p"),name:"p".into(),miner:None,listen:Some(addr(pp)),peers:vec![addr(pa)],rpc:None,stop_at:None,log:false,limits:lim},admitted(&c)).unwrap();
    for y in &theirs{np.import(y.clone(),None).unwrap();}let target=json!(hex(&np.chain.head));
    np.start_network().unwrap();std::thread::spawn(move||np.run());
    assert!(wait_for(120,&||head(ra)==target),"A adopts the honest heavier peer's head despite the false peer: {} vs {target}",head(ra));
    let st=rpc_tcp(ra,"pocol_status",json!([]));assert!(st["stats"]["syncSessions"].as_u64().unwrap_or(0)>=2,"{}",st["stats"]);
}
/// 2026-10-28 (seeded fault LF5): a journal record that cannot be read back while a block is imported is the node's
/// own failure, never a validity verdict. A node holds 40 blocks, so the oldest are pruned and offloaded (their
/// bodies are read from the journal on demand). A child process (the node) flips a byte inside block 1's journal
/// record, then imports a fork child of block 2, whose validation rebuilds block 2's state from genesis and so
/// reads block 1's record. The read fails its digest check: the node stops (exit 70) and marks nothing invalid.
/// Before the fix the failed read was a `Reject::Block`, returned as a verdict on the fork child.
#[test] fn a_journal_read_that_fails_in_an_import_stops_the_node(){
    let exe=std::env::current_exe().unwrap();
    let out=std::process::Command::new(&exe).args(["journal_read_fault_child","--exact","--ignored","--nocapture","--test-threads=1"]).env("POCOLD_JREAD_CHILD","1").output().unwrap();
    let text=format!("{}{}",String::from_utf8_lossy(&out.stdout),String::from_utf8_lossy(&out.stderr));
    println!("JREAD (exit {:?}): {}",out.status.code(),text.lines().filter(|l|l.contains("JREAD-CHILD")||l.contains("FATAL")).collect::<Vec<_>>().join(" | "));
    assert!(text.contains("JREAD-CHILD corrupted"),"positive control: the child corrupted the record: {text}");
    assert_eq!(out.status.code(),Some(70),"the node stops with exit 70: {text}");
    assert!(text.contains("FATAL local storage: journal read_at")&&!text.contains("JREAD-CHILD imported"),"a local failure, not a verdict: {text}");
}
#[test] #[ignore="the child process of a_journal_read_that_fails_in_an_import_stops_the_node"]
fn journal_read_fault_child(){
    use std::io::{Read,Seek,SeekFrom,Write};
    if std::env::var("POCOLD_JREAD_CHILD").is_err(){return}
    let mut h=net();let g=h.chain.head;let blocks=grow(&mut h,g,40,&|n,p|fallback(n,p,0));
    let (mut n,dir,_)=rnode("jread",64,16);for x in &blocks{n.import(x.clone(),None).unwrap();}
    let e1=n.chain.get(&blocks[0].hash()).unwrap().clone();
    assert!(n.chain.canonical_at(2).unwrap().pruned&&e1.offloaded,"positive control: block 2's state is pruned and block 1 is offloaded");
    // Attempt 1: attempt 0 on block 2 is block 3 itself.
    h.chain.ensure_state(&blocks[1].hash()).unwrap();let fork=fallback(&h,blocks[1].hash(),1);
    let path=dir.join("blocks.journal");let off=e1.body_off.unwrap()+60;
    {let mut f=std::fs::OpenOptions::new().read(true).write(true).open(&path).unwrap();f.seek(SeekFrom::Start(off)).unwrap();let mut b=[0u8;1];f.read_exact(&mut b).unwrap();
     f.seek(SeekFrom::Start(off)).unwrap();f.write_all(&[b[0]^0x40]).unwrap();f.sync_all().unwrap();}
    println!("\nJREAD-CHILD corrupted block 1's record at byte {off}");std::io::stdout().flush().unwrap();
    let r=n.import(fork,None);
    println!("\nJREAD-CHILD imported the fork child: {r:?}; {} blocks marked invalid",n.chain.invalid.len());std::io::stdout().flush().unwrap();
}
/// D133 at a partition's heal (2026-10-28), deterministic: three nodes over TCP, none of which mines, so the
/// work each side measured before the heal is the work it has at the heal, and nothing can change it while
/// the nodes reconnect. During the partition a and b hold one branch from a common prefix and c another.
/// The losing branch is the taller one, fallback blocks at a late attempt; the winning one is shorter,
/// signed blocks until it has strictly more total work, so the outcome follows total work, not height.
/// Healed (a and b meshed, c connected to both), the three end on the winner's tip measured before the
/// heal, the losing side's nodes each reorganized onto it and moved down in height, the winners never
/// reorganized, and every canonical block on every node is the winning branch's. Seeded faults show the
/// test fails for a node that takes the lighter branch, that never reorganizes, or that never syncs.
fn d133_heal(winner_is_c:bool)->serde_json::Value{
    use pocold::node::{Config,Limits,Node};
    let port=||std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port();
    let addr=|p:u16|->std::net::SocketAddr{format!("127.0.0.1:{p}").parse().unwrap()};
    let c=cfg(&["a","b","c"]);let mut hw=net();let g=hw.chain.head;let prefix=branch(&mut hw,g,4,&[]);
    let fork=prefix.last().unwrap().hash();let mut hl=net();for x in &prefix{import(&mut hl,x);}
    // The loser: 14 fallback blocks at attempt 3 (the latest timestamps, the easiest targets).
    let mut lose=vec![];let mut p=fork;for _ in 0..14{let x=fallback(&hl,p,3);import(&mut hl,&x);hl.chain.retain();p=x.hash();lose.push(x);}
    let (lw,lh)=(hl.chain.get(&p).unwrap().total_work,hl.chain.get(&p).unwrap().height);
    // The winner: signed blocks (the earliest timestamps) until its total work is strictly more.
    let mut win=vec![];let mut q=fork;while hw.chain.get(&q).map_or(true,|e|e.total_work<=lw){let x=signed(&hw,q,Body::default(),0);import(&mut hw,&x);hw.chain.retain();q=x.hash();win.push(x);}
    let (ww,wh)=(hw.chain.get(&q).unwrap().total_work,hw.chain.get(&q).unwrap().height);
    assert!(ww>lw&&wh<lh,"positive control: the winning branch is heavier ({ww} > {lw}) and shorter ({wh} < {lh})");
    let (ab_blocks,c_blocks)=if winner_is_c{(&lose,&win)}else{(&win,&lose)};
    let (pa,pb,pc,ra,rb,rc)=(port(),port(),port(),port(),port(),port());let tag=format!("{}-{}",if winner_is_c{"c"}else{"ab"},std::process::id());
    let dir=|x:&str|{let d=std::env::temp_dir().join(format!("pocold-heal-{x}-{tag}"));let _=std::fs::remove_dir_all(&d);d};
    let conf=|name:&str,listen:u16,peers:Vec<std::net::SocketAddr>,rpc:u16|Config{dir:dir(name),name:name.into(),miner:None,listen:Some(addr(listen)),peers,rpc:Some(addr(rpc)),stop_at:None,log:false,limits:Limits::default()};
    // The partition: each side's blocks imported while its nodes are apart; then each head is measured.
    let mut na=Node::open(conf("a",pa,vec![],ra),admitted(&c)).unwrap();let mut nb=Node::open(conf("b",pb,vec![addr(pa)],rb),admitted(&c)).unwrap();
    let mut nc=Node::open(conf("c",pc,vec![addr(pa),addr(pb)],rc),admitted(&c)).unwrap();
    for x in prefix.iter().chain(ab_blocks.iter()){na.import(x.clone(),None).unwrap();nb.import(x.clone(),None).unwrap();}
    for x in prefix.iter().chain(c_blocks.iter()){nc.import(x.clone(),None).unwrap();}
    let measured=[(na.chain.head,na.chain.head().total_work,na.chain.head().height),(nb.chain.head,nb.chain.head().total_work,nb.chain.head().height),(nc.chain.head,nc.chain.head().total_work,nc.chain.head().height)];
    let tip=if winner_is_c{measured[2]}else{measured[0]};
    assert!(measured[0]==measured[1]&&measured[0].0!=measured[2].0,"positive control: a and b agree, c differs");
    assert_eq!(tip,(q,ww,wh),"positive control: the winning side's head is the winning branch's tip");
    // The heal. No node has a miner, so no block is made after the measurement.
    for n in [&mut na,&mut nb,&mut nc]{n.start_network().unwrap();}
    for n in [na,nb,nc]{std::thread::Builder::new().stack_size(64<<20).spawn(move||{let mut n=n;n.run()}).unwrap();}
    let status=|r:u16|rpc_tcp(r,"pocol_status",json!([]));let want=json!(hex(&tip.0));
    let converged=wait_for(120,&||[ra,rb,rc].iter().all(|r|status(*r)["head"]["hash"]==want));
    let st:Vec<serde_json::Value>=[ra,rb,rc].iter().map(|r|status(*r)).collect();
    let summary=json!({"winner":if winner_is_c{"c"}else{"a/b"},"winnerTip":{"height":wh,"work":ww.to_string()},"loserTip":{"height":lh,"work":lw.to_string()},
        "after":st.iter().map(|x|json!({"head":x["head"]["hash"],"height":x["head"]["height"],"reorgs":x["stats"]["reorgs"],"syncSessions":x["stats"]["syncSessions"]})).collect::<Vec<_>>()});
    println!("D133-HEAL {summary}");
    assert!(converged,"the three nodes end on the heavier branch's tip measured before the heal: {summary}");
    let reorgs:Vec<u64>=st.iter().map(|x|x["stats"]["reorgs"].as_u64().unwrap()).collect();
    let losers:Vec<usize>=if winner_is_c{vec![0,1]}else{vec![2]};
    for i in 0..3{if losers.contains(&i){assert!(reorgs[i]>=1,"node {i} (losing side) reorganized: {summary}")}else{assert_eq!(reorgs[i],0,"node {i} (winning side) never reorganized: {summary}")}
        assert_eq!(st[i]["head"]["height"].as_u64(),Some(wh),"node {i} is at the winner's height ({wh}), below the loser's ({lh}): {summary}");}
    let canon:Vec<String>=prefix.iter().chain(win.iter()).map(|x|hex(&x.hash())).collect();
    for r in [ra,rb,rc]{for (h,want) in canon.iter().enumerate(){assert_eq!(rpc_tcp(r,"eth_getBlockByNumber",json!([format!("{:#x}",h+1),false]))["hash"],json!(want),"height {}: the canonical block is the winning branch's",h+1);}}
    summary
}
#[test] fn d133_heal_c_reorganizes_onto_the_heavier_a_b_branch(){d133_heal(false);}
#[test] fn d133_heal_a_and_b_reorganize_onto_the_heavier_c_branch(){d133_heal(true);}
/// Replay never runs with a smaller R_keep than live used: the largest value is kept beside the
/// journal (retention.rkeep). Reopening with a smaller configured R_keep still replays a branch adopted
/// at a depth the smaller window would have cut, to the same head with no record skipped.
#[test] fn replay_uses_the_largest_r_keep_the_data_directory_has_used(){
    use pocold::node::{Config,Limits,Node};
    let mut a=net();let g=a.chain.head;let ours=grow(&mut a,g,40,&|n,p|fallback(n,p,1));
    let mut b=net();let theirs=heavier(&mut b,g,a.chain.head().total_work,1);
    let dir=std::env::temp_dir().join(format!("pocold-redb-rkmax-{}",std::process::id()));let _=std::fs::remove_dir_all(&dir);
    let conf=|rk:u64|Config{dir:dir.clone(),name:"rk".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Limits{r_keep:rk,side_cap:64,..Default::default()}};
    let mut n=Node::open(conf(64),admitted(&cfg(&["a","b","c"]))).unwrap();
    for x in ours.iter().chain(theirs.iter()){n.import(x.clone(),None).unwrap();}
    let tip=theirs.last().unwrap().hash();assert_eq!(n.chain.head,tip,"adopted at depth 40 with R_keep 64");drop(n);
    let r=Node::open(conf(24),admitted(&cfg(&["a","b","c"]))).unwrap();
    assert_eq!(r.stats.replay_skipped,0,"replay used R_keep 64, not the configured 24");assert_eq!(r.chain.head,tip);assert_eq!(r.chain.r_keep,24,"the configured value applies after replay");
    assert_eq!(std::fs::read_to_string(dir.join("retention.rkeep")).unwrap().trim(),"64");
    let _=std::fs::remove_dir_all(&dir);
}
/// A session block that arrives before its parent waits as an orphan and, when the parent arrives, is
/// imported with the parent's solicited status: protected from the side cap (here 0) like the rest of
/// the session's branch.
#[test] fn a_session_block_arriving_before_its_parent_stays_protected(){
    let mut a=net();let g=a.chain.head;let ours=grow(&mut a,g,6,&|n,p|signed(n,p,Body::default(),0));
    let mut b=net();let theirs=grow(&mut b,g,3,&|n,p|fallback(n,p,1));
    let (mut n,dir,_)=rnode("orphan",64,0);for x in &ours{n.import(x.clone(),None).unwrap();}
    assert!(matches!(n.import_with(theirs[1].clone(),None,true),Err(Reject::UnknownParent)),"the child arrives first");
    n.import_with(theirs[0].clone(),None,true).unwrap();
    for x in &theirs[..2]{assert!(n.chain.blocks.contains_key(&x.hash())&&n.chain.protect.contains(&x.hash()),"h={} is stored and protected",x.header.h);}
    assert_eq!(unprotected(&n),0);
    let _=std::fs::remove_dir_all(&dir);
}
/// The R_keep boundary in retain(): a side block imported inside the window stays while it is less
/// than R_keep below the head and is removed (with its descendants) once it is exactly R_keep below.
/// Independent of the import-time refusal of new blocks below the window.
#[test] fn the_r_keep_boundary_removes_a_side_block_exactly_r_keep_below_the_head(){
    let rk=8u64;let mut a=net();let g=a.chain.head;let ours=grow(&mut a,g,rk as usize+1,&|n,p|signed(n,p,Body::default(),0));
    let side=fallback(&a,g,1);
    let (mut n,dir,_)=rnode("boundary",rk,64);n.import(ours[0].clone(),None).unwrap();n.import(side.clone(),None).unwrap();
    assert!(!n.chain.is_canonical(&side.hash()),"a side block at height 1");
    for x in &ours[1..rk as usize]{n.import(x.clone(),None).unwrap();}
    assert_eq!(n.chain.head().height,rk);assert!(n.chain.blocks.contains_key(&side.hash()),"R_keep-1 below the head: kept");
    n.import(ours[rk as usize].clone(),None).unwrap();
    assert!(!n.chain.blocks.contains_key(&side.hash()),"exactly R_keep below the head: removed by retention");
    assert!(!n.chain.invalid.contains_key(&side.hash()),"removed locally, never invalid");
    let _=std::fs::remove_dir_all(&dir);
}

// ---------------- C-1 E4.4 (decision D-6): local checkpoints ----------------
/// The blocks the E4.4 tests import, in order: the S1 chain (99 blocks with payments and logs), a side
/// branch 2 below its tip and a heavier branch back from the tip (two reorgs). Built once per process.
fn e44_blocks()->&'static Vec<Block>{
    static B:std::sync::OnceLock<Vec<Block>>=std::sync::OnceLock::new();
    B.get_or_init(||{
        let (mut h,blocks,p)=s1_chain();
        let fork=h.chain.get(&p).unwrap().parent.unwrap();let fork=h.chain.get(&fork).unwrap().parent.unwrap();
        let mut h2=net();for x in &blocks{import(&mut h2,x);}
        let bob=devnet::account_key("bob");
        let mut side=vec![signed(&h2,fork,Body{txs:vec![pay(&bob,0,3)],evidence:vec![]},0)];import(&mut h2,&side[0]);
        side.extend(heavier(&mut h2,side[0].hash(),h.chain.get(&p).unwrap().total_work,2));
        let back=heavier(&mut h,p,h2.chain.head().total_work,1);
        blocks.into_iter().chain(side).chain(back).collect()})
}
fn e44_conf(dir:&std::path::Path,every:u64)->impl Fn()->pocold::node::Config{e44_conf_with(dir,every,|_|{})}
fn e44_conf_with(dir:&std::path::Path,every:u64,f:fn(&mut pocold::node::Limits))->impl Fn()->pocold::node::Config{
    use pocold::node::{Config,Limits};let d=dir.to_path_buf();
    move||{let mut l=Limits{state_store:true,checkpoint_every:every,..Default::default()};f(&mut l);Config{dir:d.clone(),name:"e44".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:true,limits:l}}
}
fn e44_dir(tag:&str)->std::path::PathBuf{let d=std::env::temp_dir().join(format!("pocold-e44-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&d);d}
fn copy_dir(from:&std::path::Path,to:&std::path::Path){let _=std::fs::remove_dir_all(to);std::fs::create_dir_all(to).unwrap();
    // LOCK is only the journal's lock target (an open node holds a lock on it; Windows refuses to read it).
    for e in std::fs::read_dir(from).unwrap(){let e=e.unwrap();if e.file_type().unwrap().is_file()&&e.file_name()!="LOCK"{std::fs::copy(e.path(),to.join(e.file_name())).unwrap();}}}
/// Two nodes are the same node: the same head and canonical chain, every entry equal field by field (its
/// header, body, receipts, ranges, ParentResult, flags, summary and stored root: the snapshot and the
/// entry's Debug form), the same answers for every canonical block, receipt and state query at every
/// height, and in each store exactly the pins its entries hold, with node counts that recount.
fn e44_same(x:&mut pocold::node::Node,y:&mut pocold::node::Node,what:&str){
    assert_eq!(x.chain.head,y.chain.head,"{what}: head");assert_eq!(x.chain.canonical,y.chain.canonical,"{what}: canonical chain");
    let (fx,fy)=(pocold::checkpoint::fingerprint(&x.chain).unwrap(),pocold::checkpoint::fingerprint(&y.chain).unwrap());
    assert_eq!(fx.entries.len(),fy.entries.len(),"{what}: entries");
    for (a,b) in fx.entries.iter().zip(&fy.entries){assert_eq!(a,b,"{what}: entry");}
    // The streamed encoding is the encoding of the snapshot.
    for n in [&*x,&*y]{let bind=pocold::checkpoint::Binding{records:3,end:4,hash:[5;32]};
        assert_eq!(pocold::checkpoint::encode(&n.chain,&bind).unwrap().0,pocold::checkpoint::encode_snapshot(&pocold::checkpoint::snapshot_of(&n.chain,&bind).unwrap()),"{what}: encode");}
    assert_eq!(fx,fy,"{what}: chain fields");
    // The eviction set (a churn guard: an evicted block received unsolicited is refused) is the FIFO's.
    for h in &fx.evicted_order{assert!(x.chain.was_evicted(h)&&y.chain.was_evicted(h),"{what}: an evicted block is remembered");}
    // Every field but the (empty) whole state, whose HashMaps print in no fixed order.
    let dbg=|n:&pocold::node::Node|{let mut v:Vec<([u8;32],String)>=n.chain.blocks.values().map(|e|(e.hash,format!("{:?}",(
        (e.hash,&e.header,&e.body,e.height,e.parent,e.total_work,e.import_seq,e.signed,e.ts,e.target,e.base_fee,e.gas_used),
        (e.seal,e.state.accounts.len(),e.is_final,&e.result,&e.receipts,&e.tx_hashes,e.cum_txs,e.cum_tx_bytes,e.cum_log_bytes,e.pruned,e.full,e.diff.is_some()),
        (e.slim,e.drawn,&e.ranges,e.body_off,e.offloaded,e.trie.is_some(),&e.summary,e.stored.as_ref().map(|p|(p.trie.world,{let mut h:Vec<_>=p.hashes.iter().collect();h.sort();format!("{h:?}")}))))))).collect();v.sort();v};
    for (a,b) in dbg(x).iter().zip(dbg(y).iter()){if a!=b{let i=a.1.bytes().zip(b.1.bytes()).position(|(p,q)|p!=q).unwrap_or(a.1.len().min(b.1.len()));let lo=i.saturating_sub(300);
        panic!("{what}: entry {} differs at byte {i}:\n{}\n{}",B256::from(a.0),&a.1[lo..a.1.len().min(i+300)],&b.1[lo..b.1.len().min(i+300)]);}}
    assert_eq!(chain_view(x),chain_view(y),"{what}: blocks and receipts");
    let alice=devnet::account_key("alice").address();
    let accounts:Vec<String>=[alice,devnet::account_key("bob").address(),Address::ZERO,apply::escrow(),apply::registry()].into_iter().chain((0..8u64).map(|k|alice.create(k))).map(|a|format!("{a:#x}")).collect();
    let states=|n:&mut pocold::node::Node|{let top=n.chain.head().height;let mut v=vec![];
        // Not "pending": the pool is not kept across a restart, and a node compared after live imports holds
        // the transactions a reorg returned to it.
        for t in (0..=top).map(|x|json!(format!("{x:#x}"))).chain(["latest"].map(|t|json!(t))){for a in &accounts{
            for m in ["eth_getBalance","eth_getTransactionCount","eth_getCode"]{v.push((format!("{m} {a} {t}"),rpc_local(n,m,json!([a,t]))));}
            v.push((format!("eth_getStorageAt {a} {t}"),rpc_local(n,"eth_getStorageAt",json!([a,"0x0",t]))));}}v};
    let (p,q)=(states(x),states(y));
    for (a,b) in p.iter().zip(&q){assert!(a==b,"{what}: state answer {}: {} / {}",a.0,&a.1.to_string()[..a.1.to_string().len().min(200)],&b.1.to_string()[..b.1.to_string().len().min(200)]);}
    let (p,_):(Vec<serde_json::Value>,Vec<serde_json::Value>)=(p.into_iter().map(|x|x.1).collect(),vec![]);
    let answered=p.iter().filter(|r|!r.is_null()).count();assert!(answered>=8*accounts.len(),"positive control ({what}): {answered} answers");
    for n in [&*x,&*y]{let stored=n.chain.blocks.values().filter(|e|e.stored.is_some()).count();
        assert_eq!(n.chain.trie_store.as_ref().unwrap().pinned().unwrap() as usize,stored,"{what}: the entries' pins are their stored versions");
        n.state_store().unwrap().recount().unwrap_or_else(|e|panic!("{what}: {e}"));}
}
/// Opens `dir` as a reference: a copy replayed from genesis (checkpoints off, so the store starts empty).
fn e44_reference(dir:&std::path::Path,tag:&str)->(pocold::node::Node,std::path::PathBuf){
    let r=e44_dir(tag);copy_dir(dir,&r);let n=reopen(&e44_conf(&r,0));assert_eq!(n.stats.resumed_from,None,"the reference replays from genesis");(n,r)
}

/// D-6: open resumes from a checkpoint and replays only the records after it, and the node it gives is
/// the node replay from genesis gives: (1) from a checkpoint written by replay (replay from it continues
/// exactly as replay from genesis does), (2) from checkpoints written live, after both reorgs.
#[test] fn e4_checkpoint_open_resumes_and_equals_replay_from_genesis(){
    let blocks=e44_blocks();let every=7u64;
    let a=e44_dir("live");
    {let mut n=reopen(&e44_conf(&a,every));for b in blocks{n.import(b.clone(),None).unwrap();}
     assert!(n.stats.reorgs>=2,"positive control: two reorgs");assert_eq!(n.stats.checkpoints_written,blocks.len() as u64/every,"a checkpoint every {every} records");}
    let (mut g,gd)=e44_reference(&a,"live-ref");assert_eq!(g.stats.checkpoints_written,0);
    // (1) a copy without its store: replay from genesis writes checkpoints; the next open resumes from one.
    let r=e44_dir("replayed");copy_dir(&a,&r);std::fs::remove_file(r.join(pocold::state_store::FILE)).unwrap();
    {let mut n=reopen(&e44_conf(&r,every));assert_eq!(n.stats.resumed_from,None);assert_eq!(n.stats.checkpoints_written,blocks.len() as u64/every,"replay checkpoints too");
     {let st=n.state_store().unwrap();assert!(!st.is_deferring()&&st.deferred_releases()==0,"replay ends its late releases (2026-10-28)");}e44_same(&mut n,&mut g,"replayed from genesis, writing checkpoints");}
    {let st=pocold::state_store::StateStore::open(&r).unwrap();let c=st.checkpoints().unwrap();let s=pocold::checkpoint::decode(&c.last().unwrap().1).unwrap();
     assert!(s.protect.as_ref().map_or(false,|p|!p.is_empty()),"positive control: the replay checkpoint names the blocks later records build on");}
    {let mut n=reopen(&e44_conf(&r,every));let k=(blocks.len() as u64/every)*every;
     assert_eq!(n.stats.resumed_from,Some(k),"resumed after the newest checkpoint");
     // Every stored version was read, most through their diff from the parent's version (2026-10-27).
     let v=n.stats.versions_read;println!("E44-VERSIONS-READ {v:?}");assert!(v.by_diff>v.whole&&v.whole>=1,"the versions were read through their diffs: {v:?}");
     e44_same(&mut n,&mut g,"resumed from a replay checkpoint");}
    // (2) the live node's own checkpoints.
    {let mut n=reopen(&e44_conf(&a,every));assert_eq!(n.stats.resumed_from,Some((blocks.len() as u64/every)*every));e44_same(&mut n,&mut g,"resumed from a live checkpoint");
     // Resumed, it goes on as a node: one more checkpoint, then a reopen from that one.
     n.checkpoint_now().unwrap();}
    {let mut n=reopen(&e44_conf(&a,every));assert_eq!(n.stats.resumed_from,Some(blocks.len() as u64),"resumed after the last record");assert_eq!(n.chain.rebuilds,0);
     e44_same(&mut n,&mut g,"resumed from a checkpoint at the last record");}
    drop(g);for d in [a,gd,r]{let _=std::fs::remove_dir_all(&d);}
}

/// D-6 crash leftovers: a version committed to the store whose block never reached the journal (a crash
/// between the state commit and the journal append, I3), and the entry pins of the run that stopped, are
/// released at open: the store then holds exactly the checkpoints' versions and the restored entries'.
#[test] fn e4_checkpoint_open_releases_what_a_crash_left_in_the_store(){
    let blocks=e44_blocks();let a=e44_dir("leftover");
    {let mut n=reopen(&e44_conf(&a,5));for b in &blocks[..60]{n.import(b.clone(),None).unwrap();}}
    let orphan={let st=pocold::state_store::StateStore::open(&a).unwrap();assert!(!st.rebuilt);
        let mut db=admitted(&cfg(&["a","b","c"])).genesis().state().clone();
        db.insert_account_info(Address::repeat_byte(0x44),revm::primitives::AccountInfo{balance:U256::from(7u64),..Default::default()});
        let mut batch=Default::default();let t=pocol_lp4_execution::node_trie::NodeTrie::build(&db,&st,&mut batch);
        st.commit(&batch,t.world,true).unwrap();t.world.unwrap()};
    let mut n=reopen(&e44_conf(&a,5));assert_eq!(n.stats.resumed_from,Some(60));
    let st=n.state_store().unwrap().clone();
    assert_eq!(st.count(&orphan).unwrap(),None,"the version no journal record names is gone");assert!(n.stats.pins_released>=1);
    st.recount().unwrap();
    for b in &blocks[60..]{n.import(b.clone(),None).unwrap();}
    // The reference is copied from a closed data directory (Windows does not let a running node's files be read):
    // the node and every handle on its store go first.
    drop(st);drop(n);let (mut g,gd)=e44_reference(&a,"leftover-ref");let mut n=reopen(&e44_conf(&a,5));
    e44_same(&mut n,&mut g,"after the crash leftovers were released");
    drop(n);drop(g);for d in [a,gd]{let _=std::fs::remove_dir_all(&d);}
}

/// D-6 deferred release: what a dropped checkpoint owes is worked off at open by a node that stopped owing
/// it, and between a running node's events (`Node::run` takes a release step whenever no event waits), not
/// at its next checkpoint. A node imports every block but the last, a checkpoint every 7 records and no
/// release step, and stops owing. It reopens owing nothing, writes one more checkpoint (owing again), runs
/// its event loop with no event but its ticks until the store owes nothing, then imports the last block from
/// its channel, which stops it. The counts recount at each stage.
#[test] fn e4_checkpoint_release_runs_at_open_and_between_events(){
    use pocold::{net::{Msg,NetEvent},node::Event};use std::time::{Duration,Instant};
    let blocks=e44_blocks();let (last,rest)=blocks.split_last().unwrap();let a=e44_dir("release");let every=7u64;
    {let mut n=reopen(&e44_conf(&a,every));for b in rest{n.import(b.clone(),None).unwrap();}
     assert!(n.stats.checkpoints_written>=3&&n.release_pending(),"positive control: the node stops owing a dropped checkpoint's nodes");}
    let owed=pocold::state_store::StateStore::open(&a).unwrap().owed_refs().unwrap().len();assert!(owed>0,"the store owes work when it reopens");
    let mut n=reopen(&e44_conf(&a,every));
    assert!(n.stats.released_at_open>0&&!n.release_pending(),"open works off what is owed ({} released, {owed} owed)",n.stats.released_at_open);
    let (st,opened)=(n.state_store().unwrap().clone(),n.stats.released_at_open);assert!(st.owed_refs().unwrap().is_empty());st.recount().unwrap();
    n.checkpoint_now().unwrap();assert!(n.release_pending(),"positive control: the checkpoint dropped the oldest, whose nodes are owed");
    // The last block is at the head's height (it reorgs back), so it stops the loop.
    n.cfg.stop_at=Some(n.chain.head().height);let (tx,steps)=(n.tx.clone(),n.stats.release_steps);
    let (peer,_socket)=auth_peer::connect(&mut n);
    let run=std::thread::Builder::new().stack_size(64<<20).spawn(move||{n.run();n}).unwrap();
    let t=Instant::now();
    while st.release_pending(){assert!(t.elapsed()<Duration::from_secs(120),"the running node works off what is owed");std::thread::sleep(Duration::from_millis(20));}
    st.recount().unwrap();
    tx.send(Event::Net(NetEvent::Message(peer,Msg::Block{header:last.header.encoded.clone(),body:last.body.encode()}))).unwrap();
    let n=run.join().unwrap();
    println!("E44-RELEASE rows owed at reopen {owed}, rows worked off at open {opened}, steps in the event loop {}",n.stats.release_steps-steps);
    assert!(n.stats.release_steps>steps,"the release steps ran in the event loop");assert_eq!(n.chain.head,last.hash(),"the last block was imported after the release");
    st.recount().unwrap();drop(st);drop(n);let _=std::fs::remove_dir_all(&a);
}

/// D-6 (review 2026-10-27, finding 2): with a pool revalidation backlog and a dropped checkpoint's nodes both
/// pending, the background steps the event loop takes between events alternate. Before, every step went to
/// the pool while it had a backlog, so a backlog that does not empty between heads left the whole release
/// to the next checkpoint's write. A store-mode node pools four transfers, a new head puts them back in the
/// backlog (one transaction per slice), and three checkpoints drop the first: while both are pending, the
/// steps alternate, and a release step runs before the backlog is empty.
#[test] fn e4_release_steps_alternate_with_pool_slices(){
    use std::time::Duration;
    let mut h=net();let g=h.chain.head;let blocks=branch(&mut h,g,2,&[]);
    let (n,dir)=gas_node("release-turns",pocold::node::Limits{state_store:true,checkpoint_every:1000,pool_slice:Duration::ZERO,..Default::default()});let mut n=n.unwrap();
    n.import(blocks[0].clone(),None).unwrap();let alice=devnet::account_key("alice");
    for nonce in 0..4{n.submit_tx(pay(&alice,nonce,1),false).unwrap();}
    n.import(blocks[1].clone(),None).unwrap();assert!(n.pool_pending(),"positive control: the new head left a revalidation backlog");
    for _ in 0..3{n.checkpoint_now().unwrap();}assert!(n.release_pending(),"positive control: the third checkpoint dropped the first, owed");
    let mut kinds=vec![];
    while n.pool_pending()&&n.release_pending(){let (r,p)=(n.stats.release_steps,n.stats.pool_slices);n.background_step();
        kinds.push(match (n.stats.release_steps-r,n.stats.pool_slices-p){(1,0)=>'r',(0,1)=>'p',x=>panic!("one step, one kind: {x:?}")});}
    println!("E4-TURNS {}",kinds.iter().collect::<String>());
    assert!(kinds.contains(&'r'),"a release step ran while the pool still had a backlog: {kinds:?}");
    assert!(kinds.windows(2).all(|w|w[0]!=w[1]),"the steps alternate: {kinds:?}");
    while n.pool_pending()||n.release_pending(){n.background_step();}
    assert_eq!(n.pool.txs.len(),4,"every transfer is still pooled");n.state_store().unwrap().recount().unwrap();
    drop(n);let _=std::fs::remove_dir_all(&dir);
}

/// D-6 (Windows CI run 40589342452, 2026-10-27): a store the release at open finds damaged is set aside, and
/// the node replays from genesis. A node stops owing a dropped checkpoint's nodes, and a node it owes is then
/// removed from state.redb, so the release at open meets a row naming a node that is not stored and marks
/// the store damaged. Before the fix, open went on to pin genesis's version in that store and panicked (in
/// the damage test, whenever the node it removed was also owed).
#[test] fn e4_checkpoint_open_rebuilds_a_store_its_release_finds_damaged(){
    let blocks=e44_blocks();let a=e44_dir("release-damaged");let every=7u64;
    {let mut n=reopen(&e44_conf(&a,every));for b in blocks{n.import(b.clone(),None).unwrap();}assert!(n.release_pending(),"positive control: the node stops owing");}
    let (mut g,gd)=e44_reference(&a,"release-damaged-ref");
    let owed=pocold::state_store::StateStore::open(&a).unwrap().owed_refs().unwrap();assert!(!owed.is_empty(),"positive control: the store owes work");
    {let nodes:redb::TableDefinition<&[u8;32],&[u8]>=redb::TableDefinition::new("node");let db=redb::Database::open(a.join(pocold::state_store::FILE)).unwrap();let w=db.begin_write().unwrap();
     {let mut t=w.open_table(nodes).unwrap();assert!(t.remove(&owed[0].0).unwrap().is_some(),"positive control: the owed node was stored");}w.commit().unwrap();}
    let mut n=reopen(&e44_conf(&a,every));
    assert_eq!(n.stats.resumed_from,None,"replayed from genesis");assert!(!n.state_store().unwrap().is_damaged(),"the new store is whole");
    e44_same(&mut n,&mut g,"after the release at open found the store damaged");
    drop(n);drop(g);for d in [a,gd]{let _=std::fs::remove_dir_all(&d);}
}

/// Seeded fault RV11 (2026-10-28): an I/O error in the release at open is not damage. A node stops owing a
/// dropped checkpoint's nodes; the next open's release (`release_all`) meets an injected fault (`io-fault`):
/// a failed page write, which undoes it, or a failed fsync, after which it may be durable. Open fails (redb
/// refuses the store until it is opened again) and sets nothing aside: no damaged marker, no
/// `state.redb.aside`. The open after it resumes from a checkpoint, owes nothing, and is the node replay from
/// genesis gives. With RV11 (any error from the release marks the store damaged) the first open set the
/// store aside and replayed from genesis.
#[cfg(feature="io-fault")]
#[test] fn e4_checkpoint_open_that_fails_on_io_in_its_release_keeps_the_store(){
    use pocold::io_fault::{self,Op};
    let blocks=e44_blocks();let every=7u64;
    for op in [Op::Write,Op::Sync]{
        let a=e44_dir(&format!("release-io-{op:?}"));
        {let mut n=reopen(&e44_conf(&a,every));for b in blocks{n.import(b.clone(),None).unwrap();}assert!(n.release_pending(),"positive control: the node stops owing");}
        let file=a.join(pocold::state_store::FILE);
        let owed=pocold::state_store::StateStore::open(&a).unwrap().owed_refs().unwrap().len();assert!(owed>0,"positive control: the store owes work");
        let (mut g,gd)=e44_reference(&a,&format!("release-io-ref-{op:?}"));
        io_fault::arm(&file,"release-all",op);
        let e=pocold::node::Node::open(e44_conf(&a,every)(),admitted(&cfg(&["a","b","c"]))).err().expect("open fails on the I/O error");
        assert_eq!(io_fault::fired(&file),vec![("release-all".to_string(),op)],"{op:?}: positive control: the fault fired in the release at open ({e})");
        assert!(!a.join(pocold::state_store::DAMAGED).exists()&&!a.join(format!("{}.aside",pocold::state_store::FILE)).exists(),"{op:?}: nothing is set aside ({e})");
        let mut n=reopen(&e44_conf(&a,every));
        println!("E4-RELEASE-IO {op:?}: open failed ({e}); the next resumed from record {:?}, {} rows worked off at open, {owed} owed before",n.stats.resumed_from,n.stats.released_at_open);
        assert!(n.stats.resumed_from.is_some(),"{op:?}: the next open resumes from a checkpoint");
        if op==Op::Write{assert!(n.stats.released_at_open>0,"a failed write undid the release: the next open works it off");}
        assert!(!n.release_pending()&&n.state_store().unwrap().owed_refs().unwrap().is_empty(),"{op:?}: nothing is owed");
        e44_same(&mut n,&mut g,&format!("after an I/O fault in the release at open ({op:?})"));
        drop(n);drop(g);for d in [a,gd]{let _=std::fs::remove_dir_all(&d);}
    }
}
/// 2026-10-28: a storage error while a block is imported is the node's own failure, never a validity verdict
/// (blockReject is for committed content). A store-mode node imports blocks; then an injected fault
/// (`io-fault`) fails a state.redb commit: the commit of the next block's state ("import"), or a release
/// step, after which redb refuses the store, so the next block's commit fails ("release"); each with a
/// failed page write and with a failed fsync. Before the fix the import returned `Reject::Block`: the block
/// was marked invalid, and it and every descendant were refused until the node restarted. Now the node stops
/// (exit 70, as when a history or journal write fails) and marks nothing invalid. The next open is the node
/// replay from genesis gives from the same journal, and imports that block and the rest. The child process
/// is the node, since its exit is what is tested.
#[cfg(feature="io-fault")]
#[test] fn e4_a_state_store_io_fault_stops_the_node_and_marks_no_block_invalid(){
    let blocks=e44_blocks();let exe=std::env::current_exe().unwrap();
    for mode in ["import-Write","import-Sync","release-Write","release-Sync","checkpoint-Write","checkpoint-Sync"]{
        let d=e44_dir(&format!("iof-{mode}"));let file=d.with_extension("blocks");
        {let mut out=vec![];for b in blocks{for p in [b.header.encoded.clone(),b.body.encode()]{out.extend((p.len() as u32).to_be_bytes());out.extend(p);}}std::fs::write(&file,out).unwrap();}
        let out=std::process::Command::new(&exe).args(["e4_io_fault_child","--exact","--ignored","--nocapture","--test-threads=1"])
            .env("POCOLD_IOF_CHILD",&d).env("POCOLD_E44_BLOCKS",&file).env("POCOLD_IOF_MODE",mode).output().unwrap();
        let text=format!("{}{}",String::from_utf8_lossy(&out.stdout),String::from_utf8_lossy(&out.stderr));
        let at:usize=text.split("IOF-CHILD armed before block ").nth(1).and_then(|x|x.split_whitespace().next()).and_then(|x|x.parse().ok()).unwrap_or_else(||panic!("{mode}: the child: {text}"));
        println!("E4-IMPORT-IO {mode} (exit {:?}): {}",out.status.code(),text.lines().filter(|l|l.contains("IOF-CHILD")||l.contains("FATAL")||l.contains("] reject ")||l.contains("release step:")).collect::<Vec<_>>().join(" | "));
        assert!(text.contains("injected I/O fault"),"{mode}: positive control: the fault fired: {text}");
        assert_eq!(out.status.code(),Some(70),"{mode}: the node stops with exit 70: {text}");
        // "checkpoint": the blocks before the checkpoint's write are imported; the node stops in that write.
        let stopped_in=if mode.starts_with("checkpoint"){text.contains("FATAL local storage: checkpoint at record")&&!text.contains("IOF-CHILD imported block")||text.lines().filter(|l|l.contains("IOF-CHILD imported")).all(|l|l.contains("Ok(true)"))&&text.contains("FATAL local storage: checkpoint at record")}
            // "release": the node stops in the release step itself (review F4), before any read the refused store
            // would fail; not later, in the next import's commit.
            else if mode.starts_with("release"){text.contains("FATAL local storage: release step")&&!text.contains("IOF-CHILD imported")}
            else{text.contains("FATAL local storage")&&!text.contains("IOF-CHILD imported")};
        assert!(stopped_in&&!text.contains("] reject "),"{mode}: no block is rejected: {text}");
        let (mut g,gd)=e44_reference(&d,&format!("iof-ref-{mode}"));
        let mut n=reopen(&e44_conf(&d,7));e44_same(&mut n,&mut g,&format!("after the stop ({mode})"));
        for b in &blocks[at..]{if n.chain.get(&b.hash()).is_some(){continue}n.import(b.clone(),None).unwrap_or_else(|e|panic!("{mode}: the next open imports every block: {e:?}"));}
        assert_eq!(n.chain.head,blocks.last().unwrap().hash(),"{mode}: the block the fault stopped and the rest are imported");
        assert!(n.chain.invalid.is_empty(),"{mode}: no block is marked invalid");n.state_store().unwrap().recount().unwrap();
        drop(n);drop(g);for p in [d,gd]{let _=std::fs::remove_dir_all(&p);}let _=std::fs::remove_file(&file);
    }
}
/// 2026-10-28 (independent review F1, F2): an open whose checkpoint restore cannot read its storage (the journal's
/// read back of a record, or state.redb's write when a stored version is pinned) fails. It drops no checkpoint,
/// sets nothing aside and rebuilds nothing: the next open resumes from the newest checkpoint and is the node replay
/// from genesis gives. Before the fix a journal read error refused every checkpoint in turn, dropped each, and the
/// store was deleted and replayed from genesis.
#[cfg(feature="io-fault")]
#[test] fn e4_an_open_that_cannot_read_its_storage_fails_and_keeps_the_store(){
    use pocold::io_fault::{self,Op};
    let blocks=e44_blocks();let every=7u64;
    for (what,arm) in [("journal read",0u8),("state.redb write while pinning a stored version",1),("state.redb write while dropping a refused checkpoint",2),
        // Review 2026-10-28 (F-A): a state.redb read that fails when the store is opened. Before, the store was set aside
        // as if damaged (state.redb.aside), its checkpoints lost, and the node replayed from genesis.
        ("state.redb read at open",3),("history.redb read at open",4),
        ("history.redb write at open",5),("history.redb sync at open",6)]{
        let d=e44_dir(&format!("open-io-{arm}"));
        {let mut n=reopen(&e44_conf(&d,every));for b in blocks{n.import(b.clone(),None).unwrap();}}
        let ck=||{let st=pocold::state_store::StateStore::open(&d).unwrap();st.checkpoints().unwrap().iter().map(|c|c.0).collect::<Vec<_>>()};
        let before=ck();assert!(before.len()>=2,"positive control: checkpoints kept ({before:?})");
        let (jr,st)=(d.join("blocks.journal"),d.join(pocold::state_store::FILE));
        let hist=d.join(pocold::history::FILE);
        // The genesis version is pinned first (the first state commit); the stored versions after it.
        // Case 2: the newest checkpoint is made unbound to the journal (refused), and dropping it fails.
        if arm==2{use redb::ReadableTable;let db=redb::Database::open(&st).unwrap();let w=db.begin_write().unwrap();
            {let ck:redb::TableDefinition<u64,&[u8]>=redb::TableDefinition::new("checkpoint");let mut t=w.open_table(ck).unwrap();
             let (seq,bytes)={let (k,v)=t.last().unwrap().unwrap();(k.value(),v.value().to_vec())};let mut s=pocold::checkpoint::decode(&bytes).unwrap();
             s.binding.hash=[1;32];t.insert(seq,pocold::checkpoint::encode_snapshot(&s).as_slice()).unwrap();}
            w.commit().unwrap();}
        match arm{0=>io_fault::arm_now(&jr,Op::Read),1=>io_fault::arm_at(&st,"state-txn",Op::Write,2),2=>io_fault::arm(&st,"drop-checkpoint",Op::Write),3=>io_fault::arm_now(&st,Op::Read),4=>io_fault::arm_now(&hist,Op::Read),5=>io_fault::arm(&hist,"history-open-tables",Op::Write),_=>io_fault::arm(&hist,"history-open-tables",Op::Sync)}
        let r=pocold::node::Node::open(e44_conf(&d,every)(),admitted(&cfg(&["a","b","c"])));
        let fired=if arm==0{io_fault::fired(&jr)}else if arm>=4{io_fault::fired(&hist)}else{io_fault::fired(&st)};io_fault::disarm(&jr);io_fault::disarm(&st);io_fault::disarm(&hist);
        assert!(!fired.is_empty(),"{what}: positive control: the fault fired");
        let e=match r{Err(e)=>e,Ok(n)=>panic!("{what}: the open succeeded (resumed from {:?}, {} refused)",n.stats.resumed_from,n.stats.checkpoints_refused)};
        assert!(e.contains(match arm{2=>"dropping checkpoint",3..=6=>"I/O failure",_=>"could not be read"}),"{what}: {e}");
        assert!(hist.exists(),"{what}: history.redb is kept");
        assert!(!d.join(format!("{}.aside",pocold::history::FILE)).exists(),"{what}: history is not set aside");
        assert!(!d.join(pocold::history::DAMAGED).exists(),"{what}: history is not marked damaged");
        assert!(st.exists(),"{what}: state.redb is kept");assert!(!d.join(format!("{}.aside",pocold::state_store::FILE)).exists(),"{what}: nothing is set aside");assert_eq!(ck(),before,"{what}: no checkpoint is dropped");
        let (mut g,gd)=e44_reference(&d,&format!("open-io-ref-{arm}"));
        let newest=(blocks.len() as u64/every)*every;
        let mut n=reopen(&e44_conf(&d,every));
        if arm==2{assert_eq!((n.stats.resumed_from,n.stats.checkpoints_refused),(Some(newest-every),1),"{what}: the next open refuses the unbound checkpoint and resumes from the one before");}
        else{assert_eq!((n.stats.resumed_from,n.stats.checkpoints_refused),(Some(newest),0),"{what}: the next open resumes from the newest checkpoint");}e44_same(&mut n,&mut g,&format!("after an open that failed on {what}"));
        drop(n);drop(g);for p in [d,gd]{let _=std::fs::remove_dir_all(&p);}
    }
}
/// The child of `e4_a_state_store_io_fault_stops_the_node_and_marks_no_block_invalid` (`POCOLD_IOF_MODE`):
/// imports the blocks of `POCOLD_E44_BLOCKS` until three checkpoints are written ("release": and rows are
/// owed), arms the fault, takes a release step ("release"), then imports the next block. It is expected to
/// stop inside that import; if it returns, it prints the result. Not a test on its own.
#[cfg(feature="io-fault")]
#[test] #[ignore="the child process of e4_a_state_store_io_fault_stops_the_node_and_marks_no_block_invalid"]
fn e4_io_fault_child(){
    use std::io::Write;use pocold::io_fault::{self,Op};
    let (Ok(dir),Ok(file),Ok(mode))=(std::env::var("POCOLD_IOF_CHILD"),std::env::var("POCOLD_E44_BLOCKS"),std::env::var("POCOLD_IOF_MODE")) else{return};
    let bytes=std::fs::read(file).unwrap();let (mut i,mut blocks)=(0usize,vec![]);
    while i<bytes.len(){let mut part=vec![];for _ in 0..2{let n=u32::from_be_bytes(bytes[i..i+4].try_into().unwrap()) as usize;part.push(bytes[i+4..i+4+n].to_vec());i+=4+n;}
        blocks.push(Block{header:decode_header(&part[0]).unwrap(),body:Body::decode(&part[1],1<<24).unwrap()});}
    let d=std::path::PathBuf::from(&dir);let mut n=reopen(&e44_conf(&d,7));let st=d.join(pocold::state_store::FILE);
    let (op,release)=(if mode.ends_with("Sync"){Op::Sync}else{Op::Write},mode.starts_with("release"));
    let mut k=0;while k+1<blocks.len()&&!(n.stats.checkpoints_written>=3&&(!release||n.release_pending())){n.import(blocks[k].clone(),None).unwrap();k+=1;}
    assert!(n.stats.checkpoints_written>=3&&(!release||n.release_pending()),"positive control: checkpoints written (and rows owed)");
    println!("\nIOF-CHILD armed before block {k}");std::io::stdout().flush().unwrap();
    if release{io_fault::arm(&st,"release-step",op);n.release_step();println!("\nIOF-CHILD release step taken; pending {}",n.release_pending());}
    else if mode.starts_with("checkpoint"){io_fault::arm(&st,"checkpoint-txn",op);}
    else{io_fault::arm(&st,"state-txn",op);}
    std::io::stdout().flush().unwrap();
    // "checkpoint": the blocks up to the next checkpoint's write (2026-10-28, review F4).
    for b in &blocks[k..(k+if mode.starts_with("checkpoint"){8}else{1}).min(blocks.len())]{
        let r=n.import(b.clone(),None);
        println!("\nIOF-CHILD imported block {k}: {r:?}; {} blocks marked invalid",n.chain.invalid.len());std::io::stdout().flush().unwrap();}
}

/// The child of `e4_checkpoint_survives_kills`: imports the blocks of `POCOLD_E44_BLOCKS` into the node
/// at `POCOLD_E44_CHILD`, a checkpoint after every record, until it is killed. Not a test on its own.
#[test] #[ignore="the child process of e4_checkpoint_survives_kills"]
fn e4_checkpoint_kill_child(){
    use std::io::Write;
    let (Ok(dir),Ok(file))=(std::env::var("POCOLD_E44_CHILD"),std::env::var("POCOLD_E44_BLOCKS")) else{return};
    let bytes=std::fs::read(file).unwrap();let (mut i,mut blocks)=(0usize,vec![]);
    while i<bytes.len(){let mut part=vec![];for _ in 0..2{let n=u32::from_be_bytes(bytes[i..i+4].try_into().unwrap()) as usize;part.push(bytes[i+4..i+4+n].to_vec());i+=4+n;}
        blocks.push(Block{header:decode_header(&part[0]).unwrap(),body:Body::decode(&part[1],1<<24).unwrap()});}
    let every=std::env::var("POCOLD_E44_EVERY").ok().and_then(|x|x.parse().ok()).unwrap_or(1);
    let mut n=reopen(&e44_conf(std::path::Path::new(&dir),every));
    println!("\nE44READY {}",n.chain.head().height);std::io::stdout().flush().unwrap();
    // D-6: one release step after each import, as the event loop runs one between events.
    for b in blocks{let _=n.import(b,None);n.release_step();}
    println!("\nE44DONE");std::io::stdout().flush().unwrap();
    loop{std::thread::sleep(std::time::Duration::from_secs(1));}
}
/// D-6 crash test: a node importing blocks with a checkpoint after every record (each a block commit, a
/// journal append and a checkpoint transaction) is killed (SIGKILL on Unix, TerminateProcess on Windows)
/// at increasing delays. After every kill the node opens and is the node replay from genesis gives from
/// the same journal; it resumes from a checkpoint once there is one; the last run imports every block.
#[test] fn e4_checkpoint_survives_kills(){
    use std::io::BufRead;
    let blocks=e44_blocks();let a=e44_dir("kill");let file=a.with_extension("blocks");
    {let mut out=vec![];for b in blocks{for p in [b.header.encoded.clone(),b.body.encode()]{out.extend((p.len() as u32).to_be_bytes());out.extend(p);}}std::fs::write(&file,out).unwrap();}
    let exe=std::env::current_exe().unwrap();let mut resumed=0;let mut heights=vec![];let err=a.with_extension("child-stderr");
    for (k,delay) in [40u64,90,160,260,400,600,900,1300,1800,2600,0].into_iter().enumerate(){
        let mut c=std::process::Command::new(&exe).args(["e4_checkpoint_kill_child","--exact","--ignored","--nocapture","--test-threads=1"])
            .env("POCOLD_E44_CHILD",&a).env("POCOLD_E44_BLOCKS",&file).stdout(std::process::Stdio::piped()).stderr(std::fs::File::create(&err).unwrap()).spawn().unwrap();
        let mut lines=std::io::BufReader::new(c.stdout.take().unwrap()).lines();
        // The child's stderr is kept in a file, and its end is shown if the child did not open (2026-10-28: one run
        // under heavy load failed here with nothing to show why).
        let ready=lines.find_map(|l|l.ok().filter(|l|l.starts_with("E44READY"))).unwrap_or_else(||{let _=c.wait();
            let t=std::fs::read_to_string(&err).unwrap_or_default();panic!("the child opened (run {k}); its stderr ends: {}",&t[t.len().saturating_sub(3000)..])});
        if delay==0{assert!(lines.find_map(|l|l.ok().filter(|l|l=="E44DONE")).is_some(),"the last run imports every block");}
        else{std::thread::sleep(std::time::Duration::from_millis(delay));}
        c.kill().unwrap();c.wait().unwrap();
        let (mut g,gd)=e44_reference(&a,&format!("kill-ref-{k}"));
        let mut n=reopen(&e44_conf(&a,1));if n.stats.resumed_from.is_some(){resumed+=1;}
        heights.push((ready,n.chain.head().height,n.stats.resumed_from));
        e44_same(&mut n,&mut g,&format!("after kill {k} ({delay} ms)"));
        if delay==0{assert_eq!(n.chain.head,blocks.last().unwrap().hash());}
        drop(n);drop(g);let _=std::fs::remove_dir_all(&gd);
    }
    println!("E44-KILLS {heights:?}");
    assert!(resumed>=8,"positive control: most opens resumed from a checkpoint ({resumed})");
    let _=std::fs::remove_dir_all(&a);let _=std::fs::remove_file(&file);let _=std::fs::remove_file(&err);
}

/// D-6 damage: a store that is cut short, has a changed byte, is missing, or holds a checkpoint that does
/// not verify (its seal; a stored root that is not the block's committed stateRoot; a summary that is not
/// the stored version's; a binding to other records; a total work that does not follow from the parent; a
/// node whose bytes are not its id; a node that is missing), and a missing history.redb. Each time open
/// gives the node replay from genesis gives: from the older checkpoint when only the newest is wrong, from
/// genesis (the store rebuilt) when the store itself is not sound. No unverified state is used.
#[test] fn e4_checkpoint_damage_falls_back_to_a_verified_state(){
    use redb::ReadableTable;
    let blocks=e44_blocks();let every=7u64;let base=e44_dir("damage");
    {let mut n=reopen(&e44_conf(&base,every));for b in blocks{n.import(b.clone(),None).unwrap();}}
    let (mut g,gd)=e44_reference(&base,"damage-ref");
    let ckpt:redb::TableDefinition<u64,&[u8]>=redb::TableDefinition::new("checkpoint");let roots:redb::TableDefinition<u64,&[u8]>=redb::TableDefinition::new("checkpoint_roots");
    let nodes:redb::TableDefinition<&[u8;32],&[u8]>=redb::TableDefinition::new("node");
    let (older,newest)={let st=pocold::state_store::StateStore::open(&base).unwrap();let c=st.checkpoints().unwrap();assert_eq!(c.len(),2,"two checkpoints kept");
        (pocold::checkpoint::decode(&c[0].1).unwrap(),pocold::checkpoint::decode(&c[1].1).unwrap())};
    // The root node of the newest checkpoint's head version (the running node's own head version was
    // released when it stopped, unless a checkpoint holds it).
    let head_root:[u8;32]=newest.entries.iter().find(|e|e.hash==newest.head).and_then(|e|e.root).expect("the newest checkpoint holds its head's version");
    // The newest checkpoint's versions between its oldest and its head.
    let middle:Vec<[u8;32]>={let r:Vec<[u8;32]>=newest.entries.iter().filter_map(|e|e.root).collect();r[1..r.len().saturating_sub(1)].iter().copied().filter(|x|*x!=head_root).collect()};
    assert!(!middle.is_empty(),"positive control: the newest checkpoint holds versions between its oldest and its head");
    // Rewrites the newest snapshot (re-sealed, its roots record kept in step).
    let tamper=|d:&std::path::Path,f:&dyn Fn(&mut pocold::checkpoint::Snapshot)|{
        let db=redb::Database::open(d.join(pocold::state_store::FILE)).unwrap();let w=db.begin_write().unwrap();
        {let mut t=w.open_table(ckpt).unwrap();let (seq,bytes)={let (k,v)=t.last().unwrap().unwrap();(k.value(),v.value().to_vec())};
         let mut s=pocold::checkpoint::decode(&bytes).unwrap();f(&mut s);t.insert(seq,pocold::checkpoint::encode_snapshot(&s).as_slice()).unwrap();
         let flat:Vec<u8>=pocold::checkpoint::roots(&s).iter().flat_map(|r|r.to_vec()).collect();w.open_table(roots).unwrap().insert(seq,flat.as_slice()).unwrap();}
        w.commit().unwrap();};
    let cases:Vec<(&str,Box<dyn Fn(&std::path::Path)>,Option<u64>,bool)>=vec![
        ("the store cut short",Box::new(|d|{let p=d.join(pocold::state_store::FILE);let l=std::fs::metadata(&p).unwrap().len();std::fs::OpenOptions::new().write(true).open(&p).unwrap().set_len(l/2).unwrap();}),None,false),
        ("a changed byte in the store",Box::new(|d|{let p=d.join(pocold::state_store::FILE);let mut b=std::fs::read(&p).unwrap();let i=b.len()/2;b[i]^=0x40;std::fs::write(&p,b).unwrap();}),None,true),
        ("the store missing",Box::new(|d|std::fs::remove_file(d.join(pocold::state_store::FILE)).unwrap()),None,false),
        ("history.redb missing",Box::new(|d|std::fs::remove_file(d.join(pocold::history::FILE)).unwrap()),None,false),
        ("the newest checkpoint's seal",Box::new(|d|{let db=redb::Database::open(d.join(pocold::state_store::FILE)).unwrap();let w=db.begin_write().unwrap();
            {let mut t=w.open_table(ckpt).unwrap();let (seq,mut bytes)={let (k,v)=t.last().unwrap().unwrap();(k.value(),v.value().to_vec())};let i=bytes.len()/2;bytes[i]^=1;t.insert(seq,bytes.as_slice()).unwrap();}w.commit().unwrap();}),None,false),
        ("two stored roots swapped",Box::new(|d|tamper(d,&|s|{let ix:Vec<usize>=s.entries.iter().enumerate().filter(|(_,e)|e.root.is_some()).map(|(i,_)|i).collect();
            let (i,j)=(ix[ix.len()-1],ix[ix.len()-2]);let (x,y)=(s.entries[i].root,s.entries[j].root);s.entries[i].root=y;s.entries[j].root=x;})),Some(older.binding.records),false),
        // Each entry then names a version whose summary matches it: only the state-root check (the version's
        // root against the block's committed stateRoot) tells them apart.
        ("two stored roots swapped with their summaries",Box::new(|d|tamper(d,&|s|{let ix:Vec<usize>=s.entries.iter().enumerate().filter(|(_,e)|e.root.is_some()).map(|(i,_)|i).collect();
            let (i,j)=(ix[ix.len()-1],ix[ix.len()-2]);let (x,y)=((s.entries[i].root,s.entries[i].summary),(s.entries[j].root,s.entries[j].summary));
            (s.entries[i].root,s.entries[i].summary)=y;(s.entries[j].root,s.entries[j].summary)=x;})),Some(older.binding.records),false),
        ("a stored version's summary",Box::new(|d|tamper(d,&|s|{let e=s.entries.iter_mut().rev().find(|e|e.root.is_some()).unwrap();e.summary.as_mut().unwrap().user_slots+=1;})),Some(older.binding.records),false),
        ("a roots record that is not its snapshot's",Box::new(|d|{let db=redb::Database::open(d.join(pocold::state_store::FILE)).unwrap();let w=db.begin_write().unwrap();
            {let mut t=w.open_table(roots).unwrap();let (seq,mut v)={let (k,v)=t.last().unwrap().unwrap();(k.value(),v.value().to_vec())};v.truncate(v.len()-32);t.insert(seq,v.as_slice()).unwrap();}w.commit().unwrap();}),None,false),
        // Another of the checkpoint's own versions in place of one, both named by both checkpoints and the one
        // named twice also pinned by an entry of the closed node: open's pin reset then finds every count it
        // needs and releases no node, so only the check of the record against its snapshot tells (C-1 E4.4
        // retest K12).
        ("a roots record naming one of its versions in place of another",Box::new(|d|{let db=redb::Database::open(d.join(pocold::state_store::FILE)).unwrap();let w=db.begin_write().unwrap();
            {let pins:redb::TableDefinition<&[u8;32],u32>=redb::TableDefinition::new("pins");let pins=w.open_table(pins).unwrap();
             let mut t=w.open_table(roots).unwrap();let (seq,v)={let (k,v)=t.last().unwrap().unwrap();(k.value(),v.value().to_vec())};let first=t.first().unwrap().unwrap().1.value().to_vec();
             let chunks=|v:&[u8]|v.chunks(32).map(|c|c.to_vec()).collect::<Vec<_>>();let (new,old)=(chunks(&v),chunks(&first));
             let named=|r:&Vec<u8>|(new.iter().chain(&old).filter(|z|*z==r).count()) as u32;
             let held=|r:&Vec<u8>|pins.get(&<[u8;32]>::try_from(r.as_slice()).unwrap()).unwrap().map_or(0,|v|v.value());
             let both:Vec<&Vec<u8>>=new.iter().filter(|r|old.contains(r)).collect();
             let y=both.iter().find(|r|held(r)>named(r)).expect("a version both checkpoints and an entry pin").to_vec();let x=both.iter().find(|r|***r!=y).unwrap().to_vec();
             let i=new.iter().position(|r|*r==x).unwrap();let mut out=new.clone();out[i]=y;let flat:Vec<u8>=out.concat();drop(pins);t.insert(seq,flat.as_slice()).unwrap();}
            w.commit().unwrap();}),None,false),
        ("bound to other records",Box::new(|d|tamper(d,&|s|{s.binding.hash[0]^=1;})),Some(older.binding.records),false),
        ("bound to more records than the journal holds",Box::new(|d|tamper(d,&|s|{s.binding.records+=3;})),Some(older.binding.records),false),
        ("a next import sequence below an entry's",Box::new(|d|tamper(d,&|s|{s.next_seq=1;})),Some(older.binding.records),false),
        ("a total work",Box::new(|d|tamper(d,&|s|{let e=s.entries.last_mut().unwrap();e.total_work+=U256::from(1u64);})),Some(older.binding.records),false),
        ("a node whose bytes are not its id",Box::new(move|d|{let db=redb::Database::open(d.join(pocold::state_store::FILE)).unwrap();let w=db.begin_write().unwrap();
            {let mut t=w.open_table(nodes).unwrap();let mut v=t.get(&head_root).unwrap().unwrap().value().to_vec();let l=v.len();v[l-1]^=1;t.insert(&head_root,v.as_slice()).unwrap();}w.commit().unwrap();}),None,false),
        // A node only the newest checkpoint's head version names: the older checkpoint would verify without
        // it, but a store that lost a node it named is not used (marked damaged and rebuilt).
        ("a missing node only the newest checkpoint names",Box::new(move|d|{let db=redb::Database::open(d.join(pocold::state_store::FILE)).unwrap();let w=db.begin_write().unwrap();
            {let mut t=w.open_table(nodes).unwrap();
             let reach=|t:&redb::Table<&[u8;32],&[u8]>,roots:&[[u8;32]]|{let mut seen=std::collections::HashSet::new();let mut st:Vec<[u8;32]>=roots.to_vec();
                while let Some(id)=st.pop(){if !seen.insert(id){continue}let v=t.get(&id).unwrap().unwrap().value().to_vec();st.extend(pocol_lp4_execution::node_trie::Node::decode(&v[4..]).unwrap().children());}seen};
             let old=reach(&t,&pocold::checkpoint::roots(&older));let new=reach(&t,&[head_root]);
             let only=*new.iter().find(|x|!old.contains(*x)).expect("a node only the newest head version names");t.remove(&only).unwrap();}
            w.commit().unwrap();}),None,false),
        // A node only one version in the middle of the newest checkpoint's chain names (no other pinned version holds
        // it): open reads that version through its diff from its parent's (2026-10-27), and must still read it.
        ("a missing node only a middle version names",Box::new(move|d|{let db=redb::Database::open(d.join(pocold::state_store::FILE)).unwrap();let w=db.begin_write().unwrap();
            {let pins:redb::TableDefinition<&[u8;32],u32>=redb::TableDefinition::new("pins");
             let pinned:Vec<[u8;32]>={let p=w.open_table(pins).unwrap();p.iter().unwrap().map(|kv|*kv.unwrap().0.value()).collect()};
             let mut t=w.open_table(nodes).unwrap();
             let reach=|t:&redb::Table<&[u8;32],&[u8]>,roots:&[[u8;32]]|{let mut seen=std::collections::HashSet::new();let mut st:Vec<[u8;32]>=roots.to_vec();
                while let Some(id)=st.pop(){if !seen.insert(id){continue}let v=t.get(&id).unwrap().unwrap().value().to_vec();st.extend(pocol_lp4_execution::node_trie::Node::decode(&v[4..]).unwrap().children());}seen};
             let only=middle.iter().find_map(|r|{let others=reach(&t,&pinned.iter().filter(|x|*x!=r).copied().collect::<Vec<_>>());reach(&t,&[*r]).into_iter().find(|x|!others.contains(x))})
                .expect("a node only one middle version names");t.remove(&only).unwrap();}
            w.commit().unwrap();}),None,false),
        ("a missing node",Box::new(move|d|{let db=redb::Database::open(d.join(pocold::state_store::FILE)).unwrap();let w=db.begin_write().unwrap();
            {let mut t=w.open_table(nodes).unwrap();let v=t.get(&head_root).unwrap().unwrap().value().to_vec();
             let child=pocol_lp4_execution::node_trie::Node::decode(&v[4..]).unwrap().children().into_iter().next().unwrap();t.remove(&child).unwrap();}w.commit().unwrap();}),None,false),
    ];
    let mut log=vec![];
    for (k,(what,damage,want,either)) in cases.iter().enumerate(){
        let d=e44_dir(&format!("damage-{k}"));copy_dir(&base,&d);damage(&d);
        let mut n=reopen(&e44_conf(&d,every));
        let got=n.stats.resumed_from;log.push((what.to_string(),got,n.stats.checkpoints_refused));
        if *either{assert!(got.is_none()||got==Some(newest.binding.records),"{what}: {got:?}");}else{assert_eq!(got,*want,"{what}: resumed from");}
        if want.is_some(){assert!(n.stats.checkpoints_refused>=1,"{what}: the newest checkpoint was refused");}
        e44_same(&mut n,&mut g,what);
        drop(n);let _=std::fs::remove_dir_all(&d);
    }
    println!("E44-DAMAGE {log:?}");
    drop(g);for d in [base,gd]{let _=std::fs::remove_dir_all(&d);}
}
/// D-6 crash test at each step (`--features crash-test`): the child node aborts the n-th time it reaches
/// a step of a block's commit (inside the state transaction, after it, after the history write, after
/// the journal append), of a checkpoint's write (before it, inside its transaction, after it) or of a
/// release step (inside its transaction, 2026-10-27). After
/// each abort the node opens and is the node replay from genesis gives from the same journal, and a
/// second run without the crash imports every block to the same node.
#[cfg(feature="crash-test")]
#[test] fn e4_checkpoint_crash_at_each_step(){
    use std::io::BufRead;
    let blocks=e44_blocks();let exe=std::env::current_exe().unwrap();let mut log=vec![];
    let run_every=|dir:&std::path::Path,file:&std::path::Path,crash:Option<String>,every:u64|->bool{
        let mut c=std::process::Command::new(&exe);c.args(["e4_checkpoint_kill_child","--exact","--ignored","--nocapture","--test-threads=1"])
            .env("POCOLD_E44_CHILD",dir).env("POCOLD_E44_BLOCKS",file).env("POCOLD_E44_EVERY",every.to_string()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null());
        if let Some(x)=&crash{c.env("POCOLD_CRASH_AT",x);}
        let mut c=c.spawn().unwrap();let lines=std::io::BufReader::new(c.stdout.take().unwrap()).lines();
        let done=lines.map_while(|l|l.ok()).any(|l|l=="E44DONE");
        if done{c.kill().unwrap();}c.wait().unwrap();done};
    let run=|dir:&std::path::Path,file:&std::path::Path,crash:Option<String>|run_every(dir,file,crash,1);
    for (step,ns) in [("state-txn",[2u64,30,90]),("state-commit",[1,25,80]),("history",[3,40,100]),("journal",[5,50,104]),
        ("checkpoint-before",[1,33,99]),("checkpoint-txn",[2,47,103]),("checkpoint-after",[4,60,101]),("release-step",[1,20,60])]{
        for n in ns{
            let d=e44_dir(&format!("step-{step}-{n}"));let file=d.with_extension("blocks");
            {let mut out=vec![];for b in blocks{for p in [b.header.encoded.clone(),b.body.encode()]{out.extend((p.len() as u32).to_be_bytes());out.extend(p);}}std::fs::write(&file,out).unwrap();}
            assert!(!run(&d,&file,Some(format!("{step}:{n}"))),"{step}:{n}: the child stopped at the crash point");
            let (mut g,gd)=e44_reference(&d,&format!("step-ref-{step}-{n}"));
            let mut x=reopen(&e44_conf(&d,1));log.push((step,n,x.chain.head().height,x.stats.resumed_from,x.stats.pins_released));
            e44_same(&mut x,&mut g,&format!("after a crash at {step}:{n}"));drop(x);drop(g);let _=std::fs::remove_dir_all(&gd);
            assert!(run(&d,&file,None),"{step}:{n}: the next run imports every block");
            let (mut g,gd)=e44_reference(&d,&format!("step-ref2-{step}-{n}"));let mut x=reopen(&e44_conf(&d,1));
            assert_eq!(x.chain.head,blocks.last().unwrap().hash(),"{step}:{n}: every block imported");
            e44_same(&mut x,&mut g,&format!("after the run that followed a crash at {step}:{n}"));
            drop(x);drop(g);for p in [d,gd]{let _=std::fs::remove_dir_all(&p);}let _=std::fs::remove_file(&file);
        }
    }
    println!("E44-STEPS {log:?}");
    // A crash during replay, just after a replay checkpoint: replay made its history.redb writes durable
    // first, so the next open resumes from that checkpoint.
    let d=e44_dir("step-replay");let file=d.with_extension("blocks");
    {let mut out=vec![];for b in blocks{for p in [b.header.encoded.clone(),b.body.encode()]{out.extend((p.len() as u32).to_be_bytes());out.extend(p);}}std::fs::write(&file,out).unwrap();}
    assert!(run(&d,&file,None),"a first run imports every block");
    for f in [pocold::state_store::FILE,pocold::history::FILE]{std::fs::remove_file(d.join(f)).unwrap();}
    assert!(!run(&d,&file,Some("checkpoint-after:30".into())),"the replay stops at its 30th checkpoint");
    let (mut g,gd)=e44_reference(&d,"step-replay-ref");let mut x=reopen(&e44_conf(&d,1));
    assert_eq!(x.stats.resumed_from,Some(30),"resumed from the replay checkpoint written before the crash");
    e44_same(&mut x,&mut g,"after a crash just after a replay checkpoint");
    drop(x);drop(g);for p in [d,gd]{let _=std::fs::remove_dir_all(&p);}let _=std::fs::remove_file(&file);
    // 2026-10-28: a crash inside a replayed block's commit, between two replay checkpoints (one every 7 records),
    // or inside a replay checkpoint's transaction: the next open resumes from a checkpoint that was written, or
    // replays from genesis, and is the node replay from genesis gives.
    let mut lazy=vec![];
    for crash in ["state-txn:5","state-txn:23","state-txn:61","checkpoint-txn:4","checkpoint-txn:9"]{
        let d=e44_dir(&format!("lazy-{}",crash.replace(':',"-")));let file=d.with_extension("blocks");
        {let mut out=vec![];for b in blocks{for p in [b.header.encoded.clone(),b.body.encode()]{out.extend((p.len() as u32).to_be_bytes());out.extend(p);}}std::fs::write(&file,out).unwrap();}
        assert!(run_every(&d,&file,None,7),"{crash}: a first run imports every block");
        for f in [pocold::state_store::FILE,pocold::history::FILE]{std::fs::remove_file(d.join(f)).unwrap();}
        assert!(!run_every(&d,&file,Some(crash.into()),7),"{crash}: the replay stops at the crash point");
        let (mut g,gd)=e44_reference(&d,&format!("lazy-ref-{}",crash.replace(':',"-")));let mut x=reopen(&e44_conf(&d,7));
        lazy.push((crash,x.stats.resumed_from,x.chain.head().height));
        assert!(x.stats.resumed_from.map_or(true,|k|k%7==0&&k>0),"{crash}: resumed from a checkpoint: {:?}",x.stats.resumed_from);
        assert_eq!(x.stats.checkpoints_refused,0,"{crash}: no checkpoint is refused");
        e44_same(&mut x,&mut g,&format!("after a crash in replay at {crash}"));
        drop(x);drop(g);for p in [d,gd]{let _=std::fs::remove_dir_all(&p);}let _=std::fs::remove_file(&file);
    }
    println!("E44-LAZY {lazy:?}");
}
/// D-6: a checkpoint entry bound to its sibling's journal record is refused. The siblings share a parent
/// and a target (equal height and total work) and neither holds a stored version, so only the check that
/// the record's header hashes to the entry tells them apart; resumed from such a checkpoint the node would
/// serve one block's header under the other's hash.
#[test] fn e4_checkpoint_refuses_an_entry_bound_to_its_siblings_record(){
    use redb::ReadableTable;
    let mut h=net();let g=h.chain.head;let common=grow(&mut h,g,4,&|n,p|fallback(n,p,0));let fork=common.last().unwrap().hash();
    let sib=fallback(&h,fork,1);let mut h2=net();for x in &common{import(&mut h2,x);}import(&mut h2,&sib);
    let mut side=vec![sib.clone()];for _ in 0..2{let x=fallback(&h2,side.last().unwrap().hash(),0);import(&mut h2,&x);side.push(x);}
    let canon=grow(&mut h,fork,45,&|n,p|fallback(n,p,0));
    let d=e44_dir("sibling");
    {let mut n=reopen(&e44_conf(&d,1000));for x in common.iter().chain(&side).chain(&canon){n.import(x.clone(),None).unwrap();}
     assert_eq!(n.chain.head,canon.last().unwrap().hash());
     for b in [&canon[0],&sib]{let e=n.chain.get(&b.hash()).unwrap();assert!(e.stored.is_none()&&e.pruned,"positive control: neither sibling holds a state (h={})",e.height);}
     assert_eq!(n.chain.get(&canon[0].hash()).unwrap().total_work,n.chain.get(&sib.hash()).unwrap().total_work,"positive control: equal work");
     n.checkpoint_now().unwrap();}
    let (mut g0,gd)=e44_reference(&d,"sibling-ref");
    {let db=redb::Database::open(d.join(pocold::state_store::FILE)).unwrap();let w=db.begin_write().unwrap();
     {let ck:redb::TableDefinition<u64,&[u8]>=redb::TableDefinition::new("checkpoint");let mut t=w.open_table(ck).unwrap();
      let (seq,bytes)={let (k,v)=t.last().unwrap().unwrap();(k.value(),v.value().to_vec())};let mut s=pocold::checkpoint::decode(&bytes).unwrap();
      let i=s.entries.iter().position(|e|e.hash==canon[0].hash()).unwrap();let j=s.entries.iter().position(|e|e.hash==sib.hash()).unwrap();
      let (a,b)=(s.entries[i].body_off,s.entries[j].body_off);s.entries[i].body_off=b;s.entries[j].body_off=a;
      t.insert(seq,pocold::checkpoint::encode_snapshot(&s).as_slice()).unwrap();}
     w.commit().unwrap();}
    let mut n=reopen(&e44_conf(&d,1000));
    assert_eq!(n.stats.resumed_from,None,"the checkpoint is refused");assert_eq!(n.stats.checkpoints_refused,1);
    e44_same(&mut n,&mut g0,"after a sibling-bound checkpoint");
    drop(n);drop(g0);for x in [d,gd]{let _=std::fs::remove_dir_all(&x);}
}

/// D-6 with every kind of entry a checkpoint holds: canonical entries offloaded below a 4-block history
/// window and slimmed below an 8-block slim depth, side blocks offloaded, and side blocks evicted by a side
/// cap of 1 (remembered in the eviction FIFO). Replay from genesis writes checkpoints; the next open resumes
/// from one and is the node replay from genesis gives, field by field.
/// 2026-10-28: a snapshot whose entries are almost all the smallest record (no summary, no root, no body offset:
/// a slimmed, offloaded entry) reads back. decode's count guard took a record to be at least 134 bytes, 8 more than
/// put_rec writes (five u64 fields, not six), so past about 100,000 such entries every checkpoint was refused as
/// "snapshot count beyond its bytes" and open rebuilt the store from genesis (evidence/r4/hend/).
#[test] fn a_snapshot_of_minimal_entries_reads_back(){
    use pocold::checkpoint::*;
    let rec=|i:u64|Rec{hash:{let mut h=[0u8;32];h[..8].copy_from_slice(&(i+1).to_be_bytes());h},body_off:None,import_seq:i,total_work:U256::from(i),flags:0,cum_txs:0,cum_tx_bytes:0,cum_log_bytes:0,drawn:Address::ZERO,summary:None,root:None};
    let snap=|n:u64|Snapshot{genesis:[7;32],params:Params{r_keep:2048,side_cap:256,k_hist:32,side_max:2,history_window:Some(32),slim_depth:Some(100),keep_full:false},
        binding:Binding{records:n,end:1,hash:[2;32]},head:rec(n.max(1)-1).hash,next_seq:n,slim_cursor:1,history_cursor:1,evicted_total:0,unslimmed:vec![],evicted_order:vec![],protect:None,entries:(0..n).map(rec).collect()};
    let s=snap(1000);let b=encode_snapshot(&s);
    assert_eq!((b.len()-encode_snapshot(&snap(0)).len())/1000,126,"the smallest record is 126 bytes");
    assert_eq!(decode(&b).unwrap(),s,"a snapshot of minimal entries reads back");
    // Positive control for the guard itself: one record short is still refused, by the count or as truncated.
    let mut short=b[..b.len()-32-126].to_vec();let seal=revm::primitives::keccak256(&short);short.extend_from_slice(&seal.0);
    assert!(decode(&short).is_err(),"a snapshot missing a record is refused");
}
#[test] fn e4_checkpoint_resumes_slimmed_offloaded_and_evicting_chains(){
    let blocks=e44_blocks();let every=7u64;
    let lim:fn(&mut pocold::node::Limits)=|l|{l.slim_depth=Some(8);l.history_window=Some(4);l.side_cap=1;l.r_keep=24;};
    // A side block whose parent the cap evicted is an orphan here (not journaled): the journal is what live kept.
    let a=e44_dir("kinds");{let mut n=reopen(&e44_conf_with(&a,every,lim));for b in blocks{let _=n.import(b.clone(),None);}}
    let g=e44_dir("kinds-ref");copy_dir(&a,&g);let mut r0=reopen(&e44_conf_with(&g,0,lim));assert_eq!(r0.stats.resumed_from,None);
    let c=e44_dir("kinds-replayed");copy_dir(&a,&c);std::fs::remove_file(c.join(pocold::state_store::FILE)).unwrap();
    drop(reopen(&e44_conf_with(&c,every,lim)));
    let mut n=reopen(&e44_conf_with(&c,every,lim));let records=n.journal_records();assert!(records<blocks.len() as u64,"positive control: orphans were not journaled");
    assert_eq!(n.stats.resumed_from,Some((records/every)*every),"resumed after the newest replay checkpoint");
    let f=pocold::checkpoint::fingerprint(&n.chain).unwrap();let fl=|b:u16|f.entries.iter().filter(|e|e.flags&b!=0).count();
    assert!(fl(pocold::checkpoint::flags::SLIM)>50&&fl(pocold::checkpoint::flags::OFFLOADED)>50,"positive control: slimmed {} and offloaded {} entries",fl(pocold::checkpoint::flags::SLIM),fl(pocold::checkpoint::flags::OFFLOADED));
    assert!(!f.evicted_order.is_empty()&&n.chain.evicted_total>0,"positive control: side blocks were evicted");
    e44_same(&mut n,&mut r0,"resumed with slimmed, offloaded and evicted entries");
    // 2026-10-28: a restored chain rebuilds its side set (`retention_reset`); the resumed node counts the side
    // blocks the node replayed from genesis counts.
    assert!(side_matches(&n.chain)&&side_matches(&r0.chain),"side sets: resumed {:?}, replayed {:?}",n.chain.side_counts(),r0.chain.side_counts());
    // A history.redb that lost the records of one old block whose entry is offloaded and slimmed (restore
    // reads neither its receipts nor its ranges): neither checkpoint is used, and replay from genesis
    // refills the records (C-1 E4.4 retest K11b).
    let newest=pocold::checkpoint::decode(&n.state_store().unwrap().checkpoints().unwrap().last().unwrap().1).unwrap();drop(n);
    let both=pocold::checkpoint::flags::SLIM|pocold::checkpoint::flags::OFFLOADED;
    let old=blocks.iter().find(|b|!b.body.txs.is_empty()&&newest.entries.iter().any(|e|e.hash==b.hash()&&e.flags&both==both)).expect("an offloaded, slimmed block with transactions").hash();
    let h=e44_dir("kinds-history");copy_dir(&c,&h);
    {let db=redb::Database::open(h.join(pocold::history::FILE)).unwrap();let w=db.begin_write().unwrap();
     for t in ["receipts","ranges"]{let d:redb::TableDefinition<&[u8;32],&[u8]>=redb::TableDefinition::new(t);assert!(w.open_table(d).unwrap().remove(&old).unwrap().is_some());}
     w.commit().unwrap();}
    let mut n=reopen(&e44_conf_with(&h,every,lim));assert_eq!(n.stats.resumed_from,None,"no checkpoint naming a block history.redb lost is used");assert!(n.stats.checkpoints_refused>=1);
    e44_same(&mut n,&mut r0,"after history.redb lost an offloaded block's records");
    // A checkpoint written while the node holds side blocks: the node that resumes from it has them in its side set.
    drop(n);let k=e44_dir("kinds-side");copy_dir(&c,&k);
    {let mut n=reopen(&e44_conf_with(&k,every,lim));assert!(n.chain.side_counts().0>0,"positive control: side blocks before the checkpoint");n.checkpoint_now().unwrap();}
    let n=reopen(&e44_conf_with(&k,every,lim));assert_eq!(n.stats.resumed_from,Some(n.journal_records()),"resumed from that checkpoint");
    assert!(side_matches(&n.chain)&&n.chain.side_counts().0>0,"the restored side set: {:?}",n.chain.side_counts());
    drop(n);drop(r0);for d in [a,g,c,h,k]{let _=std::fs::remove_dir_all(&d);}
}

/// D-6 and replay's protect set (D-REORG-REDB: replay keeps a block whose next journal event is a child
/// record from the side cap). The journal of the flood scenario
/// (`a_session_branch_is_adopted_through_a_flood_of_competing_side_branches`) is replayed with one
/// checkpoint just after the last session block that is not adopted yet, S: the flood records between S
/// and its child take replay past cap + protect_max side blocks, each then evicts the lowest-work
/// unprotected leaf, and S (the session tip, the lowest-work leaf) is spared only by its protection.
/// Open resumes from the checkpoint with the protect set the journal gives after its last record (the set
/// the checkpoint seals), so no record is skipped and the node is the one replay from genesis gives. A
/// checkpoint whose sealed protect set is not that one is refused.
#[test] fn e4_checkpoint_resumes_with_replays_protect_set(){
    use redb::ReadableTable;
    let lim:fn(&mut pocold::node::Limits)=|l|{l.r_keep=16;l.side_cap=4;};
    let mut a=net();let g=a.chain.head;
    let ours=grow(&mut a,g,12,&|n,p|fallback(n,p,1));let parent=ours[ours.len()-2].hash();
    let mut noise:Vec<Block>=vec![];for att in 1..44u32{let y=fallback(&a,parent,att);if !noise.iter().any(|z|z.hash()==y.hash()){noise.push(y);}}
    let mut b=net();let theirs=heavier(&mut b,g,a.chain.head().total_work,1);let tip=theirs.last().unwrap().hash();
    let live=e44_dir("protect");
    // after[k]: the journal's records once theirs[k] is journaled (before the flood blocks that follow it).
    let (records,after)={let mut n=reopen(&e44_conf_with(&live,0,lim));for x in &ours{n.import(x.clone(),None).unwrap();}
        let per=noise.len().div_ceil(theirs.len()-1);let mut after=vec![];
        for (k,x) in theirs.iter().enumerate(){n.import_with(x.clone(),None,true).unwrap();after.push(n.journal_records());for y in noise.iter().skip(per*k).take(per){let _=n.import(y.clone(),None);}}
        assert_eq!(n.chain.head,tip);(n.journal_records(),after)};
    let gd=e44_dir("protect-ref");copy_dir(&live,&gd);let mut reference=reopen(&e44_conf_with(&gd,0,lim));
    assert_eq!((reference.stats.resumed_from,reference.stats.replay_skipped,reference.chain.head),(None,0,tip),"the reference replays from genesis, every record");
    // S: the session block before the one that makes the branch heavier (heavier(): the last two are the
    // first heavier block and one more). One replay checkpoint, right after S's record.
    let s_ix=theirs.len()-3;let s_hash=theirs[s_ix].hash();let every=after[s_ix];assert!(2*every>records&&records-every>=3);
    let d=e44_dir("protect-replayed");copy_dir(&live,&d);std::fs::remove_file(d.join(pocold::state_store::FILE)).unwrap();
    drop(reopen(&e44_conf_with(&d,every,lim)));
    let s={let st=pocold::state_store::StateStore::open(&d).unwrap();let c=st.checkpoints().unwrap();assert_eq!(c.len(),1);pocold::checkpoint::decode(&c[0].1).unwrap()};
    assert_eq!(s.binding.records,every);
    assert!(s.protect.as_ref().unwrap().contains(&s_hash),"positive control: the checkpoint names S, which a later record builds on");
    // At the checkpoint ours is canonical: every other block but genesis is a side block. The flood records
    // before S's child (its record is after[s_ix+1]) add one side block each.
    let side=s.entries.iter().filter(|e|e.hash!=g&&!ours.iter().any(|x|x.hash()==e.hash)).count();let flood=(after[s_ix+1]-every-1) as usize;
    assert!(side+flood>4+32,"positive control: replay passes cap + protect_max side blocks before S's child ({side} + {flood})");
    // A copy whose checkpoint seals the set without that session block (re-sealed, its roots record unchanged).
    let t=e44_dir("protect-tampered");copy_dir(&d,&t);
    {let ckpt:redb::TableDefinition<u64,&[u8]>=redb::TableDefinition::new("checkpoint");
     let db=redb::Database::open(t.join(pocold::state_store::FILE)).unwrap();let w=db.begin_write().unwrap();
     {let mut c=w.open_table(ckpt).unwrap();let (seq,bytes)={let (k,v)=c.last().unwrap().unwrap();(k.value(),v.value().to_vec())};
      let mut x=pocold::checkpoint::decode(&bytes).unwrap();let p=x.protect.as_mut().unwrap();let i=p.iter().position(|h|*h==s_hash).unwrap();p.remove(i);
      c.insert(seq,pocold::checkpoint::encode_snapshot(&x).as_slice()).unwrap();}
     w.commit().unwrap();}
    let mut n=reopen(&e44_conf_with(&d,every,lim));
    assert_eq!((n.stats.resumed_from,n.stats.replay_skipped),(Some(every),0),"resumed after the checkpoint, every later record's parent kept");
    e44_same(&mut n,&mut reference,"resumed from a replay checkpoint with replay's protect set");drop(n);
    let mut n=reopen(&e44_conf_with(&t,every,lim));
    assert_eq!(n.stats.resumed_from,None,"a checkpoint sealing another protect set is refused");assert!(n.stats.checkpoints_refused>=1);
    e44_same(&mut n,&mut reference,"after refusing a checkpoint with another protect set");
    drop(n);drop(reference);for x in [live,gd,d,t]{let _=std::fs::remove_dir_all(&x);}
}

/// C-1 E5 (A13 counts one empty state for all entries): every entry that holds no whole state, pruned,
/// slimmed or stored, shares the one empty state; none allocates its own. Whole-state and store mode.
#[test] fn e5_entries_without_a_state_share_one_empty_state(){
    let (_,blocks,_)=s1_chain();
    for store in [false,true]{
        let d=e44_dir(&format!("empty-{store}"));
        let mk=|d:&std::path::Path|{use pocold::node::{Config,Limits};let d=d.to_path_buf();move||Config{dir:d.clone(),name:"e5".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits:Limits{state_store:store,..Default::default()}}};
        let mut n=reopen(&mk(&d));for b in &blocks{n.import(b.clone(),None).unwrap();}
        let check=|n:&pocold::node::Node,what:&str|{let empty=pocold::chain::empty_state();let mut k=0;
            for e in n.chain.blocks.values(){if e.state.accounts.is_empty(){assert!(std::sync::Arc::ptr_eq(&e.state,&empty),"{what}: entry {} has its own empty state",e.height);k+=1;}}
            assert!(k>=60,"positive control ({what}): {k} entries without a state");};
        check(&n,&format!("store={store}, live"));drop(n);let n=reopen(&mk(&d));check(&n,&format!("store={store}, reopened"));
        drop(n);let _=std::fs::remove_dir_all(&d);
    }
}

/// C-1 E5: a closing node leaves its entries' versions pinned (StateStore::close) rather than releasing each
/// in a transaction of its own; the next open releases them in one durable transaction (the pin reset it
/// does after a crash) and is the node replay from genesis gives.
#[test] fn e5_a_closing_node_leaves_its_versions_to_the_next_open(){
    let blocks=e44_blocks();let a=e44_dir("close");
    let stored={let mut n=reopen(&e44_conf(&a,7));for b in &blocks[..80]{n.import(b.clone(),None).unwrap();}
        let st=n.state_store().unwrap().clone();assert_eq!(st.pins().unwrap().iter().map(|x|u64::from(x.1)).sum::<u64>(),st.checkpoint_pins().unwrap()+n.chain.blocks.values().filter(|e|e.stored.is_some()).count() as u64);
        n.chain.blocks.values().filter(|e|e.stored.is_some()).count() as u64};
    assert!(stored>=8,"positive control: {stored} stored versions");
    {let st=pocold::state_store::StateStore::open(&a).unwrap();let all:u64=st.pins().unwrap().iter().map(|x|u64::from(x.1)).sum();
     assert_eq!(all-st.checkpoint_pins().unwrap(),stored,"the closed node's versions are still pinned");}
    let n=reopen(&e44_conf(&a,7));assert_eq!(n.stats.resumed_from,Some(77));assert_eq!(n.stats.pins_released,stored,"the next open released them");
    drop(n);let (mut g,gd)=e44_reference(&a,"close-ref");let mut n=reopen(&e44_conf(&a,7));
    e44_same(&mut n,&mut g,"after a close that left its versions pinned");
    drop(n);drop(g);for d in [a,gd]{let _=std::fs::remove_dir_all(&d);}
}
/// D-7, within the resource limits: A13 counts the pool at its gas bound, so a bound the budget cannot hold is
/// refused at open (here the largest, which the count bound caps at POOL_MAX_TXS × GAS_LIMIT), and one it can
/// is taken.
#[test] fn a_pool_gas_bound_past_the_memory_budget_is_refused_at_open(){
    use pocold::node::Limits;
    let (huge,d0)=gas_node("huge",Limits{pool_gas_max:Some(u64::MAX),..Default::default()});
    match huge{Err(e)=>assert!(e.starts_with("A13 memory bound"),"{e}"),Ok(_)=>panic!("a pool gas bound past the budget opened")}
    let (fits,d1)=gas_node("fits",Limits{pool_gas_max:Some(16*8_000_000),..Default::default()});assert!(fits.is_ok(),"16 × GAS_LIMIT fits the devnet budget");
    drop(fits);for d in [d0,d1]{let _=std::fs::remove_dir_all(&d);}
}


#[test]
fn continuous_admission_caps_and_finality_are_not_a_large_finite_ceiling(){
    let mut c=cfg(&["a","b","c"]);c.h_end=0;c.h_close=0;c.tx_cap=1;c.txbytes_cap=1024;
    let mut n=net_with(&c);
    assert_eq!(n.chain.spec.cp().H_END,0);assert_eq!(n.chain.spec.cp().T_END,0);
    let a=devnet::account_key("alice");let to=devnet::account_key("bob").address();
    for nonce in 0..4{
        let raw=Tx{chain_id:c.chain_id,nonce,max_priority:U256::ZERO,max_fee:U256::from(100_000_000_000u64),gas:21000,to:Some(to),value:U256::from(1),data:Default::default()}.sign_1559(&a);
        assert!(n.chain.within_caps(n.chain.head(),&[raw.clone()]));
        assert!(!n.chain.within_caps(n.chain.head(),&[raw.clone(),raw.clone()]),"per-block cap retained");
        let b=signed(&n,n.chain.head,Body{txs:vec![raw],evidence:vec![]},0);import(&mut n,&b);
    }
    assert_eq!(n.chain.head().state.accounts[&to].info.balance,U256::from(10u128.pow(24))+U256::from(4));
    let cp=n.chain.spec.cp();let mut limits=pocold::node::Limits::default();limits.state_store=true;
    assert!(pocold::resources::derive(cp,32,2,&limits,3).is_none(),"unknown continuous history must not be treated as zero");
    limits.continuous_history=Some(0);let small=pocold::resources::derive(cp,32,2,&limits,3).unwrap();
    limits.continuous_history=Some(50_000);let larger=pocold::resources::derive(cp,32,2,&limits,3).unwrap();assert!(larger.total>small.total);
}
