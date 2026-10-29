# Consolidated SystemCall interface decision — NOT APPROVED

CODE-01 option1 is already approved and implemented in P3 R4 and P2 R20. No part
of that decision, PAY/CREATE/Registry or any previous owner approval is reopened.
This packet concerns a different missing execution contract needed by P4/P5 and
the complete P2/P3 integration/resource gates. M1 and all prior evidence are intact.

## Source requirements and the unresolved interface

Preserved M1 execution.md:7–18 defines execution context and excludes the current
winning nonce/owner/shares/signatures. Lines25–29 require closeBlock/finalize from
SYSTEM_ADDRESS and exactly selector||be256(tips),36 bytes. Lines50–53 cap each
system call at30M gas and the block atCP.G_SYS, with a system revert invalidating
the block. Lines61–71 prescribe the phase order and pending writes.

Incentives.md:71 requires closeBlock to store amount, kind and proposer, all known
from UT/body before mining. Consensus.md:53 and155 define UT.proposer and the
signed/fallback distinction. The inspected references/recorded owner decisions
do not specify system-call transaction epochs, nonce/intrinsic/refund accounting,
or how kind/proposer reach the36-byte closeBlock implementation. No EVM ORIGIN
mapping for that metadata is specified. HTTP Origin requirements are unrelated.

Existing unsigned P2 Registry tests use the old ordinary-transaction control API;
they have never been claimed to implement real system phases. Reusing that API
as the SystemCall implementation would silently choose observable consensus rules.
Neither a library default nor the new CODE-01 approval authorizes that choice.

## Reproducible diagnostic evidence — no acceptance

The source-built SystemEpochWitness and actual compiled Registry execute through
pinned revm19.7.0 and independent unmodified py-evm APIs. All25 rows are explicitly
unapproved:21 frame executions match raw gas/refund/output and storage;2 unsigned
controls match;2 ordinary-transaction validation rejections match. Full independent
roots differ between fresh and shared epochs. These are diagnostic countermodels,
not full PoCol blocks, supported-profile acceptance or implementation of a rule.

| Observable | Fresh message epoch | Shared message epoch / ordinary control |
|---|---|---|
| Second readSlot raw gas | 2323 | 323 shared |
| Persisted TLOAD after an earlier call's TSTORE | slot1=0 | slot1=7 shared |
| clearSlot raw gas/refund | 5158 /4800 | 3058 /4800 shared |
| restoreSlot raw gas/refund | 22281 /0 | 281 /−2000 shared |
| SYSTEM nonce after a call | unchanged1 | ordinary control increments to2 |
| readSlot frame / ordinary total gas | 2323 | ordinary23387 (extra21064 intrinsic) |
|30M call with block GASLIMIT8M | direct frame executes | ordinary transaction rejects |
|zero GASPRICE with block BASEFEE100 | direct frame executes | ordinary transaction rejects |

The fresh and shared complete-state roots are respectively
`0x499340f755645e065240edf8be10c9b87e4d9d90200c9deadcb14a450fb0ed01`
and `0xf0e48510b772754b7cbbdd32b1bcf7a77b3350419c4fde4642a1ff0cc07bd5ee`.
The real compiled Registry markClose/beginBlock direct calls retain SYSTEM nonce1;
the old unsigned Registry transaction control increments it. This is not just a
gas-report formatting difference. ORIGIN and CALLER can also differ in an actual
frame, providing one explicit candidate metadata channel without changing tips ABI.

## Option1 — proposed complete additive LP4-SYS-01 contract

1. A mandatory SystemCall is a host-created EVM message, not a wire/user
   transaction. CALLER is SYSTEM_ADDRESS, CALLVALUE=0, GASPRICE=0. It does not
   consume/increment/create a SYSTEM account nonce, charge sender/beneficiary
   fees, burn base fee or subtract transaction intrinsic/calldata gas. Existing
   balances/accounts remain unchanged except actual contract/state effects.
   USER transactions retain their existing authentication/fees/nonce semantics.
2. For SYSTEM messages only, ORIGIN is the validated current UT.proposer for a
   signed block, or zero for fallback. CALLER remains SYSTEM_ADDRESS. closeBlock
   uses that channel to store the required proposer and signed/fallback kind,
   retaining its exact36-byte tips-only calldata. No extra metadata SSTORE or
   extra system call is introduced. This is a NEW LP4 interface interpretation,
   not an existing explicit M1 ORIGIN rule. ORIGIN is never the current winning
   nonce's owner, a mining/share-pool value or a caller-supplied success flag.
   Make the previously underspecified packed representation explicit as part of
   this new decision: pending[j] keeps the M1 base keccak256(be256(j)||be256(3)).
   Word0 is amount. Word1 has kind0=signed /1=fallback in bits0–7 and the160-bit
   proposer in bits8–167, with all upper bits zero. Word2 has the winner in its
   low160 bits and zero upper bits, initially zero. Fallback proposer is zero;
   signed proposer is the validated nonzero UT field. The three-word count and
   all existing settlement/winner/finalization rules stay unchanged. These exact
   offsets/enum bytes are newly proposed LP4 definitions, not recovered M1 text.
3. Each mandatory call starts a fresh transaction-like execution epoch over the
   current private block candidate, including the effects of previous calls/users.
   Access warmth, original storage for EIP2200, transient storage and refund
   counters do not carry between calls or USER transactions. They do carry and
   revert normally within that call's nested frames. Initial warm addresses are
   SYSTEM, nonzero ORIGIN, callee, Escrow/COINBASE and standard Cancun precompiles;
   no storage slots are prewarmed. Approved PAY access/gas behavior is preserved.
4. Set call gas to min(30M,remainingCP.G_SYS), with a checked block-wide SYS meter.
   A zero budget cannot execute a mandatory call. Charge actual raw frame gas
   spent BEFORE refund, including nested/native costs; refund counters grant no
   SYS-budget credit or monetary fee/refund. EIP150 and all ordinary opcode gas
   rules remain active. Context GASLIMIT stays CP.GAS_LIMIT, independent of SYS
   allocation; BASEFEE/NUMBER/TIMESTAMP/etc remain their validated block values.
   This explicitly chooses a global-budget allocation/measurement rule, without
   changing either numeric ceiling. Full R18 must include EIP150 headroom and
   the remaining-budget boundary, not just optimistic sum-of-gas estimates.
5. System calls do not append USER transactions, EIP658 transaction receipts or
   tips; header USER gas/base-fee/receipt/transaction accounting stays on USER
   execution. Their state effects participate in the final world-state root.
   The initial five v1 system programs emit no EVM LOGs; prove that from compiled
   code and execution. This is not an exemption from existing LOG limits or a
   license to discard emitted logs. Settlement metadata/index remains the
   separately required M1 implementation, not a fabricated USER receipt.
6. Calls execute only from the prescribed executor phase order over validated
   CP/profile/block inputs. ParentResult comes only from stored parent headers
   and grandparent assignment, h1 empty. Native PAY's pre-transaction eligibility
   snapshot for each system message is that epoch's actual complete starting
   candidate. Existing PAY/CREATE/Registry rules are unchanged. A system error,
   revert, OOG, cap/budget violation or corrupt fact rejects the entire block;
   earlier system/user effects never reach canonical storage. EIP161 cleanup
   and actual deletions complete at each epoch boundary; enclosing block rollback
   restores all effects. Atomic persistence/crash/reorg remains mandatory.
7. Preserve M1/LP3, all CP values and every numeric gas/write/slot/account/log/code
   cap, including approved10/2/10 Registry ceilings and CODE-01's49152 initcode.
   Version the new semantic contract and bind all affected source/artifact/full
   CP/profile/resource-model facts. Old measurements and ordinary controls are
   historical evidence, not acceptance under the new contract. Boot stays false.

## Option2

The owner supplies another COMPLETE execution/metadata/epoch/gas contract before
implementation. Shared and ordinary diagnostic variants are not approved defaults.
No selected SystemCall behavior is installed by this packet or its diagnostics.

## Required acceptance after a decision — all NOT PASSED / NOT RUN

- Actual compiled protocol contracts through the host SystemCall API, signed and
  fallback validated UT metadata, exact tips36 encoding, invalid/corrupt context,
  authorization, no SYSTEM nonce/account/fee artifacts, unchanged USER behavior.
  Verify exact pending kind/proposer/winner word bytes and zero padding against
  an independent encoder; wrong kind/offset/padding mutations must be detected.
- Independent actual full roots, balances/code/storage, raw/net gas/refunds and
  resource counters; fresh/reset boundaries, nested revert and late whole-block
  failure, ordinary-call negative controls and actual native PAY integration.
-30M/G_SYS equality and minus1, zero budget, EIP150 nested-call headroom, high
  real BASEFEE and block GASLIMIT below SYS allowance; no synthetic flags or
  unsupported CP profile counted as accepted. Full proof/measurement over all
  supported CP domains and actual source-built programs.
- Current-winner independence and stored-parent/grandparent rederivation; actual
  phases0–3, actual GenesisBuilder, finalization, pending[F] absence, zero final
  subsidy, SV6 and all carried gates remain mandatory.
- Successful-build assertion mutations for shared/wrong epoch, changed SYSTEM
  nonce/fee/intrinsic, wrong ORIGIN/current winner, net-refund SYS credit, skipped
  budget, wrong gas allocation, metadata spoofing and failed rollback. No compile
  error, blocked or unrun case passes. All old Cancun and amended replacements
  remain required. GeneralR18/R21/caps64 and import/crash/reorg are not deferred
  away or marked passed by approval. No boot, main merge, deploy or billing change.
