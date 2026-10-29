# M1 Draft 0.16: Status

**Nothing in 0.16 was executed by the author.** Read with `../m1-draft-0.15/M1-STATUS-0.15.md`.

## Coverage

- **Accepted:** 36 Partial, 5 Not started (E6, E7, V2–V4), 0 Complete.
- **Proposed after a successful root review and test:** E6–E7 move to Partial. That gives **38 Partial, 3 Not started (V2–V4), 0 Complete**.
- **Not M1 completion.** E6–E7 stay Partial because:
  - the evidence is model-only;
  - A15c/A15d are unmeasured (BR22c);
  - the explicit gaps below remain;
  - there is no TypeScript implementation.

## Evidence plan (for root)

- Run `run_checks_016.py`. It must report:
  - zero FAIL;
  - `rerun015.countsMatchAccepted` (365/118/0);
  - `rerun015.oldResultFilesUnchanged`;
  - every `coverage.*` entry as pass or as an explicit partial gap.
- Review the hand-derived goldens against the cited lines. Every case carries a provenance check for its literals.

## Explicit partial gaps (recorded, never green)

1. `BR22a-sweepMax`: control not implemented.
2. `BR23-d15c.boundary@10100` and `@60300`: E5 cells, covered by 0.13 semantics, not re-run here.
3. `BR23-d13.scope`: properties only.
4. `BR24-x12b.literalConflict.RF-E6-1`: total adminStaleDropped (literal 1, model 2).

## Open for owner/root

- RF-E6-1, a baseline conflict: accept "op1 count = 1" as the meaning of the x12b literal, or amend the timeline to include T2b.
- RF-E7-1 and RF-E7-2: informational.

## Unchanged

- U01, U02, U10, U14 and CR-M1-01 open.
- CR-E4-01 and CR-E4-02 remain proposals.
- CONF_DEPTH required; full gate; no merge or deploy.
- Not executed: Chrome, CDP, `chrome.storage`, TypeScript, BR22c.
