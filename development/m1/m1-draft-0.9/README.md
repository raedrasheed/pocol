# M1 Draft 0.9: annex Q1–Q4, StoreQueue (author turn 007)

- **For Codex review. Not approved.**
- Nothing was executed by the author.
- No memory notes, installs, deployments, real transactions, merges, subagents or shell were used.
- Only new files under `m1-draft-0.9/` were written. Earlier packages were read only.

## Files

| File | Content |
|---|---|
| `M1-SPEC-0.9-AMENDMENT.md` | R9-01 to R9-04, unresolved items |
| `M1-STATUS-0.9.md` | coverage (Q1–Q4 proposed Partial), evidence split, pending decisions |
| `annex/Q-STOREQUEUE-SPEC.md` | Q1, Q2 normative restatement with citations |
| `annex/Q-TRANSITIONS.md` | replyState × ownState diagram and table |
| `annex/Q-CANONICAL-TESTS.md` | BR21a–f scripts (BR21c/d future, not executed) |
| `annex/Q4-REFERENCE-EXTENSION.md` | reference interfaces, oracle method, fault controls, P items |
| `vectors/storequeue-units.json` | UTF-8, `qbytes`, admission, quota, BG1 vectors |
| `vectors/storequeue-cases.json` | schedules with hand-written golden checkpoints, and fault controls |
| `vectors/storequeue-transitions.json` | allowed state changes and stable pairs |
| `tools/storequeue_ref.py` | pure reference model (fake clock, fake backend) |
| `tools/run_checks_09.py` | runner |

## Command (not run by the author)

From `D:\PoCol-Development`:

```
coordination\runtime\python311\python.exe m1-draft-0.9\tools\run_checks_09.py
```

It uses only the Python standard library. It writes only `m1-draft-0.9/results/run-results-0.9.json` and exits 1 on any failure. It records the input SHA-256 values; the author did not compute them.

Everything else it does:
- checks the provenance of each golden block against its cited baseline lines;
- runs every unit vector and every schedule against the golden data;
- checks criterion G and the state table on every run;
- checks that each of the eight faults is detected by its literal case.
