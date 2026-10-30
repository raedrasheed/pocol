# M1 Draft 0.21: Status

**Nothing in 0.21 was executed by the author.**

## Coverage

- **Accepted:** 0.20, at 39 Partial, 2 Not started (V3–V4), 0 Complete.
- **Proposed after a successful root review and test:** V3 Partial, giving **40 Partial, 1 Not started (V4), 0 Complete**.
- **Why V3 stays Partial:**
  - DG-V3-1..9 are open, among them the non-literal network genesis bytes, single-library H_GSV1, and the ASERT dependency not exercised;
  - P-V3-1..7 are unapproved;
  - the evidence is model-only.

## Expected on root's run

- Zero FAIL.
- `GSV1.*`, K1–K3, all 32 `neg.*`, all `profile.*` (15 RF variants), all `transport.*` and `coverage021.*` pass.
- `GSV1.hashRecorded` is recorded, not certified.
- `RF2.fixtureKeys3to6MatchKnownAddresses` compares against well-known anvil addresses I wrote from memory. If my recollection is wrong, it fails visibly.

## Unchanged

- U01, U02, U10, U14 and CR-M1-01 open.
- CR-E4-01/02 remain proposals.
- RF-E6-1 owner question pending.
- CONF_DEPTH required; full gate; no merge or deploy.
