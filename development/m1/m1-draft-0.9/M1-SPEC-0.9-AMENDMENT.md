# M1 — Draft 0.9: Amendments R9-01 to R9-04 (annex rows Q1–Q4, StoreQueue)

**Status:** review draft, **NOT approved**. Phase S only.

- **Nothing in 0.9 was executed by the author.** The session had file tools only. The coordinator runs the fixtures independently.
- Only new files under `m1-draft-0.9/` were written. All earlier drafts, `reference/`, the original Dialogue source, and the coordinator ledger, reviews and checkouts are unchanged.

**Task.** `coordination/task-007.md`, dispatched by the local broker on owner guidance: resume from 0.8 and address the open items that need no owner decision.

**Scope note.** Q1–Q4 here are the D101 annex rows of `m1-draft-0.2/M1-ANNEX-CHECKLIST-0.2.md:60–65`. They are not the consensus HeadCheck Q1–Q4, and not the publisher Q8–Q12 (X3).

## R9-01: Q1 (counters, FIFO, seats, timers) and Q2 (teardown, wait before snapshot)

`annex/Q-STOREQUEUE-SPEC.md` restates D101/D102/I54–I56 and D99 in English with line citations:
- `qbytes` with TextEncoder UTF-8 (Arabic, non-BMP and lone-surrogate cases);
- the four inclusive counters and the literal violated set;
- the frame-derived storage key;
- the per-key FIFO across tabs, and the global ready FIFO by acceptance;
- at most 2 active writes and 1 per key;
- the quota decision before any seat or backend call, and the recheck statistic;
- the exact timers;
- storeTimeout holding every resource until the actual settle;
- reload before release or pump;
- delete/clear order and missing deletes;
- immutable defaults; TQ test-only, with release rejection by BG1;
- teardown, session-end and navigator rules.

## R9-02: Q3 (canonical tests and the state machine)

- `annex/Q-CANONICAL-TESTS.md` restores, with their literal numbers:
  - BR21a q1–q10 (with q6 success and q7 failure as separate branches);
  - BR21b r1–r6 (both deadline variants);
  - the BR21e TQ 300000-byte edge;
  - BR21f and f2.
- BR21c and BR21d are written as **future** executable-test specifications. Their status is not executed, and no evidence is claimed.
- `annex/Q-TRANSITIONS.md` gives the replyState × ownState diagram and table (T1–T16), the stable pairs and the forbidden pairs.

## R9-03: Q4 (reference extension)

- `tools/storequeue_ref.py`: SiteStorageRef, admission, BG1 configuration, and the StoreQueue model with typed trace and stats. It runs over a fake clock and a holding fake backend, with eight fault controls.
- `tools/run_checks_09.py`: runs every vector and writes only `results/run-results-0.9.json`.
- `annex/Q4-REFERENCE-EXTENSION.md`: the interfaces, why the oracle comparison is not circular, the fault-detection table, and the open P items.

## R9-04: vectors

All golden values are hand calculations from the cited lines.
- `vectors/storequeue-units.json`: UTF-8, `qbytes`, admission, the BR17e quota table, UTF-8 quota boundaries, BG1 configuration.
- `vectors/storequeue-cases.json`:
  - 24 schedules: 16 baseline cases (BR21a ×6, BR21b ×7, BR21e, BR21f, f2; three of them are shared prefixes, and some carry settle steps marked `extension`) and 8 author-designed Q1/Q2 cases;
  - 13 fault-control expectations.
- `vectors/storequeue-transitions.json`: the state table used by the runner.

Literal vectors preserved: 61507, 68, 246028, 307535, 246300, 61779, 184793, 3936448, 3997955, 123014, 300000, 53972, 300068, 300067, 238493, 238561, 4109/68/67/66/66, 4177, 4244, 4310, 4376, 135, 199, 133, 987133, 987137, and the final quota totals.

## Unresolved items

These are P items, not new policy. See `annex/Q4-REFERENCE-EXTENSION.md` §5 for the full list:
- P-Q1-1 to P-Q1-5;
- P-Q2-1 to P-Q2-3;
- P-Q3-1, P-Q3-2;
- P-Q4-1.

No blocker.
