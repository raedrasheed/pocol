//! Independent test-network keys. Binary 32-byte secrets, never command-line values.
//! Plaintext at rest: protect the host and directory; this is not an encrypted keystore/HSM.
use pocol_lp4_execution::{devnet, txsign::Key};
use std::path::Path;
use zeroize::Zeroizing;

#[cfg(unix)]
pub fn load(path: &Path) -> Result<Key, String> {
    use std::io::Read;
    use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|_| "cannot open key file".to_string())?;
    let meta = file.metadata().map_err(|_| "cannot inspect key file")?;
    if !meta.is_file()
        || ![0o600, 0o400].contains(&(meta.mode() & 0o777))
        || meta.nlink() != 1
        || meta.uid() != unsafe { libc::geteuid() }
    {
        return Err(
            "key file must be a single-link regular file owned by this user, mode 0600 or 0400"
                .into(),
        );
    }
    if meta.len() != 32 {
        return Err("key file must contain exactly 32 binary bytes".into());
    }
    let mut secret = Zeroizing::new([0u8; 32]);
    file.read_exact(secret.as_mut())
        .map_err(|_| "cannot read key file")?;
    let mut extra = [0u8; 1];
    if file
        .read(&mut extra)
        .map_err(|_| "cannot finish key read")?
        != 0
    {
        return Err("key file changed while reading".into());
    }
    Key::from_secret(&secret).map_err(|_| "invalid secp256k1 secret scalar".into())
}

#[cfg(not(any(unix, windows)))]
pub fn load(_: &Path) -> Result<Key, String> {
    Err("key-file support requires Unix ownership/permission checks in this release".into())
}

#[cfg(unix)]
pub fn generate(path: &Path) -> Result<Key, String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut secret = Zeroizing::new([0u8; 32]);
    let key = loop {
        getrandom::getrandom(secret.as_mut()).map_err(|_| "OS randomness unavailable")?;
        if let Ok(key) = Key::from_secret(&secret) {
            break key;
        }
    };
    // O_EXCL: existing files and symlinks are never overwritten. No secret in stdout/errors.
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|_| "cannot create new key file (existing path or invalid parent)")?;
    file.write_all(secret.as_ref())
        .and_then(|_| file.sync_all())
        .map_err(|_| "key write failed; inspect/remove incomplete file before retrying")?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::File::open(parent)
        .and_then(|f| f.sync_all())
        .map_err(|_| "key directory sync failed")?;
    Ok(key)
}

#[cfg(not(any(unix, windows)))]
pub fn generate(_: &Path) -> Result<Key, String> {
    Err("key generation requires Unix ownership/permission checks in this release".into())
}

/// Exactly one source; keep label-derived accounts for existing devnet tests only.
fn values(args: &[String], flag: &str) -> Result<Vec<String>, String> {
    args.iter()
        .enumerate()
        .filter(|(_, a)| a.as_str() == flag)
        .map(|(i, _)| {
            args.get(i + 1)
                .filter(|v| !v.starts_with("--"))
                .cloned()
                .ok_or_else(|| format!("missing value for {flag}"))
        })
        .collect()
}

pub fn miner(args: &[String]) -> Result<Option<Key>, String> {
    let files = values(args, "--miner-key-file")?;
    let labels = values(args, "--miner")?;
    match (files.as_slice(), labels.as_slice()) {
        ([], []) => Ok(None),
        ([path], []) => load(Path::new(path)).map(Some),
        ([], [label]) => Ok(Some(devnet::miner_identity(label).key)),
        _ => Err("provide at most one --miner-key-file PATH or devnet-only --miner LABEL".into()),
    }
}

pub fn account(args: &[String]) -> Result<Key, String> {
    let files = values(args, "--key-file")?;
    let labels = values(args, "--account")?;
    match (files.as_slice(), labels.as_slice()) {
        ([path], []) => load(Path::new(path)),
        ([], [label]) => Ok(devnet::account_key(label)),
        _ => Err("provide exactly one --key-file PATH or devnet-only --account LABEL".into()),
    }
}

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{load, generate};
