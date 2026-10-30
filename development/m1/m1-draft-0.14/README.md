# M1 Draft 0.14: C26 checker-only repair (author turn 012)

- **For Codex review. Not approved.**
- Nothing was executed by the author.
- No memory notes, installs, deployments, transactions, merges, subagents or shell were used.
- Only new files under `m1-draft-0.14/` were written.

## Files

| File | Content |
|---|---|
| `M1-SPEC-0.14-AMENDMENT.md` | the defect, the in-memory repair, the evidence plan |
| `M1-STATUS-0.14.md` | coverage (proposed 36/5/0 after review), open items |
| `tools/run_checks_014.py` | loads `m1-draft-0.13/tools/run_checks_013.py` read-only, installs the safe recorder, re-runs everything |

## Inheritance

`run_checks_014.py` loads the 0.13 runner as a module. That runner in turn imports:
- `m1-draft-0.12/tools/sr_ref.py`, the 0.12 strict model;
- `m1-draft-0.13/tools/fmt2_codec.py`;
- `m1-draft-0.13/tools/recovery_ref.py`.

Its `inherited_012` step re-runs the 0.12 suite, which re-runs 0.11, which re-runs 0.10. No 0.13 code, vector or result file is modified.

## Command (not run by the author)

From `D:\PoCol-Development`:

```
coordination\runtime\python311\python.exe m1-draft-0.14\tools\run_checks_014.py
```

It writes only `m1-draft-0.14/results/run-results-0.14.json` and exits 1 on any failure, an aborted step, a non-core status, or a missing expected E4/E5 check.
