# M1 Draft 0.18: C28 observation-time capture (author turn 016)

- **For Codex review. Not approved.**
- Nothing was executed by the author.
- No memory notes, installs, deployments, transactions, merges, subagents or shell were used.
- Only new files under `m1-draft-0.18/` were written.

## Files

| File | Content |
|---|---|
| `M1-SPEC-0.18-AMENDMENT.md` | changes |
| `M1-STATUS-0.18.md` | coverage and gaps |
| `annex/C28-CAPTURE.md` | defect, callsite audit, repair, regressions |
| `tools/observe_capture.py` | capture wrappers installed into the loaded 0.16/0.17 runner globals |
| `tools/run_checks_018.py` | runner |

## Command (not run by the author)

From `D:\PoCol-Development`:

```
coordination\runtime\python311\python.exe m1-draft-0.18\tools\run_checks_018.py
```

It writes only `m1-draft-0.18/results/run-results-0.18.json` and exits 1 on any FAIL.
