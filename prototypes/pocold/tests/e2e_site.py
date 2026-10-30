#!/usr/bin/env python3
"""LP8 end-to-end: publish a multi-file website into PoCol state and fetch it
back from every node (also after restart). Checks chunk/manifest/content
hashes, versioning (v2 update, v1 still readable), and that rejected
operations (unauthorized publisher, revoke of current, oversize manifest)
revert without effect on the Website state."""
import json, os, signal, socket, subprocess, sys, tempfile, time
HERE = os.path.dirname(os.path.abspath(__file__)); ROOT = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(ROOT, "scripts")); import devnet as dn
SITE_BIN = os.path.join(ROOT, "target", "release", "pocol-site"); SIGN = os.path.join(ROOT, "target", "release", "pocol-sign")

def wait(pred, timeout, what):
    t = time.time()
    while time.time() - t < timeout:
        try:
            if pred(): return
        except Exception: pass
        time.sleep(0.5)
    raise AssertionError("timeout: " + what)

def site_files(root, version):
    files = {
        "index.html": f"<!doctype html><html><head><link rel=stylesheet href=style.css><title>PoCol site v{version}</title></head><body><h1 id=t>PoCol on-chain site v{version}</h1><script src=app.js></script><a href=docs/>docs</a></body></html>",
        "style.css": "body{font-family:sans-serif;background:#f4f6f8}h1{color:#1a5}",
        "app.js": "document.getElementById('t').dataset.loaded='yes';",
        "docs/index.html": "<!doctype html><p>Docs page served from PoCol state</p>",
        "data.json": json.dumps({"rows": [{"i": i, "v": "x" * 40} for i in range(1400)]}),  # > 2 chunks
    }
    for p, c in files.items():
        f = os.path.join(root, p); os.makedirs(os.path.dirname(f), exist_ok=True); open(f, "w").write(c)
    open(os.path.join(root, "logo.png"), "wb").write(bytes([0x89]) + b"PNG\r\n\x1a\n" + bytes(range(256)) * 4)
    return {("/" + p): open(os.path.join(root, p), "rb").read() for p in list(files) + ["logo.png"]}

def fetch(i, site):
    out = tempfile.mkdtemp()
    r = subprocess.run([SITE_BIN, "fetch", "--rpc", f"127.0.0.1:{8540+i}", "--site", site, "--out", out], capture_output=True, text=True)
    if r.returncode: raise AssertionError(r.stderr)
    meta = json.loads(r.stdout); got = {}
    for f in meta["files"]: got[f["path"]] = open(os.path.join(out, f["path"][1:]), "rb").read()
    return meta, got

def main():
    for port in [30300, 30301, 30302, 8540, 8541, 8542]:
        with socket.socket() as t:
            if t.connect_ex(("127.0.0.1", port)) == 0: raise SystemExit(f"port {port} busy")
    work = tempfile.mkdtemp(prefix="pocol-site-"); n = 0
    def ok(name, cond, d=""):
        nonlocal n; print(("PASS " if cond else "FAIL ") + name + (f" — {d}" if d else ""), flush=True)
        if not cond: raise AssertionError(name)
        n += 1
    try:
        # 8M gas limit: one full 24 KiB chunk per block.
        dn.init(work, "a,b,c", ["--t-blk", "2", "--tau", "20", "--c", "8", "--d-att", "41", "--fb-wait", "3", "--work-bits", "15"])
        info = json.load(open(os.path.join(work, "net", "devnet.json"))); cid = str(info["chainId"])
        dn.start(work, log=True); wait(lambda: min(dn.rpc(8540 + i, "pocol_status")["head"]["height"] for i in range(3)) >= 3, 90, "nodes up")
        modes = [dn.rpc(8540 + i, "pocol_status")["stateStore"] for i in range(3)]
        ok("every node runs in the requested state mode", modes == ["--state-store" in dn.EXTRA] * 3, f"stateStore={modes} (POCOLD_RUN_ARGS)")
        sdir = os.path.join(work, "site1"); want = site_files(sdir, 1)
        r = subprocess.run([SITE_BIN, "publish", "--rpc", "127.0.0.1:8540", "--chain-id", cid, "--account", "alice", "--dir", sdir], capture_output=True, text=True)
        ok("publish v1 (multi-file, multi-chunk) through node a", r.returncode == 0, r.stderr[-300:] if r.returncode else r.stdout.strip())
        pub = json.loads(r.stdout); site = pub["site"]
        time.sleep(3)
        for i in range(3):
            wait(lambda: fetch(i, site)[0]["version"] == 0, 60, f"v1 on node {i}")
            meta, got = fetch(i, site)
            ok(f"node {'abc'[i]} serves v1 with verified chunks/manifest/content hashes", got == want, f"{len(got)} files, entry {meta['entry']}")
        code = [dn.rpc(8540 + i, "eth_getCode", [site, "latest"]) for i in range(3)]
        ok("Website runtime code identical on all nodes", len(set(code)) == 1 and len(code[0]) > 100)
        # v2 update.
        sdir2 = os.path.join(work, "site2"); want2 = site_files(sdir2, 2)
        r = subprocess.run([SITE_BIN, "publish", "--rpc", "127.0.0.1:8541", "--chain-id", cid, "--account", "alice", "--dir", sdir2, "--site", site], capture_output=True, text=True)
        ok("publish v2 through node b", r.returncode == 0, r.stderr[-300:])
        for i in range(3):
            wait(lambda: fetch(i, site)[0]["version"] == 1, 60, f"v2 on {i}")
            ok(f"node {'abc'[i]} serves v2", fetch(i, site)[1] == want2)
        # Unchanged chunks are reused (idempotent CREATE2): style.css identical in v1/v2.
        # Rejected operations: unauthorized publisher (bob) and revoke(current) by the owner.
        def storage(i): return [dn.rpc(8540 + i, "eth_getStorageAt", [site, hex(k), "latest"]) for k in range(6)]
        before = storage(0)
        bob_nonce = int(dn.rpc(8540, "eth_getTransactionCount", [next(a["address"] for a in info["accounts"] if a["label"] == "bob"), "pending"]), 16)
        SEL = {k.split("(")[0]: v for k, v in json.load(open(os.path.join(ROOT, "..", "lp4-execution", "tests", "data", "website.json")))["methodIdentifiers"].items()}
        def send(account, nonce, data, gas=300000):
            raw = subprocess.run([SIGN, "--chain-id", cid, "--account", account, "--to", site, "--nonce", str(nonce), "--gas", str(gas), "--data", data], capture_output=True, text=True, check=True).stdout.strip()
            h = dn.rpc(8540, "eth_sendRawTransaction", [raw]); wait(lambda: dn.rpc(8540, "eth_getTransactionReceipt", [h]) is not None, 60, "receipt")
            return dn.rpc(8540, "eth_getTransactionReceipt", [h])
        r1 = send("bob", bob_nonce, "0x" + SEL["publish"] + "%064x" % 0)
        alice_nonce = int(dn.rpc(8540, "eth_getTransactionCount", [next(a["address"] for a in info["accounts"] if a["label"] == "alice"), "pending"]), 16)
        r2 = send("alice", alice_nonce, "0x" + SEL["revoke"] + "%064x" % 1)
        r3 = send("alice", alice_nonce + 1, "0x" + SEL["createVersion"] + "00" * 32 + "%064x" % 70000 + "%064x" % 96 + "%064x" % 0)
        ok("unauthorized publish / revoke(current) / oversize manifest all revert", (r1["status"], r2["status"], r3["status"]) == ("0x0", "0x0", "0x0"), f"{r1['status']} {r2['status']} {r3['status']}")
        ok("reverted operations leave Website storage unchanged", storage(0) == before)
        # Review fixes: (1) an oversize site is rejected before any transaction; (2) an empty file is
        # valid (pos-empty-file); (3) concurrent publishers each publish exactly their own version.
        big = os.path.join(work, "big"); os.makedirs(big)
        open(os.path.join(big, "index.html"), "w").write("<!doctype html><p>big")
        for k in range(5): open(os.path.join(big, f"f{k}.txt"), "wb").write(os.urandom(1 << 20))
        n0 = int(dn.rpc(8540, "eth_getTransactionCount", [next(a["address"] for a in info["accounts"] if a["label"] == "alice"), "pending"]), 16)
        r = subprocess.run([SITE_BIN, "publish", "--rpc", "127.0.0.1:8540", "--chain-id", cid, "--account", "alice", "--dir", big, "--site", site], capture_output=True, text=True)
        n1 = int(dn.rpc(8540, "eth_getTransactionCount", [next(a["address"] for a in info["accounts"] if a["label"] == "alice"), "pending"]), 16)
        ok("oversize site (5 MiB) rejected before any transaction (nonce unchanged)", r.returncode != 0 and "4 MiB" in r.stderr and n0 == n1, r.stderr.strip()[-80:])
        bob = next(a["address"] for a in info["accounts"] if a["label"] == "bob")
        ra = send("alice", n1, "0x" + SEL["addPublisher"] + "%064x" % int(bob, 16))
        ok("owner adds bob as a publisher", ra["status"] == "0x1")
        da, db = os.path.join(work, "pa"), os.path.join(work, "pb"); site_files(da, "A"); site_files(db, "B")
        open(os.path.join(da, "empty.txt"), "wb").write(b"")
        procs = [subprocess.Popen([SITE_BIN, "publish", "--rpc", "127.0.0.1:8540", "--chain-id", cid, "--account", acct, "--dir", d, "--site", site], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True) for acct, d in (("alice", da), ("bob", db))]
        outs = [p.communicate() for p in procs]
        ok("two concurrent publishers both succeed", all(p.returncode == 0 for p in procs), " | ".join(o[1][-120:] for o in outs if o[1]))
        res = [json.loads(o[0]) for o in outs]
        def version_hash(v): return "0x" + dn.rpc(8540, "eth_call", [{"to": site, "data": "0x" + SEL["version"] + "%064x" % v}, "latest"])[2:66]
        ok("each publisher's reported version holds exactly its own manifest", res[0]["version"] != res[1]["version"] and all(version_hash(x["version"]) == x["manifestHash"] for x in res), json.dumps([(x["version"], x["manifestHash"][:12]) for x in res]))
        cur = dn.rpc(8540, "eth_call", [{"to": site, "data": "0x" + SEL["current"]}, "latest"])
        cv = int(cur[2:66], 16); last = next(x for x in res if x["version"] == cv) if any(x["version"] == cv for x in res) else None
        meta, got = fetch(0, site)
        ok("current version is one of them and fetches fully verified", last is not None and meta["version"] == cv)
        if last is res[0]: ok("empty file round-trips (size 0, no chunks)", got.get("/empty.txt") == b"")
        else:
            # make A current to check the empty file
            an = int(dn.rpc(8540, "eth_getTransactionCount", [next(a["address"] for a in info["accounts"] if a["label"] == "alice"), "pending"]), 16)
            send("alice", an, "0x" + SEL["setCurrent"] + "%064x" % res[0]["version"]); meta, got = fetch(0, site)
            ok("empty file round-trips (size 0, no chunks)", got.get("/empty.txt") == b"" and meta["version"] == res[0]["version"])
        want2 = fetch(0, site)[1]
        # Restart every node; fetch again from each.
        dn.stop(work); time.sleep(2); dn.start(work, log=True)
        wait(lambda: all(dn.rpc(8540 + i, "pocol_status") for i in range(3)), 60, "restart")
        for i in range(3): ok(f"after restart node {'abc'[i]} serves the current version unchanged", fetch(i, site)[1] == want2)
        print(json.dumps({"result": "PASS", "checks": n, "site": site, "work": work}))
        return 0
    except Exception as e:
        error = e
    finally:
        dn.stop(work)
    time.sleep(2)  # the nodes stop: their logs and journals are whole when they are kept
    kept = dn.keep_on_failure("e2e_site", work, {"error": str(error)[-500:], "passed": n})
    print(json.dumps({"result": "FAIL", "error": str(error)[-500:], "work": work, "kept": kept})); return 1

if __name__ == "__main__":
    sys.exit(main())
