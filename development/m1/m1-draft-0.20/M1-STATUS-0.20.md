# M1 Draft 0.20: Status

**Nothing in 0.20 was executed by the author.**

## Coverage

- **Accepted:** 0.18, at 38 Partial, 3 Not started, 0 Complete.
- **Proposed after a successful root review and test:** V2 Partial, giving **39 Partial, 2 Not started (V3–V4), 0 Complete**.
- **Why V2 stays Partial:** P-V2-1..10 and the definition gaps DG-V2-1..4 are open. The experiments are specified, not executed, which M1 permits.

## Expected on root's run

- Zero FAIL.
- `C29.new.*` pass for all 25 controls.
- `C29.oldBehaviourAsDerived.*` pass, including the root false green.
- `coverage020.*` pass: 15 exact sink controls and 43 strict SinkChecker runs.
- Every `suite019.*` 0.19 entry passes under the strict checker.

## Open (recorded)

- P-V2-1..8 (from 0.19).
- P-V2-9: the sink grammar.
- P-V2-10: expanded literal replies.
- DG-V2-1..4.
- All earlier gaps, including RF-E6-1 (owner question).

## Unchanged

- U01, U02, U10, U14 and CR-M1-01 open.
- CR-E4-01/02 remain proposals.
- CONF_DEPTH required; full gate; no merge or deploy.
