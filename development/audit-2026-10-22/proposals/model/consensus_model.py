#!/usr/bin/env python3
"""Independent model of the consensus rules under proposal (audit repair, round 2).

Each rule is re-stated from the normative text and every alternative of its proposal,
without reading the Rust implementation. Scenarios are abstract (log counts and sizes, the
accounts a transaction changes, item sizes) so the expected outcome of each alternative can
be checked by hand. The vectors this writes are the expected results; the Rust tests in
lp4-execution/src/apply/tests.rs (mod proposals) and pocold/tests/consensus.rs replay the
scenarios against the implementation and compare with the *recommended* alternative only.
A passing comparison shows that the code implements the recommendation; it is not an
approval of the proposal.

Usage: python3 -I consensus_model.py OUTDIR
"""
import json, os, sys

LOG_BLOCK_MAX = 32768
LOG_COUNT_MAX = 546          # LG2 (storage.md:58)
LOG_CAP = 1 << 30
ACCT_DIRTY_BLOCK_MAX = 2048
BODY_MAX = 196608            # governance.md:12

# ---------------------------------------------------------------- RLP sizes
def rlp_len(payload_len):
    return 1 + payload_len if payload_len < 56 else 1 + len(payload_len.to_bytes((payload_len.bit_length() + 7) // 8, "big")) + payload_len

def rlp_str_len(n):
    return rlp_len(n)  # n >= 2, or empty; a single byte is never used here

# ---------------------------------------------------------------- LOG-ACCT-01
def log_rlp_len(topics, data):
    # RLP([address, [topic...], data]): a 20-byte address string, 32-byte topic strings.
    return rlp_len(21 + rlp_len(33 * topics) + rlp_str_len(data))

def log_charge(alt, topics, data):
    if alt == "a": return 36 + data                    # 36 + |data|, topics free
    if alt == "d": return 36 + log_rlp_len(topics, data)  # the RetentionStore size (storage.md:53, LG1 = 60 B)
    if alt == "b": return data                         # data bytes only
    if alt == "c": return 36 + 32 * topics + data      # recommended
    raise ValueError(alt)

def log_block(alt, txs):
    """txs: list of logs per transaction, each log (topics, data). Returns per-tx outcome."""
    used, count, out = 0, 0, []
    for logs in txs:
        n = len(logs); charge = sum(log_charge(alt, t, d) for t, d in logs)
        if alt == "c" and count + n > LOG_COUNT_MAX: out.append({"halt": "LogCount"}); continue
        if used + charge > LOG_BLOCK_MAX: out.append({"halt": "LogBytes"}); continue
        used += charge; count += n
        out.append({"halt": None, "logs": n, "charged": charge})
    return {"txs": out, "blockLogBytes": used, "blockLogCount": count}

LOG_SCENARIOS = [
    ("L1 one LOG0, 32731 B", [[(0, 32731)]]),
    ("L2 one LOG0, 32732 B (exactly 32768 under a and c)", [[(0, 32732)]]),
    ("L3 one LOG0, 32733 B", [[(0, 32733)]]),
    ("L4 LG2: 546 LOG0 of 24 B", [[(0, 24)] * 546]),
    ("L5 LG2: 547 LOG0 of 24 B", [[(0, 24)] * 547]),
    ("L6 547 empty LOG0", [[(0, 0)] * 547]),
    ("L7 910 empty LOG0", [[(0, 0)] * 910]),
    ("L8 one LOG2, 32668 B", [[(2, 32668)]]),
    ("L9 one LOG2, 32669 B", [[(2, 32669)]]),
    ("M1 three transactions: 20000, 12732, 12696 B", [[(0, 20000)], [(0, 12732)], [(0, 12696)]]),
]

def log_vectors():
    return {"rule": "LOG-ACCT-01", "constants": {"LOG_BLOCK_MAX": LOG_BLOCK_MAX, "LOG_COUNT_MAX": LOG_COUNT_MAX},
            "alternatives": {"a": "36 + |data| per log, no count limit", "b": "|data| per log, no count limit",
                             "c": "36 + 32*|topics| + |data| per log, and at most 546 logs (recommended)",
                             "d": "36 + |RLP(log)| per log (the stored size), no count limit"},
            "recommended": "c",
            "scenarios": [{"id": sid, "txs": [[{"topics": t, "data": d} for t, d in tx] if len(tx) <= 4 else
                                               {"repeat": len(tx), "log": {"topics": tx[0][0], "data": tx[0][1]}} for tx in txs],
                           "expected": {alt: log_block(alt, txs) for alt in "abcd"}} for sid, txs in LOG_SCENARIOS]}

# ---------------------------------------------------------------- D-A03-DIRTY
def dirty_block(alt, txs, count_touched=False):
    """txs: dicts {sender, changes: accounts whose value changes on success, touched: accounts
    touched without a net change, tip: bool}. Exceptional halt: nonce+fee residue = {sender,
    escrow if tip}. alt 'A' (recommended): a residue that does not fit makes the block invalid.
    alt 'B': the residue is exempt from the limit. count_touched: count touched-but-unchanged
    accounts too (sub-question; recommended False)."""
    dirty, out = set(), []
    for tx in txs:
        diff = set(tx["changes"]) | ({"escrow"} if tx["tip"] else set()) | {tx["sender"]}
        if count_touched: diff |= set(tx.get("touched", []))
        if len(dirty | diff) <= ACCT_DIRTY_BLOCK_MAX:
            dirty |= diff; out.append({"status": 1}); continue
        residue = {tx["sender"]} | ({"escrow"} if tx["tip"] else set())
        if alt == "B" or len(dirty | residue) <= ACCT_DIRTY_BLOCK_MAX:
            dirty |= residue; out.append({"status": 0, "halt": "DirtyAccounts"}); continue
        return {"blockValid": False, "invalidAt": len(out), "txs": out}
    return {"blockValid": True, "dirtyAccounts": len(dirty), "txs": out}

def spray(sender, n, start, contract="sprayerA"):
    # A contract that receives n wei and pays 1 wei to each of n new accounts: the n new
    # accounts change; the contract is touched with no net change. Replay: SPRAY pays
    # 0x1001.., SPRAY_B pays 0x3001.. (fresh accounts for the second transaction).
    return {"sender": sender, "contract": contract, "changes": [f"new{start + i}" for i in range(n)], "touched": [contract], "tip": True}

DIRTY_SCENARIOS = [
    ("D1 alice sprays 2046 (2 + 2046 = 2048 accounts)", [spray("alice", 2046, 0)]),
    ("D2 alice sprays 2047: halt, residue already dirty", [spray("alice", 2047, 0)]),
    ("D3 alice sprays 2046, then bob sprays 1 fresh account: bob's residue does not fit", [spray("alice", 2046, 0), spray("bob", 1, 5000, "sprayerB")]),
    ("D4 alice sprays 2046, then alice sprays 1 fresh account: the residue is already dirty", [spray("alice", 2046, 0), spray("alice", 1, 5000, "sprayerB")]),
]

def dirty_vectors():
    sc = []
    for sid, txs in DIRTY_SCENARIOS:
        sc.append({"id": sid, "txs": [{"sender": t["sender"], "contract": t["contract"], "newAccounts": len(t["changes"]), "tip": t["tip"]} for t in txs],
                   "expected": {"A": dirty_block("A", txs), "B": dirty_block("B", txs),
                                "A-touched": dirty_block("A", txs, count_touched=True)}})
    return {"rule": "D-A03-DIRTY", "constants": {"ACCT_DIRTY_BLOCK_MAX": ACCT_DIRTY_BLOCK_MAX},
            "alternatives": {"A": "net changes counted; a halt whose nonce/fee residue does not fit makes the block invalid (recommended)",
                             "B": "net changes counted; the halt residue is exempt from the limit",
                             "A-touched": "as A, but touched accounts without a net change also count"},
            "recommended": "A", "scenarios": sc}

# ---------------------------------------------------------------- D-A03-CAPS
def caps_block(alt, slot_cap, user_slots, sys_slots, user_writes, sys_writes, sys_slot_max):
    """One USER transaction writing `user_writes` new slots, then the system phases adding
    `sys_writes` new program slots. alt 1: SLOT_CAP counts every slot; alt 2 (recommended):
    SLOT_CAP counts the user state and SYS_SLOT_MAX the programs, checked after the block. Both
    keep SYS_SLOT_MAX. A SYSTEM call cannot halt: passing a cap there invalidates the block."""
    if alt == "1":
        if user_slots + sys_slots > slot_cap: return {"genesisValid": False}
        halted = user_slots + sys_slots + user_writes > slot_cap
        total = user_slots + sys_slots + (0 if halted else user_writes) + sys_writes
        valid = total <= slot_cap and sys_slots + sys_writes <= sys_slot_max
    else:
        halted = user_slots + user_writes > slot_cap
        valid = sys_slots + sys_writes <= sys_slot_max
    return {"genesisValid": True, "userTx": "halt SlotCap" if halted else "ok", "blockValid": valid}

def caps_vectors():
    p = dict(slot_cap=100, user_slots=30, sys_slots=60, sys_slot_max=65536)
    sc = [("C1 user writes 10 (alt 1: exactly SLOT_CAP), system phase adds 3", dict(user_writes=10, sys_writes=3)),
          ("C2 user writes 70 (alt 2: exactly SLOT_CAP), system phase adds 3", dict(user_writes=70, sys_writes=3)),
          ("C3 user writes 71", dict(user_writes=71, sys_writes=3)),
          ("C4 system storage would pass SYS_SLOT_MAX", dict(user_writes=0, sys_writes=65537 - 60))]
    return {"rule": "D-A03-CAPS", "profile": p,
            "alternatives": {"1": "SLOT_CAP/ACCT_CAP count every account and slot", "2": "SLOT_CAP/ACCT_CAP count the user state; SYS_SLOT_MAX the five programs, after the block (recommended)"},
            "recommended": "2",
            "scenarios": [{"id": sid, "input": w, "expected": {a: caps_block(a, p["slot_cap"], p["user_slots"], p["sys_slots"], w["user_writes"], w["sys_writes"], p["sys_slot_max"]) for a in "12"}} for sid, w in sc]}

# ---------------------------------------------------------------- D-BODY-MAX
def body_len(tx_sizes, ev_sizes):
    txs = rlp_len(sum(rlp_str_len(n) for n in tx_sizes)); ev = rlp_len(sum(rlp_str_len(n) for n in ev_sizes))
    return rlp_len(txs + ev)

def body_vectors():
    sc = [("B1 three transactions of 65535 B", [65535] * 3, []),
          ("B2 two transactions of 98000 B", [98000] * 2, []),
          ("B3 one transaction of 131072 B (TX_MAX) and evidence 4 x 2048 B", [131072], [2048] * 4),
          ("B4 items summing to BODY_MAX - 12", [65532, 65532, 65532], [])]
    out = []
    for sid, t, e in sc:
        items = sum(t) + sum(e); enc = body_len(t, e)
        out.append({"id": sid, "txSizes": t, "evidenceSizes": e, "itemSum": items, "encodedLen": enc,
                    "expected": {"a": {"valid": enc <= BODY_MAX}, "b": {"valid": items <= BODY_MAX}, "c": {"valid": items + 1024 <= BODY_MAX}}})
    return {"rule": "D-BODY-MAX", "constants": {"BODY_MAX": BODY_MAX},
            "alternatives": {"a": "the encoded body RLP([txs, evidence]) <= BODY_MAX (recommended; the decode path already does this)",
                             "b": "the sum of transaction and evidence item bytes <= BODY_MAX", "c": "item sum plus a fixed 1024 B margin"},
            "recommended": "a", "scenarios": out}

# ---------------------------------------------------------------- D-A03-SYSUSER
def sysuser(alt, closing, tip, sys_storage_keys):
    """alt a (recommended): only storage keys of the five programs changed by USER count; alt b:
    an Escrow tip credit also counts as a system write (contradicts D70)."""
    writes = sys_storage_keys + (1 if (alt == "b" and tip) else 0)
    if closing and writes > 0: return {"halt": "SysWriteClosing"}
    if writes > 256: return {"halt": "SysKeys"}
    return {"halt": None}

def sysuser_vectors():
    sc = [("S1 ordinary block, transaction with a tip", False, True, 0), ("S2 closing, transaction with a tip", True, True, 0),
          ("S3 closing, a USER write to program storage", True, False, 1), ("S4 257 USER writes to program storage", False, False, 257)]
    return {"rule": "D-A03-SYSUSER", "constants": {"SYS_KEYS_USER_MAX": 256, "USER_SYS_ACCOUNTS": 16},
            "alternatives": {"a": "storage keys of the five programs changed by USER count (recommended)", "b": "an Escrow tip credit counts too"},
            "recommended": "a",
            "scenarios": [{"id": s, "closing": c, "tip": t, "sysStorageKeys": k, "expected": {a: sysuser(a, c, t, k) for a in "ab"}} for s, c, t, k in sc]}

# ---------------------------------------------------------------- LOG_CAP aggregate
def logcap(alt, cum, charge, retained=None):
    if alt == "chain": return {"halt": "LogCap" if cum + charge > LOG_CAP else None}
    if alt == "block": return {"halt": "LogCap" if charge > LOG_CAP else None}
    if alt == "retained": return {"halt": "LogCap" if retained + charge > LOG_CAP else None}

def logcap_vectors():
    sc = [("G1 100 B left, LOG0 of 64 B (charge 100)", LOG_CAP - 100, 100, 0), ("G2 100 B left, LOG0 of 65 B (charge 101)", LOG_CAP - 100, 101, 0),
          ("G3 chain total far below, 32 KiB block", 10 ** 6, 32768, 0)]
    return {"rule": "LOG-CAP-AGGREGATE", "constants": {"LOG_CAP": LOG_CAP},
            "alternatives": {"chain": "charged log bytes of the canonical chain up to the parent plus this block (recommended)", "block": "per block (vacuous: LOG_BLOCK_MAX is smaller)",
                             "retained": "logs in the RetentionStore window only"},
            "recommended": "chain",
            "scenarios": [{"id": s, "cumulative": c, "charge": g, "retainedWindow": r, "expected": {a: logcap(a, c, g, r) for a in ("chain", "block", "retained")}} for s, c, g, r in sc]}

if __name__ == "__main__":
    out = sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(__file__), "..", "vectors")
    os.makedirs(out, exist_ok=True)
    for name, v in [("log-acct-01", log_vectors()), ("d-a03-dirty", dirty_vectors()), ("d-a03-caps", caps_vectors()),
                    ("d-body-max", body_vectors()), ("d-a03-sysuser", sysuser_vectors()), ("log-cap-aggregate", logcap_vectors())]:
        with open(os.path.join(out, name + ".json"), "w") as f: json.dump(v, f, indent=1, sort_keys=True); f.write("\n")
        print(name, len(v["scenarios"]), "scenarios")
