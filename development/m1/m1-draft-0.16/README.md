# M1 Draft 0.16: rows E6–E7 (author turn 014)

- **For Codex review. Not approved.**
- Nothing was executed by the author.
- No memory notes, installs, deployments, transactions, merges, subagents or shell were used.
- Only new files under `m1-draft-0.16/` were written. Older packages, results, the ledger, the GUI, reviews and checkouts are untouched.

## Files

| File | Content |
|---|---|
| `M1-SPEC-0.16-AMENDMENT.md` | what is added; findings; gaps |
| `M1-STATUS-0.16.md` | coverage (proposed 38/3/0 after review); open items |
| `annex/E6-E7.md` | limits table, DiskLedger, AdminDelete FSM, sites/TombReaper, lemmas, typed traces, audits, conventions, traceability |
| `tools/sites_ref.py` | E7 SiteLedger model (manual clock, FakeBackend) |
| `tools/admin_ref.py` | E6 AdminEngine, which subclasses the 0.13 engine (read-only import) |
| `tools/run_checks_016.py` | runner |
| `vectors/e6e7-ledger.json` | BR23 goldens, generated rules, d4/d13 properties, controls, gaps |
| `vectors/e6-admin.json` | BR24 x1–x12, x12b, BR22f-f5, d5, E6 units, controls |
| `vectors/e6e7-adapters.json` | d3 (0.12 World), d7 (0.13 codec) |
| `vectors/e7-assumption-violations.json` | A15c/A15d deliberately broken (not evidence) |

## Reuse (read-only)

- `m1-draft-0.13/tools/recovery_ref.py` and `fmt2_codec.py`;
- `m1-draft-0.12/tools/sr_ref.py` (strict and raw-namespace fixes);
- `m1-draft-0.15/tools/run_checks_015.py`: its helpers and load hook only. Its `main()` is replayed step by step, never called.

## Command (not run by the author)

From `D:\PoCol-Development`:

```
coordination\runtime\python311\python.exe m1-draft-0.16\tools\run_checks_016.py
```

It writes only `m1-draft-0.16/results/run-results-0.16.json` and exits 1 on any FAIL. This includes a missing variant, a control that does not mismatch its golden at the witness cells, a changed old result file, or 0.15 counts different from 365/118/0.
