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
//! the confirmed registerContest(reward, victim) does: a seat contest exits the victim (its bond is refunded after U), a
//! pool contest refunds a probationer's accrued bond and removes its record (its part of pots not yet settled burns);
//! both need all seats taken; this account then registers as with
//! quote_register. The registry decides again at inclusion (its phase 0 may credit shares or start an epoch first).
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
    QuoteSend { to:String, amount:String },
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
/// V3-10/V3-14 registerContest: about 2x the largest contest measured (232,451 gas, a seat contest with the victim in the
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
}
/// `t_prob`: the probation deadline in blocks of the genesis programs (`Spec::probation`), None without earn-in.
pub fn rejoin_step(record:Option<&apply::MinerRecord>,next:u64,member:bool,w_act:u64,t_prob:Option<u64>)->RejoinStep{
    let Some(r)=record else{return RejoinStep::Unavailable};
    if r.tomb||!matches!(r.bond_state,1|4){return RejoinStep::Unavailable}
    // Earn-in (EARN-IN-SPEC U4): a probation record reactivates only before registeredAt + T_PROB
    // (60,480 on the earn-in programs, CP T_PROB on the v3 programs, V3-14).
    let deadline=t_prob.filter(|_|r.bond_state==apply::SystemPrograms::EARNIN_ACCRUING).map(|t|r.registered_at.saturating_add(t));
    match r.status{
        1 if member=>RejoinStep::Active,
        1 if deadline.is_some_and(|d|next.saturating_add(w_act)>=d)=>RejoinStep::Missed,
        1=>RejoinStep::Exit,
        2=>{let opens=r.exit_height.saturating_add(w_act);let closes=deadline.map_or(r.due_height,|d|d.min(r.due_height));
            if next>=closes{RejoinStep::Missed}else if next<opens{RejoinStep::Wait{opens,closes}}else{RejoinStep::Reactivate{closes}}},
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
            format!("Its accrued bond is refunded to it and its record is removed now; its part of the pots of the last {} blocks, not yet settled, is burned. You take its probation seat.",p.d),
            format!("You register with a {} PCOL bond, filled from your rewards up to {} PCOL.",coins(self.value),coins(self.full))]}
        else{vec![format!("The member {victim:#x} credited {} shares in epoch {epoch}, below the threshold of {}.",v.count,v.threshold),
            format!("It leaves the active set; its bond is refunded after {} blocks (no penalty).",p.u),
            format!("You take its seat with a {} PCOL bond{}.",coins(self.value),if self.probation{format!(", filled from your rewards up to {} PCOL",coins(self.full))}else{String::new()})]};
        json!({"victim":format!("{victim:#x}"),"contest":{"kind":if self.pool{"pool"}else{"seat"},"count":v.count,"threshold":v.threshold.to_string(),"epoch":epoch,
            "refundAfterBlocks":if self.pool{0}else{p.u}},"steps":steps,"bondTarget":coins(self.full),"probation":self.probation,
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
        let record=n.chain.view_of(&head.hash).and_then(|s|apply::miner_record(s.v(),addr));
        let member=n.chain.membership(head,head.height+1).ok().map(|m|m.ids().contains(&types::raw_addr(addr)));
        let step=match member{Some(m)=>rejoin_step(record.as_ref(),head.height+1,m,u64::from(n.chain.spec.cp().W_act),n.chain.spec.probation().map(|p|p.blocks)),None=>RejoinStep::Unavailable};
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
        let diag=n.incompatible_json();
        crate::network_display::annotate(json!({"type":"status","address":format!("{addr:#x}"),"balance":coins(balance),"balanceWei":balance.to_string(),
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
            "incompatiblePeers":diag["incompatiblePeers"],"legacyHellos":diag["legacyHellos"],"lastIncompatibleReason":diag["lastIncompatibleReason"]}))
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
            Command::QuoteSend{to,amount:a}=>{
                if to.len()!=42||!to.starts_with("0x"){return Err("Enter the full 0x address (42 characters)".into())}
                let to:Address=to.parse().map_err(|_|"Invalid recipient address")?;
                let numeric=U256::from_be_slice(to.as_slice());
                if to.is_zero()||numeric<=U256::from(0xffffu64)||(U256::from(0xc0c001u64)..=U256::from(0xc0c0ffu64)).contains(&numeric){return Err("Cannot send to zero, precompile or reserved system addresses".into())}
                let pending=n.pool_view();
                if pending.account(&to).is_some_and(|a|a.info.code_hash!=revm::primitives::KECCAK_EMPTY&&a.info.code_hash!=revm::primitives::B256::ZERO){return Err("This send screen supports account transfers only".into())}
                self.quote(n,"transfer",to,amount(&a)?,Bytes::new(),21_000)
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
                let r=record.filter(|r|r.status==1&&!r.tomb&&r.bond_state==apply::SystemPrograms::EARNIN_ACCRUING).ok_or("Top-up needs an active probation record")?;
                let t_prob=n.chain.spec.probation().ok_or("Bond top-up exists only on the earn-in network")?.blocks;
                if n.chain.head().height+1>=r.registered_at+t_prob{return Err("The probation period has ended".into())}
                let remaining=full-r.bond;
                let value=match requested.as_deref(){None=>remaining,Some(text)=>amount(text)?};
                if value>remaining{return Err(format!("At most {} PCOL remains to fill the bond",coins(remaining)))}
                let mut q=self.quote(n,"topup",apply::program_address(0),value,keccak256("topUp()")[..4].to_vec().into(),REGISTRY_GAS)?;
                q["bondTarget"]=json!(coins(full));q["bondHeld"]=json!(coins(r.bond));
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
        };
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
        match decide(step,&plan,head,exit,last,record.is_some()){
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
    #[test] fn command_surface_is_closed(){
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
