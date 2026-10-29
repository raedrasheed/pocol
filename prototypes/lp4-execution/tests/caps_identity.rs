//! CAPS-SUSTAINABILITY (PROPOSED, not approved): genesisHash vectors of the independent model
//! development/audit-2026-10-22/proposals/model/caps_model.py, replayed through gs::encode and
//! gs::genesis_hash, and the claim that LOG_CAP is not part of the profile identity. Run in the release
//! build and with `--features caps-test`: both must pass with the same hashes.
use pocol_lp4_execution::{apply::SystemPrograms,devnet::{self,DevnetConfig},fixed::U256,genesis_spec as gs,user_phase::caps};

fn vectors()->serde_json::Value{serde_json::from_str(include_str!("../../../development/audit-2026-10-22/proposals/vectors/caps-sustainability.json")).unwrap()}
const DEVNET_HEX:&str=include_str!("../../../development/audit-2026-10-22/proposals/vectors/caps-devnet-spec.hex");
fn hex(b:&[u8])->String{b.iter().map(|x|format!("{x:02x}")).collect()}
fn unhex(s:&str)->Vec<u8>{let s=s.trim();let s=s.strip_prefix("0x").unwrap_or(s);(0..s.len()).step_by(2).map(|i|u8::from_str_radix(&s[i..i+2],16).unwrap()).collect()}
fn devnet_cfg()->DevnetConfig{DevnetConfig::new(777900,1_700_000_000,&["a","b","c"])}

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
    let v=vectors();let d=devnet::build(&devnet_cfg()).unwrap();
    println!("LOG_CAP {} genesisHash {} specBytes {}",caps::LOG_CAP,hex(&d.genesis_hash.0),d.spec_bytes.len());
    assert_eq!(hex(&d.spec_bytes),DEVNET_HEX.trim().trim_start_matches("0x"),"devnet GenesisSpec bytes (vectors/caps-devnet-spec.hex); new bytes: {}",hex(&d.spec_bytes));
    assert_eq!(hex(&d.genesis_hash.0),v["devnet"]["genesisHash"].as_str().unwrap());
    assert_eq!(hex(&gs::genesis_hash(&d.spec_bytes)),v["devnet"]["genesisHash"].as_str().unwrap());
    // Everything hashed into the identity: sysCodeHash of the bundled programs (no LOG opcode, no LOG_CAP),
    // the 52 CP values and allocRoot (whose ChainCaps storage holds the CP). ForkSchedule is not hashed.
    assert_eq!(hex(&SystemPrograms::v1().sys_code_hash().0),v["devnet"]["sysCodeHash"].as_str().unwrap());
    let s=gs::decode(&d.spec_bytes).unwrap();assert_eq!(hex(&s.sys_code_hash),v["devnet"]["sysCodeHash"].as_str().unwrap());
    assert_eq!(hex(&s.alloc_root),v["devnet"]["allocRoot"].as_str().unwrap());
    // A rebuilt genesis with TX_CAP+1 (TXBYTES_CAP+1) changes the CP bytes and allocRoot: a new chain.
    let edits:[fn(&mut DevnetConfig);2]=[|c|c.tx_cap+=1,|c|c.txbytes_cap+=1];
    for f in edits{
        let mut c=devnet_cfg();f(&mut c);let e=devnet::build(&c).unwrap();let t=gs::decode(&e.spec_bytes).unwrap();
        assert_ne!(e.genesis_hash,d.genesis_hash);assert_ne!(t.alloc_root,s.alloc_root,"ChainCaps stores every CP value");
    }
}

#[test] fn log_cap_is_not_representable_in_the_genesis_spec(){
    // The CP schema is exactly the specification's 52 fields (consensus.md:19-25, as the model read them),
    // none of them LOG_CAP or LOG_BLOCK_MAX.
    let v=vectors();let spec:Vec<&str>=v["schema"]["specCpFields"].as_array().unwrap().iter().map(|x|x.as_str().unwrap()).collect();
    let rust:Vec<&str>=gs::CP_SCHEMA.iter().map(|f|f.name).collect();assert_eq!(rust,spec);assert_eq!(rust.len(),52);
    assert!(!rust.iter().any(|n|n.starts_with("LOG")),"{rust:?}");
    // Every one of the 52 fields round-trips and enters genesisHash; nothing else is encoded.
    let g=unhex(v["gsv1"]["bytes"].as_str().unwrap());let base=gs::decode(&g).unwrap();let h0=gs::genesis_hash(&g);
    for k in 0..gs::CP_LEN{
        let mut t=base.clone();let one=U256::from_u64(1);
        t.cp[k]=if t.cp[k]>U256::from_u64(gs::CP_SCHEMA[k].lo){t.cp[k].checked_sub(&one).unwrap()}else{U256::from_u64(t.cp[k].to_u64().unwrap()+1)};
        let b=gs::encode(&t);assert_eq!(gs::decode(&b).unwrap(),t,"{} round-trips",gs::CP_SCHEMA[k].name);assert_ne!(gs::genesis_hash(&b),h0,"{} is hashed",gs::CP_SCHEMA[k].name);
    }
    // A 53rd CP item (where a LOG_CAP would have to go) is refused by the decoder: gsCount.
    let mut items=g.clone();let cp_at=g.windows(2).position(|w|w==[0xf8,0xb1]).unwrap();
    items[cp_at+1]=0xb2;items.insert(cp_at+2+177,0x01);items[2]=items[2]+1;
    assert_eq!(gs::decode(&items).unwrap_err().code,"gsCount");
    assert_eq!(caps::LOG_CAP,if cfg!(feature="caps-test"){1<<24}else{1<<30},"absent from the release (validation.md:2065)");
}
