# Memory bound recount and R14/R19 workload gap (2026-10-27)

Branch `claude/memory-r14-followup` (palcertcom/pocol#2, draft), from the merged `13f1eac`. Commits:
- `892dca6`: the four A13 reductions and their tests;
- `2839f2a`: the store-mode import fix, the R14/R19 measurement and its documents;
- `8d80354`: the `ROW` test (kills N11) and the mutation evidence;
- `69d5794`: the P6 premise test and the proof below;
- `073923f`: the e2e_integrated race fix (test only).

The other commits add documents and evidence only.

Host: 4 vCPU KVM, ext4 on virtio; rustc 1.99.1; release builds. Nothing else ran during the timing runs.
The mutation runs used a separate worktree.

## Files

| file | what | result |
|---|---|---|
| a13-models.log | `network_budget` on `2839f2a`'s tree | network store **5.086 GiB, gate 5.81 GiB**; whole-state 24.98 GiB; devnet store 2.44, whole-state 2.99; state.redb file 2.20 TiB (563 regions); 5/5 |
| lp4-release.log | lp4 release suite, `--no-fail-fast`, on the tree committed as `892dca6` | 200 passed, 0 failed |
| pocold-release.log | pocold release suite on that tree before the pin was updated | 234 passed, **1 failed**: `a13_bound_breakdown`, the pinned devnet model total, which the boxed entries changed (3,287,955,262 → 3,233,597,728 B). The pin is updated in `892dca6`, and the test then passed |
| pocold-release-2.log | pocold release suite on `2839f2a` (the restore fix included) | **236 passed, 0 failed, 24 ignored** |
| r14-gap-rootcheck-on.log | `jm_r14_workload_gap` on `892dca6` with pocold's test-only root checks on (`POCOL_ROOT_CHECK` unset) | kept as the record of the correction: every block also ran on a whole-state reference path; 1,246 s |
| r14-gap-rootcheck-off.log | the same with `POCOL_ROOT_CHECK=0`, as in the node binary | 136 s; the table in A13-DERIVATION §3b |
| r14-gap-after-fix.log | the same on `2839f2a` (the restore fix) | 113 s; store-mode imports equal their validation |
| d2-gate.log | admission of the pocold devnet and a P11-shaped devnet at t_exec = 1 and 2 s (a throwaway test, not committed) | at 2 s: R2, R3 and R7 fail; with Δ_rel and Γ raised, R3 and R7 still fail; admitted only with dFbWait and D_att (CP) raised |
| mutants.log | mutation controls (the runner is kept outside the repository) | 24 of 25 killed, 1 equivalent; see below |
| accept-8d80354/ | e2e in both modes on `8d80354`: devnet, site, browser, integrated ×2, boot timing | all PASS except integrated whole-state run 1 (FAIL 1 of 20, a test race); see below |
| witnesses-8d80354.log | the security witnesses on `8d80354` (their programs are kept outside the repository) | 9 NOT_REPRODUCED, controls OK |
| integrated-race/ | the failing integrated run's log, the three node logs, the analysis | a race in the test, fixed in `073923f` |
| modexp/ | the MODEXP fast paths (`b46658a`, `0c2ad75`), the shape sweep, suites and mutants per commit (modexp/README.md) | every measured kind under t_exec = 1 s after `0c2ad75`; see A13-DERIVATION §3b-2 |

## What the measurement shows (A13-DERIVATION §3b)

All times are with the checks off, in ms (validate median / import).

| kind | whole-state, near-cap state | store, near-cap state, after the fix |
|---|---:|---:|
| GAS_LIMIT compute loop (the calibration's workload) | 24 / 32 | 43 / 53 |
| 380 transfers | 72 / 71 | 113 / 100 |
| MODEXP, even modulus (two shapes) | 1,265–1,325 / 1,260–1,359 | 1,355–1,399 / 1,524–1,591 |
| BN pairing | 304 / 286 | 383 / 330 |
| keccak over 32 KiB | 207 / 193 | 204 / 321 |
| cold CALLs | 65 / 83 | 343 / 318 |

- Calibration: 9–10 ms per phase in memory, 12–19 ms through the store. t_exec is 1,000 ms.
- **Only MODEXP with an even modulus exceeds t_exec.** That is D-2. (Update: after the two MODEXP fast paths,
  every kind measured is under t_exec on this host; modexp/README.md and A13-DERIVATION §3b-2.)
- **Store-mode import before the fix:** MODEXP 2,535–2,609 ms and transfers 255 ms, because every import
  re-executed the head.

## Mutation controls (mutants.log)

25 mutants of the new code, each applied to a separate worktree of `2839f2a`, run against the tests named
in the log, then restored. Baseline unmutated: 7 + 1 + 5 tests pass.

| group | mutants | result |
|---|---|---|
| view.rs, the pool layer without code copies (V1–V5) | 5 | 5 killed |
| node_trie.rs, the per-node constants and the batch bounds (N1–N11) | 11 | 10 killed in the first run; N11 (`ROW` 36 → 32) survived, see below |
| resources.rs, the A13 terms (P1–P8) | 8 | 6 killed in the first run; P6 equivalent; P8 survived, see below |
| chain.rs, the store-mode import fix (F1) | 1 | killed |

Result: **24 of 25 killed, 1 equivalent (P6)**, counting the two reruns below.

- **P8** (the map slot of a block entry counted ×3 instead of ×4) survived `network_budget`, whose network
  figures do not pin the devnet total. Rerun against `consensus::a13_bound_breakdown`, which pins it: killed.
- **N11** survived both node_trie runs, also after `stored()` in node_trie/tests.rs was made to count a row's
  32 + 4 bytes itself: the bounds the tests check have slack, so a row 4 bytes short still fits. Killed by
  the new `state_store::a13_row_is_what_state_redb_keeps_per_node`, which checks `ROW` plus each node's
  encoding against the rows state.redb keeps (its `stored_bytes`), exactly.
- **P6** removes the `min` from `chain_versions_bytes`; it is equivalent (next section).

### Why P6 is equivalent

The mutant turns `Some(mul(n,whole)?.min(add(&[whole,mul(n-1,block)?])?))` into
`Some(add(&[whole,mul(n-1,block)?])?)`, with `whole = version_bytes(ka,ks)` and
`block = block_bytes(wa,ws,ka,ks)`. Both return 0 at n = 0. Otherwise the two can differ in only two ways,
and neither can happen:

1. **`n·whole` is the smaller argument.** Every term of `block` is at most the same term of `whole`:

   | term | `block_bytes(wa,ws,ka,ks)` | `version_bytes(ka,ks)` |
   |---|---|---|
   | account leaves | `min(wa,ka)·(191+36)` | `ka·(191+36)` |
   | slot leaves | `min(ws,ks)·(98+36)` | `ks·(98+36)` |
   | branches | `min(64·(wa+ws), ka+ks)·(70+36)` | `(ka+ks)·(70+36)` |
   | children | `min(16·branches, 2·(ka+ks))·66` | `2·(ka+ks)·66` |

   So `block ≤ whole`, and `whole + (n−1)·block ≤ whole + (n−1)·whole = n·whole`. The `min` always returns
   its second argument (both are equal when a block changes every key of the child).
2. **`mul(n,whole)` overflows**, so the original returns `None` where the mutant returns `Some`.
   - ACCT_CAP and SLOT_CAP are u64 and SYS_SLOT_MAX is u32, so `ka + ks < 2^66`.
   - A key costs at most 227 + 106 + 132 = 465 < 2^9 bytes, so `whole < 2^75`.
   - n is at most K_hist + 1 ≤ 2^32 (K_hist is a u32).
   - So `n·whole < 2^107`, far from u128's 2^128, and the other products are smaller.

The outputs are therefore the same for every input the types allow, and no test can kill the mutant. The
`min` is kept as a clamp. `node_trie::tests::a_blocks_addition_is_at_most_a_whole_version` pins the premise
of point 1: 200,000 random and edge counts up to u64::MAX + 2^32, with n = 2^32. A change to `block_bytes`
that broke the premise would fail that test, so the clamp cannot start to bind unnoticed.
Control (mutants-p6-premise.log): Q1, which drops the child cap from `batch_nodes`' branch count, so that
`block_bytes` can exceed `version_bytes`, is killed by that test.

## Acceptance on 8d80354 (accept-8d80354/, witnesses-8d80354.log)

Run in a separate worktree, on local nodes, local RPC and a test Chrome profile only. `HEAD` records the
prototypes tree `3a037212` and the two usual symlinks (target, node_modules). The node code is `2839f2a`'s;
`8d80354` adds tests only.

| step | whole-state mode | store mode (`--state-store`) |
|---|---|---|
| pocold-bins, ext-build | built | (same binaries) |
| jm-boot-timing (`jm_r14_boot_timing_by_mode`, 10 runs) | in memory 8–18 ms per phase | through the store 11–24 ms; t_exec 1,000 ms |
| e2e_devnet | **PASS 44/44** | **PASS 44/44** |
| e2e_site | **PASS 21/21** | **PASS 21/21** |
| e2e_browser (Chrome) | **PASS 80/80** | **PASS 80/80** |
| e2e_integrated | run 1 **FAIL 1 of 20**; run 2 **PASS 20/20** | **PASS 20/20**, two runs |

- **Security witnesses** (rust and live parts): A01, A02 (R9, R13, SYS_SLOT_MAX), A03, A04, A05, A10 and A11
  are **NOT_REPRODUCED**, and every positive control is OK.
- The extension witnesses (A06–A09, A12, A14) were not rerun. The extension's source is unchanged since
  `8584283`, where they ran: `git diff 8584283 073923f` outside its tests is empty.
- **The integrated failure** is a race in the test, not a node defect (integrated-race/README.md).
  - After the heal, c's returned transactions, which carried the v2 publication, were included at heights
    76–77.
  - That happened between the Chrome reads, which showed v1 on all three nodes, and the version read.
  - The fixed test (`073923f`) was rerun three times per mode (accept-int-073923f/).
- In every integrated run, node c offered all 7 transactions it returned after the heal, and none was left
  unoffered.
- **The boot calibration's pass does not close the R14/R19 gap.** It times the cheapest block per gas. A
  full block of MODEXP with an even modulus is still 1.2–1.6 s on this host (A13-DERIVATION §3b, D-2,
  OPEN-DECISIONS rows 1–2).

## Not established here

- Acceptance.
- Measured RSS at network scale.
- The network profile booting.
- Any adopted decision: D-2 and the calibration workload are rows 1–2 of proposals/OPEN-DECISIONS.md.
