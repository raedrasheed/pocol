//! One shared, bounded public network invitation; all secrets and state stay on the device.
//! A `pocol-network-v2` invitation binds a home to the consensus rules revision it runs (VA-SPEC §2.1), not to a build
//! digest, so a release that changes only the digest starts on existing homes, and fork entries of any version the
//! runtime schedule accepts can be appended (future-only, append-only). A `pocol-network-v1` file names the 0.4.0 build
//! digest it was made by and is accepted only through a certified `LEGACY_BUILDS` entry (plan §1.7).
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
/// The one fixed backup name PoCol 0.4.0 wrote before its full-nonce upgrade. Documentation only: it is never read or
/// written again. A backup is now named by the replaced binding's fingerprint (`backup_name`), so a second upgrade of
/// a home that already holds this file no longer collides with it.
pub const LEGACY_UPGRADE_BACKUP: &str = "network.pocol.json.before-full-nonce";
pub const SCHEMA_V1: &str = "pocol-network-v1";
/// The only schema `pocol network` writes (plan §1.7); its fingerprint is stable across digest-only releases.
pub const SCHEMA_V2: &str = "pocol-network-v2";

/// A released build whose v1 invitations this release may run (plan §1.7). An entry applies only when the chain id
/// and genesis hash computed from the invitation's genesis.spec.hex equal both those devnet.json declares and the
/// entry's (attack A1-10), and only once `certified` names the replay evidence (plan §6,
/// development/decisions/LEGACY-CERTIFICATION-0.4.0.md). `sources` is the consensus-sources aggregate that replay ran;
/// package.py refuses to package a build whose aggregate differs (A2-8), so a re-pin voids the certification.
#[derive(Clone, Copy, Debug)]
pub struct Legacy { pub digest: &'static str, pub rules: u32, pub chain_id: u64, pub genesis_hash: &'static str, pub sources: Option<&'static str>, pub certified: Option<&'static str> }
/// PoCol Mainnet 0.4.0 (H15000), shipped uncertified: owner decision D1 is open and its recommended Option B moves the
/// network to a new genesis, so no replay evidence exists. An H15000 home is refused with `legacy_refusal()` until an
/// owner certifies the entry (fail closed).
pub const LEGACY_BUILDS: &[Legacy] = &[Legacy {
    digest: "6cf38a14aa01ca3d824b0309564942cc926ca7232e2a5b2672a17e8c99e35476", rules: 1, chain_id: 1599846715,
    genesis_hash: "0xbe137bfdb4fdf332f2e1500ee1a67e0b22ce6bdf57efa46fb4ea7b0debe509ca", sources: None, certified: None,
}];
/// The R1 switch from deployed 0.4.0 (plan §6, owner decision D1). Both refusal texts exist; `SWITCH` selects one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Switch { A, B }
/// D1 is pending; the recommended Option B (ride the earn-in re-genesis) is implemented as the default.
pub const SWITCH: Switch = Switch::B;
/// The refusal of an uncertified legacy binding (plan §5.4).
pub fn legacy_refusal() -> &'static str {
    match SWITCH {
        Switch::B => "This network was replaced by a new genesis. Follow the release notes to move the old network files aside; keep your wallet key.",
        Switch::A => "This release cannot yet run the PoCol Mainnet 0.4.0 network. Keep 0.4.0 until the announced switch.",
    }
}

/// Public data only (seeds and genesis files), so Debug prints nothing secret.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Invitation {
    schema: String,
    /// v1 only: the build digest the file was made by. Skipped when absent, so a v1 file serialises to the bytes it had
    /// and its `fingerprint()` is unchanged.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    compatibility_sha256: Option<String>,
    /// v2 only: the consensus rules revision (`identity::CONSENSUS_RULES_VERSION`) the network runs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    consensus_rules_version: Option<u32>,
    seeds: Vec<SocketAddr>,
    files: BTreeMap<String, String>,
}
impl Invitation {
    /// A well-formed invitation whose consensus rules are this release's.
    pub fn parse(bytes: &[u8]) -> Result<Self, String> { Self::parse_with(bytes, LEGACY_BUILDS) }
    fn parse_with(bytes: &[u8], legacy: &[Legacy]) -> Result<Self, String> {
        let value = Self::parse_saved(bytes)?;
        let rules = value.rules_of_with(legacy)?;
        if rules != identity::CONSENSUS_RULES_VERSION {
            return Err(format!("network invitation requires a different PoCol consensus release (rules {rules}; this release runs rules {})", identity::CONSENSUS_RULES_VERSION));
        }
        Ok(value)
    }
    // Well-formed only, whatever its rules: the explicit upgrade checks the saved binding's rules itself.
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
        // v1 carries exactly a 64-hex build digest, v2 exactly a rules revision >= 1.
        let bound = match (self.schema.as_str(), &self.compatibility_sha256, self.consensus_rules_version) {
            (SCHEMA_V1, Some(d), None) => d.len() == 64 && d.bytes().all(|b| b.is_ascii_hexdigit()),
            (SCHEMA_V2, None, Some(rules)) => rules >= 1,
            _ => false,
        };
        if !bound {
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
        let info = self.info()?;
        if info["keyMode"] != "independent-key-files" {
            return Err("standalone networks require independently generated keys".into());
        }
        crate::configured_forks(&info)?;
        Ok(())
    }
    fn info(&self) -> Result<serde_json::Value, String> {
        serde_json::from_str(self.files.get("devnet.json").map_or("", String::as_str)).map_err(|_| "invalid network metadata".into())
    }
    /// The consensus rules revision the invitation binds to: v2 declares it; a v1 file has one only through a certified
    /// `LEGACY_BUILDS` entry that matches its digest and its computed chain and genesis.
    pub fn rules_of(&self) -> Result<u32, String> { self.rules_of_with(LEGACY_BUILDS) }
    fn rules_of_with(&self, legacy: &[Legacy]) -> Result<u32, String> {
        if let Some(rules) = self.consensus_rules_version { return Ok(rules) }
        let digest = self.compatibility_sha256.as_deref().unwrap_or_default();
        let Some(entry) = legacy.iter().find(|l| l.digest.eq_ignore_ascii_case(digest)) else {
            return Err("network invitation requires a different PoCol release".into());
        };
        let (chain_id, genesis) = self.computed_network()?;
        let info = self.info()?;
        let declared = (info["chainId"].as_u64(), info["genesisHash"].as_str().map(str::to_ascii_lowercase));
        if declared != (Some(chain_id), Some(genesis.clone())) || (entry.chain_id, entry.genesis_hash.to_ascii_lowercase()) != (chain_id, genesis) {
            return Err("network invitation's genesis does not match the chain and genesis it declares for its release".into());
        }
        if entry.certified.is_none() { return Err(legacy_refusal().into()) }
        Ok(entry.rules)
    }
    /// The chain id and genesis hash genesis.spec.hex itself commits to, never the declared metadata (A1-10).
    fn computed_network(&self) -> Result<(u64, String), String> {
        let bytes = types::unhex(self.files.get("genesis.spec.hex").map_or("", |s| s.trim()))?;
        if bytes.len() > crate::genesis_spec::MAX_SPEC_BYTES { return Err("genesis specification exceeds its bound".into()) }
        let spec = crate::genesis_spec::decode(&bytes).map_err(|e| format!("genesis specification: {e:?}"))?;
        Ok((spec.chain_id, types::hex(&crate::genesis_spec::genesis_hash(&bytes))))
    }
    pub fn fingerprint(&self) -> String {
        format!(
            "{:x}",
            Sha256::digest(&serde_json::to_vec(self).expect("invitation serialization"))
        )
    }
    /// What a home is bound to: the seeds and the three files, whatever the schema and binding field. A v2 twin of a
    /// saved v1 file has the same key, so importing it is the same network (plan §1.7).
    pub fn content_key(&self) -> String {
        #[derive(Serialize)]
        struct Content<'a> { seeds: &'a [SocketAddr], files: &'a BTreeMap<String, String> }
        format!("{:x}", Sha256::digest(&serde_json::to_vec(&Content { seeds: &self.seeds, files: &self.files }).expect("invitation serialization")))
    }
    /// The consensus identity a pinned release build has on this network (VA-SPEC §2.1: no caps-test or unpinned
    /// flag), which its status reports as `consensusIdentity` once it runs. None when the rules are unknown.
    fn published_identity(&self) -> Option<String> {
        let info = self.info().ok()?;
        let forks = crate::configured_forks(&info).ok()?;
        let genesis: [u8; 32] = types::unhex(info["genesisHash"].as_str()?).ok()?.try_into().ok()?;
        Some(identity::Consensus::new(info["chainId"].as_u64()?, genesis, &forks, self.rules_of().ok()?, 0, None).hex())
    }
    pub fn describe(&self) -> serde_json::Value {
        let info = self.info().unwrap_or_default();
        let forks = crate::configured_forks(&info).unwrap_or_default();
        let activation = forks.iter().find(|(version, _)| *version == 2).map(|(_, height)| height.to_string());
        let activations: Vec<&(u16, u64)> = forks.iter().filter(|(version, _)| *version > 1).collect();
        crate::network_display::annotate_genesis(serde_json::json!({"networkFingerprint":self.fingerprint(),"invitationSchema":self.schema,
            "consensusRulesVersion":self.rules_of().ok(),"consensusIdentity":self.published_identity(),"chainId":info["chainId"],"genesisHash":info["genesisHash"],
            "forkSchedule":forks,"activations":activations,"fullNonceActivationHeight":activation,"seeds":self.seeds,"scope":"experimental-testnet","publicLaunchReady":false}), &self.files["genesis.spec.hex"])
    }
    /// What `pocol network` writes: v2, bound to this release's consensus rules.
    fn new_v2(seeds: Vec<SocketAddr>, files: BTreeMap<String, String>) -> Self {
        Self { schema: SCHEMA_V2.into(), compatibility_sha256: None, consensus_rules_version: Some(identity::CONSENSUS_RULES_VERSION), seeds, files }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct UpgradeIntent {
    before: Invitation,
    after: Invitation,
}

/// The entries `next` appends to `prev`: append-only, of any version the runtime schedule accepts (future-only is
/// checked against the stored chain by the upgrade). Empty is a rebind: the same schedule in a newer file.
pub fn schedule_transition(prev: &[(u16, u64)], next: &[(u16, u64)]) -> Result<Vec<(u16, u64)>, String> {
    if !next.starts_with(prev) {
        return Err("upgrade may only append future activations; changing or removing an activation is forbidden".into());
    }
    Ok(next[prev.len()..].to_vec())
}

/// The appended fork entries of an upgrade from `before` to `after`. Both bind this release's consensus rules (an
/// uncertified legacy `before` is refused with the §5.4 text), a v2 binding never goes back to v1, and only the fork
/// schedule may change (the genesis, allocations, seeds and the rest of devnet.json stay).
fn upgrade_transition(before: &Invitation, after: &Invitation, legacy: &[Legacy]) -> Result<Vec<(u16, u64)>, String> {
    before.validate()?;
    after.validate()?;
    let ours = identity::CONSENSUS_RULES_VERSION;
    for (which, invitation) in [("saved", before), ("new", after)] {
        let rules = invitation.rules_of_with(legacy)?;
        if rules != ours {
            return Err(format!("the {which} network invitation runs consensus rules {rules}; this release runs rules {ours}"));
        }
    }
    if before.schema == SCHEMA_V2 && after.schema == SCHEMA_V1 {
        return Err("an upgrade never returns a pocol-network-v2 binding to pocol-network-v1".into());
    }
    if before.seeds != after.seeds
        || ["alloc.json", "genesis.spec.hex"].iter().any(|n| before.files[*n] != after.files[*n])
    {
        return Err("upgrade must preserve the genesis, allocations and seed addresses".into());
    }
    let mut old = before.info()?;
    let mut new = after.info()?;
    let appended = schedule_transition(&crate::configured_forks(&old)?, &crate::configured_forks(&new)?)?;
    old.as_object_mut().ok_or("invalid network metadata")?.remove("forks");
    new.as_object_mut().ok_or("invalid network metadata")?.remove("forks");
    if old != new {
        return Err("upgrade may change only the fork schedule".into());
    }
    Ok(appended)
}
/// Where an upgrade keeps the binding it replaces: keyed by that binding's fingerprint, so every upgrade has its own.
fn backup_name(before: &Invitation) -> String { format!("network.pocol.json.before-{}", &before.fingerprint()[..16]) }
/// An interrupted upgrade's intent. One this release cannot finish (a 0.4.0 intent whose saved binding it may not run)
/// is refused with the instruction to finish it with the release that started it.
fn read_intent(pending: &Path, legacy: &[Legacy]) -> Result<UpgradeIntent, String> {
    real_file(pending)?;
    if fs::metadata(pending).map_err(|e| e.to_string())?.len() > (4 * MAX_INVITATION + 4096) as u64 {
        return Err("upgrade intent is too large".into());
    }
    let intent: UpgradeIntent = serde_json::from_slice(&fs::read(pending).map_err(|e| e.to_string())?)
        .map_err(|_| "invalid interrupted upgrade intent; keep the original configuration backup")?;
    match intent.before.rules_of_with(legacy) {
        Ok(rules) if rules == identity::CONSENSUS_RULES_VERSION => Ok(intent),
        Ok(rules) => Err(format!("an interrupted upgrade for consensus rules {rules} is pending; finish this upgrade with the PoCol release that started it first")),
        Err(why) => Err(format!("an interrupted upgrade started by PoCol 0.4.0 is pending; finish this upgrade with PoCol 0.4.0 first ({why})")),
    }
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
    let bytes = read_capped(file).map_err(|e| e.to_string())?;
    if bytes.len() > MAX_INVITATION {
        return Err("file exceeds 1 MiB".into());
    }
    Ok(bytes)
}
/// At most MAX_INVITATION + 1 bytes of `r`, into a buffer reserved at exactly that size, so the read never grows it
/// (A13 counts MAX_INVITATION + 1 for the address book's read, `resources::derive`): `Take` gives `read_to_end` no
/// size hint, and a buffer grown by doubling reaches 2 MiB for any file of 1 MiB or more.
fn read_capped(r: impl Read) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::with_capacity(MAX_INVITATION + 1);
    r.take((MAX_INVITATION + 1) as u64).read_to_end(&mut bytes)?;
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
    let invitation = Invitation::new_v2(seeds, files);
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
    pub fn upgrade(&self, bytes: &[u8]) -> Result<Invitation, String> { self.upgrade_with(bytes, LEGACY_BUILDS) }
    fn upgrade_with(&self, bytes: &[u8], legacy: &[Legacy]) -> Result<Invitation, String> {
        let after = Invitation::parse_with(bytes, legacy)?;
        let saved = self.saved()?;
        let binding = self.path.join("network.pocol.json");
        let pending = self.path.join(UPGRADE_PENDING);
        let resuming = fs::symlink_metadata(&pending).is_ok();
        if !resuming && saved.fingerprint() == after.fingerprint() {
            self.network_files(&after, None)?;
            return Ok(after);
        }
        let intent = if resuming {
            let intent = read_intent(&pending, legacy)?;
            if intent.after.fingerprint() != after.fingerprint()
                || (saved.fingerprint() != intent.before.fingerprint() && saved.fingerprint() != after.fingerprint())
            {
                return Err("resume the interrupted upgrade with the same network invitation".into());
            }
            intent
        } else {
            UpgradeIntent { before: saved, after: after.clone() }
        };
        let network = self.path.join("network");
        let (_appended, _state_lock) = self.plan(&intent.before, &after, resuming, legacy)?;
        let original = serde_json::to_vec_pretty(&intent.before).map_err(|e| e.to_string())?;
        let backup = self.path.join(backup_name(&intent.before));
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
    /// A read-only dry run of `upgrade` under the state lock (plan §1.7): `change` is "none" (already applied),
    /// "rebind" (the same rules and schedule in a newer file) or "activation" (`appended` lists the new entries),
    /// beside the new invitation's `describe()`. Every refusal `upgrade` would give, it gives.
    pub fn inspect_upgrade(&self, bytes: &[u8]) -> Result<serde_json::Value, String> { self.inspect_upgrade_with(bytes, LEGACY_BUILDS) }
    fn inspect_upgrade_with(&self, bytes: &[u8], legacy: &[Legacy]) -> Result<serde_json::Value, String> {
        let after = Invitation::parse_with(bytes, legacy)?;
        let saved = self.saved()?;
        let pending = self.path.join(UPGRADE_PENDING);
        if fs::symlink_metadata(&pending).is_ok() {
            read_intent(&pending, legacy)?;
            return Err("network upgrade is incomplete; resume it with the explicit upgrade command and the same invitation".into());
        }
        let appended = if saved.fingerprint() == after.fingerprint() {
            self.network_files(&after, None)?;
            None
        } else {
            Some(self.plan(&saved, &after, false, legacy)?.0)
        };
        let mut out = after.describe();
        out["change"] = match &appended { None => "none", Some(a) if a.is_empty() => "rebind", Some(_) => "activation" }.into();
        out["appended"] = serde_json::json!(appended.unwrap_or_default());
        Ok(out)
    }
    /// The saved binding, after the checks every upgrade path makes first: the key, its address and the binding
    /// are regular files. Well-formed only; the transition checks its rules.
    fn saved(&self) -> Result<Invitation, String> {
        real_file(&self.path.join("identity.pocol-key"))?;
        real_file(&self.path.join("identity.address"))?;
        let binding = self.path.join("network.pocol.json");
        real_file(&binding)?;
        Invitation::parse_saved(&read_bounded(&binding)?)
    }
    /// The local network files equal `expected`'s, or, while resuming, devnet.json may already be `resumed`'s.
    fn network_files(&self, expected: &Invitation, resumed: Option<&Invitation>) -> Result<(), String> {
        let network = self.path.join("network");
        let meta = fs::symlink_metadata(&network).map_err(|e| e.to_string())?;
        if !meta.is_dir() || meta.file_type().is_symlink() { return Err("network must be a real directory".into()); }
        for name in FILES {
            let path = network.join(name);
            real_file(&path)?;
            let actual = read_bounded(&path)?;
            if actual != expected.files[name].as_bytes() && !(name == "devnet.json" && resumed.is_some_and(|a| actual == a.files[name].as_bytes())) {
                return Err(format!("local network file changed: {name}"));
            }
        }
        Ok(())
    }
    /// Everything an upgrade checks before it writes, shared with the dry run: the transition, the upgraded profile,
    /// the local network files and, when entries are appended, that the first is above every stored block. The
    /// state lock is always taken and is held by the returned file until the caller drops it.
    fn plan(&self, before: &Invitation, after: &Invitation, resuming: bool, legacy: &[Legacy]) -> Result<(Vec<(u16, u64)>, Option<File>), String> {
        let appended = upgrade_transition(before, after, legacy)?;
        let info = after.info()?;
        let genesis: [u8; 32] = types::unhex(info["genesisHash"].as_str().ok_or("missing genesis hash")?)?
            .try_into().map_err(|_| "invalid genesis hash")?;
        pocol_lp4_execution::profile::ProfileIdentity::validate(
            &types::unhex(after.files["genesis.spec.hex"].trim())?, &genesis,
            info["chainId"].as_u64().ok_or("missing chain ID")?,
            pocol_lp4_execution::profile::ProfileKind::Simnet,
            pocol_lp4_execution::profile::NodeKind::Simnet,
            &crate::configured_forks(&info)?,
        ).map_err(|e| format!("upgraded network profile: {e:?}"))?;
        self.network_files(before, resuming.then_some(after))?;
        let (height, lock) = stored_max_height(&self.path, before)?;
        if let Some(&(_, first)) = appended.first() {
            if first <= height {
                return Err(format!("activation {first} must be above every stored block (highest {height})"));
            }
        }
        Ok((appended, lock))
    }
    /// Binds a new home to `bytes`, or checks that an existing binding is the same network (`content_key`): a v2 twin
    /// of a saved v1 file is accepted read-only. An existing binding is never rewritten here.
    pub fn import(&self, bytes: &[u8]) -> Result<Invitation, String> { self.import_with(bytes, LEGACY_BUILDS) }
    fn import_with(&self, bytes: &[u8], legacy: &[Legacy]) -> Result<Invitation, String> {
        let pending = self.path.join(UPGRADE_PENDING);
        if fs::symlink_metadata(&pending).is_ok() {
            read_intent(&pending, legacy)?;
            return Err("network upgrade is incomplete; resume the explicit upgrade command with the same invitation".into());
        }
        let invitation = Invitation::parse_with(bytes, legacy)?;
        let binding = self.path.join("network.pocol.json");
        if binding.exists() {
            let saved = Invitation::parse_with(&read_bounded(&binding)?, legacy)?;
            if saved.content_key() != invitation.content_key() {
                return Err("this home is bound to another invitation; if this is the same network in a newer form run `pocol upgrade`; otherwise use a separate home".into());
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
                // Local P2P policy (net::Limits::peers_max, per_group_max): the established inbound peers, and inbound
                // peers per public address group. A13 counts every inbound place against POCOL_MEMORY_BUDGET_BYTES, so an
                // operator who lowers the budget lowers POCOL_PEERS_MAX with it.
                net: crate::net::Limits {
                    // An empty value counts as unset.
                    peers_max: std::env::var("POCOL_PEERS_MAX").ok().filter(|s|!s.trim().is_empty()).map(|s|crate::net::parse_peers_max(&s).map_err(|_|"Invalid POCOL_PEERS_MAX")).transpose()?.unwrap_or(crate::net::Limits::default().peers_max),
                    // An empty value counts as unset.
                    per_group_max: std::env::var("POCOL_PER_GROUP_MAX").ok().filter(|s|!s.trim().is_empty()).map(|s|s.trim().parse::<usize>().map_err(|_|"Invalid POCOL_PER_GROUP_MAX")).transpose()?.unwrap_or(crate::net::Limits::default().per_group_max),
                    ..Default::default()
                },
                // Peer discovery (VA-SPEC §1a): POCOL_DISCOVERY and POCOL_DISCOVER_LOCAL, as for `pocold run`.
                discovery: { let mut d = node::Discovery::default(); d.env(false, false)?; d },
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
        Invitation { schema: SCHEMA_V2.into(), compatibility_sha256: None, consensus_rules_version: Some(identity::CONSENSUS_RULES_VERSION),
            seeds: vec!["127.0.0.1:30300".parse().unwrap()], files: BTreeMap::from([
                ("alloc.json".into(), "{}".into()), ("genesis.spec.hex".into(), "00".into()),
                ("devnet.json".into(), info.to_string()),
            ]) }
    }
    #[test]
    fn description_does_not_relabel_fixture_networks_or_change_invitation_identity() {
        let invitation = invitation();
        let bytes = serde_json::to_vec(&invitation).unwrap();
        let fingerprint = invitation.fingerprint();
        let description = invitation.describe();
        assert!(description.get("networkName").is_none());
        assert!(description.get("networkEnvironment").is_none());
        assert_eq!(description["publicLaunchReady"], false);
        assert_eq!(description["networkFingerprint"], fingerprint);
        assert_eq!(serde_json::to_vec(&invitation).unwrap(), bytes);
        assert_eq!(invitation.fingerprint(), fingerprint);
    }
    fn journal(path: &Path, heights: &[u64]) -> Vec<u8> { journal_of(path, heights, 777915, [7; 32]) }
    fn journal_of(path: &Path, heights: &[u64], chain_id: u64, genesis: [u8; 32]) -> Vec<u8> {
        let state = path.join("state"); fs::create_dir(&state).unwrap();
        fs::write(state.join("LOCK"), b"").unwrap();
        let mut bytes = b"POCOLJ2\n".to_vec(); bytes.extend(genesis);
        for &height in heights {
            let mut h = types::blank_header(); h.h = height; h.chain_id = chain_id; h.genesis_hash = genesis;
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
    /// A13 (`queues/discovery: address-book file buffers`): the read's buffer stays within the MAX_INVITATION + 1
    /// counted for it, also for a file of exactly 1 MiB, one byte more and one far larger.
    #[test]
    fn a_bounded_read_never_grows_its_buffer_past_the_counted_bound() {
        let t = Scratch::new(); let path = t.0.join("file");
        for len in [0, 240_000, MAX_INVITATION, MAX_INVITATION + 1, 3 * MAX_INVITATION] {
            fs::write(&path, vec![b'x'; len]).unwrap();
            let bytes = read_capped(File::open(&path).unwrap()).unwrap();
            assert!(bytes.len() == len.min(MAX_INVITATION + 1) && bytes.capacity() <= MAX_INVITATION + 1, "{len} B: capacity {}", bytes.capacity());
            assert_eq!(read_bounded(&path).ok().map(|b| b.len()), (len <= MAX_INVITATION).then_some(len), "{len} B");
        }
    }
    #[test]
    fn upgrade_transition_is_append_only_and_metadata_preserving() {
        let before = invitation(); let mut after = before.clone();
        let mut info: serde_json::Value = serde_json::from_str(&after.files["devnet.json"]).unwrap();
        info["forks"] = serde_json::json!([[1,0],[2,21]]);
        after.files.insert("devnet.json".into(), info.to_string());
        assert_eq!(upgrade_transition(&before, &after, LEGACY_BUILDS).unwrap(), [(2, 21)]);
        assert!(upgrade_transition(&after, &before, LEGACY_BUILDS).is_err());
        // The same schedule appends nothing: a rebind (the same fingerprint is a no-op before this runs).
        assert_eq!(upgrade_transition(&after, &after, LEGACY_BUILDS).unwrap(), []);
        info["extra"] = serde_json::json!(true);
        after.files.insert("devnet.json".into(), info.to_string());
        assert!(upgrade_transition(&before, &after, LEGACY_BUILDS).is_err());
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

    // ---- The upgrade path (plan §1.7, §3): T20-T26 ----

    const H15000: &str = include_str!("../../../deploy/mainnet/h15000/network.pocol.json");
    /// The shipped table with its entry certified, as an owner would after the replay (Option A).
    fn certified() -> Vec<Legacy> { LEGACY_BUILDS.iter().map(|l| Legacy { certified: Some("test evidence"), ..*l }).collect() }
    fn h15000() -> Invitation { Invitation::parse_saved(H15000.as_bytes()).unwrap() }
    /// The H15000 genesis files (a real simnet genesis) under v2, with `forks` in place of the schedule when given.
    fn h15000_v2(forks: serde_json::Value) -> Invitation {
        let mut i = h15000();
        i.schema = SCHEMA_V2.into(); i.compatibility_sha256 = None; i.consensus_rules_version = Some(identity::CONSENSUS_RULES_VERSION);
        if !forks.is_null() { let mut info = i.info().unwrap(); info["forks"] = forks; i.files.insert("devnet.json".into(), info.to_string()); }
        i
    }
    fn bytes(i: &Invitation) -> Vec<u8> { serde_json::to_vec_pretty(i).unwrap() }
    /// A home bound to `saved` the way `import` leaves one: key, address, binding and the three network files.
    fn home_with(t: &Scratch, saved: &[u8]) -> Home {
        let home = Home::open(&t.0.join("home")).unwrap();
        home.key().unwrap();
        fs::write(home.path.join("network.pocol.json"), saved).unwrap();
        let invitation = Invitation::parse_saved(saved).unwrap();
        fs::create_dir(home.path.join("network")).unwrap();
        for name in FILES { fs::write(home.path.join("network").join(name), &invitation.files[name]).unwrap(); }
        home
    }
    fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        let mut out = BTreeMap::new();
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() { out.extend(snapshot(&p)) } else { out.insert(p.clone(), fs::read(&p).unwrap()); }
        }
        out
    }
    #[test]
    fn the_h15000_invitation_keeps_its_fingerprint() {
        // T20: the struct at 9c502ff, field for field; the optional fields serialise only when present.
        #[derive(Serialize, Deserialize)]
        #[serde(rename_all = "camelCase", deny_unknown_fields)]
        struct Head { schema: String, compatibility_sha256: String, seeds: Vec<SocketAddr>, files: BTreeMap<String, String> }
        let head: Head = serde_json::from_str(H15000).unwrap();
        let fingerprint = format!("{:x}", Sha256::digest(&serde_json::to_vec(&head).unwrap()));
        assert_eq!(h15000().fingerprint(), fingerprint);
        assert_eq!(h15000().describe()["invitationSchema"], SCHEMA_V1);
    }
    #[test]
    fn a_legacy_h15000_home_starts_only_when_certified_and_bound_to_its_computed_genesis() {
        // T21: the shipped (uncertified) table refuses with the §5.4 text; a certified one starts the home and writes nothing.
        let t = Scratch::new();
        let home = home_with(&t, H15000.as_bytes());
        let before = snapshot(&home.path);
        assert_eq!(home.import(H15000.as_bytes()).unwrap_err(), legacy_refusal());
        assert_eq!(Invitation::parse(H15000.as_bytes()).unwrap_err(), legacy_refusal());
        let certified = certified();
        home.import_with(H15000.as_bytes(), &certified).unwrap();
        assert_eq!(snapshot(&home.path), before, "a certified legacy home starts without any write");
        assert_eq!(h15000().rules_of_with(&certified), Ok(1));
        // Declared metadata that differs from what genesis.spec.hex computes, or an entry for another chain: refused.
        let mut forged = h15000();
        let mut info = forged.info().unwrap();
        info["genesisHash"] = types::hex(&[9; 32]).into();
        forged.files.insert("devnet.json".into(), info.to_string());
        assert!(Invitation::parse_with(&bytes(&forged), &certified).unwrap_err().contains("does not match"));
        let elsewhere: Vec<Legacy> = certified.iter().map(|l| Legacy { genesis_hash: "0x07", ..*l }).collect();
        assert!(Invitation::parse_with(H15000.as_bytes(), &elsewhere).unwrap_err().contains("does not match"));
        // The dropped arm (A2-2): a v1 file naming this build's own digest is no longer special.
        let mut own = h15000();
        own.compatibility_sha256 = Some(identity::COMPATIBILITY_HEX.into());
        assert_eq!(Invitation::parse_with(&bytes(&own), &certified).unwrap_err(), "network invitation requires a different PoCol release");
    }
    #[test]
    fn a_second_upgrade_leaves_the_full_nonce_backup_and_writes_its_own() {
        // T22: 0.4.0's fixed backup name held the first upgrade's binding; a later upgrade must not collide with it.
        let t = Scratch::new();
        let (before, after) = (h15000_v2(serde_json::json!([[1, 0]])), h15000_v2(serde_json::json!([[1, 0], [2, 15000]])));
        let home = home_with(&t, &bytes(&before));
        let legacy = home.path.join(LEGACY_UPGRADE_BACKUP);
        fs::write(&legacy, b"the 0.4.0 full-nonce backup of another binding").unwrap();
        assert_eq!(home.inspect_upgrade(&bytes(&after)).unwrap()["change"], "activation");
        home.upgrade(&bytes(&after)).unwrap();
        assert_eq!(fs::read(&legacy).unwrap(), b"the 0.4.0 full-nonce backup of another binding");
        let backup = home.path.join(backup_name(&before));
        assert_eq!(Invitation::parse_saved(&fs::read(&backup).unwrap()).unwrap().fingerprint(), before.fingerprint());
        assert_eq!(fs::read(home.path.join("network.pocol.json")).unwrap(), bytes(&after));
        assert!(!home.path.join(UPGRADE_PENDING).exists());
        assert_eq!(home.inspect_upgrade(&bytes(&after)).unwrap()["change"], "none");
    }
    #[test]
    fn a_v2_binding_does_not_depend_on_the_build_digest() {
        // T23: what `pocol network` writes names the rules, not a digest, so a release with another digest (R2) and no
        // legacy table parses and imports it unchanged.
        let packed = Invitation::new_v2(h15000().seeds, h15000().files);
        let text = String::from_utf8(bytes(&packed)).unwrap();
        assert!(text.contains("\"schema\": \"pocol-network-v2\"") && text.contains("\"consensusRulesVersion\": 1"), "{text}");
        assert!(!text.contains("compatibilitySha256") && !text.contains(identity::COMPATIBILITY_HEX));
        Invitation::parse_with(&bytes(&packed), &[]).unwrap();
        let t = Scratch::new();
        let home = home_with(&t, &bytes(&packed));
        home.import_with(&bytes(&packed), &[]).unwrap();
        assert_eq!(packed.describe()["consensusIdentity"], "bcc4b71570fed4788d84baff137e26a88527c1fd17a11f595d92ce1e5e50b110", "the T11 golden identity");
        let mut other = packed.clone();
        other.consensus_rules_version = Some(identity::CONSENSUS_RULES_VERSION + 1);
        assert!(Invitation::parse(&bytes(&other)).unwrap_err().contains("different PoCol consensus release"));
    }
    #[test]
    fn schedule_transition_returns_exactly_the_appended_entries() {
        // T24: any number of entries of any version; existing entries never change.
        assert_eq!(schedule_transition(&[(1, 0), (2, 100)], &[(1, 0), (2, 100), (3, 200)]), Ok(vec![(3, 200)]));
        assert_eq!(schedule_transition(&[(1, 0)], &[(1, 0), (2, 100), (3, 200)]), Ok(vec![(2, 100), (3, 200)]));
        assert_eq!(schedule_transition(&[(1, 0), (2, 100)], &[(1, 0), (2, 100)]), Ok(vec![]));
        for next in [&[(1, 0)][..], &[(1, 0), (2, 101)], &[(1, 0), (3, 100)], &[(1, 0), (2, 101), (3, 200)]] {
            assert!(schedule_transition(&[(1, 0), (2, 100)], next).unwrap_err().contains("forbidden"), "{next:?}");
        }
    }
    #[test]
    fn importing_the_v2_twin_of_a_saved_v1_binding_changes_nothing() {
        // T25: the same seeds and files are the same network; the binding stays the saved v1 file.
        let t = Scratch::new();
        let home = home_with(&t, H15000.as_bytes());
        let before = snapshot(&home.path);
        let twin = h15000_v2(serde_json::Value::Null);
        assert_eq!(twin.content_key(), h15000().content_key());
        assert_ne!(twin.fingerprint(), h15000().fingerprint());
        home.import_with(&bytes(&twin), &certified()).unwrap();
        assert_eq!(snapshot(&home.path), before);
        assert_eq!(home.import(&bytes(&twin)).unwrap_err(), legacy_refusal(), "the saved v1 binding still needs its certification");
        let other = h15000_v2(serde_json::json!([[1, 0], [2, 15001]]));
        assert!(home.import_with(&bytes(&other), &certified()).unwrap_err().contains("bound to another invitation; if this is the same network in a newer form run `pocol upgrade`"));
        assert_eq!(snapshot(&home.path), before);
    }
    #[test]
    fn a_certified_legacy_home_rebinds_to_its_v2_twin_and_never_goes_back() {
        let t = Scratch::new();
        let home = home_with(&t, H15000.as_bytes());
        let twin = h15000_v2(serde_json::Value::Null);
        assert_eq!(home.upgrade(&bytes(&twin)).unwrap_err(), legacy_refusal(), "uncertified as shipped");
        assert_eq!(home.inspect_upgrade(&bytes(&twin)).unwrap_err(), legacy_refusal());
        let certified = certified();
        let dry = home.inspect_upgrade_with(&bytes(&twin), &certified).unwrap();
        assert_eq!((dry["change"].as_str(), dry["appended"].as_array().map(Vec::len)), (Some("rebind"), Some(0)));
        assert_eq!(dry["consensusIdentity"], "bcc4b71570fed4788d84baff137e26a88527c1fd17a11f595d92ce1e5e50b110");
        home.upgrade_with(&bytes(&twin), &certified).unwrap();
        assert_eq!(fs::read(home.path.join("network.pocol.json")).unwrap(), bytes(&twin));
        assert_eq!(fs::read(home.path.join(backup_name(&h15000()))).unwrap(), bytes(&h15000()), "the v1 binding is kept for a rollback");
        assert!(upgrade_transition(&twin, &h15000(), &certified).unwrap_err().contains("never returns a pocol-network-v2 binding"));
    }
    #[test]
    fn an_upgrade_refuses_every_change_but_appended_future_activations() {
        // T26.
        let t = Scratch::new();
        let before = h15000_v2(serde_json::json!([[1, 0]]));
        let home = home_with(&t, &bytes(&before));
        let binding = fs::read(home.path.join("network.pocol.json")).unwrap();
        let at = |forks| bytes(&h15000_v2(forks));
        assert!(home.upgrade(&at(serde_json::json!([[1, 0], [2, 100], [3, 200]]))).unwrap_err().contains("consensus fork schedule"), "v3 under MAX_FORK_VERSION = 2");
        let mut rules = h15000_v2(serde_json::json!([[1, 0], [2, 100]]));
        rules.consensus_rules_version = Some(2);
        assert!(home.upgrade(&bytes(&rules)).unwrap_err().contains("different PoCol consensus release"));
        assert!(upgrade_transition(&before, &rules, LEGACY_BUILDS).unwrap_err().contains("runs consensus rules 2"));
        let changed = |edit: &dyn Fn(&mut Invitation)| { let mut i = h15000_v2(serde_json::json!([[1, 0], [2, 100]])); edit(&mut i); i };
        for (what, i) in [
            ("seeds", changed(&|i| i.seeds = vec!["127.0.0.1:30301".parse().unwrap()])),
            ("alloc", changed(&|i| { i.files.get_mut("alloc.json").unwrap().push(' '); })),
            ("spec", changed(&|i| { i.files.get_mut("genesis.spec.hex").unwrap().push('\n'); })),
            ("devnet.json key", changed(&|i| { let mut info = i.info().unwrap(); info["genesisTime"] = 1.into(); i.files.insert("devnet.json".into(), info.to_string()); })),
        ] {
            assert!(upgrade_transition(&before, &i, LEGACY_BUILDS).is_err(), "{what}");
        }
        // Future-only: an activation at or below a stored block is refused, and nothing is written.
        journal_of(&home.path, &[1, 20], 1599846715, types::unhex("0xbe137bfdb4fdf332f2e1500ee1a67e0b22ce6bdf57efa46fb4ea7b0debe509ca").unwrap().try_into().unwrap());
        assert!(home.upgrade(&at(serde_json::json!([[1, 0], [2, 20]]))).unwrap_err().contains("activation 20 must be above every stored block (highest 20)"));
        assert!(home.inspect_upgrade(&at(serde_json::json!([[1, 0], [2, 20]]))).is_err(), "the dry run refuses it too");
        assert_eq!(fs::read(home.path.join("network.pocol.json")).unwrap(), binding);
        assert!(!home.path.join(UPGRADE_PENDING).exists());
        assert_eq!(home.inspect_upgrade(&at(serde_json::json!([[1, 0], [2, 21]]))).unwrap()["appended"], serde_json::json!([[2, 21]]));
        // A running wallet holds the home's lock.
        assert!(Home::open(&home.path).is_err());
    }
    #[test]
    fn a_pending_upgrade_left_by_0_4_0_must_be_finished_with_0_4_0() {
        let t = Scratch::new();
        let home = home_with(&t, H15000.as_bytes());
        let mut after = h15000();
        let mut info = after.info().unwrap(); info["forks"] = serde_json::json!([[1, 0], [2, 15001]]); after.files.insert("devnet.json".into(), info.to_string());
        fs::write(home.path.join(UPGRADE_PENDING), serde_json::to_vec(&UpgradeIntent { before: h15000(), after }).unwrap()).unwrap();
        let twin = bytes(&h15000_v2(serde_json::Value::Null));
        for e in [home.upgrade(&twin).map(|_| ()), home.import(H15000.as_bytes()).map(|_| ()), home.inspect_upgrade(&twin).map(|_| ())] {
            assert!(e.unwrap_err().contains("finish this upgrade with PoCol 0.4.0 first"));
        }
    }
}
