# Consolidated Cancun storage-collision decision — PROPOSED, NOT APPROVED

This is a real fork-rule decision, not another missing test label. No M1 change,
exception waiver, or consensus implementation is assumed. P2 and T2 remain NOT PASSED.
The PAY approval is separate: [LP4-PAY-01](../../../LP4-PAY-APPROVAL.md).

## Reproduced evidence

Pinned tests: ethereum/tests c67e485ff8b5be9abc8ad15345ec21aa22e290d9, whose
LegacyTests gitlink is ethereum/legacytests 1f581b8ccdc4c63acf5f2c5c1b155c690c32a8eb.
Cancun/GeneralStateTests has 2807 JSON files and 20661 Cancun post variants.
Other fork variants are not Cancun cases and are not counted as passing them.

The initial complete run executed all 20661: 20648 matching control comparisons,
13 failures, including 1 runner input-overflow failure. Those failures are preserved.
Two failures expose revm 19.7.0 accepting maximum-nonce CREATE as a result instead
of rejecting the transaction (EIP-2681); a standard nonce validation guard fixes
these with maximum-minus-one boundary checks. The unsigned runner's >u256 value
panic is corrected as an explicit control-input error, not wire-RLP acceptance.
These three fixes do not resolve the following ten actual state-root disagreements:

| File under Cancun/GeneralStateTests | Cancun variants |
|---|---|
| stCreate2/RevertInCreateInInitCreate2Paris.json | 0 |
| stCreate2/create2collisionStorageParis.json | 0, 1, 2 |
| stExtCodeHash/dynamicAccountOverwriteEmpty_Paris.json | 0 |
| stRevertTest/RevertInCreateInInit_Paris.json | 0 |
| stSStoreTest/InitCollisionParis.json | 0, 1, 2, 3 |

All ten involve existing storage at the attempted creation address. No entire
folder or failing case has been excluded. `gst-alternatives.json` records the exact
expected and unamended/probe roots for every initial failure. The optional diagnostic
probe matches all ten expected fixture roots; its results do not approve the rule.

## Why selecting a result silently would change consensus

The fixture metadata identifies geth 1.14.0 unstable a208e725 (2024-04-16). The
[v1.14.0 geth create check](https://github.com/ethereum/go-ethereum/blob/v1.14.0/core/vm/evm.go)
rejects nonempty storage as well as nonzero nonce/code.

[EIP-7610 at commit 7707fe333322ed68d1b5efa50bdb4f35909255a7](https://github.com/ethereum/EIPs/blob/7707fe333322ed68d1b5efa50bdb4f35909255a7/EIPS/eip-7610.md)
explicitly **amends EIP-684**, requires retroactive nonempty-storage rejection and
says this is an execution-layer upgrade requiring a hard fork. It also documents
historical test/client disagreements, including py-evm's missing account-reset
handling. Thus fixture/client agreement cannot alone establish that an unspecified
additional fork rule was already in M1's term “Cancun”.

## Option 1 (recommended) — explicit additive LP4 fork rule

Approve EIP-7610's nonempty-storage condition for PoCol's LP4 execution model at
all supported execution heights, as a new explicit rule, without modifying M1.
For creation transactions, CREATE and CREATE2, nonzero nonce, nonempty code **or
any nonzero storage slot** at the destination causes creation collision, with the
same gas, nonce, warmth, journaling and rollback behavior as the existing collision
path. The complete state backend must provide actual storage-root/emptiness facts;
no caller success flag, partial loaded-slot scan or fixture-only address list.

Keep all 20661 Cancun controls and every ten affected variant mandatory. Require
independent execution/root/gas comparisons, zero versus nonzero storage, an unloaded
nonzero slot, CREATE/CREATE2/top-level creation, caller depth/funding/nonce ordering,
inner/outer revert, storage removal/restoration and compiled mutation controls.
Full PoCol exception/fee/type/caps tests still apply separately. No boot permission,
production activation or change to R18/R21/caps64/GenesisBuilder/SV6/finalization.

Option 2: owner supplies a complete alternative fork contract and individually
approved reference/replacement cases; none is silently excluded or passed. The
current implementation does not presume that such an alternative is authorized.

## Safe diagnostic scope pending decision

`cancun_control` accepts `storageCollisionProbe` only as an experimental upstream
control input. The probe uses the complete synthetic InMemoryDB storage inventory
and reverts the newly opened creation checkpoint while retaining standard caller
nonce handling. It is not a production/general state backend or accepted resource
proof. Its handler is absent from `native::execute`, PAY, consensus, RPC and boot.
Do not merge P2 or label T2 PASS while this decision or any P2 criterion is unresolved.
