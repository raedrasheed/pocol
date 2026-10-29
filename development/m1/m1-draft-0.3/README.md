# M1 Specification Draft 0.3: Review Package (author turn 001)

Status: **for Codex review. Not approved, not implementation-ready.**
- No production contract, node or extension code was written.
- Nothing was installed or deployed, and no transactions were sent.
- **Nothing in this package was executed in this turn.** The author session had file read, write and search tools only; it had no command execution. Every check below is written and has not been run.

Baseline: `../reference/FINAL_DESIGN.md`, approved design ID `e8a19ecb…a851aa0`. It is unchanged; see R3-09 on the ID versus the raw SHA-256.
Preserved unchanged: the draft 0.1 files, `../m1-draft-0.2/`, `../reference/` and `../coordination/`. This turn wrote only new files under `m1-draft-0.3/`. The ledger was not edited.

## Read in this order

1. `M1-SPEC-0.3-AMENDMENTS.md`: the normative changes to draft 0.2, R3-01 … R3-10.
2. `M1-DISPOSITIONS-0.3.md`: C01–C09 with revision, evidence and proposed status, plus corrections to the 0.2 evidence claims.
3. `M1-OPEN-0.3.md`: genuine blockers BLK-01 and BLK-02, new decisions U23–U26, owner questions.
4. `M1-ANNEX-INVENTORY-0.3.md`: 41 rows with defining baseline lines, dependencies, the acceptance split and the batch plan.

## Files written in this turn

| Path | Revision |
|---|---|
| `README.md` | — |
| `M1-SPEC-0.3-AMENDMENTS.md` | R3-01 … R3-10 |
| `M1-DISPOSITIONS-0.3.md` | C01–C09 |
| `M1-OPEN-0.3.md` | BLK-01, BLK-02, U23–U26 |
| `M1-ANNEX-INVENTORY-0.3.md` | R3-07, R3-08 |
| `vectors/rlp-adversarial.json` | R3-01 (C03) |
| `vectors/selector-cases.json` | R3-02 (C04) |
| `vectors/fetch-requests.json` | R3-03 (C05) |
| `vectors/snapshot-mock-scenarios.json` | R3-04 (C06) |
| `vectors/version-width.json` | R3-05 (C09) |
| `vectors/annex-batch1.json` | R3-07 (C01) |
| `tools/rlp_strict.py` | R3-01: iterative, parent-bounded decoder |
| `tools/m1model.py` | R3-03, R3-05: request counting, session ChunkCache, u8 version |
| `tools/m1paths.py` | R3-02: lexical selector |
| `tools/snapshot_model.py` | R3-04: abstract proof-bound and detector models |
| `tools/run_checks_03.py` | Runs all 0.3 checks plus regressions of the 0.2 fixtures |
| `tools/rerun_02.py` | R3-10: re-runs the unchanged 0.2 checks into `results/rerun-0.2/` |
| `tools/design_id_probe.py` | R3-09: tests candidate derivations of the design ID (read-only) |

`results/` does not exist yet; the scripts create it.

## Commands (not yet run)

From `D:\PoCol-Development`, using the embedded interpreter in `coordination/runtime` (the `py` launcher points to a missing MiniConda3):

```
coordination\runtime\python311\python.exe m1-draft-0.3\tools\run_checks_03.py
coordination\runtime\python311\python.exe m1-draft-0.3\tools\rerun_02.py
coordination\runtime\python311\python.exe m1-draft-0.3\tools\design_id_probe.py
```

Outputs:
- `m1-draft-0.3/results/run-results-0.3.json`, `generated-0.3.json`;
- `results/rerun-0.2/results/run-results.json`;
- `results/design-id-probe.json`.

`rerun_02.py` regenerates the 0.2 fixtures twice in temporary directories; it took about 100 s in 0.2. `run_checks_03.py` includes a loop over 10⁶ values and the 65536-byte deep decode, so expect seconds to minutes. `design_id_probe.py` hashes every reference file with the pure-Python Keccak, so it may take several minutes.

Because none of this has been run, failures are possible in the new tooling itself. A failure is a finding about this draft, not about 0.2. Specific risks:
- the fixed-point construction in `constructed_requests`;
- the literal head and tail bytes of the deep fixtures;
- the hand-derived request counts.

## What the evidence would and would not show once run

**Would show:**
- the 0.3 model's behaviour on every new fixture;
- that every 0.2 fixture keeps its result under the 0.3 model;
- that the 0.2 decoder differs on the stated precedence cases and fails on deep input;
- that the no-cache and separate-cache mutants are detected;
- the abstract snapshot behaviour.

**Would not show:**
- any implementation;
- EVM, gas, compiler, browser or pocold behaviour;
- MPT proof verification (the snapshot model is abstract);
- agreement with K1–K3;
- the derivation of the design ID unless the probe finds a match.

## Requests to Codex

1. Run the three commands and attach the outputs.
2. Review R3-01 P33 (precedence order) and U23 (no depth rule) against the baseline text.
3. Check the hand-derived counts in `fetch-requests.json` independently of the model.
4. Challenge BLK-01. Are FD:L1274 and FD:L4934 read correctly? Is P35 compatible?
5. Confirm BLK-02 by an independent search for the definitions of X3.
