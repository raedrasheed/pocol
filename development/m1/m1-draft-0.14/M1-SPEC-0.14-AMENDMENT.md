# M1 — Draft 0.14: Amendment R14-01 (C26 checker-only repair)

**Status:** review draft, **NOT approved**. **Nothing in 0.14 was executed by the author.**

- Only `m1-draft-0.14/` is new. The 0.13 package and its preserved result, and every older file, are unchanged.
- The last accepted batch is still 0.12.

## C26 (coordination/review-001/REVIEW-0.13.md)

`m1-draft-0.13/tools/run_checks_013.py:165` calls `record('e4.surrogateCollision.status', 'recorded', status=sc['status'])` against `def record(name, status, **kw)`.

Python raises a TypeError (two values for `status`) before the function body runs. So `e4_checks` stopped there, and these 0.13 checks **never ran**:
- the parse rejects;
- the base64 rejects;
- the value checks;
- the encode rejects;
- the maximum-record check;
- the disk-ledger boundary and ticket-lifecycle checks.

The 0.13 result was 409 entries: 309 pass, 99 recorded, **1 FAIL**.

## R14-01 repair (`tools/run_checks_014.py`)

1. The 0.13 runner is loaded **as a module**; its `main()` is never called and nothing is written into 0.13. Its module-global `record` is replaced in memory by `safe_record(name, core_status, /, **diag)`, which takes the core status **positionally only**.
2. Diagnostic keywords that collide with core fields are renamed: `status` becomes **`proposalStatus`**, and `check` becomes `diagCheck`.
3. The core `status` is written **last**, from the positional argument, and must be one of `pass`, `recorded`, `FAIL`. A diagnostic can therefore never overwrite it. Simply renaming the parameter, which would let `kw['status']` overwrite the output, was explicitly avoided.
4. `check()` and every 0.13 step resolve `record` through the module globals. The runner asserts this.

**Unchanged:**
- expected values and vectors;
- the codec and engine;
- the schedules;
- CR-E4-01/02 and the Unicode-scalar and u64 assumptions;
- storage policies, gates, slots, budgets, seq and the scheduler;
- owner decisions.

No E6/E7 work.

## Evidence plan

1. **C26 reproduced.** The original 0.13 `record` raises TypeError on that call. The repaired entry keeps `status: recorded` and carries `proposalStatus`.
2. **Entire 0.13 suite re-run.** The run covers:
   - input hashes and wiring;
   - all of `e4_checks`, including the previously skipped portions;
   - all of `e5_checks`, with every case and control;
   - `inherited_012`, and through it the 0.12, 0.11 and 0.10 suites.

   Results are prefixed `rerun013.`.
3. **Schema.** Every result status is in {pass, recorded, FAIL}, and no step aborted (including nested inherited steps).
4. **Coverage proof.** The list of expected E4 and E5 check names is rebuilt independently from the 0.13 vector files, covering every known answer, reject, value, encode, max-record, disk and lifecycle id, and every E5 case and control. Every name must be present, so an omitted check fails the run.
