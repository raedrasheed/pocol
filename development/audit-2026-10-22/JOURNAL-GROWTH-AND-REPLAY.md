# Journal growth and re-execution-from-genesis — model, measurements, operational limits

Status: measurement and analysis (not normative). Scope: the on-disk block journal of `pocold`
(`prototypes/pocold/src/store.rs`) and the cost of re-execution from genesis it imposes at open, on a
deep-fork state rebuild, and on restore after an adoption. **Three** obligations are left **OPEN** at the
end (§8: journal disk growth, re-execution from genesis, open-time memory). No mitigation is implemented
here (none would be purely-local storage covered by the D-REORG-REDB approval without touching validity,
fork choice or RPC-visible data — see §7); every option in §7 is **PROPOSED**.

**Round 3 note.** This revision answers the second review (J2-B1 … J2-m7, resolved in §10):
1. **Worst case per gas (J2-B1).** The round-2 "open-time worst case", a JUMPDEST loop at 9.6 ms per
   record, is the cheapest full-gas compute measured: it is a **lower bound**. Of 45 one-transaction
   full-gas block kinds validated on one parent (§3.2a), MODEXP with a small (≤ 8-byte) even modulus and an all-ones
   exponent of 0.25–4 KiB re-executes in **≈ 1.0 s per block (≈ 120–130 ns/gas, ≈ 120× the jump
   loop)** with a body under 4.4 KB; bn256 pairing and KZG point evaluation take ≈ 0.21–0.26 s. The set is
   what was measured, not a proof of the maximum.
2. **A state-size term (new).** Every block's re-execution also grows with the live state:
   `apply_block` clones the parent state, scans it several times and recomputes the whole world-state root
   (§3.2b). Near the devnet SLOT_CAP and ACCT_CAP an *empty* block re-executes in ≈ 235 ms instead of
   1.8 ms; at the network caps the root alone takes ≈ 14.4 s per block (synthetic state).
3. **Open-time bounds restated (§3.3).** Devnet H_END: from ≈ 2.1–2.3 min (empty blocks) to ≈ 6.5 h
   (worst measured, within the devnet caps). Network H_END (a projection; pocold refuses this profile):
   ≈ 64–69 min for empty blocks, ≈ 35–38 h with the worst compute, and weeks with the state term.
   The devnet verdict "open ≈ 2 min, fits" is **withdrawn**; J-V6 is **reopened**.
4. **Ranges (J2-M1).** The deep-reorg and re-fetch latencies are given from the empty-block to the worst
   measured per-block cost (§4.1, §5.3); a genesis rebuild over loaded blocks is measured (§5.1, §5.2).
5. **Mechanism of the second rebuild (J2-M2).** It comes from `retain` pruning the canonical fork point
   (below head−K_hist) while a resident side branch builds on it, already in the first side import — not
   from the order of `retain` and `restore_canonical` (§5.3). Keeping the fork-point state is listed as an
   option (§7).
6. **Open memory (J2-M3).** Peak, steady and transient are reported separately per block kind: the
   transient is ≈ 1.0× the journal; the r2 "2.3×" is the *peak* of a body-dominated journal (§3.4).
7. **Minor.** Devnet deep reorg computed at the tip (96 s for empty blocks; J2-m2); canonical worst case
   ≈ 11.3 GiB (J2-m3); "three obligations" (J2-m4); the 69 min is a range and b grows with H (J2-m5); the
   docstrings, the measured profile and the open intercept are corrected (J2-m6, J2-m7). J2-m1 (setup) is
   acknowledged: this round is based on `fa97594`, which contains the journal r2 merge `0d6f9dd`.

**Round 3b note.** This second fix pass answers the third review (J-REV3-1 … J-REV3-3, resolved in §11),
measurement and documentation only; no `src` file changed.
1. **State-interacting blocks (J-REV3-1, major).** The round-3 per-gas search omitted the state-touching
   opcode families. Six are now measured (§3.2a) and, crucially, the **CREATE path** is shown to carry an
   O(creates × live-accounts) term that the §3.2b additive model misses: `code_caps::current_total` scans
   all accounts on every successful create. §3.2c measures it (≈ 0.54 s per block at the devnet ACCT_CAP,
   ≈ 27.6 s projected at the network ACCT_CAP), so on a full state a **create-heavy** block, not MODEXP,
   is the per-block worst. §7 option 7 and §8 are updated. *[Corrected in round 3c: the create-heavy
   block is not the per-block worst and the term is not O(creates × live-accounts) alone; see the
   round-3c note and §3.2d.]*
2. **Network state term labelled (J-REV3-2, minor).** The ≈ 15–18 s/block and multi-week figures are now
   stated inline as an **extrapolation of an extrapolation**, with the chain of steps (§3.2b, §3.3, §5.4).
3. **Devnet state term at the fill vs the caps (J-REV3-3, minor).** §3.2b now states that the +0.23 s is
   measured at ≈ 95% of the caps; the at-cap value is ≈ +0.25 s (a slight under-estimate).
All three obligations stay OPEN; every mitigation stays PROPOSED. Round-3b evidence: the committed test at
`e3dfb47` (base `256428f`), logs in `evidence/journal-r3b/` (`INDEX.md`), derived figures in
`evidence/journal-r3b/derived.log` (`tools/journal_projections.py` [10]/[11]).

**Round 3c note.** This third fix pass answers the fourth review (JV3B-1 … JV3B-4, resolved in §12),
measurement and documentation only; no `src` file changed.
1. **The per-block worst is a many-transaction block, not a create-heavy one (JV3B-1, blocker).** The
   round-3b conclusion "on a full state a create-heavy block, not MODEXP, is the per-block worst" is
   **withdrawn**, as is the §3.2b wording "a fixed per-block cost, independent of what the block does".
   Every USER transaction does O(state) work: `native::execute_for_profile` runs
   `code_caps::validate_snapshot` over the whole snapshot and `native::build_engine` clones it twice
   (`with_db(snapshot.clone())`, `NativeContext::from_snapshot`). New `#[ignore]` test
   `jm_tx_state_term` (§3.2d): on a state with all three dimensions near the devnet caps
   (18,828 accounts, 46,636 slots, 2.05 MB of logical code) a block of 380 21k-gas transfers validates in
   **10.2 s** (≈ 26 ms per tx; > devnet T_blk = 4 s), against 2.4 s for the CREATE2-heavy child and
   0.41 s for an empty block.
2. **Every state scan is also O(logical code bytes) (JV3B-2, major).** `code_len` keccaks each account's
   runtime (`hash_slow`), so `validate_snapshot` and `current_total` cost O(accounts + code bytes), and
   code bytes are bounded by CODE_CAP (2 MiB devnet, 1 GiB network), not ACCT_CAP. With only 67 accounts
   but 2.05 MB of code the transfer block takes 5.3 s and the CREATE2 child 1.85 s (≈ 7 ms per create).
   The re-execution model is restated as per-block term + txs × (validate_snapshot + 2 clones) +
   creates × current_total, each scan O(accounts + slots + code bytes) (§3.2d); §7 option 7 lists the
   new scans. Four env-gated scratch mutants attribute each term (§3.2d).
3. **Create-term fit repeated (JV3B-3, minor).** `jm_create_state_term` ran twice more; the fit on the
   medians gives ≈ 0.49 s per 247-create block at the devnet ACCT_CAP (range 0.47–0.54 s over the three
   runs) and 23.0–27.6 s at the network ACCT_CAP (projection); the within-transaction O(creates²) part is
   stated (§3.2c).
4. **Index reference (JV3B-4, minor).** `evidence/journal-r3c/INDEX.md` records the correction of
   `evidence/journal-r3b/INDEX.md` (which cites §10 for the round-3b resolutions: they are in §11, with
   §3.2a–§3.2c); the round-3b file itself is not edited (evidence discipline).
All three obligations stay OPEN and are strengthened; every mitigation stays PROPOSED. Round-3c evidence:
the committed test at `1d094de` (base `11f8e89`), logs in `evidence/journal-r3c/` (`INDEX.md`), derived
figures in `evidence/journal-r3c/derived.log` (`tools/journal_projections.py` [12]–[14]).

**Round 2 note (history).** Round 2 carried the stage-A model forward and corrected it against the first
review (J-V1…J-V13, §9): a legal deep reorg pays the genesis rebuild twice (§5.3); open time under
transaction load is bounded by TX_CAP, not TXBYTES_CAP (§3.2–§3.3); the super-linear open is
retain + rebuild_canonical (§3.1); record sizes vary with the PoW share count (§1.1); the re-fetch vector
also costs a genesis rebuild per cycle below K_hist (§4); the network profile is not admissible on pocold
(§3.3, §5.4). Round 3 **withdraws** two of its conclusions — "open time under load is tens of minutes,
not hours" and "the open transient is ≈ 2.3× the journal" — and corrects its account of why the rebuild
runs twice.

Branch `wf/journal-r3`, base `fa97594` (integrated head of `claude/audit-repair-a01-a14`; it contains the
journal r2 merge `0d6f9dd`). This track changes no `src` file. Round-3 measurements: the committed test
`prototypes/pocold/tests/journal_measure.rs` at `7f587bb` (two test-only commits on fa97594, `346246e` and
`7f587bb`; every `src` file is fa97594's), logs in
`evidence/journal-r3/` (commands and runs: `evidence/journal-r3/INDEX.md`), derived figures in
`evidence/journal-r3/derived.log` printed by `tools/journal_projections.py` (stdlib). Round-3b measurements
(the state-interacting families and the create-path term, §3.2a/§3.2c): the same test extended at `e3dfb47`
(branch `wf/journal-r3b`, base `256428f`; still no `src` change from fa97594), logs in
`evidence/journal-r3b/` (`INDEX.md`), figures in `evidence/journal-r3b/derived.log`. Round-3c measurements
(the per-tx and code-byte terms, §3.2d, and the create repeats, §3.2c): `jm_tx_state_term` added at
`1d094de` (branch `wf/journal-r3c`, base `11f8e89`; still no `src` change from fa97594), logs in
`evidence/journal-r3c/` (`INDEX.md`), figures in `evidence/journal-r3c/derived.log`. Earlier rounds:
stage A `86b339e` (`evidence/journal-measure.log`), round 2 `47bfc11` (`evidence/journal-measure-r2.log`,
`evidence/witness-refetch-below-khist.log`). All runs: release, this host (Linux 6.18.44-fc-v153, 4 CPUs),
under `cpu.lock`, the test binary run directly. The test only measures existing code (`store`,
`Node::open`, `Chain::validate`, `Chain::ensure_state`, `Chain::restore_canonical`,
`apply::state_root`). The adversarial re-fetch witness (`prototypes/pocold/tests/zz_audit_witness_refetch.rs`)
is gitignored and **not committed**; it was not re-run this round.

The two profiles referenced throughout (governance.md:5–16; `prototypes/lp4-execution/src/devnet.rs`):

| | GAS_LIMIT | BODY_MAX | TX_MAX | TXBYTES_CAP (cum.) | TX_CAP (cum.) | SLOT_CAP / ACCT_CAP | H_END | T_blk | D_att | R_keep | K_hist | side_max | S_max |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| devnet (`DevnetConfig::new`) | 8,000,000 | 196,608 | 131,072 | 2^24 = 16 MiB | 20,000 | 50,000 / 20,000 | 20,000 | 4 s | 25 s | 2048 | 32 | 8 | 256 |
| network (governance.md:12–16) | 8,000,000 | 196,608 | 131,072 | 2^33 = 8 GiB | 10^7 | 4·10^6 / 10^6 | 129,600 | 60 s | 300 s | 2048 | 32 | 8 | 256 |

**Measured profile (J2-m7).** The harness (`journal_measure.rs` `cfg()`) is `DevnetConfig::new` with
`hash_rate` 1024, `d_att` 73 and `c` 8 (`tau` 40 as the default): the devnet caps, GAS_LIMIT, BODY_MAX and H_END, with an
easier PoW and a 73 s attempt window so blocks seal quickly. The timing comparisons below use the devnet
profile's own D_att = 25 s and T_blk = 4 s; the re-execution costs do not depend on these four values.

The devnet lowers TXBYTES_CAP/TX_CAP/SLOT_CAP/ACCT_CAP/H_END so an A13 resident bound fits the node budget
(`devnet.rs` comment). R_keep, K_hist, side_max (= `single_host().side_max` = 8), BODY_MAX, GAS_LIMIT,
S_max are the network values. **pocold cannot admit the network profile** (§3.3): its A13 history term
includes the whole canonical transaction byte content (TXBYTES_CAP, `resources.rs` `history/transaction
bytes`), which at network is 8 GiB — already over the 6 GiB budget, so `resources::check` refuses to boot
it. Every network figure below is therefore a projection about the spec's targets, not this node.

---

## 1. What the journal stores, and what it does not (verified)

Format v2 (`store.rs` header): `journal = MAGIC(8) ‖ binding(32) ‖ record*`, with
`record = len(4) ‖ lenCheck(4) ‖ sha256(payload)(32) ‖ payload ‖ marker(1)`, i.e. `OVERHEAD = 41`
bytes of frame per record (`store::OVERHEAD`). The hard per-record limit is `MAX_RECORD = 4 MiB`.

The **payload** is `RLP([headerBytes, bodyBytes, parentResultBytes])` (`store::encode_record`). So a
record holds, for one block:
- its encoded **header** (`header.encoded`): fixed fields plus the PoW **share list** (≤ S_max = 256
  shares; `resources::header_max = 768 + 9·S_max = 3072` B worst case);
- its encoded **body** = `RLP([txs, evidence])` (≤ BODY_MAX = 196,608 B; `types::Body::encode`);
- its **parentResult** = winner address + one `(nonce, owner)` pair per share. Each pair encodes to
  **31 B** (`RLP[u64 8 B, addr 20 B]`, measured below), so parentResult ≈ 26 + 31·shares B, up to ≈ 8.0
  KiB at 256 shares (`node::result_bytes`). This is pocold-specific journal content, re-derived and
  checked on replay (`parentResultMismatch` halts open).

The record does **not** hold receipts, logs, executed state, the trie, or any index. Those are
**re-derived by re-executing the block on replay** (`store.rs` module doc; `chain::validate`/`reexec`).
This is the single most important structural fact for both obligations:

> **Journal disk size is driven by headers + transaction/evidence bytes + the share-derived
> parentResult. It is independent of receipts, logs, code, state and trie — which instead drive
> replay CPU and resident memory.**

### 1.1 Measured bytes per record (devnet, release; `JM-BYTES`, round 2 on the committed test)

One block of each kind, built on a real parent (`jm_journal_bytes_per_record`). **Record size depends on
the PoW share count, which varies between runs** (the nonce search finds a run-dependent number of
sub-target shares ≤ S_max): both the header and parentResult grow with it, so these are single samples,
not fixed figures (J-V7). The stage-A log's single samples are not reproduced on the committed test; use
the per-share slopes below instead.

| block kind | txs | PoW shares | header B | body B | parentResult B | **record B** | receipts B (NOT stored) | charged log B (NOT stored) | gasUsed |
|---|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| empty (fallback, a=1) | 0 | 105 | 1,559 | 3 | 3,282 | **4,895** | 0 | 0 | 0 |
| full-gas transfers | 380 | 59 | 1,234 | 44,838 | 1,856 | **47,981** | 102,217 | 0 | 7,980,000 |
| log-heavy (546×LOG0) | 1 | 75 | 1,378 | 115 | 2,352 | **3,897** | 13,375 | 19,656 | 274,379 |
| full-BODY_MAX (2 big calls) | 2 | 62 | 1,261 | 196,587 | 1,949 | **199,852** | 538 | 0 | 827,344 |
| full-gas compute (1 OOG jump loop) | 1 | 4 | 737 | 103 | 149 | **1,040** | 269 | 0 | 7,950,000 |
| full-gas log-heavy (546 logs + filler) | 2 | 18 | 864 | 213 | 585 | **1,714** | 13,644 | 19,656 | 7,924,379 |

Per-share slopes (least-squares over the six samples, `evidence/journal-measure-r2.log`): **parentResult
≈ 31.0 B/share + 26 B** (→ ≈ 7,966 B at 256 shares, **0.96 GiB** cumulative at network H_END); **header
≈ 8.4 B/share + 722 B** (→ ≈ 2,863 B at 256 shares, under `header_max` = 3072).

Readings:
- The **empty** record is *not* small: at 105 shares its header (1,559 B) and parentResult (~3.3 KB)
  dominate; the body is 3 bytes. A 0-share block is ≈ 0.75 KB; a 256-share block is ≈ 10.8 KB. **The
  empty record ranges ≈ 0.75–10.8 KB**, driven by the share list, not the (absent) body.
- The **full-gas** rows confirm the structural fact from both sides. A **full-gas compute** block (one tx
  whose init code is a tight loop that runs out of 8M gas) journals only **1,040 B**. Round 2 called it
  the open-time worst case; it is in fact the **cheapest** full-gas compute per gas measured (§3.2a), so
  its replay cost is a lower bound for compute-heavy blocks. The full-gas MODEXP/pairing/KZG blocks of
  §3.2a are also one transaction with a 0.1–4.3 KB body. A **full-gas log-heavy** block (546 logs then a
  compute filler to ≈ 7.9M gas) journals **1,714 B** while producing 13,644 B of receipts and 19,656 B of
  charged logs — **none journaled**. (A *single* tx cannot be both full-gas and 546-log: exceeding the log
  caps halts the tx with all gas and **no** logs, and running out of gas reverts its logs; so the
  log-heavy full-gas block needs two txs.)
- The **full-gas transfer** block journals ~48 KB but produces 102 KB of receipts that are **not**
  journaled. The **full-BODY_MAX** record (~200 KB) is the largest a normal block reaches; `MAX_RECORD =
  4 MiB` bounds a malformed frame, not a valid block.

A linear fallback chain of empty a=0 blocks averaged ~1,144 B/record at 20k (`JM-BUILD`, stage A — fewer
shares than the a=1 samples above).

---

## 2. Disk growth model at H_END

### 2.1 Canonical content is bounded by the cumulative validity caps

The canonical chain's transaction count and bytes are bounded forever by the **chain-cumulative**
validity caps (`chain::validate` item 16: `p.cum_txs + … > TX_CAP || p.cum_tx_bytes + txBytes >
TXBYTES_CAP` ⇒ `blockReject`). Evidence is bounded by validity too: evidence against an identity without
a live record invalidates the block (`chain.rs` validate item 15, "accused has no live record"), and
valid evidence tombstones its identity, so the chain holds at most **one item per identity ever**,
M_max + R_max·H_END identities (`resources.rs:98–101`, the A13 `history/evidence` term). So the canonical
journal content at H_END is at most:

| component | bound | network value |
|---|---|---|
| headers | (H_END+1)·header_max = 129,601·3072 | **0.37 GiB** (matches storage.md:76) |
| transaction bytes | TXBYTES_CAP | **8.00 GiB** (matches storage.md:76 "transactions 8.00") |
| evidence bytes | ≤ (M_max + R_max·H_END)·EVID_BYTES_MAX = (256 + 2·129,600)·8192 (one item per identity ever; **binding**) | **1.98 GiB** |
| evidence bytes (storage.md's 1 item/block) | (H_END+1)·EVID_BYTES_MAX | 0.99 GiB (an inventory assumption, not a ceiling) |
| evidence bytes (per-block E_max = 4) | (H_END+1)·E_max·EVID_BYTES_MAX | 3.96 GiB (does not bind: above the identity bound) |
| parentResult | ≤ (H_END+1)·~7.97 KiB (31 B/share, 256 shares) | **≈ 0.96 GiB** (pocold-only; not in storage.md) |
| framing (41 B) | 41·(H_END+1) | ≈ 5.3 MB |

**Canonical journal worst case ≈ 11.3 GiB** (headers 0.371 + tx 8.000 + evidence 1.979 (the A13 term, 1.985, adds 24 B of bookkeeping per item) + parentResult
0.961 + framing 0.005; J2-m3). Round 2's "10.3–13.3 GiB" mixed the storage.md inventory figure (1 item per
block, 10.3 GiB) with a per-block ceiling that the identity bound makes unreachable (13.3 GiB). The
non-evidence terms (≈ 9.3 GiB) are firm. The rest of the storage.md ~19.65 GiB-logical / 29.5 GiB-disk
inventory (code, logs, receipts, state, live/historical trie, **reverse diffs**, side) is **not in the
journal** — it is re-derived on replay or (reverse diffs) not kept at all (§6).

### 2.2 Disk at H_END under each load

Per-record figures from §1.1; the journal file is the sum of all **appended** records
(append-only, no compaction, §3):

| load | per-block record | network H_END = 129,600 | devnet H_END = 20,000 |
|---|---|---|---|
| empty (typical ~105 shares) | ~4.9 KB | **≈ 0.6 GB** | ≈ 0.1 GB |
| empty (worst, 256 shares) | ~10.8 KB | ≈ 1.4 GB | ≈ 0.2 GB |
| empty (few shares, a=0 fallback) | ~1.1 KB | ≈ 0.15 GB | **22.9 MB** (measured, 20k records) |
| maximum (cumulative caps) | — | **≈ 11.3 GiB** (§2.1; bounded by TXBYTES_CAP and the identity bound, not H_END·BODY_MAX) | **≈ 0.22 GiB** (tx 16 MiB + headers 0.057 + parentResult 0.148 GiB; evidence ≤ 64·8 KiB) |
| adversarial (re-fetch) | +1 record per re-journal cycle per targeted block | **grows per cycle while targets stay in the R_keep window** (§4) | same |

The maximum-load figure is NOT H_END·BODY_MAX (≈ 24 GB): the cumulative TXBYTES_CAP = 8 GiB caps the
total transaction-byte content of the **canonical** chain. This cap does **not** bound side-block or
re-fetched content (§4). The measured devnet empty-chain journal at exactly 20,000 a=0 records was
**22.89 MB** (1,144 B/record average; `JM-BUILD`, stage A; round 3 identical: 22,887,077 B, 1,144 B/record, `evidence/journal-r3/replay-empty.log`).

---

## 3. Re-execution from genesis: open-time replay

`Node::open` replays and re-executes **every committed journal record in import order**
(`node.rs` open: `store::decode_record` → `chain.validate` → `chain.insert` → `chain.retain` per
record), rebuilding state, receipts, tx hashes and fork choice exactly. There is no snapshot and no
checkpoint. Open time is the sum, over every record, of the block's re-execution cost — which depends on
the block's kind (§3.2a) **and on the size of the live state** (§3.2b) — plus a per-record
retention/index term that grows with the height (§3.1) and a fixed cost (JM-OPEN0, 61.6 ms, J2-m7).

### 3.1 Measured replay (empty blocks, devnet profile, release; `JM-REPLAY`)

Empty (fallback a=0) blocks built into one journal, snapshotted at checkpoints; each snapshot reopened
and the open timed. Stage A (one run, 3 checkpoints) and round 3 (one run on `7f587bb`, 6 checkpoints;
`evidence/journal-r3/replay-empty.log`):

| height | stage A open | **round 3 open** | round 3 journal disk | round 3 per-record (avg over 0..H) |
|--:|--:|--:|--:|--:|
| 1000 | 2.63 s | **2.54 s** | 1.91 MB | 2.54 ms |
| 2500 | — | **6.85 s** | 4.81 MB | 2.74 ms |
| 5000 | 17.01 s | **15.90 s** | 9.59 MB | 3.18 ms |
| 10000 | — | **42.42 s** | 15.96 MB | 4.24 ms |
| 15000 | — | **79.45 s** | 19.46 MB | 5.30 ms |
| 20000 | 137.07 s | **128.80 s** | 22.89 MB | 6.44 ms |

**Open replay is super-linear.** The cause is in the code, not the EVM: two per-record terms grow with
the height (J-V10). The per-record split, measured on a bare replaying chain (`jm_replay_component_split`,
`JM-SPLIT`; round 3 over 20,000 blocks, `evidence/journal-r3/split.log`):

| height bucket | validate (ms) | insert → rebuild_canonical (ms) | retain (ms) |
|---|--:|--:|--:|
| 1..1000 | 2.45 | 0.04 | 0.08 |
| 4001..5000 | 2.32 | 0.38 | 0.94 |
| 9001..10000 | 2.24 | 0.92 | 2.75 |
| 14001..15000 | 2.47 | 1.61 | 4.92 |
| 19001..20000 | 2.20 | 2.15 | 6.44 |

`Chain::validate` is flat (the EVM re-execution on the small devnet state). `Chain::retain` (scans **all**
resident blocks several times per call) and `Chain::insert` → `rebuild_canonical` (walks head→genesis on
every head change) both grow with the height, so `∑_{h≤H}(a + b·h) = a·H + b·H²/2` with `a` the flat
re-execution and `b` the combined slope. **Attribution (J2-m5):** over 20,000 blocks the measured
insert + retain slope (a least-squares line through the 20 buckets) is 4.66×10⁻⁴ ms per record per height, **≈ 106% of
the fitted b**, and the bare replay's validate + insert + retain sum to 127.6 s against Node::open's 128.8 s at 20,000 records (99%). So at the devnet
H_END retain and rebuild_canonical account for the super-linear term within the measurement's resolution.
Round 2's split (6,000 blocks) explained only ≈ 79% because the per-record slope itself grows with the
height (the retain cost steepens in steps, near 7,000 and 14,000 blocks); a split shorter than the extrapolated range
under-attributes. The split is a separate run on a bare chain without Node::open's own per-record work
(record decode, the events/protect bookkeeping, the parentResult check), so the attribution is to the
measured shares only, not a proof that nothing else grows.

**Fit and its range (J2-m5).** A least-squares fit of `a·H + b·H²/2` gives a = 2.26 ms, b = 4.59×10⁻⁴ ms on the stage-A points and **a = 2.05 ms, b = 4.38×10⁻⁴ ms** on the six round-3 points (each reproduces its own 20,000-block point within 0.3 s; network H = 129,600: 69.2 and 65.7 min). The two-point
fits between adjacent round-3 checkpoints give b = 2.70, 3.53, 4.25, 4.22 and 4.57 (×10⁻⁴ ms) for 1k–2.5k, 2.5k–5k, 5k–10k, 10k–15k and 15k–20k (network projections 43, 54, 64, 64 and 68 min): **b grows with H**, so the quadratic model
is not confirmed and an extrapolation from it is not an upper bound. The network projection is therefore
a range (§3.3), not one figure.

### 3.2 Measured per-record replay by block kind (devnet profile, release; `JM-KIND`)

40 blocks of each kind (building 20,000 full blocks of each kind is infeasible). Per-record replay
includes the fixed open cost (`JM-OPEN0`, the open of an empty journal: 61.6 ms, J2-m7) amortised over
40 records; "steady" subtracts it. Round 2 (`journal-measure-r2.log`) and round 3
(`evidence/journal-r3/replay-kind.log`, on `7f587bb`):

| kind | bytes/record | r2 per-record | **r3 per-record** | r3 steady (− JM-OPEN0) | note |
|---|--:|--:|--:|--:|---|
| empty | 2316 | 4.5 ms | **3.5 ms** | 1.9 ms | |
| full_gas_transfers | 46979 | 73.4 ms | **64.6 ms** | 63.0 ms | |
| log_heavy_546 | 1905 | 5.0 ms | **5.9 ms** | 4.4 ms | |
| full_body_max | 198408 | 7.0 ms | **7.7 ms** | 6.1 ms | |
| full_gas_compute | 2241 | 9.6 ms | **8.9 ms** | 7.3 ms | |
| bn_pair_2 | 2942 | — | **229.4 ms** | 227.8 ms | |
| kzg_point_eval | 2184 | — | **214.6 ms** | 213.1 ms | |
| modexp_b8_e256_m8_even | 2748 | — | **938.7 ms** | 937.2 ms | |
| modexp_b8_e1024_m8_even | 3397 | — | **1.03 s** | 1.03 s | |

Replay time tracks **re-execution work**, not record size. The round-2 statement "the worst per-block
replay is the transaction-saturated transfer block (73 ms)" is **withdrawn**: one-transaction compute
blocks with a body of 0.1–4.3 KB replay 3–16× slower than it.

### 3.2a Re-execution cost per gas: one-transaction full-gas blocks (`JM-EXEC`, J2-B1)

`jm_exec_cost_per_gas` builds, on one parent, an empty block, the 380-transfer block and one block per
compute kind: a single creation transaction with 7,950,000 gas whose init code loops one operation until
it runs out of gas (status 0, gasUsed = 7.95M, no state change). It times `Chain::validate` — the work
that replay, TrieRebuild and restore repeat per block — 5 times (median). `net ns/gas` = (median − empty
median) / gasUsed; the 8M-gas figure is the empty median + net ns/gas × 8,000,000. Each precompile input
is also run natively (`JM-PRECOMPILE`), which checks that it is valid (the ecrecover control with v = 0
returns immediately and is the cheap control). `evidence/journal-r3/exec.log` (on `7f587bb`):

| kind | body B | validate median (min) | net ns/gas | at 8M gas | × jump loop | native µs/call (gas) |
|---|--:|--:|--:|--:|--:|--:|
| modexp_b8_e1024_m8_even | 1271 | 1.02 s (964.7 ms) | 128.48 | 1.03 s | 124.7 | 428.3 (2730) |
| modexp_b8_e4096_m8_even | 4343 | 1.01 s (972.8 ms) | 127.31 | 1.02 s | 123.5 | 1624.0 (10922) |
| modexp_b1_e1024_m1_even | 1257 | 1.00 s (945.8 ms) | 125.77 | 1.01 s | 122.0 | 325.3 (2730) |
| modexp_b4_e1024_m4_even | 1263 | 987.9 ms (901.3 ms) | 124.06 | 994.1 ms | 120.4 | 358.5 (2730) |
| modexp_b8_e256_m8_even | 503 | 979.7 ms (964.1 ms) | 123.02 | 985.8 ms | 119.4 | 89.6 (682) |
| modexp_b2_e1024_m2_even | 1258 | 957.4 ms (922.5 ms) | 120.22 | 963.4 ms | 116.7 | 332.4 (2730) |
| modexp_b8_e1024_m8_odd | 1271 | 653.6 ms (634.8 ms) | 82.00 | 657.7 ms | 79.6 | 284.3 (2730) |
| modexp_b1_e1024_m1_odd | 1257 | 532.1 ms (504.6 ms) | 66.72 | 535.4 ms | 64.8 | 172.3 (2730) |
| modexp_b8_e1024_m8_pow2 | 1271 | 423.6 ms (414.4 ms) | 53.08 | 426.3 ms | 51.6 | 178.4 (2730) |
| modexp_b16_e1024_m16_even | 1286 | 374.9 ms (368.7 ms) | 46.95 | 377.3 ms | 45.7 | 491.0 (10921) |
| modexp_b8_e32_m8_even | 278 | 328.3 ms (312.1 ms) | 41.09 | 330.4 ms | 40.0 | 14.1 (200) |
| bn_pair_10 | 2055 | 264.9 ms (226.2 ms) | 33.11 | 266.5 ms | 32.3 | 11206.0 (385000) |
| modexp_b16_e1024_m16_odd | 1287 | 262.6 ms (255.3 ms) | 32.82 | 264.2 ms | 32.0 | 405.2 (10921) |
| bn_pair_2 | 519 | 238.4 ms (226.4 ms) | 29.78 | 239.9 ms | 29.0 | 3231.8 (113000) |
| bn_pair_1 | 326 | 224.7 ms (206.5 ms) | 28.05 | 226.1 ms | 27.4 | 2249.1 (79000) |
| kzg_point_eval | 326 | 209.5 ms (200.0 ms) | 26.14 | 210.8 ms | 25.5 | 1405.5 (50000) |
| modexp_b24_e1024_m24_even | 1303 | 194.2 ms (184.3 ms) | 24.22 | 195.4 ms | 23.7 | 820.4 (24573) |
| modexp_b8_e32_m8_odd | 278 | 187.1 ms (181.3 ms) | 23.33 | 188.3 ms | 22.8 | 8.3 (200) |
| bn_mul | 226 | 178.0 ms (171.7 ms) | 22.18 | 179.1 ms | 21.7 | 137.6 (6000) |
| modexp_b1_e32_m1_odd | 260 | 177.6 ms (172.1 ms) | 22.13 | 178.7 ms | 21.6 | 9.2 (200) |
| keccak_1KiB | 110 | 140.4 ms (108.1 ms) | 17.45 | 141.3 ms | 17.1 | — |
| modexp_b32_e1024_m32_even | 1319 | 133.2 ms (130.4 ms) | 16.55 | 134.0 ms | 16.2 | 753.0 (43685) |
| bn_add | 259 | 128.9 ms (118.4 ms) | 16.00 | 129.7 ms | 15.7 | 4.1 (150) |
| modexp_b32_e1024_m32_odd | 1319 | 124.7 ms (122.2 ms) | 15.48 | 125.5 ms | 15.2 | 1034.9 (43685) |
| blake2f_100k | 347 | 114.7 ms (114.4 ms) | 14.22 | 115.5 ms | 14.0 | 1486.0 (100000) |
| keccak_32KiB | 110 | 110.0 ms (107.0 ms) | 13.63 | 110.7 ms | 13.4 | — |
| modexp_b32_e32_m32_odd | 326 | 109.4 ms (98.6 ms) | 13.55 | 110.1 ms | 13.3 | 17.1 (1360) |
| ecrecover | 259 | 94.6 ms (91.3 ms) | 11.69 | 95.2 ms | 11.5 | 41.2 (3000) |
| modexp_b64_e1024_m64_odd | 1383 | 80.3 ms (77.5 ms) | 9.89 | 80.7 ms | 9.8 | 1713.3 (174741) |
| modexp_b64_e64_m64_odd | 423 | 76.3 ms (76.3 ms) | 9.39 | 76.8 ms | 9.3 | 104.5 (10901) |
| modexp_b128_e128_m128_odd | 615 | 64.8 ms (63.5 ms) | 7.94 | 65.2 ms | 7.9 | 676.7 (87296) |
| full_gas_transfers | 44840 | 62.4 ms (59.9 ms) | 7.60 | 62.5 ms | 7.6 | — |
| keccak_32B | 110 | 62.2 ms (59.5 ms) | 7.61 | 62.6 ms | 7.6 | — |
| modexp_b256_e256_m256_odd | 999 | 54.3 ms (52.1 ms) | 6.63 | 54.7 ms | 6.6 | 4732.6 (698709) |
| modexp_b512_e512_m512_odd | 1767 | 39.6 ms (38.0 ms) | 4.77 | 39.8 ms | 4.8 | 38554.7 (5591040) |
| blake2f_12 | 347 | 26.4 ms (25.3 ms) | 3.11 | 26.6 ms | 3.2 | 0.2 (12) |
| sha256_32KiB | 129 | 16.4 ms (15.6 ms) | 1.85 | 16.5 ms | 2.0 | 23.6 (12348) |
| identity_0B | 129 | 14.5 ms (13.9 ms) | 1.62 | 14.6 ms | 1.8 | 0.0 (15) |
| ripemd160_32KiB | 129 | 9.1 ms (9.0 ms) | 0.94 | 9.2 ms | 1.1 | 122.5 (123480) |
| jump_loop | 103 | 8.2 ms (7.4 ms) | 0.82 | 8.2 ms | 1.0 | — |
| identity_32KiB | 129 | 6.6 ms (6.6 ms) | 0.62 | 6.6 ms | 0.8 | 0.1 (3087) |
| mcopy_32KiB | 112 | 4.0 ms (3.9 ms) | 0.29 | 4.0 ms | 0.5 | — |
| mem_grow_mstore | 111 | 3.4 ms (3.4 ms) | 0.22 | 3.4 ms | 0.4 | — |
| mem_frame_32KiB | 142 | 3.0 ms (3.0 ms) | 0.17 | 3.1 ms | 0.4 | — |
| ecrecover_invalid_v | 259 | 2.9 ms (2.8 ms) | 0.16 | 2.9 ms | 0.4 | 0.1 (3000) |
| mem_frame_256KiB | 142 | 2.3 ms (2.3 ms) | 0.08 | 2.3 ms | 0.3 | — |
| empty_fallback | 3 | 1.7 ms (1.7 ms) | 0.00 | 1.7 ms | 0.2 | — |

A second run on the same commit (`evidence/journal-r3/exec-repeat.log`) keeps the order: the worst MODEXP
shapes 1.01–1.08 s, bn256 pairing 0.23–0.24 s, KZG 0.22 s, ecrecover 0.10 s; the spread between the two
runs is within ≈ ±5% for the compute kinds (the transfer block's median moved 62 → 73 ms).

Readings:
- The round-2 compute kind (jump loop: 8.2 ms here; 8.9 ms per record in round-3 JM-KIND, 9.6 ms in round 2)
  is among the cheapest per gas measured (0.8 ns/gas). It is a **lower bound** for compute, not the worst case (J2-B1).
- **The worst measured kind is MODEXP** with a small even modulus and an all-ones exponent: EIP-2565
  (Cancun, execution.md:106) prices any base/modulus of ≤ 8 bytes at multiplication complexity 1, and an
  even modulus doubles the work (the library splits it into an odd part and a power of two and runs two
  exponentiations). `modexp_b8_e1024_m8_even` validates in **1.02 s** (median of 5; 128 ns/gas net; 1,271 B of body), and the 4 KiB and 256 B exponents and the 1-, 2- and 4-byte even moduli are within 6% of it (0.96–1.01 s); the same shapes with an odd modulus take 0.53–0.65 s. The worst is ≈ 125× the jump loop, ≈ 16× the transfer block and ≈ 4×
  the pairing block. The 256-byte-exponent variant fits more blocks into the devnet TXBYTES_CAP (503 B of body).
- bn256 pairing (any pair count) takes 0.22–0.26 s per block, KZG point evaluation 0.21 s, bn256 mul 0.18 s,
  bn256 add 0.13 s, keccak 0.06–0.14 s, blake2f (100k rounds) 0.11 s and ecrecover 0.09–0.10 s. Memory expansion (frame memory, MSTORE growth, MCOPY) and identity
  copies are cheap per gas (≤ 0.3 ns/gas): their quadratic or linear gas price dominates their cost.
- On this host the worst block takes longer to validate than the devnet's t_exec = 1 s
  (`admission::NodeLocal::single_host`; R14, governance.md:93: t_exec ≥ validate_p99), which bears on the
  open A13 item 2 (`timing-not-a-bound`); the gate itself was not re-measured here.
- This set is what was measured. It is not a proof of the maximum: an unmeasured operation or input shape
  (another MODEXP shape, a different exponent pattern) may cost more per gas, so the worst figure is a
  lower bound on the worst case.

**State-interacting kinds (round 3b, J-REV3-1).** The third review asked for the opcode families that
touch the state: SSTORE (cold-fresh and warm-reset), CALL to many cold accounts, EXTCODECOPY, LOG-heavy
and CREATE2. Six were added to `compute_kinds` and measured by `jm_exec_cost_per_gas` **on the small
genesis state** (`evidence/journal-r3b/exec.log`, repeat `exec-repeat.log`, on `e3dfb47`):

| kind | body B | validate median | net ns/gas | note |
|---|--:|--:|--:|---|
| create2_loop | 135 | 17.5–18.0 ms | 1.9–2.0 | 247 creates/block; the state dependence is §3.2c |
| sstore_warm | 122 | 11.1 ms | 1.16 | rewrites one slot (no state growth) |
| cold_call | 140 | 6.4–7.4 ms | 0.6–0.7 | ~3,000 value-0 cold CALLs (no creates) |
| extcodecopy_cold | 133 | 4.0–4.7 ms | 0.3–0.4 | ~3,000 cold account reads |
| sstore_fresh | 122 | 3.3–3.4 ms | 0.18 | CREATES ~359 slots; the state dependence is §3.2c |
| log0_loop | 108 | 2.5–3.0 ms | 0.07–0.15 | halts at LOG_COUNT_MAX (546), all gas charged |

On a near-empty state **none of these is the per-gas worst case: MODEXP (≈ 134 ns/gas) stays ≈ 65–180×
above them**. Logs are bounded by `LOG_COUNT_MAX` (546/block, a block meter halt) so LOG is never a per-gas
vector; cold access (CALL/EXTCODECOPY) and warm SSTORE are below the jump loop. **But two of these grow
the state** (`sstore_fresh` ≈ 359 slots, `create2_loop` ≈ 247 accounts per block), and the CREATE path
carries a cost that is NOT a per-gas constant: it is **linear in the live-account count** (§3.2c), because
`code_caps::register`'s create-return handler calls `code_caps::current_total`, an O(`db.accounts`) scan,
on every successful create. So whether a state-interacting block is the per-block worst depends on the
state, not on the gas alone (J-REV3-1).

### 3.2b Re-execution cost vs state size (`JM-STATE`, `JM-STATE-SYN`, new in round 3)

`apply_block` (lp4-execution `apply.rs`) does `parent.clone()` of the whole state, scans it several times
(`genesis.image.verify_candidate` twice: `check_caps` and `sum_balances`; `code_caps::validate_snapshot`
after every SYSTEM call; the USER phase's `live_counts`) and recomputes the whole world-state root from
every account and slot (`state_root` → `genesis_builder::world_root`). So **every** block's
re-execution grows with the live state, which only SLOT_CAP and ACCT_CAP bound. `jm_state_size_term` grows
the user state with full-gas blocks of fresh SSTOREs (≈ 359 slots per block) and/or of 1-wei CALLs to
fresh addresses (≈ 230 accounts per block), each followed by an empty block, and at each checkpoint times
the validate of an empty, a bn_pair_2 and a modexp_b8_e1024_m8_even child of the head
(`evidence/journal-r3/state.log`):

| series | height | live user slots | live user accounts | empty child | bn_pair_2 child | modexp child | state_root | clone |
|---|--:|--:|--:|--:|--:|--:|--:|--:|
| slots | 1 | 0 | 5 | 1.8 ms | 249.4 ms | 1.11 s | 0.2 ms | 0.00 ms |
| slots | 47 | 8188 | 28 | 23.7 ms | 255.2 ms | 1.06 s | 19.4 ms | 0.08 ms |
| slots | 91 | 16020 | 50 | 43.4 ms | 270.9 ms | 1.04 s | 41.1 ms | 0.18 ms |
| slots | 137 | 24208 | 73 | 66.8 ms | 329.5 ms | 1.12 s | 60.8 ms | 0.34 ms |
| slots | 181 | 32040 | 95 | 81.1 ms | 318.0 ms | 1.09 s | 77.7 ms | 0.47 ms |
| slots | 227 | 40228 | 118 | 112.2 ms | 339.6 ms | 1.18 s | 100.7 ms | 0.64 ms |
| slots | 269 | 47704 | 139 | 134.3 ms | 363.9 ms | 1.21 s | 117.5 ms | 0.76 ms |
| accounts | 1 | 0 | 5 | 2.0 ms | 244.0 ms | 1.16 s | 0.2 ms | 0.00 ms |
| accounts | 29 | 0 | 3225 | 19.2 ms | 259.3 ms | 1.13 s | 11.8 ms | 0.21 ms |
| accounts | 57 | 0 | 6445 | 37.5 ms | 294.4 ms | 1.06 s | 21.9 ms | 0.45 ms |
| accounts | 85 | 0 | 9665 | 60.2 ms | 281.0 ms | 1.07 s | 30.8 ms | 0.84 ms |
| accounts | 113 | 0 | 12885 | 76.4 ms | 304.6 ms | 1.08 s | 42.9 ms | 0.97 ms |
| accounts | 139 | 0 | 15875 | 97.3 ms | 368.1 ms | 1.15 s | 54.9 ms | 2.47 ms |
| accounts | 163 | 0 | 18635 | 117.6 ms | 347.1 ms | 1.19 s | 61.5 ms | 2.30 ms |
| both | 1 | 0 | 5 | 1.9 ms | 233.3 ms | 1.00 s | 0.2 ms | 0.00 ms |
| both | 65 | 11392 | 37 | 29.7 ms | 260.2 ms | 1.02 s | 25.5 ms | 0.13 ms |
| both | 127 | 22428 | 68 | 59.9 ms | 287.1 ms | 1.08 s | 53.3 ms | 0.32 ms |
| both | 189 | 33464 | 99 | 81.7 ms | 353.3 ms | 1.10 s | 78.6 ms | 0.53 ms |
| both | 253 | 44856 | 131 | 112.6 ms | 337.4 ms | 1.12 s | 110.2 ms | 0.78 ms |
| both | 339 | 47704 | 8189 | 189.4 ms | 415.8 ms | 1.21 s | 154.7 ms | 1.90 ms |
| both | 431 | 47704 | 18769 | 235.3 ms | 518.5 ms | 1.30 s | 190.3 ms | 3.18 ms |

- The term is linear in the state and additive to the compute: the modexp child costs its compute plus
  the same state term as the empty child (at the end of the "both" series the empty child costs 235 ms instead of 1.9 ms, the bn_pair_2 child 519 ms instead of 233 ms and the modexp child 1.30 s instead of 1.00 s: each gains ≈ 0.23–0.30 s). The world-root recomputation is most of it; the
  clone is small.
- Filling the devnet caps takes ≈ 225 full blocks (≈ 140 for SLOT_CAP, ≈ 87 for ACCT_CAP); every
  later block, empty or not, then pays ≈ 0.23 s more on every re-execution (live import, replay,
  TrieRebuild, restore). **The ≈ 0.23 s is measured at the end of the "both" series, 47,704 slots and
  18,769 accounts — ≈ 95% of SLOT_CAP and ≈ 94% of ACCT_CAP, not the caps themselves (J-REV3-3).**
  Extending by the measured slopes (≈ 2.5 µs/slot, ≈ 4.3 µs/account) gives an at-cap empty-child of
  ≈ 246 ms, i.e. a state term of ≈ +0.25 s per block; the measured figure under-states the at-cap value
  by ≈ 11 ms. (`evidence/journal-r3b/derived.log` [11].)
- **Network caps (synthetic, `jm_state_root_synthetic`).** pocold cannot hold these caps, so the dominant
  part is measured without a chain: `state_root` and `clone` of a state of 4·10^6 slots in 11,143
  contracts and 10^6 accounts (`evidence/journal-r3/state-syn.log`): `state_root` **14.4 s** and `clone` 0.87 s (0.83 s / 3.38 s for the root at 1/16 and 1/4 of both caps: linear). Scaled by the devnet
  ratio of the validate increment to the root increment (1.23 at the end of the devnet "both" series), a block at the network caps would
  re-execute in **≈ 15–18 s**, empty or not — against T_blk = 60 s. Filling them takes ≈ 11,100 +
  ≈ 4,300 full blocks.
  **This ≈ 15–18 s is an extrapolation of an extrapolation (J-REV3-2), from three compounding steps:**
  (1) `state_root` + `clone` are measured on a **synthetic** state with **no chain** (no `apply_block`,
  no per-block scans) — itself an extrapolation of how a real block's root recomputation behaves;
  (2) the non-root per-block scans are assumed to scale with the root and are added via the **devnet**
  validate/root increment ratio 1.23, applied at a state ≈ 1,000× larger than where it was measured;
  (3) the result is extended linearly to the whole block. Each step is reasonable; none is measured at
  network scale (pocold refuses that profile). Where the resulting multi-week open and ≈ 15–18 s/block
  figures appear below (§3.3, §5.4, §8) they carry this qualifier.
- This is an implementation property (no incremental trie: the root is recomputed from all accounts each
  block), not a spec requirement; storage.md:73–77 inventories a live trie. An incremental state root is
  proposed in §7 (option 7).
- **The additive model above is the per-block term only (corrected in round 3c, JV3B-1/2).** What this
  section measures (clone + root recomputation + the per-block scans) is what an *empty* block pays.
  Round 3b called it "a *fixed* per-block cost, independent of what the block does" — **withdrawn**:
  every USER transaction re-scans and clones the whole snapshot (§3.2d), and every successful create
  re-totals it (§3.2c), so a block's re-execution grows with txs × state and creates × state on top of
  this term. The state model here is also incomplete: it varies only slots and accounts, while the
  `validate_snapshot` run after every SYSTEM call is O(accounts + **logical code bytes**). On a
  code-filled state (2.05 MB of code, 67 accounts) the empty-block term is ≈ 124 ms; with all three
  dimensions near the devnet caps it is ≈ 413 ms, not the ≈ 246 ms extended above (§3.2d).

### 3.2c Re-execution of a create-heavy block: the `current_total` term (`JM-CREATE`, J-REV3-1, round 3b; corrected in round 3c)

`code_caps::register` installs a `create_return` handler that, on every successful CREATE/CREATE2, calls
`code_caps::current_total` — which iterates **all** `db.accounts` (`code_caps.rs:44`:
`db.accounts.iter()…filter_map(|(_,a)|a.info())`, chained with the journal diff) to re-total the logical
code against CODE_CAP. A block that performs C creates on a state of A live accounts therefore re-executes
in **O(C · A)** from this scan alone — a term that §3.2b's root-based additive model does not represent,
and that §7's incremental-root option (option 7) does not list among the per-block scans it would remove.
**Round 3c correction (JV3B-2):** `current_total` calls `code_len`, which keccaks each account's runtime
(`code.hash_slow()`, `code_caps.rs:13–22`), so the scan is O(A + logical code bytes) and the term is
O(C · (A + code bytes)); the code bytes are bounded by CODE_CAP, not ACCT_CAP. The account-only fit below
therefore does not predict a code-filled state (§3.2d: ≈ 7 ms per create at 67 accounts and 2.05 MB).

`jm_create_state_term` grows the live user-account state (the §3.2b account fill) and at each checkpoint
times, on the head: an empty child (the §3.2b root term), a **CREATE2-heavy** child (one tx, 247 creates
of a tiny empty-code contract), a **cold-CALL control** (one tx, ~3,000 value-0 cold CALLs — cold access
but no creates, so no `current_total` scan) and a **many-small-create** block (149 one-create txs — the
same regime spread across transactions, not one looping tx). `evidence/journal-r3b/create.log`
(on `e3dfb47`):

| live user accounts | empty child | CREATE2 child (247 creates) | create-term (= CREATE2 − empty) | µs per create | cold-CALL control | many-small (149 txs) |
|--:|--:|--:|--:|--:|--:|--:|
| 5 | 1.8 ms | 17.4 ms | 15.6 ms | 63 | 7.5 ms | 35.6 ms |
| 2,765 | 17.9 ms | 85.2 ms | 67.3 ms | 273 | 25.4 ms | 167.1 ms |
| 5,525 | 37.3 ms | 153.6 ms | 116.3 ms | 471 | 40.3 ms | 338.1 ms |
| 8,055 | 53.8 ms | 263.4 ms | 209.6 ms | 849 | 107.3 ms | 544.6 ms |
| 10,815 | 68.2 ms | 358.2 ms | 290.0 ms | 1,174 | 78.9 ms | 635.6 ms |
| 13,345 | 88.8 ms | 380.2 ms | 291.4 ms | 1,180 | 96.7 ms | 696.5 ms |
| 15,645 | 102.2 ms | 589.3 ms | 487.0 ms | 1,972 | 125.3 ms | 895.7 ms |

- **The create-specific term rises with the live-account count** — ≈ 63 µs/create at 5 accounts to
  ≈ 1,972 µs/create at 15,645 — while the **cold-CALL control tracks the empty child** (cold access of a
  similar number of accounts, but no `current_total`, so no extra growth). That isolates
  `current_total` as the cause, exactly as the review predicted. Round 3c confirms it with a mutant
  (`current_total` returning 0, §3.2d): the CREATE2 child falls from
  515 to 125 ms at 15,415 accounts and from 1,845 to 136 ms on the code-filled state, while the
  380-transfer block stays within noise (1.80 → 2.03 s, 5.18 → 4.90 s; `evidence/journal-r3c/mutants/`).
- **Within one transaction the term is O(C²) as well (JV3B-3).** `current_total` also iterates the
  journaled state, which gains one account per create inside the same transaction, so even on a tiny
  state the 247 creates of one tx cost ≈ 63–70 µs each; with `current_total` disabled the same child on the genesis state
  takes 4.4 ms instead of 18.0 ms on the same build (`mutants/mut-A.log` vs `mut-none.log`, start point),
  so ≈ 55 µs per create at A = 5 is
  this within-transaction part. The round-3b fit's negative intercept (−42 µs) is a fitting artefact.
- A least-squares fit (`derived.log` [10]) gives per-create ≈ (base) + 0.112 µs/account · A: the slope is
  the scan. At the **devnet** ACCT_CAP (20,000) a 247-create block would add ≈ **0.54 s** of
  `current_total` scan to its re-execution, on top of the ≈ 0.25 s root term — so ≈ 0.8 s for the create
  work alone (its total ≈ 1.1 s, comparable to MODEXP at the caps).
  **Round 3c (JV3B-3): that fit came from one run whose last point is its high outlier.** Two repeats on
  `1d094de` (`evidence/journal-r3c/create-r1.log`, `create-r2.log`) give, at A ≈ 15,645, 1,596 and
  1,691 µs/create against round 3b's 1,972. Fit on the per-checkpoint medians of the three runs
  (`evidence/journal-r3c/derived.log` [14]): ≈ 25 + 0.098 µs/account · A, i.e. ≈ **0.49 s** per 247-create
  block at the devnet ACCT_CAP; the three single-run fits give **0.47–0.54 s**. This is the
  accounts dimension only (code bytes, §3.2d, add to it).
- **The many-small-create form is not the same regime (corrected in round 3c).** Round 3b said it "shows
  the same regime … slower per create because each tx re-enters the create handler and the pending-pool
  scan" — **withdrawn**: block validation has no pending pool, and most of that block's cost is the
  per-transaction snapshot scans and clones of §3.2d (with `current_total` disabled the 149-tx block only falls from 967 to 776 ms
  on 15,415 accounts and from 2,896 to 2,070 ms on the code-filled state, while disabling the per-tx
  `validate_snapshot` instead brings it to 766 ms and 1,170 ms; `evidence/journal-r3c/mutants/`).
- **At the network ACCT_CAP (10^6) the projection is ≈ 27.6 s per block of `current_total` scan**
  (247 creates × ≈ 112 ms/create; **23.0–27.6 s** over the round-3c fits, ≈ 24 s on the medians),
  additive to the ≈ 15–18 s root term (§3.2b) — so a create-heavy block at the network caps would
  re-execute in **tens of seconds**, above the MODEXP ≈ 1.0 s per-gas worst and the empty-block root
  term (and below the many-transaction block of §3.2d). **This network figure is an extrapolation of an extrapolation
  (J-REV3-2):** (1) the per-create cost is fit linear in A, measured only to A = 15,645 (≈ 78% of the
  devnet ACCT_CAP); (2) extended linearly to 10^6, ≈ 64× beyond the measured range, with the per-block
  create count held at 247. pocold cannot admit the network profile, so it is a projection about the
  spec's targets, not this node.
- Classification: an **implementation property**, not a spec requirement — `current_total` is PoCol's
  CODE_CAP enforcement (`code_caps.rs`, LP4-CODE-01), and a complete re-total per create is O(accounts)
  only because there is no incremental code-size index. It strengthens the re-execution obligation (§8)
  and is added to §7 option 7's scan list. The §3.2a worst-per-gas figure (MODEXP) remains a lower bound.
  Round 3b concluded that this makes a *create-heavy* block the per-block worst once the state is large;
  **that conclusion is withdrawn (round 3c, JV3B-1)**: on every grown state measured, a block of 380
  plain transfers costs more than the CREATE2-heavy child (§3.2d).

### 3.2d Re-execution per transaction and per code byte (`JM-TXSTATE`, JV3B-1/2, round 3c)

**Mechanism (verified in the code at fa97594).** Three whole-state operations run far more often than once
per block:
- **per USER transaction**: `user_phase` executes each transaction with `native::execute_for_profile(&state, …)`
  (`user_phase.rs:247`), which first runs `code_caps::validate_snapshot` over the whole snapshot
  (`native.rs:46`: a `code_len` pass over every account, then `inventory`, a second pass) and then
  `build_engine` (`native.rs:92`), which clones the whole `InMemoryDB` twice — `with_db(snapshot.clone())`
  and `NativeContext::from_snapshot` (`pay.rs:24`, the PAY pre-transaction view) — both dropped at the end
  of the transaction;
- **per SYSTEM call** (`apply.rs` `Exec::call`, several per block): the same `build_engine` clones and a
  `validate_snapshot` after the call (`apply.rs:221`), and one more after the USER phase (`apply.rs:293`);
- **per successful create**: `code_caps::current_total` (§3.2c).
`code_len` (`code_caps.rs:13–22`) checks the content address with `code.hash_slow()`, a keccak of the
account's full runtime, on every call. So `validate_snapshot` and `current_total` are
O(accounts + **logical code bytes**), and a clone is O(accounts + slots) (runtime bytes are
reference-counted, not copied). Logical code is bounded by CODE_CAP (2^21 B devnet, 2^30 B network,
validation.md:2062 and governance.md:13), not by ACCT_CAP: 62–63 deploys of a 32,768-B runtime
(= B_code_max) fill the devnet CODE_CAP to ≈ 98–99% on ≈ 70 accounts.

**Model (replaces round 3b's "fixed per-block cost").** With the state σ = (accounts, slots, code bytes):

    T(block) ≈ T_block(σ) + Σ_tx [ validate_snapshot(σ) + 2·(clone + drop)(σ) + exec(tx) ] + Σ_create current_total(σ)

T_block(σ) is the §3.2b term (root, per-block scans, the SYSTEM calls' scans and clones). A block holds
at most ⌊GAS_LIMIT / 21,000⌋ = 380 transactions (BODY_MAX binds later), and the devnet TX_CAP (20,000
cumulative) bounds the per-tx term over a whole chain to ≈ 52 full blocks; the network TX_CAP (10^7)
does not bound it in practice.

**Measurement.** `jm_tx_state_term` (committed `#[ignore]`, `1d094de`) fills four states and, at
checkpoints, times `validate_snapshot`, `inventory` and one clone alone, then the validate of an empty
child, a block of the most 21k-gas transfers (380), a CREATE2-heavy child (247 creates) and a block of
the most one-create txs (149) (`evidence/journal-r3c/txstate.log`; every checkpoint in `derived.log` [12]):

| state (series end) | live accounts | live slots | logical code | validate_snapshot | clone | empty child | 380 transfers (per tx) | CREATE2 child (per create) | 149 one-create txs |
|---|--:|--:|--:|--:|--:|--:|--:|--:|--:|
| genesis (start) | 5 | 0 | 15,509 B | 0.09 ms | 0.00 ms | 2.1 ms | 61 ms (0.15 ms) | 18 ms (63 µs) | 34 ms |
| accounts | 15,415 | 0 | 15,509 B | 1.87 ms | 0.95 ms | 133 ms | 2.25 s (5.6 ms) | 529 ms (1.6 ms) | 1.08 s |
| code | 67 | 0 | 2,047,125 B | 11.5 ms | 0.02 ms | 124 ms | 5.30 s (13.6 ms) | 1.85 s (7.0 ms) | 2.99 s |
| slots | 136 | 46,636 | 15,509 B | 0.10 ms | 0.31 ms | 132 ms | 0.50 s (0.97 ms) | 154 ms (89 µs) | 302 ms |
| **all three** | **18,828** | **46,636** | **2,047,125 B** | 13.9 ms | 2.51 ms | **413 ms** | **10.25 s (25.9 ms)** | 2.38 s (8.0 ms) | 5.75 s |

The "all" state is ≈ 94% of ACCT_CAP, ≈ 93% of SLOT_CAP and ≈ 97.6% of CODE_CAP, reached in 274 fill
blocks (each followed by a fallback block, height 549).

- **The per-block worst on every grown state is the 380-transfer block** — no creates, no compute. It
  costs 2–4× the CREATE2-heavy child and, at the all-caps state, **10.2 s: above devnet T_blk = 4 s and
  t_exec = 1 s** for one valid block (§5.4). Round 3b's "create-heavy block is the per-block worst" is
  withdrawn. On the slots-only state the per-tx term is small (≈ 1 ms: the clones), so the slot
  dimension matters mostly through T_block.
- **The code dimension dominates the per-tx and per-create scans.** Slopes (start → end of each series,
  [12]): `validate_snapshot` ≈ 5.6 ns per code byte and ≈ 0.12 µs per account; per tx ≈ 6.6 ns per code
  byte, ≈ 0.35 µs per account, ≈ 0.017 µs per slot; the empty block ≈ 60 ns per code byte, i.e. ≈ 10.7
  `validate_snapshot`-equivalents per block (the SYSTEM calls' and the USER phase's scans). At the
  all-caps state the dimensions add super-linearly (per tx 25.9 ms against ≈ 13.6 + 5.6 + 1 ms from the
  single-dimension states).
- **Predicted vs measured per tx.** `validate_snapshot` + two standalone clones predicts 3.8 / 11.6 /
  18.9 ms per tx (accounts / code / all) against 5.6 / 13.6 / 25.9 ms measured; the standalone timings
  are a lower bound on the in-block cost (mutant C below), and two standalone `validate_snapshot`
  samples are ≈ 2× outliers (23.2 ms at the code checkpoint A = 65, 21.8 ms at the all/slots-end point)
  while the in-block per-tx at those points is not, so the report uses the in-block figures.
- **Attribution by scratch mutants** (env-gated edits to lp4-execution `src` in a scratch build, run on
  `jm_tx_state_term` with `JM_TX_SERIES=accts,code JM_TX_POINTS=1`, then reverted; logs in
  `evidence/journal-r3c/mutants/`, the mutant sources stay in scratch; [15]):

| mutant (timing only) | accounts state: empty / 380 transfers / CREATE2 / 149 creates | code state: empty / 380 transfers / CREATE2 / 149 creates |
|---|---|---|
| none (the mutant build, control) | 109 ms / 1.80 s / 515 ms / 967 ms | 124 ms / 5.18 s / 1.85 s / 2.90 s |
| A: `current_total` returns 0 | 105 ms / 2.03 s / **125 ms** / 776 ms | 128 ms / 4.90 s / **136 ms** / 2.07 s |
| B: no `validate_snapshot` in `execute_for_profile` | 116 ms / **1.07 s** / 537 ms / 766 ms | 140 ms / **0.19 s** / 1.77 s / **1.17 s** |
| C: one extra whole-snapshot clone + drop per engine | **205 ms / 4.30 s** / 553 ms / 1.18 s | 120 ms / 4.92 s / 1.68 s / 3.03 s |
| D: `code_len` skips `hash_slow` | 110 ms / 1.68 s / 503 ms / 993 ms | **1.6 ms / 35 ms / 7.9 ms / 20 ms** |

  A removes the CREATE2 child's state term and leaves the transfer block unchanged: `current_total` is
  the create term and only that. B removes ≈ 97% of the code-state transfer block (5.18 → 0.19 s) and
  ≈ 40% of the accounts-state one: the per-tx `validate_snapshot` is the per-tx term on a code-filled
  state, and the remainder on an account-filled state is the clones. C (one more clone + drop per
  engine) adds ≈ 6 ms per tx and ≈ 96 ms per empty block at 15,415 accounts — far more than the 0.95 ms
  of a warm standalone clone, so a clone's in-block cost (allocation, first touch, drop) is larger than
  `stateCloneMs` suggests — and nothing on the 67-account code state. D removes the whole code dimension
  (the code-state empty block 124 → 1.6 ms, the transfer block 5.18 s → 35 ms): it is the keccak in
  `code_len`, nothing else. (B and D are timing mutants only: they drop a check; no claim is made that
  either is a valid optimisation.) The review's mutant-B run on the accounts state did not show B's
  reduction (2.05 s against 1.78 s there); this run does (1.80 → 1.07 s); both agree that the code-state
  transfer block is the per-tx `validate_snapshot`.
- **Devnet bounds.** The empty-block state term at the all-caps state is ≈ +411 ms (not +0.23–0.25 s,
  §3.2b), so the worst devnet open (modexp_b8_e256_m8_even on that state) is ≈ 7.3 h, plus ≈ 8.5 min for
  the ≤ 52 TX_CAP-bounded 380-transfer blocks (`derived.log` [13]; §3.3). The per-create term at that
  state is ≈ 8 ms per create. Whether the cumulative number of creates over a chain is bounded is not
  analysed here.
- **Network (projection only; pocold refuses this profile).** Extending the slopes linearly to ACCT_CAP
  10^6 and CODE_CAP 2^30 gives `validate_snapshot` ≈ 6.2 s; with two clones of the synthetic network-caps
  state (0.87 s each, `evidence/journal-r3/state-syn.log`) ≈ **7.9 s per USER tx**, ≈ 50 min for a
  380-transfer block, and ≈ 11 × 6.2 ≈ 66 s of code scans in an empty block on top of the ≥ 14.4 s root
  term (≈ 80 s, above T_blk = 60 s). Filling CODE_CAP 2^30 at one 32,768-B deploy per block takes
  32,768 blocks, within H_END = 129,600. **These are linear extensions ≈ 512× (code) and ≈ 65×
  (accounts) beyond the measured range, combined with a synthetic clone: projections, not
  measurements.**
- Classification: an **implementation property**, not a spec requirement. CODE_CAP and B_code_max must
  fail the deposit (execution.md:81), which `current_total` enforces; nothing in the spec requires a
  full re-validation and two full clones of the state per transaction or a keccak of every runtime per
  scan. It strengthens the re-execution obligation (§8). The mitigations (cached code length/hash, a
  running logical total validated once per block, a copy-on-write overlay instead of the clones) are
  added to §7 option 7, PROPOSED only.

### 3.3 Open-time bounds at H_END (projections; J2-B1, J-V6 reopened)

**pocold does not admit the network profile.** `Node::open` calls `resources::check` first; the A13
history term `history/transaction bytes (TXBYTES_CAP)` is the whole canonical transaction byte content,
= 8 GiB at network — already over the 6 GiB budget — so the check refuses to boot (`resources.rs`).
Canonical bodies are resident (D-REORG-REDB offloads only *non-canonical* blocks), which is why this term
is resident. The network figures below are projections about the spec's targets, for a hypothetical node
that could hold the network caps, **not** this node.

**Method.** Open(H) = Σ per-record replay cost + b·H²/2 + JM-OPEN0. The per-record costs are the measured
steady JM-KIND figures (on the small devnet genesis state) plus, where stated, the measured §3.2b state
term; b is the round-3 least-squares fit (§3.1). Every row respects the cumulative caps: one transaction
per block within TX_CAP, the block's body bytes within TXBYTES_CAP (rows that cannot fill H_END fill the
rest with empty blocks). These are sums of measured per-block costs, not measurements of a 20,000- or
129,600-block chain of that kind. `evidence/journal-r3/derived.log` prints every figure.

| chain of H_END blocks of … (one tx each) | per-record replay, steady (JM-KIND) | blocks within the cumulative caps | devnet H_END = 20,000 | network H_END = 129,600 (projection) |
|---|--:|---|--:|--:|
| empty | 1.9 ms (+ the b·h term) | all | **129 s** (measured; stage A 137 s) | **≈ 64–69 min** (range, §3.1) |
| jump loop (round-2 "worst case"; a lower bound) | 7.3 ms | all | ≈ 3.9 min | ≈ 1.3 h |
| 380 transfers (TX_CAP binds) | 63.0 ms | devnet 52, network 26,315 (rest empty) | ≈ 2.2 min | ≈ 1.5 h |
| bn_pair_2 | 228 ms | all (519 B body: 10.4 MB < 16 MiB) | ≈ 1.3 h | ≈ 9.2 h |
| kzg_point_eval | 213 ms | all | ≈ 1.2 h | ≈ 8.7 h |
| **modexp_b8_e256_m8_even** | **937 ms** | all (503 B body: 10.1 MB < 16 MiB) | **≈ 5.2 h** | **≈ 34.8 h** |
| modexp_b8_e1024_m8_even | 1.03 s | devnet 13,200 (TXBYTES_CAP; rest empty), network all | ≈ 3.8 h | ≈ 38.0 h |
| modexp_b8_e256_m8_even on a state at SLOT_CAP/ACCT_CAP | 937 ms + the §3.2b state term | after ≈ 225 (devnet) / ≈ 15,500 (network) fill blocks | **≈ 6.5 h** (+0.23 s per block) | ≈ 21–25 days (+15–18 s per block, linear projection) |
| modexp_b8_e256_m8_even on a state with accounts, slots **and code** near the devnet caps (round 3c, §3.2d) | 937 ms + 411 ms (measured empty-block term on that state) | after 549 heights of fill (274 fill blocks, each with a fallback) | **≈ 7.3 h** | not computed (the code term at network CODE_CAP is a projection, §3.2d) |
| 380 transfers on that state (round 3c, §3.2d) | 10.2 s per block | TX_CAP: devnet ≈ 52 such blocks | + ≈ 8.5 min in total | not a practical bound (≈ 7.9 s per tx projected × TX_CAP 10^7) |

- **Round 2's conclusion is withdrawn.** "Open time under load is tens of minutes, not hours" and "a
  realistic mixed chain lands around 1–1.5 h" rested on the jump loop (9.6 ms) as the compute worst case.
  With the measured worst kind, the open at devnet H_END is ≈ 5.2 h, and ≈ 6.5 h when the state
  is also at the devnet caps. Even round 2's own numbers gave ≈ 1.9 h for the worst network mix (the
  review's figure), above the stated 1–1.5 h.
- The **empty-block** network figure is a range, ≈ 64–69 min, because b grows with H (§3.1).
- At the network caps the state term alone (≈ 15–18 s per block, §3.2b) makes the open of a
  full-state chain take weeks; the projection is linear from the synthetic root and is not a measurement.
- **Round 3c (JV3B-1/2).** The "380 transfers" row above (63 ms per record) is on the small genesis
  state. On a grown state every USER tx pays `validate_snapshot` + two snapshot clones (§3.2d): at the
  all-caps devnet state a 380-transfer block costs 10.2 s. The devnet TX_CAP (20,000 cumulative) bounds
  this to ≈ 52 full transfer blocks per chain (≈ +8.5 min of open), so the devnet worst open becomes
  ≈ 7.3 h + ≈ 8.5 min (`evidence/journal-r3c/derived.log` [13]); the network TX_CAP (10^7) bounds it only
  in principle. The empty-block state term with code at the cap (≈ 0.41 s) replaces the ≈ 0.23 s of the
  row above.

### 3.4 Open-time memory: peak, steady and transient (fresh process; `JM-OPENMEM-FRESH`, J2-M3)

`store::open` reads the whole journal (`read_to_end`) and `parse_v2` copies each payload into a `Vec`;
`Node::open` holds every payload for the whole replay, then keeps the resident chain. Round 3 separates
three quantities, each measured in a fresh process that only opens (J2-m6: the journal size now comes from
`metadata`, nothing is read before the baseline, and freed heap is returned with `malloc_trim` before
the baseline and before the steady sample):
- **peak** = VmHWM after open − RSS before open;
- **steady** = RSS after open with the node alive, after `malloc_trim` — the post-open resident chain,
  which the A13 history terms already count;
- **transient** = peak − steady — the open-only working set that the A13 steady-state bound excludes
  (`resources.rs` condition `open-replay-transient`).

Two fresh opens per journal (`evidence/journal-r3/openmem-*.log`, on `7f587bb`):

| journal (kind, records) | journal B | open s | peak / journal | steady / journal | **transient / journal** |
|---|--:|--:|--:|--:|--:|
| full_body_max, 85 (open 1) | 16862277 | 0.49 | 2.142 | 1.119 | **1.023** |
| full_body_max, 85 (open 2) | 16862277 | 0.50 | 2.139 | 1.117 | **1.022** |
| empty, 3000 (open 1) | 5780087 | 8.39 | 4.627 | 3.564 | **1.063** |
| empty, 3000 (open 2) | 5780087 | 8.13 | 4.593 | 3.542 | **1.050** |
| full_gas_transfers, 40 (open 1) | 1879159 | 2.69 | 5.672 | 4.614 | **1.057** |
| full_gas_transfers, 40 (open 2) | 1879159 | 2.51 | 5.674 | 4.651 | **1.022** |

- **The transient is ≈ 1.0× the journal** for every kind (the payload copies). Round 2's "open
  transient ≈ 2.3× the journal" was the **peak** of a body-dominated journal, which includes the steady
  resident chain; it is withdrawn (J2-M3, J-V4 resolution corrected in §9).
- **The steady part depends on the block kind**: it is 1.1× (body-max), 3.6× (empty) and 4.6× (transfers) the journal — small relative to a
  body-heavy journal, large relative to an empty-block journal (the resident per-block metadata, header
  heap and shares are larger than the 1–2 KB record), and above 1× for transfers (receipts are resident,
  not journaled). It is part of the A13 steady bound, not of the transient.
- The body-max peak is 2.14× here against round 2's 2.34×: the round-3 baseline is taken after the spec
  admission and without the journal pre-read (J2-m6), so it no longer counts those. The second review's trimmed
  measurement (`w_open_trim`, baseline before the admission) gave peak 2.34×, steady 1.32×, transient
  1.02×: the transient agrees.
- Stage A's 237 MB VmHWM was the build-phase peak (J-V5; round 2).

Consequences: the transient adds ≈ 1× the journal on top of the steady resident set at every open. At the
network canonical maximum (≈ 11.3 GiB, §2.1) that is ≈ 11.3 GiB more than the A13 bound counts (whose
network total was ≈ 151 GiB in the second review's computation at 273e785). On the devnet the canonical maximum journal is ≈ 0.22 GiB, so the
transient is ≈ 0.22 GiB, carried by the 1/8 boot reserve (≈ 767 MiB, `A13-DERIVATION.md` §2b), which
exists for allocator overheads the model does not count — not budgeted. Under the adversarial vector (§4)
the journal, and so the transient, grows without bound, so a node can be pushed into a state where it
**cannot restart**. The transient is an open obligation in its own right (§8).

---

## 4. Adversarial re-fetch re-journal (witness)

**Vector.** A sync session's blocks are **solicited**, so an evicted (honest, valid) in-window side block
is re-validated and **re-journaled** (`node::import_with`: `journal.append` before the side cap evicts it
again; `EVICTED_MAX` blocks only **unsolicited** re-delivery). The round-2 witness forks the evicted
blocks **below K_hist**, which exposes the cost stage A's genesis-forked witness could not see. It was not
re-run in round 3.

### 4.1 Two costs per cycle (`WITNESS-REFETCH-below-khist`, devnet, release, round 2)

Setup: canonical C of 600 a=0 blocks, fork point at height 300 (**below** K_hist = 32), a 40-block
lighter side branch S from the fork point; node with R_keep = 1024 (S stays in the window), side_cap = 16.
Each round re-imports S (solicited), then `protect.clear()` (mirrors `end_session`) + `retain()` (stands
in for the retain a later successful import runs). Every round, over 8 rounds:

| per cycle | measured | cost |
|---|--:|---|
| **disk** — records re-journaled | 24 records, **16,464 B** appended | append-only; linear, unbounded while in window |
| **CPU** — TrieRebuild re-executions | **316** (fork-point height 300 + the 16 resident, pruned side-prefix blocks), ≈ 0.8 s on empty blocks | genesis rebuild on the single consensus thread |
| resident side metadata | pinned at **16** (= side_cap) | the cap bounds metadata, not disk |
| evicted-hash total | grows 24→216 | — |

The **CPU amplification** (J-V2): because the lowest re-imported block's parent is the pruned canonical
fork point (below K_hist), each cycle re-executes from genesis ≈ (fork-point height + 16) blocks. **Per
cycle, as a range (J2-M1)**, with the per-block rebuild cost `a` from the empty block (2.4 ms, §5.1) to
the worst measured loaded block (≈ 1.0 s, §5.1), at a fork point near the tip (f ≈ H_END − R_keep):

| per-block rebuild cost a | devnet fork point f = 17,952 | network fork point f = 127,552 |
|---|--:|--:|
| empty, 2.4 ms | ≈ 43 s | ≈ 5.1 min |
| bn_pair_2, 230 ms | ≈ 69 min | ≈ 8.2 h |
| modexp_b8_e256_m8_even, 960 ms | ≈ 4.8 h | ≈ 34 h |
| modexp_b8_e1024_m8_even, 1.01 s | ≈ 5.1 h | ≈ 36 h |

The witness's chain is empty, so its 0.8 s is the empty end of the range. Round 2's "≈ 5 min of blocked
event loop per cycle at a network-scale fork point" was the empty-block figure; it is restated as this
range. Stage A's witness forked at genesis (headHeight 2), whose state is always resident, so it measured
0 rebuilds and saw only the disk cost. (Open replay does **not** re-pay this per round: replay's side set
is capped at side_cap + protect_max and the re-imports replay as `Duplicate` — the amplification is
live-only.)

### 4.2 What this witness drives, and the real per-session bound

The witness drives the **solicited-import + retention path directly**; it does **not** drive the TCP
sync-session lifecycle (Hello/`start_session`/`SESSION_GAP_MS`/`protect_max`/`best_h`). Two consequences,
from the code, bound the real vector (J-V3; the stage-A claim that growth is "unbounded, only the head
limits it" is downgraded to analysis):

1. **The cycle rate is gated by retain triggers.** `end_session` clears `protect` but does **not** call
   `retain` (`node.rs:578-581`); a `Duplicate` re-import does not either. So a live re-eviction (needed
   before the block can be re-fetched again) requires a **later successful import** to run `retain` — an
   honest block every T_blk, or other new blocks. The 16.5 KB "round" is a synthetic unit; the real
   cadence is paced by the honest chain's own progress.
2. **The per-session journaling is bounded.** A session ends when its protected branch reaches
   `protect_max = 2·R_keep = 4096` blocks (`sync_tick`). So one session re-journals at most
   `min(evicted-in-window, 4096)` records, ≈ **44 MB** of 256-share fallback records or ≈ **815 MB** of
   full-BODY_MAX records. The precondition for a full window of re-fetchable side blocks is more than
   side_cap resident in-window side blocks, which the attacker must first produce (real PoW cost per
   block).

So growth across sessions is **not free**: each cycle is gated by honest progress and costs the attacker a
heavier branch (or a flapping reorg), and each session's re-journal is bounded by `protect_max`. The one
**standing** limiter on cumulative disk is the **R_keep window advancing** past `height + R_keep` as the
honest head grows (`import_with` height guard, the R2.8 fix refuses a new block at or below head−R_keep
before validation/journaling). Which existing bound stops the growth:

| mechanism | bounds | stops journal re-growth? |
|---|---|---|
| side cap (`side_cap`, R_keep) | resident non-canonical **metadata** count | **No** — disk is append-only |
| `protect_max` = 2·R_keep | a live session's branch; caps **one session** at 4096 records | **Per session** — not across sessions |
| `EVICTED_MAX` = 1024 | **unsolicited** re-delivery of an evicted hash | **No** — solicited (session) re-fetch is exempt |
| per-peer / queue limits | in-flight bytes, queue depth | **No** — they pace delivery, not cumulative disk |
| ban score | 1b/templateForm faults | **No** — re-announced blocks are valid |
| **R_keep window vs the moving head** | a block ≤ head−R_keep is refused before validation/journaling | **The standing limiter** — once the head passes height+R_keep the target is no longer re-journaled |

The designed re-fetch (consensus.md §10: "الفرع الذي يصير أثقل لاحقًا يُجلب بالمزامنة") covers a branch
that becomes **heavier** (adoption), not re-fetching a **lighter** evicted branch (J-V13); the realistic
real-path growth is therefore repeated **adoption/flapping** (each adoption re-journals the adopted
offloaded branch and, below K_hist, pays the §5.3 double rebuild), bounded per session by `protect_max`.
The positive control that the real TCP session path performs a solicited re-fetch + restore end to end is
the committed test `a_sync_session_refetches_an_evicted_branch_and_serves_it` (tests/consensus.rs).

---

## 5. Deep-fork rebuild and restore

### 5.1 Deep-fork state rebuild starts from genesis (no reverse diffs)

A fork point below the K_hist window (=32) has a **pruned** state, and `ensure_state_with` rebuilds it by
**re-executing from the nearest retained ancestor, which below the window is genesis** — there are no
reverse diffs (storage.md:70 / consensus.md:230–232 expect reverse diffs up to R_keep, then TrieRebuild;
pocold keeps none, so a genesis TrieRebuild inside R_keep is a **conformance gap**, §6/§9).

Measured rebuild vs fork-point height, empty blocks (`jm_deep_fork_rebuild_vs_height`, `JM-REBUILD`,
round 2):

| fork-point height | rebuild time | per-block |
|--:|--:|--:|
| 100 | 0.245 s | 2.45 ms |
| 500 | 1.249 s | 2.50 ms |
| 1,000 | 2.402 s | 2.40 ms |
| 2,048 (R_keep) | 5.082 s | 2.48 ms |
| 3,000 | 7.407 s | 2.47 ms |

Rebuild cost is **linear in the fork-point height** (blocks re-executed from genesis), **not in the fork
depth** — the clean rolling path (`ensure_state_with`, two snapshots live), with none of the Θ(H) retain
scan of open (§3.1). Reverse diffs would make it scale with depth (≤ R_keep).

**Under load (J2-M1).** The per-block rebuild cost is the block's re-execution cost, so it depends on the
block kind like replay does. `jm_rebuild_by_kind` rebuilds from genesis a chain of 20 blocks of one kind
on one empty parent (`evidence/journal-r3/rebuild-kind.log`, on `7f587bb`):

| kind (20 blocks) | re-executions | time | **per block** |
|---|--:|--:|--:|
| empty | 20 | 0.04 s | **1.8 ms** |
| full_gas_transfers | 20 | 1.30 s | **65.0 ms** |
| full_gas_compute | 20 | 0.14 s | **7.2 ms** |
| bn_pair_2 | 20 | 4.60 s | **229.9 ms** |
| kzg_point_eval | 20 | 4.13 s | **206.5 ms** |
| modexp_b8_e256_m8_even | 20 | 19.20 s | **959.9 ms** |
| modexp_b8_e1024_m8_even | 20 | 20.28 s | **1.01 s** |

So the per-block rebuild cost ranges from **2.4 ms** (empty) to **≈ 1.0 s** (worst measured kind), on the
small devnet genesis state; the §3.2b state term (up to ≈ 0.23 s at the devnet caps) comes on top.
The second review's pairing figure (231 ms per block, `w_rebuild_kind`) is reproduced by the committed
test (230 ms).

### 5.2 Deep reorg measured end to end (`jm_deep_reorg_latency`, `JM-REORG`)

Canonical C, fork point at height f = |C|/2 (below K_hist), competing branch T of empty blocks that
becomes strictly heavier and is adopted, on a node with the default single-host limits. Round 2 (empty C,
600 blocks) and round 3 (on `7f587bb`: the same, and a loaded C of 100 modexp_b8_e256_m8_even blocks;
`evidence/journal-r3/reorg.log`, `reorg-loaded.log`):

| run | C kind | C blocks / f | first side import | adopting import | all T imports | reopen |
|---|---|--:|--:|--:|--:|--:|
| round 2 | empty | 600 / 300 | 300 rebuilds, 0.62 s | 300 rebuilds (+284 restored), 1.47 s | 600 rebuilds, 3.28 s | 600 rebuilds, 886 records, 5.26 s |
| round 3 | empty | 600 / 300 | 300 rebuilds, 0.62 s | 300 rebuilds (+299 restored), 1.17 s | 600 rebuilds, 2.65 s | 600 rebuilds, 902 records, 4.13 s |
| round 3, loaded | modexp_b8_e256_m8_even | 100 / 50 | 50 rebuilds, 48.17 s | 50 rebuilds (+50 restored), 97.73 s | 100 rebuilds, 193.72 s | 100 rebuilds, 152 records, 193.25 s |

The deep reorg re-executes the fork point from genesis **twice**: 2·f rebuilds (J-V1). With a loaded C
both stalls scale with C's per-block cost: with C made of 100 `modexp_b8_e256_m8_even` blocks (fork point at height 50), the first side import takes 48.2 s (50 rebuilds, 0.96 s each, the kind's rebuild cost) and the adopting import 97.7 s, and the reopen re-pays both rebuilds (193 s for 152 records).

### 5.3 Deep-reorg latency vs consensus timing — the genesis rebuild runs TWICE

A legal adoption at depth `d`, fork point height `f` below K_hist, pays the genesis rebuild of `f`
**twice** on the single consensus thread, in two separate blocking stalls:

1. When the **first** competing block is validated: its parent is the canonical fork point, whose state is
   pruned → `NeedsRebuild` → `ensure_state_with` from genesis (`f` re-executions).
2. When the branch is **adopted**: `restore_canonical` re-executes the adopted suffix from the fork
   point's state (`ensure_state_with(fork point)`), which is pruned again, so it is rebuilt from genesis a
   **second** time (`f` re-executions), then the `d`-block suffix is re-executed.

**Why the fork point is pruned again (corrected, J2-M2).** `Chain::retain` prunes the state of **every
canonical block** below head−K_hist (`chain.rs` retain: `canonical.iter().skip(1).take(cut)`); the
side_max state allowance applies only to non-canonical blocks. The fork point is still canonical while
the side branch grows, so the very `retain()` that ends the **first** side import prunes the state that
import just rebuilt — although every later block of the resident side branch builds on it. Round 2
attributed the second rebuild to `import_with` running `retain()` before `restore_canonical` in the
adopting import; that is not the cause: the second review moved `restore_canonical` before `retain` and
still measured 300 + 300 rebuilds. Round 3 shows the mechanism directly on a bare chain
(`JM-REORG-MECH`): the fork point holds a state after the rebuild and after the side insert, and **none
after the next `retain()`**, while the side block keeps its own state (head 600, fork point 300: `stateAfterRebuild: true`, `stateAfterSideInsert: true`, `stateAfterNextRetain: false`, `t0KeepsItsState: true`; the same on the loaded chain (head 100, fork point 50)); and on the node, the
fork point is pruned right after the first side import (`forkPointPrunedAfter: true`). A fix that only
reorders `retain` and `restore_canonical` would not remove the second rebuild; keeping the fork-point
state while a resident side branch depends on it would (§7 option 6).

**A third term under load (new in round 3).** The adopting import also returns the abandoned branch's
transactions to the pool (A11, consensus §10): `on_new_head` → `rebuild_pool` re-executes each of them on
the new head (`node.rs` `detached_txs`, walking at most REORG_RESTORE_MAX = 1024 blocks per side, pool ≤
POOL_MAX_TXS = 4096). On empty blocks this is free; in the loaded reorg of §5.2 it is the other half of the
adopting import (97.7 s = 50 rebuilds (≈ 48.2 s) + ≈ 49.6 s for the restore of 50 empty T blocks and the pool re-execution of the 50 abandoned modexp transactions), by inspection of the code path (A13 item 7, "every new head re-executes the
pool", CPU OPEN).

So **latency ≈ 2·rebuild(f) + 2·d·a = 2·f·a_C + 2·d·a_T** plus the pool re-execution of the abandoned
transactions (≤ 1024 blocks' worth), where a_C is the per-block cost of the canonical blocks up to the fork
point and a_T that of the adopted branch: a first stall of ≈ f·a_C and an adopting stall of ≈ f·a_C +
d·a_T + the detached re-execution. For a reorg at depth d = R_keep = 2048 near the tip H (f = H − d), with
both branches of one kind, latency ≈ 2·H·a (+ ≤ 1024·a_tx). **As a range (J2-M1, J2-m2: the tip height in
both rows):**

| per-block rebuild cost a (§5.1) | devnet, tip 20,000 (f = 17,952) | network, tip 129,600 (f = 127,552) |
|---|--:|--:|
| empty, 2.4 ms (JM-REBUILD) | **96 s** (stalls ≈ 43 s and ≈ 48 s) | **10.4 min** (stalls ≈ 5.1 and 5.2 min) |
| full-gas transfers, 65 ms | ≈ 43 min | ≈ 4.7 h |
| bn_pair_2, 230 ms | ≈ 2.6 h | ≈ 16.6 h |
| kzg_point_eval, 206 ms | ≈ 2.3 h | ≈ 14.9 h |
| modexp_b8_e256_m8_even, 960 ms | **≈ 10.7 h** (stalls ≈ 4.8 h and ≈ 5.3 h) | **≈ 69 h** (stalls ≈ 34 h each) |
| modexp_b8_e1024_m8_even, 1.01 s | ≈ 11.3 h (not caps-feasible on the devnet for all 20,000 blocks) | ≈ 73 h |
| modexp_b8_e256_m8_even with the devnet state term (+0.23 s) | ≈ 13.3 h | — (network state term +15–18 s per block: weeks) |

The adopting stall adds the pool re-execution of the abandoned branch's transactions (≤ 1024 blocks' worth;
≈ 1 s each for the worst kind, so up to ≈ 17 min more).

**Correction (2026-10-24, `462fffc`, PR #27 review thread).** The walk in `detached_txs` was bounded by
2·REORG_RESTORE_MAX = 2,048 steps over both sides together, not 1,024 blocks per side. A reorg whose
sides total more than 2,048 blocks lost the oldest detached transactions. The walk
(`detached_blocks`) now reaches the common ancestor, so the restore covers up to R_keep = 2,048
abandoned blocks. For the worst kind (one ≈ 1 s transaction per block) the adopting stall therefore
adds up to ≈ 34 min, not ≈ 17 min. This figure is derived, not measured. The measured figures above
(§5.2, 50 abandoned blocks) are unchanged.

The empty-block figures (96 s devnet, 10.4 min network) are **empty-chain figures** (J2-M1); round 2's
devnet row used H = 17,952 (the fork point), which dropped the 2·d·a term (J2-m2). Every stall exceeds
the attempt window D_att (governance.md:6: network 300 s, devnet 25 s) even for empty blocks, and the
safe (k_safe=6) / finalized (k_final=60) horizons are far shorter in wall-clock. A **shallow** reorg
inside K_hist (fork point in the last 32 blocks, no rebuild) is `d·a` — ≈ 14 ms for the safe depth 6 on
empty blocks, ≈ 6 s with the worst measured kind — and is the common case. The passing 2048-deep adoption
test (REPAIR-REPORT §3b, 51.8 s) is at devnet height ~611 with empty blocks; it demonstrates
**correctness** of adoption at the spec depth and does **not** bound the latency at network height or
under load. A reverse-diff store (storage.md:70) would replace each f-term with a ≤ R_keep term,
independent of height, and would also remove the double rebuild.

### 5.4 Fit to the devnet and network profiles

| aspect | devnet (H_END 20,000, T_blk 4 s, D_att 25 s, t_exec 1 s) | network (H_END 129,600, T_blk 60 s, D_att 300 s, t_exec 3 s) |
|---|---|---|
| boots at this profile? | **yes** (A13 fits) | **no** — A13 history ≥ TXBYTES_CAP = 8 GiB > 6 GiB budget (§3.3) |
| journal disk, empty/typical | 23 MB … ~0.2 GB | (projected) 0.15 … ~0.6 GB |
| journal disk, max (cumulative caps) | ≈ 0.22 GiB | (projected) **≈ 11.3 GiB** |
| journal disk, adversarial re-fetch | grows per cycle within window (§4) | (projected) same |
| one block's re-execution, worst measured | ≈ 1.0 s (compute, small state); ≈ 1.3 s with slots/accounts at the caps; **≈ 10.2 s for a block of 380 transfers on a state with accounts, slots and code near the caps (§3.2d)**, where an empty block takes ≈ 0.41 s and a CREATE2-heavy child ≈ 2.4 s — above t_exec = 1 s and T_blk = 4 s | ≈ 1.0 s (compute); ≈ 15–18 s with slots/accounts at the caps (extrapolation of an extrapolation); with code at CODE_CAP too, ≈ 8 s per USER tx, ≈ 50 min per 380-transfer block and ≈ 80 s per empty block (linear projections far beyond the measured range, §3.2d); a create-heavy block adds ≈ 23–28 s of `current_total` scan (§3.2c) — far above t_exec = 3 s and T_blk = 60 s |
| open replay at H_END | 129 s (empty, measured; stage A 137 s) … **≈ 5.2 h** (worst kind) … **≈ 6.5 h** (worst kind, slots/accounts at the caps) … **≈ 7.3 h + ≈ 8.5 min** (worst kind with code at the cap too, plus the TX_CAP-bounded transfer blocks; §3.3) | ≈ 64–69 min (empty) … **≈ 35–38 h** (worst kind) … ≈ 3 weeks (state at the caps) |
| open memory | transient ≈ 1.0× the journal on top of the steady set (1.1–4.6× by kind, §3.4) | ≈ 11.3 GiB transient at the canonical maximum, on top of the steady set |
| deep reorg at R_keep near the tip | **96 s** (empty; two stalls ≈ 43 s and ≈ 48 s, each > D_att) … **≈ 11 h** (worst kind) … ≈ 13 h (slots/accounts at the caps) … ≈ 15 h (code at the cap too: 2·20,000·(0.96 + 0.41) s, scaled, not measured) | **10.4 min** (empty) … **≈ 69–73 h** (worst kind) |

t_exec: network 3 s (governance.md:7); devnet 1 s (`admission::NodeLocal::single_host`, D-A02-PROFILE).
R14 (governance.md:93) requires t_exec ≥ validate_p99. A single valid block above t_exec on this host is
evidence for the open A13 item 2 (`timing-not-a-bound`: the admission's timing measurement is a
representative sample, not a worst-case bound), not a gate measurement.

- **Devnet**: disk fits (tens of MB, ≤ 0.22 GiB canonical). **Open does not fit in the worst case**: the
  round-2 verdict "open ≈ 2 min … fit" is withdrawn (J2-B1). Open is ≈ 2.1–2.3 min for an empty chain but
  ≈ 5.2 h for a chain of the worst measured compute kind, and ≈ 6.5 h with the state at the devnet
  caps — against T_blk = 4 s. A deep reorg near H_END does not fit even on an empty chain (stalls of ≈ 43 s and
  ≈ 48 s, each > D_att = 25 s), and takes ≈ 11 h (≈ 13 h with the state at the devnet caps) under the worst kind.
  On a state with code near CODE_CAP as well, a single valid block of 380 transfers re-executes in
  ≈ 10.2 s, above T_blk (§3.2d); TX_CAP limits such blocks to ≈ 52 per chain.
- **Network**: not admissible on this node at all; as a projection it would (1) replay from genesis in
  ≈ 64–69 min for empty blocks and ≈ 35–38 h under the worst measured compute, weeks with the state
  at the caps; (2) take ≈ 10.4 min (empty) to ≈ 69–73 h (worst kind) for a legal deep reorg at the tip in
  two blocking stalls; (3) need ≈ 1× the journal of open transient (≈ 11.3 GiB at the canonical maximum)
  on top of its steady set; (4) grow unboundedly under the re-fetch vector within the window. None is a
  correctness fault; each is an operational limit that a snapshot/compaction, reverse diffs and an
  incremental state root (§7) would reduce, and the per-gas worst case is a pricing property of the
  Cancun precompile set the spec adopts (execution.md:106).

---

## 6. Spec comparison

| spec expectation | pocold | effect / classification |
|---|---|---|
| storage.md:61 records keyed by (height, blockHash) for the canonical chain | import-ordered append-only journal; by-hash/offset index in RAM (D-REORG-REDB) | functional parity for reads; no on-disk key index |
| storage.md:70 / consensus.md:230–232 reverse diffs up to R_keep, then TrieRebuild | **none kept**; TrieRebuild from genesis inside R_keep | **conformance gap**: deep-fork rebuild/restore re-execute from genesis (§5), paid twice per reorg (§5.3) |
| storage.md:73–77 inventory ~19.65 GiB logical / 29.5 GiB disk (incl. a live trie) | journal holds only headers+bodies+parentResult (≤ ~11.3 GiB); receipts/logs/code/state/trie re-derived; the world root is recomputed from the whole state per block (no live trie) | journal ≈ half the inventory; the rest is CPU/RAM at open; the per-block state term (§3.2b) is an implementation property, not a spec requirement; so are the per-tx snapshot validation and clones and the per-scan keccak of every runtime (§3.2d) |
| execution.md:106 Cancun semantics (EIP-2565 MODEXP pricing, the Cancun precompiles) | revm 19.7 Cancun precompile set (c-kzg enabled) | conforms; the per-gas worst case (§3.2a) follows from that pricing |
| network.md:7, :26–28 D38 R stage ≤ 4096 headers/branch, ≤ 16384 total; E stage GetBodies | pocold has no headers-first split: GetFrom/Block deliver full blocks and journal each | **conformance gap** (approved text): re-fetch re-journals full blocks (§4) |
| network.md:144 adoption at depth ≤ R_keep | implemented (D-REORG-REDB) | correct; the **cost** is §5 |

---

## 7. Mitigation options (all PROPOSED; none implemented here)

**Status: PROPOSED — not approved** (every option below; option 8 is a normative change).

Per the task, a mitigation is implemented only if it is **purely local storage behaviour covered by the
owner-approved D-REORG-REDB** (no change to validity, fork choice or RPC-visible data) **and fully
tested**. None of the following meets that bar within this track, so all are proposed, not implemented.
Two are **conformance gaps against already-approved spec text** (still needing a frozen design + tests
before implementation, like D-REORG-REDB itself); one is a **normative** change; the others are local
proposals.

1. **Headers-first sync (network.md D38 R/E stages — approved text, conformance gap).** Fetch and
   H-pre-validate headers (≤ 4096/branch) before bodies, and journal a block only once its branch is
   chosen. Reduces the §4 re-journal of a losing branch to header-sized work. The staging is approved
   (network.md:26–28); the "journal once the branch is chosen" part is the behaviour change. Needs a
   frozen design + tests; PROPOSED.
2. **Dedupe markers for re-imported blocks.** Record that a within-window block was already journaled so a
   solicited re-fetch serves the existing record instead of appending a copy. A marker record could still
   assign a new `importSeq` (consensus.md:205), and D118 (≤ 2 side per height, consensus.md:233–236)
   bounds how many markers a height needs; but proving replay equivalence and crash consistency
   (D-REORG-REDB R2.5) is a design task. PROPOSED.
3. **Journal compaction / signed snapshots.** Compact records below R_keep (or below finality) behind a
   signed state snapshot, so open replays from the snapshot rather than genesis and disk is bounded past
   R_keep (storage.md already envisions a signed snapshot after ts_F+30d). The most promising
   **local-storage** direction for open time: with the per-block worst case of §3.2a–b, it is the only
   option here that bounds open by the blocks since the snapshot rather than by H. A safe compaction must
   preserve the crash invariant, replay equivalence and serve/rebuild within R_keep — a frozen design +
   acceptance tests + mutation controls first. PROPOSED.
4. **Reverse diffs (storage.md:70, consensus.md:230–232 — approved spec, conformance gap).** Keep per-block
   reverse state diffs for the last R_keep blocks so a deep-fork rebuild/restore scales with depth, not
   height, and the §5.3 double genesis rebuild disappears. The spec's own mechanism; a substantial storage
   subsystem (diff capture on commit, apply on rollback, its own A13 term and crash handling). PROPOSED.
5. **Reduce the open per-record cost (local optimisation candidate, OPT).** Make `retain` O(window) by
   tracking the non-canonical set incrementally, and keep the canonical index incremental instead of
   `rebuild_canonical` walking head→genesis per head change (§3.1), to remove the b·H² term. Likely
   validity/fork-choice/RPC-neutral, but unimplemented and unverified here; it needs its own tests and a
   mutation control. It does not touch the per-block term, which dominates under load. PROPOSED.
6. **Keep the fork-point state while a side branch depends on it (new, J2-M2).** Let `retain` keep the
   state of a canonical block below head−K_hist that is the parent of a resident non-canonical block —
   for example counted as one of the side_max side states — so the adopting import's `restore_canonical`
   starts from it and the second genesis rebuild of §5.3 disappears (one rebuild of f instead of two).
   Local retention, validity-neutral by construction (states are re-derivable), but it changes the A13
   snapshot count (one more retained state per such fork point, bounded by side_max) and needs a test that
   the second rebuild is gone and a mutation control. A partial alternative to option 4. PROPOSED.
7. **Incremental state root and O(dirty) block checks (new, §3.2b, §3.2c).** Cache per-account storage
   roots and the trie between blocks and recompute only dirty accounts, and make the per-block scans
   incremental: `check_caps`, `sum_balances`, `validate_snapshot`, `live_counts`, **and
   `code_caps::current_total`** — the last called on every successful create, giving a create-heavy block
   its O(creates × live-accounts) term (§3.2c), which a cached running logical-code total keyed by the
   create's own delta would reduce to O(creates). Then a block's re-execution is proportional to what it
   touches, not to the whole state. The root value is unchanged (same MPT), so it is validity-neutral if
   proven equal; it is an lp4-execution change with its own memory term (a resident trie and the running
   totals) and needs an equivalence test against `world_root` and against `current_total`/`live_counts` on
   every block plus mutation controls. **Round 3c additions (§3.2d):** the per-USER-tx
   `code_caps::validate_snapshot` in `native::execute_for_profile` (and after every SYSTEM call), the two
   per-tx whole-snapshot clones in `native::build_engine` (`with_db(snapshot.clone())` and
   `NativeContext::from_snapshot`'s `pre_transaction`), and the keccak of every account's runtime in
   `code_caps::code_len` (`hash_slow`, which makes each of these scans O(logical code bytes)) — for
   example a per-account cached code length and verified hash, a running logical-code total validated
   once per block, and a copy-on-write overlay instead of a full clone. Each must be proven
   result-equivalent (the same Ok/Err and the same CODE_CAP decision as the full scans, and PAY's
   pre-transaction view unchanged); PROPOSED.
8. **Bound the re-execution cost per gas (normative, new, §3.2a).** The worst measured kinds come from the
   Cancun gas schedule the spec adopts (execution.md:106), e.g. EIP-2565 MODEXP pricing of small moduli.
   Repricing (in the direction of later Ethereum proposals that raise the MODEXP minimum and small-modulus
   cost) or capping precompile inputs would change validity and the gas schedule: a normative change for
   the owner, not a local option. Listed only so the per-block worst case has an owner; PROPOSED, not
   designed here.

---

## 8. Obligations — all three remain OPEN

- **Journal disk growth** (open costs 3 and 4): OPEN. The canonical journal is bounded (≈ 11.3 GiB at
  network H_END, ≈ 0.22 GiB devnet, §2) but append-only with **no compaction**; the **adversarial
  re-fetch** vector (§4) grows it per cycle while targets stay in the R_keep window, and costs a genesis
  rebuild per cycle below K_hist (≈ 0.8 s at a height-300 fork point on empty blocks; up to ≈ 34–36 h per
  cycle at a network-scale fork point under the worst measured kind, §4.1). Per-session re-journal is
  bounded by `protect_max`; cross-session growth is gated by honest progress and the moving window. No
  mitigation implemented.
- **Re-execution from genesis** (open costs 1 and 2): OPEN. Open replays every record from genesis (§3).
  At devnet H_END it takes from ≈ 2.1–2.3 min (empty) to ≈ 6.5 h (worst measured kind with the state at the
  devnet caps); at network H_END (projection) from ≈ 64–69 min (empty) to ≈ 35–38 h (worst compute)
  and weeks with the state term. A deep-fork rebuild/restore re-executes from genesis **twice** for want of
  reverse diffs (§5.3), so a legal depth-2048 reorg near the tip takes 96 s (devnet) / 10.4 min (network)
  **on an empty chain** and up to ≈ 11 h (≈ 13 h with the state at the devnet caps) / ≈ 69–73 h under the worst measured kind, in two stalls
  that each exceed D_att. The worst figures are lower bounds on the worst case (§3.2a). Beyond the compute
  and the §3.2b per-block term, a block's re-execution grows with **txs × state** — every USER tx runs
  `validate_snapshot` and two whole-snapshot clones — and **creates × state** — every create runs
  `current_total` — and each scan is O(accounts + slots + logical code bytes) (§3.2c, §3.2d; round 3c,
  correcting round 3b). Measured at a devnet state with accounts, slots and code near the caps: an empty
  block ≈ 0.41 s, a CREATE2-heavy child ≈ 2.4 s, and a block of 380 transfers **≈ 10.2 s** (> T_blk 4 s),
  the per-block worst measured; round 3b's "create-heavy block is the per-block worst" is withdrawn. The
  devnet TX_CAP bounds the transfer blocks to ≈ 52 per chain; at network caps the per-tx term is
  projected at ≈ 8 s per tx (a linear projection far beyond the measured range, §3.2d). No snapshot,
  diff, state-root, incremental-code-total or per-tx-scan mitigation implemented.
- **Open-time memory transient**: OPEN. Open needs ≈ 1.0× the journal in RAM **on top of** the steady
  resident chain (§3.4; peak/journal is 2.1× (body-max) to 5.7× (transfers) by block kind), excluded from the A13 steady-state
  bound; at the network maximum or the adversarial journal it exceeds the budget, so a node can be pushed
  into a state where it cannot restart. No mitigation implemented.

The passing 2048-deep reorg test demonstrates **correctness** of adoption at the spec depth; it does not
bound the open time, disk growth, deep-reorg latency, or open memory this report measures, and does not
close these obligations.

---

## 9. Review findings — resolution (round 2), with round-3 corrections

| # | finding | resolution |
|---|---|---|
| J-V1 (blocker) | deep-reorg model omits the second genesis rebuild | **Fixed in report (round 2); mechanism corrected in round 3 (J2-M2).** Measured 600 rebuilds for a fork point at height 300 (`JM-REORG`); latency ≈ 2·rebuild(f) + 2·d·a in two stalls, each > D_att. Round 2's cause (retain before restore_canonical) was wrong; the cause is retain pruning the canonical fork point (§5.3). The figures are empty-chain figures; ranges in §5.3. Not a code defect; mitigation stays PROPOSED. |
| J-V2 (major) | re-fetch vector also costs a genesis rebuild per cycle | **Fixed in report + witness.** New witness forks below K_hist; `WITNESS-REFETCH-below-khist` shows 316 rebuilds/cycle (fork-point height 300 + 16). §4.1 adds the CPU amplification to the open obligation, as a range in round 3. |
| J-V3 (major) | witness skips the real session path; boundsChecked were literals | **Downgraded to analysis.** §4.2 describes the real lifecycle (end_session does not retain; per-session bound = protect_max × record size; cycle rate gated by honest progress), cites the committed positive control, and the new witness OBSERVES its bounds. The refetch claim is PARTIAL, not VERIFIED. |
| J-V4 (major) | ~2.1× journal RAM at open; network profile not admissible | **Fixed in round 2; resolution corrected in round 3 (J2-M3).** Round 2 measured the open **peak** (2.34× on a body-max journal) and called it the transient. The transient (peak − steady) is ≈ 1.0× the journal for every kind measured; the steady part varies with the block kind (§3.4). The network profile is not admissible (A13 history ≥ TXBYTES_CAP). |
| J-V5 (major) | VmHWM misattributed; peak replay RSS never measured | **Fixed.** §3.4: the stage-A VmHWM was the build-phase peak; the fresh-process measurement isolates the open peak (round 3: without a pre-read of the journal, J2-m6). |
| J-V6 (major) | wrong cap for tx-load open; compute-heavy block unmeasured | **Reopened in round 3 (J2-B1).** Round 2 measured one compute kind (the jump loop, 9.6 ms) and took it as the worst case; it is the cheapest per gas. §3.2a measures 45 one-tx full-gas kinds (worst: MODEXP, ≈ 1.0 s per block), §3.2b a state-size term, and §3.3 restates the open bounds. The TX_CAP bound for transfer load stands. |
| J-V7 (major) | JM-BYTES from a pre-fix harness; single samples | **Fixed.** JM-BYTES re-run on the committed test (`journal-measure-r2.log`); §1.1 reports record size as per-share slopes and notes the share-count variance. |
| J-V8 (major) | jm_restore_cost cannot measure a deep restore | **Fixed.** Replaced with `jm_deep_reorg_latency` (fork below K_hist); restore measured within the real deep reorg (§5.2), round 3 also with a loaded chain. |
| J-V9 (minor) | fit params were not the fit; "lower bound" artifact | **Fixed; range added in round 3 (J2-m5).** Least-squares fit reproduces 137 s; the network figure is now a range because b grows with H (§3.1, §3.3). |
| J-V10 (minor) | Θ(H²) attributed to retain alone | **Fixed; attribution limited in round 3 (J2-m5).** over 20,000 blocks retain + insert/rebuild_canonical measure ≈ 106% of the fitted b (round 2's 6,000-block split: ≈ 79%); only these measured shares are attributed (§3.1). |
| J-V11 (minor) | parentResult byte/share and worst-case evidence/total | **Fixed; corrected in round 3 (J2-m3).** 31 B/share (§1), parentResult 0.96 GiB; the canonical worst case is ≈ 11.3 GiB (evidence bounded by one item per identity), not a 10.3–13.3 GiB range (§2.1). |
| J-V12 (minor) | inconsistent devnet disk / cited line / duplicate fps | **Fixed.** devnet empty 22.9 MB measured (§2.2); `jm_deep_fork_rebuild_vs_height` fps de-duplicated and re-run. |
| J-V13 (minor) | spec-citation/classification gaps | **Fixed.** §6/§7/§9: MIT-1 and MIT-4 re-labelled conformance gaps against approved text; consensus.md §10 citation corrected (covers a heavier branch, not a lighter evicted one); D118/marker note added for MIT-2. |

---

## 10. Second review findings — resolution (round 3)

| # | finding | resolution |
|---|---|---|
| J2-B1 (blocker) | the compute worst case was the cheapest loop; open-time conclusions wrong | **Fixed in report; obligation stays OPEN.** `jm_exec_cost_per_gas` (committed, `#[ignore]`) measures 45 one-tx full-gas kinds at the 8M gas limit, including the requested precompile loops (bn256 pairing 1/2/10 pairs, ecrecover, blake2f, KZG point evaluation — enabled in this Cancun profile — and a 23-shape MODEXP grid), keccak and memory-expansion loops (§3.2a). Worst measured: `modexp_b8_e1024_m8_even` 1.02 s per block (128 ns/gas net), ≈ 125× the jump loop; pairing 0.22–0.26 s, KZG 0.21 s. 9.6 ms is labelled a lower bound. A state-size term is added (§3.2b). The open bounds are restated for both profiles (§3.3), the devnet "open fits" verdict is withdrawn (§5.4), and J-V6 is reopened (§9). |
| J2-M1 (major) | deep-reorg and re-fetch CPU projected with empty blocks only | **Fixed in report.** `jm_rebuild_by_kind` measures a genesis rebuild over loaded blocks (§5.1; pairing 230 ms per block reproduces the review's 231 ms; worst ≈ 1.0 s); `jm_deep_reorg_latency` with `JM_REORG_KIND` measures a loaded deep reorg (§5.2). §4.1, §5.3, §5.4 and §8 give ranges from the empty-block to the worst measured cost and label 96 s / 10.4 min as empty-chain figures. |
| J2-M2 (major) | wrong mechanism for the second rebuild | **Fixed in report and test docstring.** §5.3 and the `jm_deep_reorg_latency` docstring state that `retain` prunes the canonical fork point below head−K_hist while a resident side branch depends on it, already at the first side import; reordering would not help (the review's mutation). `JM-REORG-MECH` shows it on a bare chain, and `forkPointPrunedAfter` on the node. Keeping the fork-point state is option 6 in §7, beside reverse diffs. |
| J2-M3 (major) | "transient ≈ 2.3×" was the peak | **Fixed in report and test.** `jm_open_memory`'s fresh path reports peak, steady (trimmed RSS, node alive) and transient separately; §3.4 gives them per block kind (full-body-max, empty, transfers): transient ≈ 1.0×, 2.3× is the body-max peak. J-V4's resolution is corrected (§9). |
| J2-m1 (minor) | setup: 273e785 lacks the journal r2 work | **Acknowledged.** Round 3 is based on `fa97594`, which contains the journal r2 merge `0d6f9dd`; the round-3 runs are on `7f587bb` (two test-only commits on fa97594). |
| J2-m2 (minor) | devnet reorg row used the fork point as H | **Fixed.** Both rows use the tip: devnet 2·20,000·2.4 ms = 96 s (stalls ≈ 43 s and ≈ 48 s), network 622 s (§5.3, §5.4). |
| J2-m3 (minor) | canonical worst case mixed an inventory and an unreachable ceiling | **Fixed.** ≈ 11.3 GiB with the identity bound on evidence (chain.rs validate item 15, resources.rs:98–101), §2.1, §2.2, §5.4, §8. |
| J2-m4 (minor) | "two obligations" vs three | **Fixed.** "Three obligations" in the header and §8. |
| J2-m5 (minor) | 69 min from a single-run 3-point fit; split attribution | **Fixed.** Six checkpoints in round 3; the LS and two-point fits give a range (≈ 64–69 min), and the two-point b grows with H, so the projection is not an upper bound. `JM-SPLIT` over 20,000 blocks measures the insert + retain slope at ≈ 106% of the fitted b (round 2's 79% came from a 6,000-block split, where the slope is lower); only the measured shares are attributed (§3.1, §3.3). |
| J2-m6 (minor) | fresh path pre-read the journal; stale docstring | **Fixed in test.** Journal size from `std::fs::metadata`, heap trimmed (`malloc_trim`, glibc only; a no-op elsewhere) before the baseline; the docstring says what the baseline is and drops "~2x". |
| J2-m7 (minor) | stale docstring, unlabelled profile, unsourced intercept | **Fixed.** `jm_replay_empty_blocks` docstring (super-linear, not linear); the measured profile is labelled (harness `cfg()`: d_att 73, hash_rate 1024, c 8, tau 40) in the module docstring and the profile note; the intercept is measured as `JM-OPEN0` (61.6 ms) and used in §3.2. |

---

## 11. Third review findings — resolution (round 3b)

Measurement and documentation only; no `src` file changed. Code under test `e3dfb47` (base `256428f`);
evidence `evidence/journal-r3b/`.

| # | finding | resolution |
|---|---|---|
| J-REV3-1 (major) | the per-gas search omitted state-interacting families (SSTORE cold/warm, cold CALL, EXTCODECOPY, LOG, CREATE2); the create path's O(creates × live-accounts) scan (`code_caps::current_total`) is in neither the §3.2b additive model nor §7 option 7, so whether MODEXP is the per-block worst at a full state is not established | **Fixed in report; obligation stays OPEN.** Six state-interacting kinds added to `compute_kinds` and measured by `jm_exec_cost_per_gas` on the small state (§3.2a): all ≤ 2.1 ns/gas there, so MODEXP stays the per-gas worst on a small state and LOG is bounded by LOG_COUNT_MAX. New `#[ignore]` test `jm_create_state_term` (§3.2c) grows the live-account state and measures a CREATE2-heavy child, a cold-CALL control and a many-small-create block: the create-specific term (CREATE2 − empty child) rises ≈ 63 → ≈ 1,972 µs/create as accounts grow to 15,645, while the cold-CALL control tracks the empty child — isolating `current_total` as an O(creates × live-accounts) term. Projection: ≈ 0.54 s/block at the devnet ACCT_CAP, ≈ 27.6 s/block at the network ACCT_CAP, additive to the §3.2b root term; on a full state a create-heavy block, not MODEXP, is the per-block worst. §7 option 7 adds `current_total` to its scan list; §8 and §5.4 updated. The worst figures remain lower bounds. *Corrected in round 3c (§12, JV3B-1/2): the per-block worst is a many-transaction block, not the create-heavy one, and the term is O(creates · (accounts + code bytes)).* |
| J-REV3-2 (minor) | the network state-size figures (≈ 15–18 s/block, open ≈ 3 weeks) are an extrapolation of an extrapolation but stated as the obligation's magnitude | **Fixed.** §3.2b now states the three compounding steps inline (synthetic root with no chain → scaled by the devnet validate/root ratio 1.23 at a ≈ 1,000× larger state → extended linearly to the block and to H_END), and §3.3/§5.4/§8 carry the "extrapolation of an extrapolation" qualifier where the multi-week / 15–18 s figures appear; the §3.2c network create term carries the same qualifier. |
| J-REV3-3 (minor) | the devnet state term +0.23 s is from the "both" series endpoint (≈ 95% of the caps), not the caps | **Fixed.** §3.2b states the measured fill (47,704 slots ≈ 95% of SLOT_CAP, 18,769 accounts ≈ 94% of ACCT_CAP) and extends by the measured slopes (≈ 2.5 µs/slot, ≈ 4.3 µs/account) to an at-cap empty-child of ≈ 246 ms, i.e. ≈ +0.25 s per block; the measured figure under-states the at-cap value by ≈ 11 ms (`derived.log` [11]). |

---

## 12. Fourth review findings — resolution (round 3c)

Measurement and documentation only; no `src` file changed (the mutants of §3.2d are scratch edits, reverted).
Code under test `1d094de` (base `11f8e89`); evidence `evidence/journal-r3c/` (`INDEX.md`).

| # | finding | resolution |
|---|---|---|
| JV3B-1 (blocker) | "a create-heavy block, not MODEXP, is the per-block worst" and "the §3.2b term is a fixed per-block cost" are false: every USER tx runs `validate_snapshot` and two whole-snapshot clones, so a plain many-tx block is the per-block worst on a large state; the many-small-create excess was misattributed to a pending-pool scan | **Fixed in report; conclusion withdrawn; obligation stays OPEN (stronger).** New `#[ignore]` test `jm_tx_state_term` measures a 380-transfer block, a CREATE2-heavy child and a 149-create block on account-, code-, slot- and all-filled states (§3.2d). On every grown state the transfer block is the worst: 2.25 s (15,415 accounts), 5.30 s (2.05 MB of code, 67 accounts), **10.25 s** with all three near the devnet caps (> T_blk 4 s), against 0.53 / 1.85 / 2.38 s for the CREATE2 child. The model is restated (per-block term + txs × (validate_snapshot + 2 clones) + creates × current_total). Withdrawn in the header, §3.2b, §3.2c, §5.4, §8 and §11; the pending-pool explanation is withdrawn (§3.2c). TX_CAP bounds the transfer blocks to ≈ 52 per devnet chain (§3.3). Mitigations added to §7 option 7, PROPOSED. |
| JV3B-2 (major) | `code_len` keccaks every runtime, so `current_total` and `validate_snapshot` are O(accounts + logical code bytes), bounded by CODE_CAP, not ACCT_CAP; the account-only fit and the §3.2b state model miss the code dimension | **Fixed in report.** §3.2c and §3.2d state each scan as O(accounts + slots + code bytes). The code series (62 deploys of 32,768 B, 97.6% of the devnet CODE_CAP, 67 accounts) gives `validate_snapshot` 11.5 ms, an empty block 124 ms, a CREATE2 child 1.85 s (7.0 ms per create, where the account fit predicts ≈ 32 µs) and a transfer block 5.30 s. Mutant D (skip `hash_slow`) removes the whole code dimension (code-state empty block 124 → 1.6 ms), so it is the keccak in `code_len`. The devnet at-cap empty-block term is ≈ 413 ms with code (not ≈ 246 ms) and the worst devnet open ≈ 7.3 h (+ ≈ 8.5 min of transfer blocks). Network figures (≈ 6.2 s per `validate_snapshot`, ≈ 7.9 s per tx, ≈ 80 s per empty block) are labelled linear projections ≈ 512× beyond the measured code range. §7 option 7 lists `hash_slow` in `code_len` (cached code length/hash or a running total), PROPOSED. |
| JV3B-3 (minor) | the 0.112 µs/account slope and the 0.54 s / 27.6 s projections come from one run with a high last point; negative intercept; the within-tx O(creates²) part unstated | **Fixed.** Two more runs of `jm_create_state_term` on `1d094de` (`create-r1.log`, `create-r2.log`): at A ≈ 15,645 the three runs give 1,596 / 1,691 / 1,972 µs/create. The fit on per-checkpoint medians gives ≈ 0.49 s per 247-create block at the devnet ACCT_CAP (0.47–0.54 s over the single-run fits) and 23.0–27.6 s at the network ACCT_CAP (projection) (`derived.log` [14]). Mutant A shows the within-tx part: the CREATE2 child on the genesis state takes 4.4 ms without `current_total` against 18.0 ms with it on the same build (≈ 55 µs per create from the journaled state growing inside the tx); the negative intercept is a fitting artefact (§3.2c). |
| JV3B-4 (minor) | `evidence/journal-r3b/INDEX.md` cites "§10 J-REV3-1/2/3"; the round-3b table is §11, and §3.2c is omitted | **Fixed by a new file.** `evidence/journal-r3b/INDEX.md` is an existing evidence file and is not edited; `evidence/journal-r3c/INDEX.md` carries the correction (round-3b report sections: §3.2a, §3.2b, §3.2c and §11). |
| survivor: mutant B (skip `validate_snapshot` in `execute_for_profile`) | no committed child did many txs on a grown state, so the per-tx `validate_snapshot` term was never isolated | **Killed by the new measurement.** In `jm_tx_state_term` mutant B changes the code-state transfer block from 5.18 s to 0.19 s and the accounts-state one from 1.80 to 1.07 s (`mutants/mut-B.log` against `mutants/mut-none.log`, §3.2d). |
| survivor: mutant A (`current_total` → 0) | it left the many-small and transfer blocks largely in place, contradicting "the same regime" | **Fixed in report.** §3.2c withdraws "the same regime"; mutant A takes the 149-create block from 967 to 776 ms (accounts) and from 2.90 to 2.07 s (code), mutant B to 766 ms and 1.17 s; the transfer block is unchanged under A (§3.2d). |
