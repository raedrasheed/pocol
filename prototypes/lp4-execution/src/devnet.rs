//! Local development network profiles (profileKind=simnet, NodeKind::Simnet).
//! Bootable ONLY for local devnets (OWNER-STANDING-AUTHORIZATION.md §2): disposable
//! keys, no funds. The network profile (chainId 777001) is never built here.
use revm::{InMemoryDB,primitives::{AccountInfo,Address,B256,U256}};
use crate::{admission::{Admitted,NodeLocal,Request,Timing},apply::{Genesis,SystemPrograms},fixed,gate::FactClass,genesis_spec::{self as gs,GenesisSpec,Member},profile::{ProfileIdentity,ProfileKind,NodeKind},txsign::Key};

/// CP values in schema order. `DevnetConfig::new` is a v3 devnet: the rules the fresh network runs (CP schema v2,
/// specVersion 2, the V3 system programs, nonceMode 1, a continuous chain and the values of
/// development/decisions/FRESH-NETWORK-CP.md), with the timing, membership and caps scaled for a local devnet.
/// `DevnetConfig::spec1` is the retired spec-1 devnet, kept only until Phase 6a removes the spec-1 rule set.
#[derive(Clone,Debug)]
pub struct DevnetConfig{
    pub spec_version:u8,
    pub chain_id:u64,
    pub g_ts:u64,
    /// H_cal (R25d): target_g = min(2^256-1, max(1, floor(2^256/(H_cal*T_blk))-1)).
    /// For a devnet this is a chosen difficulty, recorded as a synthetic calibration.
    pub hash_rate:u64,
    pub t_blk:u32,pub tau:u32,pub d_att:u32,pub d_fb_wait:u32,
    pub d:u16,pub w_w:u16,pub m:u16,pub kappa:u8,pub s_max:u16,
    pub w_act:u32,pub theta_inact:u32,pub e_win:u32,pub u:u32,
    pub h_end:u64,pub h_close:u64,pub t_end_offset:u64,pub t_close_offset:u64,pub ret:u32,pub p2:u32,
    pub m_max:u16,pub r_max:u8,pub reg_records_max:u32,pub sys_slot_max:u32,pub z_close:u32,
    pub gas_limit:u64,pub slot_cap:u64,pub acct_cap:u64,pub code_cap:u64,pub txbytes_cap:u64,pub tx_cap:u64,
    pub alpha_bp:u16,pub gamma_bp:u16,
    /// nonceMode=0 bound multiplier: nMax=min(2^64, c·work(target)).
    pub c:u8,
    /// CP field 4: 0 bounded (v1), 1 the full 64-bit nonce domain from genesis.
    pub nonce_mode:u8,
    pub miners:Vec<String>,
    /// Extra funded devnet accounts (label, wei).
    pub accounts:Vec<(String,U256)>,
    /// CP 16, the registration bond B_reg in wei: 20 PCOL, the bond the V3 programs earn from mining (FN-0, owner 10c).
    pub b_reg:U256,
    /// `None` for a v3 devnet (the V3 programs). `Some` only through `DevnetConfig::spec1`: the programs of a retired
    /// spec-1 devnet. Phase 6a deletes it with the spec-1 rule set.
    pub spec1:Option<Spec1Programs>,
    /// CP 15, the block subsidy S0 in wei: 4 PCOL, the fresh network's S0 (FN-0); a spec-1 devnet keeps 2 PCOL.
    pub subsidy:U256,
    /// Synthetic CP-v2 fixtures; the measured v3 network profile is selected separately. Any S_tail builds here (the
    /// V3-19 supply-bound tests probe large tails); the network-profile builder (pocold testnet.rs) requires S0/32.
    pub h_half:u64,pub s_tail:U256,
    pub gamma_tip_bp:u16,pub base_fee_min:U256,
    pub r_react:u8,pub c_react:u32,pub rho_bp:u16,
}
/// The system programs of a retired spec-1 devnet (`DevnetConfig::spec1`). Phase 6a deletes them with the spec-1 rule set.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Spec1Programs{
    /// The bonded spec-1 rules (B_reg 100 PCOL): PoColSystemV1 on a finite chain, the Continuous set at H_END = 0.
    Bonded,
    /// The spec-1 earn-in set (EARN-IN-SPEC.md, B_reg 20 PCOL), the retired cutover target.
    EarnIn,
}
impl DevnetConfig{
    /// A v3 devnet: specVersion 2 (CP schema v2), the V3 programs, nonceMode 1, a continuous chain (H_END = 0),
    /// B_reg 20 PCOL, S0 4 PCOL with S_tail = S0/32 and the v3 fee and reaction values of FRESH-NETWORK-CP.md.
    /// The timing, membership and caps stay devnet-scale: D-A02-PROFILE single-host timing (admission::NodeLocal::single_host,
    /// about 2^18 hashes per block), D-A13-PROFILE caps/64 limits (validation.md T18b), M_max 64, SYS_SLOT_MAX above
    /// the derived footprint and R_max 1 (R16: the fresh network's 4 would need REG_RECORDS_MAX 8,416 at these windows,
    /// above the devnet's 8,192). A test that needs a finite chain sets `h_end` and `h_close` (the end times follow).
    pub fn new(chain_id:u64,g_ts:u64,miners:&[&str])->Self{
        Self{spec_version:2,nonce_mode:1,h_end:0,h_close:0,r_max:1,b_reg:U256::from(PCOL*20),spec1:None,
            subsidy:U256::from(PCOL*4),s_tail:U256::from(PCOL*4/32),..Self::spec1_values(chain_id,g_ts,miners)}
    }
    /// A retired spec-1 devnet: specVersion 1 (CP schema v1), nonceMode 0, H_END 20,000, R_max 0 and S0 2 PCOL, with
    /// `programs` (B_reg 100 PCOL bonded, 20 PCOL earn-in). Exactly the devnet `new` built before Phase 5a. Only tests
    /// that pin spec-1 behaviour use it; Phase 6a deletes it with the spec-1 rule set.
    pub fn spec1(chain_id:u64,g_ts:u64,miners:&[&str],programs:Spec1Programs)->Self{
        let b_reg=match programs{Spec1Programs::Bonded=>PCOL*100,Spec1Programs::EarnIn=>PCOL*20};
        Self{spec1:Some(programs),b_reg:U256::from(b_reg),..Self::spec1_values(chain_id,g_ts,miners)}
    }
    fn spec1_values(chain_id:u64,g_ts:u64,miners:&[&str])->Self{
        // D-A02-PROFILE: gate-valid single-host timing (admission::NodeLocal::single_host);
        // about 2^18 hashes per block. D-A13-PROFILE: the approved caps/64 limits
        // (validation.md T18b), SYS_SLOT_MAX above the derived footprint, R_max=0 (the v1
        // programs implement no registration) and H_END=20000, so that the A13 resident
        // bound of a node fits its memory budget.
        Self{spec_version:1,chain_id,g_ts,hash_rate:1<<16,
            t_blk:4,tau:40,d_att:25,d_fb_wait:3,d:4,w_w:8,m:32,kappa:2,s_max:256,
            w_act:32,theta_inact:64,e_win:16,u:40,h_end:20_000,h_close:19_000,t_end_offset:10_000_000,t_close_offset:9_990_000,ret:128,p2:2,m_max:64,
            r_max:0,reg_records_max:8192,sys_slot_max:65536,z_close:160,gas_limit:8_000_000,slot_cap:50_000,acct_cap:20_000,
            code_cap:1<<21,txbytes_cap:1<<24,tx_cap:20_000,alpha_bp:1000,gamma_bp:500,c:2,
            miners:miners.iter().map(|s|s.to_string()).collect(),
            accounts:vec![("alice".into(),U256::from(10u128.pow(24))),("bob".into(),U256::from(10u128.pow(24)))],b_reg:U256::from(PCOL*100),spec1:Some(Spec1Programs::Bonded),nonce_mode:0,
            subsidy:U256::from(2_000_000_000_000_000_000u128),h_half:31_557_600,s_tail:U256::from(62_500_000_000_000_000u64),
            gamma_tip_bp:5000,base_fee_min:U256::from(10_000_000),
            r_react:4,c_react:40,rho_bp:2500}
    }
    pub fn cp(&self)->Vec<fixed::U256>{
        let n=|v:u64|fixed::U256::from_u64(v);
        let mut cp=vec![n(self.g_ts),self.target_g(),n(self.t_blk.into()),n(self.tau.into()),n(self.nonce_mode.into()),n(self.c.into()),n(self.d_att.into()),n(self.d_fb_wait.into()),
         n(self.m.into()),n(self.kappa.into()),n(self.s_max.into()),n(self.d.into()),n(self.w_w.into()),n(self.alpha_bp.into()),n(self.gamma_bp.into()),
         to_fixed(self.subsidy),to_fixed(self.b_reg),n(self.r_max.into()),n(self.m_max.into()),n(1),n(self.reg_records_max.into()),n(16),n(64),n(16),
         n(self.w_act.into()),n(self.theta_inact.into()),n(self.e_win.into()),n(self.u.into()),n(4),n(8192),
         n(self.gas_limit),n(1_000_000_000),n(131072),n(196608),n(32768),n(self.code_cap),n(self.txbytes_cap),n(self.tx_cap),n(self.slot_cap),n(self.acct_cap),
         // V3-02: a CP schema v2 (v3) devnet doubles SYS_KEYS_BLOCK_MAX (2048 -> 4096): R18 counts the 3·L keys a block's
         // late shares can write (resources::late_keys), above 2048 at M_max 256; spec-1 devnets keep 2048.
         n(self.sys_slot_max.into()),n(if self.spec_version>=2{4096}else{2048}),n(256),n(512),n(96_000_000),
         n(self.h_end),n(if self.h_end==0{0}else{self.g_ts+self.t_end_offset}),n(if self.h_end==0{0}else{self.h_close}),n(if self.h_end==0{0}else{self.g_ts+self.t_close_offset}),n(self.z_close.into()),n(self.ret.into()),n(self.p2.into())];
        if self.spec_version==2{cp.extend([
            n(self.h_half),to_fixed(self.s_tail),
            n(self.gamma_tip_bp.into()),to_fixed(self.base_fee_min),
            n(self.r_react.into()),n(self.c_react.into()),n(self.rho_bp.into())]);}
        cp
    }
}
impl DevnetConfig{
    /// R25d calibration formula; a zero rate or T_blk yields the invalid target 0 (rejected by R25).
    pub fn target_g(&self)->fixed::U256{crate::gate::calibration_target(self.hash_rate,self.t_blk).unwrap_or(fixed::U256::ZERO)}
}
const PCOL:u128=1_000_000_000_000_000_000;
pub fn to_fixed(v:U256)->fixed::U256{fixed::U256::from_be_slice(&v.to_be_bytes::<32>()).unwrap()}
pub fn from_fixed(v:&fixed::U256)->U256{U256::from_be_bytes(v.to_be_bytes())}

pub struct MinerIdentity{pub label:String,pub key:Key,pub id:Address,pub reward:Address}
pub struct Devnet{
    pub spec_bytes:Vec<u8>,
    pub genesis_hash:B256,
    pub profile:ProfileIdentity,
    pub alloc:InMemoryDB,
    pub genesis:Genesis,
    pub programs:SystemPrograms,
    pub miners:Vec<MinerIdentity>,
    pub forks:Vec<(u16,u64)>,
    /// FN-5: the cap raise scheduled beside `forks` (fork version 3), admitted with `admission::admit_with`. None by
    /// default: a devnet raises no cap unless a test schedules one.
    pub cap_raise:Option<crate::profile::CapRaise>,
    /// Recorded H_cal (synthetic for a devnet), the R25d calibration of target_g.
    pub hash_rate:u64,
}
impl Devnet{
    /// The admission request for this candidate (simnet profile on a simnet node).
    pub fn request<'a>(&'a self,local:&'a NodeLocal,timing:Option<&'a Timing>)->Request<'a>{
        Request{spec_bytes:&self.spec_bytes,alloc:&self.alloc,forks:&self.forks,kind:ProfileKind::Simnet,node:NodeKind::Simnet,local,
            calibration:Some((self.hash_rate,FactClass::SyntheticFixture)),timing,scope:crate::admission::Scope::ExperimentalDevnet}
    }
    /// Measures this candidate's timing, then runs the full gate (A02). A `Devnet` is only a
    /// candidate: nothing can boot from it except through the returned `Admitted`.
    pub fn admit(&self,local:&NodeLocal)->Result<(Admitted,Timing),String>{
        match crate::admission::measure(&self.spec_bytes,&self.alloc,1){
            Ok(timing)=>crate::admission::admit_with(&self.request(local,Some(&timing)),self.cap_raise.as_ref()).map(|a|(a,timing)).map_err(|e|e.to_string()),
            // Unmeasurable (its blocks cannot be applied): the gate still names every rule.
            Err(why)=>Err(match crate::admission::admit_with(&self.request(local,None),self.cap_raise.as_ref()){Err(e)=>format!("{e}; {why}"),Ok(_)=>why}),
        }
    }
}
pub fn miner_identity(label:&str)->MinerIdentity{
    let key=Key::devnet(&format!("miner:{label}"));let reward=Key::devnet(&format!("reward:{label}")).address();
    MinerIdentity{label:label.to_string(),id:key.address(),key,reward}
}
pub fn account_key(label:&str)->Key{Key::devnet(&format!("account:{label}"))}

/// Builds alloc → derives allocRoot/sysCodeHash → encodes GenesisSpec → validates
/// the profile schema (simnet) → builds and root-checks the genesis state. The result
/// is a candidate, not an admitted profile (see `Devnet::admit`).
pub fn build(cfg:&DevnetConfig)->Result<Devnet,String>{
    if cfg.chain_id==777001{return Err("network chainId is not a devnet".into())}
    // V3-34: a specVersion-2 (CP schema v2) devnet runs the V3 programs, which read their v3 fields from it; only an
    // explicit spec-1 devnet (`DevnetConfig::spec1`, Phase 6a) runs a spec-1 set.
    let programs=match (cfg.spec_version,cfg.spec1){
        (2,None)=>SystemPrograms::v3(),
        (1,Some(Spec1Programs::EarnIn))=>SystemPrograms::earnin(),
        (1,Some(Spec1Programs::Bonded))=>if cfg.h_end==0{SystemPrograms::continuous()}else{SystemPrograms::v1()},
        (v,p)=>return Err(format!("specVersion {v} with spec-1 programs {p:?}: a v3 devnet is DevnetConfig::new, a spec-1 one DevnetConfig::spec1")),
    };
    let mut miners:Vec<MinerIdentity>=cfg.miners.iter().map(|l|miner_identity(l)).collect();
    miners.sort_by_key(|m|m.id);
    let mut alloc=InMemoryDB::default();
    for m in &miners{ // reward accounts must be durable (nonce>0), PAY-02.
        alloc.insert_account_info(m.reward,AccountInfo{nonce:1,balance:U256::ZERO,..Default::default()});
    }
    for (label,wei) in &cfg.accounts{alloc.insert_account_info(account_key(label).address(),AccountInfo{nonce:0,balance:*wei,..Default::default()});}
    let m0_list:Vec<Member>=miners.iter().map(|m|Member{id:m.id.0.0,reward_addr:m.reward.0.0}).collect();
    let mut spec=GenesisSpec{spec_version:cfg.spec_version,chain_id:cfg.chain_id,cp:cfg.cp(),alloc_root:[0;32],m0_list,sys_code_hash:programs.sys_code_hash().0};
    let forks=vec![(1u16,0u64)];
    // Provisional identity to derive allocRoot (the root does not depend on allocRoot itself).
    let provisional=identity(&spec,&forks)?;
    let root=Genesis::derive_root(&alloc,&provisional,&programs).map_err(|e|format!("{e:?}"))?;
    spec.alloc_root=root.0;
    let profile=identity(&spec,&forks)?;
    let genesis=Genesis::build(&alloc,&profile,&programs).map_err(|e|format!("{e:?}"))?;
    let spec_bytes=gs::encode(&spec);
    Ok(Devnet{genesis_hash:B256::from(profile.genesis_hash()),spec_bytes,profile,alloc,genesis,programs,miners,forks,cap_raise:None,hash_rate:cfg.hash_rate})
}
fn identity(spec:&GenesisSpec,forks:&[(u16,u64)])->Result<ProfileIdentity,String>{
    let bytes=gs::encode(spec);let hash=gs::genesis_hash(&bytes);
    ProfileIdentity::validate(&bytes,&hash,spec.chain_id,ProfileKind::Simnet,NodeKind::Simnet,forks).map_err(|e|format!("{e:?}"))
}
/// alloc JSON: {"0x..":{"nonce":n,"balance":"decimal"[,"code":"0x..","storage":{"0xslot":"0xvalue"}]}}.
/// `code` and `storage` appear only for an account that has them (a cutover genesis carries contracts), so
/// a balance-only allocation serializes exactly as before.
pub fn alloc_to_json(db:&InMemoryDB)->serde_json::Value{
    let mut m=serde_json::Map::new();
    let mut keys:Vec<_>=db.accounts.keys().collect();keys.sort();
    for a in keys{
        let acct=&db.accounts[a];let i=&acct.info;
        let mut v=serde_json::json!({"nonce":i.nonce,"balance":i.balance.to_string()});
        if let Some(code)=i.code.as_ref().filter(|c|!c.is_empty()){v["code"]=serde_json::json!(format!("0x{}",hex(code.original_byte_slice())));}
        let mut slots:Vec<_>=acct.storage.iter().filter(|(_,x)|!x.is_zero()).collect();slots.sort();
        if !slots.is_empty(){v["storage"]=serde_json::Value::Object(slots.into_iter().map(|(k,x)|(format!("{k:#x}"),serde_json::json!(format!("{x:#x}")))).collect());}
        m.insert(format!("{a:#x}"),v);
    }
    serde_json::Value::Object(m)
}
fn hex(b:&[u8])->String{b.iter().map(|x|format!("{x:02x}")).collect()}
pub fn alloc_from_json(v:&serde_json::Value)->Result<InMemoryDB,String>{
    let mut db=InMemoryDB::default();
    for (k,x) in v.as_object().ok_or("alloc object")?{
        let a:Address=k.parse().map_err(|_|"address")?;
        if x.as_object().is_some_and(|o|o.keys().any(|k|!["nonce","balance","code","storage"].contains(&k.as_str()))){return Err("alloc account: unknown field".into())}
        let nonce=x["nonce"].as_u64().ok_or("nonce")?;let balance:U256=x["balance"].as_str().ok_or("balance")?.parse().map_err(|_|"balance value")?;
        let mut info=AccountInfo{nonce,balance,..Default::default()};
        if let Some(code)=x.get("code"){
            let text=code.as_str().ok_or("code")?.strip_prefix("0x").ok_or("code prefix")?;
            if text.is_empty()||text.len()%2!=0{return Err("code hex".into())}
            let bytes=(0..text.len()).step_by(2).map(|i|u8::from_str_radix(&text[i..i+2],16)).collect::<Result<Vec<u8>,_>>().map_err(|_|"code hex")?;
            let code=revm::primitives::Bytecode::new_raw(bytes.into());info.code_hash=code.hash_slow();info.code=Some(code);
        }
        db.insert_account_info(a,info);
        if let Some(storage)=x.get("storage"){
            for (slot,value) in storage.as_object().ok_or("storage")?{
                let slot:U256=slot.parse().map_err(|_|"storage slot")?;let value:U256=value.as_str().ok_or("storage value")?.parse().map_err(|_|"storage value")?;
                if value.is_zero(){return Err("storage: zero values are not listed".into())}
                db.insert_account_storage(a,slot,value).map_err(|_|"storage")?;
            }
        }
    }
    Ok(db)
}
