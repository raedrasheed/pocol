# LP4-PAY-01 — approved additive native interface

Owner decision (2026-10-17): explicitly approves **option 1 in the immutable
[PAY-DECISION.md](lp4/p2-runtime/codex-r1/PAY-DECISION.md)** as an additive LP4
consensus-interface decision. The six numbered clauses and all listed acceptance
cases are binding together. M1 remains unchanged.

“Never fails” applies only to authorized, validated, funded calls with sufficient
gas. This is a **newly approved interpretation**, not an existing explicit M1 rule.
Invalid caller/encoding/value/mode/recipient/funds/overflow/gas gives an exceptional
call halt; failure in a mandatory system phase invalidates the block.

Implement and verify the complete approved contract, including actual compiled
Registry/Escrow calls, immediate bytecode authorization and own-balance debit,
64-byte encoding, native 5000 gas plus Cancun call charges, reward routing,
closing restrictions, no callbacks/account creation and nested journal rollback.
All original LP4 requirements remain: GST and PAY must both pass before P2 merge;
boot remains disabled. All 23 acceptance ledger obligations remain NOT PASSED
until their individual evidence has been verified. No criterion is narrowed.

The historical proposal and its “not approved” wording are preserved as historical
evidence; this later decision resolves that status. PAY semantics in new code/docs
must link to this approval and the complete immutable contract.
