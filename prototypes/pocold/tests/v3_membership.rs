//! V3-01/13 and the neutral ledger-owner iteration, on private fixture chains.
use std::collections::HashMap;
use pocol_lp4_execution::{apply,devnet::{self,DevnetConfig},txsign::{Key,Tx}};
use pocold::{assignment::{DrawWeights,Membership,ShareCount},chain::{Chain,Entry,Spec},hashes,types::*};
use revm::primitives::{keccak256,Address,Bytes,B256,U256};

struct Net { chain:Chain, keys:HashMap<Address,Key>, miners:Vec<Address> }
fn net(v3:bool,full:bool,earnin:bool,n:usize)->Net {
    let mut cfg=DevnetConfig::new(777936,1_700_000_000,&[]);
    cfg.miners=(0..n).map(|i|format!("membership-{i}")).collect();
    cfg.hash_rate=16;cfg.c=8;cfg.d_att=73;cfg.w_act=4;cfg.r_max=1;
    cfg.spec_version=if v3{2}else{1};cfg.nonce_mode=if full{1}else{0};cfg.earnin=earnin;
    cfg.m_max=cfg.m_max.max(n as u16);
    cfg.accounts.push(("carol".into(),U256::from(10u128.pow(24))));
    let candidate=devnet::build(&cfg).unwrap();
    let mut keys:HashMap<_,_>=candidate.miners.iter().map(|m|(m.id,Key::from_secret(&m.key.secret()).unwrap())).collect();
    for label in ["alice","bob","carol"] {let key=devnet::account_key(label);keys.insert(key.address(),key);}
    let miners=candidate.miners.iter().map(|m|m.id).collect();
    let (admitted,_)=candidate.admit(&pocold::single_host_local()).unwrap();
    Net{chain:Chain::new(Spec::new(admitted)),keys,miners}
}
fn solve(n:&Net,miner:Address,txs:Vec<Vec<u8>>)->Block {
    solve_template(n,miner,txs,false)
}
fn solve_template(n:&Net,miner:Address,txs:Vec<Vec<u8>>,fallback:bool)->Block {
    try_solve_template(n,miner,txs,fallback,0).expect("deterministic fixture has no solution")
}
/// A signed template is attempt 0; a fallback may be at a later `attempt`.
fn try_solve_template(n:&Net,miner:Address,txs:Vec<Vec<u8>>,fallback:bool,attempt:u32)->Option<Block> {
    assert!(fallback||attempt==0);
    let parent=n.chain.full_entry(&n.chain.head).unwrap();
    let ctx=n.chain.child_ctx(&parent,attempt).unwrap();
    let range=ctx.assignment.ranges().iter().find(|r|r.id()==raw_addr(miner)).expect("miner owns a range");
    let body=Body{txs,evidence:vec![]};
    let first=if fallback {assert!(body.txs.is_empty());ctx.window.fallback_timestamp(n.chain.spec.cp().dFbWait).unwrap()}
        else{parent.ts+u64::from(n.chain.spec.cp().T_blk)};
    let last=if fallback {first}else{ctx.window.upper()};
    for ts in first..=last {
        let (mut header,_,_)=if fallback {n.chain.build_fallback(&parent,attempt).unwrap()}
            else{n.chain.build_signed(&parent,&n.keys[&ctx.proposer],ts,body.clone(),vec![]).unwrap()};
        let tid=template_id(&header.ut_raw);let mut shares=Vec::new();
        let end=range.end_exclusive().min(range.start()+1_000_000);
        for nonce in range.start()..end {
            let hash=pow_hash(&tid,nonce as u64);
            if le_target(&hash,&ctx.target) {
                header.nonce=nonce as u64;header.shares=shares;reseal(&mut header);
                header.winner_sig=n.keys[&miner].sign_hash(&B256::from(hashes::win_msg(&tid,nonce as u64,&share_root(&header))));
                reseal(&mut header);return Some(Block{header,body});
            }
            if shares.len()<usize::from(n.chain.spec.cp().S_max)&&le_target(&hash,&ctx.t_share){shares.push(nonce as u64);}
        }
    }
    None
}
fn accept(n:&mut Net,b:Block) {
    let (entry,ctx)=n.chain.validate(&b,None,0).unwrap();
    assert_eq!(entry.drawn,ctx.proposer,"type-1 evidence keeps this branch's actual draw");
    let ranges:Vec<_>=ctx.assignment.ranges().iter().map(|r|(addr(&r.id()),r.start(),r.end_exclusive())).collect();
    assert_eq!(entry.ranges,ranges,"historical ownership retains the actual ranges");
    assert!(n.chain.insert(entry));
}
fn next(n:&mut Net,miner:Address,txs:Vec<Vec<u8>>) {let b=solve(n,miner,txs);accept(n,b);}
fn old_draw(n:&Net,p:&Entry,m:&Membership,h:u64)->DrawWeights {
    let cp=n.chain.spec.cp();let mut rows=Vec::new();
    let lo=h.saturating_sub(1).saturating_sub(u64::from(cp.W_w)).max(1);
    if h>=2 {for j in lo..=h-2 {for id in m.ids() {
        let count=apply::share_count(p.state.as_ref(),j,addr(id));
        if count>0 {rows.push(ShareCount{height:j,id:*id,count});}
    }}}
    DrawWeights::new(m,cp.W_w,&rows).unwrap()
}
fn assert_draw_parity(n:&Net) {
    let p=n.chain.full_entry(&n.chain.head).unwrap();let h=p.height+1;
    let m=n.chain.membership(&p,h).unwrap();let old=old_draw(n,&p,&m,h);
    let new=n.chain.draw_weights(&p,&m,h).unwrap();
    assert_eq!(new.weights(),old.weights());assert_eq!(new.total(),old.total());
    assert_eq!(new.draw().unwrap(),old.draw().unwrap());
    assert_eq!(n.chain.draw(&p,&m,h).unwrap(),addr(&old.draw().unwrap().0));
}
/// The shares `id` holds in the draw window of height h (heights max(1, h-1-W_w)..=h-2).
#[cfg_attr(not(feature="v3-preview"),allow(dead_code))]
fn window_shares(n:&Net,p:&Entry,h:u64,id:Address)->u64 {
    let lo=h.saturating_sub(1).saturating_sub(u64::from(n.chain.spec.cp().W_w)).max(1);
    if h<2 {return 0}(lo..=h-2).map(|j|apply::share_count(p.state.as_ref(),j,id)).sum()
}
fn tx(n:&Net,key:Address,to:Address,value:U256,data:Vec<u8>,gas:u64,nonce:Option<u64>)->Vec<u8> {
    let p=n.chain.full_entry(&n.chain.head).unwrap();let base=n.chain.child_ctx(&p,0).unwrap().base_fee;
    let nonce=nonce.unwrap_or_else(||p.state.accounts.get(&key).map_or(0,|a|a.info.nonce));
    let priority=U256::from(1_000_000_000u64);
    Tx{chain_id:n.chain.spec.chain_id(),nonce,max_priority:priority,max_fee:base*U256::from(2)+priority,
        gas,to:Some(to),value,data:Bytes::from(data)}.sign_1559(&n.keys[&key])
}
fn registration(n:&Net,key:Address,value:U256,nonce:Option<u64>)->Vec<u8> {
    let mut data=keccak256("register(address)")[..4].to_vec();data.extend_from_slice(key.into_word().as_slice());
    tx(n,key,apply::registry(),value,data,1_000_000,nonce)
}

#[test]
fn legacy_bounded_full_nonce_and_earnin_membership_and_draw_stay_unchanged() {
    for (full,earnin) in [(false,false),(true,false),(true,true)] {
        let mut n=net(false,full,earnin,3);let x=n.miners[0];
        for _ in 0..6 {assert_draw_parity(&n);next(&mut n,x,vec![]);}
        assert_draw_parity(&n);
        let p=n.chain.full_entry(&n.chain.head).unwrap();let h=p.height+1;
        assert!(!n.chain.spec.v3_active(h));
        assert_eq!(n.chain.membership(&p,h).unwrap().ids(),&[raw_addr(x)]);
        let mut copy=solve(&n,x,vec![]);let y=n.miners[1];
        let tid=template_id(&copy.header.ut_raw);
        copy.header.winner_sig=n.keys[&y].sign_hash(&B256::from(hashes::win_msg(&tid,copy.header.nonce,&share_root(&copy.header))));
        reseal(&mut copy.header);
        assert!(matches!(n.chain.validate(&copy,None,0),Err(pocold::chain::Reject::Envelope(ref why)) if why.starts_with("13 ")));
    }
}

#[cfg(feature="v3-preview")]
mod preview {
    use super::*;
    // pr15-2 pins (deterministic fixture): fallback blocks mined after the operators went idle, and the first height
    // whose drawn proposer can sign. The fixture chain runs the v3 program set (V3-PR-E0), whose bytes enter
    // the state root and so every block hash, nonce draw and solution: the unbounded pins moved from 16/19 to 15/18 (V3-PR-E0) to 14/17 (V3-PR-E1), to 13/16 (V3-PR-E2b), to 11/14 (V3-PR-E3) and to 12/15 (V3-PR-E2a: the period grew by one block. Besides the
    // bytes, E2a's reward flow, M3 issuance and the carry lagged by one block change what each fallback pays toward the
    // probationers' bonds, which likely moves the height where a drawn probationer reaches B_reg/4; this cause is inferred,
    // not traced block by block), to 11/14 (V3-PR-E4: the contest and epoch-counter bytes of PoColSystemV3.sol change
    // the state root again; the bounded pins and every behaviour assertion are unchanged) and to 13/16 (V3-PR-E4, the
    // owner decision of 2026-11-13 on the pool-contest victim: MinerRegistry's new bytes, POOL_OUT and the exit of a pool
    // victim, change the state root again; no contest runs in this fixture, so the cause is the bytes alone; the bounded
    // pins and every behaviour assertion are unchanged).
    const UNBOUNDED_PERIOD:usize=13;const UNBOUNDED_RECOVERY:u64=16;
    const BOUNDED_PERIOD:usize=4;const BOUNDED_RECOVERY:u64=7;
    #[test]
    fn another_live_miner_can_mine_after_the_only_recent_miner_stops() {
        let mut n=net(true,true,true,3);let (x,y)=(n.miners[0],n.miners[1]);
        for _ in 0..6 {next(&mut n,x,vec![]);assert_draw_parity(&n);}
        let p=n.chain.full_entry(&n.chain.head).unwrap();
        assert_eq!(n.chain.membership(&p,p.height+1).unwrap().ids().len(),3);
        // X cannot supply a signed template after it stops. Y needs only its own
        // winner signature on the fallback, whose proposer and signature are empty.
        n.keys.remove(&x).unwrap();
        let b=solve_template(&n,y,vec![],true);
        assert!(b.header.proposer.is_empty()&&b.header.sig.is_empty());
        accept(&mut n,b);assert_draw_parity(&n);
        assert_eq!(n.chain.head().height,7);
    }
    #[test]
    fn bounded_v3_ranges_keep_the_activity_rule() {
        let mut n=net(true,false,false,3);let x=n.miners[0];
        for _ in 0..6 {next(&mut n,x,vec![]);}
        let p=n.chain.full_entry(&n.chain.head).unwrap();
        assert!(n.chain.spec.v3_active(p.height+1));
        assert_eq!(n.chain.membership(&p,p.height+1).unwrap().ids(),&[raw_addr(x)]);
        assert_draw_parity(&n);
    }
    #[test]
    fn a_probationer_at_the_quarter_bond_is_eligible_and_one_wei_below_is_excluded() {
        let mut n=net(true,true,true,3);let x=n.miners[0];
        let (a,b)=(devnet::account_key("alice").address(),devnet::account_key("bob").address());
        let quarter=pocold::chain::from_fixed_lp4(&n.chain.spec.cp().B_reg)/U256::from(4);
        let t=registration(&n,a,quarter-U256::from(1),None);next(&mut n,x,vec![t]);
        let t=registration(&n,b,quarter,None);next(&mut n,x,vec![t]);
        // Both probationers mine, so both hold shares in the draw window: only the bond rule can zero one.
        next(&mut n,a,vec![]);next(&mut n,b,vec![]);next(&mut n,a,vec![]);next(&mut n,b,vec![]);
        let p=n.chain.full_entry(&n.chain.head).unwrap();let h=p.height+1;
        assert_eq!(apply::miner_record(p.state.as_ref(),a).unwrap().bond,quarter-U256::from(1));
        let rb=apply::miner_record(p.state.as_ref(),b).unwrap();assert_eq!((rb.bond_state,rb.bond),(4,quarter));
        let window=|id:Address|window_shares(&n,&p,h,id);
        assert!(window(a)>0&&window(b)>0,"each probationer has shares that would weigh in the draw");
        let members=n.chain.membership(&p,h).unwrap();
        assert!(members.accruing().contains(&raw_addr(a)));assert!(!members.accruing().contains(&raw_addr(b)));
        let weights=n.chain.draw_weights(&p,&members,h).unwrap();
        assert_eq!(weights.weights().iter().find(|r|r.0==raw_addr(a)).unwrap().1,0,"one wei below B_reg/4: its shares do not count");
        assert_eq!(weights.weights().iter().find(|r|r.0==raw_addr(b)).unwrap().1,u128::from(window(b)),"at B_reg/4: every share counts");
        assert_draw_parity(&n);
    }
    /// pr15-2. Bonded operators stay registered but stop mining (they sign nothing); only probationers registered with
    /// no bond mine. What the code does, pinned:
    /// - Unbounded nonce ranges (all-live membership): the idle operators stay members. Their shares age out of the
    ///   draw window, the probationers' shares do not count below B_reg/4, so D2 gives each operator weight 1 and
    ///   every draw picks an idle operator. No signed template can exist: a fallback-only period, mined by the
    ///   probationers, whose fallback rewards fill their bonds. Signed blocks resume at the first height where a
    ///   probationer's accrued bond has reached B_reg/4: its shares count, the operators weigh 0, and it is drawn.
    /// - Bounded ranges (the activity rule): the idle operators leave the membership after W_act heights. Every
    ///   member left is a sub-quarter probationer, so the all-probationer exception makes them candidates and a
    ///   probationer is drawn before any of them reaches B_reg/4.
    #[test]
    fn idle_bonded_operators_leave_a_fallback_only_period_until_a_probationer_can_be_drawn() {
        for (full,period,recovery) in [(true,UNBOUNDED_PERIOD,UNBOUNDED_RECOVERY),(false,BOUNDED_PERIOD,BOUNDED_RECOVERY)] {
            let mut n=net(true,full,true,3);let x=n.miners[0];let ops=n.miners.clone();
            let (a,b)=(devnet::account_key("alice").address(),devnet::account_key("bob").address());
            let quarter=pocold::chain::from_fixed_lp4(&n.chain.spec.cp().B_reg)/U256::from(4);
            let t=registration(&n,a,U256::ZERO,None);next(&mut n,x,vec![t]);
            let t=registration(&n,b,U256::ZERO,None);next(&mut n,x,vec![t]);
            for id in &ops {n.keys.remove(id).unwrap();}
            let mut fallbacks=0;
            let (h,drawn)=loop {
                let p=n.chain.full_entry(&n.chain.head).unwrap();let h=p.height+1;assert!(h<64,"full={full}: no recovery");
                let members=n.chain.membership(&p,h).unwrap();let weights=n.chain.draw_weights(&p,&members,h).unwrap();
                let drawn=n.chain.draw(&p,&members,h).unwrap();
                let record=|id:Address|apply::miner_record(p.state.as_ref(),id).unwrap();
                let weight=|id:Address|weights.weights().iter().find(|r|r.0==raw_addr(id)).map_or(0,|r|r.1);
                if !ops.contains(&drawn) {
                    for id in &ops {assert_eq!(weight(*id),0,"full={full}: an idle operator weighs nothing at recovery");}
                    let r=record(drawn);
                    if full {
                        assert_eq!(members.ids().len(),5,"the idle operators are still members");
                        assert!(r.bond>=quarter,"recovery needs the drawn probationer at B_reg/4");
                        assert!(!members.accruing().contains(&raw_addr(drawn)));
                    }else{
                        assert_eq!(members.ids(),{let mut v=vec![raw_addr(a),raw_addr(b)];v.sort();v}.as_slice(),"the activity rule removed the operators");
                        assert_eq!(members.accruing().len(),2,"all members are sub-quarter probationers");
                        assert!(r.bond<quarter,"the all-probationer exception, not the bond, makes it drawable");
                    }
                    break (h,drawn);
                }
                // The fallback-only period: an idle operator is drawn and the probationers' shares weigh nothing.
                for id in [a,b] {let r=record(id);assert_eq!(r.bond_state,4);assert!(r.bond<quarter);assert_eq!(weight(id),0,"full={full} h={h}");}
                // A fallback has one timestamp. A probationer whose range holds a solution mines it; when only the idle
                // operators' ranges hold one, the attempt passes and the next fallback attempt is mined.
                let (first,second)=if h%2==0 {(a,b)}else{(b,a)};
                let blk=(0..8).find_map(|k|try_solve_template(&n,first,vec![],true,k).or_else(||try_solve_template(&n,second,vec![],true,k))).expect("a probationer solves a fallback");
                assert!(blk.header.proposer.is_empty()&&blk.header.sig.is_empty());accept(&mut n,blk);fallbacks+=1;assert_draw_parity(&n);
            };
            assert_eq!((fallbacks,h),(period,recovery),"full={full}: the pinned fallback-only period and the first drawable height");
            // The drawn probationer signs again: signed precedence and proposer rewards resume.
            let blk=solve(&n,drawn,vec![]);assert_eq!(Address::from_slice(&blk.header.proposer),drawn);
            accept(&mut n,blk);assert_draw_parity(&n);
        }
    }
    #[test]
    fn all_probationers_below_the_quarter_bond_retain_the_owner_approved_draw_exception() {
        let mut n=net(true,true,true,3);let x=n.miners[0];
        let (a,b,c)=(devnet::account_key("alice").address(),devnet::account_key("bob").address(),devnet::account_key("carol").address());
        // Fund the existing miner identities so they can sign their own exits. Everything is
        // on this disposable fixture chain; the three transfers and registration are ordered.
        let mut txs:Vec<_>=n.miners.iter().enumerate().map(|(i,id)|tx(&n,a,*id,U256::from(100_000_000_000_000_000_000u128),vec![],21_000,Some(i as u64))).collect();
        txs.push(registration(&n,a,U256::ZERO,Some(3)));next(&mut n,x,txs);
        let t=registration(&n,b,U256::ZERO,None);next(&mut n,a,vec![t]);
        let t=registration(&n,c,U256::ZERO,None);next(&mut n,a,vec![t]);
        for miner in n.miners.clone() {
            let t=tx(&n,miner,apply::registry(),U256::ZERO,keccak256("deregister()")[..4].to_vec(),1_000_000,None);
            next(&mut n,a,vec![t]);
        }
        let p=n.chain.full_entry(&n.chain.head).unwrap();let h=p.height+1;
        let quarter=pocold::chain::from_fixed_lp4(&n.chain.spec.cp().B_reg)/U256::from(4);
        let members=n.chain.membership(&p,h).unwrap();assert_eq!(members.ids().len(),3);
        for id in members.ids() {let r=apply::miner_record(p.state.as_ref(),addr(id)).unwrap();assert_eq!(r.bond_state,4);assert!(r.bond<quarter);}
        assert_eq!(members.accruing().len(),members.ids().len());
        assert!(n.chain.draw_weights(&p,&members,h).unwrap().total()>0);
        next(&mut n,a,vec![]);assert_draw_parity(&n);
    }
}

struct Random(u64);
impl Random {fn next(&mut self)->u64 {self.0^=self.0<<13;self.0^=self.0>>7;self.0^=self.0<<17;self.0}}
fn map_slot(j:u64,k:u64)->U256 {U256::from_be_bytes(keccak256([U256::from(j).to_be_bytes::<32>(),U256::from(k).to_be_bytes::<32>()].concat()).0)}
fn put(db:&mut revm::InMemoryDB,a:Address,k:U256,v:U256) {db.insert_account_storage(a,k,v).unwrap();}
/// Populate the same distinct-owner array and count map that ShareLedger.record writes.
fn ledger_rows(db:&mut revm::InMemoryDB,j:u64,rows:&[(Address,u64)]) {
    let len=map_slot(j,2);let base=U256::from_be_bytes(keccak256(len.to_be_bytes::<32>()).0);
    let counts=map_slot(j,1);let total: u64=rows.iter().map(|r|r.1).sum();
    put(db,apply::ledger(),map_slot(j,0),U256::from(total));
    put(db,apply::ledger(),len,U256::from(rows.len()));
    for (i,(owner,count)) in rows.iter().enumerate() {
        put(db,apply::ledger(),base+U256::from(i),U256::from_be_slice(owner.as_slice()));
        let key=U256::from_be_bytes(keccak256([owner.into_word().as_slice(),counts.to_be_bytes::<32>().as_slice()].concat()).0);
        put(db,apply::ledger(),key,U256::from(*count));
    }
}
#[test]
fn streamed_owner_weights_match_the_original_member_scan_over_randomized_ledgers() {
    let mut cases=vec![(false,false,false,3),(false,true,false,16),(false,true,true,16)];
    if pocol_lp4_execution::V3_PREVIEW {cases.extend([(true,false,false,3),(true,true,true,16),(true,true,true,256)]);}
    let mut rng=Random(0xc120261111);
    for (v3,full,earnin,count) in cases {
        let n=net(v3,full,earnin,count);let genesis=n.chain.full_entry(&n.chain.head).unwrap();
        let cp=n.chain.spec.cp();let quarter=pocold::chain::from_fixed_lp4(&cp.B_reg)/U256::from(4);
        for round in 0..96u64 {
            // These are storage-view fixtures, not imported headers. Both ledger representations
            // obey the contract's uniqueness/count bounds, including non-sorted owner order.
            let h=if round<4 {round+1}else{20+rng.next()%500};
            let mut p=genesis.clone();p.height=h-1;
            for b in &mut p.seal {*b=rng.next() as u8;}
            let mut db=(*genesis.state).clone();let mut last=HashMap::new();
            let lo=h.saturating_sub(1).saturating_sub(u64::from(cp.W_w)).max(1);
            if h>=2 {for j in lo..=h-2 {
                let mut order=n.miners.clone();
                for i in (1..order.len()).rev() {let k=rng.next() as usize%(i+1);order.swap(i,k);}
                let take=if round%12==0 {0}else{rng.next() as usize%(order.len()+1)};
                let (mut rows,mut left)=(Vec::new(),u64::from(cp.S_max)+1);
                for owner in order.into_iter().take(take) {
                    if left==0 {break}let c=(1+rng.next()%4).min(left);left-=c;
                    rows.push((owner,c));last.insert(owner,j);
                }
                ledger_rows(&mut db,j,&rows);
                assert_eq!(apply::share_owners(&db,j).collect::<Vec<_>>(),rows.iter().map(|r|r.0).collect::<Vec<_>>());
            }}
            let mut live=Vec::new();let mut active=Vec::new();
            for (i,id) in n.miners.iter().enumerate() {
                let tomb=i>0&&rng.next()%7==0;
                let accruing=earnin&&!tomb&&i<8&&rng.next()%2==0;
                let state=if tomb {3}else if accruing {4}else{1};
                let bond=if tomb {U256::ZERO}else if accruing {
                    match rng.next()%3 {0=>quarter-U256::from(1),1=>quarter,_=>quarter+U256::from(1)}
                }else{pocold::chain::from_fixed_lp4(&cp.B_reg)};
                let shared=last.get(id).copied().unwrap_or(0);
                let grace=if rng.next()%3==0 {p.height}else{0};
                let slot=apply::record_slot(*id);let old=db.accounts[&apply::registry()].storage[&slot];
                let clear:U256=(U256::from(u64::MAX)<<88)|(U256::from(255)<<16)|(U256::from(255)<<8);
                put(&mut db,apply::registry(),slot,(old&!clear)|(U256::from(shared)<<88)|(U256::from(state)<<16)|(U256::from(tomb as u8)<<8));
                put(&mut db,apply::registry(),slot+U256::from(2),bond);
                put(&mut db,apply::registry(),slot+U256::from(3),U256::from(grace)<<192);
                if !tomb {live.push(raw_addr(*id));if shared>=h.saturating_sub(u64::from(cp.W_act))||grace>=h.saturating_sub(u64::from(cp.W_act)){active.push(raw_addr(*id));}}
            }
            p.state=db.into();let members=n.chain.membership(&p,h).unwrap();
            if !v3||!full {
                let mut expected=if active.is_empty(){live}else{active};expected.sort();
                assert_eq!(members.ids(),expected.as_slice(),"legacy activity selection remains exact");
            }
            let before=old_draw(&n,&p,&members,h);let after=n.chain.draw_weights(&p,&members,h).unwrap();
            assert_eq!(before.weights(),after.weights(),"v3={v3} full={full} earnin={earnin} count={count} h={h}");
            assert_eq!(before.total(),after.total());assert_eq!(before.draw().unwrap(),after.draw().unwrap());
        }
    }
}

#[cfg(feature="v3-preview")]
#[test]
fn historical_v3_draws_and_ranges_still_authenticate_type1_evidence() {
    use pocold::rlp;
    let mut n=net(true,true,true,3);let (x,y)=(n.miners[0],n.miners[1]);
    for _ in 0..6 {next(&mut n,x,vec![]);}
    let p=n.chain.full_entry(&n.chain.head).unwrap();let ctx=n.chain.child_ctx(&p,0).unwrap();
    let (a,_,_)=n.chain.build_signed(&p,&n.keys[&ctx.proposer],ctx.window.lower(),Body::default(),vec![]).unwrap();
    let (b,_,_)=n.chain.build_signed(&p,&n.keys[&ctx.proposer],ctx.window.lower()+1,Body::default(),vec![]).unwrap();
    let signed=|h:&pocold::header::Header| {
        let mut payload=h.ut_raw.clone();rlp::encode_bytes(&h.sig,&mut payload);
        let mut out=Vec::new();rlp::encode_list_payload(&payload,&mut out);out
    };
    let mut payload=Vec::new();rlp::encode_bytes(&[1],&mut payload);payload.extend(signed(&a));payload.extend(signed(&b));
    let mut evidence=Vec::new();rlp::encode_list_payload(&payload,&mut evidence);
    next(&mut n,y,vec![]);
    let p=n.chain.full_entry(&n.chain.head).unwrap();assert_eq!(p.drawn,ctx.proposer);assert_eq!(p.ranges.len(),3);
    let effects=n.chain.evidence_effects(&p,&Body{txs:vec![],evidence:vec![evidence]},p.height+1).unwrap();
    assert_eq!(effects,vec![apply::EvidenceEffect::Punish{accused:ctx.proposer}]);
}

fn mixed_owner_block(n:&Net,winner:Address)->Block {
    let p=n.chain.full_entry(&n.chain.head).unwrap();let ctx=n.chain.child_ctx(&p,0).unwrap();
    let winning=ctx.assignment.ranges().iter().find(|r|r.id()==raw_addr(winner)).unwrap();
    'templates: for ts in p.ts+u64::from(n.chain.spec.cp().T_blk)..=ctx.window.upper() {
        let (mut h,_,_)=n.chain.build_signed(&p,&n.keys[&ctx.proposer],ts,Body::default(),vec![]).unwrap();
        let tid=template_id(&h.ut_raw);let mut shares=Vec::new();
        for range in ctx.assignment.ranges() {
            let found=(range.start()..range.end_exclusive().min(range.start()+1_000_000))
                .find(|nonce|le_target(&pow_hash(&tid,*nonce as u64),&ctx.t_share));
            match found {Some(nonce)=>shares.push(nonce as u64),None=>continue 'templates}
        }
        shares.sort_unstable();
        let found=(winning.start()..winning.end_exclusive().min(winning.start()+1_000_000))
            .find(|nonce|!shares.contains(&(*nonce as u64))&&le_target(&pow_hash(&tid,*nonce as u64),&ctx.target));
        if let Some(nonce)=found {
            h.nonce=nonce as u64;h.shares=shares;reseal(&mut h);
            h.winner_sig=n.keys[&winner].sign_hash(&B256::from(hashes::win_msg(&tid,h.nonce,&share_root(&h))));
            reseal(&mut h);return Block{header:h,body:Body::default()};
        }
    }
    panic!("deterministic mixed-owner fixture has no solution");
}
#[test]
fn multiple_owners_written_by_the_vm_keep_the_original_draw_weights() {
    let mut profiles=vec![(false,false,false),(false,true,true)];
    if pocol_lp4_execution::V3_PREVIEW {profiles.push((true,true,true));}
    for (v3,full,earnin) in profiles {
        let mut n=net(v3,full,earnin,3);let x=n.miners[0];
        let b=mixed_owner_block(&n,x);accept(&mut n,b);next(&mut n,x,vec![]);
        let p=n.chain.full_entry(&n.chain.head).unwrap();
        assert_eq!(apply::share_owners(p.state.as_ref(),1).count(),3);
        for id in &n.miners {assert_eq!(apply::share_count(p.state.as_ref(),1,*id),if *id==x {2}else{1});}
        assert_draw_parity(&n);
    }
}
