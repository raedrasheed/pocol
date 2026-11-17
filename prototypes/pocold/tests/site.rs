//! LP8 T16 rejection vectors for the manifest and chunk verification.
use pocold::{site::{self,FileEntry,Manifest,decode_current},rlp,types::unhex};
use revm::primitives::{keccak256,Address,B256};

fn good()->Manifest{
    let f=|p:&str,mime:u8,data:&[u8]|FileEntry{path:p.into(),mime,size:data.len() as u32,content_hash:keccak256(data),chunks:vec![(site::chunk_address(data),data.len() as u32)]};
    Manifest{entry:1,files:vec![f("/app.js",3,b"x"),f("/index.html",1,b"<p>"),f("/style.css",2,b"b{}")]}
}
fn rejects(m:&Manifest,why:&str){let e=Manifest::decode(&m.encode()).unwrap_err();assert!(e.contains(why),"{why}: got {e}");}

#[test] fn canonical_manifest_round_trips(){let m=good();assert_eq!(Manifest::decode(&m.encode()).unwrap(),m);}
#[test] fn t16_rejection_vectors(){
    let mut m=good();m.files.swap(0,1);rejects(&m,"sorted");                              // unsorted
    let mut m=good();m.files[1]=m.files[0].clone();rejects(&m,"sorted");                    // duplicate
    let mut m=good();m.files[0].path="/./a.js".into();rejects(&m,"bad path");             // '.'
    let mut m=good();m.files[0].path="/a\u{e9}.js".into();rejects(&m,"bad path");          // non-ASCII
    let mut m=good();m.files[0].path="/a b.js".into();rejects(&m,"bad path");             // space
    let mut m=good();m.files[0].mime=11;rejects(&m,"unknown mime");                         // unknown mime
    let mut m=good();m.entry=0;rejects(&m,"entry is not html");                             // entry not html
    let mut m=good();m.files[0].size=2;rejects(&m,"size mismatch");                         // size != sum len
    let mut m=good();m.files[0].chunks[0].1=0;rejects(&m,"chunk len");                      // len 0
    let mut b=good().encode();b.push(0);assert!(Manifest::decode(&b).is_err(),"trailing byte");
    let mut b=good().encode();b[0]=0xf9;assert!(Manifest::decode(&b).is_err(),"noncanonical RLP");
    // version byte other than 1
    let m=good();let mut b=m.encode();let i=b.iter().position(|x|*x==0x01).unwrap();b[i]=0x02;assert!(Manifest::decode(&b).is_err());
}
#[test] fn chunk_code_checks(){
    let data=b"hello chunk";let a=site::chunk_address(data);
    let mut code=vec![0u8];code.extend(data);assert_eq!(site::chunk_payload(a,&code,data.len() as u32).unwrap(),data.to_vec());
    let mut bad=code.clone();bad[0]=0x60;assert!(site::chunk_payload(a,&bad,data.len() as u32).unwrap_err().contains("code[0]"));
    assert!(site::chunk_payload(a,&code,3).unwrap_err().contains("length"));
    assert!(site::chunk_payload(Address::repeat_byte(7),&code,data.len() as u32).unwrap_err().contains("ChunkFactory"),"other factory");
    let _=B256::ZERO;let _=rlp::encode_bytes;
}
#[test] fn path_normalization_d96(){
    assert_eq!(site::normalize(""),Some(None));assert_eq!(site::normalize("/"),Some(None));assert_eq!(site::normalize("/?q=1#x"),Some(None));
    assert_eq!(site::normalize("/docs/"),Some(Some("/docs/index.html".into())));assert_eq!(site::normalize("/a.css?v=2"),Some(Some("/a.css".into())));
    for bad in ["/%2e/x","//x","/./x","/../x","/a/../b"]{assert_eq!(site::normalize(bad),None,"{bad}");}
}

fn vectors(name:&str)->Vec<serde_json::Value>{
    let p=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../development/m1/m1-draft-0.2/vectors").join(name);
    let v:serde_json::Value=serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap();v["fixtures"].as_array().unwrap().clone()
}
#[test] fn manifest_vectors_positive_and_negative(){
    // M1 manifest vectors: every positive accepted (including pos-empty-file); every decode,
    // structure and semantic negative rejected (fetch/hash stages need chunk content).
    let mut acc=0;for f in vectors("manifest-positive.json"){let b=unhex(f["manifest"].as_str().unwrap()).unwrap();
        Manifest::decode(&b).unwrap_or_else(|e|panic!("{}: {e}",f["id"]));acc+=1;}
    let mut rej=0;for f in vectors("manifest-negative.json"){if !["decode","structure","semantic"].contains(&f["stage"].as_str().unwrap()){continue}
        let b=unhex(f["manifest"].as_str().unwrap()).unwrap();assert!(Manifest::decode(&b).is_err(),"{} must be rejected",f["id"]);rej+=1;}
    assert_eq!((acc,rej),(11,56));
}
#[test] fn hostile_current_output_never_panics(){
    let mut out=vec![0u8;32*7];out[4*32+31]=0xff;assert!(decode_current(&out).is_err());     // offset beyond the output
    let mut out=vec![0u8;32*7];for b in &mut out[4*32..5*32]{*b=0xff}assert!(decode_current(&out).is_err()); // huge offset
    let mut out=vec![0u8;32*7];out[4*32+31]=0xa0;out[5*32+31]=9;assert!(decode_current(&out).is_err());  // 9 chunks
    for n in 0..300{assert!(decode_current(&vec![0x41u8;n]).is_err()||n>=32*6);}
}

// ---------------- Sites W5: publish input (shared vectors with the extension, Studio and the S1 helper) ----------------
fn w5()->serde_json::Value{
    let p=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../pocol-extension/tests/data/publish-input-vectors.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}
fn strings(v:&serde_json::Value)->Vec<String>{v.as_array().unwrap().iter().map(|x|x.as_str().unwrap().to_string()).collect()}
fn pairs(v:&serde_json::Value)->Vec<(String,&'static str)>{v.as_array().unwrap().iter().map(|x|(x[0].as_str().unwrap().to_string(),*site::DENY_REASONS.iter().find(|r|**r==x[1].as_str().unwrap()).unwrap())).collect()}
#[test] fn w5_hard_deny_and_exclusion_follow_the_shared_vectors(){
    let v=w5();
    for d in v["deny"].as_array().unwrap(){assert_eq!(site::deny_reason(d[0].as_str().unwrap()),d[1].as_str(),"{}",d[0]);}
    for e in v["exclude"].as_array().unwrap(){assert_eq!(site::glob_match(e[0].as_str().unwrap(),e[1].as_str().unwrap()),e[2].as_bool().unwrap(),"{} {}",e[0],e[1]);}
    let sel=&v["select"];let s=site::select_paths(&strings(&sel["paths"]),&strings(&sel["exclude"]),None);
    assert_eq!(s.kept,strings(&sel["kept"]));assert_eq!(s.denied,pairs(&sel["denied"]));assert_eq!(s.excluded,strings(&sel["excluded"]));assert!(s.missing.is_empty());
    let inc=&v["include"];let list=site::parse_include_list(inc["text"].as_str().unwrap()).unwrap();let s=site::select_paths(&strings(&inc["paths"]),&[],Some(&list));
    assert_eq!(s.kept,strings(&inc["kept"]));assert_eq!(s.denied,pairs(&inc["denied"]));assert_eq!(s.excluded,strings(&inc["excluded"]));assert_eq!(s.missing,strings(&inc["missing"]));
    assert!(site::parse_include_list("# only a comment\n").is_err());assert!(site::parse_include_list("a\n/a").is_err());assert!(!site::glob_match("","/a"));
}
#[test] fn w5_mime_ids_12_and_13_are_added_and_11_stays_unknown(){
    let v=w5();
    for m in v["mime"].as_array().unwrap(){
        let id=site::mime_for_path(m[0].as_str().unwrap());assert_eq!(id.map(|x|x as u64),m[1].as_u64(),"{}",m[0]);
        if let Some(id)=id{assert_eq!(site::mime_name(id),m[2].as_str());}
    }
    for id in v["unknownMimeIds"].as_array().unwrap(){assert!(site::mime_name(id.as_u64().unwrap() as u8).is_none());}
    // A manifest carrying a favicon and a web manifest round-trips; the T16 unknown-MIME vector (11) is still refused.
    let f=|p:&str,mime:u8,data:&[u8]|FileEntry{path:p.into(),mime,size:data.len() as u32,content_hash:keccak256(data),chunks:vec![(site::chunk_address(data),data.len() as u32)]};
    let m=Manifest{entry:1,files:vec![f("/favicon.ico",12,&[0,0,1,0]),f("/index.html",1,b"<p>"),f("/site.webmanifest",13,b"{}")]};
    assert_eq!(Manifest::decode(&m.encode()).unwrap(),m);
    let mut bad=m.clone();bad.files[0].mime=11;rejects(&bad,"unknown mime");let mut bad=m;bad.files[2].mime=14;rejects(&bad,"unknown mime");
}

// ---------------- Sites W4: pocol-site fees (just in time, one budget, one ceiling) ----------------
#[test] fn w4_cli_fee_is_twice_the_base_plus_tip_and_the_ceiling_and_budget_refuse_before_signing(){
    use revm::primitives::U256;let g=U256::from(site::GWEI);
    let p=site::FeePolicy::default();assert_eq!((p.ceiling,p.tip,p.budget,p.economy),(U256::from(200)*g,g,None,false));
    assert_eq!(site::derive_fee(U256::from(10_000_000u64),&p,U256::ZERO,6_589_500).unwrap(),U256::from(1_020_000_000u64));
    assert!(site::derive_fee(U256::from(100)*g,&p,U256::ZERO,1).unwrap_err().contains("--max-fee-gwei"),"2*100 + 1 gwei is above 200");
    assert_eq!(site::derive_fee(U256::from(99)*g+U256::from(500_000_000u64),&p,U256::ZERO,1).unwrap(),U256::from(200)*g,"exactly the ceiling is allowed");
    let b=site::FeePolicy{budget:Some(U256::from(1_020_000_000u64*21_000)),..site::FeePolicy::default()};
    assert!(site::derive_fee(U256::from(10_000_000u64),&b,U256::ZERO,21_000).is_ok());
    assert!(site::derive_fee(U256::from(10_000_000u64),&b,U256::from(1),21_000).unwrap_err().contains("--max-total-pcol"));
    let c=site::FeePolicy::from_cli(Some("2.5"),Some("30"),Some("0.25"),true).unwrap();
    assert_eq!(c,site::FeePolicy{budget:Some(U256::from(2_500_000_000_000_000_000u128)),ceiling:U256::from(30)*g,tip:U256::from(250_000_000u64),economy:true});
    for (b,ce,t) in [(Some("0"),None,None),(None,Some("201"),None),(None,Some("0"),None),(None,None,Some("3")),(None,Some("1"),Some("2")),(Some("1.0000000000000000001"),None,None),(Some("-1"),None,None),(Some("x"),None,None),(Some("."),None,None)]{
        assert!(site::FeePolicy::from_cli(b,ce,t,false).is_err(),"{b:?} {ce:?} {t:?}");}
    assert_eq!(site::decimal_units(".5",9).unwrap(),U256::from(500_000_000u64));assert_eq!(site::decimal_units("7",0).unwrap(),U256::from(7));
}

/// A local JSON-RPC stand-in for one publish: every chunk already exists at `latest`, transactions are mined at once
/// (the deploy answers contractAddress 0x4242..), and `eth_getCode` at `safe` is refused, so the publish stops at the
/// safe sweep, after the Website deploy. Loopback only.
fn stand_in()->String{stand_in_with(|_|serde_json::json!({"error":{"code":-32000,"message":"safe state unavailable"}})).0}
/// The stand-in with `safe(n)` answering the n-th (from 0) `eth_getCode` at `safe`; it also records every method called.
fn stand_in_with(safe:impl Fn(usize)->serde_json::Value+Send+'static)->(String,std::sync::Arc<std::sync::Mutex<Vec<String>>>){
    use std::io::{Read,Write};
    let l=std::net::TcpListener::bind("127.0.0.1:0").unwrap();let addr=l.local_addr().unwrap().to_string();
    let seen=std::sync::Arc::new(std::sync::Mutex::new(Vec::<String>::new()));let log=seen.clone();let mut safe_reads=0;
    std::thread::spawn(move||for s in l.incoming(){
        let Ok(mut s)=s else{continue};let mut buf=Vec::new();let mut b=[0u8;4096];
        let body=loop{let n=s.read(&mut b).unwrap_or(0);if n==0{break None}buf.extend(&b[..n]);
            if let Some(i)=buf.windows(4).position(|w|w==b"\r\n\r\n"){
                let head=String::from_utf8_lossy(&buf[..i]).to_ascii_lowercase();
                let len:usize=head.lines().find_map(|x|x.strip_prefix("content-length:")).map(|v|v.trim().parse().unwrap()).unwrap_or(0);
                if buf.len()>=i+4+len{break Some(buf[i+4..i+4+len].to_vec())}}};
        let Some(body)=body else{continue};let req:serde_json::Value=serde_json::from_slice(&body).unwrap();
        let p=&req["params"];log.lock().unwrap().push(format!("{} {}",req["method"].as_str().unwrap(),p[1].as_str().unwrap_or("")));
        let out=match req["method"].as_str().unwrap(){
            "eth_getCode" if p[1]=="safe"=>{safe_reads+=1;safe(safe_reads-1)}
            "eth_getCode"=>serde_json::json!({"result":"0x0001"}),
            // Sites W12: the pre-check before the first signature; room for anything.
            "pocol_getCaps"=>caps_answer(&[],Some(false)),
            "eth_gasPrice"=>serde_json::json!({"result":"0x3c336080"}),
            "eth_maxPriorityFeePerGas"=>serde_json::json!({"result":"0x3b9aca00"}),
            "eth_getTransactionCount"=>serde_json::json!({"result":"0x0"}),
            "eth_sendRawTransaction"=>serde_json::json!({"result":format!("0x{}","11".repeat(32))}),
            "eth_getTransactionReceipt"=>serde_json::json!({"result":{"status":"0x1","blockNumber":"0x5","contractAddress":format!("0x{}","42".repeat(20))}}),
            m=>panic!("unexpected {m}"),
        };
        let text=serde_json::json!({"jsonrpc":"2.0","id":1}).as_object().unwrap().clone().into_iter().chain(out.as_object().unwrap().clone()).collect::<serde_json::Map<_,_>>();
        let text=serde_json::Value::Object(text).to_string();
        let _=write!(s,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",text.len());
    });
    (addr,seen)
}
#[test] fn w4_a_refusal_after_the_website_deploy_names_the_new_site_to_resume_with(){
    let rpc=site::Rpc{addr:stand_in()};let key=pocol_lp4_execution::txsign::Key::devnet("alice");
    let files:std::collections::BTreeMap<String,Vec<u8>>=[("/index.html".to_string(),b"<p>".to_vec())].into();
    let p=site::Publisher::new(&rpc,&key,7,false,site::FeePolicy::default());
    let e=p.publish(None,&files,"/index.html").unwrap_err();
    let new=format!("0x{}","42".repeat(20));
    assert!(e.contains("safe state unavailable")&&e.contains(&format!("run publish again with --site {new}")),"{e}");
    // With --site given there is nothing new to name.
    let given:Address=new.parse().unwrap();let e=p.publish(Some(given),&files,"/index.html").unwrap_err();
    assert!(e.contains("safe state unavailable")&&!e.contains("--site"),"{e}");
}
/// Sites W45 (CI browser (memory) on be7ce6b: the v3 publish after the partition heal failed with `eth_getCode:
/// {"code":-32017,...}`): a node that does not hold the state at its `safe` height answers -32017 there. The sweep reads
/// `safe` afresh on the next pass and counts such a chunk as not yet at `safe`, never as present; it never stores a
/// chunk that `latest` still holds, and a node that keeps answering -32017 ends the sweep with "run publish again",
/// before createVersion. Any other error still stops it at once (`w4_a_refusal_after_the_website_deploy...`).
#[test] fn w45_the_safe_sweep_reads_safe_again_when_the_node_does_not_hold_the_state_there(){
    let key=pocol_lp4_execution::txsign::Key::devnet("alice");let chunks:[&[u8];2]=[b"<p>",b"body{}"];
    let not_held=||serde_json::json!({"error":{"code":site::STATE_NOT_HELD,"message":"state outside the retained trie window (K_hist)"}});
    // The first three reads at `safe` (the whole first pass and one of the second) are not held; then both chunks are there.
    let (addr,seen)=stand_in_with(move|n|if n<3{not_held()}else{serde_json::json!({"result":"0x0001"})});
    let rpc=site::Rpc{addr};let mut p=site::Publisher::new(&rpc,&key,7,false,site::FeePolicy::default());p.sweep_poll=std::time::Duration::from_millis(10);
    p.safe_sweep(&chunks).unwrap();
    let seen=seen.lock().unwrap().clone();
    assert_eq!(seen.iter().filter(|m|*m=="eth_getCode safe").count(),6,"three passes of two chunks at safe: {seen:?}");
    assert_eq!(seen.iter().filter(|m|*m=="eth_getCode latest").count(),3,"each chunk not shown at safe is looked for at latest: {seen:?}");
    assert!(!seen.iter().any(|m|m.starts_with("eth_sendRawTransaction")),"a chunk latest holds is not stored again: {seen:?}");
    // A node that never holds the state at its safe height: bounded, refused with the reason, nothing signed.
    let (addr,seen)=stand_in_with(move|_|not_held());
    let rpc=site::Rpc{addr};let mut p=site::Publisher::new(&rpc,&key,7,false,site::FeePolicy::default());
    (p.sweep_poll,p.sweep_timeout)=(std::time::Duration::from_millis(10),std::time::Duration::from_millis(100));
    let e=p.safe_sweep(&chunks).unwrap_err();
    assert!(e.contains("2 chunks have not reached the safe height")&&e.contains("does not hold the state at its safe height")&&e.contains("run publish again"),"{e}");
    let seen=seen.lock().unwrap().clone();assert!(seen.iter().filter(|m|*m=="eth_getCode safe").count()>=4,"read again on every pass: {seen:?}");
    assert!(!seen.iter().any(|m|m.starts_with("eth_sendRawTransaction")),"{seen:?}");
    // Rpc::code_held: -32017 is None, any other error object is an error, code is Some.
    let (addr,_)=stand_in_with(|n|if n==0{serde_json::json!({"error":{"code":-32000,"message":"other"}})}else{serde_json::json!({"result":"0x"})});
    let rpc=site::Rpc{addr};let a=site::chunk_address(b"x");
    assert!(rpc.code_held(a,"safe").unwrap_err().contains("-32000"));assert_eq!(rpc.code_held(a,"safe").unwrap(),Some(vec![]));
}

/// Sites W10: a JSON-RPC stand-in answering each request with `answer(method, params, n)` (n: how many times that method
/// was called before); it records every call as "method tag". Loopback only.
type Calls=std::sync::Arc<std::sync::Mutex<Vec<String>>>;
fn stand_in_rpc(answer:impl Fn(&str,&serde_json::Value,usize)->serde_json::Value+Send+'static)->(String,Calls){
    use std::io::{Read,Write};
    let l=std::net::TcpListener::bind("127.0.0.1:0").unwrap();let addr=l.local_addr().unwrap().to_string();
    let seen:Calls=Default::default();let log=seen.clone();let mut counts=std::collections::HashMap::<String,usize>::new();
    std::thread::spawn(move||for s in l.incoming(){
        let Ok(mut s)=s else{continue};let mut buf=Vec::new();let mut b=[0u8;65536];
        let body=loop{let n=s.read(&mut b).unwrap_or(0);if n==0{break None}buf.extend(&b[..n]);
            if let Some(i)=buf.windows(4).position(|w|w==b"\r\n\r\n"){
                let head=String::from_utf8_lossy(&buf[..i]).to_ascii_lowercase();
                let len:usize=head.lines().find_map(|x|x.strip_prefix("content-length:")).map(|v|v.trim().parse().unwrap()).unwrap_or(0);
                if buf.len()>=i+4+len{break Some(buf[i+4..i+4+len].to_vec())}}};
        let Some(body)=body else{continue};let req:serde_json::Value=serde_json::from_slice(&body).unwrap();
        let m=req["method"].as_str().unwrap().to_string();let p=req["params"].clone();
        log.lock().unwrap().push(format!("{m} {}",p[1].as_str().unwrap_or("")));
        let n=*counts.entry(m.clone()).and_modify(|c|*c+=1).or_insert(0);
        let mut out=serde_json::json!({"jsonrpc":"2.0","id":1});
        for (k,v) in answer(&m,&p,n).as_object().unwrap(){out[k]=v.clone();}
        let text=out.to_string();
        let _=write!(s,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",text.len());
    });
    (addr,seen)
}
fn common(m:&str)->Option<serde_json::Value>{
    Some(match m{
        "eth_gasPrice"=>serde_json::json!({"result":"0x3c336080"}),"eth_maxPriorityFeePerGas"=>serde_json::json!({"result":"0x3b9aca00"}),
        "eth_getTransactionCount"=>serde_json::json!({"result":"0x0"}),"eth_blockNumber"=>serde_json::json!({"result":"0x5"}),
        "eth_sendRawTransaction"=>serde_json::json!({"result":format!("0x{}","11".repeat(32))}),_=>return None})
}
/// Sites W10: a chunk store that fails because a chain cap is reached is reported with the cap's name and counts from
/// pocol_getCaps (CODE_CAP: the code left is smaller than the chunk's code; ACCT_CAP: no account left); without
/// pocol_getCaps, or when neither cap explains it, the failed receipt is reported as before.
#[test] fn w10_a_chunk_store_refused_at_a_chain_cap_names_the_cap(){
    let key=pocol_lp4_execution::txsign::Key::devnet("alice");let data=vec![7u8;24_575];
    let run=|caps:serde_json::Value|{
        let (addr,_)=stand_in_rpc(move|m,_,_|common(m).unwrap_or_else(||match m{
            "eth_getCode"=>serde_json::json!({"result":"0x"}),
            "eth_getTransactionReceipt"=>serde_json::json!({"result":{"status":"0x0","blockNumber":"0x6","gasUsed":"0x631768"}}),
            "pocol_getCaps"=>caps.clone(),m=>panic!("unexpected {m}")}));
        let rpc=site::Rpc{addr};site::Publisher::new(&rpc,&key,7,false,site::FeePolicy::default()).store_chunk(&data).unwrap_err()
    };
    let caps=|code:u64,code_cap:u64,acct:u64,acct_cap:u64|serde_json::json!({"result":{"codeBytes":format!("0x{code:x}"),"codeBytesCap":format!("0x{code_cap:x}"),"accounts":format!("0x{acct:x}"),"accountsCap":format!("0x{acct_cap:x}")}});
    let e=run(caps(190_000,200_000,10,40));
    assert!(e.contains("code-size cap is reached (CODE_CAP): 190000 of 200000 code bytes")&&e.contains("creates 24576 more")&&e.contains("stored chunks are kept"),"{e}");
    let e=run(caps(21_525,200_000,40,40));assert!(e.contains("account cap is reached (ACCT_CAP): 40 of 40 accounts"),"{e}");
    let e=run(caps(21_525,200_000,10,40));assert!(e.starts_with("transaction failed")&&!e.contains("CAP"),"{e}");
    let e=run(serde_json::json!({"error":{"code":-32601,"message":"method not found"}}));assert!(e.starts_with("transaction failed"),"{e}");
    // The node's pool refuses a transaction whose pending state would pass a cap (AcctCap, seen on a small-cap devnet).
    let (addr,seen)=stand_in_rpc(move|m,_,_|match m{
        "eth_sendRawTransaction"=>serde_json::json!({"error":{"code":-32000,"message":"-32000 pending state would exceed a cap (AcctCap)"}}),
        "eth_getCode"=>serde_json::json!({"result":"0x"}),"pocol_getCaps"=>caps(101_931,2_097_152,8,8),m=>common(m).unwrap_or_else(||panic!("unexpected {m}"))});
    let rpc=site::Rpc{addr};let e=site::Publisher::new(&rpc,&key,7,false,site::FeePolicy::default()).store_chunk(b"<p>").unwrap_err();
    assert!(e.contains("account cap is reached (ACCT_CAP): 8 of 8 accounts")&&e.contains("would exceed a cap (AcctCap)"),"{e}");
    assert!(!seen.lock().unwrap().iter().any(|m|m.starts_with("eth_getTransactionReceipt")),"nothing was sent");
}
/// Sites W10 (the plan's reorg-before-safe case, with a mocked upstream; e2e_site.py runs it on a partitioned devnet):
/// a chunk another account stored is present at `latest`, so the publisher does not store it; then a reorganization
/// removes it before it reaches `safe`. The safe sweep finds it missing at both `safe` and `latest`, stores it again
/// (one signed store), and passes once it is at `safe`.
#[test] fn w10_a_chunk_seen_at_latest_and_reorged_out_before_safe_is_stored_again_by_the_sweep(){
    use std::sync::{Arc,atomic::{AtomicBool,Ordering}};
    let key=pocol_lp4_execution::txsign::Key::devnet("alice");let data=b"<p>stored by bob".to_vec();
    let reorged=Arc::new(AtomicBool::new(false));let restored=Arc::new(AtomicBool::new(false));
    let (r,s)=(reorged.clone(),restored.clone());
    let (addr,seen)=stand_in_rpc(move|m,p,_|common(m).unwrap_or_else(||match m{
        // Before the reorganization the chunk is at latest only; after it, nowhere until it is stored again.
        "eth_getCode"=>{let present=if r.load(Ordering::SeqCst){s.load(Ordering::SeqCst)}else{p[1]=="latest"};
            serde_json::json!({"result":if present{"0x003c703e73746f72656420627920626f62"}else{"0x"}})}
        "eth_getTransactionReceipt"=>{s.store(true,Ordering::SeqCst);serde_json::json!({"result":{"status":"0x1","blockNumber":"0x7"}})}
        m=>panic!("unexpected {m}")}));
    let rpc=site::Rpc{addr};let mut p=site::Publisher::new(&rpc,&key,7,false,site::FeePolicy::default());p.sweep_poll=std::time::Duration::from_millis(10);
    p.store_chunk(&data).unwrap();
    assert!(!seen.lock().unwrap().iter().any(|m|m.starts_with("eth_sendRawTransaction")),"present at latest: not stored");
    reorged.store(true,Ordering::SeqCst);
    p.safe_sweep(&[&data]).unwrap();
    let seen=seen.lock().unwrap().clone();
    assert_eq!(seen.iter().filter(|m|m.starts_with("eth_sendRawTransaction")).count(),1,"stored again exactly once: {seen:?}");
    assert!(seen.iter().rposition(|m|m=="eth_getCode safe")>seen.iter().position(|m|m.starts_with("eth_sendRawTransaction")),"the sweep reads safe again after storing: {seen:?}");
}

/// Sites W10 (the nightly 4 MiB run stopped at `eth_getTransactionReceipt: -32021 busy`): a read the node answers -32021
/// busy is asked again after retryAfterMs; eth_sendRawTransaction is sent once and its busy answer is returned.
#[test] fn w10_a_read_answered_busy_is_asked_again_and_a_broadcast_is_not(){
    let busy=||serde_json::json!({"error":{"code":site::BUSY,"message":"busy","data":{"reason":"busy","retryAfterMs":60}}});
    let (addr,seen)=stand_in_rpc(move|m,_,n|match m{
        "eth_getTransactionReceipt" if n<2=>busy(),
        "eth_getTransactionReceipt"=>serde_json::json!({"result":{"status":"0x1","blockNumber":"0x9"}}),
        "eth_sendRawTransaction"=>busy(),m=>panic!("unexpected {m}")});
    let rpc=site::Rpc{addr};
    let r=rpc.wait_receipt(revm::primitives::B256::ZERO,std::time::Duration::from_secs(10)).unwrap();assert_eq!(r["status"],"0x1");
    assert!(rpc.send(&[2u8]).unwrap_err().contains("-32021"));
    let seen=seen.lock().unwrap().clone();
    assert_eq!(seen.iter().filter(|m|m.starts_with("eth_getTransactionReceipt")).count(),3,"two busy answers, then the receipt: {seen:?}");
    assert_eq!(seen.iter().filter(|m|m.starts_with("eth_sendRawTransaction")).count(),1,"a broadcast is sent once: {seen:?}");
}

// ---------------- Sites W12: the pocol_getCaps pre-check, before any signature ----------------
type Files=std::collections::BTreeMap<String,Vec<u8>>;
/// tests/data/publish_needs.json, shared with the extension's publisher_test.mjs.
fn needs_fixture()->serde_json::Value{
    let p=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/publish_needs.json");
    serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()
}
fn fixture_files(c:&serde_json::Value)->Files{
    let bytes=|f:&serde_json::Value|match f["text"].as_str(){Some(t)=>t.as_bytes().to_vec(),None=>vec![f["fill"].as_u64().unwrap() as u8;f["size"].as_u64().unwrap() as usize]};
    let mut files:Files=c["files"].as_array().unwrap().iter().map(|f|(f["path"].as_str().unwrap().to_string(),bytes(f))).collect();
    if let Some(s)=c.get("series"){for i in 0..s["count"].as_u64().unwrap(){
        files.insert(format!("{}{i:03}{}",s["prefix"].as_str().unwrap(),s["suffix"].as_str().unwrap()),s["text"].as_str().unwrap().as_bytes().to_vec());}}
    files
}
/// The needs of publishing `files` when the chain holds the chunks in `stored`.
fn needs_of(files:&Files,new_site:bool,stored:&std::collections::HashSet<Address>)->site::Needs{
    let (_,mb)=site::prepare(files,"/index.html").unwrap();
    let missing:Vec<usize>=site::site_chunks(files,&mb).into_iter().filter(|(a,_)|!stored.contains(a)).map(|(_,c)|c.len()).collect();
    site::publish_needs(&missing,new_site,mb.chunks(site::CHUNK_MAX).count())
}
#[test] fn w12_publish_needs_follow_the_shared_fixture(){
    let v=needs_fixture();
    assert_eq!(v["txEnvelopeMax"].as_u64().unwrap() as u128,site::TX_ENVELOPE_MAX);
    assert_eq!(v["websiteRuntimeBytes"].as_u64().unwrap() as u128,site::website_runtime_len(),"the runtime deploy_website creates");
    assert_eq!(v["websiteInitcodeBytes"].as_u64().unwrap() as usize,site::website_initcode().len(),"the initcode deploy_website sends");
    let cases=v["cases"].as_array().unwrap();assert_eq!(cases.len(),7);
    for c in cases{
        let name=c["name"].as_str().unwrap();let files=fixture_files(c);
        let (_,mb)=site::prepare(&files,c["entry"].as_str().unwrap()).unwrap();
        assert_eq!(mb.len() as u64,c["manifestBytes"].as_u64().unwrap(),"{name}: manifest length");
        assert_eq!(format!("{:#x}",keccak256(&mb)),c["manifestHash"].as_str().unwrap(),"{name}: the manifest prepareSite builds");
        let chunks=site::site_chunks(&files,&mb);assert_eq!(chunks.len() as u64,c["chunks"].as_u64().unwrap(),"{name}: distinct chunks");
        let stored:std::collections::HashSet<Address>=if c["stored"]=="all"{chunks.iter().map(|x|x.0).collect()}else{
            strings(&c["stored"]).iter().flat_map(|p|files[p].chunks(site::CHUNK_MAX).map(site::chunk_address)).collect()};
        let missing:Vec<usize>=chunks.iter().filter(|(a,_)|!stored.contains(a)).map(|(_,d)|d.len()).collect();
        assert_eq!(missing.len() as u64,c["missingChunks"].as_u64().unwrap(),"{name}");
        let m=mb.chunks(site::CHUNK_MAX).count();assert_eq!(m as u64,c["manifestChunks"].as_u64().unwrap(),"{name}");
        let n=site::publish_needs(&missing,c["newSite"].as_bool().unwrap(),m);let w=&c["needs"];let f=|k:&str|w[k].as_u64().unwrap() as u128;
        assert_eq!(n,site::Needs{accounts:f("accounts"),storage_slots:f("storageSlots"),code_bytes:f("codeBytes"),tx_count:f("txCount"),tx_bytes:f("txBytes"),
            log_bytes:f("logBytes"),largest_tx_bytes:f("largestTxBytes")},"{name}");
    }
}
/// A pocol_getCaps answer: every row (count, cap) is (0, 2^40) unless given; `cumulative` as given.
fn caps_answer(rows:&[(&str,u128,u128)],cumulative:Option<bool>)->serde_json::Value{
    let mut r=serde_json::json!({"number":"0x5","v3":true,"alarmPercent":80,"occupancy":{}});
    for (c,k) in [("accounts","accountsCap"),("storageSlots","storageSlotsCap"),("codeBytes","codeBytesCap"),("txCount","txCap"),("txBytes","txBytesCap"),("logBytes","logCap")]{
        let (n,cap)=rows.iter().find(|x|x.0==c).map_or((0,1u128<<40),|x|(x.1,x.2));r[c]=format!("0x{n:x}").into();r[k]=format!("0x{cap:x}").into();}
    if let Some(b)=cumulative{r["cumulative"]=b.into();}
    serde_json::json!({"result":r})
}
/// Each row's room exactly the need: count `base`, cap `base + need`.
fn exact(n:&site::Needs,base:u128)->Vec<(&'static str,u128,u128)>{
    vec![("accounts",base,base+n.accounts),("storageSlots",base,base+n.storage_slots),("codeBytes",base,base+n.code_bytes),("txCount",base,base+n.tx_count),
        ("txBytes",base,base+n.tx_bytes),("logBytes",base,base+n.log_bytes)]
}
/// A stand-in node for a whole publication: chunks in `stored` hold code from the start, a chunk store gives its chunk
/// code at once, the deploy answers contractAddress 0x4242.., `safe` holds everything, and createVersion makes version
/// `n` (from 0) hold the manifest of `files`. `caps` answers pocol_getCaps. Also returns the latest-tag eth_getCode
/// reads per address. Loopback only.
type Reads=std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String,usize>>>;
fn publish_stand_in(files:&Files,caps:serde_json::Value,stored:&std::collections::HashSet<Address>)->(String,Calls,Reads){
    publish_stand_in_with_versions(files,caps,stored,0)
}
/// `publish_stand_in` for a Website that already holds `versions` versions.
fn publish_stand_in_with_versions(files:&Files,caps:serde_json::Value,stored:&std::collections::HashSet<Address>,versions:u64)->(String,Calls,Reads){
    use std::sync::{Arc,Mutex};
    let (_,mb)=site::prepare(files,"/index.html").unwrap();let (mhash,mlen)=(keccak256(&mb),mb.len());
    let have:Arc<Mutex<std::collections::HashSet<String>>>=Arc::new(Mutex::new(stored.iter().map(|a|format!("{a:#x}")).collect()));
    let versions=Arc::new(Mutex::new(versions));let reads:Reads=Default::default();let r2=reads.clone();
    let word=|v:u64|format!("{v:064x}");
    let (addr,seen)=stand_in_rpc(move|m,p,_|match m{
        "pocol_getCaps"=>caps.clone(),
        "eth_getCode"=>{let a=p[0].as_str().unwrap().to_ascii_lowercase();
            if p[1]=="safe"{return serde_json::json!({"result":"0x0001"})}
            *r2.lock().unwrap().entry(a.clone()).or_default()+=1;
            serde_json::json!({"result":if have.lock().unwrap().contains(&a){"0x0001"}else{"0x"}})}
        "eth_sendRawTransaction"=>{
            let raw=unhex(p[0].as_str().unwrap()).unwrap();let it=rlp::decode(&raw[1..]).unwrap();let f=it.list().unwrap();
            let (to,data)=(f[5].bytes().unwrap().to_vec(),f[7].bytes().unwrap().to_vec());
            if to==site::factory().as_slice(){let n=revm::primitives::U256::from_be_slice(&data[36..68]).to::<usize>();
                have.lock().unwrap().insert(format!("{:#x}",site::chunk_address(&data[68..68+n])));}
            if data.starts_with(&site::sel("createVersion(bytes32,uint32,address[])")){*versions.lock().unwrap()+=1;}
            common(m).unwrap()}
        "eth_getTransactionReceipt"=>serde_json::json!({"result":{"status":"0x1","blockNumber":"0x5","contractAddress":format!("0x{}","42".repeat(20))}}),
        "eth_call"=>{let d=p[0]["data"].as_str().unwrap().to_string();
            let out=if d.starts_with(&hex(&site::sel("versionCount()"))){word(*versions.lock().unwrap())}
                else{format!("{}{}{}{}{}{}",hex(mhash.as_slice()).trim_start_matches("0x"),word(mlen as u64),word(0),word(0),word(0xa0),word(0))};
            serde_json::json!({"result":format!("0x{out}")})}
        m=>common(m).unwrap_or_else(||panic!("unexpected {m}"))});
    (addr,seen,reads)
}
fn hex(b:&[u8])->String{format!("0x{}",b.iter().map(|x|format!("{x:02x}")).collect::<String>())}
fn sample_files()->Files{
    [("/index.html".to_string(),b"<!doctype html><h1>W12</h1>".to_vec()),("/style.css".to_string(),b"body{color:red}".to_vec()),("/big.txt".to_string(),vec![b'w';30_000])].into()
}
fn sends(seen:&Calls)->usize{seen.lock().unwrap().iter().filter(|m|m.starts_with("eth_sendRawTransaction")).count()}
fn caps_reads(seen:&Calls)->usize{seen.lock().unwrap().iter().filter(|m|m.starts_with("pocol_getCaps")).count()}
fn publisher<'a>(rpc:&'a site::Rpc,key:&'a pocol_lp4_execution::txsign::Key)->site::Publisher<'a>{
    let mut p=site::Publisher::new(rpc,key,7,false,site::FeePolicy::default());p.sweep_poll=std::time::Duration::from_millis(10);p
}
#[test] fn w12_a_site_short_of_code_bytes_or_accounts_by_one_is_refused_before_any_signature(){
    let key=pocol_lp4_execution::txsign::Key::devnet("alice");let files=sample_files();let none=Default::default();
    let n=needs_of(&files,true,&none);assert_eq!((n.accounts,n.tx_count),(6,9),"four file chunks, the manifest and the Website");
    for (rows,cap) in [(vec![("codeBytes",1000,1000+n.code_bytes-1)],"code bytes (CODE_CAP)"),(vec![("accounts",5,5+n.accounts-1)],"accounts (ACCT_CAP)")]{
        let (addr,seen,_)=publish_stand_in(&files,caps_answer(&rows,Some(false)),&none);let rpc=site::Rpc{addr};
        let e=publisher(&rpc,&key).publish(None,&files,"/index.html").unwrap_err();
        let need=if cap.starts_with("code"){n.code_bytes}else{n.accounts};
        assert!(e.contains("cannot fit")&&e.contains(&format!("it needs {need} {cap} and {} remain",need-1))&&e.contains("Nothing was signed"),"{e}");
        let seen=seen.lock().unwrap().clone();
        assert!(!seen.iter().any(|m|m.starts_with("eth_sendRawTransaction")||m.starts_with("eth_getTransactionCount")||m.starts_with("eth_gasPrice")),"nothing was signed: {seen:?}");
    }
    // Both short: each is named.
    let (addr,seen,_)=publish_stand_in(&files,caps_answer(&[("codeBytes",0,n.code_bytes-1),("accounts",0,n.accounts-1)],Some(false)),&none);let rpc=site::Rpc{addr};
    let e=publisher(&rpc,&key).publish(None,&files,"/index.html").unwrap_err();
    assert!(e.contains("accounts (ACCT_CAP)")&&e.contains("code bytes (CODE_CAP)")&&e.matches("it needs").count()==2,"{e}");assert_eq!(sends(&seen),0);
}
#[test] fn w12_a_site_that_exactly_fits_every_cap_is_published(){
    let key=pocol_lp4_execution::txsign::Key::devnet("alice");let files=sample_files();let none=Default::default();let n=needs_of(&files,true,&none);
    let (addr,seen,reads)=publish_stand_in(&files,caps_answer(&exact(&n,7),Some(true)),&none);let rpc=site::Rpc{addr};
    let (s,v,_)=publisher(&rpc,&key).publish(None,&files,"/index.html").unwrap();
    assert_eq!((format!("{s:#x}"),v),(format!("0x{}","42".repeat(20)),0));
    assert_eq!(sends(&seen) as u128,n.tx_count,"every planned transaction");assert_eq!(caps_reads(&seen),1);
    // Each missing chunk: the scan, store_chunk's read before and after its store.
    assert!(reads.lock().unwrap().values().all(|r|*r==3),"{:?}",reads.lock().unwrap());
}
#[test] fn w12_chunks_already_stored_are_read_once_and_not_counted(){
    let key=pocol_lp4_execution::txsign::Key::devnet("alice");let files=sample_files();
    let stored:std::collections::HashSet<Address>=files.values().flat_map(|d|d.chunks(site::CHUNK_MAX).map(site::chunk_address)).collect();
    let n=needs_of(&files,true,&stored);assert_eq!((n.accounts,n.tx_count),(2,5),"the manifest chunk and the Website");
    assert!(needs_of(&files,true,&Default::default()).accounts>2);
    let (addr,seen,reads)=publish_stand_in(&files,caps_answer(&[("accounts",9,9+2)],Some(false)),&stored);let rpc=site::Rpc{addr};
    publisher(&rpc,&key).publish(None,&files,"/index.html").unwrap();
    assert_eq!((sends(&seen),caps_reads(&seen)),(5,1),"checked; the manifest chunk, the deploy and the three version transactions");
    let reads=reads.lock().unwrap();for a in &stored{assert_eq!(reads[&format!("{a:#x}")],1,"a stored chunk is read once, by the scan");}
    // One account less than the two new ones: refused, so the stored chunks are left out of a check that runs.
    let (addr,seen,_)=publish_stand_in(&files,caps_answer(&[("accounts",9,9+1)],Some(false)),&stored);let rpc=site::Rpc{addr};
    let e=publisher(&rpc,&key).publish(None,&files,"/index.html").unwrap_err();assert!(e.contains("it needs 2 accounts (ACCT_CAP) and 1 remain"),"{e}");assert_eq!(sends(&seen),0);
}
#[test] fn w12_with_site_the_check_runs_without_a_website_and_is_skipped_only_when_every_chunk_is_stored(){
    let key=pocol_lp4_execution::txsign::Key::devnet("alice");let site_addr:Address=format!("0x{}","42".repeat(20)).parse().unwrap();
    let files:Files=[("/index.html".to_string(),b"<!doctype html><h1>v2</h1>".to_vec()),("/style.css".to_string(),b"body{color:red}".to_vec())].into();
    let old:std::collections::HashSet<Address>=[site::chunk_address(b"body{color:red}")].into();
    let n=needs_of(&files,false,&old);assert_eq!((n.accounts,n.storage_slots,n.tx_count),(2,4,5),"index.html and the manifest; no Website");
    // One account short: refused, nothing signed.
    let (addr,seen,_)=publish_stand_in(&files,caps_answer(&[("accounts",3,3+1)],Some(false)),&old);let rpc=site::Rpc{addr};
    let e=publisher(&rpc,&key).publish(Some(site_addr),&files,"/index.html").unwrap_err();
    assert!(e.contains("it needs 2 accounts (ACCT_CAP) and 1 remain")&&e.contains("Nothing was signed"),"{e}");assert_eq!(sends(&seen),0);
    // Exactly enough: published.
    let (addr,seen,_)=publish_stand_in(&files,caps_answer(&[("accounts",3,3+2)],Some(false)),&old);let rpc=site::Rpc{addr};
    publisher(&rpc,&key).publish(Some(site_addr),&files,"/index.html").unwrap();assert_eq!((sends(&seen),caps_reads(&seen)),(5,1));
    // Every chunk stored (a resumed publication, or the same files again): only the version transactions, no check.
    let (_,mb)=site::prepare(&files,"/index.html").unwrap();let all:std::collections::HashSet<Address>=site::site_chunks(&files,&mb).into_iter().map(|x|x.0).collect();
    let (addr,seen,_)=publish_stand_in(&files,caps_answer(&[("accounts",3,3)],Some(false)),&all);let rpc=site::Rpc{addr};
    publisher(&rpc,&key).publish(Some(site_addr),&files,"/index.html").unwrap();assert_eq!((sends(&seen),caps_reads(&seen)),(3,0));
    // Without --site every chunk stored still deploys a Website: checked, and refused at a full ACCT_CAP.
    let (addr,seen,_)=publish_stand_in(&files,caps_answer(&[("accounts",3,3)],Some(false)),&all);let rpc=site::Rpc{addr};
    let e=publisher(&rpc,&key).publish(None,&files,"/index.html").unwrap_err();assert!(e.contains("it needs 1 accounts (ACCT_CAP) and 0 remain"),"{e}");assert_eq!(sends(&seen),0);
    // --no-caps-check: no scan and no pocol_getCaps (the cap is met, and explained, at the network).
    let (addr,seen,_)=publish_stand_in(&files,caps_answer(&[("accounts",3,3)],Some(false)),&old);let rpc=site::Rpc{addr};
    let mut p=publisher(&rpc,&key);p.caps_check=false;p.publish(Some(site_addr),&files,"/index.html").unwrap();assert_eq!((sends(&seen),caps_reads(&seen)),(5,0));
}
#[test] fn w12_with_site_a_website_without_a_version_counts_its_version_counter_slot(){
    // A Website deployed by a publication that stopped before createVersion: its version counter slot is still zero,
    // and the first createVersion makes it nonzero, one slot more than an update of a site that has a version.
    let key=pocol_lp4_execution::txsign::Key::devnet("alice");let site_addr:Address=format!("0x{}","42".repeat(20)).parse().unwrap();
    let files:Files=[("/index.html".to_string(),b"<!doctype html><h1>resumed</h1>".to_vec())].into();let none=Default::default();
    let n=needs_of(&files,false,&none);assert_eq!(n.storage_slots,3+1,"the version's 3 and one manifest chunk slot");
    assert_eq!(site::publish_needs_on(&[1,2],1,revm::primitives::U256::ZERO).storage_slots,3+1+1);
    assert_eq!(site::publish_needs_on(&[1,2],1,revm::primitives::U256::from(1)),site::publish_needs(&[1,2],false,1));
    // SLOT_CAP room 3 + m: enough for an update of a site with a version, one short for a site without one.
    let rows=[("storageSlots",50,50+n.storage_slots)];
    let (addr,seen,_)=publish_stand_in_with_versions(&files,caps_answer(&rows,Some(false)),&none,0);let rpc=site::Rpc{addr};
    let e=publisher(&rpc,&key).publish(Some(site_addr),&files,"/index.html").unwrap_err();
    assert!(e.contains(&format!("it needs {} storage slots (SLOT_CAP) and {} remain",n.storage_slots+1,n.storage_slots))&&e.contains("Nothing was signed"),"{e}");
    let seen=seen.lock().unwrap().clone();
    assert!(!seen.iter().any(|m|m.starts_with("eth_sendRawTransaction")||m.starts_with("eth_getTransactionCount")||m.starts_with("eth_gasPrice")),"nothing was signed: {seen:?}");
    let (addr,seen,_)=publish_stand_in_with_versions(&files,caps_answer(&rows,Some(false)),&none,1);let rpc=site::Rpc{addr};
    let (_,v,_)=publisher(&rpc,&key).publish(Some(site_addr),&files,"/index.html").unwrap();assert_eq!((v,sends(&seen)),(1,5),"a site with a version: fits");
    // Room 4 + m: the site without a version fits.
    let (addr,seen,_)=publish_stand_in_with_versions(&files,caps_answer(&[("storageSlots",50,50+n.storage_slots+1)],Some(false)),&none,0);let rpc=site::Rpc{addr};
    let (_,v,_)=publisher(&rpc,&key).publish(Some(site_addr),&files,"/index.html").unwrap();assert_eq!((v,sends(&seen)),(0,5));
}
#[test] fn w12_a_node_without_pocol_get_caps_is_published_after_a_warning(){
    let key=pocol_lp4_execution::txsign::Key::devnet("alice");let files=sample_files();let none=Default::default();
    let (addr,seen,_)=publish_stand_in(&files,serde_json::json!({"error":{"code":-32601,"message":"method not found"}}),&none);let rpc=site::Rpc{addr};
    assert_eq!(site::caps_precheck(&rpc,&site::Needs::default()),Ok(false));
    publisher(&rpc,&key).publish(None,&files,"/index.html").unwrap();assert_eq!((sends(&seen),caps_reads(&seen)),(9,2),"asked (also by caps_precheck above), then published");
    // An answer that does not report every count and cap is skipped the same way.
    let mut c=caps_answer(&[],Some(false));c["result"].as_object_mut().unwrap().remove("txBytesCap");
    assert_eq!(site::check_caps(&c["result"],&site::Needs{accounts:u128::MAX,..Default::default()}),Ok(None));
}
#[test] fn w12_a_continuous_chain_checks_its_per_block_rows_per_transaction(){
    let files=sample_files();let n=needs_of(&files,true,&Default::default());assert!(n.largest_tx_bytes<n.tx_bytes&&n.tx_count>1);
    let rows=|tx_cap:u128,bytes_cap:u128|[("txCount",0,tx_cap),("txBytes",0,bytes_cap),("logBytes",0,0)];
    // cumulative false (H_END = 0): a block's room holds one transaction, the largest one's bytes, and its logs (none).
    assert_eq!(site::check_caps(&caps_answer(&rows(1,n.largest_tx_bytes),Some(false))["result"],&n),Ok(Some(vec!["log bytes (LOG_CAP)"])));
    let e=site::check_caps(&caps_answer(&rows(1,n.largest_tx_bytes-1),Some(false))["result"],&n).unwrap_err();
    assert!(e.contains(&format!("it needs {} transaction bytes in one block (TXBYTES_CAP) and {} remain",n.largest_tx_bytes,n.largest_tx_bytes-1)),"{e}");
    let e=site::check_caps(&caps_answer(&rows(0,n.tx_bytes),Some(false))["result"],&n).unwrap_err();assert!(e.contains("it needs 1 transactions in one block (TX_CAP) and 0 remain"),"{e}");
    // cumulative true, or the field absent (the stricter reading): the whole publication.
    for cumulative in [Some(true),None]{
        let e=site::check_caps(&caps_answer(&rows(1,n.largest_tx_bytes),cumulative)["result"],&n).unwrap_err();
        assert!(e.contains(&format!("it needs {} transactions (TX_CAP) and 1 remain",n.tx_count))&&e.contains("transaction bytes (TXBYTES_CAP)"),"{e}");
        assert_eq!(site::check_caps(&caps_answer(&exact(&n,0),cumulative)["result"],&n),Ok(Some(vec!["log bytes (LOG_CAP)"])),"a zero cap is at 100 %");
    }
}
#[test] fn w12_a_cap_at_80_percent_is_noted_and_the_site_still_fits(){
    let n=site::Needs{accounts:1,..Default::default()};
    assert_eq!(site::check_caps(&caps_answer(&[("accounts",80,100)],Some(false))["result"],&n),Ok(Some(vec!["accounts (ACCT_CAP)"])));
    assert_eq!(site::check_caps(&caps_answer(&[("accounts",79,100)],Some(false))["result"],&n),Ok(Some(vec![])));
}
#[test] fn w12_a_malformed_pocol_get_caps_answer_is_an_error_and_nothing_is_signed(){
    let key=pocol_lp4_execution::txsign::Key::devnet("alice");let files=sample_files();let none=Default::default();
    let good=caps_answer(&[],Some(false));
    let with=|k:&str,v:serde_json::Value|{let mut c=good.clone();c["result"][k]=v;c};
    for bad in [serde_json::json!({"result":"caps"}),serde_json::json!({"result":[1]}),with("codeBytes","1000".into()),with("accounts","0xzz".into()),with("accountsCap",7.into()),
        with("txCap","0x".into()),with("logCap",format!("0x1{}","0".repeat(32)).into()),with("cumulative","no".into()),
        serde_json::json!({"error":{"code":-32000,"message":"head state"}})]{
        let (addr,seen,_)=publish_stand_in(&files,bad.clone(),&none);let rpc=site::Rpc{addr};
        let e=publisher(&rpc,&key).publish(None,&files,"/index.html").unwrap_err();
        assert!(e.contains("pocol_getCaps")&&e.contains("Nothing was signed"),"{bad}: {e}");assert_eq!(sends(&seen),0,"{bad}");
    }
}
