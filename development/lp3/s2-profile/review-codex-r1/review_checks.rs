extern crate lp1_node;
use lp1_node::fixed::U256;
use lp1_node::genesis_spec::{self as gs,GenesisSpec};
use lp1_node::profile::{ConsensusParams,ForkSchedule,ProfileIdentity,ProfileError,ProfileKind,NodeKind};
fn base()->GenesisSpec { gs::decode(&lp1_node::hex::decode("f9015201830bdb2af8b1846553f1009f010000000000000000000000000000000000000000000000000000000000000a82025801023c06200282010010408203e88201f48301388089056bc75e2d631000000282010001824000104010818082040082080082089804822000837a1200843b9aca008302000083030000828000844000000085020000000083989680833d0900830f4240830200008208008201008202008405b8d80081c88465ec8780288465ec878081a0818002a02222222222222222222222222222222222222222222222222222222222222222f856ea94a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a194b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1b1ea94a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a2a294b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2b2a03333333333333333333333333333333333333333333333333333333333333333").unwrap()).unwrap() }
fn identity(s:&GenesisSpec,kind:ProfileKind,node:NodeKind,forks:&[(u16,u64)])->Result<ProfileIdentity,ProfileError>{
 let b=gs::encode(s);ProfileIdentity::validate(&b,&gs::genesis_hash(&b),s.chain_id,kind,node,forks)
}
const WIDTHS:[u32;52]=[64, 256, 32, 32, 8, 8, 32, 32, 16, 8, 16, 16, 16, 16, 16, 256, 256, 8, 16, 16, 32, 16, 16, 16, 32, 32, 32, 32, 8, 32, 64, 256, 32, 32, 32, 64, 64, 64, 64, 64, 32, 32, 32, 32, 64, 64, 64, 64, 64, 32, 32, 32];
const MINS:[u64;52]=[0, 1, 1, 1, 0, 1, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0];
fn values()->[U256;52] { let mut v=[U256::ZERO;52]; for i in 0..52 {v[i]=U256::from_u64((i as u64+2).max(MINS[i]));}v[4]=U256::ONE;v }
#[test]fn all_named_fields_and_inverse_mapping(){
 let v=values();let p=ConsensusParams::from_values(&v).unwrap();
 assert_eq!(U256::from_u64(p.g_ts as u64),v[0],"g_ts");
 assert_eq!(p.target_g,v[1],"target_g");
 assert_eq!(U256::from_u64(p.T_blk as u64),v[2],"T_blk");
 assert_eq!(U256::from_u64(p.tau as u64),v[3],"tau");
 assert_eq!(U256::from_u64(p.nonceMode as u64),v[4],"nonceMode");
 assert_eq!(U256::from_u64(p.c as u64),v[5],"c");
 assert_eq!(U256::from_u64(p.D_att as u64),v[6],"D_att");
 assert_eq!(U256::from_u64(p.dFbWait as u64),v[7],"dFbWait");
 assert_eq!(U256::from_u64(p.m as u64),v[8],"m");
 assert_eq!(U256::from_u64(p.kappa as u64),v[9],"kappa");
 assert_eq!(U256::from_u64(p.S_max as u64),v[10],"S_max");
 assert_eq!(U256::from_u64(p.D as u64),v[11],"D");
 assert_eq!(U256::from_u64(p.W_w as u64),v[12],"W_w");
 assert_eq!(U256::from_u64(p.alpha_bp as u64),v[13],"alpha_bp");
 assert_eq!(U256::from_u64(p.gamma_bp as u64),v[14],"gamma_bp");
 assert_eq!(p.subsidy,v[15],"subsidy");
 assert_eq!(p.B_reg,v[16],"B_reg");
 assert_eq!(U256::from_u64(p.R_max as u64),v[17],"R_max");
 assert_eq!(U256::from_u64(p.M_max as u64),v[18],"M_max");
 assert_eq!(U256::from_u64(p.M_min as u64),v[19],"M_min");
 assert_eq!(U256::from_u64(p.REG_RECORDS_MAX as u64),v[20],"REG_RECORDS_MAX");
 assert_eq!(U256::from_u64(p.SWEEP_MAX as u64),v[21],"SWEEP_MAX");
 assert_eq!(U256::from_u64(p.SWEEP_SCAN as u64),v[22],"SWEEP_SCAN");
 assert_eq!(U256::from_u64(p.REG_OPS as u64),v[23],"REG_OPS");
 assert_eq!(U256::from_u64(p.W_act as u64),v[24],"W_act");
 assert_eq!(U256::from_u64(p.Theta_inact as u64),v[25],"Theta_inact");
 assert_eq!(U256::from_u64(p.E_win as u64),v[26],"E_win");
 assert_eq!(U256::from_u64(p.U as u64),v[27],"U");
 assert_eq!(U256::from_u64(p.E_max as u64),v[28],"E_max");
 assert_eq!(U256::from_u64(p.EVID_BYTES_MAX as u64),v[29],"EVID_BYTES_MAX");
 assert_eq!(U256::from_u64(p.GAS_LIMIT as u64),v[30],"GAS_LIMIT");
 assert_eq!(p.baseFee0,v[31],"baseFee0");
 assert_eq!(U256::from_u64(p.TX_MAX as u64),v[32],"TX_MAX");
 assert_eq!(U256::from_u64(p.BODY_MAX as u64),v[33],"BODY_MAX");
 assert_eq!(U256::from_u64(p.B_code_max as u64),v[34],"B_code_max");
 assert_eq!(U256::from_u64(p.CODE_CAP as u64),v[35],"CODE_CAP");
 assert_eq!(U256::from_u64(p.TXBYTES_CAP as u64),v[36],"TXBYTES_CAP");
 assert_eq!(U256::from_u64(p.TX_CAP as u64),v[37],"TX_CAP");
 assert_eq!(U256::from_u64(p.SLOT_CAP as u64),v[38],"SLOT_CAP");
 assert_eq!(U256::from_u64(p.ACCT_CAP as u64),v[39],"ACCT_CAP");
 assert_eq!(U256::from_u64(p.SYS_SLOT_MAX as u64),v[40],"SYS_SLOT_MAX");
 assert_eq!(U256::from_u64(p.SYS_KEYS_BLOCK_MAX as u64),v[41],"SYS_KEYS_BLOCK_MAX");
 assert_eq!(U256::from_u64(p.SYS_KEYS_USER_MAX as u64),v[42],"SYS_KEYS_USER_MAX");
 assert_eq!(U256::from_u64(p.SYS_ACCT_BLOCK_MAX as u64),v[43],"SYS_ACCT_BLOCK_MAX");
 assert_eq!(U256::from_u64(p.G_SYS as u64),v[44],"G_SYS");
 assert_eq!(U256::from_u64(p.H_END as u64),v[45],"H_END");
 assert_eq!(U256::from_u64(p.T_END as u64),v[46],"T_END");
 assert_eq!(U256::from_u64(p.H_CLOSE as u64),v[47],"H_CLOSE");
 assert_eq!(U256::from_u64(p.T_CLOSE as u64),v[48],"T_CLOSE");
 assert_eq!(U256::from_u64(p.Z_close as u64),v[49],"Z_close");
 assert_eq!(U256::from_u64(p.RET as u64),v[50],"RET");
 assert_eq!(U256::from_u64(p.P2 as u64),v[51],"P2");
 assert_eq!(p.to_values(),v);
}
#[test]fn independent_schema_edges(){
 let start=values();
 for i in 0..52 {
  let mut v=start;
  v[i]=U256::from_u64(MINS[i]);assert!(ConsensusParams::from_values(&v).is_ok(),"min {i}");
  if MINS[i]>0 {v[i]=U256::ZERO;assert!(ConsensusParams::from_values(&v).is_err(),"below min {i}");}
  v=start;
  if WIDTHS[i]<256 {v[i]=U256::pow2(WIDTHS[i]).unwrap();assert!(ConsensusParams::from_values(&v).is_err(),"overflow {i}");}
  let high=match i {4=>1,10=>256,13=>10000,14=>10000-start[13].to_u64().unwrap(),_=>0};
  if high>0 {v=start;if i==13 {v[14]=U256::ZERO;}v[i]=U256::from_u64(high);assert!(ConsensusParams::from_values(&v).is_ok(),"upper {i}");v[i]=U256::from_u64(high+1);assert!(ConsensusParams::from_values(&v).is_err(),"above upper {i}");}
 }
 let mut v=start;v[13]=U256::from_u64(9999);v[14]=U256::ONE;assert!(ConsensusParams::from_values(&v).is_ok());v[14]=U256::from_u64(2);assert!(ConsensusParams::from_values(&v).is_err());
 for i in [1,15,16,31] {v=start;v[i]=U256::MAX;let p=ConsensusParams::from_values(&v).unwrap();assert_eq!(p.to_values(),v);}
}
#[test]fn exhaustive_version_support_and_schedule_shape(){
 assert!(ForkSchedule::new(&[]).is_err());assert!(ForkSchedule::new(&[(1,1)]).is_err());
 for ver in 0..=u16::MAX { let r=ForkSchedule::new(&[(1,0),(ver,9)]);assert_eq!(r.is_ok(),ver==1,"version {ver}"); }
 for h in [0,1,2,50,u64::MAX-1,u64::MAX] {
  let r=ForkSchedule::new(&[(1,0),(1,h)]);assert_eq!(r.is_ok(),h>0);
  if let Ok(r)=r { for query in [0,h.saturating_sub(1),h,h.saturating_add(1),u64::MAX] {assert_eq!(r.active_version(query),1);} }
 }
 assert!(ForkSchedule::new(&[(1,0),(1,20),(1,19)]).is_err());
}
#[test]fn binding_and_kind_matrix_with_nonbootable_roots(){
 for chain in [1,777001,777002,777902,u64::MAX] {
  for kind in [ProfileKind::Network,ProfileKind::Simnet,ProfileKind::Synthetic] {
   for node in [NodeKind::Node,NodeKind::Simnet] {
    let mut s=base();s.chain_id=chain;
    let r=identity(&s,kind,node,&[(1,0)]);
    let allowed=if kind==ProfileKind::Network {chain==777001} else {node==NodeKind::Simnet&&chain!=777001};
    assert_eq!(r.is_ok(),allowed);
    if let Ok(p)=r {assert!(!p.bootable());assert_eq!(p.spec(),&s);assert_eq!(p.cp().to_values(),s.cp);}
   }
  }
 }
 let mut s=base();s.chain_id=0;assert!(identity(&s,ProfileKind::Synthetic,NodeKind::Simnet,&[(1,0)]).is_err());
 let s=base();let b=gs::encode(&s);let hash=gs::genesis_hash(&b);
 let call=|bytes:&[u8],h:&[u8;32],id|ProfileIdentity::validate(bytes,h,id,ProfileKind::Synthetic,NodeKind::Simnet,&[(1,0)]);
 for i in 0..32 {let mut h=hash;h[i]^=1;assert!(call(&b,&h,s.chain_id).is_err());}
 assert!(call(&b,&hash,0).is_err());assert!(call(&b,&hash,u64::MAX).is_err());
 for i in 0..64 {let mut changed=s.clone();if i<32 {changed.alloc_root[i]^=1;}else {changed.sys_code_hash[i-32]^=1;}
  let bytes=gs::encode(&changed);assert!(call(&bytes,&hash,s.chain_id).is_err());
  // Rebinding arbitrary roots verifies identity only, never builder facts or boot.
  assert!(!call(&bytes,&gs::genesis_hash(&bytes),s.chain_id).unwrap().bootable());
 }
 // ForkSchedule is explicitly excluded from GenesisSpec hash.
 let a=identity(&s,ProfileKind::Synthetic,NodeKind::Simnet,&[(1,0)]).unwrap();
 let b=identity(&s,ProfileKind::Synthetic,NodeKind::Simnet,&[(1,0),(1,50)]).unwrap();assert_eq!(a.genesis_hash(),b.genesis_hash());
}
#[test]fn fresh_seed_roundtrips_and_bootability(){
 let mut seed=0x20261017abcdu64;
 for _ in 0..1000 {let mut v=values();
  for i in 0..52 {seed^=seed<<13;seed^=seed>>7;seed^=seed<<17;
   let n=match i {4=>seed%2,10=>seed%257,13=>seed%10001,14=>0,_=> seed & (u64::MAX >> (64-WIDTHS[i].min(64)))};
   v[i]=U256::from_u64(n.max(MINS[i]));
  }
  let p=ConsensusParams::from_values(&v).unwrap();assert_eq!(p.to_values(),v);
  let mut s=base();s.cp=v;let p=identity(&s,ProfileKind::Synthetic,NodeKind::Simnet,&[(1,0)]).unwrap();assert!(!p.bootable());assert_eq!(p.cp().to_values(),v);
 }
}
