# M1 Draft 0.30: C33/C34 repairs and the explicit conventions (author turn 028)

**For root review. Not approved. Nothing was executed by the author.**

- Only new files under `m1-draft-0.30/` were written.
- Earlier tools are loaded unchanged by path:
  - the 0.27 checker;
  - the 0.29 overlay models;
  - the 0.25 viewer;
  - the 0.4 strict parser;
  - the 0.2 validator.

| File | Content |
|---|---|
| `M1-SPEC-0.30-AMENDMENT.md` | C34, C33, the conventions, the freeze and the gate |
| `M1-STATUS-0.30.md` | rows, the exact remaining root work, the rebound failures |
| `M1-DASHBOARD-0.30-AR.md` | plain Arabic summary |
| `decisions/conventions-applied-0.30.json` | the 17 conventions, verbatim, with citations and evidence |
| `audit/acceptance-inventory-0.30.json` | 0.29 binding, changed rows, history, proposed statuses |
| `vectors/c33-guard-cases.json` | 60 receive-guard cases, plus the replay rules |
| `vectors/c34-u08-invariants.json` | per-case sum invariants, mutants, rebound failures |
| `vectors/experiment-amendments-0.30.json` | X-U14 (shared budget), new X-C33 |
| `tools/guard_ref_030.py` | guarded HeaderNetCheck adapter |
| `tools/run_checks_030.py` | the runner (root only) |

## Command (root only)

```
coordination\runtime\python311\python.exe m1-draft-0.30\tools\run_checks_030.py --root <tree> --out <dir>
```

The runner writes only into `--out`, and an earlier file is never overwritten. It writes:
- `run-results-0.30.json`;
- `acceptance-matrix-0.30.json`;
- `decision-register-0.30.json`;
- `hash-freeze-0.30.json`;
- `bindings-0.30.json`;
- `dashboard-0.30-ar.json`.
