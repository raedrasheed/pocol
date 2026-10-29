#!/usr/bin/env python3
"""Independent model for CAPS-SUSTAINABILITY (audit repair, round 2). Status: PROPOSED, not approved.

1. Its own Keccak-f[1600] (round constants and rotation offsets derived from their FIPS 202
   definitions), Keccak-256 (original Keccak padding 0x01 .. 0x80) and SHA3-256 (padding 0x06; used
   only to cross-check the permutation against hashlib over multi-block inputs).
2. Its own strict RLP and GenesisSpec v1 encoder/decoder, written from consensus.md:7-34. It rebuilds
   GSV1 twice, from the literal segments of validation.md:64-73 and from its field values, and checks
   both against the 341 bytes and the recorded three-library H_GSV1.
3. genesisHash of the variants TX_CAP+1 and TXBYTES_CAP+1 (GSV1 and the devnet), the CP field names of
   the specification (consensus.md:19-25) and of the Rust table (read as text only, to list its names),
   and the absence of LOG_CAP (and LOG_BLOCK_MAX) from both.
4. The devnet genesisHash, from the devnet GenesisSpec bytes the Rust code produces
   (vectors/caps-devnet-spec.hex, pinned by lp4-execution/tests/caps_identity.rs), and sysCodeHash
   recomputed from the bundled system programs.
5. Cap reachability before H_END for every profile, and the attacker's base-fee cost under the
   London rule (execution.md:97), by exact integer simulation.

It does not import or execute the Rust implementation. Usage:
    python3 -I caps_model.py [OUT.json]     (default: ../vectors/caps-sustainability.json)
The output is deterministic.
"""
import hashlib, json, math, os, re, sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.normpath(os.path.join(HERE, "..", "..", "..", ".."))
def path(*p): return os.path.join(ROOT, *p)
def read(*p):
    with open(path(*p), encoding="utf-8") as f: return f.read()

# ---------------------------------------------------------------- Keccak-f[1600] (FIPS 202 section 3)
M64 = (1 << 64) - 1
def rol(v, n): n %= 64; return ((v << n) | (v >> (64 - n))) & M64 if n else v

def rc_bit(t):
    """rc(t) of FIPS 202 Algorithm 5 (an LFSR over GF(2))."""
    if t % 255 == 0: return 1
    r = [1, 0, 0, 0, 0, 0, 0, 0]
    for _ in range(t % 255):
        r = [0] + r
        r[0] ^= r[8]; r[4] ^= r[8]; r[5] ^= r[8]; r[6] ^= r[8]
        r = r[:8]
    return r[0]

ROUND_CONSTANTS = []
for ir in range(24):
    c = 0
    for j in range(7): c |= rc_bit(j + 7 * ir) << ((1 << j) - 1)
    ROUND_CONSTANTS.append(c)

ROTATIONS = [[0] * 5 for _ in range(5)]   # ROTATIONS[x][y], FIPS 202 Algorithm 2
_x, _y = 1, 0
for _t in range(24):
    ROTATIONS[_x][_y] = ((_t + 1) * (_t + 2) // 2) % 64
    _x, _y = _y, (2 * _x + 3 * _y) % 5

def keccak_f(a):
    for rnd in range(24):
        c = [a[x][0] ^ a[x][1] ^ a[x][2] ^ a[x][3] ^ a[x][4] for x in range(5)]
        d = [c[(x - 1) % 5] ^ rol(c[(x + 1) % 5], 1) for x in range(5)]
        a = [[a[x][y] ^ d[x] for y in range(5)] for x in range(5)]
        b = [[0] * 5 for _ in range(5)]
        for x in range(5):
            for y in range(5): b[y][(2 * x + 3 * y) % 5] = rol(a[x][y], ROTATIONS[x][y])
        a = [[b[x][y] ^ ((~b[(x + 1) % 5][y]) & b[(x + 2) % 5][y] & M64) for y in range(5)] for x in range(5)]
        a[0][0] ^= ROUND_CONSTANTS[rnd]
    return a

def sponge256(msg, domain):
    rate = 136
    p = bytearray(msg) + bytes([domain])
    p += bytes((-len(p)) % rate)
    p[-1] |= 0x80
    a = [[0] * 5 for _ in range(5)]
    for off in range(0, len(p), rate):
        blk = p[off:off + rate]
        for i in range(rate // 8):
            a[i % 5][i // 5] ^= int.from_bytes(blk[8 * i:8 * i + 8], "little")
        a = keccak_f(a)
    out = b"".join(a[i % 5][i // 5].to_bytes(8, "little") for i in range(4))
    return out

def keccak256(m): return sponge256(m, 0x01)
def sha3_256(m): return sponge256(m, 0x06)

# ---------------------------------------------------------------- strict RLP
def be(n): return n.to_bytes((n.bit_length() + 7) // 8, "big") if n else b""
def rlp_bytes(b):
    if len(b) == 1 and b[0] < 0x80: return bytes(b)
    if len(b) < 56: return bytes([0x80 + len(b)]) + b
    l = be(len(b)); return bytes([0xb7 + len(l)]) + l + b
def rlp_list(items):
    p = b"".join(items)
    if len(p) < 56: return bytes([0xc0 + len(p)]) + p
    l = be(len(p)); return bytes([0xf7 + len(l)]) + l + p
def rlp_int(n): return rlp_bytes(be(n))

def rlp_decode(b):
    """Strict decoder: minimal lengths, no wrapped single byte, exact lengths, no trailing bytes."""
    def item(i):
        if i >= len(b): raise ValueError("truncated")
        t = b[i]
        if t < 0x80: return b[i:i + 1], i + 1
        def long(n_len, base):
            if i + 1 + n_len > len(b): raise ValueError("truncated length")
            lb = b[i + 1:i + 1 + n_len]
            if lb[0] == 0: raise ValueError("leading zero in length")
            n = int.from_bytes(lb, "big")
            if n < 56: raise ValueError("long form for short item")
            return i + 1 + n_len, n
        if t < 0xb8:
            s, n = i + 1, t - 0x80
            if n == 1 and s < len(b) and b[s] < 0x80: raise ValueError("wrapped single byte")
            if s + n > len(b): raise ValueError("truncated string")
            return b[s:s + n], s + n
        if t < 0xc0:
            s, n = long(t - 0xb7, 0);
            if s + n > len(b): raise ValueError("truncated string")
            return b[s:s + n], s + n
        if t < 0xf8: s, n = i + 1, t - 0xc0
        else: s, n = long(t - 0xf7, 0)
        if s + n > len(b): raise ValueError("truncated list")
        out, j = [], s
        while j < s + n:
            x, j = item(j)
            if j > s + n: raise ValueError("item crosses list end")
            out.append(x)
        return out, j
    v, end = item(0)
    if end != len(b): raise ValueError("trailing bytes")
    return v

def rlp_uint(x):
    if not isinstance(x, (bytes, bytearray)) or (len(x) > 0 and x[0] == 0): raise ValueError("gsInt")
    return int.from_bytes(x, "big")

# ---------------------------------------------------------------- GenesisSpec v1, from consensus.md
def spec_schema():
    """GenesisSpec top-level fields (consensus.md:7) and the 52 CP (name, width) pairs (consensus.md:19-25)."""
    text = read("development", "m1", "reference", "consensus.md").split("\n")
    top = re.search(r"GenesisSpec = RLP\(\[([^\]]+)\]\)", text[6]).group(1)
    fields = [f.strip() for f in top.split(",")]
    cp = []
    for line in text[18:25]:
        cp += [(n, int(w[1:])) for n, w in re.findall(r"([A-Za-z_][A-Za-z0-9_]*) (u8|u16|u32|u64|u256)\b", line)]
    assert fields == ["specVersion", "chainId", "CP", "allocRoot", "M_0List", "sysCodeHash"], fields
    assert len(cp) == 52, len(cp)
    return fields, cp

def rust_schema_names():
    """Names of the Rust CP table (p1_genesis_spec.rs CP_SCHEMA), read as text only."""
    src = read("prototypes", "lp4-execution", "src", "p1_genesis_spec.rs")
    body = src[src.index("pub const CP_SCHEMA"):]; body = body[:body.index("];")]
    return re.findall(r'f\("([^"]+)"', body)

def gs_encode(chain_id, cp, alloc_root, m0, sys_code_hash):
    assert len(cp) == 52 and len(alloc_root) == 32 and len(sys_code_hash) == 32
    return rlp_list([rlp_int(1), rlp_int(chain_id), rlp_list([rlp_int(v) for v in cp]), rlp_bytes(alloc_root),
                     rlp_list([rlp_list([rlp_bytes(i), rlp_bytes(r)]) for i, r in m0]), rlp_bytes(sys_code_hash)])

def gs_decode(b, schema):
    top = rlp_decode(b)
    if not isinstance(top, list) or len(top) != 6: raise ValueError("gsCount")
    if rlp_uint(top[0]) != 1: raise ValueError("gsVersion")
    chain_id = rlp_uint(top[1])
    if not isinstance(top[2], list) or len(top[2]) != 52: raise ValueError("gsCount CP")
    cp = [rlp_uint(x) for x in top[2]]
    for (name, w), v in zip(schema, cp):
        if v.bit_length() > w: raise ValueError("gsRange " + name)
    m0 = [(e[0], e[1]) for e in top[4]]
    if any(len(i) != 20 or len(r) != 20 for i, r in m0) or len(top[3]) != 32 or len(top[5]) != 32: raise ValueError("gsLen")
    return {"chainId": chain_id, "cp": cp, "allocRoot": top[3], "m0": m0, "sysCodeHash": top[5]}

# ---------------------------------------------------------------- GSV1 (validation.md:64-73)
def gsv1_literal():
    lines = read("development", "m1", "reference", "validation.md").split("\n")
    start = next(i for i, l in enumerate(lines) if l.startswith("GSV1 (341"))
    tok = re.compile(r"‹([0-9a-f]{2})×(\d+)›|\b([0-9a-f]{2})\b")
    out = bytearray(); used = []
    for l in lines[start + 1:start + 9]:
        if ":" not in l: continue
        body = l.split(":", 1)[1]
        for rep, cnt, byte in tok.findall(body):
            out += bytes([int(rep, 16)]) * int(cnt) if rep else bytes([int(byte, 16)])
        used.append(l.strip())
    lengths = re.findall(r"(?<![A-Za-z_0-9])(\d+)", lines[start + 9])
    return bytes(out), used, [int(x) for x in lengths]

GSV1_VALUES = dict(   # transcribed from the GSV1 segments, in CP order (consensus.md:19-25)
    g_ts=1700000000, target_g=1 << 240, T_blk=10, tau=600, nonceMode=1, c=2, D_att=60, dFbWait=6, m=32, kappa=2,
    S_max=256, D=16, W_w=64, alpha_bp=1000, gamma_bp=500, subsidy=80000, B_reg=10**20,
    R_max=2, M_max=256, M_min=1, REG_RECORDS_MAX=16384, SWEEP_MAX=16, SWEEP_SCAN=64, REG_OPS=16, W_act=128, Theta_inact=1024,
    E_win=2048, U=2200, E_max=4, EVID_BYTES_MAX=8192,
    GAS_LIMIT=8000000, baseFee0=10**9, TX_MAX=131072, BODY_MAX=196608, B_code_max=32768, CODE_CAP=1 << 30,
    TXBYTES_CAP=1 << 33, TX_CAP=10**7, SLOT_CAP=4 * 10**6, ACCT_CAP=10**6,
    SYS_SLOT_MAX=131072, SYS_KEYS_BLOCK_MAX=2048, SYS_KEYS_USER_MAX=256, SYS_ACCT_BLOCK_MAX=512, G_SYS=96 * 10**6,
    H_END=200, T_END=1710000000, H_CLOSE=40, T_CLOSE=1710000000, Z_close=160, RET=128, P2=2)
GSV1_TAIL = dict(chainId=777902, allocRoot=b"\x22" * 32, m0=[(b"\xa1" * 20, b"\xb1" * 20), (b"\xa2" * 20, b"\xb2" * 20)], sysCodeHash=b"\x33" * 32)

def variant(base, schema, field, delta):
    k = [n for n, _ in schema].index(field)
    cp = list(base["cp"]); cp[k] += delta
    b = gs_encode(base["chainId"], cp, base["allocRoot"], base["m0"], base["sysCodeHash"])
    return {"field": field, "index": k + 1, "value": cp[k], "bytes": b.hex(), "genesisHash": keccak256(b).hex()}

# ---------------------------------------------------------------- system programs (sysCodeHash, consensus.md:41)
def system_programs():
    v = json.loads(read("prototypes", "lp4-execution", "tests", "data", "system-v1.json"))
    codes = [bytes.fromhex(p["runtime"][2:] if p["runtime"].startswith("0x") else p["runtime"]) for p in v["programs"]]
    names = [p.get("name") for p in v["programs"]]
    h = keccak256(rlp_list([rlp_bytes(c) for c in codes] + [b"\x80"]))
    scan = []
    for n, c in zip(names, codes):
        i, logs = 0, 0
        while i < len(c):
            op = c[i]
            if 0x60 <= op <= 0x7f: i += op - 0x5f + 1; continue   # PUSH1..PUSH32 immediates are data
            if 0xa0 <= op <= 0xa4: logs += 1
            i += 1
        scan.append({"program": n, "bytes": len(c), "logOpcodes": logs})
    return h, scan

# ---------------------------------------------------------------- reachability (execution.md, governance.md)
G_TX, G_LOG, G_LOGDATA, G_MEM = 21000, 375, 8, 3            # Cancun schedule (execution.md:106)
LOG_BLOCK_MAX = 32768                                        # execution.md:82 (charged bytes, LOG-ACCT-01)

def legacy_len(chain_id, nonce, gas_price, gas, data_len, to=True):
    """Raw length of a signed EIP-155 legacy transaction with 32-byte r and s (execution.md:96)."""
    v = chain_id * 2 + 35
    payload = (len(rlp_int(nonce)) + len(rlp_int(gas_price)) + len(rlp_int(gas)) + (21 if to else 1) + 1 +
               len(rlp_bytes(b"\0" * data_len)) + len(rlp_int(v)) + 33 + 33)
    return len(rlp_list([b"\0" * payload]))

def _hdr(n): return 1 if n < 56 else 1 + len(be(n))
def body_len(lens):
    """Encoded length of Body = RLP([txs, evidence]) with empty evidence (consensus.md:51), items of these raw lengths (>= 56)."""
    inner = sum(_hdr(n) + n for n in lens)
    mid = _hdr(inner) + inner + 1
    return _hdr(mid) + mid

def max_tx_bytes_per_block(p):
    """Largest sum of raw transaction lengths in one valid body (encoded body <= BODY_MAX, each <= TX_MAX)."""
    best = (0, [])
    for k in range(1, 5):
        lens = [p["TX_MAX"]] * (k - 1)
        lo, hi = 0, p["TX_MAX"]
        while lo < hi:
            mid = (lo + hi + 1) // 2
            if body_len(lens + [mid]) <= p["BODY_MAX"]: lo = mid
            else: hi = mid - 1
        if body_len(lens + [lo]) <= p["BODY_MAX"] and sum(lens) + lo > best[0]: best = (sum(lens) + lo, lens + [lo])
    return best

def calldata_gas_zero(raw_len, chain_id):
    """Gas of a transfer whose calldata is zero bytes filling `raw_len` raw bytes (4 gas per zero byte)."""
    d = raw_len - legacy_len(chain_id, 0, 1, 1_000_000, 0)
    while legacy_len(chain_id, 0, 1, 1_000_000, d) > raw_len: d -= 1
    return G_TX + 4 * d, d

def log_fill_gas():
    """One LOG0 of d zero bytes charged exactly LOG_BLOCK_MAX (36+d): PUSH2 d, PUSH0, LOG0, STOP."""
    d = LOG_BLOCK_MAX - 36; words = (d + 31) // 32
    mem = G_MEM * words + words * words // 512
    return G_TX + 3 + 2 + G_LOG + G_LOGDATA * d + mem, d

def next_base(base, used, limit):
    """London (execution.md:97; EIP-1559): target = limit/2, change = base*|used-target|/target/8, +1 minimum."""
    t = limit // 2
    if used == t: return base
    delta = base * abs(used - t) // t // 8
    return base + max(delta, 1) if used > t else base - delta

def attack(blocks, gas, base0, limit):
    """Base fee burned (economics.md:66) by `blocks` blocks of `gas` each, from base fee base0."""
    base, cost = base0, 0
    for _ in range(blocks):
        cost += base * gas; base = next_base(base, gas, limit)
    return cost, base

PROFILES = [
    # name, source, chainId, T_blk, H_END, GAS_LIMIT, BODY_MAX, TX_MAX, TX_CAP, TXBYTES_CAP, LOG_CAP
    dict(name="network", source="governance.md:4,5,12,13,16; execution.md:82", chainId=777001, T_blk=60, H_END=129600,
         GAS_LIMIT=8_000_000, BODY_MAX=196608, TX_MAX=131072, TX_CAP=10**7, TXBYTES_CAP=1 << 33, LOG_CAP=1 << 30),
    dict(name="devnet", source="lp4-execution/src/devnet.rs:36-42 (D-A13-PROFILE, not normative)", chainId=777900, T_blk=4, H_END=20000,
         GAS_LIMIT=8_000_000, BODY_MAX=196608, TX_MAX=131072, TX_CAP=20000, TXBYTES_CAP=1 << 24, LOG_CAP=1 << 30),
    dict(name="caps/64", source="validation.md:2061-2062 on sv-fix = GSV1 CP (validation.md:79)", chainId=777903, T_blk=10, H_END=4000,
         GAS_LIMIT=8_000_000, BODY_MAX=196608, TX_MAX=131072, TX_CAP=20000, TXBYTES_CAP=1 << 24, LOG_CAP=1 << 30),
    dict(name="caps-test", source="caps/64 with the caps-test build constant LOG_CAP=2^24 (validation.md:2065)", chainId=777903, T_blk=10, H_END=4000,
         GAS_LIMIT=8_000_000, BODY_MAX=196608, TX_MAX=131072, TX_CAP=20000, TXBYTES_CAP=1 << 24, LOG_CAP=1 << 24),
]
IDLE_BASE = 7   # execution.md:97: empty blocks settle below 8 wei
COIN = 10**18

def reach(p):
    tgt = p["GAS_LIMIT"] // 2
    tx_gas = p["GAS_LIMIT"] // G_TX; tx_gas_target = tgt // G_TX
    min_tx = legacy_len(p["chainId"], 0, IDLE_BASE, G_TX, 0)
    lo, hi = 0, p["BODY_MAX"]
    while lo < hi:
        mid = (lo + hi + 1) // 2
        if body_len([min_tx] * mid) <= p["BODY_MAX"]: lo = mid
        else: hi = mid - 1
    tx_bytes_bound = lo
    per_block_tx = min(tx_gas, tx_bytes_bound)
    bmax, lens = max_tx_bytes_per_block(p)
    bgas = sum(calldata_gas_zero(n, p["chainId"])[0] for n in lens)
    lgas, ldata = log_fill_gas()
    out = {}
    for cap, unit, per_max, per_tgt, gas_max, gas_tgt in [
            ("TX_CAP", "transactions", per_block_tx, tx_gas_target, per_block_tx * G_TX, tx_gas_target * G_TX),
            ("TXBYTES_CAP", "raw transaction bytes (D49)", bmax, bmax, bgas, bgas),
            ("LOG_CAP", "charged log bytes (LOG-ACCT-01)", LOG_BLOCK_MAX, LOG_BLOCK_MAX, lgas, lgas)]:
        v = p[cap]
        b_max, b_tgt = -(-v // per_max), -(-v // per_tgt)
        r = {"value": v, "unit": unit, "perBlockMax": per_max, "gasPerBlockAtMax": gas_max,
             "minBlocks": b_max, "minSeconds": b_max * p["T_blk"], "reachableBeforeH_END": b_max <= p["H_END"],
             "perBlockAtTarget": per_tgt, "gasPerBlockAtTarget": gas_tgt, "blocksAtTarget": b_tgt, "secondsAtTarget": b_tgt * p["T_blk"],
             "reachableAtTargetBeforeH_END": b_tgt <= p["H_END"],
             "organicPerBlock": round(v / p["H_END"], 3), "organicShareOfMax": round(v / p["H_END"] / per_max, 4),
             "sizeToCoverH_END": per_max * p["H_END"]}
        if b_tgt <= p["H_END"]:
            for label, b0 in [("fromIdleBase", IDLE_BASE), ("fromBaseFee0", 10**9)]:
                cost, end = attack(b_tgt, gas_tgt, b0, p["GAS_LIMIT"])
                r["attackBaseFeeWei_" + label] = str(cost); r["attackBaseFeeCoins_" + label] = round(cost / COIN, 9); r["endBaseFee_" + label] = end
        out[cap] = r
    # Sustaining more than the target: the base fee compounds by 1 + (used-target)/target/8 per block.
    base = IDLE_BASE; n = 0
    while base * tx_gas * G_TX < 2 * 10**18 and n < 10**6: base = next_base(base, tx_gas * G_TX, p["GAS_LIMIT"]); n += 1
    out["TX_CAP"]["fullBlocksUntilOneBlockCostsASubsidy"] = n
    # A patient attacker first lets the base fee decay: empty blocks are free (review CV-7).
    base = 10**9; n = 0
    while base > IDLE_BASE: base = next_base(base, 0, p["GAS_LIMIT"]); n += 1
    out["emptyBlocksFromBaseFee0ToIdle"] = n
    out["minTxBytes"] = min_tx; out["txBytesBlock"] = {"rawLengths": lens, "gas": bgas}
    out["logFill"] = {"LOG0dataBytes": ldata, "gas": lgas}
    return out

# ---------------------------------------------------------------- main
def main():
    out_path = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "..", "vectors", "caps-sustainability.json")
    checks = []
    def check(name, ok, detail=""):
        checks.append({"check": name, "pass": bool(ok), "detail": detail})
        if not ok: raise SystemExit("FAIL " + name + " " + str(detail))

    # 1. Keccak-256 and the permutation.
    val = read("development", "m1", "reference", "validation.md")
    ks = re.findall(r"K(\d): keccak256\(([^)]*)\) = ([0-9a-f]{64})", val)
    check("K1-K3 present in validation.md", len(ks) == 3, [k[0] for k in ks])
    for k, arg, want in ks:
        m = b"" if not arg.startswith("0x") else bytes.fromhex(arg[2:])
        check("K" + k, keccak256(m).hex() == want, want)
    std = [("empty", b"", "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"),
           ("abc", b"abc", "4e03657aea45a94fc7d47ba826c8d667c0d1e6e33a64a036ec44f58fa12d6c45"),
           ("fox (43 B)", b"The quick brown fox jumps over the lazy dog", "4d741b6f1eb29cb2a9b9911c82f56fa8d73b04959d3d9d222895df6c0b28aa15")]
    for name, m, want in std: check("Keccak-256 " + name, keccak256(m).hex() == want, want)
    pattern = bytes((i * 7 + 3) & 0xff for i in range(700))
    bad = [n for n in range(len(pattern) + 1) if sha3_256(pattern[:n]) != hashlib.sha3_256(pattern[:n]).digest()]
    check("permutation = hashlib SHA3-256 for lengths 0..700 (up to 6 rate blocks with padding)", not bad, bad[:5])
    check("round constant 23", ROUND_CONSTANTS[23] == 0x8000000080008008, hex(ROUND_CONSTANTS[23]))

    # 2. GSV1.
    fields, schema = spec_schema(); names = [n for n, _ in schema]
    lit, used, lengths = gsv1_literal()
    supp = json.loads(read("development", "m1", "m1-draft-0.22", "vectors", "v3-e05-supplement.json"))["GSV1"]
    check("GSV1 literal is 341 bytes", len(lit) == 341, len(lit))
    check("GSV1 literal SHA-256 = E05 inputSha256", hashlib.sha256(lit).hexdigest() == supp["inputSha256"], supp["inputSha256"])
    built = gs_encode(GSV1_TAIL["chainId"], [GSV1_VALUES[n] for n in names], GSV1_TAIL["allocRoot"], GSV1_TAIL["m0"], GSV1_TAIL["sysCodeHash"])
    check("GSV1 from field values = literal bytes", built == lit)
    dec = gs_decode(lit, schema)
    check("GSV1 lengths CP 179, M_0List 88, payload 338", lengths == [179, 88, 338] and len(rlp_list([rlp_int(v) for v in dec["cp"]])) == 179
          and len(rlp_list([rlp_list([rlp_bytes(i), rlp_bytes(r)]) for i, r in dec["m0"]])) == 88 and lit[:3] == b"\xf9\x01\x52", lengths)
    h_gsv1 = keccak256(lit).hex()
    check("H_GSV1 = the recorded three-library value", h_gsv1 == supp["keccak256"], supp["keccak256"])
    gsv1_vars = [variant(dec, schema, "TX_CAP", 1), variant(dec, schema, "TXBYTES_CAP", 1)]
    for v in gsv1_vars: check("GSV1 " + v["field"] + "+1 changes genesisHash", v["genesisHash"] != h_gsv1, v["genesisHash"])

    # 3. Schemas: LOG_CAP is not a GenesisSpec or CP field.
    rust = rust_schema_names()
    check("Rust CP table names = specification CP names", rust == names)
    for absent in ["LOG_CAP", "LOG_BLOCK_MAX"]:
        check(absent + " absent from GenesisSpec, CP (spec) and the Rust table", absent not in fields + names + rust)

    # 4. Devnet (Rust-produced bytes) and sysCodeHash.
    dev_hex = read("development", "audit-2026-10-22", "proposals", "vectors", "caps-devnet-spec.hex").strip()
    dev = bytes.fromhex(dev_hex[2:] if dev_hex.startswith("0x") else dev_hex)
    d = gs_decode(dev, schema)
    reenc = gs_encode(d["chainId"], d["cp"], d["allocRoot"], d["m0"], d["sysCodeHash"])
    check("devnet spec re-encodes byte for byte", reenc == dev)
    cpd = dict(zip(names, d["cp"]))
    check("devnet CP: TX_CAP 20000, TXBYTES_CAP 2^24, H_END 20000, T_blk 4",
          (cpd["TX_CAP"], cpd["TXBYTES_CAP"], cpd["H_END"], cpd["T_blk"]) == (20000, 1 << 24, 20000, 4))
    sys_hash, scan = system_programs()
    check("devnet sysCodeHash = keccak256(RLP([code(C0C001..C0C005), 0x80])) of tests/data/system-v1.json", d["sysCodeHash"] == sys_hash, sys_hash.hex())
    check("no system program has a LOG opcode", all(s["logOpcodes"] == 0 for s in scan))
    h_dev = keccak256(dev).hex()
    dev_vars = [variant(d, schema, "TX_CAP", 1), variant(d, schema, "TXBYTES_CAP", 1)]
    for v in dev_vars: check("devnet " + v["field"] + "+1 changes genesisHash", v["genesisHash"] != h_dev)

    out = {
        "status": "PROPOSED - not approved (CAPS-SUSTAINABILITY.md); computed by proposals/model/caps_model.py",
        "keccak": {"roundConstants": [hex(c) for c in ROUND_CONSTANTS], "K": {k: w for k, _, w in ks},
                   "standard": {n: w for n, _, w in std}, "sha3CrossCheckLengths": len(pattern) + 1},
        "schema": {"genesisSpecFields": fields, "specCpFields": names, "rustCpFields": rust, "logCapField": None,
                   "source": "consensus.md:7 and :19-25; prototypes/lp4-execution/src/p1_genesis_spec.rs CP_SCHEMA (names only)"},
        "gsv1": {"bytes": lit.hex(), "length": len(lit), "genesisHash": h_gsv1, "inputSha256": hashlib.sha256(lit).hexdigest(),
                 "specLines": used, "variants": gsv1_vars},
        "devnet": {"config": "DevnetConfig::new(777900, 1700000000, [a, b, c]) defaults (devnet.rs)", "specBytes": len(dev),
                   "genesisHash": h_dev, "sysCodeHash": sys_hash.hex(), "allocRoot": d["allocRoot"].hex(), "variants": dev_vars,
                   "note": "CP-only variants keep allocRoot; a rebuilt genesis also changes allocRoot, since ChainCaps (C0C004) stores all 52 CP values at slots 0..51"},
        "systemPrograms": scan,
        "reachability": {p["name"]: {"source": p["source"], "T_blk": p["T_blk"], "H_END": p["H_END"], **reach(p)} for p in PROFILES},
        "assumptions": ["Cancun gas (execution.md:106): 21000 per transaction, 4 per zero calldata byte, LOG0 375 + 8 per byte + memory 3w + w^2/512",
                        "London base fee (execution.md:97), burned (economics.md:66); priority fees 0 (the attacker may be the proposer; pocold's pool is FIFO)",
                        "idle base fee 7 wei (execution.md:97: below 8 wei in empty blocks); baseFee0 = 10^9 (governance.md:12)",
                        "minimal transaction: signed EIP-155 legacy transfer with 32-byte r and s and a 1-byte gas price"],
        "checks": checks,
    }
    with open(out_path, "w", encoding="utf-8") as f:
        json.dump(out, f, indent=1, sort_keys=True, ensure_ascii=False); f.write("\n")
    print("caps_model: %d checks passed; H_GSV1 %s; devnet genesisHash %s; wrote %s" % (len(checks), h_gsv1, h_dev, os.path.relpath(out_path, ROOT)))

if __name__ == "__main__":
    main()
