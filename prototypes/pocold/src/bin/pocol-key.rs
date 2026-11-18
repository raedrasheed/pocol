//! Independent local key generation, public address output and the passphrase-encrypted portable backup of a key
//! file (`keyfile::backup`, the format of `pocol export-key`). No secret output; passphrases are read from stdin.
use pocold::keyfile::{self, backup, prompt};
use pocol_lp4_execution::txsign::Key;
use std::path::Path;
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let result = match a.as_slice() {
        [_, cmd, flag, path] if cmd == "generate" && flag == "--out" => {
            keyfile::generate(Path::new(path))
        }
        [_, cmd, flag, path] if cmd == "address" && flag == "--key-file" => {
            keyfile::load(Path::new(path))
        }
        [_, cmd, flag, path, out_flag, out] if cmd == "export" && flag == "--key-file" && out_flag == "--out" => {
            export(Path::new(path), Path::new(out))
        }
        [_, cmd, flag, path, out_flag, out] if cmd == "import" && flag == "--backup" && out_flag == "--out" => {
            import(Path::new(path), Path::new(out))
        }
        _ => Err("usage: pocol-key generate --out FILE | address --key-file FILE | export --key-file FILE --out BACKUP | import --backup BACKUP --out FILE (backup passphrases are read from stdin: export reads it twice)".into()),
    };
    match result {
        Ok(key) => println!("{:#x}", key.address()),
        Err(error) => {
            eprintln!("error: {error}");
            std::process::exit(1);
        }
    }
}

/// A new encrypted backup of the key file `path` at `out`, which is never an existing path. The passphrase is read
/// twice and must meet the new-passphrase policy; the backup is opened again before it is written.
fn export(path: &Path, out: &Path) -> Result<Key, String> {
    if std::fs::symlink_metadata(out).is_ok() {
        return Err(format!("{} exists; an existing file is never replaced", out.display()));
    }
    let key = keyfile::load(path)?;
    let pass = prompt::ask("Backup passphrase (at least 12 characters)")?;
    if *prompt::ask("Repeat the backup passphrase")? != *pass {
        return Err("the passphrases differ; nothing was exported".into());
    }
    let file = backup::seal_checked(&zeroize::Zeroizing::new(key.secret()), &pass)?;
    backup::write_new(out, &file)?;
    Ok(key)
}

/// A new key file `out` (`keyfile::store`: never an existing path) holding the key in the backup at `path`.
/// A wrong passphrase writes nothing.
fn import(path: &Path, out: &Path) -> Result<Key, String> {
    if std::fs::symlink_metadata(out).is_ok() {
        return Err(format!("{} exists; an existing file is never replaced", out.display()));
    }
    keyfile::store(out, &*keyfile::open_backup(path)?)
}
