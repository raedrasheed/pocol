//! V3 system programs (system-v3.json, PoColSystemV3.sol; V3-PR-E0, V3-34 registry; V3-PR-E1, V3-08/V3-09 registry budgets;
//! V3-PR-E2b, V3-16/V3-17 evidence penalties; V3-PR-E3 late shares; V3-PR-E2a issuance, carry, tips; FN-2b removal at a full
//! set, registerReplacing and activeSlice).
//! The v3 source is PoColSystemEarnIn.sol without the probation pool and its deadline (FN-2b, owner decisions 10b/10c of
//! 2026-11-13: ACCRUING records count within M_max and never exit by time), ChainCaps bounded by the 59-field CP schema
//! v2, and the registry budgets of V3-08/V3-09 (a maintenance budget REG_MAINT_MAX = 8 with one maintenance operation per
//! identity per block; reactivate on its own R_react budget, CP 56, with a per-record C_react clock, CP 57, and a window
//! that closes strictly before dueHeight). This file proves:
//! - the earn-in scenarios (tests/data/earnin-vectors.json) as the v3 model runs them replay on the v3 set
//!   (tests/v3/earnin-on-v3-vectors.json); outside "top-up" (V3-08), "probation-seats" (no pool) and "deadline" (no
//!   deadline) they give the earn-in model's own reverts, records, totals and payments and leave the identical Escrow
//!   storage and the identical Registry storage except for the V3-08/V3-09 state alone;
//! - the registry budgets and FN-2b's removal at a full set: every scenario of the independent model tests/v3_model.py
//!   (tests/v3/registry-budget-vectors.json, registry and chain scenarios: removeInactive and registerReplacing only at a
//!   full set, two REG_OPS units and one R_max entry, victims refunded after U and punishable until then, lastShare from
//!   header and late shares) reproduces on the v3 bytecode step by step, with held_total conserved and the accruing count
//!   (slot 8) never written; the fails-on-original attacks of V3-DESIGN.md V3-08/V3-09 freeze the earn-in registry and not
//!   the v3 one; seven planted budget mutants and eight planted removal mutants each fail;
//! - the four planted earn-in mutants that outlive FN-2b (EARN-IN-SPEC.md §9, restated on the v3 source:
//!   tests/v3/system-v3-mutants.json, compile_system_v3.cjs --mutants) each fail the earn-in scenarios as the v3 model runs them;
//! - every named and literal CP index of PoColSystemV3.sol is the schema's (a missed renumbering would make every v3 block
//!   invalid), and every program stays EIP-170-sized;
//! - the write ceilings and gas the wallet's quotes are sized from, and the beginBlock sweep within admission's GAS_ENVELOPE;
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
//! - FN-2b C4/C5: registerReplacing's call shape, write ceilings, gas and user SYS keys, the removalStatus and activeSlice
//!   views; with v3-preview, settle's worst path with every payee ACCRUING at bond 0 and promoted in one call (256 share
//!   owners, the winner and the proposer; and the carry path), M_max above the spec-1 domain, and the earn-in refund
//!   regressions (earnin_refund_fix.rs) on the v3 set.
use pocol_lp4_execution::{native,output,CHAIN_ID,apply::{ProgramSet,Probation,SystemPrograms},devnet::DevnetConfig,profile::ConsensusParams,genesis_spec,
    pay::{registry_address,escrow_address}};
use revm::{DatabaseCommit,InMemoryDB,primitives::{Address,AccountInfo,Bytecode,Bytes,TxEnv,TxKind,U256,keccak256,ResultAndState}};
use std::collections::{BTreeMap,BTreeSet};

const PCOL:u128=1_000_000_000_000_000_000;
const EARNIN:&str=include_str!("data/system-earnin.json");
const V3:&str=include_str!("data/system-v3.json");
const MUTANTS:&str=include_str!("v3/system-v3-mutants.json");
const VECTORS:&str=include_str!("data/earnin-vectors.json");
const BUDGET_VECTORS:&str=include_str!("v3/registry-budget-vectors.json");
const EVIDENCE_VECTORS:&str=include_str!("v3/evidence-penalty-vectors.json");
const LATE_VECTORS:&str=include_str!("v3/late-share-vectors.json");
const ISSUANCE_VECTORS:&str=include_str!("v3/issuance-carry-vectors.json");
const V3_SOURCE:&str=include_str!("../contracts/system/PoColSystemV3.sol");
/// The earn-in model's scenarios (both value sets) re-run through the v3 model (tests/v3_model.py earnin_on_v3).
const EARNIN_ON_V3:&str=include_str!("v3/earnin-on-v3-vectors.json");
/// The four planted earn-in mutants that outlive FN-2b (EARN-IN-SPEC.md §9; its P_MAX and T_PROB mutants went with the
/// pool and the deadline) and the seven V3-08/V3-09 budget mutants, in fixture order.
const EARNIN_MUTANTS:[&str;4]=["r9-removed","no-promotion-at-b","over-fill-bad-value","excess-kept"];
const BUDGET_MUTANTS:[&str;7]=["maint-counter-shared","maint-twice-per-block","react-budget-ignored","react-cooldown-off-by-one","react-window-at-sweep-height",
    "maint-bounded-by-reg-ops","react-counts-reg-ops"];
/// The five V3-16/V3-17 evidence-penalty mutants (MinerRegistry or RewardEscrow), in fixture order.
const EVIDENCE_MUTANTS:[&str;5]=["reporter-paid-on-fallback","whole-pot-for-bonded-accused","accruing-read-at-evidence","accruing-read-at-settle","rho-on-pot-not-bond"];
/// The five V3-02 late-share mutants (MinerRegistry or ShareLedger), in fixture order.
const LATE_MUTANTS:[&str;5]=["late-last-share-reverts","late-last-share-overwrites","late-cap-removed","late-window-one-wider","late-total-not-raised"];
/// The eight V3-19/V3-15/V3-20 issuance, carry and tip split mutants (RewardEscrow), in fixture order.
const ECON_MUTANTS:[&str;8]=["carry-not-added","tip-share-to-pot","base-burn-still-burned","issued-ignored-flat-s0","carry-kept-at-final","base-burn-into-closed-pot",
    "carry-not-lagged","lagged-carry-kept-at-final"];
/// The nine FN-2b C4 removal mutants (MinerRegistry), in fixture order.
const REPLACE_MUTANTS:[&str;9]=["removal-below-full-set","removal-theta-off-by-one","removal-ignores-grace","removal-at-m-min",
    "replacing-one-reg-ops-unit","replacing-no-r-max-entry","replacing-victim-erased-at-once","replacing-any-victim","replacing-sender-with-record"];
/// The v3 set's sysCodeHash = keccak256(RLP([code(C0C001..C0C005), 0x80])) of system-v3.json.
const V3_SYS_CODE_HASH:&str="a1d91673b4b4a2c88ff75d4828dc1932dbaa2831284a66f4b1ea77b4c5e666a2";

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
/// set, the 7 schema-v2 slots, and per-run overrides.
/// `v3`: the host conventions of the v3 program set (V3-16/V3-17: phase 0 records the parent's result before the
/// evidence, invalidate(j, accused), punish(accused, reporter)); otherwise the spec-1 host's.
#[derive(Clone)]
struct Setup{registry:Bytes,escrow:Bytes,ledger:Bytes,caps:Bytes,cp_len:u64,cp:Vec<(u64,u128)>,v3:bool}
impl Setup{
    fn earnin()->Self{Self{registry:runtime(EARNIN,"MinerRegistry"),escrow:runtime(EARNIN,"RewardEscrow"),ledger:runtime(EARNIN,"ShareLedger"),caps:runtime(EARNIN,"ChainCaps"),cp_len:52,cp:vec![],v3:false}}
    /// R_react 16 (as the harness's R_max) and C_react 40 (= U, the least ParamGate admits) in CP 56/57 (tests/v3_model.py
    /// DEFAULT_CP; scenarios override them).
    fn v3()->Self{
        Self{registry:runtime(V3,"MinerRegistry"),escrow:runtime(V3,"RewardEscrow"),ledger:runtime(V3,"ShareLedger"),caps:runtime(V3,"ChainCaps"),cp_len:59,
            cp:vec![(56,16),(57,40)],v3:true}
    }
    /// A planted mutant in place of the program it names.
    fn with_program(mut self,program:&str,code:Bytes)->Self{match program{"MinerRegistry"=>self.registry=code,"RewardEscrow"=>self.escrow=code,"ShareLedger"=>self.ledger=code,o=>panic!("no mutant of {o}")}self}
    /// M_max, R_max and REG_OPS wide enough for many registrations in one block.
    fn wide(mut self)->Self{self.cp.extend([(17,255),(18,256),(23,1024)]);self}
    fn with_registry(mut self,code:Bytes)->Self{self.registry=code;self}
}

/// `origin`: the current block's proposer (zero on a fallback block), the tx.origin of its SYSTEM calls; `issued`: the
/// issuance the chain steps credited to Escrow; `closed`: per closed height (issued, tips, the carry into pot h before
/// closeBlock, the pot closeBlock recorded) (V3-PR-E2a); `distributed`: per settled height, pot j's amount just before
/// settle(j) or settleFinal(j) distributed it; `final_carry`: at a final block F, the carries into pots F and F+1 just
/// before finalize (V3-15, lagged: block F-1's and F's own settle's).
/// `view`: the result of the last removal_status step, the removal code (FN-2b C4).
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
            // FN-2b C4: registerReplacing(reward, victim) with the bond as value.
            "register_replacing"=>{let s=self.sender(&op[1]);self.at(num(5) as u64)?;let mut d=with_arg("registerReplacing(address,address)",Some(name(&op[2])));d.extend(name(&op[3]).into_word());
                call(&mut self.db,s,d,U256::from(num(4)),self.height)}
            // FN-2b C4: the removalStatus(victim) view at the current block, kept for the caller to compare.
            "removal_status"=>{self.at(num(2) as u64)?;
                self.view=Some(self.try_removal_status(name(&op[1])).map_or_else(|e|e,|c|c.to_string()));return Ok(None)}
            "sweep"=>{self.at(num(1) as u64)?;return Ok(None)}
            // A registry scenario's punish runs outside any block's evidence: no reporter (the whole bond burns, as the
            // registry model's punish), so on v3 with tx.origin zero.
            "punish"=>{let id=name(&op[1]);self.at(num(2) as u64)?;self.origin=Address::ZERO;self.punish_call(id).map_err(|e|format!("punish: {e}"))?;return Ok(None)}
            "settle_to"=>{self.settle(name(&op[1]),U256::from(num(2)))?;return Ok(None)}
            o=>panic!("unknown op {o}"),
        };
        if res.result.is_success(){return Ok(None)}
        let out=output(&res.result);
        let known=["BadValue","BadState","Capacity","RewardNotDurable","PaymentFailed","BadCalldata","RegOpsLimit","NotRemovable","Closing","BadPhase"];
        Ok(Some(known.iter().find(|k|selector(&format!("{k}()"))==out).map(|k|k.to_string()).unwrap_or(format!("{out:?}"))))
    }
    /// FN-2b C4: removalStatus(victim) at the current height: the removal code.
    fn try_removal_status(&mut self,victim:Address)->Result<u64,String>{
        let r=call(&mut self.db,name("viewer"),with_arg("removalStatus(address)",Some(victim)),U256::ZERO,self.height);
        if !r.result.is_success(){return Err(format!("removalStatus reverted: {:?}",output(&r.result)))}
        Ok(U256::from_be_slice(output(&r.result)).to::<u64>())
    }
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
    // The earn-in model's accruing count (slot 8); FN-2b: the v3 set writes none.
    ensure_eq!(reg(&h.db,8).to::<u64>(),if s.v3{0}else{fin["accruing"].as_u64().unwrap()},"{name_} accruing");
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

/// The earn-in scenarios whose results differ on v3 (tests/v3_model.py EARNIN_ON_V3_DIFFER): "top-up" (V3-08: a second
/// topUp from one identity in one block), "probation-seats" (FN-2b: no probation pool, so the ninth probationer registers)
/// and "deadline" (FN-2b C3: no probation deadline, so nothing exits at registeredAt + T_PROB; its steps agree and its final
/// records differ).
const EARNIN_DIFFER_ON_V3:[&str;3]=["deadline","probation-seats","top-up"];
#[test]
fn v3_set_is_behaviour_equal_to_the_earnin_set_outside_v3_08_and_fn_2b(){
    let earnin=replay(VECTORS,&Setup::earnin()).unwrap();
    let v3_setup=Setup::v3();
    // The v3 set reproduces every earn-in scenario as the v3 model runs it; the step results differ only in "top-up"
    // (RegOpsLimit, V3-08) and "probation-seats" (the ninth probation registration lands: no pool).
    assert_eq!(earnin_on_v3("earnin",&v3_setup),Ok(8),"every model scenario replayed");
    assert_eq!(v3_model_differs(VECTORS,"earnin"),BTreeSet::from(["probation-seats".to_string(),"top-up".to_string()]));
    // Every other scenario but "deadline": the earn-in model's own results, and the same storage and balances as on the
    // earn-in set. "deadline" (and "probation-seats") fail the earn-in model's own vectors on v3: no deadline exit, no pool.
    let v:serde_json::Value=serde_json::from_str(VECTORS).unwrap();
    let mut v3=BTreeMap::new();let mut differ=BTreeSet::new();
    for (n,body) in v.as_object().unwrap(){
        if body.get("steps").is_none(){continue}
        match scenario(n,body,&v3_setup){Ok(o)=>{v3.insert(n.clone(),o);}Err(e)=>{eprintln!("{n} differs on v3: {e}");differ.insert(n.as_str());}}
    }
    assert_eq!(differ,BTreeSet::from(EARNIN_DIFFER_ON_V3),"the earn-in scenarios v3 runs differently");
    assert_eq!(v3.len(),5);
    let mut e1_keys=0;
    for (name_,got) in &v3{
        let want=&earnin[name_];
        assert_eq!(got.steps,want.steps,"{name_}: every step's result");
        // V3-08/V3-09 state, and only it, differs: the budget slot 9, the clocks in w1 bits 160..255 of every identity and
        // slot 3's per-block counters (bits 0..23, which every beginBlock clears). The earn-in set has none of the first two.
        let (g,extra)=without_budget_state(&got.registry,&got.ids);let (w,none)=without_budget_state(&want.registry,&want.ids);
        assert!(none.is_empty(),"{name_}: the earn-in registry holds no V3-08/V3-09 state: {none:?}");
        e1_keys+=extra.len();
        // The v3 set keeps no accruing count: the earn-in set's slot 8 is the only other difference.
        let (mut g,mut w)=(g,w);let acc=w.remove(&U256::from(8));g.remove(&U256::from(8));
        assert!(!got.registry.contains_key(&U256::from(8)),"{name_}: the v3 registry never writes slot 8");
        if let Some(a)=acc{assert!(!a.is_zero());}
        assert_eq!(g,w,"{name_}: identical MinerRegistry storage outside the V3-08/V3-09 state and the accruing count");
        assert_eq!(got.escrow,want.escrow,"{name_}: identical RewardEscrow storage");
        assert_eq!(got.balances,want.balances,"{name_}: identical balances");
    }
    // The model scenarios reactivate, so the v3 set did keep budget state (the projection is not vacuous).
    assert!(e1_keys>0,"no V3-08/V3-09 state was written");
}

/// FN-2b C3 (owner 10c): no probation deadline on the v3 set. A zero-bond record registered at 10 tops up, exits and
/// reactivates far past the earn-in deadline (60,480 blocks) and is never swept out; the earn-in set exits it at
/// registeredAt + 60,480 and refuses the top-up there. ACCRUING newcomers have no pool of their own: at M_max 4 three
/// zero-bond registrations fill the set and a fourth (or a full bond) is refused only by M_max (Capacity).
#[test]
fn v3_set_has_no_probation_pool_and_no_deadline(){
    for (v3,s) in [(false,Setup::earnin()),(true,Setup::v3())]{
        let mut h=Harness::new(&["rd".to_string()],&s);let a=name("d");
        assert_eq!(h.step(&strs(&["register","d","rd",&PCOL.to_string(),"10"])).unwrap(),None);
        assert_eq!(h.step(&strs(&["top_up","d","1","60489"])).unwrap(),None,"the last earn-in probation block");
        let at=h.step(&strs(&["top_up","d","1","60490"])).unwrap();
        if v3{
            assert_eq!(at,None,"no deadline: a top-up at registeredAt + 60,480");
            assert_eq!((bits(h.record(a).0,0,8),bits(h.record(a).0,16,8)),(1,4),"still an active ACCRUING record");
            assert_eq!(h.step(&strs(&["top_up","d","1","1000000"])).unwrap(),None);
            assert_eq!(h.step(&strs(&["deregister","d","1000001"])).unwrap(),None);
            assert_eq!(h.step(&strs(&["reactivate","d","1000033"])).unwrap(),None,"an ACCRUING record reactivates without a deadline");
            assert_eq!(bits(h.record(a).0,0,8),1);
        }else{
            assert_eq!(at.as_deref(),Some("BadState"),"the earn-in deadline");
            assert_eq!(bits(h.record(a).0,0,8),2,"the earn-in sweep exited it at the deadline");
        }
    }
    let mut s=Setup::v3();s.cp.push((18,4));
    let mut h=Harness::new(&[],&s);
    for x in ["p1","p2","p3"]{assert_eq!(h.step(&strs(&["register",x,x,"0","1"])).unwrap(),None,"{x}: zero bond, ACCRUING within M_max");}
    assert_eq!(h.step(&strs(&["register","p4","p4","0","1"])).unwrap().as_deref(),Some("Capacity"),"M_max, not a pool");
    assert_eq!(h.step(&strs(&["register","p5","p5",&b().to_string(),"1"])).unwrap().as_deref(),Some("Capacity"));
    assert_eq!(reg(&h.db,8),U256::ZERO,"no accruing count");
    let r=call(&mut h.db,name("viewer"),selector("accruingCount()"),U256::ZERO,1);
    assert!(!r.result.is_success(),"the accruingCount view is gone");
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
fn chain_caps_serves_exactly_the_v2_cp(){
    let read=|s:&Setup,n:u64|{let mut h=Harness::new(&[],s);let mut d=selector("get(uint256)");d.extend(U256::from(n).to_be_bytes::<32>());
        let r=call_to(&mut h.db,name("reader"),caps(),d,U256::ZERO,1);r.result.is_success().then(||U256::from_be_slice(output(&r.result)))};
    let v3=Setup::v3();
    assert_eq!(read(&v3,56),Some(U256::from(16)));assert_eq!(read(&v3,57),Some(U256::from(40)));
    assert_eq!(read(&v3,58),Some(U256::ZERO));assert_eq!(read(&v3,59),None,"slot 59 is outside CP schema v2 (FN-2a, FN-2b deletions)");
    assert_eq!(read(&Setup::earnin(),51),Some(U256::ZERO));assert_eq!(read(&Setup::earnin(),52),None,"the earn-in ChainCaps keeps its 52-field bound");
    assert_eq!(genesis_spec::CP_SCHEMA_V2.len(),59);
    for gone in ["P_MAX","T_PROB","E_epoch","theta_bp","k_sigma","F0_acct","F0_slot","F0_code"]{assert_eq!(genesis_spec::cp_index_for(2,gone),None,"{gone}");}
    // The harness's (and tests/v3_model.py's) named indices are the schema's own: a renumbering missed here would
    // install every scenario's field in the wrong ChainCaps slot.
    for f in ["D","R_max","M_max","M_min","SWEEP_MAX","SWEEP_SCAN","REG_OPS","W_act","Theta_inact","E_win","U","R_react","C_react",
        "m","kappa","S_max","W_w","alpha_bp","gamma_bp","H_half","S_tail","gamma_tip_bp","rho_bp"]{
        assert_eq!(Some(cp_index(f) as usize),pocol_lp4_execution::genesis_spec::cp_index_for(2,f),"{f}");}
    assert_eq!(Some(cp_index("S0") as usize),pocol_lp4_execution::genesis_spec::cp_index_for(2,"subsidy"));
}

/// FN-2b (the CP renumbering): every CP index PoColSystemV3.sol names is the schema's. A missed index would make
/// closeBlock, burnBond or a registry path read the wrong field (or revert in ChainCaps), and so make every v3 block
/// invalid. Checked on the source text the committed bytecode is compiled from (compile_system_v3.cjs, byte-for-byte in
/// CI): ChainCaps' bound is the schema length; each named constant CP_<FIELD>=n is that field's index; every literal
/// cfg(n) above the v1 prefix is one of the two v3 fields the programs read by number, at its use; and every literal
/// cfg(n) below it is a v1 field the programs read (the v1 prefix is unchanged by CP schema v2).
#[test]
fn every_cp_index_of_the_v3_source_matches_the_schema(){
    let idx=|f:&str|genesis_spec::cp_index_for(2,f).unwrap_or_else(||panic!("{f} not in CP schema v2"));
    // ChainCaps: require(n<59), the CP schema v2 length.
    assert!(V3_SOURCE.contains(&format!("require(n<{});",genesis_spec::CP_V2_LEN)),"ChainCaps' bound is not CP_V2_LEN {}",genesis_spec::CP_V2_LEN);
    assert_eq!(V3_SOURCE.matches("require(n<").count(),1);
    // Named constants.
    let named=[("CP_R_REACT","R_react"),("CP_C_REACT","C_react")];
    let mut seen=BTreeSet::new();
    for line in V3_SOURCE.lines(){
        let t=line.trim();
        if let Some(rest)=t.strip_prefix("uint256 constant CP_"){
            let (n,v)=rest.split_once('=').unwrap();let v:usize=v.trim_end_matches(';').parse().unwrap();
            let (_,field)=named.iter().find(|(c,_)|c[3..]==*n).unwrap_or_else(||panic!("an unchecked named CP index CP_{n}"));
            assert_eq!(v,idx(field),"CP_{n}");seen.insert(n.to_string());
        }
    }
    assert_eq!(seen.len(),named.len(),"every named CP index found: {seen:?}");
    // Literal indices: each cfg(n) / Sys.cfg(n).
    let mut literals=BTreeMap::<usize,usize>::new();
    for (i,_) in V3_SOURCE.match_indices("cfg("){
        let rest=&V3_SOURCE[i+4..];let end=rest.find(')').unwrap();
        if let Ok(n)=rest[..end].parse::<usize>(){*literals.entry(n).or_default()+=1;}
    }
    let v2:BTreeMap<usize,usize>=literals.iter().filter(|(n,_)|**n>=genesis_spec::CP_LEN).map(|(n,c)|(*n,*c)).collect();
    assert_eq!(v2,BTreeMap::from([(idx("gamma_tip_bp"),1),(idx("rho_bp"),1)]),"the literal CP v2 indices");
    assert!(V3_SOURCE.contains(&format!("tips*Sys.cfg({})/10000",idx("gamma_tip_bp"))),"closeBlock's tip share reads gamma_tip_bp");
    assert!(V3_SOURCE.contains(&format!("amount*Sys.cfg({})/10000",idx("rho_bp"))),"burnBond's reporter cut reads rho_bp");
    let v1=["m","kappa","S_max","D","W_w","alpha_bp","gamma_bp","B_reg","R_max","M_max","M_min","REG_RECORDS_MAX","SWEEP_MAX","SWEEP_SCAN",
        "REG_OPS","W_act","Theta_inact","E_win","U","H_CLOSE","T_CLOSE","RET"].map(|f|genesis_spec::cp_index_for(1,f).unwrap_or_else(||panic!("{f}")));
    for n in literals.keys().filter(|n|**n<genesis_spec::CP_LEN){assert!(v1.contains(n),"cfg({n}) is not a v1 field the programs read");}
}

/// EIP-170: every v3 runtime, MinerRegistry in particular (the largest), stays within 24,576 bytes (tooling and the
/// CREATE rule other EVM clients apply), with room to spare after FN-2b.
#[test]
fn the_v3_programs_fit_eip_170(){
    let j:serde_json::Value=serde_json::from_str(V3).unwrap();
    for p in j["programs"].as_array().unwrap(){
        let n=(p["runtime"].as_str().unwrap().len()-2)/2;eprintln!("EIP-170 {} {n} bytes",p["name"]);
        assert!(n<=24_576,"{} is {n} bytes",p["name"]);
    }
    assert!(runtime(V3,"MinerRegistry").len()<=24_576);
}

#[test]
fn the_planted_earnin_mutants_fail_on_the_v3_set(){
    let list=mutants();
    assert_eq!(list.iter().map(|(n,_)|n.as_str()).collect::<Vec<_>>(),[&EARNIN_MUTANTS[..],&BUDGET_MUTANTS[..],&EVIDENCE_MUTANTS[..1],&LATE_MUTANTS[..2],&REPLACE_MUTANTS[..]].concat(),"the MinerRegistry mutants");
    let list=&list[..EARNIN_MUTANTS.len()];
    let j:serde_json::Value=serde_json::from_str(MUTANTS).unwrap();let v3:serde_json::Value=serde_json::from_str(V3).unwrap();
    assert_eq!((&j["compiler"],&j["settings"]),(&v3["compiler"],&v3["settings"]),"mutants built by the same compiler and settings");
    let original=runtime(V3,"MinerRegistry");
    for (n,code) in list{
        assert_ne!(code,&original,"{n} changes the bytecode");
        let base=earnin_on_v3("earnin",&Setup::v3().with_registry(code.clone()));
        assert!(base.is_err(),"mutant {n} survives the earn-in vectors as the v3 model runs them");
        eprintln!("mutant {n}: killed ({})",base.err().unwrap());
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
    for s in [Setup::v3()]{
        let (writes,reg_gas,top_gas)=ceilings(&s);
        eprintln!("v3 CEILINGS {writes:?} (earn-in {earn_writes:?}) gas: register-zero={reg_gas} (earn-in {earn_reg}) topUp-promote-with-return={top_gas} (earn-in {earn_top})");
        assert_eq!(earn_writes,(11,3,5,2,4));
        // V3-08: a topUp is maintenance, so it also writes lastMaintHeight (w1 bits 224..255 since V3-PR-E4) and the
        // budget slot 9 where the earn-in set writes slot 3: one slot more for a plain topUp. FN-2b: no accruing count
        // (slot 8), so an ACCRUING registration and a promotion (by topUp or accrual) each write one slot less; a
        // promoting topUp's two V3-08 slots and its missing count write net out.
        assert_eq!(writes,(10,4,5,2,3));
        // pocold's wallet quotes EARNIN_REGISTER_GAS (400,000) for an earn-in/v3 registration and
        // REGISTRY_GAS (1,000,000) for a top-up; 200,000 is the tighter top-up bound kept from
        // earnin_contracts.rs, so both quotes keep their margin. The cfg() reads add only a few
        // thousand gas.
        assert!(reg_gas<=400_000&&top_gas<=200_000,"register {reg_gas}, topUp {top_gas}");
        // A zero-bond registration costs less than on the earn-in set: FN-2b removed the probation-pool check (the
        // cfg(P_MAX) read and the accruing count's read and write), more than the cold read of the V3-08 budget slot 9
        // (2,100 gas) it adds for REG_OPS less the maintenance used.
        assert!(reg_gas<earn_reg&&earn_reg-reg_gas<=25_000,"register gas {reg_gas} vs earn-in {earn_reg}");
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
    // FN-2b: the probation pool is the earn-in set's alone.
    assert_eq!(ProgramSet::EarnIn.probation(),Some(Probation{seats:8,blocks:60_480}));
    assert_eq!((ProgramSet::V3.probation(),ProgramSet::V1.probation(),ProgramSet::Continuous.probation()),(None,None,None));
}

/// The v3 contracts read CP fields 52..58 as 0 on a 52-field CP (H_half 0, R_react 0, rho_bp 0), so nothing in the programs
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
        "E_win"=>26,"U"=>27,"R_react"=>56,"C_react"=>57,
        "m"=>8,"kappa"=>9,"S_max"=>10,"W_w"=>12,"alpha_bp"=>13,"gamma_bp"=>14,"S0"=>15,"H_half"=>52,"S_tail"=>53,"gamma_tip_bp"=>54,"rho_bp"=>58,
        o=>panic!("unknown CP field {o}")}
}
fn strs(v:&[&str])->Vec<String>{v.iter().map(|x|x.to_string()).collect()}
fn ids_of(db:&InMemoryDB,base_slot:u64,len_slot:u64)->Vec<Address>{
    (0..reg(db,len_slot).to::<u64>()).map(|i|Address::from_word(storage(db,registry_address(),base(base_slot)+U256::from(i)).into())).collect()
}
/// Bytecode-only conservation, independent of any model: held_total (slot 4) is the sum of live bonds over the record
/// array; FN-2b writes no accruing count (slot 8) and no epoch totals (slot 10), and no record uses w0 bits 216..255.
fn conserved(h:&Harness)->Result<(),String>{
    let mut held=U256::ZERO;
    for id in ids_of(&h.db,5,0){
        let (w0,_,w2,_)=h.record(id);let st=bits(w0,16,8);
        ensure!(w0>>216u32==U256::ZERO,"w0 bits 216..255 written");
        if bits(w0,0,8)!=0&&bits(w0,8,8)==0&&(st==1||st==4){held+=w2;}
    }
    if h.v3{ensure_eq!(reg(&h.db,8),U256::ZERO,"the accruing count");ensure_eq!(reg(&h.db,10),U256::ZERO,"slot 10");}
    ensure_eq!(reg(&h.db,4),held,"held_total");
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
        if op[0]=="removal_status"{let got=h.view.take();ensure_eq!(got.as_deref(),step["result"].as_str(),"{name_} {op:?}: removalStatus (the removal code)");}
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
    let records=fin["records"].as_object().unwrap();
    for (n,r) in records{
        let (w0,w1,w2,w3)=h.record(name(n));
        let got=(bits(w0,0,8),bits(w0,16,8),w2.to_string(),bits(w0,8,8)!=0,bits(w0,152,64),Address::from_word(w1.into()),
            (bits(w3,0,64),bits(w3,64,64),bits(w3,128,64),bits(w3,192,64)),bits(w0,88,64),(bits(w1,224,32),bits(w1,160,64)),w0>>216u32,bits(w0,87,1));
        let u=|k:&str|r[k].as_u64().unwrap();
        // FN-2b: w0 bits 216..255 (the deleted epoch counters) and bit 87 (the deleted POOL_OUT) are never written.
        let want=(u("status"),u("bond_state"),r["bond"].as_str().unwrap().to_string(),r["tomb"].as_bool().unwrap(),u("registered_at"),name(r["reward"].as_str().unwrap()),
            (u("exit"),u("due"),u("burn"),u("grace")),u("last_share"),(u("last_maint"),u("last_react")),U256::ZERO,0);
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
const BUDGET_SCENARIOS:[&str;19]=["accruing-paths","attacker-cycle","maintenance-budget-many-identities","maintenance-clock-per-record",
    "maintenance-spam-one-identity","maintenance-top-up","random-1","random-2","random-3","react-budget-and-cooldown","react-window-unswept",
    "reg-ops-exhausted-maint-and-react","removal-at-m-min","removal-only-at-a-full-set","replacing-budget","replacing-chain-late-credit-keeps-a-member",
    "replacing-chain-shares-keep-a-member","replacing-chain-victim-payable-and-punishable","replacing-chain-without-the-late-credit"];
/// Model-vs-bytecode differential: every scenario of the independent model reproduces on the v3 programs, step by
/// step, with held_total conserved after every step (FN-2b: and no accruing count written).
#[test]
fn v3_registry_budgets_reproduce_the_independent_model(){
    let out=budget_replay(&Setup::v3());
    assert_eq!(out.keys().map(|k|k.as_str()).collect::<Vec<_>>(),BUDGET_SCENARIOS);
    for (n,r) in &out{if let Err(e)=r{panic!("{n}: {e}")}}
    // The vectors exercise what they claim: every budget refusal, the cooldown and the window, and every removal code.
    let v:serde_json::Value=serde_json::from_str(BUDGET_VECTORS).unwrap();
    let mut seen=BTreeMap::<(String,String),usize>::new();let mut codes=BTreeSet::new();
    for body in v.as_object().unwrap().values(){for st in body["steps"].as_array().unwrap(){
        let key=(st["op"][0].as_str().unwrap().to_string(),st.get("revert").and_then(|x|x.as_str()).unwrap_or("ok").to_string());*seen.entry(key).or_default()+=1;
        if st["op"][0]=="removal_status"{codes.insert(st["result"].as_str().unwrap().to_string());}
    }}
    assert_eq!(codes,BTreeSet::from(["0","1","2","3","4"].map(String::from)),"every removal code");
    for (op,res) in [("set_reward","RegOpsLimit"),("top_up","RegOpsLimit"),("register","RegOpsLimit"),("deregister","RegOpsLimit"),("reactivate","RegOpsLimit"),
        ("reactivate","BadState"),("reactivate","ok"),("claim_bond","ok"),("remove_inactive","ok"),("punish","ok"),("top_up","ok"),("set_reward","ok"),("settle_to","ok"),
        // FN-2b C4: removal below a full set, registerReplacing landing and each way it is refused.
        ("remove_inactive","BadState"),("register_replacing","ok"),("register_replacing","NotRemovable"),("register_replacing","BadState"),
        ("register_replacing","RegOpsLimit"),("register","Capacity"),("removal_status","ok")]{
        assert!(seen.get(&(op.to_string(),res.to_string())).is_some_and(|n|*n>0),"no {op} -> {res} in the vectors");
    }
}

/// The same vectors on the earn-in set: each budget scenario differs from the v3 model there (the CP schema v2
/// fields do not exist on its 52-field ChainCaps, so R_react/C_react scenarios run at its constants; it has no
/// registerReplacing and removes below a full set).
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
    for (v3,mut s) in [(false,Setup::earnin()),(true,Setup::v3())]{
        // M_max 3: m0, s and the newcomer fill the set, where v3 removes an inactive member (FN-2b C4).
        s.cp.extend([(23,16),(17,1),(18,3)]);
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
    for (v3,mut s) in [(false,Setup::earnin()),(true,Setup::v3())]{
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
        let out=budget_replay(&Setup::v3().with_registry(code.clone()));
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
    // FN-2b C4: removeInactive needs a full set: M_max = the six active records (the earn-in set does not read it).
    let full=U256::from(apply_active(&h).len());put(&mut h.db,caps(),U256::from(18),full);
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
    let (earn,earn_reg,earn_gas)=user_path_writes(&Setup::earnin());let (v3,v3_reg,v3_gas)=user_path_writes(&Setup::v3());
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
/// inside admission's GAS_ENVELOPE (spec-1's beginBlock 926,000). For the v3 set R18 composes beginBlock from
/// GAS_ENVELOPE_EARNIN (944,000: GAS_ENVELOPE_V3 has no beginBlock entry), which v3_sys_gas_and_caps_within_the_v3_envelope
/// checks; this test keeps the stricter spec-1 bound on purpose. A fifth record word would put the worst case
/// above that envelope; this pins that it is not there.
#[test]
fn v3_sweep_deletions_keep_the_sys_envelope(){
    let envelope=pocol_lp4_execution::admission::GAS_ENVELOPE.iter().find(|(n,_)|*n=="beginBlock").unwrap().1;
    let mut out=Vec::new();
    for s in [Setup::earnin().wide(),Setup::v3().wide()]{
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
    // The v3 worst case at the program caps: all 64 scans and 16 deletions, the first 48 scans over active ACCRUING
    // records (FN-2b: no deadline to read), then 16 refunds of ACCRUING records with clocks.
    let mut h=Harness::new(&[],&Setup::v3().wide());h.at(1).unwrap();
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
    let mut h=Harness::new(&strs(&["x"]),&Setup::v3());
    for (op,want) in [(vec!["register","s","s",&b().to_string(),"1"],None),(vec!["set_reward","s","x","5"],None),
        (vec!["deregister","s","6"],None),(vec!["reactivate","s","38"],None)]{
        assert_eq!(h.step(&strs(&op)).unwrap(),want,"{op:?}");
    }
    let s=name("s");let (w0,w1,_,_)=h.record(s);
    assert_eq!((bits(w1,224,32),bits(w1,160,64),w0>>216),(5,38,U256::ZERO),"the clocks in w1; w0 bits 216..255 untouched by maintenance");
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
/// step (each block's phase 0 in the v3 order, its user transactions and phase 2), with held_total (and no accruing count), the Escrow
/// invariant (with equality) and the supply conserved after every step, and every pot, record and payment at the end.
#[test]
fn v3_evidence_penalties_reproduce_the_independent_model(){
    let out=evidence_replay(&Setup::v3());
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
    let earnin=chain_exec(&body,&Setup::earnin());let v3=chain_exec(&body,&Setup::v3());
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
        assert_eq!((begin_gain(&body,&Setup::v3(),at,who),begin_gain(&body,&Setup::earnin(),at,who)),(pcol(5*PCOL),U256::ZERO),"{who} reports in block {at}");
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
    let v3=chain_exec(&body,&Setup::v3());
    let rec=v3.record(name("r"));
    assert_eq!((bits(rec.0,8,8),bits(rec.0,16,8)),(1,2),"r is a tomb with its bond burned");
    let own=b()*U256::from(2500)/U256::from(10_000);
    assert_eq!(gain(&v3,"rr"),own+own,"rho of its own bond and of c's");
    assert_eq!(b()-own,U256::from(15*PCOL),"the self-report's net loss is (1 - rho) * B = 15 PCOL");
    // The fallback block of the same evidence has no reporter: the whole bond burns.
    // The fallback block of the same evidence has no reporter (the whole bond burns); the refused blocks that named r
    // as the reporter paid it nothing.
    let body=evidence_body("fallback-has-no-reporter");let v3=chain_exec(&body,&Setup::v3());
    assert_eq!(gain(&v3,"r"),U256::ZERO,"the refused blocks paid no reporter");
    assert!(storage(&v3.db,escrow_address(),U256::ZERO)>=b(),"w's whole bond burned");
}

/// The five planted V3-16/V3-17 mutants (compile_system_v3.cjs --mutants) each fail the evidence vectors, each in the
/// scenario written for it.
#[test]
fn the_evidence_penalty_mutants_fail_the_v3_vectors(){
    let list=mutant_programs();
    assert_eq!(list.iter().map(|m|m.0.as_str()).collect::<Vec<_>>(),[&EARNIN_MUTANTS[..],&BUDGET_MUTANTS[..],&EVIDENCE_MUTANTS[..],&LATE_MUTANTS[..],&ECON_MUTANTS[..],&REPLACE_MUTANTS[..]].concat());
    let evidence=&list[EARNIN_MUTANTS.len()+BUDGET_MUTANTS.len()..EARNIN_MUTANTS.len()+BUDGET_MUTANTS.len()+EVIDENCE_MUTANTS.len()];
    let expect=[("reporter-paid-on-fallback","fallback-has-no-reporter"),("whole-pot-for-bonded-accused","type2-held-winner-pays-contributors"),
        ("accruing-read-at-evidence","accruing-fact-committed-at-record-result"),("accruing-read-at-settle","type2-accruing-winner-burns-whole-pot"),
        ("rho-on-pot-not-bond","self-report-loses-three-quarters")];
    for ((n,program,code),(want_name,killer)) in evidence.iter().zip(expect){
        assert_eq!(n,want_name);
        let original=runtime(V3,program);assert_ne!(code,&original,"{n} changes the bytecode");
        let out=evidence_replay(&Setup::v3().with_program(program,code.clone()));
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
    let mut h=chain_exec(&body,&Setup::v3());
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
/// step by step (recordLate in phase 0 after recordResult and before settle(h-D)), with held_total (and no accruing count), the
/// Escrow invariant with equality and the supply after every step, and every pot, record (lastShare included), payment
/// and retained ShareLedger height (counts and totals) at the end.
#[test]
fn v3_late_shares_reproduce_the_independent_model(){
    let out=late_replay(&Setup::v3());
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
    let v3=chain_exec(&body,&Setup::v3());
    let r=U256::from(17*PCOL/10);
    assert_eq!(gain(&v3,"b"),r/U256::from(64),"b's late share is paid at settle(4)");
    assert_eq!(bits(v3.record(name("b")).0,88,64),4,"b's lastShare");
    let original=chain_exec(&without_late(&body),&Setup::v3());
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
        let out=late_replay(&Setup::v3().with_program(program,code.clone()));
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
    let mut h=chain_exec(&body,&Setup::v3());
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
    let out=issuance_replay(&Setup::v3());
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
    let s=Setup::v3();
    let (mut held,mut accruing,mut tomb,mut fallback)=(0,0,0,0);
    for n in ISSUANCE_SCENARIOS{
        let body=issuance_body(n);
        let mut st=s.clone();for (k,v) in body["cp"].as_object().unwrap(){st.cp.push((cp_index(k),v.as_str().map_or_else(||v.as_u64().unwrap().into(),|x|x.parse::<u128>().unwrap())));}
        let durable:Vec<String>=body["durable"].as_array().unwrap().iter().map(|x|x.as_str().unwrap().to_string()).collect();
        let mut h=Harness::new(&durable,&st);let gamma=storage(&h.db,caps(),U256::from(cp_index("gamma_tip_bp")));
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
    let s=Setup::v3();
    let (ha,hb)=(budget_run("base",&a,&s).unwrap(),budget_run("variant",&b,&s).unwrap());
    let later=pots_fixed_by_their_blocks(&ha,&hb,fixed).unwrap();
    eprintln!("blocks whose late credits differ: {differ:?}; pots that differ after block {fixed}: {later:?}");
    assert!(later.iter().all(|j|ha.closed[j].2!=hb.closed[j].2&&ha.closed[j].0==hb.closed[j].0),"only the carry differs (the same issuance and tips)");
    // The property check itself fails on the planted mutant (the bytecode run without the model's per-step comparison).
    let list=mutant_programs();
    let (_,program,code)=list.iter().find(|m|m.0=="base-burn-into-closed-pot").unwrap();
    let m=Setup::v3().with_program(program,code.clone());
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
        let v3=budget_run(n,&body,&Setup::v3()).unwrap();
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
    let at=list.len()-REPLACE_MUTANTS.len()-ECON_MUTANTS.len();let econ=&list[at..at+ECON_MUTANTS.len()];
    let expect=[("carry-not-added","issuance-halving-compressed"),("tip-share-to-pot","tip-split-signed-fallback-accruing-tomb"),
        ("base-burn-still-burned","carry-fuzz-c-1-to-smax-plus-1"),("issued-ignored-flat-s0","issuance-halving-compressed"),("carry-kept-at-final","final-block-burns-the-carry"),
        ("base-burn-into-closed-pot","late-inclusion-base"),("carry-not-lagged","last-carrier-omits-late"),("lagged-carry-kept-at-final","final-block-burns-the-carry")];
    for ((n,program,code),(want_name,killer)) in econ.iter().zip(expect){
        assert_eq!(n,want_name);assert_eq!(program,"RewardEscrow");
        let original=runtime(V3,program);assert_ne!(code,&original,"{n} changes the bytecode");
        let out=issuance_replay(&Setup::v3().with_program(program,code.clone()));
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
    let s=Setup::v3();
    let (hi,ho)=(budget_run("includes",&inc,&s).unwrap(),budget_run("omits",&omit,&s).unwrap());
    let r=last_carrier_compare(&hi,&ho).unwrap();
    eprintln!("lagged carry: {r}");
    let list=mutant_programs();
    let (_,program,code)=list.iter().find(|m|m.0=="carry-not-lagged").unwrap();
    let m=Setup::v3().with_program(program,code.clone());
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
    let h=budget_run("final",&body,&Setup::v3()).unwrap();
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
    let mut h=chain_exec(&body,&Setup::v3());
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

// ---------------- FN-2b C4/C5: removal at a full set (registerReplacing, removeInactive's gate) and activeSlice

/// The nine planted FN-2b C4 mutants each fail the registry-budget vectors (the removal scenarios of tests/v3_model.py),
/// each in the scenario written for its rule.
#[test]
fn the_removal_mutants_fail_the_v3_vectors(){
    let list=mutants();let original=runtime(V3,"MinerRegistry");
    let removal=&list[list.len()-REPLACE_MUTANTS.len()..];
    assert_eq!(removal.iter().map(|(n,_)|n.as_str()).collect::<Vec<_>>(),REPLACE_MUTANTS);
    let expect=[("removal-below-full-set","removal-only-at-a-full-set"),("removal-theta-off-by-one","replacing-chain-shares-keep-a-member"),
        ("removal-ignores-grace","removal-only-at-a-full-set"),("removal-at-m-min","removal-at-m-min"),("replacing-one-reg-ops-unit","replacing-budget"),
        ("replacing-no-r-max-entry","replacing-budget"),("replacing-victim-erased-at-once","replacing-chain-victim-payable-and-punishable"),
        ("replacing-any-victim","removal-only-at-a-full-set"),("replacing-sender-with-record","replacing-budget")];
    for ((n,code),(want,killer)) in removal.iter().zip(expect){
        assert_eq!(n,want);assert_ne!(code,&original,"{n} changes the bytecode");
        let killed:Vec<String>=budget_replay(&Setup::v3().with_registry(code.clone())).into_iter().filter(|(_,r)|r.is_err()).map(|(k,_)|k).collect();
        eprintln!("mutant {n}: killed by {killed:?}");
        assert!(killed.iter().any(|k|k==killer),"mutant {n} survives {killer}: {killed:?}");
    }
}

/// A v3 registry at a full set for the C4 measurements: M_max 4 (m0 and a, b, c with the full bond, registered at 1),
/// Theta_inact 8, REG_OPS 16, R_max 4, at height 20 (every member past its grace, none with a credited share).
fn full_set()->Harness{
    let mut s=Setup::v3();s.cp.extend([(18,4),(25,8),(23,16),(17,4)]);
    let mut h=Harness::new(&[],&s);
    for x in ["a","b","c"]{assert_eq!(h.step(&strs(&["register",x,x,&b().to_string(),"1"])).unwrap(),None);}
    h.at(20).unwrap();h
}
fn replacing_data(reward:Address,victim:Address)->Vec<u8>{let mut d=with_arg("registerReplacing(address,address)",Some(reward));d.extend(victim.into_word());d}
/// Changed storage slots of the system programs (the user context's SYS keys, user_phase.rs) in one call.
fn sys_keys(r:&ResultAndState)->usize{
    (0..5).map(|i|r.state.get(&pocol_lp4_execution::apply::program_address(i)).map(|a|a.storage.values().filter(|s|s.is_changed()).count()).unwrap_or(0)).sum()
}

/// registerReplacing's write ceilings, gas and user-context SYS keys at a full set (M_max 4): the victim in the middle of
/// the active list with a full-bond newcomer (as deregister middle, plus a registration), in the middle with a zero-bond
/// newcomer, and at the tail with a full-bond newcomer. Each stays within half the wallet's REPLACE_GAS quote (500,000)
/// and the devnet's SYS_KEYS_USER_MAX (256), so one replacement leaves room for the block's other user operations.
#[test]
fn replacing_write_ceilings_gas_and_user_sys_keys(){
    let mut out=Vec::new();
    let mut h=full_set();
    // ActList m0, a, b, c: a is in the middle (c moves into its place).
    for (n,victim,value) in [("n1","a",b()),("n2","b",U256::ZERO)]{
        let s=h.sender(n);let r=call(&mut h.db,s,replacing_data(s,name(victim)),value,20);
        assert!(r.result.is_success(),"{n}: {:?} {:?}",r.result,output(&r.result));
        out.push((n,writes(&r),sys_keys(&r),r.result.gas_used()));conserved(&h).unwrap();
        assert_eq!(bits(h.record(name(victim)).0,0,8),2,"{victim} exited");
    }
    let mut h=full_set();let s=h.sender("n3");
    let r=call(&mut h.db,s,replacing_data(s,name("c")),b(),20);assert!(r.result.is_success(),"tail: {:?}",output(&r.result));
    out.push(("n3",writes(&r),sys_keys(&r),r.result.gas_used()));conserved(&h).unwrap();
    eprintln!("REPLACING-CEILINGS (newcomer, registry slots, user SYS keys, gas) {out:?}");
    // Changed slots (a slot written back to its value is unchanged). Victim in the middle, full bond: the victim's w0 and
    // w3, the moved record's w0, the two ActList elements (the newcomer takes the freed last one; ActLen nets out), the
    // newcomer's four words, its RecArr element, RecLen, held_total and slot 3: 13. A zero-bond newcomer writes no bond
    // word and leaves held_total: 11. The victim at the tail: no moved record and one ActList element: 11.
    assert_eq!(out.iter().map(|x|x.1).collect::<Vec<_>>(),[13,11,11]);
    // pocold's wallet quotes REPLACE_GAS (500,000) for registerReplacing: at least twice every replacement measured here.
    for (n,_,keys,gas) in &out{assert!(*keys<=256&&*gas<=250_000,"{n}: {keys} SYS keys, {gas} gas");}
}

/// registerReplacing's call shape and refusals: exactly 68 bytes with two clean address words (BadCalldata), at most B_reg
/// (BadValue), only after the block's beginBlock (BadPhase), a sender without a record and a live active victim (BadState),
/// a victim removeInactive could remove now (NotRemovable: below a full set, within Theta_inact of its graceStart). At the
/// full set register alone is Capacity; removeInactive below it reverts; the victim exits as by deregister (due = h + U,
/// its record and bond kept) and the sender is a new record in its seat; the removalStatus view gives the codes.
#[test]
fn v3_register_replacing_call_shapes(){
    let mut h=full_set();let n=h.sender("n");let a=name("a");
    let call_=|h:&mut Harness,data:Vec<u8>,value:U256|->Option<String>{
        let r=call(&mut h.db,n,data,value,h.height);if r.result.is_success(){return None}
        let out=output(&r.result).to_vec();Some(["BadCalldata","BadValue","BadPhase","BadState","NotRemovable","Capacity","RegOpsLimit"].iter().find(|k|selector(&format!("{k}()"))==out).map(|k|k.to_string()).unwrap_or(format!("{out:?}")))
    };
    let d=replacing_data(n,a);
    assert_eq!(call_(&mut h,d[..36].to_vec(),b()).as_deref(),Some("BadCalldata"),"36 bytes");
    let mut long=d.clone();long.extend([0u8;32]);assert_eq!(call_(&mut h,long,b()).as_deref(),Some("BadCalldata"),"100 bytes");
    let mut dirty=d.clone();dirty[4]=1;assert_eq!(call_(&mut h,dirty,b()).as_deref(),Some("BadCalldata"),"reward word upper bits");
    let mut dirty=d.clone();dirty[36]=1;assert_eq!(call_(&mut h,dirty,b()).as_deref(),Some("BadCalldata"),"victim word upper bits");
    assert_eq!(call_(&mut h,d.clone(),b()+U256::from(1)).as_deref(),Some("BadValue"),"above B_reg");
    assert_eq!(call_(&mut h,replacing_data(n,name("nobody")),b()).as_deref(),Some("BadState"),"a victim without a record");
    assert_eq!(h.try_removal_status(name("nobody")),Ok(1));
    let r=call(&mut h.db,n,d.clone(),b(),h.height+1);assert_eq!(output(&r.result),selector("BadPhase()").as_slice(),"before beginBlock");
    assert_eq!(h.step(&strs(&["register","n","n",&b().to_string(),"20"])).unwrap().as_deref(),Some("Capacity"),"register alone at a full set");
    assert_eq!(h.try_removal_status(a),Ok(0));
    assert_eq!(call_(&mut h,d.clone(),b()),None,"the shape the wallet sends");
    let (w0,_,w2,w3)=h.record(a);
    assert_eq!((bits(w0,0,8),bits(w0,16,8),w2,bits(w3,0,64),bits(w3,64,64)),(2,1,b(),20,60),"the victim exits as by deregister (due = h + U), bond kept");
    let (w0,_,w2,w3)=h.record(n);assert_eq!((bits(w0,0,8),bits(w0,16,8),w2,bits(w3,192,64)),(1,1,b(),20),"the sender is a HELD active record in grace");
    assert_eq!((reg(&h.db,0).to::<u64>(),reg(&h.db,1).to::<u64>()),(5,4),"one more record, the set still full");
    assert_eq!(call_(&mut h,replacing_data(n,name("b")),b()).as_deref(),Some("BadState"),"the sender now has a record");
    assert_eq!(h.try_removal_status(n),Ok(4),"a newcomer is in grace");
    let m=h.sender("m");let r=call(&mut h.db,m,replacing_data(m,n),U256::ZERO,20);assert_eq!(output(&r.result),selector("NotRemovable()").as_slice(),"within Theta_inact");
    let r=call(&mut h.db,m,replacing_data(m,a),U256::ZERO,20);assert_eq!(output(&r.result),selector("BadState()").as_slice(),"an exited victim");
    // Below a full set (b deregisters): code 2, NotRemovable and a removeInactive BadState.
    assert_eq!(h.step(&strs(&["deregister","b","20"])).unwrap(),None);
    assert_eq!(h.try_removal_status(name("c")),Ok(2));
    let r=call(&mut h.db,m,replacing_data(m,name("c")),U256::ZERO,20);assert_eq!(output(&r.result),selector("NotRemovable()").as_slice(),"below a full set");
    assert_eq!(h.step(&strs(&["remove_inactive","m","c","20"])).unwrap().as_deref(),Some("BadState"),"removeInactive below a full set");
    // A user call cannot reach the view's internals: removalStatus is a view (nothing written).
    let r=call(&mut h.db,m,with_arg("removalStatus(address)",Some(name("c"))),U256::ZERO,20);assert!(r.result.is_success());assert_eq!(writes(&r),0);
}

/// ABI-decodes activeSlice's (address[] ids, uint256[] w0).
fn decode_slice(out:&[u8])->(Vec<Address>,Vec<U256>){
    let word=|i:usize|U256::from_be_slice(&out[i..i+32]);
    let arr=|off:usize|{let n=word(off).to::<usize>();(0..n).map(|k|word(off+32+32*k)).collect::<Vec<U256>>()};
    let (a,b)=(word(0).to::<usize>(),word(32).to::<usize>());
    (arr(a).into_iter().map(|x|Address::from_word(x.into())).collect(),arr(b))
}
/// FN-2b C5: activeSlice(start, count) pages the ActList for paged readers (the reward gateway at M_max 10,000):
/// ActList[start, min(start + count, active)) with each one's record word w0, empty from start >= active, for any count
/// (no overflow at count = 2^256 - 1) and any start; a view (nothing written). Checked against activeAt and recordWords
/// after exits reorder the list.
#[test]
fn active_slice_pages_the_active_list(){
    let mut h=Harness::new(&[],&Setup::v3().wide());
    for i in 0..12{assert_eq!(h.step(&strs(&["register",&format!("s{i}"),&format!("s{i}"),&(if i%3==0{0}else{20*PCOL}).to_string(),"1"])).unwrap(),None);}
    for i in [2,7]{assert_eq!(h.step(&strs(&["deregister",&format!("s{i}"),"2"])).unwrap(),None);}
    let active=apply_active(&h);assert_eq!(active.len(),11);
    let slice=|h:&mut Harness,start:U256,count:U256|{
        let mut d=selector("activeSlice(uint256,uint256)");d.extend(start.to_be_bytes::<32>());d.extend(count.to_be_bytes::<32>());
        let r=call(&mut h.db,name("viewer"),d,U256::ZERO,2);assert!(r.result.is_success(),"{:?}",r.result);assert_eq!(writes(&r),0,"a view");decode_slice(output(&r.result))
    };
    let at=|h:&mut Harness,i:usize|{let mut d=selector("activeAt(uint256)");d.extend(U256::from(i).to_be_bytes::<32>());
        let r=call(&mut h.db,name("viewer"),d,U256::ZERO,2);Address::from_word(U256::from_be_slice(output(&r.result)).into())};
    for (start,count) in [(0usize,0usize),(0,1),(0,4),(3,4),(8,3),(8,4),(9,100),(10,1),(11,1),(11,0),(12,5),(0,11),(0,12)]{
        let (ids,w0)=slice(&mut h,U256::from(start),U256::from(count));
        let end=(start+count).min(active.len()).max(start.min(active.len()));
        let want:Vec<Address>=if start>=active.len(){vec![]}else{active[start..end].to_vec()};
        assert_eq!(ids,want,"activeSlice({start}, {count})");
        for (k,id) in ids.iter().enumerate(){assert_eq!(*id,at(&mut h,start+k));assert_eq!(w0[k],h.record(*id).0,"w0 of {id}");}
    }
    let (ids,_)=slice(&mut h,U256::from(5),U256::MAX);assert_eq!(ids,active[5..].to_vec(),"count 2^256 - 1");
    let (ids,w0)=slice(&mut h,U256::MAX,U256::MAX);assert!(ids.is_empty()&&w0.is_empty(),"start 2^256 - 1");
    // The full list in pages of 4 is the ActList.
    let mut all=Vec::new();let mut start=0;
    loop{let (ids,_)=slice(&mut h,U256::from(start),U256::from(4));if ids.is_empty(){break}start+=ids.len();all.extend(ids);}
    assert_eq!(all,active);
}

/// FN-2b C5, the gateway's real read: deploy/rewards/pocol_rewards.py reads the ActList in activeSlice pages of
/// ACTIVE_PAGE, each eth_call sent with ACTIVE_SLICE_GAS (its constants, read from the source here). An ActList of
/// M_max = 10,000 members (MAX_MEMBERS_V3; written into the bytecode registry's storage: ActLen, the ActList elements and
/// each member's record word w0) is read page by page in fresh transactions with exactly that gas, so every slot is cold
/// as on a node's eth_call. Every page succeeds, uses at most a quarter of the gas sent (and of pocold's CALL_GAS_MAX,
/// 30,000,000, which caps it), and the pages give back the whole list with each w0; a page of ACTIVE_PAGE needs more than
/// the gateway's ordinary CALL_GAS of 2,000,000 (two cold slots per element), which is why it is sent with more.
#[test]
fn active_slice_reads_m_max_members_in_gateway_pages(){
    const GATEWAY:&str=include_str!("../../../deploy/rewards/pocol_rewards.py");
    let constant=|n:&str|->u64{GATEWAY.lines().find_map(|l|l.strip_prefix(&format!("{n} = "))).unwrap_or_else(||panic!("{n} in pocol_rewards.py")).trim().parse().unwrap()};
    let (page,gas,call_gas,m)=(constant("ACTIVE_PAGE"),constant("ACTIVE_SLICE_GAS"),constant("CALL_GAS"),constant("MAX_MEMBERS_V3"));
    assert_eq!((m,call_gas),(10_000,2_000_000));assert!(gas<=30_000_000&&gas<=pocol_lp4_execution::apply::PER_CALL_GAS,"ACTIVE_SLICE_GAS {gas} above CALL_GAS_MAX");
    let mut h=Harness::new(&[],&Setup::v3());
    let ids:Vec<Address>=(0..m).map(|i|name(&format!("member{i}"))).collect();
    let w0=|i:u64|U256::from(1)|(U256::from(i%5)<<16u32)|(U256::from(i)<<64u32);
    put(&mut h.db,registry_address(),U256::from(1),U256::from(m));
    for (i,id) in ids.iter().enumerate(){put(&mut h.db,registry_address(),base(6)+U256::from(i),U256::from_be_slice(id.as_slice()));put(&mut h.db,registry_address(),slot(*id),w0(i as u64));}
    let (mut all,mut words,mut most)=(Vec::new(),Vec::new(),0u64);
    for start in (0..m).step_by(page as usize){
        let mut d=selector("activeSlice(uint256,uint256)");d.extend(U256::from(start).to_be_bytes::<32>());d.extend(U256::from(page).to_be_bytes::<32>());
        let tx=TxEnv{caller:name("gateway"),transact_to:TxKind::Call(registry_address()),data:Bytes::from(d),value:U256::ZERO,gas_limit:gas,gas_price:U256::ZERO,
            nonce:None,chain_id:Some(CHAIN_ID),..Default::default()};
        let r=native::execute_at(&mut h.db,tx,U256::from(2)).unwrap();
        assert!(r.result.is_success(),"activeSlice({start}, {page}) with gas {gas}: {:?}",r.result);
        let used=r.result.gas_used();most=most.max(used);
        let (ids_,w0s)=decode_slice(output(&r.result));assert_eq!(ids_.len() as u64,page.min(m-start));
        all.extend(ids_);words.extend(w0s);
    }
    eprintln!("ACTIVE-SLICE {m} members in pages of {page}: most gas per page {most} (sent {gas}, CALL_GAS {call_gas})");
    assert_eq!(all,ids);assert!(words.iter().enumerate().all(|(i,w)|*w==w0(i as u64)));
    assert!(most<=gas/4,"a page of {page} uses {most} of {gas}");
    assert!(most>call_gas,"a page of {page} fits CALL_GAS ({most}): ACTIVE_SLICE_GAS is not needed");
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

    /// One zero-bond (or `bond`) registration of a funded devnet account with itself as the reward address.
    fn joins(r:&Run,labels:&[String],bond:u128)->Vec<Vec<u8>>{labels.iter().map(|p|r.register(p,bond,0)).collect()}
    /// FN-2b: a v3 devnet with `n` ACCRUING registrants q0..q{n-1} (each funded with 1 PCOL for its fee), M_max 1,000 (above
    /// the spec-1 domain's 256: the v3 domain has no M_max term) and the GAS_DOMAIN maxima otherwise (S_max 256, D 16).
    fn accruing_config(genesis:&[&str],n:usize)->(DevnetConfig,Vec<String>){
        let mut cfg=config(2,genesis);cfg.d=16;cfg.w_w=64;cfg.s_max=256;cfg.m_max=1000;cfg.e_win=2048;cfg.u=2200;cfg.w_act=128;cfg.theta_inact=1024;cfg.c_react=2200;
        cfg.sys_slot_max=131072;cfg.slot_cap=4_000_000;cfg.acct_cap=1_000_000;cfg.r_max=16;cfg.gamma_tip_bp=0;
        let q:Vec<String>=(0..n).map(|i|format!("q{i}")).collect();
        for p in &q{cfg.accounts.push((p.clone(),U256::from(PCOL)));}
        (cfg,q)
    }
    /// FN-2b C2, the re-measured settle (GAS_ENVELOPE_V3): settle's worst path with every payee ACCRUING. A v3 chain of four
    /// genesis members and 258 zero-bond registrants q0..q257 (at most 16 REG_OPS units per block in blocks 1..17; M_max
    /// 1,000). Pot 18, raised by a large tip, names q0..q255 as its 256 share owners (C = 257 with the winner's share), q256
    /// as its winner and q257 as its signed proposer: 258 distinct payees, all ACCRUING at bond 0, so settle(18) in block
    /// 34 fills and promotes 258 bonds from 0 to B_reg in one call, as the chain's first burn (pots 1..17 have one genesis
    /// owner each and burn nothing). That is S_max + 2 of the S_max + 3 payees GAS_DOMAIN_V3_PAYEES bounds (the winner is
    /// one of the owners). Run twice: with the genesis bonds held (held_total nonzero at the first accrue), and with
    /// held_total 0 at settle(18), which also pays accrue's first held_total write from zero: the genesis members
    /// deregister in block 1 (funded by bob for the fee; U = E_win + D = 32, so due = 33), are paid their pots 1..17 while
    /// exited (live until refunded), and claim their bonds in block 33 after settle(17) (or the sweep refunds them there),
    /// so no bond is held and nothing has burned when settle(18) runs. The held-zero run is the dearer, and the envelope
    /// entry is it rounded up to 1000.
    /// Block 34 also records block 33's result naming 256 distinct share owners, 240 of them registrants r0..r239 with a
    /// record (count, owner element and record word each) and 16 without, so its SYS keys combine a maximal recordShares
    /// with the all-ACCRUING settle: they stay within resources::derive's keys_normal, which counts settle's accrual keys
    /// (2 per payee and held_total, `settle_keys`). The call stays within GAS_ENVELOPE_V3 settle and settleFinal (the same
    /// settleOne) and below the 30M per-call cap; every block within resources::derive.
    fn settle_all_accruing(held_zero:bool)->(u64,u64,resources::SysBounds){
        let (mut cfg,q)=accruing_config(&["g0","g1","g2","g3"],258);
        if held_zero{cfg.e_win=16;cfg.u=32;}
        let r_:Vec<String>=(0..240).map(|i|format!("r{i}")).collect();
        for p in &r_{cfg.accounts.push((p.clone(),U256::from(PCOL)));}
        let mut a=Run::new(&cfg);let cp=a.net.profile.cp().clone();
        assert!(admission::in_gas_domain(&cp,ProgramSet::V3)&&!admission::in_gas_domain(&cp,ProgramSet::EarnIn),"M_max 1,000: inside the v3 domain only");
        let genesis:Vec<Address>=(0..4).map(|i|a.id(i)).collect();let qid:Vec<Address>=q.iter().map(|p|devnet::account_key(p).address()).collect();
        let one=|g:Address|->Vec<(u64,Address)>{(0..256u64).map(|i|(i+1,g)).collect()};
        let bob=devnet::account_key("bob");
        let user=|a:&Run,key:&pocol_lp4_execution::txsign::Key,nonce:u64,to:Address,value:U256,data:Vec<u8>,gas:u64|Tx{chain_id:a.net.profile.spec().chain_id,nonce,max_priority:U256::ZERO,max_fee:a.base,gas,
            to:Some(to),value,data:Bytes::from(data)}.sign_1559(key);
        let mut next=0usize;
        for k in 0..17usize{
            let mut txs=Vec::new();let mut room=16;
            if held_zero&&k==0{
                for i in 0..4{txs.push(user(&a,&bob,i as u64,genesis[i],U256::from(PCOL),vec![],21_000));}
                room-=4;
            }
            let take=room.min(258-next);txs.extend(joins(&a,&q[next..next+take],0));next+=take;
            if held_zero&&k==0{for i in 0..4{txs.push(user(&a,&a.net.miners[i].key,0,registry_address(),U256::ZERO,selector("deregister()"),400_000));}}
            a.block(Proposer::Signed(genesis[k%4]),vec![],txs,genesis[(k+1)%4],one(genesis[(k+1)%4]));
        }
        assert_eq!(next,258);
        if held_zero{for g in &genesis{let x=apply::miner_record(&a.state,*g).unwrap();assert_eq!((x.status,x.due_height),(2,33),"deregistered at 1, due at 1 + U");}}
        for p in &qid{assert_eq!(apply::miner_record(&a.state,*p).map(|x|(x.status,x.bond_state,x.bond)),Some((1,4,U256::ZERO)),"a zero-bond ACCRUING registrant");}
        let alice=devnet::account_key("alice");let tip=U256::from(400_000_000_000_000_000u128);
        let big=Tx{chain_id:777943,nonce:0,max_priority:tip,max_fee:a.base+tip,gas:21_000,to:Some(devnet::account_key("bob").address()),value:U256::ZERO,data:Bytes::new()}.sign_1559(&alice);
        let mut txs=vec![big];txs.extend(joins(&a,&r_[0..16],0));
        let o=a.block(Proposer::Signed(qid[257]),vec![],txs,qid[256],(0..256u64).map(|i|(i*3+1,qid[i as usize])).collect());
        assert!(o.tips>U256::from(8000*PCOL),"pot 18 tips {}",o.tips);
        // Block 33's result (recorded by block 34): 256 distinct owners, r0..r239 and 16 without a record.
        let late_owners:Vec<Address>=r_.iter().map(|p|devnet::account_key(p).address()).chain((0..16).map(|i|name(&format!("stranger{i}")))).collect();
        for k in 19..=33u64{
            let g=genesis[(k%4) as usize];
            let mut txs=if k<=32{joins(&a,&r_[16*(k as usize-18)..16*(k as usize-17)],0)}else{vec![]};
            if held_zero&&k==33{for i in 0..4{txs.push(user(&a,&a.net.miners[i].key,1,registry_address(),U256::ZERO,selector("claimBond()"),400_000));}}
            let shares=if k==33{late_owners.iter().enumerate().map(|(i,x)|(i as u64*3+1,*x)).collect()}else{one(g)};
            a.block(Proposer::Signed(g),vec![],txs,g,shares);
        }
        for p in &r_{assert!(apply::miner_record(&a.state,devnet::account_key(p).address()).is_some(),"{p} registered");}
        assert_eq!(storage(&a.state,escrow_address(),U256::ZERO),U256::ZERO,"pots 1..17 burned nothing");
        assert_eq!(apply::share_total(&a.state,18),257);
        if held_zero{
            for g in &genesis{assert!(apply::miner_record(&a.state,*g).is_none(),"the genesis bonds are refunded at 33");}
            assert_eq!(reg(&a.state,4),U256::ZERO,"held_total is 0 when settle(18) runs");
        }else{assert_eq!(reg(&a.state,4),U256::from(4)*b(),"the four genesis bonds are held");}
        let o=a.block(Proposer::Fallback,vec![],vec![],genesis[2],one(genesis[2]));
        let g=o.meter.call_gas.iter().find(|(n,_)|*n=="settle").unwrap().1;
        for p in &qid{assert_eq!(apply::miner_record(&a.state,*p).map(|x|(x.bond_state,x.bond)),Some((1,b())),"filled from 0 to B and promoted by settle(18)");}
        // The payees settle(18) paid on the host chain: its 258 promoted bonds are within the payee bound S_max + 3.
        assert!(qid.len() as u32<=u32::from(cp.S_max)+3&&qid.len() as u32+3>u32::from(cp.S_max)+3,"258 payees near the bound");
        assert!(storage(&a.state,escrow_address(),U256::ZERO)>U256::ZERO,"settle(18) makes the chain's first burn");
        let b=resources::derive(&cp);let keys=o.meter.keys.len() as u64;
        eprintln!("V3-SETTLE-ALL-ACCRUING held_zero={held_zero} settle(18) promoting 258 ACCRUING payees: {g}; block 34 keys {keys}, run keys {} accounts {} (derive {} {})",a.keys,a.accounts,b.keys_normal,b.accts_normal);
        assert!(keys<=b.keys_normal,"block 34: {keys} SYS keys above keys_normal {}",b.keys_normal);
        assert!(keys>=2*258+3*240,"block 34 keys {keys}: the 258 accruals (bond and state word) and 240 recorded owners (3 keys each) are all in it");
        assert!(a.keys as u64<=b.keys_normal&&a.accounts as u64<=b.accts_normal,"keys {} accounts {} vs {b:?}",a.keys,a.accounts);
        (g,keys,b)
    }
    #[test]
    fn v3_settle_gas_with_every_payee_accruing(){
        let (held,keys_held,b)=settle_all_accruing(false);let (zero,keys_zero,_)=settle_all_accruing(true);
        let bound=admission::envelope(ProgramSet::V3,"settle").unwrap();
        eprintln!("V3-SETTLE-ALL-ACCRUING held_total nonzero {held}, from zero {zero}; envelope {bound}; block 34 keys {keys_held} {keys_zero} (keys_normal {})",b.keys_normal);
        assert!(zero>held,"the first held_total write from zero costs more: {zero} vs {held}");
        for g in [held,zero]{
            assert!(g<=bound&&g<=admission::envelope(ProgramSet::V3,"settleFinal").unwrap(),"settle {g} above GAS_ENVELOPE_V3 {bound}");
            assert!(g<30_000_000,"the per-call cap");
            assert!(g>admission::envelope(ProgramSet::EarnIn,"settle").unwrap()&&g>admission::envelope(ProgramSet::V1,"settle").unwrap(),"settle {g} fits an older envelope: drop its v3 entry");
        }
        // The envelope entry is the dearer measurement rounded up to 1000 (admission.rs GAS_ENVELOPE_V3).
        assert_eq!(bound,zero.max(held).div_ceil(1000)*1000,"GAS_ENVELOPE_V3 settle is the measured worst path");
    }
    /// FN-2b C2, the carry path of the same rule: a pot with C < kappa*m (62 share owners and the winner: C = 63) leaves a
    /// base burn to the carry into pot h+1 (V3-15) while it fills and promotes every one of its 64 payees (the 63 owners and
    /// the signed proposer, all ACCRUING at bond 0). Within GAS_ENVELOPE_V3 settle, far below the all-ACCRUING worst path.
    #[test]
    fn v3_settle_gas_all_accruing_with_the_carry(){
        let (cfg,q)=accruing_config(&["g0","g1","g2","g3"],64);
        let mut a=Run::new(&cfg);let genesis:Vec<Address>=(0..4).map(|i|a.id(i)).collect();let qid:Vec<Address>=q.iter().map(|p|devnet::account_key(p).address()).collect();
        let one=|g:Address|->Vec<(u64,Address)>{(0..256u64).map(|i|(i+1,g)).collect()};
        for k in 0..4usize{let j=joins(&a,&q[16*k..16*k+16],0);a.block(Proposer::Signed(genesis[k]),vec![],j,genesis[k],one(genesis[k]));}
        let alice=devnet::account_key("alice");let tip=U256::from(100_000_000_000_000_000u128);
        let big=Tx{chain_id:777943,nonce:0,max_priority:tip,max_fee:a.base+tip,gas:21_000,to:Some(devnet::account_key("bob").address()),value:U256::ZERO,data:Bytes::new()}.sign_1559(&alice);
        a.block(Proposer::Signed(qid[63]),vec![],vec![big],qid[62],(0..62u64).map(|i|(i+1,qid[i as usize])).collect());
        for k in 6..=20u64{a.block(Proposer::Signed(genesis[(k%4) as usize]),vec![],vec![],genesis[(k%4) as usize],one(genesis[(k%4) as usize]));}
        let parent=a.state.clone();assert_eq!(apply::share_total(&parent,5),63);
        let o=a.block(Proposer::Signed(genesis[1]),vec![],vec![],genesis[1],one(genesis[1]));
        let g=o.meter.call_gas.iter().find(|(n,_)|*n=="settle").unwrap().1;
        for p in &qid{assert_eq!(apply::miner_record(&a.state,*p).map(|x|(x.bond_state,x.bond)),Some((1,b())),"promoted by settle(5)");}
        assert!(!apply::escrow_carry_into(&o.state,22).is_zero(),"C < kappa*m: settle(5) carries into pot 22");
        let bound=admission::envelope(ProgramSet::V3,"settle").unwrap();
        eprintln!("V3-SETTLE-CARRY settle(5) promoting 64 ACCRUING payees with a carry write: {g}; envelope {bound}");
        assert!(g<=bound,"settle {g} above GAS_ENVELOPE_V3 {bound}");
    }

    /// SYS gas and caps of the v3 calls at the GAS_DOMAIN maxima (S_max 256, M_max 256, D 16, E_max 4): chains of 192
    /// genesis members and 64 ACCRUING registrants p0..p63 joining in blocks 1..4 (M_max records), every block from 5 on
    /// naming all 256 owners in its shares. The worst cases of the calls V3-PR-E2b and V3-PR-E3 change (settle's is
    /// v3_settle_gas_with_every_payee_accruing):
    /// - recordResult reads an ACCRUING winner's record (pots 5..8 on chain B);
    /// - invalidate (chain B, run twice, registrants with half a bond, 0 < B/2 < B): four type-2 items name p0..p3, the
    ///   ACCRUING winners of pots 5..8, each writing INVALID;
    /// - punish: the same items punish an ACCRUING accused with a nonzero bond in the middle of the active list
    ///   (removeActive, the bond burn and the reporter's PAY to the signed proposer's reward address); four type-1 items
    ///   punish HELD genesis members (never-written clock word, middle of the active list, the reporter's PAY). Each kind
    ///   comes first on one run of chain B, so each also pays the chain's first burn;
    /// - recordLate (chain C).
    /// Each call stays within admission's envelope for the v3 set (GAS_ENVELOPE_V3 where it has an entry, GAS_ENVELOPE_EARNIN
    /// otherwise: `gas_envelope(ProgramSet::V3)`), every call name measured has an entry there, and the blocks
    /// within resources::derive; the test asserts that the punish maximum is taken on these paths.
    #[test]
    fn v3_sys_gas_and_caps_within_the_v3_envelope(){
        let labels:Vec<String>=(0..192).map(|i|format!("m{i}")).collect();let refs:Vec<&str>=labels.iter().map(|s|s.as_str()).collect();
        let mut cfg=config(2,&refs);cfg.d=16;cfg.w_w=64;cfg.s_max=256;cfg.m_max=256;cfg.e_win=2048;cfg.u=2200;cfg.w_act=128;cfg.theta_inact=1024;cfg.c_react=2200;
        cfg.sys_slot_max=131072;cfg.slot_cap=4_000_000;cfg.acct_cap=1_000_000;cfg.r_max=16;
        let probation:Vec<String>=(0..64).map(|i|format!("p{i}")).collect();
        for p in &probation{cfg.accounts.push((p.clone(),U256::from(20*PCOL)));}
        let pids:Vec<Address>=probation.iter().map(|p|devnet::account_key(p).address()).collect();
        let net=Run::new(&cfg);let cp=net.net.profile.cp().clone();
        let genesis:Vec<Address>=(0..192).map(|i|net.id(i)).collect();
        let ids:Vec<Address>=genesis.iter().copied().chain(pids.iter().copied()).collect();
        let start=|bond:u128|->Run{
            let mut r=Run::new(&cfg);
            for k in 0..4usize{
                let j=joins(&r,&probation[16*k..16*k+16],bond);
                let sh=(0..256u64).map(|i|(i*3+1,ids[((k as u64+i) as usize)%192])).collect();
                r.block(Proposer::Signed(genesis[100+k]),vec![],j,genesis[k],sh);
            }
            for p in &pids{assert_eq!(apply::miner_record(&r.state,*p).map(|x|(x.status,x.bond_state,x.bond,x.tomb)),Some((1,4,U256::from(bond),false)),"an ACCRUING registrant");}
            r
        };
        let shares=|ids:&[Address],h:u64|->Vec<(u64,Address)>{(0..256u64).map(|i|(i*3+1,ids[((h+i) as usize)%256])).collect()};

        let mut gas:BTreeMap<&'static str,u64>=BTreeMap::new();let (mut keys,mut accounts)=(0usize,0usize);
        let mut fold=|r:&Run|{for (n,g) in &r.gas{let e=gas.entry(*n).or_default();*e=(*e).max(*g);}keys=keys.max(r.keys);accounts=accounts.max(r.accounts);};
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
        eprintln!("V3-GAS {gas:?}; punish ACCRUING {punish_acc} HELD {punish_held}, recordLate recorded {} strangers {}; keys {keys} (derive {}), accounts {accounts} (derive {})",late_rec.0,late_rec.1,b.keys_normal,b.accts_normal);
        assert_eq!(gas["recordLate"],late_rec.0.max(late_rec.1));
        assert!(late_rec.1>late_rec.0,"recordLate: owners without a record {} vs recorded owners {}",late_rec.1,late_rec.0);
        assert_eq!(gas["punish"],punish_acc.max(punish_held));
        // A genesis member's clock word was never written (punish writes it from zero), while register writes every
        // registrant's (graceStart): the HELD path is the worst.
        assert!(punish_held>punish_acc,"punish HELD {punish_held} vs ACCRUING with a bond {punish_acc}");
        for (n,g) in &gas{
            let bound=admission::envelope(ProgramSet::V3,n).unwrap_or_else(||panic!("no envelope entry for {n}"));
            assert!(*g<=bound,"v3 {n} = {g} above its composed envelope {bound}");
        }
        // Every other name of the composed table was measured here (recordLate, the v3-only name, included), so no
        // ordinary or closing entry is unchecked. These chains end before their first settlement (D 16), whose worst path
        // v3_settle_gas_with_every_payee_accruing measures against the settle and settleFinal entries (the same settleOne),
        // and they are continuous (h_end 0), so the final phase is not reached here: finalize is measured on v3 by
        // v3_final_block_burns_the_carry_on_real_blocks (V3-PR-E2a, against the same composed entry).
        for (n,_) in admission::gas_envelope(ProgramSet::V3){if !["finalize","settleFinal","settle"].contains(n){assert!(gas.contains_key(n),"the v3 envelope names {n}, which no run measured");}}
        assert!(admission::gas_envelope(ProgramSet::V3).iter().any(|x|x.0=="recordLate"),"the composed v3 envelope names recordLate");
        // The v3 table is needed: each of its measured entries exceeds the earn-in entry it replaces in the composition,
        // and the spec-1 entry of the same name.
        for n in ["invalidate","punish","recordResult"]{
            assert!(gas[n]>admission::envelope(ProgramSet::EarnIn,n).unwrap(),"{n} {} fits the earn-in envelope: drop its v3 entry",gas[n]);
            assert!(gas[n]>admission::envelope(ProgramSet::V1,n).unwrap(),"{n} {} fits the spec-1 envelope: drop its v3 entry",gas[n]);}
        for set in [ProgramSet::V1,ProgramSet::Continuous,ProgramSet::EarnIn]{assert_eq!(admission::envelope(set,"recordLate"),None,"recordLate exists on the v3 set only: {set:?}");}
        assert!(keys as u64<=b.keys_normal&&accounts as u64<=b.accts_normal,"keys {keys} accounts {accounts} vs {b:?}");
    }

    /// markClose's closing phase on the v3 set at the GAS_DOMAIN maxima (each refund's Escrow.releaseBond calls back into
    /// the registry's bondRecord): 192 HELD genesis members and 64 ACCRUING registrants p0..p63 with half a bond (M_max
    /// records), RET 128, closing from H_CLOSE 8. The first closing markClose refunds 128 HELD records, the second 64 HELD
    /// and the 64 ACCRUING ones (FN-2b: no accruing count to write). Both stay within admission's envelope for the v3 set
    /// (markClose in the composed v3 table), and every record ends PAID.
    #[test]
    fn v3_closing_mark_close_gas_within_the_envelope(){
        let labels:Vec<String>=(0..192).map(|i|format!("m{i}")).collect();let refs:Vec<&str>=labels.iter().map(|s|s.as_str()).collect();
        let mut cfg=config(2,&refs);cfg.d=16;cfg.w_w=64;cfg.s_max=256;cfg.m_max=256;cfg.e_win=2048;cfg.u=2200;cfg.w_act=128;cfg.theta_inact=1024;cfg.c_react=2200;
        cfg.sys_slot_max=131072;cfg.slot_cap=4_000_000;cfg.acct_cap=1_000_000;cfg.r_max=16;cfg.ret=128;cfg.h_close=8;cfg.h_end=30;
        let probation:Vec<String>=(0..64).map(|i|format!("p{i}")).collect();
        for p in &probation{cfg.accounts.push((p.clone(),U256::from(20*PCOL)));}
        let pids:Vec<Address>=probation.iter().map(|p|devnet::account_key(p).address()).collect();
        let mut r=Run::new(&cfg);let genesis:Vec<Address>=(0..192).map(|i|r.id(i)).collect();let half=10*PCOL;
        let mut closing=Vec::new();
        for h in 1..=10u64{
            let j=if h<=4{joins(&r,&probation[16*(h as usize-1)..16*h as usize],half)}else{vec![]};
            let o=r.block(Proposer::Signed(genesis[100+h as usize]),vec![],j,genesis[h as usize],vec![(1,genesis[h as usize])]);
            if h==4{for p in &pids{assert_eq!(apply::miner_record(&o.state,*p).map(|x|(x.status,x.bond_state,x.bond)),Some((1,4,U256::from(half))),"an ACCRUING registrant with half a bond");}}
            if o.facts.closing{
                let paid=genesis.iter().chain(&pids).filter(|id|apply::miner_record(&o.state,**id).is_some_and(|x|x.bond_state==3)).count();
                closing.push((h,o.meter.call_gas.iter().find(|(n,_)|*n=="markClose").unwrap().1,paid,storage(&o.state,registry_address(),U256::from(8))));
            }
        }
        eprintln!("MARK-CLOSE closing (height, gas, PAID records, slot 8): {closing:?}");
        assert!(closing.len()>=2&&closing[0].0==8,"closing from H_CLOSE: {closing:?}");
        assert_eq!((closing[0].2,closing[1].2),(128,256),"128 HELD refunds, then 64 HELD and 64 ACCRUING");
        assert!(closing.iter().all(|c|c.3.is_zero()),"no accruing count");
        for id in genesis.iter().chain(&pids){assert_eq!(apply::miner_record(&r.state,*id).map(|x|x.bond_state),Some(3),"every record PAID");}
        let worst=closing.iter().map(|c|c.1).max().unwrap();let bound=admission::envelope(ProgramSet::V3,"markClose").unwrap();
        assert!(worst<=bound,"v3 markClose {worst} above its envelope {bound}");
        // FN-2b: within the earn-in entry, which the composed v3 table takes (GAS_ENVELOPE_V3 has no markClose entry), but
        // above spec-1's (the earn-in programs' refunds cost more).
        assert!(!admission::GAS_ENVELOPE_V3.iter().any(|x|x.0=="markClose"),"markClose fits the earn-in entry: no v3 entry");
        assert_eq!(bound,admission::envelope(ProgramSet::EarnIn,"markClose").unwrap());
        assert!(worst>admission::envelope(ProgramSet::V1,"markClose").unwrap(),"markClose {worst} fits the spec-1 envelope");
        eprintln!("markClose worst {worst}; composed v3 {bound}, earn-in {}, spec-1 {}",admission::envelope(ProgramSet::EarnIn,"markClose").unwrap(),admission::envelope(ProgramSet::V1,"markClose").unwrap());
    }

    /// recordShares at its worst through apply_block at the GAS_DOMAIN maxima (S_max 256, M_max 256 genesis members): one
    /// call naming 256 distinct recorded owners (O = min(S_max + 1, M_max)), each record's w0 written with lastShare, as the
    /// chain's first call (every record cold) and the next; and the same with 257 distinct owners without a record (each
    /// read as four words). FN-2b: one pass over ownersOf, no epoch counters or totals. The maximum stays within admission's
    /// envelope for the v3 set (recordShares in the composed v3 table).
    #[test]
    fn v3_record_shares_gas_at_the_domain_maxima(){
        let labels:Vec<String>=(0..256).map(|i|format!("m{i}")).collect();let refs:Vec<&str>=labels.iter().map(|s|s.as_str()).collect();
        let mut cfg=config(2,&refs);cfg.d=16;cfg.w_w=64;cfg.s_max=256;cfg.m_max=256;cfg.e_win=2048;cfg.u=2200;cfg.w_act=128;cfg.theta_inact=1024;cfg.c_react=2200;
        cfg.sys_slot_max=131072;cfg.slot_cap=4_000_000;cfg.acct_cap=1_000_000;
        let mut out=BTreeMap::new();
        for strangers in [false,true]{
            let mut r=Run::new(&cfg);let ids:Vec<Address>=(0..256).map(|i|r.id(i)).collect();
            let owner=|i:usize|if strangers{Address::from_word(keccak256(format!("share-owner-{i}")).into())}else{ids[i%256]};
            let mut g=Vec::new();
            for h in 1..=3u64{
                let shares:Vec<(u64,Address)>=(0..256u64).map(|i|(i+1,owner(i as usize+usize::from(strangers)))).collect();
                let winner=if strangers{owner(0)}else{ids[(h as usize)%256]};
                r.block(Proposer::Signed(ids[200]),vec![],vec![],winner,shares);
                if h>=2{g.push(r.gas.get("recordShares").copied().unwrap());r.gas.clear();}
            }
            let w=apply::miner_record(&r.state,ids[5]).unwrap();
            if !strangers{assert_eq!(w.last_share,2,"recorded owners: lastShare = j");}
            assert!(storage(&r.state,registry_address(),U256::from(10)).is_zero(),"no epoch totals");
            out.insert(strangers,g);
        }
        let worst=out.values().flatten().copied().max().unwrap();
        let bound=admission::envelope(ProgramSet::V3,"recordShares").unwrap();
        eprintln!("RECORD-SHARES gas (first call, second): recorded owners {:?}, owners without a record {:?}; composed v3 {bound}, earn-in {}, spec-1 {}",
            out[&false],out[&true],admission::envelope(ProgramSet::EarnIn,"recordShares").unwrap(),admission::envelope(ProgramSet::V1,"recordShares").unwrap());
        assert!(worst<=bound,"recordShares {worst} above its envelope {bound}");
        // The v3 entry is needed: the stranger path exceeds the earn-in entry it replaces in the composition and the spec-1
        // entry of the same name; the entry is this measurement rounded up to 1000.
        assert!(worst>admission::envelope(ProgramSet::EarnIn,"recordShares").unwrap(),"recordShares {worst} fits the earn-in envelope: drop its v3 entry");
        assert!(worst>admission::envelope(ProgramSet::V1,"recordShares").unwrap(),"recordShares {worst} fits the spec-1 envelope: drop its v3 entry");
        assert_eq!(bound,worst.div_ceil(1000)*1000);
    }

    /// The earn-in refund regressions (earnin_refund_fix.rs, F1-F3) on the v3 set through apply_block: two exited records
    /// come due in one sweep, one with a durable reward and one with a reward that never sent a transaction; the block
    /// applies, the durable reward is paid its bond once, the undeliverable bond stays held and claimable, and Escrow's
    /// invariant holds; a contract that registers itself and self-destructs in one transaction stores no record.
    #[test]
    fn v3_keeps_the_earnin_refund_and_self_reward_fixes(){
        use pocol_lp4_execution::apply::{chain_facts,check_escrow_invariant,miner_record,record_list,registry,escrow};
        let mut cfg=config(2,&["a","b","c"]);cfg.r_max=16;
        let net=devnet::build(&cfg).unwrap();
        let arr=|slot:u64|U256::from_be_bytes(keccak256(U256::from(slot).to_be_bytes::<32>()).0);
        let plant=|db:&mut InMemoryDB,id:Address,reward:Address,bond:U256,due:u64|{
            let n=storage(db,registry(),U256::ZERO);let r=apply::record_slot(id);
            for (k,v) in [(r,U256::from(2u64)|(U256::from(1u64)<<16)|(n<<24)),(r+U256::from(1),U256::from_be_slice(reward.as_slice())),(r+U256::from(2),bond),(r+U256::from(3),U256::from(due)<<64),
                (arr(5)+n,U256::from_be_slice(id.as_slice())),(U256::ZERO,n+U256::from(1)),(U256::from(4),storage(db,registry(),U256::from(4))+bond)]{db.insert_account_storage(registry(),k,v).unwrap();}
            let e=escrow();let mut info=db.accounts.get(&e).unwrap().info.clone();info.balance+=bond;db.insert_account_info(e,info);
        };
        let input=|h:u64,txs:&'static [Vec<u8>]|BlockInput{height:h,timestamp:1_700_000_000+4*h,base_fee:U256::from(1_000_000_000u64),gas_limit:8_000_000,prevrandao:B256::ZERO,
            proposer:Proposer::Fallback,parent_result:None,evidence:vec![],evidence_raw:&[],transactions:txs,ancestors:vec![(0,net.genesis_hash)],cum_log_bytes:0};
        let (durable,ghost)=(name("durable-reward"),name("ghost-reward"));let (good,bad)=(name("good-id"),name("bad-id"));
        let (bg,bb)=(U256::from(5_000_000u64),U256::from(7_000_000u64));
        let mut parent=net.genesis.state().clone();
        parent.insert_account_info(durable,AccountInfo{nonce:1,balance:U256::ZERO,..Default::default()});
        plant(&mut parent,good,durable,bg,1);plant(&mut parent,bad,ghost,bb,1);
        let held0=chain_facts(&parent).held_total;
        let o=apply::apply_block(&parent,&net.genesis,&net.profile,&input(1,&[])).expect("the sweep never halts the block");
        assert!(miner_record(&o.state,good).is_none(),"the durable payee's record is erased");
        assert_eq!(balance(&o.state,durable),bg,"paid exactly its bond");
        assert_eq!(miner_record(&o.state,bad).map(|r|(r.status,r.bond)),Some((2,bb)),"the undeliverable bond stays claimable");
        assert_eq!(chain_facts(&o.state).held_total,held0-bg);
        check_escrow_invariant(&o.state,&net.profile,1,false).unwrap();
        let pr=ParentResult{block_hash:B256::from(U256::from(1)),winner:net.miners[0].id,shares:vec![]};
        let input2=BlockInput{parent_result:Some(pr),..input(2,&[])};
        let o2=apply::apply_block(&o.state,&net.genesis,&net.profile,&input2).unwrap();
        assert_eq!(balance(&o2.state,durable),bg,"not paid twice");assert_eq!(miner_record(&o2.state,bad).map(|r|r.bond),Some(bb));
        // The source fix: register(self) from a constructor that self-destructs stores nothing.
        let sel=|s:&str|{let h=keccak256(s.as_bytes());[h[0],h[1],h[2],h[3]]};
        let store=|c:&mut Vec<u8>,s:[u8;4]|{c.push(0x63);c.extend_from_slice(&s);c.extend_from_slice(&[0x60,0xE0,0x1b,0x60,0x00,0x52]);};
        let call_=|c:&mut Vec<u8>,value:u8,len:u8|c.extend_from_slice(&[0x60,0x00,0x60,0x00,0x60,len,0x60,0x00,0x60,value,0x62,0xC0,0xC0,0x01,0x5a,0xf1,0x50]);
        let mut init=Vec::new();store(&mut init,sel("register(address)"));init.extend_from_slice(&[0x30,0x60,0x04,0x52]);call_(&mut init,1,0x24);
        store(&mut init,sel("deregister()"));call_(&mut init,0,0x04);init.extend_from_slice(&[0x33,0xff]);
        let attacker=devnet::account_key("alice");let x=attacker.address().create(0);
        let tx=Tx{chain_id:777943,nonce:0,max_priority:U256::from(1_000_000_000u64),max_fee:U256::from(100_000_000_000u64),gas:3_000_000,to:None,value:U256::from(1u64),data:Bytes::from(init)}.sign_1559(&attacker);
        let txs:&'static [Vec<u8>]=Box::leak(vec![tx].into_boxed_slice());
        let o=apply::apply_block(net.genesis.state(),&net.genesis,&net.profile,&input(1,txs)).expect("the deploy block applies");
        assert!(miner_record(&o.state,x).is_none(),"a self-destructing contract registered a self reward");
        assert_eq!(record_list(&o.state).len(),3);check_escrow_invariant(&o.state,&net.profile,1,false).unwrap();
    }
    /// The rest of earnin_halt_proof.rs (the F1/F2/F3 constructor halt) on the v3 set through apply_block, every block a
    /// real one (Run checks the supply after each): a contract deployed at H_REG = 5 whose constructor calls
    /// register(reward) with 1 wei and then deregister(), and then (0) self-destructs with itself as the reward, (1) persists
    /// with itself as the reward, or (2, the positive control) self-destructs with a durable distinct reward D (a genesis
    /// member's reward address). (0) and (1) store no record (durability is judged at registration; the persistent
    /// contract exists afterwards); (2) stores an exited record paying D, due at H_REG + U = 45. The blocks DUE - 1 ..
    /// DUE + 2 all apply; in (2) the due-height sweep refunds D its bond once and erases the record.
    #[test]
    fn v3_keeps_the_earnin_halt_proof_cases(){
        let h_reg=5u64;
        let sel=|s:&str|{let h=keccak256(s.as_bytes());[h[0],h[1],h[2],h[3]]};
        let store=|c:&mut Vec<u8>,s:[u8;4]|{c.push(0x63);c.extend_from_slice(&s);c.extend_from_slice(&[0x60,0xE0,0x1b,0x60,0x00,0x52]);};
        let call_=|c:&mut Vec<u8>,value:u8,len:u8|c.extend_from_slice(&[0x60,0x00,0x60,0x00,0x60,len,0x60,0x00,0x60,value,0x62,0xC0,0xC0,0x01,0x5a,0xf1,0x50]);
        let init=|reward:Option<Address>,destroy:bool|->Vec<u8>{
            let mut c=Vec::new();store(&mut c,sel("register(address)"));
            match reward{None=>c.extend_from_slice(&[0x30,0x60,0x04,0x52]),Some(a)=>{c.push(0x73);c.extend_from_slice(a.as_slice());c.extend_from_slice(&[0x60,0x04,0x52]);}}
            call_(&mut c,1,0x24);store(&mut c,sel("deregister()"));call_(&mut c,0,0x04);
            if destroy{c.extend_from_slice(&[0x33,0xff]);}else{c.extend_from_slice(&[0x60,0x00,0x60,0x00,0xf3]);}
            c
        };
        for case in 0..3{
            let mut r=Run::new(&config(2,&["a","b","c"]));let u=u64::from(r.net.profile.cp().U);let due=h_reg+u;
            let (w,d)=(r.id(0),r.reward(1));
            while r.h+1<h_reg{r.block(Proposer::Fallback,vec![],vec![],w,vec![(1,w)]);}
            let attacker=devnet::account_key("alice");let x=attacker.address().create(0);
            let reward=if case==2{Some(d)}else{None};
            let tx=Tx{chain_id:r.net.profile.spec().chain_id,nonce:0,max_priority:U256::ZERO,max_fee:r.base,gas:3_000_000,to:None,value:U256::from(1u64),data:Bytes::from(init(reward,case!=1))}.sign_1559(&attacker);
            let d0=r.bal(d);
            r.block(Proposer::Fallback,vec![],vec![tx],w,vec![(1,w)]);assert_eq!(r.h,h_reg);
            let rec=apply::miner_record(&r.state,x);
            let code=r.state.accounts.get(&x).map(|a|(a.info.nonce,a.account_state!=revm::db::AccountState::NotExisting));
            match case{
                0=>{assert!(rec.is_none(),"a self-destructing constructor with a self reward stores no record");assert!(!code.is_some_and(|c|c.1&&c.0>0),"the account is deleted: {code:?}");}
                1=>{assert!(rec.is_none(),"durability is judged at registration, not after the constructor");assert_eq!(code.map(|c|c.0),Some(1),"the persistent contract exists");}
                _=>{let x_=rec.unwrap();assert_eq!((x_.status,x_.reward,x_.bond,x_.due_height),(2,d,U256::from(1),due),"the control registers, exits and is due at H_REG + U");}
            }
            while r.h<due+2{r.block(Proposer::Fallback,vec![],vec![],w,vec![(1,w)]);}
            if case==2{
                assert!(apply::miner_record(&r.state,x).is_none(),"the due-height sweep erased the control's record");
                assert_eq!(r.bal(d)-d0,U256::from(1),"D is refunded the 1 wei bond once");
            }
            assert_eq!(apply::record_list(&r.state).len(),3,"case {case}: only the genesis records remain");
        }
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
    /// (state word, held_total) and the excess is PAID, while held_total is zero (no bond held anywhere: the
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
