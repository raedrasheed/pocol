# M1 Draft 0.29: delegated technical decisions overlay and full gate (author turn 027)

**For root review. Not approved. Nothing was executed by the author.**

- Only new files under `m1-draft-0.29/` were written.
- Every earlier file is reused unchanged and loaded by path, including:
  - the 0.27 checker;
  - the 0.25 viewer model;
  - the 0.2 validator;
  - all saved results and reviews.

| File | Content |
|---|---|
| `M1-SPEC-0.29-OVERLAY.md` | the effective-specification overlay: the nine delegated decisions with sources, alternatives, consequences, gates and hash groups; U14 and CR-M1-01 reconciled; the V1 cost sentence; the conventions pending root decision |
| `M1-STATUS-0.29.md` | the 41 rows, the exact remaining criteria and the historical failures |
| `M1-DASHBOARD-0.29-AR.md` | plain Arabic summary |
| `decisions/adopted-decisions-0.29.json` | machine-readable register: authority, the nine decisions, reviewer tokens, pending conventions |
| `audit/acceptance-inventory-0.29.json` | row and finding deltas, historical FAIL rebinding, experiment-definition map, proposed statuses |
| `vectors/deadline-cases-0.29.json` | virtual-time deadline fixtures: HeaderNetCheck (real checker, literal replies), content load, BUD re-expression, RP pair, 0.28 binding, rerun profiles |
| `vectors/u08-canonical-split-0.29.json` | boundary lengths, noncanonical splits, 6-key proof assumptions |
| `vectors/u02-revoked-viewer-0.29.json` | revoked default versus explicit view, with and without a click |
| `vectors/u10-publisher-transfer-0.29.json` | old-publisher transfer and removal |
| `vectors/rfe61-x12b-binding-0.29.json` | x12b total 2 bound to the executed 0.18 results |
| `vectors/experiment-amendments-0.29.json` | amended E01, E03 and E07; new Phase A definition X-U14 |
| `tools/overlay_ref_029.py` | reference models: virtual deadline, content-load engine, U08 contract arguments, U02 actions, U10 roles |
| `tools/run_checks_029.py` | the runner (root only) |

## Command (root only)

```
coordination\runtime\python311\python.exe m1-draft-0.29\tools\run_checks_029.py --root <tree> --out <dir>
```

The runner writes only into `--out`, and an earlier file is never overwritten. It writes:
- `run-results-0.29.json`;
- `acceptance-matrix-0.29.json`;
- `decision-register-0.29.json`;
- `hash-bindings-0.29.json`;
- `deadline-rerun-0.29.json`;
- `dashboard-0.29-ar.json`.

It rebuilds no header, nonce or share. Expect a run of a few minutes, mostly pure-Python signature recovery while the 0.27 transcripts are replayed under three latency profiles.
