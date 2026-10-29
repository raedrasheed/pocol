# M1 Draft 0.27: C32 and V1-SHA-COST (author turn 025)

- **For root review. Not approved.**
- Nothing was executed by the author.
- No shell, network, browser, installs, deployments, transactions, merges, subagents or memory notes were used.
- Only new files under `m1-draft-0.27/` were written.
- The 0.26 package and its tools are unchanged.

## Files

| File | Content |
|---|---|
| `M1-SPEC-0.27-AMENDMENT.md` | changes |
| `M1-STATUS-0.27.md` | criteria and exact blockers |
| `M1-DECISION-PACKET-0.27-AR.md` | Arabic owner options and recommendations |
| `annex/C32-RPC-ERROR-ENVELOPE.md` | defensive envelope and retry-delay rule, P-C32-1…4 |
| `annex/V1-SHA-COST.md` | all-SHA-256 accounting; caching analysis; discrepancy |
| `owner/V1-SHA-COST-CHANGE-REQUEST.md` | owner change request (alternatives A–E, recommendation A) |
| `vectors/c32-error-cases.json` | 41 literal reply-text cases |
| `vectors/v1-sha-accounting.json` | per-case SHA-256 accounting, plus the share fixtures S1, S1bad and W |
| `audit/consolidation-0.27.json` | carry-forward and row changes |
| `tools/v1_ref_027.py`, `tools/run_checks_027.py` | repaired reference checker and runner |

## Command (root only, in a preserved review copy)

```
coordination\runtime\python311\python.exe m1-draft-0.27\tools\run_checks_027.py --root <tree> --out <dir>
```

It writes only these files in `--out` (by default `<package>/results`), never overwrites an earlier run, and exits 1 on any FAIL:
- `run-results-0.27.json`;
- `v1-transcripts-0.27.json`;
- `hash-freeze-v1-0.27.json`.

It runs pure-Python share searches for 14 headers × 256 shares, so allow a few minutes.
