#!/usr/bin/env python3
"""D-3/D-5 non-exhaustion arithmetic (scratch; PROPOSED analysis, nothing normative).

Usage: python3 -I caps_proof.py PATH/TO/caps-sustainability.json
Stdlib only, integers only, deterministic output. It does not import caps_model.py; the JSON it
reads (the study's committed vectors) is used only for the cross-check in section G.
"""
import json, sys, itertools

# ---------------------------------------------------------------- A. inputs (sources in the document)
GAS_LIMIT, BODY_MAX, TX_MAX, LOG_BLOCK_MAX = 8_000_000, 196_608, 131_072, 32_768   # governance.md:12, execution.md:82
G_TX = 21_000            # Cancun intrinsic base gas (execution.md:106 "GeneralStateTests for Cancun")
IDLE = 7                 # execution.md:97: empty blocks settle below 8 wei
BASEFEE0 = 10**9         # governance.md:12
COIN = 10**18
GiB = 1 << 30
PROFILES = {             # name: (chainId, T_blk, H_END, T_END - g_ts)
    "network":   (777001, 60, 129_600, 10_368_000),   # governance.md:4,5,16
    "devnet":    (777900, 4, 20_000, 10_000_000),     # lp4-execution/src/devnet.rs:36-38 (not normative)
    "caps/64":   (777903, 10, 4_000, 10**7),          # validation.md:2061 (sv-fix: T_END = g_ts+10^7, governance.md:155)
    "caps-test": (777903, 10, 4_000, 10**7),
}
OPTIONS = [  # (profile, option label, TX_CAP, TXBYTES_CAP, LOG_CAP)
    ("network", "current (governance.md:13; execution.md:82)", 10**7, 1 << 33, 1 << 30),
    ("network", "N1 only (LOG_CAP 2^32, TX caps unchanged)", 10**7, 1 << 33, 1 << 32),
    ("network", "O2 = N1+N2+N3 (2^32, 5e7, 24 GiB)", 5 * 10**7, 24 * GiB, 1 << 32),
    ("network", "O2 exact minima (R27-R29 with equality)", None, None, None),
    ("devnet", "current (devnet.rs:39-40)", 20_000, 1 << 24, 1 << 30),
    ("devnet", "N1 (LOG_CAP 2^32)", 20_000, 1 << 24, 1 << 32),
    ("devnet", "R28/R29 minima (O2 applied to devnet)", None, None, 1 << 32),
    ("caps/64", "current (validation.md:2062)", 20_000, 1 << 24, 1 << 30),
    ("caps-test", "caps-test build (validation.md:2065)", 20_000, 1 << 24, 1 << 24),
]

out = []
def p(*a): out.append(" ".join(str(x) for x in a))
def ceil_div(a, b): return -(-a // b)

# ---------------------------------------------------------------- B1. SSTORE lemma (EIP-2200/2929/3529)
SET, RESET, COLD, WARM, CLEARS = 20_000, 2_900, 2_100, 100, 4_800
def sstore(orig, cur, new):
    """(gas, refund delta) of one warm SSTORE under EIP-2200 with EIP-2929/3529 constants."""
    if cur == new: return WARM, 0
    if orig == cur:
        if orig == 0: return SET, 0
        return RESET, (CLEARS if new == 0 else 0)
    r = 0
    if orig != 0:
        if cur == 0: r -= CLEARS
        elif new == 0: r += CLEARS
    if orig == new:
        r += (SET - WARM) if orig == 0 else (RESET - WARM)
    return WARM, r
worst, cases = None, 0
for orig in (0, 1):
    for warmup in (("cold SSTORE", COLD), ("SLOAD first", COLD), ("access list", 1_900)):
        for n in range(1, 6):
            for seq in itertools.product((0, 1, 2), repeat=n):
                cur, paid, ref = orig, warmup[1], 0
                for v in seq:
                    g, d = sstore(orig, cur, v); paid += g; ref += d; cur = v
                cases += 1
                slack = paid - ref
                if worst is None or slack < worst[0]: worst = (slack, orig, warmup[0], seq)
p("B1 SSTORE lemma: sequences checked =", cases, "; min(paid - refund) per slot =", worst[0],
  "at original", worst[1], "warm-up", worst[2], "values", worst[3])
assert worst[0] >= 0
p("   => refund <= gas paid for that slot, so gasUsed(tx) >= intrinsic >= 21000 (cases above; slots are additive)")

# ---------------------------------------------------------------- B2. per-block transaction count
n_gas = GAS_LIMIT // G_TX
n_robust = 1 + (GAS_LIMIT - G_TX) // (G_TX * 4 // 5)   # only refund <= gasUsed/5 (EIP-3529), no SSTORE lemma
p("B2 tx per block: floor(GAS_LIMIT/21000) =", n_gas, "; without the SSTORE lemma (refund cap 1/5 only):", n_robust)

# ---------------------------------------------------------------- B3. per-block raw transaction bytes
def hdr(n): return 1 if n < 56 else 1 + (n.bit_length() + 7) // 8
CLASSES = [(1, 0, 55), (2, 56, 255), (3, 256, 65_535), (4, 65_536, TX_MAX)]   # item header size, min, max length
best = (0, None)
for k in range(1, 7):
    for combo in itertools.combinations_with_replacement(CLASSES, k):
        lo, hi = sum(c[1] for c in combo), sum(c[2] for c in combo)
        s = min(hi, BODY_MAX)
        while s >= lo:
            inner = s + sum(c[0] for c in combo)
            mid = hdr(inner) + inner + 1          # + empty evidence list 0xc0 (evidence only adds bytes)
            if hdr(mid) + mid <= BODY_MAX: break
            s -= 1
        if s >= lo and s > best[0]: best = (s, k, [c[0] for c in combo])
TXB_BLOCK = best[0]
p("B3 raw tx bytes per block: max =", TXB_BLOCK, "with", best[1], "transactions (item headers", best[2], "); trivial bound BODY_MAX =", BODY_MAX)

# ---------------------------------------------------------------- B4. per-block charged log bytes
words = ceil_div(LOG_BLOCK_MAX - 36, 32)
log_fill_gas = G_TX + 3 + 2 + 375 + 8 * (LOG_BLOCK_MAX - 36) + 3 * words + words * words // 512
max_charge_by_gas = (GAS_LIMIT - G_TX - 375) // 8     # data bytes alone at 8 gas/byte, ignoring memory
p("B4 charged log bytes per block <= LOG_BLOCK_MAX =", LOG_BLOCK_MAX, "(user_phase.rs:196 halts above it); gas alone would allow up to",
  max_charge_by_gas, "; one LOG0 filling it costs", log_fill_gas, "gas")

# ---------------------------------------------------------------- C. caps x options
PER = {"TX_CAP": n_gas, "TXBYTES_CAP": TXB_BLOCK, "LOG_CAP": LOG_BLOCK_MAX}
p("")
p("C  verdict: UNREACHABLE (proof) iff CAP >= H_END x per-block max; else REACHABLE (blocks to fill = ceil(CAP/max)).")
rows = []
for prof, label, txc, txbc, logc in OPTIONS:
    cid, tblk, hend, tend = PROFILES[prof]
    if txc is None: txc = hend * n_gas
    if txbc is None: txbc = hend * TXB_BLOCK
    if logc is None: logc = hend * LOG_BLOCK_MAX
    p("--", prof, "|", label, "| H_END", hend)
    for cap_name, cap in (("TX_CAP", txc), ("TXBYTES_CAP", txbc), ("LOG_CAP", logc)):
        per = PER[cap_name]; need = hend * per
        if cap >= need:
            v = "UNREACHABLE  margin %d (%.2f%% of cap)" % (cap - need, 100 * (cap - need) / cap)
        else:
            b = ceil_div(cap, per)
            v = "REACHABLE    filled after %d full blocks (%d s = %.1f d); organic avg %.3f/block = %.2f%% of max" % (
                b, b * tblk, b * tblk / 86400, cap / hend, 100 * cap / hend / per)
        extra = ""
        if cap_name == "TX_CAP":
            extra = " | robust (475/block): %s" % ("UNREACHABLE" if cap >= hend * n_robust else "NOT PROVEN (needs %d)" % (hend * n_robust))
        if cap_name == "TXBYTES_CAP":
            extra = " | with BODY_MAX bound: %s" % ("UNREACHABLE" if cap >= hend * BODY_MAX else "NOT PROVEN (needs %d)" % (hend * BODY_MAX))
        p("   %-11s cap %15d  H_END*max %15d  %s%s" % (cap_name, cap, need, v, extra))
        rows.append((prof, label, cap_name, cap, need, cap >= need))

# ---------------------------------------------------------------- D. attacker cost (burned base fee, zero tips)
def next_base(b, used, limit=GAS_LIMIT):
    t = limit // 2
    if used == t: return b
    d = b * abs(used - t) // t // 8
    return b + max(d, 1) if used > t else b - d
def burn(blocks, gas, b0):
    b, c = b0, 0
    for _ in range(blocks): c += b * gas; b = next_base(b, gas)
    return c
def legacy_len(chain_id, gas_price, gas, d):
    def be_len(n): return (n.bit_length() + 7) // 8
    def int_item(n): return 1 if 0 < n < 0x80 else 1 + be_len(n) if n else 1
    v = chain_id * 2 + 35
    data = (1 if d == 0 else hdr(d)) + d
    payload = int_item(0) + int_item(gas_price) + int_item(gas) + 21 + int_item(0) + data + int_item(v) + 33 + 33
    return hdr(payload) + payload
def big_tx_gas(chain_id, raw):
    d = raw - 110
    while legacy_len(chain_id, IDLE, 1_000_000, d + 1) <= raw: d += 1
    while legacy_len(chain_id, IDLE, 1_000_000, d) > raw: d -= 1
    return G_TX + 4 * d, d
b, n_empty = BASEFEE0, 0
while b > IDLE: b = next_base(b, 0); n_empty += 1
floor_ok = all(next_base(x, u) >= IDLE for x in range(IDLE, 16) for u in (0, 1, GAS_LIMIT // 4, GAS_LIMIT // 2 - 1))
p("")
p("D  empty blocks from baseFee0 1e9 to the idle fee:", n_empty, "; fee never falls below 7 from >= 7 at u <= target:", floor_ok)
for prof, (cid, tblk, hend, tend) in PROFILES.items():
    caps = [r for r in OPTIONS if r[0] == prof][0]
    _, _, txc, txbc, logc = caps
    tx_min = IDLE * G_TX * txc
    tx_model = burn(ceil_div(txc, 190), 190 * G_TX, IDLE)
    g1, d1 = big_tx_gas(cid, TX_MAX)
    txb_cheap = IDLE * g1 * ceil_div(txbc, TX_MAX)
    model_block_gas = big_tx_gas(cid, 131_072)[0] + big_tx_gas(cid, 65_520)[0]
    txb_model = burn(ceil_div(txbc, TXB_BLOCK), model_block_gas, IDLE)
    log_cost = burn(ceil_div(logc, LOG_BLOCK_MAX), log_fill_gas, IDLE)
    p("   %-9s TX_CAP  min burn 7*21000*TX_CAP = %d wei; 190-tx blocks from 7 wei: %d wei; from 1 gwei: %d wei (%.3f coins)" % (
        prof, tx_min, tx_model, burn(ceil_div(txc, 190), 190 * G_TX, BASEFEE0), burn(ceil_div(txc, 190), 190 * G_TX, BASEFEE0) / COIN))
    p("   %-9s TXBYTES 2-tx blocks (%d gas) from 7 wei: %d wei; cheaper 1-tx blocks of TX_MAX raw (%d gas, %d blocks): %d wei" % (
        prof, model_block_gas, txb_model, g1, ceil_div(txbc, TX_MAX), txb_cheap))
    if ceil_div(logc, LOG_BLOCK_MAX) <= hend:
        p("   %-9s LOG_CAP one LOG0 per block (%d gas), %d blocks from 7 wei: %d wei" % (prof, log_fill_gas, ceil_div(logc, LOG_BLOCK_MAX), log_cost))
    else:
        p("   %-9s LOG_CAP unreachable (needs %d blocks > H_END %d): no attack cost" % (prof, ceil_div(logc, LOG_BLOCK_MAX), hend))

# ---------------------------------------------------------------- E. height vs time
p("")
for prof, (cid, tblk, hend, tend) in PROFILES.items():
    p("E  %-9s H_END*T_blk = %d s (%.1f d); T_END-g_ts = %d s (%.1f d); time ends first only if the mean interval > %.2f s (%.2fx T_blk)" % (
        prof, hend * tblk, hend * tblk / 86400, tend, tend / 86400, tend / hend, tend / hend / tblk))

# ---------------------------------------------------------------- F. LOG_CAP neutrality between two values
p("")
p("F  LOG_CAP values A and B give identical execution on every branch iff H_END*LOG_BLOCK_MAX <= min(A, B)")
p("   largest H_END on which 2^30 never binds:", (1 << 30) // LOG_BLOCK_MAX, "; 2^32:", (1 << 32) // LOG_BLOCK_MAX, "; 2^24:", (1 << 24) // LOG_BLOCK_MAX)
for prof, (cid, tblk, hend, tend) in PROFILES.items():
    p("   %-9s H_END %6d: 2^30 vs 2^32 neutral: %s" % (prof, hend, hend * LOG_BLOCK_MAX <= (1 << 30)))

# ---------------------------------------------------------------- G. cross-check with the study's vectors
p("")
vec = json.load(open(sys.argv[1], encoding="utf-8"))["reachability"]
mism = 0
def cmp(what, mine, theirs):
    global mism
    ok = mine == theirs
    if not ok: mism += 1
    p("   %-58s mine %-22s model %-22s %s" % (what, mine, theirs, "=" if ok else "DIFF"))
p("G  cross-check with vectors/caps-sustainability.json")
for prof, label, cap_name, cap, need, unreach in rows:
    if not label.startswith(("current", "caps-test")): continue
    r = vec[prof][cap_name]
    cmp("%s %s perBlockMax" % (prof, cap_name), PER[cap_name], r["perBlockMax"])
    cmp("%s %s value" % (prof, cap_name), cap, r["value"])
    cmp("%s %s sizeToCoverH_END" % (prof, cap_name), need, r["sizeToCoverH_END"])
    cmp("%s %s reachableBeforeH_END" % (prof, cap_name), not unreach, r["reachableBeforeH_END"])
    if not unreach:
        cmp("%s %s minBlocks" % (prof, cap_name), ceil_div(cap, PER[cap_name]), r["minBlocks"])
for prof in PROFILES:
    cmp("%s emptyBlocksFromBaseFee0ToIdle" % prof, n_empty, vec[prof]["emptyBlocksFromBaseFee0ToIdle"])
    cmp("%s logFill gas" % prof, log_fill_gas, vec[prof]["logFill"]["gas"])
    cmp("%s txBytesBlock rawLengths sum" % prof, TXB_BLOCK, sum(vec[prof]["txBytesBlock"]["rawLengths"]))
nr = vec["network"]
cmp("network TX_CAP attack from 7 wei (190-tx blocks)", str(burn(52_632, 190 * G_TX, IDLE)), nr["TX_CAP"]["attackBaseFeeWei_fromIdleBase"])
cmp("network TX_CAP attack from 1 gwei", str(burn(52_632, 190 * G_TX, BASEFEE0)), nr["TX_CAP"]["attackBaseFeeWei_fromBaseFee0"])
cmp("network TXBYTES attack from 7 wei (2-tx blocks)", str(burn(43_695, 827_528, IDLE)), nr["TXBYTES_CAP"]["attackBaseFeeWei_fromIdleBase"])
cmp("network LOG_CAP attack from 7 wei", str(burn(32_768, log_fill_gas, IDLE)), nr["LOG_CAP"]["attackBaseFeeWei_fromIdleBase"])
cmp("network TX_CAP fullBlocksUntilOneBlockCostsASubsidy", (lambda: (lambda f: f(f, IDLE, 0))(lambda f, b, n: n if b * n_gas * G_TX >= 2 * COIN else f(f, next_base(b, n_gas * G_TX), n + 1)))(), nr["TX_CAP"]["fullBlocksUntilOneBlockCostsASubsidy"])
p("G  mismatches:", mism)
print("\n".join(out))
