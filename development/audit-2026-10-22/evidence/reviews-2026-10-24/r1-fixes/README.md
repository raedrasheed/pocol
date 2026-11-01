# Fixes for the caps review (R1)

- **R1-1 (MEDIUM): builder starvation.** `template_txs` executes each candidate, in order, on a running copy of the parent state (`execute_on_validated` with the checked code total). It skips one that fails validation, together with the rest of its sender's transactions. A transaction that depends on a skipped one, such as one paid from its transfer, is therefore never offered.
  - Test: `caps::the_builder_skips_the_dependents_of_a_skipped_transaction`, with 9 dependents, one more than the builder's 8 retries.
  - On `2ce03c1` it fails: the builder offers the 9 unexecutable transfers and not bob's (`r1-1-before-2ce03c1.log`).
- **R1-2 (MEDIUM): LOG_CAP was not pinned.** The reviewer's witness is committed as `tests/log_cap_scope.rs` (caps-test build). It covers accumulation inside a block, the per-branch scope and a pruned-state rebuild.
  - The reviewer's own runs show it kills mutants L1, L2 and L4 (`reviewer-mutation-results.json`).
- **R1-4 (LOW): over-cap block on an empty parent.** Test: `caps::a_block_over_tx_cap_on_an_empty_parent_is_refused`.
  - With both defences removed at once (the reviewer's D1: item 16 in `chain.rs` and the USER budget check in `apply.rs`), it fails (`r1-4-double-mutant.log`).

Suites:
- pocold: 176 passed, 0 failed, 15 ignored (`pocold.log`).
- caps-test build: log_cap 2/2, log_cap_scope 1/1, lp4 caps_identity 3/3.

Still open from R1:
- R1-3 (LOW): template locking against TXBYTES; evidence under item 16.
- R1-5 (NOTE): consensus.md:101 "the peer is punished" is not implemented.
- R1-6, R1-7: text.
