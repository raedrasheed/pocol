//! Independent test-network keys. Binary 32-byte secrets, never command-line values.
//! Plaintext at rest: protect the host and directory; this is not an encrypted keystore/HSM. A Linux server can instead
//! hold its key in a systemd credential encrypted at rest ([`load_credential`], `pocol start --key-credential NAME`).
use pocol_lp4_execution::{devnet, txsign::Key};
use std::path::Path;
use zeroize::Zeroizing;

#[cfg(unix)]
pub fn load(path: &Path) -> Result<Key, String> {
    use std::os::unix::fs::MetadataExt;
    let (file, meta) = open_unix(path).map_err(|_| "cannot open key file".to_string())?;
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
    read_secret(file, &meta, "key file")
}

/// Opens `path` for reading without following a final-component symlink or blocking on a FIFO.
#[cfg(unix)]
fn open_unix(path: &Path) -> std::io::Result<(std::fs::File, std::fs::Metadata)> {
    use std::os::unix::fs::OpenOptionsExt;
    let file = std::fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    let meta = file.metadata()?;
    Ok((file, meta))
}

/// The key in an opened, already checked `what` (a key file or a key credential): exactly 32 binary bytes holding
/// a valid secret scalar.
#[cfg(unix)]
fn read_secret(mut file: std::fs::File, meta: &std::fs::Metadata, what: &str) -> Result<Key, String> {
    use std::io::Read;
    if meta.len() != 32 {
        return Err(format!("{what} must contain exactly 32 binary bytes"));
    }
    let mut secret = Zeroizing::new([0u8; 32]);
    file.read_exact(secret.as_mut())
        .map_err(|_| format!("cannot read {what}"))?;
    let mut extra = [0u8; 1];
    if file
        .read(&mut extra)
        .map_err(|_| format!("cannot finish {what} read"))?
        != 0
    {
        return Err(format!("{what} changed while reading"));
    }
    Key::from_secret(&secret).map_err(|_| "invalid secp256k1 secret scalar".into())
}

/// A systemd credential name `--key-credential` accepts: 1..64 of `A-Z a-z 0-9 . _ -`, not starting with a dot, so
/// it names one file directly in the credentials directory.
pub fn credential_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || name.len() > 64
        || name.starts_with('.')
        || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
    {
        return Err("invalid key credential name (1 to 64 of A-Z a-z 0-9 . _ -, not starting with a dot)".into());
    }
    Ok(())
}

/// The key held in the systemd credential `name` (K1: `LoadCredentialEncrypted=`, sealed with `systemd-creds` to the
/// host key and/or a TPM2, so no plaintext key file is at rest): the file `name` in `dir`, the service's
/// `$CREDENTIALS_DIRECTORY`. Its plaintext is a key file's 32 binary bytes. Refused, never replaced by another key
/// source, when `dir` is unset or the credential is missing. systemd places a credential read-only, owned by the
/// service user (mode 0400) or, where it grants access by an ACL, by root (mode 0400, or 0440 showing the ACL mask);
/// any other owner or mode, a link, a directory writable by others or a file that is not a key file's bytes is
/// refused as [`load`] refuses a key file.
#[cfg(unix)]
pub fn load_credential(dir: Option<&Path>, name: &str) -> Result<Key, String> {
    use std::os::unix::fs::MetadataExt;
    credential_name(name)?;
    let dir = dir.filter(|d| !d.as_os_str().is_empty()).ok_or(
        "--key-credential needs $CREDENTIALS_DIRECTORY, which systemd sets for a service with LoadCredentialEncrypted=; the plaintext key file is never used instead",
    )?;
    if !dir.is_absolute() {
        return Err("$CREDENTIALS_DIRECTORY must be an absolute path".into());
    }
    let euid = unsafe { libc::geteuid() };
    let d = std::fs::symlink_metadata(dir).map_err(|_| "cannot inspect $CREDENTIALS_DIRECTORY")?;
    if !d.is_dir() || (d.uid() != euid && d.uid() != 0) || d.mode() & 0o022 != 0 {
        return Err("$CREDENTIALS_DIRECTORY must be a real directory owned by this user or root and writable by no one else".into());
    }
    let (file, meta) = match open_unix(&dir.join(name)) {
        Ok(opened) => opened,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(format!("key credential {name} is missing in $CREDENTIALS_DIRECTORY; the plaintext key file is never used instead"))
        }
        Err(_) => return Err("cannot open key credential".into()),
    };
    let mode = meta.mode() & 0o777;
    let allowed = (meta.uid() == euid && [0o400, 0o600].contains(&mode))
        || (meta.uid() == 0 && [0o400, 0o440].contains(&mode));
    if !meta.is_file() || meta.nlink() != 1 || !allowed {
        return Err("key credential must be a single-link regular file owned by this user (mode 0400 or 0600) or by root as systemd places it (mode 0400 or 0440)".into());
    }
    read_secret(file, &meta, "key credential")
}

#[cfg(not(unix))]
pub fn load_credential(_: Option<&Path>, _: &str) -> Result<Key, String> {
    Err("--key-credential (systemd credentials) is supported on Linux only".into())
}

#[cfg(not(any(unix, windows)))]
pub fn load(_: &Path) -> Result<Key, String> {
    Err("key-file support requires Unix ownership/permission checks in this release".into())
}

/// A fresh random valid secret, written by [`store`].
#[cfg(any(unix, windows))]
pub fn generate(path: &Path) -> Result<Key, String> {
    let mut secret = Zeroizing::new([0u8; 32]);
    loop {
        getrandom::getrandom(secret.as_mut()).map_err(|_| "OS randomness unavailable")?;
        if Key::from_secret(&secret).is_ok() {
            break;
        }
    }
    store(path, &secret)
}

/// Writes `secret` to a new key file (a restored backup, or [`generate`]'s fresh key).
#[cfg(unix)]
pub fn store(path: &Path, secret: &[u8; 32]) -> Result<Key, String> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let key = Key::from_secret(secret).map_err(|_| "invalid secp256k1 secret scalar")?;
    // O_EXCL: existing files and symlinks are never overwritten. No secret in stdout/errors.
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .map_err(|_| "cannot create new key file (existing path or invalid parent)")?;
    file.write_all(secret)
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
#[cfg(not(any(unix, windows)))]
pub fn store(_: &Path, _: &[u8; 32]) -> Result<Key, String> {
    Err("key storage requires Unix ownership/permission checks in this release".into())
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

/// [`account`], or with `--backup FILE` the key in that encrypted backup ([`open_backup`]): exactly one source.
pub fn signer(args: &[String]) -> Result<Key, String> {
    match values(args, "--backup")?.as_slice() {
        [] => account(args),
        [path] if values(args, "--key-file")?.is_empty() && values(args, "--account")?.is_empty() => {
            Key::from_secret(&*open_backup(Path::new(path))?).map_err(|_| "invalid secp256k1 secret scalar".into())
        }
        _ => Err("provide exactly one --key-file PATH, --backup FILE or devnet-only --account LABEL".into()),
    }
}

/// The secret in the passphrase-encrypted backup at `path` ([`backup`]). The file is read before the passphrase
/// is asked for ([`prompt::ask`], one stdin line); a wrong passphrase and a changed byte give the same error.
pub fn open_backup(path: &Path) -> Result<Zeroizing<[u8; 32]>, String> {
    let file = backup::read(path)?;
    let passphrase = prompt::ask("Backup passphrase")?;
    backup::open(&file, &passphrase)
}

pub mod backup;
pub mod prompt;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::{load, store};

#[cfg(all(test, unix))]
mod credential_tests {
    //! K1: the key credential is read only from `$CREDENTIALS_DIRECTORY/NAME`, with a key file's format and checks
    //! (systemd's own owner and modes aside), and is refused rather than replaced when absent. Throwaway keys only.
    use super::*;
    use std::os::unix::fs::{symlink, PermissionsExt};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Scratch(PathBuf);
    impl Scratch {
        /// A fresh credentials directory (mode 0700) beside a throwaway key file.
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!("pocol-key-credential-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            let _ = std::fs::remove_dir_all(&p);
            std::fs::create_dir_all(p.join("creds")).unwrap();
            std::fs::set_permissions(p.join("creds"), std::fs::Permissions::from_mode(0o700)).unwrap();
            Self(p)
        }
        fn dir(&self) -> PathBuf { self.0.join("creds") }
        fn put(&self, name: &str, bytes: &[u8], mode: u32) -> PathBuf {
            let path = self.dir().join(name);
            let _ = std::fs::remove_file(&path);
            std::fs::write(&path, bytes).unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(mode)).unwrap();
            path
        }
        /// A generated throwaway key file and its secret.
        fn key(&self) -> (Key, [u8; 32]) {
            let path = self.0.join(format!("k{}.pocol-key", NEXT.fetch_add(1, Ordering::Relaxed)));
            let key = generate(&path).unwrap();
            (key, std::fs::read(&path).unwrap().try_into().unwrap())
        }
    }
    impl Drop for Scratch { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
    fn err(dir: Option<&Path>, name: &str) -> String { load_credential(dir, name).err().expect("refused") }

    #[test]
    fn a_credential_holds_the_same_key_as_its_key_file() {
        let t = Scratch::new();
        let (key, secret) = t.key();
        for mode in [0o400, 0o600] {
            t.put("pocol-identity", &secret, mode);
            assert_eq!(load_credential(Some(&t.dir()), "pocol-identity").unwrap().address(), key.address(), "{mode:o}");
        }
    }
    #[test]
    fn no_credentials_directory_or_no_credential_is_refused() {
        let t = Scratch::new();
        let (_, secret) = t.key();
        assert!(err(None, "pocol-identity").contains("needs $CREDENTIALS_DIRECTORY"));
        assert!(err(Some(Path::new("")), "pocol-identity").contains("needs $CREDENTIALS_DIRECTORY"));
        assert!(err(Some(Path::new("relative/creds")), "pocol-identity").contains("absolute"));
        assert!(err(Some(&t.0.join("absent")), "pocol-identity").contains("cannot inspect $CREDENTIALS_DIRECTORY"));
        assert!(err(Some(&t.dir()), "pocol-identity").contains("key credential pocol-identity is missing"));
        t.put("other", &secret, 0o400);
        assert!(err(Some(&t.dir()), "pocol-identity").contains("is missing"), "only the named credential is read");
    }
    #[test]
    fn a_credential_name_names_one_file_in_the_directory() {
        let t = Scratch::new();
        let (_, secret) = t.key();
        t.put("pocol-identity", &secret, 0o400);
        for bad in ["", ".", "..", ".hidden", "../creds/pocol-identity", "a/b", "a b", "name\0", &"n".repeat(65)] {
            assert!(err(Some(&t.dir()), bad).contains("invalid key credential name"), "{bad:?}");
        }
        assert!(credential_name(&"n".repeat(64)).is_ok() && credential_name("pocol-identity.v1_2").is_ok());
    }
    #[test]
    fn a_wrong_format_is_refused_like_a_key_file() {
        let t = Scratch::new();
        let (_, secret) = t.key();
        for (bytes, want) in [(&secret[..31], "exactly 32 binary bytes"), (&[secret.as_slice(), b"\n"].concat()[..], "exactly 32 binary bytes"),
                              (&[0u8; 32][..], "invalid secp256k1 secret scalar"), (&[0xffu8; 32][..], "invalid secp256k1 secret scalar"), (&[][..], "exactly 32")] {
            t.put("pocol-identity", bytes, 0o400);
            assert!(err(Some(&t.dir()), "pocol-identity").contains(want), "{} bytes: {want}", bytes.len());
        }
    }
    #[test]
    fn wrong_permissions_links_and_directories_are_refused() {
        let t = Scratch::new();
        let (_, secret) = t.key();
        let root = unsafe { libc::geteuid() } == 0;
        for mode in [0o644, 0o604, 0o640, 0o660, 0o700, 0o500, 0o200, 0o000] {
            t.put("pocol-identity", &secret, mode);
            // Without root, a mode without owner read already fails the open.
            let e = err(Some(&t.dir()), "pocol-identity");
            assert!(e.contains("single-link regular file") || (!root && e.contains("cannot open key credential")), "{mode:o}: {e}");
        }
        // 0440 is systemd's ACL form, accepted from root only.
        t.put("pocol-identity", &secret, 0o440);
        assert_eq!(load_credential(Some(&t.dir()), "pocol-identity").is_ok(), root);
        // A final-component symlink, a second hard link and a directory are refused.
        let target = t.put("target", &secret, 0o400);
        std::fs::remove_file(t.dir().join("pocol-identity")).unwrap();
        symlink(&target, t.dir().join("pocol-identity")).unwrap();
        assert!(err(Some(&t.dir()), "pocol-identity").contains("cannot open key credential"));
        std::fs::remove_file(t.dir().join("pocol-identity")).unwrap();
        std::fs::hard_link(&target, t.dir().join("pocol-identity")).unwrap();
        assert!(err(Some(&t.dir()), "pocol-identity").contains("single-link regular file"));
        std::fs::remove_file(t.dir().join("pocol-identity")).unwrap();
        std::fs::remove_file(&target).unwrap();
        std::fs::create_dir(t.dir().join("pocol-identity")).unwrap();
        assert!(err(Some(&t.dir()), "pocol-identity").contains("single-link regular file"));
        std::fs::remove_dir(t.dir().join("pocol-identity")).unwrap();
        // Another owner: only root can make one here.
        if root {
            let path = t.put("pocol-identity", &secret, 0o400);
            std::os::unix::fs::chown(&path, Some(65534), Some(65534)).unwrap();
            assert!(err(Some(&t.dir()), "pocol-identity").contains("single-link regular file"));
            std::fs::remove_file(&path).unwrap();
        }
        // The directory: writable by others, or a symlink to a good directory.
        t.put("pocol-identity", &secret, 0o400);
        for mode in [0o720, 0o702, 0o777] {
            std::fs::set_permissions(t.dir(), std::fs::Permissions::from_mode(mode)).unwrap();
            assert!(err(Some(&t.dir()), "pocol-identity").contains("writable by no one else"), "{mode:o}");
        }
        std::fs::set_permissions(t.dir(), std::fs::Permissions::from_mode(0o500)).unwrap();
        assert!(load_credential(Some(&t.dir()), "pocol-identity").is_ok(), "systemd's read-only 0500 directory");
        std::fs::set_permissions(t.dir(), std::fs::Permissions::from_mode(0o700)).unwrap();
        symlink(t.dir(), t.0.join("linked")).unwrap();
        assert!(err(Some(&t.0.join("linked")), "pocol-identity").contains("real directory"));
    }
}
