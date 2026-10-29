# M1 Draft 0.19: V2 LogClient (author turn 017)

- **For Codex review. Not approved.**
- Nothing was executed by the author.
- No memory notes, installs, deployments, transactions, merges, subagents or shell were used.
- Only new files under `m1-draft-0.19/` were written.

## Files

| File | Content |
|---|---|
| `M1-SPEC-0.19-AMENDMENT.md` | additions, source points |
| `M1-STATUS-0.19.md` | coverage (proposed 39/2/0 after review), gaps |
| `annex/V2-LOGCLIENT.md` | normative English annex |
| `tools/logclient_ref.py` | step, LogClient, MockLogServer, SinkChecker, consumers, collectors, Bridge |
| `tools/run_checks_019.py` | runner |
| `vectors/lc-data.json` | branches A/B/A′, literal hashes, RefA/RefB/RefA′ |
| `vectors/lc-step-table.json` | full step table and non-Limit rows |
| `vectors/lc-cases.json` | LC1–LC18 with variants, supplements, controls |
| `vectors/lc-sink-controls.json` | SinkChecker corruption controls |

## Command (not run by the author)

From `D:\PoCol-Development`:

```
coordination\runtime\python311\python.exe m1-draft-0.19\tools\run_checks_019.py
```

It writes only `m1-draft-0.19/results/run-results-0.19.json` and exits 1 on any FAIL.
