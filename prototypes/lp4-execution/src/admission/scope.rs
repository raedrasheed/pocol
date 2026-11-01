//! D-A02-SCOPE (audit repair, round 2): what an admission establishes, and the provenance of
//! the RpcGuard (R26) inputs. A field the node does not enforce is absent; it is never given
//! a nominal value, so it cannot be counted as evidence that its mechanism exists.
use crate::param_gate::RpcLimits;

/// What an admission establishes.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum Scope{
    /// governance.md R1–R26 with every input supplied by an enforced mechanism, a measurement
    /// or a proof, and no open obligation. Any `Condition` rejects the profile.
    Specification,
    /// A restricted local devnet: a simnet profile on a simnet node only. R26 predicates that
    /// read an RpcGuard field the node does not enforce are waived and reported; every other
    /// rule is evaluated exactly as in `Specification`. Never specification compliance.
    ExperimentalDevnet,
}
impl Scope{pub fn name(self)->&'static str{match self{Scope::Specification=>"specification",Scope::ExperimentalDevnet=>"experimental-devnet"}}}

/// Why an admitted profile is not specification evidence. `Specification` rejects with `rules`.
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Condition{pub id:&'static str,pub rules:&'static [&'static str],pub detail:String}
/// R26 predicates not evaluated because they read absent fields.
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct Waiver{pub rule:&'static str,pub absent:Vec<&'static str>,pub waived:Vec<&'static str>,pub checked:Vec<&'static str>}

/// The RpcGuard fields of `param_gate::RpcLimits`, in declaration order, listed once: the macro
/// derives RPC_FIELDS, the field lookup and an exhaustive destructuring of RpcLimits (no `..`),
/// so a field added to RpcLimits and not listed here fails to compile (review F6).
macro_rules! rpc_fields{($($f:ident),* $(,)?)=>{
    pub const RPC_FIELDS:&[&str]=&[$(stringify!($f)),*];
    fn slot<'a>(l:&'a mut RpcLimits,name:&str)->Option<&'a mut u64>{match name{$(stringify!($f)=>Some(&mut l.$f),)* _=>None}}
    #[allow(dead_code)] fn every_field_is_listed(l:RpcLimits){let RpcLimits{$($f:_),*}=l;}
}}
rpc_fields!(conn_max,req_buf,conn_buf,send_sock_buf,heavy_active,light_active,light_queue,heavy_resp_mem,light_resp_mem,
    resp_max,light_resp_max,batch_resp_max,scratch_heavy,scratch_light,mem_total,batch_max,err_item_max,logs_count_max,logs_scan_bytes,
    rate,bucket_cap,queue_wait_ms,compute_heavy_ms,cancel_max_ms,grace_ms,min_send_rate,batch_wall_ms,enforce_slack_ms,timer_eps_ms);

/// R26 inputs with their provenance.
#[derive(Clone,Debug)]
pub struct RpcFacts{values:RpcLimits,fixture:bool,absent:Vec<&'static str>}
impl RpcFacts{
    /// A complete RpcGuard configuration that no node here enforces (LP3 cases, lp4 fixtures).
    /// R26 is evaluated in full; the admission reports the values as a fixture.
    pub fn fixture(l:RpcLimits)->Self{Self{values:l,fixture:true,absent:vec![]}}
    /// The fields a node enforces, by `RPC_FIELDS` name. Every other field is absent and enters
    /// the gate as 0.
    pub fn enforced(present:&[(&str,u64)])->Result<Self,String>{
        let mut v=RpcLimits::default();for f in RPC_FIELDS{*slot(&mut v,f).unwrap()=0}
        let mut seen=Vec::new();
        for (k,x) in present{
            let s=slot(&mut v,k).ok_or_else(||format!("unknown RpcGuard field {k}"))?;
            if seen.contains(k){return Err(format!("RpcGuard field {k} given twice"))}
            *s=*x;seen.push(*k);
        }
        Ok(Self{values:v,fixture:false,absent:RPC_FIELDS.iter().copied().filter(|f|!seen.contains(f)).collect()})
    }
    pub fn values(&self)->&RpcLimits{&self.values}
    pub fn absent(&self)->&[&'static str]{&self.absent}
    pub fn is_fixture(&self)->bool{self.fixture}
}

type Pred=(&'static str,&'static [&'static str],fn(&RpcLimits)->bool);
/// Every input of `RpcLimits::derive` (any of them can make it fail by overflow); a predicate
/// that calls `derive` reads all of them.
const DERIVE:&[&str]=&["req_buf","conn_buf","heavy_resp_mem","light_resp_mem","heavy_active","scratch_heavy","light_active","scratch_light","conn_max",
    "batch_max","err_item_max","batch_resp_max","min_send_rate","grace_ms","enforce_slack_ms","queue_wait_ms","compute_heavy_ms","cancel_max_ms"];
const DERIVE_MEM:&[&str]=&["req_buf","conn_buf","heavy_resp_mem","light_resp_mem","heavy_active","scratch_heavy","light_active","scratch_light","conn_max",
    "batch_max","err_item_max","batch_resp_max","min_send_rate","grace_ms","enforce_slack_ms","queue_wait_ms","compute_heavy_ms","cancel_max_ms","mem_total"];
const DERIVE_WALL:&[&str]=&["req_buf","conn_buf","heavy_resp_mem","light_resp_mem","heavy_active","scratch_heavy","light_active","scratch_light","conn_max",
    "batch_max","err_item_max","batch_resp_max","min_send_rate","grace_ms","enforce_slack_ms","queue_wait_ms","compute_heavy_ms","cancel_max_ms","batch_wall_ms"];
/// `RpcLimits::satisfies_r26` (param_gate.rs) split into its predicates, each with the fields it
/// reads. `tests::r26_predicates_are_the_gate_conjunction` checks that their conjunction is the
/// gate's verdict.
pub const R26_PREDICATES:&[Pred]=&[
    ("derive",DERIVE,|r|r.derive().is_ok()),
    ("heavyRespMem>=respMax",&["heavy_resp_mem","resp_max"],|r|r.heavy_resp_mem>=r.resp_max),
    ("lightRespMem>=connMax*lightRespMax",&["light_resp_mem","conn_max","light_resp_max"],|r|r.light_resp_mem as u128>=r.conn_max as u128*r.light_resp_max as u128),
    ("memoryRequired<=memTotal",DERIVE_MEM,
        |r|r.derive().map_or(false,|v|v.memory_required<=r.mem_total as u128)),
    ("batchRespMax>=respMax",&["batch_resp_max","resp_max"],|r|r.batch_resp_max>=r.resp_max),
    ("lightQueue>=connMax",&["light_queue","conn_max"],|r|r.light_queue>=r.conn_max),
    ("batchMax*errItemMax+64<=connBuf",&["batch_max","err_item_max","conn_buf"],|r|r.batch_max as u128*r.err_item_max as u128+64<=r.conn_buf as u128),
    ("respMax>=2*32768+546*512",&["resp_max"],|r|r.resp_max>=2*32768+546*512),
    ("logsCountMax>=546",&["logs_count_max"],|r|r.logs_count_max>=546),
    ("logsScanBytes>=32768",&["logs_scan_bytes"],|r|r.logs_scan_bytes>=32768),
    ("lightRespMax>=385KiB",&["light_resp_max"],|r|r.light_resp_max>=385*1024),
    ("batchWallMin<=batchWallMs",DERIVE_WALL,
        |r|r.derive().map_or(false,|v|v.batch_wall_min.at_most_ms(r.batch_wall_ms))),
    ("rate>=1",&["rate"],|r|r.rate>=1),
    ("bucketCap>=batchMax",&["bucket_cap","batch_max"],|r|r.bucket_cap>=r.batch_max),
    ("timerEps>0",&["timer_eps_ms"],|r|r.timer_eps_ms>0),
    ("timerEps<enforceSlack",&["timer_eps_ms","enforce_slack_ms"],|r|r.timer_eps_ms<r.enforce_slack_ms),
    ("enforceSlack<=500",&["enforce_slack_ms"],|r|r.enforce_slack_ms<=500),
    ("sendSockBuf<=64KiB",&["send_sock_buf"],|r|r.send_sock_buf<=64*1024),
];

/// R26 over the enforced fields only: the predicates reading no absent field are checked, the
/// others are waived. Returns the waiver and the checked predicates that fail.
pub fn r26_partial(f:&RpcFacts)->(Waiver,Vec<&'static str>){
    let reads_absent=|p:&Pred|p.1.iter().any(|x|f.absent.contains(x));
    let checked:Vec<&Pred>=R26_PREDICATES.iter().filter(|p|!reads_absent(p)).collect();
    let failed=checked.iter().filter(|p|!(p.2)(&f.values)).map(|p|p.0).collect();
    (Waiver{rule:"R26",absent:f.absent.clone(),waived:R26_PREDICATES.iter().filter(|p|reads_absent(p)).map(|p|p.0).collect(),checked:checked.iter().map(|p|p.0).collect()},failed)
}
