//! Native standalone entry point. No Python, shell, Docker or private-key arguments required.
use pocold::{
    launcher::{self, Home, Invitation},
    threads,
};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};
static STOP: AtomicBool = AtomicBool::new(false);
#[cfg(unix)]
extern "C" fn signal(_: i32) {
    STOP.store(true, Ordering::Relaxed);
}
#[cfg(windows)]
unsafe extern "system" fn console(event: u32) -> i32 {
    if event == 0 || event == 1 || event == 2 {
        STOP.store(true, Ordering::Relaxed);
        1
    } else {
        0
    }
}
fn main() {
    #[cfg(unix)]
    unsafe {
        libc::signal(libc::SIGINT, signal as *const () as usize);
        libc::signal(libc::SIGTERM, signal as *const () as usize);
    }
    #[cfg(windows)]
    unsafe {
        windows_sys::Win32::System::Console::SetConsoleCtrlHandler(Some(console), 1);
    }
    let result = threads::spawn("consensus", threads::CONSENSUS_STACK, command).map(|t| t.join());
    let code = match result {
        Ok(Ok(Ok(()))) => 0,
        Ok(Ok(Err(e))) => {
            eprintln!("PoCol: {e}");
            1
        }
        _ => 101,
    };
    std::process::exit(code);
}
fn command() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let command = args.first().map(String::as_str).unwrap_or("start");
    let rest = args.get(1..).unwrap_or_default();
    let allowed: &[&str] = match command {
        "identity" => &["--home"], "start" => &["--home", "--network", "--listen", "--rpc", "--peer"],
        "wallet" | "import" | "upgrade" | "inspect-upgrade" => &["--home", "--network"],
        "network" => &["--from", "--seed", "--bootstrap", "--seeds-retired", "--out"], "inspect" => &["--network"],
        "build-info" => { println!("{}", pocold::identity::BUILD_INFO); return Ok(()); }
        _ => return Err("usage: pocol identity [--home DIR] | start [--home DIR] [--network FILE] [--listen IP:PORT] [--rpc 127.0.0.1:PORT] [--peer IP:PORT]... | network --from GENESIS_DIR --seed IP:PORT... [--bootstrap IP:PORT]... [--seeds-retired true|false] --out FILE | inspect --network FILE | upgrade [--home DIR] --network FILE | inspect-upgrade [--home DIR] --network FILE".into()),
    };
    let mut values = std::collections::BTreeMap::<String, Vec<String>>::new();
    if rest.len() % 2 != 0 {
        return Err("each option requires a value".into());
    }
    for pair in rest.chunks_exact(2) {
        if !allowed.contains(&pair[0].as_str()) {
            return Err(format!("unknown option {}", pair[0]));
        }
        let v = values.entry(pair[0].clone()).or_default();
        if !v.is_empty() && !matches!(pair[0].as_str(), "--seed" | "--peer" | "--bootstrap") {
            return Err(format!("duplicate option {}", pair[0]));
        }
        v.push(pair[1].clone());
    }
    let get = |k: &str| values.get(k).and_then(|v| v.first()).map(String::as_str);
    let need = |k: &str| get(k).ok_or_else(|| format!("missing {k}"));
    if command == "network" {
        let seeds = values
            .get("--seed")
            .ok_or("missing --seed")?
            .iter()
            .map(|s| s.parse().map_err(|_| "invalid seed".to_string()))
            .collect::<Result<_, _>>()?;
        // D17, D23: the optional bootstrap list and the seeds' retirement (both outside the network's identity).
        let bootstrap = values.get("--bootstrap").map(Vec::as_slice).unwrap_or(&[]).iter()
            .map(|s| s.parse().map_err(|_| format!("invalid bootstrap address {s}"))).collect::<Result<_, _>>()?;
        let retired = match get("--seeds-retired") { None | Some("false") => false, Some("true") => true, Some(v) => return Err(format!("--seeds-retired {v}: expected true or false")) };
        return launcher::pack(Path::new(need("--from")?), seeds, bootstrap, retired, Path::new(need("--out")?));
    }
    if command == "inspect" {
        println!(
            "{}",
            Invitation::parse(&launcher::read_bounded(Path::new(need("--network")?))?)?.describe()
        );
        return Ok(());
    }
    let home_path = get("--home")
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(launcher::default_home)?;
    if command == "upgrade" || command == "inspect-upgrade" {
        let network = Path::new(need("--network")?);
        if !home_path.is_dir() {
            return Err(format!("{command} requires an existing PoCol home; it never creates a wallet"));
        }
        let home = Home::open(&home_path)?;
        let bytes = launcher::read_bounded(network)?;
        // inspect-upgrade is the read-only dry run (change none, rebind or activation); it writes nothing.
        let out = if command == "upgrade" { home.upgrade(&bytes)?.describe() } else { home.inspect_upgrade(&bytes)? };
        println!("{out}");
        return Ok(());
    }
    let home = Home::open(&home_path)?;
    println!(
        "{}",
        serde_json::json!({"address":home.address()?,"home":home_path,"scope":"experimental-testnet"})
    );
    if command == "identity" {
        return Ok(());
    }
    let adjacent = std::env::current_exe()
        .map_err(|e| e.to_string())?
        .with_file_name("network.pocol.json");
    let saved = home.path.join("network.pocol.json");
    let network = get("--network")
        .map(PathBuf::from)
        // An explicit upgrade replaces the home's binding, not the invitation
        // shipped beside the executable. Revalidate that saved binding first.
        // Even a damaged existing binding must fail, rather than bootstrap anew.
        .or_else(|| std::fs::symlink_metadata(&saved).is_ok().then_some(saved))
        .or_else(|| adjacent.is_file().then_some(adjacent));
    if let Some(network) = network {
        println!(
            "{}",
            home.import(&launcher::read_bounded(&network)?)?.describe()
        );
    }
    if !home.path.join("network.pocol.json").is_file() {
        return Err("identity ready. Install the shared network.pocol.json beside PoCol once, then start again. Share only the public address above.".into());
    }
    if command=="import" {return Ok(())}
    if command=="wallet" {return pocold::wallet::stdio(&home,||STOP.load(Ordering::Relaxed))}
    let parse = |key: &str| {
        get(key)
            .map(|s| s.parse().map_err(|_| format!("invalid {key}")))
            .transpose()
    };
    // Static peers besides the invitation's seeds (at most 7, each its own configured peer).
    let peers = pocold::node::endpoints(values.get("--peer").map(Vec::as_slice).unwrap_or(&[]), "--peer", 7, 1)?;
    let mut next = Instant::now();
    home.run(parse("--listen")?, parse("--rpc")?, &peers, |node| {
        if Instant::now() >= next {
            println!("{}", node.status_json());
            next = Instant::now() + Duration::from_secs(10);
        }
        STOP.load(Ordering::Relaxed)
    })
}
