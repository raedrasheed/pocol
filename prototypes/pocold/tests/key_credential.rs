//! K1 (b): `pocol identity|start --key-credential NAME` reads the server key from the systemd credential NAME in
//! `$CREDENTIALS_DIRECTORY` (what `LoadCredentialEncrypted=` provides to the unit that
//! deploy/standalone/encrypt-server-key.sh writes), never from the plaintext `identity.pocol-key`. Throwaway keys only;
//! every directory is removed at the end.
#![cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct Scratch(PathBuf);
impl Scratch {
    fn new(tag: &str) -> Self {
        let d = std::env::temp_dir().join(format!("pocol-key-credential-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        Self(d)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn pocol(args: &[&str], creds: Option<&Path>) -> Output {
    let mut c = Command::new(env!("CARGO_BIN_EXE_pocol"));
    c.args(args).env_remove("CREDENTIALS_DIRECTORY");
    if let Some(dir) = creds {
        c.env("CREDENTIALS_DIRECTORY", dir);
    }
    c.output().unwrap()
}
fn pocol_with_stdin(args: &[&str], creds: Option<&Path>, stdin: &str) -> Output {
    use std::io::Write;
    let mut c = Command::new(env!("CARGO_BIN_EXE_pocol"));
    c.args(args).env_remove("CREDENTIALS_DIRECTORY");
    if let Some(dir) = creds {
        c.env("CREDENTIALS_DIRECTORY", dir);
    }
    let mut child = c.stdin(std::process::Stdio::piped()).stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
}
fn text(b: &[u8]) -> String {
    String::from_utf8_lossy(b).into_owned()
}
fn address(out: &Output) -> String {
    assert!(out.status.success(), "{}", text(&out.stderr));
    let line = text(&out.stdout).lines().next().unwrap().to_string();
    serde_json::from_str::<serde_json::Value>(&line).unwrap()["address"].as_str().unwrap().to_string()
}
fn refused(out: &Output, want: &str) {
    assert_eq!(out.status.code(), Some(1), "{}", text(&out.stderr));
    assert!(text(&out.stderr).contains(want), "{want}: {}", text(&out.stderr));
    assert!(!text(&out.stdout).contains("\"address\""), "no address is reported: {}", text(&out.stdout));
}

/// A home made by `pocol identity` (its throwaway key file), and a credentials directory holding the same bytes as
/// systemd places a credential (mode 0400 in a directory nobody else can write).
fn migrated(t: &Scratch) -> (PathBuf, PathBuf, String) {
    let home = t.0.join("home");
    let file_address = address(&pocol(&["identity", "--home", home.to_str().unwrap()], None));
    let creds = t.0.join("creds");
    std::fs::create_dir(&creds).unwrap();
    std::fs::set_permissions(&creds, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::copy(home.join("identity.pocol-key"), creds.join("pocol-identity")).unwrap();
    std::fs::set_permissions(creds.join("pocol-identity"), std::fs::Permissions::from_mode(0o400)).unwrap();
    (home, creds, file_address)
}

#[test]
fn identity_and_start_read_the_credential_with_the_key_files_address() {
    let t = Scratch::new("same");
    let (home, creds, file_address) = migrated(&t);
    let h = home.to_str().unwrap();
    let with = ["--home", h, "--key-credential", "pocol-identity"];
    assert_eq!(address(&pocol(&[&["identity"][..], &with].concat(), Some(&creds))), file_address);
    // The plaintext key file removed (as encrypt-server-key.sh does once the service reported this address): the
    // credential alone serves `identity` and `start`, and nothing recreates a key file.
    std::fs::remove_file(home.join("identity.pocol-key")).unwrap();
    assert_eq!(address(&pocol(&[&["identity"][..], &with].concat(), Some(&creds))), file_address);
    // `start` reports the credential's address first (the line the helper compares), then stops here: this home has no
    // network file yet.
    let start = pocol(&[&["start"][..], &with].concat(), Some(&creds));
    assert_eq!(start.status.code(), Some(1));
    let first = text(&start.stdout).lines().next().unwrap().to_string();
    assert_eq!(serde_json::from_str::<serde_json::Value>(&first).unwrap()["address"], file_address.as_str());
    assert!(text(&start.stderr).contains("Install the shared network.pocol.json"), "{}", text(&start.stderr));
    assert!(std::fs::symlink_metadata(home.join("identity.pocol-key")).is_err(), "no key file is created");
    // Without the flag the missing key file is refused, never replaced by a new identity.
    refused(&pocol(&["identity", "--home", h], None), "existing identity key is missing");
    assert!(std::fs::symlink_metadata(home.join("identity.pocol-key")).is_err());
}

#[test]
fn an_unset_directory_a_missing_or_bad_credential_is_refused_with_the_key_file_present() {
    let t = Scratch::new("refused");
    let (home, creds, _) = migrated(&t);
    let h = home.to_str().unwrap();
    let before = std::fs::read(home.join("identity.pocol-key")).unwrap();
    for command in ["identity", "start"] {
        let with = [command, "--home", h, "--key-credential", "pocol-identity"];
        refused(&pocol(&with, None), "needs $CREDENTIALS_DIRECTORY");
        refused(&pocol(&[command, "--home", h, "--key-credential", "other"], Some(&creds)), "key credential other is missing");
        refused(&pocol(&[command, "--home", h, "--key-credential", "../home/identity.pocol-key"], Some(&creds)), "invalid key credential name");
        // Wrong permissions and a wrong format are refused like a key file.
        std::fs::set_permissions(creds.join("pocol-identity"), std::fs::Permissions::from_mode(0o644)).unwrap();
        refused(&pocol(&with, Some(&creds)), "single-link regular file");
        std::fs::remove_file(creds.join("pocol-identity")).unwrap();
        std::fs::write(creds.join("pocol-identity"), b"not a key").unwrap();
        std::fs::set_permissions(creds.join("pocol-identity"), std::fs::Permissions::from_mode(0o400)).unwrap();
        refused(&pocol(&with, Some(&creds)), "exactly 32 binary bytes");
        std::fs::remove_file(creds.join("pocol-identity")).unwrap();
        std::fs::copy(home.join("identity.pocol-key"), creds.join("pocol-identity")).unwrap();
        std::fs::set_permissions(creds.join("pocol-identity"), std::fs::Permissions::from_mode(0o400)).unwrap();
    }
    // Another account's credential does not match this home's identity.
    let other = t.0.join("other");
    pocol(&["identity", "--home", other.to_str().unwrap()], None);
    std::fs::remove_file(creds.join("pocol-identity")).unwrap();
    std::fs::copy(other.join("identity.pocol-key"), creds.join("pocol-identity")).unwrap();
    std::fs::set_permissions(creds.join("pocol-identity"), std::fs::Permissions::from_mode(0o400)).unwrap();
    refused(&pocol(&["identity", "--home", h, "--key-credential", "pocol-identity"], Some(&creds)), "does not match this home's original public identity");
    // A credential never creates a home or an identity.
    let fresh = t.0.join("fresh");
    refused(&pocol(&["identity", "--home", fresh.to_str().unwrap(), "--key-credential", "pocol-identity"], Some(&creds)), "requires an existing PoCol home");
    assert!(!fresh.exists());
    assert_eq!(std::fs::read(home.join("identity.pocol-key")).unwrap(), before, "the key file is untouched");
    // Commands that rewrite the key file, and the wallet, take no credential.
    for command in ["import-key", "inspect-key-backup", "wallet", "import"] {
        let out = pocol(&[command, "--home", h, "--key-credential", "pocol-identity"], Some(&creds));
        assert!(!out.status.success() && text(&out.stderr).contains("unknown option --key-credential"), "{command}: {}", text(&out.stderr));
    }
}

/// What deploy/standalone/encrypt-server-key.sh relies on: `pocol help` names --key-credential (its check that the
/// installed release has the option), and the first line `identity`, `start` and `inspect-key-backup` print is
/// compact JSON whose `"address":"0x` + 40 lower-case hex digits it reads (from `runuser ... pocol identity`, from
/// the journal, and from the backup check that must pass before it removes the plaintext key file).
#[test]
fn the_helper_scripts_checks_match_the_binary() {
    let help = pocol(&["help"], None);
    assert!(text(&help.stderr).contains("--key-credential NAME"), "{}", text(&help.stderr));
    let t = Scratch::new("helper");
    let (home, creds, file_address) = migrated(&t);
    for (args, dir) in [(vec!["identity", "--home", home.to_str().unwrap()], None),
                        (vec!["start", "--home", home.to_str().unwrap(), "--key-credential", "pocol-identity"], Some(creds.as_path()))] {
        let out = pocol(&args, dir);
        let line = text(&out.stdout).lines().next().unwrap().to_string();
        let digits = line.strip_prefix("{\"address\":\"0x").and_then(|r| r.get(..41)).unwrap_or_default();
        assert!(digits.len() == 41 && digits.ends_with('"') && digits[..40].bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)), "{line}");
        assert_eq!(format!("0x{}", &digits[..40]), file_address);
    }
    // A throwaway backup of this key, made from the credential; it opens to the key file's address.
    let phrase = "a long throwaway backup passphrase";
    let backup = t.0.join("backup");
    let made = pocol_with_stdin(&["export-key", "--home", home.to_str().unwrap(), "--out", backup.to_str().unwrap(),
                                  "--key-credential", "pocol-identity"], Some(&creds), &format!("{phrase}\n{phrase}\n"));
    assert_eq!(address(&made), file_address);
    let inspected = pocol_with_stdin(&["inspect-key-backup", "--home", home.to_str().unwrap(), "--backup", backup.to_str().unwrap()],
                                     None, &format!("{phrase}\n"));
    assert!(inspected.status.success(), "{}", text(&inspected.stderr));
    let line = text(&inspected.stdout).lines().next().unwrap().to_string();
    assert_eq!(line.strip_prefix("{\"address\":\"").and_then(|r| r.get(..42)), Some(file_address.as_str()), "{line}");
    let wrong = pocol_with_stdin(&["inspect-key-backup", "--home", home.to_str().unwrap(), "--backup", backup.to_str().unwrap()],
                                 None, "another passphrase entirely\n");
    assert!(!wrong.status.success() && !text(&wrong.stdout).contains("\"address\""), "a wrong passphrase reports no address");
    let script = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../deploy/standalone/encrypt-server-key.sh")).unwrap();
    assert!(script.contains(r#"s/.*"address":"\(0x[0-9a-f]\{40\}\)".*/\1/p"#) && script.contains("*--key-credential*)"));
    assert!(script.contains(r#""$OPT/pocol" inspect-key-backup --home "$STATE" --backup "$backup" | address_in"#));
}
