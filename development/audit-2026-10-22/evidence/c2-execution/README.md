# C-2: result-equivalent execution (decision package, priority 2)

These changes are conformance work: no validity, root, receipt, genesisHash or RPC-result change.

| item | change | commit |
|---|---|---|
| 1 | the USER phase checks its snapshot once, at the first transaction, with that transaction's error | `49b2bc2` |
| 3a | PAY judges durability on the engine's database, without a whole-state copy per transaction | `49b2bc2` |
| 3b | the engine reads the state through a borrowed `StateRef` (`lp4-execution/src/state_view.rs`), without a clone per transaction or SYSTEM call | this commit |
| 2 | the code total is kept along the USER phase, the SYSTEM calls and the pool. A create's CODE_CAP check reads only the journaled accounts (`current_total`), and a SYSTEM call checks only the accounts it touched (`revalidate_touched`). The first SYSTEM call, and the first call after the USER phase, still check the whole state | this commit |
| pool | the pending state's code total is checked once per new head. Each submission uses it; any failure falls back to the full check. The rebuild stops once the pool is full by count | this commit |

## Measurements (`jm_tx_state_term`; previous code `3a52ae6` against `49b2bc2`, run alternately on one host)

The other sessions' reviews loaded this host during both runs, so absolute times are higher than the
unloaded round-3 figures. The comparison is between runs made on the same states.

| state | 380 transfers, before | after | 149 small creates, before | after |
|---|---:|---:|---:|---:|
| code (67 accounts, 2.05 MB of code) | 7,891 ms (`code-base.log`) | 266 ms (`code-new.log`) | 5,081 ms | 1,711 ms |
| near all caps (18,828 accounts, 46,636 slots, 2.05 MB) | 28,463 ms (`all-base.log`) | 5,395 ms (`all-new.log`) | 11,178 ms | 5,463 ms |

The remaining cost after `49b2bc2` is the per-transaction engine clone (7.3 ms at the near-cap state)
and the create-time scan. Items 3b and 2 remove both; their measurement follows.

Correction: the column header above first said "200 small creates". The logs record `manySmallTxs` = 149.

## Final measurement (`3a52ae6` against `2ce03c1`, the code with items 1, 2, 3a and 3b)

Each side ran in its own worktree and target directory, alternating base, final, base, final
(`final-load.txt` has the load averages at the end of each run: 2.0–2.8 on 4 CPUs). Other work on the
host (builds and tests of later review fixes) ran during the last two runs, so they are not unloaded
figures. The final code ran under the same load or more, so its gains are not overstated.

| state (height) | block of | before (`final-*-base.log`) | after (`final-*-final.log`) |
|---|---|---:|---:|
| code, end (125; 67 accounts, 2.05 MB of code) | 380 transfers | 6,686 ms | 163 ms |
| | 247 CREATE2 creates | 2,386 ms | 126 ms |
| | 149 small creates | 4,150 ms | 157 ms |
| | empty child | 184.9 ms | 97.4 ms |
| near all caps (549; 18,828 accounts, 46,636 slots, 2.05 MB) | 380 transfers | 22,553 ms | 655 ms |
| | 247 CREATE2 creates | 4,862 ms | 562 ms |
| | 149 small creates | 9,867 ms | 510 ms |
| | empty child | 777.2 ms | 465.0 ms |

The whole measurement took 1,112 s before and 311 s after on the near-cap series (541 s and 43 s on the
code series).

What remains at the near-cap state is mostly per block, not per transaction. The empty child
(465 ms) has no USER transaction, and its cost includes a state root over the whole state. This
measurement does not split that cost further. Item 4 (incremental state root) targets the root part.

## Tests and mutation controls

**New tests (lp4-execution `code_caps_engine_tests`):**
- `independent_engine_limits_with_a_known_code_total`: the 76 independent vectors through the
  known-total path;
- `user_phase_keeps_the_code_total_across_transactions`;
- `user_phase_checks_its_snapshot_once_at_the_first_transaction`;
- `revalidate_touched_equals_the_full_check`.

Debug builds also assert, after every transaction and every SYSTEM call, that the running total
equals a full inventory and that the full check passes.

**Mutation controls** (`mutations.txt`, `mutations-rerun.txt`): M1–M5, M7, M8 and M9 are caught.
M6 (a stale total after the USER phase in apply.rs) survived. That code path now resets the total, so
the next SYSTEM call checks the whole state, exactly as before C-2. M6's mutant therefore no longer
applies.

**Suites on this commit:**
- lp4-execution 173/0 (`lp4-final.log`);
- pocold 174/0, 15 ignored (`pocold-final.log`);
- the ignored consensus tests 3/3, including the A11 deep reorg now at depth R_keep (`consensus-ignored.log`).

**Known, not caused by this change:**
- In debug, the lp4 admission tests fail the R14/R19 timing gate. They do so on `3a52ae6` too: unoptimized
  execution exceeds t_exec = 1 s.
- `net::reconnect_churn_keeps_threads_and_slots_bounded` failed once at a load average of about 10. It
  then passed 5/5 alone and passed in the full suite.
