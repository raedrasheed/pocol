#!/usr/bin/env python3
"""Local PoCol devnet launcher (LP10). Starts N pocold processes on loopback.

usage: devnet.py init  WORK --miners a,b,c [init-devnet flags...]
       devnet.py start WORK [--stop-at H] [--log] [--only a,b]
       devnet.py stop  WORK [--only a,b]
       devnet.py status WORK
       devnet.py rpc   WORK NODE METHOD [JSON_PARAMS]
Ports: p2p 30300+i, rpc 8540+i (i = node index). Data in WORK/data/<label>.
"""
import json, os, signal, subprocess, sys, time, urllib.request
ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
BIN = os.environ.get("POCOLD", os.path.join(ROOT, "target", "release", "pocold"))
# Extra arguments for every `pocold run` and `pocold status` (experiments, e.g. POCOLD_RUN_ARGS=--state-store).
EXTRA = os.environ.get("POCOLD_RUN_ARGS", "").split()

def labels(work): return [m["label"] for m in json.load(open(os.path.join(work, "net", "devnet.json")))["miners_order"]]

def rpc(port, method, params=None, timeout=10):
    body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": method, "params": params or []}).encode()
    req = urllib.request.Request(f"http://127.0.0.1:{port}/", data=body, headers={"content-type": "application/json"})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        v = json.load(r)
    if "error" in v: raise RuntimeError(v["error"])
    return v["result"]

def init(work, miners, extra):
    os.makedirs(work, exist_ok=True)
    out = subprocess.run([BIN, "init-devnet", "--out", os.path.join(work, "net"), "--miners", miners] + extra, check=True, capture_output=True, text=True).stdout
    info = json.load(open(os.path.join(work, "net", "devnet.json")))
    info["miners_order"] = [{"label": l} for l in miners.split(",")]
    json.dump(info, open(os.path.join(work, "net", "devnet.json"), "w"), indent=1)
    # Browser profile (browser.md D58/D77): the extension recomputes genesisHash from genesisPre.
    spec_hex = open(os.path.join(work, "net", "genesis.spec.hex")).read().strip()
    profile = {"name": "devnet", "chainId": info["chainId"], "genesisPre": spec_hex if spec_hex.startswith("0x") else "0x" + spec_hex,
               "genesisHash": info["genesisHash"], "endpoint": "http://127.0.0.1:8540", "trustLevel": "RP"}
    json.dump(profile, open(os.path.join(work, "net", "profile.json"), "w"), indent=1)
    print(out)

def start(work, stop_at=None, log=False, only=None):
    ls = labels(work); procs = {}
    for i, l in enumerate(ls):
        if only and l not in only: continue
        peers = sum([["--peer", f"127.0.0.1:{30300+j}"] for j in range(len(ls)) if j != i], [])
        cmd = [BIN, "run", "--network", os.path.join(work, "net"), "--data", os.path.join(work, "data", l), "--name", l, "--miner", l,
               "--listen", f"127.0.0.1:{30300+i}", "--rpc", f"127.0.0.1:{8540+i}"] + peers + EXTRA
        if stop_at: cmd += ["--stop-at", str(stop_at)]
        if log: cmd += ["--log"]
        logf = open(os.path.join(work, f"{l}.log"), "ab")
        p = subprocess.Popen(cmd, stdout=logf, stderr=logf, start_new_session=True)
        procs[l] = p.pid
    pidfile = os.path.join(work, "pids.json")
    old = json.load(open(pidfile)) if os.path.exists(pidfile) else {}
    old.update(procs); json.dump(old, open(pidfile, "w"))
    return procs

def stop(work, only=None, sig=signal.SIGTERM):
    pidfile = os.path.join(work, "pids.json")
    if not os.path.exists(pidfile): return
    pids = json.load(open(pidfile))
    for l, pid in list(pids.items()):
        if only and l not in only: continue
        try: os.kill(pid, sig)
        except ProcessLookupError: pass
        pids.pop(l)
    json.dump(pids, open(pidfile, "w"))

def keep_on_failure(suite, work, extra=None):
    """A failed run's evidence (2026-10-28), kept before anything cleans the work directory: every node log and
    input under WORK (net/ with the genesis, profile and keys; site files), each node's blocks.journal (its blocks,
    from which replay rebuilds the derived stores, I1) and `extra` (the run's checks and the values it drew;
    net/devnet.json holds the genesis time and chain id). The derived stores (*.redb) are listed, not copied. Also
    the commit, pocold's SHA-256 and the environment that changes a run. Into POCOL_E2E_KEEP or tests/failures/
    (ignored by git), with manifest.json (every file's size and SHA-256). Returns the directory."""
    import hashlib, shutil, re, datetime
    sha = lambda f: hashlib.sha256(open(f, "rb").read()).hexdigest()
    stamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%dT%H%M%S%fZ")
    dest = os.path.join(os.environ.get("POCOL_E2E_KEEP") or os.path.join(ROOT, "tests", "failures"), f"{suite}-{stamp}-{os.getpid()}")
    files, skipped = [], []
    for base, _, names in os.walk(work):
        for n in names:
            src = os.path.join(base, n); rel = os.path.relpath(src, work)
            if not os.path.isfile(src): continue
            if re.search(r"\.redb(\.aside|\.damaged)?$", n): skipped.append({"path": rel, "bytes": os.path.getsize(src)}); continue
            d = os.path.join(dest, "work", rel); os.makedirs(os.path.dirname(d), exist_ok=True); shutil.copyfile(src, d)
            files.append({"path": rel, "bytes": os.path.getsize(d), "sha256": sha(d)})
    os.makedirs(dest, exist_ok=True)
    try: commit = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True).stdout.strip() or None
    except OSError: commit = None
    env = {k: os.environ[k] for k in ("POCOLD", "POCOLD_RUN_ARGS", "POCOL_SIGN", "POCOL_E2E_KEEP") if k in os.environ}  # named one by one (review F9)
    json.dump({"suite": suite, "work": work, "keptAt": stamp, "commit": commit, "pocold": {"path": BIN, "sha256": sha(BIN) if os.path.exists(BIN) else None},
               "env": env, "files": files, "notCopied": skipped, "extra": extra or {}}, open(os.path.join(dest, "manifest.json"), "w"), indent=1)
    print(f"kept the failed run's logs and inputs in {dest}", flush=True); return dest

def status(work):
    out = {}
    for i, l in enumerate(labels(work)):
        try: s = rpc(8540+i, "pocol_status"); out[l] = {"height": s["head"]["height"], "hash": s["head"]["hash"][:14], "peers": len(s["peers"]), "prod": s["production"], "stats": s["stats"]}
        except Exception as e: out[l] = {"down": str(e)[:60]}
    return out

if __name__ == "__main__":
    a = sys.argv
    if a[1] == "init": init(a[2], a[a.index("--miners")+1], [x for x in a[3:] if x not in ("--miners", a[a.index("--miners")+1])])
    elif a[1] == "start": print(start(a[2], int(a[a.index("--stop-at")+1]) if "--stop-at" in a else None, "--log" in a, a[a.index("--only")+1].split(",") if "--only" in a else None))
    elif a[1] == "stop": stop(a[2], a[a.index("--only")+1].split(",") if "--only" in a else None)
    elif a[1] == "status": print(json.dumps(status(a[2]), indent=1))
    elif a[1] == "rpc":
        i = labels(a[2]).index(a[3]); print(json.dumps(rpc(8540+i, a[4], json.loads(a[5]) if len(a) > 5 else []), indent=1))
