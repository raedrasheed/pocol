# M1 Spec 0.22 Amendment: C30 parser-depth repair and E05 hash supplement

**For Codex review. Not approved. Nothing was executed by the author.**

## Basis

- **Accepted:** 0.20.
- **0.21:** the canonical run gave 110 pass / 29 recorded / 0 FAIL, but C30 is open: the 1500-level nested-list probe raises RecursionError.

## Changes

1. **`tools/iterative_parse.py`.** An explicit-stack framing parser with the 0.21 contract, installed in memory into the unchanged 0.21 model.
2. **`vectors/c30-depth.json`.** 13 hand-derived deep/shallow fixture pairs: top-level, CP leaf, M_0 entry, extra top item, and malformed framing (truncated, crossing, trailing, long form, leading zero, truncated string). Plus 3 `validate`-path profiles.
3. **`vectors/v3-e05-supplement.json`.** The three-library H_GSV1 = `0xde518e30…2277` and the GSV1 input SHA-256.
4. **`tools/run_checks_022.py`:**
   - reproduces the old RecursionError, and shows the old and new parsers agree on all non-deep inputs;
   - runs the C30 decode and `validate` paths and the E05 checks;
   - preserves and classifies the 0.21 copy-path harness failure;
   - replays the full 0.21 suite with a content-hash/role check.

   It writes only `m1-draft-0.22/results/run-results-0.22.json`.

No CP value, limit, trust policy, fixture byte or owner decision changes.
