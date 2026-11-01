# M1 Draft 0.13: annex E4–E5, storage recovery (author turn 011)

- **For Codex review. Not approved.**
- Nothing was executed by the author.
- No memory notes, installs, deployments, transactions, merges, subagents or shell were used.
- Only new files under `m1-draft-0.13/` were written. Earlier packages were read only.

## Files

| File | Content |
|---|---|
| `M1-SPEC-0.13-AMENDMENT.md` | R13-01 to R13-03, open items |
| `M1-STATUS-0.13.md` | proposed coverage 36 Partial / 5 Not started / 0 Complete |
| `annex/E4-E5-RECOVERY.md` | FSM, slots, SeqAlloc, fmt-2 codec, limits, DiskLedger, BR22d–g tables, P items and findings |
| `vectors/e4-units.json` | codec known answers (hex/base64), rejects, surrogate collision, max record, DiskLedger |
| `vectors/e5-schedules.json` | BR22d/e/f/g cases with hand-derived rows and controls |
| `tools/fmt2_codec.py` | strict fmt-2 codec reference |
| `tools/recovery_ref.py` | RecoveryGate/slots/DiskLedger engine over a FakeBackend (uses the 0.12 strict helpers) |
| `tools/run_checks_013.py` | runner |

## Command (not run by the author)

From `D:\PoCol-Development`:

```
coordination\runtime\python311\python.exe m1-draft-0.13\tools\run_checks_013.py
```

It writes only `m1-draft-0.13/results/run-results-0.13.json` and exits 1 on any failure. It includes the in-memory re-run of the 0.12, 0.11 and 0.10 suites.
