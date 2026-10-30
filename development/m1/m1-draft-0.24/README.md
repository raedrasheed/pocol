# M1 Draft 0.24: C31 X8 evaluator integrity (author turn 022)

- **For Codex review. Not approved.**
- Nothing was executed by the author.
- No memory notes, installs, deployments, transactions, merges, subagents, network, browser or shell were used.
- Only new files under `m1-draft-0.24/` were written.
- No private path or transcript is included.

## Files

| File | Content |
|---|---|
| `M1-SPEC-0.24-AMENDMENT.md` | changes |
| `M1-STATUS-0.24.md` | coverage (proposed 41/0/0 after review) |
| `annex/C31-X8-EVALUATOR.md` | defect, contract, controls, C7 per-path plan |
| `tools/x8_eval_v2.py` | repaired evaluator |
| `tools/run_checks_024.py` | runner |
| `vectors/c31-controls.json` | 34 hand-derived controls |
| `vectors/c7-path-plan.json` | supplemental per-path C7 plan |
| `pages-supplemental/C7-P1…P5/` | one page per C7 path (10 files) |

## Command (not run by the author)

From `D:\PoCol-Development`:

```
coordination\runtime\python311\python.exe m1-draft-0.24\tools\run_checks_024.py
```

It writes only `m1-draft-0.24/results/run-results-0.24.json` and exits 1 on any FAIL.
