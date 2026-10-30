# R19 separate sequential same-author review

Unchanged P2 sourceC6 c6e56b069ad3d5a7c1323cae81ea53004f542512,
local equivalent179b9cec/tree d643038c4a6210587c14c364a041f161c14980b7.
All runtime/contract/lock hashes checked in verified.json. New tests only were
installed in a detached scratch worktree; PR24 source and foundation unchanged.
No Claude/coordinator/subagents, production connection, actual keys or activation.

Added independently reproducible actual compiled SELFDESTRUCT transfers1000 to
Escrow and700 to Registry, neither of which accepts ordinary value calls. Cancun
retains the pre-existing force-contract code/nonce. Forced value does not become
held bond, registered value, payout, callback or user withdrawal authority.
Two complete sequences: normal exit/claim and closing-cursor refunds. Registered
bonds remain exact10 each; reward31 gets10, reward32 gets10 only in closing; surplus
Escrow1000 and Registry700 remain locked. Unauthorized release, release with value
and ordinary value calls revert without moving forced balances. All22 individual
EVM calls match a separate py-evm PAY-02 implementation for complete state root,
code/accounts/storage/balances, result bytes and gas. This oracle is explicitly
native/unsigned/spoof control, not unmodified signed transaction proof or P3 CI2.

A source-built mutant changes Escrow's PAY amount from actual bond to whole Escrow
balance. Solidity and Rust compile successfully; the surplus/bond assertion fails.
No compilation/import error is counted as a kill. Force fixture/compiler settings
reproduce byte-for-byte under pinned solc0.8.28. Existing approved write ceilings,
M_min and protocol/default limits remain unchanged. The closing sequence explicitly
uses H_CLOSE=2 in its isolated synthetic Caps state, not an edited accepted profile
or protocol setting. Initial single-active-member
fixture correctly failed M_min's exit precondition; preserved that failed attempt,
then registered a second active member through the actual contract, not by lowering
a limit. Final22-step sequences and assertion control verified afterward.

This is synthetic COMPLETE snapshot coverage of the compiled bond bridge only,
not GenesisBuilder, complete Escrow settlement, real stored-parent block phases,
SV6/finalization, general R18/R21/caps64,64MiB pool memory or disk/crash/reorg.
No whole P2/T2b/LP4 acceptance. All26 final gates remain NOT PASSED; boot false.
No source/binary redistribution of external tools, dependency changes or M1/LP3 edits.

R3 work on PR25 remains separate, source3bdbd149ccabfe21ef6cdd7fbaaa2afdabb1f59b,
with a new unresolved code-limit/accounting consensus proposal. Do not implement
its option1/2 until actual owner answer is recorded; prior PAY/CREATE/Registry and
named-test approvals remain valid and do not need renewed approval. Continue the
remaining permitted tests/source/license work without interpreting a pending answer.

The saved reproduce.py was executed from this evidence-only C6 worktree. It
passed the baseline/22 independent comparisons/compiled assertion control; its
full native vectors and independent receipts equal the initial bytes exactly.
