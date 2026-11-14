#!/usr/bin/env python3
"""Mixed-build interop with real pocold processes on loopback (plan PR5 commit 8, T10, T35; VA-SPEC §2).

Four builds of one network:
  A  this tree, built with POCOL_RELEASE=1 (it must be pinned);
  B  a copy that differs from A only in a comment appended to pocold/src/rpc.rs (a node-local file): another build
     digest (compatibilitySha256), the same consensus identity. A and B must peer (compatiblePeers 1 on both) and
     reach one head, with blocks mined by both in it. HEAD (whose Hello carried the build digest) fails here (T35);
  C  a copy with CONSENSUS_RULES_VERSION incremented (2 -> 3) and the lock re-pinned as kind=bump: pinned, rules 3. A and C
     never peer, C stays at genesis, and each logs and reports "peer runs consensus rules N";
  D  a copy with one byte appended to lp4-execution/src/fees.rs and no re-pin (T10): it builds, reports
     consensusPinned false and another consensusIdentity, is refused by A, and says why itself ("this node runs an
     unpinned (development) build; the peer runs the pinned rules"). A POCOL_RELEASE=1 build of D's tree fails.
A pinned node cannot name an unpinned peer's state (it does not know the peer's aggregate), so A's own line for D is
the generic "different consensus rules…" text; the unpinned side is the one that says "unpinned".

The copies are one tracked-files copy of prototypes/, mutated and restored in turn, built with their own
CARGO_TARGET_DIR (not this tree's), so the registry dependencies compile once and this tree's target/ is never
written by a mutated build. Every wait is bounded and names what it waited for. Exit status 0 only if every check
passes; on failure the node logs and network files are kept (devnet.keep_on_failure).

Environment: POCOLD uses that binary as A instead of building this tree. POCOL_MIXED_BUILD keeps the copy and its
target directory there across runs (default: a temporary directory, removed afterwards). CARGO selects cargo.
"""
import contextlib, io, json, os, shutil, signal, socket, subprocess, sys, tempfile, time
HERE = os.path.dirname(os.path.abspath(__file__)); ROOT = os.path.dirname(HERE); PROTO = os.path.dirname(ROOT)
sys.path.insert(0, os.path.join(ROOT, "scripts"))
import devnet as dn
# Ports clear of e2e_devnet's (30300+i, 8540+i).
P2P, RPC = 30310, 8550
IDX = {l: i for i, l in enumerate("abcd")}
CARGO = os.environ.get("CARGO", "cargo")
RPC_FILE, RULES_FILE, FEES_FILE = "pocold/src/rpc.rs", "pocold/src/identity.rs", "lp4-execution/src/fees.rs"
LOCK_FILE = "pocold/consensus-sources.lock"
RULES, NEXT = 2, 3
RULES_LINE = f"pub const CONSENSUS_RULES_VERSION: u32 = {RULES};"

def waited(pred, timeout, what):
    t = time.time(); met = False
    while time.time() - t < timeout:
        try:
            if pred(): met = True; break
        except Exception: pass
        time.sleep(0.5)
    print(f"WAIT {what}: {'met' if met else 'NOT met'} after {time.time()-t:.1f}s (bound {timeout}s)", flush=True)
    return met
def wait(pred, timeout, what):
    if not waited(pred, timeout, what): raise AssertionError(f"timeout waiting for {what}")
def st(l): return dn.rpc(RPC + IDX[l], "pocol_status")
def head(l): return st(l)["head"]
def port_open(p):
    with socket.socket() as s: s.settimeout(0.5); return s.connect_ex(("127.0.0.1", p)) == 0

def cargo(tree, target, args, env_extra=None, release=False):
    """Runs cargo for `tree`'s pocold from its own directory (so its rust-toolchain.toml applies), with no
    re-pin or release variable inherited from the caller."""
    env = {k: v for k, v in os.environ.items() if not k.startswith("POCOL_REPIN_") and k not in ("POCOL_RELEASE", "CARGO_TARGET_DIR")}
    env["CARGO_TARGET_DIR"] = target
    if release: env["POCOL_RELEASE"] = "1"
    env.update(env_extra or {})
    cmd = CARGO.split() + args + ["--release", "--locked", "--manifest-path", os.path.join(tree, "pocold", "Cargo.toml")]
    t = time.time(); r = subprocess.run(cmd, cwd=os.path.join(tree, "pocold"), env=env, capture_output=True, text=True)
    print(f"BUILD {' '.join(args)} in {tree} (release={release}): exit {r.returncode} after {time.time()-t:.0f}s", flush=True)
    return r
def build(tree, target, out, release):
    """Builds pocold and copies it to `out`: the next build in the same target directory replaces target/release/pocold."""
    r = cargo(tree, target, ["build", "--bin", "pocold"], release=release)
    if r.returncode: raise AssertionError(f"building {tree} failed:\n{r.stderr[-4000:]}")
    shutil.copy2(os.path.join(target, "release", "pocold"), out); return out
def build_info(binary):
    return json.loads(subprocess.run([binary, "build-info"], capture_output=True, text=True, check=True).stdout)

def copy_tree(dest):
    """The tracked and untracked-but-not-ignored files of prototypes/ (no target/, no kept failures)."""
    names = subprocess.run(["git", "ls-files", "-z", "--cached", "--others", "--exclude-standard", "--", "."], cwd=PROTO,
                           capture_output=True, check=True).stdout.split(b"\0")
    for n in filter(None, names):
        rel = n.decode(); src = os.path.join(PROTO, rel)
        if not os.path.isfile(src) or os.path.islink(src): continue
        d = os.path.join(dest, rel); os.makedirs(os.path.dirname(d), exist_ok=True); data = open(src, "rb").read()
        # A kept copy (POCOL_MIXED_BUILD) is refreshed by content: an unchanged file keeps its mtime, a changed one
        # gets a new mtime, so Cargo never takes a stale artifact for a fresh one.
        if not os.path.isfile(d) or open(d, "rb").read() != data: open(d, "wb").write(data); shutil.copymode(src, d)

def main():
    busy = [p for l in IDX for p in (P2P + IDX[l], RPC + IDX[l]) if port_open(p)]
    if busy: print(json.dumps({"result": "FAIL", "error": f"ports busy {busy}"})); return 1
    t_start = time.time(); checks = []; procs = {}
    work = tempfile.mkdtemp(prefix="pocol-mixed-"); net = os.path.join(work, "net")
    keep = os.environ.get("POCOL_MIXED_BUILD"); bdir = keep or tempfile.mkdtemp(prefix="pocol-mixed-build-")
    tree, target, bins = os.path.join(bdir, "tree"), os.path.join(bdir, "target"), os.path.join(work, "bin")
    os.makedirs(bins, exist_ok=True)
    def ok(name, cond, detail=""):
        checks.append((name, bool(cond), detail)); print(("PASS " if cond else "FAIL ") + name + (f" — {detail}" if detail else ""), flush=True)
        if not cond: raise AssertionError(name)
    def run_node(l, binary, miner, peers):
        i = IDX[l]
        if port_open(RPC + i) or port_open(P2P + i): raise AssertionError(f"{l} is still running")
        # No discovery: the only connections are the configured ones, so each refusal is attributable.
        cmd = [binary, "run", "--network", net, "--data", os.path.join(work, "data", l), "--name", l, "--listen", f"127.0.0.1:{P2P+i}",
               "--rpc", f"127.0.0.1:{RPC+i}", "--log", "--no-discovery"] + sum([["--peer", f"127.0.0.1:{P2P+IDX[p]}"] for p in peers], [])
        if miner: cmd += ["--miner", l]
        procs[l] = subprocess.Popen(cmd, stdout=open(os.path.join(work, f"{l}.log"), "ab"), stderr=subprocess.STDOUT, start_new_session=True)
        wait(lambda: st(l) is not None, 60, f"{l} answers RPC")
    def log_of(l): return open(os.path.join(work, f"{l}.log"), errors="replace").read()
    error = None
    try:
        # Build A from this tree (a no-op when it is current), unless POCOLD names it.
        a_bin = os.environ.get("POCOLD")
        if not a_bin:
            a_target = os.environ.get("CARGO_TARGET_DIR") or os.path.join(ROOT, "target")
            a_bin = build(PROTO, a_target, os.path.join(bins, "pocold-a"), release=True)
        a = build_info(a_bin)
        ok("A is a pinned build", a["consensusPinned"] is True, a["consensusSourcesSha256"])
        # B, C and D: one copy, mutated and restored in turn.
        copy_tree(tree)
        path = lambda f: os.path.join(tree, f)
        pristine = {f: open(os.path.join(PROTO, f), "rb").read() for f in (RPC_FILE, RULES_FILE, FEES_FILE, LOCK_FILE)}
        def restore():
            # Written, not copied with its old mtime: Cargo must see every restored file as changed.
            for f, b in pristine.items(): open(path(f), "wb").write(b)
        restore()
        open(path(RPC_FILE), "ab").write(b"\n// e2e_mixed_release: build B differs from A only in this comment.\n")
        b_bin = build(tree, target, os.path.join(bins, "pocold-b"), release=True); restore()
        b = build_info(b_bin)
        ok("B (a node-local comment) has another build digest and the same pinned consensus sources",
           b["compatibilitySha256"] != a["compatibilitySha256"] and b["consensusPinned"] is True and b["consensusSourcesSha256"] == a["consensusSourcesSha256"],
           f"A {a['compatibilitySha256'][:16]} B {b['compatibilitySha256'][:16]}")
        text = pristine[RULES_FILE].decode()
        if text.count(RULES_LINE) != 1: raise AssertionError(f"{RULES_FILE} must define {RULES_LINE!r} once")
        open(path(RULES_FILE), "w").write(text.replace(RULES_LINE, RULES_LINE.replace(f"= {RULES};", f"= {NEXT};")))
        r = cargo(tree, target, ["test", "--test", "consensus_guard", "the_tree_matches_the_consensus_lock"],
                  {"POCOL_REPIN_CONSENSUS": "bump", "POCOL_REPIN_REF": "e2e", "POCOL_REPIN_NOTE": f"e2e_mixed_release: rules {NEXT} with the same genesis (a test only)"})
        if r.returncode: raise AssertionError(f"re-pin kind=bump failed:\n{r.stdout[-2000:]}{r.stderr[-4000:]}")
        c_bin = build(tree, target, os.path.join(bins, "pocold-c"), release=True); restore()
        c = build_info(c_bin)
        ok(f"C (rules {NEXT}, re-pinned kind=bump) is pinned at rules {NEXT}", c["consensusPinned"] is True and c["consensusRulesVersion"] == NEXT, c["consensusSourcesSha256"])
        open(path(FEES_FILE), "ab").write(b"\n")
        d_bin = build(tree, target, os.path.join(bins, "pocold-d"), release=False)
        d = build_info(d_bin)
        ok("D (a consensus byte, no re-pin) builds unpinned", d["consensusPinned"] is False and d["consensusSourcesSha256"] != a["consensusSourcesSha256"], d["consensusSourcesSha256"])
        r = cargo(tree, target, ["build", "--bin", "pocold"], release=True); restore()
        ok("T10: a POCOL_RELEASE=1 build of D's tree fails with the instruction text",
           r.returncode != 0 and "POCOL_RELEASE=1 refuses an unpinned consensus tree" in r.stderr and "re-pin" in r.stderr, f"exit {r.returncode}")

        # One network, made by A; only a and b mine.
        dn.BIN = a_bin
        with contextlib.redirect_stdout(io.StringIO()):  # init-devnet echoes devnet.json
            dn.init(work, "a,b", ["--t-blk", "2", "--tau", "20", "--c", "8", "--d-att", "41", "--fb-wait", "3", "--work-bits", "15"])
        # Only the later nodes dial a, so each pair holds one connection and compatiblePeers counts peers, not links.
        run_node("a", a_bin, True, []); run_node("b", b_bin, True, ["a"])
        sa, sb = st("a"), st("b")
        ok("A and B report their own build digests and one consensus identity",
           sa["compatibilitySha256"] == a["compatibilitySha256"] and sb["compatibilitySha256"] == b["compatibilitySha256"]
           and sa["consensusIdentity"] == sb["consensusIdentity"] and sa["consensusPinned"] is True and sb["consensusPinned"] is True, sa["consensusIdentity"][:16])
        wait(lambda: st("a")["compatiblePeers"] == 1 and st("b")["compatiblePeers"] == 1, 60, "A and B peer")
        ok("T35: builds with different digests and equal rules peer", True)
        def one_head():
            ha, hb = head("a"), head("b")
            return ha["height"] >= 8 and ha["hash"] == hb["hash"]
        # Each node imported a block it solved itself (stats.mined), so blocks of both builds crossed the link.
        wait(lambda: one_head() and st("a")["stats"]["mined"] > 0 and st("b")["stats"]["mined"] > 0, 180, "A and B both mine and reach one head at height >= 8")
        top = head("a")["height"]
        ok("A and B reach the same head after mining, with blocks of both", True,
           f"height {top}, mined a={st('a')['stats']['mined']} b={st('b')['stats']['mined']}")

        # C: other rules, same genesis.
        refused = lambda l: st(l)["incompatiblePeers"]["consensus"]
        a0 = refused("a")
        run_node("c", c_bin, False, ["a"])
        wait(lambda: refused("a") > a0 and refused("c") > 0, 60, "A and C refuse each other")
        waited(lambda: st("c")["compatiblePeers"] != 0, 10, "C peers (must not)")
        sa, sc = st("a"), st("c")
        ok("A and C never peer and C stays at genesis",
           sc["compatiblePeers"] == 0 and sc["head"]["height"] == 0 and sa["compatiblePeers"] == 1 and sc["consensusIdentity"] != sa["consensusIdentity"],
           f"a peers {sa['compatiblePeers']}, c head {sc['head']['height']}")
        ok(f"A logs and reports 'peer runs consensus rules {NEXT}'", f"peer c: peer runs consensus rules {NEXT}; this node runs rules {RULES}" in log_of("a")
           and sa["lastIncompatibleReason"] == f"peer c: peer runs consensus rules {NEXT}; this node runs rules {RULES}", sa["lastIncompatibleReason"])
        ok(f"C logs and reports 'peer runs consensus rules {RULES}'", f"peer a: peer runs consensus rules {RULES}; this node runs rules {NEXT}" in log_of("c")
           and sc["lastIncompatibleReason"] == f"peer a: peer runs consensus rules {RULES}; this node runs rules {NEXT}", sc["lastIncompatibleReason"])

        # D: an unpinned consensus edit isolates itself.
        a1 = refused("a")
        run_node("d", d_bin, False, ["a"])
        wait(lambda: refused("a") > a1 and refused("d") > 0, 60, "A and D refuse each other")
        sa, sd = st("a"), st("d")
        ok("D reports consensusPinned false and another consensus identity",
           sd["consensusPinned"] is False and sd["consensusIdentity"] != sa["consensusIdentity"] and sd["compatiblePeers"] == 0 and sd["head"]["height"] == 0,
           sd["consensusIdentity"][:16])
        unpinned = "peer a: this node runs an unpinned (development) build; the peer runs the pinned rules"
        ok("A refuses D, and D names its unpinned build", sd["lastIncompatibleReason"].startswith(unpinned) and unpinned in log_of("d")
           and "peer d: different consensus rules" in log_of("a") and sa["compatiblePeers"] == 1, sd["lastIncompatibleReason"])
        wait(one_head, 60, "A and B still share one head")
        ok("A and B keep one chain while C and D are refused", head("a")["height"] > top, f"height {head('a')['height']}")
        print(json.dumps({"result": "PASS", "checks": len(checks), "elapsed_s": round(time.time() - t_start), "work": work}))
        return 0
    except Exception as e:
        error = e
    finally:
        for p in procs.values():
            try: os.killpg(p.pid, signal.SIGTERM); p.wait(30)
            except Exception: pass
        if not keep: shutil.rmtree(bdir, ignore_errors=True)
        shutil.rmtree(bins, ignore_errors=True)  # not kept: their build-info is in the checks
    kept = dn.keep_on_failure("e2e_mixed_release", work, {"error": str(error), "checks": [{"name": c[0], "pass": c[1], "detail": str(c[2])[:300]} for c in checks]})
    print(json.dumps({"result": "FAIL", "error": str(error)[-4000:], "checks": [c[0] for c in checks if c[1]], "elapsed_s": round(time.time() - t_start), "work": work, "kept": kept}))
    return 1

if __name__ == "__main__":
    sys.exit(main())
