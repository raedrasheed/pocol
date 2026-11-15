#!/usr/bin/env python3
"""LP8 / Sites W10 end-to-end: publish websites into PoCol state with the Rust CLI on a v3 devnet (specVersion 2, the
V3 programs, nonceMode 1, continuous: `devnet.py init`) and fetch them back from every node, also after a restart.

    python3 tests/e2e_site.py [--profile pr|nightly]      (or SITES_E2E_PROFILE; default pr)

pr (every PR, about 1 MiB of chunks): the W10 site (tests/w10_site.mjs: HTML, CSS with a woff2 @font-face, a classic
script, an ES module graph main.mjs -> lib.mjs -> util.mjs with a literal import(), JSON, SVG, PNG) with a file of
exactly 1,048,576 B; 1,048,577 B, 4 MiB + 1 B and 257 files refused before any signature; 256 files with a manifest
spilling into 3 chunks accepted; gas per chunk, at most one full chunk per block, idempotent re-store; the base fee
follows the London recurrence and stays within the W4 publisher's simulated schedule (tests via
pocol-extension/src/lib/fees.js), no "fee rose" refusal, the spend within the simulated budget; economy pacing keeps
the base fee at its floor; no charge beyond gas (FN-2a); setCurrent and revoke; concurrent publishers; a chunk stored by
another account and reorganized out before `safe` is stored again by the safe sweep (a partitioned devnet); digests
identical on all 3 nodes before and after a restart; CODE_CAP and ACCT_CAP give a clear CLI error (small-cap devnets).
nightly adds: a site of 4 MiB - 4 KiB (about 171 chunks) accepted, with the same fee checks over its whole trajectory.
"""
import json, os, re, shutil, signal, socket, subprocess, sys, tempfile, threading, time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import urllib.request
HERE = os.path.dirname(os.path.abspath(__file__)); ROOT = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(ROOT, "scripts")); import devnet as dn
BIN = os.environ.get("POCOLD_BIN_DIR") or os.path.join(ROOT, "target", "release")
SITE_BIN = os.path.join(BIN, "pocol-site"); SIGN = os.path.join(BIN, "pocol-sign")
EXT = os.path.join(ROOT, "..", "pocol-extension")
W10_SITE = os.path.join(EXT, "tests", "w10_site.mjs"); FEES_JS = os.path.join(EXT, "src", "lib", "fees.js")
SEL = {k.split("(")[0]: v for k, v in json.load(open(os.path.join(ROOT, "..", "lp4-execution", "tests", "data", "website.json")))["methodIdentifiers"].items()}
CHUNK_MAX, FILE_MAX, SITE_MAX, FILES_MAX, MANIFEST_MAX = 24575, 1 << 20, 4 << 20, 256, 65536
GWEI, FLOOR = 10**9, 10**7          # the v3 baseFeeMin (FRESH-NETWORK-CP row 55)
FACTORY = "0x0000000000000000000000000000000000c0c005"
DEVNET_ARGS = ["--t-blk", "2", "--tau", "20", "--c", "8", "--d-att", "41", "--fb-wait", "3", "--work-bits", "15"]

# ---------------- keccak-256 (for chunk addresses and selectors; the standard library has only SHA3) ----------------
_RC = [0x0000000000000001, 0x0000000000008082, 0x800000000000808A, 0x8000000080008000, 0x000000000000808B, 0x0000000080000001, 0x8000000080008081, 0x8000000000008009,
       0x000000000000008A, 0x0000000000000088, 0x0000000080008009, 0x000000008000000A, 0x000000008000808B, 0x800000000000008B, 0x8000000000008089, 0x8000000000008003,
       0x8000000000008002, 0x8000000000000080, 0x000000000000800A, 0x800000008000000A, 0x8000000080008081, 0x8000000000008080, 0x0000000080000001, 0x8000000080008008]
_ROT = [[0, 36, 3, 41, 18], [1, 44, 10, 45, 2], [62, 6, 43, 15, 61], [28, 55, 25, 21, 56], [27, 20, 39, 8, 14]]
def keccak256(data):
    M = (1 << 64) - 1; rol = lambda v, n: ((v << n) | (v >> (64 - n))) & M if n else v
    s = [[0] * 5 for _ in range(5)]; rate = 136
    p = bytearray(data) + b"\x01"; p += b"\x00" * (-len(p) % rate); p[-1] |= 0x80
    for off in range(0, len(p), rate):
        for i in range(rate // 8): s[i % 5][i // 5] ^= int.from_bytes(p[off + 8 * i: off + 8 * i + 8], "little")
        for rc in _RC:
            c = [s[x][0] ^ s[x][1] ^ s[x][2] ^ s[x][3] ^ s[x][4] for x in range(5)]
            d = [c[(x - 1) % 5] ^ rol(c[(x + 1) % 5], 1) for x in range(5)]
            s = [[s[x][y] ^ d[x] for y in range(5)] for x in range(5)]
            b = [[0] * 5 for _ in range(5)]
            for x in range(5):
                for y in range(5): b[y][(2 * x + 3 * y) % 5] = rol(s[x][y], _ROT[x][y])
            s = [[b[x][y] ^ ((~b[(x + 1) % 5][y]) & b[(x + 2) % 5][y]) for y in range(5)] for x in range(5)]
            s[0][0] ^= rc
    return b"".join(s[i % 5][i // 5].to_bytes(8, "little") for i in range(4))
assert keccak256(b"").hex() == "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
def chunk_address(data):
    init = b"\x61" + (1 + len(data)).to_bytes(2, "big") + bytes([0x80, 0x60, 0x0a, 0x5f, 0x39, 0x5f, 0xf3, 0x00]) + data
    return "0x" + keccak256(b"\xff" + bytes.fromhex(FACTORY[2:]) + keccak256(data) + keccak256(init))[12:].hex()

# ---------------- manifest size (the encoding's length depends only on lengths, not on hash values) ----------------
def _rlp_len(payload): return payload + (1 if payload < 56 else 1 + (payload.bit_length() + 7) // 8)
def _int_len(v): return 1 if v < 128 else _rlp_len((v.bit_length() + 7) // 8) if v else 1
def manifest_len(files):
    """files: {path: size}; RLP([1, entry, [[path, mime, size, hash32, [[addr20, len]...]]...]]) as site.rs encodes it."""
    items = 0
    for p, size in sorted(files.items()):
        chunks = [min(CHUNK_MAX, size - o) for o in range(0, size, CHUNK_MAX)]
        cl = sum(_rlp_len(21 + _int_len(c)) for c in chunks)
        f = _rlp_len(len(p)) if len(p) != 1 else 1
        items += _rlp_len(f + 1 + _int_len(size) + 33 + _rlp_len(cl))
    entry = sorted(files).index("/index.html")
    return _rlp_len(1 + _int_len(entry) + _rlp_len(items))

def wait(pred, timeout, what):
    t = time.time(); last = None
    while time.time() - t < timeout:
        try:
            v = pred()
            if v: return v
        except Exception as e: last = e
        time.sleep(0.5)
    raise AssertionError("timeout: " + what + (f" ({last})" if last else ""))
q = lambda v: int(v, 16)

def write_w10(d, **kw):
    a = ["node", W10_SITE, d]
    for k, v in kw.items():
        if v is True: a.append("--" + k.replace("_", "-"))
        elif v is not False and v is not None: a += ["--" + k.replace("_", "-"), str(v)]
    subprocess.run(a, check=True, capture_output=True, text=True)
    return {"/" + os.path.relpath(os.path.join(b, f), d).replace(os.sep, "/"): open(os.path.join(b, f), "rb").read() for b, _, fs in os.walk(d) for f in fs}

def fetch(port, site):
    out = tempfile.mkdtemp(prefix="pocol-fetch-")
    try:
        r = subprocess.run([SITE_BIN, "fetch", "--rpc", f"127.0.0.1:{port}", "--site", site, "--out", out], capture_output=True, text=True)
        if r.returncode: raise AssertionError(r.stderr)
        meta = json.loads(r.stdout); got = {f["path"]: open(os.path.join(out, f["path"][1:]), "rb").read() for f in meta["files"]}
        return meta, got
    finally: shutil.rmtree(out, ignore_errors=True)
def digest(got): return keccak256(b"".join(p.encode() + b"\0" + got[p] for p in sorted(got))).hex()[:16]

def simulate(txs, base_next, tip, economy=False):
    """The W4 publisher's schedule (pocol-extension/src/lib/fees.js feeSchedule) for txs [{id, gas, gasUsed}]."""
    js = ("import { feeSchedule } from " + json.dumps("file://" + os.path.abspath(FEES_JS)) + ";"
          "let s='';process.stdin.on('data',c=>s+=c).on('end',()=>{const a=JSON.parse(s);"
          "console.log(JSON.stringify(feeSchedule({txs:a.txs,baseNext:a.baseNext,tip:a.tip,economy:a.economy})))});")
    r = subprocess.run(["node", "--input-type=module", "-e", js], input=json.dumps({"txs": txs, "baseNext": str(base_next), "tip": str(tip), "economy": economy}), capture_output=True, text=True, check=True)
    return json.loads(r.stdout)
def london(parent, used, limit, floor=FLOOR):
    """lp4-execution/src/fees.rs next_base_fee (fees.js nextBaseFee)."""
    target = limit // 2
    if used == target: return max(parent, floor)
    dist = abs(used - target); delta = parent * dist // target // 8
    fee = parent + max(delta, 1) if used > target else parent - delta
    return max(fee, floor)
def plan_txs(files, new_site, existing=frozenset()):
    """The CLI's transaction sequence (site.rs Publisher::publish): every missing chunk of every file in path order, the
    manifest chunks, the Website deploy for a new site, createVersion, publish, setCurrent."""
    txs, seen = [], set(existing)
    def chunk(c):
        a = chunk_address(c)
        if a in seen: return
        seen.add(a); txs.append({"id": "chunk:" + a, "gas": str(min(200000 + 260 * len(c), 7900000)), "gasUsed": str(70000 + 218 * len(c)), "size": len(c)})
    for p in sorted(files):
        d = files[p]
        for o in range(0, len(d), CHUNK_MAX): chunk(d[o:o + CHUNK_MAX])
    ml = manifest_len({p: len(d) for p, d in files.items()})
    for o in range(0, ml, CHUNK_MAX): txs.append({"id": f"manifest:{o}", "gas": str(200000 + 260 * min(CHUNK_MAX, ml - o)), "gasUsed": str(70000 + 218 * min(CHUNK_MAX, ml - o)), "size": min(CHUNK_MAX, ml - o)})
    if new_site: txs.append({"id": "deploy", "gas": "1500000"})
    txs += [{"id": "createVersion", "gas": "600000"}, {"id": "publish", "gas": "200000"}, {"id": "setCurrent", "gas": "200000"}]
    return txs, ml

class RpcProxy:
    """A JSON-RPC relay for the CLI: forwards each request to `upstream` (switchable), and can hold every
    eth_getCode at `safe` until released (the safe sweep's first read), so a test can reorganize the chain under it."""
    def __init__(self, port, upstream):
        self.upstream, self.hold, self.held, self.log = upstream, False, threading.Event(), []
        self.release = threading.Event(); proxy = self
        class H(BaseHTTPRequestHandler):
            def log_message(self, *a): pass
            def do_POST(self):
                body = self.rfile.read(int(self.headers.get("Content-Length", "0")))
                try: m = json.loads(body)
                except ValueError: m = {}
                proxy.log.append((m.get("method"), (m.get("params") or [None, None])[1:2]))
                if proxy.hold and m.get("method") == "eth_getCode" and (m.get("params") or [None, None])[1] == "safe":
                    proxy.held.set(); proxy.release.wait(25)
                try:
                    req = urllib.request.Request(f"http://127.0.0.1:{proxy.upstream}/", data=body, headers={"content-type": "application/json"})
                    with urllib.request.urlopen(req, timeout=30) as r: out = r.read()
                    self.send_response(200)
                except Exception as e:
                    out = json.dumps({"jsonrpc": "2.0", "id": 1, "error": {"code": -32000, "message": "proxy: " + str(e)}}).encode(); self.send_response(502)
                self.send_header("Content-Type", "application/json"); self.send_header("Content-Length", str(len(out))); self.send_header("Connection", "close"); self.end_headers(); self.wfile.write(out)
        self.server = ThreadingHTTPServer(("127.0.0.1", port), H); self.server.daemon_threads = True
        threading.Thread(target=self.server.serve_forever, daemon=True).start()
    def close(self): self.release.set(); self.server.shutdown(); self.server.server_close()

def main():
    profile = os.environ.get("SITES_E2E_PROFILE", "pr")
    if "--profile" in sys.argv: profile = sys.argv[sys.argv.index("--profile") + 1]
    if profile not in ("pr", "nightly"): raise SystemExit("profile must be pr or nightly")
    for port in [30300, 30301, 30302, 8540, 8541, 8542, 8549]:
        with socket.socket() as t:
            if t.connect_ex(("127.0.0.1", port)) == 0: raise SystemExit(f"port {port} busy")
    work = tempfile.mkdtemp(prefix="pocol-site-"); n = 0; extra_procs = []; records = []; error = None; proxy = None
    def ok(name, cond, d=""):
        nonlocal n; print(("PASS " if cond else "FAIL ") + name + (f" — {d}" if d != "" else ""), flush=True)
        records.append({"name": name, "pass": bool(cond), "detail": str(d)[:300]})
        if not cond: raise AssertionError(name + (f" — {d}" if d != "" else ""))
        n += 1
    rpc = lambda i, m, p=None: dn.rpc(8540 + i, m, p)
    def run_cli(args, port=8540, account="alice", timeout=1800):
        r = subprocess.run([SITE_BIN, "publish", "--rpc", f"127.0.0.1:{port}", "--chain-id", cid, "--account", account] + args, capture_output=True, text=True, timeout=timeout)
        return r
    def addr(label): return next(a["address"] for a in info["accounts"] if a["label"] == label)
    def nonce(label, i=0): return q(rpc(i, "eth_getTransactionCount", [addr(label), "pending"]))
    def balance(label): return q(rpc(0, "eth_getBalance", [addr(label), "latest"]))
    def send(account, data, to=None, gas=300000, i=0):
        raw = subprocess.run([SIGN, "--chain-id", cid, "--account", account, "--to", to or site, "--nonce", str(nonce(account, i)), "--gas", str(gas), "--data", data], capture_output=True, text=True, check=True).stdout.strip()
        h = rpc(i, "eth_sendRawTransaction", [raw]); wait(lambda: rpc(i, "eth_getTransactionReceipt", [h]) is not None, 120, "receipt")
        return rpc(i, "eth_getTransactionReceipt", [h])
    def call(target, data, i=0): return rpc(i, "eth_call", [{"to": target, "data": data}, "latest"])
    def head(i=0): return q(rpc(i, "eth_blockNumber"))
    def blocks(lo, hi, i=0): return [rpc(i, "eth_getBlockByNumber", [hex(b), True]) for b in range(lo, hi + 1)]
    def full_creations(bl):
        """{block: full chunks created in it}: ChunkFactory stores of CHUNK_MAX bytes that created code (a store of a chunk
        that already exists returns it at a small cost; ChunkFactory.sol), over all senders."""
        out = {}
        for blk in bl:
            for tx in blk["transactions"]:
                if tx["to"] and tx["to"].lower() == FACTORY and q(tx["input"][2 + 8 + 64:2 + 8 + 128]) == CHUNK_MAX and q(rpc(0, "eth_getTransactionReceipt", [tx["hash"]])["gasUsed"]) > 4000000:
                    out[q(blk["number"])] = out.get(q(blk["number"]), 0) + 1
        return out
    def wait_floor():
        """Until the base fee is at its floor and the chain idle (the genesis base fee falls 12.5 % per empty block)."""
        wait(lambda: q(rpc(0, "eth_getBlockByNumber", ["latest", False])["baseFeePerGas"]) == FLOOR, 300, "base fee at the floor")
    def chain_publish(name, files, d, extra=(), new_site=True, check_budget=True):
        """Publishes `d` with the budget and ceiling of the W4 simulation, then checks the run block by block."""
        # Chunks already on the chain (e.g. files an earlier version shares) are not stored again.
        existing = {chunk_address(c[o:o + CHUNK_MAX]) for c in files.values() for o in range(0, len(c), CHUNK_MAX)}
        existing = {a for a in existing if rpc(0, "eth_getCode", [a, "latest"]) != "0x"}
        txs, ml = plan_txs(files, new_site, existing)
        tip = q(rpc(0, "eth_maxPriorityFeePerGas")); base_next = q(rpc(0, "eth_gasPrice")) - tip
        sim = simulate([{k: t[k] for k in ("id", "gas", "gasUsed") if k in t} for t in txs], base_next, tip, economy="--economy" in extra)
        # The CLI signs maxFee = 2 * base_next + tip just in time (site.rs derive_fee): its budget over the simulated
        # trajectory, and the simulation's ceiling (twice its largest cap).
        cli_budget = sum(int(t["gas"]) * (2 * int(s["base"]) + tip) for t, s in zip(txs, sim["schedule"]))
        fee_args = ["--max-total-pcol", f"{cli_budget // 10**18}.{cli_budget % 10**18:018d}", "--max-fee-gwei", f"{int(sim['ceilingWei']) // GWEI}.{int(sim['ceilingWei']) % GWEI:09d}"] if check_budget else []
        b0, n0, h0 = balance("alice"), nonce("alice"), head()
        t0 = time.time(); r = run_cli(["--dir", d] + list(extra) + fee_args)
        ok(f"{name}: published ({len(files)} files, {sum(map(len, files.values()))} B, {len(txs)} planned transactions) in {time.time() - t0:.0f} s, no fee-rose refusal",
           r.returncode == 0 and "fee rose" not in r.stderr, r.stderr[-400:] if r.returncode else r.stdout.strip()[:240])
        out = json.loads(r.stdout); h1 = head(); b1, n1 = balance("alice"), nonce("alice")
        ok(f"{name}: the CLI signed exactly the planned transactions (nonce {n0} -> {n1}) and the manifest is {ml} B in {-(-ml // CHUNK_MAX)} chunk(s)", n1 - n0 == len(txs) and out["manifestBytes"] == ml, f"{n1 - n0} vs {len(txs)}; manifest {out['manifestBytes']} vs {ml}")
        bl = blocks(max(h0 - 1, 0), h1); mine = []
        for blk in bl:
            for tx in blk["transactions"]:
                if tx["from"].lower() == addr("alice").lower() and q(tx["nonce"]) >= n0: mine.append((blk, tx, rpc(0, "eth_getTransactionReceipt", [tx["hash"]])))
        mine.sort(key=lambda x: q(x[1]["nonce"]))
        ok(f"{name}: every planned transaction is on the chain in order, each succeeded", len(mine) == len(txs) and all(m[2]["status"] == "0x1" for m in mine), f"{len(mine)} of {len(txs)}")
        sizes = []
        for (blk, tx, rc), t in zip(mine, txs):
            if t["id"].startswith(("chunk:", "manifest:")):
                size = q(tx["input"][2 + 8 + 64:2 + 8 + 128]); sizes.append(size)
                if not (tx["to"].lower() == FACTORY and size == t["size"] and q(rc["gasUsed"]) <= 200000 + 260 * size and q(tx["gas"]) == int(t["gas"])):
                    ok(f"{name}: chunk store {tx['hash']} within its declared gas", False, f"size {size}/{t['size']} used {q(rc['gasUsed'])} gas {q(tx['gas'])}")
        full = full_creations(bl)
        ok(f"{name}: each chunk store used at most its declared 200000 + 260 * size gas ({len(sizes)} stores, largest gasUsed {max((q(m[2]['gasUsed']) for m in mine if m[1]['to'] and m[1]['to'].lower() == FACTORY), default=0)}); at most one full chunk per block ({sum(full.values())} full chunks in {len(full)} blocks)",
           len(sizes) == sum(1 for t in txs if t["id"].startswith(("chunk:", "manifest:"))) and all(v <= 1 for v in full.values()))
        # The base fee of every block follows the London recurrence of its parent (fees.rs, with the floor).
        bad = [q(b["number"]) for p, b in zip(bl, bl[1:]) if q(b["baseFeePerGas"]) != london(q(p["baseFeePerGas"]), q(p["gasUsed"]), q(p["gasLimit"]))]
        ok(f"{name}: the base fee of each of {len(bl) - 1} blocks follows London from its parent (floor {FLOOR})", not bad, bad[:5])
        over = [(i, q(m[0]["baseFeePerGas"]), int(s["base"])) for i, (m, s) in enumerate(zip(mine, sim["schedule"])) if q(m[0]["baseFeePerGas"]) > -(-int(s["base"]) * 1125 // 1000)]
        top = max(q(m[0]["baseFeePerGas"]) for m in mine); sim_top = max(int(s["base"]) for s in sim["schedule"])
        ratio = max(q(m[0]["baseFeePerGas"]) / int(s["base"]) for m, s in zip(mine, sim["schedule"]))
        ok(f"{name}: the base fee at each transaction stays within the W4 simulation's margin (12.5 %): at most {ratio:.4f} x the simulated base; it rose from {q(mine[0][0]['baseFeePerGas'])} to {top} wei (simulated peak {sim_top})", not over, over[:4])
        spent = sum(q(rc["gasUsed"]) * q(rc["effectiveGasPrice"]) for _, _, rc in mine)
        eff = [(q(rc["effectiveGasPrice"]), q(blk["baseFeePerGas"]) + min(q(tx["maxPriorityFeePerGas"]), q(tx["maxFeePerGas"]) - q(blk["baseFeePerGas"]))) for blk, tx, rc in mine]
        ok(f"{name}: the account paid exactly gas: balance fell by sum(gasUsed * effectiveGasPrice) = {spent} wei, each price = base + min(tip, maxFee - base) (no burn beyond gas, FN-2a)",
           b0 - b1 == spent and all(a == b for a, b in eff), f"{b0 - b1} vs {spent}")
        if check_budget:
            ok(f"{name}: committed {out['committedFeeWei']} wei <= the CLI budget {cli_budget}; spent {spent} <= the W4 approval budget {sim['budgetWei']}", int(out["committedFeeWei"]) <= cli_budget and spent <= int(sim["budgetWei"]))
        return out, mine, bl

    try:
        # A devnet's CODE_CAP is 2 MiB (D-A13-PROFILE caps/64): the nightly 4 MiB site, after the pr cases, needs a larger
        # one (the network profile's own cap is not a devnet's). The pr profile keeps the default.
        dn.init(work, "a,b,c", DEVNET_ARGS + (["--code-cap", str(16 << 20)] if profile == "nightly" else []))
        info = json.load(open(os.path.join(work, "net", "devnet.json"))); cid = str(info["chainId"])
        dn.start(work, log=True); wait(lambda: min(head(i) for i in range(3)) >= 3, 120, "nodes up")
        caps = rpc(0, "pocol_getCaps")
        ok("the devnet is v3: specVersion 2, the V3 programs, nonceMode 1, continuous (devnet.json, pocol_getCaps, pocol_getParams)",
           info.get("specVersion") == 2 and info.get("v3") is True and info.get("fullNonce") is True and caps["v3"] is True and caps["cumulative"] is False and rpc(0, "pocol_getParams")["nonceMode"] == 1, json.dumps({k: info.get(k) for k in ("specVersion", "v3", "fullNonce")}))
        modes = [rpc(i, "pocol_status")["stateStore"] for i in range(3)]
        ok("every node runs in the requested state mode", modes == ["--state-store" in dn.EXTRA] * 3, f"stateStore={modes} (POCOLD_RUN_ARGS)")
        wait_floor()

        # ---- v1: the W10 site with a file of exactly SITE_FILE_MAX (1,048,576 B) ----
        s1 = os.path.join(work, "site1"); want1 = write_w10(s1, version=1, max_file=True)
        ok("a file of exactly 1,048,576 B is in the site", len(want1["/max.txt"]) == FILE_MAX)
        pub, _, _ = chain_publish("v1 (W10 site + 1 MiB file)", want1, s1)
        site = pub["site"]
        for i in range(3):
            wait(lambda: fetch(8540 + i, site)[0]["version"] == 0, 90, f"v1 on node {i}")
            meta, got = fetch(8540 + i, site)
            ok(f"node {'abc'[i]} serves v1 with verified chunks, manifest and content hashes ({len(got)} files)", got == want1, f"entry {meta['entry']}")
        code = [rpc(i, "eth_getCode", [site, "latest"]) for i in range(3)]
        ok("Website runtime code identical on all nodes", len(set(code)) == 1 and len(code[0]) > 100)
        # Idempotent re-store: publishing the same files again stores nothing (every chunk exists).
        n0 = nonce("alice"); r = run_cli(["--dir", s1, "--site", site])
        ok("publishing v1 again stores no chunk: only createVersion, publish and setCurrent are signed", r.returncode == 0 and nonce("alice") - n0 == 3 and json.loads(r.stdout)["manifestHash"] == pub["manifestHash"], f"{nonce('alice') - n0} transactions; {r.stderr[-200:]}")
        # ---- v2 through node b: only the changed files' chunks are stored ----
        s2 = os.path.join(work, "site2"); want2 = write_w10(s2, version=2, max_file=True)
        ok("v2 keeps the 1 MiB file's bytes (its chunks are reused)", want2["/max.txt"] == want1["/max.txt"])
        changed = {p for p in want2 if want2[p] != want1.get(p)}
        n0 = nonce("alice"); r = run_cli(["--dir", s2, "--site", site], port=8541)
        txs2, _ = plan_txs(want2, False, {chunk_address(d[o:o + CHUNK_MAX]) for d in want1.values() for o in range(0, len(d), CHUNK_MAX)})
        ok(f"publish v2 through node b: only the {len(changed)} changed files' chunks and the manifest are stored ({len(txs2)} transactions)", r.returncode == 0 and nonce("alice") - n0 == len(txs2), r.stderr[-300:])
        v2 = json.loads(r.stdout)["version"]
        for i in range(3):
            wait(lambda: fetch(8540 + i, site)[0]["version"] == v2, 90, f"v2 on {i}")
            ok(f"node {'abc'[i]} serves v2", fetch(8540 + i, site)[1] == want2)
        # ---- setCurrent and revoke (Website v2 versioning) ----
        cur = lambda: q(call(site, "0x" + SEL["current"])[2:66])
        rs = send("alice", "0x" + SEL["setCurrent"] + "%064x" % 0)
        wait(lambda: all(fetch(8540 + i, site)[0]["version"] == 0 for i in range(3)), 90, "setCurrent(0) everywhere")
        ok("setCurrent(0): every node serves v1 again (earlier versions stay readable)", rs["status"] == "0x1" and cur() == 0 and all(fetch(8540 + i, site)[1] == want1 for i in range(3)))
        rv = send("alice", "0x" + SEL["revoke"] + "%064x" % v2)
        status_of = lambda v: q(call(site, "0x" + SEL["version"] + "%064x" % v)[2 + 128:2 + 192])
        rr = send("alice", "0x" + SEL["setCurrent"] + "%064x" % v2)
        ok(f"revoke({v2}) of a version that is not current succeeds and marks it revoked; setCurrent({v2}) then reverts", rv["status"] == "0x1" and status_of(v2) != 1 and rr["status"] == "0x0" and cur() == 0, f"status {status_of(v2)} setCurrent {rr['status']}")
        def storage(i): return [rpc(i, "eth_getStorageAt", [site, hex(k), "latest"]) for k in range(6)]
        before = storage(0)
        r1 = send("bob", "0x" + SEL["publish"] + "%064x" % 0)
        r2 = send("alice", "0x" + SEL["revoke"] + "%064x" % 0)
        r3 = send("alice", "0x" + SEL["createVersion"] + "00" * 32 + "%064x" % 70000 + "%064x" % 96 + "%064x" % 0)
        ok("unauthorized publish / revoke(current) / oversize manifest all revert", (r1["status"], r2["status"], r3["status"]) == ("0x0", "0x0", "0x0"), f"{r1['status']} {r2['status']} {r3['status']}")
        ok("reverted operations leave Website storage unchanged", storage(0) == before)
        rs = send("alice", "0x" + SEL["setCurrent"] + "%064x" % 1)
        ok("setCurrent(1) (the re-published v1 manifest)", rs["status"] == "0x1" and cur() == 1)

        # ---- limits refused before any signature (the nonce does not move) ----
        def refused(name, files, needle):
            d = os.path.join(work, "refuse-" + re.sub(r"\W+", "-", name)); os.makedirs(d)
            for p, c in files.items():
                f = os.path.join(d, p.lstrip("/")); os.makedirs(os.path.dirname(f), exist_ok=True); open(f, "wb").write(c)
            n0 = nonce("alice"); r = run_cli(["--dir", d, "--site", site])
            ok(f"{name} refused before any transaction (nonce unchanged)", r.returncode != 0 and needle in r.stderr and nonce("alice") == n0, r.stderr.strip()[-120:])
            shutil.rmtree(d)
        idx = {"/index.html": b"<!doctype html><p>limits"}
        refused("a file of 1,048,577 B", {**idx, "/big.txt": b"b" * (FILE_MAX + 1)}, "size 0..1 MiB")
        refused("a site of 4 MiB + 1 B", {**idx, **{f"/f{k}.txt": b"x" * FILE_MAX for k in range(3)}, "/last.txt": b"y" * (SITE_MAX - 3 * FILE_MAX - len(idx["/index.html"]) + 1)}, "site > 4 MiB")
        refused("257 files", {**idx, **{f"/f{k:03d}.txt": b"z" for k in range(256)}}, "1..256 files")
        # ---- 256 files, the manifest spilling into 3 chunks ----
        # Long paths make each manifest entry about 220 B: 256 entries are 49,151..65,536 B, three chunks.
        many = {"/index.html": b"<!doctype html><p>256 files"}
        for k in range(255): many[f"/d/f{k:03d}-" + "p" * 140 + ".txt"] = b"shared chunk"
        ml = manifest_len({p: len(c) for p, c in many.items()})
        ok("the 256-file manifest needs 3 chunks (49,151..65,536 B)", 2 * CHUNK_MAX < ml <= MANIFEST_MAX, ml)
        dm = os.path.join(work, "many")
        for p, c in many.items():
            f = os.path.join(dm, p.lstrip("/")); os.makedirs(os.path.dirname(f), exist_ok=True); open(f, "wb").write(c)
        pm, _, _ = chain_publish("256 files", many, dm)
        site256 = pm["site"]
        ok("256 files accepted; the manifest is stored as 3 chunks", pm["files"] == 256 and -(-pm["manifestBytes"] // CHUNK_MAX) == 3, pm["manifestBytes"])
        wait(lambda: all(fetch(8540 + i, site256)[0]["version"] == 0 for i in range(3)), 90, "256 files everywhere")
        d256 = {digest(fetch(8540 + i, site256)[1]) for i in range(3)}
        ok("every node serves the 256 files with verified hashes (one digest)", len(d256) == 1 and fetch(8540, site256)[1] == many, d256)
        if profile == "nightly":
            # ---- 4 MiB - 4 KiB (about 171 chunks) accepted, with the fee checks over its whole trajectory ----
            wait_floor()
            base_bytes = sum(map(len, write_w10(os.path.join(work, "probe")).values())); shutil.rmtree(os.path.join(work, "probe"))
            sb = os.path.join(work, "big"); wantb = write_w10(sb, version=9, pad=SITE_MAX - 4096 - base_bytes, seed=3)
            total = sum(map(len, wantb.values()))
            ok(f"the large site is 4 MiB - 4 KiB ({total} B, {sum(-(-len(c) // CHUNK_MAX) for c in wantb.values())} chunks)", total == SITE_MAX - 4096)
            pb, _, _ = chain_publish("4 MiB - 4 KiB site", wantb, sb)
            wait(lambda: all(fetch(8540 + i, pb["site"])[0]["version"] == 0 for i in range(3)), 180, "large site everywhere")
            ok("every node serves the large site identically", len({digest(fetch(8540 + i, pb["site"])[1]) for i in range(3)}) == 1 and fetch(8541, pb["site"])[1] == wantb)
        # ---- economy pacing keeps the base fee at its floor ----
        wait_floor()
        se = os.path.join(work, "econ"); wante = write_w10(se, version=5, pad=8 * CHUNK_MAX if profile == "pr" else 24 * CHUNK_MAX, seed=5)
        pe, mine, bl = chain_publish("economy", wante, se, extra=["--economy"])
        hs = sorted({q(m[0]["number"]) for m in mine})
        peak = london(FLOOR, max(q(b["gasUsed"]) for b in bl), 8000000)
        ok(f"economy: one publication transaction every second block at most ({len(hs)} blocks), and the base fee never leaves [floor, one full block above it] ({FLOOR}..{peak} wei)",
           all(b - a >= 2 for a, b in zip(hs, hs[1:])) and all(FLOOR <= q(b["baseFeePerGas"]) <= peak for b in bl) and q(rpc(0, "eth_getBlockByNumber", ["latest", False])["baseFeePerGas"]) <= peak, [q(b["baseFeePerGas"]) for b in bl][:12])

        # ---- concurrent publishers (each with full chunks): both succeed, one full chunk per block ----
        ra = send("alice", "0x" + SEL["addPublisher"] + "%064x" % int(addr("bob"), 16))
        ok("owner adds bob as a publisher", ra["status"] == "0x1")
        da, db = os.path.join(work, "pa"), os.path.join(work, "pb")
        fa, fb = write_w10(da, version=11, pad=2 * CHUNK_MAX, seed=11), write_w10(db, version=12, pad=2 * CHUNK_MAX, seed=12)
        open(os.path.join(da, "empty.txt"), "wb").write(b""); fa["/empty.txt"] = b""
        h0 = head()
        procs = [subprocess.Popen([SITE_BIN, "publish", "--rpc", "127.0.0.1:8540", "--chain-id", cid, "--account", acct, "--dir", d, "--site", site], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True) for acct, d in (("alice", da), ("bob", db))]
        outs = [p.communicate(timeout=1800) for p in procs]
        ok("two concurrent publishers both succeed", all(p.returncode == 0 for p in procs), " | ".join(o[1][-160:] for o in outs if o[1]))
        res = [json.loads(o[0]) for o in outs]
        def version_hash(v): return "0x" + call(site, "0x" + SEL["version"] + "%064x" % v)[2:66]
        ok("each publisher's reported version holds exactly its own manifest", res[0]["version"] != res[1]["version"] and all(version_hash(x["version"]) == x["manifestHash"] for x in res), json.dumps([(x["version"], x["manifestHash"][:12]) for x in res]))
        fullc = full_creations(blocks(h0, head()))
        # Both sites share data.json's second chunk (same bytes): whichever store lands second returns the existing chunk.
        ok(f"concurrent publishers: at most one full chunk created per block ({sum(fullc.values())} full chunks in {len(fullc)} blocks)", sum(fullc.values()) >= 4 and all(v <= 1 for v in fullc.values()), fullc)
        an = nonce("alice"); send("alice", "0x" + SEL["setCurrent"] + "%064x" % res[0]["version"])
        meta, got = fetch(8540, site)
        ok("empty file round-trips (size 0, no chunks)", got.get("/empty.txt") == b"" and meta["version"] == res[0]["version"] and got == fa)

        # ---- a chunk stored by another account, then reorganized out before `safe`, is stored again by the sweep ----
        # c is cut off and mines alone; bob stores chunk X there (a one-file site), so X is at c's `latest`. Alice
        # publishes through a relay to c: the CLI finds X present and does not store it, stores Y and the manifest on c,
        # and its safe sweep's first read is held. Then c's branch is dropped from the CLI's view: c stops, and the
        # relay forwards to a, on the heavier a+b branch, which has never seen X, Y or the manifest. The sweep must find
        # them missing at both `safe` and `latest` and store them again before createVersion. c later rejoins without
        # mining and adopts the a+b branch (reorganizing its own away); every node then serves the same site.
        dn.stop(work); time.sleep(2)
        def node(i, peers, miner=True, listen=True, what=""):
            l = "abc"[i]; logf = open(os.path.join(work, f"{l}.log"), "ab"); logf.write(f"\n=== {what}\n".encode()); logf.flush()
            cmd = [dn.BIN, "run", "--network", os.path.join(work, "net"), "--data", os.path.join(work, "data", l), "--name", l, "--rpc", f"127.0.0.1:{8540 + i}", "--log", "--no-discovery"]
            cmd += (["--listen", f"127.0.0.1:{30300 + i}"] if listen else []) + sum([["--peer", f"127.0.0.1:{30300 + j}"] for j in peers], []) + (["--miner", l] if miner else []) + dn.EXTRA
            p = subprocess.Popen(cmd, stdout=logf, stderr=logf, start_new_session=True); extra_procs.append(p); return p
        def stop(p): os.killpg(p.pid, signal.SIGTERM); p.wait(60)
        pa, pb_ = node(0, [1], what="partition: a with b"), node(1, [0], what="partition: b with a"); pc = node(2, [], listen=False, what="partition: c alone")
        wait(lambda: all(rpc(i, "pocol_status") for i in range(3)), 60, "partitioned nodes up")
        wait(lambda: rpc(2, "pocol_status")["peers"] == [] and rpc(0, "pocol_status")["peers"], 60, "c isolated, a and b meshed")
        x = b"<!doctype html><p>chunk X, first stored by bob on a branch that loses"
        xd = os.path.join(work, "bobx"); os.makedirs(xd); open(os.path.join(xd, "index.html"), "wb").write(x)
        r = run_cli(["--dir", xd], port=8542, account="bob")
        X = chunk_address(x)
        ok("partition: bob stores chunk X on the isolated node c (absent on a)", r.returncode == 0 and rpc(2, "eth_getCode", [X, "latest"]) != "0x" and rpc(0, "eth_getCode", [X, "latest"]) == "0x", X if r.returncode == 0 else r.stderr[-200:])
        proxy = RpcProxy(8549, 8542); proxy.hold = True
        yd = os.path.join(work, "alicexy"); os.makedirs(yd); open(os.path.join(yd, "index.html"), "wb").write(x); open(os.path.join(yd, "y.txt"), "wb").write(b"chunk Y, stored by alice on c")
        nc0 = nonce("alice", 2)
        cli = subprocess.Popen([SITE_BIN, "publish", "--rpc", "127.0.0.1:8549", "--chain-id", cid, "--account", "alice", "--dir", yd, "--site", site], stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True)
        held = proxy.held.wait(240)
        y_at_c = rpc(2, "eth_getCode", [chunk_address(b"chunk Y, stored by alice on c"), "latest"]) != "0x"
        sent_c = nonce("alice", 2) - nc0
        ok("the CLI found X present at c's latest (not stored), stored Y and the manifest on c (2 transactions) and reached the safe sweep", held and y_at_c and sent_c == 2, f"held={held} y={y_at_c} sent={sent_c}")
        c_work = q(rpc(2, "pocol_status")["head"]["totalWork"]); stop(pc); proxy.upstream = 8540; proxy.hold = False; proxy.release.set()
        out, err = cli.communicate(timeout=900)
        ok("after the reorganization the safe sweep stored X again (and Y and the manifest), then the version was published",
           cli.returncode == 0 and f"storing again {X}" in err and err.count("storing again") >= 3, " | ".join(l for l in err.splitlines() if "storing again" in l) if cli.returncode == 0 else err[-500:])
        vx = json.loads(out)["version"]
        ok("the a+b chain holds X and serves the new version", rpc(0, "eth_getCode", [X, "safe"]) != "0x" and fetch(8540, site)[0]["version"] == vx and fetch(8540, site)[1] == {"/index.html": x, "/y.txt": b"chunk Y, stored by alice on c"})
        wait(lambda: q(rpc(0, "pocol_status")["head"]["totalWork"]) > c_work, 240, "a+b heavier than c's branch")
        pc = node(2, [0, 1], miner=False, what="rejoin: c without miner")
        wait(lambda: rpc(2, "eth_getBlockByNumber", [hex(head(0) - 2), False])["hash"] == rpc(0, "eth_getBlockByNumber", [hex(head(0) - 2), False])["hash"], 240, "c on the a+b branch")
        ok("c rejoins and reorganizes onto the a+b branch: it serves the same version", fetch(8542, site)[0]["version"] == vx and digest(fetch(8542, site)[1]) == digest(fetch(8540, site)[1]))
        proxy.close(); proxy = None
        rs = send("alice", "0x" + SEL["setCurrent"] + "%064x" % res[0]["version"])
        for p in (pa, pb_, pc): stop(p)
        extra_procs.clear(); time.sleep(1)

        # ---- restart every node: identical digests before and after ----
        dn.start(work, log=True); wait(lambda: all(head(i) > 0 for i in range(3)), 90, "nodes up again")
        wait(lambda: len({rpc(i, "eth_getBlockByNumber", [hex(min(head(j) for j in range(3)) - 1), False])["hash"] for i in range(3)}) == 1, 180, "nodes agree")
        before = {s: digest(fetch(8540, s)[1]) for s in (site, site256)}
        ok("before the restart every node serves identical digests (W10 site and 256-file site)", all(digest(fetch(8540 + i, s)[1]) == before[s] for i in range(3) for s in before), before)
        dn.stop(work); time.sleep(2); dn.start(work, log=True)
        wait(lambda: all(rpc(i, "pocol_status") for i in range(3)), 90, "restart")
        for i in range(3): ok(f"after restart node {'abc'[i]} serves the current versions unchanged", all(digest(fetch(8540 + i, s)[1]) == before[s] for s in before) and fetch(8540 + i, site)[1] == fa)
        dn.stop(work); time.sleep(2)

        # ---- small-cap devnets: CODE_CAP and ACCT_CAP give a clear CLI error ----
        def capnet(tag, flags):
            w = os.path.join(work, "cap-" + tag)
            dn.init(w, "a", DEVNET_ARGS + flags); dn.start(w, log=True)
            wait(lambda: head() >= 3, 90, f"{tag} devnet up"); return w
        w = capnet("probe", []); c0 = rpc(0, "pocol_getCaps"); dn.stop(w); time.sleep(2)
        code0, acct0 = q(c0["codeBytes"]), q(c0["accounts"])
        w = capnet("code", ["--code-cap", str(code0 + 30000)]); cid0 = cid; cid = str(json.load(open(os.path.join(w, "net", "devnet.json")))["chainId"])
        cd = os.path.join(work, "capsite"); write_w10(cd, pad=3 * CHUNK_MAX, seed=21)
        r = run_cli(["--dir", cd]); cc = rpc(0, "pocol_getCaps")
        ok(f"CODE_CAP {code0 + 30000}: the CLI stops with a clear error naming the cap", r.returncode != 0 and "code-size cap is reached (CODE_CAP)" in r.stderr and "stored chunks are kept" in r.stderr, r.stderr.strip()[-260:])
        ok("the chain stayed within CODE_CAP", q(cc["codeBytes"]) <= q(cc["codeBytesCap"]), f"{q(cc['codeBytes'])} of {q(cc['codeBytesCap'])}")
        dn.stop(w); time.sleep(2)
        w = capnet("acct", ["--acct-cap", str(acct0 + 3)])
        r = run_cli(["--dir", cd]); cc = rpc(0, "pocol_getCaps")
        ok(f"ACCT_CAP {acct0 + 3}: the CLI stops with a clear error naming the cap", r.returncode != 0 and "account cap is reached (ACCT_CAP)" in r.stderr, r.stderr.strip()[-260:])
        ok("the chain stayed within ACCT_CAP", q(cc["accounts"]) <= q(cc["accountsCap"]), f"{q(cc['accounts'])} of {q(cc['accountsCap'])}")
        dn.stop(w); cid = cid0
        print(json.dumps({"result": "PASS", "profile": profile, "checks": n, "site": site}))
        time.sleep(2); shutil.rmtree(work, ignore_errors=True)
        return 0
    except Exception as e:
        error = e
    finally:
        if proxy: proxy.close()
        for p in extra_procs:
            try: os.killpg(p.pid, signal.SIGTERM)
            except ProcessLookupError: pass
        dn.stop(work)
        for d in os.listdir(work) if os.path.isdir(work) else []:
            if d.startswith("cap-"): dn.stop(os.path.join(work, d))
    time.sleep(2)  # the nodes stop: their logs and journals are whole when they are kept
    kept = dn.keep_on_failure("e2e_site", work, {"error": str(error)[-500:], "passed": n, "profile": profile, "results": records})
    print(json.dumps({"result": "FAIL", "error": str(error)[-500:], "work": work, "kept": kept})); return 1

if __name__ == "__main__":
    sys.exit(main())
