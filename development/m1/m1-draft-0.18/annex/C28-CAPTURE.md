# Annex C28: observation-time capture (M1 draft 0.18)

**Proposed for review. Not approved. Nothing here was executed by the author.**

## Defect

This is a checker defect; the model and the goldens are correct.

- `run_checks_016.admin_cells` runs the engine to a row's time and appends `admin_actual(...)` to its cell list. It then runs the **later** events on the same engine.
- Some actuals are live engine objects: `e.violations`, `e.keys[k]['dict']`, and dictionaries held inside snapshot records.
- So when the 6000 violation is appended, the cell already recorded for row 5000 changes from `[]` to that violation. That gave the single 0.17 FAIL, `…adminEarlyRelease.row@5000.violations`.

Deep-copying after `admin_cells` returns would be too late, so 0.18 copies each cell at the moment it is observed.

## Repair (`tools/observe_capture.py`)

The repair is installed into the globals of the **loaded** 0.16 and 0.17 runner modules. Their source files are unchanged.

| Callsite | Risk | Repair |
|---|---|---|
| 0.16 `admin_cells` (rows, final) | future events run between observations | Same algorithm; expected **and** actual deep-copied per cell when observed |
| 0.16 `admin_actual` | any caller (baselineWithoutFault, control_check) holds live objects | Returns a deep copy |
| 0.16 `ledger_cells` | holds live `L.violations` | Copied; it observes once after the run and runs no event inside |
| 0.16 `record` / `check` / `gap` | diagnostics such as `actual=`, `audit=`, `witnesses=` | Positional-only; diagnostics deep-copied at call time |
| 0.17 `record` / `check` | `violations=e.violations` (C27), fifo logs, `dict=` (RF-E6-1) | Same |
| 0.15 replay and nested 0.10–0.13 loggers | accepted; counts checked (365/118/0) | Unchanged |

Unchanged: models, faults, schedules, expected goldens, labels, the source/derived tags, the row at 5000 (zero violations) and the exact violation at 6000.

## Regressions (`run_checks_018.py`)

An adversarial `MutatingEngine` mutates nested state **in place** on each step:
- t=2: appends to a list, updates a dict, appends to a list nested in a dict, and adds a snapshot record that aliases the live dict;
- t=3: mutates a dict inside the violation list and appends to the nested list again;
- t=4: appends another violation and clears the dict.

The checks:
1. Every captured cell equals the value at its own instant. Row 1 is unchanged by later mutation, and row 2 reflects the change.
2. The captured cells are byte-identical (`json.dumps`) after further in-place mutation (`run(4)`).
3. Expected isolation: mutating the fixture objects after capture changes nothing that was captured.
4. Logger isolation: a mutable diagnostic changed after the call leaves the entry unchanged.
5. **Control:** the pristine 0.16 `admin_cells`, loaded separately and never patched, mismatches on `row@1.violations`, `row@1.dict` and `row@2.dict`. This shows the regression detects the defect.
6. The patched `admin_actual` returns an independent object.

## Coverage

- The whole 0.17 scope is replayed from `run_checks_017.py` without calling its `main()` or writing its results. That scope covers C27, RF-E6-1, the conventions and `coverage017`, and includes the whole 0.16 suite and the read-only 0.15 replay. Its entries carry the prefix `suite017.`.
- Per-row history for x9 under the fault: `row@5000.violations = []`, `row@6000.violations = [{t:6000, count:3}]`, `final.violations = [same]`. Each must pass with expected equal to actual.
- `coverage018` requires the x9 rows and final cells, the C27 and RF-E6-1 assertions, the 0.15 counts, and the 0.17 coverage guards to be present and passing.
- `RF-E6-1.x12b.literalTotal` must still be an open partial gap.
- Preserved result files (0.13–0.17, including the review copies) are hashed before and after.

## Unchanged

- C27 stays closed.
- RF-E6-1: the source conflict and options A, B and C are not approved. The reviewer recommends a supplemental source correction that keeps the fault global; that is noted, not adopted.
- Owner decisions, the full gate and CONF_DEPTH are unchanged.
