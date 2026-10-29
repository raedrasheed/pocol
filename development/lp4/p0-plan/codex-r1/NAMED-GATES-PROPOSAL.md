# Missing named LP4 gate definitions — explicit mapping proposal

Status: owner definition decision requested; NOT PASSED. These proposed definitions
are new LP4 test bindings, not recovered original M1 vectors or a change to its rules.
Search evidence: reference/execution.md:106 lists Cancun GeneralStateTests then T2b/T2c;
reference/validation.md:897 and implementation.md:36 list T2/T2b/T2c/T24/T26. Identical
lists appear in FINAL_DESIGN.md:843,3411,4695. No concrete per-label test definitions
were found in those preserved sources. Existing CI1–3/GB1/GB-END/L4s/SV definitions
remain as written; their missing execution results are not passes.

| New proposed gate | Concrete acceptance scope, with no narrowed underlying obligation |
|---|---|
| T2 | Cancun GeneralStateTests vs independent EVM/state roots; enumerate every PoCol exception individually, with replacement tests; never blanket-exclude failing tests. |
| T2b | PAY/transfer deviations: no callback, no implicit account creation, forced-transfer accounting/lock semantics and rollback, correct registered reward routing; actual storage/balance effects. |
| T2c | Execution caps: code/slot/account/dirty/log and transaction count/bytes limits; exact equality and +1; distinguish exceptional halt from block-invalid admission; rejected execution never commits state. |
| T24 | Ordered system phases, SYSTEM_ADDRESS-only calls, actual validated parentResult derivation/reexecution; h1 empty, closeBlock leaves current winner unset, recordResult uses parent only; current winner/share leakage mutation must fail. |
| T26 | H-full10–17 roots/gas/receipt/body acceptance, ordinary/closing/final execution, atomic disk commit/recovery and inverse-diff rollback; invalid roots/gas/body/no-child-of-final leave state/disk unchanged. |

Each gate requires positive, boundary, negative and compiled mutation controls plus
independent comparison of the relevant state effects. General R18/R21, caps64 minimum,
GenesisBuilder, SV6 and real finalization remain separate mandatory gates; this
proposal does not replace or waive any of them. Boot remains disabled.

Decision: approve this mapping as new LP4 test definitions, or provide original
per-label definitions. No case will be called PASS before both definition resolution
and actual execution/evidence. Unblocked canonical trie work may continue meanwhile.
