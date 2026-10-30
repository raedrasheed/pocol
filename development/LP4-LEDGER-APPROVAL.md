# LP4-LEDGER-01 — owner approval of option 1

Authority: [OWNER-STANDING-AUTHORIZATION.md](OWNER-STANDING-AUTHORIZATION.md)
(2026-10-21: "approve the recommended option for every decision").

Decision record: `development/lp4/p4-genesis/codex-r8/LEDGER-STATE-DECISION.md`.
**Option 1 (recommended) is approved in full**, all eight clauses:

1. Physical v1 layout at C0C003: slot0 `mapping(uint64=>uint256) total`, slot1
   `mapping(uint64=>mapping(address=>uint256)) count`, slot2
   `mapping(uint64=>address[]) owners`, standard 32-byte Solidity addressing.
2. `Registry.recordShares(uint64,address[])` is the one SYSTEM message of the
   recordShares phase; Registry makes a nested CALL to `Ledger.record`, which
   accepts CALLER=Registry only; `Ledger.pruneShares()` accepts SYSTEM only.
3. Parent-only content: the address sequence is the committed shareList owners in
   nonce order followed by the implicit winner once; Registry updates lastShare=j
   once per existing distinct record; absent all-zero records keep their count
   without creating a record.
4. Pruning: K=max(D,W_w); `pruneShares()` at h deletes exactly j=h−K−1 when h>K+1.
5. Ledger genesis storage is empty; nonzero alloc storage at C0C003 is rejected.
6. Settlement/assignment read actual Ledger facts; M1 formulas unchanged.
7. No resource waiver; all caps and the approved 10/2/10 Registry ceilings stay.
8. Acceptance remains separate from approval.

Option 2 is not selected. The historical proposal file is unchanged.
