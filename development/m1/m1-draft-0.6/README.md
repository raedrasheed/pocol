# M1 Specification Draft 0.6: Review Package (author turn 004)

Status:
- **For Codex review. Not approved. Phase S only.**
- No MV3 extension or production module; no installs, deployments, transactions or subagents; no memory notes.
- **Nothing in 0.6 was executed by the author.** The session had file tools only.

Preserved: all earlier revisions, `reference/`, and the coordinator and recovered-source evidence are unchanged. Only new files under `m1-draft-0.6/` were written.

## Read in this order

1. `M1-SPEC-0.6-AMENDMENTS.md` (R6-01 … R6-07).
2. `annex/X3-RESTORED.md`.
3. `M1-STATUS-0.6.md`.

## Files

| Path | Revision |
|---|---|
| `README.md`, `M1-SPEC-0.6-AMENDMENTS.md`, `M1-STATUS-0.6.md` | all |
| `vectors/c14-head.json` | R6-01 |
| `vectors/id-integer-cases.json`, `tools/id_oracle_06.cjs`, `tools/bridge_ref_06.py` | R6-02 |
| `annex/X3-RESTORED.md`, `annex/x3-restored.json` | R6-04 |
| `tools/profile_race_ref.py`, `vectors/profile-race-cases.json` | R6-03, R6-04 |
| `tools/dnr_ref.py`, `vectors/dnr-cases.json` | R6-04 |
| `vectors/br19-tables.json`, `tools/bridgeref_06.py` | R6-05 |
| `annex/br16-generator.json`, `tools/br16_gen.py`, `tools/br16_gen.cjs` | R6-06 |
| `tools/url_oracle_06.cjs` | the 0.6 URL oracle (writes only to 0.6) |
| `tools/run_checks_06.py` | the runner |

## One check sequence (not run by the author)

From `D:\PoCol-Development` (Node 22; embedded Python 3.11):

```
node m1-draft-0.6\tools\url_oracle_06.cjs
node m1-draft-0.6\tools\id_oracle_06.cjs
node m1-draft-0.6\tools\br16_gen.cjs
coordination\runtime\python311\python.exe m1-draft-0.6\tools\run_checks_06.py
```

- Outputs go only to `m1-draft-0.6/results/`: `url-oracle-0.6.json`, `id-oracle-0.6.json`, `br16-node.json` and `run-results-0.6.json`.
- If any Node output is missing, the dependent checks FAIL.
- The 0.2–0.5 tools are imported by path and are not modified.
- Expect one to a few minutes: the 2848-chunk manifest, and the 10⁴ BR16 messages, about 12 MB of text in total.

## Most likely failure points

These are author-only code that has not been executed:
- equality of the Python and JS BR16 streams: any divergence in PRNG draw order or serialization shows up as a hash mismatch, which is the purpose of the check;
- the profile-race expectations, especially the event order in Q9b and Q9c;
- the hand-derived BR19c synthetic counts (52/8; 4 forwarded and 1 pending);
- `4294967295.99999999999` rounding, which is expected to match between Node and Python.
