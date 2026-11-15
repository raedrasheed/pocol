# FN-3c (2026-11-14): C6 evidence at 10,000 members, and design risk 1 corrected

Status: **evidence record, node-local.** No consensus rule, consensus source or pinned vector changed in FN-3c; it adds
tests, a test-support genesis helper, one CI line and documents. FN-1b (§6, added the same day) derives a new parent's
membership inputs across blocks; it is node-local too, and §6 records its before/after run of the same harness. It is the FN-3c step of the FN-3 plan (§5.13, §5.14,
§7) for the fresh network's C6 row (FRESH-NETWORK-DESIGN.md, which is kept outside this repository; the corrected risk-1
text is recorded below so that the repository holds it).

Sources:
- `prototypes/pocold/tests/fn3_scale.rs`: the A13 test, the genesis test and the `#[ignore]` growth and CPU harness.
- `prototypes/pocold/tests/support/ten_thousand.rs`: the fresh-network CP and the 10,000-identity genesis helper.
- `development/audit-2026-10-22/evidence/fn3c/`: the printed `FN3C-*` lines of the runs recorded here.
- `development/audit-2026-10-22/evidence/fn1b/`: the same harness's lines before and after FN-1b (§6).
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
> 6 GiB) by itself. A13 refused the node: 10,035,259,610 B (9.35 GiB) at the C6-row CP before FN-3 (10,078,213,165 B
> after FN-3a, before FN-3b), and about 12.60 GB at the §1a CP (an estimate: today's total less FN-3's member terms,
> plus the ranges term).
> history.redb also stored 32 + 32 + 52·M_max = 520,064 B of ranges a block, about 4.49 GB a day.
> FN-3 (3a, 3b) stores each block's member set as a delta from its parent's (at most 24 ids on this CP) with a
> snapshot every 256 heights, and recomputes a stored block's owners from it. A13 now admits the node: 2,894,088,421 B
> (2.70 GiB) at the C6-row CP and 5,461,945,293 B (5.09 GiB) at the §1a CP, 175,199,283 B (167 MiB) inside the usable
> 5.25 GiB. The membership part of history.redb projects to 8.0 MB a day idle (9.9 MB a day as measured over 600
> blocks, which overweights snapshots) and 12.1 MB a day if every block changed 24 ids. Memory and history are no
> longer launch blockers. Two items remained for M1: the §1a CP's A13 margin is thin, and a cold child_ctx took about
> 105-115 ms at p50 at 10,000 members in store mode (see §3). FN-1b closed the second: a new parent's membership is
> now derived from its own parent's and the block's registry writes, and the cold child_ctx takes about 5 ms at p50 and
> 9-11 ms at p99 (§6). FN-1b's cache adds 23,821,796 B to A13 at 10,000 members: 2,917,910,217 B at the C6-row CP and
> 5,485,767,089 B at the §1a CP, 151,377,487 B (144 MiB) inside the usable 5.25 GiB.

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
| Kind-3 and type-2 evidence across slim/unslim, restart and reorg | Pass (tests/fn3_members.rs §5.4-§5.9). At 10,000 members the churn run also included 242 type-2 punishments of earlier winners in its 64 churn blocks (up to E_max = 4 a block: 4 in 59 blocks, 3 in one, 1 in three, 0 in one), against ancestors up to about 200 blocks deep (offloaded below the 32-block window). The import phase's member counters, taken before the reconstruction loop (both idle runs show 0 there), give the evidence path: 236 set reconstructions and 415 record reads from history.redb, 1 disk set read (the other walks ended at a set still in the sets cache), 0 faults. The disk snapshot-set path at 10,000 members is exercised by the proposer-pool case (§3): 64 cold reconstructions, each ending at a snapshot set read from history.redb, 0 faults. | tests/fn3_members.rs; tests/fn3_scale.rs harness |
| A13 store mode at M_max 10,000, E_win 360, REG_RECORDS_MAX 24,576 | Pass. Admitted at both CPs (§1); the totals are pinned on Linux x86-64. The pre-FN-3 ranges term alone, 7,181,440,000 B, is asserted to exceed the usable 5,637,144,576 B. | `a13_admits_the_fresh_network_cp_at_ten_thousand_members` |
| history.redb growth a day ≪ 100 MB | Pass. The membership tables project to 8.0 MB a day idle, 8.4 MB a day at the harness's churn mix, and 12.1 MB a day if every block changed 24 ids; over the 600 blocks measured (3 snapshots, which overweights them) they took 9.9 MB a day idle and churning (§3). The harness asserts the measured bytes stay below 100 MB a day. | `growth_and_cpu_at_ten_thousand_members` |
| One-pass membership and cache; `owner()` binary search (FN-1) | Done in FN-1. Measured again here at 10,000 members in store mode (§3). | tests/fn1_membership.rs |

## 3. M1 measurements at 10,000 members (oracle off)

The run: `cargo +1.99.1 test --release --locked --manifest-path prototypes/pocold/Cargo.toml --test fn3_scale -- --ignored --nocapture --test-threads=1`.
It ran on a 4-vCPU Intel Xeon 2.1 GHz Linux VM with 15 GB, on one test thread, in about 25 minutes.

Each run uses a store-mode node with eviction on, the default window and the oracle off (its default), and imports
600 blocks: three snapshots, at h = 1, 257 and 513. The three runs share one test process, in this order:
- **10k-churn:** 10,000 genesis members. Blocks 1-3 fund 768 members. Blocks 201-264 each carry 12 deregistrations and
  4 registrations (REG_OPS 16), 4 reactivations from the 33rd block on (R_react), and up to 4 type-2 punishments of
  earlier winners (E_max): 242 in all (4 in 59 blocks, 3 in one, 1 in three, 0 in one; a winner already punished,
  gone or past E_win is skipped). The member records' |Δ| over the run: 20 in 33 blocks, 21 in 3, 23 in 1 and 24, the
  CP's delta_max, in 26 (block 257, a snapshot, is the 64th churn block). 9,374 members remain at the end.
- **10k-idle:** the same genesis and no transactions.
- **4-idle:** a 4-member chain on the same CP, as the baseline.

Before each block is imported, its parent's child_ctx is timed with FN-1's cache cleared (cold) and then again
(warm). h_full's member record (FN-3 §1.4) is then built by the harness from the same inputs as h_full builds it
(S_b from the context, the parent's set and digest) and timed, with the row's encoding for history.redb; after the
import it is checked to equal the record the chain built. The block is validated and imported, each with the cache
cleared, as on a node that did not build it. After the 600 blocks, every 5th height's assignment is reconstructed
with every member cache cleared. The idle runs then fill the proposer's pool (FN-3 §5.12) at the head.

| Measure | 10k-idle | 10k-churn | 4-idle |
|---|---|---|---|
| member record a block (key included) | 144 B | 189.7 B mean (144 + 20·|Δ|) | 144 B |
| snapshot row | 200,064 B | 192,051 B mean (fewer members) | 144 B |
| membership tables, steady state a day (8,640 records, 33.75 snapshots): a projection | **7,996,320 B** | 8,390,880 B | — |
| the same with every block at |Δ| = 24: a projection | **12,143,520 B** | | |
| membership tables a day as measured over 600 blocks (3 snapshots in 600 blocks overweights snapshots) | 9,886,925 B | 9,935,309 B | 1,250,381 B |
| redb stored bytes, 10k-idle less 4-idle, over 600 blocks | 599,760 B (8.64 MB a day) | | |
| history.redb file length at the end (redb preallocates; it never moved idle) | 1,589,248 B | 3,694,592 B (1,920 transactions' receipts) | 1,589,248 B |
| child_ctx cold p50 / p99 / max | 106.1 / **171.3** / 287.6 ms | 105.5 / **188.3** / 267.2 ms | 0.10 / 0.22 ms |
| of which the one-pass membership p50 / p99 | 103.6 / 186.5 ms | 105.8 / 173.0 ms | 0.03 / 0.08 ms |
| child_ctx warm (FN-1 cache hit) p50 / p99 | 0.106 / 0.164 ms | 0.108 / 0.173 ms | 0.012 / 0.047 ms |
| h_full's member record, a delta block (the set's Arc, the diff, the digest step) p50 / p99 / max | 0.095 / 0.160 / 0.262 ms | 0.097 / 0.154 / 0.260 ms | 0.001 / 0.001 ms |
| h_full's member record, a snapshot block (3 of them; the digest of 10,000 ids) p50 / max | 3.74 / 4.30 ms | 3.64 / 3.76 ms | 0.002 ms |
| the record's row encoding (and the set's at a snapshot) p50 / p99 | 0.0005 / 0.0013 ms | 0.0005 / 0.0011 ms | 0.0002 / 0.0004 ms |
| validate (cache cleared) p50 / p99 | 280 / 421 ms | 298 / 493 ms | 10.5 / 16.6 ms |
| import (validate, journal, state.redb, history.redb, checkpoints) p50 / p99 / max | 402 / 579 / 987 ms | 427 / 625 / 2,051 ms | 12.5 / 19.2 / 30.9 ms |
| reconstruction cold (all caches cleared, 120 heights: walk to the snapshot, from history.redb) p50 / p99 | 6.5 / 9.4 ms | 7.1 / 9.5 ms | 0.16 / 0.39 ms |
| reconstruction warm | < 0.001 ms | < 0.001 ms | < 0.001 ms |
| member counters of the import phase (before the reconstruction loop): reconstructions / record reads / disk set reads / faults | 0 / 0 / 0 / 0 | 236 / 415 / 1 / 0 (the type-2 evidence) | 0 / 0 / 0 / 0 |
| member counters of the reconstruction loop (120 heights, caches cleared before each) | 120 / 13,967 / 120 / 0 | 120 / 13,967 / 120 / 0 | 120 / 13,967 / 120 / 0 |
| proposer's pool, 64 type-2 items (heights 283-598, every 5th; 58 offloaded): signed template cold, empty pool / full pool | 331 / 803 ms (**+472 ms**) | | 5.9 / 36.0 ms (+30 ms) |
| the same warm (the owner memo) | 141 / 160 ms (**+18 ms**) | | 5.5 / 24.2 ms (+19 ms) |
| the full pool's cold template: reconstructions / record reads / disk set reads / faults; the warm one's | 64 / 7,095 / 64 / 0; 0 / 0 / 0 / 0 (78 owner-memo hits) | | 64 / 7,095 / 64 / 0; 0 (86 hits) |
| process peak RSS (VmHWM, reset at the node's open to the RSS then, not to zero) | 194.0 MiB | 190.8 MiB | 177.7 MiB (residue only, see below) |
| process RSS at the node's open, and the peak above it | 191.0 MiB, +3.0 MiB | 110.8 MiB, +80.0 MiB | 177.7 MiB, +0 |
| A13 model at the end (files observed) | 5,461,945,681 B | 5,461,946,195 B | |

The RSS figures are the test process's, not a node's alone. At each node's open the process still holds the 10,000
genesis, the admission measurement's leftovers and what earlier runs left resident. The churn run, first, rose 80 MiB
above the process's 111 MiB at its open. The later runs reuse memory the earlier ones freed, so their rise above the
open (3 MiB idle, 0 for 4-idle) understates their own use, and the 4-idle peak is only the earlier runs' residue: it
says nothing about a 4-member node. The 10k peaks are upper bounds on the node's own RSS.

The previous run of this harness (at 93a5857, without the record, counter-phase and pool measurements) gave the same
picture: child_ctx cold p99 235.3 and 184.4 ms (idle, churn), import p99 679 and 656 ms, reconstruction cold p99 11.4
and 16.3 ms. The run before it gave child_ctx cold p99 171.7 and 215.6 ms and import p99 593 and 700 ms.

Against M1's criteria:
- **History growth a day within C6's target: met.** It projects to 8.0-12.1 MB a day (9.9 MB a day as measured over
  600 blocks), against 4.49 GB before FN-3.
- **RSS within the A13 bound: met.** The process peaked at about 0.2 GB against a 5.46 GB bound, residue included. A13
  is a worst-case model; the harness has no peers and no RPC, and its pool case holds 64 items only at the end.
- **child_ctx p99 < 100 ms: met warm, not met cold (before FN-1b; met cold after it, §6).** A cold child_ctx (one per new parent, the FN-1 cache's miss) at
  10,000 members in store mode takes about 105 ms at p50 and 171-188 ms at p99 on this VM (up to 235 ms at p99 in the
  previous run). Almost all of it is the one-pass membership read of the parent's ActList and 10,000 records through
  state.redb. FN-1's in-memory benchmark (tests/fn1_membership.rs) did not see this cost. A node takes the miss once
  per block it validates; every later context of that parent is a hit (0.2 ms).
  - The owner (or the main session) should decide whether M1's 100 ms is meant for the cached path, which meets it, or
    for the cold one.
  - If it is the cold one, a node-local fix would keep the membership across blocks: derive the child's membership from
    the parent's cached one and the block's ActList changes, instead of re-reading every record. That is not FN-3c's
    scope. FN-1b did it (§6): the cold p99 is now 8.9 ms idle and 10.6 ms churning.
- **h_full's overhead (FN-3 §5.14): negligible.** The member record FN-3 added to h_full takes 0.1 ms at p50 and
  0.16 ms at p99 on a delta block, and about 4 ms on a snapshot block (one in 256: the digest of 10,000 ids), against a
  402-427 ms import at p50. Its encoding for history.redb takes under 0.002 ms at p99. h_full's other FN-3 steps, the
  parent's set from the sets cache (a hit in steady state; the import-phase counters show 0 misses idle) and the
  child's set put into it, were not timed apart; each is one cache operation on an `Arc`.
- **The proposer's pool (FN-3 §5.12) at 10,000 members: measured.** A full pool, 64 type-2 items against distinct
  ancestors (58 of them offloaded below the window), adds 472 ms to a signed template built with every member cache
  cleared: each item's owner is recomputed from the records, 64 reconstructions that each end at a snapshot set read
  from history.redb, 0 faults. The next template takes the owner memo: +18 ms, no reconstruction. A node meets the cold
  case when a pool of new items is first evaluated, or after its caches were cleared (a restart); within T_blk 10 s
  either way, and it is node-local (the template's build, not validation).
- **Validation and import at 10,000 members** take 0.28-0.43 s at p50 and up to 0.63 s at p99 here (one churn import
  took 2.05 s), against 10-13 ms at 4 members. The cold child_ctx accounts for about 105 ms of that and h_full's member
  record for 0.1 ms. The remaining roughly 170-320 ms that grows with M is not attributed here. The likely places are
  the system programs' reads at a full set and DrawWeights. It matters for T3 (t_exec ≤ 1 s, validate+commit p99) on
  the founders' machines, and is left to M1/T3 in the rehearsal (§5).
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
- The harness passed in 1,463 s; its output is the one recorded in §3.

## 5. Deferred, and why

- **M1's own registration rehearsal** (about 2,500 blocks, about 7 h at T_blk 10 s, registering up to 10,000
  identities): the harness seeds the identities at genesis instead, so it can run on one machine in about 25 minutes.
  Registration cost at a full set belongs to M2 and M3 (gas and the capacity path) on the rehearsal network.
- **Gateway reward computation, T4, T5:** these need the reward gateway and live multi-host mining, which this harness
  does not run (Phase 5 / rehearsal).
- **Cold child_ctx above 100 ms:** §3. Closed by FN-1b (§6) for every new parent whose own parent's inputs the node
  holds. What stays at about 115 ms (p99 187-211 ms) is the fallback that reads the inputs whole: the first context
  after a restart or a checkpoint restore, a parent reached by a deep reorg whose inputs were evicted, a block whose
  registry write set exceeded 4,096 slots. These are one block each, not the per-block path.
- **The M-dependent validation cost** beyond child_ctx: §3, for T3.
- **The §1a CP's A13 margin** (167 MiB at empty files; 144 MiB after FN-1b): it is admitted while state.redb stays
  below 1,317.6 GiB (with history.redb empty) or history.redb below 668.3 GiB (with state.redb empty); after FN-1b
  below 1,137.6 GiB and 577.5 GiB (`evidence/fn1b/a13-and-genesis.log`). The largest terms are the CODE_CAP 2^30
  code set (1.21 GB) and the execution's memory classes (1.05 GB). The design's own fallback (CODE_CAP 2^29) remains
  available if FN-4's profile adds terms.
- **A node's own RSS apart from the test process:** the three runs share one process, so their RSS figures include
  what the genesis, the admission and earlier runs left resident (§3). Running each case in its own process would
  separate them; the bound is met either way, since the residue only raises the figure.
- **A dedicated CI job for the harness:** not added. At about 25 minutes and a 100-second admission measurement it is
  not cheap. It is documented with the command above. The three default tests (about 2 s) run in
  `full-nonce-upgrade.yml` and in the Windows verification's pocold group.

## 6. FN-1b: the cold child_ctx with membership inputs derived across blocks

FN-1b (node-local; `chain::Chain::member_base`, `members::MemberBase`) keeps what membership() reads of a state (the
ActList in order and, per id on it, the record fields the one pass reads: tomb, last_share, grace_start, bond_state and
whether the bond is below B_reg/4) for at most 3 states, and each executed block's registry write set (the overlay's
complete write set; in store mode the slots its stored version was updated with) for at most 8 blocks of at most 4,096
slots, keyed by block hash and state identity like FN-1's rows. A new parent's inputs are derived from its own parent's
and its block's write set: the list length and each written list element are re-read, and the records of ids one of whose
four words was written or that are new to the list; an id that left is dropped. Anything not held (a restart, a
checkpoint restore, a rebuilt or materialized state, an evicted row, a larger write set) reads the inputs whole, as before.
The membership is then computed from the inputs by the same code as the one pass (`membership_from_base`), so equal inputs
give the same set, order and error. With the member oracle on (every test) each derived or cached membership is compared
with the one-pass reading, and each executed block's derived inputs with those read whole from its state. A difference
is counted as an oracle mismatch and a member fault, every held input is dropped, and the one-pass result is the one
returned, so no verdict follows the derivation. No verdict, root, persisted byte or identity changes.

The run: the harness of §3, same command, same VM, oracle off. "Before" is the pre-FN-1b library: its counters have no
FN-1b fields and its A13 model at the end is FN-3c's. "After" is this branch's. Both ran one after the other on an idle
machine: after in 1,023 s, before in 1,423 s. The harness now times the cold context of a new parent the way a node meets
it. FN-1's rows are cleared, and the parent's own inputs are forgotten (its block's write set and its parent's inputs are
kept), so the context derives them. From the second block on, the harness asserts by count that it derived one input and
read none whole. The same context with every input cleared (the fallback) is timed too, and asserted equal. Printed lines:
`development/audit-2026-10-22/evidence/fn1b/growth-and-cpu-600-{before,after}.log`.

| Measure (ms) | 10k-idle before | 10k-idle after | 10k-churn before | 10k-churn after | 4-idle before / after |
|---|---|---|---|---|---|
| child_ctx cold p50 / p99 / max | 103.9 / 163.0 / 290.4 | **4.8 / 8.9** / 262.3 | 102.0 / 167.1 / 270.0 | **5.4 / 10.6** / 262.4 | 0.11 / 0.23; 0.08 / 0.18 |
| child_ctx with the inputs read whole (the fallback) p50 / p99 | — | 116.0 / 210.9 | — | 113.8 / 187.4 | — ; 0.10 / 0.22 |
| child_ctx warm (FN-1 hit) p50 / p99 | 0.104 / 0.156 | 0.060 / 0.127 | 0.106 / 0.174 | 0.064 / 0.149 | 0.012 / 0.052; 0.012 / 0.056 |
| the one-pass membership alone p50 / p99 | 102.3 / 157.7 | 101.2 / 175.7 | 101.8 / 159.5 | 98.9 / 157.5 | 0.03 / 0.09; 0.03 / 0.08 |
| validate (FN-1 rows cleared) p50 / p99 | 275.9 / 395.9 | 166.9 / 232.5 | 288.9 / 482.0 | 183.9 / 336.4 | 10.6 / 15.9; 9.9 / 22.9 |
| import p50 / p99 | 398.6 / 561.3 | 179.8 / 281.9 | 411.4 / 618.1 | 199.6 / 365.5 | 12.7 / 19.0; 11.7 / 31.1 |
| reconstruction cold p50 / p99 | 6.3 / 9.8 | 6.5 / 7.6 | 6.4 / 13.3 | 6.6 / 12.4 | 0.22 / 0.54; 0.16 / 0.38 |
| proposer's pool, signed template cold empty / full | 260 / 790 | 236 / 723 | | | 5.7 / 46.3; 7.6 / 44.8 |
| A13 model at the end (B) | 5,461,945,681 | 5,485,767,477 | 5,461,946,195 | 5,485,767,991 | |
| process peak RSS (VmHWM) | 197.8 MiB | 203.2 MiB | 191.6 MiB | 199.5 MiB | |

The cold max of the after runs (262 ms) is block 1: genesis has no parent inputs held, so its inputs are read whole
once. Validation and import fell by about 110-220 ms at p50. Before FN-1b every validation and every import re-read all
10,000 records. After it, they do not take the cold child_ctx's path: FN-1's rows are cleared before each, but
`clear_ctx_cache` keeps the inputs, so validation and import find the parent's inputs already held (here from the
harness's own whole read in the fallback timing just before; in a node, from the context it derives of the new head right
after the import). The counters show it: 1,200 input hits, two a block, and 601 whole reads, the harness's 600 plus
genesis. The validate figures therefore contain no derivation; the derivation's ~5 ms is in the import figure (the
post-import context of the new head, about 600 of the 1,199 derivations) and in the cold child_ctx row.

What a derivation read (the import phase's FN-1b counters, 600 blocks; besides these O(writes) reads it does O(M_max)
work in memory, copying the parent's inputs as `MemberBase::derive` builds the new ones, part of the ~5 ms with the
membership computed from them): idle, 1,199 derivations, 0 list elements and
2,044 records, about 3.4 records a block (the records one of whose words the block wrote). Churning, 1,199 derivations, 2,340 list elements and 4,287 records. 1,200 hits; 601 whole reads (block 1
and the harness's 600 fallback timings). The pool case's cold template read the inputs whole once (its caches were
cleared).

Against M1's **child_ctx p99 < 100 ms**: met cold as well as warm, for every new parent whose own parent's inputs the
node holds. That is the per-block path of a synced node, including the side blocks of a shallow fork. It is not met by
the fallback (about 115 ms at p50, 187-211 ms at p99 here), which a node meets once: the first context after a restart
or a checkpoint restore, or a parent of a reorg deeper than the 3 inputs kept. That is no worse than before FN-1b.

Memory (A13, `resources::member_bases_bytes`; A13-DERIVATION.md "FN-1b"): 23,821,796 B at M_max 10,000 (3 held inputs
and one being derived, at 5,451,520 B each as modeled, and 8 write sets of 4,096 slots). The C6-row CP's total is now
2,917,910,217 B. The §1a CP's is 5,485,767,089 B, admitted with 151,377,487 B (144 MiB) to spare (175,199,283 B
before). The file headroom shrinks with it: state.redb below 1,137.6 GiB (1,317.6 before) with history.redb empty, or
history.redb below 577.5 GiB (668.3 before) with state.redb empty (`evidence/fn1b/a13-and-genesis.log`). At the
devnet's M_max 64 the term is 1,207,460 B.

Tests (FN-1b, `tests/fn1b_derivation.rs`, and the oracle in every chain-running suite):
- The derivation over 400 steps of synthetic registry changes (v3 and spec 1) equals the whole reading, and the membership
  computed from it (or its error) equals the one-pass reading at several parent heights. Leaving a written slot out
  makes it differ (the negative control).
- v3 and spec-1 chains, at every new head: registrations, an unbonded probationer's bond through B_reg/4 to HELD,
  registerReplacing, a type-2 punishment, an exit, the snapshot height 257, a reorg across the membership change and
  back, and a restart. Each derives (counted) and equals the one-pass and two-pass readings.
- A store-mode node: every import derives, and a rebuild derives along its re-execution. The first context after a
  checkpoint restore reads whole, after a replay it derives, and every later import derives.
- The cache stays within its bounds.
- Inputs damaged on purpose are caught. On a chain that tolerates it (the negative controls) the one-pass result is
  returned, the mismatch is counted, and the inputs are dropped; on any other chain with the oracle on a mismatch panics,
  so a suite that reads no counter still fails.
- A write set that leaves out the slots its block wrote is caught at import and not kept, and a write set above the cap
  (lowered for the test) is not kept; either way the next context reads its inputs whole. The write set is copied once,
  after the block's store commit, only within the cap and after the oldest row made room (no in-flight copy outside the
  A13 term), and a registry replaced whole or an unknown write set is not kept (a unit test).
- In every other suite the oracle compares each executed block's derived inputs with those read whole, and the
  end-of-test sweep (or, in fn1_membership and v3_fork_choice, a counter check) asserts that it compared and found no
  mismatch.

Results on this branch (`cargo +1.99.1 test --release --locked`, Linux; the counts of §4 unless named):
- `--lib` (267, with the review fixes' unit test of the kept write set) and every other pocold test target pass.
  - The new `fn1b_derivation` target passes 10 (7, and the review fixes' three: a mismatch panics where it is not
    tolerated, a write set above the cap is not kept, a write set missing its slots is caught at import).
  - `fn3_scale` passes 3, with the harness ignored.
  - `consensus` (165, 10 ignored) and `caps` (11) pass on one thread.
- `consensus_guard` passes 20 of 21. The one failure is `the_tree_matches_the_consensus_lock`, as expected: FN-1b changes
  `pocold/src/chain.rs` and `pocold/src/members.rs`, which are consensus sources. The change is node-local (kind=neutral):
  no verdict, root, vector or persisted byte changes. The lock is left for the owner to re-pin.
