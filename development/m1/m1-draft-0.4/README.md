# M1 Specification Draft 0.4: Review Package (author turn 002)

Status:
- **For Codex review. Not approved. Phase S (specifications) only.**
- No production code; nothing installed or deployed; no transactions; no subagents.
- **Nothing in 0.4 was executed by the author.** The session had file tools only.

Preserved: draft 0.1, `../m1-draft-0.2/`, `../m1-draft-0.3/`, `../reference/` and `../coordination/` were not modified. Only new files under `m1-draft-0.4/` were written. Unchanged material is inherited by reference (`M1-SPEC-0.4-AMENDMENTS.md`, inheritance paragraph).

## Read in this order

1. `M1-SPEC-0.4-AMENDMENTS.md`: R4-01 … R4-08.
2. `CR-M1-01-STATE-PROOF-READS.md`: the concrete addendum for BLK-01.
3. `M1-STATUS-0.4.md`: dispositions C10–C13, the inventory delta, blockers BLK-01–03, pending owner decisions, and reviewer items U31–U37.
4. `annex/`:
   - `bridge-matrix.json`: B3 schemas, B5 deny list;
   - `dependency-rules.json`: B7;
   - `recv-limits.json`: R1, plus the CR-M1-01 bound;
   - `addendum-x3-proposal.json`: ADD-M1-01, unsigned.

## Files written

| Path | Revision |
|---|---|
| `README.md`, `M1-SPEC-0.4-AMENDMENTS.md`, `M1-STATUS-0.4.md` | all |
| `CR-M1-01-STATE-PROOF-READS.md` | R4-05 |
| `annex/bridge-matrix.json` | R4-06 (B3, B5) |
| `annex/dependency-rules.json` | R4-06 (B7) |
| `annex/recv-limits.json` | R4-07, R4-05 |
| `annex/addendum-x3-proposal.json` | R4-04 |
| `vectors/snapshot-invariants.json` | R4-01 |
| `vectors/request-bound.json` | R4-03 |
| `vectors/bridge-check-cases.json` | R4-06 (B1, B2, B3) |
| `vectors/dependency-graphs.json` | R4-06 (B7) |
| `tools/snapshot_model_04.py` | R4-01 |
| `tools/bridge_ref.py` | R4-06 |
| `tools/run_checks_04.py` | the runner for all of the above |

## Command for Codex (not run by the author)

From `D:\PoCol-Development`, with the workspace's embedded Python:

```
coordination\runtime\python311\python.exe m1-draft-0.4\tools\run_checks_04.py
```

The runner:
- writes only `m1-draft-0.4/results/run-results-0.4.json` and `generated-0.4.json`;
- uses only the standard library;
- needs no `sys.path` setup by the caller;
- loads the unchanged 0.3 and 0.2 modules by path.

Expected duration is tens of seconds: the 2847-chunk and 2848-chunk fixtures use the pure-Python Keccak.

Places where author-only, unexecuted code is most likely to be wrong:
- the literal head bytes and the 65535-byte length of `rb-max-2847`;
- the JSON-escaping of `raw` cases in `bridge-check-cases.json`;
- the field-order rules in `bridge_ref._object_error`;
- the construction that splits a surrogate pair.

A failure is a finding against 0.4.
