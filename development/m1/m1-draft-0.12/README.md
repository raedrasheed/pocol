# M1 Draft 0.12: C25 repair, physical epoch-namespace accounting (author turn 010)

- **For Codex review. Not approved.**
- Nothing was executed by the author.
- No memory notes, installs, deployments, transactions, merges, subagents or shell were used.
- Only new files under `m1-draft-0.12/` were written. 0.10, 0.11 and everything earlier were read only.

## Files

| File | Content |
|---|---|
| `M1-SPEC-0.12-AMENDMENT.md` | R12-01: the defect, the repair, the fixtures, the single amended inherited expectation (old vs new) |
| `M1-STATUS-0.12.md` | coverage (unchanged until review; proposed 34/7/0), preserved evidence, open items |
| `tools/sr_ref.py` | drop-in repaired SR-model (0.11 read-only, then the raw-namespace gate and invariant); `self_check()`, `load_pristine_011()`, `load_pristine_010()` |
| `tools/run_checks_012.py` | runner |
| `vectors/c25-namespace.json` | review probe, cases N1–N5b, the amendment record |

## Command (not run by the author)

From `D:\PoCol-Development`:

```
coordination\runtime\python311\python.exe m1-draft-0.12\tools\run_checks_012.py
```

It writes only `m1-draft-0.12/results/run-results-0.12.json` and exits 1 on any failure. It includes the in-memory re-run of the full 0.11 and 0.10 suites.

## Using the repaired module in independent probes

Put `m1-draft-0.12/tools` first on `sys.path` and `import sr_ref`.
