# M1 Draft 0.23: V4 X8 fixtures (author turn 021)

- **For Codex review. Not approved.**
- Nothing was executed by the author.
- No memory notes, installs, deployments, transactions, merges, subagents or shell were used.
- Only new files under `m1-draft-0.23/` were written.
- No private path or transcript is included.

## Files

| File | Content |
|---|---|
| `M1-SPEC-0.23-AMENDMENT.md` | additions |
| `M1-STATUS-0.23.md` | coverage (proposed 41/0/0 after review) |
| `annex/V4-X8.md` | canonical vs historical vs proposed; layers; schema; plan; verdict; precision blocks |
| `pages/C1…C8/index.html`, `main.js` | literal fixture pages (16 files) |
| `vectors/x8-config.json` | configuration table |
| `vectors/x8-synthetic-runs.json` | synthetic logs and hand-derived verdicts |
| `tools/x8_eval.py` | offline verdict evaluator |
| `tools/run_checks_023.py` | runner |

## Command (not run by the author)

From `D:\PoCol-Development`:

```
coordination\runtime\python311\python.exe m1-draft-0.23\tools\run_checks_023.py
```

It writes only `m1-draft-0.23/results/run-results-0.23.json` and exits 1 on any FAIL.
