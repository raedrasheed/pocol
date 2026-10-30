# M1 Draft 0.22: Status

**Nothing in 0.22 was executed by the author.**

## Coverage

- **Accepted:** 0.20, at 39 Partial, 2 Not started (V3–V4), 0 Complete.
- **Proposed after a successful root review and test:** V3 Partial, giving **40 Partial, 1 Not started (V4), 0 Complete**.
- **Why V3 stays Partial:** DG-V3-1, 2 and 4–9 and P-V3-1..7 remain open. DG-V3-3 is resolved by E05.

## Expected on root's run

- Zero FAIL.
- `C30.old.*.deep` reproduces RecursionError, except the two header-truncated cases.
- All `C30.*` deep/shallow, equivalence, profile and repeat checks pass.
- `E05.*` passes.
- Every `suite021.*` entry passes or is recorded, including `suite021.reuse.keccakFrom0.2` under the content-hash/role check, whether canonical or preserved-copy paths are used.

## Next

V4 X8 fixtures, then the final specification completeness audit.

## Unchanged

- U01, U02, U10, U14 and CR-M1-01 open.
- CR-E4-01/02 remain proposals.
- RF-E6-1 owner question pending.
- CONF_DEPTH required; full gate; no merge or deploy.
