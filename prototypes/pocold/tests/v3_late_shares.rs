//! V3-PR-E3, late share credit (V3-02; V3-DESIGN.md, V3-DECISIONS.md D-V3-02): the kind-3 evidence rules of chain.rs.
//!
//! - The validity vectors of the independent model (lp4-execution/tests/v3_model.py `late_validity_vectors`,
//!   tests/v3/late-share-validity-vectors.json) replay through chain.rs `late_parse`, `late_window` and `late_check`:
//!   every case gives the model's verdict (the same reason at the same item, or the same owners per template). The model
//!   also plants 17 mutants of these rules, each of which fails some vector (`v3_model.py --check`).
//! - Real v3 chains (a specVersion-2 devnet, the v3 program set): the omitted share of a
//!   contributor is credited by LS(j) in block j+1 and paid at settle(j), which a spec-1 chain refuses as an evidence
//!   form; the negative cases on real branches (a fallback carrier, a sibling template's share, a duplicate across
//!   carriers, the cap read from the parent state); a reorg across a carrier rolls the credit and its deduplication
//!   back; replay determinism; the Escrow invariant on every block.
#[path="support/members.rs"] mod members_sweep;
use pocold::chain::{late_check,late_parse,late_window,LateBase,LateBlock,LateRules};
use pocold::types::Fixed;
use revm::primitives::{keccak256,Address};
use std::collections::BTreeSet;

const VECTORS:&str=include_str!("../../lp4-execution/tests/v3/late-share-validity-vectors.json");
fn name(n:&str)->Address{Address::from_slice(&keccak256(format!("earnin:{n}"))[12..])}
fn unhex(s:&str)->Vec<u8>{(0..s.len()).step_by(2).map(|i|u8::from_str_radix(&s[i..i+2],16).unwrap()).collect()}

/// One case through chain.rs's kind-3 rules, as the model runs it: the verdict in the vectors' shape.
fn verdict(case:&serde_json::Value)->serde_json::Value{
    let r=&case["rules"];let u=|k:&str|r[k].as_u64().unwrap();
    let rules=LateRules{h:u("h"),d:u("D"),s_max:u("S_max"),kappa_m:u("kappa_m")};
    let mut block=LateBlock::default();let mut effects=Vec::new();
    let no=|i:usize,e:pocold::chain::Reject|->serde_json::Value{
        let why=match e{pocold::chain::Reject::Block(w)=>w,o=>panic!("not a block verdict: {o:?}")};
        serde_json::json!({"ok":false,"at":i,"reason":why.strip_prefix("15 ").unwrap_or(&why)})
    };
    for (i,raw) in case["items"].as_array().unwrap().iter().enumerate(){
        let (j,nonces)=match late_parse(&unhex(raw.as_str().unwrap()),rules.s_max){Ok(x)=>x,Err(e)=>return no(i,e)};
        if let Err(e)=late_window(&rules,j){return no(i,e)}
        let b=&case["bases"][j.to_string()];
        let tid:[u8;32]=unhex(b["tid"].as_str().unwrap()).try_into().unwrap();
        let ranges:Vec<(Address,u128,u128)>=b["ranges"].as_array().unwrap().iter().map(|x|(name(x[0].as_str().unwrap()),x[1].as_u64().unwrap().into(),x[2].as_u64().unwrap().into())).collect();
        let header:Vec<u64>=b["header"].as_array().unwrap().iter().map(|x|x.as_u64().unwrap()).collect();
        let credited:BTreeSet<u64>=b["credited"].as_array().unwrap().iter().map(|x|x.as_u64().unwrap()).collect();
        let base=LateBase{j,tid,nonce:b["nonce"].as_u64().unwrap(),t_share:Fixed::from_be_slice(&unhex(b["tShare"].as_str().unwrap())).unwrap(),
            ranges:&ranges,owners:&ranges,header:&header,credited:&credited,total:b["total"].as_u64().unwrap()};
        match late_check(&rules,&base,&nonces,&mut block){
            Ok(owners)=>effects.push((j,owners)),
            Err(e)=>return no(i,e),
        }
    }
    // Owners back to the model's names.
    let names=["a","b","c"];
    let effects:Vec<serde_json::Value>=effects.into_iter().map(|(j,o)|serde_json::json!([j,o.iter().map(|a|*names.iter().find(|n|name(n)==*a).expect("an owner of the ranges")).collect::<Vec<_>>()])).collect();
    serde_json::json!({"ok":true,"effects":effects})
}

/// Model-vs-chain.rs differential on every validity vector.
#[test]
fn late_share_validity_reproduces_the_independent_model(){
    let v:serde_json::Value=serde_json::from_str(VECTORS).unwrap();
    let cases=v.as_object().unwrap();
    assert!(cases.len()>=28,"{} cases",cases.len());
    let mut reasons=BTreeSet::new();
    for (n,c) in cases{
        let got=verdict(c);
        assert_eq!(got,c["expect"],"{n}");
        if let Some(r)=c["expect"]["reason"].as_str(){reasons.insert(r.to_string());}
    }
    // Every rule of the design is exercised by a refusal (V3-DESIGN.md V3-02 negative vectors).
    let want=["evidence form","ls form","ls count","ls order","ls window","ls cap","ls nonce outside nMax","ls winning nonce","ls header share",
        "ls credited on the branch","ls repeated in the block","ls share target"];
    assert_eq!(reasons,want.iter().map(|x|x.to_string()).collect::<BTreeSet<_>>());
}

/// A13 under the v3 domain only (V3-02): on CP schema v2 the model adds the late-share cache (D templates of 4·S_max
/// nonces) and the kind-3 evidence offloaded entries keep, and bounds a window's evidence per block; on CP schema v1
/// (every spec-1 profile) none of these terms exists and the bound is the one pinned in tests/consensus.rs.
#[test]
fn a13_counts_late_share_memory_on_cp_schema_v2_only(){
    use pocol_lp4_execution::{devnet::{self,DevnetConfig},genesis_spec as gs,profile::ConsensusParams,fixed::U256 as F};
    let mut c=DevnetConfig::new(777910,1_700_000_000,&["a","b","c"]);c.hash_rate=1024;c.d_att=73;c.tau=40;c.c=8;
    let v1=pocold::chain::Spec::new(devnet::build(&c).unwrap().admit(&pocold::single_host_local()).unwrap().0).cp().clone();
    let mut values=v1.to_values();
    for f in &gs::CP_SCHEMA_V2[gs::CP_LEN..]{values.push(F::from_u64(match f.name{"R_react"=>4,"C_react"=>v1.U as u64,_=>f.lo}));}
    let v2=ConsensusParams::from_values(&values).unwrap();assert_eq!((v1.schema_version(),v2.schema_version()),(1,2));
    let limits=pocold::node::Limits::default();let l=pocold::single_host_local();
    let at=|cp:&ConsensusParams|pocold::resources::derive(cp,l.k_hist,l.side_max,&limits,2).unwrap();
    let (a,b)=(at(&v1),at(&v2));
    let item=|d:&pocold::resources::Bound,k:&str|d.items.iter().find(|x|x.0.starts_with(k)).map(|x|x.1);
    for k in ["queues/late-share cache","history/late-share evidence kept on offloaded entries"]{assert_eq!(item(&a,k),None,"{k} on CP schema v1");}
    // The cache: D = 4 canonical templates, LATE_PENDING_MAX = 2 of the production height and one between a stash and
    // its prune, of 4·256 nonces at memory::VEC, beside the type's own size.
    let per=std::mem::size_of::<pocold::node::LateTemplate>() as u128+3*4*256*8;
    assert_eq!(pocold::node::LATE_PENDING_MAX,2);
    assert_eq!(item(&b,"queues/late-share cache"),Some(3*(4+2+1)*per+64));
    assert_eq!(pocold::resources::late_cache_bytes(&v1),Some(0));assert_eq!(pocold::resources::late_kept_bytes(&v1),Some(0));
    // Per offloaded entry: L = min(E_max·min(S_max, kappa*m - 1), D·(kappa*m - 1)) = min(4·63, 4·63) = 252 late shares in at most E_max = 4
    // items: 4·(24 + 17) + 9·252 = 2,432 bytes.
    assert_eq!(pocol_lp4_execution::resources::late_shares_max(&v2),252);
    assert_eq!(pocold::resources::late_kept_bytes(&v2),Some(2_432));
    let kept=item(&b,"history/late-share evidence kept on offloaded entries").unwrap();assert_eq!(kept%2_432,0);assert!(kept>0);
    assert!(b.total>a.total,"the v3 terms add to the bound");
    // The spec-1 pin of tests/consensus.rs: unchanged by V3-02. (Merging main moved that pin from 3,396,912,610 to
    // 3,397,791,234 B: V3-PR-C3's receipt-encoding correction, +878,624 B on every profile; this test follows it. FN-1's
    // child-context cache term, +185,920 B on this profile, moved it to 3,397,977,154 B; FN-3a's member-record terms,
    // +3,836,884 B, to 3,401,814,038 B.)
    if cfg!(all(target_os="linux",target_arch="x86_64")){assert_eq!(a.total,3_401_814_038,"the spec-1 A13 model total");}
}

mod on_chain {
    use std::collections::HashMap;
    use std::path::{Path,PathBuf};
    use pocol_lp4_execution::{apply::{self,EvidenceEffect,ProgramSet},devnet::{self,DevnetConfig},txsign::Key};
    use pocold::{chain::{late_encode,Chain,Reject,Spec},hashes,node::{Config,Limits,Node},types::*};
    use revm::primitives::{Address,B256,U256};

    const PCOL:u128=1_000_000_000_000_000_000;
    struct Net{chain:Chain,keys:HashMap<Address,Key>,cfg:DevnetConfig,ids:Vec<Address>}
    /// FN-3a: every test ends with the cold sweep of its chain (tests/support/members.rs).
    impl Drop for Net{fn drop(&mut self){crate::members_sweep::sweep_on_drop(&self.chain);}}
    fn config(spec:u8)->DevnetConfig{
        let mut c=DevnetConfig::new(777952,1_700_000_000,&["one","two","three"]);
        c.hash_rate=16;c.c=8;c.d_att=73;c.r_max=1;c.earnin=true;c.spec_version=spec;c
    }
    fn spec_of(c:&DevnetConfig)->Spec{Spec::new(devnet::build(c).unwrap().admit(&pocold::single_host_local()).unwrap().0)}
    fn net(spec:u8)->Net{
        let cfg=config(spec);let d=devnet::build(&cfg).unwrap();
        let keys=d.miners.iter().map(|m|(m.id,Key::from_secret(&m.key.secret()).unwrap())).collect();
        let ids=d.miners.iter().map(|m|m.id).collect();
        let chain=Chain::new(spec_of(&cfg));
        assert_eq!(chain.spec.programs(),if spec==2{ProgramSet::V3}else{ProgramSet::EarnIn});
        assert_eq!(chain.late_shares(1),spec==2,"kind 3 only on the v3 program set");
        Net{chain,keys,cfg,ids}
    }
    /// A mined block: its header shares, the omitted owners' shares and the shares left out past `header_max`.
    struct Mined{block:Block,omitted:Vec<u64>,spare:Vec<u64>}
    /// A signed block on `parent` (Draw(h,0) signs it; the template from the `skip`-th usable timestamp) carrying
    /// `evidence` (its effects applied when `valid`; otherwise the header commits the body and the execution ignores it,
    /// so validation reaches item 15), won in `winner`'s range, its header shares every share found (smallest first)
    /// except those in `omit`'s ranges, at most `header_max`.
    fn mine(n:&Net,parent:[u8;32],evidence:Vec<Vec<u8>>,valid:bool,winner:Address,omit:&[Address],header_max:usize,skip:usize)->Mined{
        let p=n.chain.full_entry(&parent).unwrap();let ctx=n.chain.child_ctx(&p,0).unwrap();
        let body=Body{txs:vec![],evidence};
        let effects=if valid{n.chain.evidence_effects(&p,&body,p.height+1).unwrap()}else{vec![]};
        let s_max=usize::from(n.chain.spec.cp().S_max);let mut skipped=0;
        for ts in p.ts+u64::from(n.chain.spec.cp().T_blk)..=ctx.window.upper(){
            let (mut h,_,_)=n.chain.build_signed(&p,&n.keys[&ctx.proposer],ts,body.clone(),effects.clone()).unwrap();
            let tid=template_id(&h.ut_raw);
            let top=ctx.assignment.nonce_max().min(1<<16) as u64;
            let mut shares:Vec<(u64,Address)>=Vec::new();let mut win=None;
            for nonce in 0..top{
                let ph=pow_hash(&tid,nonce);let owner=addr(&ctx.assignment.owner(nonce).unwrap());
                if win.is_none()&&owner==winner&&le_target(&ph,&ctx.target){win=Some(nonce);continue}
                if le_target(&ph,&ctx.t_share){shares.push((nonce,owner));}
            }
            let Some(nonce)=win else{continue};
            if skipped<skip{skipped+=1;continue}
            let omitted:Vec<u64>=shares.iter().filter(|s|omit.contains(&s.1)).map(|s|s.0).collect();
            let kept:Vec<u64>=shares.iter().filter(|s|!omit.contains(&s.1)).map(|s|s.0).collect();
            let take=header_max.min(s_max).min(kept.len());
            h.nonce=nonce;h.shares=kept[..take].to_vec();reseal(&mut h);
            h.winner_sig=n.keys[&winner].sign_hash(&B256::from(hashes::win_msg(&tid,nonce,&share_root(&h))));reseal(&mut h);
            return Mined{block:Block{header:h,body},omitted,spare:kept[take..].to_vec()};
        }
        panic!("no solution in the winner's range at any usable timestamp")
    }
    /// The attempt-0 fallback block on `parent`, its header committing `evidence` (a valid fallback commits none).
    fn fallback(n:&Net,parent:[u8;32],evidence:Vec<Vec<u8>>)->Block{
        let p=n.chain.full_entry(&parent).unwrap();let (mut h,_,ctx)=n.chain.build_fallback(&p,0).unwrap();
        if !evidence.is_empty(){h.evidence_root=body_commitment(&evidence).unwrap();reseal(&mut h);}
        let tid=template_id(&h.ut_raw);
        for nonce in 0..ctx.assignment.nonce_max().min(1<<16) as u64{
            if le_target(&pow_hash(&tid,nonce),&ctx.target){
                h.nonce=nonce;reseal(&mut h);let owner=addr(&ctx.assignment.owner(nonce).unwrap());
                h.winner_sig=n.keys[&owner].sign_hash(&B256::from(hashes::win_msg(&tid,nonce,&share_root(&h))));reseal(&mut h);
                return Block{header:h,body:Body{txs:vec![],evidence}};
            }
        }
        panic!("no fallback solution")
    }
    /// Validates and inserts `b`, with the Escrow invariant on its state (T17), whether or not it becomes the head.
    fn accept(n:&mut Net,b:&Block)->[u8;32]{
        let (e,_)=n.chain.validate(b,None,0).unwrap_or_else(|e|panic!("h={}: {e:?}",b.header.h));let h=e.hash;let height=e.height;n.chain.insert(e);
        let st=n.chain.state_of(&h).unwrap();apply::check_escrow_invariant(&*st,n.chain.spec.profile(),height,false).unwrap();h
    }
    fn refused(n:&Net,b:&Block)->String{match n.chain.validate(b,None,0){Err(Reject::Block(w))=>w,r=>panic!("expected a block verdict: {:?}",r.map(|x|x.0.height))}}
    fn count(n:&Net,at:&[u8;32],j:u64,who:Address)->u64{apply::share_count(&*n.chain.state_of(at).unwrap(),j,who)}
    fn total(n:&Net,at:&[u8;32],j:u64)->u64{apply::share_total(&*n.chain.state_of(at).unwrap(),j)}
    fn reward(n:&Net,i:usize)->Address{devnet::build(&n.cfg).unwrap().miners[i].reward}
    fn balance(n:&Net,at:&[u8;32],a:Address)->U256{n.chain.state_of(at).unwrap().accounts.get(&a).map(|x|x.info.balance).unwrap_or_default()}

    /// Blocks 1..3, then block 4 won by `one` whose header omits every share of `two` (Mined.omitted).
    fn omitted_chain(n:&mut Net)->(Vec<Block>,Mined){
        let (one,two)=(n.ids[0],n.ids[1]);let mut blocks=Vec::new();
        for _ in 0..3{let m=mine(n,n.chain.head,vec![],true,one,&[],8,0);accept(n,&m.block);blocks.push(m.block);}
        let m=mine(n,n.chain.head,vec![],true,one,&[two],8,0);accept(n,&m.block);blocks.push(m.block.clone());
        assert!(m.omitted.len()>=4,"two found shares that the winner omitted: {}",m.omitted.len());
        (blocks,m)
    }

    /// V3-DESIGN.md V3-02, fails on the original: the winner of block 4 omits two's shares. On the v3 chain block 5
    /// carries LS(4) with two of them; recordLate credits two (owner from block 4's stored ranges) and settle(4) at h = 8
    /// pays two R*2/Q (R = 1.7 PCOL, Q = max(C, kappa*m) = 64) besides whatever pot 4 owed two as its proposer. On the
    /// spec-1 earn-in chain the same carrier is "15 evidence form" and the omission stands.
    #[test]
    fn the_omitted_share_is_credited_by_the_next_block_and_paid_at_settlement(){
        for spec in [1u8,2]{
            let mut n=net(spec);let (one,two)=(n.ids[0],n.ids[1]);
            let (_,m4)=omitted_chain(&mut n);let h4=n.chain.head;
            let ls=late_encode(4,&m4.omitted[..2]);
            if spec==1{
                let b=mine(&n,h4,vec![ls],false,one,&[],8,0);
                assert_eq!(refused(&n,&b.block),"15 evidence form","no kind 3 before v3");
                continue
            }
            let p=n.chain.full_entry(&h4).unwrap();
            assert_eq!(n.chain.evidence_effects(&p,&Body{txs:vec![],evidence:vec![ls.clone()]},5).unwrap(),vec![EvidenceEffect::Late{j:4,owners:vec![two,two]}]);
            let b5=mine(&n,h4,vec![ls],true,one,&[],8,0);let h5=accept(&mut n,&b5.block);
            let header=m4.block.header.shares.len() as u64+1;
            assert_eq!((count(&n,&h5,4,two),total(&n,&h5,4)),(2,header+2),"two's two late shares on top of the header's");
            assert_eq!(apply::miner_record(&*n.chain.state_of(&h5).unwrap(),two).unwrap().last_share,4);
            for _ in 6..8{let m=mine(&n,n.chain.head,vec![],true,one,&[],8,0);accept(&mut n,&m.block);}
            let before=balance(&n,&n.chain.head.clone(),reward(&n,1));
            let m8=mine(&n,n.chain.head,vec![],true,one,&[],8,0);let h8=accept(&mut n,&m8.block);
            let gain=balance(&n,&h8,reward(&n,1))-before;
            let pot4_proposer=Address::from_slice(&m4.block.header.proposer);
            let amount=U256::from(2*PCOL);let r=amount-amount/U256::from(10)-amount/U256::from(20);
            let q=U256::from(header.max(64));
            let want=r*U256::from(2)/q+if pot4_proposer==two{amount/U256::from(20)}else{U256::ZERO};
            assert_eq!(gain,want,"two is paid its late shares at settle(4)");
        }
    }

    /// The design's negative cases on real branches (V3-DESIGN.md V3-02 Tests), each refused with its rule.
    #[test]
    fn late_shares_refused_on_a_real_branch(){
        let mut n=net(2);let (one,two,three)=(n.ids[0],n.ids[1],n.ids[2]);
        let (_,m4)=omitted_chain(&mut n);let h4=n.chain.head;let b4=&m4.block.header;
        let reason=|n:&Net,parent:[u8;32],items:Vec<Vec<u8>>|refused(n,&mine(n,parent,items,false,one,&[],8,0).block);
        let o=&m4.omitted;
        // A fallback body may carry no evidence (item 12), kind 3 included.
        assert_eq!(refused(&n,&fallback(&n,h4,vec![late_encode(4,&o[..1])])),"12 fallback body");
        // Template j's winning nonce and header shares are credited already; a target miss; nMax.
        assert_eq!(reason(&n,h4,vec![late_encode(4,&[b4.nonce])]),"15 ls winning nonce");
        assert_eq!(reason(&n,h4,vec![late_encode(4,&b4.shares[..1])]),"15 ls header share");
        let p4=n.chain.full_entry(&h4).unwrap();let tid4=template_id(&b4.ut_raw);
        // Review E3 (blocking): judged at a height other than a child of the parent (a received template's claim), an
        // item is a verdict on it (the window), never a local fault (which stopped the node: j = h-1 is not held).
        for h in [p4.height+2,p4.height,p4.height+9]{
            match n.chain.evidence_effects(&p4,&pocold::types::Body{txs:vec![],evidence:vec![late_encode(p4.height+1,&o[..1])]},h){
                Err(Reject::Block(w))=>assert_eq!(w,"15 ls window","h = {h}"),r=>panic!("FAIL-ON-ORIGINAL: h = {h}: {r:?}")}
        }
        match n.chain.evidence_effects(&p4,&pocold::types::Body{txs:vec![],evidence:vec![late_encode(4,&o[..1])]},p4.height+1){Ok(e)=>assert_eq!(e.len(),1),r=>panic!("positive control: {r:?}")}
        let ctx4=n.chain.child_ctx(&n.chain.full_entry(&p4.parent.unwrap()).unwrap(),0).unwrap();
        let nmax=ctx4.assignment.nonce_max() as u64;
        let miss=(0..nmax).find(|x|!le_target(&pow_hash(&tid4,*x),&ctx4.t_share)).unwrap();
        assert_eq!(reason(&n,h4,vec![late_encode(4,&[miss])]),"15 ls share target");
        assert_eq!(reason(&n,h4,vec![late_encode(4,&[nmax])]),"15 ls nonce outside nMax");
        // A share mined on a sibling template of height 4 (another timestamp of the same parent; not an ancestor of
        // this branch) is checked against the branch's template j and misses it.
        let sib=mine(&n,p4.parent.unwrap(),vec![],true,one,&[],8,1);let tid_sib=template_id(&sib.block.header.ut_raw);assert_ne!(tid_sib,tid4);
        // (Another nonce than `miss`: the same item would be the same block, whose verdict the chain's cache already holds.)
        let foreign=(0..nmax).find(|x|*x!=miss&&le_target(&pow_hash(&tid_sib,*x),&ctx4.t_share)&&!le_target(&pow_hash(&tid4,*x),&ctx4.t_share)).unwrap();
        assert_eq!(reason(&n,h4,vec![late_encode(4,&[foreign])]),"15 ls share target");
        // Duplicates within the list (not strictly ascending), within the block (two items), and across carriers.
        assert_eq!(reason(&n,h4,vec![late_encode(4,&[o[0],o[0]])]),"15 ls order");
        assert_eq!(reason(&n,h4,vec![late_encode(4,&o[..2]),late_encode(4,&o[1..3])]),"15 ls repeated in the block");
        let b5=mine(&n,h4,vec![late_encode(4,&o[..2])],true,one,&[],8,0);let h5=accept(&mut n,&b5.block);
        assert_eq!(reason(&n,h5,vec![late_encode(4,&o[1..3])]),"15 ls credited on the branch");
        let b6=mine(&n,h5,vec![late_encode(4,&o[2..3])],true,one,&[],8,0);let h6=accept(&mut n,&b6.block);
        assert_eq!(count(&n,&h6,4,two),3);
        // The window: j = h is no ancestor, and j < h - D is settled (D = 4): the carrier at h = 8 is the last for j = 4.
        assert_eq!(reason(&n,h6,vec![late_encode(7,&o[..1])]),"15 ls window");
        let h7={let m_=mine(&n,h6,vec![],true,one,&[],8,0);accept(&mut n,&m_.block)};
        let h8={let m_=mine(&n,h7,vec![late_encode(4,&o[3..4])],true,one,&[],8,0);accept(&mut n,&m_.block)};
        assert_eq!(count(&n,&h8,4,two),4,"a carrier at h = j + D credits j before settle(j)");
        assert_eq!(reason(&n,h8,vec![late_encode(4,&o[4..5])]),"15 ls window");
        // The cap, read from the parent state: block 9's header holds 63 shares and its winner's (kappa*m = 64).
        let m9=mine(&n,h8,vec![],true,one,&[three],63,0);assert_eq!(m9.block.header.shares.len(),63);let h9=accept(&mut n,&m9.block);
        assert_eq!(reason(&n,h9,vec![late_encode(9,&m9.omitted[..1])]),"15 ls cap");
        // Block 10's header holds 60: four late shares refused, three accepted (filling C to kappa*m exactly).
        let m10=mine(&n,h9,vec![],true,one,&[three],60,0);let h10=accept(&mut n,&m10.block);
        let mut late=m10.omitted.clone();late.extend(&m10.spare);late.sort_unstable();
        assert_eq!(reason(&n,h10,vec![late_encode(10,&late[..4])]),"15 ls cap");
        let h11={let m_=mine(&n,h10,vec![late_encode(10,&late[..3])],true,one,&[],8,0);accept(&mut n,&m_.block)};
        assert_eq!(total(&n,&h11,10),64);
        let _=two;
    }

    /// A reorg across a carrier rolls the credit and its deduplication back: branch A carries LS(4) in block 5; branch B
    /// (a fallback 5' and 6') overtakes it, so its head's state holds no late credit, and B can carry the same nonce;
    /// A then overtakes B again with the credit, where the same nonce is refused as credited on the branch.
    #[test]
    fn a_reorg_across_a_carrier_rolls_the_credit_back(){
        let mut n=net(2);let (one,two)=(n.ids[0],n.ids[1]);
        let (_,m4)=omitted_chain(&mut n);let h4=n.chain.head;let o=m4.omitted.clone();
        let a5={let m_=mine(&n,h4,vec![late_encode(4,&o[..2])],true,one,&[],8,0);accept(&mut n,&m_.block)};
        assert_eq!((n.chain.head,count(&n,&a5,4,two)),(a5,2));
        let b5={let f_=fallback(&n,h4,vec![]);accept(&mut n,&f_)};
        let b6={let m_=mine(&n,b5,vec![],true,one,&[],8,0);accept(&mut n,&m_.block)};
        assert_eq!(n.chain.head,b6,"branch B is heavier");
        assert_eq!(count(&n,&b6,4,two),0,"the credit of A's carrier is not on B");
        let b7={let m_=mine(&n,b6,vec![late_encode(4,&o[..1])],true,one,&[],8,0);accept(&mut n,&m_.block)};
        assert_eq!((n.chain.head,count(&n,&b7,4,two)),(b7,1),"B credits the same nonce once");
        let a6={let m_=mine(&n,a5,vec![],true,one,&[],8,0);accept(&mut n,&m_.block)};
        let a7={let m_=mine(&n,a6,vec![],true,one,&[],8,0);accept(&mut n,&m_.block)};
        assert_eq!(refused(&n,&mine(&n,a7,vec![late_encode(4,&o[..1])],false,one,&[],8,0).block),"15 ls credited on the branch");
        let a8={let m_=mine(&n,a7,vec![late_encode(4,&o[2..3])],true,one,&[],8,0);accept(&mut n,&m_.block)};
        assert_eq!(n.chain.head,a8,"A overtakes B again");
        assert_eq!(count(&n,&a8,4,two),3,"A's credits, not B's");
        // Replay determinism: a second chain importing every stored block in another order reaches the same head and roots.
        let mut m=net(2);
        let mut all:Vec<[u8;32]>=n.chain.blocks.keys().copied().filter(|h|*h!=n.chain.spec.genesis_hash()).collect();
        all.sort_by_key(|h|(n.chain.blocks[h].height,std::cmp::Reverse(*h)));
        for h in &all{let e=&n.chain.blocks[h];let b=Block{header:e.header.clone(),body:n.chain.body_of(e,None).unwrap()};accept(&mut m,&b);}
        assert_eq!(m.chain.head,n.chain.head);
        for h in &all{assert_eq!(m.chain.blocks[h].header.state_root,n.chain.blocks[h].header.state_root);}
        assert_eq!(count(&m,&a8,4,two),3);assert_eq!(count(&m,&b7,4,two),1);
    }

    /// Node-local (V3-02): a proposer carries the uncredited shares its late-share cache saw for an ancestor template.
    /// The cache (node::LateCache) holds two's omitted shares of block 4 among a header share, the winning nonce, a
    /// target miss, nMax and a duplicate; the drawn proposer's template for h = 5 carries one kind-3 item with exactly
    /// the omitted shares (smallest first, within the cap), the block validates and credits them. The cache keeps at
    /// most D templates of at most 4·S_max shares, and takes a later share only below nMax and on target.
    #[test]
    fn a_proposer_carries_the_uncredited_shares_it_saw(){
        use pocold::node::{LateCache,LateTemplate,Selection};
        let mut n=net(2);let one=n.ids[0];
        let (blocks,m4)=omitted_chain(&mut n);let h4=n.chain.head;let b4=m4.block.header.clone();
        let p=n.chain.full_entry(&h4).unwrap();let proposer=n.chain.child_ctx(&p,0).unwrap().proposer;
        let ctx4=n.chain.child_ctx(&n.chain.full_entry(&p.parent.unwrap()).unwrap(),0).unwrap();let tid4=template_id(&b4.ut_raw);
        let nmax=ctx4.assignment.nonce_max();
        let miss=(0..nmax as u64).find(|x|!le_target(&pow_hash(&tid4,*x),&ctx4.t_share)).unwrap();
        let d=dir("proposer");let key=Key::from_secret(&n.keys[&proposer].secret()).unwrap();
        let mut node=Node::open(Config{miner:Some(key),..node_config(&d,Limits::default())},spec_of(&n.cfg)).unwrap();
        for b in &blocks{assert!(node.import(b.clone(),None).unwrap());}
        let mut shares=m4.omitted.clone();shares.extend([b4.shares[0],b4.nonce,miss,nmax as u64,m4.omitted[0]]);
        node.late_cache.stash(LateTemplate{tid:tid4,height:4,nonce_max:nmax,t_share:ctx4.t_share,shares},4*256);
        let (h,body)=node.build_template_unrecorded(&p,Selection::default(),p.ts+4).expect("a template");
        // The no-dilution cap leaves kappa*m - C_header(4) of them (two found more than that).
        let room=(64-(b4.shares.len()+1)).min(m4.omitted.len());assert!(room<m4.omitted.len(),"positive control: the cap binds");
        assert_eq!(body.evidence,vec![late_encode(4,&m4.omitted[..room])],"the smallest omitted shares, within the cap, in one item");
        assert_eq!(node.stats.late_items,1);
        // The template is valid: mined, it credits two's omitted shares.
        let pe=n.chain.full_entry(&h4).unwrap();let ctx=n.chain.child_ctx(&pe,0).unwrap();let tid=template_id(&h.ut_raw);
        let nonce=(0..ctx.assignment.nonce_max() as u64).find(|x|le_target(&pow_hash(&tid,*x),&ctx.target)).unwrap();
        let mut hh=h.clone();hh.nonce=nonce;hh.shares=vec![];reseal(&mut hh);let owner=addr(&ctx.assignment.owner(nonce).unwrap());
        hh.winner_sig=n.keys[&owner].sign_hash(&B256::from(hashes::win_msg(&tid,nonce,&share_root(&hh))));reseal(&mut hh);
        let h5=accept(&mut n,&Block{header:hh,body});
        assert_eq!((count(&n,&h5,4,n.ids[1]),total(&n,&h5,4)),(room as u64,64),"filled to kappa*m");
        crate::members_sweep::sweep(&node.chain);drop(node);let _=std::fs::remove_dir_all(&d);
        // The cache's own bounds (review E3: it keeps the canonical templates of the carrier window, never the last D
        // locked ones). Head at height 8, D = 4: the window of the next carrier is [5, 8]; heights 5..8 each have a
        // canonical template ([h;32]) and dead ones (an attempt, an equivocation's other template: [h+100;32]),
        // stashed after it; height 9 is being produced (three attempts); height 4 is out of the window.
        let mut c=LateCache::default();let lt=|tid:[u8;32],height:u64|LateTemplate{tid,height,nonce_max:100,t_share:ctx4.t_share,shares:(0..10).collect()};
        for h in 4..=8u8{c.stash(lt([h;32],u64::from(h)),8);c.stash(lt([h+100;32],u64::from(h)),8);c.stash(lt([h+150;32],u64::from(h)),8);}
        for a in 0..3u8{c.stash(lt([200+a;32],9),8);}
        c.prune(8,4,|j|(1..=8).contains(&j).then(||[j as u8;32]));
        assert_eq!(c.len(),4+pocold::node::LATE_PENDING_MAX,"the window's canonical templates and the newest of the production height");
        for h in 5..=8u8{assert!(c.get(&[h;32]).is_some(),"FAIL-ON-ORIGINAL: canonical {h} kept past the dead templates");assert!(c.get(&[h+100;32]).is_none()&&c.get(&[h+150;32]).is_none());}
        assert!(c.get(&[4;32]).is_none(),"below the window");
        assert!(c.get(&[200;32]).is_none()&&c.get(&[201;32]).is_some()&&c.get(&[202;32]).is_some(),"the newest pending ones");
        // A reorg to a shorter head drops what is above its production height; a re-locked template leaves the cache.
        c.prune(7,4,|j|(1..=7).contains(&j).then(||[j as u8;32]));assert!(c.get(&[8;32]).is_some()&&c.get(&[201;32]).is_none(),"8 is now pending, 9 gone");
        assert!(c.remove(&[8;32]).is_some()&&c.get(&[8;32]).is_none());
        assert_eq!(c.get(&[5;32]).unwrap().shares,(0..8).collect::<Vec<u64>>(),"at most the cap, smallest first");
        let t=[9u8;32];c.stash(LateTemplate{tid:t,height:7,nonce_max:100,t_share:ctx4.t_share,shares:vec![]},8);
        let hit=(0..100u64).find(|x|le_target(&pow_hash(&t,*x),&ctx4.t_share)).unwrap();let off=(0..100u64).find(|x|!le_target(&pow_hash(&t,*x),&ctx4.t_share)).unwrap();
        assert_eq!((c.take(&t,hit,8),c.take(&t,hit,8),c.take(&t,off,8),c.take(&t,100,8),c.take(&[77;32],hit,8)),(Some(true),Some(false),Some(false),Some(false),None));
        assert_eq!(c.get(&t).unwrap().shares,vec![hit],"only a share below nMax and on target");
        let _=one;
    }

    fn dir(label:&str)->PathBuf{
        let d=std::env::temp_dir().join(format!("pocol-v3-late-{label}-{}",std::process::id()));let _=std::fs::remove_dir_all(&d);d
    }
    fn node_config(d:&Path,limits:Limits)->Config{Config{name:"late".into(),dir:d.to_path_buf(),listen:None,peers:vec![],miner:None,stop_at:None,log:false,rpc:None,limits}}

    /// The deduplication state survives a restart: carriers are offloaded (history window 1) and slimmed,
    /// so the walk reads kind-3 items kept on offloaded entries and, for slimmed ones (slim depth 0), restored from the journal; after a
    /// reopen that replays the journal, and after one that resumes from a checkpoint (store mode), a duplicate of a
    /// credited nonce is still refused and a fresh one accepted.
    #[test]
    fn restart_and_checkpoint_restore_keep_the_deduplication(){
        for (store,slim) in [(false,true),(true,true),(false,false),(true,false)]{
            let mut n=net(2);let one=n.ids[0];let d=dir(&format!("restart-{store}-{slim}"));
            let limits=||Limits{r_keep:4,side_cap:2,state_store:store,checkpoint_every:if store{2}else{0},slim_depth:slim.then_some(0),history_window:Some(1),..Default::default()};
            let (blocks,m4)=omitted_chain(&mut n);let o=m4.omitted.clone();
            let b5=mine(&n,n.chain.head,vec![late_encode(4,&o[..2])],true,one,&[],8,0);accept(&mut n,&b5.block);
            let b6=mine(&n,n.chain.head,vec![],true,one,&[],8,0);accept(&mut n,&b6.block);
            {
                let mut node=Node::open(node_config(&d,limits()),spec_of(&n.cfg)).unwrap();
                for b in blocks.iter().chain([&b5.block,&b6.block]){assert!(node.import(b.clone(),None).unwrap());}
                let c5=node.chain.get(&b5.block.hash()).unwrap();
                assert!(c5.offloaded&&c5.slim==slim,"positive control: the carrier is offloaded, and slimmed or not");
                if !slim{assert_eq!(c5.body.evidence,b5.block.body.evidence,"an offloaded entry keeps its kind-3 items (only them)");assert!(c5.body.txs.is_empty());}
                // Review E3: slimmed before it is offloaded (slim depth 0 <= history window 1), it keeps no evidence either,
                // as a checkpoint-restored copy (offload, then slim) and the A13 late-share term have it.
                else{assert!(c5.body.evidence.is_empty(),"FAIL-ON-ORIGINAL: a slimmed and offloaded carrier drops its kind-3 items");}
                for e in node.chain.blocks.values(){if e.slim&&e.offloaded{assert!(e.body.evidence.is_empty()&&e.body.txs.is_empty(),"height {}",e.height);}}
                if store{node.checkpoint_now().unwrap();}
            }
            let mut node=Node::open(node_config(&d,limits()),spec_of(&n.cfg)).unwrap();
            assert_eq!(node.chain.head,n.chain.head);
            if store{assert!(node.stats.resumed_from.is_some(),"positive control: resumed from a checkpoint");}
            let dup=mine(&n,n.chain.head,vec![late_encode(4,&o[1..2])],false,one,&[],8,0);
            let unslims=node.stats.unslims;
            match node.import(dup.block,None){Err(Reject::Block(w))=>assert_eq!(w,"15 ls credited on the branch"),r=>panic!("{r:?}")}
            assert_eq!(node.stats.unslims>unslims,slim,"a slimmed carrier and template are restored for the walk");
            let fresh=mine(&n,n.chain.head,vec![late_encode(4,&o[2..3])],true,one,&[],8,0);
            assert!(node.import(fresh.block.clone(),None).unwrap());accept(&mut n,&fresh.block);
            assert_eq!(apply::share_count(node.chain.view_of(&node.chain.head.clone()).unwrap().v(),4,n.ids[1]),3);
            // FN-3a: after the restart (replay or checkpoint restore), the cold sweep.
            crate::members_sweep::sweep(&node.chain);
            drop(node);let _=std::fs::remove_dir_all(&d);
        }
    }

    /// A pruned state's rebuild (`Node::rebuild_state`, the node's own re-execution path) re-executes carriers whose
    /// kind-3 evidence reads slimmed ancestors (the template and an earlier carrier): the evidence walk restores them
    /// as it does for type 2 (a walk that still meets a slimmed entry answers NeedsRebuild, which the test's `rebuild`
    /// answers by restoring that entry and the slimmed canonical entries in the E_win heights below it; import instead
    /// restores only the named entry per try), and the rebuild reaches the same state as a node that never slims.
    #[test]
    fn a_rebuild_through_slimmed_entries_with_late_shares(){
        let mut n=net(2);let one=n.ids[0];
        let (mut blocks,m4)=omitted_chain(&mut n);let o=m4.omitted.clone();
        for items in [vec![late_encode(4,&o[..2])],vec![late_encode(4,&o[2..3])]]{let m=mine(&n,n.chain.head,items,true,one,&[],8,0);accept(&mut n,&m.block);blocks.push(m.block);}
        let at_s=n.chain.head;
        for _ in 0..40{let m=mine(&n,n.chain.head,vec![],true,one,&[],8,0);accept(&mut n,&m.block);blocks.push(m.block);}
        let mut roots=Vec::new();
        for (tag,depth) in [("rb-slim",3u64),("rb-full",u64::MAX)]{
            let d=dir(tag);let limits=||Limits{state_store:true,slim_depth:Some(depth),..Default::default()};
            {let mut node=Node::open(node_config(&d,limits()),spec_of(&n.cfg)).unwrap();for b in &blocks{assert!(node.import(b.clone(),None).unwrap());}}
            let mut node=Node::open(node_config(&d,limits()),spec_of(&n.cfg)).unwrap();
            assert!(node.chain.view_of(&at_s).is_none(),"the state at S is pruned before the rebuild");
            if depth==3{for h in [m4.block.hash(),blocks[4].hash()]{assert!(node.chain.get(&h).unwrap().slim,"positive control: the template and the first carrier are slimmed");}}
            let rounds=rebuild(&mut node,&at_s);assert!(rounds>=1);
            let st=node.chain.view_of(&at_s).unwrap();
            assert_eq!(apply::share_count(st.v(),4,n.ids[1]),3,"both carriers' credits");
            roots.push((rounds,pocold::chain::whole_state(st.v()).accounts.len()));
            drop(st);crate::members_sweep::sweep(&node.chain);drop(node);let _=std::fs::remove_dir_all(&d);
        }
        assert_eq!(roots[0].1,roots[1].1);
    }
    /// The state of `hash` rebuilt through `Node::rebuild_state`, restoring (`Node::unslim`) a slimmed entry that a walk
    /// meets and the slimmed canonical entries in the E_win heights below it, then trying again. Every round but the
    /// last restores a slimmed entry, so the bound only turns a fault into a failure. Returns the rounds used.
    fn rebuild(node:&mut Node,hash:&[u8;32])->u32{
        let rounds=1+node.chain.blocks.values().filter(|e|e.slim).count() as u32;
        let e_win=u64::from(node.chain.spec.cp().E_win);
        for round in 1..=rounds{
            match node.rebuild_state(hash){
                Ok(())=>return round,
                Err(Reject::NeedsRebuild(p)) if node.chain.get(&p).is_some_and(|e|e.slim)=>{
                    let ph=node.chain.get(&p).map_or(0,|e|e.height);
                    let mut restore=vec![p];
                    restore.extend((ph.saturating_sub(e_win)..ph).filter_map(|h|node.chain.canonical_at(h)).filter(|e|e.slim).map(|e|e.hash));
                    for x in restore{node.unslim(&x).unwrap();}
                }
                r=>panic!("rebuild: {r:?}"),
            }
        }
        panic!("rebuild: no state after {rounds} rounds")
    }
}
