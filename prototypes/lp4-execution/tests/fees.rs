use pocol_lp4_execution::fees::{next_base_fee,effective_fees,FeeError};
use revm::primitives::U256;
#[test] fn london_fixed_boundaries_and_wide_intermediate(){
 assert_eq!(next_base_fee(U256::from(1_000_000_000u64),8_000_000,8_000_000),Ok(U256::from(1_125_000_000u64)));
 assert_eq!(next_base_fee(U256::from(1_000_000_000u64),0,8_000_000),Ok(U256::from(875_000_000u64)));
 for fee in 0..8{assert_eq!(next_base_fee(U256::from(fee),0,8_000_000),Ok(U256::from(fee)));}
 assert_eq!(next_base_fee(U256::from(1),4_000_001,8_000_000),Ok(U256::from(2)));
 assert_eq!(next_base_fee(U256::MAX,4_000_000,8_000_000),Ok(U256::MAX));
 assert_eq!(next_base_fee(U256::MAX,8_000_000,8_000_000),Err(FeeError::Overflow));
 assert!(next_base_fee(U256::MAX,0,u64::MAX).is_ok());
 assert_eq!(next_base_fee(U256::from(1),8_000_001,8_000_000),Err(FeeError::ParentGas));
 assert_eq!(next_base_fee(U256::from(1),0,1),Err(FeeError::ParentGas));
}
#[test] fn fee_cap_priority_and_equality(){
 assert_eq!(effective_fees(U256::from(100),U256::from(300),Some(U256::from(10))),Ok((U256::from(110),U256::from(10))));
 assert_eq!(effective_fees(U256::from(100),U256::from(105),Some(U256::from(10))),Ok((U256::from(105),U256::from(5))));
 assert_eq!(effective_fees(U256::from(100),U256::from(100),None),Ok((U256::from(100),U256::ZERO)));
 assert_eq!(effective_fees(U256::from(100),U256::from(99),None),Err(FeeError::FeeCap));
 assert_eq!(effective_fees(U256::from(100),U256::from(105),Some(U256::from(106))),Err(FeeError::FeeCap));
}

#[test] fn independent_bigint_london_vectors(){
 let rows:Vec<serde_json::Value>=serde_json::from_str(include_str!("data/london-fees.json")).unwrap();
 assert_eq!(rows.len(),192);
 for row in rows{let actual=next_base_fee(U256::from_str_radix(row["base"].as_str().unwrap(),10).unwrap(),row["used"].as_u64().unwrap(),row["limit"].as_u64().unwrap());
 let expected=row["expected"].as_str().unwrap();match expected{"ParentGas"=>assert_eq!(actual,Err(FeeError::ParentGas)),"Overflow"=>assert_eq!(actual,Err(FeeError::Overflow)),_=>assert_eq!(actual.unwrap().to_string(),expected)}}
}
