//! V3 system programs (system-v3.json, PoColSystemV3.sol; V3-PR-E0, V3-14 cfg part, V3-34 registry; V3-PR-E1,
//! V3-08/V3-09 registry budgets; V3-PR-E2b, V3-16/V3-17 evidence penalties; V3-PR-E3 late shares; V3-PR-E2a issuance, carry, tips).
//!
//! The v3 source is PoColSystemEarnIn.sol with P_MAX/T_PROB read through cfg() (CP schema v2 fields 54/55),
//! ChainCaps bounded by the v2 CP length, and the registry budgets of V3-08/V3-09 (a maintenance budget
//! REG_MAINT_MAX = 8 with one maintenance operation per identity per block; reactivate on its own R_react budget,
//! CP 61, with a per-record C_react clock, CP 62, and a window that closes strictly before dueHeight). This file
//! proves:
//! - behaviour-equal outside the budgets: with P_MAX=8/T_PROB=60480 in the CP, every model scenario of
//!   tests/data/earnin-vectors.json replays on the v3 set exactly as the v3 model runs it
//!   (tests/v3/earnin-on-v3-vectors.json); all but "top-up" (whose second topUp in one block from one identity V3-08
//!   refuses) give the earn-in model's own reverts, records, totals and payments and leave the identical Escrow
//!   storage and the identical Registry storage except for the V3-08/V3-09 state alone (the budget slot 9, the clocks
//!   in w1 bits 160..255, and slot 3's per-block counters);
//! - the registry budgets: every scenario of the independent model tests/v3_model.py
//!   (tests/v3/registry-budget-vectors.json) reproduces on the v3 bytecode step by step, with ACC_COUNT and
//!   held_total conserved after every step; the fails-on-original attacks of V3-DESIGN.md V3-08/V3-09 freeze the
//!   earn-in registry and not the v3 one; seven planted budget mutants each fail;
//! - the CP is really read: at P_MAX=64/T_PROB=86400 (O-14=b) the v3 set reproduces the model at those values
//!   (tests/v3/earnin-vectors-p64.json, tests/v3_model_vectors.py; "top-up" as the v3 model runs it), which the
//!   earn-in set does not;
//! - the six planted earn-in mutants (EARN-IN-SPEC.md §9), restated on the v3 source
//!   (tests/v3/system-v3-mutants.json, compile_system_v3.cjs --mutants), each fail both replays (the earn-in
//!   scenarios as the v3 model runs them, at both value sets);
//! - the write ceilings and gas the wallet's bond quotes are sized from: unchanged except for the measured V3-08/09
//!   additions (REGISTRY-LAYOUT.md), and the beginBlock sweep within admission's GAS_ENVELOPE;
//! - sysCodeHash pins and the registry: the v3 set is selected only on a CP schema v2 (specVersion 2) genesis;
//! - V3-PR-E2b, the evidence penalties (V3-16, V3-17): every scenario of the independent chain model of tests/v3_model.py
//!   (tests/v3/evidence-penalty-vectors.json: whole blocks with phase 0 in the v3 order, user transactions, phase 2,
//!   settlement through RewardEscrow) reproduces on the v3 bytecode step by step, with the Escrow invariant held with
//!   equality and the supply conserved after every step; type-2 evidence before settle(j) pays the contributors and
//!   burns only the offender's parts (fails on the earn-in set, which burns the whole pot); an ACCRUING accused winner
//!   burns the whole pot; the settlement boundary; a self-report loses (1 - rho) * B; five planted mutants each fail;
//!   with v3-preview, real blocks through apply_block (the host's v3 phase 0) and the SYS gas of the changed calls
//!   within admission's GAS_ENVELOPE_V3, keys and accounts within resources::derive.
//! - V3-PR-E3, late share credit (V3-02): every scenario of tests/v3/late-share-vectors.json (recordLate in phase 0
//!   after recordResult and before settle(h-D), the no-dilution cap, the window, lastShare = max(lastShare, j), owners
//!   without a record, tombed or ACCRUING) reproduces on the v3 bytecode step by step, the ShareLedger counts and totals
//!   included, with the Escrow invariant and the supply after every step; the omitted contributor is paid at settle(j)
//!   (fails on the earn-in set, which has no recordLate); five planted mutants each fail; with v3-preview, real blocks
//!   with Late effects through apply_block and recordLate's SYS gas at the worst path within GAS_ENVELOPE_V3.
//! - V3-PR-E2a, issuance M3, carry and tip split (V3-19, V3-15, V3-20): the host's issuance(h) reproduces the model's
//!   schedule table (h = 1, H_half - 1, H_half, 256·H_half and more); every scenario of tests/v3/issuance-carry-vectors.json
//!   reproduces on the bytecode with the host's issuance and every recorded pot compared per step and the Escrow equality
//!   (carry term included) after every SYSTEM step; each pot is issued + tips - share + carry and the share reaches a HELD,
//!   ACCRUING or tombed proposer as payTo pays it, nothing on a fallback; pot j+1 is fixed by block j+1 whatever later
//!   blocks credit late; the carry is lagged by one block (D-V3-15): the last carrier of template j (block j + D) gains
//!   nothing in its own pot by omitting LS(j), the omission's base burn reaching pot j + D + 1 (fails on the previous,
//!   unlagged rule, the planted carry-not-lagged); the earn-in set records CP 15 + tips (fails on the original); eight
//!   planted RewardEscrow mutants each fail; with v3-preview, real blocks: host credit = recorded pot every block over
//!   3·H_half (H_half 8) with the carry of settle(h-D) in pot h+1, and CP 15 on the spec-1 earn-in devnet, a final block
//!   burning every outstanding carry (its own settle's included), the invariant's carry terms, the supply bound at
//!   genesis, and closeBlock/finalize's SYS gas on their worst paths within GAS_ENVELOPE_V3.
//! - V3-PR-E4, contestable seats and the probation-pool contest (V3-10, V3-14): every scenario of
//!   tests/v3/contest-vectors.json (the epoch counters of recordShares/recordLate, registerContest's seat and pool contests,
//!   every refusal: above the threshold, in grace, the set not full (for a pool contest too, so exits cannot raise E),
//!   joined after the start of the measured epoch, the late window open, the budgets; a pool victim's pending pots, which
//!   burn; seat victims punished before their due height) reproduces on the bytecode step by step, with every contestStatus view, the counters
//!   and epoch totals compared and the Escrow invariant and supply after every step; the threshold table reproduces through
//!   contestThreshold (threshold and integer square root); thirteen planted mutants each fail; the counters saturate and a
//!   saturated count is never contestable; the contest's write ceilings, gas and user SYS keys; with v3-preview the worst
//!   recordShares/recordLate gas with the counters within admission's envelope.
use pocol_lp4_execution::{native,output,CHAIN_ID,apply::{ProgramSet,Probation,SystemPrograms},devnet::DevnetConfig,profile::ConsensusParams,
    pay::{registry_address,escrow_address}};
use revm::{DatabaseCommit,InMemoryDB,primitives::{Address,AccountInfo,Bytecode,Bytes,TxEnv,TxKind,U256,keccak256,ResultAndState}};
use std::collections::{BTreeMap,BTreeSet};

const PCOL:u128=1_000_000_000_000_000_000;
const EARNIN:&str=include_str!("data/system-earnin.json");
const V3:&str=include_str!("data/system-v3.json");
const MUTANTS:&str=include_str!("v3/system-v3-mutants.json");
const VECTORS:&str=include_str!("data/earnin-vectors.json");
const VECTORS_P64:&str=include_str!("v3/earnin-vectors-p64.json");
const BUDGET_VECTORS:&str=include_str!("v3/registry-budget-vectors.json");
const EVIDENCE_VECTORS:&str=include_str!("v3/evidence-penalty-vectors.json");
const LATE_VECTORS:&str=include_str!("v3/late-share-vectors.json");
const ISSUANCE_VECTORS:&str=include_str!("v3/issuance-carry-vectors.json");
const CONTEST_VECTORS:&str=include_str!("v3/contest-vectors.json");
/// The earn-in model's scenarios (both value sets) re-run through the v3 model (tests/v3_model.py earnin_on_v3).
const EARNIN_ON_V3:&str=include_str!("v3/earnin-on-v3-vectors.json");
/// The six planted earn-in mutants (EARN-IN-SPEC.md §9) and the seven V3-08/V3-09 budget mutants, in fixture order.
const EARNIN_MUTANTS:[&str;6]=["p-max-plus-one","r9-removed","no-promotion-at-b","t-prob-plus-one","over-fill-bad-value","excess-kept"];
const BUDGET_MUTANTS:[&str;7]=["maint-counter-shared","maint-twice-per-block","react-budget-ignored","react-cooldown-off-by-one","react-window-at-sweep-height",
    "maint-bounded-by-reg-ops","react-counts-reg-ops"];
/// The five V3-16/V3-17 evidence-penalty mutants (MinerRegistry or RewardEscrow), in fixture order.
const EVIDENCE_MUTANTS:[&str;5]=["reporter-paid-on-fallback","whole-pot-for-bonded-accused","accruing-read-at-evidence","accruing-read-at-settle","rho-on-pot-not-bond"];
/// The five V3-02 late-share mutants (MinerRegistry or ShareLedger), in fixture order.
const LATE_MUTANTS:[&str;5]=["late-last-share-reverts","late-last-share-overwrites","late-cap-removed","late-window-one-wider","late-total-not-raised"];
/// The eight V3-19/V3-15/V3-20 issuance, carry and tip split mutants (RewardEscrow), in fixture order.
const ECON_MUTANTS:[&str;8]=["carry-not-added","tip-share-to-pot","base-burn-still-burned","issued-ignored-flat-s0","carry-kept-at-final","base-burn-into-closed-pot",
    "carry-not-lagged","lagged-carry-kept-at-final"];
/// The thirteen V3-10/V3-14 contest mutants (MinerRegistry), in fixture order.
const CONTEST_MUTANTS:[&str;13]=["contest-threshold-off-by-one","contest-grace-ignored","contest-joined-in-epoch-allowed","contest-set-not-full-allowed",
    "pool-contest-set-not-full-allowed","contest-late-window-open","pool-contest-refund-skipped","pool-contest-exits-instead","pool-contest-held-victim","counters-not-rolled","late-not-counted",
    "isqrt-not-floored","contest-one-reg-ops-unit"];
/// The v3 set's sysCodeHash = keccak256(RLP([code(C0C001..C0C005), 0x80])) of system-v3.json.
const V3_SYS_CODE_HASH:&str="2702736463d3a053bd7b8b224d1d5a369f01f9f75068d3b672e982c0968be9a5";

fn b()->U256{U256::from(20*PCOL)}
fn system()->Address{Address::from_word((U256::from(1)<<160u32).wrapping_sub(U256::from(2)).into())}
fn caps()->Address{Address::from_word(U256::from(0xc0c004u64).into())}
fn name(n:&str)->Address{Address::from_slice(&keccak256(format!("earnin:{n}"))[12..])}
fn hex(b:&[u8])->String{b.iter().map(|x|format!("{x:02x}")).collect()}
fn unhex(s:&str)->Vec<u8>{let s=s.trim_start_matches("0x");(0..s.len()).step_by(2).map(|i|u8::from_str_radix(&s[i..i+2],16).unwrap()).collect()}
fn runtime(json:&str,n:&str)->Bytes{
    let j:serde_json::Value=serde_json::from_str(json).unwrap();
    let p=j["programs"].as_array().unwrap().iter().find(|p|p["name"]==n).unwrap();
    Bytes::from(unhex(p["runtime"].as_str().unwrap()))
}
/// (name, program, runtime) of every planted mutant.
fn mutant_programs()->Vec<(String,String,Bytes)>{
    let j:serde_json::Value=serde_json::from_str(MUTANTS).unwrap();
    j["mutants"].as_array().unwrap().iter().map(|m|(m["name"].as_str().unwrap().to_string(),m["program"].as_str().unwrap().to_string(),Bytes::from(unhex(m["runtime"].as_str().unwrap())))).collect()
}
/// The MinerRegistry mutants (E0's six, E1's seven, and E2b's punish mutant).
fn mutants()->Vec<(String,Bytes)>{mutant_programs().into_iter().filter(|m|m.1=="MinerRegistry").map(|(n,_,c)|(n,c)).collect()}
fn insert(db:&mut InMemoryDB,a:Address,balance:U256,nonce:u64,code:Bytecode){db.insert_account_info(a,AccountInfo::new(balance,nonce,code.hash_slow(),code));}
fn slot(id:Address)->U256{U256::from_be_bytes(keccak256([id.into_word().as_slice(),U256::from(7).to_be_bytes::<32>().as_slice()].concat()).0)}
fn base(n:u64)->U256{U256::from_be_bytes(keccak256(U256::from(n).to_be_bytes::<32>()).0)}
fn storage(db:&InMemoryDB,a:Address,k:U256)->U256{db.accounts.get(&a).and_then(|x|x.storage.get(&k).copied()).unwrap_or_default()}
fn nonzero(db:&InMemoryDB,a:Address)->BTreeMap<U256,U256>{db.accounts.get(&a).map(|x|x.storage.iter().filter(|(_,v)|!v.is_zero()).map(|(k,v)|(*k,*v)).collect()).unwrap_or_default()}
fn put(db:&mut InMemoryDB,a:Address,k:U256,v:U256){db.insert_account_storage(a,k,v).unwrap();}
fn reg(db:&InMemoryDB,k:u64)->U256{storage(db,registry_address(),U256::from(k))}
fn selector(s:&str)->Vec<u8>{keccak256(s.as_bytes())[..4].to_vec()}
fn balance(db:&InMemoryDB,a:Address)->U256{db.accounts.get(&a).map(|x|x.info.balance).unwrap_or_default()}
fn call_to(db:&mut InMemoryDB,caller:Address,to:Address,data:Vec<u8>,value:U256,h:u64)->ResultAndState{
    let tx=TxEnv{caller,transact_to:TxKind::Call(to),data:Bytes::from(data),value,gas_limit:30_000_000,gas_price:U256::ZERO,
        nonce:Some(db.accounts.get(&caller).map(|a|a.info.nonce).unwrap_or(0)),chain_id:Some(CHAIN_ID),..Default::default()};
    let r=native::execute_at(db,tx,U256::from(h)).unwrap();db.commit(r.state.clone());r
}
fn call(db:&mut InMemoryDB,caller:Address,data:Vec<u8>,value:U256,h:u64)->ResultAndState{call_to(db,caller,registry_address(),data,value,h)}
fn with_arg(sig:&str,arg:Option<Address>)->Vec<u8>{let mut d=selector(sig);if let Some(a)=arg{d.extend(a.into_word());}d}
fn bits(w:U256,lo:usize,len:usize)->u64{((w>>lo)&((U256::from(1)<<len)-U256::from(1))).to::<u64>()}
macro_rules! ensure{($c:expr,$($m:tt)*)=>{if !$c{return Err(format!($($m)*))}}}
macro_rules! ensure_eq{($a:expr,$b:expr,$($m:tt)*)=>{{let (a,b)=(&$a,&$b);if a!=b{return Err(format!("{}: {:?} != {:?}",format!($($m)*),a,b))}}}}

/// The program set the harness runs, and its CP: the earn-in harness CP (earnin_contracts.rs), plus, for a v3
/// set, the 15 schema-v2 slots with P_MAX/T_PROB, and per-run overrides.
/// `v3`: the host conventions of the v3 program set (V3-16/V3-17: phase 0 records the parent's result before the
/// evidence, invalidate(j, accused), punish(accused, reporter)); otherwise the spec-1 host's.
#[derive(Clone)]
struct Setup{registry:Bytes,escrow:Bytes,ledger:Bytes,caps:Bytes,cp_len:u64,cp:Vec<(u64,u128)>,v3:bool}
impl Setup{
    fn earnin()->Self{Self{registry:runtime(EARNIN,"MinerRegistry"),escrow:runtime(EARNIN,"RewardEscrow"),ledger:runtime(EARNIN,"ShareLedger"),caps:runtime(EARNIN,"ChainCaps"),cp_len:52,cp:vec![],v3:false}}
    /// R_react 16 (as the harness's R_max) and C_react 40 (= U, the least ParamGate admits) in CP 61/62; V3-10 (O-10 = b)
    /// E_epoch 2880, theta_bp 1250 and k_sigma 3 in CP 63..65 (tests/v3_model.py DEFAULT_CP; scenarios override them).
    fn v3(p_max:u64,t_prob:u64)->Self{
        Self{registry:runtime(V3,"MinerRegistry"),escrow:runtime(V3,"RewardEscrow"),ledger:runtime(V3,"ShareLedger"),caps:runtime(V3,"ChainCaps"),cp_len:67,
            cp:vec![(54,p_max.into()),(55,t_prob.into()),(61,16),(62,40),(63,2880),(64,1250),(65,3)],v3:true}
    }
    /// A planted mutant in place of the program it names.
    fn with_program(mut self,program:&str,code:Bytes)->Self{match program{"MinerRegistry"=>self.registry=code,"RewardEscrow"=>self.escrow=code,"ShareLedger"=>self.ledger=code,o=>panic!("no mutant of {o}")}self}
    /// The model at P_MAX 64 runs M_MAX 256 and registers 65 probationers in one block (v3_model_vectors.py):
    /// M_max, R_max and REG_OPS wide enough that only P_MAX refuses a seat.
    fn wide(mut self)->Self{self.cp.extend([(17,255),(18,256),(23,1024)]);self}
    fn with_registry(mut self,code:Bytes)->Self{self.registry=code;self}
}

/// `origin`: the current block's proposer (zero on a fallback block), the tx.origin of its SYSTEM calls; `issued`: the
/// issuance the chain steps credited to Escrow; `closed`: per closed height (issued, tips, the carry into pot h before
/// closeBlock, the pot closeBlock recorded) (V3-PR-E2a); `distributed`: per settled height, pot j's amount just before
/// settle(j) or settleFinal(j) distributed it; `final_carry`: at a final block F, the carries into pots F and F+1 just
/// before finalize (V3-15, lagged: block F-1's and F's own settle's).
/// `view`: the result of the last contest_status step, "code,count,threshold" (V3-PR-E4).
struct Harness{db:InMemoryDB,height:u64,senders:BTreeSet<Address>,settled:BTreeMap<Address,U256>,burned:U256,start:BTreeMap<Address,U256>,v3:bool,origin:Address,issued:U256,
    closed:BTreeMap<u64,(U256,U256,U256,U256)>,distributed:BTreeMap<u64,U256>,final_carry:Option<(U256,U256)>,view:Option<String>}
/// V3-15 (lagged by one block): RewardEscrow slot 4 + (t & 1) of the v3 programs holds the carry into pot t.
fn carry_into(db:&InMemoryDB,t:u64)->U256{storage(db,escrow_address(),U256::from(4+(t&1)))}
/// The account the harness's tips come from (a block's transactions pay them to Escrow, the coinbase).
fn tipper()->Address{name("tipper")}
/// SYSTEM calls with a chosen tx.origin (apply.rs runs each SYSTEM call with the block proposer as tx.origin and the
/// SYSTEM address as msg.sender): this code, installed at the SYSTEM address for one transaction from the origin,
/// calls the address in its first calldata word with the rest and returns or reverts with the callee's data.
const FORWARDER:&str="602036036020600037600060006020360360006000600035 5af13d600060003e602757 3d6000fd5b3d6000f3";
fn ledger()->Address{Address::from_word(U256::from(0xc0c003u64).into())}
/// ShareLedger's mapping slots for height j: k = 0 total, 1 count (a mapping by owner), 2 owners.
fn ledger_slot(j:u64,k:u64)->U256{U256::from_be_bytes(keccak256([U256::from(j).to_be_bytes::<32>(),U256::from(k).to_be_bytes::<32>()].concat()).0)}
fn pot(db:&InMemoryDB,j:u64)->(U256,U256,U256){
    let b=U256::from_be_bytes(keccak256([U256::from(j).to_be_bytes::<32>(),U256::from(3).to_be_bytes::<32>()].concat()).0);
    (storage(db,escrow_address(),b),storage(db,escrow_address(),b+U256::from(1)),storage(db,escrow_address(),b+U256::from(2)))
}
impl Harness{
    fn new(durable:&[String],s:&Setup)->Self{
        let mut db=InMemoryDB::default();
        insert(&mut db,system(),U256::ZERO,1,Bytecode::default());
        for (a,code) in [(registry_address(),&s.registry),(escrow_address(),&s.escrow),(ledger(),&s.ledger),(caps(),&s.caps)]{insert(&mut db,a,U256::ZERO,1,Bytecode::new_raw(code.clone()));}
        for n in 0..s.cp_len{put(&mut db,caps(),U256::from(n),U256::ZERO);}
        // CP of a continuous network (47/48 zero), with per-block limits wide enough for the vectors'
        // many same-block registrations; consensus values otherwise as Mainnet (as earnin_contracts.rs).
        for (n,v) in [(11,4u128),(16,20*PCOL),(17,16),(18,64),(19,1),(20,8192),(21,16),(22,64),(23,64),(24,32),(25,64),(26,16),(27,40),(50,64)].into_iter().chain(s.cp.iter().copied()){
            assert!(n<s.cp_len,"CP slot {n} outside the program's ChainCaps");put(&mut db,caps(),U256::from(n),U256::from(v));
        }
        // m0: a HELD genesis record with bond B whose bond Escrow holds (the model's first member).
        let m0=name("m0");insert(&mut db,m0,U256::ZERO,1,Bytecode::default());let r=slot(m0);
        for (k,v) in [(U256::ZERO,U256::from(1)),(U256::from(1),U256::from(1)),(U256::from(4),b()),(base(5),U256::from_be_slice(m0.as_slice())),(base(6),U256::from_be_slice(m0.as_slice())),
            (r,U256::from(1)|(U256::from(1)<<16u32)),(r+U256::from(1),U256::from_be_slice(m0.as_slice())),(r+U256::from(2),b())]{put(&mut db,registry_address(),k,v);}
        db.accounts.get_mut(&escrow_address()).unwrap().info.balance=b();
        for d in durable{insert(&mut db,name(d),U256::ZERO,1,Bytecode::default());}
        let start=db.accounts.iter().map(|(a,x)|(*a,x.info.balance)).collect();
        Harness{db,height:0,senders:BTreeSet::new(),settled:BTreeMap::new(),burned:U256::ZERO,start,v3:s.v3,origin:Address::ZERO,issued:U256::ZERO,closed:BTreeMap::new(),distributed:BTreeMap::new(),final_carry:None,view:None}
    }
    /// One SYSTEM call at the current height with tx.origin = self.origin, through FORWARDER. The origin's nonce and
    /// the SYSTEM account are restored afterwards, so the call leaves only its own effects.
    fn sys_call(&mut self,to:Address,data:Vec<u8>)->ResultAndState{
        let origin=self.origin;let before=self.db.accounts.get(&origin).map(|a|a.info.nonce);
        let code=Bytecode::new_raw(Bytes::from(unhex(&FORWARDER.replace(' ',""))));
        insert(&mut self.db,system(),U256::ZERO,1,code);
        let mut d=to.into_word().to_vec();d.extend(data);
        let r=call_to(&mut self.db,origin,system(),d,U256::ZERO,self.height);
        insert(&mut self.db,system(),U256::ZERO,1,Bytecode::default());
        match before{Some(n)=>self.db.accounts.get_mut(&origin).unwrap().info.nonce=n,None=>{self.db.accounts.remove(&origin);}}
        r
    }
    /// A SYSTEM call that must succeed or name its revert.
    fn sys(&mut self,what:&str,to:Address,data:Vec<u8>)->Result<(),String>{
        let r=self.sys_call(to,data);
        if r.result.is_success(){return Ok(())}
        let out=output(&r.result).to_vec();
        let known=["BadCalldata","BadState","BadPending","BadSystemCall","Unauthorized","BadPhase","HeightOverflow","PaymentFailed"];
        Err(known.iter().find(|k|selector(&format!("{k}()"))==out).map(|k|k.to_string()).unwrap_or(format!("{what}: {:?} {out:?}",r.result)))
    }
    /// punish as the host calls it on this program set: v3 `punish(accused, reporter)` with the reporter = tx.origin,
    /// spec-1 `punish(accused)`.
    fn punish_call(&mut self,accused:Address)->Result<(),String>{
        if self.v3{let mut d=with_arg("punish(address,address)",Some(accused));d.extend(self.origin.into_word());self.sys("punish",registry_address(),d)}
        else{self.sys("punish",registry_address(),with_arg("punish(address)",Some(accused)))}
    }
    /// Phase 0 of block h (tests/v3_model.py Chain.begin): v3 records the parent's result first, then the evidence;
    /// the spec-1 host runs the evidence first. A revert invalidates the block: the state is restored.
    fn begin(&mut self,step:&serde_json::Value)->Result<Option<String>,String>{
        let op=step["op"].as_array().unwrap();let h:u64=op[1].as_str().unwrap().parse().unwrap();let proposer=op[2].as_str().unwrap();
        ensure!(h==self.height+1,"begin {h} after {}",self.height);
        let saved=(self.db.clone(),self.height,self.origin);
        self.height=h;self.origin=if proposer.is_empty(){Address::ZERO}else{name(proposer)};
        let r=self.phase0(step,h);
        if let Err(e)=r{(self.db,self.height,self.origin)=saved;return Ok(Some(e))}
        Ok(None)
    }
    fn phase0(&mut self,step:&serde_json::Value,h:u64)->Result<(),String>{
        let cp_d=storage(&self.db,caps(),U256::from(11)).to::<u64>();
        let record=|x:&mut Harness|->Result<(),String>{
            if h<2{return Ok(())}
            let winner=name(step["winner"].as_str().unwrap());
            let mut seq:Vec<Address>=step["shares"].as_array().unwrap().iter().map(|x|name(x.as_str().unwrap())).collect();seq.push(winner);
            let mut data=selector("recordShares(uint64,address[])");data.extend(U256::from(h-1).to_be_bytes::<32>());data.extend(U256::from(64).to_be_bytes::<32>());data.extend(U256::from(seq.len()).to_be_bytes::<32>());
            for a in &seq{data.extend(a.into_word());}
            x.sys("recordShares",registry_address(),data)?;
            let mut data=selector("recordResult(uint256,address)");data.extend(U256::from(h-1).to_be_bytes::<32>());data.extend(winner.into_word());
            x.sys("recordResult",escrow_address(),data)
        };
        if self.v3{record(self)?;}
        for e in step["evidence"].as_array().unwrap(){
            // V3-02: a late credit is recordLate(j, owners) (selector absent from the earn-in programs: the call reverts).
            if e[0]=="late"{
                let owners:Vec<Address>=e[2].as_array().unwrap().iter().map(|x|name(x.as_str().unwrap())).collect();
                let mut data=selector("recordLate(uint64,address[])");data.extend(U256::from(e[1].as_u64().unwrap()).to_be_bytes::<32>());
                data.extend(U256::from(64).to_be_bytes::<32>());data.extend(U256::from(owners.len()).to_be_bytes::<32>());
                for a in &owners{data.extend(a.into_word());}
                self.sys("recordLate",registry_address(),data)?;continue
            }
            // The spec-1 host has no reporter: punish(accused) whatever the evidence names.
            let accused=name(e[0].as_str().unwrap());let reporter=e[2].as_str().unwrap();
            if let Some(j)=e[1].as_u64(){
                if !pot(&self.db,j).1.is_zero(){
                    let mut data=selector(if self.v3{"invalidate(uint256,address)"}else{"invalidate(uint256)"});data.extend(U256::from(j).to_be_bytes::<32>());
                    if self.v3{data.extend(accused.into_word());}
                    self.sys("invalidate",escrow_address(),data)?;
                }
            }
            if self.v3{
                // The reporter the evidence names (the honest host passes the proposer; the vectors also try others).
                let mut data=with_arg("punish(address,address)",Some(accused));data.extend((if reporter.is_empty(){Address::ZERO}else{name(reporter)}).into_word());
                self.sys("punish",registry_address(),data)?;
            }else{self.punish_call(accused)?;}
        }
        self.sys("markClose",registry_address(),selector("markClose()"))?;
        self.sys("pruneShares",ledger(),selector("pruneShares()"))?;
        if !self.v3{record(self)?;}
        if h>cp_d{
            self.distributed.insert(h-cp_d,pot(&self.db,h-cp_d).0);
            let mut data=selector("settle(uint256)");data.extend(U256::from(h-cp_d).to_be_bytes::<32>());self.sys("settle",escrow_address(),data)?;
        }
        self.sys("beginBlock",registry_address(),selector("beginBlock()"))
    }
    /// The block's tips reach Escrow (the coinbase) from the tipper, as a transaction's priority fees would.
    fn pay_tips(&mut self,tips:U256){
        if tips.is_zero(){return}
        if !self.db.accounts.contains_key(&tipper()){let v=U256::from(1_000_000*PCOL);insert(&mut self.db,tipper(),v,1,Bytecode::default());self.start.insert(tipper(),v);}
        self.db.accounts.get_mut(&tipper()).unwrap().info.balance-=tips;self.db.accounts.get_mut(&escrow_address()).unwrap().info.balance+=tips;
    }
    /// Phase 2 of the current block, as the host runs it on this program set. V3-19/V3-20 (v3): the host credits
    /// issuance(h) of lp4-execution issuance.rs (S0 = CP 15, H_half = CP 52, S_tail = CP 53) and calls closeBlock(tips,
    /// issued); the spec-1 host credits CP 15 and calls closeBlock(tips). Returns the issuance.
    fn close(&mut self,h:u64,tips:U256)->Result<U256,String>{
        ensure!(h==self.height,"close {h} at {}",self.height);
        let cp=|n:u64|storage(&self.db,caps(),U256::from(n));
        let issued=if self.v3{pocol_lp4_execution::issuance::issuance(h,cp(15),cp(52).to::<u64>(),cp(53))}else{cp(15)};
        self.db.accounts.get_mut(&escrow_address()).unwrap().info.balance+=issued;self.issued+=issued;
        self.pay_tips(tips);
        let carry=carry_into(&self.db,h);
        let mut data=selector(if self.v3{"closeBlock(uint256,uint256)"}else{"closeBlock(uint256)"});data.extend(tips.to_be_bytes::<32>());
        if self.v3{data.extend(issued.to_be_bytes::<32>());}
        self.sys("closeBlock",escrow_address(),data).map_err(|e|format!("closeBlock at {h}: {e}"))?;
        self.closed.insert(h,(issued,tips,carry,pot(&self.db,h).0));
        Ok(issued)
    }
    /// Phase 2 of a final block F: finalize(tips), then settleFinal(j) for every pending pot j in [F-D+1, F-1].
    fn finalize(&mut self,h:u64,tips:U256)->Result<(),String>{
        ensure!(h==self.height,"finalize {h} at {}",self.height);
        self.pay_tips(tips);
        self.final_carry=Some((carry_into(&self.db,h),carry_into(&self.db,h+1)));
        let mut data=selector("finalize(uint256)");data.extend(tips.to_be_bytes::<32>());
        self.sys("finalize",escrow_address(),data).map_err(|e|format!("finalize at {h}: {e}"))?;
        let d=storage(&self.db,caps(),U256::from(11)).to::<u64>();
        for j in h.saturating_sub(d)+1..h{
            if pot(&self.db,j).1.is_zero(){continue}
            self.distributed.insert(j,pot(&self.db,j).0);
            let mut data=selector("settleFinal(uint256)");data.extend(U256::from(j).to_be_bytes::<32>());
            self.sys("settleFinal",escrow_address(),data).map_err(|e|format!("settleFinal({j}) at {h}: {e}"))?;
        }
        Ok(())
    }
    /// A model "close" or "final" step: its tips, and (close) the issuance and pot the model recorded.
    fn phase2(&mut self,step:&serde_json::Value)->Result<(),String>{
        let op=step["op"].as_array().unwrap();let h:u64=op[1].as_str().unwrap().parse().unwrap();
        let tips=step.get("tips").map_or(U256::ZERO,|t|t.as_str().unwrap().parse().unwrap());
        if op[0]=="final"{return self.finalize(h,tips)}
        let issued=self.close(h,tips)?;
        if let Some(want)=step.get("issued"){ensure_eq!(issued.to_string(),want.as_str().unwrap(),"issuance at {h} (host issuance.rs vs the model)");}
        if let Some(want)=step.get("amount"){ensure_eq!(self.closed[&h].3.to_string(),want.as_str().unwrap(),"pot {h} as closeBlock recorded it");}
        Ok(())
    }
    /// Bytecode-only conservation of a chain (no model): Escrow's balance is exactly the pending pots plus the held
    /// bonds plus everything burned plus (V3-15) every outstanding carry, both carry slots (T17 with equality: no forced
    /// transfers), and the supply is the start plus issuance.
    fn chain_conserved(&self)->Result<(),String>{
        let pending=(1..=self.height).map(|j|pot(&self.db,j).0).fold(U256::ZERO,|a,b|a+b);
        let need=pending+reg(&self.db,4)+storage(&self.db,escrow_address(),U256::ZERO)+carry_into(&self.db,0)+carry_into(&self.db,1);
        ensure_eq!(balance(&self.db,escrow_address()),need,"Escrow balance = pending + held + burned");
        let total=self.db.accounts.values().map(|a|a.info.balance).fold(U256::ZERO,|a,b|a+b);
        let start=self.start.values().copied().fold(U256::ZERO,|a,b|a+b);
        ensure_eq!(total,start+self.issued,"supply = start + issuance");
        Ok(())
    }
    fn at(&mut self,h:u64)->Result<(),String>{
        if h>self.height{for s in ["markClose()","beginBlock()"]{let r=call(&mut self.db,system(),selector(s),U256::ZERO,h);ensure!(r.result.is_success(),"{s} at {h}: {:?}",r.result);}self.height=h;}
        Ok(())
    }
    /// A fresh sender: nonce 0, so a self reward exercises R9.
    fn sender(&mut self,n:&str)->Address{
        let a=name(n);if self.senders.insert(a)&&!self.db.accounts.contains_key(&a){insert(&mut self.db,a,U256::from(1000*PCOL),0,Bytecode::default());self.start.insert(a,U256::from(1000*PCOL));}a
    }
    fn record(&self,id:Address)->(U256,U256,U256,U256){let r=slot(id);(storage(&self.db,registry_address(),r),storage(&self.db,registry_address(),r+U256::from(1)),storage(&self.db,registry_address(),r+U256::from(2)),storage(&self.db,registry_address(),r+U256::from(3)))}
    /// One model step: Ok(None) success, Ok(Some(revert name)) a revert, Err a harness/system failure.
    fn step(&mut self,op:&[String])->Result<Option<String>,String>{
        let num=|i:usize|op[i].parse::<u128>().unwrap();
        let res=match op[0].as_str(){
            "register"=>{let s=self.sender(&op[1]);self.at(num(4) as u64)?;call(&mut self.db,s,with_arg("register(address)",Some(name(&op[2]))),U256::from(num(3)),self.height)}
            "top_up"=>{let s=self.sender(&op[1]);self.at(num(3) as u64)?;call(&mut self.db,s,selector("topUp()"),U256::from(num(2)),self.height)}
            "deregister"|"reactivate"=>{let s=self.sender(&op[1]);self.at(num(2) as u64)?;call(&mut self.db,s,selector(&format!("{}()",op[0])),U256::ZERO,self.height)}
            "claim_bond"=>{let s=self.sender(&op[1]);self.at(num(2) as u64)?;call(&mut self.db,s,selector("claimBond()"),U256::ZERO,self.height)}
            "set_reward"=>{let s=self.sender(&op[1]);self.at(num(3) as u64)?;call(&mut self.db,s,with_arg("setRewardAddress(address)",Some(name(&op[2]))),U256::ZERO,self.height)}
            "remove_inactive"=>{let s=self.sender(&op[1]);self.at(num(3) as u64)?;call(&mut self.db,s,with_arg("removeInactive(address)",Some(name(&op[2]))),U256::ZERO,self.height)}
            // V3-10/V3-14: registerContest(reward, victim) with the bond as value.
            "register_contest"=>{let s=self.sender(&op[1]);self.at(num(5) as u64)?;let mut d=with_arg("registerContest(address,address)",Some(name(&op[2])));d.extend(name(&op[3]).into_word());
                call(&mut self.db,s,d,U256::from(num(4)),self.height)}
            // V3-10: the contestStatus(victim, probation) view at the current block, kept for the caller to compare.
            "contest_status"=>{self.at(num(3) as u64)?;
                self.view=Some(self.try_contest_status(name(&op[1]),op[2]=="true").map_or_else(|e|e,|(c,n,t)|format!("{c},{n},{t}")));return Ok(None)}
            "sweep"=>{self.at(num(1) as u64)?;return Ok(None)}
            // A registry scenario's punish runs outside any block's evidence: no reporter (the whole bond burns, as the
            // registry model's punish), so on v3 with tx.origin zero.
            "punish"=>{let id=name(&op[1]);self.at(num(2) as u64)?;self.origin=Address::ZERO;self.punish_call(id).map_err(|e|format!("punish: {e}"))?;return Ok(None)}
            "settle_to"=>{self.settle(name(&op[1]),U256::from(num(2)))?;return Ok(None)}
            o=>panic!("unknown op {o}"),
        };
        if res.result.is_success(){return Ok(None)}
        let out=output(&res.result);
        let known=["BadValue","BadState","Capacity","RewardNotDurable","PaymentFailed","BadCalldata","RegOpsLimit","NotContestable","Closing","BadPhase"];
        Ok(Some(known.iter().find(|k|selector(&format!("{k}()"))==out).map(|k|k.to_string()).unwrap_or(format!("{out:?}"))))
    }
    /// V3-10: contestStatus(victim, probation) at the current height: (code, count, threshold).
    fn try_contest_status(&mut self,victim:Address,probation:bool)->Result<(u64,u64,U256),String>{
        let mut d=with_arg("contestStatus(address,bool)",Some(victim));d.extend(U256::from(probation as u8).to_be_bytes::<32>());
        let r=call(&mut self.db,name("viewer"),d,U256::ZERO,self.height);if !r.result.is_success(){return Err(format!("contestStatus reverted: {:?}",output(&r.result)))}
        let o=output(&r.result);Ok((U256::from_be_slice(&o[0..32]).to::<u64>(),U256::from_be_slice(&o[32..64]).to::<u64>(),U256::from_be_slice(&o[64..96])))
    }
    fn contest_status(&mut self,victim:Address,probation:bool)->(u64,u64,U256){self.try_contest_status(victim,probation).unwrap()}
    /// Escrow payTo/distribute for one owner: an ACCRUING record goes through the real accrue();
    /// HELD is paid, absent/tomb burned (unchanged v1 behaviour, modelled here).
    fn settle(&mut self,id:Address,amount:U256)->Result<(),String>{
        let (w0,w1,_,_)=self.record(id);
        if w0.is_zero()||bits(w0,8,8)!=0{self.burned+=amount;return Ok(())}
        let reward=Address::from_word(w1.into());
        let excess=if bits(w0,16,8)==4{self.accrue(id,amount)?}else{amount};
        if !excess.is_zero(){*self.settled.entry(reward).or_default()+=excess;}
        Ok(())
    }
    /// The Registry admits only Escrow; an account with code cannot originate a transaction, so the harness
    /// calls from Escrow's address with its code set aside for this one call.
    fn accrue_call(&mut self,id:Address,amount:U256)->ResultAndState{
        let code=self.db.accounts[&escrow_address()].info.clone();
        insert(&mut self.db,escrow_address(),code.balance,code.nonce,Bytecode::default());
        let mut d=selector("accrue(address,uint256)");d.extend(id.into_word());d.extend(amount.to_be_bytes::<32>());
        let r=call(&mut self.db,escrow_address(),d,U256::ZERO,self.height);
        let info=self.db.accounts.get_mut(&escrow_address()).unwrap();info.info.code=code.code.clone();info.info.code_hash=code.code_hash;info.info.nonce=code.nonce;
        r
    }
    fn accrue(&mut self,id:Address,amount:U256)->Result<U256,String>{
        let r=self.accrue_call(id,amount);ensure!(r.result.is_success(),"accrue: {:?}",r.result);Ok(U256::from_be_slice(&output(&r.result)[..32]))
    }
}

/// The observable outcome of one scenario, for the earn-in vs v3 comparison.
#[derive(Debug,PartialEq)]
struct Outcome{steps:Vec<Option<String>>,registry:BTreeMap<U256,U256>,escrow:BTreeMap<U256,U256>,balances:BTreeMap<Address,U256>,ids:BTreeSet<Address>}

/// Replays one model scenario and checks every model result (as earnin_contracts.rs); Err names the first mismatch.
fn scenario(name_:&str,body:&serde_json::Value,s:&Setup)->Result<Outcome,String>{
    let durable:Vec<String>=body["durable"].as_array().unwrap().iter().map(|x|x.as_str().unwrap().to_string()).collect();
    let mut h=Harness::new(&durable,s);let mut steps=Vec::new();
    for step in body["steps"].as_array().unwrap(){
        let op:Vec<String>=step["op"].as_array().unwrap().iter().map(|x|x.as_str().unwrap().to_string()).collect();
        let got=h.step(&op)?;
        match (&got,step["ok"].as_bool().unwrap()){
            (None,true)=>{}
            (Some(e),false)=>ensure_eq!(e.as_str(),step["revert"].as_str().unwrap(),"{name_} {op:?}"),
            _=>return Err(format!("{name_} {op:?}: contract {got:?}, model {step}")),
        }
        steps.push(got);
    }
    let fin=&body["final"];
    ensure_eq!(reg(&h.db,4).to_string(),fin["heldTotal"].as_str().unwrap(),"{name_} heldTotal");
    ensure_eq!(reg(&h.db,8).to::<u64>(),fin["accruing"].as_u64().unwrap(),"{name_} accruing");
    let burned=storage(&h.db,escrow_address(),U256::ZERO)+h.burned;
    ensure_eq!(burned.to_string(),fin["burned"].as_str().unwrap(),"{name_} burned");
    let active:BTreeSet<Address>=(0..reg(&h.db,1).to::<u64>()).map(|i|Address::from_word(storage(&h.db,registry_address(),base(6)+U256::from(i)).into())).collect();
    let want:BTreeSet<Address>=fin["active"].as_array().unwrap().iter().map(|x|name(x.as_str().unwrap())).collect();
    ensure_eq!(active,want,"{name_} active set");
    let records=fin["records"].as_object().unwrap();
    ensure_eq!(reg(&h.db,0).to::<usize>(),records.len(),"{name_} record count");
    for (n,r) in records{
        let (w0,w1,w2,w3)=h.record(name(n));
        let got=(bits(w0,0,8),bits(w0,16,8),w2.to_string(),bits(w0,8,8)!=0,bits(w0,152,64),bits(w3,0,64),bits(w3,64,64),Address::from_word(w1.into()));
        let want=(r["status"].as_u64().unwrap(),r["bond_state"].as_u64().unwrap(),r["bond"].as_str().unwrap().to_string(),
            r["tomb"].as_bool().unwrap(),r["registered_at"].as_u64().unwrap(),r["exit"].as_u64().unwrap(),r["due"].as_u64().unwrap(),name(r["reward"].as_str().unwrap()));
        ensure_eq!(got,want,"{name_} record {n}");
    }
    for (n,amount) in fin["paid"].as_object().unwrap(){
        let a=name(n);let refunds=if h.senders.contains(&a){U256::ZERO}else{balance(&h.db,a)-h.start.get(&a).copied().unwrap_or_default()};
        let got=refunds+h.settled.get(&a).copied().unwrap_or_default();
        ensure_eq!(got.to_string(),amount.as_str().unwrap(),"{name_} paid {n}");
    }
    // Each sender's net PCOL into the Registry (bonds and top-ups, less returned excess and refunds).
    for a in &h.senders{
        let n=fin["records"].as_object().unwrap().keys().chain(fin["spent"].as_object().unwrap().keys()).find(|n|name(n)==*a).cloned();
        let Some(n)=n else{continue};
        let field=|k:&str|fin[k].get(&n).and_then(|x|x.as_str()).map(|x|x.parse::<U256>().unwrap()).unwrap_or_default();
        let refunds=field("paid")-h.settled.get(a).copied().unwrap_or_default();
        ensure_eq!(h.start[a]-balance(&h.db,*a),field("spent")-refunds,"{name_} sender {n} net PCOL");
    }
    let balances=h.db.accounts.iter().map(|(a,x)|(*a,x.info.balance)).collect();
    let ids=h.senders.iter().copied().chain([name("m0")]).collect();
    Ok(Outcome{steps,registry:nonzero(&h.db,registry_address()),escrow:nonzero(&h.db,escrow_address()),balances,ids})
}
/// Every scenario of a vector file; Err at the first scenario that differs from the model.
fn replay(vectors:&str,s:&Setup)->Result<BTreeMap<String,Outcome>,String>{
    let v:serde_json::Value=serde_json::from_str(vectors).unwrap();let mut out=BTreeMap::new();
    for (scenario_,body) in v.as_object().unwrap(){
        if body.get("steps").is_none(){continue}
        out.insert(scenario_.clone(),scenario(scenario_,body,s)?);
    }
    Ok(out)
}

/// The earn-in model's scenarios as the v3 model runs them ("earnin/<name>" or "earnin-p64/<name>"), on a v3 set:
/// every step, record, clock and total as in budget_scenario.
fn earnin_on_v3(label:&str,s:&Setup)->Result<usize,String>{
    let v:serde_json::Value=serde_json::from_str(EARNIN_ON_V3).unwrap();let mut n=0;
    for (k,body) in v.as_object().unwrap(){if k.split('/').next()==Some(label){budget_scenario(k,body,s)?;n+=1;}}
    Ok(n)
}
/// Scenarios whose step results the v3 model and the earn-in model give differently (V3-08/V3-09 refusals).
fn v3_model_differs(vectors:&str,label:&str)->BTreeSet<String>{
    let (e,v):(serde_json::Value,serde_json::Value)=(serde_json::from_str(vectors).unwrap(),serde_json::from_str(EARNIN_ON_V3).unwrap());
    let key=|st:&serde_json::Value|(st["ok"].clone(),st.get("revert").cloned());
    e.as_object().unwrap().iter().filter(|(_,b)|b.get("steps").is_some()).filter(|(n,b)|{
        let w=&v[format!("{label}/{n}")]["steps"];
        b["steps"].as_array().unwrap().iter().zip(w.as_array().unwrap()).any(|(a,c)|key(a)!=key(c))
    }).map(|(n,_)|n.clone()).collect()
}

#[test]
fn v3_set_at_the_earnin_values_is_behaviour_equal_to_the_earnin_set(){
    let earnin=replay(VECTORS,&Setup::earnin()).unwrap();
    let v3_setup=Setup::v3(Probation::EARNIN.seats,Probation::EARNIN.blocks);
    // The v3 set reproduces every earn-in scenario as the v3 model runs it. The two models give the same step results
    // everywhere except "top-up", whose second topUp from one identity in one block is the maintenance operation
    // V3-08 refuses (tests/v3_model.py earnin_on_v3 asserts that the first difference is that RegOpsLimit).
    assert_eq!(earnin_on_v3("earnin",&v3_setup),Ok(8),"every model scenario replayed");
    assert_eq!(v3_model_differs(VECTORS,"earnin"),BTreeSet::from(["top-up".to_string()]));
    // Every other scenario: the earn-in model's own results, and the same storage and balances as on the earn-in set.
    let v:serde_json::Value=serde_json::from_str(VECTORS).unwrap();
    let mut v3=BTreeMap::new();
    for (n,body) in v.as_object().unwrap(){if body.get("steps").is_some()&&n!="top-up"{v3.insert(n.clone(),scenario(n,body,&v3_setup).unwrap());}}
    assert_eq!(v3.len(),7);
    let mut e1_keys=0;
    for (name_,got) in &v3{
        let want=&earnin[name_];
        assert_eq!(got.steps,want.steps,"{name_}: every step's result");
        // V3-08/V3-09 state, and only it, differs: the budget slot 9, the clocks in w1 bits 160..255 (and the V3-10
        // epoch counters in w0 bits 216..255, which these scenarios never write: no recordShares runs) of every identity
        // and slot 3's per-block counters (bits 0..23, which every beginBlock clears). The earn-in set has none of the
        // first two.
        let (g,extra)=without_budget_state(&got.registry,&got.ids);let (w,none)=without_budget_state(&want.registry,&want.ids);
        assert!(none.is_empty(),"{name_}: the earn-in registry holds no V3-08/V3-09 state: {none:?}");
        e1_keys+=extra.len();
        assert_eq!(g,w,"{name_}: identical MinerRegistry storage outside the V3-08/V3-09 state");
        assert_eq!(got.escrow,want.escrow,"{name_}: identical RewardEscrow storage");
        assert_eq!(got.balances,want.balances,"{name_}: identical balances");
    }
    // The model scenarios top up and reactivate, so the v3 set did keep budget state (the projection is not vacuous).
    assert!(e1_keys>0,"no V3-08/V3-09 state was written");
}

/// Splits the V3-08/V3-09 state out of a Registry storage map: slot 9 and the clock bits (w1 160..255; w0 216..255, the
/// V3-10 counters)
/// of the given ids are returned separately (keyed by slot), and slot 3's per-block counters (bits 0..23, which both
/// sets keep but count differently) are cleared in the rest.
fn without_budget_state(m:&BTreeMap<U256,U256>,ids:&BTreeSet<Address>)->(BTreeMap<U256,U256>,BTreeMap<U256,U256>){
    let mask=|bits:usize|(U256::from(1)<<bits)-U256::from(1);
    let mut rest=m.clone();let mut extra=BTreeMap::new();
    if let Some(c)=rest.get_mut(&U256::from(3)){*c&=!mask(24);}
    let mut split=|k:U256,keep:U256|{if let Some(v)=rest.get_mut(&k){let e=*v&!keep;if !e.is_zero(){extra.insert(k,e);}*v&=keep;}};
    split(U256::from(9),U256::ZERO);
    for a in ids{split(slot(*a),mask(216));split(slot(*a)+U256::from(1),mask(160));}
    rest.retain(|_,v|!v.is_zero());
    (rest,extra)
}

#[test]
fn v3_set_reads_p_max_and_t_prob_from_the_cp(){
    let p64=Setup::v3(64,86_400).wide();
    let v:serde_json::Value=serde_json::from_str(VECTORS_P64).unwrap();
    assert_eq!(v["params"],serde_json::json!({"P_MAX":64,"T_PROB":86400,"M_MAX":256}));
    // Every model scenario at P_MAX 64 / T_PROB 86400, as the v3 model runs them; outside "top-up" (V3-08, above) also
    // with the earn-in model's own results.
    assert_eq!(earnin_on_v3("earnin-p64",&p64),Ok(8),"every model scenario at P_MAX 64 / T_PROB 86400");
    assert_eq!(v3_model_differs(VECTORS_P64,"earnin-p64"),BTreeSet::from(["top-up".to_string()]));
    for (n,body) in v.as_object().unwrap(){if body.get("steps").is_some()&&n!="top-up"{scenario(n,body,&p64).unwrap();}}
    // The earn-in set keeps its constants 8 and 60480 whatever the CP says: it fails the same vectors at the
    // 9th probation seat. (The model's deadline scenario checks only final states, which both deadlines reach;
    // the deadline itself is probed block by block below.)
    let mut fails=BTreeSet::new();
    for (scenario_,body) in v.as_object().unwrap(){
        if body.get("steps").is_none(){continue}
        if scenario(scenario_,body,&Setup::earnin().wide()).is_err(){fails.insert(scenario_.as_str());}
    }
    assert_eq!(fails,BTreeSet::from(["probation-seats"]));
    // And the v3 set at the O-14=b values fails the earn-in vectors at the 9th seat and at their deadline (whose
    // last sweep, 2·60480+90, still precedes a deadline of 86400 blocks after the second registration). "top-up"
    // differs on any v3 CP (V3-08), so it is no evidence of CP reads and is left out here.
    let v:serde_json::Value=serde_json::from_str(VECTORS).unwrap();let mut fails=BTreeSet::new();
    for (scenario_,body) in v.as_object().unwrap(){
        if body.get("steps").is_none()||scenario_=="top-up"{continue}
        if scenario(scenario_,body,&p64).is_err(){fails.insert(scenario_.as_str());}
    }
    assert_eq!(fails,BTreeSet::from(["deadline","probation-seats"]));
    // The deadline, block by block: a probation record registered at 10 tops up at 10+T_PROB-1, is swept out by
    // markClose at 10+T_PROB, and cannot top up there. T_PROB is the earn-in constant on the earn-in set
    // whatever the CP holds, and the CP value on the v3 set.
    for (s,t) in [(Setup::earnin(),60_480u64),(Setup::v3(8,60_480),60_480),(Setup::v3(64,86_400),86_400),(Setup::v3(64,1_000),1_000)]{
        let mut h=Harness::new(&["rd".to_string()],&s);let a=name("d");
        assert_eq!(h.step(&["register".into(),"d".into(),"rd".into(),PCOL.to_string(),"10".into()]).unwrap(),None);
        assert_eq!(h.step(&["top_up".into(),"d".into(),"1".into(),(10+t-1).to_string()]).unwrap(),None,"T_PROB {t}: last probation block");
        assert_eq!((bits(h.record(a).0,0,8),bits(h.record(a).0,16,8)),(1,4),"T_PROB {t}: still an active probation record");
        assert_eq!(h.step(&["top_up".into(),"d".into(),"1".into(),(10+t).to_string()]).unwrap().as_deref(),Some("BadState"),"T_PROB {t}: deadline");
        assert_eq!(bits(h.record(a).0,0,8),2,"T_PROB {t}: the sweep exited it at the deadline");
    }
    // Edges: P_MAX 0 refuses every probation seat (the full bond still registers); T_PROB 1 ends probation in the
    // block after registration, so a top-up there is BadState.
    let mut h=Harness::new(&[],&Setup::v3(0,60_480));h.at(1).unwrap();
    let a=h.sender("edge-a");assert_eq!(h.step(&["register".into(),"edge-a".into(),"edge-a".into(),"0".into(),"1".into()]).unwrap().as_deref(),Some("Capacity"));
    assert_eq!(h.step(&["register".into(),"edge-a".into(),"edge-a".into(),b().to_string(),"1".into()]).unwrap(),None);
    assert_eq!(bits(h.record(a).0,16,8),1,"HELD");
    let mut h=Harness::new(&[],&Setup::v3(8,1));
    assert_eq!(h.step(&["register".into(),"edge-b".into(),"edge-b".into(),"0".into(),"5".into()]).unwrap(),None);
    assert_eq!(h.step(&["top_up".into(),"edge-b".into(),"1".into(),"5".into()]).unwrap(),None,"inside the probation block");
    assert_eq!(h.step(&["top_up".into(),"edge-b".into(),"1".into(),"6".into()]).unwrap().as_deref(),Some("BadState"),"registeredAt + T_PROB");
}

#[test]
fn chain_caps_serves_exactly_the_v2_cp(){
    let read=|s:&Setup,n:u64|{let mut h=Harness::new(&[],s);let mut d=selector("get(uint256)");d.extend(U256::from(n).to_be_bytes::<32>());
        let r=call_to(&mut h.db,name("reader"),caps(),d,U256::ZERO,1);r.result.is_success().then(||U256::from_be_slice(output(&r.result)))};
    let v3=Setup::v3(64,86_400);
    assert_eq!(read(&v3,54),Some(U256::from(64)));assert_eq!(read(&v3,55),Some(U256::from(86_400)));
    assert_eq!(read(&v3,66),Some(U256::ZERO));assert_eq!(read(&v3,67),None,"slot 67 is outside CP schema v2");
    assert_eq!(read(&Setup::earnin(),51),Some(U256::ZERO));assert_eq!(read(&Setup::earnin(),52),None,"the earn-in ChainCaps keeps its 52-field bound");
    assert_eq!(pocol_lp4_execution::genesis_spec::CP_SCHEMA_V2.len(),67);
    assert_eq!((pocol_lp4_execution::genesis_spec::cp_index_for(2,"P_MAX"),pocol_lp4_execution::genesis_spec::cp_index_for(2,"T_PROB")),(Some(54),Some(55)));
}

#[test]
fn the_six_planted_mutants_fail_on_the_v3_set(){
    let list=mutants();
    assert_eq!(list.iter().map(|(n,_)|n.as_str()).collect::<Vec<_>>(),[&EARNIN_MUTANTS[..],&BUDGET_MUTANTS[..],&EVIDENCE_MUTANTS[..1],&LATE_MUTANTS[..2],&CONTEST_MUTANTS[..]].concat(),"the MinerRegistry mutants");
    let list=&list[..EARNIN_MUTANTS.len()];
    let j:serde_json::Value=serde_json::from_str(MUTANTS).unwrap();let v3:serde_json::Value=serde_json::from_str(V3).unwrap();
    assert_eq!((&j["compiler"],&j["settings"]),(&v3["compiler"],&v3["settings"]),"mutants built by the same compiler and settings");
    let original=runtime(V3,"MinerRegistry");
    for (n,code) in list{
        assert_ne!(code,&original,"{n} changes the bytecode");
        let base=earnin_on_v3("earnin",&Setup::v3(8,60_480).with_registry(code.clone()));
        let p64=earnin_on_v3("earnin-p64",&Setup::v3(64,86_400).wide().with_registry(code.clone()));
        assert!(base.is_err(),"mutant {n} survives the earn-in vectors at P_MAX 8 / T_PROB 60480");
        assert!(p64.is_err(),"mutant {n} survives the vectors at P_MAX 64 / T_PROB 86400");
        eprintln!("mutant {n}: killed ({}; {})",base.err().unwrap(),p64.err().unwrap());
    }
}

fn writes(r:&ResultAndState)->usize{r.state.get(&registry_address()).map(|a|a.storage.values().filter(|s|s.is_changed()).count()).unwrap_or(0)}
/// earn_in_write_ceilings on a program set: SSTORE counts and the gas the wallet's bond quotes are sized from.
fn ceilings(s:&Setup)->((usize,usize,usize,usize,usize),u64,u64){
    let mut h=Harness::new(&[],s);h.at(1).unwrap();
    let a=h.sender("w-a");
    let r=call(&mut h.db,a,with_arg("register(address)",Some(a)),U256::from(5*PCOL),1);assert!(r.result.is_success());let reg_partial=writes(&r);
    let r=call(&mut h.db,a,selector("topUp()"),U256::from(5*PCOL),1);assert!(r.result.is_success());let top=writes(&r);
    // The promoting top-up in the next block: V3-08 allows one maintenance operation per identity per block (the
    // earn-in set measures the same either way).
    h.at(2).unwrap();
    let r=call(&mut h.db,a,selector("topUp()"),U256::from(12*PCOL),2);assert!(r.result.is_success(),"topUp promote, 2 PCOL returned");
    let top_promote=writes(&r);let top_gas=r.result.gas_used();
    let b2=h.sender("w-b");
    let r=call(&mut h.db,b2,with_arg("register(address)",Some(b2)),U256::ZERO,2);assert!(r.result.is_success());let reg_gas=r.result.gas_used();
    let r=h.accrue_call(b2,U256::from(PCOL));assert!(r.result.is_success());let accrue=writes(&r);
    let r=h.accrue_call(b2,U256::from(30*PCOL));assert!(r.result.is_success());let accrue_promote=writes(&r);
    ((reg_partial,top,top_promote,accrue,accrue_promote),reg_gas,top_gas)
}

#[test]
fn wallet_bond_features_keep_their_write_ceilings_and_gas_on_the_v3_set(){
    let (earn_writes,earn_reg,earn_top)=ceilings(&Setup::earnin());
    for s in [Setup::v3(8,60_480),Setup::v3(64,86_400)]{
        let (writes,reg_gas,top_gas)=ceilings(&s);
        eprintln!("v3 CEILINGS {writes:?} (earn-in {earn_writes:?}) gas: register-zero={reg_gas} (earn-in {earn_reg}) topUp-promote-with-return={top_gas} (earn-in {earn_top})");
        assert_eq!(earn_writes,(11,3,5,2,4));
        // V3-08: a topUp is maintenance, so it also writes lastMaintHeight (w1 bits 224..255 since V3-PR-E4) and the
        // budget slot 9 where the earn-in set writes slot 3: one slot more for a plain topUp, and for a promoting topUp
        // (which writes w0 for the promotion anyway); registration and accrual are unchanged.
        assert_eq!(writes,(11,4,6,2,4));
        // pocold's wallet quotes EARNIN_REGISTER_GAS (400,000) for an earn-in/v3 registration and
        // REGISTRY_GAS (1,000,000) for a top-up; 200,000 is the tighter top-up bound kept from
        // earnin_contracts.rs, so both quotes keep their margin. The cfg() reads add only a few
        // thousand gas.
        assert!(reg_gas<=400_000&&top_gas<=200_000,"register {reg_gas}, topUp {top_gas}");
        // The difference is the cfg(P_MAX) read (V3-14) and the cold read of the budget slot 9 (2,100 gas, V3-08), which
        // a registration needs for REG_OPS less the maintenance used.
        assert!(reg_gas>=earn_reg&&reg_gas-earn_reg<=7_500,"register gas {reg_gas} vs earn-in {earn_reg}");
    }
}

#[test]
fn v3_sys_code_hash_is_pinned_and_selected_only_on_cp_schema_v2(){
    let v3=SystemPrograms::v3();
    assert_eq!(hex(&v3.sys_code_hash().0),V3_SYS_CODE_HASH);
    // The existing sets' hashes are unchanged (spec-1 genesis bytes commit them).
    let (v1,continuous,earnin)=(SystemPrograms::v1().sys_code_hash(),SystemPrograms::continuous().sys_code_hash(),SystemPrograms::earnin().sys_code_hash());
    assert!(![v1,continuous,earnin].contains(&v3.sys_code_hash()));
    // MinerRegistry, RewardEscrow (V3-16/V3-17), ShareLedger (V3-02 recordLate) and ChainCaps differ from the earn-in
    // set; ChunkFactory is the same bytes.
    let e=SystemPrograms::earnin();
    assert!(v3.codes[4]==e.codes[4]);
    assert!(v3.codes[0]!=e.codes[0]&&v3.codes[1]!=e.codes[1]&&v3.codes[2]!=e.codes[2]&&v3.codes[3]!=e.codes[3]);
    // EIP-170-sized for tooling (roadmap risk 10).
    for (i,c) in v3.codes.iter().enumerate(){assert!(c.len()<=24_576,"program {i} is {} bytes",c.len());}
    let mut cfg=DevnetConfig::new(777940,1_700_000_000,&["one"]);cfg.h_end=0;cfg.earnin=true;
    let spec1=ConsensusParams::from_values(&cfg.cp()).unwrap();cfg.spec_version=2;let spec2=ConsensusParams::from_values(&cfg.cp()).unwrap();
    assert_eq!((spec1.schema_version(),spec2.schema_version()),(1,2));
    let hash=v3.sys_code_hash().0;
    assert_eq!(SystemPrograms::program_set(&spec2,&hash),ProgramSet::V3);
    assert_eq!(SystemPrograms::for_spec(&spec2,&hash).sys_code_hash(),v3.sys_code_hash());
    // On a spec-1 CP the v3 hash names nothing: the CP default, whose hash then fails the genesis code check.
    assert_eq!(SystemPrograms::program_set(&spec1,&hash),ProgramSet::Continuous);
    assert_eq!(SystemPrograms::for_spec(&spec1,&hash).sys_code_hash(),continuous);
    for (kind,h) in [(ProgramSet::V1,v1),(ProgramSet::Continuous,continuous),(ProgramSet::EarnIn,earnin)]{
        for cp in [&spec1,&spec2]{assert_eq!(SystemPrograms::program_set(cp,&h.0),kind);assert_eq!(SystemPrograms::for_spec(cp,&h.0).sys_code_hash(),h);}
    }
    assert_eq!(ProgramSet::EarnIn.probation(&spec2),Some(Probation{seats:8,blocks:60_480}));
    assert_eq!(ProgramSet::V3.probation(&spec2),Some(Probation{seats:u64::from(spec2.P_MAX),blocks:u64::from(spec2.T_PROB)}));
    assert_eq!((ProgramSet::V1.probation(&spec1),ProgramSet::Continuous.probation(&spec1)),(None,None));
}

/// The v3 contracts read CP fields 52..66 as 0 on a 52-field CP (P_MAX 0, T_PROB 0), so nothing in the programs
/// themselves keeps them off a spec-1 genesis: the registry gate does. A spec-1 GenesisSpec that names the v3
/// sysCodeHash, with the allocRoot the v3 programs would give it, is built here and refused by every path that
/// turns a GenesisSpec into a genesis (admission's measurement and the gate itself), with the genesis code check.
#[test]
fn a_spec1_genesis_naming_the_v3_set_is_refused_by_admission(){
    use pocol_lp4_execution::{admission::{self,NodeLocal},apply::Genesis,devnet,genesis_spec as gs,profile::{ProfileIdentity,ProfileKind,NodeKind}};
    let mut cfg=DevnetConfig::new(777942,1_700_000_000,&["one"]);cfg.h_end=0;cfg.earnin=true;
    let honest=devnet::build(&cfg).unwrap();
    assert_eq!(honest.programs.sys_code_hash(),SystemPrograms::earnin().sys_code_hash(),"a spec-1 earn-in devnet runs the earn-in set");
    let local=NodeLocal::single_host();
    // Control: the honest candidate passes the same paths.
    let timing=admission::measure(&honest.spec_bytes,&honest.alloc,1).unwrap();
    admission::admit(&honest.request(&local,Some(&timing))).unwrap_or_else(|e|panic!("honest spec-1 earn-in candidate: {e}"));

    let mut spec=gs::decode(&honest.spec_bytes).unwrap();assert_eq!(spec.spec_version,1);
    let v3=SystemPrograms::v3();spec.sys_code_hash=v3.sys_code_hash().0;
    let identity=|s:&gs::GenesisSpec|{let b=gs::encode(s);ProfileIdentity::validate(&b,&gs::genesis_hash(&b),s.chain_id,ProfileKind::Simnet,NodeKind::Simnet,&honest.forks).unwrap()};
    // The v3 programs themselves would construct this genesis (only the registry stands in the way).
    spec.alloc_root=Genesis::derive_root(&honest.alloc,&identity(&spec),&v3).expect("the v3 programs construct on a spec-1 CP").0;
    let cp=ConsensusParams::from_values(&spec.cp).unwrap();assert_eq!(cp.schema_version(),1);
    assert_eq!(SystemPrograms::program_set(&cp,&spec.sys_code_hash),ProgramSet::Continuous,"the v3 hash names nothing on CP schema v1");
    let forged=devnet::Devnet{spec_bytes:gs::encode(&spec),..honest};
    let measured=admission::measure(&forged.spec_bytes,&forged.alloc,1).unwrap_err();
    assert!(measured.contains("Code"),"measurement: {measured}");
    let refused=match admission::admit(&forged.request(&local,None)){Err(e)=>e.to_string(),Ok(_)=>panic!("a spec-1 genesis naming the v3 set was admitted")};
    eprintln!("spec-1 + v3 hash: measurement {measured}; admission {refused}");
    assert!(refused.contains("R24")&&refused.contains("Code"),"admission: {refused}");
}

// ---------------- V3-PR-E1: registry budgets (V3-08, V3-09)

/// CP schema names of tests/v3_model.py's CP_INDEX and CHAIN_CP_INDEX.
fn cp_index(n:&str)->u64{
    match n{"D"=>11,"R_max"=>17,"M_max"=>18,"M_min"=>19,"SWEEP_MAX"=>21,"SWEEP_SCAN"=>22,"REG_OPS"=>23,"W_act"=>24,"Theta_inact"=>25,
        "E_win"=>26,"U"=>27,"P_MAX"=>54,"T_PROB"=>55,"R_react"=>61,"C_react"=>62,"E_epoch"=>63,"theta_bp"=>64,"k_sigma"=>65,
        "m"=>8,"kappa"=>9,"S_max"=>10,"W_w"=>12,"alpha_bp"=>13,"gamma_bp"=>14,"S0"=>15,"H_half"=>52,"S_tail"=>53,"gamma_tip_bp"=>59,"rho_bp"=>66,
        o=>panic!("unknown CP field {o}")}
}
fn strs(v:&[&str])->Vec<String>{v.iter().map(|x|x.to_string()).collect()}
fn ids_of(db:&InMemoryDB,base_slot:u64,len_slot:u64)->Vec<Address>{
    (0..reg(db,len_slot).to::<u64>()).map(|i|Address::from_word(storage(db,registry_address(),base(base_slot)+U256::from(i)).into())).collect()
}
/// Bytecode-only conservation, independent of any model: ACC_COUNT (slot 8) is the number of live ACCRUING records
/// and held_total (slot 4) the sum of live bonds, over the record array.
fn conserved(h:&Harness)->Result<(),String>{
    let (mut accruing,mut held)=(0u64,U256::ZERO);
    for id in ids_of(&h.db,5,0){
        let (w0,_,w2,_)=h.record(id);let st=bits(w0,16,8);
        if bits(w0,0,8)!=0&&bits(w0,8,8)==0&&(st==1||st==4){held+=w2;if st==4{accruing+=1}}
    }
    ensure_eq!(reg(&h.db,8).to::<u64>(),accruing,"ACC_COUNT");ensure_eq!(reg(&h.db,4),held,"held_total");
    Ok(())
}
/// One scenario of registry-budget-vectors.json or evidence-penalty-vectors.json on a program set; Err names the first
/// difference from the model. A chain scenario (one whose final state has pots) also checks the Escrow invariant and
/// the supply after every step, and every pending pot at the end.
fn budget_scenario(name_:&str,body:&serde_json::Value,base_setup:&Setup)->Result<(),String>{
    budget_run(name_,body,base_setup).map(|_|())
}
fn budget_run(name_:&str,body:&serde_json::Value,base_setup:&Setup)->Result<Harness,String>{
    let mut s=base_setup.clone();
    for (k,v) in body["cp"].as_object().unwrap(){s.cp.push((cp_index(k),v.as_str().map_or_else(||v.as_u64().unwrap().into(),|x|x.parse::<u128>().unwrap())));}
    if s.cp.iter().any(|(n,_)|*n>=s.cp_len){return Err(format!("{name_}: a CP field outside this program set's ChainCaps"))}
    let durable:Vec<String>=body["durable"].as_array().unwrap().iter().map(|x|x.as_str().unwrap().to_string()).collect();
    let mut h=Harness::new(&durable,&s);
    let chain=body["final"].get("pots").is_some();
    for step in body["steps"].as_array().unwrap(){
        let op:Vec<String>=step["op"].as_array().unwrap().iter().map(|x|x.as_str().unwrap().to_string()).collect();
        let got=match op[0].as_str(){"begin"=>h.begin(step)?,"close"|"final"=>{h.phase2(step).map_err(|e|format!("{name_}: {e}"))?;None},_=>h.step(&op)?};
        match (&got,step["ok"].as_bool().unwrap()){
            (None,true)=>{}
            (Some(e),false)=>ensure_eq!(e.as_str(),step["revert"].as_str().unwrap(),"{name_} {op:?}"),
            _=>return Err(format!("{name_} {op:?}: contract {got:?}, model {step}")),
        }
        if op[0]=="contest_status"{let got=h.view.take();ensure_eq!(got.as_deref(),step["result"].as_str(),"{name_} {op:?}: contestStatus (code, count, threshold)");}
        conserved(&h).map_err(|e|format!("{name_} after {op:?}: {e}"))?;
        if chain{h.chain_conserved().map_err(|e|format!("{name_} after {op:?}: {e}"))?;}
    }
    let fin=&body["final"];
    ensure_eq!(reg(&h.db,4).to_string(),fin["heldTotal"].as_str().unwrap(),"{name_} heldTotal");
    ensure_eq!(reg(&h.db,8).to::<u64>(),fin["accruing"].as_u64().unwrap(),"{name_} accruing");
    let burned=storage(&h.db,escrow_address(),U256::ZERO)+h.burned;
    ensure_eq!(burned.to_string(),fin["burned"].as_str().unwrap(),"{name_} burned");
    let active:BTreeSet<Address>=ids_of(&h.db,6,1).into_iter().collect();
    let want:BTreeSet<Address>=fin["active"].as_array().unwrap().iter().map(|x|name(x.as_str().unwrap())).collect();
    ensure_eq!(active,want,"{name_} active set");
    let order:Vec<Address>=fin["order"].as_array().unwrap().iter().map(|x|name(x.as_str().unwrap())).collect();
    ensure_eq!(ids_of(&h.db,5,0),order,"{name_} record array order");
    ensure_eq!(reg(&h.db,2).to::<u64>(),fin["cursor"].as_u64().unwrap(),"{name_} sweep cursor");
    // Per-block counters at the last height: slot 3 (REG_OPS and R_max users) and slot 9 (maintenance, reactivations).
    let (c,b)=(reg(&h.db,3),reg(&h.db,9));let current=bits(b,16,64)==h.height;
    let got=(bits(c,0,16),bits(c,16,8),if current{bits(b,0,8)}else{0},if current{bits(b,8,8)}else{0});
    let k=&fin["counters"];
    ensure_eq!(got,(k["ops"].as_u64().unwrap(),k["regs"].as_u64().unwrap(),k["maint"].as_u64().unwrap(),k["react"].as_u64().unwrap()),"{name_} counters");
    // V3-10: the epoch totals (slot 10: epoch 0..63, its total 64..159, the previous epoch's 160..255).
    let t=reg(&h.db,10);let tw=&fin["epochTotals"];
    ensure_eq!((bits(t,0,64),bits(t,64,96),bits(t,160,96)),(tw["epoch"].as_u64().unwrap(),tw["cur"].as_u64().unwrap(),tw["prev"].as_u64().unwrap()),"{name_} epoch totals");
    let records=fin["records"].as_object().unwrap();
    for (n,r) in records{
        let (w0,w1,w2,w3)=h.record(name(n));
        let got=(bits(w0,0,8),bits(w0,16,8),w2.to_string(),bits(w0,8,8)!=0,bits(w0,152,64),Address::from_word(w1.into()),
            (bits(w3,0,64),bits(w3,64,64),bits(w3,128,64),bits(w3,192,64)),bits(w0,88,64),(bits(w1,224,32),bits(w1,160,64)),(bits(w0,216,20),bits(w0,236,20)));
        let u=|k:&str|r[k].as_u64().unwrap();
        let want=(u("status"),u("bond_state"),r["bond"].as_str().unwrap().to_string(),r["tomb"].as_bool().unwrap(),u("registered_at"),name(r["reward"].as_str().unwrap()),
            (u("exit"),u("due"),u("burn"),u("grace")),u("last_share"),(u("last_maint"),u("last_react")),(u("cnt_cur"),u("cnt_prev")));
        ensure_eq!(got,want,"{name_} record {n}");
        // w1 is exactly the reward address, lastReactHeight (160..223) and lastMaintHeight mod 2^32 (224..255).
        let w1_want=U256::from_be_slice(name(r["reward"].as_str().unwrap()).as_slice())|(U256::from(u("last_react"))<<160u32)|(U256::from(u("last_maint"))<<224u32);
        ensure_eq!(w1,w1_want,"{name_} record {n}: w1 holds only the reward address and the two clocks");
    }
    // An erased record leaves no word behind, clocks included (SYS_SLOT_MAX counts nonzero slots), and no fifth word
    // is ever written.
    for a in &h.senders{
        if !records.keys().any(|n|name(n)==*a){
            for k in 0..4u64{ensure!(storage(&h.db,registry_address(),slot(*a)+U256::from(k)).is_zero(),"{name_}: erased record keeps word {k}");}
        }
        ensure!(storage(&h.db,registry_address(),slot(*a)+U256::from(4)).is_zero(),"{name_}: a fifth record word");
    }
    // Payments to a reward address that never sent a transaction: its balance gain plus the harness's settled payouts.
    // A sender's refunds are checked through its net PCOL below.
    for (n,amount) in fin["paid"].as_object().unwrap(){
        let a=name(n);if h.senders.contains(&a){continue}
        let refunds=balance(&h.db,a)-h.start.get(&a).copied().unwrap_or_default();
        ensure_eq!((refunds+h.settled.get(&a).copied().unwrap_or_default()).to_string(),amount.as_str().unwrap(),"{name_} paid {n}");
    }
    for a in &h.senders{
        let n=records.keys().chain(fin["spent"].as_object().unwrap().keys()).chain(fin["paid"].as_object().unwrap().keys()).find(|n|name(n)==*a).cloned();
        let Some(n)=n else{ensure_eq!(h.start[a],balance(&h.db,*a),"{name_}: a sender with no bond moved PCOL");continue};
        let field=|k:&str|fin[k].get(&n).and_then(|x|x.as_str()).map(|x|x.parse::<U256>().unwrap()).unwrap_or_default();
        let refunds=field("paid")-h.settled.get(a).copied().unwrap_or_default();
        ensure_eq!(h.start[a]-balance(&h.db,*a),field("spent")-refunds,"{name_} sender {n} net PCOL");
    }
    if let Some(pots)=fin.get("pots").and_then(|p|p.as_object()){
        // Every pending pot: amount, proposer and kind, winner, the V3-16 ACCRUING bit (winner word bit 160) and INVALID
        // (meta bit 168); every other height holds no pot.
        for j in 1..=h.height{
            let (amount,meta,winner)=pot(&h.db,j);
            match pots.get(&j.to_string()){
                None=>ensure!(amount.is_zero()&&meta.is_zero()&&winner.is_zero(),"{name_}: pot {j} should be settled"),
                Some(p)=>{
                    let proposer=p["proposer"].as_str().unwrap();
                    let want_meta=if proposer.is_empty(){U256::from(1)}else{U256::from_be_slice(name(proposer).as_slice())<<8};
                    let want_winner=match p["winner"].as_str().unwrap(){""=>U256::ZERO,w=>U256::from_be_slice(name(w).as_slice())};
                    let one=U256::from(1);let got=(amount.to_string(),meta&!(one<<168u32),meta>>168u32,winner&((one<<160u32)-one),winner>>160u32);
                    let want=(p["amount"].as_str().unwrap().to_string(),want_meta,U256::from(p["invalid"].as_bool().unwrap() as u8),want_winner,U256::from(p["accruing"].as_bool().unwrap() as u8));
                    ensure_eq!(got,want,"{name_} pot {j}");
                }
            }
        }
        ensure_eq!(balance(&h.db,escrow_address()).to_string(),fin["escrowBalance"].as_str().unwrap(),"{name_} Escrow balance");
        ensure_eq!(h.issued.to_string(),fin["issued"].as_str().unwrap(),"{name_} issuance");
        // V3-15: every outstanding carry by the pot it goes into (slot 4 + (t & 1)); the other slot is empty, and slot 2
        // (the unlagged carry of an earlier draft) is never written.
        let want:BTreeMap<u64,U256>=fin["carry"].as_object().unwrap().iter().map(|(t,v)|(t.parse().unwrap(),v.as_str().unwrap().parse().unwrap())).collect();
        ensure!(want.keys().all(|t|*t==h.height+1),"{name_}: the model carries into {want:?} after block {}",h.height);
        for t in [h.height,h.height+1]{ensure_eq!(carry_into(&h.db,t),want.get(&t).copied().unwrap_or_default(),"{name_} carry into pot {t}");}
        ensure!(storage(&h.db,escrow_address(),U256::from(2)).is_zero(),"{name_}: slot 2 written");
    }
    if let Some(ledger_)=fin.get("ledger").and_then(|l|l.as_object()){
        // V3-02: every retained ShareLedger height: its total and each owner's count; pruned heights hold nothing.
        for j in 1..=h.height{
            let total=storage(&h.db,ledger(),ledger_slot(j,0));
            match ledger_.get(&j.to_string()){
                None=>ensure!(total.is_zero(),"{name_}: ledger height {j} should be pruned"),
                Some(l)=>{
                    ensure_eq!(total.to::<u64>(),l["total"].as_u64().unwrap(),"{name_} ledger total {j}");
                    for (o,c) in l["counts"].as_object().unwrap(){
                        let slot=U256::from_be_bytes(keccak256([name(o).into_word().as_slice(),ledger_slot(j,1).to_be_bytes::<32>().as_slice()].concat()).0);
                        ensure_eq!(storage(&h.db,ledger(),slot).to::<u64>(),c.as_u64().unwrap(),"{name_} ledger count {j} {o}");
                    }
                }
            }
        }
    }
    Ok(h)
}
fn budget_replay(s:&Setup)->BTreeMap<String,Result<(),String>>{
    let v:serde_json::Value=serde_json::from_str(BUDGET_VECTORS).unwrap();
    v.as_object().unwrap().iter().map(|(n,body)|(n.clone(),budget_scenario(n,body,s))).collect()
}
const BUDGET_SCENARIOS:[&str;12]=["acc-count-paths","attacker-cycle","maintenance-budget-many-identities","maintenance-clock-per-record",
    "maintenance-spam-one-identity","maintenance-top-up","random-1","random-2","random-3","react-budget-and-cooldown","react-window-unswept",
    "reg-ops-exhausted-maint-and-react"];

/// Model-vs-bytecode differential: every scenario of the independent model reproduces on the v3 programs, step by
/// step, with ACC_COUNT and held_total conserved after every step.
#[test]
fn v3_registry_budgets_reproduce_the_independent_model(){
    let out=budget_replay(&Setup::v3(8,60_480));
    assert_eq!(out.keys().map(|k|k.as_str()).collect::<Vec<_>>(),BUDGET_SCENARIOS);
    for (n,r) in &out{if let Err(e)=r{panic!("{n}: {e}")}}
    // The vectors exercise what they claim: every budget refusal, the cooldown and the window.
    let v:serde_json::Value=serde_json::from_str(BUDGET_VECTORS).unwrap();
    let mut seen=BTreeMap::<(String,String),usize>::new();
    for body in v.as_object().unwrap().values(){for st in body["steps"].as_array().unwrap(){
        let key=(st["op"][0].as_str().unwrap().to_string(),st.get("revert").and_then(|x|x.as_str()).unwrap_or("ok").to_string());*seen.entry(key).or_default()+=1;
    }}
    for (op,res) in [("set_reward","RegOpsLimit"),("top_up","RegOpsLimit"),("register","RegOpsLimit"),("deregister","RegOpsLimit"),("reactivate","RegOpsLimit"),
        ("reactivate","BadState"),("reactivate","ok"),("claim_bond","ok"),("remove_inactive","ok"),("punish","ok"),("top_up","ok"),("set_reward","ok"),("settle_to","ok")]{
        assert!(seen.get(&(op.to_string(),res.to_string())).is_some_and(|n|*n>0),"no {op} -> {res} in the vectors");
    }
}

/// The same vectors on the earn-in set: each budget scenario differs from the v3 model there (the CP schema v2
/// fields do not exist on its 52-field ChainCaps, so P_MAX/T_PROB/R_react/C_react scenarios run at its constants).
#[test]
fn registry_budget_vectors_fail_on_the_earnin_set(){
    let mut earnin=Setup::earnin();earnin.cp_len=52;
    let v:serde_json::Value=serde_json::from_str(BUDGET_VECTORS).unwrap();
    for (n,body) in v.as_object().unwrap(){
        // Run with the v1 CP fields only; the v2 ones are what the earn-in programs cannot read.
        let mut b=body.clone();b["cp"].as_object_mut().unwrap().retain(|k,_|cp_index(k)<52);
        let r=budget_scenario(n,&b,&earnin);
        eprintln!("earn-in {n}: {}",r.as_ref().err().map_or("reproduces",|e|e.as_str()));
        assert!(r.is_err(),"{n} reproduces on the earn-in set: it does not test V3-08/V3-09");
    }
}

/// V3-DESIGN.md V3-08, fails on the original: one HELD identity calls setRewardAddress 16 times in a block (REG_OPS
/// 16, R_max 1 as live). On the earn-in set every call succeeds and then another identity's registration and the
/// spammer's own eviction revert RegOpsLimit; on v3 only the first call succeeds, the registration lands and the
/// spammer is still evictable in the same block.
#[test]
fn one_identity_spamming_set_reward_address_freezes_earnin_joins_but_not_v3(){
    for (v3,mut s) in [(false,Setup::earnin()),(true,Setup::v3(8,60_480))]{
        s.cp.extend([(23,16),(17,1)]);
        let mut h=Harness::new(&strs(&["x"]),&s);
        assert_eq!(h.step(&strs(&["register","s","s",&b().to_string(),"1"])).unwrap(),None);
        let spam:Vec<Option<String>>=(0..16).map(|_|h.step(&strs(&["set_reward","s","x","100"])).unwrap()).collect();
        let join=h.step(&strs(&["register","n","n",&b().to_string(),"100"])).unwrap();
        let evict=h.step(&strs(&["remove_inactive","n","s","100"])).unwrap();
        eprintln!("v3={v3}: spam {spam:?} join {join:?} evict {evict:?}");
        if v3{
            assert_eq!(spam[0],None);assert!(spam[1..].iter().all(|r|r.as_deref()==Some("RegOpsLimit")),"one maintenance operation per identity per block");
            assert_eq!((join,evict),(None,None),"the join lands and the spammer is evicted");
            assert_eq!(bits(h.record(name("s")).0,0,8),2,"the spammer exited");
        }else{
            assert!(spam.iter().all(|r|r.is_none()),"the earn-in set spends REG_OPS on maintenance");
            assert_eq!((join.as_deref(),evict.as_deref()),(Some("RegOpsLimit"),Some("RegOpsLimit")),"the earn-in registry is frozen for the block");
        }
    }
}

/// V3-DESIGN.md V3-09, fails on the original: an attacker with W_act+1 = 33 records deregisters one and reactivates
/// another every block, its transactions first (R_max 1). On the earn-in set the reactivation takes the entry slot,
/// so a newcomer's registration reverts in every block of the cycle; on v3 a newcomer registers in every block
/// (k = 1), and each attacker record's second reactivation inside C_react (= U = 40 > 33) reverts, so the cycle
/// cannot be kept up beyond one pass.
#[test]
fn deregister_reactivate_cycle_holds_the_entry_slot_on_earnin_but_not_v3(){
    let (records,start,attack)=(33u64,40u64,40u64);let first=start+records-1;
    for (v3,mut s) in [(false,Setup::earnin()),(true,Setup::v3(8,60_480))]{
        s.cp.extend([(23,16),(17,1),(18,128)]);
        let mut h=Harness::new(&[],&s);
        for i in 0..records{assert_eq!(h.step(&strs(&["register",&format!("a{i}"),&format!("a{i}"),&b().to_string(),&(1+i).to_string()])).unwrap(),None);}
        for t in start..first{assert_eq!(h.step(&strs(&["deregister",&format!("a{}",(t-start)%records),&t.to_string()])).unwrap(),None);}
        let (mut joined,mut refused,mut reacts)=(Vec::new(),Vec::new(),Vec::new());
        for t in first..first+attack{
            let _=h.step(&strs(&["deregister",&format!("a{}",(t-start)%records),&t.to_string()])).unwrap();
            reacts.push(h.step(&strs(&["reactivate",&format!("a{}",(t-start+1)%records),&t.to_string()])).unwrap());
            match h.step(&strs(&["register",&format!("n{t}"),&format!("n{t}"),&b().to_string(),&t.to_string()])).unwrap(){
                None=>joined.push(t),Some(e)=>{assert_eq!(e,"RegOpsLimit");refused.push(t)}
            }
        }
        let first_pass=reacts[..records as usize].iter().filter(|r|r.is_none()).count();
        let later=reacts[records as usize..].iter().filter(|r|r.is_none()).count();
        eprintln!("v3={v3}: newcomers joined {} refused {}; attacker reactivations first pass {first_pass}, later {later}",joined.len(),refused.len());
        if v3{
            assert_eq!(joined,(first..first+attack).collect::<Vec<_>>(),"a newcomer registers in every block of the cycle");
            assert_eq!(first_pass,records as usize);
            assert_eq!(later,0,"a second reactivation within C_react reverts");
            assert!(reacts[records as usize..].iter().all(|r|r.as_deref()==Some("BadState")));
        }else{
            assert!(joined.is_empty(),"the attacker holds the earn-in entry slot: {joined:?}");
            assert_eq!(first_pass+later,attack as usize,"every attacker reactivation succeeds on the earn-in set");
        }
    }
}

/// The seven planted V3-08/V3-09 mutants (compile_system_v3.cjs --mutants) each fail the budget vectors. The two that
/// also bound maintenance or reactivate by REG_OPS (the budgets are independent, D-V3-08/09) fail the scenario that
/// runs maintenance and reactivations after REG_OPS is used up.
#[test]
fn the_registry_budget_mutants_fail_the_v3_vectors(){
    let list=mutants();let original=runtime(V3,"MinerRegistry");
    let budget=&list[EARNIN_MUTANTS.len()..EARNIN_MUTANTS.len()+BUDGET_MUTANTS.len()];
    assert_eq!(budget.iter().map(|(n,_)|n.as_str()).collect::<Vec<_>>(),BUDGET_MUTANTS);
    for (n,code) in budget{
        assert_ne!(code,&original,"{n} changes the bytecode");
        let out=budget_replay(&Setup::v3(8,60_480).with_registry(code.clone()));
        let killed:Vec<&String>=out.iter().filter(|(_,r)|r.is_err()).map(|(k,_)|k).collect();
        eprintln!("mutant {n}: killed by {killed:?}");
        assert!(!killed.is_empty(),"mutant {n} survives the registry-budget vectors");
        if n.ends_with("-reg-ops"){assert!(killed.iter().any(|k|k.as_str()=="reg-ops-exhausted-maint-and-react"),"mutant {n}: {killed:?}");}
    }
}

/// ParamGate's v3 premises hold for the harness CP the vectors run at (lp1-node/src/gate.rs R8 on CP schema v2):
/// U >= W_act + 2 and C_react >= U, so the reactivation window [exit+W_act, exit+U) holds at least two blocks.
#[test]
fn budget_vectors_run_at_gate_valid_parameters(){
    let v:serde_json::Value=serde_json::from_str(BUDGET_VECTORS).unwrap();
    for (n,body) in v.as_object().unwrap(){
        let cp=|k:&str,d:u64|body["cp"].get(k).and_then(|x|x.as_u64()).unwrap_or(d);
        let (w_act,u,c_react)=(cp("W_act",32),cp("U",40),cp("C_react",40));
        assert!(u>=w_act+2&&c_react>=u,"{n}: W_act {w_act} U {u} C_react {c_react}");
    }
}

/// Changed Registry slots of one successful call (the earn_in_write_ceilings measure).
fn changed(r:&ResultAndState,what:&str)->usize{assert!(r.result.is_success(),"{what}: {:?} {:?}",r.result,output(&r.result));writes(r)}
/// Every user path on a program set: (register, setRewardAddress, deregister middle, deregister tail, reactivate,
/// removeInactive tail, claimBond middle, claimBond tail) changed slots, and the largest gas of a registration and of
/// any other user path.
fn user_path_writes(s:&Setup)->([usize;8],u64,u64){
    let mut s=s.clone();s.cp.extend([(23,64),(17,16)]);
    let mut h=Harness::new(&strs(&["x"]),&s);h.at(1).unwrap();
    let ids:Vec<Address>=(0..6).map(|i|h.sender(&format!("u{i}"))).collect();
    let mut gas=0u64;let mut go=|h:&mut Harness,who:Address,data:Vec<u8>,value:U256,what:&str|{let r=call(&mut h.db,who,data,value,h.height);gas=gas.max(r.result.gas_used());changed(&r,what)};
    let r=call(&mut h.db,ids[0],with_arg("register(address)",Some(ids[0])),b(),1);let reg_gas=r.result.gas_used();let register=changed(&r,"register");
    for a in &ids[1..]{let r=call(&mut h.db,*a,with_arg("register(address)",Some(*a)),b(),1);changed(&r,"register");}
    h.at(2).unwrap();
    let reward=go(&mut h,ids[1],with_arg("setRewardAddress(address)",Some(name("x"))),U256::ZERO,"setRewardAddress");
    for a in &ids[2..5]{go(&mut h,*a,with_arg("setRewardAddress(address)",Some(*a)),U256::ZERO,"setRewardAddress");}
    // ActList: m0,u0..u5. u1 is in the middle; u5 is the tail.
    let dereg_mid=go(&mut h,ids[1],selector("deregister()"),U256::ZERO,"deregister middle");
    let last=*apply_active(&h).last().unwrap();
    let dereg_tail=go(&mut h,last,selector("deregister()"),U256::ZERO,"deregister tail");
    h.at(34).unwrap();
    let react=go(&mut h,ids[1],selector("reactivate()"),U256::ZERO,"reactivate");
    h.at(100).unwrap();
    let last=*apply_active(&h).last().unwrap();
    let remove_tail=go(&mut h,ids[0],with_arg("removeInactive(address)",Some(last)),U256::ZERO,"removeInactive tail");
    // claimBond needs a record the sweep has not refunded at its due height: a harness whose beginBlock deletes
    // nothing (SWEEP_MAX 0). The records do maintenance first, so on v3 their clocks are set when erased.
    let mut s2=s.clone();s2.cp.push((21,0));let mut h2=Harness::new(&strs(&["x"]),&s2);h2.at(1).unwrap();
    let c:Vec<Address>=(0..3).map(|i|h2.sender(&format!("c{i}"))).collect();
    for a in &c{let r=call(&mut h2.db,*a,with_arg("register(address)",Some(*a)),b(),1);changed(&r,"register");}
    h2.at(2).unwrap();
    for a in &c{let r=call(&mut h2.db,*a,with_arg("setRewardAddress(address)",Some(*a)),U256::ZERO,2);changed(&r,"reward");}
    for a in &c{let r=call(&mut h2.db,*a,selector("deregister()"),U256::ZERO,2);changed(&r,"deregister");}
    h2.at(42).unwrap();
    // RecArr: m0,c0,c1,c2. c0 is in the middle (c2 moves into its place); c1 is then the tail.
    let r=call(&mut h2.db,c[0],selector("claimBond()"),U256::ZERO,42);gas=gas.max(r.result.gas_used());let claim_mid=changed(&r,"claimBond middle");
    let tail=*ids_of(&h2.db,5,0).last().unwrap();
    let r=call(&mut h2.db,tail,selector("claimBond()"),U256::ZERO,42);gas=gas.max(r.result.gas_used());let claim_tail=changed(&r,"claimBond tail");
    ([register,reward,dereg_mid,dereg_tail,react,remove_tail,claim_mid,claim_tail],reg_gas,gas)
}
fn apply_active(h:&Harness)->Vec<Address>{ids_of(&h.db,6,1)}

/// Write ceilings (as earn_in_write_ceilings) of every user path on the v3 set, against the earn-in set: V3-08/V3-09
/// add one slot exactly where REGISTRY-LAYOUT.md says (w1 on reactivate; slot 9 replaces slot 3 on maintenance and
/// reactivation; setRewardAddress's clock shares w1 with the reward address), and nothing elsewhere.
#[test]
fn v3_registry_write_ceilings(){
    let (earn,earn_reg,earn_gas)=user_path_writes(&Setup::earnin());let (v3,v3_reg,v3_gas)=user_path_writes(&Setup::v3(8,60_480));
    eprintln!("WRITE-CEILINGS register,reward,deregister-middle,deregister-tail,reactivate,removeInactive-tail,claim-middle,claim-tail earn-in {earn:?} v3 {v3:?}; gas register earn-in {earn_reg} v3 {v3_reg}, other paths earn-in {earn_gas} v3 {v3_gas}");
    // The earn-in counts are REGISTRY-LAYOUT.md's register10, reward2, deactivate middle7/tail5, reactivate5,
    // claim middle10/tail8.
    assert_eq!(earn,[10,2,7,5,5,5,10,8]);
    // V3-08/V3-09: setRewardAddress writes w1 (reward and lastMaintHeight, w1 bits 224..255 since V3-PR-E4) and slot 9 in
    // place of slot 3, so as many slots as on earn-in; reactivate +1 (w1 for lastReactHeight; slot 9 in place of slot 3);
    // register, deregister, removeInactive and claimBond unchanged.
    assert_eq!(v3,[10,2,7,5,6,5,10,8]);
    // pocold's wallet quotes EARNIN_REGISTER_GAS (400,000) for a registration and REGISTRY_GAS (1,000,000) for
    // every other registry call.
    assert!(v3_reg<=400_000&&v3_gas<=200_000,"v3 register {v3_reg}, largest other user path {v3_gas} gas");
}

/// The beginBlock sweep at its program caps (64 scans, 16 deletions) deleting records that carry clocks: the same
/// changed slots per deletion as earn-in (the clocks sit in words erase clears anyway) and its gas, which stays
/// inside admission's GAS_ENVELOPE (the per-call maxima R18 composes). A fifth record word would put the worst case
/// above that envelope; this pins that it is not there.
#[test]
fn v3_sweep_deletions_keep_the_sys_envelope(){
    let envelope=pocol_lp4_execution::admission::GAS_ENVELOPE.iter().find(|(n,_)|*n=="beginBlock").unwrap().1;
    let mut out=Vec::new();
    for s in [Setup::earnin().wide(),Setup::v3(8,60_480).wide()]{
        let mut h=Harness::new(&[],&s);
        let n=80u64;let ids:Vec<Address>=(0..n).map(|i|h.sender(&format!("sw{i}"))).collect();
        h.at(1).unwrap();
        for a in &ids{let r=call(&mut h.db,*a,with_arg("register(address)",Some(*a)),b(),1);changed(&r,"register");}
        // Maintenance (8 per block on v3) sets every record's lastMaintHeight.
        for (i,a) in ids.iter().enumerate(){h.at(2+i as u64/8).unwrap();let r=call(&mut h.db,*a,with_arg("setRewardAddress(address)",Some(*a)),U256::ZERO,h.height);changed(&r,"reward");}
        let t=h.height+1;h.at(t).unwrap();
        for a in &ids{let r=call(&mut h.db,*a,selector("deregister()"),U256::ZERO,t);changed(&r,"deregister");}
        // The next beginBlock at due height deletes 16 records with refunds.
        let due=t+40;
        let r=call(&mut h.db,system(),selector("markClose()"),U256::ZERO,due);assert!(r.result.is_success());
        let r=call(&mut h.db,system(),selector("beginBlock()"),U256::ZERO,due);assert!(r.result.is_success(),"{:?}",r.result);
        let removed=n+1-reg(&h.db,0).to::<u64>();assert_eq!(removed,16,"16 deletions per beginBlock");
        out.push((writes(&r),r.result.gas_used()));
        conserved(&h).unwrap();
    }
    eprintln!("SWEEP 16 deletions: earn-in {:?} v3 {:?}; GAS_ENVELOPE beginBlock {envelope}",out[0],out[1]);
    assert_eq!(out[1].0,out[0].0,"the same changed slots per deletion");
    assert!(out[1].1<=envelope,"v3 beginBlock {} gas above the admission envelope {envelope}",out[1].1);
    // The v3 worst case at the program caps: all 64 scans and 16 deletions, the first 48 scans over active probation
    // records (each reads T_PROB through cfg), then 16 refunds of ACCRUING records with clocks (each also lowers
    // ACC_COUNT).
    let mut h=Harness::new(&[],&Setup::v3(64,86_400).wide());h.at(1).unwrap();
    let probation:Vec<Address>=(0..47).map(|i|h.sender(&format!("pp{i}"))).collect();
    let held:Vec<Address>=(0..16).map(|i|h.sender(&format!("ph{i}"))).collect();
    for a in &probation{let r=call(&mut h.db,*a,with_arg("register(address)",Some(*a)),U256::ZERO,1);changed(&r,"register probation");}
    for a in &held{let r=call(&mut h.db,*a,with_arg("register(address)",Some(*a)),U256::from(19*PCOL),1);changed(&r,"register");}
    for (i,a) in held.iter().enumerate(){h.at(2+i as u64/8).unwrap();let r=call(&mut h.db,*a,with_arg("setRewardAddress(address)",Some(*a)),U256::ZERO,h.height);changed(&r,"reward");}
    let t=h.height+1;h.at(t).unwrap();
    for a in &held{let r=call(&mut h.db,*a,selector("deregister()"),U256::ZERO,t);changed(&r,"deregister");}
    assert_eq!(reg(&h.db,2),U256::ZERO,"the sweep cursor starts at m0");
    let due=t+40;
    let r=call(&mut h.db,system(),selector("markClose()"),U256::ZERO,due);assert!(r.result.is_success());
    let r=call(&mut h.db,system(),selector("beginBlock()"),U256::ZERO,due);assert!(r.result.is_success(),"{:?}",r.result);
    assert_eq!(reg(&h.db,0).to::<u64>(),48,"16 deletions after 48 scans of live records");
    conserved(&h).unwrap();
    let worst=r.result.gas_used();
    eprintln!("SWEEP worst case (64 scans, 16 deletions) v3 {worst} gas; GAS_ENVELOPE beginBlock {envelope}");
    assert!(worst<=envelope,"v3 beginBlock worst case {worst} gas above the admission envelope {envelope}");
}

/// The clocks sit in spare record bits: lastMaintHeight modulo 2^32 (w1 bits 224..255, moved from w0 bits 216..255 by
/// V3-PR-E4 for the epoch counters), lastReactHeight in full (w1 bits 160..223). The modular clock can only refuse: a
/// maintenance operation exactly 2^32 blocks after the previous one reverts once and succeeds a block later. The views:
/// recordWords returns w1 as the reward address alone, recordClocks the two clocks.
#[test]
fn record_clocks_layout_and_views(){
    let wrap=1u64<<32;
    let mut h=Harness::new(&strs(&["x"]),&Setup::v3(8,60_480));
    for (op,want) in [(vec!["register","s","s",&b().to_string(),"1"],None),(vec!["set_reward","s","x","5"],None),
        (vec!["deregister","s","6"],None),(vec!["reactivate","s","38"],None)]{
        assert_eq!(h.step(&strs(&op)).unwrap(),want,"{op:?}");
    }
    let s=name("s");let (w0,w1,_,_)=h.record(s);
    assert_eq!((bits(w1,224,32),bits(w1,160,64),w0>>216),(5,38,U256::ZERO),"the clocks in w1; w0's counters bits untouched by maintenance");
    assert_eq!(Address::from_word(w1.into()),name("x"),"the reward address keeps its bits");
    let view=|h:&mut Harness,sig:&str|{let r=call(&mut h.db,name("viewer"),with_arg(sig,Some(s)),U256::ZERO,h.height);assert!(r.result.is_success());output(&r.result).to_vec()};
    let words=view(&mut h,"recordWords(address)");
    assert_eq!(U256::from_be_slice(&words[32..64]),U256::from_be_slice(name("x").as_slice()),"recordWords: w1 as the reward address alone");
    assert_eq!(U256::from_be_slice(&words[0..32]),w0,"recordWords: w0 raw");
    let clocks=view(&mut h,"recordClocks(address)");
    assert_eq!((U256::from_be_slice(&clocks[0..32]),U256::from_be_slice(&clocks[32..64])),(U256::from(5),U256::from(38)));
    // Modulo 2^40: refused exactly 2^40 blocks later (the only effect of the 40-bit clock), admitted a block on.
    assert_eq!(h.step(&strs(&["set_reward","s","s",&(5+wrap).to_string()])).unwrap().as_deref(),Some("RegOpsLimit"));
    assert_eq!(h.step(&strs(&["set_reward","s","s",&(6+wrap).to_string()])).unwrap(),None);
    assert_eq!(bits(h.record(s).1,224,32),6);
    // The C_react clock is a full height: far heights compare exactly.
    let far=6+wrap;
    assert_eq!(h.step(&strs(&["deregister","s",&far.to_string()])).unwrap(),None);
    assert_eq!(h.step(&strs(&["reactivate","s",&(far+32).to_string()])).unwrap(),None);
    assert_eq!(bits(h.record(s).1,160,64),far+32);
    assert_eq!(h.step(&strs(&["deregister","s",&(far+33).to_string()])).unwrap(),None);
    assert_eq!(h.step(&strs(&["reactivate","s",&(far+71).to_string()])).unwrap().as_deref(),Some("BadState"),"inside C_react 40");
    assert_eq!(h.step(&strs(&["reactivate","s",&(far+72).to_string()])).unwrap(),None,"exactly last + C_react");
}

// ---------------- V3-PR-E2b: evidence penalties (V3-16, V3-17)

const EVIDENCE_SCENARIOS:[&str;10]=["accruing-fact-committed-at-record-result","evidence-at-the-settlement-boundary","fallback-has-no-reporter",
    "non-winner-accused-settles-normally","random-1","random-2","reporter-cut-paid-directly","self-report-loses-three-quarters",
    "type2-accruing-winner-burns-whole-pot","type2-held-winner-pays-contributors"];
fn evidence_replay(s:&Setup)->BTreeMap<String,Result<(),String>>{
    let v:serde_json::Value=serde_json::from_str(EVIDENCE_VECTORS).unwrap();
    v.as_object().unwrap().iter().map(|(n,body)|(n.clone(),budget_scenario(n,body,s))).collect()
}
fn evidence_body(n:&str)->serde_json::Value{let v:serde_json::Value=serde_json::from_str(EVIDENCE_VECTORS).unwrap();v[n].clone()}
/// Runs a chain scenario's steps on a program set with that set's host conventions, without comparing them to the
/// model (the earn-in set's own outcome).
fn chain_exec(body:&serde_json::Value,s:&Setup)->Harness{
    let mut s=s.clone();
    for (k,v) in body["cp"].as_object().unwrap(){let n=cp_index(k);if n<s.cp_len{s.cp.push((n,v.as_u64().unwrap().into()));}}
    let durable:Vec<String>=body["durable"].as_array().unwrap().iter().map(|x|x.as_str().unwrap().to_string()).collect();
    let mut h=Harness::new(&durable,&s);
    for step in body["steps"].as_array().unwrap(){
        let op:Vec<String>=step["op"].as_array().unwrap().iter().map(|x|x.as_str().unwrap().to_string()).collect();
        match op[0].as_str(){"begin"=>{h.begin(step).unwrap();}"close"|"final"=>{let mut st=step.clone();st.as_object_mut().unwrap().retain(|k,_|k=="op"||k=="tips");h.phase2(&st).unwrap()}_=>{h.step(&op).unwrap();}}
    }
    h
}
/// What `who` gains in the phase 0 of block `at` of a chain scenario run on a program set (its steps up to that begin).
fn begin_gain(body:&serde_json::Value,s:&Setup,at:u64,who:&str)->U256{
    let mut b=body.clone();let steps=b["steps"].as_array_mut().unwrap();
    let k=steps.iter().position(|st|st["op"][0]=="begin"&&st["op"][1]==at.to_string()&&st.get("revert").is_none()).unwrap();
    steps.truncate(k);let before=gain(&chain_exec(&b,s),who);
    let mut b2=body.clone();b2["steps"].as_array_mut().unwrap().truncate(k+1);
    gain(&chain_exec(&b2,s),who)-before
}
/// What an account received: its balance less its start, plus the bond it sent if it is a sender (every sender of the
/// scenarios these tests read registered once, with B).
fn gain(h:&Harness,n:&str)->U256{
    let a=name(n);let sent=if h.senders.contains(&a){b()}else{U256::ZERO};
    balance(&h.db,a)+sent-h.start.get(&a).copied().unwrap_or_default()
}

/// Model-vs-bytecode differential: every chain scenario of the independent model reproduces on the v3 programs step by
/// step (each block's phase 0 in the v3 order, its user transactions and phase 2), with ACC_COUNT, held_total, the Escrow
/// invariant (with equality) and the supply conserved after every step, and every pot, record and payment at the end.
#[test]
fn v3_evidence_penalties_reproduce_the_independent_model(){
    let out=evidence_replay(&Setup::v3(8,60_480));
    assert_eq!(out.keys().map(|k|k.as_str()).collect::<Vec<_>>(),EVIDENCE_SCENARIOS);
    for (n,r) in &out{if let Err(e)=r{panic!("{n}: {e}")}}
    // The vectors exercise what they claim: whole-pot burns and normal settlements of pots with type-2 evidence, both
    // refusals of a block (a reporter that is not the proposer; evidence against a tomb), type 1 and type 2.
    let v:serde_json::Value=serde_json::from_str(EVIDENCE_VECTORS).unwrap();
    let (mut reverts,mut type1,mut type2)=(BTreeSet::new(),0,0);
    for body in v.as_object().unwrap().values(){for st in body["steps"].as_array().unwrap(){
        if st["op"][0]=="begin"{
            if let Some(r)=st.get("revert"){reverts.insert(r.as_str().unwrap().to_string());}
            else{for e in st["evidence"].as_array().unwrap(){if e[1].is_null(){type1+=1}else{type2+=1}}}
        }
    }}
    assert_eq!(reverts,BTreeSet::from(["BadCalldata".to_string(),"BadState".to_string()]));
    assert!(type1>=10&&type2>=20,"type 1 {type1}, type 2 {type2}");
}

/// V3-DESIGN.md V3-16, fails on the original: type-2 evidence against a HELD winner before settle(j), in block j+1 (pot 3)
/// and in block j+2 (pot 5). On the earn-in set both pots burn whole, so the contributor c and the signed proposer p of
/// both pots receive nothing; on v3 they are paid and only the tombed winners' parts burn, and the reporters receive
/// rho of the burned bonds instead of nothing.
#[test]
fn type2_evidence_before_settlement_pays_contributors_on_v3_but_burns_the_pot_on_earnin(){
    let body=evidence_body("type2-held-winner-pays-contributors");
    let earnin=chain_exec(&body,&Setup::earnin());let v3=chain_exec(&body,&Setup::v3(8,60_480));
    let pcol=|x:u128|U256::from(x);
    // Pot j: winner part 10%, proposer part 5%, R = the rest over Q = max(C, 64) = 64. Pots 3 and 5 are 2 PCOL: the carry
    // is lagged by one block (V3-15), so the first settlement, settle(1) in block 5, carries into pot 6, not pot 5 (the
    // parts are read from the pots closeBlock recorded).
    let (pp,r)=(|a:U256|a*pcol(500)/pcol(10_000),|a:U256|a-a*pcol(1000)/pcol(10_000)-a*pcol(500)/pcol(10_000));
    let (a3,a5)=(v3.closed[&3].3,v3.closed[&5].3);
    assert_eq!((a3,a5),(pcol(2*PCOL),pcol(2*PCOL)),"no carry before pot 6");assert!(!v3.closed[&6].2.is_zero(),"pot 6 carries pot 1's base burn");
    assert_eq!((gain(&earnin,"c"),gain(&earnin,"p")),(U256::ZERO,U256::ZERO),"the earn-in set burns pots 3 and 5 whole");
    assert_eq!(gain(&v3,"c"),r(a3)*pcol(2)/pcol(64)+r(a5)/pcol(64),"two shares in pot 3, one in pot 5");
    assert_eq!(gain(&v3,"p"),pp(a3)+pp(a5),"the signed proposer part of pots 3 and 5");
    // The reporters: r (block 4) and m0 (block 7) receive 25% of the 20 PCOL bond on v3, nothing on the earn-in set, in the
    // phase 0 that carries the evidence (neither is paid by that block's settlement).
    for (who,at) in [("r",4u64),("m0",7)]{
        assert_eq!((begin_gain(&body,&Setup::v3(8,60_480),at,who),begin_gain(&body,&Setup::earnin(),at,who)),(pcol(5*PCOL),U256::ZERO),"{who} reports in block {at}");
    }
    assert_eq!((gain(&v3,"w"),gain(&v3,"v")),(U256::ZERO,U256::ZERO),"the tombed winners receive nothing");
    // Earn-in burns both bonds and both pots; v3 burns 75% of each bond and only what of each pot it does not pay, and
    // carries base burns into later pots: every wei v3 pays beyond the earn-in set is one the earn-in set burns or holds
    // in a pending pot (the same issuance at these heights and the same held bonds); on v3 the carry into the next pot
    // (lagged by one block, V3-15) is held too.
    let burned=|h:&Harness|storage(&h.db,escrow_address(),U256::ZERO);
    let pending=|h:&Harness|(1..=h.height).map(|j|pot(&h.db,j).0).fold(U256::ZERO,|a,b|a+b)+carry_into(&h.db,h.height+1);
    assert!(carry_into(&earnin.db,0).is_zero()&&carry_into(&earnin.db,1).is_zero()&&carry_into(&v3.db,v3.height).is_zero());
    assert_eq!((earnin.issued,reg(&earnin.db,4)),(v3.issued,reg(&v3.db,4)));
    let more:U256=v3.db.accounts.iter().filter(|(a,_)|**a!=escrow_address()).map(|(a,x)|x.info.balance-balance(&earnin.db,*a)).fold(U256::ZERO,|x,y|x+y);
    assert_eq!(burned(&earnin)+pending(&earnin),burned(&v3)+pending(&v3)+more);
    for h in [&earnin,&v3]{h.chain_conserved().unwrap();}
}

/// The evidence vectors test V3-16/V3-17: run with the spec-1 host conventions on the earn-in set, every scenario
/// differs from the v3 model.
#[test]
fn evidence_penalty_vectors_fail_on_the_earnin_set(){
    let v:serde_json::Value=serde_json::from_str(EVIDENCE_VECTORS).unwrap();
    for (n,body) in v.as_object().unwrap(){
        let r=budget_scenario(n,body,&Setup::earnin());
        eprintln!("earn-in {n}: {}",r.as_ref().err().map_or("reproduces",|e|e.as_str()));
        assert!(r.is_err(),"{n} reproduces on the earn-in set: it does not test V3-16/V3-17");
    }
}

/// V3-17: a self-report loses exactly (1 - rho) * B. r (reward rr) reports itself and c in its own block: its 20 PCOL bond
/// burns, its reward address receives 25% of it, and 25% of c's as the block's reporter.
#[test]
fn a_self_report_loses_three_quarters_of_the_bond(){
    let body=evidence_body("self-report-loses-three-quarters");
    let v3=chain_exec(&body,&Setup::v3(8,60_480));
    let rec=v3.record(name("r"));
    assert_eq!((bits(rec.0,8,8),bits(rec.0,16,8)),(1,2),"r is a tomb with its bond burned");
    let own=b()*U256::from(2500)/U256::from(10_000);
    assert_eq!(gain(&v3,"rr"),own+own,"rho of its own bond and of c's");
    assert_eq!(b()-own,U256::from(15*PCOL),"the self-report's net loss is (1 - rho) * B = 15 PCOL");
    // The fallback block of the same evidence has no reporter: the whole bond burns.
    // The fallback block of the same evidence has no reporter (the whole bond burns); the refused blocks that named r
    // as the reporter paid it nothing.
    let body=evidence_body("fallback-has-no-reporter");let v3=chain_exec(&body,&Setup::v3(8,60_480));
    assert_eq!(gain(&v3,"r"),U256::ZERO,"the refused blocks paid no reporter");
    assert!(storage(&v3.db,escrow_address(),U256::ZERO)>=b(),"w's whole bond burned");
}

/// The five planted V3-16/V3-17 mutants (compile_system_v3.cjs --mutants) each fail the evidence vectors, each in the
/// scenario written for it.
#[test]
fn the_evidence_penalty_mutants_fail_the_v3_vectors(){
    let list=mutant_programs();
    assert_eq!(list.iter().map(|m|m.0.as_str()).collect::<Vec<_>>(),[&EARNIN_MUTANTS[..],&BUDGET_MUTANTS[..],&EVIDENCE_MUTANTS[..],&LATE_MUTANTS[..],&ECON_MUTANTS[..],&CONTEST_MUTANTS[..]].concat());
    let evidence=&list[EARNIN_MUTANTS.len()+BUDGET_MUTANTS.len()..EARNIN_MUTANTS.len()+BUDGET_MUTANTS.len()+EVIDENCE_MUTANTS.len()];
    let expect=[("reporter-paid-on-fallback","fallback-has-no-reporter"),("whole-pot-for-bonded-accused","type2-held-winner-pays-contributors"),
        ("accruing-read-at-evidence","accruing-fact-committed-at-record-result"),("accruing-read-at-settle","type2-accruing-winner-burns-whole-pot"),
        ("rho-on-pot-not-bond","self-report-loses-three-quarters")];
    for ((n,program,code),(want_name,killer)) in evidence.iter().zip(expect){
        assert_eq!(n,want_name);
        let original=runtime(V3,program);assert_ne!(code,&original,"{n} changes the bytecode");
        let out=evidence_replay(&Setup::v3(8,60_480).with_program(program,code.clone()));
        let killed:Vec<&String>=out.iter().filter(|(_,r)|r.is_err()).map(|(k,_)|k).collect();
        eprintln!("mutant {n} ({program}): killed by {killed:?}");
        assert!(killed.iter().any(|k|k.as_str()==killer),"mutant {n} survives {killer}: {killed:?}");
    }
}

/// punish and invalidate as the v3 host calls them, and only so: the reporter must be the block's tx.origin (none on a
/// fallback), the accused 20-byte clean; invalidate needs the recorded winner; the spec-1 selectors are gone from the
/// v3 programs (a v3 genesis can never run the spec-1 host's calls by mistake).
#[test]
fn v3_punish_and_invalidate_call_shapes(){
    let body=evidence_body("type2-held-winner-pays-contributors");
    let mut h=chain_exec(&body,&Setup::v3(8,60_480));
    let next=h.height+1;let step=serde_json::json!({"op":["begin",next.to_string(),"c"],"winner":"m0","shares":[],"evidence":[]});
    assert_eq!(h.begin(&step).unwrap(),None);
    let c=name("c");
    assert_eq!(h.sys("punish",registry_address(),with_arg("punish(address)",Some(c))),Err("BadCalldata".into()),"the spec-1 punish selector reaches only the user-path fallback");
    let mut d=with_arg("punish(address,address)",Some(c));d.extend(name("p").into_word());
    assert_eq!(h.sys("punish",registry_address(),d),Err("BadCalldata".into()),"a reporter other than tx.origin");
    let mut d=with_arg("punish(address,address)",Some(c));d.extend((U256::from(1)<<200u32).to_be_bytes::<32>());
    assert!(h.sys("punish",registry_address(),d).is_err(),"a dirty reporter word (the ABI decoder refuses it)");
    let mut d=selector("invalidate(uint256)");d.extend(U256::from(next-1).to_be_bytes::<32>());
    assert_eq!(h.sys("invalidate",escrow_address(),d).map_err(|e|e.contains("output: 0x ")),Err(true),"the spec-1 invalidate selector is not in the v3 Escrow");
    // Pot `next` has no recorded winner while its block is open: invalidate refuses it.
    h.close(next,U256::ZERO).unwrap();
    let mut d=selector("invalidate(uint256,address)");d.extend(U256::from(next).to_be_bytes::<32>());d.extend(c.into_word());
    assert_eq!(h.sys("invalidate",escrow_address(),d),Err("BadPending".into()),"j is the current height");
    let mut d=with_arg("punish(address,address)",Some(c));d.extend(name("c").into_word());
    assert_eq!(h.sys("punish",registry_address(),d),Ok(()),"the proposer reports");
    h.chain_conserved().unwrap();
}

// ---------------- V3-PR-E3: late share credit (V3-02)
const LATE_SCENARIOS:[&str;7]=["late-last-share-is-monotone","late-random-1","late-random-2","late-share-at-the-settlement-boundary",
    "late-share-cap-fills-only-the-uncovered-pool","late-share-owners-absent-tomb-and-accruing","late-share-pays-the-omitted-contributor"];
fn late_replay(s:&Setup)->BTreeMap<String,Result<(),String>>{
    let v:serde_json::Value=serde_json::from_str(LATE_VECTORS).unwrap();
    v.as_object().unwrap().iter().map(|(n,body)|(n.clone(),budget_scenario(n,body,s))).collect()
}
fn late_body(n:&str)->serde_json::Value{let v:serde_json::Value=serde_json::from_str(LATE_VECTORS).unwrap();v[n].clone()}
/// A scenario's steps with every late credit taken out of its blocks (what the chain was without V3-02).
fn without_late(body:&serde_json::Value)->serde_json::Value{
    let mut b=body.clone();
    for st in b["steps"].as_array_mut().unwrap(){if let Some(e)=st.get_mut("evidence"){e.as_array_mut().unwrap().retain(|x|x[0]!="late");}}
    b
}

/// Model-vs-bytecode differential: every late-share scenario of the independent model reproduces on the v3 programs
/// step by step (recordLate in phase 0 after recordResult and before settle(h-D)), with ACC_COUNT, held_total, the
/// Escrow invariant with equality and the supply after every step, and every pot, record (lastShare included), payment
/// and retained ShareLedger height (counts and totals) at the end.
#[test]
fn v3_late_shares_reproduce_the_independent_model(){
    let out=late_replay(&Setup::v3(8,60_480));
    assert_eq!(out.keys().map(|k|k.as_str()).collect::<Vec<_>>(),LATE_SCENARIOS);
    for (n,r) in &out{if let Err(e)=r{panic!("{n}: {e}")}}
    // The vectors exercise what they claim: accepted late credits, both refusals (the window: BadPhase; the cap and an
    // unrecorded height: BadState) and type-1 evidence in the same blocks.
    let v:serde_json::Value=serde_json::from_str(LATE_VECTORS).unwrap();
    let (mut reverts,mut credits,mut punish)=(BTreeSet::new(),0,0);
    for body in v.as_object().unwrap().values(){
        assert!(body["final"].get("ledger").is_some(),"every late scenario compares the ledger");
        for st in body["steps"].as_array().unwrap(){
            if st["op"][0]=="begin"{
                if let Some(r)=st.get("revert"){reverts.insert(r.as_str().unwrap().to_string());}
                else{for e in st["evidence"].as_array().unwrap(){if e[0]=="late"{credits+=e[2].as_array().unwrap().len()}else{punish+=1}}}
            }
        }
    }
    assert_eq!(reverts,BTreeSet::from(["BadPhase".to_string(),"BadState".to_string()]));
    assert!(credits>=60&&punish>=2,"late credits {credits}, punish {punish}");
}

/// V3-DESIGN.md V3-02, fails on the original: the winner of block 4 omits b's share. On the v3 set LS(4) carried by
/// block 5 credits b, and settle(4) at h = 8 pays b R/Q (R = 1.7 PCOL, Q = max(C, kappa*m) = 64) and sets its lastShare
/// to 4. The earn-in programs have no recordLate (the block's call reverts), and without the late credit b is paid
/// nothing: the winner alone decided b's credit. The header shares keep their value: c is paid the same R/Q either way.
#[test]
fn late_share_pays_the_omitted_contributor_on_v3_not_on_earnin(){
    let body=late_body("late-share-pays-the-omitted-contributor");
    let v3=chain_exec(&body,&Setup::v3(8,60_480));
    let r=U256::from(17*PCOL/10);
    assert_eq!(gain(&v3,"b"),r/U256::from(64),"b's late share is paid at settle(4)");
    assert_eq!(bits(v3.record(name("b")).0,88,64),4,"b's lastShare");
    let original=chain_exec(&without_late(&body),&Setup::v3(8,60_480));
    assert_eq!(gain(&original,"b"),U256::ZERO,"without V3-02 the winner's omission removes b's pay");
    assert_eq!(gain(&original,"c"),gain(&v3,"c"),"no header share loses value (no dilution: Q is unchanged)");
    assert_eq!(bits(original.record(name("b")).0,88,64),0);
    // The earn-in set refuses the block that carries it: recordLate does not exist there.
    let mut h=Harness::new(&[],&Setup::earnin());
    let mut refused=None;
    for st in body["steps"].as_array().unwrap(){
        let op:Vec<String>=st["op"].as_array().unwrap().iter().map(|x|x.as_str().unwrap().to_string()).collect();
        match op[0].as_str(){"begin"=>{if let Some(e)=h.begin(st).unwrap(){refused=Some((op[1].clone(),e));break}}"close"=>{h.close(op[1].parse().unwrap(),U256::ZERO).unwrap();}_=>{h.step(&op).unwrap();}}
    }
    let (at,why)=refused.expect("the earn-in programs refuse the carrying block");
    assert_eq!(at,"5");eprintln!("earn-in refuses the carrying block: {why}");
    for x in [&v3,&original]{x.chain_conserved().unwrap();}
}

/// The late-share vectors test V3-02: on the earn-in set (no recordLate) every scenario differs from the model.
#[test]
fn late_share_vectors_fail_on_the_earnin_set(){
    let v:serde_json::Value=serde_json::from_str(LATE_VECTORS).unwrap();
    for (n,body) in v.as_object().unwrap(){
        let r=budget_scenario(n,body,&Setup::earnin());
        eprintln!("earn-in {n}: {}",r.as_ref().err().map_or("reproduces",|e|e.as_str()));
        assert!(r.is_err(),"{n} reproduces on the earn-in set: it does not test V3-02");
    }
}

/// The five planted V3-02 mutants (compile_system_v3.cjs --mutants) each fail the late-share vectors, each in the
/// scenario written for it.
#[test]
fn the_late_share_mutants_fail_the_v3_vectors(){
    let list=mutant_programs();
    let at=EARNIN_MUTANTS.len()+BUDGET_MUTANTS.len()+EVIDENCE_MUTANTS.len();let late=&list[at..at+LATE_MUTANTS.len()];
    let expect=[("late-last-share-reverts","late-last-share-is-monotone"),("late-last-share-overwrites","late-last-share-is-monotone"),
        ("late-cap-removed","late-share-cap-fills-only-the-uncovered-pool"),("late-window-one-wider","late-share-at-the-settlement-boundary"),
        ("late-total-not-raised","late-share-pays-the-omitted-contributor")];
    assert_eq!(late.len(),expect.len());
    for ((n,program,code),(want_name,killer)) in late.iter().zip(expect){
        assert_eq!(n,want_name);
        let original=runtime(V3,program);assert_ne!(code,&original,"{n} changes the bytecode");
        let out=late_replay(&Setup::v3(8,60_480).with_program(program,code.clone()));
        let killed:Vec<&String>=out.iter().filter(|(_,r)|r.is_err()).map(|(k,_)|k).collect();
        eprintln!("mutant {n} ({program}): killed by {killed:?}");
        assert!(killed.iter().any(|k|k.as_str()==killer),"mutant {n} survives {killer}: {killed:?}");
    }
}

/// recordLate as the v3 host calls it, and only so: SYSTEM only on the registry, the registry only on the ledger; the
/// exact calldata shape, 1 <= n <= S_max, nonzero clean owners; a recorded height inside the window under the cap.
#[test]
fn v3_record_late_call_shapes(){
    let body=late_body("late-share-pays-the-omitted-contributor");
    let mut h=chain_exec(&body,&Setup::v3(8,60_480));
    let next=h.height+1;let step=serde_json::json!({"op":["begin",next.to_string(),"m0"],"winner":"m0","shares":[],"evidence":[]});
    assert_eq!(h.begin(&step).unwrap(),None);
    let call=|j:u64,owners:&[U256],off:u64,count:Option<u64>|->Vec<u8>{
        let mut d=selector("recordLate(uint64,address[])");d.extend(U256::from(j).to_be_bytes::<32>());d.extend(U256::from(off).to_be_bytes::<32>());
        d.extend(U256::from(count.unwrap_or(owners.len() as u64)).to_be_bytes::<32>());for o in owners{d.extend(o.to_be_bytes::<32>());}d
    };
    let b_=U256::from_be_slice(name("b").as_slice());let j=next-1;
    assert_eq!(h.sys("recordLate",registry_address(),call(j,&[],64,None)),Err("BadCalldata".into()),"no owner");
    // A malformed array (offset, or a count the calldata does not hold) fails the ABI decoding before the checks.
    assert!(h.sys("recordLate",registry_address(),call(j,&[b_],32,None)).is_err(),"offset");
    assert!(h.sys("recordLate",registry_address(),call(j,&[b_],64,Some(2))).is_err(),"length");
    assert_eq!(h.sys("recordLate",registry_address(),call(j,&[U256::ZERO],64,None)),Err("BadCalldata".into()),"zero owner");
    assert!(h.sys("recordLate",registry_address(),call(j,&[b_|(U256::from(1)<<160u32)],64,None)).is_err(),"dirty owner word (refused by the ABI re-encoding of the nested call)");
    assert_eq!(h.sys("recordLate",registry_address(),call(j,&vec![b_;257],64,None)),Err("BadCalldata".into()),"n > S_max");
    assert_eq!(h.sys("recordLate",registry_address(),call(next,&[b_],64,None)),Err("BadPhase".into()),"j = h");
    assert_eq!(h.sys("recordLate",registry_address(),call(next-5,&[b_],64,None)),Err("BadPhase".into()),"j < h - D");
    // A user may not call either: only SYSTEM reaches the registry's recordLate, only the registry the ledger's.
    let user=h.sender("u");
    let r=call_to(&mut h.db,user,registry_address(),call(j,&[b_],64,None),U256::ZERO,next);assert!(!r.result.is_success(),"a user call");
    assert_eq!(h.sys("recordLate",ledger(),call(j,&[b_],64,None)),Err("Unauthorized".into()),"SYSTEM on the ledger directly");
    assert_eq!(h.sys("recordLate",registry_address(),call(j,&[b_],64,None)),Ok(()),"the shape the host sends");
    assert_eq!(h.sys("recordLate",registry_address(),call(next-4,&[b_],64,None)),Ok(()),"j = h - D, before settle(h - D)");
    h.close(next,U256::ZERO).unwrap();h.chain_conserved().unwrap();
}

// ---------------- V3-PR-E2a: issuance M3, carry and tip split (V3-19, V3-15, V3-20)
const ISSUANCE_SCENARIOS:[&str;9]=["carry-fuzz-c-1-to-smax-plus-1","carry-random-late-and-tips","final-block-burns-the-carry","issuance-halving-compressed",
    "last-carrier-includes-late","last-carrier-omits-late","late-inclusion-after-7-dropped","late-inclusion-base","tip-split-signed-fallback-accruing-tomb"];
fn issuance_body(n:&str)->serde_json::Value{let v:serde_json::Value=serde_json::from_str(ISSUANCE_VECTORS).unwrap();v[n].clone()}
fn issuance_replay(s:&Setup)->BTreeMap<String,Result<Harness,String>>{
    let v:serde_json::Value=serde_json::from_str(ISSUANCE_VECTORS).unwrap();
    v.as_object().unwrap().iter().filter(|(_,b)|b.get("steps").is_some()).map(|(n,body)|(n.clone(),budget_run(n,body,s))).collect()
}

/// V3-19: the host's issuance(h) (issuance.rs) reproduces the model's schedule table: the owner's values (S0 1 PCOL,
/// H_half 4,207,680, S_tail S0/32) at h = 0, 1, H_half - 1, H_half, ..., 256·H_half, 2^64 - 1, a schedule without a tail, a
/// U256-sized S0 and the compressed fixture.
#[test]
fn issuance_schedule_vectors_reproduce_on_the_host(){
    let v:serde_json::Value=serde_json::from_str(ISSUANCE_VECTORS).unwrap();
    let cases=v["schedule"]["cases"].as_array().unwrap();
    let num=|c:&serde_json::Value,k:&str|c[k].as_str().unwrap().parse::<U256>().unwrap();
    for c in cases{
        let (h,s0,hh,tail)=(num(c,"h").to::<u64>(),num(c,"S0"),num(c,"H_half").to::<u64>(),num(c,"S_tail"));
        assert_eq!(pocol_lp4_execution::issuance::issuance(h,s0,hh,tail),num(c,"issued"),"{c}");
    }
    let owner:Vec<u64>=cases.iter().filter(|c|c["label"]=="owner").map(|c|num(c,"h").to::<u64>()).collect();
    let h=4_207_680u64;
    for want in [1,h-1,h,256*h]{assert!(owner.contains(&want),"the owner table has h = {want}");}
    let at=|x:u64|cases.iter().find(|c|c["label"]=="owner"&&num(c,"h").to::<u64>()==x).map(|c|num(c,"issued")).unwrap();
    assert_eq!((at(1),at(h-1),at(h),at(256*h)),(U256::from(PCOL),U256::from(PCOL),U256::from(PCOL/2),U256::from(PCOL/32)));
}

/// Model-vs-bytecode differential: every scenario of tests/v3/issuance-carry-vectors.json reproduces on the v3 programs
/// step by step: the host's issuance(h) and every pot closeBlock records equal the model's, with the Escrow invariant with
/// equality (the carry term included inside a block) and the supply after every step, and every pot, record, payment,
/// ledger height and the carry at the end.
#[test]
fn v3_issuance_carry_and_tips_reproduce_the_independent_model(){
    let out=issuance_replay(&Setup::v3(8,60_480));
    assert_eq!(out.keys().map(|k|k.as_str()).collect::<Vec<_>>(),ISSUANCE_SCENARIOS);
    let mut carried=0;let mut cs=BTreeSet::new();let (mut signed_tips,mut fallback_tips)=(0,0);
    for (n,r) in &out{
        let h=r.as_ref().unwrap_or_else(|e|panic!("{n}: {e}"));
        carried+=h.closed.values().filter(|c|!c.2.is_zero()).count();
        let body=issuance_body(n);
        for st in body["steps"].as_array().unwrap(){
            if st["op"][0]=="begin"&&st["op"][1]!="1"&&st.get("revert").is_none(){cs.insert((n.clone(),st["shares"].as_array().unwrap().len()+1));}
        }
        let proposer:BTreeMap<u64,bool>=body["steps"].as_array().unwrap().iter().filter(|st|st["op"][0]=="begin").map(|st|(st["op"][1].as_str().unwrap().parse().unwrap(),st["op"][2]!="")).collect();
        for (j,c) in &h.closed{if !c.1.is_zero(){if proposer[j]{signed_tips+=1}else{fallback_tips+=1}}}
    }
    // The vectors exercise what they claim: carries into pots, C from 1 to S_max + 1 = 17, tips in signed and fallback
    // blocks, the halving (six distinct issuances in the compressed fixture) and a final block.
    assert!(carried>=60,"pots with a carry: {carried}");
    for c in 1..=17usize{assert!(cs.contains(&("carry-fuzz-c-1-to-smax-plus-1".to_string(),c)),"C = {c}");}
    assert!(signed_tips>=10&&fallback_tips>=4,"tips in signed blocks {signed_tips}, fallback {fallback_tips}");
    let comp=out["issuance-halving-compressed"].as_ref().unwrap();
    assert_eq!(comp.closed.values().map(|c|c.0).collect::<BTreeSet<_>>().len(),6,"S0 >> k for k = 0..4 and the tail");
}

/// Host credit = recorded pot, and the tip split conserves value (V3-19, V3-15, V3-20), on the bytecode alone: every pot
/// closeBlock records is issued + tips - share + carry, with share = floor(tips·gamma_tip_bp/10^4) for a signed block and 0
/// for a fallback block, and the share reaches the proposer as payTo pays it: a HELD record's reward address receives it, an
/// ACCRUING record's bond takes it first (the rest paid), a tomb's share burns. Nothing else moves at closeBlock.
#[test]
fn host_credit_equals_the_recorded_pot_and_the_tip_split_conserves_value(){
    let s=Setup::v3(8,60_480);
    let (mut held,mut accruing,mut tomb,mut fallback)=(0,0,0,0);
    for n in ISSUANCE_SCENARIOS{
        let body=issuance_body(n);
        let mut st=s.clone();for (k,v) in body["cp"].as_object().unwrap(){st.cp.push((cp_index(k),v.as_str().map_or_else(||v.as_u64().unwrap().into(),|x|x.parse::<u128>().unwrap())));}
        let durable:Vec<String>=body["durable"].as_array().unwrap().iter().map(|x|x.as_str().unwrap().to_string()).collect();
        let mut h=Harness::new(&durable,&st);let gamma=storage(&h.db,caps(),U256::from(59));
        for step in body["steps"].as_array().unwrap(){
            let op:Vec<String>=step["op"].as_array().unwrap().iter().map(|x|x.as_str().unwrap().to_string()).collect();
            match op[0].as_str(){
                "begin"=>{h.begin(step).unwrap();}
                "close"=>{
                    let at:u64=op[1].parse().unwrap();let tips:U256=step["tips"].as_str().unwrap().parse().unwrap();
                    let origin=h.origin;let (w0,w1,bond,_)=h.record(origin);let reward=Address::from_word(w1.into());
                    let before=(balance(&h.db,reward),storage(&h.db,escrow_address(),U256::ZERO),balance(&h.db,escrow_address()));
                    h.phase2(step).unwrap();
                    let (issued,_,carry,amount)=h.closed[&at];
                    let share=if origin.is_zero(){U256::ZERO}else{tips*gamma/U256::from(10_000)};
                    assert_eq!(amount,issued+tips-share+carry,"{n}: pot {at}");
                    assert!(carry_into(&h.db,at).is_zero(),"{n}: carry into pot {at} left after closeBlock {at}");
                    let (w0b,_,bond_after,_)=h.record(origin);
                    let paid=balance(&h.db,reward)-before.0;let burned=storage(&h.db,escrow_address(),U256::ZERO)-before.1;
                    if origin.is_zero(){assert_eq!((paid,burned),(U256::ZERO,U256::ZERO));if !tips.is_zero(){fallback+=1}}
                    else if bits(w0,8,8)!=0{assert_eq!((paid,burned),(U256::ZERO,share),"{n}: a tomb's share burns at {at}");if !share.is_zero(){tomb+=1}}
                    else if bits(w0,16,8)==4{assert_eq!(bond_after-bond+paid,share,"{n}: an ACCRUING proposer's share fills its bond first at {at}");assert!(burned.is_zero());if !share.is_zero(){accruing+=1;if bits(w0b,16,8)==1{eprintln!("{n}: proposer promoted by its tip share at {at}")}}}
                    else{assert_eq!((paid,burned),(share,U256::ZERO),"{n}: a HELD proposer's reward address receives its share at {at}");if !share.is_zero(){held+=1}}
                    // Escrow keeps everything but what it paid: issued + tips in, the paid share out.
                    assert_eq!(balance(&h.db,escrow_address()),before.2+issued+tips-paid,"{n}: Escrow at {at}");
                }
                "final"=>{h.phase2(step).unwrap();}
                _=>{h.step(&op).unwrap();}
            }
            h.chain_conserved().unwrap_or_else(|e|panic!("{n} after {op:?}: {e}"));
        }
    }
    assert!(held>=5&&accruing>=2&&tomb>=1&&fallback>=4,"tip shares: HELD {held}, ACCRUING {accruing}, tomb {tomb}, fallback blocks with tips {fallback}");
}

/// V3-DESIGN.md V3-15 test: changing LS inclusion in blocks j+2..j+D (block j+1 fixed) does not change pot j+1. The two
/// scenarios are the same chain except that the second drops late credits of blocks after 7 (those of blocks 13 and 14,
/// to template 10, as the seeded draw fell): with j+1 the last block both share (12, at least 8), every pot up to j+1 is
/// the same on the bytecode, both as closeBlock recorded it and as settle distributed it (a design adding value to an
/// already-closed pot at settle time, safety finding 8's rejected shape, changes the second: the planted
/// base-burn-into-closed-pot moves half of settle(10)'s base burn into pot 11 and fails here), and a later pot differs (the
/// carry of a settle follows the late credits to its pot, which the carriers up to the settling block commit).
#[test]
fn pot_j_plus_1_is_fixed_by_block_j_plus_1(){
    let (a,b)=(issuance_body("late-inclusion-base"),issuance_body("late-inclusion-after-7-dropped"));
    let differ:Vec<u64>=a["steps"].as_array().unwrap().iter().zip(b["steps"].as_array().unwrap())
        .filter(|(x,y)|x["op"][0]=="begin"&&x["evidence"]!=y["evidence"]).map(|(x,_)|x["op"][1].as_str().unwrap().parse().unwrap()).collect();
    assert_ne!(a["steps"],b["steps"],"the late credits differ");
    let fixed=differ[0]-1;
    assert!(fixed>=8,"blocks up to {fixed} are the same in both");
    let s=Setup::v3(8,60_480);
    let (ha,hb)=(budget_run("base",&a,&s).unwrap(),budget_run("variant",&b,&s).unwrap());
    let later=pots_fixed_by_their_blocks(&ha,&hb,fixed).unwrap();
    eprintln!("blocks whose late credits differ: {differ:?}; pots that differ after block {fixed}: {later:?}");
    assert!(later.iter().all(|j|ha.closed[j].2!=hb.closed[j].2&&ha.closed[j].0==hb.closed[j].0),"only the carry differs (the same issuance and tips)");
    // The property check itself fails on the planted mutant (the bytecode run without the model's per-step comparison).
    let list=mutant_programs();
    let (_,program,code)=list.iter().find(|m|m.0=="base-burn-into-closed-pot").unwrap();
    let m=Setup::v3(8,60_480).with_program(program,code.clone());
    let e=pots_fixed_by_their_blocks(&chain_exec(&a,&m),&chain_exec(&b,&m),fixed).err().expect("base-burn-into-closed-pot survives the fixedness check");
    eprintln!("base-burn-into-closed-pot: {e}");
    assert!(e.contains("as settle distributed it"),"{e}");
}
/// Pots 1..=fixed agree between two runs as closeBlock recorded them and as settle distributed them (each of them settled
/// in both); returns the later heights whose recorded pot differs (at least one: else the comparison tests nothing).
fn pots_fixed_by_their_blocks(ha:&Harness,hb:&Harness,fixed:u64)->Result<Vec<u64>,String>{
    for j in 1..=fixed{
        ensure_eq!(ha.closed[&j],hb.closed[&j],"pot {j} as closeBlock recorded it");
        ensure!(ha.distributed.contains_key(&j)&&hb.distributed.contains_key(&j),"pot {j} settled in both runs");
        ensure_eq!(ha.distributed[&j],hb.distributed[&j],"pot {j} as settle distributed it");
    }
    let later:Vec<u64>=(fixed+1..=ha.height).filter(|j|ha.closed[j].3!=hb.closed[j].3).collect();
    ensure!(!later.is_empty(),"some later pot carries a different base burn");
    Ok(later)
}

/// Fails on the original: the earn-in programs with the spec-1 host conventions record every pot as CP 15 + tips (flat
/// issuance, no tip split, no carry), so the compressed fixture's halvings, the proposer's tip share and the carried base
/// burns are absent (the scenarios without late credits, which the earn-in set refuses); the vectors fail there as a whole.
#[test]
fn issuance_carry_and_tip_split_fail_on_the_earnin_set(){
    let s0=U256::from(2*PCOL);
    for n in ["issuance-halving-compressed","tip-split-signed-fallback-accruing-tomb"]{
        let body=issuance_body(n);let e=chain_exec(&body,&Setup::earnin());
        let v3=budget_run(n,&body,&Setup::v3(8,60_480)).unwrap();
        for (j,c) in &e.closed{assert_eq!(c.3,s0+c.1,"{n}: earn-in pot {j} = CP 15 + tips");}
        let differ=e.closed.iter().filter(|(j,c)|c.3!=v3.closed[j].3).count();
        assert!(differ*2>e.closed.len(),"{n}: only {differ} of {} pots differ",e.closed.len());
        assert!(storage(&e.db,escrow_address(),U256::ZERO)>storage(&v3.db,escrow_address(),U256::ZERO),"{n}: the earn-in set burns what v3 carries");
    }
    let v:serde_json::Value=serde_json::from_str(ISSUANCE_VECTORS).unwrap();
    for (n,body) in v.as_object().unwrap(){
        if body.get("steps").is_none(){continue}
        let r=budget_scenario(n,body,&Setup::earnin());
        assert!(r.is_err(),"{n} reproduces on the earn-in set: it does not test V3-19/V3-15/V3-20");
    }
}

/// The eight planted V3-19/V3-15/V3-20 mutants (compile_system_v3.cjs --mutants, RewardEscrow) each fail the issuance-carry
/// vectors, each in the scenario written for it.
#[test]
fn the_issuance_carry_mutants_fail_the_v3_vectors(){
    let list=mutant_programs();
    let at=list.len()-CONTEST_MUTANTS.len()-ECON_MUTANTS.len();let econ=&list[at..at+ECON_MUTANTS.len()];
    let expect=[("carry-not-added","issuance-halving-compressed"),("tip-share-to-pot","tip-split-signed-fallback-accruing-tomb"),
        ("base-burn-still-burned","carry-fuzz-c-1-to-smax-plus-1"),("issued-ignored-flat-s0","issuance-halving-compressed"),("carry-kept-at-final","final-block-burns-the-carry"),
        ("base-burn-into-closed-pot","late-inclusion-base"),("carry-not-lagged","last-carrier-omits-late"),("lagged-carry-kept-at-final","final-block-burns-the-carry")];
    for ((n,program,code),(want_name,killer)) in econ.iter().zip(expect){
        assert_eq!(n,want_name);assert_eq!(program,"RewardEscrow");
        let original=runtime(V3,program);assert_ne!(code,&original,"{n} changes the bytecode");
        let out=issuance_replay(&Setup::v3(8,60_480).with_program(program,code.clone()));
        let killed:Vec<&String>=out.iter().filter(|(_,r)|r.is_err()).map(|(k,_)|k).collect();
        eprintln!("mutant {n} ({program}): killed by {killed:?}");
        assert!(killed.iter().any(|k|k.as_str()==killer),"mutant {n} survives {killer}: {killed:?}");
    }
}

/// D-V3-15 (the carry lagged by one block), fails on the previous rule: block h = j + D is the last block that can carry
/// late shares of template j (window [max(1, h-D), h-1]), and its signed proposer a gains nothing in its own pot by
/// leaving out x's eight late shares of j = 2: pot h is the same in both runs as closeBlock(h) recorded it and as
/// settle(h) distributed it, and so is everything a receives; the extra base burn of settle(j) the omission causes is
/// the carry into pot h + 1 (pot h + 1 = issued + carry, its carry the base burn of settle(j) computed from pot j and its
/// ledger total in each run), a pot whose block can no longer carry LS(j); x loses its late shares' pay. The planted
/// carry-not-lagged (settle(h-D)'s base burn into pot h, the unlagged rule) makes a gain from the omission and fails.
#[test]
fn last_carrier_gains_nothing_by_omitting_late_shares(){
    let (inc,omit)=(issuance_body("last-carrier-includes-late"),issuance_body("last-carrier-omits-late"));
    let s=Setup::v3(8,60_480);
    let (hi,ho)=(budget_run("includes",&inc,&s).unwrap(),budget_run("omits",&omit,&s).unwrap());
    let r=last_carrier_compare(&hi,&ho).unwrap();
    eprintln!("lagged carry: {r}");
    let list=mutant_programs();
    let (_,program,code)=list.iter().find(|m|m.0=="carry-not-lagged").unwrap();
    let m=Setup::v3(8,60_480).with_program(program,code.clone());
    let e=last_carrier_compare(&chain_exec(&inc,&m),&chain_exec(&omit,&m)).err().expect("carry-not-lagged survives the last-carrier check");
    let (mi,mo)=(chain_exec(&inc,&m),chain_exec(&omit,&m));
    assert!(gain(&mo,"a")>gain(&mi,"a"),"under the unlagged rule the omitting proposer gains");
    eprintln!("carry-not-lagged: {e}; the omitting proposer gains {}",gain(&mo,"a")-gain(&mi,"a"));
}
/// The last-carrier comparison of `last_carrier_gains_nothing_by_omitting_late_shares` (j = 2, D = 4, h = 6): Err names
/// the first difference the lagged rule forbids.
fn last_carrier_compare(hi:&Harness,ho:&Harness)->Result<String,String>{
    let (j,h)=(2u64,6u64);
    let d=storage(&hi.db,caps(),U256::from(11)).to::<u64>();ensure_eq!(j+d,h,"block h is the last carrier of j");
    for k in 1..=h{
        ensure_eq!(hi.closed[&k],ho.closed[&k],"pot {k} as closeBlock({k}) recorded it");
        ensure_eq!(hi.distributed.get(&k),ho.distributed.get(&k),"pot {k} as settle({k}) distributed it");
    }
    ensure!(hi.distributed.contains_key(&h),"pot h settled");
    ensure_eq!(gain(hi,"a"),gain(ho,"a"),"what the last carrier a receives");
    // The carry into pot h+1 is settle(j)'s base burn, from pot j's amount and C = 2 header shares (+ 8 late ones).
    let cp=|n:u64|storage(&hi.db,caps(),U256::from(n));
    let a=hi.distributed[&j];ensure_eq!(a,ho.distributed[&j],"pot j");
    let r=a-a*cp(13)/U256::from(10_000)-a*cp(14)/U256::from(10_000);let q=cp(8)*cp(9);
    let burn=|c:u64|{let c=U256::from(c);r-r*c/q.max(c)};
    let (ci,co)=(hi.closed[&(h+1)],ho.closed[&(h+1)]);
    ensure_eq!((ci.2,co.2),(burn(10),burn(2)),"the carry into pot h+1 is settle(j)'s base burn");
    ensure_eq!((ci.3,co.3),(ci.0+ci.2,co.0+co.2),"pot h+1 = issued + carry (no tips)");
    ensure!(co.2>ci.2,"the omission raises the carry into pot h+1");
    ensure!(gain(hi,"x")>gain(ho,"x"),"x is paid its late shares only when they are carried");
    Ok(format!("pot {h} {} both runs; pot {} {} with LS({j}), {} without; a receives {}; x loses {}",hi.closed[&h].3,h+1,ci.3,co.3,gain(hi,"a"),gain(hi,"x")-gain(ho,"x")))
}

/// V3-15 at a final block with the lagged carry: two carries are outstanding when finalize runs, the carry into pot F
/// (block F-1's settle(F-1-D)) and the carry into pot F+1 of F's own phase-0 settle(F-D); finalize burns both with the
/// tips, so after F neither carry slot holds anything, no pot is pending and Escrow holds exactly the held bonds and the
/// burn (the scenario is checked against the model step by step; carry-kept-at-final and lagged-carry-kept-at-final, which
/// keep one of the two, fail it in `the_issuance_carry_mutants_fail_the_v3_vectors`).
#[test]
fn the_final_block_burns_every_outstanding_carry(){
    let body=issuance_body("final-block-burns-the-carry");
    let h=budget_run("final",&body,&Setup::v3(8,60_480)).unwrap();
    let (into_f,own)=h.final_carry.expect("a final block ran");
    assert!(!into_f.is_zero()&&!own.is_zero(),"both carries outstanding at finalize: into pot F {into_f}, of F's own settle {own}");
    assert!(carry_into(&h.db,h.height).is_zero()&&carry_into(&h.db,h.height+1).is_zero(),"no carry after F");
    for j in 1..=h.height{assert_eq!(pot(&h.db,j),(U256::ZERO,U256::ZERO,U256::ZERO),"pending[{j}] after F");}
    assert_eq!(balance(&h.db,escrow_address()),reg(&h.db,4)+storage(&h.db,escrow_address(),U256::ZERO),"Escrow = held + burned after F");
}

/// closeBlock and finalize as the v3 host calls them: SYSTEM only, exact calldata (the spec-1 closeBlock(uint256) is gone
/// from the v3 Escrow); the carry view (both carry slots).
#[test]
fn v3_close_block_call_shapes(){
    let body=issuance_body("issuance-halving-compressed");
    let mut h=chain_exec(&body,&Setup::v3(8,60_480));
    let next=h.height+1;let step=serde_json::json!({"op":["begin",next.to_string(),"m0"],"winner":"m0","shares":[],"evidence":[]});
    assert_eq!(h.begin(&step).unwrap(),None);
    let mut d=selector("closeBlock(uint256)");d.extend(U256::ZERO.to_be_bytes::<32>());
    assert!(h.sys("closeBlock",escrow_address(),d).is_err(),"the spec-1 closeBlock selector is not in the v3 Escrow");
    let mut d=selector("closeBlock(uint256,uint256)");d.extend(U256::ZERO.to_be_bytes::<32>());
    assert!(h.sys("closeBlock",escrow_address(),d).is_err(),"one word short (the ABI decoder refuses it)");
    let mut d=selector("closeBlock(uint256,uint256)");d.extend([0u8;96]);
    assert_eq!(h.sys("closeBlock",escrow_address(),d),Err("BadSystemCall".into()),"one word long");
    let user=h.sender("u");let mut d=selector("closeBlock(uint256,uint256)");d.extend([0u8;64]);
    let r=call_to(&mut h.db,user,escrow_address(),d,U256::ZERO,next);assert!(!r.result.is_success(),"a user call");
    let r=call_to(&mut h.db,user,escrow_address(),selector("carry()"),U256::ZERO,next);assert!(r.result.is_success());
    assert!(!carry_into(&h.db,next+1).is_zero(),"settle(h-D) left a carry into pot h+1");
    assert_eq!(U256::from_be_slice(output(&r.result)),carry_into(&h.db,next)+carry_into(&h.db,next+1),"carry() reads both carry slots");
    h.close(next,U256::from(10)).unwrap();h.chain_conserved().unwrap();
    let mut d=selector("closeBlock(uint256,uint256)");d.extend([0u8;64]);
    assert_eq!(h.sys("closeBlock",escrow_address(),d),Err("BadPending".into()),"pot h exists");
}

// ---------------- V3-PR-E4: contestable seats and the probation-pool contest (V3-10, V3-14)

const CONTEST_SCENARIOS:[&str;23]=["acc-count-promote-exit-displace-punish","contest-budget","contest-in-grace","contest-joined-after-the-measured-epoch-start",
    "counters-roll-at-the-epoch-boundary","late-credit-counts-in-the-statistic","omitted-shares-without-late-credit","pool-and-seats-full","pool-contest",
    "pool-contest-above-the-threshold-at-the-full-set","pool-contest-exits-cannot-raise-the-threshold","pool-rotation-race","pool-victim-pending-pots-burn",
    "pool-victim-pending-pots-without-a-contest","seat-contest-below-threshold-refund-after-u","seat-contest-probation-newcomer-accruing-victim",
    "seat-contest-refusals","seat-victims-punished-before-due","seat-victims-punished-then-swept","threshold-boundary-6-shares","threshold-boundary-7-shares",
    "tiny-epoch-never-contestable","threshold-table"];
fn contest_body(n:&str)->serde_json::Value{let v:serde_json::Value=serde_json::from_str(CONTEST_VECTORS).unwrap();v[n].clone()}
fn contest_replay(s:&Setup)->BTreeMap<String,Result<(),String>>{
    let v:serde_json::Value=serde_json::from_str(CONTEST_VECTORS).unwrap();
    v.as_object().unwrap().iter().filter(|(_,b)|b.get("steps").is_some()).map(|(n,body)|(n.clone(),budget_scenario(n,body,s))).collect()
}
/// contestThreshold(tprev, active) with theta_bp and k_sigma in CP 64/65: (threshold, isqrt(k^2*N*Dn)).
fn contest_threshold(h:&mut Harness,tprev:U256,active:u64,theta_bp:u64,k_sigma:u64)->Result<(U256,U256),String>{
    put(&mut h.db,caps(),U256::from(64),U256::from(theta_bp));put(&mut h.db,caps(),U256::from(65),U256::from(k_sigma));
    let mut d=selector("contestThreshold(uint256,uint256)");d.extend(tprev.to_be_bytes::<32>());d.extend(U256::from(active).to_be_bytes::<32>());
    let r=call(&mut h.db,name("viewer"),d,U256::ZERO,h.height.max(1));
    if !r.result.is_success(){return Err(format!("contestThreshold reverted: {:?}",output(&r.result)))}
    let o=output(&r.result);Ok((U256::from_be_slice(&o[0..32]),U256::from_be_slice(&o[32..64])))
}
/// Every row of the model's threshold table (tests/v3/contest-vectors.json) through contestThreshold on a registry.
fn threshold_table_on(registry:Bytes)->Result<usize,String>{
    let t=contest_body("threshold-table");let mut h=Harness::new(&[],&Setup::v3(8,60_480).with_registry(registry));
    let rows=t["rows"].as_array().unwrap();
    for row in rows{
        let num=|k:&str|row[k].as_u64().unwrap();let big=|k:&str|row[k].as_str().unwrap().parse::<U256>().unwrap();
        let got=contest_threshold(&mut h,big("tprev"),num("active"),num("theta_bp"),num("k_sigma"))?;
        ensure_eq!(got,(big("threshold"),big("root")),"threshold row {}",row["label"]);
    }
    Ok(rows.len())
}

/// Model-vs-bytecode differential of V3-10/V3-14: every scenario of tests/v3/contest-vectors.json reproduces on the v3
/// programs step by step (each contestStatus view's code, count and threshold included), with the records' epoch counters,
/// the epoch totals, ACC_COUNT, held_total, the Escrow invariant and the supply after every step.
#[test]
fn v3_contests_reproduce_the_independent_model(){
    let out=contest_replay(&Setup::v3(8,60_480));
    let mut names:Vec<&str>=out.keys().map(|k|k.as_str()).collect();names.push("threshold-table");
    assert_eq!(names,CONTEST_SCENARIOS);
    for (n,r) in &out{if let Err(e)=r{panic!("{n}: {e}")}}
    // The vectors exercise what they claim: a contest landing and each way it is refused, every view code but 3 (the pool
    // contest's HELD victim, a BadState revert), and the epoch counters of a record.
    let v:serde_json::Value=serde_json::from_str(CONTEST_VECTORS).unwrap();
    let (mut seen,mut codes,mut counted)=(BTreeSet::new(),BTreeSet::new(),0usize);
    for body in v.as_object().unwrap().values(){
        let Some(steps)=body.get("steps") else{continue};
        for st in steps.as_array().unwrap(){
            let op=st["op"][0].as_str().unwrap();
            if op=="register_contest"{seen.insert(st.get("revert").and_then(|x|x.as_str()).unwrap_or("ok").to_string());}
            if op=="contest_status"{codes.insert(st["result"].as_str().unwrap().split(',').next().unwrap().to_string());}
        }
        counted+=body["final"]["records"].as_object().unwrap().values().filter(|r|r["cnt_cur"].as_u64().unwrap()>0&&r["cnt_prev"].as_u64().unwrap()>0).count();
    }
    assert_eq!(seen,BTreeSet::from(["ok","NotContestable","BadState","RegOpsLimit"].map(String::from)));
    assert_eq!(codes,BTreeSet::from(["0","1","2","4","5","6","7"].map(String::from)));
    assert!(counted>0,"no record holds both epoch counters");
}

/// The same vectors on the earn-in set, which has no registerContest and keeps no epoch counters: every scenario differs.
#[test]
fn contest_vectors_fail_on_the_earnin_set(){
    let v:serde_json::Value=serde_json::from_str(CONTEST_VECTORS).unwrap();
    for (n,body) in v.as_object().unwrap(){
        if body.get("steps").is_none(){continue}
        let mut b=body.clone();b["cp"].as_object_mut().unwrap().retain(|k,_|cp_index(k)<52);
        let r=budget_scenario(n,&b,&Setup::earnin());
        eprintln!("earn-in {n}: {}",r.as_ref().err().map_or("reproduces",|e|e.as_str()));
        assert!(r.is_err(),"{n} reproduces on the earn-in set: it does not test V3-10/V3-14");
    }
}

/// The threshold table: contestThreshold returns the model's threshold and integer square root for every row (the owner's
/// values, k_sigma 0, theta 0, one seat, k^2*N*Dn a perfect square, one below a square where an unfloored Newton estimate
/// is the root plus one, and huge values); active 0 reverts.
#[test]
fn the_threshold_table_reproduces_on_the_bytecode(){
    assert_eq!(threshold_table_on(runtime(V3,"MinerRegistry")),Ok(27));
    let t=contest_body("threshold-table");
    assert!(!t["sensitive"].as_array().unwrap().is_empty(),"rows where an isqrt one off moves the threshold");
    let owner=t["rows"].as_array().unwrap().iter().find(|r|r["label"]=="owner-e45-256-seats").unwrap();
    assert_eq!(owner["threshold"],"25","V3-DESIGN.md: E = 45 at 256 seats, threshold about 25 shares per day");
    let mut h=Harness::new(&[],&Setup::v3(8,60_480));
    assert!(contest_threshold(&mut h,U256::from(100),0,1250,3).is_err(),"active 0");
}

/// The thirteen planted V3-10/V3-14 mutants each fail tests/v3/contest-vectors.json (a chain scenario or the threshold
/// table), each in the scenario written for it.
#[test]
fn the_contest_mutants_fail_the_contest_vectors(){
    let list=mutants();
    let contest=&list[list.len()-CONTEST_MUTANTS.len()..];
    let expect=[("contest-threshold-off-by-one","threshold-boundary-7-shares"),("contest-grace-ignored","contest-in-grace"),
        ("contest-joined-in-epoch-allowed","contest-joined-after-the-measured-epoch-start"),("contest-set-not-full-allowed","seat-contest-refusals"),
        ("pool-contest-set-not-full-allowed","pool-contest-exits-cannot-raise-the-threshold"),("contest-late-window-open","seat-contest-below-threshold-refund-after-u"),("pool-contest-refund-skipped","pool-contest"),
        ("pool-contest-exits-instead","pool-contest"),("pool-contest-held-victim","pool-contest"),("counters-not-rolled","counters-roll-at-the-epoch-boundary"),
        ("late-not-counted","late-credit-counts-in-the-statistic"),("isqrt-not-floored","threshold-table"),("contest-one-reg-ops-unit","contest-budget")];
    let original=runtime(V3,"MinerRegistry");
    for ((n,code),(want,killer)) in contest.iter().zip(expect){
        assert_eq!(n,want);assert_ne!(code,&original,"{n} changes the bytecode");
        let mut killed:Vec<String>=contest_replay(&Setup::v3(8,60_480).with_registry(code.clone())).into_iter().filter(|(_,r)|r.is_err()).map(|(k,_)|k).collect();
        if threshold_table_on(code.clone()).is_err(){killed.push("threshold-table".into());}
        eprintln!("mutant {n}: killed by {killed:?}");
        assert!(killed.iter().any(|k|k==killer),"mutant {n} survives {killer}: {killed:?}");
    }
}

/// Runs a contest scenario's steps until the first step matching `stop` (not run), for measurements at that point.
fn contest_until(name_:&str,stop:impl Fn(&[String])->bool)->Harness{
    let body=contest_body(name_);let mut s=Setup::v3(8,60_480);
    for (k,v) in body["cp"].as_object().unwrap(){s.cp.push((cp_index(k),v.as_str().map_or_else(||v.as_u64().unwrap().into(),|x|x.parse::<u128>().unwrap())));}
    let durable:Vec<String>=body["durable"].as_array().unwrap().iter().map(|x|x.as_str().unwrap().to_string()).collect();
    let mut h=Harness::new(&durable,&s);
    for step in body["steps"].as_array().unwrap(){
        let op:Vec<String>=step["op"].as_array().unwrap().iter().map(|x|x.as_str().unwrap().to_string()).collect();
        if stop(&op){return h}
        match op[0].as_str(){"begin"=>{h.begin(step).unwrap();},"close"|"final"=>h.phase2(step).unwrap(),_=>{h.step(&op).unwrap();}}
    }
    panic!("{name_}: no step to stop at")
}
/// Changed storage slots of the system programs (the user context's SYS keys, user_phase.rs) in one call.
fn sys_keys(r:&ResultAndState)->usize{
    (0..5).map(|i|r.state.get(&pocol_lp4_execution::apply::program_address(i)).map(|a|a.storage.values().filter(|s|s.is_changed()).count()).unwrap_or(0)).sum()
}

/// registerContest's write ceilings, gas and user-context SYS keys: the seat contest with the victim in the middle of the
/// active list (as deregister middle, plus a registration) and at its tail, and the pool contest (refund, erase, register)
/// with the victim in the middle and with a bonded victim at the tail. Each stays within half the wallet's CONTEST_GAS
/// quote and the devnet's SYS_KEYS_USER_MAX (256): one contest leaves room for the block's other user operations.
#[test]
fn contest_write_ceilings_gas_and_user_sys_keys(){
    let mut out=Vec::new();
    for (scenario_,sender,victim,value) in [("contest-joined-after-the-measured-epoch-start","n1","c",b()),("seat-contest-below-threshold-refund-after-u","n","c",b()),
        ("pool-and-seats-full","q","p2",U256::ZERO),("pool-victim-pending-pots-burn","q","p2",U256::ZERO)]{
        let mut h=contest_until(scenario_,|op|op[0]=="register_contest"&&op[1]==sender&&op[3]==victim&&op[5]=="19");
        let s=h.sender(sender);let mut d=with_arg("registerContest(address,address)",Some(s));d.extend(name(victim).into_word());
        let r=call(&mut h.db,s,d,value,h.height);
        assert!(r.result.is_success(),"{scenario_}: {:?} {:?}",r.result,output(&r.result));
        out.push((scenario_,writes(&r),sys_keys(&r),r.result.gas_used()));
        conserved(&h).unwrap();h.chain_conserved().unwrap();
    }
    eprintln!("CONTEST-CEILINGS (scenario, registry slots, user SYS keys, gas) {out:?}");
    // Changed slots (a slot written back to its value is unchanged). Seat contest, victim in the middle: the victim's w0
    // and w3, the moved record's w0, the two ActList elements (the newcomer takes the freed last one; ActLen nets out),
    // the newcomer's four words, its RecArr element, RecLen, held_total and slot 3: 13; at the tail 11 (no moved record).
    // Pool contest, victim and newcomer at bond 0, victim in the middle of both lists: the moved record's w0 (both
    // positions), two ActList and two RecArr elements (RecLen, ActLen and ACC_COUNT net out, held_total unchanged), the
    // victim's w0, w1 and w3 zeroed (w2 is already 0), the newcomer's w0, w1 and w3, slot 3: 12; a 5 PCOL victim at the
    // tail: its w2 and held_total as well, no moved record: 11.
    assert_eq!(out.iter().map(|x|x.1).collect::<Vec<_>>(),[13,11,12,11]);
    // pocold's wallet quotes CONTEST_GAS (500,000) for registerContest: at least twice every contest measured here.
    for (n,_,keys,gas) in &out{assert!(*keys<=256&&*gas<=250_000,"{n}: {keys} SYS keys, {gas} gas");}
}

/// V3-10 counters at their bounds, on the bytecode alone: a record's count saturates at 2^20 - 1 (a credit past it), rolls
/// into countPrev at the next epoch, and a saturated measured count is never contestable even below the threshold, while
/// one below saturation is.
#[test]
fn epoch_counters_saturate_and_a_saturated_count_is_never_contestable(){
    let sat=(1u64<<20)-1;
    let mut s=Setup::v3(8,60_480);s.cp.extend([(10,256),(63,8),(64,10_000),(65,1),(18,4),(25,4)]);
    let mut h=Harness::new(&[],&s);
    for x in ["a","b","c"]{assert_eq!(h.step(&strs(&["register",x,x,&b().to_string(),"1"])).unwrap(),None);}
    let c=name("c");let r=slot(c);
    let record=|h:&mut Harness,j:u64,seq:&[Address]|{
        let mut data=selector("recordShares(uint64,address[])");data.extend(U256::from(j).to_be_bytes::<32>());data.extend(U256::from(64).to_be_bytes::<32>());
        data.extend(U256::from(seq.len()).to_be_bytes::<32>());for a in seq{data.extend(a.into_word());}h.sys("recordShares",registry_address(),data).unwrap();
    };
    // c's record in epoch 1 (lastShare 8) with countCur 2^20 - 3: five shares of template 9 saturate it.
    h.at(10).unwrap();let w0=storage(&h.db,registry_address(),r);
    put(&mut h.db,registry_address(),r,(w0&!(U256::MAX<<88u32))|(U256::from(8)<<88u32)|(U256::from(sat-2)<<216u32));
    record(&mut h,9,&[c,c,c,c,c]);
    let w0=storage(&h.db,registry_address(),r);assert_eq!((bits(w0,88,64),bits(w0,216,20),bits(w0,236,20)),(9,sat,0),"saturated at 2^20 - 1");
    // Template 16 (epoch 2) rolls it into countPrev.
    h.at(17).unwrap();record(&mut h,16,&[c]);
    let w0=storage(&h.db,registry_address(),r);assert_eq!((bits(w0,88,64),bits(w0,216,20),bits(w0,236,20)),(16,1,sat),"rolled into countPrev");
    // At 19 epoch 1 is measured: c's count is countPrev = 2^20 - 1. With Tprev = 2^24 the threshold (about 2^22) is far
    // above it, yet a saturated count may hide any larger one: code 7. One below saturation is contestable.
    h.at(19).unwrap();
    let t=U256::from(2)|(U256::from(1)<<64u32)|(U256::from(1u64<<24)<<160u32);put(&mut h.db,registry_address(),U256::from(10),t);
    let (code,count,thr)=h.contest_status(c,false);
    assert_eq!((code,count),(7,sat));assert!(thr>U256::from(sat),"threshold {thr}");
    let w0=storage(&h.db,registry_address(),r);put(&mut h.db,registry_address(),r,(w0&!(U256::from(sat)<<236u32))|(U256::from(sat-1)<<236u32));
    assert_eq!(h.contest_status(c,false),(0,sat-1,thr));
    let mut d=with_arg("registerContest(address,address)",Some(name("n")));d.extend(c.into_word());let n=h.sender("n");
    let r=call(&mut h.db,n,d,b(),19);assert!(r.result.is_success(),"{:?}",output(&r.result));
    assert_eq!(bits(storage(&h.db,registry_address(),slot(c)),0,8),2,"c exits");
}

/// registerContest's call shape: exactly 68 bytes with two clean address words (BadCalldata), at most B_reg (BadValue), only
/// after the block's beginBlock (BadPhase), a sender without a record and a recorded victim (BadState); contestStatus and
/// contestThreshold are views.
#[test]
fn v3_register_contest_call_shapes(){
    let mut h=contest_until("seat-contest-below-threshold-refund-after-u",|op|op[0]=="register_contest"&&op[5]=="19");
    let n=h.sender("n");let c=name("c");
    let call_=|h:&mut Harness,data:Vec<u8>,value:U256|->Option<String>{
        let r=call(&mut h.db,n,data,value,h.height);if r.result.is_success(){return None}
        let out=output(&r.result).to_vec();Some(["BadCalldata","BadValue","BadPhase","BadState","NotContestable","Capacity"].iter().find(|k|selector(&format!("{k}()"))==out).map(|k|k.to_string()).unwrap_or(format!("{out:?}")))
    };
    let mut d=with_arg("registerContest(address,address)",Some(n));d.extend(c.into_word());
    assert_eq!(call_(&mut h,d[..36].to_vec(),b()).as_deref(),Some("BadCalldata"),"36 bytes");
    let mut long=d.clone();long.extend([0u8;32]);assert_eq!(call_(&mut h,long,b()).as_deref(),Some("BadCalldata"),"100 bytes");
    let mut dirty=d.clone();dirty[4]=1;assert_eq!(call_(&mut h,dirty,b()).as_deref(),Some("BadCalldata"),"reward word upper bits");
    let mut dirty=d.clone();dirty[36]=1;assert_eq!(call_(&mut h,dirty,b()).as_deref(),Some("BadCalldata"),"victim word upper bits");
    assert_eq!(call_(&mut h,d.clone(),b()+U256::from(1)).as_deref(),Some("BadValue"),"above B_reg");
    let mut stranger=with_arg("registerContest(address,address)",Some(n));stranger.extend(name("nobody").into_word());
    assert_eq!(call_(&mut h,stranger,b()).as_deref(),Some("BadState"),"a victim without a record");
    // A later height whose beginBlock has not run.
    let r=call(&mut h.db,n,d.clone(),b(),h.height+1);assert_eq!(output(&r.result),selector("BadPhase()").as_slice(),"before beginBlock");
    assert_eq!(call_(&mut h,d.clone(),b()),None,"the shape the wallet sends");
    assert_eq!(call_(&mut h,d,b()).as_deref(),Some("BadState"),"the sender now has a record");
}

#[cfg(feature="v3-preview")]
mod preview{
    //! Real blocks through apply_block on a specVersion-2 devnet (the v3 program set), so through the host's v3 phase 0
    //! (apply.rs): the parent's result recorded first, invalidate(j, accused), punish(accused, reporter = the signed
    //! proposer). The spec-1 earn-in devnet runs the same blocks for the fails-on-original comparison.
    use super::*;
    use pocol_lp4_execution::{apply::{self,BlockInput,BlockOutput,EvidenceEffect,ParentResult,Proposer},admission,devnet::{self,Devnet},resources,txsign::Tx};
    use revm::primitives::B256;

    struct Run{net:Devnet,state:InMemoryDB,prev:Option<ParentResult>,h:u64,base:U256,gas:BTreeMap<&'static str,u64>,keys:usize,accounts:usize,burnt_fees:U256,issued:U256,supply:U256}
    fn total(db:&InMemoryDB)->U256{db.accounts.values().map(|a|a.info.balance).fold(U256::ZERO,|a,b|a+b)}
    impl Run{
        fn new(cfg:&DevnetConfig)->Self{
            let net=devnet::build(cfg).unwrap();let state=net.genesis.state().clone();let supply=total(&state);
            Run{net,state,prev:None,h:0,base:U256::from(1_000_000_000u64),gas:BTreeMap::new(),keys:0,accounts:0,burnt_fees:U256::ZERO,issued:U256::ZERO,supply}
        }
        fn id(&self,i:usize)->Address{self.net.miners[i].id}
        fn reward(&self,i:usize)->Address{self.net.miners[i].reward}
        /// Block h+1 with this proposer, evidence and transactions; its own result (consumed by the next block) is `winner`
        /// with `shares`.
        fn block(&mut self,proposer:Proposer,evidence:Vec<EvidenceEffect>,txs:Vec<Vec<u8>>,winner:Address,shares:Vec<(u64,Address)>)->BlockOutput{
            self.h+=1;let h=self.h;
            let input=BlockInput{height:h,timestamp:1_700_000_000+4*h,base_fee:self.base,gas_limit:8_000_000,prevrandao:B256::ZERO,proposer,
                parent_result:self.prev.clone(),evidence,evidence_raw:&[],transactions:&txs,ancestors:vec![],cum_log_bytes:0};
            let o=apply::apply_block(&self.state,&self.net.genesis,&self.net.profile,&input).unwrap_or_else(|e|panic!("h={h}: {e:?}"));
            for (n,g) in &o.meter.call_gas{let e=self.gas.entry(*n).or_default();*e=(*e).max(*g);}
            self.keys=self.keys.max(o.meter.keys.len());self.accounts=self.accounts.max(o.meter.accounts.len());
            self.burnt_fees+=U256::from(o.gas_used)*self.base;self.issued+=o.issued;
            self.state=o.state.clone();self.prev=Some(ParentResult{block_hash:B256::from(U256::from(h)),winner,shares});
            // The supply: genesis plus issuance less the burned base fees.
            assert_eq!(total(&self.state)+self.burnt_fees,self.supply+self.issued,"supply at {h}");
            o
        }
        fn register(&self,label:&str,value:u128,nonce:u64)->Vec<u8>{
            let key=devnet::account_key(label);let mut data=selector("register(address)");data.extend(key.address().into_word());
            Tx{chain_id:self.net.profile.spec().chain_id,nonce,max_priority:U256::ZERO,max_fee:self.base,gas:400_000,to:Some(registry_address()),value:U256::from(value),data:Bytes::from(data)}.sign_1559(&key)
        }
        fn bal(&self,a:Address)->U256{balance(&self.state,a)}
    }
    fn config(spec:u8,miners:&[&str])->DevnetConfig{
        let mut cfg=DevnetConfig::new(777943,1_700_000_000,miners);cfg.h_end=0;cfg.earnin=true;cfg.spec_version=spec;cfg.r_max=8;cfg
    }

    /// V3-16/V3-17 through the host: block 3 carries type-2 evidence against b, the HELD winner of pot 1, and against
    /// alice, the ACCRUING (zero-bond) winner of pot 2, with signed proposer a as the reporter. On v3 pot 1 settles
    /// normally (c and a paid their shares, b's parts burn), pot 2 burns whole (alice was ACCRUING when its result was
    /// recorded, in block 3's phase 0 before the evidence), and a's reward receives 25% of b's 20 PCOL bond. The spec-1
    /// earn-in devnet runs the same blocks and burns both pots whole, paying no reporter (fails on the original).
    #[test]
    fn v3_host_runs_the_evidence_penalties_on_real_blocks(){
        let mut out=Vec::new();
        for spec in [1u8,2]{
            let mut r=Run::new(&config(spec,&["a","b","c"]));
            let (a,b_,c)=(r.id(0),r.id(1),r.id(2));let alice=devnet::account_key("alice").address();
            let t=r.register("alice",0,0);
            r.block(Proposer::Signed(a),vec![],vec![t],b_,vec![(1,a),(2,c),(3,c)]);
            assert_eq!(apply::miner_record(&r.state,alice).unwrap().bond_state,4,"alice is on probation");
            r.block(Proposer::Signed(c),vec![],vec![],alice,vec![(1,c)]);
            let before:Vec<U256>=(0..3).map(|i|r.bal(r.reward(i))).collect();
            let o=r.block(Proposer::Signed(a),vec![EvidenceEffect::PunishAndInvalidate{accused:b_,j:1},EvidenceEffect::PunishAndInvalidate{accused:alice,j:2}],vec![],c,vec![]);
            let (p1,p2)=(pot(&o.state,1),pot(&o.state,2));
            let reporter_cut=r.bal(r.reward(0))-before[0];
            let mut a6=U256::ZERO;
            for _ in 0..7{let w=r.id(2);let o=r.block(Proposer::Fallback,vec![],vec![],w,vec![]);if r.h==6{a6=pot(&o.state,6).0;}}
            let gains:Vec<U256>=(0..3).map(|i|r.bal(r.reward(i))-before[i]).collect();
            out.push((p1.1>>168u32,p2.1>>168u32,p2.2>>160u32,reporter_cut,gains,storage(&r.state,escrow_address(),U256::ZERO),a6));
        }
        let (earnin,v3)=(&out[0],&out[1]);
        eprintln!("spec-1 earn-in {earnin:?}\nv3 {v3:?}");
        // INVALID bits after block 3: both pots on the earn-in set, only pot 2 on v3 (with its ACCRUING winner bit).
        assert_eq!((earnin.0,earnin.1),(U256::from(1),U256::from(1)));
        assert_eq!((v3.0,v3.1,v3.2),(U256::ZERO,U256::from(1),U256::from(1)));
        assert_eq!(earnin.3,U256::ZERO,"no reporter on the earn-in set");
        assert_eq!(v3.3,U256::from(5*PCOL),"25% of b's 20 PCOL bond to the signed proposer a's reward address");
        // Pot 1 (2 PCOL, 3 header shares + the winner's): a holds 1 share, c 2; R = 1.7 over Q = max(4, kappa*m = 64).
        let r=U256::from(17*PCOL/10);let share=|n:u64|r*U256::from(n)/U256::from(64);
        // Pot 6 (fallback, won by c, C = 1) settles at h = 10. On v3 it also holds pot 1's base burn R - floor(R·4/64), the
        // carry of settle(1) in block 5 into pot 6 (V3-15, lagged by one block), so c's winner part, proposer part and share
        // of it grow by exactly that (pot 5, settled at h = 9, is 2 PCOL on both sets: no settlement ran before block 5).
        assert_eq!(earnin.6,U256::from(2*PCOL));assert_eq!(v3.6-earnin.6,r-r*U256::from(4)/U256::from(64),"pot 6 carries pot 1's base burn");
        let c_parts=|a:U256|{let (wp,pp)=(a/U256::from(10),a*U256::from(500)/U256::from(10_000));wp+pp+(a-wp-pp)/U256::from(64)};
        assert_eq!(v3.4[2]-earnin.4[2],share(2)+c_parts(v3.6)-c_parts(earnin.6),"c is paid its pot 1 shares on v3 only, and pot 6's carry");
        assert_eq!(v3.4[0]-earnin.4[0],share(1)+U256::from(5*PCOL)+U256::from(PCOL/10),"a: its share, the reporter cut and the pot 1 proposer part");
        assert_eq!(v3.4[1],earnin.4[1],"b, tombed, receives nothing more on either set");
    }

    /// V3-02 through the host: block 2's winner b omits a's share; block 3 (signed by c) carries Late{j: 2, owners: [a, a]}.
    /// On v3 settle(2) at h = 6 pays a two shares of pot 2 (R = 1.7 PCOL over Q = 64) and a's lastShare is 2; the spec-1
    /// earn-in set refuses the effect (no recordLate: apply fails before any call), so there a is paid nothing.
    #[test]
    fn v3_host_runs_late_credit_on_real_blocks(){
        let mut gains=Vec::new();
        for spec in [1u8,2]{
            let mut r=Run::new(&config(spec,&["a","b","c"]));
            let (a,b_,c)=(r.id(0),r.id(1),r.id(2));
            r.block(Proposer::Signed(a),vec![],vec![],c,vec![]);
            r.block(Proposer::Signed(c),vec![],vec![],b_,vec![(7,c)]);
            let before=r.bal(r.reward(0));
            let late=vec![EvidenceEffect::Late{j:2,owners:vec![a,a]}];
            if spec==1{
                let input=BlockInput{height:3,timestamp:1_700_000_012,base_fee:r.base,gas_limit:8_000_000,prevrandao:B256::ZERO,proposer:Proposer::Signed(c),
                    parent_result:r.prev.clone(),evidence:late,evidence_raw:&[],transactions:&[],ancestors:vec![],cum_log_bytes:0};
                assert_eq!(apply::apply_block(&r.state,&r.net.genesis,&r.net.profile,&input).err(),Some(apply::ApplyError::Context("late shares need the v3 program set")));
                r.block(Proposer::Signed(c),vec![],vec![],c,vec![]);
            }else{
                let o=r.block(Proposer::Signed(c),late,vec![],c,vec![]);
                assert_eq!(apply::share_total(&o.state,2),4,"the header's two shares and the two late ones");
                assert_eq!(apply::share_count(&o.state,2,a),2);
                assert_eq!(apply::miner_record(&o.state,a).unwrap().last_share,2);
            }
            for _ in 0..4{let w=r.id(2);r.block(Proposer::Fallback,vec![],vec![],w,vec![]);}
            gains.push(r.bal(r.reward(0))-before);
        }
        let share=U256::from(17*PCOL/10)/U256::from(64);
        assert_eq!(gains[1]-gains[0],share*U256::from(2),"a's two late shares of pot 2, on v3 only");
    }

    /// SYS gas and caps of the v3 calls at the GAS_DOMAIN maxima (S_max 256, M_max 256, D 16, E_max 4) with the v3
    /// probation pool full (P_MAX 64): chains of 192 genesis members and 64 probationers p0..p63 joining in blocks 1..4
    /// (M_max records), every block from 5 on naming all 256 owners in its shares. The worst cases of the calls
    /// V3-PR-E2b changes, and of settlement on the v3 set:
    /// - settle (chain A, run twice): pot 5, raised by a large tip and won by p63, settles in block 21 while all 64
    ///   probationers are live, ACCRUING and at bond 0, as the chain's first burn: exactly 64 bonds go from 0 to B and are
    ///   promoted in that one call, besides 192 HELD payees; pot 5's proposer is HELD on one run, ACCRUING on the other;
    /// - recordResult reads an ACCRUING winner's record (pot 5 on chain A, pots 5..8 on chain B);
    /// - invalidate (chain B, run twice, probationers with half a bond, 0 < B/2 < B): four type-2 items name p0..p3, the
    ///   ACCRUING winners of pots 5..8, each writing INVALID;
    /// - punish: the same items punish an ACCRUING accused with a nonzero bond in the middle of the active list (the
    ///   ACC_COUNT write, removeActive, the bond burn and the reporter's PAY to the signed proposer's reward address);
    ///   four type-1 items punish HELD genesis members (never-written clock word, middle of the active list, the
    ///   reporter's PAY). Each kind comes first on one run of chain B, so each also pays the chain's first burn.
    /// Each call stays within admission's envelope for the v3 set (GAS_ENVELOPE_V3 where it has an entry, GAS_ENVELOPE_EARNIN
    /// otherwise: `gas_envelope(ProgramSet::V3)`), every call name measured has an entry there, and the blocks
    /// within resources::derive; the test asserts that the settle and punish maxima are taken on these paths.
    #[test]
    fn v3_sys_gas_and_caps_within_the_v3_envelope(){
        let labels:Vec<String>=(0..192).map(|i|format!("m{i}")).collect();let refs:Vec<&str>=labels.iter().map(|s|s.as_str()).collect();
        let mut cfg=config(2,&refs);cfg.d=16;cfg.w_w=64;cfg.s_max=256;cfg.m_max=256;cfg.e_win=2048;cfg.u=2200;cfg.w_act=128;cfg.theta_inact=1024;cfg.c_react=2200;
        cfg.sys_slot_max=131072;cfg.slot_cap=4_000_000;cfg.acct_cap=1_000_000;cfg.p_max=64;cfg.r_max=16;
        let probation:Vec<String>=(0..64).map(|i|format!("p{i}")).collect();
        for p in &probation{cfg.accounts.push((p.clone(),U256::from(20*PCOL)));}
        let pids:Vec<Address>=probation.iter().map(|p|devnet::account_key(p).address()).collect();
        // Chain B starts with its 64 probationers joining in blocks 1..4 with `bond` each (the shares of blocks 1..4 name
        // only the 192 genesis members, so pots 1..4 pay no probationer).
        let net=Run::new(&cfg);let cp=net.net.profile.cp().clone();
        let genesis:Vec<Address>=(0..192).map(|i|net.id(i)).collect();
        let ids:Vec<Address>=genesis.iter().copied().chain(pids.iter().copied()).collect();
        let start=|bond:u128|->Run{
            let mut r=Run::new(&cfg);
            for k in 0..4usize{
                let joins:Vec<Vec<u8>>=probation[16*k..16*k+16].iter().map(|p|r.register(p,bond,0)).collect();
                let sh=(0..256u64).map(|i|(i*3+1,ids[((k as u64+i) as usize)%192])).collect();
                r.block(Proposer::Signed(genesis[100+k]),vec![],joins,genesis[k],sh);
            }
            for p in &pids{assert_eq!(apply::miner_record(&r.state,*p).map(|x|(x.status,x.bond_state,x.bond,x.tomb)),Some((1,4,U256::from(bond),false)),"an ACCRUING probationer");}
            r
        };
        let shares=|ids:&[Address],h:u64|->Vec<(u64,Address)>{(0..256u64).map(|i|(i*3+1,ids[((h+i) as usize)%256])).collect()};

        let mut gas:BTreeMap<&'static str,u64>=BTreeMap::new();let (mut keys,mut accounts)=(0usize,0usize);
        let mut fold=|r:&Run|{for (n,g) in &r.gas{let e=gas.entry(*n).or_default();*e=(*e).max(*g);}keys=keys.max(r.keys);accounts=accounts.max(r.accounts);};
        // Chain A: the worst settlement. Pots 1..4 burn nothing (one owner, the HELD winner, holds every share, and
        // C = 256 >= kappa*m), so pot 5, raised by a large tip and won by p63, settles in block 21 as the chain's first
        // burn (Escrow's burnedTotal from 0 to nonzero). All 64 probationers are then live, ACCRUING and at bond 0 (no
        // evidence names them on this chain), and pot 5 pays each more than B: exactly 64 bonds go from 0 to B and are
        // promoted in that one call (p63 through its winner part, p0..p62 through their shares), besides 192 HELD payees.
        // Pot 5's proposer is a HELD genesis member or the ACCRUING p62 (whose proposer part then fills its bond through
        // payTo before distribute).
        // The tip split (V3-20) is off on chain A (gamma_tip_bp 0): a tip share would fill the ACCRUING proposer p62's bond
        // at closeBlock(5); closeBlock's own worst path is measured in v3_close_and_finalize_gas_within_the_v3_envelope.
        let mut settle5=0u64;let mut cfg_a=cfg.clone();cfg_a.gamma_tip_bp=0;
        for proposer in [genesis[110],pids[62]]{
            let mut a=Run::new(&cfg_a);
            for k in 0..4usize{
                let joins:Vec<Vec<u8>>=probation[16*k..16*k+16].iter().map(|p|a.register(p,0,0)).collect();
                a.block(Proposer::Signed(genesis[100+k]),vec![],joins,genesis[k],(0..256u64).map(|i|(i+1,genesis[k])).collect());
            }
            let alice=devnet::account_key("alice");let tip=U256::from(400_000_000_000_000_000u128);
            let big=Tx{chain_id:777943,nonce:0,max_priority:tip,max_fee:a.base+tip,gas:21_000,to:Some(devnet::account_key("bob").address()),value:U256::ZERO,data:Bytes::new()}.sign_1559(&alice);
            let o=a.block(Proposer::Signed(proposer),vec![],vec![big],pids[63],shares(&ids,5));
            assert!(o.tips>U256::from(8000*PCOL),"pot 5 tips {}",o.tips);
            for k in 6..=20u64{a.block(Proposer::Signed(genesis[(114+k) as usize]),vec![],vec![],genesis[k as usize],shares(&ids,k));}
            for p in &pids{assert_eq!(apply::miner_record(&a.state,*p).map(|x|(x.status,x.bond_state,x.bond,x.tomb)),Some((1,4,U256::ZERO,false)),"zero-bond ACCRUING until pot 5 settles");}
            assert_eq!(storage(&a.state,escrow_address(),U256::ZERO),U256::ZERO,"pots 1..4 burned nothing");
            let o=a.block(Proposer::Signed(genesis[140]),vec![],vec![],genesis[21],shares(&ids,21));
            let g=o.meter.call_gas.iter().find(|(n,_)|*n=="settle").unwrap().1;
            for p in &pids{assert_eq!(apply::miner_record(&a.state,*p).map(|x|(x.bond_state,x.bond,x.tomb)),Some((1,b(),false)),"filled from 0 to B and promoted by pot 5's settlement");}
            assert!(storage(&a.state,escrow_address(),U256::ZERO)>U256::ZERO,"pot 5's settlement burns");
            eprintln!("settle(5) with proposer {proposer}: {g}");settle5=settle5.max(g);fold(&a);
        }

        // Chain B: type-2 evidence against ACCRUING winners with a nonzero bond (block 9 or 10) and type-1 evidence against
        // HELD genesis members (the other block). Each order once, so that each path also pays the first burn of the
        // chain (Escrow's burnedTotal from 0 to nonzero), which only the first punish of a chain does.
        let half=10*PCOL;let (mut punish_acc,mut punish_held)=(0u64,0u64);
        for held_first in [false,true]{
            let mut c=start(half);
            for k in 0..4usize{c.block(Proposer::Signed(genesis[120+k]),vec![],vec![],pids[k],shares(&ids,5+k as u64));}
            for step in 0..2u64{
                let rep=130+step as usize;let reporter=c.net.miners[rep].reward;let before=c.bal(reporter);
                if held_first==(step==0){
                    let ev=(0..4usize).map(|i|EvidenceEffect::Punish{accused:genesis[40+i]}).collect();
                    let o=c.block(Proposer::Signed(genesis[rep]),ev,vec![],genesis[5+step as usize],shares(&ids,9+step));
                    punish_held=punish_held.max(o.meter.call_gas.iter().filter(|(n,_)|*n=="punish").map(|x|x.1).max().unwrap());
                    assert_eq!(c.bal(reporter)-before,U256::from(4*b().to::<u128>()/4),"rho_bp 2500 of each 20 PCOL bond to the signed proposer's reward address");
                }else{
                    let ev=(0..4usize).map(|i|EvidenceEffect::PunishAndInvalidate{accused:pids[i],j:5+i as u64}).collect();
                    let o=c.block(Proposer::Signed(genesis[rep]),ev,vec![],genesis[5+step as usize],shares(&ids,9+step));
                    punish_acc=punish_acc.max(o.meter.call_gas.iter().filter(|(n,_)|*n=="punish").map(|x|x.1).max().unwrap());
                    assert_eq!(c.bal(reporter)-before,U256::from(4*half/4),"rho_bp 2500 of each half bond to the signed proposer's reward address");
                    for j in 5..9u64{assert_eq!(pot(&o.state,j).1>>168u32,U256::from(1),"pot {j} INVALID");}
                }
            }
            for x in pids[..4].iter().chain(genesis[40..44].iter()){assert!(apply::miner_record(&c.state,*x).unwrap().tomb);}
            fold(&c);
        }

        // Chain C (V3-02): recordLate at its worst. Pots 5..8 carry only their winners' shares (C_header = 1), so the
        // no-dilution cap (kappa*m = 64) admits kappa*m - 1 = 63 late shares each. Block 9 carries E_max = 4 kind-3
        // effects, j = 5..8, each crediting 63 distinct owners new to the ledger's list for j, the first call of the
        // block reaching every owner's record cold. Run once with genesis members (each record's lastShare word
        // written: lastShare < j) and once with owners without a record (recordLate reads the record's four words to
        // check it is erased, which costs more than a write of a warm-loaded word).
        let mut late_rec=(0u64,0u64);
        for strangers in [false,true]{
            let mut c=Run::new(&cfg);
            for k in 1..=8u64{let sh=if k<=4{(0..192u64).map(|i|(i+1,genesis[i as usize])).collect()}else{vec![]};c.block(Proposer::Signed(genesis[100+k as usize]),vec![],vec![],genesis[k as usize],sh);}
            let owner=|i:usize|if strangers{Address::from_word(keccak256(format!("late-owner-{i}")).into())}else{genesis[i%192]};
            let ev=(0..4usize).map(|t|EvidenceEffect::Late{j:5+t as u64,owners:(0..63).map(|i|owner(63*t+i)).collect()}).collect();
            let o=c.block(Proposer::Signed(genesis[150]),ev,vec![],genesis[9],vec![]);
            let g=o.meter.call_gas.iter().filter(|(n,_)|*n=="recordLate").map(|x|x.1).max().unwrap();
            for j in 5..9u64{assert_eq!(apply::share_total(&o.state,j),64,"pot {j} filled to kappa*m");}
            if strangers{late_rec.1=g}else{late_rec.0=g;for i in 0..192{let l=apply::miner_record(&o.state,genesis[i]).unwrap().last_share;assert!(l>=5,"genesis {i}: lastShare {l} = max(4, the late heights)");}}
            fold(&c);
        }

        let b=resources::derive(&cp);
        eprintln!("V3-GAS {gas:?}; settle(5) {settle5}, punish ACCRUING {punish_acc} HELD {punish_held}, recordLate recorded {} strangers {}; keys {keys} (derive {}), accounts {accounts} (derive {})",late_rec.0,late_rec.1,b.keys_normal,b.accts_normal);
        assert_eq!(gas["recordLate"],late_rec.0.max(late_rec.1));
        assert!(late_rec.1>late_rec.0,"recordLate: owners without a record {} vs recorded owners {}",late_rec.1,late_rec.0);
        // The maxima are taken on the paths named above.
        assert_eq!(gas["settle"],settle5,"the worst settle is pot 5's 64 bond fills");
        assert_eq!(gas["punish"],punish_acc.max(punish_held));
        // A genesis member's clock word was never written (punish writes it from zero), while register writes every
        // probationer's (graceStart), which outweighs the ACCRUING path's ACC_COUNT write: the HELD path is the worst.
        assert!(punish_held>punish_acc,"punish HELD {punish_held} vs ACCRUING with a bond {punish_acc}");
        for (n,g) in &gas{
            let bound=admission::envelope(ProgramSet::V3,n).unwrap_or_else(||panic!("no envelope entry for {n}"));
            assert!(*g<=bound,"v3 {n} = {g} above its composed envelope {bound}");
        }
        // Every other name of the composed table was measured here (recordLate, the v3-only name, included), so no
        // ordinary or closing entry is unchecked. These chains are continuous (h_end 0), so the final phase is not
        // reached here: finalize is measured on v3 by v3_final_block_burns_the_carry_on_real_blocks (V3-PR-E2a, against
        // the same composed entry), and settleFinal runs the same settleOne as settle, so its entry is the settle maximum
        // measured here.
        for (n,_) in admission::gas_envelope(ProgramSet::V3){if !["finalize","settleFinal"].contains(n){assert!(gas.contains_key(n),"the v3 envelope names {n}, which no run measured");}}
        assert!(admission::gas_envelope(ProgramSet::V3).iter().any(|x|x.0=="recordLate"),"the composed v3 envelope names recordLate");
        assert!(gas["settle"]<=admission::envelope(ProgramSet::V3,"settleFinal").unwrap(),"settleFinal covers the measured settle");
        // The v3 table is needed: each of its measured entries exceeds the earn-in entry it replaces in the composition,
        // and the spec-1 entry of the same name.
        for n in ["invalidate","punish","recordResult","settle"]{
            assert!(gas[n]>admission::envelope(ProgramSet::EarnIn,n).unwrap(),"{n} {} fits the earn-in envelope: drop its v3 entry",gas[n]);
            assert!(gas[n]>admission::envelope(ProgramSet::V1,n).unwrap(),"{n} {} fits the spec-1 envelope: drop its v3 entry",gas[n]);}
        for set in [ProgramSet::V1,ProgramSet::Continuous,ProgramSet::EarnIn]{assert_eq!(admission::envelope(set,"recordLate"),None,"recordLate exists on the v3 set only: {set:?}");}
        assert!(keys as u64<=b.keys_normal&&accounts as u64<=b.accts_normal,"keys {keys} accounts {accounts} vs {b:?}");
    }

    /// markClose's closing phase on the v3 set at the GAS_DOMAIN maxima (review of V3-PR-E4: the larger registry dispatch
    /// adds gas to every nested Registry call, and each refund's Escrow.releaseBond calls back into bondRecord): 192 HELD
    /// genesis members and 64 ACCRUING probationers p0..p63 with half a bond (M_max records, P_MAX 64), RET 128, closing
    /// from H_CLOSE 8. The first closing markClose refunds 128 HELD records, the second 64 HELD and the 64 ACCRUING ones
    /// (each refund pays a bond and the ACCRUING ones also write ACC_COUNT). Both stay within admission's envelope for the
    /// v3 set (GAS_ENVELOPE_V3 markClose in the composed v3 table, above the earn-in and spec-1 entries), every record ends PAID and ACC_COUNT 0.
    #[test]
    fn v3_closing_mark_close_gas_within_the_envelope(){
        let labels:Vec<String>=(0..192).map(|i|format!("m{i}")).collect();let refs:Vec<&str>=labels.iter().map(|s|s.as_str()).collect();
        let mut cfg=config(2,&refs);cfg.d=16;cfg.w_w=64;cfg.s_max=256;cfg.m_max=256;cfg.e_win=2048;cfg.u=2200;cfg.w_act=128;cfg.theta_inact=1024;cfg.c_react=2200;
        cfg.sys_slot_max=131072;cfg.slot_cap=4_000_000;cfg.acct_cap=1_000_000;cfg.p_max=64;cfg.r_max=16;cfg.ret=128;cfg.h_close=8;cfg.h_end=30;
        let probation:Vec<String>=(0..64).map(|i|format!("p{i}")).collect();
        for p in &probation{cfg.accounts.push((p.clone(),U256::from(20*PCOL)));}
        let pids:Vec<Address>=probation.iter().map(|p|devnet::account_key(p).address()).collect();
        let mut r=Run::new(&cfg);let genesis:Vec<Address>=(0..192).map(|i|r.id(i)).collect();let half=10*PCOL;
        let mut closing=Vec::new();
        for h in 1..=10u64{
            let joins:Vec<Vec<u8>>=if h<=4{probation[16*(h as usize-1)..16*h as usize].iter().map(|p|r.register(p,half,0)).collect()}else{vec![]};
            let o=r.block(Proposer::Signed(genesis[100+h as usize]),vec![],joins,genesis[h as usize],vec![(1,genesis[h as usize])]);
            if h==4{for p in &pids{assert_eq!(apply::miner_record(&o.state,*p).map(|x|(x.status,x.bond_state,x.bond)),Some((1,4,U256::from(half))),"an ACCRUING probationer with half a bond");}}
            if o.facts.closing{closing.push((h,o.meter.call_gas.iter().find(|(n,_)|*n=="markClose").unwrap().1,storage(&o.state,registry_address(),U256::from(8))));}
        }
        eprintln!("MARK-CLOSE closing (height, gas, ACC_COUNT after): {closing:?}");
        assert!(closing.len()>=2&&closing[0].0==8,"closing from H_CLOSE: {closing:?}");
        assert_eq!((closing[0].2,closing[1].2),(U256::from(64),U256::ZERO),"128 HELD refunds, then 64 HELD and 64 ACCRUING");
        for id in genesis.iter().chain(&pids){assert_eq!(apply::miner_record(&r.state,*id).map(|x|x.bond_state),Some(3),"every record PAID");}
        let worst=closing.iter().map(|c|c.1).max().unwrap();let bound=admission::envelope(ProgramSet::V3,"markClose").unwrap();
        assert!(worst<=bound,"v3 markClose {worst} above its envelope {bound}");
        assert_eq!(worst,closing[1].1,"the worst refunds the 64 ACCRUING records");
        // The v3 entry is needed: the worst exceeds the earn-in entry it replaces in the composed v3 table (PoColSystemV3.sol
        // is built on the earn-in programs) and the spec-1 entry of the same name.
        assert!(worst>admission::envelope(ProgramSet::EarnIn,"markClose").unwrap(),"markClose {worst} fits the earn-in envelope: drop its v3 entry");
        assert!(worst>admission::envelope(ProgramSet::V1,"markClose").unwrap(),"markClose {worst} fits the spec-1 envelope: drop its v3 entry");
    }

    /// V3-PR-E4 (V3-10): recordShares at its worst with the epoch counters, through apply_block at the GAS_DOMAIN maxima
    /// (S_max 256, M_max 256 genesis members): one call naming 256 distinct recorded owners (O = min(S_max + 1, M_max)),
    /// each record's w0 written with lastShare and its counters, as the chain's first call (every record cold, the epoch
    /// totals' slot 10 from zero to nonzero), and the next call rolling every record's counters into a new epoch
    /// (E_epoch 2); and the same with 257 distinct owners without a record (each read as four words). The maximum stays
    /// within admission's envelope for the v3 set (GAS_ENVELOPE_V3 recordShares), which the earn-in and spec-1 entries no longer hold.
    #[test]
    fn v3_record_shares_gas_with_the_epoch_counters(){
        let labels:Vec<String>=(0..256).map(|i|format!("m{i}")).collect();let refs:Vec<&str>=labels.iter().map(|s|s.as_str()).collect();
        let mut cfg=config(2,&refs);cfg.d=16;cfg.w_w=64;cfg.s_max=256;cfg.m_max=256;cfg.e_win=2048;cfg.u=2200;cfg.w_act=128;cfg.theta_inact=1024;cfg.c_react=2200;
        cfg.sys_slot_max=131072;cfg.slot_cap=4_000_000;cfg.acct_cap=1_000_000;cfg.p_max=64;cfg.e_epoch=2;
        let mut out=BTreeMap::new();
        for strangers in [false,true]{
            let mut r=Run::new(&cfg);let ids:Vec<Address>=(0..256).map(|i|r.id(i)).collect();
            let owner=|i:usize|if strangers{Address::from_word(keccak256(format!("share-owner-{i}")).into())}else{ids[i%256]};
            // Template 1 (recorded in block 2's phase 0, the chain's first recordShares) and template 2 (block 3: epoch 1).
            let mut g=Vec::new();
            for h in 1..=3u64{
                let shares:Vec<(u64,Address)>=(0..256u64).map(|i|(i+1,owner(i as usize+usize::from(strangers)))).collect();
                let winner=if strangers{owner(0)}else{ids[(h as usize)%256]};
                r.block(Proposer::Signed(ids[200]),vec![],vec![],winner,shares);
                if h>=2{g.push(r.gas.get("recordShares").copied().unwrap());r.gas.clear();}
            }
            let w=apply::miner_record(&r.state,ids[5]).unwrap();
            if !strangers{assert_eq!(w.last_share,2,"recorded owners: lastShare = j");}
            let t=storage(&r.state,registry_address(),U256::from(10));assert_eq!((bits(t,0,64),bits(t,64,96),bits(t,160,96)),(1,257,257),"epoch totals rolled at template 2");
            out.insert(strangers,g);
        }
        let worst=out.values().flatten().copied().max().unwrap();
        let bound=admission::envelope(ProgramSet::V3,"recordShares").unwrap();
        eprintln!("RECORD-SHARES gas (first call, epoch roll): recorded owners {:?}, owners without a record {:?}; composed v3 {bound}, earn-in {}, spec-1 {}",
            out[&false],out[&true],admission::envelope(ProgramSet::EarnIn,"recordShares").unwrap(),admission::envelope(ProgramSet::V1,"recordShares").unwrap());
        assert!(worst<=bound,"recordShares {worst} above its envelope {bound}");
        // As for markClose: the worst exceeds the earn-in entry it replaces in the composition and the spec-1 entry.
        assert!(worst>admission::envelope(ProgramSet::EarnIn,"recordShares").unwrap(),"recordShares {worst} fits the earn-in envelope: drop its v3 entry");
        assert!(worst>admission::envelope(ProgramSet::V1,"recordShares").unwrap(),"recordShares {worst} fits the spec-1 envelope: drop its v3 entry");
    }

    // ---------------- V3-PR-E2a through the host (V3-19, V3-15, V3-20)
    fn tip_tx(r:&Run,nonce:u64,per_gas:U256)->Vec<u8>{
        let alice=devnet::account_key("alice");
        Tx{chain_id:r.net.profile.spec().chain_id,nonce,max_priority:per_gas,max_fee:r.base+per_gas,gas:21_000,to:Some(devnet::account_key("bob").address()),value:U256::ZERO,data:Bytes::new()}.sign_1559(&alice)
    }
    /// The base burn settle(j) puts into the carry, from the parent state alone (independent of the program): pot j's amount
    /// A and its ledger total C; R = A - floor(A·alpha) - floor(A·gamma), Q = max(C, kappa·m), baseBurn = R - floor(R·C/Q);
    /// zero for an INVALID pot (its whole amount burns).
    fn base_burn(parent:&InMemoryDB,cp:&ConsensusParams,j:u64)->U256{
        let (a,meta,_)=pot(parent,j);if meta>>168u32!=U256::ZERO{return U256::ZERO}
        let r=a-a*U256::from(cp.alpha_bp)/U256::from(10_000)-a*U256::from(cp.gamma_bp)/U256::from(10_000);
        let c=U256::from(apply::share_total(parent,j));let q=c.max(U256::from(u64::from(cp.kappa)*u64::from(cp.m)));
        r-r*c/q
    }
    /// issuance(h) written out again here (V3-19), so the host's issuance.rs is compared with a second implementation.
    fn m3(h:u64,cp:&ConsensusParams)->U256{
        if h==0{return U256::ZERO}
        let k=h/cp.H_half;let s0=devnet::from_fixed(&cp.subsidy);let tail=devnet::from_fixed(&cp.S_tail);
        (if k>=256{U256::ZERO}else{s0>>(k as usize)}).max(tail)
    }

    /// V3-19/V3-15/V3-20 test "host credit = recorded pot, every block for 3·H_half on a compressed fixture": a v3 devnet
    /// with H_half = 8 (S0 2 PCOL, S_tail S0/32) runs 30 blocks (eras 0..3) with tips in signed and fallback blocks. Every
    /// block's issued is issuance(h) (a second implementation here), Escrow gains exactly issued + tips at phase 2, and pot
    /// h = issued + tips - floor(tips·gamma_tip/10^4) (0 on a fallback) + the carry into pot h, which is the base burn of
    /// settle(h - 1 - D) computed from the parent of block h - 1 (V3-15, lagged by one block: block h's own settle(h - D)
    /// leaves exactly its base burn, computed from block h's parent, as the carry into pot h + 1); the invariant holds on
    /// every block. Old-chain replay: the same blocks on the spec-1 earn-in devnet keep
    /// CP 15 every block and pot h = CP 15 + tips.
    #[test]
    fn v3_host_credit_equals_the_recorded_pot_every_block(){
        for spec in [1u8,2]{
            let mut cfg=config(spec,&["a","b","c","d"]);cfg.h_half=8;cfg.s_tail=U256::from(2*PCOL/32);
            let mut r=Run::new(&cfg);let cp=r.net.profile.cp().clone();let ids:Vec<Address>=(0..4).map(|i|r.id(i)).collect();
            let (d,gamma)=(u64::from(cp.D),U256::from(cp.gamma_tip_bp));let mut nonce=0u64;let mut eras=BTreeSet::new();
            let mut pending_carry=U256::ZERO;
            for h in 1..=30u64{
                let proposer=if h%5==0{Proposer::Fallback}else{Proposer::Signed(ids[(h%4) as usize])};
                let txs=if h%3==0{nonce+=1;vec![tip_tx(&r,nonce-1,U256::from(1_000_000_000u64+h))]}else{vec![]};
                let parent=r.state.clone();
                let o=r.block(proposer,vec![],txs,ids[((h+1)%4) as usize],(0..(h%3)).map(|i|(i+1,ids[(i%4) as usize])).collect());
                let share=if matches!(proposer,Proposer::Fallback)||spec==1{U256::ZERO}else{o.tips*gamma/U256::from(10_000)};
                if spec==1{
                    assert_eq!(o.issued,U256::from(2*PCOL),"spec 1 keeps CP 15 at {h}");
                    assert_eq!(pot(&o.state,h).0,o.issued+o.tips,"spec 1: pot {h} = CP 15 + tips");
                    continue
                }
                assert_eq!(o.issued,m3(h,&cp),"issuance({h})");assert_eq!(o.issued,pocol_lp4_execution::issuance::for_cp(&cp,h));eras.insert(o.issued);
                let carry=pending_carry;
                assert_eq!(pot(&o.state,h).0,o.issued+o.tips-share+carry,"pot {h}: issued {} tips {} share {share} carry {carry}",o.issued,o.tips);
                assert!(apply::escrow_carry_into(&o.state,h).is_zero(),"the carry into pot {h} is drained");
                pending_carry=if h>d{base_burn(&parent,&cp,h-d)}else{U256::ZERO};
                assert_eq!(apply::escrow_carry_into(&o.state,h+1),pending_carry,"settle({}) at {h} carries into pot {}",h.saturating_sub(d),h+1);
                assert_eq!(apply::escrow_carry(&o.state),pending_carry,"one carry outstanding between blocks");
                apply::check_escrow_invariant(&o.state,&r.net.profile,h,false).unwrap();
                // Conservation with equality: Escrow holds exactly the pending pots, the held bonds, the burn and the carry.
                let f=apply::chain_facts(&o.state);let pending=(h.saturating_sub(d)+1..=h).map(|j|pot(&o.state,j).0).fold(U256::ZERO,|a,b|a+b);
                assert_eq!(r.bal(escrow_address()),pending+f.held_total+f.burned_total+pending_carry,"Escrow at {h}");
                if h>d+2{assert!(!carry.is_zero(),"C < kappa·m leaves a base burn to carry into {h}");}
            }
            if spec==2{assert_eq!(eras.len(),4,"eras 0..3 of H_half = 8 over 30 blocks");}
        }
    }

    /// V3-15 at a final block (a finite v3 devnet, H_CLOSE 10, H_END 16, RET 2), the carry lagged by one block: block 15's
    /// settle(11) left the carry into pot 16, which no pot receives, and settle(12) in phase 0 of F = 16 adds the carry into
    /// pot 17; finalize burns both with the tips, settleFinal(13..15) burns their base burns directly; after F nothing is
    /// pending or carried, Escrow holds exactly burnedTotal, and every wei issued and every bond went to a reward address or
    /// the burn. Run twice: with every pot but 11 and 12 fully covered (C = kappa·m, R divisible by 64: nothing burns before
    /// F, so finalize burns two nonzero carries as the chain's first burn, its worst path) and with small C everywhere (the
    /// carry recycles base burns into later pots: the members receive more than on the spec-1 earn-in devnet, which burns
    /// them).
    #[test]
    fn v3_final_block_burns_the_carry_on_real_blocks(){
        let mut finalize_gas=0u64;let mut paid=BTreeMap::new();let mut v3_calls:BTreeMap<&str,u64>=BTreeMap::new();
        for (spec,covered) in [(2u8,true),(2,false),(1,false)]{
            let mut cfg=config(spec,&["a","b","c"]);cfg.h_close=10;cfg.h_end=16;cfg.ret=2;
            let mut r=Run::new(&cfg);let cp=r.net.profile.cp().clone();let ids:Vec<Address>=(0..3).map(|i|r.id(i)).collect();
            let bonds=apply::chain_facts(&r.state).held_total;let start:Vec<U256>=(0..3).map(|i|r.bal(r.reward(i))).collect();
            let km=u64::from(cp.kappa)*u64::from(cp.m);let mut finals=Vec::new();let mut tips=U256::ZERO;
            for h in 1..=16u64{
                let c=if covered&&h!=11&&h!=12{km}else{1+h%3};
                let sh:Vec<(u64,Address)>=(1..c).map(|i|(i,ids[(i%3) as usize])).collect();
                let txs=if h==16{vec![tip_tx(&r,0,U256::from(3_000_000_000u64))]}else{vec![]};
                if h==16&&spec==2{
                    // Both carries are outstanding at finalize: the one into pot F and the one F's own settle(12) writes.
                    assert!(!apply::escrow_carry_into(&r.state,16).is_zero(),"block 15 left a carry into pot 16");
                    assert!(!base_burn(&r.state,&cp,12).is_zero(),"settle(12) at F carries");
                }
                let o=r.block(if h%2==0{Proposer::Signed(ids[(h%3) as usize])}else{Proposer::Fallback},vec![],txs,ids[(h%3) as usize],sh);
                finals.push(o.is_final);tips+=o.tips;
                apply::check_escrow_invariant(&o.state,&r.net.profile,h,o.is_final).unwrap();
                if spec==2{for (n,g) in &o.meter.call_gas{let e=v3_calls.entry(*n).or_insert(0);*e=(*e).max(*g);}}
                if spec==2&&covered&&h<16{assert!(apply::chain_facts(&o.state).burned_total.is_zero(),"nothing burns before F at {h}");}
                if o.is_final&&spec==2{
                    let g=o.meter.call_gas.iter().find(|(n,_)|*n=="finalize").unwrap().1;finalize_gas=finalize_gas.max(g);
                    assert!(!o.tips.is_zero(),"F's tips burn");
                }
            }
            assert_eq!(finals.iter().filter(|f|**f).count(),1);assert!(finals[15],"h = 16 is final");
            for j in 1..=16{assert_eq!(pot(&r.state,j),(U256::ZERO,U256::ZERO,U256::ZERO),"pending[{j}]");}
            assert!(apply::escrow_carry(&r.state).is_zero(),"no carry after F (both carry slots)");
            if spec==2{
                // The invariant's final arm: a carry into pot F+1 is the one a non-final block above D keeps (block 15
                // kept the carry into pot 16 and passed), but after F it is refused even when the balance covers it,
                // and it is still counted when it does not. Without the is_final term the covered case would pass at
                // F = 16 > D.
                let x=U256::from(12_345u64);let next=U256::from(4+(17u64&1));
                let mut with=r.state.clone();with.insert_account_storage(escrow_address(),next,x).unwrap();
                assert_eq!(apply::escrow_carry_into(&with,17),x);
                assert_eq!(apply::check_escrow_invariant(&with,&r.net.profile,16,true),Err(apply::ApplyError::Supply("escrow surplus negative")),"an uncovered carry into pot F+1 is counted");
                with.accounts.get_mut(&escrow_address()).unwrap().info.balance+=x;
                assert_eq!(apply::check_escrow_invariant(&with,&r.net.profile,16,true),Err(apply::ApplyError::Supply("carry left after phase 2")),"a covered carry into pot F+1 after F");
                apply::check_escrow_invariant(&r.state,&r.net.profile,16,true).unwrap();
            }
            let f=apply::chain_facts(&r.state);assert_eq!(f.held_total,U256::ZERO,"the closing cursor refunded every bond");
            assert_eq!(r.bal(escrow_address()),f.burned_total,"Escrow holds exactly the burn after F");
            let gains=(0..3).map(|i|r.bal(r.reward(i))-start[i]).fold(U256::ZERO,|a,b|a+b);
            // Every wei issued, every tip and every genesis bond went to a reward address or the burn.
            assert_eq!(gains+f.burned_total,r.issued+bonds+tips,"conservation");
            assert_eq!(r.issued,(1..16u64).map(|h|if spec==2{m3(h,&cp)}else{U256::from(2*PCOL)}).fold(U256::ZERO,|a,b|a+b),"no issuance at F");
            if !covered{paid.insert(spec,(gains,f.burned_total));}
        }
        eprintln!("finalize (two carries, first burn) {finalize_gas}; small-C chains: v3 {:?} earn-in {:?}",paid[&2],paid[&1]);
        assert!(paid[&2].0>paid[&1].0&&paid[&2].1<paid[&1].1,"the carry pays members what the spec-1 set burns");
        assert!(finalize_gas<=admission::envelope(ProgramSet::V3,"finalize").unwrap(),"finalize {finalize_gas} above the v3 envelope");
        assert!(finalize_gas>admission::envelope(ProgramSet::EarnIn,"finalize").unwrap(),"finalize {finalize_gas} fits the earn-in envelope: drop its v3 entry");
        // Every call these v3 chains ran, the final block's settleFinal among them, within its composed v3 entry (a
        // small-chain check of settleFinal on a real v3 final block; its entry's bound by the measured settle maximum is
        // v3_sys_gas_and_caps_within_the_v3_envelope's).
        eprintln!("v3 final-chain call gas {v3_calls:?}");
        for n in ["finalize","settleFinal"]{assert!(v3_calls.contains_key(n),"the v3 final chain ran {n}");}
        for (n,g) in &v3_calls{
            let bound=admission::envelope(ProgramSet::V3,n).unwrap_or_else(||panic!("no envelope entry for {n}"));
            assert!(*g<=bound,"v3 {n} = {g} above its composed envelope {bound}");
        }
    }

    /// closeBlock's worst path on the v3 set (V3-15/V3-20): the carry into pot h is nonzero (block h-1's settle(h-1-D) left
    /// a base burn; the carry is lagged by one block, so the measured blocks start at h = D + 2) and the
    /// tip share goes to an ACCRUING proposer at bond 0 whose share exceeds B_reg: accrue fills the bond, promotes it
    /// (state word, ACC_COUNT, held_total) and the excess is PAID, while held_total is zero (no bond held anywhere: the
    /// genesis members' bonds were burned by evidence and the probationers are at bond 0), so accrue's held_total store
    /// goes from zero to nonzero (EIP-2200: 20,000 instead of 2,900). Measured against the same promotion with
    /// held_total nonzero, a partial fill, a HELD proposer (PAY only), a proposer that reports itself in its own block (a
    /// tomb: the share burns) and a fallback block; every block's keys and accounts within resources::derive.
    #[test]
    fn v3_close_and_finalize_gas_within_the_v3_envelope(){
        let mut cfg=config(2,&["a","b","c","d"]);for q in ["q0","q1"]{cfg.accounts.push((q.into(),U256::from(20*PCOL)));}
        let mut r=Run::new(&cfg);let cp=r.net.profile.cp().clone();let ids:Vec<Address>=(0..4).map(|i|r.id(i)).collect();
        let (q0,q1)=(devnet::account_key("q0").address(),devnet::account_key("q1").address());
        let joins=vec![r.register("q0",0,0),r.register("q1",10*PCOL,0)];
        r.block(Proposer::Signed(ids[0]),vec![],joins,ids[1],vec![(1,ids[2])]);
        for h in 2..=5u64{r.block(Proposer::Signed(ids[h as usize%4]),vec![],vec![],ids[1],vec![(1,ids[2])]);}
        let b=resources::derive(&cp);let mut g=BTreeMap::new();let mut nonce=0u64;let mut carry_settle=0u64;let mut calls:BTreeMap<&str,u64>=BTreeMap::new();
        let big=U256::from(2_000_000_000_000_000u64);// 42 PCOL of tips: a 21 PCOL share
        for (label,proposer,per_gas,evidence) in [("accruing-promoted",q0,big,vec![]),("accruing-partial",q1,U256::from(100_000_000_000_000u64),vec![]),
            ("held",ids[2],big,vec![]),("tomb",ids[3],big,vec![EvidenceEffect::Punish{accused:ids[3]}]),("fallback",Address::ZERO,big,vec![])]{
            let parent=r.state.clone();let txs=vec![tip_tx(&r,nonce,per_gas)];nonce+=1;
            let p=if proposer.is_zero(){Proposer::Fallback}else{Proposer::Signed(proposer)};
            let o=r.block(p,evidence,txs,ids[1],vec![(1,ids[2])]);
            assert!(!apply::escrow_carry_into(&parent,r.h).is_zero(),"{label}: a carry into pot h to zero");
            g.insert(label,o.meter.call_gas.iter().find(|(n,_)|*n=="closeBlock").unwrap().1);
            carry_settle=carry_settle.max(o.meter.call_gas.iter().find(|(n,_)|*n=="settle").unwrap().1);
            for (n,gas) in &o.meter.call_gas{let e=calls.entry(*n).or_insert(0);*e=(*e).max(*gas);}
            assert!(o.meter.keys.len() as u64<=b.keys_normal&&o.meter.accounts.len() as u64<=b.accts_normal,"{label}: keys {} accounts {} vs {b:?}",o.meter.keys.len(),o.meter.accounts.len());
            let rec=apply::miner_record(&o.state,proposer);
            match label{
                "accruing-promoted"=>assert_eq!(rec.map(|x|(x.bond_state,x.bond)),Some((1,b_reg())),"the tip share filled q0's bond and promoted it"),
                "accruing-partial"=>assert_eq!(rec.map(|x|(x.bond_state,x.bond)),Some((4,U256::from(10*PCOL)+o.tips/U256::from(2))),"q1's bond took its whole share"),
                "tomb"=>assert!(rec.unwrap().tomb),
                _=>{}
            }
        }
        let (held_zero_close,held_zero_settle)=(held_zero_gas(false),held_zero_gas(true));
        g.insert("accruing-promoted-held-zero",held_zero_close);
        eprintln!("closeBlock gas {g:?}; settle with a carry write {carry_settle}; settle accruing from held_total 0 {held_zero_settle}");
        assert!(carry_settle<=admission::envelope(ProgramSet::V3,"settle").unwrap());
        assert!(held_zero_settle<=admission::envelope(ProgramSet::V3,"settle").unwrap());
        let worst=*g.values().max().unwrap();
        assert_eq!(g["accruing-promoted-held-zero"],worst,"the promotion from held_total 0 is closeBlock's worst path");
        assert!(worst>g["accruing-promoted"],"held_total's zero-to-nonzero store costs more");
        assert!(worst<=admission::envelope(ProgramSet::V3,"closeBlock").unwrap(),"closeBlock {worst} above the v3 envelope");
        // The v3 entry is needed: closeBlock's worst path exceeds the earn-in entry it replaces in the composition and
        // the spec-1 entry of the same name.
        assert!(worst>admission::envelope(ProgramSet::EarnIn,"closeBlock").unwrap(),"closeBlock {worst} fits the earn-in envelope: drop its v3 entry");
        assert!(worst>admission::envelope(ProgramSet::V1,"closeBlock").unwrap(),"closeBlock {worst} fits the spec-1 envelope: drop its v3 entry");
        // Every other call the measured blocks ran, within its composed v3 entry.
        eprintln!("measured blocks' call gas {calls:?}");
        for (n,gas) in &calls{
            let bound=admission::envelope(ProgramSet::V3,n).unwrap_or_else(||panic!("no envelope entry for {n}"));
            assert!(*gas<=bound,"v3 {n} = {gas} above its composed envelope {bound}");
        }
    }
    fn b_reg()->U256{U256::from(20*PCOL)}
    /// A v3 chain on which held_total reaches zero: genesis members a..d at B_reg, probationers q0 and q1 registered at
    /// bond 0 in block 1, and in blocks 2..5 each genesis member, as the signed proposer, reports itself (type-1 evidence:
    /// its bond burns, its record is a tomb), so every pot's winner, proposer and shares are tombs and their payments
    /// burn. Fallback blocks follow up to h = D + 2. `settle_payee`: pot 2's winner is q1 instead, so the measured block's
    /// settle(2) accrues into q1's bond from held_total 0 (returns settle's gas); otherwise the measured block, signed by
    /// q0 with 42 PCOL of tips, has a carry to zero and a 21 PCOL tip share that fills q0's bond from held_total 0,
    /// promotes it and pays the excess (returns closeBlock's gas).
    fn held_zero_gas(settle_payee:bool)->u64{
        let mut cfg=config(2,&["a","b","c","d"]);for q in ["q0","q1"]{cfg.accounts.push((q.into(),U256::from(20*PCOL)));}
        let mut r=Run::new(&cfg);let cp=r.net.profile.cp().clone();let ids:Vec<Address>=(0..4).map(|i|r.id(i)).collect();
        let (q0,q1)=(devnet::account_key("q0").address(),devnet::account_key("q1").address());
        let joins=vec![r.register("q0",0,0),r.register("q1",0,0)];
        r.block(Proposer::Signed(ids[0]),vec![],joins,ids[1],vec![(1,ids[2])]);
        for k in 0..4usize{
            let winner=if settle_payee&&k==0{q1}else{ids[1]};
            r.block(Proposer::Signed(ids[k]),vec![EvidenceEffect::Punish{accused:ids[k]}],vec![],winner,vec![(1,ids[2])]);
        }
        while r.h<u64::from(cp.D)+1{r.block(Proposer::Fallback,vec![],vec![],ids[1],vec![(1,ids[2])]);}
        let f=apply::chain_facts(&r.state);
        assert_eq!(f.held_total,U256::ZERO,"no bond is held before the measured block");
        for (q,i) in [(q0,0),(q1,1)]{assert_eq!(apply::miner_record(&r.state,q).map(|x|(x.bond_state,x.bond)),Some((4,U256::ZERO)),"q{i} ACCRUING at bond 0");}
        for i in 0..4{assert!(apply::miner_record(&r.state,ids[i]).unwrap().tomb);}
        let parent=r.state.clone();let j=r.h+1-u64::from(cp.D);
        assert!(!apply::escrow_carry_into(&parent,r.h+1).is_zero(),"a carry into the measured block's pot to zero");
        if settle_payee{assert_eq!(pot(&parent,j).2&((U256::from(1)<<160u32)-U256::from(1)),U256::from_be_slice(q1.as_slice()),"pot {j} won by q1");}
        let o=r.block(Proposer::Signed(q0),vec![],vec![tip_tx(&r,0,U256::from(2_000_000_000_000_000u64))],ids[1],vec![(1,ids[2])]);
        let b=resources::derive(&cp);
        assert!(o.meter.keys.len() as u64<=b.keys_normal&&o.meter.accounts.len() as u64<=b.accts_normal,"keys {} accounts {} vs {b:?}",o.meter.keys.len(),o.meter.accounts.len());
        let gas=|n:&str|o.meter.call_gas.iter().find(|(x,_)|*x==n).unwrap().1;
        if settle_payee{
            assert_eq!(apply::miner_record(&o.state,q1).unwrap().bond_state,4);assert!(!apply::miner_record(&o.state,q1).unwrap().bond.is_zero(),"settle(2) filled q1's bond in part");
            gas("settle")
        }else{
            assert_eq!(apply::miner_record(&o.state,q0).map(|x|(x.bond_state,x.bond)),Some((1,b_reg())),"the tip share filled q0's bond and promoted it");
            assert_eq!(apply::chain_facts(&o.state).held_total,b_reg(),"held_total went from zero to B_reg in closeBlock");
            gas("closeBlock")
        }
    }

    /// V3-15 invariant (apply.rs check_escrow_invariant), the carry lagged by one block: on the v3 set need = Σpending +
    /// held + burned + both carries, after phase 2 of block h the carry into pot h is zero, and the carry into pot h+1 may
    /// remain only once a settlement ran (h > D). On a real chain (h = 6 > D) the carry into pot 7 is outstanding and
    /// counted: raising it by x without the balance is "escrow surplus negative" (an invariant without that term would
    /// pass it: this test fails on it), with the balance it passes; a carry into pot h is "carry left after phase 2" when
    /// covered and "escrow surplus negative" when not; at h = 3 <= D a carry into pot 4 is "carry left after phase 2".
    /// On the spec-1 earn-in set slots 4 and 5 mean nothing and every verdict is unchanged.
    #[test]
    fn v3_escrow_invariant_counts_the_carry(){
        for spec in [2u8,1]{
            let mut r=Run::new(&config(spec,&["a","b","c"]));let ids:Vec<Address>=(0..3).map(|i|r.id(i)).collect();
            let mut early=None;
            for h in 1..=6u64{r.block(Proposer::Signed(ids[(h%3) as usize]),vec![],vec![],ids[0],vec![(1,ids[1])]);if h==3{early=Some(r.state.clone());}}
            let h=r.h;let x=U256::from(12_345u64);let p=&r.net.profile;
            apply::check_escrow_invariant(&r.state,p,h,false).unwrap();
            if spec==2{assert!(!apply::escrow_carry_into(&r.state,h+1).is_zero(),"the carry into pot h+1 outlives block h");}
            let slot=|t:u64|U256::from(4+(t&1));
            let with=|db:&InMemoryDB,t:u64,cover:bool|{let mut d=db.clone();let v=storage(&d,escrow_address(),slot(t))+x;d.insert_account_storage(escrow_address(),slot(t),v).unwrap();
                if cover{d.accounts.get_mut(&escrow_address()).unwrap().info.balance+=x;}d};
            let verdict=|db:&InMemoryDB,at:u64|apply::check_escrow_invariant(db,p,at,false);
            let got=[verdict(&with(&r.state,h+1,false),h),verdict(&with(&r.state,h+1,true),h),verdict(&with(&r.state,h,false),h),verdict(&with(&r.state,h,true),h)];
            let e=early.unwrap();let at_d=[verdict(&e,3),verdict(&with(&e,4,true),3)];
            if spec==2{
                let (neg,left)=(Err(apply::ApplyError::Supply("escrow surplus negative")),Err(apply::ApplyError::Supply("carry left after phase 2")));
                assert_eq!(got,[neg.clone(),Ok(()),neg,left.clone()],"carry into pot h+1: counted and allowed; into pot h: counted and refused");
                assert_eq!(at_d,[Ok(()),left],"no carry into pot h+1 before a settlement ran");
            }else{assert!(got.iter().chain(at_d.iter()).all(|v|v.is_ok()),"spec 1: slots 4 and 5 are not part of the invariant: {got:?} {at_d:?}");}
        }
    }

    /// V3-19 supply bound (genesis_builder.rs): a genesis running the v3 set needs allocations + bonds + 2·S0·H_half +
    /// S_tail·2^64 < 2^127 (the M3 schedule summed over every height), checked when the genesis is built and on every
    /// candidate. At the largest S_tail that keeps the total below 2^127 the devnet builds and runs; one more wei of tail
    /// refuses it (Supply), as does the same total reached through H_half. The flat CP 15 · u64::MAX term of the spec-1
    /// continuous rule (2 PCOL · (2^64 - 1) < 2^126) would have admitted both.
    #[test]
    fn v3_genesis_supply_bound_refuses_at_2_pow_127(){
        let base=config(2,&["a","b","c"]);
        let probe=devnet::build(&base).unwrap();let cp=probe.profile.cp().clone();
        let s0=devnet::from_fixed(&cp.subsidy);
        let alloc:U256=probe.alloc.accounts.values().map(|a|a.info.balance).fold(U256::ZERO,|a,b|a+b);
        let bonds=devnet::from_fixed(&cp.B_reg)*U256::from(3);
        let limit=U256::from(1)<<127;let two64=U256::from(1)<<64;
        let rest=alloc+bonds+U256::from(2)*s0*U256::from(cp.H_half);
        let tail=(limit-U256::from(1)-rest)/two64;
        assert!(s0*U256::from(u64::MAX)+alloc+bonds<limit,"the flat term alone would admit the refused geneses below");
        let mut ok=base.clone();ok.s_tail=tail;
        let mut r=Run::new(&ok);for _ in 0..3{let w=r.id(0);r.block(Proposer::Fallback,vec![],vec![],w,vec![]);}
        assert_eq!(r.issued,tail*U256::from(3),"the tail floors issuance at the boundary");
        let mut bad=base.clone();bad.s_tail=tail+U256::from(1);
        let e=devnet::build(&bad).err().expect("one more wei of tail reaches 2^127");assert!(e.contains("Supply"),"{e}");
        // The halving term: with the same tail, H_half + k crosses 2^127 exactly when 2·S0·k exceeds the slack left.
        let slack:U256=limit-U256::from(1u64)-rest-tail*two64;let k:u64=(slack/(U256::from(2u64)*s0)).to::<u64>();
        let mut near=base.clone();near.s_tail=tail;near.h_half=cp.H_half+k;devnet::build(&near).unwrap();
        let mut far=base.clone();far.s_tail=tail;far.h_half=cp.H_half+k+1;
        let e=devnet::build(&far).err().expect("2·S0 more of halving term");assert!(e.contains("Supply"),"{e}");
    }
}
