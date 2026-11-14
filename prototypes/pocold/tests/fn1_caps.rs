//! FN-1 (C9, CAPS-SUSTAINABILITY N6): pocol_getCaps, the head branch's live counts against the chain caps, in process
//! and over the RPC socket, on a spec-1 genesis and a v3 one. Every count is recomputed here from the head
//! state, the canonical bodies and their receipts (the charged log bytes), not read from the node's incremental summary
//! or counters. And a publication of the shipped Website contract adds what the extension publisher's pre-check expects.
//! Private fixture chains.
use std::collections::HashMap;
use std::io::{Read,Write};
use pocold::{chain::{Chain,Spec},node::{Config,Limits,Node},types::*,hashes};
use pocol_lp4_execution::{apply,code_caps,devnet::{self,DevnetConfig},txsign::{Key,Tx}};
use revm::primitives::{Address,B256,U256};
use serde_json::{json,Value};

const CHAIN:u64=777951;
struct Net{chain:Chain,keys:HashMap<Address,Key>}
fn cfg(v3:bool)->DevnetConfig{
    let mut c=DevnetConfig::new(CHAIN,1_700_000_000,&["a","b","c"]);c.hash_rate=1024;c.d_att=73;c.tau=40;c.c=8;
    if v3{c.spec_version=2;c.nonce_mode=1;c.r_max=1;}c
}
fn spec(c:&DevnetConfig)->Spec{Spec::new(devnet::build(c).unwrap().admit(&pocold::single_host_local()).unwrap_or_else(|e|panic!("{e}")).0)}
fn net(c:&DevnetConfig)->Net{let d=devnet::build(c).unwrap();Net{chain:Chain::new(spec(c)),keys:d.miners.iter().map(|m|(m.id,Key::from_secret(&m.key.secret()).unwrap())).collect()}}
fn dir(tag:&str)->std::path::PathBuf{let d=std::env::temp_dir().join(format!("pocold-fn1-caps-{tag}-{}",std::process::id()));let _=std::fs::remove_dir_all(&d);d}
fn node(c:&DevnetConfig,tag:&str,rpc:Option<u16>)->(Node,std::path::PathBuf){
    let d=dir(tag);
    (Node::open(Config{dir:d.clone(),name:tag.into(),miner:None,listen:None,peers:vec![],rpc:rpc.map(|p|format!("127.0.0.1:{p}").parse().unwrap()),stop_at:None,log:false,limits:Limits::default()},spec(c)).unwrap(),d)
}
fn try_seal(n:&Net,mut h:pocold::header::Header,body:Body,attempt:u32)->Option<Block>{
    let p=n.chain.full_entry(&h.parent_hash).unwrap();let ctx=n.chain.child_ctx(&p,attempt).unwrap();
    let tid=template_id(&h.ut_raw);let mut shares=vec![];let mut x=0u64;
    loop{if (x as u128)>=ctx.assignment.nonce_max(){return None}let ph=pow_hash(&tid,x);if le_target(&ph,&ctx.t_share){if le_target(&ph,&ctx.target){break}shares.push(x);}x+=1;}
    let owner=addr(&ctx.assignment.owner(x).unwrap());h.nonce=x;shares.truncate(256);h.shares=shares;reseal(&mut h);
    h.winner_sig=n.keys[&owner].sign_hash(&B256::from(hashes::win_msg(&tid,x,&share_root(&h))));reseal(&mut h);Some(Block{header:h,body})
}
/// A signed block by Draw(h,0) on the head carrying `txs`.
fn signed(n:&Net,txs:Vec<Vec<u8>>)->Block{
    let p=n.chain.full_entry(&n.chain.head).unwrap();let ctx=n.chain.child_ctx(&p,0).unwrap();let body=Body{txs,evidence:vec![]};
    for off in 0..u64::from(n.chain.spec.cp().D_att){let (h,_,_)=n.chain.build_signed(&p,&n.keys[&ctx.proposer],ctx.window.lower()+off,body.clone(),vec![]).unwrap();
        if let Some(b)=try_seal(n,h,body.clone(),0){return b}}panic!("no signed solution")
}
fn import(n:&mut Net,b:&Block){let (e,_)=n.chain.validate(b,None,0).unwrap_or_else(|e|panic!("h={} {e:?}",b.header.h));n.chain.insert(e);}
fn tx(n:&Net,key:&Key,nonce:u64,to:Option<Address>,data:Vec<u8>,gas:u64)->Vec<u8>{tx_value(n,key,nonce,to,data,gas,u64::from(to.is_some()))}
fn tx_value(n:&Net,key:&Key,nonce:u64,to:Option<Address>,data:Vec<u8>,gas:u64,value:u64)->Vec<u8>{
    let p=n.chain.full_entry(&n.chain.head).unwrap();let base=n.chain.child_ctx(&p,0).unwrap().base_fee;let priority=U256::from(1_000_000_000u64);
    Tx{chain_id:CHAIN,nonce,max_priority:priority,max_fee:base*U256::from(2)+priority,gas,to,value:U256::from(value),data:data.into()}.sign_1559(key)
}
/// Init code that stores two nonzero slots, emits one LOG1 (topic 7, 32 data bytes: a charge of 36 + 32 + 32 = 100
/// log bytes, LOG-ACCT-01) and deploys a 32-byte runtime (all STOP).
fn deploy_code()->Vec<u8>{vec![0x60,0x01,0x60,0x00,0x55,0x60,0x02,0x60,0x01,0x55,0x60,0x07,0x60,0x20,0x60,0x00,0xa1,0x60,0x20,0x60,0x00,0xf3]}
const DEPLOY_LOG_BYTES:u128=100;
fn fresh(i:u8)->Address{Address::from([0xa0,i,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0,0x42])}
fn system(a:&Address)->bool{(0..5).any(|i|*a==apply::program_address(i))}
/// The charged log bytes (36 + 32 per topic + the data, LOG-ACCT-01) of every receipt of the canonical blocks above
/// genesis, read through eth_getTransactionReceipt for each body transaction's hash: not the node's counter.
fn charged_logs(n:&mut Node)->u128{
    let mut total=0u128;
    for h in 1..=n.chain.head().height{
        let e=n.chain.canonical_at(h).unwrap().hash;let b=n.body_of(&e).unwrap();
        for t in &b.txs{
            let r=rpc_call(n,"eth_getTransactionReceipt",json!([format!("{}",revm::primitives::keccak256(t))]));
            assert_eq!(r["blockNumber"],json!(format!("0x{h:x}")),"the receipt of a canonical transaction");
            for l in r["logs"].as_array().unwrap(){
                total+=36+32*l["topics"].as_array().unwrap().len() as u128+(l["data"].as_str().unwrap().len() as u128-2)/2;
            }
        }
    }
    total
}
fn rpc_call(n:&mut Node,method:&str,params:Value)->Value{
    let s=pocold::rpc::handle(n,json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}).to_string().as_bytes());
    let v:Value=serde_json::from_str(&s).unwrap();assert!(v.get("error").is_none(),"{method}: {v}");v["result"].clone()
}
/// The caps' counts of the head, recomputed: user accounts and their nonzero slots from the whole state, the logical code
/// inventory, the transaction counters summed over the canonical bodies and the log bytes charged in their receipts.
fn recount(n:&mut Node)->[u128;6]{
    let logs=charged_logs(n);
    let head=n.chain.head().hash;let v=n.chain.view_of(&head).unwrap();let v=v.v();
    let (mut accounts,mut slots)=(0u128,0u128);let mut user=Vec::new();
    v.each_account(&mut |a,acc|{if acc.exists()&&!system(a){accounts+=1;user.push(*a);}});
    for a in &user{v.each_slot(a,&mut |_,x|{if *x!=U256::ZERO{slots+=1}});}
    let code=u128::from(code_caps::inventory(v).unwrap());
    let (mut txs,mut bytes)=(0u128,0u128);
    for h in 1..=n.chain.head().height{let e=n.chain.canonical_at(h).unwrap();let b=n.body_of(&e.hash).unwrap();txs+=b.txs.len() as u128;bytes+=b.txs.iter().map(|t|t.len() as u128).sum::<u128>();}
    let cumulative=n.chain.spec.cp().H_END!=0;
    [accounts,slots,code,if cumulative{txs}else{0},if cumulative{bytes}else{0},if cumulative{logs}else{0}]
}
const NAMES:[(&str,&str);6]=[("accounts","accountsCap"),("storageSlots","storageSlotsCap"),("codeBytes","codeBytesCap"),("txCount","txCap"),("txBytes","txBytesCap"),("logBytes","logCap")];
fn q(v:&Value)->u128{u128::from_str_radix(v.as_str().unwrap_or_else(||panic!("a quantity: {v}")).trim_start_matches("0x"),16).unwrap()}
fn in_process(n:&mut Node)->Value{
    let s=pocold::rpc::handle(n,br#"{"jsonrpc":"2.0","id":7,"method":"pocol_getCaps","params":[]}"#);
    let v:Value=serde_json::from_str(&s).unwrap();assert!(v.get("error").is_none(),"{v}");assert!(s.len()<2048,"the answer is under 2 KiB: {} B",s.len());v["result"].clone()
}
/// Every field of an answer against the recount and the CP.
fn check(n:&mut Node,r:&Value){
    let counts=recount(n);let cp=n.chain.spec.cp();
    let caps=[u128::from(cp.ACCT_CAP),u128::from(cp.SLOT_CAP),u128::from(cp.CODE_CAP),u128::from(cp.TX_CAP),u128::from(cp.TXBYTES_CAP),u128::from(pocol_lp4_execution::user_phase::caps::LOG_CAP)];
    assert_eq!(q(&r["number"]),u128::from(n.chain.head().height));assert_eq!(r["hash"],json!(hex(&n.chain.head().hash)));
    assert_eq!(r["cumulative"],json!(cp.H_END!=0));assert_eq!(r["alarmPercent"],json!(80));
    assert_eq!(r["v3"],json!(n.chain.spec.v3_active(n.chain.head().height+1)));
    let mut any=false;
    for (i,(name,cap)) in NAMES.iter().enumerate(){
        assert_eq!(q(&r[name]),counts[i],"{name}");assert_eq!(q(&r[cap]),caps[i],"{cap}");
        let o=&r["occupancy"][name];let alarm=counts[i]*100>=caps[i]*80;any|=alarm;
        assert_eq!(o["alarm"],json!(alarm),"{name}");assert_eq!(q(&o["remaining"]),caps[i]-counts[i],"{name}");
        let pct=o["percent"].as_f64().unwrap();assert!((pct-(counts[i]*10_000/caps[i]) as f64/100.0).abs()<1e-9,"{name}: {pct}");
    }
    assert_eq!(r["alarm"],json!(any));
    // Item 16's room for a child of the head is what the answer leaves.
    let (tx_room,bytes_room)=n.chain.caps_room(n.chain.head());
    assert_eq!(q(&r["occupancy"]["txCount"]["remaining"]),u128::from(tx_room));assert_eq!(q(&r["occupancy"]["txBytes"]["remaining"]),u128::from(bytes_room));
}
/// The fixture's activity: two new accounts and a contract with two slots, 32 bytes of runtime and one log of 100 charged
/// bytes (block 1), an empty block (2) and two transfers to an existing account (3).
fn activity(h:&mut Net,n:&mut Node){
    let alice=devnet::account_key("alice");
    let b1=signed(h,vec![tx(h,&alice,0,Some(fresh(1)),vec![],21_000),tx(h,&alice,1,Some(fresh(2)),vec![],21_000),tx(h,&alice,2,None,deploy_code(),300_000)]);
    import(h,&b1);assert!(n.import(b1,None).unwrap());
    let b2=signed(h,vec![]);import(h,&b2);assert!(n.import(b2,None).unwrap());
    let b3=signed(h,vec![tx(h,&alice,3,Some(fresh(1)),vec![1,2,3],30_000),tx(h,&alice,4,Some(fresh(2)),vec![],21_000)]);
    import(h,&b3);assert!(n.import(b3,None).unwrap());
}
fn rpc_tcp(port:u16,body:&str)->Value{
    let t=std::time::Instant::now();
    loop{
        if let Ok(mut s)=std::net::TcpStream::connect(("127.0.0.1",port)){
            s.set_read_timeout(Some(std::time::Duration::from_secs(10))).unwrap();
            let _=s.write_all(format!("POST / HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",body.len()).as_bytes());
            let mut o=String::new();let _=s.read_to_string(&mut o);
            if let Some(v)=o.split("\r\n\r\n").nth(1).and_then(|b|serde_json::from_str::<Value>(b).ok()){return v}
        }
        assert!(t.elapsed()<std::time::Duration::from_secs(20),"the RPC socket did not answer");std::thread::sleep(std::time::Duration::from_millis(50));
    }
}
fn free_port()->u16{std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()}

fn counts_follow_the_head_branch(v3:bool){
    let c=cfg(v3);let mut h=net(&c);let (mut n,d)=node(&c,if v3{"v3"}else{"spec1"},None);
    let g=in_process(&mut n);check(&mut n,&g);
    assert_eq!(q(&g["txCount"]),0);assert_eq!(q(&g["txBytes"]),0);
    activity(&mut h,&mut n);
    let r=in_process(&mut n);check(&mut n,&r);
    assert_eq!(q(&r["accounts"])-q(&g["accounts"]),3,"two transfers' recipients and one contract");
    assert_eq!(q(&r["storageSlots"])-q(&g["storageSlots"]),2,"the contract's two slots");
    assert_eq!(q(&r["codeBytes"])-q(&g["codeBytes"]),32,"the contract's runtime");
    assert_eq!(q(&r["txCount"]),5);
    assert_eq!(charged_logs(&mut n),DEPLOY_LOG_BYTES,"block 1's log, recounted from its receipt");
    assert_eq!(q(&g["logBytes"]),0);assert_eq!(q(&r["logBytes"]),DEPLOY_LOG_BYTES,"H_END > 0: block 1's log still counts at block 3");
    assert_eq!(r["alarm"],json!(false),"the devnet caps are far from full");
    std::fs::remove_dir_all(d).unwrap();
}
#[test] fn pocol_get_caps_counts_the_head_branch_on_a_spec1_genesis(){counts_follow_the_head_branch(false)}
#[test] fn pocol_get_caps_counts_the_head_branch_on_a_v3_genesis(){counts_follow_the_head_branch(true)}

/// A reorg to a branch without the activity: the answer is the new head branch's, not a running total.
#[test] fn pocol_get_caps_follows_a_reorg(){
    let c=cfg(false);let mut h=net(&c);let (mut n,d)=node(&c,"reorg",None);let g=in_process(&mut n);
    let alice=devnet::account_key("alice");
    let a1=signed(&h,vec![tx(&h,&alice,0,Some(fresh(9)),vec![],21_000)]);
    // The other branch: three empty blocks from genesis (more work than the one block with the transfer).
    let mut other=net(&c);let mut theirs=Vec::new();for _ in 0..3{let b=signed(&other,vec![]);import(&mut other,&b);theirs.push(b);}
    import(&mut h,&a1);assert!(n.import(a1,None).unwrap());
    let r=in_process(&mut n);check(&mut n,&r);assert_eq!(q(&r["txCount"]),1);assert_eq!(q(&r["accounts"])-q(&g["accounts"]),1);
    for b in theirs{n.import(b,None).unwrap();}
    assert_eq!(n.chain.head().height,3,"the heavier empty branch is the head");
    let r=in_process(&mut n);check(&mut n,&r);assert_eq!(q(&r["txCount"]),0);assert_eq!(q(&r["accounts"]),q(&g["accounts"]));
    std::fs::remove_dir_all(d).unwrap();
}

/// The alarm at >= 80 % (exact): an account cap sized so that genesis is below it and the activity reaches it.
#[test] fn pocol_get_caps_raises_the_alarm_at_eighty_percent(){
    let probe={let c=cfg(false);let (mut n,d)=node(&c,"alarm-probe",None);let g=q(&in_process(&mut n)["accounts"]);std::fs::remove_dir_all(d).unwrap();g};
    let mut c=cfg(false);c.acct_cap=u64::try_from((probe+3)*100/80).unwrap();
    assert!(probe*100<u128::from(c.acct_cap)*80&&(probe+3)*100>=u128::from(c.acct_cap)*80,"the fixture straddles 80 %");
    let mut h=net(&c);let (mut n,d)=node(&c,"alarm",None);
    let g=in_process(&mut n);check(&mut n,&g);assert_eq!(g["occupancy"]["accounts"]["alarm"],json!(false));assert_eq!(g["alarm"],json!(false));
    activity(&mut h,&mut n);
    let r=in_process(&mut n);check(&mut n,&r);
    assert_eq!(r["occupancy"]["accounts"]["alarm"],json!(true));assert_eq!(r["alarm"],json!(true));
    assert_eq!(r["occupancy"]["storageSlots"]["alarm"],json!(false),"one cap's alarm does not raise another's");
    std::fs::remove_dir_all(d).unwrap();
}

#[test] fn occupancy_is_exact_at_the_boundaries(){
    use pocold::rpc::occupancy_of;
    assert_eq!(occupancy_of(0,10),(0.0,false));assert_eq!(occupancy_of(79,100),(79.0,false));assert_eq!(occupancy_of(80,100),(80.0,true));
    assert_eq!(occupancy_of(4,5),(80.0,true));assert_eq!(occupancy_of(3,4),(75.0,false));assert_eq!(occupancy_of(799_999,1_000_000),(79.99,false));
    assert_eq!(occupancy_of(1,3),(33.33,false));assert_eq!(occupancy_of(10,10),(100.0,true));assert_eq!(occupancy_of(0,0),(100.0,true),"a zero cap admits nothing");
    assert_eq!(occupancy_of(u128::from(u64::MAX),u128::from(u64::MAX)),(100.0,true));
}

/// The same answer over the socket (light lane, any Origin-free client), and the method stays in the light table.
#[test] fn pocol_get_caps_over_the_socket_equals_the_in_process_answer(){
    assert!(pocold::rpc::LIGHT_METHODS.contains(&"pocol_getCaps"));
    assert!(pocold::rpc::is_light(br#"{"jsonrpc":"2.0","id":1,"method":"pocol_getCaps","params":[]}"#));
    let c=cfg(false);let mut h=net(&c);let port=free_port();let (mut n,d)=node(&c,"socket",Some(port));
    activity(&mut h,&mut n);let want=in_process(&mut n);check(&mut n,&want);
    n.start_network().unwrap();std::thread::spawn(move||n.run());
    let got=rpc_tcp(port,r#"{"jsonrpc":"2.0","id":3,"method":"pocol_getCaps","params":[]}"#);
    assert_eq!(got["id"],json!(3));assert_eq!(got["result"],want,"socket and in-process answers agree");
    // A batch member is answered the same.
    let batch=rpc_tcp(port,r#"[{"jsonrpc":"2.0","id":4,"method":"pocol_getCaps","params":[]},{"jsonrpc":"2.0","id":5,"method":"eth_blockNumber","params":[]}]"#);
    assert_eq!(batch[0]["result"],want);assert_eq!(batch[1]["result"],json!("0x3"));
    let _=std::fs::remove_dir_all(d);
}

/// A continuous v3 network (H_END 0, the fresh network's shape): item 16 and LOG_CAP count each block from zero, so the
/// transaction and log counts are 0 and their room is the whole cap; the state caps still count the branch.
#[test] fn pocol_get_caps_on_a_continuous_v3_genesis_counts_state_only(){
    let mut c=cfg(true);c.h_end=0;let mut h=net(&c);let (mut n,d)=node(&c,"v3-continuous",None);let g=in_process(&mut n);
    activity(&mut h,&mut n);let r=in_process(&mut n);check(&mut n,&r);
    assert_eq!(r["cumulative"],json!(false));assert_eq!(q(&r["txCount"]),0);
    assert_eq!(charged_logs(&mut n),DEPLOY_LOG_BYTES,"block 1 charged its log");assert_eq!(q(&r["logBytes"]),0,"H_END = 0: each block counts its logs from zero");
    assert_eq!(q(&r["occupancy"]["txCount"]["remaining"]),u128::from(n.chain.spec.cp().TX_CAP));
    assert_eq!(q(&r["accounts"])-q(&g["accounts"]),3);assert_eq!(r["v3"],json!(true));
    std::fs::remove_dir_all(d).unwrap();
}

/// The extension publisher's pre-check (pocol-extension src/lib/publisher.js `publishNeeds`) against an executed
/// publication of the shipped Website contract (lp4-execution contracts/web/Website.sol, `site::website_initcode`):
/// a new site adds 4 + 3 + n user slots (the constructor's owner, publishers[sender] and publisherCount, the version
/// counter; the version's manifestHash, meta, chunk-array length and one slot per manifest chunk), an update 3 + n;
/// one account per stored chunk and the site; each chunk's 0x00-prefixed runtime and the site's runtime in code bytes;
/// no log. publish and setCurrent only rewrite slots that are already nonzero. If Website.sol's layout changes, this
/// test and publishNeeds change together.
#[test] fn a_website_publication_adds_what_the_publisher_pre_check_expects(){
    let c=cfg(false);let mut h=net(&c);let (mut n,d)=node(&c,"website",None);let mut p=Publisher{key:devnet::account_key("alice"),nonce:0};
    let delta=|a:&Value,b:&Value,name:&str|q(&b[name])-q(&a[name]);
    // A new site with a two-chunk manifest.
    let (a,b)=(vec![0x41u8;100],vec![0x42u8;50]);
    let (site,before,after)=p.publish(&mut h,&mut n,None,&[&a,&b]);
    let runtime=rpc_call(&mut n,"eth_getCode",json!([format!("{site:#x}"),"latest"]));let runtime=(runtime.as_str().unwrap().len() as u128-2)/2;
    assert!(runtime>0,"the site is deployed");
    assert_eq!(delta(&before,&after,"storageSlots"),4+3+2,"a new site: the constructor's 3 slots, the counter, the version's 3 and 2 chunk slots");
    assert_eq!(delta(&before,&after,"accounts"),2+1,"two chunk accounts and the site");
    assert_eq!(delta(&before,&after,"codeBytes"),(1+100)+(1+50)+runtime,"each chunk's 0x00-prefixed runtime and the site's");
    assert_eq!(delta(&before,&after,"logBytes"),0,"Website and ChunkFactory emit no logs");
    assert_eq!(delta(&before,&after,"txCount"),2+1+3,"the chunks, the deployment and the three version transactions");
    // An update of that site with a one-chunk manifest.
    let c1=vec![0x43u8;30];
    let (again,before,after)=p.publish(&mut h,&mut n,Some(site),&[&c1]);assert_eq!(again,site);
    assert_eq!(delta(&before,&after,"storageSlots"),3+1,"an update: the version's 3 and 1 chunk slot");
    assert_eq!(delta(&before,&after,"accounts"),1);assert_eq!(delta(&before,&after,"codeBytes"),1+30);assert_eq!(delta(&before,&after,"logBytes"),0);
    std::fs::remove_dir_all(d).unwrap();
}
struct Publisher{key:Key,nonce:u64}
impl Publisher{
    fn send(&mut self,h:&Net,to:Option<Address>,data:Vec<u8>,gas:u64)->Vec<u8>{let t=tx_value(h,&self.key,self.nonce,to,data,gas,0);self.nonce+=1;t}
    /// Stores the manifest chunks (and deploys the site when `site` is None) in one block, then createVersion, publish
    /// and setCurrent in the next, as the publisher sends them; every receipt succeeds. Returns the site and the caps
    /// answers before and after.
    fn publish(&mut self,h:&mut Net,n:&mut Node,site:Option<Address>,manifest:&[&[u8]])->(Address,Value,Value){
        use pocold::site::{abi_bytes_call,abi_create_version,abi_u256_call,chunk_address,factory,website_initcode};
        let before=in_process(n);check(n,&before);
        let mut txs:Vec<Vec<u8>>=manifest.iter().map(|m|self.send(h,Some(factory()),abi_bytes_call("store(bytes)",m),200_000+m.len() as u64*260)).collect();
        let site=match site{Some(s)=>s,None=>{let at=self.key.address().create(self.nonce);txs.push(self.send(h,None,website_initcode(),1_500_000));at}};
        let b=signed(h,txs.clone());import(h,&b);assert!(n.import(b,None).unwrap());
        let v=version_count(n,site);
        let chunks:Vec<Address>=manifest.iter().map(|m|chunk_address(m)).collect();let len=manifest.iter().map(|m|m.len()).sum::<usize>() as u32;
        let more=vec![self.send(h,Some(site),abi_create_version(B256::from([0x5a;32]),len,&chunks),600_000),
            self.send(h,Some(site),abi_u256_call("publish(uint256)",v),200_000),self.send(h,Some(site),abi_u256_call("setCurrent(uint256)",v),200_000)];
        let b=signed(h,more.clone());import(h,&b);assert!(n.import(b,None).unwrap());
        for t in txs.iter().chain(&more){let r=rpc_call(n,"eth_getTransactionReceipt",json!([format!("{}",revm::primitives::keccak256(t))]));assert_eq!(r["status"],json!("0x1"),"{r}");}
        assert_eq!(version_count(n,site),v+1,"the version was created");
        let after=in_process(n);check(n,&after);(site,before,after)
    }
}
fn version_count(n:&mut Node,site:Address)->u64{
    let out=rpc_call(n,"eth_call",json!([{"to":format!("{site:#x}"),"data":format!("0x{}",hex_bytes(&pocold::site::sel("versionCount()")))},"latest"]));
    let out=out.as_str().unwrap().trim_start_matches("0x");if out.is_empty(){return 0}u64::from_str_radix(out.trim_start_matches('0'),16).unwrap_or(0)
}
fn hex_bytes(b:&[u8])->String{b.iter().map(|x|format!("{x:02x}")).collect()}
