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
        "wallet" | "import" | "upgrade" | "inspect-upgrade" => &["--home", "--network"],
        // Passphrase-encrypted key backup (CRY-02): the passphrase is read from stdin, never from an argument.
        "export-key" => &["--home", "--out"], "inspect-key-backup" => &["--home", "--backup"],
        "import-key" => &["--home", "--backup", "--replace"],
        "network" => &["--from", "--seed", "--out"], "inspect" => &["--network"],
        "build-info" => { println!("{}", pocold::identity::BUILD_INFO); return Ok(()); }
        _ => return Err("usage: pocol identity [--home DIR] | start [--home DIR] [--network FILE] [--listen IP:PORT] [--rpc 127.0.0.1:PORT] | network --from GENESIS_DIR --seed IP:PORT... --out FILE | inspect --network FILE | upgrade [--home DIR] --network FILE | inspect-upgrade [--home DIR] --network FILE | export-key [--home DIR] --out FILE | inspect-key-backup [--home DIR] --backup FILE | import-key [--home DIR] --backup FILE [--replace CURRENT_ADDRESS] (key backup passphrases are read from stdin: export-key reads it twice)".into()),
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
    if command == "export-key" {
        let out = Path::new(need("--out")?);
        if std::fs::symlink_metadata(out).is_ok() {
            return Err(format!("{} exists; an existing file is never replaced", out.display()));
        }
        if !home_path.is_dir() {
            return Err("export-key requires an existing PoCol home; it never creates a wallet".into());
        }
        let home = Home::open(&home_path)?;
        let pass = passphrase("Backup passphrase (at least 12 characters)")?;
        if *passphrase("Repeat the backup passphrase")? != *pass {
            return Err("the passphrases differ; nothing was exported".into());
        }
        let (file, address) = home.export_key_backup(&pass)?;
        write_new_private(out, &file)?;
        println!("{}", serde_json::json!({"address": address, "backup": out}));
        return Ok(());
    }
    if command == "inspect-key-backup" || command == "import-key" {
        let file = launcher::read_bounded(Path::new(need("--backup")?))?;
        let home = Home::open(&home_path)?;
        let pass = passphrase("Backup passphrase")?;
        let out = if command == "import-key" { home.import_key_backup(&file, &pass, get("--replace"))? } else { home.inspect_key_backup(&file, &pass)? };
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
    let mut next = Instant::now();
    home.run(parse("--listen")?, parse("--rpc")?, |node| {
        if Instant::now() >= next {
            println!("{}", node.status_json());
            next = Instant::now() + Duration::from_secs(10);
        }
        STOP.load(Ordering::Relaxed)
    })
}

/// A backup file that never replaces an existing path (mode 0600 on Unix): only ciphertext is written.
fn write_new_private(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|e| format!("create {}: {e} (an existing file is never replaced)", path.display()))?;
    file.write_all(bytes).and_then(|_| file.sync_all()).map_err(|e| e.to_string())
}

/// One passphrase line from stdin without its line ending. When stdin is a terminal the prompt goes to stderr and
/// echo is off while the line is typed; the Windows wallet writes the line through a pipe instead.
fn passphrase(prompt: &str) -> Result<zeroize::Zeroizing<Vec<u8>>, String> {
    use std::io::{BufRead, Read};
    let max = pocold::keyfile::backup::MAX_PASSPHRASE_BYTES;
    let echo = Echo::off();
    if echo.terminal {
        eprint!("{prompt}: ");
    }
    let mut line = zeroize::Zeroizing::new(Vec::with_capacity(max + 2));
    let read = std::io::stdin().lock().take(max as u64 + 2).read_until(b'\n', &mut line);
    if echo.terminal {
        eprintln!();
    }
    drop(echo);
    read.map_err(|e| e.to_string())?;
    if line.last() == Some(&b'\n') {
        line.pop();
        if line.last() == Some(&b'\r') {
            line.pop();
        }
    } else if line.is_empty() {
        return Err("no passphrase on stdin".into());
    }
    if line.len() > max {
        return Err(format!("passphrase exceeds {max} bytes"));
    }
    Ok(line)
}

/// Terminal echo switched off until drop (restored even on an error path).
struct Echo {
    terminal: bool,
    #[cfg(unix)]
    saved: Option<libc::termios>,
    #[cfg(windows)]
    saved: Option<(windows_sys::Win32::Foundation::HANDLE, windows_sys::Win32::System::Console::CONSOLE_MODE)>,
}
impl Echo {
    #[cfg(unix)]
    fn off() -> Self {
        unsafe {
            if libc::isatty(0) != 1 {
                return Self { terminal: false, saved: None };
            }
            let mut mode: libc::termios = std::mem::zeroed();
            if libc::tcgetattr(0, &mut mode) != 0 {
                return Self { terminal: true, saved: None };
            }
            let mut quiet = mode;
            quiet.c_lflag &= !libc::ECHO;
            let saved = (libc::tcsetattr(0, libc::TCSANOW, &quiet) == 0).then_some(mode);
            Self { terminal: true, saved }
        }
    }
    #[cfg(windows)]
    fn off() -> Self {
        use windows_sys::Win32::System::Console::{GetConsoleMode, GetStdHandle, SetConsoleMode, ENABLE_ECHO_INPUT, STD_INPUT_HANDLE};
        unsafe {
            let handle = GetStdHandle(STD_INPUT_HANDLE);
            let mut mode = 0;
            if GetConsoleMode(handle, &mut mode) == 0 {
                return Self { terminal: false, saved: None };
            }
            let saved = (SetConsoleMode(handle, mode & !ENABLE_ECHO_INPUT) != 0).then_some((handle, mode));
            Self { terminal: true, saved }
        }
    }
    #[cfg(not(any(unix, windows)))]
    fn off() -> Self {
        Self { terminal: false }
    }
}
impl Drop for Echo {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(mode) = self.saved.take() {
            unsafe { libc::tcsetattr(0, libc::TCSANOW, &mode) };
        }
        #[cfg(windows)]
        if let Some((handle, mode)) = self.saved.take() {
            unsafe { windows_sys::Win32::System::Console::SetConsoleMode(handle, mode) };
        }
    }
}
