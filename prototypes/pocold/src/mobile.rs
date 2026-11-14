//! Narrow C ABI used by the Android JNI bridge. Java never receives a private key.
use crate::{
    launcher::{self, Home, Invitation},
    threads,
};
use std::{
    ffi::{c_char, CStr, CString},
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::{Duration, Instant},
};
static STOP: AtomicBool = AtomicBool::new(false);
static STATUS: Mutex<String> = Mutex::new(String::new());
static COMMANDS: Mutex<std::collections::VecDeque<String>> = Mutex::new(std::collections::VecDeque::new());
fn reply(result: Result<serde_json::Value, String>) -> *mut c_char {
    let value = result.unwrap_or_else(|e| serde_json::json!({"error":e}));
    CString::new(value.to_string())
        .expect("JSON contains no NUL")
        .into_raw()
}
unsafe fn text<'a>(s: *const c_char) -> Result<&'a str, String> {
    if s.is_null() {
        return Err("missing native argument".into());
    }
    unsafe { CStr::from_ptr(s) }
        .to_str()
        .map_err(|_| "native argument must be UTF-8".into())
}
#[no_mangle]
pub unsafe extern "C" fn pocol_mobile_identity(home: *const c_char) -> *mut c_char {
    reply(std::panic::catch_unwind(|| {
        let home = Home::open(Path::new(unsafe { text(home)? }))?;
        use crate::identity as id;
        Ok(serde_json::json!({"address":home.address()?,"compatibilitySha256":id::COMPATIBILITY_HEX,"consensusRulesVersion":id::CONSENSUS_RULES_VERSION,
            "protocolVersion":id::PROTOCOL_VERSION,"protocolVersionMin":id::PROTOCOL_VERSION_MIN}))
    }).unwrap_or_else(|_| Err("identity failed".into())))
}
#[no_mangle]
pub unsafe extern "C" fn pocol_mobile_import(
    home: *const c_char,
    path: *const c_char,
) -> *mut c_char {
    reply(
        std::panic::catch_unwind(|| {
            let bytes = launcher::read_bounded(Path::new(unsafe { text(path)? }))?;
            Ok(Home::open(Path::new(unsafe { text(home)? }))?
                .import(&bytes)?
                .describe())
        })
        .unwrap_or_else(|_| Err("network import failed".into())),
    )
}
/// Public configuration preview. It cannot import, upgrade or start a wallet.
#[no_mangle]
pub unsafe extern "C" fn pocol_mobile_inspect_network(path: *const c_char) -> *mut c_char {
    reply(std::panic::catch_unwind(|| {
        let bytes = launcher::read_bounded(Path::new(unsafe { text(path)? }))?;
        Ok(Invitation::parse(&bytes)?.describe())
    }).unwrap_or_else(|_| Err("network inspection failed".into())))
}
/// Explicit user-confirmed upgrade; Home's lock refuses a running wallet.
#[no_mangle]
pub unsafe extern "C" fn pocol_mobile_upgrade(home: *const c_char, path: *const c_char) -> *mut c_char {
    reply(std::panic::catch_unwind(|| {
        let home = Path::new(unsafe { text(home)? });
        if !home.is_dir() { return Err("upgrade requires an existing PoCol home".into()); }
        let bytes = launcher::read_bounded(Path::new(unsafe { text(path)? }))?;
        Ok(Home::open(home)?.upgrade(&bytes)?.describe())
    }).unwrap_or_else(|_| Err("network upgrade failed".into())))
}
/// Read-only dry run of an upgrade (`Home::inspect_upgrade`): what applying the file would change, for the confirmation.
#[no_mangle]
pub unsafe extern "C" fn pocol_mobile_inspect_upgrade(home: *const c_char, path: *const c_char) -> *mut c_char {
    reply(std::panic::catch_unwind(|| {
        let home = Path::new(unsafe { text(home)? });
        if !home.is_dir() { return Err("upgrade inspection requires an existing PoCol home".into()); }
        let bytes = launcher::read_bounded(Path::new(unsafe { text(path)? }))?;
        Home::open(home)?.inspect_upgrade(&bytes)
    }).unwrap_or_else(|_| Err("network upgrade inspection failed".into())))
}
/// Borrowed bytes from the bridge (a passphrase or a backup file), bounded so a bad length cannot read far.
unsafe fn bytes<'a>(p: *const u8, len: usize, max: usize) -> Result<&'a [u8], String> {
    if p.is_null() || len > max {
        return Err("missing or oversized native argument".into());
    }
    Ok(unsafe { std::slice::from_raw_parts(p, len) })
}
const MAX_BACKUP_FILE: usize = 4096;
/// A passphrase-encrypted key backup (`keyfile::backup`, CRY-02). Only ciphertext crosses to Java (hex in
/// `backup`), which writes it where the user chose; the plaintext key never leaves Rust or touches a disk.
#[no_mangle]
pub unsafe extern "C" fn pocol_mobile_export_key(home: *const c_char, pass: *const u8, pass_len: usize) -> *mut c_char {
    reply(std::panic::catch_unwind(|| {
        let home = Path::new(unsafe { text(home)? });
        if !home.is_dir() { return Err("key backup requires an existing PoCol wallet".into()); }
        let pass = unsafe { bytes(pass, pass_len, crate::keyfile::backup::MAX_PASSPHRASE_BYTES)? };
        let (file, address) = Home::open(home)?.export_key_backup(pass)?;
        let name = format!("pocol-key-{}.{}", &address[..10], crate::keyfile::backup::EXTENSION);
        Ok(serde_json::json!({"address": address, "backup": crate::types::hex(&file), "fileName": name}))
    }).unwrap_or_else(|_| Err("key backup failed".into())))
}
/// Read-only: the backup's address and whether restoring it replaces this wallet's account (`Home::inspect_key_backup`).
#[no_mangle]
pub unsafe extern "C" fn pocol_mobile_inspect_key_backup(home: *const c_char, file: *const u8, file_len: usize, pass: *const u8, pass_len: usize) -> *mut c_char {
    reply(std::panic::catch_unwind(|| {
        let home = Path::new(unsafe { text(home)? });
        let (file, pass) = unsafe { (bytes(file, file_len, MAX_BACKUP_FILE)?, bytes(pass, pass_len, crate::keyfile::backup::MAX_PASSPHRASE_BYTES)?) };
        Home::open_existing(home, "key backup inspection")?.inspect_key_backup(file, pass)
    }).unwrap_or_else(|_| Err("key backup inspection failed".into())))
}
/// Restores a backup. `replace` is "" or the current address the user confirmed replacing (`Home::import_key_backup`).
#[no_mangle]
pub unsafe extern "C" fn pocol_mobile_import_key(home: *const c_char, file: *const u8, file_len: usize, pass: *const u8, pass_len: usize, replace: *const c_char) -> *mut c_char {
    reply(std::panic::catch_unwind(|| {
        let home = Path::new(unsafe { text(home)? });
        let (file, pass) = unsafe { (bytes(file, file_len, MAX_BACKUP_FILE)?, bytes(pass, pass_len, crate::keyfile::backup::MAX_PASSPHRASE_BYTES)?) };
        let replace = unsafe { text(replace)? };
        Home::open(home)?.import_key_backup(file, pass, (!replace.is_empty()).then_some(replace))
    }).unwrap_or_else(|_| Err("key restore failed".into())))
}
/// What one pass of the mobile engine loop drives: the wallet engine (`crate::wallet::Wallet`), as the desktop stdio loop
/// drives it.
pub(crate) trait Engine<N> {
    fn handle(&mut self, n: &mut N, raw: &str) -> serde_json::Value;
    fn status(&self, n: &N) -> serde_json::Value;
    fn advance(&mut self, n: &mut N) -> Option<serde_json::Value>;
}
impl Engine<crate::node::Node> for crate::wallet::Wallet<'_> {
    fn handle(&mut self, n: &mut crate::node::Node, raw: &str) -> serde_json::Value { crate::wallet::Wallet::handle(self, n, raw) }
    fn status(&self, n: &crate::node::Node) -> serde_json::Value { crate::wallet::Wallet::status(self, n) }
    fn advance(&mut self, n: &mut crate::node::Node) -> Option<serde_json::Value> { crate::wallet::Wallet::advance(self, n) }
}
/// The Android engine loop (`pocol_mobile_run`). A command's answer is published at once as `response` (with
/// `responseId`); every 2 s, as in the desktop stdio loop, `advance` moves a confirmed rejoin forward (it signs the
/// reactivation when the window opens) and its event, if any, is published as `event` (with `eventId`), apart from the
/// command answers so neither hides the other.
#[derive(Default)]
pub(crate) struct MobileLoop {
    response: serde_json::Value,
    response_id: u64,
    event: serde_json::Value,
    event_id: u64,
    next: Option<Instant>,
}
impl MobileLoop {
    /// One pass with the queued `command`, if any; the status text to publish when one is due.
    pub(crate) fn tick<N>(&mut self, wallet: &mut impl Engine<N>, node: &mut N, command: Option<String>, now: Instant) -> Option<String> {
        if let Some(command) = command {
            self.response = wallet.handle(node, &command);
            self.response_id += 1;
            self.next = Some(now);
        }
        if self.next.is_some_and(|t| now < t) {
            return None;
        }
        if let Some(event) = wallet.advance(node) {
            self.event = event;
            self.event_id += 1;
        }
        let mut v = wallet.status(node);
        v["response"] = self.response.clone();
        v["responseId"] = serde_json::json!(self.response_id);
        v["event"] = self.event.clone();
        v["eventId"] = serde_json::json!(self.event_id);
        self.next = Some(now + Duration::from_secs(2));
        Some(v.to_string())
    }
}
#[no_mangle]
pub unsafe extern "C" fn pocol_mobile_run(home: *const c_char) -> *mut c_char {
    let result = std::panic::catch_unwind(|| {
        let path = unsafe { text(home)? }.to_owned();
        let handle = threads::spawn("consensus", threads::CONSENSUS_STACK, move || {
            let home = Home::open(Path::new(&path))?;
            let mut wallet=crate::wallet::Wallet::new(&home)?;
            let mut engine=MobileLoop::default();
            home.run_mode(None, None, &[], false, |node| {
                let command=COMMANDS.lock().ok().and_then(|mut q|q.pop_front());
                if let Some(text)=engine.tick(&mut wallet,node,command,Instant::now()){
                    if let Ok(mut status) = STATUS.lock() {*status=text;}
                }
                STOP.load(Ordering::Relaxed)||wallet.stopped
            })?;
            Ok(serde_json::json!({"state":"stopped"}))
        })
        .map_err(|e| e.to_string())?;
        handle
            .join()
            .map_err(|_| "node thread failed".to_string())?
    })
    .unwrap_or_else(|_| Err("native node failed".into()));
    reply(result)
}
#[no_mangle]
pub unsafe extern "C" fn pocol_mobile_command(command:*const c_char)->*mut c_char{
    reply(std::panic::catch_unwind(||{
        let command=unsafe{text(command)?};
        if command.len()>crate::wallet::MAX_COMMAND{return Err("Command exceeds 4 KiB".into())}
        serde_json::from_str::<crate::wallet::Command>(command).map_err(|_|"Invalid wallet command")?;
        let mut q=COMMANDS.lock().map_err(|_|"Wallet unavailable")?;
        if q.len()>=8{return Err("Wallet busy, try again".into())}q.push_back(command.to_owned());
        Ok(serde_json::json!({"queued":true}))
    }).unwrap_or_else(|_|Err("Wallet command failed".into())))
}
#[no_mangle]
pub extern "C" fn pocol_mobile_stop() {
    STOP.store(true, Ordering::Relaxed);
}
#[no_mangle]
pub extern "C" fn pocol_mobile_status() -> *mut c_char {
    let text = STATUS
        .lock()
        .ok()
        .map(|s| s.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "{\"state\":\"starting\"}".into());
    CString::new(text).expect("JSON contains no NUL").into_raw()
}
#[no_mangle]
pub unsafe extern "C" fn pocol_mobile_free(s: *mut c_char) {
    if !s.is_null() {
        drop(unsafe { CString::from_raw(s) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{collections::BTreeMap, path::PathBuf};
    #[test]
    fn explicit_upgrade_does_not_create_a_missing_wallet() {
        let path = std::env::temp_dir().join(format!("pocol-mobile-upgrade-missing-{}", std::process::id()));
        assert!(!path.exists());
        let home = CString::new(path.to_str().unwrap()).unwrap();
        let input = CString::new("not-a-network-file").unwrap();
        // The dry run (plan §1.7) is read-only too: it never creates a home either.
        for call in [pocol_mobile_upgrade, pocol_mobile_inspect_upgrade] {
            unsafe {
                let result = call(home.as_ptr(), input.as_ptr());
                let value: serde_json::Value = serde_json::from_str(CStr::from_ptr(result).to_str().unwrap()).unwrap();
                pocol_mobile_free(result);
                assert!(value["error"].as_str().unwrap().contains("existing PoCol home"));
            }
        }
        assert!(!path.exists());
    }
    /// The bridge's JSON reply to `call(home, file)`.
    fn ask(call: unsafe extern "C" fn(*const c_char, *const c_char) -> *mut c_char, home: &Path, file: &Path) -> serde_json::Value {
        let (home, file) = (CString::new(home.to_str().unwrap()).unwrap(), CString::new(file.to_str().unwrap()).unwrap());
        unsafe {
            let result = call(home.as_ptr(), file.as_ptr());
            let value = serde_json::from_str(CStr::from_ptr(result).to_str().unwrap()).unwrap();
            pocol_mobile_free(result);
            value
        }
    }
    #[test]
    fn inspect_upgrade_reports_none_rebind_activation_and_an_interrupted_upgrade_to_resume() {
        // T33: a generated continuous spec-1 network (`pocold init-testnet`) under pocol-network-v2 (this release's rules), as
        // `pocol network` packs it from R1 on.
        let dir = std::env::temp_dir().join(format!("pocol-mobile-inspect-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).unwrap();
        let miner = pocol_lp4_execution::devnet::miner_identity("mobile-inspect");
        let config = serde_json::json!({"continuous":true,"chainId":777918,"genesisTime":1_800_000_000u64,"registrationPerBlock":1,
            "miners":[{"id":miner.id,"reward":miner.reward}],
            "accounts":[{"address":pocol_lp4_execution::devnet::account_key("mobile-inspect").address(),"wei":"1000000000000000000000000"}]});
        crate::testnet::prepare(config.to_string().as_bytes(), &dir.join("genesis")).unwrap();
        let files: serde_json::Map<String, serde_json::Value> = ["genesis.spec.hex", "alloc.json", "devnet.json"].iter()
            .map(|n| (n.to_string(), std::fs::read_to_string(dir.join("genesis").join(n)).unwrap().into())).collect();
        let v2 = serde_json::json!({"schema": launcher::SCHEMA_V2, "consensusRulesVersion": crate::identity::CONSENSUS_RULES_VERSION,
            "seeds": ["127.0.0.1:30300"], "files": files});
        let devnet = |forks: serde_json::Value, pretty: bool| {
            let mut info: serde_json::Value = serde_json::from_str(v2["files"]["devnet.json"].as_str().unwrap()).unwrap();
            info["forks"] = forks;
            let mut file = v2.clone();
            file["files"]["devnet.json"] = if pretty { serde_json::to_string_pretty(&info) } else { serde_json::to_string(&info) }.unwrap().into();
            file
        };
        let write = |name: &str, value: &serde_json::Value| { let p = dir.join(name); std::fs::write(&p, value.to_string()).unwrap(); p };
        let base = write("base.json", &devnet(serde_json::json!([[1, 0]]), false));
        let rebind = write("rebind.json", &devnet(serde_json::json!([[1, 0]]), true));
        let activation = write("activation.json", &devnet(serde_json::json!([[1, 0], [2, 1000]]), false));
        let home = dir.join("home");
        let identity = CString::new(home.to_str().unwrap()).unwrap();
        unsafe { pocol_mobile_free(pocol_mobile_identity(identity.as_ptr())) };
        assert!(ask(pocol_mobile_import, &home, &base).get("error").is_none(), "the v2 file imports into a fresh home");
        let files = |dir: &Path| -> BTreeMap<PathBuf, Vec<u8>> {
            let mut out = BTreeMap::new(); let mut stack = vec![dir.to_path_buf()];
            while let Some(d) = stack.pop() { for e in std::fs::read_dir(&d).unwrap() { let p = e.unwrap().path();
                if p.is_dir() { stack.push(p) } else { out.insert(p.clone(), std::fs::read(&p).unwrap()); } } }
            out
        };
        let before = files(&home);
        for (file, change, appended) in [(&base, "none", serde_json::json!([])), (&rebind, "rebind", serde_json::json!([])),
                                         (&activation, "activation", serde_json::json!([[2, 1000]]))] {
            let dry = ask(pocol_mobile_inspect_upgrade, &home, file);
            assert_eq!((dry["change"].as_str(), &dry["appended"]), (Some(change), &appended), "{dry}");
            assert_eq!(files(&home), before, "the dry run writes nothing ({change})");
        }
        // The identity a pinned release node reports on the activated schedule (identity.rs pins the formula, T11).
        let info: serde_json::Value = serde_json::from_str(v2["files"]["devnet.json"].as_str().unwrap()).unwrap();
        let genesis: [u8; 32] = crate::types::unhex(info["genesisHash"].as_str().unwrap()).unwrap().try_into().unwrap();
        let expected = crate::identity::Consensus::new(777918, genesis, &[(1, 0), (2, 1000)], crate::identity::CONSENSUS_RULES_VERSION, 0, None).hex();
        assert_eq!(ask(pocol_mobile_inspect_upgrade, &home, &activation)["consensusIdentity"], expected.as_str());
        assert_ne!(ask(pocol_mobile_inspect_upgrade, &home, &base)["consensusIdentity"], expected.as_str(), "the activation changes the identity");
        // An upgrade killed after its intent (the app has no CLI): the same file's dry run offers to resume it, so the
        // confirm dialog opens and the upgrade finishes; another file is refused.
        let read = |p: &Path| -> serde_json::Value { serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap() };
        std::fs::write(home.join("network-upgrade.pending.json"), serde_json::json!({"before": read(&base), "after": read(&activation)}).to_string()).unwrap();
        std::fs::write(home.join("network/devnet.json"), read(&activation)["files"]["devnet.json"].as_str().unwrap()).unwrap();
        let dry = ask(pocol_mobile_inspect_upgrade, &home, &activation);
        assert_eq!((dry["change"].as_str(), &dry["resuming"]), (Some("activation"), &serde_json::json!(true)), "{dry}");
        assert!(ask(pocol_mobile_inspect_upgrade, &home, &rebind)["error"].as_str().unwrap().contains("same network invitation"));
        assert!(ask(pocol_mobile_upgrade, &home, &activation).get("error").is_none());
        assert!(!home.join("network-upgrade.pending.json").exists());
        assert_eq!(ask(pocol_mobile_inspect_upgrade, &home, &activation)["change"], "none");
        std::fs::remove_dir_all(&dir).unwrap();
    }
    fn json(result: *mut c_char) -> serde_json::Value {
        let value = serde_json::from_str(unsafe { CStr::from_ptr(result) }.to_str().unwrap()).unwrap();
        unsafe { pocol_mobile_free(result) };
        value
    }
    #[test]
    fn key_backup_round_trips_through_the_bridge_and_only_ciphertext_crosses_it() {
        let dir = std::env::temp_dir().join(format!("pocol-mobile-backup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let (old, new) = (CString::new(dir.join("old").to_str().unwrap()).unwrap(), CString::new(dir.join("new").to_str().unwrap()).unwrap());
        let pass = "a long backup passphrase".as_bytes();
        unsafe {
            // Export never creates a wallet.
            let missing = json(pocol_mobile_export_key(old.as_ptr(), pass.as_ptr(), pass.len()));
            assert!(missing["error"].as_str().unwrap().contains("existing PoCol wallet"));
            assert!(!dir.join("old").exists());
            let address = json(pocol_mobile_identity(old.as_ptr()))["address"].as_str().unwrap().to_owned();
            let secret = std::fs::read(dir.join("old/identity.pocol-key")).unwrap();
            let short = json(pocol_mobile_export_key(old.as_ptr(), pass.as_ptr(), 5));
            assert!(short["error"].as_str().unwrap().contains("at least 12"));
            let out = json(pocol_mobile_export_key(old.as_ptr(), pass.as_ptr(), pass.len()));
            assert_eq!(out["address"], address.as_str());
            assert_eq!(out["fileName"], format!("pocol-key-{}.pocol-backup", &address[..10]));
            let hex = out["backup"].as_str().unwrap();
            let file = crate::types::unhex(hex).unwrap();
            assert_eq!(file.len(), crate::keyfile::backup::LEN);
            assert!(!hex.contains(&crate::types::hex(&secret)[2..]), "the plaintext key never crosses the bridge");
            let new_identity = json(pocol_mobile_identity(new.as_ptr()))["address"].as_str().unwrap().to_owned();
            let preview = json(pocol_mobile_inspect_key_backup(new.as_ptr(), file.as_ptr(), file.len(), pass.as_ptr(), pass.len()));
            assert_eq!((&preview["address"], &preview["current"], &preview["replaces"]), (&serde_json::json!(address), &serde_json::json!(new_identity), &serde_json::json!(true)));
            let none = CString::new("").unwrap();
            let refused = json(pocol_mobile_import_key(new.as_ptr(), file.as_ptr(), file.len(), pass.as_ptr(), pass.len(), none.as_ptr()));
            assert!(refused["error"].as_str().unwrap().contains("Confirm replacing it"));
            let wrong = json(pocol_mobile_import_key(new.as_ptr(), file.as_ptr(), file.len(), pass.as_ptr(), pass.len() - 1, none.as_ptr()));
            assert!(wrong["error"].as_str().unwrap().contains("Wrong passphrase"));
            let confirm = CString::new(new_identity.as_str()).unwrap();
            let done = json(pocol_mobile_import_key(new.as_ptr(), file.as_ptr(), file.len(), pass.as_ptr(), pass.len(), confirm.as_ptr()));
            assert_eq!(done["changed"], true, "{done}");
            assert_eq!(json(pocol_mobile_identity(new.as_ptr()))["address"], address.as_str());
            let oversized = json(pocol_mobile_inspect_key_backup(new.as_ptr(), file.as_ptr(), 4097, pass.as_ptr(), pass.len()));
            assert!(oversized["error"].as_str().unwrap().contains("oversized"));
            let null = json(pocol_mobile_inspect_key_backup(new.as_ptr(), std::ptr::null(), 0, pass.as_ptr(), pass.len()));
            assert!(null["error"].is_string());
        }
        std::fs::remove_dir_all(&dir).unwrap();
    }
    /// The Android engine loop advances a confirmed rejoin on its 2 s tick, as the desktop stdio loop does (review: Rejoin
    /// signed the exit but never the reactivation on Android), and publishes the event apart from the command answers.
    #[test]
    fn the_mobile_loop_advances_a_rejoin_and_publishes_its_events() {
        #[derive(Default)]
        struct Fake { advances: u32, events: std::collections::VecDeque<serde_json::Value>, handled: Vec<String> }
        impl Engine<()> for Fake {
            fn handle(&mut self, _: &mut (), raw: &str) -> serde_json::Value { self.handled.push(raw.to_owned()); serde_json::json!({"type":"answer","to":raw}) }
            fn status(&self, _: &()) -> serde_json::Value { serde_json::json!({"type":"status","address":"0x01"}) }
            fn advance(&mut self, _: &mut ()) -> Option<serde_json::Value> { self.advances += 1; self.events.pop_front() }
        }
        let parse = |t: Option<String>| serde_json::from_str::<serde_json::Value>(&t.expect("a status is due")).unwrap();
        let (mut engine, mut wallet, t0) = (MobileLoop::default(), Fake::default(), Instant::now());
        wallet.events.push_back(serde_json::json!({"type":"rejoin","state":"Reactivating","message":"submitted"}));
        let first = parse(engine.tick(&mut wallet, &mut (), None, t0));
        assert_eq!((wallet.advances, &first["event"]["state"], &first["eventId"], &first["responseId"]), (1, &serde_json::json!("Reactivating"), &serde_json::json!(1), &serde_json::json!(0)));
        assert!(engine.tick(&mut wallet, &mut (), None, t0 + Duration::from_millis(500)).is_none(), "nothing due before 2 s");
        assert_eq!(wallet.advances, 1);
        let second = parse(engine.tick(&mut wallet, &mut (), None, t0 + Duration::from_secs(2)));
        assert_eq!((wallet.advances, &second["eventId"]), (2, &serde_json::json!(1)), "advance runs on every tick; the last event stays published");
        // A command is answered at once; an event in the same pass does not hide the answer.
        wallet.events.push_back(serde_json::json!({"type":"rejoin","state":"Stopped","message":"window passed"}));
        let third = parse(engine.tick(&mut wallet, &mut (), Some("{\"method\":\"status\"}".into()), t0 + Duration::from_secs(3)));
        assert_eq!((&third["response"]["type"], &third["responseId"], &third["event"]["state"], &third["eventId"]), (&serde_json::json!("answer"), &serde_json::json!(1), &serde_json::json!("Stopped"), &serde_json::json!(2)));
        assert_eq!((wallet.advances, wallet.handled.len()), (3, 1));
    }
    #[test]
    fn network_preview_rejects_missing_input_without_side_effects() {
        unsafe {
            let result = pocol_mobile_inspect_network(std::ptr::null());
            let value: serde_json::Value = serde_json::from_str(CStr::from_ptr(result).to_str().unwrap()).unwrap();
            pocol_mobile_free(result);
            assert_eq!(value["error"], "missing native argument");
        }
    }
}
