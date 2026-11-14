# FN-3c (2026-11-14): C6 evidence at 10,000 members, and design risk 1 corrected

Status: **evidence record, node-local.** No consensus rule, consensus source or pinned vector changed in FN-3c; it adds
tests, a test-support genesis helper, one CI line and documents. It is the FN-3c step of the FN-3 plan (§5.13, §5.14,
§7) for the fresh network's C6 row (FRESH-NETWORK-DESIGN.md, which is kept outside this repository; the corrected risk-1
text is recorded below so that the repository holds it).

Sources:
- `prototypes/pocold/tests/fn3_scale.rs`: the A13 test, the genesis test and the `#[ignore]` growth and CPU harness.
- `prototypes/pocold/tests/support/ten_thousand.rs`: the fresh-network CP and the 10,000-identity genesis helper.
- `development/audit-2026-10-22/evidence/fn3c/`: the printed `FN3C-*` lines of the runs recorded here.
- `development/audit-2026-10-22/A13-DERIVATION.md`, section "FN-3c".

## 1. Design risk 1, corrected

The design's text (FRESH-NETWORK-DESIGN.md §4a, risk 1) read:

> Node memory at 10,000 members. Without FN-3, entry ranges take about 2.8 GiB of the 6 GiB `MEM_BUDGET`, and history
> grows about 4.5 GB per day. This is a launch blocker until C6 lands.

The 2.8 GiB counted the canonical slim depth only (64 B × 10,000 × 4,713 entries). The A13 bound counts the per-block
ranges term in every entry-equivalent it holds, so the real figure was more than twice that. Replacement text:

> **1. Node memory and history at 10,000 members (C6; closed by FN-3, evidence FN-3c).** Before FN-3 every block kept
> its nonce ranges: 64 B × M_max = 640,000 B in each of the 11,221 entry-equivalents A13 counts in store mode with
> eviction on (the window 32, the offloaded canonical metadata 4,681, the rebuild shadow 361, the side metadata 6,144,
> one full side block, two views), that is 7,181,440,000 B (6.69 GiB), more than the whole usable 5.25 GiB (7/8 of
> 6 GiB) by itself. A13 refused the node: 10,078,213,165 B at the C6-row CP, at least 12,643,385,293 B at the §1a CP.
> history.redb also stored 32 + 32 + 52·M_max = 520,064 B of ranges a block, about 4.49 GB a day.
> FN-3 (3a, 3b) stores each block's member set as a delta from its parent's (at most 24 ids on this CP) with a
> snapshot every 256 heights, and recomputes a stored block's owners from it. A13 now admits the node: 2,894,088,421 B
> (2.70 GiB) at the C6-row CP and 5,461,945,293 B (5.09 GiB) at the §1a CP, 175,199,283 B (167 MiB) inside the usable
> 5.25 GiB. The membership part of history.redb measures 8.0 MB a day idle and 12.1 MB a day if every block changed 24
> ids. Memory and history are no longer launch blockers. Two items remain for M1: the §1a CP's A13 margin is thin, and
> a cold child_ctx takes about 110 ms at 10,000 members in store mode (see §3).

"C6-row CP": the generated continuous network's CP (builder defaults: CP schema v1, nonceMode 0, devnet caps) with the C6
row's fields set (M_max 10,000, E_win 360, D 4, E_max 4, R_react 4, REG_OPS 16, REG_RECORDS_MAX 24,576, SYS_SLOT_MAX
147,456). FN-3a and FN-3b recorded their A13 figures at this CP. "§1a CP": every A13 input at the value in the design's
§1a table (CP schema v2, nonceMode 1, T_blk 10, CODE_CAP 2^30, SLOT_CAP 4,000,000, ACCT_CAP 1,000,000, Z_close 256,
C_react 380, ...; `ten_thousand::fresh_config`). Only the subsidy S0 (2 PCOL from the builder, not 4) and target_g (a
fixture difficulty) differ from the design, and neither enters A13. Both CPs are built by hand because no committed
fresh-network profile (deploy/mainnet/v3-profile.json, FN-4) is on this base. FN-4 should re-run the A13 test against
that profile once it lands.

## 2. C6 verification results

| C6 criterion | Result | Where |
|---|---|---|
| Identical verdicts on every v3 test | Pass. The FN-3a/3b oracle runs in every chain-running suite: import, every reader, and the end-of-test cold sweep against the capture. It found 0 mismatches and 0 member faults; this branch re-ran the suites (§4). | A13-DERIVATION.md FN-3b (suite list); tests/support/members.rs |
| Kind-3 and type-2 evidence across slim/unslim, restart and reorg | Pass (tests/fn3_members.rs §5.4-§5.9). At 10,000 members the harness also included 4 type-2 punishments a block for 64 blocks, against ancestors up to about 200 blocks deep (offloaded below the 32-block window). Their owners were recomputed from history.redb: 121 disk set reads, 0 faults. | tests/fn3_members.rs; tests/fn3_scale.rs harness |
| A13 store mode at M_max 10,000, E_win 360, REG_RECORDS_MAX 24,576 | Pass. Admitted at both CPs (§1); the totals are pinned on Linux x86-64. The pre-FN-3 ranges term alone, 7,181,440,000 B, is asserted to exceed the usable 5,637,144,576 B. | `a13_admits_the_fresh_network_cp_at_ten_thousand_members` |
| history.redb growth a day ≪ 100 MB | Pass. The membership tables take 8.0 MB a day idle, 8.4 MB a day at the harness's churn mix, and 12.1 MB a day if every block changed 24 ids (§3). The harness asserts the measured bytes stay below 100 MB a day. | `growth_and_cpu_at_ten_thousand_members` |
| One-pass membership and cache; `owner()` binary search (FN-1) | Done in FN-1. Measured again here at 10,000 members in store mode (§3). | tests/fn1_membership.rs |

## 3. M1 measurements at 10,000 members (oracle off)

The run: `cargo +1.99.1 test --release --locked --manifest-path prototypes/pocold/Cargo.toml --test fn3_scale -- --ignored --nocapture --test-threads=1`.
It ran on a 4-vCPU Intel Xeon 2.1 GHz Linux VM with 15 GB, on one test thread, in about 26 minutes.

Each run uses a store-mode node with eviction on, the default window and the oracle off (its default), and imports
600 blocks: three snapshots, at h = 1, 257 and 513.
- **10k-idle:** 10,000 genesis members and no transactions.
- **10k-churn:** the same genesis. Blocks 1-3 fund 768 members. Blocks 201-264 each carry 12 deregistrations and 4
  registrations (REG_OPS 16), 4 reactivations from the 33rd block on (R_react), and up to 4 type-2 punishments of
  earlier winners (E_max). This gives |Δ| = 20 in 33 blocks and |Δ| = 24, the CP's delta_max, in 26 blocks. 9,374
  members remain at the end.
- **4-idle:** a 4-member chain on the same CP, as the baseline.

Before each block is imported, its parent's child_ctx is timed with FN-1's cache cleared (cold) and then again
(warm). The block is then validated and imported, each with the cache cleared, as on a node that did not build it.

| Measure | 10k-idle | 10k-churn | 4-idle |
|---|---|---|---|
| member record a block (key included) | 144 B | 189.7 B mean (144 + 20·|Δ|) | 144 B |
| snapshot row | 200,064 B | 192,051 B mean (fewer members) | 144 B |
| membership tables, steady state a day (8,640 records, 33.75 snapshots) | **7,996,320 B** | 8,390,880 B | — |
| the same with every block at |Δ| = 24 | **12,143,520 B** | | |
| membership tables a day as measured over 600 blocks (3 snapshots in 600 blocks overweights snapshots) | 9,886,925 B | 9,935,309 B | 1,250,381 B |
| redb stored bytes, 10k-idle less 4-idle, over 600 blocks | 599,760 B (8.64 MB a day) | | |
| history.redb file length at the end (redb preallocates; it never moved idle) | 1,589,248 B | 3,694,592 B (1,920 transactions' receipts) | 1,589,248 B |
| child_ctx cold p50 / p99 / max | 115.4 / **235.3** / 330.9 ms | 109.9 / **184.4** / 370.3 ms | 0.15 / 0.34 ms |
| of which the one-pass membership p50 / p99 | 109.8 / 222.3 ms | 105.0 / 170.0 ms | 0.04 / 0.10 ms |
| child_ctx warm (FN-1 cache hit) p50 / p99 | 0.108 / 0.200 ms | 0.108 / 0.202 ms | 0.012 / 0.081 ms |
| validate (cache cleared) p50 / p99 | 295 / 534 ms | 305 / 533 ms | 13.4 / 19.6 ms |
| import (validate, journal, state.redb, history.redb, checkpoints) p50 / p99 / max | 446 / 679 / 956 ms | 436 / 656 / 746 ms | 15.4 / 25.7 ms |
| reconstruction cold (all caches cleared, 120 heights: walk to the snapshot, from history.redb) p50 / p99 | 8.5 / 11.4 ms | 8.8 / 16.3 ms | 0.3 / 0.8 ms |
| reconstruction warm | < 0.002 ms | < 0.002 ms | |
| peak RSS of the run (VmHWM reset at the node's open) | 197 MiB | 191 MiB | 167 MiB |
| A13 model at the end (files observed) | 5,461,945,681 B | 5,461,946,195 B | |

An earlier run of the same harness, without the validate and membership timings, gave the same picture: child_ctx
cold p99 171.7 and 215.6 ms, import p99 593 and 700 ms, reconstruction cold p99 8.3 and 9.4 ms.

Against M1's criteria:
- **History growth a day within C6's target: met.** 8.0-12.1 MB a day, against 4.49 GB before FN-3.
- **RSS within the A13 bound: met.** About 0.2 GB measured against a 5.46 GB bound. A13 is a worst-case model; the
  harness has no peers, no RPC and no pool.
- **child_ctx p99 < 100 ms: met warm, not met cold.** A cold child_ctx (one per new parent, the FN-1 cache's miss) at
  10,000 members in store mode takes about 110 ms at p50 and 184-235 ms at p99 on this VM. Almost all of it (about 95 %)
  is the one-pass membership read of the parent's ActList and 10,000 records through state.redb. FN-1's in-memory
  benchmark (tests/fn1_membership.rs) did not see this cost. A node takes the miss once per block it validates; every
  later context of that parent is a hit (0.2 ms).
  - The owner (or the main session) should decide whether M1's 100 ms is meant for the cached path, which meets it, or
    for the cold one.
  - If it is the cold one, a node-local fix would keep the membership across blocks: derive the child's membership from
    the parent's cached one and the block's ActList changes, instead of re-reading every record. That is not FN-3c's
    scope.
- **Validation and import at 10,000 members** take 0.3-0.45 s at p50 and up to 0.68 s at p99 here, against 13-15 ms at
  4 members. The cold child_ctx accounts for about 110 ms of that. The remaining roughly 200-300 ms that grows with M is
  not attributed here. The likely places are the system programs' reads at a full set and DrawWeights. It matters for
  T3 (t_exec ≤ 1 s, validate+commit p99) on the founders' machines, and is left to M1/T3 in the rehearsal (§5).
- **Gateway reward computation < 1 s a block; T4 and T5 unchanged: not measured here** (§5).

## 4. Test results on this branch

The suites were run with `cargo +1.99.1 test --release --locked`, on Linux.
- `--lib` and every other pocold test target with default features: all pass.
  - lib 266
  - admission 7, checkpoint_source 2, cli 16, consensus_guard 21, decoder_reference 3, discovery 9
  - earnin 1, eviction_credit 1, eviction_memory 1, fee_pool 17
  - fn1_caps 8, fn1_membership 10, fn3_members 15, fn3_scale 3 (1 ignored: the harness)
  - fork_configuration 1, full_nonce 6, journal_measure 2 (22 ignored, as before), launch_delay_asert 5
  - log_cap 0 and log_cap_scope 0 (they need `--features caps-test`), memory_bounds 9, memory_window 1, mining_work 4
  - net 67, network_budget 15, open_transient 1, outbound_threads 1, rejoin 2, relay 9, secp_fixture 1, site 6
  - state_store 14 (1 ignored, as before), store 13, unhex_fix 3, unhex_invitation_panic 2
  - v3_earnin 9, v3_execution 39, v3_fork_choice 16, v3_late_shares 8, v3_membership 9, v3_reward_gateway 1
  - v3_scaffold 11, wire_mutation 1
- `consensus` and `caps` on one thread: all pass (consensus 165, 10 ignored as before; caps 11).
- `consensus_guard::the_tree_matches_the_consensus_lock` passes: no consensus source changed.
- The harness passed in 1,549 s; its output is the one recorded in §3.

## 5. Deferred, and why

- **M1's own registration rehearsal** (about 2,500 blocks, about 7 h at T_blk 10 s, registering up to 10,000
  identities): the harness seeds the identities at genesis instead, so it can run on one machine in about 26 minutes.
  Registration cost at a full set belongs to M2 and M3 (gas and the capacity path) on the rehearsal network.
- **Gateway reward computation, T4, T5:** these need the reward gateway and live multi-host mining, which this harness
  does not run (Phase 5 / rehearsal).
- **Cold child_ctx above 100 ms:** §3. This needs an owner reading of M1, and possibly a node-local incremental
  membership (an FN-1 follow-up).
- **The M-dependent validation cost** beyond child_ctx: §3, for T3.
- **The §1a CP's A13 margin** (167 MiB at empty files): it is admitted while state.redb stays below 1,317.6 GiB (with
  history.redb empty) or history.redb below 668.3 GiB (with state.redb empty). The largest terms are the CODE_CAP 2^30
  code set (1.21 GB) and the execution's memory classes (1.05 GB). The design's own fallback (CODE_CAP 2^29) remains
  available if FN-4's profile adds terms.
- **A dedicated CI job for the harness:** not added. At about 26 minutes and a 100-second admission measurement it is
  not cheap. It is documented with the command above. The three default tests (about 2 s) run in
  `full-nonce-upgrade.yml` and in the Windows verification's pocold group.
