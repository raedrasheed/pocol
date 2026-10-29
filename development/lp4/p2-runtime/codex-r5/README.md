# PAY-02 native component evidence — P2 NOT PASSED

Parent published PR24 head: ddb26352d803c099bf3fbac0d9c1d5d0b71a4102.
Foundation unchanged: c0ef36c68fb83f9474de7e70fe61973b466647a8.
Authority: complete owner approval in development/LP4-PAY-LIFECYCLE-APPROVAL.md.
Historical R4 decision proposal/witnesses are preserved, including the positive
PAY-01 lifetime gap. This implementation intentionally rejects those calls under
the newly approved rule, never relabels the old witness as an old failure.

Native eligibility uses private immutable complete pre-transaction account facts,
not caller flags. Balance-only, empty and newly created destinations fail even
for zero; pre-existing nonce-only/code-only destinations succeed. The old-code
SELFDESTRUCT cross-transaction test preserves code under Cancun and remains payable.
The existing transfer/current-state/authorization/gas/rollback checks remain.

Measured controls: 29 Rust tests; 44 independently executed py-evm boundary
comparisons; two independently executed compiled lifetime attacks; 16 reviewed
compiled PAY mutations killed by assertions. First 15-mutation run is also retained.
Comparison fields: exact returned bytes, gas used, source/destination balances and
log count; lifetime controls also compare user balance and actual child absence.
These are synthetic unfunded test accounts, not measurements of full system bounds.

Prior-head GitHub receipts are for ddb2635 ONLY: native job success with all 20661
upstream Cancun controls (19488 state/log roots and 1173 exception-presence checks,
not exact exception/wire equivalence); Windows 16 PASS, 8 BLOCKED BY PLATFORM and
2 NOT RUN. Downloaded ZIP SHA256s were verified before extraction. Do not attribute
those runs to the new source or treat blocked/unrun checks as passing. New-head CI
must be inspected separately after publication.

The full seven-part lifecycle contract remains mandatory. Actual compiled system
admission/custody/bridge/refunds, GenesisBuilder, SV5 fixture, real closing/finalization,
SV6, caps64, R18/R21, persistence/crash/reorg and every old requirement remain
NOT PASSED. P2 remains draft, unmerged; no boot authorization. No M1/LP3 edits.
See REVIEW.md and the acceptance ledger; no partial result is full LP4 acceptance.
