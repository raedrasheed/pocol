#!/usr/bin/env python3
"""LP5/LP7 end-to-end devnet test with real pocold processes on loopback.

Gates exercised (LOCAL-DEVNET-PLAN.md LP5/LP7):
  * >=100 freshly produced H-full blocks from a fresh genesis across 3 nodes
  * late join (node c starts after height 15) and sync
  * partition and heal with a known winner that is strictly heavier and strictly LOWER than the
    other tip, so most work (consensus.md:209) and longest chain disagree (D133, consensus.md §9):
    c is isolated and frozen (no miner); a and b mine on until they lead c by LEAD c tip blocks of
    work and are frozen on one tip; c then mines late (PAUSE s after its tip), when ASERT makes its
    blocks cheap, until it is at least 2 blocks higher than a and b, and is frozen too: no tip can
    move. Heal: c fetches a/b's branch by Status sync (consensus.md:235) and reorganizes once; every
    head is exactly a/b's tip, c's tip is canonical on no node, and a and b neither reorganize nor
    start a session. Node d, a copy of c's store from before the heal, is sent a/b's branch tip
    first by a peer claiming genesis (no session, so through the orphan pool) and must switch to
    a/b's tip once, never sending a Status lighter than c's tip. Node f, a copy of b's store after
    the heal (head a/b's tip, none of c's late blocks), is sent c's lighter branch tip first the same
    way and must keep a/b's tip: no reorganization and no Status naming another head (the orphan
    path in both directions). Then a, which holds none of c's late blocks, is sent c's branch twice: below
    c's tip by a peer claiming genesis (no session; the branch ends above a's head), and in full in
    a sync session with a peer claiming more work than a; a stores it and keeps its head (the head
    is the maximum of ≻ whatever the order of arrival, consensus.md:215), with no reorganization,
    no other session and no Status naming another head, also after replaying its journal alone
  * heal while the miners run: node e, a second copy of c's store from before the heal, joins the
    mining a, b and c and must follow their chain (bounded wait)
  * SIGKILL crash of node b during production: b replays its journal alone (to its pre-kill
    height, less a torn tail of at most one record, holding the pre-kill head's parent and the
    pre-kill canonical block CONFIRM below that head), then rejoins
  * malicious peer: a garbage frame and a wrong-network Hello close the connection (a well-formed
    Hello does not); a corrupted block sent after a valid Hello adds exactly one reject entry, for
    a block a does not store; a keeps producing
  * user transactions submitted over RPC are included and their receipts match
  * work: every canonical block's difficulty and totalDifficulty, and the heavier/lighter
    precondition, are checked against W computed here from the committed targets, not the nodes'
    own totalWork (review R4-1)
  * relay and Status refresh: a listening test peer receives every block a imported from the relay
    peer, and a second Hello on that connection claiming more work starts a session (review R4-2)
  * convergence: identical canonical hash, stateRoot and receiptsRoot at every confirmed height on
    every node while mining; after the miners stop, the frozen tips have one totalWork and the
    canonical chains agree up to at most one block below the heads; full restart replays to the
    same head
Every wait is bounded, names what it waited for and prints how long it took. Exit status 0 only if
every check passes. POCOLD selects the pocold binary; pocol-sign is taken from the same directory
(or POCOL_SIGN).
"""
import contextlib, io, json, os, shutil, signal, socket, struct, subprocess, sys, tempfile, time
HERE = os.path.dirname(os.path.abspath(__file__)); ROOT = os.path.dirname(HERE)
sys.path.insert(0, os.path.join(ROOT, "scripts"))
import devnet as dn
SIGN = os.environ.get("POCOL_SIGN", os.path.join(os.path.dirname(dn.BIN), "pocol-sign"))
L = "abc"
# Node index: RPC port 8540+i, P2P port 30300+i. d and e are copies of c's store taken before the heal,
# f a copy of b's store taken after it.
IDX = {l: i for i, l in enumerate("abcdef")}
# Partition: a/b's lead over frozen c, in works of c's tip block, and when c mines late (s after its
# tip's timestamp; > D_att + Gamma = 46, so the first late block takes attempt >= 1).
LEAD, PAUSE = 4, 120
# Depth at which a canonical block is compared while miners run (equal-work tip forks resolve by
# D133 once one branch is extended first).
CONFIRM = 6

def waited(pred, timeout, what=None):
    t = time.time(); met = False
    while time.time() - t < timeout:
        try:
            if pred(): met = True; break
        except Exception: pass
        time.sleep(0.5)
    if what: print(f"WAIT {what}: {'met' if met else 'NOT met'} after {time.time()-t:.1f}s (bound {timeout}s)", flush=True)
    return met
def wait(pred, timeout, what):
    if not waited(pred, timeout, what): raise AssertionError(f"timeout waiting for {what}")

def st(i): return dn.rpc(8540 + i, "pocol_status")
def h(i): return st(i)["head"]["height"]
def tip(i): x = st(i)["head"]; return {"hash": x["hash"], "number": x["height"], "work": int(x["totalWork"], 16)}
def canon(i, n): b = dn.rpc(8540 + i, "eth_getBlockByNumber", [hex(n), False]); return b and b["hash"]
def blk(i, hsh): return dn.rpc(8540 + i, "eth_getBlockByHash", [hsh, False])
def short(t): return f"{t['number']}:{t['hash'][:12]}:w={t['work']:#x}"
def counters(i): s = st(i)["stats"]; return {"reorgs": s["reorgs"], "syncSessions": s["syncSessions"]}
def port_open(p):
    with socket.socket() as s: s.settimeout(0.5); return s.connect_ex(("127.0.0.1", p)) == 0

def frame(payload): return struct.pack(">I", len(payload)) + payload
def rlp_bytes(b):
    if len(b) == 1 and b[0] < 0x80: return b
    if len(b) < 56: return bytes([0x80 + len(b)]) + b
    l = len(b).to_bytes((len(b).bit_length() + 7) // 8, "big"); return bytes([0xb7 + len(l)]) + l + b
def rlp_list(items):
    p = b"".join(items)
    if len(p) < 56: return bytes([0xc0 + len(p)]) + p
    l = len(p).to_bytes((len(p).bit_length() + 7) // 8, "big"); return bytes([0xf7 + len(l)]) + l + p
def uint(v): return rlp_bytes(v.to_bytes((v.bit_length() + 7) // 8, "big") if v else b"")
# VA-SPEC wire: Hello = [0, chainId, genesisHash, headHash, height, totalWork (32 B), name]; Block = [1, header, body].
def hello(chain_id, genesis, head, height, work, name): return frame(rlp_list([uint(0), uint(chain_id), rlp_bytes(genesis), rlp_bytes(head), uint(height), rlp_bytes(work.to_bytes(32, "big")), rlp_bytes(name)]))
def block_frame(blk): return frame(rlp_list([uint(1), rlp_bytes(bytes.fromhex(blk["header"][2:])), rlp_bytes(bytes.fromhex(blk["body"][2:]))]))
def closed_by_peer(s, t):
    """Drops what the node sends until it closes the connection: True if it does within t s."""
    end = time.time() + t
    try:
        while time.time() < end:
            s.settimeout(max(0.05, end - time.time()))
            try:
                if not s.recv(65536): return True
            except socket.timeout: return False
    except OSError: return True
    return False
def msg_type(p):
    """The message type of a frame payload (an RLP list whose first item is the type)."""
    k = 1 if p[0] <= 0xf7 else 1 + p[0] - 0xf7; b = p[k]
    return 0 if b == 0x80 else b if b < 0x80 else None
def frames(s, t, until=None, buf=b""):
    """Reads the node's frames for t s, or until one of type `until` arrives or the node closes:
    (the frame payloads read, the bytes of a partial frame left over)."""
    end = time.time() + t; out = []
    try:
        while True:
            while len(buf) >= 4 and len(buf) >= 4 + struct.unpack(">I", buf[:4])[0]:
                n = struct.unpack(">I", buf[:4])[0]; p, buf = buf[4:4 + n], buf[4 + n:]
                if p: out.append(p)
                if p and until is not None and msg_type(p) == until: return out, buf
            if time.time() >= end: break
            s.settimeout(max(0.05, end - time.time()))
            try: chunk = s.recv(65536)
            except socket.timeout: break
            if not chunk: break
            buf += chunk
    except OSError: pass
    return out, buf
def rlp_items(p):
    """The items of an RLP list of byte strings (a frame payload)."""
    i, out = (1 if p[0] <= 0xf7 else 1 + p[0] - 0xf7), []
    while i < len(p):
        b = p[i]
        if b < 0x80: out.append(p[i:i + 1]); i += 1
        elif b <= 0xb7: out.append(p[i + 1:i + 1 + b - 0x80]); i += 1 + b - 0x80
        elif b <= 0xbf: k = b - 0xb7; n = int.from_bytes(p[i + 1:i + 1 + k], "big"); out.append(p[i + 1 + k:i + 1 + k + n]); i += 1 + k + n
        else: raise ValueError("nested list")
    return out
def status_claims(ps):
    """(head hash, totalWork) of every Status (Hello, network.md:7) among frame payloads. pocold sends
    one on connect and one on every new head (its Status refresh), so a head change shows here even if
    the node reverts it before its head is read."""
    return [("0x" + it[3].hex(), int.from_bytes(it[5], "big")) for it in (rlp_items(p) for p in ps if msg_type(p) == 0)]
def status_heads(ps): return [x for x, _ in status_claims(ps)]
def rlp_decode(b):
    """A whole RLP item: bytes, or a list of items (nested)."""
    def item(i):
        c = b[i]
        if c < 0x80: return b[i:i + 1], i + 1
        if c <= 0xb7: return b[i + 1:i + 1 + c - 0x80], i + 1 + c - 0x80
        if c <= 0xbf: k = c - 0xb7; n = int.from_bytes(b[i + 1:i + 1 + k], "big"); return b[i + 1 + k:i + 1 + k + n], i + 1 + k + n
        if c <= 0xf7: k, n = 0, c - 0xc0
        else: k = c - 0xf7; n = int.from_bytes(b[i + 1:i + 1 + k], "big")
        j, end, out = i + 1 + k, i + 1 + k + n, []
        while j < end: x, j = item(j); out.append(x)
        return out, end
    x, _ = item(0); return x
def target_of(i, n):
    """The target committed in canonical block n's header on node i: item 8 of the UT list (after TAG,
    chainId, genesisHash, parentHash, h, a, protocolVersion, ts), read from the header RLP."""
    return int.from_bytes(rlp_decode(bytes.fromhex(dn.rpc(8540 + i, "pocol_getBlockRlp", [hex(n)])["header"][2:]))[0][0][8], "big")
def pywork(i, top):
    """Review R4-1: W(t) computed here, independently of the node: Σ floor(2^256/(target+1)) over node i's
    canonical blocks 1..top (consensus.md:117, §9). Every block's totalDifficulty and difficulty over RPC
    must equal it; the first block where one differs is returned with the sum."""
    total, bad = 0, None
    if int(dn.rpc(8540 + i, "eth_getBlockByNumber", ["0x0", False])["totalDifficulty"], 16) != 0: bad = 0
    for n in range(1, top + 1):
        w = 2**256 // (target_of(i, n) + 1); total += w; b = dn.rpc(8540 + i, "eth_getBlockByNumber", [hex(n), False])
        if bad is None and (int(b["difficulty"], 16) != w or int(b["totalDifficulty"], 16) != total): bad = n
    return total, bad

def main():
    busy = [p for p in (30300, 30301, 30302, 30303, 30305, 8540, 8541, 8542, 8543, 8544, 8545) if port_open(p)]
    if busy: print(json.dumps({"result": "FAIL", "error": f"ports busy {busy}"})); return 1
    work = tempfile.mkdtemp(prefix="pocol-e2e-"); net = os.path.join(work, "net"); t_start = time.time()
    checks = []; procs = {}; mined = {l: 0 for l in L}
    def ok(name, cond, detail=""):
        checks.append((name, bool(cond), detail)); print(("PASS " if cond else "FAIL ") + name + (f" — {detail}" if detail else ""), flush=True)
        if not cond: raise AssertionError(name)
    def run_node(l, miner=True, connect=True, listen=None, peers=None):
        # As devnet.py start. connect=False: no listener and no peers; listen/peers override that
        # (d listens with no peers; e dials a, b and c with no listener).
        i = IDX[l]
        if port_open(8540 + i) or port_open(30300 + i): raise AssertionError(f"{l} is still running")  # else RPC would answer from the old process
        listen = connect if listen is None else listen
        peers = ([j for j in range(3) if j != i] if connect else []) if peers is None else peers
        cmd = [dn.BIN, "run", "--network", net, "--data", os.path.join(work, "data", l), "--name", l, "--rpc", f"127.0.0.1:{8540+i}", "--log"] + dn.EXTRA
        if miner: cmd += ["--miner", l]
        if listen: cmd += ["--listen", f"127.0.0.1:{30300+i}"]
        cmd += sum([["--peer", f"127.0.0.1:{30300+j}"] for j in peers], [])
        procs[l] = subprocess.Popen(cmd, stdout=open(os.path.join(work, f"{l}.log"), "ab"), stderr=subprocess.STDOUT, start_new_session=True)
        pf = os.path.join(work, "pids.json"); pids = json.load(open(pf)) if os.path.exists(pf) else {}
        pids[l] = procs[l].pid; json.dump(pids, open(pf, "w"))
        wait(lambda: st(i) is not None, 60, f"{l} answers RPC")
    def stop_nodes(ls, sig=signal.SIGTERM):
        # Signals all of `ls` at once. The `mined` counter restarts with its process, so it is added up
        # just before each stop; a block mined after this read is not counted, so the sum can only be
        # low. A process leaves `procs` only once it has exited (the finally block kills what is left).
        for l in ls:
            if l not in mined: continue
            try: mined[l] += st(IDX[l])["stats"]["mined"]
            except Exception: pass
        dn.stop(work, only=list(ls), sig=sig)
        for l in ls:
            p = procs[l]
            try: p.wait(30)
            except subprocess.TimeoutExpired:
                os.killpg(p.pid, signal.SIGKILL); p.wait(30); procs.pop(l)
                raise AssertionError(f"{l} still running 30 s after {sig.name}")
            procs.pop(l)
        for l in ls: i = IDX[l]; wait(lambda: not port_open(8540 + i) and not port_open(30300 + i), 30, f"{l} stopped")
    def stop_node(l, sig=signal.SIGTERM): stop_nodes([l], sig)
    try:
        with contextlib.redirect_stdout(io.StringIO()):  # init-devnet echoes devnet.json
            dn.init(work, "a,b,c", ["--t-blk", "2", "--tau", "20", "--c", "8", "--d-att", "41", "--fb-wait", "3", "--work-bits", "15"])
        info = json.load(open(os.path.join(net, "devnet.json"))); genesis = bytes.fromhex(info["genesisHash"][2:])
        run_node("a"); run_node("b")
        wait(lambda: h(0) >= 15 and h(1) >= 15, 120, "a,b reach 15")
        ok("two nodes produce blocks from fresh genesis", True, f"a={h(0)} b={h(1)}")
        # Late join.
        run_node("c")
        wait(lambda: h(2) >= h(0) - 1 >= 15, 90, "c syncs")
        ok("late-joining node c syncs to the head", True, f"c={h(2)}")
        # The state mode every node reports (POCOLD_RUN_ARGS=--state-store runs the suite in store mode).
        modes = [dn.rpc(8540 + i, "pocol_status")["stateStore"] for i in range(3)]
        ok("every node runs in the requested state mode", modes == ["--state-store" in dn.EXTRA] * 3, f"stateStore={modes}")
        # R14/R19: a store-mode node's admission timing includes the calibration through a scratch state store.
        timed = [any("node store" in c["detail"] for c in dn.rpc(8540 + i, "pocol_status")["admission"]["conditions"] if c["id"] == "timing-not-a-bound") for i in range(3)]
        ok("each node's R14/R19 timing covers the state mode it runs", timed == ["--state-store" in dn.EXTRA] * 3, f"store timing={timed}")
        # Transactions over RPC (alice -> bob), via node a.
        alice = next(x for x in info["accounts"] if x["label"] == "alice"); bob = next(x for x in info["accounts"] if x["label"] == "bob")
        txs = []
        sign = subprocess.run([SIGN, "--chain-id", str(info["chainId"]), "--account", "alice", "--to", bob["address"], "--value", "1000", "--count", "5"],
                              capture_output=True, text=True, check=True).stdout.split()
        for raw in sign: txs.append(dn.rpc(8540, "eth_sendRawTransaction", [raw]))
        def receipts_ok():
            return all(dn.rpc(8540 + i, "eth_getTransactionReceipt", [t]) not in (None,) for t in txs for i in range(3))
        wait(receipts_ok, 90, "tx receipts on all nodes")
        def same_receipts():
            rs = [dn.rpc(8540 + i, "eth_getTransactionReceipt", [txs[-1]]) for i in range(3)]
            return all(r == rs[0] for r in rs)
        waited(same_receipts, 60, "identical receipts on all nodes")
        rs = [dn.rpc(8540 + i, "eth_getTransactionReceipt", [txs[-1]]) for i in range(3)]
        if not all(r == rs[0] for r in rs): print(json.dumps(rs, indent=1))
        ok("user transactions included; receipts identical on all nodes", all(r == rs[0] for r in rs) and rs[0]["status"] == "0x1", rs[0]["blockNumber"])
        bal = int(dn.rpc(8541, "eth_getBalance", [bob["address"], "latest"]), 16)
        ok("transfer value applied", bal == 10**24 + 5000, str(bal))
        # Malicious peer against node a (VA-SPEC: malformed frames and a wrong network close the
        # connection). The control, a well-formed Hello of this network, keeps it open, so a close
        # is attributable to the input; the corrupted block is sent after that Hello, so it reaches
        # validation (before a Hello any other frame closes the connection unread).
        s = socket.create_connection(("127.0.0.1", 30300)); s.sendall(frame(b"\xff\xfe garbage")); garbage = closed_by_peer(s, 5); s.close()
        s = socket.create_connection(("127.0.0.1", 30300)); s.sendall(hello(1, b"\x01" * 32, b"\x02" * 32, 5, 5, b"evil")); wrong = closed_by_peer(s, 5); s.close()
        s = socket.create_connection(("127.0.0.1", 30300)); s.sendall(hello(info["chainId"], genesis, genesis, 0, 0, b"e2e")); kept = not closed_by_peer(s, 2)
        ok("malicious peer: a garbage frame and a wrong-network Hello close the connection; a well-formed Hello does not", garbage and wrong and kept,
           f"garbage closed={garbage} wrong-network closed={wrong} valid kept={kept}")
        rej0 = {json.dumps(r, sort_keys=True) for r in dn.rpc(8540, "pocol_getRejects")}
        def new_rejects(): return [r for r in dn.rpc(8540, "pocol_getRejects") if json.dumps(r, sort_keys=True) not in rej0]
        n = h(0); raw = dn.rpc(8540, "pocol_getBlockRlp", [hex(n)]); orig = dn.rpc(8540, "eth_getBlockByNumber", [hex(n), False]); sr = bytes.fromhex(orig["stateRoot"][2:])
        hdr = bytearray(bytes.fromhex(raw["header"][2:])); k = hdr.find(sr); hdr[k + 16 if k >= 0 else 200] ^= 0xff  # a stateRoot byte: still decodes, new hash
        s.sendall(frame(rlp_list([uint(1), rlp_bytes(bytes(hdr)), rlp_bytes(bytes.fromhex(raw["body"][2:]))])))
        waited(lambda: new_rejects(), 15, "a new reject entry"); s.close(); nr = new_rejects()
        # Exactly one new entry, for a block a does not store (a rejected block is not stored) and that
        # is not the original: the entry is the corrupted block's (its hash is not recomputed here).
        rejected = len(nr) == 1 and nr[0]["hash"] != orig["hash"] and blk(0, nr[0]["hash"]) is None
        before = h(0); progressed = waited(lambda: h(0) > before, 60, "a produces after the corrupted block")
        ok("malicious peer: a corrupted block after a valid Hello is rejected and a keeps producing", rejected and progressed,
           f"new rejects={nr} a {before}->{h(0)}")
        # Partition and heal (D133, consensus.md §9) with a winner fixed before the heal that is
        # strictly heavier and strictly LOWER than the other tip, so the most-work rule
        # (consensus.md:209) and a longest-chain rule choose differently.
        # 1. c is isolated and frozen: restarted with no listener, no peers and no miner, at head c0.
        stop_node("c"); run_node("c", miner=False, connect=False)
        c0 = tip(2); t_c = st(2)["head"]["ts"]; d_c = int(blk(2, c0["hash"])["difficulty"], 16)  # the work of c's tip block
        # 2. a and b mine on until they lead c0 by LEAD c tip blocks of work, >= 2 blocks past it; then
        #    they are frozen on ONE tip (an equal-work sibling tie would be local, D133): a's miner stops
        #    first, a follows b's next block, then b's miner stops too.
        t0 = time.time(); lead = waited(lambda: min(h(0), h(1)) >= c0["number"] + 2 and min(tip(0)["work"], tip(1)["work"]) > c0["work"] + LEAD * d_c, 180, "a/b lead isolated c")
        ok(f"partition: a and b lead isolated c by more than {LEAD} c tip blocks of work", lead, f"a/b={[short(tip(i)) for i in (0, 1)]} c={short(c0)} in {time.time()-t0:.0f}s")
        stop_node("a"); run_node("a", miner=False)
        hb = h(1); wait(lambda: h(1) > hb and tip(0)["hash"] == tip(1)["hash"], 180, "a follows b's next block")  # a lone miner may wait out an attempt (46 s)
        stop_node("b"); run_node("b", miner=False)
        wait(lambda: tip(0)["hash"] == tip(1)["hash"], 60, "a and b settle on one tip")
        ab_tip = tip(0)
        # 3. c mines LATE, from PAUSE s after its tip's timestamp: that tip's attempt 0 has expired
        #    (C > U(0)+Γ), so its child takes an attempt >= 1 fallback timestamp L(a) >= p.ts+42
        #    (consensus.md:93-97), and ASERT's target grows with p.ts-g_ts-T_blk·p.h (consensus.md §7):
        #    every late block after the first is cheap. c climbs at least 2 blocks past a/b's height
        #    with little work (the precondition below checks it), so the relay without a session (step
        #    5) ends with a block that is higher than a's head and lighter. c is then frozen again. From
        #    here no tip can move.
        time.sleep(max(0.0, t_c + PAUSE - time.time()))  # a wall-clock deadline: nothing to poll
        stop_node("c"); run_node("c", connect=False)
        need = ab_tip["number"] + 2 - c0["number"]
        t0 = time.time(); up = waited(lambda: h(2) >= ab_tip["number"] + 2, 240, f"c climbs to a/b's height + 2 ({need} late blocks)"); t_up = time.time() - t0
        stop_node("c"); run_node("c", miner=False, connect=False); c_tip = tip(2)
        fork = next(n for n in range(ab_tip["number"], -1, -1) if canon(0, n) == canon(2, n))
        late = [dn.rpc(8542, "eth_getBlockByNumber", [hex(n), False]) for n in range(c0["number"] + 1, c_tip["number"] + 1)]  # c's late blocks
        # The work is computed here from the committed targets (R4-1), not taken from the nodes' totalWork.
        (w_ab, bad_ab), (w_c, bad_c) = pywork(0, ab_tip["number"]), pywork(2, c_tip["number"])
        ok("work: on a's and c's branches every block's difficulty and totalDifficulty equal W computed here from its committed target",
           bad_ab is None and bad_c is None and (w_ab, w_c) == (ab_tip["work"], c_tip["work"]), f"first mismatch a: {bad_ab}, c: {bad_c}")
        ok("partition: a/b's tip is strictly heavier than c's tip (by the W computed here) and at least 2 blocks lower, both >= 2 blocks past the fork",
           up and w_ab > w_c and ab_tip["number"] + 2 <= c_tip["number"] and fork + 2 <= ab_tip["number"],
           f"a/b={short(ab_tip)} c={short(c_tip)} fork={fork}; work above c0 in c tip blocks: a/b {(ab_tip['work']-c0['work'])/d_c:.2f}, "
           f"late c {(c_tip['work']-c0['work'])/d_c:.2f}; {len(late)} late blocks, attempts {[b['pocol']['attempt'] for b in late]} in {t_up:.0f}s")
        relay = [dn.rpc(8542, "pocol_getBlockRlp", [hex(n)]) for n in range(fork + 1, c_tip["number"] + 1)]  # c's branch, parents first
        relay_h = [canon(2, n) for n in range(fork + 1, c_tip["number"] + 1)]  # their hashes (c's branch is canonical on c only until the heal)
        r0 = st(2)["stats"]["reorgs"]
        # 4. Heal: c rejoins without a miner. Nothing is mined, so no block is broadcast and the heal can
        #    only start from Status sync (consensus.md:235): c sees a and b claim more work and fetches
        #    their branch. a and b, which claim more work than c, start no session with c and keep their
        #    head. Before c rejoins, its store (head c's tip) is copied twice, for d and e below.
        ab0 = [counters(i) for i in (0, 1)]
        stop_node("c")
        for l in "de": shutil.copytree(os.path.join(work, "data", "c"), os.path.join(work, "data", l))
        run_node("c", miner=False)
        t0 = time.time(); healed = waited(lambda: all(tip(i)["hash"] == ab_tip["hash"] for i in range(3)), 120, "every head is a/b's tip")
        heads = [tip(i) for i in range(3)]
        ok("heal: every node's head is a/b's tip (heavier, and lower than c's tip)", healed, f"heads a/b/c={[short(t) for t in heads]} a/b tip={short(ab_tip)} in {time.time()-t0:.0f}s")
        ok("heal: on every node the canonical block at a/b's tip height is a/b's tip", all(canon(i, ab_tip["number"]) == ab_tip["hash"] for i in range(3)))
        ok("heal: c's tip is not canonical on any node", all(canon(i, c_tip["number"]) != c_tip["hash"] for i in range(3)), f"c tip={short(c_tip)}")
        r = st(2)["stats"]["reorgs"]
        ok("heal: c reorganized exactly once (one head switch, nothing mined)", r - r0 == 1, f"c reorgs {r0} -> {r}")
        ab1 = [counters(i) for i in (0, 1)]
        ok("heal: a and b neither reorganized nor started a sync session", ab1 == ab0, f"a/b counters {ab0} -> {ab1}")
        # Heal without a session, through the orphan pool: d, the copy of c's pre-heal store (head c's
        # tip), listens with no peers and no miner. A test peer claiming genesis (so d starts no session)
        # sends a/b's branch above the fork TIP FIRST: each block arrives before its parent and waits
        # in the orphan pool until the lowest one, whose parent d holds, releases the chain. d must
        # switch to a/b's tip with exactly one reorganization, and every Status it sends the peer (on
        # connect, then on each new head) must name c's tip or a head with at least its work: fork
        # choice never takes a lighter block, even one released from the orphan pool.
        abr = [dn.rpc(8540, "pocol_getBlockRlp", [hex(n)]) for n in range(fork + 1, ab_tip["number"] + 1)]
        run_node("d", miner=False, listen=True, peers=[])
        d0 = st(IDX["d"])
        s = socket.create_connection(("127.0.0.1", 30300 + IDX["d"])); s.sendall(hello(info["chainId"], genesis, genesis, 0, 0, b"e2e-orphans"))
        for b in reversed(abr): s.sendall(block_frame(b))
        adopted = waited(lambda: tip(IDX["d"])["hash"] == ab_tip["hash"], 60, "d adopts a/b's tip"); seen, _ = frames(s, 2); s.close(); d1 = st(IDX["d"]); ann = status_claims(seen)
        ok("heal without a session: d (c's pre-heal store), sent a/b's branch tip first by a peer claiming genesis, adopts a/b's tip through the orphan pool, reorganizing once",
           d0["head"]["hash"] == c_tip["hash"] and adopted and d1["stats"]["reorgs"] - d0["stats"]["reorgs"] == 1 and d1["stats"]["syncSessions"] == 0,
           f"{len(abr)} blocks h={fork+1}..{ab_tip['number']} sent tip first; d head {d0['head']['height']}->{d1['head']['height']} "
           f"reorgs {d0['stats']['reorgs']}->{d1['stats']['reorgs']} sessions {d1['stats']['syncSessions']}")
        ok("heal without a session: every Status d sent the test peer names c's tip or a head with at least its work, the first c's tip, the last a/b's tip",
           ann and ann[0] == (c_tip["hash"], c_tip["work"]) and ann[-1][0] == ab_tip["hash"] and all(w >= c_tip["work"] for _, w in ann),
           f"{len(ann)} Status (head, work above c's tip in c tip blocks) {[(x[:12], round((w - c_tip['work']) / d_c, 2)) for x, w in ann]}")
        stop_node("d")
        # 4c. The orphan path the other way: f, a copy of b's store after the heal (head a/b's tip, none
        #     of c's late blocks), listens with no peers and no miner. A test peer claiming genesis sends
        #     c's lighter branch above the fork TIP FIRST, so its late blocks, higher than f's head and
        #     lighter, are released from the orphan pool. f must keep a/b's tip (the head is the maximum
        #     of ≻ whatever the order of arrival, consensus.md:215): no reorganization, no session, and
        #     every Status it sends the peer names a/b's tip.
        i_f = IDX["f"]; stop_node("b"); shutil.copytree(os.path.join(work, "data", "b"), os.path.join(work, "data", "f")); run_node("b", miner=False)
        run_node("f", miner=False, listen=True, peers=[]); f0 = st(i_f); held = sum(1 for b in late if blk(i_f, b["hash"]) is not None)
        s = socket.create_connection(("127.0.0.1", 30300 + i_f)); s.sendall(hello(info["chainId"], genesis, genesis, 0, 0, b"e2e-orphans-light"))
        for b in reversed(relay): s.sendall(block_frame(b))
        fst = waited(lambda: blk(i_f, c_tip["hash"]) is not None, 60, "f stores c's tip"); seen, _ = frames(s, 2); s.close(); ann = status_heads(seen); f1 = st(i_f)
        ok("heal without a session, lighter branch: f (b's store: head a/b's tip, none of c's late blocks), sent c's branch tip first by a peer claiming genesis, stores c's tip through the orphan pool and keeps a/b's tip, c's tip not canonical, no reorganization, no session",
           f0["head"]["hash"] == ab_tip["hash"] and held == 0 and fst and f1["head"]["hash"] == ab_tip["hash"] and canon(i_f, c_tip["number"]) != c_tip["hash"]
           and f1["stats"]["reorgs"] == f0["stats"]["reorgs"] and f1["stats"]["syncSessions"] == f0["stats"]["syncSessions"],
           f"{len(relay)} blocks h={fork+1}..{c_tip['number']} sent tip first; f held {held}/{len(late)} late blocks; stored={fst} f head {short(tip(i_f))} "
           f"reorgs {f0['stats']['reorgs']}->{f1['stats']['reorgs']} sessions {f0['stats']['syncSessions']}->{f1['stats']['syncSessions']}")
        ok("heal without a session, lighter branch: every Status f sent the test peer names a/b's tip",
           ann and all(x == ab_tip["hash"] for x in ann), f"{len(ann)} Status heads {[x[:12] for x in ann]}")
        stop_node("f")
        # 5. Relay without a session: a test peer (valid Hello claiming genesis, so a starts no session
        #    with it) delivers c's branch below c's tip, parents first, to a, which holds none of c's late
        #    blocks (c mined them isolated; c0 itself may be a side block a holds). The last block sent
        #    is higher than a's head (precondition) and lighter. a must not reorganize at all, not even
        #    for a moment (the head changes only at import, at most once per import, consensus.md:220):
        #    its counters must not move, and every Status it sends the test peer (on connect, then on
        #    each new head) must name a/b's tip. The connect Status is the control that the peer reads.
        ok("relay: a holds none of c's late blocks", all(blk(0, b["hash"]) is None for b in late), f"{len(late)} blocks h={c0['number']+1}..{c_tip['number']}")
        a0 = counters(0)
        # Review R4-2: a second test peer, claiming genesis, only reads. a must forward to it every block it
        # imports from the relay peer (network.md "Receiving block" step 3, then the broadcast): no node of the
        # full mesh needs relay, this peer does.
        #    A block a already holds is a duplicate and is not imported again, so only the others are expected.
        new_to_a = [blk(0, x) is None for x in relay_h[:-1]]
        lis = socket.create_connection(("127.0.0.1", 30300)); lis.sendall(hello(info["chainId"], genesis, genesis, 0, 0, b"e2e-listen")); frames(lis, 1)
        s = socket.create_connection(("127.0.0.1", 30300)); s.sendall(hello(info["chainId"], genesis, genesis, 0, 0, b"e2e-relay"))
        for b in relay[:-1]: s.sendall(block_frame(b))
        stored = waited(lambda: blk(0, late[-2]["hash"]) is not None, 60, "a stores c's branch below c's tip")
        seen, rest = frames(s, 2); seen5 = seen; ann = status_heads(seen)
        got, _ = frames(lis, 3); lis.close(); fwd = {bytes(rlp_items(p)[1]) for p in got if msg_type(p) == 1}
        sent = [bytes.fromhex(b["header"][2:]) for b, new in zip(relay[:-1], new_to_a) if new]
        ok("relay: a forwards every block it imported from the relay peer to its other peers (a listening test peer receives each one)",
           sent and all(x in fwd for x in sent), f"{sum(1 for x in sent if x in fwd)}/{len(sent)} blocks new to a received by the listening peer")
        a_now = tip(0); a1 = counters(0)
        ok("relay: a stores c's lighter branch below c's tip (higher than a's head), keeps its head, the branch is not canonical (head = maximum of ≻, consensus.md:215), no reorganization and no session",
           stored and a_now["hash"] == ab_tip["hash"] and canon(0, c_tip["number"] - 1) != late[-2]["hash"] and a1 == a0,
           f"stored={stored} up to h={c_tip['number']-1} a head={short(a_now)} a counters {a0} -> {a1}")
        ok("relay: every Status a sent the relay peer names a/b's tip (no head change, not even one reverted later)",
           ann and all(x == ab_tip["hash"] for x in ann), f"{len(ann)} Status heads {[x[:12] for x in ann]} a/b tip={ab_tip['hash'][:12]}")
        # 6. Relay in a sync session: a second test peer claims c's tip with more work than a's head, so
        #    a opens a session with it and asks (GetFrom); the peer answers with all of c's branch. a
        #    stores c's tip and keeps its head: the work of the blocks decides, not the peer's claim.
        #    Every Status a sends this peer must name a/b's tip, as in step 5.
        #    Review R4-2: the claim is a SECOND Hello on step 5's connection (a Status refresh), not a first
        #    Hello on a new one: a must take it into its round-robin and ask within SESSION_GAP_MS (1 s) and
        #    a tick. 15 s is the bound.
        a0 = counters(0)
        s.sendall(hello(info["chainId"], genesis, bytes.fromhex(c_tip["hash"][2:]), c_tip["number"], ab_tip["work"] + 1, b"e2e-relay"))
        t_ask = time.time(); seen, rest = frames(s, 15, until=7, buf=rest); asked = any(msg_type(p) == 7 for p in seen); t_ask = time.time() - t_ask  # 7 = GetFrom
        ok("relay: a Status refresh (a second Hello on the same connection) claiming more work starts a session: a asks with GetFrom",
           asked, f"GetFrom after {t_ask:.1f}s")
        if asked:
            for b in relay: s.sendall(block_frame(b))
        stored = asked and waited(lambda: blk(0, c_tip["hash"]) is not None, 60, "a stores c's tip from the session")
        more, _ = frames(s, 2, buf=rest); s.close(); ann = status_heads(seen5 + seen + more)  # one connection: its connect Status came in step 5
        a_now = tip(0); a1 = counters(0)
        ok("relay: a syncs c's tip in one session from a peer claiming more work, keeps its head, c's tip is not canonical, no reorganization",
           stored and a1["syncSessions"] == a0["syncSessions"] + 1 and a1["reorgs"] == a0["reorgs"] and a_now["hash"] == ab_tip["hash"] and canon(0, c_tip["number"]) != c_tip["hash"],
           f"GetFrom={asked} stored={stored} a counters {a0} -> {a1} a head={short(a_now)}")
        ok("relay: every Status a sent the session peer on that connection (the connect Status of step 5 and any later one) names a/b's tip",
           ann and all(x == ab_tip["hash"] for x in ann), f"{len(ann)} Status heads {[x[:12] for x in ann]}")
        # 7. a restarts alone (no peers, no miner) and replays a journal whose last records are c's
        #    branch: the replayed head is still a/b's tip, whatever the order of the records (consensus.md:215).
        stop_node("a"); run_node("a", miner=False, connect=False)
        a_now = tip(0)
        ok("replay after the relay: a's head is a/b's tip; c's tip is stored and not canonical",
           a_now["hash"] == ab_tip["hash"] and blk(0, c_tip["hash"]) is not None and canon(0, c_tip["number"]) != c_tip["hash"], f"a head={short(a_now)}")
        # Every node mines again on the healed chain.
        stop_nodes(L)
        for l in L: run_node(l)
        wait(lambda: min(h(i) for i in range(3)) > c_tip["number"], 120, "production resumes on the healed chain")
        # Heal while the miners run (F10): e, the second copy of c's pre-heal store (head c's tip, lighter
        # than the live chain), dials a, b and c, which keep mining, so its target moves while it syncs
        # (sessions, broadcasts and parent fetches overlap). Bounded: e's canonical block CONFIRM below
        # its head is a's, above c's tip height; c's tip is not canonical on e; e reorganized. Telling
        # most work from longest chain remains the frozen heal's job (the live chain is also higher).
        # e first opens its store alone, to read its starting head (connected, it syncs at once).
        ie = IDX["e"]; run_node("e", miner=False, connect=False); e0 = tip(ie); stop_node("e")
        run_node("e", miner=False, listen=False, peers=[0, 1, 2])
        def e_follows():
            n = h(ie) - CONFIRM
            return n > c_tip["number"] and canon(ie, n) == canon(0, n) and canon(ie, c_tip["number"]) != c_tip["hash"]
        t0 = time.time(); followed = waited(e_follows, 120, "e follows the mining nodes"); t_e = time.time() - t0
        e1 = tip(ie); es = st(ie)["stats"]; live = [h(i) for i in range(3)]
        ok("heal while mining: e (c's pre-heal store) joins the mining nodes and follows their chain: its block CONFIRM below its head is a's, c's tip is not canonical, e reorganized",
           e0["hash"] == c_tip["hash"] and followed and es["reorgs"] >= 1,
           f"e head {short(e0)} -> {short(e1)} in {t_e:.0f}s, reorgs {es['reorgs']} sessions {es['syncSessions']} imported {es['imported']}; a/b/c at {live}")
        stop_node("e")
        # Crash: SIGKILL b mid-production. b first replays its journal alone (no peers, no miner), so a
        # lost or truncated journal cannot be hidden by a resync: its replayed head is at least its
        # pre-kill height less one (a torn tail of one record), it holds the pre-kill head's parent
        # (journaled before that head, so never the torn record), and its canonical block CONFIRM below
        # the pre-kill head is the one it had. b mines until the kill, so a tip race between these reads
        # and the kill may replace its canonical block one below the head, not one CONFIRM deep.
        # Then it rejoins.
        kb = tip(1); kp = blk(1, kb["hash"])["parentHash"]; kc = canon(1, kb["number"] - CONFIRM)
        stop_node("b", signal.SIGKILL); run_node("b", miner=False, connect=False); rb = tip(1)
        ok("SIGKILL crash: b replays its journal alone to at least its pre-kill height less one, holds the pre-kill head's parent, and keeps its canonical block CONFIRM below the pre-kill head",
           rb["number"] >= kb["number"] - 1 and blk(1, kp) is not None and canon(1, kb["number"] - CONFIRM) == kc,
           f"pre-kill head {short(kb)}, replayed head {short(rb)}; pre-kill parent canonical after replay: {canon(1, kb['number'] - 1) == kp}")
        stop_node("b"); run_node("b")
        wait(lambda: abs(h(1) - h(0)) <= 1, 90, "b recovers")
        ok("SIGKILL crash: b rejoins", True, f"b={h(1)}")
        # Reach >= 100 blocks.
        wait(lambda: min(h(i) for i in range(3)) >= 100, 400, "100 blocks on all nodes")
        # Convergence on every height of the confirmed common prefix (>= 100). Equal-work tip forks
        # (simultaneous seals while ASERT is easy after the partition/crash) resolve by D133 as soon
        # as one branch is extended first, so the very tip is compared only once it is CONFIRM deep.
        def common_top():
            top = min(h(i) for i in range(3)) - CONFIRM
            if top < 100: return None
            return top if len({canon(i, top) for i in range(3)}) == 1 else None
        wait(lambda: common_top() is not None, 300, "confirmed common prefix >= 100")
        top = common_top() or min(h(i) for i in range(3)) - CONFIRM
        mism = []
        for n in range(1, top + 1):
            bs = [dn.rpc(8540 + i, "eth_getBlockByNumber", [hex(n), False]) for i in range(3)]
            if not all((b["hash"], b["stateRoot"], b["receiptsRoot"]) == (bs[0]["hash"], bs[0]["stateRoot"], bs[0]["receiptsRoot"]) for b in bs): mism.append(n)
        ok(f"identical canonical hash/stateRoot/receiptsRoot at heights 1..{top} on 3 nodes", not mism, str(mism[:5]))
        # Every miner produced blocks: each process's `mined` counter (own blocks imported) added up
        # over its restarts. Cross-check on the chain (winner = owner(nonce)): a and b won canonical
        # blocks (they mine heights 1..15 alone), and every block of c's isolated late branch names c
        # (c mined it alone; c's canonical wins are not certain, its branch is abandoned at the heal).
        stats = {l: st(i)["stats"] for i, l in enumerate(L)}
        total = {k: mined[k] + stats[k]["mined"] for k in L}
        ok("every miner produced blocks", all(v > 0 for v in total.values()), json.dumps(total))
        blocks = [dn.rpc(8540, "eth_getBlockByNumber", [hex(n), False]) for n in range(1, top + 1)]
        ids = {m["id"].lower(): m["label"] for m in info["miners"]}
        wins = {l: sum(1 for b in blocks if ids.get(b["miner"].lower()) == l) for l in L}
        late_c = sum(1 for b in late if ids.get(b["miner"].lower()) == "c")
        ok("every miner won blocks: a and b canonical ones, c every block of its isolated late branch", wins["a"] > 0 and wins["b"] > 0 and late_c == len(late),
           json.dumps({"canonical": wins, "c late": f"{late_c}/{len(late)}"}))
        signed = sum(1 for b in blocks if b["pocol"]["signed"])
        ok("signed and fallback templates both occur", 0 < signed < top, f"signed={signed}/{top}")
        # Frozen tips: the miners stop and a, b and c restart connected without miners, so no tip can
        # move. They must reach one totalWork (each holds a most-work tip, consensus.md:209), and their
        # canonical chains must agree up to the last height all share, at most one block below the
        # lowest head: two equal-work siblings may stay apart (D133's tie-break is local).
        stop_nodes(L)
        for l in L: run_node(l, miner=False)
        same = waited(lambda: len({tip(i)["work"] for i in range(3)}) == 1, 60, "one totalWork on the frozen tips")
        heads = [tip(i) for i in range(3)]
        ok("frozen tips: a, b and c reach the same totalWork", same, f"heads a/b/c={[short(t) for t in heads]}")
        pw = [pywork(i, heads[i]["number"]) for i in range(3)]
        ok("frozen tips: each head's totalWork is the W computed here from the committed targets of its canonical chain",
           all(bad is None and w == heads[i]["work"] for i, (w, bad) in enumerate(pw)), f"first mismatch a/b/c: {[bad for _, bad in pw]}")
        low = min(t["number"] for t in heads)
        shared = next(n for n in range(low, -1, -1) if len({canon(i, n) for i in range(3)}) == 1)
        mism = []
        for n in range(1, shared + 1):
            bs = [dn.rpc(8540 + i, "eth_getBlockByNumber", [hex(n), False]) for i in range(3)]
            if not all((b["hash"], b["stateRoot"], b["receiptsRoot"]) == (bs[0]["hash"], bs[0]["stateRoot"], bs[0]["receiptsRoot"]) for b in bs): mism.append(n)
        ok(f"frozen tips: identical canonical hash/stateRoot/receiptsRoot at heights 1..{shared}, at most one block below the lowest head",
           not mism and shared >= low - 1, f"lowest head h={low}; heads {'identical' if len({t['hash'] for t in heads}) == 1 else 'equal-work siblings'}; {str(mism[:5])}")
        # Full restart: deterministic replay. Each journal is replayed by a fresh process with no
        # peers and no miner; the replayed canonical chain must contain the live head observed
        # before the stop (a block imported between that read and SIGTERM may extend it), with the
        # same stateRoot/receiptsRoot, and two independent replays must agree byte-for-byte.
        live = {}
        for i, l in enumerate(L):
            hd = st(i)["head"]; b = dn.rpc(8540 + i, "eth_getBlockByNumber", [hex(hd["height"]), False])
            live[l] = (hd["height"], b["hash"], b["stateRoot"], b["receiptsRoot"])
        for l in L: stop_node(l)
        for i, l in enumerate(L):
            port = 8540 + i
            rp = subprocess.Popen([dn.BIN, "run", "--network", net, "--data", os.path.join(work, "data", l), "--name", l + "-replay", "--rpc", f"127.0.0.1:{port}"] + dn.EXTRA,
                                  stdout=open(os.path.join(work, f"{l}-replay.log"), "ab"), stderr=subprocess.STDOUT, start_new_session=True)
            try:
                wait(lambda: dn.rpc(port, "pocol_status") is not None, 60, f"replay {l}")
                hgt, hsh, sr, rr = live[l]; b = dn.rpc(port, "eth_getBlockByNumber", [hex(hgt), False])
                rhead = dn.rpc(port, "pocol_status")["head"]
                ok(f"replay of {l} reproduces its live head h={hgt} (hash/stateRoot/receiptsRoot)", (b["hash"], b["stateRoot"], b["receiptsRoot"]) == (hsh, sr, rr), f"replayed head h={rhead['height']}")
            finally:
                os.killpg(rp.pid, signal.SIGTERM); rp.wait()
            st1 = subprocess.run([dn.BIN, "status", "--network", net, "--data", os.path.join(work, "data", l)] + dn.EXTRA, capture_output=True, text=True, check=True).stdout
            st2 = subprocess.run([dn.BIN, "status", "--network", net, "--data", os.path.join(work, "data", l)] + dn.EXTRA, capture_output=True, text=True, check=True).stdout
            ok(f"two independent replays of {l} agree", json.loads(st1)["head"] == json.loads(st2)["head"] and json.loads(st1)["head"]["hash"] == rhead["hash"], json.loads(st1)["head"]["hash"][:14])
        print(json.dumps({"result": "PASS", "checks": len(checks), "elapsed_s": round(time.time() - t_start), "work": work}))
        return 0
    except Exception as e:
        error = e
    finally:
        dn.stop(work)
        for p in procs.values():
            try: os.killpg(p.pid, signal.SIGTERM); p.wait(30)
            except Exception: pass
    # The nodes have stopped: their logs and journals are whole when they are kept.
    kept = dn.keep_on_failure("e2e_devnet", work, {"error": str(error), "checks": [{"name": c[0], "pass": c[1], "detail": str(c[2])[:300]} for c in checks]})
    print(json.dumps({"result": "FAIL", "error": str(error), "checks": [c[0] for c in checks if c[1]], "elapsed_s": round(time.time() - t_start), "work": work, "kept": kept}))
    return 1

if __name__ == "__main__":
    sys.exit(main())
