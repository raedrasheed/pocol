# M1 Draft 0.17: C27 repair and RF-E6-1 traceability (author turn 015)

- **For Codex review. Not approved.**
- Nothing was executed by the author.
- No memory notes, installs, deployments, transactions, merges, subagents or shell were used.
- Only new files under `m1-draft-0.17/` were written. Older packages, results, the ledger, the GUI, reviews and checkouts are untouched.

## Files

| File | Content |
|---|---|
| `M1-SPEC-0.17-AMENDMENT.md` | changes |
| `M1-STATUS-0.17.md` | coverage (proposed 38/3/0 after review), gaps, open choice |
| `annex/C27-RFE61.md` | C27 full timeline; RF-E6-1 timelines, blocker and options; conventions |
| `tools/admin_trace.py` | AdminFIFO trace subclass of the 0.16 AdminEngine (read-only import) |
| `tools/run_checks_017.py` | runner |
| `vectors/c27-x9-early-release.json` | corrected x9 adminEarlyRelease faulty path (rows tagged source/derived) and FIFO logs |
| `vectors/rf-e6-1-x12-timelines.json` | x12 / x12b / op1-only supplemental timelines, with source and derived cells |

## Command (not run by the author)

From `D:\PoCol-Development`:

```
coordination\runtime\python311\python.exe m1-draft-0.17\tools\run_checks_017.py
```

It writes only `m1-draft-0.17/results/run-results-0.17.json` and exits 1 on any FAIL.
