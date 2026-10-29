use super::*;
use crate::devnet::{self,DevnetConfig};
use revm::primitives::B256;

fn devnet3()->devnet::Devnet{devnet::build(&DevnetConfig::new(777900,1_800_000_000,&["a","b","c"])).expect("devnet")}

fn run_chain(net:&devnet::Devnet,blocks:u64,txs_at:&dyn Fn(u64)->Vec<Vec<u8>>)->Vec<(BlockOutput,ParentResult)>{
    let mut state=net.genesis.state().clone();let mut out=Vec::new();let mut prev:Option<ParentResult>=None;
    let ids:Vec<Address>=net.miners.iter().map(|m|m.id).collect();
    let mut base_fee=U256::from(1_000_000_000u64);let mut parent_gas=0u64;
    for h in 1..=blocks{
        if h>1{base_fee=crate::fees::next_base_fee(base_fee,parent_gas,8_000_000).unwrap();}
        let txs=txs_at(h);
        let input=BlockInput{height:h,timestamp:1_800_000_000+2*h,base_fee,gas_limit:8_000_000,prevrandao:B256::ZERO,
            proposer:if h%2==0{Proposer::Signed(ids[(h as usize)%3])}else{Proposer::Fallback},
            parent_result:prev.clone(),evidence:vec![],evidence_raw:&[],transactions:&txs,ancestors:vec![(0,net.genesis_hash)],cum_log_bytes:0};
        let o=apply_block(&state,&net.genesis,&net.profile,&input).unwrap_or_else(|e|panic!("h={h}: {e:?}"));
        state=o.state.clone();parent_gas=o.gas_used;
        // Winner rotates; shares: one for each other miner, two for the next one.
        let w=ids[(h as usize)%3];let s=vec![(10,ids[(h as usize+1)%3]),(20,ids[(h as usize+1)%3]),(30,ids[(h as usize+2)%3])];
        let pr=ParentResult{block_hash:B256::from(U256::from(h)),winner:w,shares:s};
        prev=Some(pr.clone());out.push((o,pr));
    }
    out
}

#[test] fn genesis_builds_with_v1_programs_and_ledger_empty(){
    let net=devnet3();let g=net.genesis.state();
    assert_eq!(active_list(g).len(),3);assert_eq!(record_list(g).len(),3);
    let f=chain_facts(g);assert_eq!(f.held_total,U256::from(3u64)*U256::from(10u128.pow(20)));assert!(!f.closing);
    assert!(g.accounts[&ledger()].storage.values().all(|v|*v==U256::ZERO));
    assert_eq!(state_root(g).unwrap(),net.genesis.root());
}

#[test] fn twenty_blocks_settle_and_keep_invariants(){
    let net=devnet3();let chain=run_chain(&net,20,&|_|vec![]);
    let d=u64::from(net.profile.cp().D);let k=d.max(u64::from(net.profile.cp().W_w));
    let last=&chain.last().unwrap().0.state;
    // Ledger keeps exactly heights [h-K, h-1] after pruning (h=20,K=8 -> 12..19).
    for j in 1..=19{ let present=share_total(last,j)!=0; assert_eq!(present,j>=20-k,"ledger height {j}"); }
    for j in 12..=19{assert_eq!(share_total(last,j),4);}
    // Every settled pot paid winner/proposer/share owners: reward balances positive.
    for m in &net.miners{assert!(last.accounts[&m.reward].info.balance>U256::ZERO,"{}",m.label);}
    // lastShare updated to the latest recorded height for every miner (all appear each height).
    for m in &net.miners{assert_eq!(miner_record(last,m.id).unwrap().last_share,19);}
    // pending window [h-D+1,h].
    for j in 1..=20{let present=pending(last,j).1!=U256::ZERO;assert_eq!(present,j>20-d);}
}

#[test] fn settlement_matches_independent_formula(){
    // Block 1 is fallback; settle(1) runs at h=1+D. Reproduce SV-style arithmetic.
    let net=devnet3();let d=u64::from(net.profile.cp().D);
    let chain=run_chain(&net,d+1,&|_|vec![]);
    let before=&chain[(d-1) as usize].0.state;let after=&chain[d as usize].0.state;
    let pr=&chain[0].1; // ParentResult of block 1
    let (amount,_,_)=pending(before,1);assert_eq!(amount,U256::from(2_000_000_000_000_000_000u128));
    let wp=amount*U256::from(1000)/U256::from(10000);let pp=amount*U256::from(500)/U256::from(10000);
    let r=amount-wp-pp;let c=U256::from(4);let q=U256::from(64);
    let mut expect=std::collections::HashMap::new();
    let mut add=|a:Address,v:U256|{*expect.entry(a).or_insert(U256::ZERO)+=v;};
    add(pr.winner,wp);add(pr.winner,pp); // fallback: propPart to winner
    let mut counts=std::collections::HashMap::new();for a in pr.owner_sequence(){*counts.entry(a).or_insert(0u64)+=1;}
    let mut sum=U256::ZERO;for (a,n) in &counts{let p=r*U256::from(*n)/q;sum+=p;add(*a,p);}
    let a_total=r*c/q;let burn=(r-a_total)+(a_total-sum);
    // Delta of each reward balance at h=D+1 equals settle(1) payout plus nothing else.
    for m in &net.miners{
        let delta=after.accounts[&m.reward].info.balance-before.accounts[&m.reward].info.balance;
        assert_eq!(delta,*expect.get(&m.id).unwrap_or(&U256::ZERO),"{}",m.label);
    }
    let burned=chain_facts(after).burned_total-chain_facts(before).burned_total;assert_eq!(burned,burn);
}

#[test] fn user_transfer_tips_flow_into_pot(){
    use crate::txsign::Tx;
    let net=devnet3();let alice=devnet::account_key("alice");let bob=devnet::account_key("bob").address();
    let tx=Tx{chain_id:777900,nonce:0,max_priority:U256::from(2_000_000_000u64),max_fee:U256::from(10_000_000_000u64),gas:21000,to:Some(bob),value:U256::from(12345),data:Default::default()}.sign_1559(&alice);
    let chain=run_chain(&net,2,&|h|if h==2{vec![tx.clone()]}else{vec![]});
    let o=&chain[1].0;assert_eq!(o.gas_used,21000);assert_eq!(o.tips,U256::from(21000u64*2_000_000_000));
    let (amount,meta,_)=pending(&o.state,2);assert_eq!(amount,U256::from(2_000_000_000_000_000_000u128)+o.tips);
    assert_eq!(meta>>8u32,U256::from_be_slice(net.miners[2].id.as_slice()),"signed proposer recorded");
    assert_eq!(o.state.accounts[&bob].info.balance,U256::from(10u128.pow(24))+U256::from(12345));
}

#[test] fn wrong_parent_result_presence_and_reverted_system_call_reject(){
    let net=devnet3();let g=net.genesis.state();
    let bad=BlockInput{height:1,timestamp:1_800_000_002,base_fee:U256::from(1_000_000_000u64),gas_limit:8_000_000,prevrandao:B256::ZERO,proposer:Proposer::Fallback,
        parent_result:Some(ParentResult{block_hash:B256::ZERO,winner:net.miners[0].id,shares:vec![]}),evidence:vec![],evidence_raw:&[],transactions:&[],ancestors:vec![],cum_log_bytes:0};
    assert!(matches!(apply_block(g,&net.genesis,&net.profile,&bad),Err(ApplyError::Context(_))));
    let chain=run_chain(&net,1,&|_|vec![]);
    // h=2 with a winner of address zero: recordResult reverts -> whole block rejected.
    let input=BlockInput{height:2,timestamp:1_800_000_004,base_fee:U256::from(875_000_000u64),gas_limit:8_000_000,prevrandao:B256::ZERO,proposer:Proposer::Fallback,
        parent_result:Some(ParentResult{block_hash:B256::ZERO,winner:Address::ZERO,shares:vec![]}),evidence:vec![],evidence_raw:&[],transactions:&[],ancestors:vec![],cum_log_bytes:0};
    assert!(matches!(apply_block(&chain[0].0.state,&net.genesis,&net.profile,&input),Err(ApplyError::System{..})));
}

#[test] fn evidence_burns_bond_and_tombs_identity(){
    let net=devnet3();let chain=run_chain(&net,3,&|_|vec![]);let s=&chain[2].0.state;let accused=net.miners[1].id;
    let input=BlockInput{height:4,timestamp:1_800_000_008,base_fee:U256::from(1_000_000_000u64)/U256::from(2),gas_limit:8_000_000,prevrandao:B256::ZERO,proposer:Proposer::Fallback,
        parent_result:Some(chain[2].1.clone()),evidence:vec![EvidenceEffect::PunishAndInvalidate{accused,j:3}],evidence_raw:&[vec![0xc0]],transactions:&[],ancestors:vec![],cum_log_bytes:0};
    let base=crate::fees::next_base_fee(chain[2].0.state.accounts.get(&escrow()).map(|_|U256::from(1u64)).unwrap()*U256::ZERO+chain_base(&chain),0,8_000_000).unwrap();
    let input=BlockInput{base_fee:base,..input};
    let o=apply_block(s,&net.genesis,&net.profile,&input).unwrap();
    let r=miner_record(&o.state,accused).unwrap();assert!(r.tomb);assert_eq!(r.bond_state,2);assert_eq!(r.status,2);
    assert_eq!(active_list(&o.state).len(),2);
    assert_eq!(o.facts.held_total,U256::from(2u64)*U256::from(10u128.pow(20)));
    assert_eq!(pending(&o.state,3).1>>168u32,U256::from(1),"pot 3 invalidated");
    // Punishing a tomb again rejects the block.
    let again=BlockInput{height:5,timestamp:1_800_000_010,base_fee:crate::fees::next_base_fee(base,0,8_000_000).unwrap(),parent_result:Some(chain[2].1.clone()),evidence:vec![EvidenceEffect::Punish{accused}],..input};
    assert!(apply_block(&o.state,&net.genesis,&net.profile,&again).is_err());
}
fn chain_base(chain:&[(BlockOutput,ParentResult)])->U256{let mut b=U256::from(1_000_000_000u64);for (o,_) in chain.iter().take(chain.len()-1){b=crate::fees::next_base_fee(b,o.gas_used,8_000_000).unwrap();}b}

#[test] fn closing_cursor_pays_bonds_and_final_block_settles_everything(){
    let mut cfg=DevnetConfig::new(777901,1_800_000_000,&["a","b","c"]);cfg.h_close=10;cfg.h_end=16;cfg.ret=2;
    let net=devnet::build(&cfg).unwrap();
    let ids:Vec<Address>=net.miners.iter().map(|m|m.id).collect();
    let mut state=net.genesis.state().clone();let mut prev:Option<ParentResult>=None;let mut base=U256::from(1_000_000_000u64);
    let bond=U256::from(10u128.pow(20));
    let mut finals=vec![];
    for h in 1..=16u64{
        let input=BlockInput{height:h,timestamp:1_800_000_000+2*h,base_fee:base,gas_limit:8_000_000,prevrandao:B256::ZERO,proposer:Proposer::Fallback,
            parent_result:prev.clone(),evidence:vec![],evidence_raw:&[],transactions:&[],ancestors:vec![],cum_log_bytes:0};
        let o=apply_block(&state,&net.genesis,&net.profile,&input).unwrap_or_else(|e|panic!("h={h} {e:?}"));
        base=crate::fees::next_base_fee(base,o.gas_used,8_000_000).unwrap();
        if h>=10{assert!(o.facts.closing,"closing from H_CLOSE");assert_eq!(o.facts.close_h,10);}
        if h==10{assert!(!o.facts.cursor_done,"RET=2 of 3 records");assert_eq!(o.facts.held_total,bond);}
        if h==11{assert!(o.facts.cursor_done);assert_eq!(o.facts.held_total,U256::ZERO);}
        finals.push(o.is_final);
        state=o.state.clone();prev=Some(ParentResult{block_hash:B256::ZERO,winner:ids[(h%3) as usize],shares:vec![]});
    }
    assert_eq!(finals.iter().filter(|f|**f).count(),1);assert!(finals[15],"h=16 is final");
    // F: no pending at all, every pot settled, no subsidy issued at F.
    for j in 1..=16{assert_eq!(pending(&state,j),(U256::ZERO,U256::ZERO,Address::ZERO),"pending[{j}]");}
    for m in &net.miners{let r=miner_record(&state,m.id).unwrap();assert_eq!(r.bond_state,3,"PAID");}
    // Supply identity: escrow balance == burnedTotal exactly (all held/pending gone).
    let f=chain_facts(&state);assert_eq!(state.accounts[&escrow()].info.balance,f.burned_total);
    // Total value: issued subsidy over 15 blocks + bonds = reward balances + burned.
    let rewards:U256=net.miners.iter().map(|m|state.accounts[&m.reward].info.balance).fold(U256::ZERO,|a,b|a+b);
    assert_eq!(rewards+f.burned_total,U256::from(15u64)*U256::from(2_000_000_000_000_000_000u128)+U256::from(3u64)*bond);
    // No child of F.
    let child=BlockInput{height:17,timestamp:1_800_000_034,base_fee:base,gas_limit:8_000_000,prevrandao:B256::ZERO,proposer:Proposer::Fallback,
        parent_result:prev.clone(),evidence:vec![],evidence_raw:&[],transactions:&[],ancestors:vec![],cum_log_bytes:0};
    assert!(apply_block(&state,&net.genesis,&net.profile,&child).is_err());
}

// ---- ExecFixture SV1–SV7s (incentives.md), executed through the compiled Escrow ----
struct Sv{net:devnet::Devnet,state:InMemoryDB}
fn put(db:&mut InMemoryDB,a:Address,k:U256,v:U256){db.insert_account_storage(a,k,v).unwrap();}
fn kslot(a:U256,b:U256)->U256{U256::from_be_bytes(keccak256([a.to_be_bytes::<32>(),b.to_be_bytes::<32>()].concat()).0)}
impl Sv{
    fn new(alpha:u16,gamma:u16)->Self{let mut c=DevnetConfig::new(777902,1_800_000_000,&["w"]);c.alpha_bp=alpha;c.gamma_bp=gamma;
        let net=devnet::build(&c).unwrap();let state=net.genesis.state().clone();Self{net,state}}
    fn identity(&mut self,label:&str,tomb:bool,record:bool)->Address{
        let id=crate::txsign::Key::devnet(&format!("sv-id:{label}")).address();let reward=crate::txsign::Key::devnet(&format!("sv-rw:{label}")).address();
        self.state.insert_account_info(reward,revm::primitives::AccountInfo{nonce:1,..Default::default()});
        if record{let r=record_slot(id);let w0=U256::from(1)|(U256::from(if tomb{2}else{1})<<16)|(U256::from(tomb as u8)<<8);
            put(&mut self.state,registry(),r,w0);put(&mut self.state,registry(),r+U256::from(1),U256::from_be_slice(reward.as_slice()));}
        id
    }
    fn reward(&self,id:Address)->Address{let r=record_slot(id);Address::from_word(B256::from(word(&self.state,registry(),r+U256::from(1))))}
    fn ledger(&mut self,j:u64,counts:&[(Address,u64)]){
        let total:u64=counts.iter().map(|c|c.1).sum();put(&mut self.state,ledger(),kslot(U256::from(j),U256::ZERO),U256::from(total));
        let n=kslot(U256::from(j),U256::from(1));let l=kslot(U256::from(j),U256::from(2));put(&mut self.state,ledger(),l,U256::from(counts.len()));
        let e=U256::from_be_bytes(keccak256(l.to_be_bytes::<32>()).0);
        for (i,(a,c)) in counts.iter().enumerate(){put(&mut self.state,ledger(),U256::from_be_bytes(keccak256([a.into_word().as_slice(),n.to_be_bytes::<32>().as_slice()].concat()).0),U256::from(*c));put(&mut self.state,ledger(),e+U256::from(i),U256::from_be_slice(a.as_slice()));}
    }
    fn pot(&mut self,j:u64,amount:u64,meta:U256,winner:Address){
        let b=kslot(U256::from(j),U256::from(3));put(&mut self.state,escrow(),b,U256::from(amount));put(&mut self.state,escrow(),b+U256::from(1),meta);put(&mut self.state,escrow(),b+U256::from(2),U256::from_be_slice(winner.as_slice()));
        self.state.accounts.get_mut(&escrow()).unwrap().info.balance+=U256::from(amount);
    }
    fn settle(&mut self,j:u64)->U256{
        let d=u64::from(self.net.profile.cp().D);let before=chain_facts(&self.state).burned_total;
        let block=NativeBlockContext{height:U256::from(j+d),timestamp:U256::from(1_800_000_100u64),gas_limit:U256::from(8_000_000u64),base_fee:U256::ZERO,prevrandao:B256::ZERO};
        let cp=self.net.profile.cp();let mut ex=Exec{state:self.state.clone(),profile:&self.net.profile,block,origin:Address::ZERO,used:0,limits:CodeLimits{runtime:cp.B_code_max,logical:cp.CODE_CAP},meter:Meter::default(),code_total:None};
        let mut data=selector("settle(uint256)");enc_u256(&mut data,U256::from(j));ex.call("settle",escrow(),data).unwrap();
        let after=chain_facts(&ex.state).burned_total;self.state=ex.state;after-before
    }
    fn bal(&self,id:Address)->U256{self.state.accounts.get(&self.reward(id)).map(|a|a.info.balance).unwrap_or_default()}
}
#[test] fn sv1_to_sv7s_exec_fixture_values(){
    // SV1: R=64000 {w:1} -> A=1000, baseBurn=63000.
    let mut s=Sv::new(0,0);let w=s.identity("w",false,true);s.ledger(1,&[(w,1)]);s.pot(1,64000,U256::from(1),w);
    assert_eq!(s.settle(1),U256::from(63000));assert_eq!(s.bal(w),U256::from(1000));
    // SV2: R=101 {w:1, 64 ids x1} -> A=101, sum p=65, roundBurn=36.
    let mut s=Sv::new(0,0);let w=s.identity("w",false,true);let mut c=vec![(w,1)];for i in 0..64{c.push((s.identity(&format!("i{i}"),false,true),1));}
    s.ledger(1,&c);s.pot(1,101,U256::from(1),w);assert_eq!(s.settle(1),U256::from(36));
    // SV3: R=1000 {w:3,x:2} -> 46/31, roundBurn=1, baseBurn=922.
    let mut s=Sv::new(0,0);let w=s.identity("w",false,true);let x=s.identity("x",false,true);s.ledger(1,&[(w,3),(x,2)]);s.pot(1,1000,U256::from(1),w);
    assert_eq!(s.settle(1),U256::from(923));assert_eq!((s.bal(w),s.bal(x)),(U256::from(46),U256::from(31)));
    // SV4: R=6400 {w:1, t:1 tomb} -> paid 100, penaltyBurn 100, baseBurn 6200.
    let mut s=Sv::new(0,0);let w=s.identity("w",false,true);let t=s.identity("t",true,true);s.ledger(1,&[(w,1),(t,1)]);s.pot(1,6400,U256::from(1),w);
    assert_eq!(s.settle(1),U256::from(6300));assert_eq!(s.bal(w),U256::from(100));assert_eq!(s.bal(t),U256::ZERO);
    // SV5: as SV4 but t has no record -> payBurn 100.
    let mut s=Sv::new(0,0);let w=s.identity("w",false,true);let t=s.identity("t",false,false);s.ledger(1,&[(w,1),(t,1)]);s.pot(1,6400,U256::from(1),w);
    assert_eq!(s.settle(1),U256::from(6300));assert_eq!(s.bal(w),U256::from(100));
    // SV6: invalid pot -> invalidBurn 5000.
    let mut s=Sv::new(1000,500);let w=s.identity("w",false,true);s.ledger(1,&[(w,1)]);s.pot(1,5000,U256::from(1)|(U256::from(1)<<168),w);
    assert_eq!(s.settle(1),U256::from(5000));assert_eq!(s.bal(w),U256::ZERO);
    // SV7: fallback pot 80000, C=1 -> 8000+4000 to winner, A=1062, baseBurn=66938, paid 13062.
    let mut s=Sv::new(1000,500);let w=s.identity("w",false,true);s.ledger(1,&[(w,1)]);s.pot(1,80000,U256::from(1),w);
    assert_eq!(s.settle(1),U256::from(66938));assert_eq!(s.bal(w),U256::from(13062));
    // SV7s: signed pot 164000 (proposer = w), C=1 -> paid 26778, baseBurn 137222.
    let mut s=Sv::new(1000,500);let w=s.identity("w",false,true);s.ledger(1,&[(w,1)]);s.pot(1,164000,U256::from_be_slice(w.as_slice())<<8u32,w);
    assert_eq!(s.settle(1),U256::from(137222));assert_eq!(s.bal(w),U256::from(26778));
}

// ---------------- A03: runtime limits are exceptional halts of the transaction ----------------
mod a03{
    use super::*;
    use crate::{rlp,txsign::Tx,user_phase::{caps,Halt}};
    use revm::primitives::{AccountInfo,Bytecode,Bytes};
    /// LOG0 x calldata[0..32] times, each with calldata[32..64] zero bytes of data.
    const LOGN:&[u8]=&[0x5f,0x35,0x5b,0x80,0x15,0x61,0x00,0x15,0x57,0x60,0x20,0x35,0x5f,0xa0,0x60,0x01,0x90,0x03,0x60,0x02,0x56,0x5b,0x00];
    /// One LOG2 (topics 9, 7) with calldata[32..64] data bytes.
    const LOG2:&[u8]=&[0x60,0x07,0x60,0x09,0x60,0x20,0x35,0x5f,0xa2,0x00];
    /// SSTORE(0,1) then one LOG0 with calldata[32..64] data bytes.
    const STORE_LOG:&[u8]=&[0x60,0x01,0x5f,0x55,0x60,0x20,0x35,0x5f,0xa0,0x00];
    /// SSTORE(i+1, 1) for i in 0..calldata[0..32].
    const FILL:&[u8]=&[0x5f,0x35,0x5b,0x80,0x15,0x61,0x00,0x14,0x57,0x60,0x01,0x81,0x55,0x60,0x01,0x90,0x03,0x60,0x02,0x56,0x5b,0x00];
    /// CALL(gas, address i+0x1000, value 1) for i in 0..calldata[0..32] (new accounts).
    const SPRAY:&[u8]=&[0x5f,0x35,0x5b,0x80,0x15,0x61,0x00,0x1e,0x57,0x5f,0x5f,0x5f,0x5f,0x60,0x01,0x85,0x61,0x10,0x00,0x01,0x5a,0xf1,0x50,0x60,0x01,0x90,0x03,0x60,0x02,0x56,0x5b,0x00];
    fn addr(n:u8)->Address{Address::from([n;20])}
    fn word(v:u64)->[u8;32]{U256::from(v).to_be_bytes()}
    fn data(a:u64,b:u64)->Bytes{[word(a),word(b)].concat().into()}
    struct Fx{net:devnet::Devnet,parent:revm::InMemoryDB}
    fn fx(c:&DevnetConfig,code:&[(Address,&[u8])])->Fx{
        let net=devnet::build(c).unwrap();let mut parent=net.genesis.state().clone();
        for (a,c) in code{let b=Bytecode::new_raw(Bytes::copy_from_slice(c));parent.insert_account_info(*a,AccountInfo{code_hash:b.hash_slow(),code:Some(b),..Default::default()});}
        Fx{net,parent}
    }
    fn cfg()->DevnetConfig{DevnetConfig::new(777900,1_800_000_000,&["a","b","c"])}
    fn tx(nonce:u64,to:Address,d:Bytes,gas:u64,value:u64)->Vec<u8>{
        Tx{chain_id:777900,nonce,max_priority:U256::from(1_000_000_000u64),max_fee:U256::from(100_000_000_000u64),gas,to:Some(to),value:U256::from(value),data:d}.sign_1559(&devnet::account_key("alice"))
    }
    fn apply(f:&Fx,txs:&[Vec<u8>],cum_log_bytes:u64)->Result<BlockOutput,ApplyError>{
        let input=BlockInput{height:1,timestamp:1_800_000_004,base_fee:U256::from(1_000_000_000u64),gas_limit:f.net.profile.cp().GAS_LIMIT,prevrandao:B256::ZERO,
            proposer:Proposer::Fallback,parent_result:None,evidence:vec![],evidence_raw:&[],transactions:txs,ancestors:vec![(0,f.net.genesis_hash)],cum_log_bytes};
        apply_block(&f.parent,&f.net.genesis,&f.net.profile,&input)
    }
    /// (status, cumulative gas, number of logs) of each receipt.
    fn receipts(o:&BlockOutput)->Vec<(bool,u64,usize)>{o.receipts.iter().map(|r|{let it=rlp::decode(&r[1..]).unwrap();let l=it.list().unwrap();
        let g=l[1].bytes().unwrap().iter().fold(0u64,|n,b|n<<8|u64::from(*b));(l[0].bytes().unwrap()==[1],g,l[3].list().unwrap().len())}).collect()}
    fn alice()->Address{devnet::account_key("alice").address()}

    #[test] fn log_block_max_below_at_and_above(){
        let f=fx(&cfg(),&[(addr(0x41),LOGN)]);
        // One LOG0 with d bytes is charged 36+d: 32731 and 32732 fit, 32733 exceeds 32768.
        for (d,ok) in [(32731u64,true),(32732,true),(32733,false)]{
            let o=apply(&f,&[tx(0,addr(0x41),data(1,d),2_000_000,0)],0).unwrap();let r=receipts(&o);
            assert_eq!((r[0].0,r[0].2),(ok,usize::from(ok)),"data={d}");
            assert_eq!(o.log_bytes,if ok{36+d}else{0});
            if !ok{assert_eq!(o.halts,vec![(0,Halt::LogBytes)]);assert_eq!(r[0].1,2_000_000,"a halt charges all gas");assert_eq!(o.logs_bloom,[0u8;256]);}
        }
    }
    #[test] fn lg2_546_logs_succeed_547_halt_and_topics_are_charged(){
        let f=fx(&cfg(),&[(addr(0x41),LOGN),(addr(0x42),LOG2)]);
        let r=|n:u64,d:u64|{let o=apply(&f,&[tx(0,addr(0x41),data(n,d),7_000_000,0)],0).unwrap();(receipts(&o)[0],o.halts)};
        assert_eq!(r(546,24).0.2,546,"LG2: 546 LOG0 of 24 bytes (546*60 = 32760) succeed");
        assert_eq!(r(547,24).1,vec![(0,Halt::LogCount)],"LG2: the 547th halts");
        assert_eq!(r(546,0).0.2,546);assert_eq!(r(547,0).1,vec![(0,Halt::LogCount)],"547 empty logs fit the bytes but not the count");
        // LOG2: 36 + 2*32 + d bytes.
        let t=|d:u64|apply(&f,&[tx(0,addr(0x42),data(0,d),2_000_000,0)],0).unwrap();
        assert_eq!(t(32768-100).log_bytes,32768);assert_eq!(t(32768-99).halts,vec![(0,Halt::LogBytes)]);
    }
    #[test] fn cumulative_logs_across_transactions_and_a_halt_consumes_no_budget(){
        let f=fx(&cfg(),&[(addr(0x41),LOGN)]);
        let txs=[tx(0,addr(0x41),data(1,20000),1_000_000,0),tx(1,addr(0x41),data(1,12732),1_000_000,0),tx(2,addr(0x41),data(1,12696),1_000_000,0)];
        let o=apply(&f,&txs,0).unwrap();let r=receipts(&o);
        assert_eq!(r.iter().map(|x|(x.0,x.2)).collect::<Vec<_>>(),vec![(true,1),(false,0),(true,1)]);
        assert_eq!(o.halts,vec![(1,Halt::LogBytes)]);assert_eq!(o.log_bytes,20036+12732);assert_eq!(o.log_bytes,32768);
        assert_eq!(r[1].1-r[0].1,1_000_000,"the halted transaction is charged its whole gas limit");
        assert_eq!(o.gas_used,r[2].1);
        // Deterministic: re-application yields the same roots.
        let again=apply(&f,&txs,0).unwrap();assert_eq!((again.state_root,again.receipts_root,again.logs_bloom),(o.state_root,o.receipts_root,o.logs_bloom));
    }
    #[test] fn a_halt_reverts_state_except_nonce_and_fee(){
        let f=fx(&cfg(),&[(addr(0x43),STORE_LOG)]);
        let before=f.parent.accounts[&alice()].info.clone();let escrow0=f.parent.accounts[&crate::pay::escrow_address()].info.balance;
        let o=apply(&f,&[tx(0,addr(0x43),data(0,32733),400_000,5)],0).unwrap();
        assert_eq!(o.halts,vec![(0,Halt::LogBytes)]);
        let a=&o.state.accounts[&alice()].info;let price=U256::from(2_000_000_000u64);// base 1 gwei + tip 1 gwei
        assert_eq!(a.nonce,before.nonce+1);assert_eq!(a.balance,before.balance-price*U256::from(400_000u64),"all gas at the effective price; the value is not transferred");
        let c=&o.state.accounts[&addr(0x43)];assert_eq!(c.info.balance,U256::ZERO);assert!(c.storage.get(&U256::ZERO).copied().unwrap_or_default()==U256::ZERO,"SSTORE reverted");
        // The Escrow coinbase receives tip*gas_limit; closeBlock(tips) records the same tips.
        assert_eq!(o.tips,U256::from(1_000_000_000u64)*U256::from(400_000u64));
        assert!(o.state.accounts[&crate::pay::escrow_address()].info.balance>=escrow0+o.tips);
        // The same transaction within the limit succeeds and stores.
        let ok=apply(&f,&[tx(0,addr(0x43),data(0,100),400_000,5)],0).unwrap();assert!(ok.halts.is_empty());
        assert_eq!(ok.state.accounts[&addr(0x43)].storage[&U256::ZERO],U256::from(1));assert_eq!(ok.state.accounts[&addr(0x43)].info.balance,U256::from(5));
        assert_ne!(ok.state_root,o.state_root);
    }
    #[test] fn log_cap_is_aggregate_over_the_chain(){
        let f=fx(&cfg(),&[(addr(0x41),LOGN)]);
        let at=|d:u64|apply(&f,&[tx(0,addr(0x41),data(1,d),1_000_000,0)],caps::LOG_CAP-100).unwrap();
        assert!(at(64).halts.is_empty(),"36+64 = the remaining 100 bytes");assert_eq!(at(65).halts,vec![(0,Halt::LogCap)]);
    }
    #[test] fn dirty_keys_and_accounts_per_block(){
        let mut c=cfg();c.gas_limit=120_000_000;
        let f=fx(&c,&[(addr(0x44),FILL),(addr(0x45),SPRAY)]);
        let fill=|n:u64|apply(&f,&[tx(0,addr(0x44),data(n,0),110_000_000,0)],0).unwrap();
        assert!(fill(4096).halts.is_empty(),"4096 changed keys");assert_eq!(fill(4097).halts,vec![(0,Halt::DirtyKeys)]);
        // Dirty accounts: alice, the Escrow coinbase (tip) and n new accounts; the sprayer
        // receives n wei and pays out n wei, so its state does not change.
        let spray=|n:u64|apply(&f,&[tx(0,addr(0x45),Bytes::from(word(n).to_vec()),110_000_000,n)],0).unwrap();
        assert!(spray(2046).halts.is_empty(),"2+2046 = 2048 accounts");assert_eq!(spray(2047).halts,vec![(0,Halt::DirtyAccounts)]);
    }
    /// SLOAD(i) and CALL(address i+0x2000, value 0) for i in 0..calldata[0..32].
    const READ:&[u8]=&[0x5f,0x35,0x5b,0x80,0x15,0x61,0x00,0x20,0x57,0x80,0x54,0x50,0x5f,0x5f,0x5f,0x5f,0x5f,0x85,0x61,0x20,0x00,0x01,0x5a,0xf1,0x50,0x60,0x01,0x90,0x03,0x60,0x02,0x56,0x5b,0x00];
    #[test] fn a13_reads_and_empty_calls_leave_no_container_residue(){
        let mut c=cfg();c.gas_limit=30_000_000;let f=fx(&c,&[(addr(0x46),READ)]);
        let o=apply(&f,&[tx(0,addr(0x46),Bytes::from(word(1000).to_vec()),29_000_000,0)],0).unwrap();
        assert!(o.halts.is_empty());assert!(receipts(&o)[0].0,"the reads and calls succeed");
        assert!(o.state.accounts.values().all(|a|a.info().is_some()),"no NotExisting account entries remain");
        assert!(o.state.accounts.values().all(|a|a.storage.values().all(|v|*v!=U256::ZERO)),"no zero-valued slots remain");
        let r=&o.state.accounts[&addr(0x46)];assert!(r.storage.is_empty()&&r.storage.capacity()<=8,"read-only keys left no table: {}",r.storage.capacity());
        assert!(o.state.accounts.capacity()<=2*o.state.accounts.len()+16,"accounts table shrunk after 1000 empty calls");
        assert_eq!(o.state.accounts.len(),f.parent.accounts.len(),"the empty call targets do not persist");
    }
    /// Proposal vectors (development/audit-2026-10-22/proposals): the expected results of an
    /// independent model, replayed against the implementation for the recommended alternative.
    /// Agreement shows the code implements the recommendation; it is not an approval.
    mod proposals{
        use super::*;
        fn vectors(json:&str)->serde_json::Value{serde_json::from_str(json).unwrap()}
        fn tx_by(key:&crate::txsign::Key,nonce:u64,to:Address,d:Bytes,gas:u64,value:u64)->Vec<u8>{
            Tx{chain_id:777900,nonce,max_priority:U256::from(1_000_000_000u64),max_fee:U256::from(100_000_000_000u64),gas,to:Some(to),value:U256::from(value),data:d}.sign_1559(key)
        }
        fn halt_name(h:&Halt)->&'static str{match h{Halt::LogBytes=>"LogBytes",Halt::LogCount=>"LogCount",Halt::LogCap=>"LogCap",Halt::DirtyKeys=>"DirtyKeys",Halt::DirtyAccounts=>"DirtyAccounts",_=>"other"}}
        #[test] fn log_acct_01_vectors(){
            let v=vectors(include_str!("../../../../development/audit-2026-10-22/proposals/vectors/log-acct-01.json"));let rec=v["recommended"].as_str().unwrap();
            let f=fx(&cfg(),&[(addr(0x41),LOGN),(addr(0x42),LOG2)]);let mut n=0;
            for s in v["scenarios"].as_array().unwrap(){
                let txs:Vec<Vec<u8>>=s["txs"].as_array().unwrap().iter().enumerate().map(|(i,t)|{
                    let (count,log)=match t.get("repeat"){Some(r)=>(r.as_u64().unwrap(),&t["log"]),None=>(t.as_array().unwrap().len() as u64,&t[0])};
                    let (topics,d)=(log["topics"].as_u64().unwrap(),log["data"].as_u64().unwrap());
                    if topics==2{assert_eq!(count,1);tx(i as u64,addr(0x42),data(0,d),2_000_000,0)}else{assert_eq!(topics,0);tx(i as u64,addr(0x41),data(count,d),2_000_000,0)}
                }).collect();
                let e=&s["expected"][rec];let o=apply(&f,&txs,0).unwrap_or_else(|x|panic!("{}: {x:?}",s["id"]));let r=receipts(&o);
                for (i,t) in e["txs"].as_array().unwrap().iter().enumerate(){
                    let got=o.halts.iter().find(|(k,_)|*k==i).map(|(_,h)|halt_name(h));
                    assert_eq!(got,t["halt"].as_str(),"{} tx {i}",s["id"]);
                    assert_eq!(r[i].2 as u64,t["logs"].as_u64().unwrap_or(0),"{} tx {i} logs",s["id"]);
                }
                assert_eq!(o.log_bytes,e["blockLogBytes"].as_u64().unwrap(),"{}",s["id"]);n+=1;
            }
            assert_eq!(n,10);
        }
        /// SPRAY with its payees at 0x3001.. instead of 0x1001.. (fresh accounts).
        fn spray_b()->Vec<u8>{let mut c=SPRAY.to_vec();let i=c.windows(3).position(|w|w==[0x61,0x10,0x00]).unwrap();c[i+1]=0x30;c}
        #[test] fn d_a03_dirty_vectors(){
            let v=vectors(include_str!("../../../../development/audit-2026-10-22/proposals/vectors/d-a03-dirty.json"));let rec=v["recommended"].as_str().unwrap();
            let mut c=cfg();c.gas_limit=120_000_000;let b=spray_b();let f=fx(&c,&[(addr(0x45),SPRAY),(addr(0x47),&b)]);
            let (alice,bob)=(devnet::account_key("alice"),devnet::account_key("bob"));
            for s in v["scenarios"].as_array().unwrap(){
                let mut nonces=std::collections::HashMap::new();
                let txs:Vec<Vec<u8>>=s["txs"].as_array().unwrap().iter().map(|t|{
                    let key=if t["sender"]=="bob"{&bob}else{&alice};let k=nonces.entry(t["sender"].as_str().unwrap().to_string()).or_insert(0u64);let nonce=*k;*k+=1;
                    let to=if t["contract"]=="sprayerB"{addr(0x47)}else{addr(0x45)};let n=t["newAccounts"].as_u64().unwrap();
                    // About 36,600 gas per new payee (CALL with value to a new account).
                    tx_by(key,nonce,to,Bytes::from(word(n).to_vec()),if n>1{110_000_000}else{5_000_000},n)
                }).collect();
                let e=&s["expected"][rec];let o=apply(&f,&txs,0);
                if e["blockValid"]==false{
                    // The halt's own nonce/fee residue does not fit: the block is invalid (builders skip it).
                    let err=o.err().unwrap_or_else(||panic!("{}: expected an invalid block",s["id"]));
                    assert!(format!("{err:?}").contains(&format!("DirtyAccounts({})",e["invalidAt"])),"{}: {err:?}",s["id"]);continue;
                }
                let o=o.unwrap_or_else(|x|panic!("{}: {x:?}",s["id"]));let r=receipts(&o);
                for (i,t) in e["txs"].as_array().unwrap().iter().enumerate(){
                    assert_eq!(u64::from(r[i].0),t["status"].as_u64().unwrap(),"{} tx {i}",s["id"]);
                    // The precise outcome: which halt (none for a successful transaction).
                    let halt=o.halts.iter().find(|(j,_)|*j==i).map(|(_,h)|halt_name(h));
                    assert_eq!(halt,t["halt"].as_str(),"{} tx {i}: halt",s["id"]);
                }
                assert_eq!(o.dirty_accounts,e["dirtyAccounts"].as_u64().unwrap(),"{}: dirty accounts after the block",s["id"]);
            }
        }
        /// C1–C3 with SLOT_CAP = the genesis user slots + the model's headroom; C4 (system storage past SYS_SLOT_MAX)
        /// is `system::issuance_tests::actual_phase2_post_storage_cap_rejects_entire_candidate`.
        #[test] fn d_a03_caps_vectors(){
            let v=vectors(include_str!("../../../../development/audit-2026-10-22/proposals/vectors/d-a03-caps.json"));let rec=v["recommended"].as_str().unwrap();
            let p=&v["profile"];let headroom=p["slot_cap"].as_u64().unwrap()-p["user_slots"].as_u64().unwrap();
            let net=devnet::build(&cfg()).unwrap();let (slots,_)=crate::user_phase::live_counts(net.genesis.state());
            let mut c=cfg();c.slot_cap=slots+headroom;let f=fx(&c,&[(addr(0x44),FILL)]);
            for s in v["scenarios"].as_array().unwrap().iter().filter(|s|s["input"]["sys_writes"].as_u64().unwrap()<1000){
                let e=&s["expected"][rec];let o=apply(&f,&[tx(0,addr(0x44),data(s["input"]["user_writes"].as_u64().unwrap(),0),2_000_000,0)],0);
                assert_eq!(o.is_ok(),e["blockValid"]==true,"{}",s["id"]);let o=o.unwrap();
                let got=if o.halts.iter().any(|(_,h)|*h==Halt::SlotCap){"halt SlotCap"}else{"ok"};assert_eq!(got,e["userTx"],"{}",s["id"]);
            }
        }
        /// The vectors' cumulative counts are absolute for the release LOG_CAP (2^30); the caps-test build
        /// (LOG_CAP = 2^24) is checked by pocold tests/log_cap.rs instead.
        #[cfg(not(feature="caps-test"))]
        #[test] fn log_cap_aggregate_vectors(){
            let v=vectors(include_str!("../../../../development/audit-2026-10-22/proposals/vectors/log-cap-aggregate.json"));let rec=v["recommended"].as_str().unwrap();
            let f=fx(&cfg(),&[(addr(0x41),LOGN)]);
            for s in v["scenarios"].as_array().unwrap(){
                let charge=s["charge"].as_u64().unwrap();let o=apply(&f,&[tx(0,addr(0x41),data(1,charge-36),1_000_000,0)],s["cumulative"].as_u64().unwrap()).unwrap();
                assert_eq!(o.halts.first().map(|(_,h)|halt_name(h)),s["expected"][rec]["halt"].as_str(),"{}",s["id"]);
            }
        }
    }
    #[test] fn slot_and_account_caps_halt_the_transaction(){
        let net=devnet::build(&cfg()).unwrap();let (slots,accts)=crate::user_phase::live_counts(net.genesis.state());
        let mut c=cfg();c.slot_cap=slots+10;c.acct_cap=accts+2+5;// +2: the injected contracts
        let f=fx(&c,&[(addr(0x44),FILL),(addr(0x45),SPRAY)]);
        // Filling the user state exactly to SLOT_CAP leaves the block valid although the
        // system phases write new SYS slots in the same block (D-A03-CAPS).
        let fill=|n:u64|apply(&f,&[tx(0,addr(0x44),data(n,0),2_000_000,0)],0).unwrap();
        assert_eq!(fill(10).halts,vec![]);assert_eq!(fill(11).halts,vec![(0,Halt::SlotCap)]);
        let spray=|n:u64|apply(&f,&[tx(0,addr(0x45),Bytes::from(word(n).to_vec()),2_000_000,n)],0).unwrap();
        assert!(spray(5).halts.is_empty());assert_eq!(spray(6).halts,vec![(0,Halt::AcctCap)]);
    }
}

/// C-1 E1/E2: a chain applied with the parent's trie and summary gives, block by block, the outputs of
/// `apply_block` without them (the whole-state computations): roots, receipts, gas, the state and the
/// contracts map, with transfers, contract creations and empty blocks. In this crate's tests
/// `root_check()` is on, so every moved summary and outcome is also compared inside `apply_block_with`.
#[test] fn e2_blocks_with_trie_and_summary_equal_the_reference(){
    let net=devnet3();let ids:Vec<Address>=net.miners.iter().map(|m|m.id).collect();
    let alice=devnet::account_key("alice");let bob=devnet::account_key("bob").address();
    let tx=|nonce:u64,to:Option<Address>,data:Vec<u8>,gas:u64|crate::txsign::Tx{chain_id:777900,nonce,max_priority:U256::from(1_000_000_000u64),max_fee:U256::from(100_000_000_000u64),gas,to,value:U256::from(if to.is_some(){7u64}else{0}),data:data.into()}.sign_1559(&alice);
    // Init code returning a 3-byte runtime (PUSH3 0x600055, then MSTORE/RETURN).
    let init=vec![0x62,0x60,0x00,0x55,0x60,0,0x52,0x60,3,0x60,29,0xf3];
    let (mut state,mut trie,mut sum)=(net.genesis.state().clone(),None::<crate::state_trie::StateTrie>,None::<crate::state_summary::Summary>);
    let (mut prev,mut base_fee,mut parent_gas,mut nonce)=(None::<ParentResult>,U256::from(1_000_000_000u64),0u64,0u64);
    for h in 1..=30u64{
        if h>1{base_fee=crate::fees::next_base_fee(base_fee,parent_gas,8_000_000).unwrap();}
        let txs:Vec<Vec<u8>>=match h%3{0=>vec![],1=>{nonce+=2;vec![tx(nonce-2,Some(bob),vec![],21000),tx(nonce-1,None,init.clone(),200_000)]},_=>{nonce+=1;vec![tx(nonce-1,Some(bob),vec![],21000)]}};
        let input=BlockInput{height:h,timestamp:1_800_000_000+2*h,base_fee,gas_limit:8_000_000,prevrandao:B256::ZERO,
            proposer:if h%2==0{Proposer::Signed(ids[(h as usize)%3])}else{Proposer::Fallback},
            parent_result:prev.clone(),evidence:vec![],evidence_raw:&[],transactions:&txs,ancestors:vec![(0,net.genesis_hash)],cum_log_bytes:0};
        let r=apply_block(&state,&net.genesis,&net.profile,&input).unwrap_or_else(|e|panic!("h={h}: {e:?}"));
        let o=apply_block_with(&state,trie.as_ref(),sum.as_ref(),&net.genesis,&net.profile,&input).unwrap_or_else(|e|panic!("h={h}: {e:?}"));
        assert_eq!((o.state_root,o.receipts_root,o.gas_used,o.receipts.clone()),(r.state_root,r.receipts_root,r.gas_used,r.receipts.clone()),"h={h}");
        assert_eq!(o.summary,r.summary,"h={h}");
        let mut ka:Vec<_>=o.state.contracts.keys().copied().collect();let mut kb:Vec<_>=r.state.contracts.keys().copied().collect();ka.sort();kb.sort();assert_eq!(ka,kb,"h={h}");
        assert_eq!(o.state.accounts.len(),r.state.accounts.len());
        for (a,x) in &r.state.accounts{let y=&o.state.accounts[a];assert_eq!((&y.info.nonce,&y.info.balance,&y.info.code_hash,&y.storage,&y.account_state),(&x.info.nonce,&x.info.balance,&x.info.code_hash,&x.storage,&x.account_state),"h={h}");}
        parent_gas=o.gas_used;state=o.state;trie=Some(o.trie);sum=Some(o.summary);
        let w=ids[(h as usize)%3];prev=Some(ParentResult{block_hash:B256::from(U256::from(h)),winner:w,shares:vec![(10,ids[(h as usize+1)%3])]});
    }
    let g=crate::state_summary::Summary::scan(net.genesis.state(),net.profile.cp().B_code_max);let s=sum.unwrap();
    assert!(state.contracts.len()>net.genesis.state().contracts.len()&&s.code_total>=g.code_total+30,"positive control: ten runtimes were created ({s:?})");
}

/// C-1 E4.1: a node-trie version kept in a store from each block's write set (`BlockOutput.writes`), as
/// pocold will keep the head, has the block's stateRoot, and every account, code and slot of the block's
/// state reads the same through `node_trie::TrieState`. Two versions stay pinned, older ones are released,
/// and the store's reference counts equal a recount after every block.
#[test] fn e4_a_node_trie_kept_from_the_write_sets_reads_as_every_block_state(){
    use crate::node_trie::{Batch,MemStore,NodeTrie,TrieState};use crate::view::View;
    // C-1 E4.3: and every block run on its parent read from the store gives the same outputs.
    let net=devnet3();let ids:Vec<Address>=net.miners.iter().map(|m|m.id).collect();
    let alice=devnet::account_key("alice");let bob=devnet::account_key("bob").address();
    let tx=|nonce:u64,to:Option<Address>,data:Vec<u8>,gas:u64|crate::txsign::Tx{chain_id:777900,nonce,max_priority:U256::from(1_000_000_000u64),max_fee:U256::from(100_000_000_000u64),gas,to,value:U256::from(if to.is_some(){7u64}else{0}),data:data.into()}.sign_1559(&alice);
    // Init code storing slot 1 and returning a 5-byte runtime (PUSH1 1 PUSH1 0 SSTORE) that stores slot 0
    // when called.
    let init=vec![0x60,0x2a,0x60,0x01,0x55,0x64,0x60,0x01,0x60,0x00,0x55,0x60,0,0x52,0x60,5,0x60,27,0xf3];
    let (mut state,mut trie,mut sum)=(net.genesis.state().clone(),None::<crate::state_trie::StateTrie>,None::<crate::state_summary::Summary>);
    let mut store=MemStore::default();let mut b=Batch::default();let mut nt=NodeTrie::build(&state,&store,&mut b);store.commit(&b,nt.world);
    let mut pinned=std::collections::VecDeque::from([nt.world]);
    let (mut prev,mut base_fee,mut parent_gas,mut nonce)=(None::<ParentResult>,U256::from(1_000_000_000u64),0u64,0u64);
    let (mut with_writes,mut on_store)=(0,0);
    for h in 1..=30u64{
        if h>1{base_fee=crate::fees::next_base_fee(base_fee,parent_gas,8_000_000).unwrap();}
        let created=|n:u64|alice.address().create(n);
        let txs:Vec<Vec<u8>>=match h%3{0=>{nonce+=1;vec![tx(nonce-1,Some(created(1)),vec![],60_000)]},1=>{nonce+=2;vec![tx(nonce-2,Some(bob),vec![],21000),tx(nonce-1,None,init.clone(),200_000)]},_=>{nonce+=1;vec![tx(nonce-1,Some(bob),vec![],21000)]}};
        let input=BlockInput{height:h,timestamp:1_800_000_000+2*h,base_fee,gas_limit:8_000_000,prevrandao:B256::ZERO,
            proposer:if h%2==0{Proposer::Signed(ids[(h as usize)%3])}else{Proposer::Fallback},
            parent_result:prev.clone(),evidence:vec![],evidence_raw:&[],transactions:&txs,ancestors:vec![(0,net.genesis_hash)],cum_log_bytes:0};
        let o=apply_block_with(&state,trie.as_ref(),sum.as_ref(),&net.genesis,&net.profile,&input).unwrap_or_else(|e|panic!("h={h}: {e:?}"));
        // The same block run on the parent read from the store (`apply_block_on` over a `TrieState`), its
        // root from the node trie updated through the child overlay: the same outputs.
        if let Some(ps)=&sum{
            let parent_ts=TrieState{store:&store,trie:nt,block_hashes:&state.block_hashes};
            let run=apply_block_on(&parent_ts,ps,&net.genesis,&net.profile,&input).unwrap_or_else(|e|panic!("h={h} on the store: {e:?}"));
            let child=crate::view::Overlay::from_layer(&parent_ts,run.layer.clone());
            let mut b=Batch::default();let via=nt.update(&store,&mut b,&child,&run.writes);
            assert_eq!(via.root(&store,&b),o.state_root,"h={h}: the root from the store-backed run");
            assert_eq!((run.receipts_root,run.transactions_root,run.gas_used,&run.receipts,run.tips,run.issued,run.is_final,run.facts),(o.receipts_root,o.transactions_root,o.gas_used,&o.receipts,o.tips,o.issued,o.is_final,o.facts),"h={h}");
            assert_eq!(crate::state_summary::Summary{contracts_clean:true,..run.summary},crate::state_summary::Summary{contracts_clean:true,..o.summary},"h={h}: summary");
            on_store+=1;
        }
        let touched=match &o.writes{Some(w)=>{with_writes+=1;w.clone()},None=>crate::state_trie::Touched::between(&state,&o.state)};
        let mut b=Batch::default();let next=nt.update(&store,&mut b,&o.state,&touched);
        assert_eq!(next.root(&store,&b),o.state_root,"h={h}: the node trie's root is the block's stateRoot");
        store.commit(&b,next.world);pinned.push_back(next.world);
        if pinned.len()>2{if let Some(old)=pinned.pop_front().unwrap(){store.unpin(&old)}}
        store.recount().unwrap_or_else(|e|panic!("h={h}: {e}"));
        let ts=TrieState{store:&store,trie:next,block_hashes:&o.state.block_hashes};
        let mut n_slots=0;
        for (a,x) in &o.state.accounts{
            let exists=!matches!(x.account_state,revm::db::AccountState::NotExisting);
            let got=ts.account(a);assert_eq!(got.is_some(),exists,"h={h}: {a} exists");
            if let Some(g)=got{
                assert_eq!((g.info.nonce,g.info.balance,g.info.code_hash),(x.info.nonce,x.info.balance,x.info.code_hash),"h={h}: {a}");
                let want=x.info.code.clone().or_else(||o.state.contracts.get(&x.info.code_hash).cloned()).map(|c|c.original_bytes());
                assert_eq!(g.info.code.map(|c|c.original_bytes()),want,"h={h}: code of {a}");
                for (k,v) in &x.storage{assert_eq!(ts.slot(a,k),*v,"h={h}: {a} slot {k}");if *v!=U256::ZERO{n_slots+=1}}
                let mut got_slots=Vec::new();ts.each_slot(a,&mut |k,v|got_slots.push((*k,*v)));got_slots.sort();
                let mut want_slots:Vec<(U256,U256)>=x.storage.iter().filter(|(_,v)|**v!=U256::ZERO).map(|(k,v)|(*k,*v)).collect();want_slots.sort();
                assert_eq!(got_slots,want_slots,"h={h}: slots of {a}");
            }
        }
        assert!(n_slots>0,"positive control: the state holds slots");
        parent_gas=o.gas_used;state=o.state;trie=Some(o.trie);sum=Some(o.summary);nt=next;
        let w=ids[(h as usize)%3];prev=Some(ParentResult{block_hash:B256::from(U256::from(h)),winner:w,shares:vec![(10,ids[(h as usize+1)%3])]});
    }
    assert!(with_writes>=29,"positive control: the blocks ran on overlays and gave their write sets ({with_writes})");
    assert!(on_store>=29,"positive control: the blocks also ran on the store ({on_store})");
    let created=alice.address().create(1);assert_ne!(state.slot(&created,&U256::from(0)),U256::ZERO,"positive control: the created contract's runtime wrote a slot");
}

/// C-1 E2: a candidate that fails one check fails it with the summary as without it (the same error).
#[test] fn e2_failing_candidates_fail_alike(){
    use crate::state_summary::Summary;use revm::primitives::{AccountInfo,Bytecode,Bytes};
    let net=devnet3();let cp=net.profile.cp().clone();let img=&net.genesis.image;let g=net.genesis.state().clone();
    let same=|db:&InMemoryDB|{let s=Summary::scan(db,cp.B_code_max);let (a,b)=(img.verify_summary(db,&s,&net.profile),img.verify_candidate(db,&net.profile));assert_eq!(a,b);a};
    assert!(same(&g).is_ok());
    let mut d=g.clone();d.insert_account_info(Address::from([0x77;20]),AccountInfo::from_balance(U256::MAX>>1));assert!(same(&d).is_err());
    let mut d=g.clone();let c=Bytecode::new_raw(Bytes::from(vec![0u8;cp.B_code_max as usize+1]));d.insert_account_info(Address::from([0x78;20]),AccountInfo{code_hash:c.hash_slow(),code:Some(c),..Default::default()});assert!(same(&d).is_err());
    let mut d=g.clone();d.insert_account_info(Address::from([0x79;20]),AccountInfo{code_hash:B256::from([5;32]),code:None,..Default::default()});assert!(same(&d).is_err());
    let mut d=g.clone();let a=Address::from([0x7a;20]);d.insert_account_info(a,AccountInfo::from_balance(U256::from(1)));
    for k in 0..=cp.SLOT_CAP{d.insert_account_storage(a,U256::from(k),U256::from(1)).unwrap();}assert!(same(&d).is_err());
    let mut d=g.clone();let sys=program_address(4);for k in 0..=u64::from(cp.SYS_SLOT_MAX){d.insert_account_storage(sys,U256::from(k)+U256::from(1u64<<40),U256::from(1)).unwrap();}assert!(same(&d).is_err());
    let mut d=g.clone();for i in 0..=cp.ACCT_CAP{d.insert_account_info(Address::from_word(B256::from(U256::from(i+1000))),AccountInfo::from_balance(U256::from(1)));}assert!(same(&d).is_err());
    // And at each cap exactly, both pass: the counts are the checks' (system programs and their slots are
    // not user accounts or slots).
    let (slots,accts)=crate::user_phase::live_counts(&g);
    let mut d=g.clone();for i in 0..cp.ACCT_CAP-accts{d.insert_account_info(Address::from_word(B256::from(U256::from(i+1000))),AccountInfo::from_balance(U256::from(1)));}
    let a=Address::from_word(B256::from(U256::from(1000)));let k0=crate::user_phase::live_counts(&d).0;for k in 0..cp.SLOT_CAP-k0{d.insert_account_storage(a,U256::from(k),U256::from(1)).unwrap();}
    assert_eq!(crate::user_phase::live_counts(&d),(cp.SLOT_CAP,cp.ACCT_CAP),"positive control: both caps reached exactly");let _=slots;
    assert!(same(&d).is_ok());
    let sys_now=Summary::scan(&g,cp.B_code_max).sys_slots;let sys=program_address(4);let mut d=g.clone();
    for k in 0..u64::from(cp.SYS_SLOT_MAX)-sys_now{d.insert_account_storage(sys,U256::from(k)+U256::from(1u64<<40),U256::from(1)).unwrap();}
    assert_eq!(Summary::scan(&d,cp.B_code_max).sys_slots,u64::from(cp.SYS_SLOT_MAX));assert!(same(&d).is_ok());
}

/// C-1 E4.3: a child that fails a whole-state check fails it on the overlay path (`apply_block_on`, the
/// parent read through a view) as without the summary: a parent at exactly SYS_SLOT_MAX whose next block's
/// SYSTEM calls add a system slot, and a parent whose escrow is one wei short of what it holds. Execution
/// alone never reaches either (the admission gate and the escrow program keep them); the states are made.
#[test] fn e4_failing_children_fail_alike_on_the_overlay_path(){
    use crate::state_summary::Summary;
    let net=devnet3();let cp=net.profile.cp().clone();let ids:Vec<Address>=net.miners.iter().map(|m|m.id).collect();
    let input=|h:u64,prev:Option<ParentResult>|BlockInput{height:h,timestamp:1_800_000_000+2*h,base_fee:U256::from(1_000_000_000u64),gas_limit:cp.GAS_LIMIT,prevrandao:B256::ZERO,
        proposer:Proposer::Fallback,parent_result:prev,evidence:vec![],evidence_raw:&[],transactions:&[],ancestors:vec![(0,net.genesis_hash)],cum_log_bytes:0};
    let pr=|h:u64|Some(ParentResult{block_hash:B256::from(U256::from(h)),winner:ids[0],shares:vec![(10,ids[1])]});
    let o1=apply_block(net.genesis.state(),&net.genesis,&net.profile,&input(1,None)).unwrap();
    // A parent at exactly SYS_SLOT_MAX: block 2's SYSTEM calls record its parent's shares and result.
    let mut d=o1.state.clone();let sys=program_address(4);let now=Summary::scan(&d,cp.B_code_max).sys_slots;
    for k in 0..u64::from(cp.SYS_SLOT_MAX)-now{d.insert_account_storage(sys,U256::from(k)+U256::from(1u64<<40),U256::from(1)).unwrap();}
    let sum=Summary::scan(&d,cp.B_code_max);assert_eq!(sum.sys_slots,u64::from(cp.SYS_SLOT_MAX),"positive control: the parent is at the bound");
    let whole=apply_block(&d,&net.genesis,&net.profile,&input(2,pr(1)));assert!(matches!(whole,Err(ApplyError::Genesis(_))),"positive control: the child is over it ({:?})",whole.as_ref().err());
    let on=apply_block_on(&d,&sum,&net.genesis,&net.profile,&input(2,pr(1)));
    assert!(matches!(on,Err(ApplyError::Genesis(_))),"the overlay path refuses it too");
    assert!(matches!(apply_block_with(&d,None,Some(&sum),&net.genesis,&net.profile,&input(2,pr(1))),Err(ApplyError::Genesis(_))));
    // A parent one system slot over the bound is refused as the parent, before the block runs.
    d.insert_account_storage(sys,U256::from(1u64<<41),U256::from(1)).unwrap();let sum=Summary::scan(&d,cp.B_code_max);
    let parent_err=|r:Result<BlockRun,ApplyError>|match r{Err(ApplyError::Genesis(m))=>m.starts_with("parent"),_=>false};
    assert!(parent_err(apply_block_on(&d,&sum,&net.genesis,&net.profile,&input(2,pr(1)))),"the parent check refuses a parent over the bound");
    // A parent whose escrow is one wei short: the child's escrow invariant fails.
    let o2=apply_block(&o1.state,&net.genesis,&net.profile,&input(2,pr(1))).unwrap();
    let mut d=o2.state.clone();let e=escrow();let mut info=d.accounts[&e].info.clone();info.balance-=U256::from(1);d.insert_account_info(e,info);
    let sum=Summary::scan(&d,cp.B_code_max);
    let whole=apply_block(&d,&net.genesis,&net.profile,&input(3,pr(2)));assert!(matches!(whole,Err(ApplyError::Supply(_))),"positive control: the invariant fails ({:?})",whole.as_ref().err());
    assert!(matches!(apply_block_on(&d,&sum,&net.genesis,&net.profile,&input(3,pr(2))),Err(ApplyError::Supply(_))),"the overlay path refuses it too");
}

/// C-1 E2: CODE_CAP reached exactly by a create, with the parent's summary as without it: the running code
/// total the SYSTEM calls and the USER phase start from is the parent's exact total.
#[test] fn e2_code_cap_reached_exactly_with_the_summary(){
    use crate::state_summary::Summary;
    let first=devnet3();let total=Summary::scan(first.genesis.state(),first.profile.cp().B_code_max).code_total as u64;
    let mut c=DevnetConfig::new(777900,1_800_000_000,&["a","b","c"]);c.code_cap=total+3;let net=devnet::build(&c).expect("devnet");
    assert_eq!(Summary::scan(net.genesis.state(),net.profile.cp().B_code_max).code_total as u64,total);
    let alice=devnet::account_key("alice");let init=vec![0x62,0x60,0x00,0x55,0x60,0,0x52,0x60,3,0x60,29,0xf3];
    let tx=|nonce:u64|crate::txsign::Tx{chain_id:777900,nonce,max_priority:U256::from(1_000_000_000u64),max_fee:U256::from(100_000_000_000u64),gas:200_000,to:None,value:U256::ZERO,data:init.clone().into()}.sign_1559(&alice);
    let ids:Vec<Address>=net.miners.iter().map(|m|m.id).collect();let g=net.genesis.state().clone();
    let s=Summary::scan(&g,net.profile.cp().B_code_max);let t=crate::state_trie::StateTrie::build(&g);
    // Two creates: the first reaches CODE_CAP exactly, the second would pass it.
    let txs=vec![tx(0),tx(1)];
    let input=BlockInput{height:1,timestamp:1_800_000_002,base_fee:U256::from(1_000_000_000u64),gas_limit:8_000_000,prevrandao:B256::ZERO,proposer:Proposer::Signed(ids[1]),
        parent_result:None,evidence:vec![],evidence_raw:&[],transactions:&txs,ancestors:vec![(0,net.genesis_hash)],cum_log_bytes:0};
    let r=apply_block(&g,&net.genesis,&net.profile,&input).unwrap();
    let o=apply_block_with(&g,Some(&t),Some(&s),&net.genesis,&net.profile,&input).unwrap();
    assert_eq!((o.state_root,o.receipts_root,o.summary),(r.state_root,r.receipts_root,r.summary));
    assert_eq!(r.summary.code_total as u64,total+3,"positive control: the first create reached CODE_CAP");
    assert_eq!(o.state.contracts.len(),r.state.contracts.len());
    // A child of the full state: its SYSTEM calls start from a total of exactly CODE_CAP.
    let pr=ParentResult{block_hash:B256::from(U256::from(1)),winner:ids[1],shares:vec![]};
    let input2=BlockInput{height:2,timestamp:1_800_000_004,base_fee:crate::fees::next_base_fee(U256::from(1_000_000_000u64),r.gas_used,8_000_000).unwrap(),gas_limit:8_000_000,prevrandao:B256::ZERO,proposer:Proposer::Fallback,
        parent_result:Some(pr),evidence:vec![],evidence_raw:&[],transactions:&[],ancestors:vec![(0,net.genesis_hash)],cum_log_bytes:0};
    let r2=apply_block(&r.state,&net.genesis,&net.profile,&input2).unwrap();
    let o2=apply_block_with(&o.state,Some(&o.trie),Some(&o.summary),&net.genesis,&net.profile,&input2).unwrap();
    assert_eq!((o2.state_root,o2.summary),(r2.state_root,r2.summary));
}
