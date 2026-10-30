//! D-DEP-SECP: the accepted LP3 chain fixture (lp1-node/fixtures) verified through pocold's k256
//! secp_backend. Every proposer signature recovers the profile signer, every winner signature
//! recovers, and the a3 negatives give the same faults as lp1-node's libsecp256k1 backend.
use pocold::{hashes::{self,SigFault},header,hex};
fn fixture(name:&str)->serde_json::Value{
    serde_json::from_str(&std::fs::read_to_string(format!("{}/../lp1-node/fixtures/{name}",env!("CARGO_MANIFEST_DIR"))).unwrap()).unwrap()
}
#[test] fn accepted_chain_fixture_signatures_recover_through_k256(){
    let chain=fixture("chain.json");let signer=hex::decode(fixture("profile.json")["signer_address"].as_str().unwrap()).unwrap();
    let (mut signed,mut winners)=(0,0);let mut first=None;
    for h in chain["headers"].as_array().unwrap(){
        let x=header::decode(&hex::decode(h["rlp_hex"].as_str().unwrap()).unwrap()).unwrap();let tid=hashes::sha256(&x.ut_raw);
        if !x.sig.is_empty(){assert_eq!(hashes::recover_address(&hashes::sig_msg(&tid),&x.sig).unwrap().to_vec(),signer);signed+=1;if first.is_none(){first=Some((tid,x.sig.clone()));}}
        assert!(hashes::recover_address(&hashes::win_msg(&tid,x.nonce,&hashes::share_root(&x.share_list_raw)),&x.winner_sig).is_ok());winners+=1;
    }
    assert!(signed>=1&&winners==20,"signed={signed} winners={winners}");
    // a3 negatives (lp1-node tests/fixtures_integration.rs) on the first signed header.
    let (tid,sig)=first.unwrap();let msg=hashes::sig_msg(&tid);
    let mut hs=sig.clone();let mut b=[0u8;32];let mut borrow=0i32;
    for i in (0..32).rev(){let d=hashes::N_ORDER[i] as i32-hs[32+i] as i32-borrow;b[i]=d.rem_euclid(256) as u8;borrow=(d<0) as i32;}
    hs[32..64].copy_from_slice(&b);
    assert_eq!(hashes::recover_address(&msg,&hs),Err(SigFault::HighS));
    let mut bv=sig.clone();bv[64]=27;assert_eq!(hashes::recover_address(&msg,&bv),Err(SigFault::BadV));
    let mut fv=sig.clone();fv[64]^=1;assert_ne!(hashes::recover_address(&msg,&fv).ok().map(|a|a.to_vec()),Some(signer.clone()));
    let mut other=msg;other[0]^=1;assert_ne!(hashes::recover_address(&other,&sig).ok().map(|a|a.to_vec()),Some(signer.clone()));
    assert_eq!(hashes::recover_address(&msg,&sig[..64]),Err(SigFault::Length));
}
