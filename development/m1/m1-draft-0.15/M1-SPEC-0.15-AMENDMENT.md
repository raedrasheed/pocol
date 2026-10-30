# M1 — Draft 0.15: Amendment R15-01 (complete C26 repair of the diagnostic logging boundary)

**Status:** review draft, **NOT approved**. **Nothing in 0.15 was executed by the author.**

- Only `m1-draft-0.15/` is new. The 0.13 and 0.14 packages and their failed results stay preserved.
- The last accepted batch is still 0.12.

## C26 history

| Revision | Failing call | Effect |
|---|---|---|
| 0.13 | `record('e4.surrogateCollision.status', 'recorded', status=…)` | TypeError: E4 stopped at line 165 |
| 0.14 | repaired `record` only; then `check('e4.maxRecord', …, name=len(name))` | TypeError: 9 E4 checks never ran (maximum record, 7 disk boundaries, ticket lifecycle). 463 entries, 347 pass, 113 recorded, 3 FAIL (REVIEW-0.14.md) |

## R15-01 repair (`tools/run_checks_015.py`)

**Signatures.** Both `record(label, core_status, /, **diag)` and `check(label, condition, /, **diag)` take their label, core status and condition **positionally only**, so no keyword can bind to them.

**Renames.** Reserved diagnostic keys are moved:

| Diagnostic key | Becomes |
|---|---|
| `name` | `diagName` |
| `check` | `diagCheck` |
| `status` | `proposalStatus` |
| `ok` | `diagOk` |
| `label` | `diagLabel` |
| `condition` | `diagCondition` |

**Schema.** The core fields `check` and `status` are written **last**, and `status` must be in {pass, recorded, FAIL}.

**Installation.**
- A temporary hook on `importlib.util.spec_from_file_location` wraps `exec_module` for every `run_checks_0*.py` loaded during the run. The 0.13 runner and the nested 0.12, 0.11 and 0.10 runners each get the safe pair installed in **their own module globals**, right after loading and before any step runs. The hook is removed afterwards.
- `e4_checks` and `e5_checks` therefore resolve the safe `check` and `record`. The runner asserts this through those functions' `__globals__`, not through an unused export.
- No older file is edited, and no older `main()` is called.

**Caller scan.** An `ast` scan of the 0.13 to 0.10 runners and of 0.14 records every call to `record`/`check` that passes a reserved keyword. It must find both known C26 call sites: 0.13 `record(status=)` and 0.13 `check(name=)`.

**Unchanged:**
- models, vectors and expected values;
- the codec and engine;
- the schedules and scheduler;
- budgets and gates;
- CR-E4-01/02;
- owner policies.

No E6/E7 work.

## Evidence plan

1. **Pristine reproductions** in controlled copies:
   - the 0.13 `record` status TypeError;
   - the 0.13 `check` name TypeError;
   - the same name TypeError on the 0.14 path (0.14's repaired `record` installed, `check` unchanged).
2. **Boundary tests.** All reserved diagnostic names are injected at once into `check` (true and false) and into `record`. The label, core status and condition stay correct, every diagnostic is retained under its new key, a non-core status is rejected, and both functions have 2 positional-only parameters.
3. **Entire 0.13 suite re-run**, with every step and the nested inherited 0.12 → 0.11 → 0.10 chain. Results are prefixed `rerun013.`.
4. **Guards.**
   - The status enum holds, and no step aborted (nested included).
   - The 0.14 functions rebuild the expected E4 and E5 names from the 0.13 vectors, and every name must be present.
   - **All 9 previously skipped checks must be present.** Their actual statuses are recorded, with no pass assumed.
   - `e4.maxRecord` keeps its diagnostic as `diagName`.
   - The inherited chain reaches 0.10.
