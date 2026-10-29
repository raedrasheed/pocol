# M1 Draft 0.22: C30 parser depth and E05 (author turn 020)

- **For Codex review. Not approved.**
- Nothing was executed by the author.
- No memory notes, installs, deployments, transactions, merges, subagents or shell were used.
- Only new files under `m1-draft-0.22/` were written.

## Files

| File | Content |
|---|---|
| `M1-SPEC-0.22-AMENDMENT.md` | changes |
| `M1-STATUS-0.22.md` | coverage and expectations |
| `annex/C30-PARSER-DEPTH.md` | defect, repair, fixtures, harness note, E05 |
| `tools/iterative_parse.py` | explicit-stack framing parser |
| `tools/run_checks_022.py` | runner |
| `vectors/c30-depth.json` | deep/shallow fixture pairs and validate-path profiles |
| `vectors/v3-e05-supplement.json` | three-library H_GSV1 |

## Command (not run by the author)

From `D:\PoCol-Development`:

```
coordination\runtime\python311\python.exe m1-draft-0.22\tools\run_checks_022.py
```

It writes only `m1-draft-0.22/results/run-results-0.22.json` and exits 1 on any FAIL.
