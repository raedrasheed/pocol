//! pocold — PoCol local development node (simnet devnets only).
use pocold::{node,types,load_spec_for,single_host_local};
use pocol_lp4_execution::admission::NodeLocal;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool,Ordering};
use pocol_lp4_execution::devnet;

fn arg(args:&[String],k:&str)->Option<String>{args.iter().position(|a|a==k).and_then(|i|args.get(i+1)).cloned()}
fn args_all(args:&[String],k:&str)->Vec<String>{args.windows(2).filter(|w|w[0]==k).map(|w|w[1].clone()).collect()}
fn flag(args:&[String],k:&str)->bool{args.iter().any(|a|a==k)}
fn num<T:std::str::FromStr>(args:&[String],k:&str,d:T)->T{arg(args,k).and_then(|v|v.parse().ok()).unwrap_or(d)}

/// Set by SIGINT and SIGTERM (Ctrl-C, close and shutdown on Windows) once `pocold run` started its network
/// (`catch_stop`): it then stops at its next event boundary, saves the address book and its anchors
/// (`Node::run_controlled`), prints its status and exits 0, so `systemctl stop` is a clean stop. As bin/pocol.rs.
static STOP:AtomicBool=AtomicBool::new(false);
#[cfg(unix)]
extern "C" fn signal(_:i32){STOP.store(true,Ordering::Relaxed);}
#[cfg(windows)]
unsafe extern "system" fn console(event:u32)->i32{if event==0||event==1||event==2{STOP.store(true,Ordering::Relaxed);1}else{0}}
/// `pocold run`'s handlers, installed just before its network starts. Only `run` reads STOP, so every other command
/// (cutover's replay, status's, init-*) and `run`'s own Node::open keep the default disposition and end at once on
/// Ctrl-C or SIGTERM (coreutils `timeout`), as before the handlers existed.
fn catch_stop(){
    #[cfg(unix)]
    unsafe{libc::signal(libc::SIGINT,signal as *const () as usize);libc::signal(libc::SIGTERM,signal as *const () as usize);}
    #[cfg(windows)]
    unsafe{windows_sys::Win32::System::Console::SetConsoleCtrlHandler(Some(console),1);}
}
/// main only collects the arguments, then runs the command on the consensus thread, whose
/// stack reservation is explicit (A13 thread inventory), and joins it.
fn main(){
    // A first argument that is not text is a command this build does not have (args() would panic on it).
    if let Some(c)=std::env::args_os().nth(1).filter(|c|c.to_str().is_none()){eprintln!("error: unknown command {c:?}\n{USAGE}");std::process::exit(2)}
    let args:Vec<String>=std::env::args().collect();
    let code=match pocold::threads::spawn("consensus",pocold::threads::CONSENSUS_STACK,move||command(&args)).map(|t|t.join()){
        Ok(Ok(code))=>code,Ok(Err(_))=>101,Err(e)=>{eprintln!("error: spawn consensus thread: {e}");1}};
    std::process::exit(code);
}
const USAGE:&str="usage:\n  pocold init-devnet --out DIR --miners a,b,c [--chain-id N] [--genesis-time S] [--t-blk S] [--tau S] [--d-att S] [--fb-wait S] [--c N] [--work-bits N] [--h-end N] [--h-close N] [--accounts x,y] [--full-nonce] [LOCAL]\n  pocold init-testnet --config PUBLIC.json --out NEW_DIR\n  pocold cutover --network OLD_DIR --data OLD_DATA --height S --config NEW.json --out NEW_DIR [--export-chain EXPORT_DIR] [NODE] [LOCAL]   (the old node stopped; --export-chain: also the V3-33 old-chain data of S, as export-chain writes it)\n  pocold export-chain --network OLD_DIR --data OLD_DATA --to-height H --out NEW_DIR [NODE] [LOCAL]   (the old node stopped; V3-33 old-chain data)
  pocold verify-export --dir EXPORT_DIR [--network OLD_DIR] [--manifest-sha256 HEX] [--head-hash HEX] [LOCAL]   (verified is true only with --head-hash, the hash of block H from your own node; else selfConsistent)
  pocold run --network DIR --data DIR --name N [--miner-key-file FILE | --miner DEVNET_LABEL] [--listen ADDR] [--peer ADDR]... [--seed ADDR]... [--bootstrap ADDR]... [--outbound N|auto] [--rpc ADDR] [--stop-at H] [--log] [--no-discovery] [--discover-local] [NODE]\n  pocold status --network DIR --data DIR [NODE]\n  pocold build-info   (this build's identity, as JSON)\n  pocold help   (this text)\nNODE: [--mine-threads N] [--work-rpc ADDR] [--freeze-user-txs-after S|off] [--pool-gas-max GAS] [--state-store [--checkpoint-every N]] [--peers-max N] [--per-group-max N] [--memory-budget-bytes N] [--evict-depth N] (--mine-threads: V3-30, the miner scans this node's own nonce range with N threads on disjoint slices, 1..64, default 1, best at most the host's cores less one (more than its cores is warned); --work-rpc ADDR: V3-30, a loopback work port of its own serving pocol_getWork and pocol_submitWork only (own range only), off by default, never to be reverse-proxied; --freeze-user-txs-after: V3-33 re-genesis conduct, this node builds no block above S with user transactions and its pool refuses and relays none once its head is at S, other nodes' blocks are judged as before; run also reads POCOL_FREEZE_USER_TXS_AFTER; --memory-budget-bytes: the A13 memory budget, default 6 GiB; run also reads POCOL_MEMORY_BUDGET_BYTES; --evict-depth: canonical blocks kept in memory below the head, 0 off; --state-store is experimental; open a data directory in the mode it was made in; --peers-max: established inbound peers, default 32, 1..128; run also reads POCOL_PEERS_MAX; --per-group-max: inbound peers per public /16 or /32, default 4, 0 off; run also reads POCOL_PER_GROUP_MAX)\nDISCOVERY (run, VA-SPEC §1a): on unless --no-discovery or POCOL_DISCOVERY=0; --discover-local or POCOL_DISCOVER_LOCAL=1 also learns loopback and private addresses from peers that have them; automatic outbound: --outbound N (0..8) or auto (default: 4 with --listen, else 2; POCOL_OUTBOUND), plus 1 spare slot; --seed (<= 7) peers used only to bootstrap and refill; --bootstrap (<= 16) unverified addresses to try; a flag wins over its variable\nLOCAL (ParamGate local timing, whole seconds; default single host, but run and status on a v3 network default to its V3-05 timing): [--delta S] [--phi S] [--t-exec S] [--delta-rel S] [--gamma S]\nEvery profile is admitted by the full ParamGate (R1-R26) before boot; a rejection lists the violated rules (-32018).\nExit status: 0 done; 1 the command failed; 2 no command, or one this build does not have; 70 a local storage failure stopped the node; 101 an internal failure.";
/// Runs one command and returns the process exit status. `help` (also `--help`, `-h`) prints the
/// usage on stdout and exits 0. No command, or a command this build does not have, prints the
/// usage on stderr and exits 2, so a script never takes a command that did not run for success.
fn command(args:&[String])->i32{
    let Some(cmd)=args.get(1).map(|s|s.as_str()) else {eprintln!("{USAGE}");return 2};
    let r=match cmd{
        "build-info"=>{println!("{}",pocold::readiness::build_info());Ok(())},
        "init-devnet"=>init(args),
        "init-testnet"=>pocold::testnet::init(args),
        "cutover"=>cutover(args),
        "export-chain"=>export_chain(args),
        "verify-export"=>verify_export(args),
        "run"=>run(args),
        "status"=>status(args),
        // A reader that closes the pipe early (`pocold help | head`) is not an error.
        "help"|"--help"|"-h"=>{use std::io::Write;match writeln!(std::io::stdout(),"{USAGE}"){Err(e) if e.kind()!=std::io::ErrorKind::BrokenPipe=>Err(format!("stdout: {e}")),_=>Ok(())}},
        _=>{eprintln!("error: unknown command {cmd:?}\n{USAGE}");return 2}
    };
    if let Err(e)=r{eprintln!("error: {e}");return 1}
    0
}
/// Local node limits from the arguments: the defaults, with
/// - `--pool-gas-max GAS` (decision D-7: the bound on the pooled transactions' summed declared gas limits; at
///   least GAS_LIMIT, and A13 counts the pool at it);
/// - `--state-store` (C-1 E4, experimental: states kept as trie versions in state.redb; the default stays
///   whole-state until store mode is accepted). A data directory must be opened in the mode it was made in;
///   `--checkpoint-every N` sets store mode's checkpoint interval in journal records (0: none);
/// - `--peers-max N` (local P2P policy, `net::Limits::peers_max`, default 32, 1..=PEERS_MAX_LIMIT): the established
///   inbound peers; past it a newcomer evicts one or is closed. A13 counts every place, so a larger value needs a
///   larger memory budget and task limit. `run` also reads POCOL_PEERS_MAX when the flag is absent (`p2p_env`);
/// - `--per-group-max N` (local P2P policy, `net::Limits::per_group_max`, default 4): the inbound connections one
///   public address group (IPv4 /16, IPv6 /32) may hold, plus one spare place that must evict within the group; 0
///   turns grouping off. `run` also reads POCOL_PER_GROUP_MAX when the flag is absent (`p2p_env`);
/// - `--no-discovery` and `--discover-local` (VA-SPEC §1a, local policy, `node::Limits::discovery`): peer discovery
///   off (the escape hatch: no listenPort, GetAddr, answers or address book), and local mode on (loopback and
///   private addresses learned from peers that have them). `run` also reads POCOL_DISCOVERY and POCOL_DISCOVER_LOCAL
///   when the flag is absent (`Discovery::env`);
/// - `--outbound N|auto` (VA-SPEC §1a.7): the automatic outbound slots, resolved at `Node::open`. `run` also reads
///   POCOL_OUTBOUND when the flag is absent (`Discovery::outbound_env`);
/// - `--freeze-user-txs-after S|off` (V3-33 re-genesis conduct, local builder and pool policy, never validity): this
///   node's blocks above S take no user transaction and its pool refuses and relays none once its head is at S
///   (`node::Limits::freeze_user_txs_after`). `run` also reads POCOL_FREEZE_USER_TXS_AFTER when the flag is absent;
/// - `--mine-threads N` (V3-30 S1, local mining policy, never validity): the miner scans the own nonce range of each
///   template with N threads, each a disjoint slice (`miner::slices`), 1..=64, default 1; A13 counts each thread;
/// - `--work-rpc ADDR` (V3-30 S2, local): a loopback work port of its own that serves pocol_getWork and
///   pocol_submitWork and nothing else, to loopback callers without Origin (off by default; the main RPC port never
///   serves them, so a reverse proxy in front of it never reaches them; submitWork accepts only the locked template and
///   nonces of this node's own range);
/// - `--memory-budget-bytes N` (A13, local): the node memory budget the bound must fit with its 1/8 reserve (default
///   6 GiB, `node::MEM_BUDGET`). `run` also reads POCOL_MEMORY_BUDGET_BYTES when the flag is absent (`budget_env`);
/// - `--evict-depth N` (C-1 S5, local retention, never validity): canonical entries more than N blocks below the head
///   leave memory for the canonical index (canon.redb); 0 turns eviction off (the behaviour before PR7, with its
///   record-count stop); the default is `resources::evict_depth_of`'s, and a depth below the safe minimum is refused.
fn limits(args:&[String])->Result<node::Limits,String>{
    let mut l=node::Limits::default();
    if let Some(v)=arg(args,"--memory-budget-bytes"){l.mem_budget=v.parse().map_err(|e|format!("--memory-budget-bytes {v}: {e}"))?;}
    if let Some(v)=arg(args,"--evict-depth"){l.evict_depth=Some(v.parse().map_err(|e|format!("--evict-depth {v}: {e}"))?);}
    if let Some(g)=arg(args,"--pool-gas-max"){l.pool_gas_max=Some(g.parse().map_err(|e|format!("--pool-gas-max {g}: {e}"))?);}
    if let Some(v)=arg(args,"--peers-max"){l.net.peers_max=pocold::net::parse_peers_max(&v).map_err(|e|format!("--peers-max {v}: {e}"))?;}
    if let Some(v)=arg(args,"--per-group-max"){l.net.per_group_max=v.parse().map_err(|e|format!("--per-group-max {v}: {e}"))?;}
    l.discovery.enabled=!flag(args,"--no-discovery");l.discovery.allow_local=flag(args,"--discover-local");
    if let Some(v)=arg(args,"--outbound"){l.discovery.outbound=node::parse_outbound(&v).map_err(|e|format!("--outbound {v}: {e}"))?;}
    if let Some(v)=arg(args,"--freeze-user-txs-after"){l.freeze_user_txs_after=node::parse_freeze(&v).map_err(|e|format!("--freeze-user-txs-after {v}: {e}"))?;}
    if let Some(v)=arg(args,"--mine-threads"){l.mine_threads=pocold::miner::parse_threads(&v).map_err(|e|format!("--mine-threads {v}: {e}"))?;}
    if flag(args,"--work-rpc"){
        let v=arg(args,"--work-rpc").ok_or("--work-rpc needs ADDR (a loopback address such as 127.0.0.1:8547)")?;
        let a:std::net::SocketAddr=v.parse().map_err(|e|format!("--work-rpc {v}: {e}"))?;
        if !a.ip().is_loopback(){return Err(format!("--work-rpc {v}: the work port binds loopback only"))}
        l.work_rpc=Some(a);
    }
    l.state_store=args.iter().any(|a|a=="--state-store");
    if let Some(c)=arg(args,"--checkpoint-every"){
        if !l.state_store{return Err("--checkpoint-every needs --state-store".into())}
        l.checkpoint_every=c.parse().map_err(|e|format!("--checkpoint-every {c}: {e}"))?;
    }
    Ok(l)
}
/// Node-local ParamGate inputs: the single-host defaults with optional overrides.
fn local(args:&[String])->NodeLocal{local_over(args,single_host_local())}
/// `--delta`, `--phi`, `--t-exec`, `--delta-rel` and `--gamma` over the defaults `d`.
fn local_over(args:&[String],d:NodeLocal)->NodeLocal{
    NodeLocal{delta:num(args,"--delta",d.delta),phi:num(args,"--phi",d.phi),t_exec:num(args,"--t-exec",d.t_exec),delta_rel:num(args,"--delta-rel",d.delta_rel),gamma:num(args,"--gamma",d.gamma),..d}
}
/// The local inputs a node on the network in `net` runs with (`run`, `status`). Spec 1 (every committed network):
/// `local`, unchanged. A v3 (spec-2) network: V3-05's timing from its files (`testnet::local_for_network`, as the
/// launcher), with the same overrides. A network whose spec cannot be read keeps `local`, and opening it reports why.
fn local_at(args:&[String],net:&std::path::Path)->Result<NodeLocal,String>{
    match pocold::testnet::spec_version_of(net){
        Ok(v) if v>=2=>Ok(local_over(args,pocold::testnet::local_for_network(net)?)),
        _=>Ok(local(args)),
    }
}
fn init(args:&[String])->Result<(),String>{
    let out=PathBuf::from(arg(args,"--out").ok_or("--out")?);
    let miners:Vec<String>=arg(args,"--miners").ok_or("--miners")?.split(',').map(|s|s.to_string()).collect();
    let refs:Vec<&str>=miners.iter().map(|s|s.as_str()).collect();
    let g_ts=num(args,"--genesis-time",node::now_s());
    let mut cfg=devnet::DevnetConfig::new(num(args,"--chain-id",777900u64),g_ts,&refs);
    cfg.t_blk=num(args,"--t-blk",cfg.t_blk);cfg.tau=num(args,"--tau",cfg.tau);cfg.d_att=num(args,"--d-att",cfg.d_att);cfg.d_fb_wait=num(args,"--fb-wait",cfg.d_fb_wait);cfg.c=num(args,"--c",cfg.c);
    cfg.h_end=num(args,"--h-end",cfg.h_end);cfg.h_close=num(args,"--h-close",cfg.h_close);cfg.d=num(args,"--d",cfg.d);cfg.w_w=num(args,"--w-w",cfg.w_w);
    // About 2^bits hashes per block: H_cal = 2^bits / T_blk (R25d calibration, synthetic for a devnet).
    // --full-nonce: nonceMode 1, the full 64-bit nonce domain from genesis, so no attempt can run out of
    // nonces (as Mainnet after its v2 height). Without it the profile keeps the v1 bounded range.
    if args.iter().any(|a|a=="--full-nonce"){cfg.nonce_mode=1;}
    let bits:u32=num(args,"--work-bits",18);cfg.hash_rate=(1u64<<bits.min(63))/u64::from(cfg.t_blk.max(1));
    if let Some(a)=arg(args,"--accounts"){cfg.accounts=a.split(',').map(|l|(l.to_string(),revm::primitives::U256::from(10u128.pow(24)))).collect();}
    let net=devnet::build(&cfg)?;
    // A02: measure this profile's timing, then run the full gate; nothing is written on rejection.
    let (admitted,timing)=net.admit(&local(args))?;
    std::fs::create_dir_all(&out).map_err(|e|e.to_string())?;
    std::fs::write(out.join("genesis.spec.hex"),types::hex(&net.spec_bytes)).map_err(|e|e.to_string())?;
    std::fs::write(out.join("alloc.json"),serde_json::to_string_pretty(&devnet::alloc_to_json(&net.alloc)).unwrap()).map_err(|e|e.to_string())?;
    let info=serde_json::json!({"chainId":cfg.chain_id,"genesisHash":types::hex(&net.genesis_hash.0),"genesisTime":g_ts,"forks":[[1,0]],"profileKind":"simnet",
        "note":"Local devnet only. Keys are deterministic disposable devnet keys derived from labels (pocol_lp4_execution::txsign::Key::devnet).",
        "miners":net.miners.iter().map(|m|serde_json::json!({"label":m.label,"id":format!("{:#x}",m.id),"reward":format!("{:#x}",m.reward)})).collect::<Vec<_>>(),
        "accounts":cfg.accounts.iter().map(|(l,w)|serde_json::json!({"label":l,"address":format!("{:#x}",devnet::account_key(l).address()),"wei":w.to_string()})).collect::<Vec<_>>(),
        "calibration":{"hashesPerSecond":cfg.hash_rate,"class":"synthetic-fixture","note":"devnet difficulty chosen with --work-bits, not a measured hash rate"},
        "timing":timing.to_json(),
        "admission":pocold::admission_json(&admitted)});
    let mut info=info;if cfg.nonce_mode==1{info["fullNonce"]=serde_json::json!(true);}
    std::fs::write(out.join("devnet.json"),serde_json::to_string_pretty(&info).unwrap()).map_err(|e|e.to_string())?;
    println!("{}",serde_json::to_string_pretty(&info).unwrap());Ok(())
}
/// Local P2P settings a service may give in its environment instead of its command line (only `run` opens
/// the network): POCOL_PEERS_MAX and POCOL_PER_GROUP_MAX when `--peers-max` and `--per-group-max` are absent, and POCOL_DISCOVERY and POCOL_DISCOVER_LOCAL
/// when `--no-discovery` and `--discover-local` are (`Discovery::env`), POCOL_OUTBOUND when `--outbound` is
/// (`Discovery::outbound_env`), and POCOL_FREEZE_USER_TXS_AFTER when `--freeze-user-txs-after` is (V3-33). An empty
/// value counts as unset.
fn p2p_env(args:&[String],l:&mut node::Limits)->Result<(),String>{
    if arg(args,"--peers-max").is_none(){
        if let Some(v)=std::env::var("POCOL_PEERS_MAX").ok().filter(|v|!v.trim().is_empty()){
            l.net.peers_max=pocold::net::parse_peers_max(&v).map_err(|e|format!("POCOL_PEERS_MAX {v}: {e}"))?;
        }
    }
    if arg(args,"--per-group-max").is_none(){
        if let Some(v)=std::env::var("POCOL_PER_GROUP_MAX").ok().filter(|v|!v.trim().is_empty()){
            l.net.per_group_max=v.trim().parse().map_err(|e|format!("POCOL_PER_GROUP_MAX {v}: {e}"))?;
        }
    }
    if arg(args,"--freeze-user-txs-after").is_none(){
        if let Some(v)=std::env::var("POCOL_FREEZE_USER_TXS_AFTER").ok().filter(|v|!v.trim().is_empty()){
            l.freeze_user_txs_after=node::parse_freeze(&v).map_err(|e|format!("POCOL_FREEZE_USER_TXS_AFTER {v}: {e}"))?;
        }
    }
    l.discovery.env(flag(args,"--no-discovery"),flag(args,"--discover-local"))?;
    l.discovery.outbound_env(arg(args,"--outbound").is_some())
}
/// The memory budget a service may give in its environment (C-1 S5, PR7 §4.5): POCOL_MEMORY_BUDGET_BYTES when
/// `--memory-budget-bytes` is absent, read as `pocol start` reads it (launcher.rs): a value that is not a number,
/// an empty one included, is refused.
fn budget_env(args:&[String],l:&mut node::Limits)->Result<(),String>{
    if arg(args,"--memory-budget-bytes").is_none(){
        if let Ok(v)=std::env::var("POCOL_MEMORY_BUDGET_BYTES"){l.mem_budget=v.parse().map_err(|e|format!("POCOL_MEMORY_BUDGET_BYTES {v:?}: {e}"))?;}
    }
    Ok(())
}
/// `pocold run`. The P2P settings are checked before the network is read, so a bad value fails at once: the seeds
/// (`--seed`, VA-SPEC §1a.8) are configured peers too, except this host's own (`is_own_seed`, which makes it a seed
/// node that keeps the others) and those already given as `--peer` (static); bootstrap addresses (`--bootstrap`,
/// §1a.9) are unverified entries; neither is allowed with discovery off; and the automatic slots must fit the 18
/// outbound places with the configured peers (`Discovery::resolve`, which `Node::open` repeats).
fn run(args:&[String])->Result<(),String>{
    let miner=pocold::keyfile::miner(args)?;
    let mut limits=limits(args)?;p2p_env(args,&mut limits)?;budget_env(args,&mut limits)?;
    let parse=|s:&str|s.parse::<std::net::SocketAddr>().map_err(|e|format!("{s}: {e}"));
    let listen=arg(args,"--listen").map(|s|parse(&s)).transpose()?;
    let mut peers:Vec<std::net::SocketAddr>=args_all(args,"--peer").iter().map(|s|parse(s)).collect::<Result<_,_>>()?;
    let seeds=node::endpoints(&args_all(args,"--seed"),"--seed",node::SEEDS_MAX,1)?;
    let bootstrap=node::endpoints(&args_all(args,"--bootstrap"),"--bootstrap",node::BOOTSTRAP_MAX,1024)?;
    if !limits.discovery.enabled&&!(seeds.is_empty()&&bootstrap.is_empty()){return Err("--seed and --bootstrap need peer discovery, which is off".into())}
    limits.discovery.is_seed=seeds.iter().any(|s|node::is_own_seed(listen,*s));
    let seeds:Vec<_>=seeds.into_iter().filter(|s|!node::is_own_seed(listen,*s)&&!peers.contains(s)).collect();
    peers.extend(seeds.iter().copied());limits.discovery.seeds=seeds;limits.discovery.bootstrap=bootstrap;
    limits.discovery.resolve(listen.is_some(),peers.len())?;
    let net=PathBuf::from(arg(args,"--network").ok_or("--network")?);
    let data=PathBuf::from(arg(args,"--data").ok_or("--data")?);
    let spec=load_spec_for(&net,&local_at(args,&net)?,limits.state_store.then_some(data.as_path()))?;
    let name=arg(args,"--name").unwrap_or_else(||"node".into());
    let cfg=node::Config{dir:data,name,miner,listen,peers,
        rpc:arg(args,"--rpc").map(|s|parse(&s)).transpose()?,
        stop_at:arg(args,"--stop-at").and_then(|s|s.parse().ok()),log:args.iter().any(|a|a=="--log"),limits};
    let mut n=node::Node::open(cfg,spec)?;
    if !n.chain.spec.admission().specification_compliant(){
        eprintln!("pocold: {} admission, not specification compliance ({} conditions; see status \"admission\")",n.chain.spec.admission().scope().name(),n.chain.spec.admission().report().conditions.len());
    }
    // The R14/R19 times this boot was admitted on (owner decision PG-1, PARAMGATE-BOOT-TIMING-DECISIONS-2026-11-11.md).
    if let Some(t)=n.chain.spec.admission().timing(){eprintln!("pocold: ParamGate admitted, {}",t.report(n.chain.spec.admission().local().t_exec));}
    n.log(&format!("opened: head h={} records={}",n.chain.head().height,n.journal_records()));
    catch_stop();n.start_network()?;n.run_controlled(|_|STOP.load(Ordering::Relaxed));
    println!("{}",n.status_json());Ok(())
}
/// The new network's genesis from a stopped old node's data at height S (EARN-IN-SPEC.md section 7).
fn cutover(args:&[String])->Result<(),String>{
    let net=PathBuf::from(arg(args,"--network").ok_or("--network OLD_GENESIS_DIR")?);let data=PathBuf::from(arg(args,"--data").ok_or("--data OLD_DATA_DIR")?);
    let height:u64=arg(args,"--height").ok_or("--height S")?.parse().map_err(|e|format!("--height: {e}"))?;
    let config_path=PathBuf::from(arg(args,"--config").ok_or("--config NEW.json")?);let out=PathBuf::from(arg(args,"--out").ok_or("--out NEW_DIR")?);
    let config=std::fs::read(&config_path).map_err(|e|format!("{}: {e}",config_path.display()))?;
    if config.len()>262144{return Err("new genesis config exceeds 256 KiB".into())}
    // V3-33: --export-chain DIR also writes the old-chain data export of S from the same rebuilt state.
    let export=arg(args,"--export-chain").map(PathBuf::from);
    pocold::cutover::run_with_export(&net,&data,height,&config,&out,export.as_deref(),limits(args)?,&local(args))
}
/// V3-33 (O-33=b): the old chain's canonical blocks up to H and the state after block H, with a manifest (`cutover::export`).
fn export_chain(args:&[String])->Result<(),String>{
    let net=PathBuf::from(arg(args,"--network").ok_or("--network OLD_GENESIS_DIR")?);let data=PathBuf::from(arg(args,"--data").ok_or("--data OLD_DATA_DIR")?);
    let height:u64=arg(args,"--to-height").ok_or("--to-height H")?.parse().map_err(|e|format!("--to-height: {e}"))?;
    let out=PathBuf::from(arg(args,"--out").ok_or("--out NEW_DIR")?);
    let m=pocold::cutover::export::run(&net,&data,height,&out,limits(args)?,&local(args))?;
    println!("{}",serde_json::to_string_pretty(&m).unwrap());Ok(())
}
/// The options `verify-export` takes, each with a value: a check the caller asked for is never skipped because its
/// option was misspelt, given without a value or as `--name=value`, so every other argument is refused.
const VERIFY_EXPORT_OPTIONS:[&str;9]=["--dir","--network","--manifest-sha256","--head-hash","--delta","--phi","--t-exec","--delta-rel","--gamma"];
/// V3-33: checks an export (`cutover::export::verify`); with --network, against this node's own network files.
fn verify_export(args:&[String])->Result<(),String>{
    let mut seen=Vec::new();let mut i=2;
    while i<args.len(){
        let k=args[i].as_str();
        if !VERIFY_EXPORT_OPTIONS.contains(&k){return Err(format!("verify-export: unknown argument {k:?} (options: {})",VERIFY_EXPORT_OPTIONS.join(" ")))}
        if seen.contains(&k){return Err(format!("verify-export: {k} is given twice"))}
        match args.get(i+1){Some(v) if !v.starts_with("--")=>{},_=>return Err(format!("verify-export: {k} needs a value"))}
        seen.push(k);i+=2;
    }
    let dir=PathBuf::from(arg(args,"--dir").ok_or("--dir EXPORT_DIR")?);
    // --network: the chain id, genesis hash and consensus parameters of this node's network files.
    let network=match arg(args,"--network"){Some(n)=>Some(pocold::cutover::export::Network::of(&load_spec_for(std::path::Path::new(&n),&local(args),None)?)),None=>None};
    // --manifest-sha256: the published digest of manifest.json, which commits every part. --head-hash: the hash of
    // block H from a source other than the publisher (the caller's own node); `verified` is true only with it.
    let (sha,head)=(arg(args,"--manifest-sha256"),arg(args,"--head-hash"));
    let r=pocold::cutover::export::verify(&dir,network.as_ref(),sha.as_deref(),head.as_deref())?;
    println!("{}",serde_json::to_string_pretty(&r).unwrap());Ok(())
}
fn status(args:&[String])->Result<(),String>{
    let limits=limits(args)?;let net=PathBuf::from(arg(args,"--network").ok_or("--network")?);let data=PathBuf::from(arg(args,"--data").ok_or("--data")?);
    let spec=load_spec_for(&net,&local_at(args,&net)?,limits.state_store.then_some(data.as_path()))?;
    let cfg=node::Config{dir:data,name:"status".into(),miner:None,listen:None,peers:vec![],rpc:None,stop_at:None,log:false,limits};
    let n=node::Node::open(cfg,spec)?;println!("{}",n.status_json());Ok(())
}
