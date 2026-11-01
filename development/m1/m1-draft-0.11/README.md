# M1 Draft 0.11: C24 repair of 0.10 (author turn 009)

- **For Codex review. Not approved.**
- Nothing was executed by the author.
- No memory notes, installs, deployments, transactions, merges, subagents or shell were used.
- Only new files under `m1-draft-0.11/` were written. 0.10 and everything earlier were read only.

## Files

| File | Content |
|---|---|
| `M1-SPEC-0.11-AMENDMENT.md` | R11-01: the 7 counterexamples, the repair, P-C24-2/3/4, RF-8/9 |
| `M1-STATUS-0.11.md` | coverage (unchanged until review; proposed 34/7/0), preserved evidence |
| `tools/sr_ref.py` | drop-in repaired SR-model: loads 0.10 read-only, rebinds its helpers, replaces `World.epochs`; exports `self_check()`, `load_pristine_010()` |
| `tools/run_checks_011.py` | runner |
| `vectors/c24-regressions.json` | probes, names, generators, values, World scenarios |

## Command (not run by the author)

From `D:\PoCol-Development`:

```
coordination\runtime\python311\python.exe m1-draft-0.11\tools\run_checks_011.py
```

It writes only `m1-draft-0.11/results/run-results-0.11.json` and exits 1 on any failure.

It re-runs the entire 0.10 suite with the repaired module: 2 × 729 h5 runs and 520 BR22a combinations included. Expect a runtime of several seconds or more.

## Using the repaired module in independent probes

To use it, put `m1-draft-0.11/tools` first on `sys.path` and `import sr_ref`, exactly as with 0.10. `sr_ref.self_check()` shows that `World` uses the repaired helpers.
