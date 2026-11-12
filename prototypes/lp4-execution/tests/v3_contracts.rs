//! V3 system programs (system-v3.json, PoColSystemV3.sol; V3-PR-E0, V3-14 cfg part, V3-34 registry; V3-PR-E1,
//! V3-08/V3-09 registry budgets).
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
//!   in w0 bits 216..255 and w1 bits 160..223, and slot 3's per-block counters);
//! - the registry budgets: every scenario of the independent model tests/v3_model.py
//!   (tests/v3/registry-budget-vectors.json) reproduces on the v3 bytecode step by step, with ACC_COUNT and
//!   held_total conserved after every step; the fails-on-original attacks of V3-DESIGN.md V3-08/V3-09 freeze the
//!   earn-in registry and not the v3 one; five planted budget mutants each fail;
//! - the CP is really read: at P_MAX=64/T_PROB=86400 (O-14=b) the v3 set reproduces the model at those values
//!   (tests/v3/earnin-vectors-p64.json, tests/v3_model_vectors.py; "top-up" as the v3 model runs it), which the
//!   earn-in set does not;
//! - the six planted earn-in mutants (EARN-IN-SPEC.md §9), restated on the v3 source
//!   (tests/v3/system-v3-mutants.json, compile_system_v3.cjs --mutants), each fail both replays (the earn-in
//!   scenarios as the v3 model runs them, at both value sets);
//! - the write ceilings and gas the wallet's bond quotes are sized from: unchanged except for the measured V3-08/09
//!   additions (REGISTRY-LAYOUT.md), and the beginBlock sweep within admission's GAS_ENVELOPE;
//! - sysCodeHash pins and the registry: the v3 set is selected only on a CP schema v2 (specVersion 2) genesis.
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
/// The earn-in model's scenarios (both value sets) re-run through the v3 model (tests/v3_model.py earnin_on_v3).
const EARNIN_ON_V3:&str=include_str!("v3/earnin-on-v3-vectors.json");
/// The six planted earn-in mutants (EARN-IN-SPEC.md §9) and the five V3-08/V3-09 budget mutants, in fixture order.
const EARNIN_MUTANTS:[&str;6]=["p-max-plus-one","r9-removed","no-promotion-at-b","t-prob-plus-one","over-fill-bad-value","excess-kept"];
const BUDGET_MUTANTS:[&str;5]=["maint-counter-shared","maint-twice-per-block","react-budget-ignored","react-cooldown-off-by-one","react-window-at-sweep-height"];
/// The v3 set's sysCodeHash = keccak256(RLP([code(C0C001..C0C005), 0x80])) of system-v3.json.
const V3_SYS_CODE_HASH:&str="4f42aeb76a6aa1cad6ba54ea045b3bbd2d105c84dd7a6cd226f4eead22114c1a";

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
fn mutants()->Vec<(String,Bytes)>{
    let j:serde_json::Value=serde_json::from_str(MUTANTS).unwrap();
    j["mutants"].as_array().unwrap().iter().map(|m|(m["name"].as_str().unwrap().to_string(),Bytes::from(unhex(m["runtime"].as_str().unwrap())))).collect()
}
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
#[derive(Clone)]
struct Setup{registry:Bytes,escrow:Bytes,caps:Bytes,cp_len:u64,cp:Vec<(u64,u128)>}
impl Setup{
    fn earnin()->Self{Self{registry:runtime(EARNIN,"MinerRegistry"),escrow:runtime(EARNIN,"RewardEscrow"),caps:runtime(EARNIN,"ChainCaps"),cp_len:52,cp:vec![]}}
    /// R_react 16 (as the harness's R_max) and C_react 40 (= U, the least ParamGate admits) in CP 61/62.
    fn v3(p_max:u64,t_prob:u64)->Self{
        Self{registry:runtime(V3,"MinerRegistry"),escrow:runtime(V3,"RewardEscrow"),caps:runtime(V3,"ChainCaps"),cp_len:67,cp:vec![(54,p_max.into()),(55,t_prob.into()),(61,16),(62,40)]}
    }
    /// The model at P_MAX 64 runs M_MAX 256 and registers 65 probationers in one block (v3_model_vectors.py):
    /// M_max, R_max and REG_OPS wide enough that only P_MAX refuses a seat.
    fn wide(mut self)->Self{self.cp.extend([(17,255),(18,256),(23,1024)]);self}
    fn with_registry(mut self,code:Bytes)->Self{self.registry=code;self}
}

struct Harness{db:InMemoryDB,height:u64,senders:BTreeSet<Address>,settled:BTreeMap<Address,U256>,burned:U256,start:BTreeMap<Address,U256>}
impl Harness{
    fn new(durable:&[String],s:&Setup)->Self{
        let mut db=InMemoryDB::default();
        insert(&mut db,system(),U256::ZERO,1,Bytecode::default());
        for (a,code) in [(registry_address(),&s.registry),(escrow_address(),&s.escrow),(caps(),&s.caps)]{insert(&mut db,a,U256::ZERO,1,Bytecode::new_raw(code.clone()));}
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
        Harness{db,height:0,senders:BTreeSet::new(),settled:BTreeMap::new(),burned:U256::ZERO,start}
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
            "sweep"=>{self.at(num(1) as u64)?;return Ok(None)}
            "punish"=>{let id=name(&op[1]);self.at(num(2) as u64)?;let r=call(&mut self.db,system(),with_arg("punish(address)",Some(id)),U256::ZERO,self.height);ensure!(r.result.is_success(),"punish: {:?}",r.result);return Ok(None)}
            "settle_to"=>{self.settle(name(&op[1]),U256::from(num(2)))?;return Ok(None)}
            o=>panic!("unknown op {o}"),
        };
        if res.result.is_success(){return Ok(None)}
        let out=output(&res.result);
        let known=["BadValue","BadState","Capacity","RewardNotDurable","PaymentFailed","BadCalldata","RegOpsLimit"];
        Ok(Some(known.iter().find(|k|selector(&format!("{k}()"))==out).map(|k|k.to_string()).unwrap_or(format!("{out:?}"))))
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
        // V3-08/V3-09 state, and only it, differs: the budget slot 9, the clocks in w0 bits 216..255 and w1 bits
        // 160..223 of every identity and slot 3's per-block counters (bits 0..23, which every beginBlock clears). The
        // earn-in set has none of the first two.
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

/// Splits the V3-08/V3-09 state out of a Registry storage map: slot 9 and the clock bits (w0 216..255, w1 160..255)
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
    assert_eq!(list.iter().map(|(n,_)|n.as_str()).collect::<Vec<_>>(),[&EARNIN_MUTANTS[..],&BUDGET_MUTANTS[..]].concat());
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
        // V3-08: a topUp is maintenance, so it also writes w0 (lastMaintHeight) and the budget slot 9 where the earn-in
        // set writes slot 3: one slot more for a plain topUp; a promoting topUp writes w0 anyway; registration and
        // accrual are unchanged.
        assert_eq!(writes,(11,4,5,2,4));
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
    // Only MinerRegistry and ChainCaps differ from the earn-in set; the other three runtimes are the same bytes.
    let e=SystemPrograms::earnin();
    assert_eq!((v3.codes[1]==e.codes[1],v3.codes[2]==e.codes[2],v3.codes[4]==e.codes[4]),(true,true,true));
    assert!(v3.codes[0]!=e.codes[0]&&v3.codes[3]!=e.codes[3]);
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

/// CP schema names of tests/v3_model.py's CP_INDEX.
fn cp_index(n:&str)->u64{
    match n{"D"=>11,"R_max"=>17,"M_max"=>18,"M_min"=>19,"SWEEP_MAX"=>21,"SWEEP_SCAN"=>22,"REG_OPS"=>23,"W_act"=>24,"Theta_inact"=>25,
        "E_win"=>26,"U"=>27,"P_MAX"=>54,"T_PROB"=>55,"R_react"=>61,"C_react"=>62,o=>panic!("unknown CP field {o}")}
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
/// One scenario of registry-budget-vectors.json on a program set; Err names the first difference from the model.
fn budget_scenario(name_:&str,body:&serde_json::Value,base_setup:&Setup)->Result<(),String>{
    let mut s=base_setup.clone();
    for (k,v) in body["cp"].as_object().unwrap(){s.cp.push((cp_index(k),v.as_u64().unwrap().into()));}
    if s.cp.iter().any(|(n,_)|*n>=s.cp_len){return Err(format!("{name_}: a CP field outside this program set's ChainCaps"))}
    let durable:Vec<String>=body["durable"].as_array().unwrap().iter().map(|x|x.as_str().unwrap().to_string()).collect();
    let mut h=Harness::new(&durable,&s);
    for step in body["steps"].as_array().unwrap(){
        let op:Vec<String>=step["op"].as_array().unwrap().iter().map(|x|x.as_str().unwrap().to_string()).collect();
        let got=h.step(&op)?;
        match (&got,step["ok"].as_bool().unwrap()){
            (None,true)=>{}
            (Some(e),false)=>ensure_eq!(e.as_str(),step["revert"].as_str().unwrap(),"{name_} {op:?}"),
            _=>return Err(format!("{name_} {op:?}: contract {got:?}, model {step}")),
        }
        conserved(&h).map_err(|e|format!("{name_} after {op:?}: {e}"))?;
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
            (bits(w3,0,64),bits(w3,64,64),bits(w3,128,64),bits(w3,192,64)),bits(w0,88,64),(bits(w0,216,40),bits(w1,160,64)));
        let u=|k:&str|r[k].as_u64().unwrap();
        let want=(u("status"),u("bond_state"),r["bond"].as_str().unwrap().to_string(),r["tomb"].as_bool().unwrap(),u("registered_at"),name(r["reward"].as_str().unwrap()),
            (u("exit"),u("due"),u("burn"),u("grace")),u("last_share"),(u("last_maint"),u("last_react")));
        ensure_eq!(got,want,"{name_} record {n}");
        ensure!(w1>>224==U256::ZERO,"{name_} record {n}: w1 bits outside the layout");
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
    Ok(())
}
fn budget_replay(s:&Setup)->BTreeMap<String,Result<(),String>>{
    let v:serde_json::Value=serde_json::from_str(BUDGET_VECTORS).unwrap();
    v.as_object().unwrap().iter().map(|(n,body)|(n.clone(),budget_scenario(n,body,s))).collect()
}
const BUDGET_SCENARIOS:[&str;10]=["acc-count-paths","attacker-cycle","maintenance-budget-many-identities","maintenance-spam-one-identity",
    "maintenance-top-up","random-1","random-2","random-3","react-budget-and-cooldown","react-window-unswept"];

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

/// The five planted V3-08/V3-09 mutants (compile_system_v3.cjs --mutants) each fail the budget vectors.
#[test]
fn the_registry_budget_mutants_fail_the_v3_vectors(){
    let list=mutants();let original=runtime(V3,"MinerRegistry");
    let budget=&list[EARNIN_MUTANTS.len()..];
    assert_eq!(budget.iter().map(|(n,_)|n.as_str()).collect::<Vec<_>>(),BUDGET_MUTANTS);
    for (n,code) in budget{
        assert_ne!(code,&original,"{n} changes the bytecode");
        let out=budget_replay(&Setup::v3(8,60_480).with_registry(code.clone()));
        let killed:Vec<&String>=out.iter().filter(|(_,r)|r.is_err()).map(|(k,_)|k).collect();
        eprintln!("mutant {n}: killed by {killed:?}");
        assert!(!killed.is_empty(),"mutant {n} survives the registry-budget vectors");
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
/// add one slot exactly where REGISTRY-LAYOUT.md says (w0 on setRewardAddress, w1 on reactivate; slot 9 replaces
/// slot 3 on maintenance and reactivation), and nothing elsewhere.
#[test]
fn v3_registry_write_ceilings(){
    let (earn,earn_reg,earn_gas)=user_path_writes(&Setup::earnin());let (v3,v3_reg,v3_gas)=user_path_writes(&Setup::v3(8,60_480));
    eprintln!("WRITE-CEILINGS register,reward,deregister-middle,deregister-tail,reactivate,removeInactive-tail,claim-middle,claim-tail earn-in {earn:?} v3 {v3:?}; gas register earn-in {earn_reg} v3 {v3_reg}, other paths earn-in {earn_gas} v3 {v3_gas}");
    // The earn-in counts are REGISTRY-LAYOUT.md's register10, reward2, deactivate middle7/tail5, reactivate5,
    // claim middle10/tail8.
    assert_eq!(earn,[10,2,7,5,5,5,10,8]);
    // V3-08/V3-09: setRewardAddress +1 (w0 for lastMaintHeight; slot 9 in place of slot 3), reactivate +1 (w1 for
    // lastReactHeight; slot 9 in place of slot 3); register, deregister, removeInactive and claimBond unchanged.
    assert_eq!(v3,[10,3,7,5,6,5,10,8]);
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

/// The clocks sit in spare record bits: lastMaintHeight modulo 2^40 (w0 bits 216..255), lastReactHeight in full (w1
/// bits 160..223). The modular clock can only refuse: a maintenance operation exactly 2^40 blocks after the previous
/// one reverts once and succeeds a block later. The views: recordWords returns w1 as the reward address alone,
/// recordClocks the two clocks.
#[test]
fn record_clocks_layout_and_views(){
    let wrap=1u64<<40;
    let mut h=Harness::new(&strs(&["x"]),&Setup::v3(8,60_480));
    for (op,want) in [(vec!["register","s","s",&b().to_string(),"1"],None),(vec!["set_reward","s","x","5"],None),
        (vec!["deregister","s","6"],None),(vec!["reactivate","s","38"],None)]{
        assert_eq!(h.step(&strs(&op)).unwrap(),want,"{op:?}");
    }
    let s=name("s");let (w0,w1,_,_)=h.record(s);
    assert_eq!((bits(w0,216,40),bits(w1,160,64),w1>>224),(5,38,U256::ZERO));
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
    assert_eq!(bits(h.record(s).0,216,40),6);
    // The C_react clock is a full height: far heights compare exactly.
    let far=6+wrap;
    assert_eq!(h.step(&strs(&["deregister","s",&far.to_string()])).unwrap(),None);
    assert_eq!(h.step(&strs(&["reactivate","s",&(far+32).to_string()])).unwrap(),None);
    assert_eq!(bits(h.record(s).1,160,64),far+32);
    assert_eq!(h.step(&strs(&["deregister","s",&(far+33).to_string()])).unwrap(),None);
    assert_eq!(h.step(&strs(&["reactivate","s",&(far+71).to_string()])).unwrap().as_deref(),Some("BadState"),"inside C_react 40");
    assert_eq!(h.step(&strs(&["reactivate","s",&(far+72).to_string()])).unwrap(),None,"exactly last + C_react");
}
