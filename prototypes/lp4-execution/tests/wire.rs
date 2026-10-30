use pocol_lp4_execution::wire::{decode_signed,WireError};
pub fn unhex(s:&str)->Vec<u8>{s.as_bytes().chunks(2).map(|p|u8::from_str_radix(std::str::from_utf8(p).unwrap(),16).unwrap()).collect()}
pub fn vectors()->serde_json::Value{serde_json::from_str(include_str!("data/wire-envelopes.json")).unwrap()}
#[test] fn signed_envelopes_match_independent_oracle(){
 let data=vectors();let mut positive=0;let mut negative=0;
 for row in data["cases"].as_array().unwrap(){let raw=unhex(row["raw"].as_str().unwrap());let result=decode_signed(&raw,raw.len());
  if let Some(error)=row["error"].as_str(){assert_eq!(result.is_err(),true,"{} unexpectedly accepted",row["name"]);assert_eq!(format!("{:?}",result.unwrap_err()),error,"{}",row["name"]);negative+=1;}
  else{let envelope=result.unwrap();assert_eq!(envelope.view(),row["view"],"{}",row["name"]);assert_eq!(envelope.raw(),raw);positive+=1;}
 }assert!(positive>=18 && negative>=50);
}
#[test] fn byte_budget_equality_and_minus_one(){
 let data=vectors();for row in data["cases"].as_array().unwrap().iter().filter(|r|r["error"].is_null()){
  let raw=unhex(row["raw"].as_str().unwrap());assert!(decode_signed(&raw,raw.len()).is_ok());assert!(matches!(decode_signed(&raw,raw.len()-1),Err(WireError::Budget)));
 }
}
#[test] fn specified_chain_type_pool_mapping_is_not_block_acceptance(){
 for error in [WireError::ChainId,WireError::UnprotectedLegacy,WireError::UnsupportedType]{assert_eq!(error.specified_pool_code(),Some(-32015));}
 assert_eq!(WireError::Signature.specified_pool_code(),None);
}
