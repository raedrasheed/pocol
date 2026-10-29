//! A02 boot admission (governance.md R1–R26). The accepted LP3 evaluator `gate::check`
//! (shared byte-for-byte with lp1-node) decides every rule; this module only assembles
//! its inputs from the actual artifacts (GenesisSpec bytes, alloc, the bundled system
//! programs, the node's local parameters, a recorded calibration and timing measurement)
//! and then builds the genesis. `Admitted` is the only bootable configuration and has no
//! other constructor.
//!
//! Fact provenance (each is reported; none is a caller-supplied "ok"):
//! - identity (R23/R24a): allocRoot re-derived from the alloc, sysCodeHash of the bundled
//!   programs, alloc balances. Exact builder outputs.
//! - SYS slots, keys, accounts (R18): the P7 derivation `resources::derive` over the CP,
//!   raised to at least the system slots measured in the constructed genesis. The user
//!   context is charged its full allowance (SYS_KEYS_USER_MAX keys, 16 accounts).
//! - SYS gas (R18): `GAS_ENVELOPE`, the per-call maxima measured by the worst-case fixture
//!   `resources::tests::sys_gas_envelope_fixture`, composed into the worst call sequence of
//!   each phase class. Applicable only inside `GAS_DOMAIN`; gas is assumed monotone in the
//!   domain fields (open derivation obligation, A13). Outside the domain the facts are absent.
//! - timing (R14/R19): a `Timing` record measured on this machine for this exact profile
//!   (`measure`). Absent or for another genesis: R14/R19 are violated.
//! - R21: (K_hist + side) snapshots of a state at SLOT_CAP/ACCT_CAP/SYS_SLOT_MAX, checked
//!   arithmetic. The simnet retentionStress exception is the gate's own (governance.md R21).
//! - calibration (R25d): the recorded H_cal and its class; a devnet records its chosen rate
//!   as a synthetic fixture.
//! - RpcGuard (R26): `RpcFacts`. Only fields the node enforces are present; an absent field
//!   enters the gate as 0 and is never given a nominal value (`scope`).
//!
//! Scope (D-A02-SCOPE): `Scope::Specification` admits only when no `Condition` remains (an
//! absent or fixture RpcGuard field, a synthetic calibration, a timing measurement that is not
//! a bound, an assumed gas monotonicity, the gate's deferred LP3 items, the R21 exception).
//! `Scope::ExperimentalDevnet` admits a simnet profile on a simnet node with those conditions
//! reported, and waives only the R26 predicates that read absent fields.
//!
//! No bypass: outside this crate a `Genesis` can only be derived from, never built for,
//! an unadmitted identity, and `Admitted` has no public constructor.
//! ```
//! use pocol_lp4_execution::{apply::{Genesis,SystemPrograms},profile::ProfileIdentity};
//! fn ok(a:&revm::InMemoryDB,p:&ProfileIdentity){let _=Genesis::derive_root(a,p,&SystemPrograms::v1());}
//! ```
//! ```compile_fail,E0624
//! use pocol_lp4_execution::{apply::{Genesis,SystemPrograms},profile::ProfileIdentity};
//! fn bypass(a:&revm::InMemoryDB,p:&ProfileIdentity){let _=Genesis::build(a,p,&SystemPrograms::v1());}
//! ```
//! ```compile_fail,E0451
//! fn forge(p:pocol_lp4_execution::profile::ProfileIdentity,g:pocol_lp4_execution::apply::Genesis)->pocol_lp4_execution::admission::Admitted{
//!     pocol_lp4_execution::admission::Admitted{profile:p,genesis:g,spec_bytes:vec![],local:pocol_lp4_execution::admission::NodeLocal::single_host(),report:todo!()}
//! }
//! ```
mod scope;
pub use scope::{Condition,RpcFacts,Scope,Waiver,RPC_FIELDS,R26_PREDICATES,r26_partial};
use revm::{InMemoryDB,primitives::{AccountInfo,Address,B256,Bytecode,Bytes,U256,keccak256}};
use crate::{apply::{self,BlockInput,Genesis,ParentResult,Proposer,SystemPrograms},
    gate::{self,Calibration,FactClass,GateProfile,IdentityFacts,LocalParams,Phase,PhaseFacts,ResourceFacts},
    genesis_spec as gs,hashes::sha256,param_gate::RpcLimits,profile::{ConsensusParams,NodeKind,ProfileIdentity,ProfileKind},resources};

/// governance.md constants entering R8/R11/R21.
pub const R_KEEP:u32=2048;pub const K_FINAL:u32=60;pub const K_VIEW:u32=12;pub const HIST_TRIE_BUDGET:u64=1_717_986_918;
/// pocold's frame limit (net::MAX_FRAME, 256 KiB) applies to every gossip message type.
pub const DEFAULT_GOSSIP:[u32;6]=[256<<10;6];
/// User-context SYS allowance (execution.md, D50/D54/D70): 16 accounts; 0 while closing.
pub const USER_SYS_ACCOUNTS:u32=16;

/// Node-local inputs. Times are whole seconds; Φ = 2φ (R1).
#[derive(Clone,Debug)]
pub struct NodeLocal{
    pub delta:u32,pub phi:u32,pub t_exec:u32,pub delta_rel:u32,pub gamma:u32,
    /// Full state snapshots retained below the head, and the cap on side-branch blocks holding state.
    pub k_hist:u32,pub side_max:u32,
    /// tx, template, block, share, evidence1, evidence2 byte limits.
    pub gossip:[u32;6],
    /// R21 exception; the gate honours it only for a simnet profile on a simnet node.
    pub retention_stress:bool,
    pub rpc:RpcFacts,
}
impl NodeLocal{
    /// D-A02-PROFILE (b): one host, one clock, so φ=Φ=0 exactly; Δ=1, t_exec=1, Δ_rel=5, Γ=5.
    pub fn single_host()->Self{Self{delta:1,phi:0,t_exec:1,delta_rel:5,gamma:5,k_hist:32,side_max:8,gossip:DEFAULT_GOSSIP,retention_stress:true,rpc:RpcFacts::fixture(RpcLimits::default())}}
    /// P11 (governance.md): Δ=1, φ=1 (Φ=2), t_exec=1, Δ_rel=8, Γ=5. For multi-host use.
    pub fn p11()->Self{Self{delta:1,phi:1,t_exec:1,delta_rel:8,gamma:5,k_hist:32,side_max:8,gossip:DEFAULT_GOSSIP,retention_stress:false,rpc:RpcFacts::fixture(RpcLimits::default())}}
    /// Φ, saturating so that an overflowing φ fails R1 instead of wrapping.
    pub fn capital_phi(&self)->u32{self.phi.checked_mul(2).unwrap_or(u32::MAX)}
}

/// Measured SYS gas per call name: the largest value in any phase class over the
/// scenarios of `resources::tests::sys_gas_envelope_fixture`, rounded up to 1000. That
/// test fails if a measurement ever exceeds an entry, so the table cannot silently go stale.
pub const GAS_ENVELOPE:&[(&str,u64)]=&[("beginBlock",926_000),("closeBlock",53_000),("finalize",32_000),("invalidate",7_000),("markClose",2_655_000),
    ("pruneShares",2_781_000),("punish",81_000),("recordResult",26_000),("recordShares",13_216_000),("settle",4_305_000),("settleFinal",4_230_000)];
/// Gas-driving CP fields of the fixture (loop bounds in PoColSystemV1: S_max+1 owners,
/// M_max records, RET retirements, D pots, E_max evidence; SWEEP_SCAN/SWEEP_MAX are
/// capped at 64/16 in the program and were at those caps).
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct GasDomain{pub s_max:u16,pub m_max:u16,pub d:u16,pub e_max:u8,pub ret:u32}
pub const GAS_DOMAIN:GasDomain=GasDomain{s_max:256,m_max:256,d:16,e_max:4,ret:128};

/// Worst SYSTEM call sequence of each phase class (apply_block phase 0 and 2).
fn phase_calls(cp:&ConsensusParams,class:u8)->Vec<(&'static str,u64)>{
    let mut v:Vec<(&'static str,u64)>=Vec::new();
    for _ in 0..cp.E_max{v.push(("invalidate",1));v.push(("punish",1));}
    for n in ["markClose","pruneShares","recordShares","recordResult","settle","beginBlock"]{v.push((n,1))}
    if class==2{v.push(("finalize",1));v.push(("settleFinal",u64::from(cp.D).saturating_sub(1)));}else{v.push(("closeBlock",1));}
    v
}
fn in_gas_domain(cp:&ConsensusParams)->bool{
    let d=GAS_DOMAIN;cp.S_max<=d.s_max&&cp.M_max<=d.m_max&&cp.D<=d.d&&cp.E_max<=d.e_max&&cp.RET<=d.ret
}
fn envelope(name:&str)->Option<u64>{GAS_ENVELOPE.iter().find(|(n,_)|*n==name).map(|x|x.1)}
fn phase_gas(cp:&ConsensusParams,class:u8)->Option<Vec<u64>>{
    if !in_gas_domain(cp){return None}
    let mut out=Vec::new();
    for (name,count) in phase_calls(cp,class){let g=envelope(name)?;for _ in 0..count{out.push(g)}}
    Some(out)
}

/// Timing of this profile's blocks on this machine (`measure`). Bound to one genesis.
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Timing{pub genesis_hash:[u8;32],/** Largest apply time per phase class (ordinary, closing, final), ms. */pub phase_ms:[u64;3],pub samples:u32,
    /// The blocks also went through a node store (`measure_on_store`), each committed durably: the timing of a
    /// node that keeps its states there (C-1 store mode).
    pub stored:bool}
impl Timing{
    pub fn to_json(&self)->serde_json::Value{serde_json::json!({"genesisHash":crate::hex::encode(&self.genesis_hash),"phaseMs":self.phase_ms,"samples":self.samples,"stored":self.stored,
        "method":if self.stored{"apply_block on a scratch copy of the genesis, and apply_block_on through a scratch node store with each block's nodes committed durably; the larger time per phase; every block carries a GAS_LIMIT compute loop; closing and final reached by timestamps"}
            else{"apply_block on a scratch copy of the genesis; every block carries a GAS_LIMIT compute loop; closing and final reached by timestamps"}})}
    pub fn from_json(v:&serde_json::Value)->Option<Self>{
        let h=crate::hex::decode(v["genesisHash"].as_str()?)?;let p=v["phaseMs"].as_array()?;
        if p.len()!=3{return None}
        Some(Self{genesis_hash:h.try_into().ok()?,phase_ms:[p[0].as_u64()?,p[1].as_u64()?,p[2].as_u64()?],samples:u32::try_from(v["samples"].as_u64()?).ok()?,
            stored:v["stored"].as_bool().unwrap_or(false)})
    }
    /// The timing of a store-mode node: per phase class the larger of the in-memory measurement and the one
    /// through its node store (both for this genesis).
    pub fn with_store(&self,store:&Timing)->Result<Timing,String>{
        if store.genesis_hash!=self.genesis_hash{return Err("timing: the store measurement is for another genesis".into())}
        let mut phase_ms=self.phase_ms;for (m,s) in phase_ms.iter_mut().zip(store.phase_ms){*m=(*m).max(s);}
        Ok(Timing{genesis_hash:self.genesis_hash,phase_ms,samples:self.samples.saturating_add(store.samples),stored:true})
    }
}

pub struct Request<'a>{
    pub spec_bytes:&'a [u8],
    pub alloc:&'a InMemoryDB,
    pub forks:&'a [(u16,u64)],
    pub kind:ProfileKind,
    pub node:NodeKind,
    pub local:&'a NodeLocal,
    /// Recorded H_cal and its provenance class.
    pub calibration:Option<(u64,FactClass)>,
    pub timing:Option<&'a Timing>,
    pub scope:Scope,
}

/// Every violated rule in numeric order, the gate's missing-fact entries, and notes
/// naming the cause of each. Rendered as boot error −32018.
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Rejection{pub rules:Vec<String>,pub missing:Vec<String>,pub notes:Vec<String>}
impl std::fmt::Display for Rejection{
    fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{
        write!(f,"-32018 ParamGate rejected [{}]",self.rules.join(", "))?;
        if !self.missing.is_empty(){write!(f,"; missing {}",self.missing.join(", "))?}
        if !self.notes.is_empty(){write!(f,"; {}",self.notes.join("; "))?}
        Ok(())
    }
}
impl std::error::Error for Rejection{}
fn terminal(rule:&str,note:String)->Rejection{Rejection{rules:vec![rule.into()],missing:vec![],notes:vec![note]}}

/// What admitted the profile: fact classes, the deferred LP3 items and notes.
#[derive(Clone,Debug)]
pub struct Report{
    pub binding:[u8;32],
    pub resource_class:Option<FactClass>,pub identity_class:Option<FactClass>,pub calibration_class:Option<FactClass>,
    pub deferred:[&'static str;3],
    /// R21 bound above HIST_TRIE_BUDGET, admitted only by the simnet retentionStress exception.
    pub r21_exception:bool,
    pub historical_bytes:u64,
    pub notes:Vec<String>,
    pub scope:Scope,
    /// Rules partly not evaluated (ExperimentalDevnet only).
    pub waivers:Vec<Waiver>,
    /// Why this admission is not specification evidence; empty only for `Scope::Specification`.
    pub conditions:Vec<Condition>,
}

/// The only bootable configuration. Fields are private; `admit` is the only constructor.
pub struct Admitted{profile:ProfileIdentity,genesis:Genesis,spec_bytes:Vec<u8>,local:NodeLocal,report:Report}
impl Admitted{
    pub fn profile(&self)->&ProfileIdentity{&self.profile}
    pub fn genesis(&self)->&Genesis{&self.genesis}
    pub fn spec_bytes(&self)->&[u8]{&self.spec_bytes}
    pub fn genesis_hash(&self)->[u8;32]{self.profile.genesis_hash()}
    pub fn local(&self)->&NodeLocal{&self.local}
    pub fn report(&self)->&Report{&self.report}
    pub fn scope(&self)->Scope{self.report.scope}
    /// True only for a `Scope::Specification` admission, which has no waiver and no condition.
    pub fn specification_compliant(&self)->bool{self.report.scope==Scope::Specification&&self.report.waivers.is_empty()&&self.report.conditions.is_empty()}
}

/// Identity used only to derive builder facts: the derived allocRoot depends on the CP,
/// M_0 and programs, not on profileKind, nodeKind or the fork schedule.
fn fact_identity(bytes:&[u8],hash:&[u8;32],chain_id:u64)->Result<ProfileIdentity,String>{
    let (k,n)=if chain_id==777001{(ProfileKind::Network,NodeKind::Node)}else{(ProfileKind::Synthetic,NodeKind::Simnet)};
    ProfileIdentity::validate(bytes,hash,chain_id,k,n,&[(1,0)]).map_err(|e|format!("{e:?}"))
}
fn sat_u32(v:u64)->u32{u32::try_from(v).unwrap_or(u32::MAX)}
fn sat_u64(v:u128)->u64{u64::try_from(v).unwrap_or(u64::MAX)}
/// R21: (k_hist + side_max) full snapshots (pocold keeps full states, no trie sharing) of a
/// state at ACCT_CAP + 5 accounts and SLOT_CAP + SYS_SLOT_MAX slots, by the container model
/// `resources::memory` (A13). Saturates on overflow, which then fails R21.
pub fn historical_bytes(cp:&ConsensusParams,local:&NodeLocal)->u64{
    let a=u128::from(cp.ACCT_CAP)+5;let per=resources::memory::snapshot(a,u128::from(cp.SLOT_CAP)+u128::from(cp.SYS_SLOT_MAX),a+2).unwrap_or(u128::MAX);
    sat_u64((u128::from(local.k_hist)+u128::from(local.side_max)).saturating_mul(per))
}
fn digest(tag:&str,parts:&[&[u8]])->[u8;32]{
    let mut b=tag.as_bytes().to_vec();for p in parts{b.extend_from_slice(&(p.len() as u64).to_be_bytes());b.extend_from_slice(p);}sha256(&b)
}

/// Runs the full gate on the actual artifacts and, only if it accepts, builds the genesis.
pub fn admit(r:&Request<'_>)->Result<Admitted,Rejection>{
    // validation.md:2065: the caps-test build constants are simnet only.
    if cfg!(feature="caps-test")&&(r.kind!=ProfileKind::Simnet||r.node!=NodeKind::Simnet){
        return Err(terminal("caps-test","the caps-test build (validation.md:2065) admits only a simnet profile on a simnet node".into()))}
    // Decode, hash binding and CP schema (R24a/e). Without them nothing else is evaluable.
    if r.spec_bytes.len()>gs::MAX_SPEC_BYTES{return Err(terminal("R24","GenesisSpec exceeds MAX_SPEC_BYTES".into()))}
    let hash=gs::genesis_hash(r.spec_bytes);
    let spec=gs::verify_hash(r.spec_bytes,&hash).map_err(|e|terminal("R24",format!("GenesisSpec decode: {e:?}")))?;
    let cp=ConsensusParams::from_values(&spec.cp).map_err(|e|terminal("R24",format!("CP schema: {e:?}")))?;
    let mut notes=Vec::new();
    let programs=SystemPrograms::v1();

    // Builder facts: allocRoot and system slots of the constructed genesis (before the
    // allocRoot comparison, which the gate makes as R24).
    let (root,genesis_slots)=match fact_identity(r.spec_bytes,&hash,spec.chain_id).and_then(|p|
        crate::genesis_builder::construct_state(r.alloc,&p,&programs.codes).map_err(|e|format!("genesis construction: {e:?}"))){
        Ok((root,state))=>(Some(root),Some(resources::sys_slots(&state))),
        Err(e)=>{notes.push(e);(None,None)}
    };

    let l=r.local;let big_phi=l.capital_phi();
    let m=crate::timing::metrics(u64::from(l.delta),u64::from(big_phi),u64::from(l.t_exec),u64::from(l.delta_rel),u64::from(cp.dFbWait));
    let gp=GateProfile{id:"pocold-admission-v1".into(),cp:cp.clone(),genesis_pre:r.spec_bytes.to_vec(),genesis_hash:hash,chain_id:spec.chain_id,kind:r.kind,forks:r.forks.to_vec(),
        local:LocalParams{delta:l.delta,phi:l.phi,capital_phi:big_phi,t_exec:l.t_exec,delta_rel:l.delta_rel,gamma:l.gamma,x_h:sat_u64(m.x_h),x_w:sat_u64(m.x_w),
            r_keep:R_KEEP,k_hist:l.k_hist,k_final:K_FINAL,k_view:K_VIEW,hist_trie_budget:HIST_TRIE_BUDGET,gossip:l.gossip,retention_stress:l.retention_stress},
        rpc:l.rpc.values().clone()};
    let binding=gp.binding();
    let model=gate::RESOURCE_MODEL.to_string();

    let identity=root.map(|root|{
        let mut balances:Vec<(Address,U256)>=r.alloc.accounts.iter().filter_map(|(a,x)|x.info().map(|i|(*a,i.balance))).collect();balances.sort();
        let flat:Vec<u8>=balances.iter().flat_map(|(a,b)|[a.as_slice(),&b.to_be_bytes::<32>()].concat()).collect();
        IdentityFacts{profile_binding:binding,model:model.clone(),class:FactClass::Measurement,artifact:digest("PoColIdentityFacts",&[&root.0,&programs.sys_code_hash().0,&flat]),
            alloc_root:root.0,sys_code_hash:programs.sys_code_hash().0,
            alloc_balances:balances.iter().map(|(_,b)|crate::devnet::to_fixed(*b)).collect()}
    });

    let timing=r.timing.filter(|t|t.genesis_hash==hash);
    if r.timing.is_none(){notes.push("timing: no measurement recorded (R14/R19)".into())}
    else if timing.is_none(){notes.push("timing: measurement recorded for another genesis (R14/R19)".into())}
    let b=resources::derive(&cp);
    let live=sat_u32(b.slot_min.max(genesis_slots.unwrap_or(0)));
    if !in_gas_domain(&cp){notes.push(format!("SYS gas: CP outside the measured domain {:?} (R18)",GAS_DOMAIN))}
    let phase=|class:u8,p:Phase,keys:u64,accts:u64|PhaseFacts{phase:p,system_keys:sat_u32(keys),user_keys:if class==0{cp.SYS_KEYS_USER_MAX}else{0},
        system_accounts:sat_u32(accts),user_accounts:if class==0{USER_SYS_ACCOUNTS}else{0},live_slots:live,
        call_gas:phase_gas(&cp,class).unwrap_or_default(),duration_ms:timing.map_or(u64::MAX,|t|t.phase_ms[class as usize]),
        retired_records:if class==1&&cp.RET<=GAS_DOMAIN.ret{cp.RET}else{0}};
    let phases=vec![phase(0,Phase::Ordinary,b.keys_normal,b.accts_normal),phase(1,Phase::Closing,b.keys_closing,b.accts_closing),phase(2,Phase::Final,b.keys_final,b.accts_final)];
    let hist=historical_bytes(&cp,l);
    let measured=timing.map(|t|t.phase_ms).unwrap_or([u64::MAX;3]);
    let resource=ResourceFacts{profile_binding:binding,model:model.clone(),class:FactClass::Measurement,
        artifact:digest("PoColResourceFacts",&[&binding,format!("{phases:?}").as_bytes(),&hist.to_be_bytes(),&measured.iter().flat_map(|v|v.to_be_bytes()).collect::<Vec<u8>>()]),
        validate_p99_ms:measured.into_iter().max().unwrap_or(u64::MAX),historical_trie_bytes:hist,retained_blocks:u64::from(l.k_hist),phases};

    let calibration=r.calibration.map(|(rate,class)|Calibration{profile_binding:binding,model:model.clone(),class,hashes_per_second:rate,duration_seconds:1,
        artifact:digest("PoColCalibration",&[&rate.to_be_bytes(),&cp.T_blk.to_be_bytes()])});
    if r.calibration.is_none(){notes.push("calibration: no H_cal recorded (R25d)".into())}

    let mut report=gate::check(&gp,Some(&resource),identity.as_ref(),calibration.as_ref(),r.node);
    let hist_exception=hist>HIST_TRIE_BUDGET;
    let conditions=conditions(&cp,r,l,timing,hist_exception,&report.deferred);
    let mut waivers=Vec::new();
    match r.scope{
        Scope::ExperimentalDevnet=>{
            // Only the intended kind: a Simnet profile on a Simnet node (not Network, not Synthetic).
            if r.kind!=ProfileKind::Simnet||r.node!=NodeKind::Simnet{
                return Err(terminal("R22","the experimental-devnet scope admits only a simnet profile on a simnet node".into()))}
            if !l.rpc.absent().is_empty(){
                let (w,failed)=r26_partial(&l.rpc);
                if failed.is_empty(){report.violations.retain(|v|v!="R26")}else{notes.push(format!("R26: enforced RpcGuard fields fail {failed:?}"))}
                waivers.push(w);
            }
        }
        Scope::Specification=>{
            if !conditions.is_empty(){
                let mut rules=report.violations.clone();rules.extend(conditions.iter().flat_map(|c|c.rules.iter().map(|x|x.to_string())));
                rules.sort_by_key(|x|x[1..].parse::<u32>().unwrap_or(u32::MAX));rules.dedup();
                let mut missing:Vec<String>=report.missing.iter().map(|s|s.to_string()).collect();missing.extend(conditions.iter().map(|c|format!("specification:{}",c.id)));
                notes.extend(conditions.iter().map(|c|c.detail.clone()));
                return Err(Rejection{rules,missing,notes})
            }
        }
    }
    if !report.accepted(){return Err(Rejection{rules:report.violations,missing:report.missing.iter().map(|s|s.to_string()).collect(),notes})}

    // Accepted: the identity and genesis that boot uses.
    let profile=ProfileIdentity::validate(r.spec_bytes,&hash,spec.chain_id,r.kind,r.node,r.forks).map_err(|e|terminal(match e{crate::profile::ProfileError::R22=>"R22",_=>"R24"},format!("{e:?}")))?;
    let genesis=Genesis::build(r.alloc,&profile,&programs).map_err(|e|terminal("R24",format!("genesis: {e:?}")))?;
    let r21_exception=hist_exception;
    if r21_exception{notes.push(format!("R21: retained-state bound {hist} B exceeds {HIST_TRIE_BUDGET} B; admitted by the simnet retentionStress exception"))}
    Ok(Admitted{profile,genesis,spec_bytes:r.spec_bytes.to_vec(),local:l.clone(),
        report:Report{binding,resource_class:report.resource_class,identity_class:report.identity_class,calibration_class:report.calibration_class,deferred:report.deferred,r21_exception,historical_bytes:hist,notes,
            scope:r.scope,waivers,conditions}})
}

/// Every reason the admitted facts are not specification evidence (D-A02-SCOPE).
fn conditions(cp:&ConsensusParams,r:&Request<'_>,l:&NodeLocal,timing:Option<&Timing>,hist_exception:bool,deferred:&[&'static str;3])->Vec<Condition>{
    let mut c=Vec::new();
    let mut add=|id:&'static str,rules:&'static [&'static str],detail:String|c.push(Condition{id,rules,detail});
    if l.rpc.is_fixture(){add("rpc-fixture",&["R26"],"RpcGuard: the values are a fixture, not limits this node enforces".into())}
    if !l.rpc.absent().is_empty(){add("rpc-absent",&["R26"],format!("RpcGuard: mechanisms this node does not enforce: {}",l.rpc.absent().join(", ")))}
    if matches!(r.calibration,Some((_,FactClass::SyntheticFixture))){add("calibration-synthetic",&["R25"],"calibration: H_cal is a synthetic fixture (devnet --work-bits), not a measured hash rate".into())}
    if let Some(t)=timing{add("timing-not-a-bound",&["R14","R19"],format!("timing: {} apply samples of the synthetic GAS_LIMIT compute workload on this host{}; not a p99 or worst-case bound (A13-DERIVATION §4)",t.samples,
        if t.stored{", in memory and through a scratch node store committed durably per block (store mode), the larger per phase"}else{""}))}
    if in_gas_domain(cp){add("gas-monotonicity-assumed",&["R18"],"SYS gas: GAS_ENVELOPE holds at the fixture's maxima; monotonicity in the domain fields is assumed (A13-DERIVATION §4)".into())}
    for d in deferred{add(match *d{"LP4:general-R21"=>"gate-deferred-general-R21","LP4:general-R18"=>"gate-deferred-general-R18",_=>"gate-deferred-caps64"},
        if d.ends_with("R21"){&["R21"]}else{&["R18"]},format!("gate: deferred LP3 obligation {d}"))}
    if hist_exception{add("r21-retention-stress",&["R21"],"R21: admitted only by the simnet retentionStress exception".into())}
    c
}

/// Calibration code: JUMPDEST; PUSH1 0; JUMP — loops until the transaction's gas is spent.
const SPIN:[u8;4]=[0x5b,0x60,0x00,0x56];
fn scratch_address(tag:&str)->Address{Address::from_slice(&keccak256(tag.as_bytes())[12..])}

/// Measures R14/R19 timing for this exact profile: blocks are applied to a scratch copy of
/// its genesis (nothing is persisted), each carrying a transaction that spends GAS_LIMIT
/// in a compute loop, with full parent results. Ordinary blocks come first; then
/// timestamps at T_END drive closing (RET records per block) and the final block.
/// Every block is applied `repeats` times; the largest time per phase class is recorded.
pub fn measure(spec_bytes:&[u8],alloc:&InMemoryDB,repeats:u32)->Result<Timing,String>{measure_with_gas(spec_bytes,alloc,repeats).map(|(t,_)|t)}
/// `measure` for a node that keeps its states in a node store (C-1 store mode): the same blocks, each applied
/// with `apply_block_on` to the parent version read in place, its trie updated, its root computed and its nodes
/// committed to `store` (durably for pocold's state.redb), all inside the timed span. `store` should be a
/// scratch store on the node's own disk; every version the measurement pins is released at the end.
pub fn measure_on_store(spec_bytes:&[u8],alloc:&InMemoryDB,repeats:u32,store:&dyn crate::node_trie::TrieStore)->Result<Timing,String>{
    measure_run(spec_bytes,alloc,repeats,Some(store)).map(|(t,_,_)|t)}
/// `measure`, also returning the gas each sampled block used (in height order), so tests can check
/// the workload of every sample independently of measure's own guard (PR #27 review R5-2).
pub(crate) fn measure_with_gas(spec_bytes:&[u8],alloc:&InMemoryDB,repeats:u32)->Result<(Timing,Vec<u64>),String>{measure_run(spec_bytes,alloc,repeats,None).map(|(t,g,_)|(t,g))}
/// The measurement in memory (`store` None) or through a node store; also the state root of each block, so
/// tests can check that both paths ran the same blocks.
pub(crate) fn measure_run(spec_bytes:&[u8],alloc:&InMemoryDB,repeats:u32,store:Option<&dyn crate::node_trie::TrieStore>)->Result<(Timing,Vec<u64>,Vec<B256>),String>{
    let mut gas_log=Vec::new();let mut roots=Vec::new();
    let hash=gs::genesis_hash(spec_bytes);let spec=gs::verify_hash(spec_bytes,&hash).map_err(|e|format!("{e:?}"))?;
    let profile=fact_identity(spec_bytes,&hash,spec.chain_id)?;let cp=profile.cp().clone();
    let genesis=Genesis::build(alloc,&profile,&SystemPrograms::v1()).map_err(|e|format!("{e:?}"))?;
    let mut state=genesis.state().clone();
    // Fund the calibration sender by moving balance (total supply unchanged).
    let key=crate::txsign::Key::devnet("admission-calibration");let sender=key.address();let spin=scratch_address("pocol-admission-spin");
    // Every sampled block carries the GAS_LIMIT workload at the base fee baseFee0. apply_block takes the
    // base fee as an input and the work measured does not depend on it; under the London rule full
    // blocks would compound it by 1/8 per block past any fixed fee cap, and later samples (closing,
    // final) would then be empty blocks (PR #27 review).
    let base=crate::devnet::from_fixed(&cp.baseFee0);let max_fee=base.max(U256::from(1u64));
    let records=u64::try_from(spec.m0_list.len()).unwrap_or(u64::MAX);
    let blocks=2+records.div_ceil(u64::from(cp.RET.max(1)))+u64::from(cp.D)+4;let need=max_fee.saturating_mul(U256::from(cp.GAS_LIMIT)).saturating_mul(U256::from(blocks));
    let reserved:Vec<Address>=(0..5).map(apply::program_address).chain([crate::pay_address(),crate::pay::escrow_address(),apply::system_address()]).collect();
    let donor=state.accounts.iter().filter(|(a,x)|!reserved.contains(a)&&x.info().is_some()).max_by_key(|(_,x)|x.info.balance).map(|(a,_)|*a)
        .filter(|a|state.accounts[a].info.balance>=need).ok_or("calibration: no alloc account can fund the compute transactions")?;
    state.accounts.get_mut(&donor).unwrap().info.balance-=need;
    state.insert_account_info(sender,AccountInfo{balance:need,..Default::default()});
    let code=Bytecode::new_raw(Bytes::copy_from_slice(&SPIN));state.insert_account_info(spin,AccountInfo{code_hash:code.hash_slow(),code:Some(code),..Default::default()});
    let members:Vec<Address>=spec.m0_list.iter().map(|m|Address::from(m.id)).collect();
    if members.is_empty(){return Err("calibration: empty M_0".into())}
    let mut prev:Option<ParentResult>=None;let mut nonce=0u64;let mut phase_ms=[0u64;3];let mut samples=0u32;let mut seen=[false;3];
    // Store mode: the prepared genesis as the first stored version, read in place from then on.
    let hashes:std::collections::HashMap<U256,B256>=[(U256::ZERO,B256::from(hash))].into_iter().collect();
    let mut stored=match store{
        Some(s)=>{let mut b=crate::node_trie::Batch::default();let t=crate::node_trie::NodeTrie::build(&state,s,&mut b);s.commit(&b,t.world)?;
            Some((s,t,crate::state_summary::Summary::scan(&state,cp.B_code_max)))}
        None=>None};
    for h in 1..=blocks.min(cp.H_END){
        let ts=if h<=2{spec_ts(&cp,h)}else{cp.T_END.max(cp.T_CLOSE).saturating_add(h)};
        let tx=crate::txsign::Tx{chain_id:spec.chain_id,nonce,max_priority:U256::ZERO,max_fee,gas:cp.GAS_LIMIT,to:Some(spin),value:U256::ZERO,data:Bytes::new()};nonce+=1;
        let txs=vec![tx.sign_1559(&key)];
        let input=BlockInput{height:h,timestamp:ts,base_fee:base,gas_limit:cp.GAS_LIMIT,prevrandao:B256::ZERO,proposer:Proposer::Fallback,parent_result:prev.clone(),
            evidence:vec![],evidence_raw:&[],transactions:&txs,ancestors:vec![(0,B256::from(hash))],cum_log_bytes:0};
        let ms=|t0:std::time::Instant|u64::try_from(t0.elapsed().as_millis()).unwrap_or(u64::MAX).max(1);
        let mut worst=0u64;
        let (gas_used,is_final,closing)=if let Some((s,trie,sum))=&mut stored{
            let view=crate::node_trie::TrieState{store:*s,trie:*trie,block_hashes:&hashes};let mut out=None;
            for _ in 0..repeats.max(1){
                let t0=std::time::Instant::now();
                let mut run=apply::apply_block_on(&view,sum,&genesis,&profile,&input).map_err(|e|format!("calibration h={h}: {e:?}"))?;
                let child=crate::view::Overlay::from_layer(&view,std::mem::take(&mut run.layer));
                let mut b=crate::node_trie::Batch::default();let next=trie.update(*s,&mut b,&child,&run.writes);let root=next.root(*s,&b);
                worst=worst.max(ms(t0));samples+=1;out=Some((run,next,b,root));
            }
            // The block's nodes are committed once, durably, inside its timed span.
            let (run,next,b,root)=out.unwrap();let t0=std::time::Instant::now();s.commit(&b,next.world)?;worst=worst.saturating_add(ms(t0));
            if let Some(old)=trie.world{s.unpin(&old)?;}
            *trie=next;*sum=run.summary;roots.push(root);(run.gas_used,run.is_final,run.facts.closing)
        }else{
            let mut out=None;
            for _ in 0..repeats.max(1){
                let t0=std::time::Instant::now();let o=apply::apply_block(&state,&genesis,&profile,&input).map_err(|e|format!("calibration h={h}: {e:?}"))?;
                worst=worst.max(ms(t0));samples+=1;out=Some(o);
            }
            let o=out.unwrap();roots.push(o.state_root);let r=(o.gas_used,o.is_final,o.facts.closing);state=o.state;r
        };
        gas_log.push(gas_used);
        if gas_used<cp.GAS_LIMIT{return Err(format!("calibration h={h}: the block used {gas_used} gas, not the GAS_LIMIT workload"))}
        let class=if is_final{2}else if closing{1}else{0};
        phase_ms[class]=phase_ms[class].max(worst);seen[class]=true;
        let shares:Vec<(u64,Address)>=(0..u64::from(cp.S_max)).map(|i|(i+1,members[(i as usize+h as usize)%members.len()])).collect();
        prev=Some(ParentResult{block_hash:B256::from(U256::from(h)),winner:members[h as usize%members.len()],shares});
        if is_final{break}
    }
    if let Some((s,trie,_))=&stored{if let Some(r)=trie.world{s.unpin(&r)?;}}
    // H_CLOSE <= 1: no ordinary block can exist in this profile; its fact takes the larger
    // measured phase (conservative), so R14/R19 still cover every reachable block.
    if !seen[0]&&cp.H_CLOSE<=1{phase_ms[0]=phase_ms[1].max(phase_ms[2]);seen[0]=true;}
    if !seen.iter().all(|s|*s){return Err(format!("calibration: phase classes reached {seen:?}"))}
    Ok((Timing{genesis_hash:hash,phase_ms,samples,stored:store.is_some()},gas_log,roots))
}
fn spec_ts(cp:&ConsensusParams,h:u64)->u64{cp.g_ts.saturating_add(h.saturating_mul(u64::from(cp.T_blk)))}

#[cfg(test)] mod tests;
