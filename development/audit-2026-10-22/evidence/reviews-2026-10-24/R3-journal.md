# R3 — Independent review of the journal study (round 3c, `1d094de` / merge `bce7a6c`) and of `462fffc`

Reviewer: independent agent, 2026-10-24. Subject: `development/audit-2026-10-22/JOURNAL-GROWTH-AND-REPLAY.md`
(cited as "the study", line numbers at `ec148cf`), `prototypes/pocold/tests/journal_measure.rs`,
`development/audit-2026-10-22/tools/journal_projections.py`, `evidence/journal-r3*`, and the code it measures
(`store.rs`, `node.rs`, `chain.rs`, `resources.rs`, lp4-execution `native.rs`/`code_caps.rs`), A13-DERIVATION §4/§4a.

## Summary

| ID | sev | one line | status |
|---|---|---|---|
| R3-2 | HIGH | a reverting CREATE2-heavy tx pays every `current_total` scan without growing the state → repeatable at every height; devnet open ≈ 12.6 h, deep reorg ≈ 26 h (not 7.3 h / 15 h) | REPRODUCED |
| R3-3 | HIGH | any reorg in the journal (even depth 1) → the end-of-open `restore_canonical(0)` re-executes the chain up to it again (genesis rebuild or restore span): open ≈ 2× | REPRODUCED |
| R3-1 | MEDIUM | "7.3 h + 8.5 min" double-counts TX_CAP (room for 0.7 transfer blocks, not 52) and omits the modexp tx's per-tx term; correct ≈ 7.45 h (before R3-2/R3-3) | REPRODUCED |
| R3-6 | MEDIUM | every new head re-executes the whole pool; no gas bound → ≈ 68–95 min per head with 4096 MODEXP txs | REPRODUCED |
| R3-7 | MEDIUM | 462fffc: note's bound (≤ 2,048 blocks, ≈ 34 min) right; removed hash filter makes each already-included detached tx pay `validate_snapshot` (≈ 14–24 ms, ≤ TX_CAP); stale ≤ 1024 / 17 min text; measured figures unchanged | REPRODUCED |
| R3-8 | LOW | network empty open "64–69 min" too narrow: 63–101 min across models; steps = hashbrown resizes | REPRODUCED / CODE-READING |
| R3-9 | LOW | no re-run within ±5 %: +15–63 % compute, 1.4–3.5× scans under load; structure reproduces | REPRODUCED / NOT-REPRODUCED (absolute) |
| R3-10 | LOW | open transient up to ≈ 2× the journal (parse holds raw bytes + payload copies), not 1× for re-journal-dominated journals; no streaming mitigation proposed | CODE-READING |
| R3-11 | LOW | `retain` orphan cascade O(depth × blocks) per deep-branch removal | CODE-READING |
| R3-12 | NOTE | 11.3 GiB verified; RLP headers and honest side records omitted | REPRODUCED |
| R3-13 | NOTE | side_max is 2 (SIDE_MAX), not 8 | CODE-READING |
| R3-14 | NOTE | "src equals fa97594" no longer literally true at HEAD (measured paths unchanged) | REPRODUCED |
| R3-15 | NOTE | EVICTED_MAX is a 1,024-entry FIFO; attacker imports run retain — §4.2 pacing not established | CODE-READING |

Verified correct: the a·H + b·H²/2 fit (a = 2.0479 ms, b = 4.3789×10⁻⁴ ms; 65.7 / 69.2 min), the 96 s / 10.4 min
and 10.7 h / 69 h reorg rows, 11.316 GiB, 0.222 GiB devnet, spec inputs, the J2-M2 mechanism (re-run on HEAD),
and byte-identical script output.

## 0. Setup and method

- The assigned worktree was on `main` content; it was detached onto `claude/audit-repair-a01-a14` tip
  `ec148cf` (no branch moved, nothing committed). `git status` is clean apart from one **untracked** scratch
  test (`prototypes/pocold/tests/zz_r3review_scratch.rs`, deleted at the end, see §0.2).
- Code-under-test check: `git diff --stat fa97594 HEAD -- prototypes/{pocold,lp4-execution}/src` touches only
  `node.rs` (462fffc), `admission.rs` (462fffc calibration), `rpc.rs`/`net.rs`/`lib.rs`/`resources.rs` (RpcGuard
  comments/limits). `store.rs`, `chain.rs`, lp4 `apply`/`native`/`code_caps`/`user_phase` are identical to
  what the study measured, so its replay/validate/rebuild measurements still apply to HEAD.
- Build: `CARGO_TARGET_DIR=…/scratchpad/review-target cargo test --offline --release --test journal_measure --no-run`
  (0 warnings); the binary was run directly, `--exact --ignored --nocapture --test-threads=1`, `TMPDIR` in the
  scratchpad, 900 s timeout per run, load average logged at start and end of each run.
- **Host load caveat.** Other review sessions were compiling and running tests in the same target dir; the
  1-minute load average was 1.9–2.2 during the exec run, ≈ 3.9–6.5 during the rebuild/txstate runs and up to
  ≈ 8.8 later (4 CPUs). Every re-run number below must be read with that load. Logs:
  `/tmp/claude-0/-home-user-Proof-of-Collaboration/5c42c059-8cb3-5c8b-83e8-431ffbd082d1/scratchpad/reviews/r3-runs/`.

### 0.1 Re-runs of committed `#[ignore]` measurements (Q1)

| measurement | committed (log) | re-run (this review) | ratio |
|---|---|---|---|
| `jm_exec_cost_per_gas` empty_fallback | 1.7 ms (r3 exec.log) | 2.76 ms | 1.62 |
| … full_gas_transfers (380 tx) | 62.4 ms (73 ms in exec-repeat) | 97.6 ms | 1.34–1.56 |
| … jump_loop | 8.2 ms | 9.59 ms | 1.17 |
| … ecrecover | 94.6 ms | 145.8 ms | 1.54 |
| … bn_pair_2 | 238.4 ms | 315.1 ms | 1.32 |
| … kzg_point_eval | 209.5 ms | 257.4 ms | 1.23 |
| … modexp_b8_e256_m8_even | 979.7 ms | 1191.1 ms | 1.22 |
| … modexp_b8_e1024_m8_even | 1023.0 ms | 1174.3 ms | 1.15 |
| … native MODEXP e1024 (JM-PRECOMPILE) | 428.3 µs | 416.4 µs | 0.97 |
| `jm_rebuild_by_kind` empty / transfers / bn_pair_2 / modexp_e256 per block | 1.79 / 65.0 / 229.9 / 959.9 ms | 2.91 / 103.6 / 339.6 / 1183.1 ms | 1.63 / 1.59 / 1.48 / 1.23 |
| `jm_tx_state_term` accts-end (15,415 accts): empty / 380 transfers / CREATE2 | 132.8 ms / 2.25 s / 529 ms (txstate.log); 108.8 ms / 1.80 s / 515 ms (mut-none) | 330.8 ms / 6.26 s / 1.84 s | 2.5–3.5 |
| `jm_tx_state_term` code-end (2.05 MB code): empty / 380 transfers / CREATE2 | 124.4 ms / 5.30 s / 1.85 s | 170.6 ms / 7.90 s / 2.52 s | 1.37 / 1.49 / 1.36 |
| `jm_tx_state_term` all series, code-end | 119.5 ms / 4.96 s / 1.63 s | 202.4 ms / 7.78 s / 3.01 s | 1.7 / 1.57 / 1.85 |
| `jm_tx_state_term` all series, slots-end (198 accts, 46,636 slots, 2.05 MB) | 256.3 ms / 5.34 s / 1.80 s | 407.5 ms / 13.0 s / 3.30 s | 1.6 / 2.4 / 1.8 |
| `jm_tx_state_term` all series, accts-end (the 10.25 s headline) | 412.6 ms / **10.25 s** / 2.39 s | not reached: the run hit the 900 s cap after slots-end (committed run took 686 s on an idle host) | — |
| `jm_deep_reorg_latency` modexp_e256, C = 100, f = 50 (**after 462fffc**) | first 48.17 s (50 rebuilds) / adopting 97.73 s (50 rebuilds + 50 restored) / reopen 193.25 s | first 62.19 s (50) / adopting 131.40 s (50 + 50 restored) / reopen not reached (900 s cap); JM-REORG-MECH identical (`stateAfterNextRetain: false`) | 1.29 / 1.34; adopting/first 2.11 vs 2.03 |

(The `exit=` field of my run logs is the exit status of `date`, not of the test; the two runs that hit the cap
are identified by their missing `test result` line.)

Reading: **the structure reproduces** (kind order modexp > pairing > KZG > ecrecover > transfers > jump loop;
modexp ≈ 124× the jump loop again; transfer block the per-block worst on every grown state; CREATE2 child grows
with state) but **no absolute figure reproduced within the study's ±5 %**: every re-run was 15–63 % slower
on the compute kinds and 1.4–3.5× slower on the state-scan/clone terms. The native precompile timing is
unchanged (0.97×) while the in-EVM block is 1.15–1.22× and the clone-dominated accounts state 2.5–3.5×, i.e.
the variance is memory-bandwidth/cache contention, not the code. Finding R3-9.

### 0.2 New scratch measurements (untracked test, deleted after the runs)

`zz_r3review_scratch.rs` (untracked, deleted after the runs; worktree returned clean) = `journal_measure.rs` (inner docs removed) plus two `#[ignore]` tests:
`r3_revert_create_pool` (findings R3-2, R3-4, R3-5) and `r3_open_restore_span` (finding R3-3). Results are
in `r3-runs/scratch.log` and `r3-runs/span.log`; the source of the two tests is kept in
`r3-runs/r3_addition.rs` and `r3-runs/r3_addition2.rs`.

### 0.3 Projections (Q2)

`python3 -I tools/journal_projections.py evidence` reproduces `evidence/journal-r3c/derived.log`
**byte for byte** (`r3-runs/derived-rerun.log`, `cmp` identical). An independent stdlib script
(`r3-runs/indep.py`, output `indep.log`) re-derives from the raw JM-* lines:

| quantity | study | independent | status |
|---|---|---|---|
| LS fit a·H + b·H²/2 (r3, 6 pts) | a = 2.05 ms, b = 4.38×10⁻⁴ ms | a = 2.0479 ms, b = 4.3789×10⁻⁴ ms | match |
| network empty open | 65.7 min (r3), 69.2 min (stage A), range 64–69 | 65.7 / 69.2 min; 63.6–69.2 incl. two-point fits | match (range: see R3-8) |
| devnet empty open | 129 s measured | fit 128.5 s | match |
| canonical worst-case journal (network) | ≈ 11.3 GiB | 0.371 + 8.000 + 1.979 + 0.961 + 0.005 = 11.316 GiB | match (R3-12 nit) |
| devnet canonical max | ≈ 0.22 GiB | 0.222 GiB | match |
| devnet worst open (all-caps state) | ≈ 7.3 h + ≈ 8.5 min | 7.31 h by the study's formula; TX_CAP leaves room for 0.72 transfer blocks, not 52; with the modexp tx's own per-tx term 7.45 h | **arithmetic right, decomposition wrong (R3-1)** |
| deep reorg 2·(f+d)·a, d = 2048 at the tip | 96 s / 10.4 min (empty); 10.7 h / 69 h (modexp e256); 11.3 h / 73 h (e1024); 13.3 h (+0.23 s) | 96 s / 622 s; 10.67 h / 69.1 h; 11.27 h / 73.0 h; 13.26 h | match |
| stalls (empty, devnet) | ≈ 43 s and ≈ 48 s | 43.1 s / 48.0 s | match |

Spec inputs checked against `governance.md:5–16` (T_blk 60, D_att 300, t_exec 3, S_max 256, M_max 256, R_max 2,
E_max 4, EVID_BYTES_MAX 8192, GAS_LIMIT 8·10⁶, BODY_MAX 196,608, TX_MAX 131,072, CODE_CAP 2³⁰, TXBYTES_CAP 2³³,
TX_CAP 10⁷, SLOT_CAP 4·10⁶, ACCT_CAP 10⁶, K_hist 32, R_keep 2048, H_END 129,600), `storage.md:70,76,79`,
`consensus.md` §10 (reverse diffs, rollback to R_keep then TrieRebuild), `network.md:144` (adoption ≤ R_keep),
and `devnet.rs:36–41` (H_END 20,000, TX_CAP 20,000, TXBYTES_CAP 2²⁴, SLOT_CAP 50,000, ACCT_CAP 20,000, CODE_CAP
2²¹, M_max 64, R_max 0, T_blk 4, D_att 25). All match except side_max (R3-10). `HDR_MAX` = 3067 ≤ the
3072 B header bound used in §2.1.

---

## 1. Findings

Severity scale: HIGH = a stated obligation/bound is wrong by a large factor or a major cost is missing;
MEDIUM = a headline figure or the 462fffc assessment is wrong or incomplete; LOW = a qualifier, range or
attribution is wrong; NOTE = editorial/stale text.

### R3-2 — HIGH — a CREATE2-heavy transaction that REVERTs pays all its `current_total` scans and leaves the state unchanged: the state-scan worst case is repeatable at every height, so the devnet open, rebuild and reorg bounds are ≈ 1.7× too low
- **Where.** Study §3.2c/§3.2d ("Whether the cumulative number of creates over a chain is bounded is not
  analysed here", L670–671), §3.3 rows L712–714, §5.3 table, §5.4, §8; A13 §4a; lp4 `code_caps.rs:43–76`.
- **Claim.** The per-block worst on a grown state is the 380-transfer block (TX_CAP-bounded to ≈ 52 per chain);
  the open bound is all-modexp on the all-caps state (≈ 7.3 h); the CREATE2 child "grows the state".
- **Evidence.** `register`'s `create_return` handler calls `current_total` at every successful inner create,
  before the enclosing frame decides its fate. Scratch test `r3_revert_create_pool` (`scratch2.log`):
  `create2_loop` with its final STOP replaced by PUSH0 PUSH0 REVERT. On the genesis state the reverting block
  takes 25.9 ms against 28.2 ms and adds **0** accounts against 247; on the code-filled state (62 × 32 KiB,
  72 accounts) **3.28 s against 3.36 s, 0 new accounts against 247**, gas 7,942,088 in both. So one valid
  1-tx block costs ≈ the CREATE2 child (2.38 s on the all-caps state, committed txstate.log), does not grow
  the state, uses 1 tx of TX_CAP and ≈ 135 B of TXBYTES_CAP, and can sit at every height. Per TX_CAP unit it
  costs ≈ 1.7× the modexp block on that state (2.38 s against 0.937 + 0.411 + 0.026 s). Devnet worst open
  becomes ≈ 19,451 × ≈ 2.33 s + 88 s ≈ **12.6 h** (vs 7.3–7.45 h), and the devnet depth-2048 reorg at the tip
  ≈ 2·20,000·2.33 s ≈ **26 h** (vs ≈ 15 h). The network figure is unmeasurable here (O(creates·(accounts + code
  bytes)) at 10⁶ accounts / 1 GiB code).
- **Status.** REPRODUCED (scratch measurement under load ≈ 10; the ratio revert/normal is what matters).
- **Correction.** Withdraw "whether cumulative creates are bounded is not analysed": they are not bounded (a
  reverting create costs the scan without growing the state). Make the reverting create-heavy block the
  per-TX_CAP-unit worst on a grown state, re-derive §3.3/§5.3/§5.4/§8 and A13 §4a (devnet open ≈ 12.6 h, deep
  reorg ≈ 26 h, both lower bounds), and add a reverting-create kind to `compute_kinds`. Option 7's running
  logical-code total removes the term.

### R3-3 — HIGH — any reorg in the journal, even depth 1, makes every open re-execute the chain up to that fork point a second time (end-of-open `restore_canonical`)
- **Where.** `node.rs:226–228` (`chain.restore_canonical(0, …)` once, after replay), `chain.rs:557–580`; study
  §3 (open model), §3.3 bounds, §5.2 (JM-REORG-OPEN attributed only to "replay re-pays both rebuilds" of a deep
  reorg), §8.
- **Claim.** Open(H) = Σ per-record re-execution + b·H²/2 + JM-OPEN0; a deep reorg's rebuilds are re-paid at
  open; a shallow reorg inside K_hist is "the common case" and cheap.
- **Evidence.** Replay offloads every side block at `retain` (body_off is set before insert, `node.rs:218`) but
  never restores at an adoption, so every adopted branch stays offloaded until the single
  `restore_canonical(0)` at the end of open. That call (a) rebuilds the state of the parent of the **lowest**
  offloaded canonical block — pruned by then, so **from genesis** — and (b) re-executes **every** canonical
  block from there to the **highest** offloaded one, resident ones included (`for hgt in first..=last`). Scratch
  test `r3_open_restore_span` (`span.log`, `span-control.log`), 1,600 empty blocks, depth-1/2 shallow
  reorgs, two opens each:

  | journal | open (s) | rebuilds during open | restored at open |
  |---|--:|--:|--:|
  | no reorg | 6.40 / 6.34 | 0 | 0 |
  | one reorg at height 5 | 7.71 / 6.55 | 4 | 1 |
  | one reorg at height 1,500 | 14.06 / 11.77 | **1,499** | 1 |
  | reorgs at 5 and 1,500 | 12.17 / 11.00 | 4 (+ ≈ 1,496 re-executed by the restore loop) | 2 |

  A single depth-1 reorg near the tip roughly **doubles** the open (+5.4–7.7 s ≈ 1,500 re-executions); the
  study's own JM-REORG-OPEN (600 rebuilds for 902 records) is this effect, not only the deep reorg's two
  rebuilds. For a chain whose history includes any late reorg (normal for PoW), every open bound in §3.3/§5.4
  is ≈ 2× (devnet worst ≈ 2 × 7.45 h, ≈ 2 × 12.6 h with R3-2; network empty ≈ +1 replay of the re-execution
  term). Live import is not affected (it restores at each adoption).
- **Status.** REPRODUCED (scratch measurement; the rebuild counts are exact, the timings are under load ≈ 9–10).
- **Correction.** Add the term to §3 (Open(H) += re-execution of heights 1..h_last_reorg) and to the §3.3
  bounds and §8; propose the local fix (restore each adopted branch during replay, as live does, or keep the
  states/receipts needed so the final restore spans only offloaded blocks) — validity-neutral, needs a replay
  equivalence test.


### R3-1 — MEDIUM — devnet worst open "≈ 7.3 h + ≈ 8.5 min" double-counts TX_CAP and omits the modexp tx's own per-tx term
- **Where.** Study §3.2d "Devnet bounds" (L667–671), §3.3 table rows L713–714 and bullet L724–730, §5.4 L1002,
  §8; A13-DERIVATION §4a row "the worst devnet open at H_END"; `journal_projections.py` [13] L299–301.
- **Claim.** Open ≈ 7.3 h for `modexp_b8_e256_m8_even` on every block after the 549-height fill of the
  all-caps state, **plus** ≈ 8.5 min for "the ≤ 52 TX_CAP-bounded 380-transfer blocks".
- **Evidence.** TX_CAP is cumulative per branch (`chain.rs:351–354`). The 7.3 h chain already spends
  274 (fill) + 19,451 (one modexp tx per remaining height) = **19,725 of the 20,000** transactions; 275 remain,
  i.e. **0.72** of a 380-transfer block, not 52 (`indep.log` [B]). The script prints the two terms
  independently (`ntr = TX_CAP // transferTxs`) and the study adds them. Separately, the 7.3 h uses
  `modexp steady (small state) + empty-block state term (0.411 s)`, but the modexp tx itself also pays the
  per-USER-tx term of §3.2d (validate_snapshot + 2 clones, 25.9 ms on that state). Maximising
  Σ cost subject to H_END and TX_CAP (LP over transfer/modexp/empty blocks: each transfer block displaces
  ≈ 380 modexp txs, dObjective/dx = 9.8 − 0.94·380 < 0) gives all-modexp, ≈ **7.45 h** — numerically equal to
  "7.3 h + 8.5 min" by coincidence.
- **Status.** REPRODUCED (arithmetic on the committed evidence).
- **Correction.** Replace "≈ 7.3 h + ≈ 8.5 min of TX_CAP-bounded transfer blocks" by "≈ 7.45 h (all modexp,
  including each tx's 26 ms per-tx state term); TX_CAP leaves no room for 380-transfer blocks on that chain";
  in [13] compute the per-tx term and the TX_CAP room after the modexp blocks. (R3-2 raises the bound further.)

### R3-6 — MEDIUM — pool re-execution on **every** new head is a missed, gas-unbounded term (larger than the restore term the study analyses)
- **Where.** Study §5.3 "A third term under load" (L947–952) and the 2026-10-24 correction (L974–980);
  §8; A13 §4 item 7; `node.rs:387–446` (`on_new_head` → `rebuild_pool`), `node.rs:399–416` (`submit_tx`),
  lp4 `user_phase.rs:169–176` (`PoolMeter`), `node.rs:97–101` (POOL_MAX_TXS comment).
- **Claim (study).** The pool term is the re-execution of the *abandoned branch's* transactions in the
  adopting import (≤ 2,048 blocks' worth after 462fffc, ≈ 34 min worst).
- **Evidence.** `rebuild_pool` runs on every new head (not only reorgs) and re-executes (a) the detached
  txs and (b) **every transaction already in the pool**, each through `submit_tx` → `execute_for_profile`
  (full EVM run plus `validate_snapshot` and two snapshot clones). The pool is bounded by POOL_MAX_TXS = 4096
  and 32 MiB, and `PoolMeter` meters slots/accounts only — there is **no gas bound**, so 4096 pooled
  7.95 M-gas MODEXP txs (≈ 0.5 KB each, 2 MiB in all, each valid on the pending state) are admissible. Every
  new head then re-executes ≈ 4096 × ≈ 1 s ≈ **68 min** on the consensus thread (devnet T_blk 4 s); on the
  all-caps devnet state even plain transfers cost 4096 × ≈ 26 ms ≈ **106 s** per head. The node comment
  "about ten blocks of plain transfers: every new head re-executes the pool, so its length bounds that work"
  bounds the count, not the work. Scratch measurement `R3-POOL` (`scratch2.log`, code-filled
  state, load ≈ 10): 6 pooled `modexp_b8_e256_m8_even` txs (each `submit_tx` itself 1.31–1.57 s) make the
  import of one **empty** fallback block take **8.57 s instead of 0.23 s** (1.39 s per pooled tx) → ≈ 95 min per
  new head at POOL_MAX_TXS under this load (≈ 68 min at the idle-host 1.0 s).
- **Status.** REPRODUCED (scaled from 6 pooled txs; the 4096 figure is linear arithmetic).
- **Correction.** Add the term to §5.3/§8 ("every new head: ≤ POOL_MAX_TXS × per-tx re-execution cost,
  unbounded by gas") and to A13 item 7 with a figure; propose a local pool-gas bound (e.g. Σ gas_limit ≤ k·
  GAS_LIMIT) and/or incremental revalidation (only the senders the new block touches). Both are local
  mempool policy (not consensus).

### R3-7 — MEDIUM — 462fffc: the correction note's bound is right, but the change adds an O(state) cost per detached tx that the new branch already includes, and the old text is left stale
- **Where.** Study §5.3 correction (L974–980); stale text L949 (`detached_txs`, "REORG_RESTORE_MAX = 1024
  blocks per side"), L955–958 ("≤ 1024 blocks"), L971–972 ("≈ 17 min"); `derived.log` [7] / script L191
  ("+ <= 1024 detached txs"); `node.rs:421–446` (new `detached_blocks`, `rebuild_pool`); lp4 `native.rs:44–48`.
- **Claim.** "The walk … was bounded by 2·REORG_RESTORE_MAX = 2,048 steps over both sides together … now
  reaches the common ancestor, so the restore covers up to R_keep = 2,048 abandoned blocks … ≈ 34 min, not ≈ 17
  min … The measured figures above are unchanged."
- **Evidence.**
  1. *Bound* — correct. Old loop `for _ in 0..2*REORG_RESTORE_MAX` advanced one side per step; the new
     `while a!=b` walk reaches the LCA. Abandoned side ≤ r_keep: `import_with` refuses a new block with
     `h + r_keep ≤ head` (`node.rs:315`) and `retain` removes side blocks at `height + r_keep ≤ head`
     (`chain.rs:461`), so the fork point is ≥ head − R_keep. 2,048 × ≈ 1.0 s = 34.1 min (`indep.log` [D]).
     The old "≤ 1,024 per side" was already wrong for an asymmetric reorg (the old loop walked the higher
     side first, up to ≈ 2,047 abandoned blocks when the adopted side is short).
  2. *New cost the note omits.* The old code skipped detached txs whose hash was in the new branch (`kept`
     set). 462fffc removed that filter: every detached tx now goes through `submit_tx` → wire decode
     (signature recovery) → `execute_for_profile`, which runs `code_caps::validate_snapshot` over the whole
     pending state **before** `validate_nonce` (`native.rs:45–47`, then `execute_on_chain`). So a tx that the
     new branch includes (the common honest-reorg case) is rejected by nonce only after an O(accounts + code
     bytes) scan: ≈ 13.9 ms each on the devnet all-caps state (txstate.log `validateSnapshotMs`), bounded by
     the abandoned branch's txs ≤ TX_CAP = 20,000 → up to ≈ **4.6 min** added to the adopting stall that the
     old code did not pay. Scratch measurement of a nonce-rejected `submit_tx` on the code-filled state:
     **24.1 ms median (20.1 ms min)** per rejected tx (`scratch2.log`, load ≈ 10; `validate_snapshot` alone
     there ≈ 11.5 ms idle / 17–18 ms under load).
  3. *Memory* — the A13 item-7 note that the detached-tx copy (≤ TXBYTES_CAP) is gone is correct; bodies are
     read one at a time with `read_at`.
  4. *Measured figures* — the loaded deep reorg (50 abandoned 1-tx modexp blocks, none in the new branch)
     re-run after 462fffc: first side import 62.2 s (50 rebuilds), adopting import 131.4 s (50 rebuilds + 50
     restored + pool re-execution of 50 detached txs); adopting/first = 2.11 against the committed 2.03, all
     ≈ 1.3× slower under load. The structure is unchanged, as the note says (none of those txs is in the new
     branch, so the new rejected-tx cost does not show here).
- **Status.** Bound and arithmetic REPRODUCED (code reading + arithmetic); per-rejected-tx cost REPRODUCED
  (scratch); loaded reorg REPRODUCED (structure; absolute ≈ 1.3× under load; reopen not reached in 900 s).
- **Correction.** Keep the note; add the rejected-tx term ("each detached tx included by the new branch costs
  one `validate_snapshot` of the head state before its nonce rejection; ≤ TX_CAP of them"); or restore a cheap
  pre-filter (nonce < account nonce on the new head, checked before `execute_for_profile`) — local, validity
  neutral. Replace L949/L955–958/L971–972 instead of appending, rename `detached_txs` → `detached_blocks`,
  fix script [7]'s "+ <= 1024 detached txs", and add the old-pool term of R3-6.

### R3-8 — LOW — the network empty-block open "≈ 64–69 min (range)" understates the extrapolation uncertainty; the "b grows with H" steps are hashbrown resizes
- **Where.** Study §3.1 L300–303, §3.3 L705/L721, §5.4, §8; script [1].
- **Claim.** Range 64–69 min from the two LS fits and the highest two-point fits; "b grows with H".
- **Evidence.** The retain steps in `split.log` fall in 7,001–8,000 (1.48 → 2.17 ms) and 14,001–15,000 (3.87 →
  4.92 ms): exactly hashbrown's 7/8-load resize thresholds 7,168 (→ 8,192 buckets) and 14,336 (→ 16,384);
  `retain` iterates `self.blocks` three times per call (`chain.rs:459–466`), so its cost tracks bucket count,
  which jumps ×2 at each resize. All candidate models fit the six devnet points within 0.3 s but disagree at
  6.5× the measured range: quadratic LS 65.7 min, stage-A 69.2 min, bucket model 62.6 min, cubic
  (b linear in H) **100.6 min** (`indep.log` [A]). At H = 129,600 the table has 262,144 buckets (2.02/block)
  vs 1.64/block at 20,000.
- **Status.** REPRODUCED (fits); hashbrown attribution CODE-READING.
- **Correction.** State "≈ 63–100 min, model-dependent (6.5× extrapolation); ≥ ≈ 63 min", and name the
  bucket-doubling cause in §3.1.

### R3-9 — LOW — measurement variance: no re-run reproduced within ±5 %; the figures are host- and load-specific lower bounds
- **Where.** Study §3.2a L388–390 ("within ≈ ±5 %"), all absolute timing figures; A13 §4a "measured on this host".
- **Evidence.** §0.1: compute kinds +15–63 %, clone/scan terms 1.4–3.5×, with native precompile time unchanged
  (0.97×): the in-node cost is memory-bandwidth/cache sensitive. Load averages are not recorded in the
  committed logs ("under cpu.lock" serialises only this track's runs, not other sessions on the host).
- **Status.** REPRODUCED (structure), NOT-REPRODUCED (absolute values within ±5 %).
- **Correction.** Record the load average and CPU model/frequency in each evidence log; state the spread as
  "back-to-back on an idle host"; label every per-block figure a host-specific lower bound (contention raises
  it: the 380-transfer block is ≈ 13.0 s at the slots-end point under load ≈ 7, against 5.3 s committed).

### R3-10 — LOW — open transient "≈ 1.0× the journal for every kind" holds only when the steady set ≥ the journal; the parse step alone holds ≈ 2× the journal
- **Where.** Study §3.4 L756–775, §5.4 "open memory", §8 third bullet; `store.rs:131,145` and
  `Recovered` (L47, L172); A13 condition `open-replay-transient`.
- **Claim.** Transient ≈ 1.0× the journal for every kind; under the §4 vector the transient grows with the
  journal.
- **Evidence.** `store::open` reads the file into `bytes` (`read_to_end`), then `parse_v2` copies every
  payload (`payload.to_vec()`) while `bytes` is still alive: ≈ 2 J live at the end of the parse, before any
  chain state exists. So peak ≥ max(2 J, J + steady) and transient = peak − steady ≥ 2 J − steady. The three
  measured kinds all have steady ≥ 1.1 J (bodies or per-block metadata are resident), hence ≈ 1.0 J. A
  journal dominated by records whose blocks are offloaded or duplicates — exactly the §4 re-journal vector
  the study applies the figure to — has steady ≪ J and a transient approaching **2 J**.
- **Status.** CODE-READING (consistent with the body-max peak 2.14 J ≈ max(2.0, 1 + 1.12)).
- **Correction.** "transient ≈ max(1, 2 − steady/J) × J: ≈ 1× for canonical journals, up to ≈ 2× for
  re-journal-dominated ones". Add the obvious local mitigation to §7 (streaming parse: verify and replay one
  record at a time, drop `bytes`/payloads; O(MAX_RECORD) transient) — none of §7's options targets this
  obligation directly.

### R3-11 — LOW — `retain`'s orphan cascade is O(branch depth × resident blocks): a missed super-linear term
- **Where.** `chain.rs:461–465`; study §3.1 (only the per-call O(H) scans are modelled), §4.
- **Evidence.** When the base of a deep side branch reaches `height + r_keep ≤ head`, only that block is in
  `gone`; its descendants are found by `loop{ scan all self.blocks for a missing parent }`, one generation
  per pass. A 2,048-deep abandoned branch (every legal deep reorg leaves one) costs 2,048 full scans of the
  map in one `retain` call: ≈ 2,048 × 22,000 ≈ 4.5·10⁷ entry visits on the devnet, ≈ 2,048 × 131,700 ≈ 2.7·10⁸
  at network H (seconds per event, on live import and again on replay).
- **Status.** CODE-READING (not measured).
- **Correction.** Mention it in §3.1/§5 and in §7 option 5 (collect descendants with one children map, as the
  side-cap branch of `retain` already does).

### R3-12 — NOTE — §2.1 canonical worst case: arithmetic and spec inputs verified; two small omissions
- **Where.** Study §2.1 L216–229; script [9].
- **Evidence.** 0.371 + 8.000 + 1.979 + 0.961 + 0.005 = 11.316 GiB; inputs match governance.md:9–15 and
  storage.md:76; header bound 3,072 ≥ HDR_MAX 3,067. Omitted: the record RLP headers (≈ 14 B/record, ≈ 1.7 MB)
  and the body RLP item headers (≤ 3–4 B per tx beyond its raw bytes, ≤ ≈ 0.04 GiB at TX_CAP 10⁷), so ≈ 11.36
  GiB at most. The figure is canonical-only; honest (non-adversarial) side blocks — at most one per height
  (D118), each ≤ BODY_MAX — are not bounded by TXBYTES_CAP either and are treated only under §4.
- **Status.** REPRODUCED.
- **Correction.** "≈ 11.3–11.4 GiB canonical, excluding side records".

### R3-13 — NOTE — `side_max` is 2, not 8
- **Where.** Study L116–127 (profile table, "side_max (= `single_host().side_max` = 8)"), §7 option 6 L1081–1084.
- **Evidence.** `pocold::single_host_local()` overrides `side_max: SIDE_MAX = 2` (`lib.rs:56–59`, D-A13-BUDGET);
  the harness `admitted()` uses it, so every measurement ran with 2. Results are unaffected (each T block's
  parent is the previous, retained T block), but option 6's "counted as one of the side_max side states"
  leaves room for one other side state, not seven.
- **Status.** CODE-READING. **Correction.** Fix the table and option 6.

### R3-14 — NOTE — "code whose src equals fa97594's, so they apply to the head"
- **Where.** A13-DERIVATION §4a lead; study header ("every `src` file is fa97594's").
- **Evidence.** At HEAD `ec148cf`, src differs from fa97594 in node.rs/admission.rs (462fffc) and
  rpc.rs/net.rs/lib.rs/resources.rs (RpcGuard r3/r3c). The measured paths (store, chain, lp4 apply/native/
  code_caps) are unchanged, so the conclusion holds; the sentence does not.
- **Status.** REPRODUCED (`git diff --stat`). **Correction.** "the measured paths are unchanged at HEAD;
  462fffc changes only the detached-tx restore (R3-7)".

### R3-15 — NOTE — §4.2 pacing argument: EVICTED_MAX is a 1,024-entry FIFO, and an attacker's own imports run `retain`
- **Where.** Study §4.2 L827–853; `chain.rs:26,484–485`; `node.rs:310–311,347`.
- **Evidence.** The evicted-hash set forgets after 1,024 evictions, so with more than side_cap + 1,024 resident
  attacker side blocks in the window an unsolicited re-delivery is re-validated and re-journaled too (no
  session needed); and each such successful import runs `retain`, which can evict the next block, so the
  cycle is not necessarily "paced by the honest chain's own progress". The study already rates the vector as
  analysis and the conclusion (OPEN) stands.
- **Status.** CODE-READING. **Correction.** Qualify point 1 of §4.2 and the EVICTED_MAX row ("a 1,024-entry
  cache: blocks only re-delivery within the last 1,024 evictions").

---

## 2. Answers by question

**Q1 Measurements.** They measure what they say for the code paths named: `Chain::validate` is the
replay/rebuild unit (same `execute` + child_ctx + evidence path as `reexec`; PoW/signature checks are
µs-scale), JM-KIND subtracts the measured open intercept, medians absorb the cold first rep, the
fallback-vs-signed baseline difference is negligible, and the synthetic network-caps state is labelled. Gaps:
host variance (R3-9); the per-tx term is omitted where it applies to the single-tx worst kinds (R3-1); the
worst single-tx kind on a full state is the create-heavy one, which can be made state-neutral (R3-2); the
end-of-open restore span is not exercised by any measurement (R3-3).

**Q2 Projections.** All key projections re-derive exactly (§0.3) and the script output is byte-identical;
inputs match the spec. Wrong: the devnet TX_CAP decomposition (R3-1); the network-open range is too narrow
(R3-8). The deep-reorg table is arithmetically right, but it omits the pool term (R3-6) and the
reverting create-heavy per-block worst (R3-2). The open bounds also omit the end-of-open restore (R3-3),
which applies to replay only, not to live import.

**Q3 Unbounded resources.** The three obligations (disk growth with solicited re-journal, Θ(H²) open replay +
re-execution from genesis, open transient) and the 10.2 s 380-transfer block are real and correctly stated in
kind. Missed or mis-stated: R3-2 (create-heavy block repeatable every height), R3-3 (end-of-open restore span ≈
re-executes the chain between the first and last reorg), R3-6 (pool re-execution per head, gas-unbounded),
R3-7.2 (rejected detached txs pay validate_snapshot), R3-10 (transient up to 2×), R3-11 (orphan cascade).

**Q4 462fffc.** No measured figure changes (R3-7.4). The derived restore term doubles as the note says (≤ 2,048
abandoned blocks, ≈ 34 min worst), and the bound is right; but the note omits the new validate_snapshot cost of
already-included detached txs and leaves the superseded text in place (R3-7), and the larger pool term R3-6 is
not mentioned.

**Q5 Mitigations** (effect on: D = journal disk growth, R = open replay/re-execution from genesis incl. deep
reorg, M = open transient, B = per-block cost / 10.2 s block):

| # | mitigation | consensus validity? | D | R | M | B |
|---|---|---|---|---|---|---|
| 1 | headers-first sync, journal a block once its branch is chosen | no (P2P staging per approved network.md D38; fork choice unchanged if bodies are validated before adoption) | removes the re-journal of losing/re-fetched branches; canonical bound unchanged | none (re-fetch CPU amplification remains when bodies are fetched) | smaller journal → smaller transient | none |
| 2 | dedupe markers | no, if replay assigns the same new importSeq (D133 keys, consensus.md:205) | record → marker per re-fetch | none: the re-fetched block is still re-validated (genesis rebuild per cycle below K_hist stays) | marginal | none |
| 3 | compaction / local state checkpoint (signed snapshot) | no for a self-produced, integrity-checked snapshot; a snapshot taken from others is a trust-model change; storage.md:70 requires blocks/receipts/logs to the end, so canonical bytes cannot be dropped — only side/duplicate records | bounds the adversarial growth (canonical ≤ 11.3 GiB stays) | open = blocks since the snapshot (re-execution); the O(H) metadata index load remains; also removes R3-3 | only with streaming load | none |
| 4 | reverse diffs (spec mechanism, consensus.md §10, storage.md:70) | no (spec-approved local rollback) | +≈ 1.9 GiB (storage.md:79) | deep-reorg/re-fetch rebuild O(depth ≤ R_keep), double rebuild gone; open unaffected | none | none |
| 5 | O(window) retain + incremental canonical index | no if D133 output identical | none | removes b·H²/2 (88 s devnet, ≈ 61 min network empty) and R3-11 | none | none |
| 6 | keep the fork-point state | no (re-derivable state; +1 snapshot per such fork point in A13) | none | one genesis rebuild instead of two in a deep reorg; does **not** fix the re-fetch cycle (the side branch is evicted, so the pin goes) | none | none |
| 7 | incremental state root, O(dirty) scans, cached code hash/length, copy-on-write overlay | no if result-equivalent (same root, same Ok/Err and CODE_CAP decisions; hash verification moved to insertion) | none | divides every re-execution term on a grown state (open, rebuild, restore, pool) | +resident trie term | the 10.2 s block → ≈ its small-state 60–100 ms; removes R3-2's state dependence |
| 8 | MODEXP/precompile repricing | **yes** (gas schedule, validity) | none | per-gas worst ≈ 1 s → lower | none | compute kinds only |
| — | (missing) streaming open (R3-10) | no | none | none | transient → O(MAX_RECORD) | none |
| — | (missing) pool gas bound / incremental revalidation (R3-6) | no (local mempool policy) | none | removes the per-head pool term and caps the reorg restore term | none | none |

**Q6 Other.** R3-12…R3-15.
