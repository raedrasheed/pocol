# Separate sequential review — actual Registry/payment component

Same author, separate review after author implementation and initial execution tests;
not an independent human review. Independent py-evm engine and domain-object model
are separate from the Rust/Solidity implementation and import no tested transition code.

Findings/fixes:
- removeActive initially stored its own word before the caller stored the exited
  status. Fixed before initial execution verification: return the cleared position
  and write the own word once. The unchanged7-write ceiling counts SSTORE operations,
  not only final dirty slots. No extra-write exception is used.
- Complete-state root comparison initially interpreted decimal balance strings with
  the older GST helper's hexadecimal number convention. Failed logs remain. Convert
  balances explicitly to integers; nonce/storage/code checks and final whole roots
  match independently, including reverted transactions and system callers.
- A Rust expanded-test fixture had a conflicting mutable/immutable borrow; its build
  failure is retained and never counted as a passing check. Fixed by reading the old
  word before calling the mutation helper. No assertion was removed.
- Initial mutation classifier demanded the word assertion; three genuine expected
  assertion panics used a custom success diagnostic without that word. Initial run
  is marked failed, not16 passes. Add an explicit assertion diagnostic prefix without
  changing any condition, then rerun; final controls require separate Solidity/Rust
  build success and a selected assertion failure. Final source snapshot hashes are
  recorded and frozen before mutation generation.
- Closing processing starts in closeH itself; initialization does not add an extra
  block beyond M1 cursorDone. Non-tomb HELD refunds count every inspected entry toward
  RET, including skipped BURNED/PAID/tomb entries. No invalid-recipient burn/skip.
- Registry counters, cursor and phase metadata share original slot3. Two counters
  consume one final user write; no hidden persistent slot or unjournaled success flag.
- Missing recipients and maximum-bond overflow tests are explicit unadmitted/corrupt
  execution fixtures, not claimed valid profiles or GenesisBuilder admission evidence.
- Block timestamp can be supplied explicitly for experimental execution controls;
  no validated PoCol apply/phase import or current-block mining data is added.

Outstanding: complete admitted CP/profile resource binding and re-derived R18/SYS,
R21/caps64/R16/LAG; semantic GenesisRewardNotDurable and real GenesisBuilder1024;
ShareLedger/complete Escrow settlement/SV5/SV6/finalization; whole-block invalidation,
persisted crash/reorg and remaining P2 GST/deviation/license acceptance. Bond-module
RewardEscrow is not a fake completed settlement implementation. All26 ledger gates
remain NOT PASSED. No M1/prior evidence changes; no boot, merge or deployment.
