//! One shared, bounded public network invitation; all secrets and state stay on the device.
use crate::{identity, keyfile, node, testnet, types};
use pocol_lp4_execution::txsign::Key;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, HashSet},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    net::SocketAddr,
    path::{Path, PathBuf},
};
pub const MAX_INVITATION: usize = 1 << 20;
const FILES: [&str; 3] = ["alloc.json", "devnet.json", "genesis.spec.hex"];

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Invitation {
    schema: String,
    compatibility_sha256: String,
    seeds: Vec<SocketAddr>,
    files: BTreeMap<String, String>,
}
impl Invitation {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
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
            || self.compatibility_sha256 != identity::COMPATIBILITY_HEX
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
        serde_json::json!({"networkFingerprint":self.fingerprint(),"chainId":info["chainId"],"genesisHash":info["genesisHash"],"seeds":self.seeds,"scope":"experimental-testnet","publicLaunchReady":false})
    }
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
    pub fn import(&self, bytes: &[u8]) -> Result<Invitation, String> {
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
