# Owner approval: new CF1–CF6 LP3 core-test definitions

The owner explicitly approved the proposal in S7 codex-r1/CF-DECISION.md on
2026-10-17. This approval is recorded separately; the proposal remains an immutable
historical OPEN snapshot. These are **newly approved LP3 test definitions**, not
recovered original CF definitions, and they do not modify M1.

Approved mapping: CF1 closing height/time trigger; CF2 sticky first close height;
CF3 checked cursor completion bound; CF4 gate2 threshold; CF5 final predicate on
supplied validated parent facts; CF6 parent-final and height ceilings. Concrete
examples and rule citations are in the proposal. Implementations must agree with
an independent integer/boolean oracle, exercise boundaries, and detect mutations
before any PASS claim. S7 codex-r2 records the actual verification results.

Pure APIs evaluate supplied numeric/boolean facts, not provenance of parent state.
Future callers must independently validate parent state and ordinary child linkage.
The predicates do not mutate a cursor, finalize blocks, or provide a certificate.
For an empty record set the cursor-bound helper returns closeH (already done);
invalid RET=0, impossible gate2 threshold subtraction, or an unrepresentable u64
cursor bound return errors. Widened final arithmetic cannot wrap into acceptance.

## Mandatory LP4 execution obligations — NOT RUN / NOT PASSED here

1. Drive real closing/finalization transitions through the actual execution engine;
   verify the first close height is sticky and the ledger state is correct.
2. Mutate the actual cursor over the actual closeH record set, observing <=RET
   records per block, exact completion, and correct root/state persistence.
3. At real finalization verify `pending[F]` is absent in the resulting state,
   including adversarial pending entries and rollback behavior.
4. Verify zero final subsidy through real balance/supply/state changes, and
   reject an otherwise valid child of a final block (T0 V20–V22 execution surface).

Core tests must not imply these execution passes, H-full acceptance, original CF
recovery, production/network acceptance, or boot authorization. Boot remains off.
