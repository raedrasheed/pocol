# M1 Draft 0.32: C37 transitive history closure (supplement)

**For root review. The author ran nothing. M1 is not complete.** Closure is claimed only after root executes this supplement.

## 1. The defect (C37)

`run_checks_031.history_final()` built its status map **before** it recorded `history.HF-8`. `history.HF-12` therefore saw `None` for `history.HF-8`, even though the completed 0.31 run contains it as `pass`.

- The single 0.31 FAIL is `history.HF-12`.
- All six old 0.30 failures have passing repair checks in the completed 0.31 results.
- C35 and C36 are verified closed (REVIEW-0.31).

## 2. What 0.32 adds

This supplement checks metadata and history only.
- It rebuilds and re-runs nothing: no model, fixture, hash, signature or native codec execution.
- It starts no Node process.
- It reads root's completed artifacts and binds them by sha256.

### One closure method

`close()` in `tools/run_checks_032.py` is the only closure method. It is used for all four computations:
- the fresh HF-12 recompute;
- the stale-versus-fresh demonstration;
- the mutations;
- the global closure.

Status maps are built from complete result lists at the moment of use.

A failure closes only when **exactly one** registry entry (`audit/history-registry-0.32.json`) maps it and every witness is proved. The witness kinds are:
- **exact:** exactly one entry with that label, and it passes. A duplicate label is ambiguous.
- **pattern:** at least one matching entry passes and none fails.
- **via:** transitive; the named failure must itself close, with a cycle guard.
- **file:** a later root-evidence file records zero failures.
- **self:** a check of this run, read from the fresh map built after it was recorded.

### Enumeration (no blanket discard)

The runner scans every preserved `run-results*.json`. It finds 25 FAIL entries:

| Run | Failure | Repaired by |
|---|---|---|
| 0.4, 0.5 | C14 | 0.6 `c14.*` |
| 0.6 | br19a | 0.8 `br19a.*` |
| 0.7 | an aborted step | 0.8 `snapshotE.*` |
| 0.13, 0.14 (four entries) | C26 | 0.15 E4 checks and step guards |
| 0.16 (two entries) | C27 | 0.18 C27 |
| 0.17 | C28 | 0.18, the same row passing |
| 0.21, 0.25 | copy-path failures | 0.22, 0.25 |
| 0.29 (four entries) | C34 (HF-8) | see below |
| 0.30 (six entries) | C36 | 0.31 |
| 0.31 (one entry) | C37 | this run's fresh recompute |

It also scans the top-level root-evidence files. Nine record failures, and each is bound to its corrected successor:
- `independent-hashes`;
- `epoch` 0.10;
- `genesis` initial expectation;
- `x8` 0.23;
- the malformed-error probes of 0.26;
- the 611 null-reference triad failures;
- the three guard-integration probes;
- the two native-comparison loader files.

Files named `-private` are deliberately not opened. Any unmapped failure prevents closure.

### Positive proofs

- **Exact old failure sets:**
  - 0.29: four entries;
  - 0.30: the six named entries;
  - 0.31: `history.HF-12` only.
- Each 0.30 repair target passes **exactly once** in the completed 0.31 run.
- **The HF-8 chain.** Of the four 0.29 failures:
  - three are repaired by 0.30 passes;
  - `coverage029.required` closes via the failing 0.30 `coverage030.required`, which closes via 0.31 `coverage031.required`.

### Live order

The same `close()` is applied twice:
- to the 0.31 result list **cut just before `history.HF-8`** (the 0.31 snapshot point), where it must not close;
- to the complete list, where it must close.

### Mutations

Each of these must prevent the global closure:
1. remove HF-8;
2. mark HF-8 FAIL;
3. mark a required C36 repair FAIL;
4. remove a repair label;
5. add an unmapped old FAIL;
6. a duplicate HF-8 that fails;
7. a duplicate HF-8 where both pass (ambiguous);
8. remove a registry entry;
9. break the transitive chain (`coverage031.required` FAIL);
10. a duplicate registry entry;
11. no fresh self check.

The old failures stay in their files. Nothing is deleted or relabelled.

## 3. Bindings

Read from saved artifacts, not recomputed:

**The completed 0.31 run.**
- Summary: 383 entries, 280 passed, 102 recorded, 1 FAIL, 123 native codec calls.
- Rows: 38 CompleteCandidate and 3 PendingRootReview.

**The unchanged 0.31 package.** Main tree, review copy, review-copy manifest and the sha256 values recorded by that run must all agree.

**Runtime**, as recorded in the 0.31 artifacts: Python 3.11.1, Node v22.13.1, and the codec sha256.

**Root's independent evidence:**
- 24 guard cases;
- 668 triad preimages;
- 3677 + 347 V1 hashes;
- 31 signatures;
- 1027 ASERT cases;
- 1120 deadline cases.

**The 0.30 freeze:** 668 legacy entries and 4024 V1 entries, none blocked, and three opaque groups labelled, not frozen.

**Experiments.** Definitions only. No Phase A measurement is claimed.

**Review decisions:**
- REVIEW-DECISIONS-0.31 accepts C3, C4 and V1 at specification scope across c1–c5. It is qualified only by C37, and `globalAcceptance` is false.
- REVIEW-DECISIONS-0.30 accepts C1, C2, R1 and E6 as unaffected.

**Named external parameters** (not assumed):
- `CONF_DEPTH` is a runtime input with no default;
- the real factory deployment address is outside M1;
- the Phase A outcomes;
- there are no owner answers.

## 4. Candidate gate

All 41 rows are recomputed from the root-executed 0.31 matrix. That matrix is cross-checked entry by entry against the 0.31 run's own row checks.
- **c1–c4.** For C3, C4 and V1 these become satisfied only through REVIEW-DECISIONS-0.31. The other 38 rows keep their satisfied 0.31 criteria with links.
- **c5.** Satisfied only if `history.globalClosure` passes in this run and the row has no genuine blocker. Otherwise it is blocked with the reason.

**Expected:**
- 41 CompleteCandidate rows;
- F01–F25 closable at specification scope;
- F26 waits for root to record all 41.

This is a **candidate** only. `globalAcceptance` and `m1Complete` stay false. Root records Complete, closes C37 and records F26 only after reviewing this run.
