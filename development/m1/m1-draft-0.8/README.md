# M1 Draft 0.8: C23 repair (author turn 006)

- **For Codex review. Not approved.** Small repair only.
- Nothing was executed by the author.
- No memory notes, installs, deployments, real transactions, merges or subagents.
- All earlier packages and `coordination/shared-repo` are untouched. Only new files under `m1-draft-0.8/` were written.

Files:
- `M1-SPEC-0.8-AMENDMENT.md` (R8-01)
- `tools/run_checks_08.py`

Command (not run by the author), from `D:\PoCol-Development`:

```
coordination\runtime\python311\python.exe m1-draft-0.8\tools\run_checks_08.py
```

It uses only the Python standard library and writes only `m1-draft-0.8/results/run-results-0.8.json`. Expected: the 0.7 checks plus 7 lineage and path checks, with the alternative-E cases now executed. Any lineage-hash mismatch is reported as a failure.
