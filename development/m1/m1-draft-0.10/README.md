# M1 Draft 0.10: annex E1–E3, worker-generation recovery (author turn 008)

- **For Codex review. Not approved.**
- Nothing was executed by the author.
- No memory notes, installs, deployments, transactions, merges, subagents or shell were used.
- Only new files under `m1-draft-0.10/` were written. Earlier packages were read only.

## Files

| File | Content |
|---|---|
| `M1-SPEC-0.10-AMENDMENT.md` | R10-01 to R10-04 |
| `M1-STATUS-0.10.md` | coverage: E1–E3 proposed Partial (34 Partial, 7 Not started, 0 Complete) |
| `annex/E1-NAMES-RECORDS.md` | epoch items, record names and values, largest-wins |
| `annex/E2-FENCE.md` | D134 gate, window, sweep, lemmas (A15c/A15d explicit), BR22h |
| `annex/E3-RECOVERY.md` | recovery sequence, lemmas, viewer, BR22a/b, BR22c future specification |
| `annex/E-REFERENCE-AND-FINDINGS.md` | reference model, P items, reviewer findings RF-1 to RF-6, E4–E7 boundaries |
| `vectors/e1-units.json` | E1 unit vectors |
| `vectors/e2-fence-cases.json` | E2 fence, BR22h and BR22b cases with hand-written expectations |
| `vectors/e3-br22a.json` | BR22a axes, criteria, rule tables, controls |
| `tools/sr_ref.py` | SR-model (manual clock, controllable FakeBackend) |
| `tools/run_checks_010.py` | runner |

## Command (not run by the author)

From `D:\PoCol-Development`:

```
coordination\runtime\python311\python.exe m1-draft-0.10\tools\run_checks_010.py
```

It uses only the Python standard library. It writes only `m1-draft-0.10/results/run-results-0.10.json`, exits 1 on any failure, and records the input SHA-256 values. It runs about 3,000 small schedules: 2 × 729 for h5, 520 for BR22a, and the controls.
