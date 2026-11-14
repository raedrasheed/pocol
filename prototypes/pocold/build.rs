//! Conservative testnet compatibility identity (informational build digest) and the consensus boundary check
//! (plan §1.5): the consensus identity of a tree whose boundary sources differ from consensus-sources.lock is
//! unpinned, and a POCOL_RELEASE=1 build of such a tree fails. No Git metadata, timestamps or secrets.
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

#[allow(dead_code)]
mod boundary {
    include!("consensus_boundary.rs");
}

/// The constants build-info reports, read from the sources that define them (the guard checks they agree).
fn constant(root: &Path, file: &str, name: &str) -> u64 {
    let src = fs::read_to_string(root.join(file)).expect("constant source");
    boundary::const_u64(&src, name).unwrap_or_else(|| panic!("{file} must define const {name}"))
}

/// Computes the boundary aggregate, emits the pinned/unpinned environment and returns the build-info fields.
fn consensus(root: &Path) -> serde_json::Value {
    println!("cargo:rerun-if-env-changed=POCOL_RELEASE");
    for f in [boundary::BOUNDARY_FILE, boundary::LOCK_FILE, boundary::CARGO_LOCK] {
        println!("cargo:rerun-if-changed={}", root.join(f).display());
    }
    // A tree whose boundary cannot be computed (missing file, symlink, unresolvable lock) does not build.
    let tree = boundary::compute(root).unwrap_or_else(|e| panic!("consensus boundary: {e}"));
    for w in &tree.watched {
        println!("cargo:rerun-if-changed={}", root.join(w).display());
    }
    let k = boundary::Constants {
        rules: constant(root, "pocold/src/identity.rs", "CONSENSUS_RULES_VERSION"),
        max_fork: constant(root, "lp4-execution/src/p1_profile.rs", "MAX_FORK_VERSION"),
        spec: constant(root, "lp1-node/src/genesis_spec.rs", "SPEC_VERSION"),
        protocol: constant(root, "pocold/src/identity.rs", "PROTOCOL_VERSION"),
        protocol_min: constant(root, "pocold/src/identity.rs", "PROTOCOL_VERSION_MIN"),
    };
    let lock = fs::read_to_string(root.join(boundary::LOCK_FILE))
        .map_err(|e| e.to_string())
        .and_then(|t| boundary::parse_lock(&t));
    // Pinned means the tree's aggregate is the one the last history line recorded (A1-2): anything else
    // builds, but with an unpinned identity that peers only with byte-identical trees.
    let pinned = lock
        .as_ref()
        .ok()
        .and_then(|l| l.history.last())
        .is_some_and(|h| h.sources == tree.sources);
    if env::var("POCOL_RELEASE").as_deref() == Ok("1") {
        // A release must also pass the whole guard (history, constants, classification), not only the aggregate.
        let verdict = lock.and_then(|l| boundary::check(&l, &tree, &k));
        if let Err(why) = verdict {
            panic!("POCOL_RELEASE=1 refuses an unpinned consensus tree:\n{why}");
        }
    }
    println!("cargo:rustc-env=POCOL_CONSENSUS_PINNED={}", pinned as u8);
    println!("cargo:rustc-env=POCOL_CONSENSUS_SOURCES_ACTUAL={}", tree.sources);
    // Always set, so a value in the caller's environment can never reach option_env!/env!.
    println!(
        "cargo:rustc-env=POCOL_CONSENSUS_SOURCES={}",
        if pinned { "" } else { &tree.sources }
    );
    serde_json::json!({"consensusPinned":pinned,"consensusSourcesSha256":tree.sources,
        "consensusRulesVersion":k.rules,"protocolVersionMin":k.protocol_min,"maxForkVersion":k.max_fork})
}

fn sources(dir: &Path, out: &mut Vec<PathBuf>) {
    println!("cargo:rerun-if-changed={}", dir.display());
    for entry in fs::read_dir(dir).expect("source directory") {
        let entry = entry.expect("source entry");
        let ty = entry.file_type().expect("source type");
        assert!(!ty.is_symlink(), "source symlinks are not supported");
        if ty.is_dir() {
            sources(&entry.path(), out);
        } else if entry.path().extension().is_some_and(|x| x == "rs") {
            out.push(entry.path());
        }
    }
}
fn main() {
    let here = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let root = here.parent().unwrap();
    let mut files = vec![
        here.join("build.rs"),
        here.join("Cargo.lock"),
        root.join("lp4-execution/tests/data/system-v1.json"),
        root.join("lp4-execution/tests/data/system-continuous.json"),
        root.join("lp4-execution/tests/data/system-earnin.json"),
        // Informational build digest only: the consensus identity never includes it (identity.rs).
        root.join("lp4-execution/tests/data/system-v3.json"),
        root.join("lp4-execution/tests/data/website.json"),
    ];
    for krate in ["pocold", "lp1-node", "lp4-execution"] {
        files.push(root.join(krate).join("Cargo.toml"));
        sources(&root.join(krate).join("src"), &mut files);
    }
    // Path patches have no registry checksum in Cargo.lock. Bind their actual
    // manifests and Rust sources into the same release identity as the node.
    // Keep names relative to prototypes so existing safe-path guards apply.
    for krate in [
        "alloy-primitives-0.8.26",
        "alloy-eip7702-0.5.1",
        "revm-precompile-16.2.0",
    ] {
        let directory = root.join("third-party/evm-crypto").join(krate);
        files.push(directory.join("Cargo.toml"));
        files.push(directory.join("README.md"));
        sources(&directory.join("src"), &mut files);
    }
    // Sort the exact normalized relative names that enter the digest. Absolute host paths
    // can use different separator spellings on Windows, even within one Cargo build.
    let mut files: Vec<(String, PathBuf)> = files
        .into_iter()
        .map(|path| {
            let name = path
                .strip_prefix(root)
                .unwrap()
                .to_str()
                .unwrap()
                .replace('\\', "/");
            (name, path)
        })
        .collect();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    let mut hash = Sha256::new();
    hash.update(b"pocol-testnet-source-compatibility-v1\0");
    let mut manifest = serde_json::Map::new();
    for (name, path) in files {
        println!("cargo:rerun-if-changed={}", path.display());
        assert!(
            !manifest.contains_key(&name),
            "duplicate normalized source path"
        );
        // Text files are hashed as bytes: Git must not rewrite line endings on release builds.
        let bytes = fs::read(path).expect("source bytes");
        hash.update((name.len() as u64).to_be_bytes());
        hash.update(name.as_bytes());
        hash.update((bytes.len() as u64).to_be_bytes());
        hash.update(&bytes);
        manifest.insert(
            name,
            serde_json::json!(format!("{:x}", Sha256::digest(&bytes))),
        );
    }
    let mut features = Vec::new();
    for (var, name) in [
        ("CARGO_FEATURE_CAPS_TEST", "caps-test"),
        ("CARGO_FEATURE_CRASH_TEST", "crash-test"),
        ("CARGO_FEATURE_IO_FAULT", "io-fault"),
    ] {
        println!("cargo:rerun-if-env-changed={var}");
        if env::var_os(var).is_some() {
            features.push(name);
        }
    }
    let flags = features.join(",");
    hash.update(flags.as_bytes());
    let digest = format!("{:x}", hash.finalize());
    println!("cargo:rustc-env=POCOL_COMPATIBILITY_SHA256={digest}");
    let mut info = serde_json::json!({"schema":"pocol-testnet-build-v1", "protocolVersion":2,
        "compatibilitySha256":digest,"features":features,"sourceSha256":manifest,
        "scope":"experimental-testnet"});
    // publicLaunchReady and launchBlockers are computed at run time from this build's facts (pocold::readiness).
    for (key, value) in consensus(root).as_object().unwrap() {
        info[key] = value.clone();
    }
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("build-info.json"),
        serde_json::to_vec(&info).unwrap(),
    )
    .unwrap();
}
