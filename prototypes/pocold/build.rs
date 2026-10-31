//! Conservative testnet compatibility identity. No Git metadata, timestamps or secrets.
use sha2::{Digest, Sha256};
use std::{
    env, fs,
    path::{Path, PathBuf},
};

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
        root.join("lp4-execution/tests/data/website.json"),
    ];
    for krate in ["pocold", "lp1-node", "lp4-execution"] {
        files.push(root.join(krate).join("Cargo.toml"));
        sources(&root.join(krate).join("src"), &mut files);
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
    let info = serde_json::json!({"schema":"pocol-testnet-build-v1", "protocolVersion":2,
        "compatibilitySha256":digest,"features":features,"sourceSha256":manifest,
        "scope":"experimental-testnet","publicLaunchReady":false});
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("build-info.json"),
        serde_json::to_vec(&info).unwrap(),
    )
    .unwrap();
}
