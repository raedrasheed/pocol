"""Observation-time capture for M1 draft 0.18 (C28 repair). Python standard library only.
REFERENCE / SPECIFICATION FIXTURE TOOLING ONLY. NOT executed by the author.

C28 (coordination/review-001/REVIEW-0.17.md): run_checks_016.admin_cells appends
admin_actual(...) results to its cell list and then runs FUTURE events on the same engine.
Several actuals are live engine objects (e.violations, e.keys[k]['dict'], snapshot dicts), so
an earlier row's recorded actual changes when a later event mutates the engine. Deep-copying
the returned list after admin_cells returns is too late.

Repair, installed into the globals of the LOADED runner modules (their source files are not
touched):
  run_checks_016:
    admin_actual -> returns a deep copy of the original result (every caller is protected)
    admin_cells  -> same algorithm as 0.16, deep-copying expected AND actual at the moment each
                    row/final cell is observed, before any further event runs
    ledger_cells -> deep copy of its result; it observes once after the run and runs no events
    record/check -> positional-only, deep-copy the diagnostics at call time
  run_checks_017:
    record/check -> positional-only, deep-copy the diagnostics at call time (C27/RF-E6-1 diags
                    such as violations=e.violations were live references)
Nothing else changes: models, faults, schedules, expected goldens, labels, statuses.
"""

import copy
import json


def snapshot(value):
    """Deep, independent copy. Falls back to a JSON round trip for anything deepcopy refuses."""
    try:
        return copy.deepcopy(value)
    except Exception:                                                        # never alias on failure
        return json.loads(json.dumps(value, default=str))


def make_safe_capture(sink, make_entry):
    def record(label, core_status, /, **diag):
        sink.append(make_entry(label, core_status, snapshot(diag)))

    def check(label, condition, /, **diag):
        record(label, 'pass' if condition else 'FAIL', **diag)
        return condition
    return record, check


def make_admin_cells(R16, actual):
    """run_checks_016.admin_cells with capture at observation time."""
    def admin_cells(A, e, spec, until):
        out = []
        for row in spec.get('rows', []):
            e.run(row['t'])
            for f, v in row.items():
                if f != 't':
                    out.append(('row@%d.%s' % (row['t'], f), snapshot(R16.admin_norm(f, v)), snapshot(actual(A, e, f, v))))
        e.run(until)
        for f, v in spec.get('final', {}).items():
            out.append(('final.' + f, snapshot(R16.admin_norm(f, v)), snapshot(actual(A, e, f, v))))
        return out
    admin_cells.c28_capture = True
    return admin_cells


def install(R16, R17=None):
    """Patch the loaded modules' globals. Returns the originals (for the pristine-defect control)."""
    orig = {'admin_actual': R16.admin_actual, 'admin_cells': R16.admin_cells, 'ledger_cells': R16.ledger_cells,
            'record16': R16.record, 'check16': R16.check}
    orig_actual = R16.admin_actual

    def admin_actual(A, e, field, want):
        return snapshot(orig_actual(A, e, field, want))
    admin_actual.c28_capture = True

    orig_ledger = R16.ledger_cells

    def ledger_cells(L, exp):
        return snapshot(orig_ledger(L, exp))                                 # observes once; no event runs inside
    ledger_cells.c28_capture = True

    R16.admin_actual = admin_actual
    R16.admin_cells = make_admin_cells(R16, orig_actual)
    R16.ledger_cells = ledger_cells
    R16.record, R16.check = make_safe_capture(R16.results, R16.make_entry)
    if R17 is not None:
        orig['record17'], orig['check17'] = R17.record, R17.check
        R17.record, R17.check = make_safe_capture(R17.results, R17.make_entry)
    return orig
