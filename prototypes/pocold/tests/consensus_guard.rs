//! The consensus guard (plan §1.5, VA-SPEC §2.4): the boundary sources equal consensus-sources.lock, its history is
//! consistent with the code's constants, every module is classified and no `+` file reaches a node-local module
//! unannounced. Re-pin mode (POCOL_REPIN_CONSENSUS=<kind> POCOL_REPIN_REF=<PR> POCOL_REPIN_NOTE=<why>
//! [POCOL_REPIN_GATE=fork:V]) rewrites the lock with one appended history line instead of checking it.
#[path = "../consensus_boundary.rs"]
#[allow(dead_code)]
mod boundary;
use boundary::*;
use std::path::{Path, PathBuf};

fn root() -> PathBuf { Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf() }
fn constants() -> Constants {
    Constants { rules: pocold::identity::CONSENSUS_RULES_VERSION as u64, max_fork: pocol_lp4_execution::profile::MAX_FORK_VERSION as u64,
        spec: pocold::genesis_spec::SPEC_VERSION as u64, protocol: pocold::identity::PROTOCOL_VERSION, protocol_min: pocold::identity::PROTOCOL_VERSION_MIN }
}

#[test]
fn the_tree_matches_the_consensus_lock() {
    let root = root();
    let tree = compute(&root).unwrap();
    let text = std::fs::read_to_string(root.join(LOCK_FILE));
    if let Ok(kind) = std::env::var("POCOL_REPIN_CONSENSUS") {
        let lock = text.as_ref().ok().map(|t| parse_lock(t).unwrap());
        let var = |n: &str| std::env::var(n).unwrap_or_default();
        let out = repin(lock.as_ref(), &tree, &constants(), &kind, &var("POCOL_REPIN_REF"), &var("POCOL_REPIN_NOTE"), std::env::var("POCOL_REPIN_GATE").ok().as_deref())
            .unwrap_or_else(|e| panic!("re-pin refused: {e}"));
        std::fs::write(root.join(LOCK_FILE), out).unwrap();
        eprintln!("re-pinned {LOCK_FILE} as kind={kind}: sources {}", tree.sources);
        return;
    }
    let lock = parse_lock(&text.expect("consensus-sources.lock")).unwrap();
    if let Err(why) = check(&lock, &tree, &constants()) { panic!("{why}") }
    // The aggregate build.rs saw is the one the guard computes, so the build's identity is this tree's.
    assert_eq!(env!("POCOL_CONSENSUS_SOURCES_ACTUAL"), tree.sources, "stale build: build.rs saw another tree");
    assert_eq!(env!("POCOL_CONSENSUS_PINNED"), "1");
    assert_eq!(env!("POCOL_CONSENSUS_SOURCES"), "");
}

#[test]
fn build_info_reports_the_consensus_constants_and_the_pinned_aggregate() {
    let info: serde_json::Value = serde_json::from_str(pocold::identity::BUILD_INFO).unwrap();
    let k = constants();
    assert_eq!(info["consensusRulesVersion"], k.rules);
    assert_eq!(info["maxForkVersion"], k.max_fork);
    assert_eq!(info["protocolVersionMin"], pocold::identity::PROTOCOL_VERSION_MIN);
    assert_eq!(info["protocolVersion"], pocold::identity::PROTOCOL_VERSION);
    assert_eq!(info["consensusSourcesSha256"], env!("POCOL_CONSENSUS_SOURCES_ACTUAL"));
    assert_eq!(info["consensusPinned"], env!("POCOL_CONSENSUS_PINNED") == "1");
}

#[test]
fn caps_test_is_taken_from_the_crate_that_changes_the_constant() {
    // The compile-time assertion in pocold/src/lib.rs refuses lp4's caps-test without pocold's feature.
    assert_eq!(pocol_lp4_execution::CAPS_TEST, cfg!(feature = "caps-test"));
    assert_eq!(pocol_lp4_execution::user_phase::caps::LOG_CAP == 1 << 24, pocol_lp4_execution::CAPS_TEST);
}

// ---- T7: check() on fixture trees and locks ----

/// A scratch directory under the target's tmp, removed and recreated.
fn scratch(name: &str) -> PathBuf {
    let d = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("consensus-guard-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}
fn put(root: &Path, path: &str, text: &str) {
    let p = root.join(path);
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}
const FIXTURE_LOCK: &str = "version = 4\n\n[[package]]\nname = \"k256\"\nversion = \"0.14.0\"\nsource = \"registry+x\"\nchecksum = \"aa\"\ndependencies = [\n \"sha2\",\n]\n\n\
[[package]]\nname = \"sha2\"\nversion = \"0.10.9\"\nsource = \"registry+x\"\nchecksum = \"bb\"\n\n[[package]]\nname = \"redb\"\nversion = \"2.6.4\"\nsource = \"registry+x\"\nchecksum = \"cc\"\n";
const FIXTURE_BOUNDARY: &str = "+lp4-execution/src/\n-lp4-execution/src/pool.rs\n-*tests.rs\n+lp4-execution/tests/data/\nvector lp4-execution/tests/data/\n\
+pocold/src/{lib,chain}.rs\n+pocold/rust-toolchain.toml\nroot k256\n=pocold::net\n";
const K: Constants = Constants { rules: 1, max_fork: 2, spec: 1, protocol: 2, protocol_min: 2 };
/// A miniature two-crate tree with the real layout: pocold (lib, chain +, net =) and lp4 (fees +, pool -, tests).
fn fixture(name: &str) -> PathBuf {
    let r = scratch(name);
    put(&r, BOUNDARY_FILE, FIXTURE_BOUNDARY);
    put(&r, CARGO_LOCK, FIXTURE_LOCK);
    put(&r, "pocold/rust-toolchain.toml", "[toolchain]\nchannel = \"1.99.1\"\n");
    put(&r, "pocold/src/lib.rs", "pub mod chain;\npub mod net;\n");
    put(&r, "pocold/src/chain.rs", "use crate::lib_item; use pocol_lp4_execution::fees;\n");
    put(&r, "pocold/src/net.rs", "// node-local\n");
    put(&r, "lp4-execution/src/lib.rs", "pub mod fees;\npub mod pool;\n#[cfg(test)] mod fee_tests;\n");
    put(&r, "lp4-execution/src/fees.rs", "pub fn base_fee() -> u64 { 7 }\n");
    put(&r, "lp4-execution/src/pool.rs", "use crate::fees;\n");
    put(&r, "lp4-execution/src/fee_tests.rs", "#[test] fn t() {}\n");
    put(&r, "lp4-execution/tests/data/v.json", "[1]\n");
    put(&r, WIRE_FILE, WIRE_V2);
    r
}
const WIRE_V2: &str = "[{\"key\":\"v2/hello\",\"v\":2,\"hex\":\"c180\"},{\"key\":\"v2/get-addr\",\"v\":2,\"hex\":\"c108\"}]";
fn tree(r: &Path) -> Tree { compute(r).unwrap() }
fn base(t: &Tree) -> Lock { parse_lock(&repin(None, t, &K, "base", "PR5", "fixture", None).unwrap()).unwrap() }
fn refused(r: Result<(), String>, needle: &str) {
    let e = r.expect_err("the guard must refuse");
    assert!(e.contains(needle), "missing {needle:?} in:\n{e}");
    assert!(e.contains(INSTRUCTIONS), "the failure carries the instruction text:\n{e}");
}
fn append(r: &Path, path: &str, extra: &str) { let mut s = std::fs::read_to_string(r.join(path)).unwrap(); s += extra; put(r, path, &s) }

#[test]
fn a_pinned_fixture_passes_and_a_changed_file_without_a_history_line_fails() {
    let r = fixture("changed");
    let t = tree(&r);
    let lock = base(&t);
    check(&lock, &t, &K).unwrap();
    assert!(t.files.contains_key("lp4-execution/src/fees.rs") && !t.files.contains_key("lp4-execution/src/pool.rs"));
    assert!(!t.files.contains_key("lp4-execution/src/fee_tests.rs"), "a #[cfg(test)] tests file is excluded");
    append(&r, "lp4-execution/src/fees.rs", " ");
    refused(check(&lock, &tree(&r), &K), "changed: lp4-execution/src/fees.rs");
}

#[test]
fn neutral_cannot_change_rules_and_bump_needs_the_constant() {
    let r = fixture("rules");
    let lock = base(&tree(&r));
    append(&r, "lp4-execution/src/fees.rs", " ");
    let t = tree(&r);
    let mut forged = lock.clone();
    let mut line = forged.history[0].clone();
    line.n = 2; line.kind = "neutral".into(); line.rules = 2; line.sources = t.sources.clone(); line.note = "x".into();
    forged.history.push(line);
    forged.files = t.files.clone();
    refused(check(&forged, &t, &K), "kind=neutral must keep rules=1");
    // bump without incrementing CONSENSUS_RULES_VERSION: refused by re-pin and by the guard.
    assert!(repin(Some(&lock), &t, &K, "bump", "PR9", "new genesis", None).unwrap_err().contains("CONSENSUS_RULES_VERSION = 2"));
    forged.history[1].kind = "bump".into();
    refused(check(&forged, &t, &K), "the code has CONSENSUS_RULES_VERSION=1");
    let bumped = Constants { rules: 2, ..K };
    check(&parse_lock(&repin(Some(&lock), &t, &bumped, "bump", "PR9", "new genesis", None).unwrap()).unwrap(), &t, &bumped).unwrap();
}

#[test]
fn neutral_is_refused_over_a_changed_vector_or_dependency() {
    let r = fixture("labels");
    let lock = base(&tree(&r));
    append(&r, "lp4-execution/tests/data/v.json", " ");
    let t = tree(&r);
    assert!(repin(Some(&lock), &t, &K, "neutral", "PR9", "x", None).unwrap_err().contains("use neutral-vector"));
    let mut forged = parse_lock(&repin(Some(&lock), &t, &K, "neutral-vector", "PR9", "vector regenerated", None).unwrap()).unwrap();
    forged.history[1].kind = "neutral".into();
    refused(check(&forged, &t, &K), "a vector file changed");
    // A closure package (sha2 under k256) changed; redb is outside the closure and needs nothing.
    let r = fixture("deps");
    let lock = base(&tree(&r));
    put(&r, CARGO_LOCK, &FIXTURE_LOCK.replace("version = \"2.6.4\"", "version = \"2.6.5\""));
    check(&lock, &tree(&r), &K).unwrap();
    put(&r, CARGO_LOCK, &FIXTURE_LOCK.replace("checksum = \"bb\"", "checksum = \"bd\""));
    let t = tree(&r);
    refused(check(&lock, &t, &K), "dep added or changed (DEP: neutral-dep, presumed bump): sha2 0.10.9 registry+x bd");
    assert!(repin(Some(&lock), &t, &K, "neutral", "PR9", "x", None).unwrap_err().contains("use neutral-dep"));
    let mut forged = parse_lock(&repin(Some(&lock), &t, &K, "neutral-dep", "PR9", "sha2 patch release", None).unwrap()).unwrap();
    forged.history[1].kind = "neutral".into();
    refused(check(&forged, &t, &K), "the dependency closure or a rust-toolchain.toml changed");
}

#[test]
fn a_spent_gate_value_is_refused() {
    let r = fixture("gate");
    let lock = base(&tree(&r));
    append(&r, "lp4-execution/src/fees.rs", " ");
    let t = tree(&r);
    // maxFork=2 is recorded on the base line, so fork:2 is spent once merged (D2); fork:3 is fresh.
    let spent = repin(Some(&lock), &t, &K, "neutral", "PR9", "v2 tweak", Some("fork:2")).unwrap_err();
    assert!(spent.contains("gate=fork:2 is spent") && spent.contains(INSTRUCTIONS), "{spent}");
    assert!(repin(Some(&lock), &t, &K, "neutral", "PR9", "x", Some("spec:1")).unwrap_err().contains("gate=spec:1 is spent"));
    let fresh = parse_lock(&repin(Some(&lock), &t, &K, "neutral", "PR9", "v3 rules gated", Some("fork:3")).unwrap()).unwrap();
    check(&fresh, &t, &K).unwrap();
    // The v3 PR's line records maxFork=3, so a later line cannot gate on fork:3 again.
    let k3 = Constants { max_fork: 3, ..K };
    let mut reused = fresh.clone();
    reused.history[1].max_fork = 3;
    append(&r, "lp4-execution/src/fees.rs", " ");
    let t2 = tree(&r);
    let mut line = reused.history[1].clone();
    line.n = 3; line.sources = t2.sources.clone();
    reused.history.push(line);
    reused.files = t2.files.clone();
    refused(check(&reused, &t2, &k3), "gate=fork:3 is spent (an earlier line records maxFork=3");
}

#[test]
fn an_unclassified_module_is_refused() {
    let r = fixture("classify");
    append(&r, "pocold/src/lib.rs", "pub mod gossip;\n");
    put(&r, "pocold/src/gossip.rs", "\n");
    let t = tree(&r);
    refused(check(&base_unchecked(&t), &t, &K), "classify module pocold::gossip");
    assert!(repin(None, &t, &K, "base", "PR5", "x", None).unwrap_err().contains("classify first"));
}
/// A base lock for a tree that re-pin would refuse (the guard must still report the tree's problems).
fn base_unchecked(t: &Tree) -> Lock {
    let h = History { n: 1, kind: "base".into(), rules: 1, max_fork: 2, spec: 1, protocol: 2, protocol_min: 2, sources: t.sources.clone(), vectors: t.vectors.clone(), deps: t.dep_digest.clone(), wire: t.wire_digest.clone(), gate: None, reference: "PR5".into(), note: "x".into() };
    parse_lock(&render_lock(&[h], t)).unwrap()
}

#[test]
fn a_consensus_file_referencing_a_node_local_module_is_refused() {
    // At 9c502ff, hashes.rs reached crate::secp_backend while secp_backend was not in the boundary (A1-1).
    let r = fixture("reference");
    put(&r, BOUNDARY_FILE, &FIXTURE_BOUNDARY.replace("{lib,chain}", "{lib,chain,hashes}").replace("=pocold::net\n", "=pocold::net\n=pocold::secp_backend\n"));
    append(&r, "pocold/src/lib.rs", "pub mod hashes;\npub mod secp_backend;\n");
    put(&r, "pocold/src/hashes.rs", "pub fn recover() { crate::secp_backend::recover() }\n");
    put(&r, "pocold/src/secp_backend.rs", "pub fn recover() {}\n");
    let t = tree(&r);
    refused(check(&base_unchecked(&t), &t, &K), "pocold/src/hashes.rs (+) references pocold::secp_backend");
    // Grouped imports and the crate root's bare module paths count too; comments do not.
    put(&r, "pocold/src/hashes.rs", "// crate::secp_backend in a comment\nuse crate::{chain, secp_backend::{self, recover}};\n");
    assert!(tree(&r).problems.iter().any(|p| p.contains("references pocold::secp_backend")));
    put(&r, "pocold/src/hashes.rs", "/// crate::secp_backend\npub fn x() {}\n");
    append(&r, "pocold/src/lib.rs", "pub fn f() { net::start() }\n");
    let t = tree(&r);
    assert_eq!(t.problems.len(), 1, "{:?}", t.problems);
    assert!(t.problems[0].contains("pocold/src/lib.rs (+) references pocold::net"));
    // An announced reference passes; an announcement nothing uses is itself refused.
    put(&r, BOUNDARY_FILE, &(std::fs::read_to_string(r.join(BOUNDARY_FILE)).unwrap() + "~pocold::net reviewed: f() only starts the node\n~pocold::secp_backend unused\n"));
    let t = tree(&r);
    assert_eq!(t.problems.len(), 1, "{:?}", t.problems);
    assert!(t.problems[0].contains("~pocold::secp_backend in pocold/consensus-boundary.txt is unused"));
}

#[test]
fn a_tests_file_not_declared_under_cfg_test_is_consensus() {
    let r = fixture("tests-file");
    let lock = base(&tree(&r));
    put(&r, "lp4-execution/src/lib.rs", "pub mod fees;\npub mod pool;\nmod fee_tests;\n");
    let t = tree(&r);
    assert!(t.files.contains_key("lp4-execution/src/fee_tests.rs"));
    refused(check(&lock, &t, &K), "added: lp4-execution/src/fee_tests.rs [matches -*tests.rs but lp4-execution/src/lib.rs declares it without #[cfg(test)]");
}

#[test]
fn a_tests_file_a_consensus_file_loads_as_a_live_module_is_refused() {
    // lib.rs declares fee_tests.rs under #[cfg(test)], so check 7 excludes it; fees.rs loading it too must not hide it.
    let r = fixture("tests-live");
    let lock = base(&tree(&r));
    append(&r, "lp4-execution/src/fees.rs", "#[path = \"fee_tests.rs\"] pub mod live;\n");
    let t = tree(&r);
    assert!(!t.files.contains_key("lp4-execution/src/fee_tests.rs"));
    refused(check(&lock, &t, &K), "lp4-execution/src/fees.rs loads lp4-execution/src/fee_tests.rs (module lp4::live) without #[cfg(test)]");
    assert!(repin(Some(&lock), &t, &K, "neutral", "PR5", "x", None).unwrap_err().contains("classify first"));
    put(&r, "lp4-execution/src/fees.rs", "pub fn base_fee() -> u64 { 7 }\n#[cfg(test)] #[path = \"fee_tests.rs\"] mod again;\n");
    assert_eq!(tree(&r).problems, Vec::<String>::new(), "a gated second declaration stays test-only");
}

#[test]
fn the_reference_scanner_skips_comments_test_modules_and_char_literals() {
    let src = "use crate::a; // crate::b\n/* crate::c */ fn f() { let _ = '\\''; let _ = '{'; }\n#[cfg(test)] mod tests { use crate::e; const X: char = '}'; }\n\
        mod inner { use super::*; }\nuse super::{g, h::{self, i}};\n";
    let got = references(src, "pocold", 1, &std::collections::BTreeSet::new());
    let want: Vec<(String, String)> = ["a", "g", "h"].iter().map(|n| ("pocold".to_string(), n.to_string())).collect();
    assert_eq!(got, want);
    let decls = mod_decls(src);
    assert_eq!(decls.iter().map(|d| (d.name.as_str(), d.test, d.inline)).collect::<Vec<_>>(), [("tests", true, true), ("inner", false, true)]);
}

#[test]
fn a_self_or_super_chain_that_climbs_to_the_crate_root_is_a_crate_reference() {
    let mods = |n: &[&str]| n.iter().map(|x| x.to_string()).collect::<std::collections::BTreeSet<_>>();
    let names = |v: Vec<(String, String)>| v.into_iter().map(|(c, n)| format!("{c}::{n}")).collect::<Vec<_>>();
    // The crate root: `super::` from a non-test inline module (lib.rs has threads, io_fault and alloc_meter) and `self::`.
    let root = "pub mod threads { pub const X: usize = super::net::MAX_FRAME; mod deeper { use super::super::rpc as r; } }\n\
        fn f() { self::addrbook::open() }\n#[cfg(test)] mod tests { fn g() { super::miner::x() } }\n";
    assert_eq!(names(references(root, "pocold", 0, &mods(&[]))), ["pocold::addrbook", "pocold::net", "pocold::rpc"]);
    // Depth 2: two `super::`s reach the root, one stays in the parent module; an inline module adds one level.
    let deep = "fn f() { super::super::net::X; super::sibling::Y; self::own::Z }\nmod m { fn g() { super::super::super::resources::Y; super::super::not_root() } }\n";
    assert_eq!(names(references(deep, "lp4", 2, &mods(&[]))), ["lp4::net", "lp4::resources"]);
    // In a fixture: lib.rs's inline module reaching the node-local net through `super::` is refused like `crate::net`.
    let r = fixture("relative");
    append(&r, "pocold/src/lib.rs", "pub mod threads { pub fn f() { super::net::start() } }\n");
    let t = tree(&r);
    refused(check(&base_unchecked(&t), &t, &K), "pocold/src/lib.rs (+) references pocold::net");
}

#[test]
fn a_dollar_crate_path_in_a_macro_is_a_crate_reference() {
    // `$crate::` resolves to the defining crate's root, exactly like `crate::`; a metavariable path (`$m::x`) does not.
    let names = |v: Vec<(String, String)>| v.into_iter().map(|(c, n)| format!("{c}::{n}")).collect::<Vec<_>>();
    let src = "macro_rules! frame { ($m:ident) => { $crate::net::MAX_FRAME + $crate::{rpc::X, chain::Y} + $m::z() } }\n";
    assert_eq!(names(references(src, "pocold", 1, &std::collections::BTreeSet::new())), ["pocold::chain", "pocold::net", "pocold::rpc"]);
    // In a fixture: a + file's macro reaching the node-local net through `$crate::` fails check 6.
    let r = fixture("dollar-crate");
    append(&r, "pocold/src/chain.rs", "#[macro_export] macro_rules! frame { () => { $crate::net::MAX_FRAME } }\n");
    let t = tree(&r);
    refused(check(&base_unchecked(&t), &t, &K), "pocold/src/chain.rs (+) references pocold::net");
}

#[test]
fn a_hand_edited_history_line_is_not_accepted() {
    let r = fixture("canonical");
    let text = repin(None, &tree(&r), &K, "base", "PR5", "fixture", None).unwrap();
    assert!(parse_lock(&text.replace("kind=base rules=1", "kind=base  rules=1")).unwrap_err().contains("not in the form re-pin writes"));
}

#[test]
fn history_lines_are_append_only_numbered_and_labelled() {
    let r = fixture("history");
    let t = tree(&r);
    let lock = base(&t);
    let mut bad = lock.clone();
    bad.history[0].kind = "neutral".into();
    refused(check(&bad, &t, &K), "only the first line is kind=base");
    let mut bad = lock.clone();
    bad.history[0].note = String::new();
    refused(check(&bad, &t, &K), "needs a non-empty ref and note");
    let mut bad = lock.clone();
    bad.history[0].max_fork = 3;
    refused(check(&bad, &t, &K), "MAX_FORK_VERSION=2");
    assert!(repin(Some(&lock), &t, &K, "neutral", "PR9", "x", None).unwrap_err().contains("nothing to re-pin"));
    // base regenerates a lock with no appended lines (PR5 only: the lock is new); it never rewrites history.
    append(&r, "lp4-execution/src/fees.rs", " ");
    let t = tree(&r);
    let two = parse_lock(&repin(Some(&lock), &t, &K, "neutral", "PR9", "refactor", None).unwrap()).unwrap();
    assert_eq!((two.history.len(), &two.history[0]), (2, &lock.history[0]));
    assert!(repin(Some(&two), &t, &K, "base", "PR9", "x", None).unwrap_err().contains("kind=base only"));
    check(&parse_lock(&repin(Some(&lock), &t, &K, "base", "PR5", "regenerated", None).unwrap()).unwrap(), &t, &K).unwrap();
}

// ---- T8: mutation proof on a scratch copy of the real boundary (no rebuild) ----

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap() {
        let e = e.unwrap();
        if e.file_type().unwrap().is_dir() { copy_dir(&e.path(), &to.join(e.file_name())) } else { std::fs::copy(e.path(), to.join(e.file_name())).unwrap(); }
    }
}

#[test]
fn the_aggregate_moves_exactly_with_consensus_inputs() {
    let real = root();
    let r = scratch("mutation");
    for d in ["lp4-execution/src", "lp4-execution/tests/data", "lp1-node/src", "lp1-node/tests/data", "third-party/evm-crypto", "pocold/src"] { copy_dir(&real.join(d), &r.join(d)) }
    for f in ["pocold/build.rs", "pocold/consensus_boundary.rs", BOUNDARY_FILE, CARGO_LOCK, "pocold/Cargo.toml", "lp4-execution/Cargo.toml", "lp1-node/Cargo.toml", "pocold/rust-toolchain.toml", "lp4-execution/rust-toolchain.toml", WIRE_FILE] {
        std::fs::create_dir_all(r.join(f).parent().unwrap()).unwrap();
        std::fs::copy(real.join(f), r.join(f)).unwrap();
    }
    let t0 = tree(&r);
    assert_eq!(t0.sources, tree(&real).sources, "the copy is the tree");
    assert!(t0.problems.is_empty(), "{:?}", t0.problems);
    // The closure parser keeps the engine and the crypto stack (risk table): pinned names and a floor on its size.
    for name in ["revm ", "revm-precompile ", "k256 ", "alloy-primitives ", "sha2 0.9.9", "sha3 ", "pocol-lp4-execution "] {
        assert!(t0.deps.iter().any(|d| d.starts_with(name)), "{name} is in the closure");
    }
    assert!(t0.deps.len() >= 284, "closure has {} packages (284 at PR5)", t0.deps.len());
    assert!(!t0.deps.iter().any(|d| d.starts_with("redb ")), "redb is node-local storage");
    let moved = |path: &str, edit: &dyn Fn(&str) -> String| {
        let p = r.join(path);
        let before = std::fs::read(&p).unwrap();
        std::fs::write(&p, edit(&String::from_utf8(before.clone()).unwrap())).unwrap();
        let s = tree(&r).sources;
        std::fs::write(&p, before).unwrap();
        s != t0.sources
    };
    let append = |s: &str| format!("{s} ");
    for path in ["lp4-execution/src/fees.rs", "pocold/src/secp_backend.rs", "lp1-node/tests/data/lp4-trie-vectors.json", "pocold/rust-toolchain.toml"] {
        assert!(moved(path, &append), "{path} is consensus");
    }
    assert!(moved(CARGO_LOCK, &|s| s.replacen("name = \"revm\"\nversion = \"19.7.0\"", "name = \"revm\"\nversion = \"19.7.1\"", 1)), "the revm version is consensus");
    assert!(!moved("pocold/src/net.rs", &append), "net.rs is node-local");
    assert!(!moved(CARGO_LOCK, &|s| s.replacen("name = \"redb\"\nversion = \"2.6.4\"", "name = \"redb\"\nversion = \"2.6.5\"", 1)), "redb is outside the closure");
    assert!(!moved("lp4-execution/tests/data/website.json", &append), "website.json is excluded");
    assert!(moved("lp4-execution/src/admission.rs", &append), "admission builds the profile chain::Spec wraps");
    assert!(!moved(WIRE_FILE, &|s| s.replacen("c108", "c109", 1)), "the wire golden file is pinned by wire lines, not by the aggregate");
}

// ---- T19: the wire golden rules (plan §1.6) ----

fn wire_refused(r: Result<(), String>, needle: &str) {
    let e = r.expect_err("the guard must refuse");
    assert!(e.contains(needle) && e.contains(WIRE_INSTRUCTIONS), "missing {needle:?} or the wire instructions in:\n{e}");
}

#[test]
fn a_pinned_wire_key_never_changes_or_disappears_while_its_version_is_accepted() {
    let r = fixture("wire-frozen");
    let lock = base(&tree(&r));
    assert_eq!(lock.wire.iter().map(|w| w.key.as_str()).collect::<Vec<_>>(), ["v2/get-addr", "v2/hello"], "the lock pins one line per key");
    put(&r, WIRE_FILE, &WIRE_V2.replace("c180", "c20180"));
    let t = tree(&r);
    wire_refused(check(&lock, &t, &K), "wire key v2/hello (v2 >= PROTOCOL_VERSION_MIN=2) changed");
    assert!(repin(Some(&lock), &t, &K, "wire", "PR9", "new hello field", None).unwrap_err().contains("v2/hello (v2 >= PROTOCOL_VERSION_MIN=2) changed"));
    put(&r, WIRE_FILE, "[{\"key\":\"v2/hello\",\"v\":2,\"hex\":\"c180\"}]");
    let t = tree(&r);
    wire_refused(check(&lock, &t, &K), "wire key v2/get-addr (v2 >= PROTOCOL_VERSION_MIN=2) was removed");
    assert!(repin(Some(&lock), &t, &K, "wire", "PR9", "drop GetAddr", None).is_err());
    // Raising PROTOCOL_VERSION_MIN past v2 (with a v3 wire) retires the v2 keys: they may then change or go.
    let k3 = Constants { protocol: 3, protocol_min: 3, ..K };
    let retired = parse_lock(&repin(Some(&lock), &t, &k3, "wire", "PR9", "wire 3 only", None).unwrap()).unwrap();
    check(&retired, &t, &k3).unwrap();
}

#[test]
fn a_new_wire_key_needs_a_new_wire_version() {
    let r = fixture("wire-new");
    let lock = base(&tree(&r));
    put(&r, WIRE_FILE, &WIRE_V2.replace("]", ",{\"key\":\"v2/ping\",\"v\":2,\"hex\":\"c10a\"}]"));
    let t = tree(&r);
    wire_refused(check(&lock, &t, &K), "new wire key v2/ping at v2");
    assert!(repin(Some(&lock), &t, &K, "wire", "PR9", "ping", None).unwrap_err().contains("new wire key v2/ping at v2"));
    put(&r, WIRE_FILE, &WIRE_V2.replace("]", ",{\"key\":\"v3/ping\",\"v\":3,\"hex\":\"c10a\"}]"));
    let t = tree(&r);
    assert!(repin(Some(&lock), &t, &K, "wire", "PR9", "ping", None).unwrap_err().contains("new wire key v3/ping at v3"), "v3 needs PROTOCOL_VERSION=3");
    let k3 = Constants { protocol: 3, ..K };
    let three = parse_lock(&repin(Some(&lock), &t, &k3, "wire", "PR9", "wire 3 adds Ping", None).unwrap()).unwrap();
    check(&three, &t, &k3).unwrap();
    assert_eq!((three.history[1].kind.as_str(), &three.history[1].sources), ("wire", &lock.history[0].sources), "a wire line keeps the sources");
    // Once v3 keys are pinned, another v3 key needs v4.
    put(&r, WIRE_FILE, &WIRE_V2.replace("]", ",{\"key\":\"v3/ping\",\"v\":3,\"hex\":\"c10a\"},{\"key\":\"v3/pong\",\"v\":3,\"hex\":\"c10b\"}]"));
    wire_refused(check(&three, &tree(&r), &k3), "new wire key v3/pong at v3");
}

#[test]
fn a_wire_line_records_only_a_wire_change() {
    let r = fixture("wire-kind");
    let lock = base(&tree(&r));
    let k3 = Constants { protocol: 3, ..K };
    put(&r, WIRE_FILE, &WIRE_V2.replace("]", ",{\"key\":\"v3/ping\",\"v\":3,\"hex\":\"c10a\"}]"));
    assert!(repin(Some(&lock), &tree(&r), &k3, "neutral", "PR9", "x", None).unwrap_err().contains("re-pin kind=wire"));
    append(&r, "lp4-execution/src/fees.rs", " ");
    let t = tree(&r);
    assert!(repin(Some(&lock), &t, &k3, "wire", "PR9", "x", None).unwrap_err().contains("kind=wire records only a wire change"));
    // Together with a consensus change the consensus kind carries it, and the wire rules still apply.
    let both = parse_lock(&repin(Some(&lock), &t, &k3, "neutral", "PR9", "refactor and wire 3", None).unwrap()).unwrap();
    check(&both, &t, &k3).unwrap();
    let mut forged = both.clone();
    forged.history[1].kind = "wire".into();
    refused(check(&forged, &t, &k3), "kind=wire records only a wire change");
    // An unlisted or malformed wire file fails closed.
    put(&r, WIRE_FILE, "[{\"key\":\"hello\",\"v\":2,\"hex\":\"c180\"}]");
    assert!(compute(&r).unwrap_err().contains("must be v<version>/<name>"));
    put(&r, WIRE_FILE, "[{\"key\":\"v3/hello\",\"v\":2,\"hex\":\"c180\"}]");
    assert!(compute(&r).unwrap_err().contains("differs from the key's version"));
    std::fs::remove_file(r.join(WIRE_FILE)).unwrap();
    assert!(compute(&r).unwrap_err().contains(WIRE_FILE));
}

#[test]
fn the_real_lock_pins_every_wire_golden_key() {
    let lock = parse_lock(&std::fs::read_to_string(root().join(LOCK_FILE)).unwrap()).unwrap();
    let keys: Vec<&str> = lock.wire.iter().map(|w| w.key.as_str()).collect();
    for k in ["v2/hello", "v2/block", "v2/template", "v2/share", "v2/get-block", "v2/get-range", "v2/tx", "v2/get-from", "v2/get-addr", "v2/addr", "v2/limits"] {
        assert!(keys.contains(&k), "{k} is pinned");
    }
}
