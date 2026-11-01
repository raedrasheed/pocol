# M1 Draft 0.20: C29 SinkChecker grammar repair (author turn 018)

- **For Codex review. Not approved.**
- Nothing was executed by the author.
- No memory notes, installs, deployments, transactions, merges, subagents or shell were used.
- Only new files under `m1-draft-0.20/` were written.

## Files

| File | Content |
|---|---|
| `M1-SPEC-0.20-AMENDMENT.md` | changes |
| `M1-STATUS-0.20.md` | coverage, open items |
| `annex/C29-SINK-GRAMMAR.md` | defect, closed grammar, controls, deferred experiments, definition gaps |
| `tools/sink_checker_strict.py` | strict SinkChecker |
| `tools/run_checks_020.py` | runner |
| `vectors/c29-controls.json` | 25 controls with strict and pristine expectations |
| `vectors/v2-deferred-experiments.json` | TS-LC, BR10, RG3, RG3-bridge, RG3b, RG9 specifications; DG-V2-1..4 |

## Command (not run by the author)

From `D:\PoCol-Development`:

```
coordination\runtime\python311\python.exe m1-draft-0.20\tools\run_checks_020.py
```

It writes only `m1-draft-0.20/results/run-results-0.20.json` and exits 1 on any FAIL.
