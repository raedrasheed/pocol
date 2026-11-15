//! CAPS-SUSTAINABILITY (PROPOSED, not approved): genesisHash vectors of the independent model
//! development/audit-2026-10-22/proposals/model/caps_model.py (CP schema v1, the audited spec-1 encoding, which the
//! decoder still reads so that admission can refuse it by name), replayed through gs::encode and gs::genesis_hash, and
//! the claim that LOG_CAP is not part of the profile identity, on the v3 devnet (CP schema v2, the V3 programs; Phase 6a
//! moved it there from the retired spec-1 devnet). Run in the release build and with `--features caps-test`: both must
//! pass with the same hashes.
use pocol_lp4_execution::{apply::SystemPrograms,devnet::{self,DevnetConfig},fixed::U256,genesis_spec as gs,user_phase::caps};

fn vectors()->serde_json::Value{serde_json::from_str(include_str!("../../../development/audit-2026-10-22/proposals/vectors/caps-sustainability.json")).unwrap()}
const DEVNET_HEX:&str=include_str!("../../../development/audit-2026-10-22/proposals/vectors/caps-devnet-spec.hex");
fn hex(b:&[u8])->String{b.iter().map(|x|format!("{x:02x}")).collect()}
fn unhex(s:&str)->Vec<u8>{let s=s.trim();let s=s.strip_prefix("0x").unwrap_or(s);(0..s.len()).step_by(2).map(|i|u8::from_str_radix(&s[i..i+2],16).unwrap()).collect()}
/// The v3 devnet (DevnetConfig::new: CP schema v2, the V3 programs, nonceMode 1).
fn devnet_cfg()->DevnetConfig{DevnetConfig::new(777900,1_700_000_000,&["a","b","c"])}
/// Its genesisHash, the same in the release build and the caps-test build.
const V3_DEVNET_GENESIS_HASH:&str="addf5bee1aa8012cad1cc2c3b3f110d2ca0b2d68ac103915220d1145daffad65";

#[test] fn gsv1_and_the_cap_variants_match_the_model(){
    let v=vectors();let g=unhex(v["gsv1"]["bytes"].as_str().unwrap());assert_eq!(g.len(),341);
    let s=gs::decode(&g).unwrap();assert_eq!(gs::encode(&s),g);
    let h=hex(&gs::genesis_hash(&g));assert_eq!(h,v["gsv1"]["genesisHash"].as_str().unwrap());
    assert_eq!(h,"de518e30a5e333ac3ddf127b6201f1a66f80dce2b56566524df4037bcdeb2277","H_GSV1 (E05 three-library record)");
    let mut n=0;
    for (base,key) in [(s.clone(),"gsv1"),(gs::decode(&unhex(DEVNET_HEX)).unwrap(),"devnet")]{
        for var in v[key]["variants"].as_array().unwrap(){
            let k=gs::cp_index(var["field"].as_str().unwrap()).unwrap();assert_eq!(k+1,var["index"].as_u64().unwrap() as usize);
            let mut t=base.clone();t.cp[k]=U256::from_u64(var["value"].as_u64().unwrap());let b=gs::encode(&t);
            assert_eq!(hex(&b),var["bytes"].as_str().unwrap(),"{key} {}",var["field"]);
            assert_eq!(hex(&gs::genesis_hash(&b)),var["genesisHash"].as_str().unwrap());assert_ne!(gs::genesis_hash(&b),gs::genesis_hash(&gs::encode(&base)));n+=1;
        }
    }
    assert_eq!(n,4);
}

#[test] fn the_devnet_genesis_hash_does_not_depend_on_log_cap(){
    // Same bytes and hash in the release build (LOG_CAP = 2^30) and the caps-test build (2^24).
    let d=devnet::build(&devnet_cfg()).unwrap();
    println!("LOG_CAP {} genesisHash {} specBytes {}",caps::LOG_CAP,hex(&d.genesis_hash.0),d.spec_bytes.len());
    assert_eq!(hex(&d.genesis_hash.0),V3_DEVNET_GENESIS_HASH,"the v3 devnet genesisHash");
    assert_eq!(hex(&gs::genesis_hash(&d.spec_bytes)),V3_DEVNET_GENESIS_HASH);
    // Everything hashed into the identity: sysCodeHash of the bundled programs (no LOG_CAP), the 59 CP values of CP
    // schema v2 and allocRoot (whose ChainCaps storage holds the CP). ForkSchedule is not hashed.
    let s=gs::decode(&d.spec_bytes).unwrap();assert_eq!((s.spec_version,s.cp.len()),(2,gs::CP_V2_LEN));
    assert_eq!(s.sys_code_hash,SystemPrograms::v3().sys_code_hash().0,"the V3 programs");
    // A rebuilt genesis with TX_CAP+1 (TXBYTES_CAP+1) changes the CP bytes and allocRoot: a new chain.
    let edits:[fn(&mut DevnetConfig);2]=[|c|c.tx_cap+=1,|c|c.txbytes_cap+=1];
    for f in edits{
        let mut c=devnet_cfg();f(&mut c);let e=devnet::build(&c).unwrap();let t=gs::decode(&e.spec_bytes).unwrap();
        assert_ne!(e.genesis_hash,d.genesis_hash);assert_ne!(t.alloc_root,s.alloc_root,"ChainCaps stores every CP value");
    }
}

/// The RLP item at `at` of `b`: (header bytes, payload bytes).
fn item(b:&[u8],at:usize)->(usize,usize){
    let be=|k:usize|b[at+1..at+1+k].iter().fold(0usize,|a,x|a<<8|*x as usize);
    match b[at]{x if x<0x80=>(0,1),x if x<=0xb7=>(1,(x-0x80) as usize),x if x<=0xbf=>{let k=(x-0xb7) as usize;(1+k,be(k))},
        x if x<=0xf7=>(1,(x-0xc0) as usize),x=>{let k=(x-0xf7) as usize;(1+k,be(k))}}
}
/// A list header for `payload` bytes.
fn list(payload:Vec<u8>)->Vec<u8>{
    let n=payload.len();let mut out=if n<56{vec![0xc0+n as u8]}else{let be:Vec<u8>=n.to_be_bytes().iter().copied().skip_while(|x|*x==0).collect();let mut h=vec![0xf7+be.len() as u8];h.extend(be);h};
    out.extend(payload);out
}

#[test] fn log_cap_is_not_representable_in_the_genesis_spec(){
    // The CP schema v2 is the specification's 52 fields (consensus.md:19-25, as the model read them) and the seven
    // appended v3 fields (D-V3-34), none of them LOG_CAP or LOG_BLOCK_MAX.
    let v=vectors();let spec:Vec<&str>=v["schema"]["specCpFields"].as_array().unwrap().iter().map(|x|x.as_str().unwrap()).collect();
    let rust:Vec<&str>=gs::CP_SCHEMA_V2.iter().map(|f|f.name).collect();assert_eq!(rust[..gs::CP_LEN],spec[..]);
    assert_eq!(rust[gs::CP_LEN..],["H_half","S_tail","gamma_tip_bp","baseFeeMin","R_react","C_react","rho_bp"]);assert_eq!(rust.len(),59);
    assert!(!rust.iter().any(|n|n.starts_with("LOG")),"{rust:?}");
    // Every one of the 59 fields of the v3 devnet's genesis round-trips and enters genesisHash; nothing else is encoded.
    let g=devnet::build(&devnet_cfg()).unwrap().spec_bytes;let base=gs::decode(&g).unwrap();let h0=gs::genesis_hash(&g);
    for k in 0..gs::CP_V2_LEN{
        let mut t=base.clone();let one=U256::from_u64(1);let f=&gs::CP_SCHEMA_V2[k];
        t.cp[k]=if t.cp[k]>U256::from_u64(f.lo){t.cp[k].checked_sub(&one).unwrap()}else{U256::from_u64(t.cp[k].to_u64().unwrap()+1)};
        let b=gs::encode(&t);assert_eq!(gs::decode(&b).unwrap(),t,"{} round-trips",f.name);assert_ne!(gs::genesis_hash(&b),h0,"{} is hashed",f.name);
    }
    // A 60th CP item (where a LOG_CAP would have to go) is refused by the decoder: gsCount.
    let (th,tp)=item(&g,0);assert_eq!(th+tp,g.len());let mut at=th;let mut children=Vec::new();
    while at<g.len(){let (h,p)=item(&g,at);children.push(g[at..at+h+p].to_vec());at+=h+p;}
    assert_eq!(children.len(),6,"version, chainId, CP, allocRoot, M_0List, sysCodeHash");
    let (ch,_)=item(&children[2],0);let mut cp=children[2][ch..].to_vec();cp.push(0x01);children[2]=list(cp);
    let items=list(children.concat());
    assert_eq!(gs::decode(&items).unwrap_err().code,"gsCount");
    assert_eq!(caps::LOG_CAP,if cfg!(feature="caps-test"){1<<24}else{1<<30},"absent from the release (validation.md:2065)");
}
