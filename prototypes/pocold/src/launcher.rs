//! One shared, bounded public network invitation; all secrets and state stay on the device.
use crate::{identity, keyfile, node, testnet, types};
use pocol_lp4_execution::txsign::Key;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{BufReader, Read, Write},
    net::SocketAddr,
    path::{Path, PathBuf},
};
pub const MAX_INVITATION: usize = 1 << 20;
const FILES: [&str; 3] = ["alloc.json", "devnet.json", "genesis.spec.hex"];
const UPGRADE_PENDING: &str = "network-upgrade.pending.json";
const UPGRADE_BACKUP: &str = "network.pocol.json.before-full-nonce";

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Invitation {
    schema: String,
    compatibility_sha256: String,
    seeds: Vec<SocketAddr>,
    files: BTreeMap<String, String>,
}
impl Invitation {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let value = Self::parse_saved(bytes)?;
        if value.compatibility_sha256 != identity::COMPATIBILITY_HEX {
            return Err("network invitation requires a different PoCol release".into());
        }
        Ok(value)
    }
    // The only caller accepting another build is the explicit same-chain upgrade.
    fn parse_saved(bytes: &[u8]) -> Result<Self, String> {
        if bytes.len() > MAX_INVITATION {
            return Err("network invitation exceeds 1 MiB".into());
        }
        let value: Self =
            serde_json::from_slice(bytes).map_err(|_| "invalid network invitation")?;
        value.validate()?;
        Ok(value)
    }
    fn validate(&self) -> Result<(), String> {
        if self.schema != "pocol-network-v1"
            || self.compatibility_sha256.len() != 64
            || !self.compatibility_sha256.bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err("network invitation requires a different PoCol release".into());
        }
        if self.files.len() != FILES.len() || FILES.iter().any(|n| !self.files.contains_key(*n)) {
            return Err("invitation must contain exactly the three public genesis files".into());
        }
        let unique: HashSet<_> = self.seeds.iter().collect();
        if !(1..=7).contains(&self.seeds.len())
            || unique.len() != self.seeds.len()
            || self
                .seeds
                .iter()
                .any(|a| a.port() == 0 || a.ip().is_unspecified() || a.ip().is_multicast())
        {
            return Err("provide 1..7 distinct reachable seed addresses".into());
        }
        let info: serde_json::Value = serde_json::from_str(&self.files["devnet.json"])
            .map_err(|_| "invalid network metadata")?;
        if info["keyMode"] != "independent-key-files" {
            return Err("standalone networks require independently generated keys".into());
        }
        crate::configured_forks(&info)?;
        Ok(())
    }
    pub fn fingerprint(&self) -> String {
        format!(
            "{:x}",
            Sha256::digest(&serde_json::to_vec(self).expect("invitation serialization"))
        )
    }
    pub fn describe(&self) -> serde_json::Value {
        let info: serde_json::Value =
            serde_json::from_str(&self.files["devnet.json"]).unwrap_or_default();
        let forks = crate::configured_forks(&info).unwrap_or_default();
        let activation = forks.iter().find(|(version, _)| *version == 2).map(|(_, height)| height.to_string());
        serde_json::json!({"networkFingerprint":self.fingerprint(),"chainId":info["chainId"],"genesisHash":info["genesisHash"],"forkSchedule":forks,"fullNonceActivationHeight":activation,"seeds":self.seeds,"scope":"experimental-testnet","publicLaunchReady":false})
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpgradeIntent {
    before: Invitation,
    after: Invitation,
}

fn upgrade_transition(before: &Invitation, after: &Invitation) -> Result<u64, String> {
    before.validate()?;
    after.validate()?;
    if after.compatibility_sha256 != identity::COMPATIBILITY_HEX
        || before.seeds != after.seeds
        || ["alloc.json", "genesis.spec.hex"].iter().any(|n| before.files[*n] != after.files[*n])
    {
        return Err("upgrade must preserve the genesis, allocations and seed addresses".into());
    }
    let mut old: serde_json::Value = serde_json::from_str(&before.files["devnet.json"]).map_err(|e| e.to_string())?;
    let mut new: serde_json::Value = serde_json::from_str(&after.files["devnet.json"]).map_err(|e| e.to_string())?;
    let previous = crate::configured_forks(&old)?;
    let next = crate::configured_forks(&new)?;
    if previous.iter().any(|(v, _)| *v != 1)
        || next.len() != previous.len() + 1
        || !next.starts_with(&previous)
        || next.last().map(|(v, _)| *v) != Some(2)
    {
        return Err("upgrade must append one future full-nonce v2 activation; changing or removing an activation is forbidden".into());
    }
    old.as_object_mut().ok_or("invalid network metadata")?.remove("forks");
    new.as_object_mut().ok_or("invalid network metadata")?.remove("forks");
    if old != new {
        return Err("upgrade may change only the fork schedule and release compatibility digest".into());
    }
    Ok(next.last().unwrap().1)
}

fn real_file(path: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !meta.is_file() || meta.file_type().is_symlink() {
        return Err(format!("expected a regular non-symlink file: {}", path.display()));
    }
    Ok(())
}

fn sync_directory(path: &Path) -> Result<(), String> {
    #[cfg(unix)]
    File::open(path).and_then(|f| f.sync_all()).map_err(|e| e.to_string())?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

// A leftover temporary file is reusable only when it is these public bytes or a
// strict prefix left by an interrupted write. Unexpected data is never discarded.
fn replace_public(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp = path.with_extension("upgrade-tmp");
    match fs::symlink_metadata(&temp) {
        Ok(_) => {
            real_file(&temp)?;
            let mut options = OpenOptions::new(); options.read(true).write(true);
            #[cfg(unix)]
            { use std::os::unix::fs::OpenOptionsExt; options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK); }
            let mut file = options.open(&temp).map_err(|e| e.to_string())?;
            let meta = file.metadata().map_err(|e| e.to_string())?;
            #[cfg(unix)]
            { use std::os::unix::fs::MetadataExt;
              if meta.nlink() != 1 { return Err("upgrade temporary file must have one link".into()); } }
            if !meta.is_file() || meta.len() > bytes.len() as u64 {
                return Err(format!("unexpected interrupted upgrade file: {}", temp.display()));
            }
            let mut saved = Vec::new(); file.read_to_end(&mut saved).map_err(|e| e.to_string())?;
            if !bytes.starts_with(&saved) { return Err(format!("unexpected interrupted upgrade file: {}", temp.display())); }
            if saved != bytes {
                use std::io::{Seek, SeekFrom};
                file.set_len(0).and_then(|_| file.seek(SeekFrom::Start(0))).map_err(|e| e.to_string())?;
                file.write_all(bytes).and_then(|_| file.sync_all()).map_err(|e| e.to_string())?;
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => write_new(&temp, bytes)?,
        Err(e) => return Err(e.to_string()),
    }
    fs::rename(&temp, path).map_err(|e| e.to_string())?;
    sync_directory(path.parent().ok_or("upgrade path has no parent")?)
}

// Read only. A torn/corrupt journal must be recovered by the existing release first.
// Count every stored branch, including a complete uncommitted tail, conservatively.
fn stored_max_height(home: &Path, before: &Invitation) -> Result<(u64, Option<File>), String> {
    let state = home.join("state");
    let meta = match fs::symlink_metadata(&state) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((0, None)),
        Err(e) => return Err(e.to_string()),
    };
    if !meta.is_dir() || meta.file_type().is_symlink() { return Err("state must be a real directory".into()); }
    let lock_path = state.join("LOCK");
    real_file(&lock_path)?;
    let lock = OpenOptions::new().read(true).write(true).open(&lock_path).map_err(|e| e.to_string())?;
    lock.try_lock().map_err(|_| "stop the node before upgrading its network configuration")?;
    let path = state.join("blocks.journal");
    real_file(&path)?;
    let mut reader = BufReader::new(File::open(path).map_err(|e| e.to_string())?);
    let mut head = [0u8; 40];
    reader.read_exact(&mut head).map_err(|_| "incomplete journal; recover with the existing release before upgrading")?;
    let info: serde_json::Value = serde_json::from_str(&before.files["devnet.json"]).map_err(|e| e.to_string())?;
    let genesis = types::unhex(info["genesisHash"].as_str().ok_or("missing genesis hash")?)?;
    if &head[..8] != b"POCOLJ2\n" || head[8..] != genesis {
        return Err("journal identity/version differs; recover with the existing release before upgrading".into());
    }
    let forks = pocol_lp4_execution::profile::ForkSchedule::new(&crate::configured_forks(&info)?).map_err(|e| format!("fork schedule: {e:?}"))?;
    let mut highest = 0;
    loop {
        let mut frame = [0u8; 8];
        if reader.read(&mut frame[..1]).map_err(|e| e.to_string())? == 0 { break; }
        reader.read_exact(&mut frame[1..]).map_err(|_| "incomplete journal record; recover before upgrading")?;
        let length = u32::from_be_bytes(frame[..4].try_into().unwrap()) as usize;
        let mut check = Sha256::new(); check.update(b"POCOL-REC-LEN"); check.update(&frame[..4]);
        if length > crate::store::MAX_RECORD || check.finalize()[..4] != frame[4..] {
            return Err("invalid journal frame; upgrade did not change chain state".into());
        }
        let mut content = vec![0; 32 + length + 1];
        reader.read_exact(&mut content).map_err(|_| "incomplete journal record; recover before upgrading")?;
        let payload = &content[32..32 + length];
        if content[32 + length] != 1 || Sha256::digest(payload)[..] != content[..32] {
            return Err("invalid journal checksum; upgrade did not change chain state".into());
        }
        let (header, _, _) = crate::store::decode_record(payload)?;
        let header = types::decode_header(&header)?;
        if header.genesis_hash.as_slice() != genesis || Some(header.chain_id) != info["chainId"].as_u64()
            || header.protocol_version != forks.active_version(header.h)
        {
            return Err("stored header does not match the existing network schedule".into());
        }
        highest = highest.max(header.h);
    }
    Ok((highest, Some(lock)))
}
pub fn read_bounded(path: &Path) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|e| format!("open {}: {e}", path.display()))?;
    if !file.metadata().map_err(|e| e.to_string())?.is_file() {
        return Err("expected a regular file".into());
    }
    let mut bytes = Vec::new();
    file.take((MAX_INVITATION + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > MAX_INVITATION {
        return Err("file exceeds 1 MiB".into());
    }
    Ok(bytes)
}
pub fn pack(network: &Path, seeds: Vec<SocketAddr>, out: &Path) -> Result<(), String> {
    let mut files = BTreeMap::new();
    for name in FILES {
        files.insert(
            name.into(),
            String::from_utf8(read_bounded(&network.join(name))?)
                .map_err(|_| "genesis files must be UTF-8")?,
        );
    }
    let invitation = Invitation {
        schema: "pocol-network-v1".into(),
        compatibility_sha256: identity::COMPATIBILITY_HEX.into(),
        seeds,
        files,
    };
    let bytes = serde_json::to_vec_pretty(&invitation).map_err(|e| e.to_string())?;
    Invitation::parse(&bytes)?;
    // Full admission still runs on each device, including its local storage measurement.
    crate::load_spec_for(network, &testnet::local(), None)?;
    write_new(out, &bytes)?;
    println!("{}", invitation.describe());
    Ok(())
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut opts = OpenOptions::new();
    opts.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opts.mode(0o600);
    }
    let mut file = opts
        .open(path)
        .map_err(|e| format!("create {}: {e}", path.display()))?;
    file.write_all(bytes)
        .and_then(|_| file.sync_all())
        .map_err(|e| e.to_string())
}
pub fn default_home() -> Result<PathBuf, String> {
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA")
            .map(|p| PathBuf::from(p).join("PoCol"))
            .ok_or("LOCALAPPDATA unavailable; specify --home".into())
    }
    #[cfg(not(windows))]
    {
        std::env::var_os("HOME")
            .map(|p| PathBuf::from(p).join(".local/share/pocol"))
            .ok_or("HOME unavailable; specify --home".into())
    }
}
pub struct Home {
    pub path: PathBuf,
    _lock: File,
}
impl Home {
    pub fn open(path: &Path) -> Result<Self, String> {
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(path).map_err(|e| e.to_string())?;
        let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
        if !meta.is_dir() || meta.file_type().is_symlink() {
            return Err("home must be a real directory".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 {
                return Err("PoCol home must be owned by this user, mode 0700".into());
            }
        }
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options
                .mode(0o600)
                .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        }
        let lock = options
            .open(path.join("launcher.lock"))
            .map_err(|e| e.to_string())?;
        if !lock.metadata().map_err(|e| e.to_string())?.is_file() {
            return Err("invalid launcher lock".into());
        }
        lock.try_lock()
            .map_err(|_| "PoCol is already running or the home cannot be locked")?;
        Ok(Self {
            path: path.to_path_buf(),
            _lock: lock,
        })
    }
    pub fn key(&self) -> Result<Key, String> {
        let path = self.path.join("identity.pocol-key");
        // Any existing path, including a bad file or dangling link, must fail rather than rotate identity.
        let marker = self.path.join("identity.address");
        let key = match fs::symlink_metadata(&path) {
            Ok(_) => keyfile::load(&path)?,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if marker.exists()
                    || self.path.join("state").exists()
                    || self.path.join("network.pocol.json").exists()
                {
                    return Err("existing identity key is missing; restore its backup, never silently replace it".into());
                }
                keyfile::generate(&path)?
            }
            Err(e) => return Err(e.to_string()),
        };
        let address = format!("{:#x}", key.address());
        if marker.exists() {
            if read_bounded(&marker)? != address.as_bytes() {
                return Err("key does not match this home's original public identity".into());
            }
        } else {
            write_new(&marker, address.as_bytes())?;
        }
        Ok(key)
    }
    pub fn address(&self) -> Result<String, String> {
        Ok(format!("{:#x}", self.key()?.address()))
    }
    /// Explicit, resumable public configuration upgrade. Never opens the wallet key or
    /// the state database for writing, and never creates a new identity or chain.
    pub fn upgrade(&self, bytes: &[u8]) -> Result<Invitation, String> {
        let after = Invitation::parse(bytes)?;
        real_file(&self.path.join("identity.pocol-key"))?;
        real_file(&self.path.join("identity.address"))?;
        let binding = self.path.join("network.pocol.json");
        real_file(&binding)?;
        let saved = Invitation::parse_saved(&read_bounded(&binding)?)?;
        let pending = self.path.join(UPGRADE_PENDING);
        let resuming = fs::symlink_metadata(&pending).is_ok();
        if !resuming && saved.fingerprint() == after.fingerprint() {
            let network = self.path.join("network");
            let meta = fs::symlink_metadata(&network).map_err(|e| e.to_string())?;
            if !meta.is_dir() || meta.file_type().is_symlink() { return Err("network must be a real directory".into()); }
            for name in FILES {
                let path = network.join(name); real_file(&path)?;
                if read_bounded(&path)? != after.files[name].as_bytes() {
                    return Err(format!("local network file changed: {name}"));
                }
            }
            return Ok(after);
        }
        let intent = if resuming {
            real_file(&pending)?;
            if fs::metadata(&pending).map_err(|e| e.to_string())?.len() > (4 * MAX_INVITATION + 4096) as u64 {
                return Err("upgrade intent is too large".into());
            }
            let intent: UpgradeIntent = serde_json::from_slice(&fs::read(&pending).map_err(|e| e.to_string())?)
                .map_err(|_| "invalid interrupted upgrade intent; keep the original configuration backup")?;
            if intent.after.fingerprint() != after.fingerprint()
                || (saved.fingerprint() != intent.before.fingerprint() && saved.fingerprint() != after.fingerprint())
            {
                return Err("resume the interrupted upgrade with the same network invitation".into());
            }
            intent
        } else {
            UpgradeIntent { before: saved, after: after.clone() }
        };
        let activation = upgrade_transition(&intent.before, &after)?;
        let info: serde_json::Value = serde_json::from_str(&after.files["devnet.json"]).map_err(|e| e.to_string())?;
        let genesis: [u8; 32] = types::unhex(info["genesisHash"].as_str().ok_or("missing genesis hash")?)?
            .try_into().map_err(|_| "invalid genesis hash")?;
        pocol_lp4_execution::profile::ProfileIdentity::validate(
            &types::unhex(after.files["genesis.spec.hex"].trim())?, &genesis,
            info["chainId"].as_u64().ok_or("missing chain ID")?,
            pocol_lp4_execution::profile::ProfileKind::Simnet,
            pocol_lp4_execution::profile::NodeKind::Simnet,
            &crate::configured_forks(&info)?,
        ).map_err(|e| format!("upgraded network profile: {e:?}"))?;
        let network = self.path.join("network");
        let meta = fs::symlink_metadata(&network).map_err(|e| e.to_string())?;
        if !meta.is_dir() || meta.file_type().is_symlink() { return Err("network must be a real directory".into()); }
        for name in FILES {
            let path = network.join(name);
            real_file(&path)?;
            let actual = read_bounded(&path)?;
            if actual != intent.before.files[name].as_bytes()
                && !(resuming && name == "devnet.json" && actual == after.files[name].as_bytes())
            {
                return Err(format!("local network file changed: {name}"));
            }
        }
        let (height, _state_lock) = stored_max_height(&self.path, &intent.before)?;
        if activation <= height {
            return Err(format!("activation {activation} must be above every stored block (highest {height})"));
        }
        let original = serde_json::to_vec_pretty(&intent.before).map_err(|e| e.to_string())?;
        let backup = self.path.join(UPGRADE_BACKUP);
        if fs::symlink_metadata(&backup).is_ok() {
            real_file(&backup)?;
            if Invitation::parse_saved(&read_bounded(&backup)?)?.fingerprint() != intent.before.fingerprint() {
                return Err("existing original-invitation backup differs; nothing changed".into());
            }
        } else {
            replace_public(&backup, &original)?;
        }
        if !resuming {
            replace_public(&pending, &serde_json::to_vec(&intent).map_err(|e| e.to_string())?)?;
        }
        crate::crash_point("network-upgrade-intent");
        replace_public(&network.join("devnet.json"), after.files["devnet.json"].as_bytes())?;
        crate::crash_point("network-upgrade-metadata");
        replace_public(&binding, bytes)?;
        crate::crash_point("network-upgrade-binding");
        fs::remove_file(pending).map_err(|e| e.to_string())?;
        sync_directory(&self.path)?;
        Ok(after)
    }
    pub fn import(&self, bytes: &[u8]) -> Result<Invitation, String> {
        if fs::symlink_metadata(self.path.join(UPGRADE_PENDING)).is_ok() {
            return Err("network upgrade is incomplete; resume the explicit upgrade command with the same invitation".into());
        }
        let invitation = Invitation::parse(bytes)?;
        let binding = self.path.join("network.pocol.json");
        if binding.exists() {
            let saved = Invitation::parse(&read_bounded(&binding)?)?;
            if saved.fingerprint() != invitation.fingerprint() {
                return Err("this home is bound to another invitation; use a separate home".into());
            }
        } else {
            write_new(&binding, bytes)?;
        }
        let network = self.path.join("network");
        fs::create_dir_all(&network).map_err(|e| e.to_string())?;
        for name in FILES {
            let path = network.join(name);
            let expected = invitation.files[name].as_bytes();
            if path.exists() {
                if read_bounded(&path)? != expected {
                    return Err(format!("local network file changed: {name}"));
                }
            } else {
                write_new(&path, expected)?;
            }
        }
        Ok(invitation)
    }
    pub fn run(
        &self,
        listen: Option<SocketAddr>,
        rpc: Option<SocketAddr>,
        stop: impl FnMut(&mut node::Node) -> bool,
    ) -> Result<(), String> {
        self.run_mode(listen,rpc,true,stop)
    }
    pub fn run_mode(
        &self, listen: Option<SocketAddr>, rpc: Option<SocketAddr>, mining: bool,
        mut stop: impl FnMut(&mut node::Node) -> bool,
    ) -> Result<(), String> {
        let key = self.key()?;
        let invitation = self.import(&read_bounded(&self.path.join("network.pocol.json"))?)?;
        if rpc.is_some_and(|a| !a.ip().is_loopback()) {
            return Err("native RPC must remain loopback-only".into());
        }
        let data = self.path.join("state");
        let spec =
            crate::load_spec_for(&self.path.join("network"), &testnet::local(), Some(&data))?;
        let declared: serde_json::Value =
            serde_json::from_str(&invitation.files["devnet.json"]).map_err(|e| e.to_string())?;
        if declared["genesisHash"] != types::hex(&spec.genesis_hash()) {
            return Err("invitation genesis hash mismatch".into());
        }
        let peers = invitation
            .seeds
            .into_iter()
            .filter(|a| Some(*a) != listen)
            .collect();
        let cfg = node::Config {
            dir: data,
            name: "pocol".into(),
            miner: mining.then_some(key),
            listen,
            peers,
            rpc,
            stop_at: None,
            log: true,
            limits: node::Limits {
                state_store: true,
                mem_budget: std::env::var("POCOL_MEMORY_BUDGET_BYTES").ok().map(|s|s.parse::<u64>().map_err(|_|"Invalid POCOL_MEMORY_BUDGET_BYTES")).transpose()?.unwrap_or(node::MEM_BUDGET),
                ..Default::default()
            },
        };
        let mut node = node::Node::open(cfg, spec)?;
        eprintln!("PoCol experimental test currency. Existing admission, memory and lifetime limits apply.");
        node.start_network()?;
        node.run_controlled(|n| stop(n));
        node.checkpoint_now()?;
        Ok(())
    }
}

#[cfg(test)]
mod upgrade_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!("pocol-upgrade-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            fs::create_dir(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for Scratch { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
    fn invitation() -> Invitation {
        let info = serde_json::json!({"keyMode":"independent-key-files", "chainId":777915,
            "genesisHash":types::hex(&[7; 32]), "forks":[[1,0]]});
        Invitation { schema: "pocol-network-v1".into(), compatibility_sha256: identity::COMPATIBILITY_HEX.into(),
            seeds: vec!["127.0.0.1:30300".parse().unwrap()], files: BTreeMap::from([
                ("alloc.json".into(), "{}".into()), ("genesis.spec.hex".into(), "00".into()),
                ("devnet.json".into(), info.to_string()),
            ]) }
    }
    fn journal(path: &Path, heights: &[u64]) -> Vec<u8> {
        let state = path.join("state"); fs::create_dir(&state).unwrap();
        fs::write(state.join("LOCK"), b"").unwrap();
        let mut bytes = b"POCOLJ2\n".to_vec(); bytes.extend([7; 32]);
        for &height in heights {
            let mut h = types::blank_header(); h.h = height; h.chain_id = 777915; h.genesis_hash = [7; 32];
            types::reseal(&mut h);
            bytes.extend(crate::store::encode_frame(&crate::store::encode_record(&h.encoded, &types::EMPTY_BODY, b"")));
        }
        fs::write(state.join("blocks.journal"), &bytes).unwrap(); bytes
    }
    #[test]
    fn upgrade_scans_all_branches_without_mutating_journal_and_holds_lock() {
        let t = Scratch::new(); let original = journal(&t.0, &[1, 20, 3]);
        let (max, lock) = stored_max_height(&t.0, &invitation()).unwrap();
        assert_eq!(max, 20);
        assert!(stored_max_height(&t.0, &invitation()).is_err());
        drop(lock);
        assert_eq!(stored_max_height(&t.0, &invitation()).unwrap().0, 20);
        assert_eq!(fs::read(t.0.join("state/blocks.journal")).unwrap(), original);
    }
    #[test]
    fn upgrade_refuses_torn_corrupt_or_wrong_identity_journal_without_repair() {
        let t = Scratch::new(); let original = journal(&t.0, &[1]);
        let path = t.0.join("state/blocks.journal");
        for changed in [original[..original.len()-1].to_vec(), {
            let mut b = original.clone(); b[8] ^= 1; b
        }, {
            let mut b = original.clone(); b[50] ^= 1; b
        }] {
            fs::write(&path, &changed).unwrap();
            assert!(stored_max_height(&t.0, &invitation()).is_err());
            assert_eq!(fs::read(&path).unwrap(), changed);
        }
    }
    #[test]
    fn upgrade_transition_is_append_only_and_metadata_preserving() {
        let before = invitation(); let mut after = before.clone();
        let mut info: serde_json::Value = serde_json::from_str(&after.files["devnet.json"]).unwrap();
        info["forks"] = serde_json::json!([[1,0],[2,21]]);
        after.files.insert("devnet.json".into(), info.to_string());
        assert_eq!(upgrade_transition(&before, &after).unwrap(), 21);
        assert!(upgrade_transition(&after, &before).is_err());
        assert!(upgrade_transition(&after, &after).is_err());
        info["extra"] = serde_json::json!(true);
        after.files.insert("devnet.json".into(), info.to_string());
        assert!(upgrade_transition(&before, &after).is_err());
    }
    #[test]
    fn upgrade_public_write_resumes_partial_temp_and_preserves_unexpected_data() {
        let t = Scratch::new(); let path = t.0.join("devnet.json"); let temp = path.with_extension("upgrade-tmp");
        fs::write(&path, b"old").unwrap(); fs::write(&temp, b"ne").unwrap();
        replace_public(&path, b"new metadata").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"new metadata");
        fs::write(&temp, b"unexpected").unwrap();
        assert!(replace_public(&path, b"replacement").is_err());
        assert_eq!(fs::read(&path).unwrap(), b"new metadata");
        assert_eq!(fs::read(&temp).unwrap(), b"unexpected");
        #[cfg(unix)]
        {
            fs::remove_file(&temp).unwrap();
            let protected = t.0.join("protected"); fs::write(&protected, b"ne").unwrap();
            fs::hard_link(&protected, &temp).unwrap();
            assert!(replace_public(&path, b"new contents").is_err());
            assert_eq!(fs::read(&protected).unwrap(), b"ne");
        }
    }
}
