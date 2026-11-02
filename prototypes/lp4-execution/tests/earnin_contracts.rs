//! Earn-in MinerRegistry (system-earnin.json) replays every scenario of tests/data/earnin-vectors.json,
//! the independent model in tests/earnin_model.py (EARN-IN-SPEC.md). Contract-level EVM calls at
//! chosen heights; settlement through RewardEscrow is covered by the pocold devnet test.
use pocol_lp4_execution::{native,output,CHAIN_ID,pay::{registry_address,escrow_address}};
use revm::{DatabaseCommit,InMemoryDB,primitives::{Address,AccountInfo,Bytecode,Bytes,TxEnv,TxKind,U256,keccak256,ResultAndState}};
use std::collections::{BTreeMap,BTreeSet};

const PCOL:u128=1_000_000_000_000_000_000;
fn b()->U256{U256::from(20*PCOL)}
fn system()->Address{Address::from_word((U256::from(1)<<160u32).wrapping_sub(U256::from(2)).into())}
fn caps()->Address{Address::from_word(U256::from(0xc0c004u64).into())}
fn name(n:&str)->Address{Address::from_slice(&keccak256(format!("earnin:{n}"))[12..])}
fn program(n:&str)->Bytecode{
    let j:serde_json::Value=serde_json::from_str(include_str!("data/system-earnin.json")).unwrap();
    let p=j["programs"].as_array().unwrap().iter().find(|p|p["name"]==n).unwrap();
    let s=p["runtime"].as_str().unwrap().trim_start_matches("0x");
    Bytecode::new_raw(Bytes::from((0..s.len()).step_by(2).map(|i|u8::from_str_radix(&s[i..i+2],16).unwrap()).collect::<Vec<_>>()))
}
fn insert(db:&mut InMemoryDB,a:Address,balance:U256,nonce:u64,code:Bytecode){db.insert_account_info(a,AccountInfo::new(balance,nonce,code.hash_slow(),code));}
fn slot(id:Address)->U256{U256::from_be_bytes(keccak256([id.into_word().as_slice(),U256::from(7).to_be_bytes::<32>().as_slice()].concat()).0)}
fn base(n:u64)->U256{U256::from_be_bytes(keccak256(U256::from(n).to_be_bytes::<32>()).0)}
fn storage(db:&InMemoryDB,a:Address,k:U256)->U256{db.accounts.get(&a).and_then(|x|x.storage.get(&k).copied()).unwrap_or_default()}
fn put(db:&mut InMemoryDB,a:Address,k:U256,v:U256){db.insert_account_storage(a,k,v).unwrap();}
fn reg(db:&InMemoryDB,k:u64)->U256{storage(db,registry_address(),U256::from(k))}
fn selector(s:&str)->Vec<u8>{keccak256(s.as_bytes())[..4].to_vec()}
fn balance(db:&InMemoryDB,a:Address)->U256{db.accounts.get(&a).map(|x|x.info.balance).unwrap_or_default()}
fn call(db:&mut InMemoryDB,caller:Address,data:Vec<u8>,value:U256,h:u64)->ResultAndState{
    let tx=TxEnv{caller,transact_to:TxKind::Call(registry_address()),data:Bytes::from(data),value,gas_limit:30_000_000,gas_price:U256::ZERO,
        nonce:Some(db.accounts.get(&caller).map(|a|a.info.nonce).unwrap_or(0)),chain_id:Some(CHAIN_ID),..Default::default()};
    let r=native::execute_at(db,tx,U256::from(h)).unwrap();db.commit(r.state.clone());r
}
fn with_arg(sig:&str,arg:Option<Address>)->Vec<u8>{let mut d=selector(sig);if let Some(a)=arg{d.extend(a.into_word());}d}
fn ok(r:&ResultAndState,what:&str){assert!(r.result.is_success(),"{what}: {:?} {:?}",r.result,output(&r.result));}
fn bits(w:U256,lo:usize,len:usize)->u64{((w>>lo)&((U256::from(1)<<len)-U256::from(1))).to::<u64>()}

struct Harness{db:InMemoryDB,height:u64,senders:BTreeSet<Address>,settled:BTreeMap<Address,U256>,burned:U256,start:BTreeMap<Address,U256>}
impl Harness{
    fn new(durable:&[String])->Self{
        let mut db=InMemoryDB::default();
        insert(&mut db,system(),U256::ZERO,1,Bytecode::default());
        for (a,n) in [(registry_address(),"MinerRegistry"),(escrow_address(),"RewardEscrow"),(caps(),"ChainCaps")]{insert(&mut db,a,U256::ZERO,1,program(n));}
        for n in 0..52{put(&mut db,caps(),U256::from(n),U256::ZERO);}
        // CP of a continuous network (47/48 zero), with per-block limits wide enough for the vectors'
        // many same-block registrations; consensus values otherwise as Mainnet.
        for (n,v) in [(11,4u128),(16,20*PCOL),(17,16),(18,64),(19,1),(20,8192),(21,16),(22,64),(23,64),(24,32),(25,64),(26,16),(27,40),(50,64)]{put(&mut db,caps(),U256::from(n),U256::from(v));}
        // m0: a HELD genesis record with bond B whose bond Escrow holds (the model's first member).
        let m0=name("m0");insert(&mut db,m0,U256::ZERO,1,Bytecode::default());let r=slot(m0);
        for (k,v) in [(U256::ZERO,U256::from(1)),(U256::from(1),U256::from(1)),(U256::from(4),b()),(base(5),U256::from_be_slice(m0.as_slice())),(base(6),U256::from_be_slice(m0.as_slice())),
            (r,U256::from(1)|(U256::from(1)<<16u32)),(r+U256::from(1),U256::from_be_slice(m0.as_slice())),(r+U256::from(2),b())]{put(&mut db,registry_address(),k,v);}
        db.accounts.get_mut(&escrow_address()).unwrap().info.balance=b();
        for d in durable{insert(&mut db,name(d),U256::ZERO,1,Bytecode::default());}
        let start=db.accounts.iter().map(|(a,x)|(*a,x.info.balance)).collect();
        Harness{db,height:0,senders:BTreeSet::new(),settled:BTreeMap::new(),burned:U256::ZERO,start}
    }
    fn at(&mut self,h:u64){
        if h>self.height{for s in ["markClose()","beginBlock()"]{let r=call(&mut self.db,system(),selector(s),U256::ZERO,h);ok(&r,s);}self.height=h;}
    }
    /// A fresh sender: nonce 0, so a self reward exercises R9.
    fn sender(&mut self,n:&str)->Address{
        let a=name(n);if self.senders.insert(a)&&!self.db.accounts.contains_key(&a){insert(&mut self.db,a,U256::from(1000*PCOL),0,Bytecode::default());self.start.insert(a,U256::from(1000*PCOL));}a
    }
    fn record(&self,id:Address)->(U256,U256,U256,U256){let r=slot(id);(storage(&self.db,registry_address(),r),storage(&self.db,registry_address(),r+U256::from(1)),storage(&self.db,registry_address(),r+U256::from(2)),storage(&self.db,registry_address(),r+U256::from(3)))}
    fn step(&mut self,op:&[String])->Result<Option<String>,String>{
        let num=|i:usize|op[i].parse::<u128>().unwrap();
        let res=match op[0].as_str(){
            "register"=>{let s=self.sender(&op[1]);self.at(num(4) as u64);call(&mut self.db,s,with_arg("register(address)",Some(name(&op[2]))),U256::from(num(3)),self.height)}
            "top_up"=>{let s=self.sender(&op[1]);self.at(num(3) as u64);call(&mut self.db,s,selector("topUp()"),U256::from(num(2)),self.height)}
            "deregister"|"reactivate"=>{let s=self.sender(&op[1]);self.at(num(2) as u64);call(&mut self.db,s,selector(&format!("{}()",op[0])),U256::ZERO,self.height)}
            "sweep"=>{self.at(num(1) as u64);return Ok(None)}
            "punish"=>{let id=name(&op[1]);self.at(num(2) as u64);let r=call(&mut self.db,system(),with_arg("punish(address)",Some(id)),U256::ZERO,self.height);ok(&r,"punish");return Ok(None)}
            "settle_to"=>return Ok(Some(self.settle(name(&op[1]),U256::from(num(2))))),
            o=>panic!("unknown op {o}"),
        };
        if res.result.is_success(){Ok(None)}else{
            let out=output(&res.result);
            let known=["BadValue","BadState","Capacity","RewardNotDurable","PaymentFailed","BadCalldata"];
            Err(known.iter().find(|k|selector(&format!("{k}()"))==out).map(|k|k.to_string()).unwrap_or(format!("{out:?}")))
        }
    }
    /// Escrow payTo/distribute for one owner: an ACCRUING record goes through the real accrue();
    /// HELD is paid, absent/tomb burned (unchanged v1 behaviour, modelled here).
    fn settle(&mut self,id:Address,amount:U256)->String{
        let (w0,w1,_,_)=self.record(id);
        if w0.is_zero()||bits(w0,8,8)!=0{self.burned+=amount;return amount.to_string()}
        let reward=Address::from_word(w1.into());
        let excess=if bits(w0,16,8)==4{
            // The Registry admits only Escrow; an account with code cannot originate a transaction,
            // so the harness calls from Escrow's address with its code set aside for this one call.
            let code=self.db.accounts.get_mut(&escrow_address()).unwrap().info.clone();
            insert(&mut self.db,escrow_address(),code.balance,code.nonce,Bytecode::default());
            let mut d=selector("accrue(address,uint256)");d.extend(id.into_word());d.extend(amount.to_be_bytes::<32>());
            let r=call(&mut self.db,escrow_address(),d,U256::ZERO,self.height);
            let info=self.db.accounts.get_mut(&escrow_address()).unwrap();info.info.code=code.code.clone();info.info.code_hash=code.code_hash;info.info.nonce=code.nonce;
            ok(&r,"accrue");U256::from_be_slice(&output(&r.result)[..32])
        }else{amount};
        if !excess.is_zero(){*self.settled.entry(reward).or_default()+=excess;}
        "0".into()
    }
}

#[test]
fn earn_in_registry_reproduces_every_model_scenario(){
    let v:serde_json::Value=serde_json::from_str(include_str!("data/earnin-vectors.json")).unwrap();
    let mut checked=0;
    for (scenario,body) in v.as_object().unwrap(){
        let Some(steps)=body.get("steps") else{continue};
        let durable:Vec<String>=body["durable"].as_array().unwrap().iter().map(|x|x.as_str().unwrap().to_string()).collect();
        let mut h=Harness::new(&durable);
        for step in steps.as_array().unwrap(){
            let op:Vec<String>=step["op"].as_array().unwrap().iter().map(|x|x.as_str().unwrap().to_string()).collect();
            match (h.step(&op),step["ok"].as_bool().unwrap()){
                (Ok(_),true)=>{}
                (Err(e),false)=>assert_eq!(e,step["revert"].as_str().unwrap(),"{scenario} {op:?}"),
                (got,_)=>panic!("{scenario} {op:?}: contract {got:?}, model {step}"),
            }
        }
        let fin=&body["final"];
        assert_eq!(reg(&h.db,4).to_string(),fin["heldTotal"].as_str().unwrap(),"{scenario} heldTotal");
        assert_eq!(reg(&h.db,8).to::<u64>(),fin["accruing"].as_u64().unwrap(),"{scenario} accruing");
        let burned=storage(&h.db,escrow_address(),U256::ZERO)+h.burned;
        assert_eq!(burned.to_string(),fin["burned"].as_str().unwrap(),"{scenario} burned");
        let active:BTreeSet<Address>=(0..reg(&h.db,1).to::<u64>()).map(|i|Address::from_word(storage(&h.db,registry_address(),base(6)+U256::from(i)).into())).collect();
        let want:BTreeSet<Address>=fin["active"].as_array().unwrap().iter().map(|x|name(x.as_str().unwrap())).collect();
        assert_eq!(active,want,"{scenario} active set");
        let records=fin["records"].as_object().unwrap();
        assert_eq!(reg(&h.db,0).to::<usize>(),records.len(),"{scenario} record count");
        for (n,r) in records{
            let (w0,w1,w2,w3)=h.record(name(n));
            let got=(bits(w0,0,8),bits(w0,16,8),w2.to_string(),bits(w0,8,8)!=0,bits(w0,152,64),bits(w3,0,64),bits(w3,64,64),Address::from_word(w1.into()));
            let want=(r["status"].as_u64().unwrap(),r["bond_state"].as_u64().unwrap(),r["bond"].as_str().unwrap().to_string(),
                r["tomb"].as_bool().unwrap(),r["registered_at"].as_u64().unwrap(),r["exit"].as_u64().unwrap(),r["due"].as_u64().unwrap(),name(r["reward"].as_str().unwrap()));
            assert_eq!(got,want,"{scenario} record {n}");
        }
        for (n,amount) in fin["paid"].as_object().unwrap(){
            let a=name(n);let refunds=if h.senders.contains(&a){U256::ZERO}else{balance(&h.db,a)-h.start.get(&a).copied().unwrap_or_default()};
            let got=refunds+h.settled.get(&a).copied().unwrap_or_default();
            assert_eq!(got.to_string(),amount.as_str().unwrap(),"{scenario} paid {n}");
        }
        // Each sender's net PCOL into the Registry (bonds and top-ups, less returned excess and refunds).
        for a in &h.senders{
            let n=fin["records"].as_object().unwrap().keys().chain(fin["spent"].as_object().unwrap().keys()).find(|n|name(n)==*a).cloned();
            let Some(n)=n else{continue};
            let field=|k:&str|fin[k].get(&n).and_then(|x|x.as_str()).map(|x|x.parse::<U256>().unwrap()).unwrap_or_default();
            let refunds=field("paid")-h.settled.get(a).copied().unwrap_or_default();
            assert_eq!(h.start[a]-balance(&h.db,*a),field("spent")-refunds,"{scenario} sender {n} net PCOL");
        }
        checked+=1;
    }
    assert_eq!(checked,8,"every model scenario replayed");
}

fn writes(r:&ResultAndState)->usize{r.state.get(&registry_address()).map(|a|a.storage.values().filter(|s|s.is_changed()).count()).unwrap_or(0)}

#[test]
fn earn_in_write_ceilings(){
    let mut h=Harness::new(&[]);h.at(1);
    let a=h.sender("w-a");
    let r=call(&mut h.db,a,with_arg("register(address)",Some(a)),U256::from(5*PCOL),1);ok(&r,"register partial");
    let reg_partial=writes(&r);
    let r=call(&mut h.db,a,selector("topUp()"),U256::from(5*PCOL),1);ok(&r,"topUp");
    let top=writes(&r);
    let r=call(&mut h.db,a,selector("topUp()"),U256::from(12*PCOL),1);ok(&r,"topUp promote, 2 PCOL returned");
    let top_promote=writes(&r);let top_gas=r.result.gas_used();
    let b2=h.sender("w-b");
    let r=call(&mut h.db,b2,with_arg("register(address)",Some(b2)),U256::ZERO,1);ok(&r,"register zero");
    let reg_gas=r.result.gas_used();
    let acc=|h:&mut Harness,amount:u128|{
        let code=h.db.accounts[&escrow_address()].info.clone();
        insert(&mut h.db,escrow_address(),code.balance,code.nonce,Bytecode::default());
        let mut d=selector("accrue(address,uint256)");d.extend(b2.into_word());d.extend(U256::from(amount).to_be_bytes::<32>());
        let r=call(&mut h.db,escrow_address(),d,U256::ZERO,1);
        let info=h.db.accounts.get_mut(&escrow_address()).unwrap();info.info.code=code.code.clone();info.info.code_hash=code.code_hash;info.info.nonce=code.nonce;
        ok(&r,"accrue");writes(&r)
    };
    let accrue=acc(&mut h,PCOL);let accrue_promote=acc(&mut h,30*PCOL);
    eprintln!("CEILINGS register-partial={reg_partial} topUp={top} topUp-promote={top_promote} accrue={accrue} accrue-promote={accrue_promote} gas: register-zero={reg_gas} topUp-promote-with-return={top_gas}");
    assert_eq!((reg_partial,top,top_promote,accrue,accrue_promote),(11,3,5,2,4));
    // The wallet's fee reservation and the fee faucet amount are sized from these (deploy/faucet/README.md).
    assert!(reg_gas<=400_000&&top_gas<=200_000,"register {reg_gas}, topUp {top_gas}");
}
