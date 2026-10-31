//! pocold — PoCol local development node library (simnet devnets only).
// Accepted LP1 consensus sources, shared byte-for-byte (not copied).
#[path="../../lp1-node/src/fixed.rs"] #[allow(dead_code)] pub mod fixed;
#[path="../../lp1-node/src/rlp.rs"] #[allow(dead_code)] pub mod rlp;
#[path="../../lp1-node/src/hex.rs"] #[allow(dead_code)] pub mod hex;
#[path="../../lp1-node/src/hashes.rs"] #[allow(dead_code)] pub mod hashes;
#[path="../../lp1-node/src/decimal.rs"] #[allow(dead_code)] pub mod decimal;
#[path="../../lp1-node/src/genesis.rs"] #[allow(dead_code)] pub mod genesis;
#[path="../../lp1-node/src/genesis_spec.rs"] #[allow(dead_code)] pub mod genesis_spec;
#[path="../../lp1-node/src/header.rs"] #[allow(dead_code)] pub mod header;
#[path="../../lp1-node/src/asert.rs"] #[allow(dead_code)] pub mod asert;
#[path="../../lp1-node/src/assignment.rs"] #[allow(dead_code)] pub mod assignment;
#[path="../../lp1-node/src/timing.rs"] #[allow(dead_code)] pub mod timing;
pub mod site;
pub mod keyfile;
pub mod launcher;
pub mod wallet;
#[cfg(any(target_os = "android", test))]
mod mobile;
pub mod testnet;
pub mod identity;
pub mod secp_backend;
/// A13 obligation 3 (enforced): every pocold thread is created here with an explicit stack
/// reservation, and its resident stack cannot exceed that reservation (an overflow hits the
/// guard page and aborts). The thread inventory is in `resources::threads`.
pub mod threads{
    /// Network, RPC connection, forwarding, timer and miner threads.
    pub const IO_STACK:usize=256*1024;
    /// The consensus thread: admission, journal replay, block execution, RPC methods.
    pub const CONSENSUS_STACK:usize=16<<20;
    pub fn spawn<F,T>(name:impl Into<String>,stack:usize,f:F)->std::io::Result<std::thread::JoinHandle<T>>
    where F:FnOnce()->T+Send+'static,T:Send+'static{std::thread::Builder::new().name(name.into()).stack_size(stack).spawn(f)}
    /// Joins the finished threads among `hs` and drops their handles. A thread whose function
    /// has returned only has to exit, so the join is prompt; a caller that counts `hs` after
    /// this counts running and exiting threads together (A13 thread inventory).
    pub fn reap(hs:&mut Vec<std::thread::JoinHandle<()>>){
        let mut i=0;while i<hs.len(){if hs[i].is_finished(){let _=hs.swap_remove(i).join();}else{i+=1}}
    }
}
pub mod history;
pub mod state_store;
pub mod checkpoint;
/// C-1 E4.4 crash tests (`--features crash-test`, absent from the release): with POCOLD_CRASH_AT =
/// "<step>:<n>", the process aborts (as a kill would stop it, nothing flushed or dropped) the n-th time
/// it reaches `step` (counted from 1).
#[cfg(feature="crash-test")]
pub fn crash_point(step:&str){
    static SEEN:std::sync::Mutex<u64>=std::sync::Mutex::new(0);
    let Ok(v)=std::env::var("POCOLD_CRASH_AT") else{return};let Some((s,n))=v.split_once(':') else{return};
    if s!=step{return}let mut k=SEEN.lock().unwrap();*k+=1;if Some(*k)==n.parse().ok(){std::process::abort()}
}
#[cfg(not(feature="crash-test"))]
#[inline(always)]
pub fn crash_point(_step:&str){}
/// Storage fault tests (`--features io-fault`, absent from the release): deterministic I/O errors in
/// state.redb and history.redb. `arm(file,step,op)` makes the store at `file` fail its next `op` (a read, write or fsync) with
/// an I/O error, once, after it next reaches `step` (`io_point`, beside the crash points). The faults are
/// kept per file, so tests running in one process do not meet each other's. redb then refuses every
/// read and write of that database (`PreviousIo`) until it is opened again, as after a real I/O error.
#[cfg(feature="io-fault")]
pub mod io_fault{
    use std::path::{Path,PathBuf};use std::sync::Mutex;
    #[derive(Clone,Copy,PartialEq,Eq,Debug)]
    pub enum Op{Write,Sync,Read}
    struct Plan{file:PathBuf,step:String,op:Op,armed:bool,skip:u32}
    static PLANS:Mutex<Vec<Plan>>=Mutex::new(Vec::new());
    static FIRED:Mutex<Vec<(PathBuf,String,Op)>>=Mutex::new(Vec::new());
    pub fn arm(file:&Path,step:&str,op:Op){arm_at(file,step,op,1)}
    /// As `arm`, armed the `n`-th time the step is reached.
    pub fn arm_at(file:&Path,step:&str,op:Op,n:u32){PLANS.lock().unwrap().push(Plan{file:file.to_path_buf(),step:step.into(),op,armed:false,skip:n.saturating_sub(1)});}
    /// A fault armed at once: the next `op` on `file` fails (the journal's reads: `Op::Read`, `read`).
    pub fn arm_now(file:&Path,op:Op){PLANS.lock().unwrap().push(Plan{file:file.to_path_buf(),step:"now".into(),op,armed:true,skip:0});}
    /// The journal's read back of a record (`Journal::payload_at`) fails as armed.
    pub fn read(file:&Path)->std::io::Result<()>{take(file,Op::Read)}
    /// The faults injected into `file` so far: (step, op), oldest first.
    pub fn fired(file:&Path)->Vec<(String,Op)>{FIRED.lock().unwrap().iter().filter(|x|x.0==file).map(|x|(x.1.clone(),x.2)).collect()}
    /// Drops the faults armed for `file` that have not fired.
    pub fn disarm(file:&Path){PLANS.lock().unwrap().retain(|p|p.file!=file);}
    pub(crate) fn reached(file:&Path,step:&str){for p in PLANS.lock().unwrap().iter_mut(){if p.file==file&&p.step==step&&!p.armed{if p.skip>0{p.skip-=1}else{p.armed=true;}}}}
    fn take(file:&Path,op:Op)->std::io::Result<()>{
        let mut plans=PLANS.lock().unwrap();
        let Some(i)=plans.iter().position(|p|p.armed&&p.op==op&&p.file==file) else{return Ok(())};
        let p=plans.remove(i);FIRED.lock().unwrap().push((p.file,p.step,op));
        Err(std::io::Error::other("injected I/O fault (io-fault)"))
    }
    /// redb's file backend, failing as armed.
    #[derive(Debug)]
    pub(crate) struct Faulty{inner:redb::backends::FileBackend,file:PathBuf}
    impl redb::StorageBackend for Faulty{
        fn len(&self)->std::io::Result<u64>{self.inner.len()}
        fn read(&self,offset:u64,len:usize)->std::io::Result<Vec<u8>>{take(&self.file,Op::Read)?;self.inner.read(offset,len)}
        fn set_len(&self,len:u64)->std::io::Result<()>{self.inner.set_len(len)}
        fn sync_data(&self,eventual:bool)->std::io::Result<()>{take(&self.file,Op::Sync)?;self.inner.sync_data(eventual)}
        fn write(&self,offset:u64,data:&[u8])->std::io::Result<()>{take(&self.file,Op::Write)?;self.inner.write(offset,data)}
    }
    /// `Builder::create(path)` with the failing backend.
    pub(crate) fn create(b:&redb::Builder,path:&Path)->Result<redb::Database,redb::DatabaseError>{
        let f=std::fs::OpenOptions::new().read(true).write(true).create(true).truncate(false).open(path)?;
        b.create_with_backend(Faulty{inner:redb::backends::FileBackend::new(f)?,file:path.to_path_buf()})
    }
}
/// Where an armed storage fault (`io_fault`) is triggered; nothing without the feature.
#[cfg(feature="io-fault")]
pub fn io_point(file:&std::path::Path,step:&str){io_fault::reached(file,step)}
#[cfg(not(feature="io-fault"))]
#[inline(always)]
pub fn io_point(_file:&std::path::Path,_step:&str){}
pub mod statediff;pub mod types;pub mod chain;pub mod store;pub mod miner;pub mod net;pub mod node;pub mod rpc;pub mod resources;
/// Unit tests only: a counting allocator (per thread, while switched on) so tests can bound the
/// heap a code path requests.
#[cfg(test)]
pub(crate) mod alloc_meter{
    use std::alloc::{GlobalAlloc,Layout,System};use std::cell::Cell;
    thread_local!{static ON:Cell<bool>=const{Cell::new(false)};static NOW:Cell<isize>=const{Cell::new(0)};static PEAK:Cell<isize>=const{Cell::new(0)};}
    fn add(d:isize){let _=ON.try_with(|on|if on.get(){let _=NOW.try_with(|n|{n.set(n.get()+d);let _=PEAK.try_with(|p|p.set(p.get().max(n.get())));});});}
    struct Meter;
    unsafe impl GlobalAlloc for Meter{
        unsafe fn alloc(&self,l:Layout)->*mut u8{let p=unsafe{System.alloc(l)};if !p.is_null(){add(l.size() as isize)}p}
        unsafe fn dealloc(&self,p:*mut u8,l:Layout){unsafe{System.dealloc(p,l)};add(-(l.size() as isize))}
        unsafe fn realloc(&self,p:*mut u8,l:Layout,n:usize)->*mut u8{let q=unsafe{System.realloc(p,l,n)};if !q.is_null(){add(n as isize-l.size() as isize)}q}
    }
    #[global_allocator] static A:Meter=Meter;
    /// Highest heap bytes requested at once while `f` runs (relative to its start).
    pub fn peak<T>(f:impl FnOnce()->T)->(T,isize){ON.with(|o|o.set(true));NOW.with(|n|n.set(0));PEAK.with(|p|p.set(0));let r=f();let x=PEAK.with(|p|p.get());ON.with(|o|o.set(false));(r,x)}
}

use pocol_lp4_execution::{admission::{self,NodeLocal,Request},devnet,gate::FactClass,profile::{NodeKind,ProfileKind}};

/// The node-local parameters of a single-host devnet node, with pocold's actual
/// gossip frame limit (net::MAX_FRAME) for every message type and its RPC limits.
/// D-A13-BUDGET (local, owner-approved option 1): side_max 8 -> 2 (side-branch states; a pruned
/// side state is rebuilt on demand from its nearest retained ancestor). K_hist stays 32.
pub const SIDE_MAX:u32=2;
pub fn single_host_local()->NodeLocal{NodeLocal{gossip:[net::MAX_FRAME as u32;6],rpc:rpc_facts(&rpc::Limits::default()),side_max:SIDE_MAX,..NodeLocal::single_host()}}
/// R26 facts (D-A02-SCOPE): the RpcGuard fields pocold enforces, or enforces by an equivalent (or
/// stronger) bound within its architecture, at their running values (development/audit-2026-10-22/
/// RPCGUARD.md maps each). heavy_active and light_active are reported at 1, the equivalent bound of
/// the single consensus thread (active work ≤ 1 ≤ the spec's 4/16); conn_buf is SMALL_REPLY_MAX, the
/// structural bound on a connection's small answers (the spec's CONN_BUF use); rate and bucket_cap
/// are the per-connection bucket on a connection's one request (rate is vacuous there: RPC_EQUIVALENT
/// reports it in the admission notes); send_sock_buf is SEND_SOCK_BUF, set on every accepted
/// connection (one whose buffer cannot be set to it is not served). Four fields stay absent,
/// never nominal, so admission can only be an experimental-devnet admission: scratch_heavy and scratch_light (only the log handlers are charged; the
/// others, eth_call's EVM memory included, are not), mem_total (pocold bounds RPC memory through its
/// own A13 budget, not a 192 MiB field), and cancel_max_ms (a precompile call is not interruptible,
/// so the cancellation gap is not bounded by 100 ms).
pub const RPC_ENFORCED:[&str;25]=["conn_max","req_buf","conn_buf","send_sock_buf","heavy_active","light_active","light_queue","heavy_resp_mem","light_resp_mem",
    "resp_max","light_resp_max","batch_resp_max","batch_max","err_item_max","logs_count_max","logs_scan_bytes",
    "rate","bucket_cap","queue_wait_ms","compute_heavy_ms","grace_ms","min_send_rate","batch_wall_ms","enforce_slack_ms","timer_eps_ms"];
pub fn rpc_facts(l:&rpc::Limits)->admission::RpcFacts{
    let ms=|d:std::time::Duration|d.as_millis() as u64;
    let v:[u64;25]=[l.conn_max as u64,rpc::REQ_BUF as u64,rpc::SMALL_REPLY_MAX as u64,rpc::SEND_SOCK_BUF as u64,1,1,l.light_queue as u64,rpc::HEAVY_RESP_POOL as u64,rpc::light_pool(l) as u64,
        rpc::RESP_MAX as u64,rpc::LIGHT_RESP_MAX as u64,rpc::BATCH_RESP_MAX as u64,rpc::BATCH_MAX as u64,rpc::ERR_ITEM_MAX as u64,rpc::LOGS_COUNT_MAX as u64,rpc::LOGS_SCAN_BYTES as u64,
        l.rate as u64,l.bucket_cap as u64,ms(l.queue_wait),ms(l.compute_heavy),ms(l.grace),l.min_send_rate,ms(l.batch_wall),ms(l.enforce_slack),ms(l.timer_eps)];
    let present:Vec<(&str,u64)>=RPC_ENFORCED.iter().zip(v).map(|(k,x)|(*k,x)).collect();
    admission::RpcFacts::enforced(&present).expect("RPC_ENFORCED names RpcGuard fields")
}

/// The R26 facts of RPC_ENFORCED that hold by an equivalent bound, not by the spec's mechanism, and
/// why (RPCGUARD.md). Reported in the admission notes, so none reads as an enforced mechanism.
pub const RPC_EQUIVALENT:&[(&str,&str)]=&[
    ("heavy_active","1, the single consensus thread (active heavy work <= 1 <= HEAVY_ACTIVE)"),
    ("light_active","1, the single consensus thread drains the light lane (<= 1 <= LIGHT_ACTIVE)"),
    ("conn_buf","SMALL_REPLY_MAX, the structural bound of a connection's busy or rate answer (<= 64 KiB)"),
    ("light_resp_mem","(conn_max+1) x LIGHT_RESP_MAX, 512 KiB above the spec's 32 x 512 KiB: one reservation per connection plus the one being computed for a connection that left, so a light reservation never fails (S1)"),
    ("rate","vacuous per connection: a connection carries one request of k <= BATCH_MAX = bucket_cap tokens from a full bucket, so the refill rate is never read; the aggregate rate across connections is open (proposals/RPCGUARD-R2-PROPOSALS.md P1)"),
    ("bucket_cap","the per-connection bucket on the connection's one request (k > bucket_cap is -32021 {rate})"),
];
/// The admission scope, waivers and conditions, for devnet.json and status (D-A02-SCOPE). The notes
/// are the gate's, then the RPC_EQUIVALENT facts.
pub fn admission_json(a:&admission::Admitted)->serde_json::Value{
    let r=a.report();
    let notes:Vec<String>=r.notes.iter().cloned().chain(RPC_EQUIVALENT.iter().map(|(f,w)|format!("R26 {f}: equivalent, {w}"))).collect();
    serde_json::json!({"scope":r.scope.name(),"specificationCompliant":a.specification_compliant(),"binding":types::hex(&r.binding),
        "waivers":r.waivers.iter().map(|w|serde_json::json!({"rule":w.rule,"absent":w.absent,"waived":w.waived,"checked":w.checked})).collect::<Vec<_>>(),
        "conditions":r.conditions.iter().map(|c|serde_json::json!({"id":c.id,"rules":c.rules,"detail":c.detail})).collect::<Vec<_>>(),
        "notes":notes,"deferred":r.deferred})
}

/// Loads a devnet's published files and runs the full ParamGate admission (A02): the
/// GenesisSpec, alloc, the declared calibration (devnet.json), a timing measurement taken now on
/// this machine and this node's local parameters. Boot is refused with every violated rule.
/// The timing recorded by init-devnet is informational only (review finding: a recorded value
/// could come from another machine or be edited).
pub fn load_spec(dir:&std::path::Path,local:&NodeLocal)->Result<chain::Spec,String>{load_spec_for(dir,local,None)}
/// Operator-approved consensus schedule, separate from the immutable genesis.
/// Missing schedules retain the historical v1 behavior; malformed schedules fail closed.
pub fn configured_forks(info:&serde_json::Value)->Result<Vec<(u16,u64)>,String>{
    if !info.is_object(){return Err("network metadata must be a JSON object".into())}
    let forks = match info.get("forks") {
        None => vec![(1,0)],
        Some(value) => {
            if value.as_array().map_or(true,|a|a.is_empty()||a.len()>16){return Err("invalid consensus fork schedule".into())}
            serde_json::from_value::<Vec<(u16,u64)>>(value.clone()).map_err(|_|"invalid consensus fork schedule".to_string())?
        }
    };
    pocol_lp4_execution::profile::ForkSchedule::new(&forks).map_err(|e|format!("consensus fork schedule: {e:?}"))?;
    if forks!=[(1,0)] && !(forks.len()==2 && forks[0]==(1,0) && forks[1].0==2){
        return Err("runtime schedule must be [(1,0)] or [(1,0),(2,H)]".into());
    }
    Ok(forks)
}
/// The scratch state store of a store-mode boot's R14/R19 measurement, in the node's data directory.
pub const CALIBRATION_STORE:&str="calibration.store";
/// `load_spec` for a node about to open `store_data` in store mode (C-1 E4; `None`: whole-state mode). R14/R19 are
/// measured on the path that node runs: the calibration blocks are also applied through a scratch state store in
/// `store_data` (each block's nodes committed and fsynced to its state.redb, on the node's own disk), and each phase
/// class takes the larger of the two times. The scratch store is removed before the node opens.
pub fn load_spec_for(dir:&std::path::Path,local:&NodeLocal,store_data:Option<&std::path::Path>)->Result<chain::Spec,String>{
    let hexs=std::fs::read_to_string(dir.join("genesis.spec.hex")).map_err(|e|format!("genesis.spec.hex: {e}"))?;
    let spec_bytes=types::unhex(hexs.trim())?;
    let alloc:serde_json::Value=serde_json::from_str(&std::fs::read_to_string(dir.join("alloc.json")).map_err(|e|format!("alloc.json: {e}"))?).map_err(|e|e.to_string())?;
    let alloc=devnet::alloc_from_json(&alloc)?;
    let info:serde_json::Value=match std::fs::read_to_string(dir.join("devnet.json")){
        Ok(text)=>serde_json::from_str(&text).map_err(|e|format!("devnet.json: {e}"))?,
        Err(e) if e.kind()==std::io::ErrorKind::NotFound=>serde_json::json!({}),
        Err(e)=>return Err(format!("devnet.json: {e}")),
    };
    let forks=configured_forks(&info)?;
    let calibration=info["calibration"]["hashesPerSecond"].as_u64().map(|r|(r,FactClass::SyntheticFixture));
    let mut measured=admission::measure(&spec_bytes,&alloc,1);
    if let (Some(data),Ok(mem))=(store_data,&measured){measured=store_timing(data,&spec_bytes,&alloc).and_then(|s|mem.with_store(&s));}
    let req=Request{spec_bytes:&spec_bytes,alloc:&alloc,forks:&forks,kind:ProfileKind::Simnet,node:NodeKind::Simnet,local,calibration,timing:measured.as_ref().ok(),scope:admission::Scope::ExperimentalDevnet};
    admission::admit(&req).map(chain::Spec::new).map_err(|e|match &measured{Err(why)=>format!("{e}; timing: {why}"),Ok(_)=>e.to_string()})
}
/// The calibration through a scratch state store at `data/CALIBRATION_STORE`, removed afterwards (also when the
/// measurement fails; one a crash left is removed first).
fn store_timing(data:&std::path::Path,spec_bytes:&[u8],alloc:&revm::InMemoryDB)->Result<admission::Timing,String>{
    let scratch=data.join(CALIBRATION_STORE);let _=std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).map_err(|e|format!("calibration store {}: {e}",scratch.display()))?;
    let r=state_store::StateStore::open(&scratch).map_err(|e|format!("calibration store: {}",e.0)).and_then(|st|admission::measure_on_store(spec_bytes,alloc,1,&st));
    let _=std::fs::remove_dir_all(&scratch);r
}
