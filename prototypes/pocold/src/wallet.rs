//! Local wallet commands. No signing/control method is exposed to P2P or HTTP RPC.
//! A quote is immutable, expires, and is consumed exactly once by an explicit confirmation.
use crate::{launcher::Home,node::Node,types};
use pocol_lp4_execution::{apply,txsign::{Key,Tx},view::View};
use revm::primitives::{Address,Bytes,U256,keccak256};
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
    QuoteRegister,
    Confirm { quote_id:String },
    Cancel,
    Stop,
}
struct Quote { id:String, tx:Tx, deadline:Instant, kind:&'static str }
pub struct Wallet<'a> { home:&'a Home,key:Key,quote:Option<Quote>,activity:VecDeque<Value>,pub stopped:bool }

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
    pub fn new(home:&'a Home)->Result<Self,String>{
        let path=home.path.join("wallet-activity.json");
        let activity=if path.exists(){let bytes=crate::launcher::read_bounded(&path)?;let a:VecDeque<Value>=serde_json::from_slice(&bytes).map_err(|_|"Wallet activity is damaged; preserve it before recovery")?;if a.len()>100{return Err("Wallet activity exceeds limit".into())}a}else{VecDeque::new()};
        Ok(Self{home,key:home.key()?,quote:None,activity,stopped:false})
    }
    pub fn status(&self,n:&Node)->Value{
        let addr=self.key.address();let head=n.chain.head();let state=n.chain.view_of(&head.hash);
        let account=state.as_ref().and_then(|s|s.v().account(&addr));
        let balance=account.map(|a|a.info.balance).unwrap_or_default();
        let record=state.as_ref().and_then(|s|apply::miner_record(s.v(),addr));
        let eligible=record.as_ref().is_some_and(|r|r.status==1&&!r.tomb&&r.bond_state==1);
        let mut activity=self.activity.clone();
        for item in &mut activity{
            let parsed=item["hash"].as_str().and_then(|s|s.parse().ok());
            if let Some(hash)=parsed{
                item["state"]=json!(if n.pool.txs.iter().any(|(h,_)|*h==hash){"Pending"}else{"Unconfirmed"});
                if let Ok(locations)=n.history.tx_locations(&hash){
                    for (block,index) in locations{
                        if let Some(e)=n.chain.get(&block){if n.chain.canonical_at(e.height).is_some_and(|c|c.hash==block){
                            if let Some(full)=n.view(e){if let Some(raw)=full.receipts.get(index){
                                // EIP-2718 receipts: the first RLP field is the execution status.
                                let bytes=if raw.first().is_some_and(|b|*b<0x80){&raw[1..]}else{raw.as_slice()};
                                let success=crate::rlp::decode(bytes).ok().is_some_and(|r|r.list().and_then(|l|l.first()).and_then(|v|v.bytes())==Some(&[1]));
                                item["state"]=json!(if success{"Included"}else{"Reverted"});item["height"]=json!(e.height);item["confirmations"]=json!(head.height-e.height+1);
                            }}break;
                        }}
                    }
                }
            }
        }
        json!({"type":"status","address":format!("{addr:#x}"),"balance":coins(balance),"balanceWei":balance.to_string(),
            "chainId":n.chain.spec.chain_id(),"height":head.height,"peers":n.peer_count(),"headTime":head.ts,
            "miningEnabled":n.cfg.miner.is_some(),"miningEligible":eligible,"registered":record.is_some(),
            "miningState":if n.cfg.miner.is_some(){if eligible{"Enabled — waiting for assigned work"}else{"Enabled — awaiting eligible membership"}}else{"Off — wallet keeps validating"},
            "blocksMined":n.stats.mined,"bond":coins(crate::chain::from_fixed_lp4(&n.chain.spec.cp().B_reg)),
            "lifetime":n.status_json()["lifetime"],"activity":activity,"currency":"PCOL","testCurrency":true})
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
            Command::QuoteRegister=>{
                if self.status(n)["registered"].as_bool().unwrap_or(false){return Err("This identity already has a registry record".into())}
                let mut data=keccak256("register(address)")[..4].to_vec();data.extend_from_slice(self.key.address().into_word().as_slice());
                self.quote(n,"registration",apply::program_address(0),crate::chain::from_fixed_lp4(&n.chain.spec.cp().B_reg),data.into(),1_000_000)
            },
            Command::Confirm{quote_id}=>{
                let q=self.quote.take().ok_or("Review a transaction first")?;
                if q.id!=quote_id||Instant::now()>q.deadline{return Err("Review expired or changed. Review the transaction again.".into())}
                if n.pool_view().account(&self.key.address()).map(|a|a.info.nonce).unwrap_or(0)!=q.tx.nonce{return Err("Account nonce changed. Review again.".into())}
                let hash=n.submit_tx(q.tx.sign_1559(&self.key),true)?;
                self.activity.push_front(json!({"hash":format!("{hash:#x}"),"kind":q.kind,"to":q.tx.to.map(|a|format!("{a:#x}")),"amount":coins(q.tx.value),"state":"Pending","time":crate::node::now_s()}));
                self.activity.truncate(100);
                let file=self.home.path.join("wallet-activity.json");
                let bytes=serde_json::to_vec(&self.activity).map_err(|e|e.to_string())?;
                // Public history only. Failure does not pretend that an already relayed transaction was unsent.
                let saved=save_activity(&file,&bytes).is_ok();
                Ok(json!({"type":"submitted","hash":format!("{hash:#x}"),"historySaved":saved}))
            }
        }
    }
    fn quote(&mut self,n:&Node,kind:&'static str,to:Address,value:U256,data:Bytes,gas:u64)->Result<Value,String>{
        if n.peer_count()==0{return Err("Connect to the network before sending".into())}
        let head=n.chain.head();
        if head.ts.saturating_add(180)<crate::node::now_s(){return Err("The local chain is behind. Wait for synchronization.".into())}
        let pending=n.pool_view();let account=pending.account(&self.key.address());
        let nonce=account.as_ref().map(|a|a.info.nonce).unwrap_or(0);
        let balance=account.map(|a|a.info.balance).unwrap_or_default();
        let base=if head.height==0{crate::chain::from_fixed_lp4(&n.chain.spec.cp().baseFee0)}else{pocol_lp4_execution::fees::next_base_fee(head.base_fee,head.gas_used,head.header.gas_limit).map_err(|_|"Invalid base fee")?};
        let priority=U256::from(1_000_000_000u64);let fee=base.checked_mul(U256::from(2)).and_then(|b|b.checked_add(priority)).ok_or("Fee overflow")?;
        let cost=fee.checked_mul(U256::from(gas)).ok_or("Fee overflow")?;
        if value.checked_add(cost).ok_or("Total overflow")?>balance{return Err("Insufficient available balance for the amount and maximum fee".into())}
        let mut id=[0u8;16];getrandom::getrandom(&mut id).map_err(|_|"Secure randomness unavailable")?;let id=types::hex(&id);
        let result=json!({"type":"quote","quoteId":id,"kind":kind,"to":format!("{to:#x}"),"from":format!("{:#x}",self.key.address()),"amount":coins(value),"maximumFee":coins(cost),"maximumTotal":coins(value+cost),"chainId":n.chain.spec.chain_id(),"expiresInSeconds":60});
        self.quote=Some(Quote{id,deadline:Instant::now()+Duration::from_secs(60),kind,tx:Tx{chain_id:n.chain.spec.chain_id(),nonce,max_priority:priority,max_fee:fee,gas,to:Some(to),value,data}});Ok(result)
    }
}
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
    home.run_mode(None,None,false,|n|{
        match rx.try_recv(){Ok(command)=>emit(wallet.handle(n,&command)),Err(mpsc::TryRecvError::Disconnected)=>return true,Err(mpsc::TryRecvError::Empty)=>{}}
        if Instant::now()>=next{emit(wallet.status(n));next=Instant::now()+Duration::from_secs(2)}
        wallet.stopped||stop()
    })
}
#[cfg(test)] mod tests{
    use super::*;
    #[test] fn decimal_amounts_never_use_floats(){
        for s in ["1","0.000000000000000001","123456789.012345678901234567"]{assert_eq!(coins(amount(s).unwrap()),s)}
        for s in ["", "0", "-1", "NaN", "1e18", "1.0000000000000000001", ".1", "1..2", " 1"]{assert!(amount(s).is_err(),"{s}")}
    }
    #[test] fn command_surface_is_closed(){assert!(serde_json::from_str::<Command>(r#"{"method":"mining","enabled":true,"secret":"x"}"#).is_err());assert!(serde_json::from_str::<Command>(r#"{"method":"send","amount":1}"#).is_err());}
}
