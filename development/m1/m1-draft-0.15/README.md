# M1 Draft 0.15: complete C26 logging-boundary repair (author turn 013)

- **For Codex review. Not approved.**
- Nothing was executed by the author.
- No memory notes, installs, deployments, transactions, merges, subagents or shell were used.
- Only new files under `m1-draft-0.15/` were written.

## Files

| File | Content |
|---|---|
| `M1-SPEC-0.15-AMENDMENT.md` | history, repair, evidence plan |
| `M1-STATUS-0.15.md` | coverage (proposed 36/5/0 after review), open items |
| `tools/run_checks_015.py` | runner |

## Inheritance (read-only)

- `run_checks_015.py` loads `m1-draft-0.13/tools/run_checks_013.py` through a temporary load hook that installs a collision-free `record`/`check` in it.
- The 0.13 runner's `inherited_012` loads the 0.12 runner, which loads 0.11, which loads 0.10. Each one is patched the same way as it loads.
- The models used are those runners' own imports:
  - the 0.12 `sr_ref` (on top of 0.11 and 0.10);
  - the 0.13 `fmt2_codec` and `recovery_ref`.
- The 0.14 runner is loaded plainly, only for its expected-name guards and its `make_entry`, which are used to reproduce the 0.14 failure path.

## Command (not run by the author)

From `D:\PoCol-Development`:

```
coordination\runtime\python311\python.exe m1-draft-0.15\tools\run_checks_015.py
```

It writes only `m1-draft-0.15/results/run-results-0.15.json` and exits 1 on any failure, an aborted step, a non-core status, or a missing expected or previously skipped check.
