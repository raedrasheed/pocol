> **Status: DRAFT. Not accepted and not closed. Do not merge or deploy.**
> - No finding is closed. Acceptance belongs to the owner, and none is recorded. No full acceptance is claimed.
> - **Owner-approved and implemented:**
>   - D-A13-BUDGET, **D-REORG-REDB** and the **RpcGuard** implementation;
>   - **D-1 option (a)**: storage conformance S1–S5 toward 6 GiB for the network profile. S1, S2, S3a, S4, E1–E4 and E5 have landed. The state store (E4) is **opt-in**: `Limits::state_store`, or `pocold run --state-store` for experiments. The default stays whole-state;
>   - **D-6**: local state checkpoints, implemented in store mode (E4.4);
>   - **D-7 (approved 2026-10-25)**: sliced pool revalidation, and a local bound of **4·GAS_LIMIT** on the pooled transactions' summed declared gas limits, configurable within the resource limits. It is pool policy only (`ee14458`). Within its re-insertion behaviour, a transaction returned from an abandoned block is offered to the peers (`8584283`, local relay policy).
> - Still **PROPOSED, not approved**, with no normative text edited:
>   - the six consensus proposals (LOG-CAP-AGGREGATE among them);
>   - CAPS-SUSTAINABILITY N1–N8;
>   - RPCGUARD P1–P4;
>   - every journal mitigation;
>   - D-2, D-3, D-4, D-5;
>   - the A13 key-path assumption. It is presented with its definition, rationale, worst case and the conservative alternative, and is **not adopted**.
> - The network profile does **not** boot within 6 GiB. These are computed bounds, not measured RAM:
>   - store mode: **7.61 GiB (gate 8.69)**, after D-7 and the code-loading audit (9.67 before);
>   - whole-state mode: 25.24 GiB;
>   - the gate needs at most **5.25 GiB**, so store mode is over by **2.36 GiB**.
>   - No combination of local, non-normative options reaches it. The options, with their numbers, are in the decision package.
>   - The earlier projection of ≈ 5.3 GiB is **withdrawn**. Its gas-derived bound on the code an execution reads does not hold: revm loads an account, and in store mode its code, before it charges for it.
> - Windows:
>   - runs 64 (`9bd9bc0`), 65 (`919a50c`) and 66 (`8584283`) **passed every check**; the commits after `733a52d` have no Windows run yet;
>   - the CI does not run the e2e or Chrome suites, the crash-step feature or the `#[ignore]` tests. **A Windows pass is not acceptance.**
> - Per-finding status is on five axes in [FINDING-STATUS.json](development/audit-2026-10-22/FINDING-STATUS.json). The disposition is the weakest axis.

## D-7 and the code-loading audit (2026-10-25; [evidence/d7-bound](development/audit-2026-10-22/evidence/d7-bound/README.md))

**D-7 at 4·GAS_LIMIT (`ee14458`).** G = 32,000,000 at both profiles.
- **Configuration:** `--pool-gas-max` sets another value. One below GAS_LIMIT is refused at open, and so is one the A13 budget cannot hold.
- **Admission:** past the bound, the answer is −32016 "pool full (gas)" before execution, and nothing changes.
- **No replacement:** a same-nonce transaction fails on the pending state.
- **Removal:** only at a new head, with a recount.
- **After a reorg:** the abandoned blocks' transactions come first, then the pool, each within every bound.
- **Relay of a returned transaction (`8584283`):** once admitted again, it is offered to every peer. A peer with half its queue in use does not get it and is kept. The previous pool's transactions are not sent again.
  - **Found by e2e_integrated on `919a50c`**, which failed 2 of 4 runs: node c held 7 returned transactions that no other node had.
  - c had left the active set during a partition longer than W_act = 32 heights (consensus.md §2). It was not drawn at any of the 215–224 heights after the heal, so it built no block.
  - That membership behaviour is the specified one and is unchanged. The defect was the missing relay.
  - Mutation controls: **7 of 7 killed** after `90c4185`, whose node-level test gives a peer a queue past half full (6 of 7 on `8584283`, where R5, the offer arm calling `send`, survived).
  - Open: a returned transaction that no peer took stays with this node until it is drawn.
- **Counters:** checked against the contents through 160 mixed steps.
- **Consensus:** under `Node::run`, with the pool at its bound, a block is imported while revalidation is pending.
- **Gas is not time.** What bounds the hold is the 50 ms slice plus one transaction, measured at 193 ms.
- **Mutation controls:** **18 of 18 killed**, none classed equivalent.

**The audit of revm 19.7's loading paths** (A13-DERIVATION §2d):
- **Store mode reads code with every account load,** and revm loads before it charges (as little as 100 gas per account). So the code an execution reads stays one CODE_CAP set.
- **A created account keeps every slot it loaded,** including zero-valued slots and slots loaded in frames that ran out of gas. A local witness, kept outside the repository, left 2,000 entries for 1M gas.
- **Terms re-derived:**
  - the block overlay was an undercount;
  - the pool's layer now follows G instead of "one whole state", which did not bound those slots;
  - the pool's code is its touched accounts' code and its creations.

**The network model after it:**

| | whole-state | store |
|---|---:|---:|
| before (§2c, `fc24969`) | 27.92 GiB | 9.67 GiB |
| after (§2d, `ee14458`) | 25.24 GiB | **7.61 GiB** |

The devnet bounds rose (whole-state 2.91 → 3.04 GiB, store 3.36 → 3.48 GiB) because of the undercount above.

**Options to reach 5.25 GiB** (computed network store-mode totals; none adopted; [DECISION-PACKAGE.md](development/audit-2026-10-22/proposals/DECISION-PACKAGE.md), "After D-7 and the code-loading audit"):

| option | kind | total |
|---|---|---:|
| G = 2·GAS_LIMIT | your D-7 value | 7.23 |
| key paths ≤ 21 nodes | A13 assumption | 6.79 |
| CODE_CAP 1 GiB → 256 MiB | normative | 6.43 |
| key paths ≤ 21 + CODE_CAP 128 MiB | assumption + normative | 5.33 |

**Store mode end to end.** `--state-store`; the e2e scripts pass `POCOLD_RUN_ARGS` and check the mode each node reports.

**Acceptance on `8584283`** (the relay fix; [accept-8584283](development/audit-2026-10-22/evidence/d7-bound/accept-8584283/)):

| step | result |
|---|---|
| pocold (release) | 231 passed, 0 failed, 22 ignored |
| e2e, both modes | devnet 43/43, site 21/21; integrated **20/20 in three runs of three per mode**, and in each, c offered its 7 returned transactions |
| e2e_browser | store 80/80; whole-state FAIL 1 of 49, a navigation race in the test, fixed in `d939d1a`. On `d939d1a`: whole-state 3 of 3, store 1 of 3, the two failures being BR14 reading a receipt once across a short fork, fixed in `0833b6c`. On `0833b6c`: PASS 80/80 in three runs of three per mode |
| witnesses A01–A14 | all NOT_REPRODUCED, every positive control OK. A02, A05 and A10 were rerun after a full disk interrupted their builds |
| Windows run 66 | PASS on every check |

Not rerun on `8584283`, because the change does not touch them: lp4, lp1-node, the crash/kill/damage steps, the spec-depth and deep-reorg tests, the A13 models, RSS, R14 and churn. Their runs on `9bd9bc0` follow.

**Acceptance on `9bd9bc0`** (the node code of `ee14458`):

| step | result |
|---|---|
| lp4 (release) | 193 passed, 0 failed |
| pocold (release) | 229 passed, 0 failed, 22 ignored |
| crash steps, kills, damage; store mode at the spec depth 2048/2049; deep-reorg tests | passed |
| e2e, store mode | devnet 43/43, site 21/21, browser (Chromium) 80/80, integrated 19/19 |
| e2e, whole-state mode | devnet 43/43, site 21/21, browser 80/80; integrated **FAIL 1 of 11** on `9bd9bc0` (a nonce race in the test after the heal; `919a50c` fixes the test, and its reruns exposed the relay defect fixed in `8584283`) |
| extension | 234/234 |
| lp1-node (1.58.1) | 137 passed; the 11 Windows-only store_journal tests return UnsupportedPlatform on Linux. **Not a pass on Linux** |
| witnesses A01–A14 | all NOT_REPRODUCED, every positive control OK |
| Windows run 64 | PASS on every check |

**Measured** on this host; these are not bounds:
- **RSS under load:** 13 MB whole-state, 15.7 MB store mode.
- **state.redb under churn:** stable at 67.9 MB, at most 7.95 times what it stores. A13 counts 12.
- **R14/R19:** the admission timing did not include store mode's commit and fsync. A GAS_LIMIT compute block imports in 64 ms (whole-state 29 ms); state-heavy blocks took up to 4.8 s.

**R14/R19 in the booting mode (`24cc167`).**
- A store-mode node is now admitted on a timing that includes the calibration blocks applied through a scratch state store on its own disk, each committed durably. The gate takes the larger time per phase.
- Measured on this host: 10–26 ms per phase through the store against 8–18 ms in memory (15 runs); t_exec is 1,000 ms.
- Mutation controls: **11 of 12 killed**. C12 (not clearing a crash's leftover scratch store first) survives: the directory is removed afterwards either way, so no test can observe it.
- **Decision for you, not adopted:** the calibration is a compute workload. State-heavy blocks exceed t_exec = 1 s on this host in both modes (up to 1,068.5 ms), so calibrating on them would refuse both modes there.

**Acceptance on `24cc167`** ([accept-24cc167](development/audit-2026-10-22/evidence/d7-bound/accept-24cc167/)):
- lp4 196/0 and pocold 233/0 with 23 ignored, on its prototypes tree;
- e2e in both modes: devnet 44/44 (the new check: each node's timing covers its mode), site 21/21, browser 80/80, integrated 20/20 in two runs of two;
- witnesses (rust and live): all NOT_REPRODUCED, controls OK;
- **Windows not run yet:** these commits were made while GitHub refused the session's credentials, and are pushed with this description.

**Not run, not counted:** the debug-mode suites; the private adversarial EVM measurements; the other `jm_*` measurements; RSS at the network scale.

## Decision package (2026-10-24, updated 2026-10-25)

- [DECISION-PACKAGE.md](development/audit-2026-10-22/proposals/DECISION-PACKAGE.md), with the English summary for approval in [DECISION-PACKAGE-EN.md](development/audit-2026-10-22/proposals/DECISION-PACKAGE-EN.md). "Measured after E4 and E5" holds the store-mode model, the D-7 decision and the disk and time measurements.
- The D-3/D-5 analysis is in [D3-D5-HEND-ANALYSIS.md](development/audit-2026-10-22/proposals/D3-D5-HEND-ANALYSIS.md): H_END behaviour, non-exhaustion proofs and a release plan. Nothing in it is normative.
- The storage design and the per-stage status are in [C1-STORAGE-DESIGN.md](development/audit-2026-10-22/proposals/C1-STORAGE-DESIGN.md). The A13 store-mode terms are in [A13-DERIVATION.md](development/audit-2026-10-22/A13-DERIVATION.md) §2c.

**Storage conformance (D-1 (a)), measured by `derive()`.** No validity, root, receipt, genesisHash or RPC-result change.

| stage | change | network model | evidence |
|---|---|---:|---|
| S2 (`d567ce1`) | code shared between state copies | 150.83 → 105.83 GiB | evidence/decision-package |
| S1 (`b517b0f`) | canonical history below the K_hist window is off the heap | → 90.93 GiB | [evidence/c1-s1](development/audit-2026-10-22/evidence/c1-s1/README.md) |
| S3a (`bef6c4f`, `4b49d03`) | retained canonical states are diffs from their parents | → 41.41 GiB | [evidence/c1-s3a](development/audit-2026-10-22/evidence/c1-s3a/README.md) |
| S4 (`ddb0de2`) | old canonical entries slimmed in memory, restored exactly | → 37.96 GiB | [evidence/c1-s4](development/audit-2026-10-22/evidence/c1-s4/README.md) |
| E1 (`28594cc`) | incremental state root | → 40.20 GiB | [evidence/c1-e1](development/audit-2026-10-22/evidence/c1-e1/README.md) |
| E2 (`1774b90`) | the per-block whole-state checks from a state summary | 40.20 GiB | [evidence/c1-e2](development/audit-2026-10-22/evidence/c1-e2/README.md) |
| E3.1–E3.3 (`4ad4e83`) | the executor, production, the pool and RPC on overlays | → **27.92 GiB** whole-state mode (28.08 before E5's shared empty state) | [evidence/c1-e3](development/audit-2026-10-22/evidence/c1-e3/README.md) |
| E4.1–E4.3 (`7606a31`, `f8a58bb`, `6c6e192`, `8f4b473`, `b91c490`) | every retained state is a trie version pinned in `state.redb` and read in place (store mode, opt-in); E3.4 folded in | store mode | [evidence/c1-e4](development/audit-2026-10-22/evidence/c1-e4/README.md) |
| E4.4, D-6 (`1a5b792`, `3c0fe15`; test fixes `45d540f`, `53a80e9`) | sealed checkpoints bound to the journal; open resumes from the newest that verifies, never shortens the journal or history.redb | | same |
| E4.5 (`314de11`) | store-mode A13 terms | store mode 10.58 GiB | same |
| E5 (`90ba824`, `5722c23`, `fc24969`) | one shared empty state; key preimages in the leaves; redb's allocator counted as built; state.redb's file factor measured (12); a close leaves its versions to the next open | store mode → **9.67 GiB** | same |

How E4 and E5 are proven:
- **Store mode answers as whole-state mode.** Every block, receipt and state query at every height is compared, with the retained states and the pins:
  - live, after reorgs and after reopening;
  - at the R_keep boundary;
  - with a fork deeper than K_hist;
  - at the spec depth 2048/2049 (`--ignored`, run on the final tree).
- **Open from a checkpoint is the node replay from genesis gives, field by field:**
  - from checkpoints written by replay and written live;
  - with slimmed, offloaded and evicted entries;
  - with replay's protect set mid-flood;
  - after kills at increasing delays;
  - after an abort at each of seven steps of a block's commit and a checkpoint's write (22 cases);
  - after 17 kinds of damage and a history.redb that lost an old block's records;
  - no unverified state is used.
- **Mutation controls:**
  - E4.2: 12/12; E4.3: 11/13, with C7 and C10 killed on the retest;
  - E4.4: 10/20 on pass 1 and 8/13 on the retest; the 5 retest survivors have tests since `fc24969`;
  - **the final pass on `fc24969`: 30 of 30 killed**, none classed equivalent (E4.4's 21, E4.3's C7/C10, E5's 7; evidence/c1-e4/mutants-final.log).
- **Defects the work found, each fixed with a test:**
  - restore refused a slimmed entry that still held a version;
  - a damaged store still served cached nodes;
  - a node's close released every version in its own non-durable transaction, which grew the file 2–4× (the A/B is in evidence/c1-e4).

**Disk and time** (measured, devnet profile, [disk-impact-final.md](development/audit-2026-10-22/evidence/c1-e4/disk-impact-final.md)):
- import is 1.8–2.4× slower per block on a small store (fsync per block) and about equal on a larger state;
- reorg is 1.5–2.7× faster;
- open from a checkpoint is 1.8–4.0× faster than whole-state replay;
- RSS after sync is within 15% of whole-state mode.

**Final acceptance on `fc24969`** (kept as recorded; the `8584283` and `9bd9bc0` passes above are the latest) ([evidence/c1-e4/accept](development/audit-2026-10-22/evidence/c1-e4/accept/)):

| step | result |
|---|---|
| lp4 (release) | 193 passed, 0 failed |
| pocold (release) | 221 passed, 0 failed, 20 ignored (each listed in accept/README.md with where it ran) |
| crash steps (`--features crash-test`), kills, damage | passed |
| store mode at the spec depth 2048/2049 (`--ignored`) | passed |
| deep-reorg `--ignored` tests (2048/2049 adoption, D133 at 400, A11) | all three passed |
| e2e_devnet (network, 3 nodes) | PASS 42/42 |
| e2e_site | PASS 20/20 |
| e2e_browser (real Chromium) | PASS 79/79 |
| e2e_integrated (LP10, Chromium) | PASS 18/18 |
| extension | build passed, 234/234 unit tests |
| lp1-node (1.58.1) | 137 passed; the 11 store_journal tests return UnsupportedPlatform on Linux (that journal is Windows-only; 12/12 on Windows). **Not a pass on Linux** |
| Windows CI run 61 (`fc24969`), run 62 (`f14f2c2`, docs only) | PASS on every check |

**Not run in this pass, and not counted:**
- the debug-mode suites;
- the witnesses;
- the D-REORG-REDB and RpcGuard mutation passes;
- the RSS run under load;
- the R14/R19 calibration;
- the `#[ignore]` measurement tests other than the disk-impact one.

**About store mode's coverage (as recorded on `fc24969`):** store mode then had no command-line switch, so those e2e runs used whole-state mode. Since `ee14458` it has one, and the `9bd9bc0` and `8584283` passes above ran every e2e suite in both modes.

---

This PR repairs the 14 findings of the adversarial audit of `090eae4` (REPORT.md), together with the dependency review and the independent reviews.
- Everything is under [`development/audit-2026-10-22/`](development/audit-2026-10-22/).
- The full report is [REPAIR-REPORT.md](development/audit-2026-10-22/REPAIR-REPORT.md).
- The English dashboard is [DASHBOARD-EN.md](development/audit-2026-10-22/DASHBOARD-EN.md).

## Commit roles (tree equivalence in [EVIDENCE-RECONCILIATION.md](development/audit-2026-10-22/EVIDENCE-RECONCILIATION.md))

| role | commit |
|---|---|
| Audited snapshot | `090eae4` |
| Last implementation commit | `24cc167` (R14/R19 for a store-mode boot). Before it: `90c4185` (a relay test), `78fe971` (this resume point, documents only), `733a52d` (evidence), `8584283` (D-7 re-insertion relay; after it, tests only: `d939d1a`, `0833b6c`). Before them: `ee14458` (D-7, the A13 audit terms, `--state-store`), `fc24969` (C-1 E5 part 3), `53a80e9`, `5722c23`, `90ba824`, `3c0fe15`, `314de11`, `45d540f`, `1a5b792`, `b91c490`, `8f4b473`, `6c6e192`, `f8a58bb`, `7606a31` (E4), `4ad4e83` (E3), `1774b90` (E2), `28594cc` (E1), `ddb0de2` (S4), `4b49d03` and `bef6c4f` (S3a), `b517b0f` (S1), `d1604d0` (D-7), `d567ce1` (S2), and the earlier rounds |
| Suites, acceptance and mutation controls | `24cc167` for the boot timing and the e2e suites; `8584283` and `0833b6c` for what the relay fix touches; `9bd9bc0` for the rest (above); `fc24969` before |
| Mutation-controlled | D-REORG-REDB `3342af3`; round 3: caps `e985ce1`, RpcGuard `dd7d1ec`, e2e test `ad84942`; C-1 S1 `b517b0f`; S3a `4b49d03`; S4 `13aeba7`; E1 `28594cc`; E2 `1774b90`; E3 `4ad4e83`; E4.2/E4.3 `b91c490`; E4.4 `1a5b792` and `22942c7` (that is `3c0fe15` before the rebase onto `45d540f`); final E4/E5 `fc24969`; D-7 `ee14458` (18/18); relay `90c4185` (7/7); R14/R19 store boot `24cc167` (11/12) |

The exploit-shaped witness, adversarial and mutation-runner programs are **not** in this public repository. Only their logs and hashes are committed.

No merge into main, no deployment, no real funds, no billing change, no declaration of full acceptance.

🤖 Generated with [Claude Code](https://claude.com/claude-code)

https://claude.ai/code/session_01WdDy26DUcwZsDK64zKqRL4
