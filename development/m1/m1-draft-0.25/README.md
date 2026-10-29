# M1 Draft 0.25: consolidated M1 specification audit (author turn 023)

- **For root review. Not approved.**
- Nothing was executed by the author.
- No memory notes, installs, deployments, transactions, merges, subagents, network, browser or shell were used.
- Only new files under `m1-draft-0.25/` were written.
- No private path, transcript, key, dependency or binary is included.

## Files

| File | Content |
|---|---|
| `M1-SPEC-0.25-AMENDMENT.md` | changes |
| `M1-STATUS-0.25.md` | acceptance report: satisfied, pending and blocked criteria |
| `M1-DECISION-PACKET-0.25-AR.md` | Arabic decision packet: owner choices, kept separate from reviewer conventions |
| `audit/M1-AUDIT-0.25.md` | readable audit summary |
| `audit/row-inventory.json` | 41 rows with clauses, fixtures, evidence patterns, reviews, experiments, decisions, missing definitions |
| `audit/findings-trace.json` | F01–F26 and the eight Codex corrections |
| `audit/decision-register.json` | owner items, items that need no owner, reviewer items, each tied to its source |
| `audit/gap-registry.json` | classification of every saved partial gap |
| `hash/hash-freeze-plan.json` | hash classes, parameters, blockers and export rules |
| `supplements/s25-*.json` | sweepMax, LogClient literal replies, RG3b, E07 table, E01–E07 specifications |
| `representation/*.json` | CR-E4-01, CR-E4-02, CID-1 with boundary fixtures |
| `tools/run_checks_025.py` | acceptance runner |
| `tools/supplements_ref.py` | reference helpers of the supplements and alternatives |
| `tools/js_number_probe.cjs` | optional Node probe (JSON numbers, TextEncoder, BigInt) |

## Command (not run by the author)

From `D:\PoCol-Development`:

```
coordination\runtime\python311\python.exe m1-draft-0.25\tools\run_checks_025.py
```

It writes only these files under `m1-draft-0.25/results/`, and exits 1 on any FAIL:
- `run-results-0.25.json`. A later run writes `run-results-0.25-rerun-<n>.json` instead of overwriting it.
- `hash-freeze-export-0.25.json`, deterministic.
- `lc-literal-replies-0.25.json`, deterministic.

It reads large files (the 11.8 MB triad TSV) and runs about 3,900 BR22a schedules. Expect a run of a minute or more.

Optional, root only: `node m1-draft-0.25\tools\js_number_probe.cjs` prints how Node reads the boundary values.
