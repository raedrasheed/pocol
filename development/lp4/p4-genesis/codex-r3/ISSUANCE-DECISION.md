# Protocol subsidy funding point — NOT APPROVED

LP4-SYS-01 is APPROVED in development/LP4-SYS-APPROVAL.md against immutable
b1f25e0613fbeb3832dfb20252f76b92ef72b90d. It is not reopened. All PAY/CREATE/
Registry/CODE, test-mapping and prior owner decisions stay approved. This proposal
is a NEW additive LP4-ISSUANCE-01 decision about protocol issuance timing and its
host interface. No selected mint operation is installed; boot=false.

## Existing requirements and the remaining decision

Preserved M1 incentives.md:71 requires pot=subsidy+blockTips in phase2, and:92–104
requires final subsidy0, escrow custody and issuance bounds. execution.md:25–29,
61–73 prescribes phases and36-byte tips-only closeBlock/finalize;:76–78 makes
COINBASE the actual Escrow account. economics.md:65–69 and governance.md:R23 bound
issuance; M0 bond credit at genesis is already covered by approved PAY-02.

Neither the inspected reference nor recorded decisions specifies when within
apply() the newly issued subsidy is credited to Escrow's spendable native balance,
or an EVM interface that can mint native currency. Ordinary EVM contract opcodes
cannot mint native balance. PAY only transfers existing funds; PAY-01 explicitly
adds no automatic minting and no arbitrary debit. SYS-01 specifies zero CALLVALUE,
zero fee and no synthetic SYSTEM nonce/balance changes. It does not select this
separate issuance point. The pot storage formula alone does not fix balance
visibility in earlier phases. Existing model arithmetic is not execution evidence.

Inspected reference execution/incentives/economics/governance and FINAL_DESIGN.md,
all recorded LP4 approval files and decision packets; no funding-point rule was
found. The proposal does not assert that absence of a search hit proves intent.
It identifies two distinct actual executions still requiring a rule selection.

## Reproducible counterexample — neither model is an approved rule

The compiled SystemBoundaryWitness.captureCoinbaseBalance() executes BALANCE on
COINBASE and stores it. Use an existing Escrow with balance1000, positive synthetic
CP.subsidy80000, and the same ordinary unsigned EVM transaction control. This is
not a fully validated PoCol block or signed-wire acceptance fixture.

- Credit the80000 before USER: the contract records81000. Complete post root:
  0x60025380d016ee7bdc6f7ab101be45cf7217c3d35175997e0528cad33cbaab39.
- Credit the same80000 after USER, immediately before closeBlock: it records1000.
  Complete post root:
  0xfd7a260b248eb962da76f86ea166190994422c16a9c9a6ba6452ed8623459087.

Both controls use43452 gas and end with the same Escrow balance and total supply;
the USER storage and state roots differ. Unmodified independent py-evm confirms
actual compiled bytecode/gas/storage/full roots for both. Compiler, primary and
independent outputs reproduce byte-for-byte. No pending settlement, valid block,
issuance authorization or acceptance is claimed by these diagnostics.

## Option1 — proposed complete additive LP4-ISSUANCE-01 contract

1. For every validated NONFINAL block, mint exactly CP.subsidy into the existing
   RewardEscrow native balance ONCE in phase2, immediately before the prescribed
   closeBlock(tips) message and after the complete USER phase/tips calculation.
   No credit occurs in phase0 or before/during USER. Signed and fallback blocks
   use the same subsidy rule. This is a newly selected observable execution rule,
   not a recovered explicit M1 definition.
2. For the actual final block F, credit ZERO; invoke finalize(tips) without any
   subsidy. Existing finalBurn=tips, pending[F] absence and all actual remaining
   settlements are unchanged. Zero-subsidy nonfinal CP is allowed unchanged.
3. The credit is an internal protocol host transition, not PAY, a CALL with value,
   an ordinary transaction, a transfer from SYSTEM or a general-purpose mint
   precompile. It creates no account, changes no nonce, invokes no recipient code
   and emits no USER receipt/log/tip. No additional system call or calldata field
   is introduced. closeBlock/finalize keep exact selector||be256(tips),36 bytes;
   SYS-01 ORIGIN/CALLER/epoch semantics remain intact.
4. Require the already validated complete GenesisBuilder/system manifest and
   existing Escrow account/code identity. Missing/corrupt account, code, CP/profile
   or issuance facts reject the block. Do checked u256 balance addition and enforce
   original R23 and all existing supply/custody/account/code/dirty/resource bounds;
   overflow or incomplete facts reject. Never silently create an Escrow account,
   use its surplus as issuance or debit another account to fabricate funding.
5. The host step is reachable only through the validated phase2 executor schedule,
   with a single-use per-block phase transition. No USER/EVM call, external RPC,
   caller boolean or replay may authorize another credit. A duplicate phase2 or
   mismatch between final/nonfinal metadata rejects the block.
6. The approved SYS meter still charges actual raw frame gas before refunds.
   The host credit has no EVM opcode/intrinsic/transaction gas; it is a separate
   host state transition whose CPU/memory/account dirtiness MUST be accounted for
   in general R18/R21 and existing global/SYS caps. It is not a cap exemption or
   license to invent free user operations, a new numeric ceiling or extra storage
   metadata. Bind full CP/profile, phase/semantic version, actual source/artifact
   and resource-model facts. Reprove available call budget/EIP150 and invariants.
7. The following closeBlock epoch starts from the actual private candidate AFTER
   this credit, including SYS-01's complete PAY eligibility snapshot. Pot amount
   is checked CP.subsidy+the authenticated USER tips accumulator; no balance-delta,
   SELFBALANCE, transient or caller success flag supplies tips. Existing held
   bonds, pending liabilities, burnedTotal and forced surplus stay distinct.
8. Credit, USER effects, prior SYS effects, pending writes and all resource/supply
   accounting belong to ONE enclosing private block candidate. Any subsequent
   error, OOG, cap violation, corrupt fact, commitment mismatch or persistence
   failure rejects/restores the entire block, including the mint. Actual atomic
   disk commit, crash recovery and reorg/inverse state are mandatory; an in-memory
   clone alone does not pass those gates. No canonical intermediate credit exists.

M1/LP3 and all existing numeric values, Registry10/2/10, CODE-01 and SYS-01 remain
unchanged. M0 genesis bond funding is already approved and is not reopened.
This proposal neither accepts P2/P3/LP4 nor authorizes boot/main merge/deployment/
billing/protection bypass. All27 full gates remain NOT PASSED.

## Required verification after approval — all NOT PASSED / NOT RUN

- Real compiled five programs, actual GenesisBuilder and validated phase0-3 apply;
  authenticated USER BALANCE/COINBASE read proves no current subsidy before USER,
  exact credit immediately before closeBlock and exact pending/tips36 encoding.
- Signed/fallback/nonfinal/zero-subsidy/F0 cases, no pending[F], actual finalization,
  winner/UT/current-mining independence and preserved SV6/bond/surplus accounting.
- Actual full roots, balances/code/storage, original total issuance/base-fee burn,
  logs/receipts/tips, gas before refunds and all resource counters, independently
  compared. Genesis/custody identities are verified, not seeded state assumptions.
- Missing/corrupt manifest/profile/account, balance/supply/CP overflow, duplicate
  phase2, unauthorized/replayed credit and wrong final context reject. No hidden
  alternative credit path, SYSTEM debit, native recipient callback or account create.
- Nested/late system or USER/commitment failure, EVM failure after credit, real disk
  failure/crash/reorg: no canonical mint or earlier block effect survives rejection.
- Successful-build assertion mutations for early/duplicate/final/missing issuance,
  wrong amount/recipient, unchecked overflow, disabled rollback, arbitrary mint
  authorization and omitted host resource accounting. No compile/blocked failure
  counts as a detected semantic mutation.
- Recomputed general R18/R21/caps64 over all supported CP domains, original/amended
  full20661 corpus controls, GitHub requirements and all carried acceptance gates.

## Option2

The owner supplies a different COMPLETE funding point and host/gas/resource/
rollback contract before implementation. Neither diagnostic model is a default.
