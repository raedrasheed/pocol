# M1 Draft 0.26: V1 completion and consolidated criteria (author turn 024)

- **For root review. Not approved.**
- Nothing was executed by the author.
- No shell, network, browser, installs, deployments, transactions, merges, subagents or memory notes were used.
- Only new files under `m1-draft-0.26/` were written.
- No private key, private path, transcript, dependency or binary is included.

## Files

| File | Content |
|---|---|
| `M1-SPEC-0.26-AMENDMENT.md` | changes |
| `M1-STATUS-0.26.md` | per-row criteria and exact remaining blockers |
| `M1-DECISION-PACKET-0.26-AR.md` | Arabic owner packet |
| `annex/V1-HEADERNETCHECK.md` | normative V1 annex, with P-V1-1…10 |
| `vectors/v1-network.json`, `v1-chains.json`, `v1-window-cases.json`, `v1-units.json` | literal V1 fixtures |
| `supplements/s26-v1-experiments.json` | deferred V1 experiment definitions |
| `normative/N26-REPRESENTATION.md`, `vectors/n26-representation-cases.json` | CR-E4-01 A, CR-E4-02, CID-1 as normative text and fixtures |
| `owner/U08-CHANGE-REQUEST.md` | U08 routed to the owner |
| `audit/consolidation-0.26.json`, `audit/findings-trace-0.26.json` | consolidated decisions, rows and findings |
| `hash/hash-freeze-plan-0.26.json` | hash-freeze extension |
| `tools/v1_ref.py`, `tools/run_checks_026.py` | V1 reference checker and runner |

## Command (not run by the author)

From `D:\PoCol-Development`:

```
coordination\runtime\python311\python.exe m1-draft-0.26\tools\run_checks_026.py
```

It also runs from a preserved review copy:
- `--root <tree>` (or `POCOL_M1_ROOT`) sets where the inputs are read; by default it is the nearest ancestor holding both `reference/` and `m1-draft-0.2/`.
- `--out <dir>` (or `POCOL_M1_OUT`) sets where outputs go; by default `<package>/results`.

It writes only in the output directory, never overwrites an earlier run, and exits 1 on any FAIL:
- `run-results-0.26.json`;
- `v1-transcripts-0.26.json`;
- `hash-freeze-v1-0.26.json`.

It runs pure-Python PoW nonce searches (about 2.2 million SHA-256 calls), secp256k1 operations and a 10^6-height exhaustive check, so expect about a minute.
