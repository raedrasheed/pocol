//! FN-3c (C6, test support): a v3 genesis whose ActList holds up to 10,000 identities, for the tests and measurements at
//! the fresh network's M_max (FN-3 plan §5.14; design launch evidence M1). It bypasses the 256-miner limit of the
//! public testnet builder (`pocold::testnet`, which `pocold init-testnet` reads) by building the candidate at the host
//! level (`devnet::build`): the identities are the genesis M_0 list, so the genesis state registers every one exactly as
//! a network registers its founders, and the chain's own admission gate admits it. No registration transaction is sent:
//! 10,000 registrations at R_max 4 a block would take 2,500 blocks (about 7 h at T_blk 10 s, the M1 recipe).
//! The keys are derived from labels (`devnet::miner_identity`), so a test holds every member's key and can sign as Draw(h,0)
//! and as any winner. Private fixture keys only.
#![allow(dead_code)]
use std::collections::HashMap;
use pocol_lp4_execution::{admission::Timing,devnet::{self,DevnetConfig,Devnet},txsign::{Key,Tx}};
use pocold::chain::Chain;
use pocold::types::*;
use revm::primitives::{keccak256,Address,Bytes,B256,U256};

/// The fresh network's M_max (owner decision 2026-11-13).
pub const M_MAX:u16=10_000;
/// The chain id of these fixtures (never a network's).
pub const CHAIN_ID:u64=777_960;

/// The fresh-network CP (FRESH-NETWORK-DESIGN.md §1a, the values FN-3c's A13 test and harness use) as a devnet candidate
/// with `n` genesis members: CP schema v2 (the v3 programs, earn-in bonds, the full nonce domain), T_blk 10, tau 600, c 2,
/// D_att 90, dFbWait 8, m 32, kappa 2, S_max 256, D 4, W_w 8, R_max 4, M_max 10,000, REG_RECORDS_MAX 24,576, REG_OPS 16,
/// W_act 32, Theta_inact 360, E_win 360, U 380, E_max 4, GAS_LIMIT 8,000,000, CODE_CAP 2^30, TXBYTES_CAP 2^24, TX_CAP
/// 20,000, SLOT_CAP 4,000,000, ACCT_CAP 1,000,000, SYS_SLOT_MAX 147,456, H_END 0 (continuous), Z_close 256, RET 128, P2 2,
/// H_half 12,623,040, S_tail 1.25·10^17, gamma_tip 5000, baseFeeMin 10^7, R_react 4, C_react 380, rho 2500. Two values are not the network's and
/// enter no A13 term and no membership rule: the subsidy S0 (the builder writes 2 PCOL, the design 4) and target_g,
/// calibrated at `hash_rate` (a fixture difficulty, so a test solves a block in a few hundred hashes).
pub fn fresh_config(n:usize,hash_rate:u64)->DevnetConfig{
    let mut c=DevnetConfig::new(CHAIN_ID,1_700_000_000,&[]);
    c.miners=labels(n);
    c.spec_version=2;c.earnin=true;c.nonce_mode=1;c.hash_rate=hash_rate;
    c.t_blk=10;c.tau=600;c.c=2;c.d_att=90;c.d_fb_wait=8;c.m=32;c.kappa=2;c.s_max=256;c.d=4;c.w_w=8;c.alpha_bp=1000;c.gamma_bp=500;
    c.r_max=4;c.m_max=M_MAX;c.reg_records_max=24_576;c.w_act=32;c.theta_inact=360;c.e_win=360;c.u=380;
    c.gas_limit=8_000_000;c.code_cap=1<<30;c.txbytes_cap=1<<24;c.tx_cap=20_000;c.slot_cap=4_000_000;c.acct_cap=1_000_000;c.sys_slot_max=147_456;
    c.h_end=0;c.z_close=256;c.ret=128;c.p2=2;
    c.h_half=12_623_040;c.s_tail=U256::from(125_000_000_000_000_000u64);c.gamma_tip_bp=5000;c.base_fee_min=U256::from(10_000_000u64);c.r_react=4;c.c_react=380;c.rho_bp=2500;
    c
}
/// The member labels `fresh-0` .. `fresh-{n-1}`.
pub fn labels(n:usize)->Vec<String>{(0..n).map(|i|format!("fresh-{i}")).collect()}
/// The candidate, every member's key and every funded account's key by address, and the member ids (sorted).
pub struct Genesis{pub candidate:Devnet,pub keys:HashMap<Address,Key>,pub ids:Vec<Address>}
/// Builds the candidate of `cfg` (its members are `cfg.miners`, its funded accounts `cfg.accounts`): `devnet::build`
/// derives each member's key from its label, writes the M_0 list into the genesis spec and registers every member in the
/// genesis state (10,000 members: about 1.2 s in release on one core).
pub fn genesis(cfg:&DevnetConfig)->Genesis{
    let candidate=devnet::build(cfg).unwrap_or_else(|e|panic!("the {}-member candidate: {e}",cfg.miners.len()));
    let mut keys:HashMap<Address,Key>=candidate.miners.iter().map(|m|(m.id,Key::from_secret(&m.key.secret()).unwrap())).collect();
    for (label,_) in &cfg.accounts{let k=devnet::account_key(label);keys.insert(k.address(),k);}
    let ids=candidate.miners.iter().map(|m|m.id).collect();
    Genesis{candidate,keys,ids}
}
/// The ActList of the candidate's genesis state: every member the genesis registered, in list order.
pub fn genesis_active(g:&Genesis)->Vec<Address>{pocol_lp4_execution::apply::active_list(g.candidate.genesis.state())}
/// The admitted spec of a candidate (the chain's own admission gate at the single-host local profile) and the timing the
/// gate measured. The measurement applies 2 + ceil(|M_0|/RET) + D + 4 full blocks three times each: about 100 s in
/// release at 10,000 members. `readmit` admits the same candidate again from that timing, without measuring.
pub fn admit(g:&Genesis)->(pocold::chain::Spec,Timing){
    let (a,t)=g.candidate.admit(&pocold::single_host_local()).unwrap_or_else(|e|panic!("the candidate is admitted: {e}"));
    (pocold::chain::Spec::new(a),t)
}
/// The candidate admitted again by the gate from a timing it measured before (`admit`): a second chain or node of the
/// same genesis without a second measurement.
pub fn readmit(g:&Genesis,t:&Timing)->pocold::chain::Spec{
    let local=pocold::single_host_local();
    pocold::chain::Spec::new(pocol_lp4_execution::admission::admit(&g.candidate.request(&local,Some(t))).unwrap_or_else(|e|panic!("the candidate is admitted again: {e}")))
}

/// The nonces a solver scans in each member's range (the full domain gives every range 2^64 / M nonces).
pub const SCAN:u128=64;
/// A signed attempt-0 block on `parent` at ts = parent ts + T_blk by Draw(h,0), carrying `body` (its evidence effects
/// applied), and its winner. The solver starts at a member's range picked from the height (`h · 2654435761 mod M`) and
/// scans SCAN nonces of each range from there, a pass over every range at a time, so the winners spread over the
/// members; the header carries up to 8 shares found on the way.
pub fn mine(c:&Chain,keys:&HashMap<Address,Key>,parent:&[u8;32],body:Body)->(Block,Address){
    let p=c.full_entry(parent).expect("the parent is held");let h=p.height+1;
    let effects=c.evidence_effects(&p,&body,h).unwrap_or_else(|e|panic!("h={h}: evidence effects: {e:?}"));
    let ctx=c.child_ctx(&p,0).unwrap_or_else(|e|panic!("h={h}: child context: {e:?}"));
    let proposer=keys.get(&ctx.proposer).unwrap_or_else(||panic!("h={h}: no key for the proposer {}",ctx.proposer));
    let ts=p.ts+u64::from(c.spec.cp().T_blk);
    let (mut header,_,_)=c.build_signed(&p,proposer,ts,body.clone(),effects).unwrap_or_else(|e|panic!("h={h}: build_signed: {e:?}"));
    let tid=template_id(&header.ut_raw);let ranges=ctx.assignment.ranges();let n=ranges.len();
    let first=((h as u128*2_654_435_761)%n as u128) as usize;let mut shares=Vec::new();
    // Pass k scans nonces [start + k·SCAN, start + (k+1)·SCAN) of every range, from the picked one on (a few members
    // need more than one pass).
    for pass in 0..1u128<<16{
        for i in 0..n{
            let r=&ranges[(first+i)%n];let lo=r.start()+pass*SCAN;
            for nonce in lo..r.end_exclusive().min(lo+SCAN){
                let nonce=nonce as u64;let ph=pow_hash(&tid,nonce);
                if le_target(&ph,&ctx.target){
                    let owner=addr(&r.id());let key=keys.get(&owner).unwrap_or_else(||panic!("h={h}: no key for the winner {owner}"));
                    shares.sort();header.nonce=nonce;header.shares=shares;reseal(&mut header);
                    header.winner_sig=key.sign_hash(&B256::from(pocold::hashes::win_msg(&tid,nonce,&share_root(&header))));reseal(&mut header);
                    return (Block{header,body},owner);
                }
                if le_target(&ph,&ctx.t_share)&&shares.len()<8{shares.push(nonce);}
            }
        }
    }
    panic!("h={h}: no solution in the scanned nonces")
}
/// Type-2 evidence against block `b` won by `winner`: a second share root signed by the winner for its TemplateID and
/// nonce (as tests/fn3_members.rs `type2`).
pub fn type2(keys:&HashMap<Address,Key>,b:&Block,winner:Address)->Vec<u8>{
    use pocold::rlp;
    let tid=template_id(&b.header.ut_raw);
    let mut alt=b.header.clone();alt.shares=if alt.shares.is_empty(){vec![0]}else{vec![]};reseal(&mut alt);
    let (r1,r2)=(share_root(&b.header),share_root(&alt));
    let sig2=keys[&winner].sign_hash(&B256::from(pocold::hashes::win_msg(&tid,b.header.nonce,&r2)));
    let mut ev=Vec::new();rlp::encode_bytes(&[2],&mut ev);rlp::encode_bytes(&tid,&mut ev);rlp::encode_bytes(&b.header.nonce.to_be_bytes(),&mut ev);
    rlp::encode_bytes(&r1,&mut ev);rlp::encode_bytes(&b.header.winner_sig,&mut ev);rlp::encode_bytes(&r2,&mut ev);rlp::encode_bytes(&sig2,&mut ev);
    let mut out=Vec::new();rlp::encode_list_payload(&ev,&mut out);out
}
/// A call of the registry (`apply::registry`) from `from` at `nonce`: `sel` with an optional address argument.
pub fn registry_call(c:&Chain,keys:&HashMap<Address,Key>,from:Address,nonce:u64,base_fee:U256,sel:&str,arg:Option<Address>)->Vec<u8>{
    let mut data=keccak256(sel.as_bytes())[..4].to_vec();if let Some(a)=arg{data.extend_from_slice(a.into_word().as_slice());}
    let prio=U256::from(1_000_000_000u64);
    Tx{chain_id:c.spec.chain_id(),nonce,max_priority:prio,max_fee:base_fee*U256::from(2)+prio,gas:600_000,to:Some(pocol_lp4_execution::apply::registry()),value:U256::ZERO,data:Bytes::from(data)}.sign_1559(&keys[&from])
}
/// A transfer of `value` from `from` at `nonce` to `to`.
pub fn transfer(c:&Chain,keys:&HashMap<Address,Key>,from:Address,nonce:u64,base_fee:U256,to:Address,value:U256)->Vec<u8>{
    let prio=U256::from(1_000_000_000u64);
    Tx{chain_id:c.spec.chain_id(),nonce,max_priority:prio,max_fee:base_fee*U256::from(2)+prio,gas:21_000,to:Some(to),value,data:Bytes::new()}.sign_1559(&keys[&from])
}
