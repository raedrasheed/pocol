//! One shared, bounded public network invitation; all secrets and state stay on the device.
//! A `pocol-network-v2` invitation binds a home to the consensus rules revision it runs (VA-SPEC §2.1), not to a build
//! digest, so a release that changes only the digest starts on existing homes, and fork entries of any version the
//! runtime schedule accepts can be appended (future-only, append-only). A `pocol-network-v1` file names the 0.4.0 build
//! digest it was made by and is accepted only through a certified `LEGACY_BUILDS` entry (plan §1.7).
use crate::{identity, keyfile, keyfile::backup, node, testnet, types};
use pocol_lp4_execution::txsign::Key;
use zeroize::Zeroizing;
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
    /// v2 only: advisory bootstrap addresses (VA-SPEC §1a.9, D17), community listeners that agreed to be listed. Not
    /// part of the network (`content_key`): a newer file with another list is the same network, which `pocol upgrade`
    /// rebinds to. Skipped when empty, so files without it keep their bytes and fingerprints.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    bootstrap: Vec<SocketAddr>,
    /// v2 only: the seeds are retired (VA-SPEC §1a.8, D23): nodes dial none of them. Not part of the network either;
    /// skipped when false.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    seeds_retired: bool,
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
        // D17, D23: the advisory fields are v2 only; a bootstrap list holds at most 16 distinct learnable addresses
        // (ports >= 1024, VA-SPEC §1a.3), none a seed.
        if (!self.bootstrap.is_empty() || self.seeds_retired) && self.schema != SCHEMA_V2 {
            return Err("bootstrap addresses and seedsRetired need a pocol-network-v2 invitation".into());
        }
        let distinct: HashSet<_> = self.bootstrap.iter().collect();
        if self.bootstrap.len() > crate::node::BOOTSTRAP_MAX || distinct.len() != self.bootstrap.len()
            || self.bootstrap.iter().any(|a| a.port() < 1024 || a.ip().is_unspecified() || a.ip().is_multicast() || self.seeds.contains(a))
        {
            return Err("provide at most 16 distinct bootstrap addresses with ports >= 1024, none a seed".into());
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
        let mut out = serde_json::json!({"networkFingerprint":self.fingerprint(),"invitationSchema":self.schema,
            "consensusRulesVersion":self.rules_of().ok(),"consensusIdentity":self.published_identity(),"chainId":info["chainId"],"genesisHash":info["genesisHash"],
            "forkSchedule":forks,"activations":activations,"fullNonceActivationHeight":activation,"seeds":self.seeds,"bootstrap":self.bootstrap,"seedsRetired":self.seeds_retired,"scope":"experimental-testnet"});
        crate::readiness::annotate(&mut out, &self.launch_blockers(&info, &forks));
        crate::network_display::annotate_genesis(out, &self.files["genesis.spec.hex"])
    }
    /// V3-28 H1: this build's readiness for the invitation's network, from what genesis.spec.hex commits to (chain,
    /// genesis hash, spec version), the fork schedule and the admission devnet.json records. The launcher admits
    /// nothing itself; a node's status reports the admission it booted with.
    fn launch_blockers(&self, info: &serde_json::Value, forks: &[(u16, u64)]) -> Vec<crate::readiness::Blocker> {
        use crate::readiness::{blockers, AdmissionFacts, Build, Network, Subject};
        let genesis = types::unhex(self.files.get("genesis.spec.hex").map_or("", |s| s.trim())).and_then(|bytes| {
            if bytes.len() > crate::genesis_spec::MAX_SPEC_BYTES { return Err("genesis specification exceeds its bound".into()) }
            let spec = crate::genesis_spec::decode(&bytes).map_err(|e| format!("genesis specification: {e:?}"))?;
            Ok((spec, crate::genesis_spec::genesis_hash(&bytes)))
        });
        match genesis {
            Err(why) => blockers(&Build::this(), Subject::Unreadable(why)),
            Ok((spec, genesis_hash)) => blockers(&Build::this(), Subject::Network(&Network { chain_id: spec.chain_id, genesis_hash,
                v3_active: spec.spec_version >= 2 || forks.iter().any(|&(version, _)| version >= 3), admission: AdmissionFacts::recorded(info) })),
        }
    }
    /// What `pocol network` writes: v2, bound to this release's consensus rules, with its bootstrap list and whether the
    /// seeds are retired.
    fn new_v2(seeds: Vec<SocketAddr>, bootstrap: Vec<SocketAddr>, seeds_retired: bool, files: BTreeMap<String, String>) -> Self {
        Self { schema: SCHEMA_V2.into(), compatibility_sha256: None, consensus_rules_version: Some(identity::CONSENSUS_RULES_VERSION), seeds, files, bootstrap, seeds_retired }
    }
    /// D17: the advisory bootstrap addresses.
    pub fn bootstrap(&self) -> &[SocketAddr] { &self.bootstrap }
    /// D23: whether the seeds are retired.
    pub fn seeds_retired(&self) -> bool { self.seeds_retired }
    /// VA-SPEC §1a.8 (D23, D27), what a home runs with: the configured peers (the static `peers`, then the seeds), the
    /// seeds, and whether this host is a seed. No seed at all once the invitation retires them; a seed that is this
    /// host's own listener (`node::is_own_seed`) is dropped and makes the node a seed host, which keeps the others; a
    /// seed also given as a static peer stays static.
    pub fn launch_peers(&self, listen: Option<SocketAddr>, peers: &[SocketAddr]) -> (Vec<SocketAddr>, Vec<SocketAddr>, bool) {
        if self.seeds_retired { return (peers.to_vec(), vec![], false) }
        let is_seed = self.seeds.iter().any(|s| node::is_own_seed(listen, *s));
        let seeds: Vec<SocketAddr> = self.seeds.iter().copied().filter(|a| !node::is_own_seed(listen, *a) && !peers.contains(a)).collect();
        let mut all = peers.to_vec(); all.extend(seeds.iter().copied());
        (all, seeds, is_seed)
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
/// The pending `intent` resumes only with its own file, from a home still bound to its `before` or already to `after`.
fn resumable(intent: UpgradeIntent, saved: &Invitation, after: &Invitation) -> Result<UpgradeIntent, String> {
    if intent.after.fingerprint() != after.fingerprint() || (saved.fingerprint() != intent.before.fingerprint() && saved.fingerprint() != after.fingerprint()) {
        return Err("resume the interrupted upgrade with the same network invitation".into());
    }
    Ok(intent)
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
pub fn pack(network: &Path, seeds: Vec<SocketAddr>, bootstrap: Vec<SocketAddr>, seeds_retired: bool, out: &Path) -> Result<(), String> {
    let mut files = BTreeMap::new();
    for name in FILES {
        files.insert(
            name.into(),
            String::from_utf8(read_bounded(&network.join(name))?)
                .map_err(|_| "genesis files must be UTF-8")?,
        );
    }
    let invitation = Invitation::new_v2(seeds, bootstrap, seeds_retired, files);
    let bytes = serde_json::to_vec_pretty(&invitation).map_err(|e| e.to_string())?;
    Invitation::parse(&bytes)?;
    // Full admission still runs on each device, including its local storage measurement.
    crate::load_spec_for(network, &testnet::local_for_network(network)?, None)?;
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
const KEY_FILE: &str = "identity.pocol-key";
const ADDRESS_FILE: &str = "identity.address";
/// The wallet's own transfer history; it belongs to the account in [`ADDRESS_FILE`].
pub(crate) const ACTIVITY_FILE: &str = "wallet-activity.json";
/// The restored key, written before the swap (`import_key_backup`).
const STAGED_KEY: &str = "identity.pocol-key.restoring";
/// The restored account's address while a restore swaps files; renamed onto [`ADDRESS_FILE`] as the last step, so
/// an interrupted restore is finished, not undone, by the next [`Home::key`] (or the next restore).
const RESTORE_RECORD: &str = "identity.address.restoring";
#[cfg(test)]
thread_local! {
    /// Test hook: `import_key_backup` stops (as a crash would) right before step N of its swap.
    static CRASH_BEFORE: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}
fn crash_point(_step: u8) -> Result<(), String> {
    #[cfg(test)]
    if CRASH_BEFORE.with(|c| c.get()) == _step {
        return Err(format!("simulated crash before restore step {_step}"));
    }
    Ok(())
}
struct Current {
    address: String,
    readable: bool,
}
fn backup_address(secret: &[u8; 32]) -> Result<String, String> {
    Ok(format!("{:#x}", Key::from_secret(secret).map_err(|_| "key backup holds an invalid secp256k1 secret")?.address()))
}
/// `stem` + `suffix` in `dir`, or the first `stem.N` + `suffix` (N < 100) that does not exist yet.
fn free_name(dir: &Path, stem: &str, suffix: &str) -> Result<String, String> {
    (0..100)
        .map(|n| if n == 0 { format!("{stem}{suffix}") } else { format!("{stem}.{n}{suffix}") })
        .find(|name| fs::symlink_metadata(dir.join(name)).is_err())
        .ok_or_else(|| format!("too many files named {stem}*{suffix}; move old ones aside"))
}
/// The address in the identity marker at `marker`, which must be a real file holding `0x` and 40 hex digits.
fn read_marker(marker: &Path) -> Result<String, String> {
    real_file(marker)?;
    let text = String::from_utf8(read_bounded(marker)?).map_err(|_| "invalid identity address marker")?;
    if text.len() != 42 || !text.starts_with("0x") || !text[2..].bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("invalid identity address marker".into());
    }
    Ok(text)
}
/// Rewrites the public identity address marker atomically (temporary file, then rename).
fn set_marker(dir: &Path, address: &str) -> Result<(), String> {
    let marker = dir.join(ADDRESS_FILE);
    if fs::symlink_metadata(&marker).is_ok() {
        real_file(&marker)?;
        if read_bounded(&marker)? == address.as_bytes() {
            return Ok(());
        }
    }
    let temp = dir.join(format!("{ADDRESS_FILE}.tmp"));
    if fs::symlink_metadata(&temp).is_ok() {
        real_file(&temp)?;
        fs::remove_file(&temp).map_err(|e| e.to_string())?;
    }
    write_new(&temp, address.as_bytes())?;
    fs::rename(&temp, &marker).map_err(|e| e.to_string())
}
impl Home {
    /// [`Home::open`] for a command that must never create a wallet: `path` must already be a directory, so a
    /// mistyped `--home` creates no directory and no lock file.
    pub fn open_existing(path: &Path, command: &str) -> Result<Self, String> {
        if !path.is_dir() {
            return Err(format!("{command} requires an existing PoCol home; it never creates a wallet"));
        }
        Self::open(path)
    }
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
        self.finish_restore()?;
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
    /// A passphrase-encrypted backup of this home's key (`keyfile::backup`, CRY-02) and its address. The
    /// file is opened again before it is returned, so a backup that would not restore is never handed out.
    /// The plaintext key is never written anywhere; the caller stores the returned ciphertext.
    pub fn export_key_backup(&self, passphrase: &[u8]) -> Result<(Vec<u8>, String), String> {
        backup::check_new_passphrase(passphrase)?;
        let key = self.key()?;
        let secret = Zeroizing::new(key.secret());
        let file = backup::seal(&secret, passphrase)?;
        if backup::open(&file, passphrase)?.as_ref() != secret.as_ref() {
            return Err("key backup self-check failed; nothing was exported".into());
        }
        Ok((file, format!("{:#x}", key.address())))
    }
    /// What restoring `file` would do, without writing anything: the backup's address, the account this home
    /// holds now (`null` for a new home) and whether restoring replaces it (then `import_key_backup` needs
    /// `replace` = `current`).
    pub fn inspect_key_backup(&self, file: &[u8], passphrase: &[u8]) -> Result<serde_json::Value, String> {
        let address = backup_address(&*backup::open(file, passphrase)?)?;
        let current = self.current_identity()?;
        let replaces = current.as_ref().is_some_and(|c| !c.address.eq_ignore_ascii_case(&address));
        Ok(serde_json::json!({"address": address, "current": current.as_ref().map(|c| &c.address),
            "currentKeyReadable": current.as_ref().map(|c| c.readable), "replaces": replaces}))
    }
    /// Restores the key in `file` into this home. Restoring the account the home already holds only repairs a
    /// missing key file or address marker. Another account replaces the current one only when `replace` names
    /// the current address (the confirmation the user saw); the old key file is kept beside it as
    /// `identity.pocol-key.replaced-<address>`, never deleted, and its wallet activity moves aside with it.
    pub fn import_key_backup(&self, file: &[u8], passphrase: &[u8], replace: Option<&str>) -> Result<serde_json::Value, String> {
        let secret = backup::open(file, passphrase)?;
        self.finish_restore()?;
        let address = backup_address(&secret)?;
        let current = self.current_identity()?;
        let key_path = self.path.join(KEY_FILE);
        let same = current.as_ref().is_some_and(|c| c.address.eq_ignore_ascii_case(&address));
        match &current {
            // The key file already holds this account: only the marker can need repair.
            Some(c) if same && c.readable => {
                set_marker(&self.path, &c.address)?;
                return Ok(serde_json::json!({"address": c.address, "changed": false}));
            }
            Some(c) if !same && !replace.is_some_and(|r| r.eq_ignore_ascii_case(&c.address)) => {
                return Err(format!("This wallet holds account {}. Confirm replacing it to restore {address}", c.address));
            }
            _ => {}
        }
        // Stage the restored key and the record naming its address, move the current key aside, swap, then
        // rename the record onto the marker. The directory is synced after every step, and a crash at any point
        // leaves a state the next `key()` finishes (`finish_restore`): before the old key moves, nothing changed;
        // after, the restore completes. A staged key or record left by an interrupted restore that never reached
        // the swap holds a key from a backup file, so it is replaced.
        let (staged, record) = (self.path.join(STAGED_KEY), self.path.join(RESTORE_RECORD));
        if let Some(c) = &current {
            // The marker names the account being replaced, so a finished restore knows whose activity moves aside.
            set_marker(&self.path, &c.address)?;
        }
        for leftover in [&staged, &record] {
            if fs::symlink_metadata(leftover).is_ok() {
                real_file(leftover)?;
                fs::remove_file(leftover).map_err(|e| e.to_string())?;
            }
        }
        keyfile::store(&staged, &secret)?;
        write_new(&record, address.as_bytes())?;
        sync_directory(&self.path)?;
        crash_point(1)?;
        let mut retired = serde_json::Value::Null;
        if let Some(c) = &current {
            if fs::symlink_metadata(&key_path).is_ok() {
                let name = free_name(&self.path, &format!("{KEY_FILE}.replaced-{}", c.address), "")?;
                fs::rename(&key_path, self.path.join(&name)).map_err(|e| e.to_string())?;
                sync_directory(&self.path)?;
                retired = name.into();
            }
            crash_point(2)?;
            self.retire_activity(&address)?;
        }
        crash_point(3)?;
        fs::rename(&staged, &key_path).map_err(|e| e.to_string())?;
        sync_directory(&self.path)?;
        crash_point(4)?;
        fs::rename(&record, self.path.join(ADDRESS_FILE)).map_err(|e| e.to_string())?;
        sync_directory(&self.path)?;
        Ok(serde_json::json!({"address": address, "changed": true,
            "previous": current.map(|c| c.address), "previousKeyFile": retired}))
    }
    /// Finishes a restore that a crash interrupted after the current key was moved aside: the staged key takes
    /// the key file's place if that is still empty, and the record becomes the address marker once the key file
    /// holds the recorded account, the replaced account's activity moving aside first. A record whose restore
    /// never reached the swap is left for the next restore to replace; nothing is ever deleted here.
    fn finish_restore(&self) -> Result<(), String> {
        let record = self.path.join(RESTORE_RECORD);
        if fs::symlink_metadata(&record).is_err() {
            return Ok(());
        }
        real_file(&record)?;
        let recorded = read_bounded(&record)?;
        let address_of = |path: &Path| keyfile::load(path).ok().map(|k| format!("{:#x}", k.address()));
        let (key_path, staged) = (self.path.join(KEY_FILE), self.path.join(STAGED_KEY));
        if fs::symlink_metadata(&key_path).is_err() && fs::symlink_metadata(&staged).is_ok() {
            real_file(&staged)?;
            if address_of(&staged).is_some_and(|a| a.as_bytes() == recorded) {
                fs::rename(&staged, &key_path).map_err(|e| e.to_string())?;
                sync_directory(&self.path)?;
            }
        }
        if address_of(&key_path).is_some_and(|a| a.as_bytes() == recorded) {
            self.retire_activity(&String::from_utf8_lossy(&recorded))?;
            fs::rename(&record, self.path.join(ADDRESS_FILE)).map_err(|e| e.to_string())?;
            sync_directory(&self.path)?;
        }
        Ok(())
    }
    /// Another account's transfers are not `restored`'s history: while the marker still names the replaced
    /// account, its `wallet-activity.json` moves aside as `wallet-activity.replaced-<address>.json` (a lost or
    /// unreadable key of the same account keeps it). Idempotent, and run before the record becomes the marker
    /// by both the restore and [`Home::finish_restore`], so no crash point leaves the old history in place.
    fn retire_activity(&self, restored: &str) -> Result<(), String> {
        let (marker, activity) = (self.path.join(ADDRESS_FILE), self.path.join(ACTIVITY_FILE));
        if fs::symlink_metadata(&marker).is_err() || fs::symlink_metadata(&activity).is_err() {
            return Ok(());
        }
        let old = read_marker(&marker)?;
        if old.eq_ignore_ascii_case(restored) {
            return Ok(());
        }
        let name = free_name(&self.path, &format!("wallet-activity.replaced-{old}"), ".json")?;
        fs::rename(&activity, self.path.join(name)).map_err(|e| e.to_string())?;
        sync_directory(&self.path)
    }
    /// The account this home is bound to: its key file's address, or, when the key file is missing or cannot
    /// be read (another Windows profile, a damaged file), the address marker's. `None` for a new home.
    fn current_identity(&self) -> Result<Option<Current>, String> {
        let (key_path, marker) = (self.path.join(KEY_FILE), self.path.join(ADDRESS_FILE));
        let readable = match fs::symlink_metadata(&key_path) {
            Ok(_) => keyfile::load(&key_path).ok().map(|k| format!("{:#x}", k.address())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => return Err(e.to_string()),
        };
        if let Some(address) = readable {
            return Ok(Some(Current { address, readable: true }));
        }
        match fs::symlink_metadata(&marker) {
            Ok(_) => Ok(Some(Current { address: read_marker(&marker)?, readable: false })),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if fs::symlink_metadata(&key_path).is_ok() {
                    return Err("the identity key cannot be read and no address marker names it; nothing changed".into());
                }
                Ok(None)
            }
            Err(e) => Err(e.to_string()),
        }
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
        let intent = if resuming { resumable(read_intent(&pending, legacy)?, &saved, &after)? } else { UpgradeIntent { before: saved, after: after.clone() } };
        let network = self.path.join("network");
        let (_appended, _state_lock) = self.plan(&intent.before, &after, resuming, legacy)?;
        let original = serde_json::to_vec_pretty(&intent.before).map_err(|e| e.to_string())?;
        let backup = self.path.join(backup_name(&intent.before));
        if !self.backup_kept(&intent.before)? { replace_public(&backup, &original)?; }
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
    /// beside the new invitation's `describe()`. It makes every check `upgrade` makes before it writes. An upgrade this
    /// release left pending is finished only by `upgrade` with the same file, so for that file the dry run reports the
    /// interrupted transition (from the intent's binding) with `resuming: true` and the wallets confirm and resume it;
    /// any other file is refused, as `upgrade` refuses it. Refusing the same file left a wallet with no way to resume.
    pub fn inspect_upgrade(&self, bytes: &[u8]) -> Result<serde_json::Value, String> { self.inspect_upgrade_with(bytes, LEGACY_BUILDS) }
    fn inspect_upgrade_with(&self, bytes: &[u8], legacy: &[Legacy]) -> Result<serde_json::Value, String> {
        let after = Invitation::parse_with(bytes, legacy)?;
        let saved = self.saved()?;
        let pending = self.path.join(UPGRADE_PENDING);
        let resuming = fs::symlink_metadata(&pending).is_ok();
        let before = if resuming { resumable(read_intent(&pending, legacy)?, &saved, &after)?.before } else { saved };
        let appended = if !resuming && before.fingerprint() == after.fingerprint() {
            self.network_files(&after, None)?;
            None
        } else {
            let (appended, _state_lock) = self.plan(&before, &after, resuming, legacy)?;
            self.backup_kept(&before)?;
            Some(appended)
        };
        let mut out = after.describe();
        out["change"] = match &appended { None => "none", Some(a) if a.is_empty() => "rebind", Some(_) => "activation" }.into();
        out["appended"] = serde_json::json!(appended.unwrap_or_default());
        out["resuming"] = resuming.into();
        // A7 (D17): what changes beside the network itself, to compare with the published list and fingerprint.
        out["bootstrapBefore"] = serde_json::json!(before.bootstrap);out["bootstrapAfter"] = serde_json::json!(after.bootstrap);
        out["seedsRetiredBefore"] = before.seeds_retired.into();out["seedsRetiredAfter"] = after.seeds_retired.into();
        out["fingerprintBefore"] = before.fingerprint().into();out["fingerprintAfter"] = after.fingerprint().into();
        Ok(out)
    }
    /// Whether `before`'s backup exists. One that exists must be a regular file holding `before`, or nothing is written.
    fn backup_kept(&self, before: &Invitation) -> Result<bool, String> {
        let backup = self.path.join(backup_name(before));
        if fs::symlink_metadata(&backup).is_err() { return Ok(false) }
        real_file(&backup)?;
        if Invitation::parse_saved(&read_bounded(&backup)?)?.fingerprint() != before.fingerprint() {
            return Err("existing original-invitation backup differs; nothing changed".into());
        }
        Ok(true)
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
    /// `pocol start`: the node with `peers` as static configured peers (`--peer`) besides the invitation's seeds.
    pub fn run(
        &self,
        listen: Option<SocketAddr>,
        rpc: Option<SocketAddr>,
        peers: &[SocketAddr],
        stop: impl FnMut(&mut node::Node) -> bool,
    ) -> Result<(), String> {
        self.run_mode(listen,rpc,peers,true,stop)
    }
    pub fn run_mode(
        &self, listen: Option<SocketAddr>, rpc: Option<SocketAddr>, peers: &[SocketAddr], mining: bool,
        mut stop: impl FnMut(&mut node::Node) -> bool,
    ) -> Result<(), String> {
        let key = self.key()?;
        let invitation = self.import(&read_bounded(&self.path.join("network.pocol.json"))?)?;
        if rpc.is_some_and(|a| !a.ip().is_loopback()) {
            return Err("native RPC must remain loopback-only".into());
        }
        let data = self.path.join("state");
        let spec =
            crate::load_spec_for(&self.path.join("network"), &testnet::local_for_network(&self.path.join("network"))?, Some(&data))?;
        let declared: serde_json::Value =
            serde_json::from_str(&invitation.files["devnet.json"]).map_err(|e| e.to_string())?;
        if declared["genesisHash"] != types::hex(&spec.genesis_hash()) {
            return Err("invitation genesis hash mismatch".into());
        }
        let (peers, seeds, is_seed) = invitation.launch_peers(listen, peers);
        let bootstrap = invitation.bootstrap().to_vec();
        let (freeze, not_applied) = self.effective_user_tx_freeze()?;
        if let Some(note) = not_applied { eprintln!("{note}"); }
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
                // Peer discovery (VA-SPEC §1a): POCOL_DISCOVERY, POCOL_DISCOVER_LOCAL and POCOL_OUTBOUND, as for `pocold run`;
                // the invitation's seeds and bootstrap addresses.
                discovery: { let mut d = node::Discovery::default(); d.env(false, false)?; d.outbound_env(false)?;
                    d.seeds = seeds; d.is_seed = is_seed; d.bootstrap = bootstrap; d.seeds_retired = invitation.seeds_retired(); d },
                // V3-33: the home's saved freeze for this network (`pocol freeze-user-txs`), or POCOL_FREEZE_USER_TXS_AFTER.
                freeze_user_txs_after: freeze,
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

/// V3-33 re-genesis conduct (O-33=b): the home's saved freeze setting (`Home::set_user_tx_freeze`), which `pocol start`
/// (and the Android service, which runs the same `run_mode`) passes to its node as `node::Limits::freeze_user_txs_after`.
/// The setting names the network it was saved on (the chain ID and genesis hash that the home's genesis.spec.hex
/// commits to), and applies only to that network: a home that keeps its key and moves to a new network after the
/// re-genesis must not freeze the new chain once its head reaches the old S.
pub const FREEZE_FILE: &str = "freeze-user-txs.json";
pub const FREEZE_SCHEMA: &str = "pocol-freeze-user-txs-v1";
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FreezeFile { schema: String, pub freeze_user_txs_after: u64, pub chain_id: u64, pub genesis_hash: String }
impl Home {
    /// The chain ID and genesis hash of the network this home is bound to (computed from its saved invitation's
    /// genesis.spec.hex, not the declared metadata), or None when it is bound to none yet.
    fn bound_network(&self) -> Result<Option<(u64, String)>, String> {
        let binding = self.path.join("network.pocol.json");
        if fs::symlink_metadata(&binding).is_err() { return Ok(None) }
        real_file(&binding)?;
        Invitation::parse_saved(&read_bounded(&binding)?)?.computed_network().map(Some)
    }
    /// The saved freeze file, whatever network it names, or None when the home has none. A file that is not exactly
    /// this schema, or not a regular file, is an error (the node does not start on a setting it cannot read).
    pub fn saved_user_tx_freeze(&self) -> Result<Option<FreezeFile>, String> {
        let path = self.path.join(FREEZE_FILE);
        if fs::symlink_metadata(&path).is_err() { return Ok(None) }
        real_file(&path)?;
        let f: FreezeFile = serde_json::from_slice(&read_bounded(&path)?).map_err(|_| format!("invalid {FREEZE_FILE}"))?;
        if f.schema != FREEZE_SCHEMA { return Err(format!("invalid {FREEZE_FILE}")) }
        Ok(Some(f))
    }
    /// The saved freeze height S for the network this home is bound to now. A setting saved on another network
    /// (the old chain, after the home moved to the new one) is not applied: None, with the reason as the second value.
    pub fn user_tx_freeze(&self) -> Result<(Option<u64>, Option<String>), String> {
        let Some(f) = self.saved_user_tx_freeze()? else { return Ok((None, None)) };
        match self.bound_network()? {
            Some((chain_id, genesis)) if chain_id == f.chain_id && genesis.eq_ignore_ascii_case(&f.genesis_hash) => Ok((Some(f.freeze_user_txs_after), None)),
            bound => Ok((None, Some(format!("{FREEZE_FILE} was saved for chain {} genesis {}, and this home is bound to {}: not applied; `pocol freeze-user-txs --after off` removes it",
                f.chain_id, f.genesis_hash, bound.map_or("no network".into(), |(c, g)| format!("chain {c} genesis {g}")))))),
        }
    }
    /// The freeze `pocol start` uses: POCOL_FREEZE_USER_TXS_AFTER when it is set (a height, or `off`; an empty value counts
    /// as unset), as a variable wins for the other launcher settings, else the saved one for this home's network.
    /// The variable names no network: it applies to whatever network the home runs, so unset it after the re-genesis.
    pub fn effective_user_tx_freeze(&self) -> Result<(Option<u64>, Option<String>), String> {
        match std::env::var("POCOL_FREEZE_USER_TXS_AFTER").ok().filter(|v| !v.trim().is_empty()) {
            Some(v) => Ok((node::parse_freeze(&v).map_err(|_| "Invalid POCOL_FREEZE_USER_TXS_AFTER")?, None)),
            None => self.user_tx_freeze(),
        }
    }
    /// Saves (Some) or removes (None) the freeze setting, atomically (temporary file, rename, directory sync), for the
    /// network the home is bound to now (a home bound to none is refused). It takes effect at the next start.
    /// Returns the setting as JSON.
    pub fn set_user_tx_freeze(&self, after: Option<u64>) -> Result<String, String> {
        let path = self.path.join(FREEZE_FILE);
        let mut network = None;
        match after {
            Some(s) => {
                let (chain_id, genesis_hash) = self.bound_network()?.ok_or("this home is bound to no network yet: run `pocol start --network FILE` first, then set the freeze")?;
                network = Some(serde_json::json!({"chainId": chain_id, "genesisHash": genesis_hash}));
                let temp = self.path.join(format!("{FREEZE_FILE}.tmp"));
                if fs::symlink_metadata(&temp).is_ok() { real_file(&temp)?; fs::remove_file(&temp).map_err(|e| e.to_string())?; }
                let bytes = serde_json::to_vec_pretty(&FreezeFile { schema: FREEZE_SCHEMA.into(), freeze_user_txs_after: s, chain_id, genesis_hash }).map_err(|e| e.to_string())?;
                write_new(&temp, &bytes)?;
                fs::rename(&temp, &path).map_err(|e| e.to_string())?;
            }
            None => if fs::symlink_metadata(&path).is_ok() { real_file(&path)?; fs::remove_file(&path).map_err(|e| e.to_string())?; },
        }
        sync_directory(&self.path)?;
        Ok(serde_json::json!({"freezeUserTxsAfter": after, "network": network, "file": path, "appliesAt": "next start"}).to_string())
    }
    /// `pocol freeze-user-txs` without `--after`: the saved file, whether it applies to this home's network, and what
    /// the next start uses (POCOL_FREEZE_USER_TXS_AFTER included).
    pub fn describe_user_tx_freeze(&self) -> Result<String, String> {
        let saved = self.saved_user_tx_freeze()?;
        let (applied, note) = self.user_tx_freeze()?;
        let (effective, _) = self.effective_user_tx_freeze()?;
        Ok(serde_json::json!({"freezeUserTxsAfter": applied, "saved": saved.map(|f| serde_json::json!({"freezeUserTxsAfter": f.freeze_user_txs_after,
            "chainId": f.chain_id, "genesisHash": f.genesis_hash})), "notApplied": note, "effective": effective}).to_string())
    }
}

#[cfg(test)]
mod freeze_tests {
    use super::*;
    #[test]
    fn the_freeze_setting_round_trips_through_the_home_and_applies_only_to_its_network() {
        let dir = std::env::temp_dir().join(format!("pocol-freeze-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let home = Home::open(&dir).unwrap();
        assert_eq!(home.user_tx_freeze().unwrap(), (None, None), "no setting in a new home");
        assert!(home.set_user_tx_freeze(Some(5)).unwrap_err().contains("bound to no network"), "a home bound to no network is refused");
        assert!(!dir.join(FREEZE_FILE).exists());
        // Bound to the H15000 network (its saved invitation): the setting names that network.
        let saved = include_str!("../../../deploy/mainnet/h15000/network.pocol.json");
        fs::write(dir.join("network.pocol.json"), saved).unwrap();
        let (chain_id, genesis) = Invitation::parse_saved(saved.as_bytes()).unwrap().computed_network().unwrap();
        let out: serde_json::Value = serde_json::from_str(&home.set_user_tx_freeze(Some(1234)).unwrap()).unwrap();
        assert_eq!((&out["freezeUserTxsAfter"], &out["network"]), (&serde_json::json!(1234), &serde_json::json!({"chainId": chain_id, "genesisHash": genesis})));
        assert_eq!(home.user_tx_freeze().unwrap(), (Some(1234), None), "read back as saved");
        assert_eq!(home.saved_user_tx_freeze().unwrap().map(|f| (f.chain_id, f.genesis_hash)), Some((chain_id, genesis.clone())));
        assert!(!dir.join(format!("{FREEZE_FILE}.tmp")).exists(), "no temporary file is left");
        home.set_user_tx_freeze(Some(0)).unwrap();
        assert_eq!(home.user_tx_freeze().unwrap(), (Some(0), None), "replaced");
        // After the re-genesis the home keeps its key and is bound to the new network: a setting saved on the old one
        // is not applied (the new chain is not frozen at the old S), and says why.
        for (c, g) in [(chain_id, format!("0x{}", "11".repeat(32))), (chain_id + 1, genesis.clone())] {
            let other = FreezeFile { schema: FREEZE_SCHEMA.into(), freeze_user_txs_after: 7, chain_id: c, genesis_hash: g };
            fs::write(dir.join(FREEZE_FILE), serde_json::to_vec(&other).unwrap()).unwrap();
            let (applied, note) = home.user_tx_freeze().unwrap();
            assert_eq!(applied, None, "a setting of another network is not applied");
            assert!(note.as_deref().is_some_and(|n| n.contains("not applied") && n.contains("--after off")), "{note:?}");
            let shown: serde_json::Value = serde_json::from_str(&home.describe_user_tx_freeze().unwrap()).unwrap();
            assert_eq!((&shown["freezeUserTxsAfter"], &shown["saved"]["freezeUserTxsAfter"]), (&serde_json::Value::Null, &serde_json::json!(7)), "{shown}");
        }
        // The same network in upper case hex still applies.
        let upper = FreezeFile { schema: FREEZE_SCHEMA.into(), freeze_user_txs_after: 9, chain_id, genesis_hash: format!("0x{}", genesis[2..].to_ascii_uppercase()) };
        fs::write(dir.join(FREEZE_FILE), serde_json::to_vec(&upper).unwrap()).unwrap();
        assert_eq!(home.user_tx_freeze().unwrap(), (Some(9), None));
        // Unbound again (the old network files moved aside): not applied.
        fs::remove_file(dir.join("network.pocol.json")).unwrap();
        assert_eq!(home.user_tx_freeze().unwrap().0, None);
        fs::write(dir.join("network.pocol.json"), saved).unwrap();
        home.set_user_tx_freeze(None).unwrap();
        assert_eq!(home.user_tx_freeze().unwrap(), (None, None), "off removes it");
        home.set_user_tx_freeze(None).unwrap();
        // A file this release cannot read stops the start instead of running without the freeze.
        let g = &genesis;
        for bad in [String::from("{}"), format!(r#"{{"schema":"other","freezeUserTxsAfter":5,"chainId":{chain_id},"genesisHash":"{g}"}}"#),
            format!(r#"{{"schema":"pocol-freeze-user-txs-v1","freezeUserTxsAfter":-1,"chainId":{chain_id},"genesisHash":"{g}"}}"#),
            format!(r#"{{"schema":"pocol-freeze-user-txs-v1","freezeUserTxsAfter":5,"chainId":{chain_id},"genesisHash":"{g}","x":1}}"#),
            r#"{"schema":"pocol-freeze-user-txs-v1","freezeUserTxsAfter":5}"#.into()] {
            fs::write(dir.join(FREEZE_FILE), &bad).unwrap();
            assert!(home.user_tx_freeze().is_err(), "{bad}");
        }
        fs::remove_file(dir.join(FREEZE_FILE)).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(dir.join("elsewhere"), dir.join(FREEZE_FILE)).unwrap();
            assert!(home.user_tx_freeze().is_err(), "a symlink is refused");
            fs::remove_file(dir.join(FREEZE_FILE)).unwrap();
        }
        drop(home);
        let _ = fs::remove_dir_all(&dir);
    }
    #[test]
    fn the_freeze_value_is_a_height_or_off() {
        assert_eq!(node::parse_freeze("15000").unwrap(), Some(15000));
        assert_eq!(node::parse_freeze(" 0 ").unwrap(), Some(0));
        assert_eq!(node::parse_freeze("off").unwrap(), None);
        for bad in ["", "-1", "+5", "0x10", "1e3", "18446744073709551616", "on"] { assert!(node::parse_freeze(bad).is_err(), "{bad}"); }
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
            ]), bootstrap: vec![], seeds_retired: false }
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
        let ids: Vec<&str> = description["launchBlockers"].as_array().unwrap().iter().map(|b| b["id"].as_str().unwrap()).collect();
        assert_eq!(ids.last(), Some(&"genesis-unreadable"), "{ids:?}");
        assert_eq!(description["networkFingerprint"], fingerprint);
        assert_eq!(serde_json::to_vec(&invitation).unwrap(), bytes);
        assert_eq!(invitation.fingerprint(), fingerprint);
    }
    /// V3-28 H1: the designated PoCol Mainnet invitation is not launch ready, for computed reasons, and its
    /// designation clears none of them.
    #[test]
    fn the_designated_mainnet_reports_computed_launch_blockers() {
        let invitation = h15000();
        let description = invitation.describe();
        assert_eq!(description["networkName"], "PoCol Mainnet");
        assert_eq!(description["publicLaunchReady"], false);
        let blockers = description["launchBlockers"].as_array().unwrap();
        let ids: Vec<&str> = blockers.iter().map(|b| b["id"].as_str().unwrap()).collect();
        for id in ["spec-version", "plan-gate-Q1", "v3-inactive", "network-not-release-pinned", "profile-kind"] {
            assert!(ids.contains(&id), "{id}: {ids:?}");
        }
        assert!(!ids.contains(&"no-network") && !ids.contains(&"genesis-unreadable"), "{ids:?}");
        let info = invitation.info().unwrap();
        let (chain_id, genesis) = invitation.computed_network().unwrap();
        let pinned = blockers.iter().find(|b| b["id"] == "network-not-release-pinned").unwrap()["detail"].as_str().unwrap().to_string();
        assert!(pinned.contains(&chain_id.to_string()) && pinned.contains(&genesis), "{pinned}");
        // Exactly the computed list: build facts, then the network's (devnet.json's recorded admission).
        let forks = crate::configured_forks(&info).unwrap();
        assert_eq!(description["launchBlockers"], crate::readiness::json(&invitation.launch_blockers(&info, &forks))["launchBlockers"]);
        // A pinned v3 schedule entry would make the rules active; the schedule here has none.
        assert!(forks.iter().all(|&(v, _)| v < 3));
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
    /// Every file under `dir` with its bytes, except `launcher.lock`, which the open `Home` holds locked: a Windows
    /// byte-range lock is mandatory, so reading it through a second handle fails with ERROR_LOCK_VIOLATION (33). The
    /// launcher never writes to it, so its length stands in for its bytes.
    fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        let mut out = BTreeMap::new();
        for e in fs::read_dir(dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() { out.extend(snapshot(&p)); continue }
            let bytes = if p.file_name() == Some("launcher.lock".as_ref()) { fs::metadata(&p).unwrap().len().to_le_bytes().to_vec() } else { fs::read(&p).unwrap() };
            out.insert(p, bytes);
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
        let packed = Invitation::new_v2(h15000().seeds, vec![], false, h15000().files);
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
    /// D17, D23: the bootstrap list and the seeds' retirement are v2 fields outside the network's identity: an empty
    /// list and `false` are not written (a file without them keeps its bytes and fingerprint), the content key ignores
    /// them, and a list holds at most 16 distinct addresses with ports >= 1024, none a seed.
    #[test]
    fn the_bootstrap_list_is_validated_and_kept_out_of_the_content_key() {
        let plain = h15000_v2(serde_json::Value::Null);
        let b: Vec<SocketAddr> = (0..16).map(|i| SocketAddr::new([81, 2, 0, i + 1].into(), 30300)).collect();
        let text = String::from_utf8(bytes(&plain)).unwrap();assert!(!text.contains("bootstrap") && !text.contains("seedsRetired"), "absent when empty");
        let listed = Invitation { bootstrap: b.clone(), seeds_retired: true, ..plain.clone() };
        let again = Invitation::parse_saved(&bytes(&listed)).unwrap();
        assert_eq!((again.bootstrap(), again.seeds_retired()), (&b[..], true));
        assert_eq!(listed.content_key(), plain.content_key(), "the same network");
        assert_ne!(listed.fingerprint(), plain.fingerprint(), "another file");
        assert_eq!(listed.describe()["bootstrap"].as_array().map(Vec::len), Some(16));
        let bad = |i: Invitation| Invitation::parse_saved(&bytes(&i)).unwrap_err();
        let mut more = b.clone(); more.push("81.3.0.1:30300".parse().unwrap());
        for (why, i) in [("17", Invitation { bootstrap: more, ..plain.clone() }), ("twice", Invitation { bootstrap: vec![b[0], b[0]], ..plain.clone() }),
            ("port 80", Invitation { bootstrap: vec!["81.2.0.1:80".parse().unwrap()], ..plain.clone() }),
            ("a seed", Invitation { bootstrap: vec![plain.seeds[0]], ..plain.clone() })] {
            assert!(bad(i).contains("at most 16 distinct bootstrap addresses"), "{why}");
        }
        for v1 in [Invitation { bootstrap: vec![b[0]], ..h15000() }, Invitation { seeds_retired: true, ..h15000() }] {
            assert!(bad(v1).contains("need a pocol-network-v2 invitation"));
        }
    }
    /// D17, D23 (A7): an upgrade may change only the bootstrap list or retire the seeds: a rebind (no activation), which
    /// `inspect-upgrade` shows with both lists, both flags and both fingerprints.
    #[test]
    fn an_upgrade_may_change_only_the_bootstrap_list_or_retire_the_seeds() {
        let t = Scratch::new();
        let before = h15000_v2(serde_json::Value::Null);
        let home = home_with(&t, &bytes(&before));
        let b: SocketAddr = "81.2.0.9:30300".parse().unwrap();
        let after = Invitation { bootstrap: vec![b], ..before.clone() };
        let shown = home.inspect_upgrade(&bytes(&after)).unwrap();
        assert_eq!((shown["change"].as_str(), shown["bootstrapBefore"].clone(), shown["bootstrapAfter"].clone()), (Some("rebind"), serde_json::json!([]), serde_json::json!([b])));
        assert_eq!((shown["fingerprintBefore"].as_str(), shown["fingerprintAfter"].as_str()), (Some(before.fingerprint().as_str()), Some(after.fingerprint().as_str())));
        home.upgrade(&bytes(&after)).unwrap();
        let retired = Invitation { seeds_retired: true, ..after.clone() };
        let shown = home.inspect_upgrade(&bytes(&retired)).unwrap();
        assert_eq!((shown["change"].as_str(), shown["seedsRetiredBefore"].as_bool(), shown["seedsRetiredAfter"].as_bool()), (Some("rebind"), Some(false), Some(true)));
        home.upgrade(&bytes(&retired)).unwrap();
        assert_eq!(fs::read(home.path.join("network.pocol.json")).unwrap(), bytes(&retired));
    }
    /// D23, D27 (B12): with retired seeds a home dials none of them; otherwise its own seed is dropped and makes it a
    /// seed host, and a seed also given with `--peer` stays a static peer.
    #[test]
    fn retired_seeds_are_not_dialed() {
        let seeds: Vec<SocketAddr> = ["127.0.0.1:30311", "81.2.0.1:30300", "81.2.0.2:30300"].iter().map(|s| s.parse().unwrap()).collect();
        let i = Invitation { seeds: seeds.clone(), ..h15000_v2(serde_json::Value::Null) };
        let p: SocketAddr = "81.9.0.1:30300".parse().unwrap();
        assert_eq!(i.launch_peers(Some(seeds[0]), &[p, seeds[2]]), (vec![p, seeds[2], seeds[1]], vec![seeds[1]], true), "own seed dropped, a static one stays static");
        assert_eq!(i.launch_peers(None, &[]), (seeds.clone(), seeds.clone(), false));
        let r = Invitation { seeds_retired: true, ..i };
        assert_eq!(r.launch_peers(Some(seeds[0]), &[p]), (vec![p], vec![], false), "retired: no seed at all");
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
    fn the_dry_run_of_an_interrupted_upgrade_offers_to_resume_it_with_the_same_file() {
        // The wallets run the dry run before every upgrade, so refusing the interrupted upgrade's own file left them no
        // way to resume it (and an Android home no way out but clearing the key). Both crash windows: before and after
        // the binding is written.
        let (before, after) = (h15000_v2(serde_json::json!([[1, 0]])), h15000_v2(serde_json::json!([[1, 0], [2, 15000]])));
        for bound in [&before, &after] {
            let t = Scratch::new();
            let home = home_with(&t, &bytes(&before));
            // What `upgrade` leaves at network-upgrade-metadata (or -binding): the backup, the intent and the new devnet.json.
            fs::write(home.path.join(backup_name(&before)), serde_json::to_vec_pretty(&before).unwrap()).unwrap();
            fs::write(home.path.join(UPGRADE_PENDING), serde_json::to_vec(&UpgradeIntent { before: before.clone(), after: after.clone() }).unwrap()).unwrap();
            fs::write(home.path.join("network").join("devnet.json"), &after.files["devnet.json"]).unwrap();
            fs::write(home.path.join("network.pocol.json"), bytes(bound)).unwrap();
            let interrupted = snapshot(&home.path);
            let dry = home.inspect_upgrade(&bytes(&after)).unwrap();
            assert_eq!((dry["change"].as_str(), &dry["appended"], &dry["resuming"]), (Some("activation"), &serde_json::json!([[2, 15000]]), &serde_json::json!(true)), "{dry}");
            let other = h15000_v2(serde_json::json!([[1, 0], [2, 15001]]));
            assert_eq!(home.inspect_upgrade(&bytes(&other)).unwrap_err(), "resume the interrupted upgrade with the same network invitation");
            assert_eq!(home.upgrade(&bytes(&other)).unwrap_err(), "resume the interrupted upgrade with the same network invitation");
            assert_eq!(snapshot(&home.path), interrupted, "neither the dry run nor a refused file writes");
            home.upgrade(&bytes(&after)).unwrap();
            assert!(!home.path.join(UPGRADE_PENDING).exists());
            assert_eq!(fs::read(home.path.join("network.pocol.json")).unwrap(), bytes(&after));
            let done = home.inspect_upgrade(&bytes(&after)).unwrap();
            assert_eq!((done["change"].as_str(), &done["resuming"]), (Some("none"), &serde_json::json!(false)));
        }
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

#[cfg(test)]
pub(crate) mod key_backup_tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    const PASS: &[u8] = b"a long backup passphrase";
    /// A new home in its own scratch directory; the directory goes when the guard drops.
    pub(crate) struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            let p = std::env::temp_dir().join(format!("pocol-key-backup-{}-{}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
            let _ = fs::remove_dir_all(&p);
            Self(p)
        }
        fn home(&self, name: &str) -> Home { Home::open(&self.0.join(name)).unwrap() }
    }
    impl Drop for Scratch { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
    fn names(home: &Home) -> Vec<String> {
        let mut v: Vec<String> = fs::read_dir(&home.path).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
        v.sort();
        v
    }

    #[test]
    fn export_then_restore_into_a_new_home_gives_the_same_account() {
        let t = Scratch::new();
        let old = t.home("old");
        let address = old.address().unwrap();
        let (file, exported) = old.export_key_backup(PASS).unwrap();
        assert_eq!((file.len(), exported.as_str()), (backup::LEN, address.as_str()));
        assert!(!file.windows(32).any(|w| w == old.key().unwrap().secret()), "no plaintext key in the backup");
        assert!(old.export_key_backup(b"too short").is_err(), "the export passphrase policy applies");
        let new = t.home("new");
        let preview = new.inspect_key_backup(&file, PASS).unwrap();
        assert_eq!(preview, serde_json::json!({"address": address, "current": null, "currentKeyReadable": null, "replaces": false}));
        assert!(names(&new).iter().all(|n| n == "launcher.lock"), "inspecting writes nothing: {:?}", names(&new));
        assert_eq!(new.import_key_backup(&file, PASS, None).unwrap()["changed"], true);
        assert_eq!(new.address().unwrap(), address);
        assert_eq!(fs::read(new.path.join(ADDRESS_FILE)).unwrap(), address.as_bytes());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(fs::metadata(new.path.join(KEY_FILE)).unwrap().permissions().mode() & 0o777, 0o600);
        }
        assert!(new.import_key_backup(&file, b"a long backup passphrasE", None).unwrap_err().contains("Wrong passphrase"));
        assert_eq!(new.import_key_backup(&file, PASS, None).unwrap()["changed"], false, "restoring the same account again is a no-op");
    }
    #[test]
    fn restoring_another_account_needs_the_confirmed_current_address_and_keeps_the_old_key() {
        let t = Scratch::new();
        let source = t.home("source");
        let (file, restored) = source.export_key_backup(PASS).unwrap();
        let target = t.home("target");
        let current = target.address().unwrap();
        let old_key = fs::read(target.path.join(KEY_FILE)).unwrap();
        fs::write(target.path.join("wallet-activity.json"), b"[]").unwrap();
        let preview = target.inspect_key_backup(&file, PASS).unwrap();
        assert_eq!((&preview["current"], &preview["replaces"]), (&serde_json::json!(current), &serde_json::json!(true)));
        let before = names(&target);
        for replace in [None, Some("0x0000000000000000000000000000000000000000"), Some(restored.as_str())] {
            let e = target.import_key_backup(&file, PASS, replace).unwrap_err();
            assert!(e.contains("Confirm replacing it"), "{e}");
            assert_eq!(names(&target), before, "a refused restore changes nothing");
        }
        let done = target.import_key_backup(&file, PASS, Some(&current.to_uppercase().replace("0X", "0x"))).unwrap();
        assert_eq!((&done["changed"], &done["previous"]), (&serde_json::json!(true), &serde_json::json!(current)));
        assert_eq!(target.address().unwrap(), restored);
        let kept = format!("{KEY_FILE}.replaced-{current}");
        assert_eq!(done["previousKeyFile"], kept.as_str());
        assert_eq!(fs::read(target.path.join(&kept)).unwrap(), old_key, "the replaced key is kept, byte for byte");
        assert!(target.path.join(format!("wallet-activity.replaced-{current}.json")).is_file());
        assert!(!target.path.join("wallet-activity.json").exists());
        assert!(!target.path.join(format!("{KEY_FILE}.restoring")).exists());
        // Back again: the second replaced file gets its own name, nothing is overwritten.
        let (back, _) = { let s = t.home("again"); fs::copy(target.path.join(&kept), s.path.join(KEY_FILE)).unwrap();
            #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; fs::set_permissions(s.path.join(KEY_FILE), fs::Permissions::from_mode(0o600)).unwrap(); }
            s.export_key_backup(PASS).unwrap() };
        target.import_key_backup(&back, PASS, Some(&restored)).unwrap();
        assert_eq!(target.address().unwrap(), current);
        target.import_key_backup(&file, PASS, Some(&current)).unwrap();
        assert!(target.path.join(format!("{kept}.1")).is_file());
    }
    #[test]
    fn a_lost_key_is_restored_only_from_its_own_backup() {
        let t = Scratch::new();
        let home = t.home("home");
        let address = home.address().unwrap();
        let (file, _) = home.export_key_backup(PASS).unwrap();
        let (other, _) = t.home("other").export_key_backup(PASS).unwrap();
        fs::remove_file(home.path.join(KEY_FILE)).unwrap();
        fs::create_dir(home.path.join("state")).unwrap();
        assert!(home.key().err().unwrap().contains("restore its backup"));
        let preview = home.inspect_key_backup(&other, PASS).unwrap();
        assert_eq!((&preview["current"], &preview["currentKeyReadable"], &preview["replaces"]), (&serde_json::json!(address), &serde_json::json!(false), &serde_json::json!(true)));
        assert!(home.import_key_backup(&other, PASS, None).is_err());
        assert_eq!(home.import_key_backup(&file, PASS, None).unwrap()["changed"], true);
        assert_eq!(home.address().unwrap(), address);
    }
    #[test]
    fn an_unreadable_key_file_is_moved_aside_by_its_own_backup() {
        let t = Scratch::new();
        let home = t.home("home");
        let address = home.address().unwrap();
        let (file, _) = home.export_key_backup(PASS).unwrap();
        fs::write(home.path.join(KEY_FILE), b"damaged").unwrap();
        assert!(home.key().is_err());
        assert_eq!(home.inspect_key_backup(&file, PASS).unwrap()["replaces"], false);
        let done = home.import_key_backup(&file, PASS, None).unwrap();
        assert_eq!(done["changed"], true);
        assert_eq!(home.address().unwrap(), address);
        assert_eq!(fs::read(home.path.join(format!("{KEY_FILE}.replaced-{address}"))).unwrap(), b"damaged");
    }
    #[test]
    fn read_only_commands_never_create_a_home() {
        let t = Scratch::new();
        let missing = t.0.join("typo");
        let e = Home::open_existing(&missing, "inspect-key-backup").err().unwrap();
        assert_eq!(e, "inspect-key-backup requires an existing PoCol home; it never creates a wallet");
        assert!(!missing.exists() && !t.0.exists(), "no directory and no lock file");
        let home = t.home("home");
        drop(home);
        assert!(Home::open_existing(&t.0.join("home"), "inspect-key-backup").is_ok());
    }
    /// The old account's wallet activity in [`crash_restore`].
    pub(crate) const OLD_ACTIVITY: &[u8] = br#"[{"hash":"0x01","kind":"transfer"}]"#;
    /// A restore of another account that stops (a crash) right before `step`, then the home opened again. With
    /// `lose_key` the current key file is gone first (only its marker names the account).
    pub(crate) fn crash_restore(step: u8, lose_key: bool) -> (String, String, Home, Scratch) {
        let t = Scratch::new();
        let (file, restored) = t.home("source").export_key_backup(PASS).unwrap();
        let target = t.home("target");
        let current = target.address().unwrap();
        fs::write(target.path.join(ACTIVITY_FILE), OLD_ACTIVITY).unwrap();
        if lose_key {
            fs::remove_file(target.path.join(KEY_FILE)).unwrap();
        }
        CRASH_BEFORE.with(|c| c.set(step));
        let e = target.import_key_backup(&file, PASS, Some(&current)).unwrap_err();
        CRASH_BEFORE.with(|c| c.set(0));
        assert!(e.contains("simulated crash"), "{e}");
        let path = target.path.clone();
        drop(target);
        (current, restored, Home::open(&path).unwrap(), t)
    }
    #[test]
    fn a_restore_interrupted_after_the_swap_is_finished_on_the_next_open() {
        // Before the key moves aside nothing changed (a lost key has nothing to move); from then on the next
        // key() completes the restore. Before this was recorded, a crash between the key swap and the marker left the
        // wallet unable to start. The old account's activity moves aside too, whichever step the crash interrupted.
        for (step, lose_key) in (1..=4u8).flat_map(|s| [(s, false), (s, true)]) {
            let (current, restored, home, _t) = crash_restore(step, lose_key);
            let finished = step > 1 || lose_key;
            let expected = if finished { &restored } else { &current };
            let case = format!("crash before step {step}, lost key {lose_key}");
            assert_eq!(&home.address().unwrap(), expected, "{case}");
            assert_eq!(fs::read(home.path.join(ADDRESS_FILE)).unwrap(), expected.as_bytes(), "{case}");
            let moved = home.path.join(format!("wallet-activity.replaced-{current}.json"));
            let (kept, aside) = if finished { (&moved, home.path.join(ACTIVITY_FILE)) } else { (&home.path.join(ACTIVITY_FILE), moved.clone()) };
            assert_eq!(fs::read(kept).unwrap(), OLD_ACTIVITY, "{case}");
            assert!(!aside.exists(), "{case}");
            if finished {
                assert!(!home.path.join(RESTORE_RECORD).exists() && !home.path.join(STAGED_KEY).exists(), "{case}");
                assert_eq!(home.path.join(format!("{KEY_FILE}.replaced-{current}")).is_file(), !lose_key, "the old key is kept");
            }
        }
    }
    #[test]
    fn a_restore_interrupted_before_the_swap_is_repeated_cleanly() {
        let (current, restored, home, t) = crash_restore(1, false);
        assert_eq!(home.address().unwrap(), current);
        // The staged key and the record of the interrupted attempt are replaced by the next restore.
        let (file, _) = {
            let source = Home::open(&t.0.join("source")).unwrap();
            source.export_key_backup(PASS).unwrap()
        };
        let done = home.import_key_backup(&file, PASS, Some(&current)).unwrap();
        assert_eq!(done["address"], restored.as_str());
        assert_eq!(home.address().unwrap(), restored);
        assert!(!home.path.join(RESTORE_RECORD).exists() && !home.path.join(STAGED_KEY).exists());
    }
    #[test]
    fn a_running_wallet_holds_the_lock_so_no_backup_runs_beside_it() {
        let t = Scratch::new();
        let home = t.home("home");
        home.address().unwrap();
        assert!(Home::open(&home.path).is_err(), "export and import open the home, which a running wallet holds");
    }
}
