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
#[no_mangle]
pub unsafe extern "C" fn pocol_mobile_run(home: *const c_char) -> *mut c_char {
    let result = std::panic::catch_unwind(|| {
        let path = unsafe { text(home)? }.to_owned();
        let handle = threads::spawn("consensus", threads::CONSENSUS_STACK, move || {
            let home = Home::open(Path::new(&path))?;
            let mut wallet=crate::wallet::Wallet::new(&home)?;
            let mut response=serde_json::Value::Null;let mut response_id=0u64;
            let mut next = Instant::now();
            home.run_mode(None, None, &[], false, |node| {
                let command=COMMANDS.lock().ok().and_then(|mut q|q.pop_front());
                if let Some(command)=command{response=wallet.handle(node,&command);response_id+=1;next=Instant::now();}
                if Instant::now() >= next {
                    if let Ok(mut status) = STATUS.lock() {
                        let mut v=wallet.status(node);v["response"]=response.clone();v["responseId"]=serde_json::json!(response_id);*status=v.to_string();
                    }
                    next = Instant::now() + Duration::from_secs(2);
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
        // T33: the H15000 genesis files under pocol-network-v2 (rules 1), as `pocol network` writes them from R1 on.
        let dir = std::env::temp_dir().join(format!("pocol-mobile-inspect-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir(&dir).unwrap();
        let mut v2: serde_json::Value = serde_json::from_str(include_str!("../../../deploy/mainnet/h15000/network.pocol.json")).unwrap();
        let o = v2.as_object_mut().unwrap();
        o.remove("compatibilitySha256");
        o.insert("schema".into(), launcher::SCHEMA_V2.into());
        o.insert("consensusRulesVersion".into(), crate::identity::CONSENSUS_RULES_VERSION.into());
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
        let activation = write("activation.json", &devnet(serde_json::json!([[1, 0], [2, 15000]]), false));
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
                                         (&activation, "activation", serde_json::json!([[2, 15000]]))] {
            let dry = ask(pocol_mobile_inspect_upgrade, &home, file);
            assert_eq!((dry["change"].as_str(), &dry["appended"]), (Some(change), &appended), "{dry}");
            assert_eq!(files(&home), before, "the dry run writes nothing ({change})");
        }
        assert_eq!(ask(pocol_mobile_inspect_upgrade, &home, &activation)["consensusIdentity"],
                   "bcc4b71570fed4788d84baff137e26a88527c1fd17a11f595d92ce1e5e50b110", "the T11 golden identity");
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
