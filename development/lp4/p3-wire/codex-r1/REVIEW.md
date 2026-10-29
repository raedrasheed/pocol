# Separate sequential author review (not independent reviewer approval)

After implementation and positive tests, reviewed M1 execution.md lines20–22,
95–109, accepted P3 scope and oracle isolation. Reviewed canonical integers,
exact field counts, low-s/order/parity, EIP155 and typed signing domains,
access-list duplicate preservation, fixed chain identity and sender recovery.
London intermediates use512 bits and reject >u256 output. Bloom uses actual
address/topics, correct bit order, receipt status/type prefix, cumulative gas,
and unchanged P1 indexed-MPT code. No caller-supplied sender or success accepted.
Independent py-evm validates signatures and executes signed envelopes directly;
its complete state root, gas/output/logs/receipts and balances match Rust.
Python bigint, RLP, eth_bloom and HexaryTrie do not import Rust arithmetic/root code.

Findings fixed before publication:
- Lowercase address normalization fixed an initial fixture-format mismatch.
- External relative source-module reuse broke copied-crate mutation compilation;
  changed to byte-identical in-crate P1 copies, enforced with four CI cmp checks.
- Copied RLP unit tests needed unchanged P1 hex helper, now included.
- First mutation detector required literal 'assertion', while custom assert!
  output omitted that word. Preserved failed attempt (not PASS); negative test
  now uses assert_eq!(is_err(),true), preserving stronger error classification.
  All13 final mutants compiled and failed their selected meaningful assertion.
- Receipt-oracle value accounting explicitly preserves value on reverted calls.

61 Rust tests passed (including copied primitive tests); no ignored/filtered tests
in full suite. Final wire suite rerun after assertion-display fix passed3.
79 envelope fixtures reproduce byte-for-byte;18 actual signed executions include
15 successes/3reverts/3creations.192 bigint London boundary vectors;5 root-only
fixtures0/1/2/18/130 include index127/128. Repeated envelopes in130 are encoding
fixtures, not valid accepted blocks. This is partial P3, not acceptance.

Open: actual RPC/pool CI2 and included-block CI3 rule17, validated header-bound
base fee, signed state-validation/error precedence, SSTORE refund limits,
transactional user-body validation/wrong roots/gas no commit and persistence.
Byte/entry budgets here are local API bounds, not proven CP/profile resource caps.
No claim of full P2/P3/LP4 acceptance, boot authorization or independent-human review.
Existing P2 hosted checks must run again on the new P3 source; not counted passed.
