# M1 Spec 0.16 Amendment: rows E6 and E7

**For Codex review. Not approved. Nothing was executed by the author.**

## Basis

- **Accepted:** 0.15 as tooling, at 365 pass / 118 recorded / 0 fail; E4/E5 were independently confirmed 30/30.
- **Still open:** owner decisions U01, U02, U10, U14 and CR-M1-01; CR-E4-01 and CR-E4-02 remain proposals; CONF_DEPTH, the full gate, and no merge or deploy are unchanged.

## What 0.16 adds

1. **E6 normative text** (`annex/E6-E7.md` §2–3):
   - DiskLedger: admission, synchronous reserve, ticket life with settleSeq, serialized refresh, zero remove ticket, tomb byte exemption with the names and META caps;
   - the AdminDelete FSM (table T1–T12), the D111 guard, and AdminSlots ownership released only by settle;
   - the outcomes deleted, failed, uncertain and adminBusy, the 45 s bound, and lemmas L-E6-1..3.
2. **E7 normative text** (§4):
   - sitesLive with pins, creating operations, and lateGens/LATE_SITES dropped only through a refresh issued after expiry;
   - admission, and the exact `retryAfterMs` rule;
   - TombReaper;
   - the conditional sites lemma L-E7-1 (A15c, A15d).
3. **Models** (stdlib, manual clock, FakeBackend):
   - `tools/sites_ref.py`: SiteLedger, with the faults noLateSites, dropPinned, seatOnCounted, epochReserveOmitted, diskNoReserve and noDiskGate.
   - `tools/admin_ref.py`: AdminEngine, which subclasses the read-only 0.13 engine. It adds the faults adminNoCancel, adminStaleIssue, adminEarlyRelease, deleteUnordered and recoveryNoCancel.
4. **Hand-derived goldens:**
   - `vectors/e6e7-ledger.json`: BR23 d1, d8–d15c, the dropPinned broken path, x6 sites, d2/d6 rules, and d4/d13 properties;
   - `vectors/e6-admin.json`: BR24 x1–x12, x12b, BR22f-f5, d5, and the E6 units;
   - `vectors/e6e7-adapters.json`: d3 (0.12 World) and d7 (0.13 codec);
   - `vectors/e7-assumption-violations.json`.
5. **Runner** `tools/run_checks_016.py`:
   - a safe positional-only `record`/`check`;
   - a read-only replay of the 0.15 suite (`main()` not called; old result files hashed before and after);
   - goldens and faulty-path goldens, controls with witness cells, enumerations (256 / 729 / 9), event audits, and assumption-violated cases;
   - a coverage assertion for every historical variant and control.

   It writes only `m1-draft-0.16/results/run-results-0.16.json`.

## Model conventions and findings

- **Conventions:** P-E6-1..5 and P-E7-1..5 (annex §8) are labelled assumptions, not approvals.
- **RF-E6-1:**
  - The total `adminStaleDropped` in x12b is 2 in the model; the literal is 1.
  - The difference is T2b, which the literal timeline omits.
  - The runner checks the op1 count (1) and records the total as a partial gap.
  - This is a baseline conflict for owner/root. No baseline text is changed.
- **RF-E7-1:** the d1 reaper issues at 300 per the literal; D113's scan at every refresh would allow 250. Both are shown, and the outcome is the same.
- **RF-E7-2:** the d13 premise that every dead generation leaves two pending creating operations is not guaranteed under the literal try schedule. Witnesses are recorded.

## Explicit gaps

- BR22a sweepMax control (not implemented in the E3 World).
- d15c boundary cells at 10100 and 60300 (E5 semantics from 0.13).
- d13 is checked as properties only.
- BR22c (Chrome + CDP) is not executed.
