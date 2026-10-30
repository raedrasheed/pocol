# PAY native boundary — consolidated owner decision required

Status: PROPOSED, NOT APPROVED. No M1 text or rule is changed. P2 is incomplete.
This is a consensus-interface decision, not another undefined test label.

## What is already binding

- M1 reference/execution.md:33–41 assigns native PAY to 0x…C0C0FF.
- reference/incentives.md:159: 5000 gas, never fails, executes no recipient code.
- decisions.json D51: every **system** transfer goes through PAY; its rationale is
  that value CALL permits a recipient to invalidate a block.
- execution.md:50–52 protects system entry points with SYSTEM_ADDRESS and makes
  a reverting system call invalidate the block. D70 says forced transfers and
  coinbase tips are not system writes. None of this authorizes arbitrary debits.
- Accepted new T2b definition also requires no implicit account creation, forced
  transfer lock/accounting, registered reward routing and rollback tests.

Search of all preserved M1 reference files for PAY, native address, ABI/calldata,
CALLER and transfer rules found no complete native call encoding or authorization
contract. PAY's empty sysCodeHash entry cannot commit an unspecified implementation.
We must not infer that all ordinary EVM value transfers become PAY.

## Proposed coherent contract (option 1)

Approve the following as an additive LP4 implementation decision, subject to all
existing transfer/closing/accounting rules (none waived):

1. PAY is a native **CALL-only** entry at C0C0FF, reached by compiled system contracts,
   not a public arbitrary-debit API. Permit immediate bytecode callers Registry
   (C0C001) and Escrow (C0C002) only, using their own balances as debit sources.
   SYSTEM_ADDRESS does not acquire authority to debit arbitrary accounts.
2. Exact calldata: 64 bytes `pad32(recipient) || be256(amount)`, no selector,
   high 12 address bytes zero, CALL value zero. Empty return data and no logs.
3. A valid call charges 5000 native execution gas, **in addition** to Cancun CALL
   overhead/access charges. No recipient code, fallback, hook or callback executes.
4. A valid transfer requires nonzero existing recipient, sufficient caller balance,
   no recipient overflow, and a recipient permitted by existing registry/reward/
   closing rules. It creates no account. Zero amount is a no-op after validation.
   Self-transfer is balance-neutral after validation. PAY writes share the current
   journal and roll back if an enclosing execution reverts.
5. Invalid caller/encoding/value, STATICCALL/DELEGATECALL/CALLCODE, missing recipient,
   insufficient balance, overflow, or insufficient gas is **not** a valid PAY
   operation: exceptional halt of that call; in a mandatory system phase this makes
   the block invalid. “Never fails” applies to authorized, validated and funded
   system transfers; the system must establish these preconditions before calling.
   This interpretation needs explicit owner approval; it is NOT already in M1.
6. Direct user transactions to PAY receive the same invalid-call result, without
   transferring value or committing any PAY state. No automatic funds minting,
   burn, redirection, or success on invalid input is introduced.

These details are a proposal, not implementation authority. In particular, if
“never fails” must cover malformed/underfunded/OOG calls too, option 1 is unsuitable:
provide a complete alternative that defines those outcomes and gas accounting.
A host-only primitive is an alternative, but needs a specified bytecode-to-host
bridge; choosing one without a contract would merely relocate this missing rule.

## Required acceptance after a decision

Actual compiled Registry/Escrow calls; independent balances/storage/gas/root oracle;
no callback even for reverting/reentrant recipient code; absent/zero/self recipient;
amount zero, exact balance and balance+1; overflow; exact gas and gas−1; every listed
call mode, caller and encoding rejection; value CALL and forced-transfer exceptions
checked separately; closing locks/caps and allowed reward routing; nested revert
restores both sides; compiled mutations for auth, debit source, callback, account
creation, gas, journal and each rejection boundary. Full T2b remains NOT PASSED
until all required execution cases are verified, not merely these sandbox guards.

## Current safe implementation

The isolated control sandbox rejects any observed call-family access to PAY at its
outer API boundary, even if nested code ignores the call result. This is a temporary
experiment guard, NOT a consensus PAY implementation or selected rejection rule.
No system contracts, apply path, boot entry point, RPC or production code uses it.
Do not merge P2 before its full accepted criteria (including GST and PAY) pass.
