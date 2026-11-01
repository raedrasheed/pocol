use pocol_lp4_execution::receipts::{self,ReceiptError};
#[test] fn empty_root_and_budget_and_supported_types(){
 let root=receipts::ordered_root(&[],0).unwrap();assert_eq!(format!("{root}"),"0x56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421");
 assert_eq!(receipts::ordered_root(&[vec![]],1),Err(ReceiptError::EmptyEnvelope));
 assert_eq!(receipts::ordered_root(&[vec![1]],0),Err(ReceiptError::Budget));
 for kind in [3,4,255]{assert_eq!(receipts::encode(kind,true,0,&[]),Err(ReceiptError::UnsupportedType));}
 let receipt=receipts::encode(0,true,21000,&[]).unwrap();for kind in [1,2]{let typed=receipts::encode(kind,true,21000,&[]).unwrap();assert_eq!(typed[0],kind);assert_eq!(&typed[1..],receipt);}
 assert_ne!(receipts::encode(0,false,21000,&[]).unwrap(),receipt);
}

// Constants generated independently by eth_bloom and HexaryTrie, not this crate.
#[test] fn independent_bloom_and_indexed_root(){
 use revm::primitives::{Address,B256,Bytes,Log};
 let log=Log::new(Address::from([0x22;20]),vec![B256::from(revm::primitives::U256::from(17))],Bytes::from(vec![123])).unwrap();
 let actual=receipts::bloom(&[log]).iter().map(|b|format!("{b:02x}")).collect::<String>();
 assert_eq!(actual,"00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004000001000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000400000000001000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000");
 assert_eq!(format!("{}",receipts::ordered_root(&[vec![1],vec![2]],2).unwrap()),"0x08693bf7194a9e5481f518cc27be677512d038ddfb72263e13f0663e299ce2d8");
}
