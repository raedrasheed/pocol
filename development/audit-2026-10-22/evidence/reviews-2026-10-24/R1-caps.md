# R1: independent review of the caps track (round 3b), TX_CAP / TXBYTES_CAP / LOG_CAP

- **Reviewed tree:** `claude/audit-repair-a01-a14` at `ec148cf`. The worktree was detached onto that commit. The caps code is as at `e985ce1`; the only later code commit, `462fffc`, touches A11 restore and calibration, not the caps.
- **Build:** release, `--offline`, `CARGO_TARGET_DIR=.../scratchpad/review-target`.
- **Baseline at ec148cf:**
  - `tests/caps.rs`: 9 pass.
  - `caps_identity`: 3 pass, in both the release and the caps-test build.
  - `tests/log_cap.rs` (caps-test): 2 pass.
  - lp4 `apply::tests`: 20 pass.
- **Clean tree:** no tracked file is left modified. The scratch tests used the gitignored `tests/zz_audit_witness*` names; they were placed one at a time and deleted afterwards (`git status --short --ignored` is empty).
- **Scratch artefacts** are all under `/tmp/claude-0/-home-user-Proof-of-Collaboration/5c42c059-8cb3-5c8b-83e8-431ffbd082d1/scratchpad/reviews/r1/`:
  - the witness tests `zz_audit_witness_r1_dep.rs`, `zz_audit_witness_r1_logcap.rs` and `zz_audit_witness_r1_d1.rs`, with sha256 values in the appendix;
  - the mutation runner (`mutants.py`, `run_mutants.py`, `run_d1.py`);
  - per-mutant logs (`mut/`) and `mutation-results.json`;
  - `my_caps_check.py` and its output `my_caps_check.out`;
  - the model output `caps-model-out.json`.

## Summary

| ID | Sev | Subject | Status |
|---|---|---|---|
| R1-1 | MEDIUM | Builder: the TXBYTES skip breaks dependencies between senders, so an includable transaction is starved and with 9 or more dependents no signed template is produced | REPRODUCED |
| R1-2 | MEDIUM | Tests: LOG_CAP accumulation inside a block, its branch scope and pruned rebuild are untested (L1, L2 and L4 survive). The implementation itself is correct | REPRODUCED |
| R1-3 | LOW | Tests: template locking is tested for TX_CAP only (C6 survives), and nothing tests evidence in item 16 (C7 survives) | REPRODUCED |
| R1-4 | LOW | A block with more than TX_CAP transactions on a zero-count parent is pinned only by two defences that mask each other; the D1 double mutant accepts an invalid block and all tests pass | REPRODUCED |
| R1-5 | NOTE | consensus.md:101 "the peer is punished" is not implemented (no penalty for any invalid template) | CODE-READING |
| R1-6 | NOTE | `apply.rs:250` labels a TX_CAP check `Budget("TX_MAX")`; doc comments say "canonical ancestors" for a per-branch counter | CODE-READING |
| R1-7 | NOTE | Study line citations are stale after 462fffc | CODE-READING |
| R1-8 | NOTE | The genesisHash claim is correct; the mixed-build split risk is confirmed (already in study §4) | REPRODUCED |

No deviation from the spec was found in item 16, in the TX_CAP/TXBYTES_CAP builder bounds or in LOG_CAP enforcement at any boundary examined (Q1). The builder cannot produce an invalid own block through the caps (Q2), but it can starve (R1-1).

---

## Q1. Implementation against the spec

Spec read:
- execution.md:83: "TXBYTES_CAP and TX_CAP: the block is invalid, and the builder skips the transaction";
- :82: LOG_CAP = 1 GiB, an exceptional halt;
- consensus.md:23: TXBYTES_CAP and TX_CAP are CP fields 37 and 38, and LOG_CAP is absent;
- consensus.md:101: a violating template counts as absent, and the peer is punished;
- consensus.md:170: item 16, "txBytes and txCount within the caps";
- consensus.md:205;
- validation.md:2061–2065: caps-test sets LOG_CAP = 2^24, simnet only;
- storage.md:76–77: 8.00 GiB of transactions = 2^33, 1.00 GiB of logs;
- D49: TXBYTES_CAP on len(rawTx).

| Boundary | Code | Verdict |
|---|---|---|
| Exactly full | `chain.rs:351-353`: `n<=TX_CAP`, `bytes<=TXBYTES_CAP`, overflow-checked, raw `t.len()` (D49). `user_phase.rs:197`: halt only if `cum+block+charge > LOG_CAP` | Correct. Exact fit is accepted and one over is refused. TX: boundary matrix. LOG: lp4 `log_cap_is_aggregate_over_the_chain` and my witness (Z fills to exactly 2^24, status 1) |
| Per-branch, not head | Every counter derives from the parent entry: `validate` stores `p.cum_*+block` (`chain.rs:344`); `execute` passes `p.cum_log_bytes` (`chain.rs:225`) | Correct. LOG branch scope is untested, see R1-2 |
| Reorg, both directions | Counters live in `Entry`; adoption needs no recount. `offload` (`chain.rs:94`) keeps `cum_*` (metadata) and `meta_clone` copies them | Correct. TX in both directions is tested; C2 (offload zeroes the counters) is killed |
| Replay and sync | `Node::open` replays every record through `chain.validate` (node.rs, around line 208) and sync imports through `import` → `validate`. `reimport` (a duplicate) keeps the stored counters. `ensure_state_with` and `restore_canonical` re-execute with `meta_clone` parents, which carry `cum_log_bytes` | Correct by construction. No caps test reopens a node at the cap. My witness pins rebuild (L4) |
| Received templates | `try_lock_signed` (`node.rs:556`) checks `within_caps(&p, body.txs)` before rebuild, with `p` the production parent | Correct for both counters by reading. Only TX_CAP is tested (R1-3). No peer penalty (R1-5) |
| Count over TX_CAP, bytes within | `within_caps` ANDs both counters | Correct. Matrix TX cases (TXBYTES 2^24) |
| Bytes over, count within | same | Correct. Matrix bytes cases (TX_CAP 1000) |
| Count over TX_CAP on a zero-count parent | item 16 rejects first (`chain.rs:335`, before `execute`); `apply.rs:250` duplicates the check | Correct. The test matrix cannot build this case, so it is unpinned (R1-4) |

## Q2. Builder (`node.rs:450-464`, `build_signed_template` `:501-526`)

**Invalid own block.** None is possible through the caps. `template_txs` checks each candidate against the room left by the ones before it, and `build_signed_template` only removes transactions: it pops for BODY_MAX, `remove(i)` on a USER error, or `clear`. Item 16 is monotone under removal, and evidence does not count.

**Sender's later nonces.** These are skipped correctly through `skipped`. C1b (`break` instead of skip) is killed.

**Byte straddle.** `raw.len() > room_bytes` skips; equality fits. This is correct and well tested (B, T and Y mutants).

**Starvation.** It is real. See R1-1.

---

## Findings

### R1-1 MEDIUM: the TXBYTES skip makes the builder offer transactions that cannot execute and starves includable ones

- **File:line:**
  - `prototypes/pocold/src/node.rs:458` (skip);
  - `:505-519` (8 retries, then `None`).
- **Claim.**
  - The pool is an executable sequence on the head state (`submit_tx` executes each transaction on `pending_state`; `rebuild_pool` re-executes in order).
  - The old gas and BODY_MAX `break` took a prefix, which stays executable. The caps skip takes a subsequence, which need not be.
  - If a skipped transaction from sender A funds, or otherwise enables, a later transaction from sender B, `template_txs` offers B's transaction anyway. It uses room, the block fails with `User(Execution(i, LackOfFund…))`, and `build_signed_template` drops it without refilling the room.
  - So an independent transaction C that fits the room is starved at every height while the pool is unchanged. The pool has no expiry.
  - With 9 or more such dependents, the 8 retries run out, `build_signed_template` returns `None`, and the drawn proposer emits no signed template. Every block is then a fallback and carries no transactions at all.
  - The trigger needs only a room smaller than one large transaction (room < TX_MAX = 131,072 B, the squatting window of study §3a). The transactions are relayed to peers on submit and never charged.
- **Evidence.** `zz_audit_witness_r1_dep.rs`, log `r1/r1-dep.log`. Setup: TXBYTES room = 2 × 113 B; alice sends 2124 B to an unfunded dave; dave sends 113 B transfers; bob sends one independent 113 B transfer.
  - Harness: `template_txs` = [dave0, dave1]. `build_signed` gives `apply User("Execution(0, … LackOfFundForMaxFee { fee: 2100000000000001, balance: 0 })")`. A block holding only bob's transfer validates and imports.
  - Live single-miner devnet, 2 dependents: head 4, `cumTxs 0`, every block signed and empty, pool 4. Bob's transfer is never included.
  - Live, 9 dependents: head 3, `cumTxs 0`, `signed_templates 0`, `fallback_templates 3`, all blocks unsigned.
- **Status:** REPRODUCED.
- **Fix.** `template_txs` should simulate its selection: execute each candidate on a running copy of `p.state`, as `submit_tx` does, and skip, with the sender-skip, any that fails. Then room is spent only on transactions that execute.
  - At least, after any caps skip, re-validate later candidates from other senders.
  - Do not cap the retry loop at a number below the candidate count.
  - Add a regression test with the setup above.
  - Study proposal N5 (eviction) reduces the persistence but not the starvation.

### R1-2 MEDIUM: LOG_CAP accumulation inside a block, branch scope and rebuild are untested

- **File:line:**
  - `lp4-execution/src/user_phase.rs:197`;
  - `pocold/src/chain.rs:225`, `:101`;
  - tests: `pocold/tests/log_cap.rs`, `lp4-execution/src/apply/tests.rs:292-296`.
- **Claim.** `log_cap.rs` uses one logging transaction per block on one branch, and never prunes and rebuilds near the cap. The lp4 LOG_CAP tests are single-transaction. So three plausible consensus-critical regressions survive every existing test:
  - L1: the check ignores this block's earlier bytes. A block can then push `cum_log_bytes` past LOG_CAP; the witness saw 16,780,832 > 2^24.
  - L2: the counter is read from the head, not the parent.
  - L4: `meta_clone` drops `cum_log_bytes`, so a pruned block re-executes differently and the node can no longer rebuild ("rebuild stateRoot mismatch").

  Each breaks agreement between nodes: a split at the cap, or a failure to adopt a branch.
- **Evidence.**
  - L1, L2 and L4 survive `log_cap` (caps-test), `caps` and lp4 `apply::tests` (mutation table).
  - My witness `zz_audit_witness_r1_logcap.rs` passes on the unmutated code (`witness-logcap-unmutated.log`):
    - Y = two 10,000 B charges on 16,384 B of room gives [ok, halt];
    - Z = a sibling of Y on X with an exact 16,384 B fit gives ok, cum = 2^24, while the head's room is 6,384;
    - W = Y rebuilt after pruning (514 re-executions), then a child of Y, gives ok.
  - The witness kills L1w (Y = [true, true]), L2w (Z halted) and L4w (rebuild mismatch).
- **Status:** REPRODUCED. The implementation is correct; the tests are missing.
- **Fix.** Add the witness's three cases to `tests/log_cap.rs`: two transactions in one block across the boundary, a sibling fit on a non-head branch, and `ensure_state` across a LogCap-halt block.

### R1-3 LOW: received-template locking and evidence handling under item 16 are not pinned

- **File:line:** `node.rs:556`; `chain.rs:335`; `tests/caps.rs` (`a_received_template_past_the_caps_is_treated_as_absent`, TX_CAP 2 only).
- **Claim.**
  - C6 (template locking checks TX_CAP only) survives. No test sends a template whose bytes pass TXBYTES_CAP.
  - C7 (evidence items counted under item 16) survives. No test has a block with evidence near a cap, so the D49 reading "len(rawTx) only" is unpinned at the validity level.
- **Evidence:** the mutation table.
- **Status:** REPRODUCED.
- **Fix.**
  - Add the bytes case to the template test (`cfg(1000, l(a0)+l(a1)-1)`).
  - Add a block with one valid evidence item on a parent at exactly the cap; it must be valid.

### R1-4 LOW: the zero-count-parent case with more than TX_CAP transactions is held by two defences that mask each other

- **File:line:** `lp4-execution/src/apply.rs:250`; `chain.rs:353`; `tests/caps.rs:150-152` (the case is skipped).
- **Claim.**
  - R7 (the study's "equivalent" survivor) is equivalent only because `apply.rs:250` exists.
  - C5 (`apply.rs:250` count removed) is equivalent only because item 16 exists.
  - The matrix skips the case because `build_signed` cannot build it.
  - So the double mutant D1 (R7 + C5) accepts an invalid block, and every committed test passes.
  - The R7 probe that forged such a block was deliberately not committed.
- **Evidence.**
  - D1: `tests/caps.rs` 9 pass (`mut/D1-caps.log`).
  - Probe under D1: "validate of 3 txs at TX_CAP 2: Ok(3)".
  - Probe unmutated: `build_signed` gives `apply Budget("TX_MAX")`.
- **Status:** REPRODUCED.
- **Fix.** Commit a forged-block test (sealed, correctly committed, TX_CAP+1 on genesis) that asserts `Reject::Block` from `validate`. Either reason string is acceptable, or better, require "16 caps".

### R1-5 NOTE: consensus.md:101 "the peer is punished" is not implemented

- **File:line:** `node.rs:547-560`, `:690-702`.
- **Claim.**
  - `try_lock_signed` returns `false` silently. No peer score, disconnect or evidence exists for any invalid template, including one past the caps.
  - The study cites consensus.md:101 as implemented. Only the "as if absent" half is.
- **Status:** CODE-READING.
- **Fix:** state the gap in the study, or add a local penalty (disconnect) when a template from Draw(h,0) fails item 16.

### R1-6 NOTE: misleading labels

- **File:line:**
  - `apply.rs:250`: a whole-block count against TX_CAP reported as `Budget("TX_MAX")`;
  - `apply.rs:89` and `user_phase.rs:40`: "canonical ancestors", where the value is the parent branch's counter.
- **Status:** CODE-READING.
- **Fix:** rename the error to `Budget("TX_CAP")` (or remove the duplicate with an explicit test, per R1-4), and say "ancestors on the parent's branch".

### R1-7 NOTE: stale study citations

- **File:line:** CAPS-SUSTAINABILITY.md header and §7e.
- **Claim.**
  - The study gives fa97594 lines: try_lock_signed's caps check at :559, `submit_tx` at :400, `debug_assert` at :465.
  - After 462fffc (on this branch) they are :556, :399 and :462.
- **Status:** CODE-READING.
- **Fix:** re-anchor the citations to ec148cf, or cite by function.

### R1-8 NOTE: the genesisHash claim is correct

- **File:line:** `lp4-execution/src/p1_genesis_spec.rs:68-121` (CP_SCHEMA, 52 fields: TXBYTES_CAP at index 36, TX_CAP at 37), `:546-569` (`encode`: version, chainId, CP, allocRoot, M_0List, sysCodeHash), `:572` (keccak); `user_phase.rs:24-25` (LOG_CAP is a cfg constant); `tests/caps_identity.rs`.
- **Claim.** "Changing LOG_CAP does not change genesisHash; changing TX_CAP or TXBYTES_CAP does" is correct.
  - LOG_CAP enters no encoded field. `sysCodeHash` covers the bundled programs, and LOG_CAP enters neither them nor allocRoot.
  - TX_CAP and TXBYTES_CAP are encoded CP values. A devnet rebuild also changes allocRoot, because ChainCaps stores the CP.
  - `caps_identity` passes with identical hashes in both builds.
- **Consequence (already in study §4).**
  - A caps-test node and a release node on one simnet genesis are indistinguishable at Hello (`net.rs:65`: chainId, genesis, head, height, work; no build marker, no protocolVersion), and they split at 2^24 log bytes.
  - Admission blocks caps-test on non-simnet, but not a release node on simnet.
- **Status:** REPRODUCED (tests, both builds).
- **Fix:** optional. Add a build flag to Hello's `node` field, or refuse to peer when they differ.

---

## Q3. Mutation table

These are new mutants, not in `evidence/*mutations*`. Each was applied alone, its tests were run, and the original was restored by copy and checked against it.

| ID | File | Mutation | Tests run | Verdict |
|---|---|---|---|---|
| L1 | user_phase.rs:197 | LOG_CAP check ignores the block's earlier charged bytes (`cum+charge`) | log_cap, lp4 apply::tests | **SURVIVED** |
| L2 | chain.rs:225 | LOG counter from the head, not the parent | log_cap, caps | **SURVIVED** |
| L3 | user_phase.rs:197 | `>=` LOG_CAP (exact fit refused) | log_cap, lp4 | KILLED (both) |
| L4 | chain.rs:101 | `meta_clone` drops `cum_log_bytes` | log_cap, caps | **SURVIVED** |
| L5 | user_phase.rs:252 | a halted transaction's logs are charged | log_cap, lp4 | KILLED (both) |
| C1 | node.rs:458 | byte skip → `break` | caps | INCOMPLETE (did not compile: type inference) |
| C1b | node.rs:458 | byte skip → `{skipped.insert; break}` | caps | KILLED (txbytes_cap_exhaustion…) |
| C2 | chain.rs:94 | `offload` zeroes `cum_txs` and `cum_tx_bytes` | caps | KILLED (reorg test) |
| C3 | chain.rs:353 | item 16 count of this block only | caps | KILLED (4 tests) |
| C4 | apply.rs:250 | `>= TX_CAP` per block | caps | KILLED (5 tests) |
| C5 | apply.rs:250 | TX_CAP part removed | caps | SURVIVED (equivalent alone, see R1-4) |
| C6 | node.rs:556 | template locking checks TX_CAP only | caps | **SURVIVED** |
| C7 | chain.rs:335 | item 16 counts evidence too | caps | **SURVIVED** |
| C8 | node.rs:452 | builder room from `chain.head()` instead of `p` | caps | SURVIVED (equivalent: production calls `template_txs` only with `p` = head, `restart_production`) |
| D1 | chain.rs:353 + apply.rs:250 | R7 (+1 slack on a zero parent) together with C5 | caps | **SURVIVED**; probe: invalid block accepted |
| L1w | as L1 | | R1 witness | KILLED |
| L2w | as L2 | | R1 witness | KILLED |
| L4w | as L4 | | R1 witness | KILLED |

Thirteen single mutants compiled (C1 did not; C1b is its corrected form). The committed tests kill 6 of them: L3, L5, C1b, C2, C3 and C4. They miss 7: L1, L2, L4, C5, C6, C7 and C8, of which C5 and C8 are equivalent, so 5 are real gaps. They also miss the double mutant D1.

The TX side of `tests/caps.rs` is strong; my boundary, offload and scope mutants there were all killed. The gaps are LOG_CAP (R1-2), bytes in templates and evidence (R1-3), and the zero-parent over-cap case (R1-4).

## Q4. Study numbers (CAPS-SUSTAINABILITY.md §2)

**The model.** `python3 -I model/caps_model.py <scratch>/caps-model-out.json` reports 25 checks passed, H_GSV1 de518e30…2277 and devnet 4db6aa5c…e638. Its output is **byte-identical** to `vectors/caps-sustainability.json` (sha256 db23bd3f…c069 for both). It was run with an output argument so that no tracked file was written.

**My check.** `my_caps_check.py` is independent of the model. It takes its inputs from governance.md:4-16 and execution.md:82, the EIP-1559 integer rule as in `fees.rs:4-11`, and the RLP Body encoding. I checked separately that empty blocks have gasUsed 0: `apply.rs:323` sets gasUsed from the USER commitments only.

| Number | Study | Mine | |
|---|---|---|---|
| Per block: transactions / raw bytes / log bytes | 380 / 196,592 (131,072 + 65,520) / 32,768 | 380 / 196,592 / 32,768 | ✓ |
| Network fastest, TX_CAP | 26,316 blocks (18.3 d) | 26,316 (18.27 d) | ✓ |
| Network fastest, TXBYTES_CAP | 43,695 (30.3 d) | 43,695 (30.34 d) | ✓ |
| Network fastest, LOG_CAP | 32,768 (22.8 d) | 32,768 (22.76 d) | ✓ |
| TX_CAP at the target | 52,632 (36.6 d) | 52,632 | ✓ |
| Organic per block: TX | 77.2 tx (20.3%, 1.29 tx/s) | 77.16 (20.31%, 1.286 tx/s) | ✓ |
| Organic per block: TXBYTES | 66,280 B (33.7%) | 66,280.36 B (33.71%) | ✓ |
| Organic per block: LOG | 8,285 B (25.3%) | 8,285.04 B (25.28%) | ✓ |
| Devnet and caps/64 | 53 / 86 blocks; 1.0 tx; 839 B; 5.0 tx; 4,194 B; caps-test LOG 512 blocks, 12.8% | same | ✓ |
| Empty blocks from 10^9 to the floor of 7 | **145** (2.4 h network, 580 s devnet) | 145 | ✓ |
| Empty blocks from 2·10^9, 10^10, 10^12 | 150 / 162 / 196 | 150 / 162 / 196 | ✓ |
| Full blocks from 7 wei until one block costs the subsidy | 212 | 212 | ✓ |
| TX_CAP attack at the target, from 7 wei / from 10^9 | 1.47·10^12 wei / 12.8 coins | 1.47·10^12 / 12.768 coins | ✓ |

All re-derived numbers match. I found no error in §2.

## Q5. genesisHash

The claim is correct. See R1-8.

## Q6. Other observations

The study already covers the following, and I confirmed each by reading the code:
- pool squatting near the TXBYTES boundary, with no eviction (`submit_tx`, `node.rs:405`);
- `PoolMeter` does not model the log meter;
- eth_call does not report A03 halts;
- a halted transaction charges its whole gas limit to gasUsed, which drives the base fee.

R1-1 is a separate, stronger consequence of the skip than squatting. It is not in the study.

## Appendix: scratch file hashes (sha256)

- `zz_audit_witness_r1_dep.rs`: 86877af4d0c672ec7dcd2cd72304e045b496f1c908e11964377a640890947205
- `zz_audit_witness_r1_logcap.rs`: d3641ffd208c7c582a660cebf24ed15c222d852d471d4d50672c7cd0c1de8a7e
- `zz_audit_witness_r1_d1.rs`: 0f0b4dc7f05d5f3611a6834d320e7b1ccbaead132412fe5c8c0e003845bbc580
- `my_caps_check.py`: fc2c53720542db99738a6cf3c5335d7abc54db40adf76ef5a5a5fd1139ec969d
