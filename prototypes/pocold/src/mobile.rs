//! Narrow C ABI used by the Android JNI bridge. Java never receives a private key.
use crate::{
    launcher::{self, Home},
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
        Ok(serde_json::json!({"address":home.address()?,"compatibilitySha256":crate::identity::COMPATIBILITY_HEX}))
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
#[no_mangle]
pub unsafe extern "C" fn pocol_mobile_run(home: *const c_char) -> *mut c_char {
    let result = std::panic::catch_unwind(|| {
        let path = unsafe { text(home)? }.to_owned();
        let handle = threads::spawn("consensus", threads::CONSENSUS_STACK, move || {
            let home = Home::open(Path::new(&path))?;
            let mut wallet=crate::wallet::Wallet::new(&home)?;
            let mut response=serde_json::Value::Null;let mut response_id=0u64;
            let mut next = Instant::now();
            home.run_mode(None, None, false, |node| {
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
