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
fn stand_in()->String{
    use std::io::{Read,Write};
    let l=std::net::TcpListener::bind("127.0.0.1:0").unwrap();let addr=l.local_addr().unwrap().to_string();
    std::thread::spawn(move||for s in l.incoming(){
        let Ok(mut s)=s else{continue};let mut buf=Vec::new();let mut b=[0u8;4096];
        let body=loop{let n=s.read(&mut b).unwrap_or(0);if n==0{break None}buf.extend(&b[..n]);
            if let Some(i)=buf.windows(4).position(|w|w==b"\r\n\r\n"){
                let head=String::from_utf8_lossy(&buf[..i]).to_ascii_lowercase();
                let len:usize=head.lines().find_map(|x|x.strip_prefix("content-length:")).map(|v|v.trim().parse().unwrap()).unwrap_or(0);
                if buf.len()>=i+4+len{break Some(buf[i+4..i+4+len].to_vec())}}};
        let Some(body)=body else{continue};let req:serde_json::Value=serde_json::from_slice(&body).unwrap();
        let p=&req["params"];
        let out=match req["method"].as_str().unwrap(){
            "eth_getCode" if p[1]=="safe"=>serde_json::json!({"error":{"code":-32000,"message":"safe state unavailable"}}),
            "eth_getCode"=>serde_json::json!({"result":"0x0001"}),
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
    addr
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
