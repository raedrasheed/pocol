//! Local wallet commands. No signing/control method is exposed to P2P or HTTP RPC.
//! A quote is immutable, expires, and is consumed exactly once by an explicit confirmation.
//!
//! Rejoin (client side only; the Registry rules are unchanged). A status-1 member whose last share
//! and grace start are both older than W_act is outside A_h and gets no nonce range; only
//! `reactivate()` resets graceStart, and it requires an exited record. One confirmed rejoin quote
//! therefore covers, with the fee cap it shows:
//! 1. `deregister()` now (HELD, status 1, more than M_min active; status 2, exit=h, due=h+U);
//! 2. `reactivate()` once the next block is in [exit+W_act, due), at most REACTIVATE_ATTEMPTS
//!    signatures, one per new head (R_max can let another registration take the block's slot);
//! 3. mining on once the account is in A_{h+1} again.
//! If the window passes, the Registry sweep refunds the bond and erases the record at `due`.
//!
//! Contest (V3-10/V3-14, the v3 program set only). `quote_contest` names a victim and a bond. The registry's own view,
//! contestStatus(victim, probation), is run on the head state at the next height, so the wallet's checks are the program's:
//! every refusal is explained before anything is signed, with the measured count and threshold, and the quote states what
//! the confirmed registerContest(reward, victim) does: both exit the victim (its bond is refunded after U, no penalty), and
//! a pool contest also frees the probationer's probation seat at once (its part of pots not yet settled still fills its
//! accrued bond; owner decision 2026-11-13); both need all seats taken; this account then registers as with
//! quote_register. The registry decides again at inclusion (its phase 0 may credit shares or start an epoch first).
//! `contest_candidates` (read-only) lists the members the registry's view accepts for this account now.
//!
//! V3 (the v3 program set only; spec-1 answers are unchanged). Rejoin reads the record's own reactivation clock (V3-09,
//! `rejoin_step_with`): one reactivation per C_react blocks, so the window after an exit opens at
//! max(exit + W_act, lastReactHeight + C_react) and a window the clock empties is missed at once; no reactivation is signed
//! while it would revert with Capacity (a full set, or a full probation pool for a pool-contest victim). The status shows
//! the bond still to accrue on probation, the clock, and the pool-contest victim (V3-14, owner decision 2026-11-13,
//! option a: exited, still paid into its bond and punishable until its due height). A top-up refusal says why
//! (HELD, exited, no record).
//!
//! Creation burn (D-V3-21 option A, client part; only where the next block charges it): a transfer quote takes its gas
//! limit from the burn-aware eth_estimateGas, or from the user's `fee_bound` (max_fee*gas_limit at the quoted max_fee),
//! shows the quoted burn apart from the gas fee and the amount, and is refused before signing when the quoted gas and burn
//! exceed the signed maximum (`BurnPlan`). Registry calls write only system storage and carry no creation burn: they keep
//! their measured gas limits.
use crate::{launcher::Home,node::Node,types};
use pocol_lp4_execution::{apply,txsign::{Key,Tx},view::View};
use revm::primitives::{Address,B256,Bytes,U256,keccak256};
use serde::Deserialize;
use serde_json::{json,Value};
use std::{collections::VecDeque,time::{Duration,Instant},io::{BufRead,Write},sync::mpsc};

pub const MAX_COMMAND:usize=4096;
#[derive(Deserialize)]
#[serde(tag="method",rename_all="snake_case",deny_unknown_fields)]
pub enum Command {
    Status,
    Mining { enabled:bool },
    /// `fee_bound` (D-V3-21, only where the next block charges a creation burn): the most PCOL the user signs for gas and
    /// burn together, max_fee*gas_limit (default: the burn-aware estimate's).
    QuoteSend { to:String, amount:String, #[serde(default)] fee_bound:Option<String> },
    /// V3-10/V3-14 (read-only, nothing is signed): the members the registry would let this account contest now. A struct
    /// variant, so that unknown fields are refused (serde accepts any field beside a unit variant's tag).
    ContestCandidates {},
    /// `bond`: the PCOL locked now. Earn-in networks take 0..B (default 0: the rest is earned from rewards);
    /// other networks take exactly B (the default).
    QuoteRegister { #[serde(default)] bond:Option<String> },
    QuoteRejoin,
    /// Earn-in: fill an ACCRUING bond (default: the whole remainder).
    QuoteTopUp { #[serde(default)] amount:Option<String> },
    /// V3-10/V3-14: register by displacing `victim` (a 0x address) from a full set or probation pool; `bond` as in
    /// QuoteRegister (default 0 on these networks: a probation bond).
    QuoteContest { victim:String, #[serde(default)] bond:Option<String> },
    Confirm { quote_id:String },
    Cancel,
    Stop,
}
/// `tx` is signed on confirmation; `rejoin` starts the confirmed rejoin plan with the quoted fee cap.
struct Quote { id:String, tx:Option<Tx>, deadline:Instant, kind:&'static str, rejoin:Option<(U256,U256)> }
/// Reactivation signatures one rejoin confirmation allows (each counted in its maximum fee).
pub const REACTIVATE_ATTEMPTS:u8=3;
const REGISTRY_GAS:u64=1_000_000;
/// Earn-in registration: about 2x the 194,143 gas a zero-bond first registration measures
/// (lp4-execution tests/earnin_contracts.rs), so the fee faucet's 0.002 PCOL covers its maximum fee.
const EARNIN_REGISTER_GAS:u64=400_000;
/// V3-10/V3-14 registerContest: about 2x the largest contest measured (232,489 gas, a seat contest with the victim in the
/// middle of the active list; lp4-execution tests/v3_contracts.rs contest_write_ceilings_gas_and_user_sys_keys), so that a
/// zero-bond newcomer's fee faucet (0.002 PCOL) covers its maximum fee while the base fee stays below 1.5 gwei
/// (deploy/faucet/README.md).
pub const CONTEST_GAS:u64=500_000;
/// Heads the exit may stay unincluded before the plan stops (nothing changed on chain then).
pub const EXIT_WAIT_BLOCKS:u64=30;
struct Rejoin { exit:Option<(B256,u64)>, max_fee:U256, priority:U256, attempts:u8, last:Option<B256>, tried_at:Option<u64> }
pub struct Wallet<'a> { home:&'a Home,key:Key,quote:Option<Quote>,rejoin:Option<Rejoin>,activity:VecDeque<Value>,pub stopped:bool }

/// Where a registry record stands for rejoining, at the next block `next`. `member` is whether the
/// account is in A_next (consensus membership from the head state).
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum RejoinStep {
    /// No record, a tomb, a bond that is not HELD, or a closing record.
    Unavailable,
    /// In the active set: it has a nonce range.
    Active,
    /// Status 1 but outside A_next: exit, then reactivate.
    Exit,
    /// Exited: reactivation opens at `opens`; the sweep refunds at `closes`.
    Wait { opens:u64, closes:u64 },
    /// Exited and the next block may reactivate (`next` < `closes`).
    Reactivate { closes:u64 },
    /// Exited and past the window: the sweep refunds the bond and erases the record.
    Missed,
    /// V3-09 (v3 only): status 1 outside A_next, but the identity reactivated less than C_react blocks ago, so an exit now
    /// could not be reactivated before its refund sweep; exiting is useful from `exit_from`.
    Cooldown { exit_from:u64 },
}
/// V3-09 (the v3 program set only): an identity reactivates at most once per C_react blocks, counted from its previous
/// reactivation (the per-record clock lastReactHeight, w1 bits 160..223; 0 before its first reactivation).
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct ReactClock{pub last:u64,pub c_react:u64,pub u:u64}
impl ReactClock{
    /// The first height at which the clock allows a reactivation (0: any).
    pub fn allows_from(&self)->u64{if self.last==0{0}else{self.last.saturating_add(self.c_react)}}
}
/// The registry's per-record reactivation clock (V3-09; PoColSystemV3.sol recordClocks): lastReactHeight in w1 bits
/// 160..223, 0 for an identity that never reactivated or has no record. Only the v3 program set writes it.
pub fn react_clock(v:&dyn View,id:Address)->u64{
    let w1=v.slot(&apply::program_address(0),&(apply::record_slot(id)+U256::from(1)));
    (w1>>160usize).as_limbs()[0]
}
/// V3-14 (the v3 program set only): a probation-pool contest victim (POOL_OUT, w0 bit 87) gave up its probation seat at
/// the contest; it stays exited, payable and punishable until its due height (owner decision 2026-11-13, option a).
pub fn pool_contested(v:&dyn View,id:Address)->bool{
    v.slot(&apply::program_address(0),&apply::record_slot(id)).bit(87)
}
/// `t_prob`: the probation deadline in blocks of the genesis programs (`Spec::probation`), None without earn-in.
/// The spec-1 rule; `rejoin_step_with` adds the v3 reactivation clock.
pub fn rejoin_step(record:Option<&apply::MinerRecord>,next:u64,member:bool,w_act:u64,t_prob:Option<u64>)->RejoinStep{
    rejoin_step_with(record,next,member,w_act,t_prob,None)
}
/// The rejoin step with the v3 programs' reactivation clock (`clock`, V3-09; None on the spec-1 program sets, where
/// the result is `rejoin_step`'s). reactivate() is valid at height h only when exit + W_act <= h < due (strictly before
/// the refund sweep), before registeredAt + T_PROB for a probation record, and, once the identity has reactivated,
/// at or after lastReactHeight + C_react. So on v3:
/// - an exited record's window opens at max(exit + W_act, lastReactHeight + C_react), and a window the clock empties
///   is `Missed` at once (the sweep refunds at due);
/// - a status-1 record outside A_next is told to exit only when the window after an exit at `next` would hold at least
///   two blocks (the least window ParamGate R8 admits); before that it is `Cooldown` until the first such exit height.
pub fn rejoin_step_with(record:Option<&apply::MinerRecord>,next:u64,member:bool,w_act:u64,t_prob:Option<u64>,clock:Option<ReactClock>)->RejoinStep{
    let Some(r)=record else{return RejoinStep::Unavailable};
    if r.tomb||!matches!(r.bond_state,1|4){return RejoinStep::Unavailable}
    // Earn-in (EARN-IN-SPEC U4): a probation record reactivates only before registeredAt + T_PROB
    // (60,480 on the earn-in programs, CP T_PROB on the v3 programs, V3-14).
    let deadline=t_prob.filter(|_|r.bond_state==apply::SystemPrograms::EARNIN_ACCRUING).map(|t|r.registered_at.saturating_add(t));
    let gate=clock.map_or(0,|c|c.allows_from());
    // V3-09: an exit now leaves [max(next + W_act, gate), next + U); two blocks of it once next >= gate + 2 - U.
    match r.status{
        1 if member=>RejoinStep::Active,
        1 if deadline.is_some_and(|d|next.saturating_add(w_act).max(gate)>=d)=>RejoinStep::Missed,
        1=>match clock.filter(|c|gate.saturating_add(2)>next.saturating_add(c.u)){Some(c)=>RejoinStep::Cooldown{exit_from:gate.saturating_add(2)-c.u},None=>RejoinStep::Exit},
        2=>{let opens=r.exit_height.saturating_add(w_act).max(gate);let closes=deadline.map_or(r.due_height,|d|d.min(r.due_height));
            if next>=closes||(clock.is_some()&&opens>=closes){RejoinStep::Missed}else if next<opens{RejoinStep::Wait{opens,closes}}else{RejoinStep::Reactivate{closes}}},
        _=>RejoinStep::Unavailable,
    }
}
fn step_json(step:RejoinStep)->Value{
    match step{
        RejoinStep::Unavailable=>json!({"step":"unavailable"}),
        RejoinStep::Active=>json!({"step":"active"}),
        RejoinStep::Exit=>json!({"step":"exit"}),
        RejoinStep::Wait{opens,closes}=>json!({"step":"wait","opensAt":opens,"closesAt":closes}),
        RejoinStep::Reactivate{closes}=>json!({"step":"reactivate","closesAt":closes}),
        RejoinStep::Missed=>json!({"step":"missed"}),
        RejoinStep::Cooldown{exit_from}=>json!({"step":"cooldown","exitFrom":exit_from}),
    }
}
/// PAY-02 judges a reward address on the pre-transaction state: it must have sent a transaction
/// (or hold code). The wallet registers with itself as the reward address, so the included nonce decides.
fn registration_reward_durable(included_nonce:Option<u64>)->bool{included_nonce.is_some_and(|n|n>0)}
fn selector(signature:&str)->Bytes{keccak256(signature)[..4].to_vec().into()}
/// registerContest(reward, victim): the account registers with itself as the reward address, as quote_register does.
pub fn contest_calldata(reward:Address,victim:Address)->Bytes{
    let mut d=keccak256("registerContest(address,address)")[..4].to_vec();d.extend_from_slice(reward.into_word().as_slice());d.extend_from_slice(victim.into_word().as_slice());d.into()
}
/// What the registry's contestStatus view returned, with the network values the explanation needs.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct ContestView{pub code:u64,pub count:u64,pub threshold:U256}
/// The CP values a contest explanation quotes.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct ContestParams{pub next:u64,pub e_epoch:u64,pub d:u64,pub theta_inact:u64,pub u:u64,pub m_max:u64,pub p_max:u64,pub grace_start:u64}
/// The first height at which the late window of the epoch before `h`'s has closed (contests are judged from there).
fn contest_opens(p:&ContestParams)->u64{
    let start=(p.next/p.e_epoch).max(1)*p.e_epoch;let open=start+p.d.saturating_sub(1);
    if p.e_epoch<p.d.max(1){u64::MAX}else if p.next>=open{p.next}else{open}
}
/// V3-10/V3-14: the registry's contestStatus(victim, probation) view, run on `state` (a head state) at height `next` with
/// timestamp `ts`, as an eth_call would run it: the program's own rule, never a copy of it.
pub fn contest_view_on(state:&dyn View,profile:&pocol_lp4_execution::profile::ProfileIdentity,next:u64,ts:u64,caller:Address,victim:Address,probation:bool)->Result<ContestView,String>{
    let cp=profile.cp();
    let mut data=keccak256("contestStatus(address,bool)")[..4].to_vec();data.extend_from_slice(victim.into_word().as_slice());data.extend_from_slice(&U256::from(probation as u8).to_be_bytes::<32>());
    let tx=revm::primitives::TxEnv{caller,transact_to:revm::primitives::TxKind::Call(apply::program_address(0)),gas_limit:cp.GAS_LIMIT.clamp(1_000_000,30_000_000),gas_price:U256::ZERO,
        value:U256::ZERO,data:data.into(),nonce:None,chain_id:Some(profile.spec().chain_id),..Default::default()};
    let ctx=pocol_lp4_execution::native::NativeBlockContext{height:U256::from(next),timestamp:U256::from(ts),gas_limit:U256::from(cp.GAS_LIMIT.clamp(1_000_000,30_000_000)),base_fee:U256::ZERO,prevrandao:B256::ZERO};
    let st=pocol_lp4_execution::view::Overlay::new(state);
    let r=pocol_lp4_execution::native::simulate_for_profile(&st,tx,&ctx,profile,std::rc::Rc::new(||false)).map_err(|e|format!("The registry could not be read: {e:?}"))?;
    match r.result{
        revm::primitives::ExecutionResult::Success{output,..} if output.data().len()==96=>{let o=output.data();
            Ok(ContestView{code:U256::from_be_slice(&o[0..32]).to::<u64>(),count:U256::from_be_slice(&o[32..64]).to::<u64>(),threshold:U256::from_be_slice(&o[64..96])})}
        other=>Err(format!("The registry could not judge the contest: {other:?}")),
    }
}
/// The registry's refusal codes (PoColSystemV3.sol contestCheck) as the wallet says them; Ok(true) for a pool contest.
pub fn contest_check(v:&ContestView,p:&ContestParams,probation:bool,pool_full:bool)->Result<bool,String>{
    match v.code{
        0=>Ok(probation&&pool_full),
        1=>Err("That address has no active registry record, so there is nothing to contest.".into()),
        // A pool contest needs the full set as well (O-10=b: contests only when all seats are full).
        2=>Err(format!("Contests open only when all {} seats are taken. Register normally instead.",p.m_max)),
        3=>Err(format!("All {} probation seats are taken, so a probation bond can contest only another probationer. Pick a probationer, or contest with the full bond while all seats are taken.",p.p_max)),
        4=>Err(match contest_opens(p){u64::MAX=>"This network's epoch is shorter than its late-share window, so no member can ever be contested.".into(),
            b=>format!("No full epoch can be judged yet: the previous epoch's late shares are still being credited. Contests open at block {b}.")}),
        5=>Err(format!("The member is still in its grace period, until block {}.",p.grace_start.saturating_add(p.theta_inact))),
        6=>{let first=p.grace_start.div_ceil(p.e_epoch)*p.e_epoch;
            Err(format!("The member joined or rejoined during the epoch being measured. It can be judged on its first full epoch, from block {} at the earliest.",(first+p.e_epoch).saturating_add(p.d.saturating_sub(1))))}
        7=>Err(format!("The member credited {} shares in the last full epoch, not below the threshold of {}. It cannot be contested.",v.count,v.threshold)),
        c=>Err(format!("The registry refused the contest (code {c}).")),
    }
}
/// What a contest quote signs and states, decided from the head state alone (no fee, no peers), so that tests can drive
/// the whole quote decision on a real chain: the guards, the bond, the registry's verdict and the seat or pool routing.
#[derive(Clone,Debug)]
pub struct ContestPlan{pub victim:Address,pub value:U256,pub full:U256,pub probation:bool,pub pool:bool,pub view:ContestView,pub params:ContestParams}
/// V3-10/V3-14: the contest `me` would send against `victim` with `bond` (a decimal PCOL amount; default 0) at the head.
pub fn contest_plan(chain:&crate::chain::Chain,me:Address,victim:&str,bond:Option<&str>)->Result<ContestPlan,String>{
    if chain.spec.programs()!=apply::ProgramSet::V3{return Err("Contests exist only on the v3 network".into())}
    let head=chain.head();let cp=chain.spec.cp();
    let state=chain.view_of(&head.hash).ok_or("The head state is not available")?;
    if apply::miner_record(state.v(),me).is_some(){return Err("This identity already has a registry record".into())}
    if victim.len()!=42||!victim.starts_with("0x"){return Err("Enter the member's full 0x address (42 characters)".into())}
    let victim:Address=victim.parse().map_err(|_|"Invalid member address")?;
    if victim==me{return Err("An identity cannot contest itself".into())}
    let full=crate::chain::from_fixed_lp4(&cp.B_reg);
    let value=match bond{None|Some("0")=>U256::ZERO,Some(text)=>amount(text)?};
    if value>full{return Err(format!("The bond is at most {} PCOL",coins(full)))}
    let probation=value<full;
    let taken=state.v().slot(&apply::program_address(0),&U256::from(8));let seats=chain.spec.probation().map_or(0,|p|p.seats);
    let record=apply::miner_record(state.v(),victim);
    let params=ContestParams{next:head.height+1,e_epoch:u64::from(cp.E_epoch).max(1),d:u64::from(cp.D),theta_inact:u64::from(cp.Theta_inact),u:u64::from(cp.U),
        m_max:u64::from(cp.M_max),p_max:seats,grace_start:record.map_or(0,|r|r.grace_start)};
    let view=contest_view_on(state.v(),chain.spec.profile(),head.height+1,head.ts+1,me,victim,probation)?;
    let pool=contest_check(&view,&params,probation,taken>=U256::from(seats))?;
    Ok(ContestPlan{victim,value,full,probation,pool,view,params})
}
impl ContestPlan{
    /// The quote's contest fields: what the confirmed registerContest does, in the wallet's words.
    pub fn describe(&self)->Value{
        let (victim,v,p,epoch)=(self.victim,&self.view,&self.params,self.params.next/self.params.e_epoch-1);
        let steps=if self.pool{vec![format!("The probationer {victim:#x} credited {} shares in epoch {epoch}, below the threshold of {}.",v.count,v.threshold),
            format!("It leaves the active set and its probation seat now; its accrued bond, with its part of the pots of the last {} blocks not yet settled, is refunded after {} blocks (no penalty). You take its probation seat.",p.d,p.u),
            format!("You register with a {} PCOL bond, filled from your rewards up to {} PCOL.",coins(self.value),coins(self.full))]}
        else{vec![format!("The member {victim:#x} credited {} shares in epoch {epoch}, below the threshold of {}.",v.count,v.threshold),
            format!("It leaves the active set; its bond is refunded after {} blocks (no penalty).",p.u),
            format!("You take its seat with a {} PCOL bond{}.",coins(self.value),if self.probation{format!(", filled from your rewards up to {} PCOL",coins(self.full))}else{String::new()})]};
        json!({"victim":format!("{victim:#x}"),"contest":{"kind":if self.pool{"pool"}else{"seat"},"count":v.count,"threshold":v.threshold.to_string(),"epoch":epoch,
            "refundAfterBlocks":p.u},"steps":steps,"bondTarget":coins(self.full),"probation":self.probation,
            "note":"Checked against the current head. The registry judges the contest again when the transaction is included; if a share or a new epoch changes the result first, it reverts and only the fee is spent."})
    }
}
/// The status flags of a contest (v3 program set only): whether a newcomer would need quote_contest (all seats taken;
/// a pool contest needs them too), and whether the seats and the probation seats are all taken.
pub fn contest_flags(chain:&crate::chain::Chain,v:&dyn View,registered:bool)->(bool,bool,bool){
    let a=apply::active_list(v).len() as u64;let taken=v.slot(&apply::program_address(0),&U256::from(8));
    let seats_full=a>=u64::from(chain.spec.cp().M_max);let pool_full=chain.spec.probation().is_some_and(|p|taken>=U256::from(p.seats));
    (chain.spec.programs()==apply::ProgramSet::V3&&!registered&&seats_full,seats_full,pool_full)
}
/// V3 (the v3 program set): why the registry's topUp() would refuse this record (BadState), in the wallet's words; None
/// for an active probation record (the deadline is checked by the caller).
pub fn v3_top_up_refusal(record:Option<&apply::MinerRecord>,full:U256)->Option<String>{
    let Some(r)=record.filter(|r|!r.tomb&&matches!(r.bond_state,1|4)) else{return Some("Top-up needs a registry record on probation. Register first (a probation bond can start at 0 PCOL).".into())};
    if r.bond_state==1{return Some(format!("The bond is already held in full ({} PCOL): there is nothing to top up.",coins(full)))}
    if r.status!=1{return Some("This record has exited (by its own exit or a contest), and the registry accepts a top-up only from an active probation record. Its accrued bond is refunded when it is removed; reactivate it first to keep filling it.".into())}
    None
}
/// The v3 programs' reactivation clock of `id` (V3-09), None on the spec-1 program sets (no clock).
pub fn reactivation_clock(chain:&crate::chain::Chain,v:&dyn View,id:Address)->Option<ReactClock>{
    (chain.spec.programs()==apply::ProgramSet::V3).then(||{let cp=chain.spec.cp();ReactClock{last:react_clock(v,id),c_react:u64::from(cp.C_react),u:u64::from(cp.U)}})
}
/// V3 (the v3 program set): why a reactivation would revert with Capacity now, so the wallet waits instead of spending a
/// fee: the active set is full (M_max), or a pool-contest victim still on probation finds every probation seat taken
/// (V3-14: it takes a probation seat again when it reactivates). None when nothing blocks it, and always on spec 1.
pub fn reactivation_blocked(chain:&crate::chain::Chain,v:&dyn View,record:&apply::MinerRecord)->Option<String>{
    if chain.spec.programs()!=apply::ProgramSet::V3||record.status!=2{return None}
    let m_max=u64::from(chain.spec.cp().M_max);
    if apply::active_list(v).len() as u64>=m_max{return Some(format!("All {m_max} seats are taken. Reactivation waits until a seat is free (a member leaves or is removed)."))}
    let seats=chain.spec.probation().map_or(0,|p|p.seats);
    if record.bond_state==apply::SystemPrograms::EARNIN_ACCRUING&&pool_contested(v,record.id)&&v.slot(&apply::program_address(0),&U256::from(8))>=U256::from(seats){
        return Some(format!("All {seats} probation seats are taken. A record contested out of the probation pool reactivates only into a free probation seat (or once rewards fill its bond)."))
    }
    None
}
/// D-V3-21 option A, the client part (v3 only): what a transfer review signs and states. The consensus charges a creation
/// burn only if gas_used*effective_price + burn <= max_fee*gas_limit, else the transaction halts with CreationFee and pays
/// only the gas it spent; so the signed maximum max_fee*gas_limit bounds gas and burn together. The wallet takes the gas
/// limit from the burn-aware eth_estimateGas (it covers the burn with a margin of a few creations ahead), or from the
/// user's bound at the same max_fee, and refuses before signing when the quoted gas and burn do not fit the bound.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct BurnPlan{pub gas:u64,pub suggested:u64,pub gas_used:u64,pub max_fee:U256,pub quote:crate::rpc::BurnQuote,pub user_bound:bool}
/// The gas limit a transfer signs, and the suggested one (the estimate's, at least `floor`), for a quote `q` at `max_fee`
/// and an optional user bound (PCOL wei, max_fee*gas_limit); `gas_cap` is the block gas limit.
pub fn burn_bound(floor:u64,q:&crate::rpc::BurnQuote,max_fee:U256,bound:Option<U256>,gas_cap:u64)->Result<(u64,u64),String>{
    if max_fee.is_zero(){return Err("Invalid fee".into())}
    let suggested=floor.max(q.gas_limit);
    let gas=match bound{None=>suggested,Some(b)=>u64::try_from(b/max_fee).unwrap_or(u64::MAX)};
    let need_gas=U256::from(q.gas_used).checked_mul(max_fee).and_then(|g|g.checked_add(q.total)).ok_or("Fee overflow")?;
    let need=need_gas.div_ceil(max_fee).max(U256::from(floor.max(q.gas_used)))*max_fee;
    if gas>gas_cap{return Err(format!("That maximum fee is above what one block allows at the current fee rate ({} PCOL).",coins(U256::from(gas_cap)*max_fee)))}
    if U256::from(gas)*max_fee<need_gas||gas<floor.max(q.gas_used){
        return Err(if q.total.is_zero(){format!("The maximum fee must be at least {} PCOL to cover the gas of this transfer.",coins(need))}
            else{format!("The quoted creation burn of {} PCOL plus up to {} PCOL of gas exceeds your maximum fee of {} PCOL. Raise the maximum fee to at least {} PCOL, or send to an address that already exists. Nothing was signed.",
                coins(q.total),coins(U256::from(q.gas_used)*max_fee),coins(U256::from(gas)*max_fee),coins(need))})
    }
    Ok((gas,suggested))
}
/// The burn-aware transfer plan of `from` sending `value` to `to` at the wallet's fee fields (v3 only: the caller checks
/// that the next block charges a creation burn). `bound`: the user's maximum fee in PCOL.
pub fn transfer_burn_plan(n:&Node,from:Address,to:Address,value:U256,max_fee:U256,priority:U256,bound:Option<&str>)->Result<BurnPlan,String>{
    let bound=bound.map(amount).transpose()?;
    let call=json!({"from":format!("{from:#x}"),"to":format!("{to:#x}"),"value":format!("{value:#x}"),"maxFeePerGas":format!("{max_fee:#x}"),"maxPriorityFeePerGas":format!("{priority:#x}")});
    let quote=crate::rpc::burn_quote(n,&call).map_err(|e|if e.contains("insufficient funds"){"Insufficient available balance for the amount and maximum fee".to_owned()}else{e})?;
    let (gas,suggested)=burn_bound(21_000,&quote,max_fee,bound,n.chain.spec.cp().GAS_LIMIT)?;
    Ok(BurnPlan{gas,suggested,gas_used:quote.gas_used,max_fee,quote,user_bound:bound.is_some()})
}
impl BurnPlan{
    /// The review fields: the quoted burn apart from the gas fee and the amount, and the signed maximum that bounds both.
    pub fn describe(&self)->Value{
        let q=&self.quote;let bound=U256::from(self.gas)*self.max_fee;let suggested=U256::from(self.suggested)*self.max_fee;
        let mut note=vec![if q.total.is_zero(){"This transfer creates no new account, so no creation burn is due at the current occupancy.".to_owned()}
            else{"The creation burn pays for the new account this transfer creates. It is burned, not paid to anyone, and it is quoted at the network's current occupancy: transactions included first can raise it.".to_owned()},
            "The maximum fee you sign bounds the gas and the burn together. If they would exceed it, the transfer halts and only the gas it used is charged.".to_owned()];
        if self.user_bound&&bound<suggested{note.push(format!("Your maximum fee is below the suggested {} PCOL: if a few accounts are created first, this transfer may halt (only its gas is charged).",coins(suggested)));}
        json!({"creationBurn":coins(q.total),"creationBurnParts":{"accounts":coins(q.accounts),"slots":coins(q.slots),"code":coins(q.code)},
            "gasFee":coins(U256::from(self.gas_used)*self.max_fee),"gasLimit":self.gas,"maxFeePerGas":self.max_fee.to_string(),
            "feeBound":coins(bound),"suggestedFeeBound":coins(suggested),"feeBoundSet":self.user_bound,"burnNote":note})
    }
}
/// V3-10/V3-14: the contest candidates `me` may name now (read-only): every active member the registry's contestStatus
/// view accepts for a seat contest with the full bond, and, when every probation seat is taken, every probationer it
/// accepts for a pool contest with a probation bond. Each with its measured count and the threshold.
pub fn contest_candidates(chain:&crate::chain::Chain,me:Address)->Result<Value,String>{
    if chain.spec.programs()!=apply::ProgramSet::V3{return Err("Contests exist only on the v3 network".into())}
    let head=chain.head();let state=chain.view_of(&head.hash).ok_or("The head state is not available")?;
    let (_,seats_full,pool_full)=contest_flags(chain,state.v(),false);
    let (mut seat,mut pool)=(Vec::new(),Vec::new());
    if seats_full{
        for victim in apply::active_list(state.v()){
            if victim==me{continue}
            for probation in [false,true]{
                if probation&&!pool_full{continue}
                let v=contest_view_on(state.v(),chain.spec.profile(),head.height+1,head.ts+1,me,victim,probation)?;
                if v.code==0{(if probation{&mut pool}else{&mut seat}).push(json!({"address":format!("{victim:#x}"),"count":v.count,"threshold":v.threshold.to_string()}));}
            }
        }
    }
    let why=if !seats_full{Some(format!("Contests open only when all {} seats are taken. Register normally instead.",chain.spec.cp().M_max))}
        else if seat.is_empty()&&pool.is_empty(){Some("No member can be contested now: every member credited enough shares in the last full epoch, or is still in its grace period.".to_owned())}else{None};
    Ok(json!({"type":"contestCandidates","seat":seat,"pool":pool,"seatsFull":seats_full,"probationSeatsFull":pool_full,"message":why,
        "note":"Checked against the current head. Review a contest to see the registry's verdict for your bond."}))
}
/// Registry calls of a rejoin, sent by the member itself to `apply::program_address(0)` with value 0.
pub fn exit_calldata()->Bytes{selector("deregister()")}
pub fn reactivate_calldata()->Bytes{selector("reactivate()")}
#[derive(Debug,PartialEq,Eq)]
enum Action { Wait, Stop(String), Done, Reactivate }
/// One step of a confirmed rejoin. `exit`/`last` are the local states (tx_state) of the exit and of
/// the latest reactivation; `head` is the local head height.
fn decide(step:RejoinStep,plan:&Rejoin,head:u64,exit:Option<&str>,last:Option<&str>,has_record:bool)->Action{
    match step{
        // Active while the exit is still unresolved (the empty-set fallback lists every live member):
        // finishing now would leave the exit to land with no reactivation behind it.
        RejoinStep::Active=>match plan.exit{
            Some((_,since)) if matches!(exit,Some("Pending"|"Unconfirmed"))&&head<=since.saturating_add(EXIT_WAIT_BLOCKS)=>Action::Wait,
            _=>Action::Done,
        },
        RejoinStep::Exit=>match plan.exit{
            None=>Action::Stop("The account has not exited yet. Review Rejoin mining again.".into()),
            Some(_) if exit==Some("Reverted")=>Action::Stop("The exit transaction reverted. Nothing changed; review Rejoin mining again.".into()),
            Some((_,since)) if head>since.saturating_add(EXIT_WAIT_BLOCKS)=>Action::Stop("The exit transaction was not included. Nothing changed; review Rejoin mining again.".into()),
            Some(_)=>Action::Wait,
        },
        RejoinStep::Wait{..}=>Action::Wait,
        RejoinStep::Reactivate{closes}=>{
            // An included reactivation shows as Active from the state; one in the pool is left to land.
            if matches!(last,Some("Pending"|"Included"))||plan.tried_at==Some(head){Action::Wait}
            else if plan.attempts>=REACTIVATE_ATTEMPTS{Action::Stop(format!("Reactivation did not succeed after {REACTIVATE_ATTEMPTS} attempts. The bond is refunded at block {closes}; register again after that."))}
            else{Action::Reactivate}
        }
        RejoinStep::Missed=>Action::Stop("The reactivation window passed. The bond is refunded when the record is removed; register again after that.".into()),
        RejoinStep::Cooldown{exit_from}=>Action::Stop(format!("This identity reactivated recently; review Rejoin mining again from block {exit_from}.")),
        RejoinStep::Unavailable=>Action::Stop(if has_record{"The registry record can no longer be reactivated."}else{"The registry record was removed and its bond refunded. Register again to mine."}.into()),
    }
}
/// A transaction's local state: in the pool, or its canonical receipt (Included/Reverted at a height).
fn tx_state(n:&Node,hash:&B256)->(&'static str,Option<u64>){
    let state=if n.pool.txs.iter().any(|(h,_)|h==hash){"Pending"}else{"Unconfirmed"};
    if let Ok(locations)=n.history.tx_locations(hash){
        for (block,index) in locations{
            // C-1 S5: the canonical block, held in memory or below the window (canonical index).
            if let Ok(Some(height))=n.canon_height(&block){
                if let Ok(Some(full))=n.view_by_hash(&block,crate::node::Want{receipts:true}){if let Some(raw)=full.receipts.get(index){
                    // EIP-2718 receipts: the first RLP field is the execution status.
                    let bytes=if raw.first().is_some_and(|b|*b<0x80){&raw[1..]}else{raw.as_slice()};
                    let success=crate::rlp::decode(bytes).ok().is_some_and(|r|r.list().and_then(|l|l.first()).and_then(|v|v.bytes())==Some(&[1]));
                    return (if success{"Included"}else{"Reverted"},Some(height));
                }}break;
            }
        }
    }
    (state,None)
}

/// Human coin amounts are decimal strings, never IEEE754/JSON numbers.
pub fn amount(text:&str)->Result<U256,String>{
    if text.is_empty()||text.len()>80{return Err("Enter a positive amount with up to 18 decimal places".into())}
    let mut parts=text.split('.');let whole=parts.next().unwrap();let fraction=parts.next().unwrap_or("");
    if parts.next().is_some()||whole.is_empty()||!whole.bytes().all(|c|c.is_ascii_digit())||!fraction.bytes().all(|c|c.is_ascii_digit())||fraction.len()>18{return Err("Use digits and at most 18 decimal places".into())}
    let value=format!("{whole}{fraction}{}","0".repeat(18-fraction.len())).parse::<U256>().map_err(|_|"Amount is too large")?;
    if value.is_zero(){return Err("Amount must be greater than zero".into())}Ok(value)
}
pub fn coins(wei:U256)->String{
    let s=format!("{:0>19}",wei.to_string());let at=s.len()-18;let frac=s[at..].trim_end_matches('0');
    if frac.is_empty(){s[..at].to_owned()}else{format!("{}.{}",&s[..at],frac)}
}
impl<'a> Wallet<'a>{
    /// The key comes first: `Home::key` finishes an interrupted restore, which moves another account's activity aside.
    pub fn new(home:&'a Home)->Result<Self,String>{
        let key=home.key()?;let path=home.path.join(crate::launcher::ACTIVITY_FILE);
        let activity=if path.exists(){let bytes=crate::launcher::read_bounded(&path)?;let a:VecDeque<Value>=serde_json::from_slice(&bytes).map_err(|_|"Wallet activity is damaged; preserve it before recovery")?;if a.len()>100{return Err("Wallet activity exceeds limit".into())}a}else{VecDeque::new()};
        Ok(Self{home,key,quote:None,rejoin:None,activity,stopped:false})
    }
    /// The head's registry record, whether the account is in A_{head+1} (None if unknown) and the step.
    fn rejoin_view(&self,n:&Node)->(Option<apply::MinerRecord>,Option<bool>,RejoinStep){
        let addr=self.key.address();let head=n.chain.head();
        let state=n.chain.view_of(&head.hash);
        let record=state.as_ref().and_then(|s|apply::miner_record(s.v(),addr));
        let member=n.chain.membership(head,head.height+1).ok().map(|m|m.ids().contains(&types::raw_addr(addr)));
        let clock=state.as_ref().and_then(|s|reactivation_clock(&n.chain,s.v(),addr));
        let step=match member{Some(m)=>rejoin_step_with(record.as_ref(),head.height+1,m,u64::from(n.chain.spec.cp().W_act),n.chain.spec.probation().map(|p|p.blocks),clock),None=>RejoinStep::Unavailable};
        (record,member,step)
    }
    pub fn status(&self,n:&Node)->Value{
        let addr=self.key.address();let head=n.chain.head();let state=n.chain.view_of(&head.hash);
        let account=state.as_ref().and_then(|s|s.v().account(&addr));
        let balance=account.map(|a|a.info.balance).unwrap_or_default();
        let (record,member,step)=self.rejoin_view(n);
        // A HELD or (earn-in) ACCRUING active record mines: probation keeps a nonce range.
        let eligible=record.as_ref().is_some_and(|r|r.status==1&&!r.tomb&&matches!(r.bond_state,1|4));
        let earn=n.chain.spec.earnin_rules();let target=crate::chain::from_fixed_lp4(&n.chain.spec.cp().B_reg);
        let probation=record.as_ref().filter(|r|r.status==1&&!r.tomb&&r.bond_state==apply::SystemPrograms::EARNIN_ACCRUING);
        let mut activity=self.activity.clone();
        for item in &mut activity{
            let parsed=item["hash"].as_str().and_then(|s|s.parse::<B256>().ok());
            if let Some(hash)=parsed{
                let (state,height)=tx_state(n,&hash);item["state"]=json!(state);
                if let Some(h)=height{item["height"]=json!(h);item["confirmations"]=json!(head.height-h+1);}
            }
        }
        let enabled=n.cfg.miner.is_some();
        let mining_state=if let Some(plan)=&self.rejoin{
            match step{
                RejoinStep::Exit=>"Rejoining — waiting for the exit transaction".to_owned(),
                RejoinStep::Wait{opens,..}=>format!("Rejoining — reactivation opens at block {opens}"),
                RejoinStep::Reactivate{closes}=>format!("Rejoining — reactivation attempt {} of {REACTIVATE_ATTEMPTS}, before block {closes}",plan.attempts.max(1)),
                _=>"Rejoining — updating".to_owned(),
            }
        }else{match (enabled,step){
            (true,RejoinStep::Active)=>"Enabled — in the active set, mining assigned work".to_owned(),
            (false,RejoinStep::Active)=>"Off — in the active set; enable mining to keep the place".to_owned(),
            (_,RejoinStep::Exit)=>"Inactive — no recent shares, so no assigned work. Use Rejoin mining.".to_owned(),
            (_,RejoinStep::Wait{opens,closes})=>format!("Exited — reactivate between blocks {opens} and {} with Rejoin mining",closes-1),
            (_,RejoinStep::Reactivate{closes})=>format!("Exited — reactivate before block {closes} with Rejoin mining"),
            (_,RejoinStep::Missed)=>"Exited — the bond is being refunded; register again to mine".to_owned(),
            // V3-09 (v3 only): one reactivation per C_react blocks.
            (_,RejoinStep::Cooldown{exit_from})=>format!("Inactive — no recent shares. This identity reactivated recently (one reactivation per {} blocks): Rejoin mining opens at block {exit_from}",n.chain.spec.cp().C_react),
            (true,_)=>"Enabled — awaiting eligible membership".to_owned(),
            (false,_)=>"Off — wallet keeps validating".to_owned(),
        }};
        let mut rejoin=step_json(step);
        rejoin["inProgress"]=json!(self.rejoin.is_some());rejoin["attempts"]=json!(self.rejoin.as_ref().map(|p|p.attempts).unwrap_or(0));
        let registry=match &record{None=>"none",Some(r) if r.tomb=>"removed",Some(r) if !matches!(r.bond_state,1|4)=>"closed",
            Some(r) if r.status==1&&r.bond_state==apply::SystemPrograms::EARNIN_ACCRUING=>"probation",Some(r) if r.status==1=>"member",Some(r) if r.status==2=>"exited",Some(_)=>"closed"};
        let ends=probation.zip(n.chain.spec.probation()).map(|(r,p)|r.registered_at+p.blocks);
        let mining_state=match (probation,ends){
            (Some(r),Some(end)) if self.rejoin.is_none()=>format!("{mining_state} · Probation: bond {} of {} PCOL, filled from your rewards (by block {end})",coins(r.bond),coins(target)),
            _=>mining_state,
        };
        let (contest_open,seats_full,pool_full)=state.as_ref().map(|s|contest_flags(&n.chain,s.v(),record.is_some())).unwrap_or((false,false,false));
        // V3 (the v3 program set): the pool-contest victim, the reactivation clock and what blocks a reactivation now.
        let v3=n.chain.spec.programs()==apply::ProgramSet::V3;
        let contested=v3&&record.as_ref().is_some_and(|r|r.status==2&&!r.tomb)&&state.as_ref().is_some_and(|s|pool_contested(s.v(),addr));
        let blocked=record.as_ref().filter(|_|matches!(step,RejoinStep::Wait{..}|RejoinStep::Reactivate{..})).and_then(|r|state.as_ref().and_then(|s|reactivation_blocked(&n.chain,s.v(),r)));
        let registry=if contested{"contested"}else{registry};
        let mining_state=match (&record,contested,&blocked){
            (Some(r),true,_) if self.rejoin.is_none()=>format!("{mining_state} · Contested out of the probation pool at block {}: your accrued bond ({} of {} PCOL) is still filled from rewards not yet settled, and it stays accountable for evidence until block {}, when it is refunded",
                r.exit_height,coins(r.bond),coins(target),r.due_height),
            _=>mining_state,
        };
        let mining_state=match &blocked{Some(why)=>format!("{mining_state} · {why}"),None=>mining_state};
        let diag=n.incompatible_json();
        let mut status=json!({"type":"status","address":format!("{addr:#x}"),"balance":coins(balance),"balanceWei":balance.to_string(),
            "chainId":n.chain.spec.chain_id(),"genesisHash":types::hex(&n.chain.spec.genesis_hash()),"height":head.height,"peers":n.peer_count(),"headTime":head.ts,
            "miningEnabled":enabled,"miningEligible":eligible,"registered":record.is_some(),
            "miningActive":member,"registryStatus":registry,"rejoin":rejoin,
            "rejoinAvailable":self.rejoin.is_none()&&matches!(step,RejoinStep::Exit|RejoinStep::Wait{..}|RejoinStep::Reactivate{..}),
            "miningState":mining_state,
            "blocksMined":n.stats.mined,"bond":coins(target),
            "earnIn":earn,"bondHeld":record.as_ref().map(|r|coins(r.bond)),"bondTarget":coins(target),"probationEndsAt":ends,
            "topUpAvailable":earn&&probation.is_some()&&ends.is_some_and(|e|head.height+1<e),
            // V3-10/V3-14: whether a newcomer would need quote_contest (the v3 program set only).
            "contestAvailable":contest_open,"seatsFull":seats_full,"probationSeatsFull":pool_full,
            "lifetime":n.status_json()["lifetime"],"activity":activity,"currency":"PCOL","testCurrency":true,
            // Why peers were refused (VA-SPEC §2.5, attack A2-6), so the wallet can tell a lagging node what to do.
            "incompatiblePeers":diag["incompatiblePeers"],"legacyHellos":diag["legacyHellos"],"lastIncompatibleReason":diag["lastIncompatibleReason"]});
        if v3{
            let clock=state.as_ref().and_then(|s|reactivation_clock(&n.chain,s.v(),addr));
            status["poolContested"]=json!(contested);
            // The bond still to accrue on probation (ACCRUING, active or exited): filled from rewards or by a top-up.
            status["bondRemaining"]=json!(record.as_ref().filter(|r|!r.tomb&&r.bond_state==apply::SystemPrograms::EARNIN_ACCRUING).map(|r|coins(target.saturating_sub(r.bond))));
            status["reactivation"]=json!(clock.map(|c|json!({"lastAt":(c.last!=0).then_some(c.last),"cooldownBlocks":c.c_react,"allowedFrom":(c.last!=0).then_some(c.allows_from())})));
            status["reactivationBlocked"]=json!(blocked);
        }
        // D-V3-21: the next block charges a creation burn; a transfer review shows it and takes a maximum fee.
        if n.chain.spec.v3_active(head.height.saturating_add(1)){status["creationBurn"]=json!(true);}
        crate::network_display::annotate(status)
    }
    pub fn handle(&mut self,n:&mut Node,raw:&str)->Value{
        let result=(||{
            if raw.len()>MAX_COMMAND{return Err("Command exceeds 4 KiB".into())}
            let command:Command=serde_json::from_str(raw).map_err(|_|"Invalid wallet command")?;
            self.execute(n,command)
        })();
        result.unwrap_or_else(|e:String|json!({"type":"error","message":e}))
    }
    fn execute(&mut self,n:&mut Node,command:Command)->Result<Value,String>{
        match command{
            Command::Status=>Ok(self.status(n)),
            Command::Stop=>{self.stopped=true;Ok(json!({"type":"stopping"}))},
            Command::Cancel=>{self.quote=None;Ok(json!({"type":"cancelled"}))},
            Command::Mining{enabled}=>{
                if enabled&&!self.status(n)["miningEligible"].as_bool().unwrap_or(false){return Err("This account is not eligible to mine. Register and wait for inclusion and eligibility first.".into())}
                n.set_mining_key(if enabled{Some(self.home.key()?)}else{None});let mut v=self.status(n);v["type"]=json!("mining");Ok(v)
            },
            Command::ContestCandidates{}=>contest_candidates(&n.chain,self.key.address()),
            Command::QuoteSend{to,amount:a,fee_bound}=>{
                if to.len()!=42||!to.starts_with("0x"){return Err("Enter the full 0x address (42 characters)".into())}
                let to:Address=to.parse().map_err(|_|"Invalid recipient address")?;
                let numeric=U256::from_be_slice(to.as_slice());
                if to.is_zero()||numeric<=U256::from(0xffffu64)||(U256::from(0xc0c001u64)..=U256::from(0xc0c0ffu64)).contains(&numeric){return Err("Cannot send to zero, precompile or reserved system addresses".into())}
                let pending=n.pool_view();
                if pending.account(&to).is_some_and(|a|a.info.code_hash!=revm::primitives::KECCAK_EMPTY&&a.info.code_hash!=revm::primitives::B256::ZERO){return Err("This send screen supports account transfers only".into())}
                let value=amount(&a)?;
                // D-V3-21 option A: where the next block charges a creation burn, the gas limit comes from the burn-aware
                // estimate (or the user's bound) and the review shows the burn apart from the gas fee.
                if !n.chain.spec.v3_active(n.chain.head().height.saturating_add(1)){
                    if fee_bound.is_some(){return Err("A maximum fee applies only on networks with a creation burn.".into())}
                    return self.quote(n,"transfer",to,value,Bytes::new(),21_000)
                }
                let fee=self.fee(n)?;
                let plan=transfer_burn_plan(n,self.key.address(),to,value,fee.max_fee,fee.priority,fee_bound.as_deref())?;
                let mut q=self.quote(n,"transfer",to,value,Bytes::new(),plan.gas)?;
                if let Value::Object(fields)=plan.describe(){for (k,v) in fields{q[k]=v;}}
                Ok(q)
            },
            Command::QuoteRegister{bond}=>{
                if self.status(n)["registered"].as_bool().unwrap_or(false){return Err("This identity already has a registry record".into())}
                let earn=n.chain.spec.earnin_rules();let full=crate::chain::from_fixed_lp4(&n.chain.spec.cp().B_reg);
                let value=match bond.as_deref(){
                    None=>if earn{U256::ZERO}else{full},
                    Some("0")=>U256::ZERO,
                    Some(text)=>amount(text)?,
                };
                if value>full{return Err(format!("The bond is at most {} PCOL",coins(full)))}
                if !earn&&value!=full{return Err(format!("This network takes the full {} PCOL bond",coins(full)))}
                if earn&&value<full{
                    let head=n.chain.head();
                    let taken=n.chain.view_of(&head.hash).map(|s|s.v().slot(&apply::program_address(0),&U256::from(8))).unwrap_or_default();
                    let seats=n.chain.spec.probation().map_or(0,|p|p.seats);
                    if taken>=U256::from(seats){return Err(format!("All {seats} probation seats are taken. Register with the full {} PCOL bond, or try again later.",coins(full)))}
                }
                // PAY-02: the reward address (this identity) must have nonce > 0 in the state before the
                // registration, or register(address) reverts with RewardNotDurable and only costs the fee.
                // The earn-in Registry accepts a self reward without that probe (R9).
                if !earn&&!registration_reward_durable(n.chain.view_of(&n.chain.head().hash).and_then(|s|s.v().account(&self.key.address())).map(|a|a.info.nonce)){
                    return Err("This identity has not sent a transaction yet, and the registry pays rewards only to an account that has. Send one transfer first (a small amount, even to your own address), wait for it to be included, then register.".into())
                }
                let mut data=keccak256("register(address)")[..4].to_vec();data.extend_from_slice(self.key.address().into_word().as_slice());
                let mut q=self.quote(n,"registration",apply::program_address(0),value,data.into(),if earn{EARNIN_REGISTER_GAS}else{REGISTRY_GAS})?;
                q["bondTarget"]=json!(coins(full));q["probation"]=json!(value<full);
                Ok(q)
            },
            Command::QuoteTopUp{amount:requested}=>{
                if !n.chain.spec.earnin_rules(){return Err("Bond top-up exists only on the earn-in network".into())}
                let (record,_,_)=self.rejoin_view(n);let full=crate::chain::from_fixed_lp4(&n.chain.spec.cp().B_reg);
                // V3: the registry's topUp takes value only from a live, active probation (ACCRUING) record before its
                // deadline; say which of those the record is missing.
                if n.chain.spec.programs()==apply::ProgramSet::V3{if let Some(why)=v3_top_up_refusal(record.as_ref(),full){return Err(why)}}
                let r=record.filter(|r|r.status==1&&!r.tomb&&r.bond_state==apply::SystemPrograms::EARNIN_ACCRUING).ok_or("Top-up needs an active probation record")?;
                let t_prob=n.chain.spec.probation().ok_or("Bond top-up exists only on the earn-in network")?.blocks;
                if n.chain.head().height+1>=r.registered_at+t_prob{return Err("The probation period has ended".into())}
                let remaining=full-r.bond;
                let value=match requested.as_deref(){None=>remaining,Some(text)=>amount(text)?};
                if value>remaining{return Err(format!("At most {} PCOL remains to fill the bond",coins(remaining)))}
                let mut q=self.quote(n,"topup",apply::program_address(0),value,keccak256("topUp()")[..4].to_vec().into(),REGISTRY_GAS)?;
                q["bondTarget"]=json!(coins(full));q["bondHeld"]=json!(coins(r.bond));
                if n.chain.spec.programs()==apply::ProgramSet::V3{
                    // V3-08: topUp is a maintenance operation, at most one per identity per block.
                    q["bondRemaining"]=json!(coins(remaining));q["probationEndsAt"]=json!(r.registered_at+t_prob);
                    q["note"]=json!(format!("The bond becomes HELD once it reaches {} PCOL; the rest of this top-up returns to you if rewards fill part of it first. One bond top-up or reward-address change per block.",coins(full)));
                }
                Ok(q)
            },
            Command::QuoteRejoin=>self.quote_rejoin(n),
            Command::QuoteContest{victim,bond}=>self.quote_contest(n,&victim,bond.as_deref()),
            Command::Confirm{quote_id}=>{
                let q=self.quote.take().ok_or("Review a transaction first")?;
                if q.id!=quote_id||Instant::now()>q.deadline{return Err("Review expired or changed. Review the transaction again.".into())}
                if q.rejoin.is_some()&&self.rejoin.is_some(){return Err("Rejoining is already in progress".into())}
                let submitted=match &q.tx{
                    Some(tx)=>{
                        if n.pool_view().account(&self.key.address()).map(|a|a.info.nonce).unwrap_or(0)!=tx.nonce{return Err("Account nonce changed. Review again.".into())}
                        let hash=n.submit_tx(tx.sign_1559(&self.key),true)?;
                        Some((hash,self.record_activity(hash,q.kind,tx)))
                    }
                    None=>None,
                };
                if let Some((max_fee,priority))=q.rejoin{
                    self.rejoin=Some(Rejoin{exit:submitted.map(|(h,_)|(h,n.chain.head().height)),max_fee,priority,attempts:0,last:None,tried_at:None});
                }
                Ok(match submitted{
                    Some((hash,saved))=>json!({"type":"submitted","hash":format!("{hash:#x}"),"historySaved":saved}),
                    None=>json!({"type":"rejoin","state":"Scheduled","message":"Reactivation is scheduled. Keep the wallet open until the account is back in the active set."}),
                })
            }
        }
    }
    /// Public history only. Failure does not pretend that an already relayed transaction was unsent.
    fn record_activity(&mut self,hash:B256,kind:&str,tx:&Tx)->bool{
        self.activity.push_front(json!({"hash":format!("{hash:#x}"),"kind":kind,"to":tx.to.map(|a|format!("{a:#x}")),"amount":coins(tx.value),"state":"Pending","time":crate::node::now_s()}));
        self.activity.truncate(100);
        let file=self.home.path.join("wallet-activity.json");
        serde_json::to_vec(&self.activity).ok().is_some_and(|bytes|save_activity(&file,&bytes).is_ok())
    }
    fn quote_contest(&mut self,n:&Node,victim:&str,bond:Option<&str>)->Result<Value,String>{
        let plan=contest_plan(&n.chain,self.key.address(),victim,bond)?;
        let mut q=self.quote(n,"contest",apply::program_address(0),plan.value,contest_calldata(self.key.address(),plan.victim),CONTEST_GAS)?;
        if let Value::Object(fields)=plan.describe(){for (k,v) in fields{q[k]=v;}}
        Ok(q)
    }
    fn quote_rejoin(&mut self,n:&Node)->Result<Value,String>{
        if self.rejoin.is_some(){return Err("Rejoining is already in progress".into())}
        let cp=n.chain.spec.cp();let (w_act,u)=(u64::from(cp.W_act),u64::from(cp.U));
        if w_act>=u{return Err("This network's registry leaves no reactivation window".into())}
        let (_,_,step)=self.rejoin_view(n);
        let (kind,tx_data)=match step{
            RejoinStep::Exit=>{
                let head=n.chain.head();
                let active=n.chain.view_of(&head.hash).map(|s|apply::active_list(s.v()).len()).unwrap_or(0);
                if active as u64<=u64::from(cp.M_min){return Err("The registry needs more active members before this account can exit and reactivate".into())}
                ("rejoin",Some(exit_calldata()))
            }
            RejoinStep::Wait{..}|RejoinStep::Reactivate{..}=>("reactivation",None),
            RejoinStep::Active=>return Err("This account is already in the active mining set".into()),
            RejoinStep::Missed=>return Err("The reactivation window has passed. The bond is refunded when the record is removed; register again after that.".into()),
            RejoinStep::Unavailable=>return Err("Rejoining needs a registry record with a held bond. Register first.".into()),
            RejoinStep::Cooldown{exit_from}=>return Err(format!("This identity reactivated recently, and the registry allows one reactivation per {} blocks: an exit now could not be reactivated before the bond is refunded. Rejoin mining from block {exit_from}.",cp.C_react)),
        };
        // V3: a reactivation that would revert with Capacity now is not quoted (its fee would be spent for nothing).
        let (record,_,_)=self.rejoin_view(n);
        if tx_data.is_none(){if let Some(why)=record.as_ref().and_then(|r|n.chain.view_of(&n.chain.head().hash).and_then(|st|reactivation_blocked(&n.chain,st.v(),r))){return Err(why)}}
        let fee=self.fee(n)?;
        let count=u64::from(REACTIVATE_ATTEMPTS)+u64::from(tx_data.is_some());
        let cost=fee.max_fee.checked_mul(U256::from(REGISTRY_GAS*count)).ok_or("Fee overflow")?;
        if cost>fee.balance{return Err("Insufficient available balance for the maximum fees".into())}
        let to=apply::program_address(0);
        let tx=tx_data.map(|data|Tx{chain_id:n.chain.spec.chain_id(),nonce:fee.nonce,max_priority:fee.priority,max_fee:fee.max_fee,gas:REGISTRY_GAS,to:Some(to),value:U256::ZERO,data});
        let mut steps=Vec::new();
        if tx.is_some(){steps.push("Sign now: leave the registry (deregister). The bond stays locked.".to_owned());
            steps.push(format!("After {w_act} blocks, the wallet signs reactivate() and has {} blocks for it to be included.",u-w_act));}
        else{steps.push(format!("The wallet signs reactivate() when the window opens, up to {REACTIVATE_ATTEMPTS} times."));}
        if n.chain.spec.programs()==apply::ProgramSet::V3{
            // V3-09: the reactivation starts the record's C_react clock; V3-10: an exit frees the seat for others.
            steps.push(format!("A reactivation starts a {}-block cooldown: this identity cannot reactivate again before it ends.",cp.C_react));
            if tx.is_some(){steps.push("While this account is out, a newcomer may take the free seat; if every seat is taken, the wallet waits for one to free up.".to_owned());}
        }
        steps.push("Mining turns on once this account is back in the active set. Keep the wallet open until then.".to_owned());
        steps.push(format!("If the window passes, the registry refunds the bond after {u} blocks and you register again."));
        let id=quote_id()?;
        let mut result=json!({"type":"quote","quoteId":id,"kind":kind,"to":format!("{to:#x}"),"from":format!("{:#x}",self.key.address()),"amount":"0",
            "maximumFee":coins(cost),"maximumTotal":coins(cost),"transactions":count,"steps":steps,"chainId":n.chain.spec.chain_id(),"expiresInSeconds":60});
        result["rejoin"]=step_json(step);
        self.quote=Some(Quote{id,deadline:Instant::now()+Duration::from_secs(60),kind,tx,rejoin:Some((fee.max_fee,fee.priority))});Ok(result)
    }
    /// Advances a confirmed rejoin from the head. Called by the event loop; returns an event to show.
    pub fn advance(&mut self,n:&mut Node)->Option<Value>{
        let mut plan=self.rejoin.take()?;
        let (record,_,step)=self.rejoin_view(n);let head=n.chain.head().height;
        let exit=plan.exit.map(|(h,_)|tx_state(n,&h).0);let last=plan.last.map(|h|tx_state(n,&h).0);
        let stop=|message:String|Some(json!({"type":"rejoin","state":"Stopped","message":message}));
        // V3: no reactivation is signed while it would revert with Capacity (a full set or, for a pool-contest victim, a
        // full probation pool); the window may still close, which stops the plan as Missed.
        let blocked=record.as_ref().and_then(|r|n.chain.view_of(&n.chain.head().hash).and_then(|st|reactivation_blocked(&n.chain,st.v(),r)));
        let action=match decide(step,&plan,head,exit,last,record.is_some()){Action::Reactivate if blocked.is_some()=>Action::Wait,a=>a};
        match action{
            Action::Wait=>{self.rejoin=Some(plan);None}
            Action::Stop(message)=>stop(message),
            Action::Done=>match self.home.key(){
                Ok(key)=>{n.set_mining_key(Some(key));Some(json!({"type":"rejoin","state":"Rejoined","message":"Back in the active mining set. Mining is on; keep it running to stay active."}))}
                Err(e)=>stop(format!("Back in the active set, but mining could not start: {e}")),
            },
            Action::Reactivate=>{
                plan.tried_at=Some(head);plan.attempts+=1;
                let nonce=n.pool_view().account(&self.key.address()).map(|a|a.info.nonce).unwrap_or(0);
                let tx=Tx{chain_id:n.chain.spec.chain_id(),nonce,max_priority:plan.priority,max_fee:plan.max_fee,gas:REGISTRY_GAS,to:Some(apply::program_address(0)),value:U256::ZERO,data:reactivate_calldata()};
                let event=match n.submit_tx(tx.sign_1559(&self.key),true){
                    Ok(hash)=>{plan.last=Some(hash);self.record_activity(hash,"reactivation",&tx);
                        json!({"type":"rejoin","state":"Reactivating","hash":format!("{hash:#x}"),"attempt":plan.attempts,"message":format!("Reactivation submitted (attempt {} of {REACTIVATE_ATTEMPTS}).",plan.attempts)})}
                    Err(e)=>json!({"type":"rejoin","state":"Retrying","attempt":plan.attempts,"message":format!("Reactivation was not accepted: {e}")}),
                };
                self.rejoin=Some(plan);Some(event)
            }
        }
    }
    fn fee(&self,n:&Node)->Result<Fee,String>{
        if n.peer_count()==0{return Err("Connect to the network before sending".into())}
        let head=n.chain.head();
        if head.ts.saturating_add(180)<crate::node::now_s(){return Err("The local chain is behind. Wait for synchronization.".into())}
        let pending=n.pool_view();let account=pending.account(&self.key.address());
        let nonce=account.as_ref().map(|a|a.info.nonce).unwrap_or(0);
        let balance=account.map(|a|a.info.balance).unwrap_or_default();
        let floor=n.chain.spec.base_fee_floor(head.height.saturating_add(1));
        let base=if head.height==0{crate::chain::from_fixed_lp4(&n.chain.spec.cp().baseFee0).max(floor)}else{pocol_lp4_execution::fees::next_base_fee(head.base_fee,head.gas_used,head.header.gas_limit,floor).map_err(|_|"Invalid base fee")?};
        let priority=U256::from(1_000_000_000u64);let max_fee=base.checked_mul(U256::from(2)).and_then(|b|b.checked_add(priority)).ok_or("Fee overflow")?;
        Ok(Fee{nonce,balance,priority,max_fee})
    }
    fn quote(&mut self,n:&Node,kind:&'static str,to:Address,value:U256,data:Bytes,gas:u64)->Result<Value,String>{
        let Fee{nonce,balance,priority,max_fee:fee}=self.fee(n)?;
        let cost=fee.checked_mul(U256::from(gas)).ok_or("Fee overflow")?;
        if value.checked_add(cost).ok_or("Total overflow")?>balance{return Err("Insufficient available balance for the amount and maximum fee".into())}
        let id=quote_id()?;
        let result=json!({"type":"quote","quoteId":id,"kind":kind,"to":format!("{to:#x}"),"from":format!("{:#x}",self.key.address()),"amount":coins(value),"maximumFee":coins(cost),"maximumTotal":coins(value+cost),"chainId":n.chain.spec.chain_id(),"expiresInSeconds":60});
        self.quote=Some(Quote{id,deadline:Instant::now()+Duration::from_secs(60),kind,rejoin:None,tx:Some(Tx{chain_id:n.chain.spec.chain_id(),nonce,max_priority:priority,max_fee:fee,gas,to:Some(to),value,data})});Ok(result)
    }
}
struct Fee { nonce:u64, balance:U256, priority:U256, max_fee:U256 }
fn quote_id()->Result<String,String>{let mut id=[0u8;16];getrandom::getrandom(&mut id).map_err(|_|"Secure randomness unavailable")?;Ok(types::hex(&id))}
fn save_activity(path:&std::path::Path,bytes:&[u8])->Result<(),String>{
    let mut random=[0u8;16];getrandom::getrandom(&mut random).map_err(|_|"Secure randomness unavailable")?;
    let temp=path.with_file_name(format!("wallet-activity-{}.tmp",types::hex(&random)));
    let result=(||{
        let mut options=std::fs::OpenOptions::new();options.write(true).create_new(true);
        #[cfg(unix)]{use std::os::unix::fs::OpenOptionsExt;options.mode(0o600);}
        let mut file=options.open(&temp).map_err(|e|e.to_string())?;
        file.write_all(bytes).and_then(|_|file.sync_all()).map_err(|e|e.to_string())?;
        drop(file);std::fs::rename(&temp,path).map_err(|e|e.to_string())?;
        #[cfg(unix)]std::fs::File::open(path.parent().ok_or("Missing wallet directory")?).and_then(|d|d.sync_all()).map_err(|e|e.to_string())?;
        Ok(())
    })();
    if result.is_err(){let _=std::fs::remove_file(temp);}result
}
/// Desktop uses anonymous child-process pipes. EOF stops the engine, not an exposed TCP admin port.
pub fn stdio(home:&Home,stop:impl Fn()->bool)->Result<(),String>{
    let (tx,rx)=mpsc::sync_channel(8);
    crate::threads::spawn("wallet-input",crate::threads::IO_STACK,move||{
        let stdin=std::io::stdin();let mut input=stdin.lock();
        loop{let mut bytes=Vec::new();use std::io::Read;
            match input.by_ref().take((MAX_COMMAND+1) as u64).read_until(b'\n',&mut bytes){Ok(0)|Err(_)=>break,_=>{}}
            if bytes.len()>MAX_COMMAND{break}
            if tx.send(String::from_utf8(bytes).unwrap_or_default()).is_err(){return}
        }
    }).map_err(|e|e.to_string())?;
    let mut wallet=Wallet::new(home)?;let mut next=Instant::now();
    let emit=|v:Value|{println!("{v}");let _=std::io::stdout().flush();};
    home.run_mode(None,None,&[],false,|n|{
        match rx.try_recv(){Ok(command)=>emit(wallet.handle(n,&command)),Err(mpsc::TryRecvError::Disconnected)=>return true,Err(mpsc::TryRecvError::Empty)=>{}}
        if Instant::now()>=next{if let Some(event)=wallet.advance(n){emit(event)}emit(wallet.status(n));next=Instant::now()+Duration::from_secs(2)}
        wallet.stopped||stop()
    })
}
#[cfg(test)] mod tests{
    use super::*;
    #[test] fn a_finished_restore_never_shows_the_replaced_accounts_activity(){
        use crate::launcher::key_backup_tests::{crash_restore,OLD_ACTIVITY};
        for step in 1..=4u8{
            let (_,_,home,_t)=crash_restore(step,false);let w=Wallet::new(&home).unwrap();
            let old:VecDeque<Value>=serde_json::from_slice(OLD_ACTIVITY).unwrap();
            assert_eq!(w.activity,if step==1{old}else{VecDeque::new()},"crash before restore step {step}");
        }
    }
    #[test] fn decimal_amounts_never_use_floats(){
        for s in ["1","0.000000000000000001","123456789.012345678901234567"]{assert_eq!(coins(amount(s).unwrap()),s)}
        for s in ["", "0", "-1", "NaN", "1e18", "1.0000000000000000001", ".1", "1..2", " 1"]{assert!(amount(s).is_err(),"{s}")}
    }
    fn record(status:u8,exit:u64,due:u64)->apply::MinerRecord{
        apply::MinerRecord{id:Address::repeat_byte(7),status,tomb:false,bond_state:1,last_share:5,reward:Address::repeat_byte(7),bond:U256::from(100u64),exit_height:exit,due_height:due,burn_height:0,grace_start:0,registered_at:0}
    }
    #[test] fn rejoin_step_follows_the_registry_window(){
        // Mainnet: W_act=32, U=40. Exit at 100 → reactivate() may be included at 132..=139; the sweep refunds at 140.
        // Record states 1-3 have no probation deadline, so the earn-in T_PROB changes nothing here.
        let w=32;let t=Some(apply::Probation::EARNIN.blocks);
        assert_eq!(rejoin_step(None,200,false,w,t),RejoinStep::Unavailable);
        assert_eq!(rejoin_step(Some(&record(1,0,0)),200,true,w,t),RejoinStep::Active);
        assert_eq!(rejoin_step(Some(&record(1,0,0)),200,false,w,t),RejoinStep::Exit);
        let exited=record(2,100,140);
        assert_eq!(rejoin_step(Some(&exited),101,false,w,t),RejoinStep::Wait{opens:132,closes:140});
        assert_eq!(rejoin_step(Some(&exited),131,false,w,t),RejoinStep::Wait{opens:132,closes:140});
        assert_eq!(rejoin_step(Some(&exited),132,false,w,t),RejoinStep::Reactivate{closes:140});
        assert_eq!(rejoin_step(Some(&exited),139,false,w,t),RejoinStep::Reactivate{closes:140});
        assert_eq!(rejoin_step(Some(&exited),140,false,w,t),RejoinStep::Missed);
        // An exited record is never "active", even on the empty-set fallback that lists every live member.
        assert_eq!(rejoin_step(Some(&exited),135,true,w,t),RejoinStep::Reactivate{closes:140});
        for r in [apply::MinerRecord{tomb:true,..record(1,0,0)},apply::MinerRecord{bond_state:2,..record(2,100,140)},apply::MinerRecord{bond_state:3,..record(1,0,0)},record(3,0,0)]{
            assert_eq!(rejoin_step(Some(&r),135,false,w,t),RejoinStep::Unavailable,"{r:?}");
        }
    }
    fn plan()->Rejoin{Rejoin{exit:Some((B256::repeat_byte(1),50)),max_fee:U256::from(3u64),priority:U256::from(1u64),attempts:0,last:None,tried_at:None}}
    #[test] fn rejoin_plan_decisions(){
        let reactivate=RejoinStep::Reactivate{closes:140};
        assert_eq!(decide(RejoinStep::Active,&plan(),60,Some("Included"),None,true),Action::Done);
        assert_eq!(decide(RejoinStep::Active,&Rejoin{exit:None,..plan()},60,None,None,true),Action::Done);
        assert_eq!(decide(RejoinStep::Active,&plan(),60,Some("Pending"),None,true),Action::Wait);
        assert_eq!(decide(RejoinStep::Active,&plan(),51+EXIT_WAIT_BLOCKS,Some("Unconfirmed"),None,true),Action::Done);
        // The exit: wait while pending or unconfirmed for EXIT_WAIT_BLOCKS heads; stop if it reverted.
        assert_eq!(decide(RejoinStep::Exit,&plan(),50+EXIT_WAIT_BLOCKS,Some("Pending"),None,true),Action::Wait);
        assert!(matches!(decide(RejoinStep::Exit,&plan(),51+EXIT_WAIT_BLOCKS,Some("Pending"),None,true),Action::Stop(_)));
        assert!(matches!(decide(RejoinStep::Exit,&plan(),51,Some("Reverted"),None,true),Action::Stop(_)));
        assert!(matches!(decide(RejoinStep::Exit,&Rejoin{exit:None,..plan()},51,None,None,true),Action::Stop(_)));
        assert_eq!(decide(RejoinStep::Wait{opens:132,closes:140},&plan(),120,Some("Included"),None,true),Action::Wait);
        // Reactivation: sign when the window is open, never twice per head, never while one is in flight.
        assert_eq!(decide(reactivate,&plan(),131,Some("Included"),None,true),Action::Reactivate);
        assert_eq!(decide(reactivate,&Rejoin{tried_at:Some(131),attempts:1,..plan()},131,None,Some("Reverted"),true),Action::Wait);
        assert_eq!(decide(reactivate,&Rejoin{tried_at:Some(131),attempts:1,..plan()},133,None,Some("Pending"),true),Action::Wait);
        assert_eq!(decide(reactivate,&Rejoin{tried_at:Some(131),attempts:1,..plan()},133,None,Some("Reverted"),true),Action::Reactivate);
        assert_eq!(decide(reactivate,&Rejoin{tried_at:Some(131),attempts:1,..plan()},133,None,Some("Unconfirmed"),true),Action::Reactivate);
        assert!(matches!(decide(reactivate,&Rejoin{tried_at:Some(135),attempts:REACTIVATE_ATTEMPTS,..plan()},136,None,Some("Reverted"),true),Action::Stop(_)));
        assert!(matches!(decide(RejoinStep::Missed,&plan(),140,None,None,true),Action::Stop(_)));
        assert!(matches!(decide(RejoinStep::Unavailable,&plan(),141,None,None,false),Action::Stop(m) if m.contains("refunded")));
    }
    #[test] fn registration_needs_a_prior_transaction(){
        assert!(!registration_reward_durable(None));
        assert!(!registration_reward_durable(Some(0)));
        assert!(registration_reward_durable(Some(1)));
    }
    #[test] fn rejoin_calls_match_the_registry_selectors(){
        assert_eq!(exit_calldata().as_ref(),&keccak256("deregister()")[..4]);
        assert_eq!(reactivate_calldata().as_ref(),&keccak256("reactivate()")[..4]);
    }
    #[test] fn probation_rejoin_ends_at_the_deadline(){
        // The earn-in programs' constant T_PROB and a v3 CP T_PROB (V3-14: 30 days at 30 s) give the same windows.
        for t in [apply::Probation::EARNIN.blocks,86_400]{
            // Heights offset by T_PROB so registeredAt stays positive.
            let p=Some(t);
            let mut r=record(2,t+100,t+140);r.bond_state=apply::SystemPrograms::EARNIN_ACCRUING;r.registered_at=t;
            assert_eq!(rejoin_step(Some(&r),t+132,false,32,p),RejoinStep::Reactivate{closes:t+140},"before the deadline the usual window applies");
            r.registered_at=135;
            assert_eq!(rejoin_step(Some(&r),t+132,false,32,p),RejoinStep::Reactivate{closes:t+135},"the deadline closes the window early");
            assert_eq!(rejoin_step(Some(&r),t+135,false,32,p),RejoinStep::Missed);
            let mut active=record(1,0,0);active.bond_state=apply::SystemPrograms::EARNIN_ACCRUING;active.registered_at=200;
            assert_eq!(rejoin_step(Some(&active),t+168,false,32,p),RejoinStep::Missed,"an exit at T+168 would reopen at the deadline");
            assert_eq!(rejoin_step(Some(&active),t+167,false,32,p),RejoinStep::Exit);
            assert_eq!(rejoin_step(Some(&active),t+500,true,32,p),RejoinStep::Active);
        }
        assert_eq!(apply::Probation::EARNIN,apply::Probation{seats:8,blocks:60_480},"PoColSystemEarnIn.sol P_MAX and T_PROB");
    }
    /// V3-10/V3-14: the contest action explains each refusal code of the registry's contestCheck and calls the selector the
    /// v3 MinerRegistry dispatches.
    #[test] fn contest_checks_explain_every_registry_refusal(){
        let p=ContestParams{next:100,e_epoch:8,d:4,theta_inact:16,u:140,m_max:256,p_max:64,grace_start:90};
        let v=|code:u64|ContestView{code,count:3,threshold:U256::from(25u64)};
        assert_eq!(contest_check(&v(0),&p,false,false),Ok(false),"a seat contest");
        assert_eq!(contest_check(&v(0),&p,true,true),Ok(true),"a probation bond against a full pool: the pool contest");
        assert_eq!(contest_check(&v(0),&p,true,false),Ok(false),"a probation bond with pool room: a seat contest");
        let msg=|code:u64,probation:bool|contest_check(&v(code),&p,probation,false).unwrap_err();
        assert!(msg(1,false).contains("no active registry record"));
        assert!(msg(2,false).contains("all 256 seats")&&msg(2,true).contains("all 256 seats")&&!msg(2,true).contains("probation seats"),"a pool contest needs every seat taken too");
        assert!(msg(3,true).contains("only another probationer"));
        // Epoch 12 starts at 96 and its predecessor's late window closes at 96 + D - 1 = 99; in epoch 0 nothing is measured
        // until 8 + 3 = 11.
        let early=ContestParams{next:97,..p};assert!(contest_check(&v(4),&early,false,false).unwrap_err().contains("block 99"));
        let first=ContestParams{next:3,..p};assert!(contest_check(&v(4),&first,false,false).unwrap_err().contains("block 11"));
        let tiny=ContestParams{e_epoch:2,..p};assert!(contest_check(&v(4),&tiny,false,false).unwrap_err().contains("can ever be contested"));
        assert!(msg(5,false).contains("until block 106"));
        // graceStart 90 lies in epoch 11 (88..95): the first full epoch is 12 (96..103), judged from 104 + 3 = 107; a
        // graceStart of exactly 96, the first block of epoch 12, is allowed for it too.
        assert!(msg(6,false).contains("block 107"),"{}",msg(6,false));
        let on=ContestParams{grace_start:96,..p};assert!(contest_check(&v(6),&on,false,false).unwrap_err().contains("block 107"));
        assert!(msg(7,false).contains("3 shares")&&msg(7,false).contains("threshold of 25"));
        let d=contest_calldata(Address::repeat_byte(1),Address::repeat_byte(2));
        assert_eq!(&d[..4],&keccak256("registerContest(address,address)")[..4]);assert_eq!(d.len(),68);
        assert_eq!(&d[16..36],Address::repeat_byte(1).as_slice());assert_eq!(&d[48..68],Address::repeat_byte(2).as_slice());
    }
    fn react(last:u64,c_react:u64,u:u64)->Option<ReactClock>{Some(ReactClock{last,c_react,u})}
    /// V3-09: the v3 rejoin step reads the record's reactivation clock; without a clock (spec 1) it is the earn-in rule.
    #[test] fn rejoin_step_reads_the_v3_reactivation_clock(){
        let (w,t)=(32,Some(apply::Probation::EARNIN.blocks));
        let exited=record(2,100,140);let active=record(1,0,0);
        for (r,next,member) in [(&exited,101,false),(&exited,132,false),(&exited,140,false),(&active,200,false),(&active,200,true),(&exited,135,true)]{
            assert_eq!(rejoin_step_with(Some(r),next,member,w,t,None),rejoin_step(Some(r),next,member,w,t),"no clock: the spec-1 rule");
            assert_eq!(rejoin_step_with(Some(r),next,member,w,t,react(0,40,40)),rejoin_step(Some(r),next,member,w,t),"a record that never reactivated");
        }
        // Reactivated at 95 with C_react 40: allowed from 135, inside the [132, 140) window.
        assert_eq!(rejoin_step_with(Some(&exited),133,false,w,t,react(95,40,40)),RejoinStep::Wait{opens:135,closes:140});
        assert_eq!(rejoin_step_with(Some(&exited),135,false,w,t,react(95,40,40)),RejoinStep::Reactivate{closes:140});
        assert_eq!(rejoin_step_with(Some(&exited),140,false,w,t,react(95,40,40)),RejoinStep::Missed);
        // Reactivated at 90 with C_react 200: the clock opens at 290, past the sweep at 140, so the window is gone now.
        assert_eq!(rejoin_step(Some(&exited),101,false,w,t),RejoinStep::Wait{opens:132,closes:140},"the earn-in rule would wait");
        assert_eq!(rejoin_step_with(Some(&exited),101,false,w,t,react(90,200,40)),RejoinStep::Missed,"the v3 clock: missed at once");
        // A status-1 member outside A_next that reactivated at 100 (C_react 200, U 40): an exit is useful from 262, when
        // [max(e + 32, 300), e + 40) holds two blocks.
        assert_eq!(rejoin_step_with(Some(&active),200,false,w,t,react(100,200,40)),RejoinStep::Cooldown{exit_from:262});
        assert_eq!(rejoin_step_with(Some(&active),261,false,w,t,react(100,200,40)),RejoinStep::Cooldown{exit_from:262});
        assert_eq!(rejoin_step_with(Some(&active),262,false,w,t,react(100,200,40)),RejoinStep::Exit);
        assert_eq!(rejoin_step_with(Some(&active),200,true,w,t,react(100,200,40)),RejoinStep::Active);
        // A probation record whose clock reaches its T_PROB deadline cannot come back: missed.
        let mut p=record(1,0,0);p.bond_state=apply::SystemPrograms::EARNIN_ACCRUING;p.registered_at=0;
        assert_eq!(rejoin_step_with(Some(&p),100,false,w,Some(400),react(300,200,40)),RejoinStep::Missed,"clock 500 >= deadline 400");
        assert_eq!(rejoin_step_with(Some(&p),100,false,w,Some(400),None),RejoinStep::Exit);
        assert_eq!(step_json(RejoinStep::Cooldown{exit_from:262}),json!({"step":"cooldown","exitFrom":262}));
        // A cooldown is no plan step: a running plan stops and says when to review again.
        assert!(matches!(decide(RejoinStep::Cooldown{exit_from:262},&plan(),200,None,None,true),Action::Stop(m) if m.contains("262")));
    }
    /// The clock and the pool-contest mark are read from the record words (PoColSystemV3.sol w1 bits 160..223, w0 bit 87).
    #[test] fn record_clock_and_pool_mark_bits(){
        let mut db=revm::InMemoryDB::default();let id=Address::repeat_byte(9);let r=apply::record_slot(id);let reg=apply::program_address(0);
        assert_eq!((react_clock(&db,id),pool_contested(&db,id)),(0,false));
        let w1=(U256::from(0xdead_beefu64)<<224usize)|(U256::from(123_456_789u64)<<160usize)|U256::from_be_slice(Address::repeat_byte(7).as_slice());
        let w0=(U256::from(1u64)<<87usize)|U256::from(2u64);
        db.insert_account_storage(reg,r+U256::from(1),w1).unwrap();db.insert_account_storage(reg,r,w0).unwrap();
        assert_eq!((react_clock(&db,id),pool_contested(&db,id)),(123_456_789,true));
        assert_eq!(apply::miner_record(&db,id).unwrap().reward,Address::repeat_byte(7),"the host reads the reward address alone");
        assert_eq!(ReactClock{last:0,c_react:40,u:40}.allows_from(),0);assert_eq!(ReactClock{last:7,c_react:40,u:40}.allows_from(),47);
    }
    /// D-V3-21 option A: the signed maximum max_fee*gas_limit must cover the quoted gas and burn, or nothing is signed.
    #[test] fn the_signed_maximum_bounds_gas_and_burn(){
        let fee=U256::from(100u64);
        let q=crate::rpc::BurnQuote{gas_limit:35_000,gas_used:21_000,accounts:U256::from(1_000_000u64),slots:U256::ZERO,code:U256::ZERO,total:U256::from(1_000_000u64)};
        assert_eq!(burn_bound(21_000,&q,fee,None,8_000_000),Ok((35_000,35_000)),"the estimate's limit");
        // Gas 21000*100 plus the burn 1e6 need 31000 gas at max_fee 100.
        assert_eq!(burn_bound(21_000,&q,fee,Some(U256::from(3_100_000u64)),8_000_000),Ok((31_000,35_000)),"a lowered bound that still covers");
        assert_eq!(burn_bound(21_000,&q,fee,Some(U256::from(3_100_099u64)),8_000_000),Ok((31_000,35_000)),"max_fee*gas_limit at or below the bound");
        let low=burn_bound(21_000,&q,fee,Some(U256::from(3_099_999u64)),8_000_000).unwrap_err();
        assert!(low.contains("creation burn")&&low.contains("at least 0.0000000000031 PCOL")&&low.contains("Nothing was signed"),"{low}");
        assert_eq!(burn_bound(21_000,&q,fee,Some(U256::from(5_000_000u64)),8_000_000),Ok((50_000,35_000)),"a raised bound");
        assert!(burn_bound(21_000,&q,fee,Some(U256::from(900_000_000u64)),8_000_000).unwrap_err().contains("one block allows"));
        // No burn: the bound must still carry the transfer's 21000 gas.
        let none=crate::rpc::BurnQuote{gas_limit:21_976,total:U256::ZERO,accounts:U256::ZERO,..q};
        assert_eq!(burn_bound(21_000,&none,fee,None,8_000_000),Ok((21_976,21_976)));
        assert!(burn_bound(21_000,&none,fee,Some(U256::from(2_099_999u64)),8_000_000).unwrap_err().contains("cover the gas"));
        // A quote the estimate's own limit does not cover is refused, never signed.
        let short=crate::rpc::BurnQuote{gas_limit:30_000,..q};assert!(burn_bound(21_000,&short,fee,None,8_000_000).is_err());
        assert!(burn_bound(21_000,&q,U256::ZERO,None,8_000_000).is_err());
        let p=BurnPlan{gas:31_000,suggested:35_000,gas_used:21_000,max_fee:fee,quote:q,user_bound:true};let d=p.describe();
        assert_eq!((d["creationBurn"].as_str(),d["gasFee"].as_str(),d["feeBound"].as_str(),d["suggestedFeeBound"].as_str()),(Some("0.000000000001"),Some("0.0000000000021"),Some("0.0000000000031"),Some("0.0000000000035")));
        assert!(d["burnNote"][2].as_str().unwrap().contains("below the suggested"),"{d}");
    }
    /// V3: the registry's topUp() refusals, said before anything is signed.
    #[test] fn v3_top_up_refusals_name_the_reason(){
        let full=U256::from(20_000_000_000_000_000_000u128);let mut r=record(1,0,0);r.bond_state=apply::SystemPrograms::EARNIN_ACCRUING;
        assert_eq!(v3_top_up_refusal(Some(&r),full),None);
        assert!(v3_top_up_refusal(None,full).unwrap().contains("Register first"));
        assert!(v3_top_up_refusal(Some(&record(1,0,0)),full).unwrap().contains("already held in full (20 PCOL)"));
        let exited=apply::MinerRecord{status:2,..r};assert!(v3_top_up_refusal(Some(&exited),full).unwrap().contains("has exited"));
        assert!(v3_top_up_refusal(Some(&apply::MinerRecord{tomb:true,..r}),full).is_some());
    }
    #[test] fn command_surface_is_closed(){
        assert!(matches!(serde_json::from_str::<Command>(r#"{"method":"quote_send","to":"0x01","amount":"1"}"#),Ok(Command::QuoteSend{fee_bound:None,..})));
        assert!(matches!(serde_json::from_str::<Command>(r#"{"method":"quote_send","to":"0x01","amount":"1","fee_bound":"0.01"}"#),Ok(Command::QuoteSend{fee_bound:Some(b),..}) if b=="0.01"));
        assert!(serde_json::from_str::<Command>(r#"{"method":"quote_send","to":"0x01","amount":"1","fee_bound":1}"#).is_err(),"amounts are decimal strings");
        assert!(serde_json::from_str::<Command>(r#"{"method":"quote_send","to":"0x01","amount":"1","gas":"21000"}"#).is_err(),"the gas limit is the wallet's");
        assert!(matches!(serde_json::from_str::<Command>(r#"{"method":"contest_candidates"}"#),Ok(Command::ContestCandidates{})));
        assert!(serde_json::from_str::<Command>(r#"{"method":"contest_candidates","victim":"0x01"}"#).is_err());
        assert!(matches!(serde_json::from_str::<Command>(r#"{"method":"quote_rejoin"}"#),Ok(Command::QuoteRejoin)));
        assert!(matches!(serde_json::from_str::<Command>(r#"{"method":"quote_register"}"#),Ok(Command::QuoteRegister{bond:None})));
        assert!(matches!(serde_json::from_str::<Command>(r#"{"method":"quote_register","bond":"0"}"#),Ok(Command::QuoteRegister{bond:Some(b)}) if b=="0"));
        assert!(matches!(serde_json::from_str::<Command>(r#"{"method":"quote_top_up"}"#),Ok(Command::QuoteTopUp{amount:None})));
        assert!(matches!(serde_json::from_str::<Command>(r#"{"method":"quote_contest","victim":"0x01"}"#),Ok(Command::QuoteContest{bond:None,..})));
        assert!(serde_json::from_str::<Command>(r#"{"method":"quote_contest"}"#).is_err(),"a contest names its victim");
        assert!(serde_json::from_str::<Command>(r#"{"method":"quote_contest","victim":"0x01","bond":20}"#).is_err(),"amounts are decimal strings");
        assert!(serde_json::from_str::<Command>(r#"{"method":"quote_register","bond":20}"#).is_err(),"amounts are decimal strings");
        assert!(serde_json::from_str::<Command>(r#"{"method":"quote_top_up","value":"1"}"#).is_err());
        assert!(serde_json::from_str::<Command>(r#"{"method":"reactivate"}"#).is_err());assert!(serde_json::from_str::<Command>(r#"{"method":"mining","enabled":true,"secret":"x"}"#).is_err());assert!(serde_json::from_str::<Command>(r#"{"method":"send","amount":1}"#).is_err());}
}
