# Separate sequential same-author review

No second reviewer/agent, Claude or coordinator was invoked. This is a distinct
review pass after implementation/author tests, not a claim of independent authorship.

- Authority: PAY-01 records the owner's entire option-1 approval and conditional
  “never fails” interpretation as new. Historical M1/LP3/P0/P1/R1 evidence unchanged.
- Native interception precedes ordinary frame/value transfer. Immediate active code
  identity is recorded by execute_frame, preventing delegated foreign code from
  inheriting Registry debit authority. Direct transaction depth zero is rejected.
- All input/mode/value/gas/account/funds/overflow checks precede balance mutation.
  In particular, revm's transfer helper debits before its overflow check: precheck
  the addition before using that helper. All valid effects use its enclosing journal.
- Callback trap, exact/minus-one gas, self/zero/exact balance, invalid encoding/value/
  caller/modes, inherited static context and outer revert execute compiled bytecode.
  Independent py-evm reproduces 28 boundary executions including a warm second call.
- Initial padding mutation survived because a separate overflow precondition masked
  the missing pad check. Restored a non-overflowing recipient for encoding tests.
  Initial static mutation survived because STATICCALL's scheme already rejected it;
  added inherited-static normal CALL to isolate the static flag. Both survival runs
  are preserved and not counted as successful controls.
- Added allocated-zero recipient to isolate zero-address rejection from absence,
  plus account-creation, journal, callback, authorization and bytecode-identity controls.
- EIP-2681 max-nonce CREATE rejection was missing in revm 19.7.0; added explicit
  actual-snapshot/transaction nonce checks and max-minus-one boundary. Invalid
  >u256 transaction value is an input error in the unsigned control adapter, not
  proof of actual signed wire-RLP rejection. Error reason equivalence remains unverified.
- The initial full GST run's 13 failures remain visible. Ten root disagreements
  are reproduced by a separate diagnostic creation-collision probe, absent from
  native/consensus execution. The probe's complete 20661 matching comparisons are
  NOT T2 acceptance: EIP-7610's added fork rule requires an explicit decision, and
  PoCol fields/deviations, signed envelopes and real system integration remain.
- Full legacy local run initially hit socket permission errors. With network-enabled
  sandbox it passed HTTP, then failed the intentionally Windows-only journal tests
  as UnsupportedPlatform. No portable-lock/full-regression PASS is claimed. New
  CI asks the existing supported Windows verifier to run at this PR head.
- All 266 locked registry archive SHA256 values were checked; SPDX/license-file
  inventory and active dependency tree are preserved. This is not completion of
  license/notice terms audit. Installed minimal Rust lacks rustfmt; no fmt PASS.

Outstanding acceptance: complete real Registry/Escrow policy/phase integration,
reward routing, closing/forced-transfer locks, caps, full required GST semantics
and signed-transaction validation, independent full PoCol EVM/root comparisons,
complete license audit and actual supported-platform CI receipts. Do not merge P2.
