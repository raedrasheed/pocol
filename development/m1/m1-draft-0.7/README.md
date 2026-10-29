# M1 Specification Draft 0.7: Review Package (author turn 005)

Status:
- **For Codex review. Not approved. Phase S only.**
- No production code, installs, deployments, real transactions, subagents or memory notes.
- **Nothing in 0.7 was executed by the author.** The session had file tools only.

Preserved: all earlier revisions, `reference/`, the recovered source and the coordinator's evidence are unchanged. Only new files under `m1-draft-0.7/` were written.

## Read in this order

1. `M1-SPEC-0.7-AMENDMENTS.md`
2. `M1-STATUS-0.7.md`
3. `annex/snapshot-alternative-E.md`

## Files

| Path | Revision |
|---|---|
| `README.md`, `M1-SPEC-0.7-AMENDMENTS.md`, `M1-STATUS-0.7.md` | all |
| `tools/m1model_07.py`, `vectors/c20-cases.json` | R7-02 |
| `tools/profile_race_ref_07.py`, `vectors/epoch-cases.json` | R7-03 |
| `annex/recv-scopes.json`, `annex/malicioushttp-scripts.json`, `tools/recvguard_ref.py`, `vectors/mr10-cases.json` | R7-04 |
| `tools/session_ref.py`, `vectors/br20-cases.json` | R7-05 |
| `annex/snapshot-alternative-E.md`, `tools/snapshot_e_ref.py`, `vectors/snapshot-e-cases.json` | R7-06 |
| `annex/fetch-rule-criterion.json` | R7-07 |
| `tools/run_checks_07.py` | the runner, including R7-01 |

## Check sequence (not run by the author)

From `D:\PoCol-Development`:

```
coordination\runtime\python311\python.exe m1-draft-0.7\tools\run_checks_07.py
```

- Python standard library only; no Node step is needed for 0.7.
- Writes only `m1-draft-0.7/results/run-results-0.7.json`.
- Imports the 0.2–0.6 tools by path; `m1model_07` patches the in-memory 0.3 model and does not touch the file.
- Expect about a minute: MR4 compresses 1 GiB of zeros lazily to record its length and sha256.

## Most likely failure points

These are author-only code that has not been executed:
- the MR6d feeHistory padding;
- the hand-derived MR10 timings;
- the BR20c model loop;
- the regression of all 0.2 isolation claims under the new path traversal;
- the cache demonstrations that rebind `Fetcher.fetch`.
