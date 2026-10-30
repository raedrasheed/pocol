#!/usr/bin/env python3
"""Measured resident memory of a 3-node pocold devnet, against the A13 model (D-A13-BUDGET).

usage: python3 measure_rss.py OUT.json [--idle S] [--load S]

Starts the release devnet (same parameters as tests/e2e_devnet.py), lets it produce blocks for --idle
seconds, then applies ordinary and bounded adversarial RPC load for --load seconds:
  - 16 threads of light requests, 4 threads of 30M-gas eth_calls, 2 threads of 20-member heavy batches;
  - signed transfers (pocol-sign) submitted throughout.
Every second it samples VmRSS and VmHWM (/proc/<pid>/status) of each node. It reports the peak per node
next to the model total each node reports in `status.a13.boundBytes`.

What the figures are: RSS is what this run touched on this host. It is not the model bound (which
counts every cap at its maximum at once) and not a guaranteed ceiling (the model leaves allocator
overhead and fragmentation open; nothing in the process enforces a ceiling).
"""
import json, os, subprocess, sys, tempfile, threading, time, urllib.request
HERE = os.path.dirname(os.path.abspath(__file__)); ROOT = os.path.dirname(os.path.dirname(os.path.dirname(HERE)))
POCOLD = os.path.join(ROOT, "prototypes", "pocold")
sys.path.insert(0, os.path.join(POCOLD, "scripts"))
import devnet as dn

HEAVY = {"jsonrpc": "2.0", "id": 1, "method": "eth_call", "params": [{"data": "0x5b600056", "gas": "0x1c9c380"}, "latest"]}
LIGHT = {"jsonrpc": "2.0", "id": 2, "method": "eth_blockNumber", "params": []}

def post(port, body, timeout=60):
    req = urllib.request.Request(f"http://127.0.0.1:{port}/", data=json.dumps(body).encode(), headers={"content-type": "application/json"})
    try:
        with urllib.request.urlopen(req, timeout=timeout) as r: return r.status
    except urllib.error.HTTPError as e: return e.code
    except Exception: return 0

def mem(pid):
    out = {}
    for l in open(f"/proc/{pid}/status"):
        k, _, v = l.partition(":")
        if k in ("VmRSS", "VmHWM", "Threads"): out[k] = int(v.split()[0]) * (1024 if k != "Threads" else 1)
    return out

def main():
    a = sys.argv[1:]; out_path = a[0]
    idle = int(a[a.index("--idle") + 1]) if "--idle" in a else 60
    load = int(a[a.index("--load") + 1]) if "--load" in a else 60
    work = tempfile.mkdtemp(prefix="pocol-rss-")
    commit = subprocess.run(["git", "-C", ROOT, "rev-parse", "HEAD"], capture_output=True, text=True).stdout.strip()
    dirty = subprocess.run(["git", "-C", ROOT, "status", "--porcelain", "--", "prototypes"], capture_output=True, text=True).stdout.strip()
    dn.init(work, "a,b,c", ["--t-blk", "2", "--tau", "20", "--c", "8", "--d-att", "41", "--fb-wait", "3", "--work-bits", "15"])
    procs = dn.start(work)
    samples = {l: [] for l in procs}; stop = threading.Event(); phase = {"name": "idle"}
    def sampler():
        while not stop.is_set():
            for l, pid in procs.items():
                try: m = mem(pid); m["phase"] = phase["name"]; m["t"] = round(time.time(), 1); samples[l].append(m)
                except FileNotFoundError: pass
            time.sleep(1)
    threading.Thread(target=sampler, daemon=True).start()
    try:
        t = time.time()
        while time.time() - t < 60:
            try:
                if all(dn.rpc(8540 + i, "pocol_status")["head"]["height"] >= 1 for i in range(3)): break
            except Exception: pass
            time.sleep(0.5)
        time.sleep(idle)
        phase["name"] = "load"; done = threading.Event(); counts = {"light": 0, "heavy": 0, "batch": 0, "tx": 0}
        def loop(kind, body, port):
            while not done.is_set():
                code = post(port, body)
                if code == 200: counts[kind] += 1
        threads = [threading.Thread(target=loop, args=("light", LIGHT, 8540 + i % 3), daemon=True) for i in range(16)]
        threads += [threading.Thread(target=loop, args=("heavy", HEAVY, 8540 + i % 3), daemon=True) for i in range(4)]
        threads += [threading.Thread(target=loop, args=("batch", [HEAVY] * 20, 8540 + i % 3), daemon=True) for i in range(2)]
        for th in threads: th.start()
        info = json.load(open(os.path.join(work, "net", "devnet.json")))
        bob = next(x for x in info["accounts"] if x["label"] == "bob")
        sign = os.path.join(POCOLD, "target", "release", "pocol-sign")
        raws = subprocess.run([sign, "--chain-id", str(info["chainId"]), "--account", "alice", "--to", bob["address"], "--value", "1", "--count", "200"],
                              capture_output=True, text=True, check=True).stdout.split()
        t = time.time(); i = 0
        while time.time() - t < load:
            if i < len(raws):
                try: dn.rpc(8540, "eth_sendRawTransaction", [raws[i]]); counts["tx"] += 1
                except Exception: pass
                i += 1
            time.sleep(load / max(1, len(raws)))
        done.set(); phase["name"] = "after"; time.sleep(5)
        status = {}
        for k, l in enumerate(procs):
            try: s = dn.rpc(8540 + k, "pocol_status"); status[l] = {"height": s["head"]["height"], "peers": len(s["peers"]), "a13": s.get("a13")}
            except Exception as e: status[l] = {"error": str(e)[:80]}
    finally:
        stop.set(); dn.stop(work)
    nodes = {}
    for l, ss in samples.items():
        if not ss: continue
        hwm = max(s["VmHWM"] for s in ss); a13 = (status.get(l) or {}).get("a13") or {}
        bound = int(a13.get("boundBytes", 0)) if isinstance(a13, dict) else 0
        nodes[l] = {"peakRssBytes": max(s["VmRSS"] for s in ss), "vmHwmBytes": hwm, "maxThreads": max(s.get("Threads", 0) for s in ss),
                    "peakRssIdleBytes": max((s["VmRSS"] for s in ss if s["phase"] == "idle"), default=0),
                    "peakRssLoadBytes": max((s["VmRSS"] for s in ss if s["phase"] != "idle"), default=0),
                    "modelBoundBytes": bound, "hwmOverModel": round(hwm / bound, 4) if bound else None, "status": status.get(l)}
    res = {"commit": commit, "uncommittedPrototypePaths": dirty.splitlines(), "host": os.uname().release, "nproc": os.cpu_count(),
           "idleSeconds": idle, "loadSeconds": load, "requestsAnswered200": counts, "nodes": nodes,
           "note": "measured RSS of this run; not the model bound (all caps at once) and not a guaranteed ceiling"}
    json.dump(res, open(out_path, "w"), indent=1)
    print(json.dumps({l: {k: v for k, v in n.items() if k != "status"} for l, n in nodes.items()}, indent=1))

if __name__ == "__main__":
    main()
