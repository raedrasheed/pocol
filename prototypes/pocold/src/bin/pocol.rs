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
        // --key-credential NAME (K1, Linux servers): the key is the systemd credential NAME in $CREDENTIALS_DIRECTORY
        // (LoadCredentialEncrypted=), never the plaintext key file (deploy/standalone/encrypt-server-key.sh).
        "identity" => &["--home", "--key-credential"], "start" => &["--home", "--network", "--listen", "--rpc", "--peer", "--key-credential"],
        "wallet" | "import" => &["--home", "--network"],
        "upgrade" | "inspect-upgrade" => &["--home", "--network", "--key-credential"],
        // Passphrase-encrypted key backup (CRY-02): the passphrase is read from stdin, never from an argument.
        "export-key" => &["--home", "--out", "--key-credential"], "inspect-key-backup" => &["--home", "--backup"],
        "import-key" => &["--home", "--backup", "--replace"],
        "network" => &["--from", "--seed", "--bootstrap", "--seeds-retired", "--out"], "inspect" => &["--network"],
        // This device's mining rate (R25d input for the genesis ceremony's -HashesPerSecond); reads and writes nothing.
        "hashrate" => &["--seconds"],
        "build-info" => { println!("{}", pocold::readiness::build_info()); return Ok(()); }
        _ => return Err("usage: pocol identity [--home DIR] [--key-credential NAME] | start [--home DIR] [--network FILE] [--listen IP:PORT] [--rpc 127.0.0.1:PORT] [--peer IP:PORT]... [--key-credential NAME] | network --from GENESIS_DIR --seed IP:PORT... [--bootstrap IP:PORT]... [--seeds-retired true|false] --out FILE | inspect --network FILE | hashrate [--seconds 1..600] | upgrade [--home DIR] --network FILE [--key-credential NAME] | inspect-upgrade [--home DIR] --network FILE [--key-credential NAME] | export-key [--home DIR] --out FILE [--key-credential NAME] | inspect-key-backup [--home DIR] --backup FILE | import-key [--home DIR] --backup FILE [--replace CURRENT_ADDRESS] (key backup passphrases are read from stdin: export-key reads it twice; --key-credential NAME reads the key from the systemd credential NAME in $CREDENTIALS_DIRECTORY, never from the key file)".into()),
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
    // With --key-credential the home's key is that systemd credential; an unset $CREDENTIALS_DIRECTORY or a missing
    // credential is refused when the key is read, never answered from the key file.
    let keyed = |home: Home| match get("--key-credential") {
        Some(name) => home.with_key_credential(name, std::env::var_os("CREDENTIALS_DIRECTORY").map(PathBuf::from)),
        None => Ok(home),
    };
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
    if command == "hashrate" {
        // Before any home is opened: it reads no key and no network and writes nothing. Ctrl-C cancels it (STOP).
        let seconds = match get("--seconds") {
            None => 30,
            Some(v) => v.parse::<u64>().ok().filter(|s| (1..=600).contains(s)).ok_or_else(|| format!("invalid --seconds {v}: expected 1 to 600"))?,
        };
        let m = pocold::miner::measure(seconds, &|| STOP.load(Ordering::Relaxed))?;
        println!(
            "{}",
            serde_json::json!({"hashesPerSecond": m.rate, "hashes": m.hashes, "elapsedMs": m.elapsed.as_millis() as u64, "seconds": seconds,
                "threads": pocold::miner::MEASURE_THREADS,
                "note": "this device, on the one miner thread pocol start and the wallet mine with; -HashesPerSecond is the sum of the four founders' values"})
        );
        return Ok(());
    }
    let home_path = get("--home")
        .map(PathBuf::from)
        .map(Ok)
        .unwrap_or_else(launcher::default_home)?;
    if command == "upgrade" || command == "inspect-upgrade" {
        let network = Path::new(need("--network")?);
        let home = keyed(Home::open_existing(&home_path, command)?)?;
        let bytes = launcher::read_bounded(network)?;
        // inspect-upgrade is the read-only dry run (change none, rebind or activation); it writes nothing.
        let out = if command == "upgrade" { home.upgrade(&bytes)?.describe() } else { home.inspect_upgrade(&bytes)? };
        println!("{out}");
        return Ok(());
    }
    if command == "export-key" {
        let out = Path::new(need("--out")?);
        if std::fs::symlink_metadata(out).is_ok() {
            return Err(format!("{} exists; an existing file is never replaced", out.display()));
        }
        let home = keyed(Home::open_existing(&home_path, command)?)?;
        // An unreadable key credential is refused before any passphrase is asked for.
        if get("--key-credential").is_some() {
            home.key()?;
        }
        let pass = passphrase("Backup passphrase (at least 12 characters)")?;
        if *passphrase("Repeat the backup passphrase")? != *pass {
            return Err("the passphrases differ; nothing was exported".into());
        }
        let (file, address) = home.export_key_backup(&pass)?;
        pocold::keyfile::backup::write_new(out, &file)?;
        println!("{}", serde_json::json!({"address": address, "backup": out}));
        return Ok(());
    }
    if command == "inspect-key-backup" || command == "import-key" {
        let file = launcher::read_bounded(Path::new(need("--backup")?))?;
        // inspect-key-backup is read-only: it never creates a home. import-key may restore into a new one.
        let home = if command == "import-key" { Home::open(&home_path)? } else { Home::open_existing(&home_path, command)? };
        let pass = passphrase("Backup passphrase")?;
        let out = if command == "import-key" { home.import_key_backup(&file, &pass, get("--replace"))? } else { home.inspect_key_backup(&file, &pass)? };
        println!("{out}");
        return Ok(());
    }
    // A key credential serves an existing home only, so a mistyped --home creates nothing.
    let home = keyed(if get("--key-credential").is_some() { Home::open_existing(&home_path, command)? } else { Home::open(&home_path)? })?;
    println!(
        "{}",
        serde_json::json!({"address":home.address()?,"home":home_path,"scope":"mainnet"})
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

/// One passphrase line from stdin (`keyfile::prompt::passphrase`: echo off on a terminal, refused when it cannot be
/// switched off). Ctrl-C cancels the prompt. The Windows wallet writes the line through a pipe instead.
fn passphrase(prompt: &str) -> Result<zeroize::Zeroizing<Vec<u8>>, String> {
    pocold::keyfile::prompt::passphrase(prompt, &|| STOP.load(Ordering::Relaxed))
}
