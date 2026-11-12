//! V3 system programs (system-v3.json, PoColSystemV3.sol; V3-PR-E0, V3-14 cfg part, V3-34 registry).
//!
//! The v3 source is PoColSystemEarnIn.sol with only P_MAX/T_PROB read through cfg() (CP schema v2 fields 54/55)
//! and ChainCaps bounded by the v2 CP length. This file proves:
//! - behaviour-equal: with P_MAX=8/T_PROB=60480 in the CP, every model scenario of tests/data/earnin-vectors.json
//!   replays on the v3 set with the same reverts, records, totals and payments as on the earn-in set, and leaves
//!   the identical Registry and Escrow storage;
//! - the CP is really read: at P_MAX=64/T_PROB=86400 (O-14=b) the v3 set reproduces the model at those values
//!   (tests/v3/earnin-vectors-p64.json, tests/v3_model_vectors.py), which the earn-in set does not;
//! - the six planted earn-in mutants (EARN-IN-SPEC.md §9), restated on the v3 source
//!   (tests/v3/system-v3-mutants.json, compile_system_v3.cjs --mutants), each fail both replays;
//! - the write ceilings and gas the wallet's bond quotes are sized from are unchanged;
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
/// The v3 set's sysCodeHash = keccak256(RLP([code(C0C001..C0C005), 0x80])) of system-v3.json.
const V3_SYS_CODE_HASH:&str="1e7116a71e9bc605989f41fdd48cf2bdb8336b151a4622219cf733f3b044629e";

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
    fn v3(p_max:u64,t_prob:u64)->Self{
        Self{registry:runtime(V3,"MinerRegistry"),escrow:runtime(V3,"RewardEscrow"),caps:runtime(V3,"ChainCaps"),cp_len:67,cp:vec![(54,p_max.into()),(55,t_prob.into())]}
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
            "sweep"=>{self.at(num(1) as u64)?;return Ok(None)}
            "punish"=>{let id=name(&op[1]);self.at(num(2) as u64)?;let r=call(&mut self.db,system(),with_arg("punish(address)",Some(id)),U256::ZERO,self.height);ensure!(r.result.is_success(),"punish: {:?}",r.result);return Ok(None)}
            "settle_to"=>{self.settle(name(&op[1]),U256::from(num(2)))?;return Ok(None)}
            o=>panic!("unknown op {o}"),
        };
        if res.result.is_success(){return Ok(None)}
        let out=output(&res.result);
        let known=["BadValue","BadState","Capacity","RewardNotDurable","PaymentFailed","BadCalldata"];
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
struct Outcome{steps:Vec<Option<String>>,registry:BTreeMap<U256,U256>,escrow:BTreeMap<U256,U256>,balances:BTreeMap<Address,U256>}

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
    Ok(Outcome{steps,registry:nonzero(&h.db,registry_address()),escrow:nonzero(&h.db,escrow_address()),balances})
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

#[test]
fn v3_set_at_the_earnin_values_is_behaviour_equal_to_the_earnin_set(){
    let earnin=replay(VECTORS,&Setup::earnin()).unwrap();
    let v3=replay(VECTORS,&Setup::v3(Probation::EARNIN.seats,Probation::EARNIN.blocks)).unwrap();
    assert_eq!(v3.len(),8,"every model scenario replayed");
    for (name_,want) in &earnin{
        let got=&v3[name_];
        assert_eq!(got.steps,want.steps,"{name_}: every step's result");
        assert_eq!(got.registry,want.registry,"{name_}: identical MinerRegistry storage");
        assert_eq!(got.escrow,want.escrow,"{name_}: identical RewardEscrow storage");
        assert_eq!(got.balances,want.balances,"{name_}: identical balances");
    }
}

#[test]
fn v3_set_reads_p_max_and_t_prob_from_the_cp(){
    let p64=Setup::v3(64,86_400).wide();
    let v:serde_json::Value=serde_json::from_str(VECTORS_P64).unwrap();
    assert_eq!(v["params"],serde_json::json!({"P_MAX":64,"T_PROB":86400,"M_MAX":256}));
    assert_eq!(replay(VECTORS_P64,&p64).unwrap().len(),8,"every model scenario at P_MAX 64 / T_PROB 86400");
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
    // last sweep, 2·60480+90, still precedes a deadline of 86400 blocks after the second registration).
    let v:serde_json::Value=serde_json::from_str(VECTORS).unwrap();let mut fails=BTreeSet::new();
    for (scenario_,body) in v.as_object().unwrap(){
        if body.get("steps").is_none(){continue}
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
    assert_eq!(list.iter().map(|(n,_)|n.as_str()).collect::<Vec<_>>(),
        ["p-max-plus-one","r9-removed","no-promotion-at-b","t-prob-plus-one","over-fill-bad-value","excess-kept"]);
    let j:serde_json::Value=serde_json::from_str(MUTANTS).unwrap();let v3:serde_json::Value=serde_json::from_str(V3).unwrap();
    assert_eq!((&j["compiler"],&j["settings"]),(&v3["compiler"],&v3["settings"]),"mutants built by the same compiler and settings");
    let original=runtime(V3,"MinerRegistry");
    for (n,code) in &list{
        assert_ne!(code,&original,"{n} changes the bytecode");
        let base=replay(VECTORS,&Setup::v3(8,60_480).with_registry(code.clone()));
        let p64=replay(VECTORS_P64,&Setup::v3(64,86_400).wide().with_registry(code.clone()));
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
    let r=call(&mut h.db,a,selector("topUp()"),U256::from(12*PCOL),1);assert!(r.result.is_success(),"topUp promote, 2 PCOL returned");
    let top_promote=writes(&r);let top_gas=r.result.gas_used();
    let b2=h.sender("w-b");
    let r=call(&mut h.db,b2,with_arg("register(address)",Some(b2)),U256::ZERO,1);assert!(r.result.is_success());let reg_gas=r.result.gas_used();
    let r=h.accrue_call(b2,U256::from(PCOL));assert!(r.result.is_success());let accrue=writes(&r);
    let r=h.accrue_call(b2,U256::from(30*PCOL));assert!(r.result.is_success());let accrue_promote=writes(&r);
    ((reg_partial,top,top_promote,accrue,accrue_promote),reg_gas,top_gas)
}

#[test]
fn wallet_bond_features_keep_their_write_ceilings_and_gas_on_the_v3_set(){
    let (earn_writes,earn_reg,earn_top)=ceilings(&Setup::earnin());
    for s in [Setup::v3(8,60_480),Setup::v3(64,86_400)]{
        let (writes,reg_gas,top_gas)=ceilings(&s);
        eprintln!("v3 CEILINGS {writes:?} gas: register-zero={reg_gas} (earn-in {earn_reg}) topUp-promote-with-return={top_gas} (earn-in {earn_top})");
        assert_eq!(writes,(11,3,5,2,4));assert_eq!(writes,earn_writes);
        // pocold's wallet quotes 400,000 gas for an earn-in/v3 registration and 200,000 for a top-up
        // (EARNIN_REGISTER_GAS, REGISTRY_GAS); the cfg() reads add only a few thousand gas.
        assert!(reg_gas<=400_000&&top_gas<=200_000,"register {reg_gas}, topUp {top_gas}");
        assert!(reg_gas>=earn_reg&&reg_gas-earn_reg<=5_000,"register gas {reg_gas} vs earn-in {earn_reg}");
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
