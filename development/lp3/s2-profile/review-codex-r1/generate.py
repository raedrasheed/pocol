from pathlib import Path
import json,re,hashlib
root=Path('/workspace/Proof-of-Collaboration'); out=Path('/workspace/pocol-s2-review')
# Reviewer transcription of consensus.md CP groups 1-52, not imported from CP_SCHEMA.
names='g_ts target_g T_blk tau nonceMode c D_att dFbWait m kappa S_max D W_w alpha_bp gamma_bp subsidy B_reg R_max M_max M_min REG_RECORDS_MAX SWEEP_MAX SWEEP_SCAN REG_OPS W_act Theta_inact E_win U E_max EVID_BYTES_MAX GAS_LIMIT baseFee0 TX_MAX BODY_MAX B_code_max CODE_CAP TXBYTES_CAP TX_CAP SLOT_CAP ACCT_CAP SYS_SLOT_MAX SYS_KEYS_BLOCK_MAX SYS_KEYS_USER_MAX SYS_ACCT_BLOCK_MAX G_SYS H_END T_END H_CLOSE T_CLOSE Z_close RET P2'.split()
widths=[64,256,32,32,8,8,32,32,16,8,16,16,16,16,16,256,256,8,16,16,32,16,16,16,32,32,32,32,8,32,64,256,32,32,32,64,64,64,64,64,32,32,32,32,64,64,64,64,64,32,32,32]
mins=[1 if n in ['target_g','T_blk','tau','c','m','kappa','M_max','M_min','RET'] else 0 for n in names]
assert len(names)==len(widths)==52
source=(root/'prototypes/lp1-node/src/profile.rs').read_text()
decls=re.findall(r'pub (\w+): (u\d+|U256),',source)
expected=[(n,'U256' if w==256 else 'u'+str(w)) for n,w in zip(names,widths)]
assert decls==expected
mapping=[]
for i,(n,w) in enumerate(zip(names,widths)):
 line=re.search(r'            '+n+r': (v\[\d+\][^,]*),',source).group(1)
 assert line.startswith(f'v[{i}]')
 mapping.append({'index':i,'name':n,'bits':w,'minimum':mins[i],'declarationMatches':True,'fromIndexMatches':True})
(out/'cp-audit.json').write_text(json.dumps({'reference':'development/m1/reference/consensus.md:17-24','referenceSha256':hashlib.sha256((root/'development/m1/reference/consensus.md').read_bytes()).hexdigest(),'fields':mapping},indent=2)+'\n')
fixture=json.loads((root/'prototypes/lp1-node/fixtures/profile.json').read_text())['genesis_pre_hex']
s='''extern crate lp1_node;
use lp1_node::fixed::U256;
use lp1_node::genesis_spec::{self as gs,GenesisSpec};
use lp1_node::profile::{ConsensusParams,ForkSchedule,ProfileIdentity,ProfileError,ProfileKind,NodeKind};
fn base()->GenesisSpec { gs::decode(&lp1_node::hex::decode("'''+fixture+'''").unwrap()).unwrap() }
fn identity(s:&GenesisSpec,kind:ProfileKind,node:NodeKind,forks:&[(u16,u64)])->Result<ProfileIdentity,ProfileError>{
 let b=gs::encode(s);ProfileIdentity::validate(&b,&gs::genesis_hash(&b),s.chain_id,kind,node,forks)
}
const WIDTHS:[u32;52]='''+str(widths)+''';
const MINS:[u64;52]='''+str(mins)+''';
fn values()->[U256;52] { let mut v=[U256::ZERO;52]; for i in 0..52 {v[i]=U256::from_u64((i as u64+2).max(MINS[i]));}v[4]=U256::ONE;v }
#[test]fn all_named_fields_and_inverse_mapping(){
 let v=values();let p=ConsensusParams::from_values(&v).unwrap();
'''
for i,(n,w) in enumerate(zip(names,widths)):
 s+=f' assert_eq!('+(f'p.{n}' if w==256 else f'U256::from_u64(p.{n} as u64)')+f',v[{i}],"{n}");\n'
s+=''' assert_eq!(p.to_values(),v);
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
'''
(out/'review_checks.rs').write_text(s)
