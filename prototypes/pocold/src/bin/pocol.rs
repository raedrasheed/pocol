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
        "identity" => &["--home"], "start" => &["--home", "--network", "--listen", "--rpc"],
        "wallet" | "import" => &["--home", "--network"],
        "network" => &["--from", "--seed", "--out"], "inspect" => &["--network"],
        "build-info" => { println!("{}", pocold::identity::BUILD_INFO); return Ok(()); }
        _ => return Err("usage: pocol identity [--home DIR] | start [--home DIR] [--network FILE] [--listen IP:PORT] [--rpc 127.0.0.1:PORT] | network --from GENESIS_DIR --seed IP:PORT... --out FILE | inspect --network FILE".into()),
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
        if !v.is_empty() && pair[0] != "--seed" {
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
        return launcher::pack(Path::new(need("--from")?), seeds, Path::new(need("--out")?));
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
    let network = get("--network")
        .map(PathBuf::from)
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
    let mut next = Instant::now();
    home.run(parse("--listen")?, parse("--rpc")?, |node| {
        if Instant::now() >= next {
            println!("{}", node.status_json());
            next = Instant::now() + Duration::from_secs(10);
        }
        STOP.load(Ordering::Relaxed)
    })
}
