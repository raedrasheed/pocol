# R4 — approved CODE-01 implementation, not P2/P3/LP4 acceptance

Authority: [LP4-CODE-APPROVAL.md](../../../LP4-CODE-APPROVAL.md), recording the
owner's complete option1 approval separately from the unchanged historical R3
proposal. M1, LP3, default CP values and all other limits are unchanged. The ledger
adds the mandatory CODE-01 gate; all 27 obligations remain NOT PASSED, boot false.

## Implementation and scope

`native::execute_for_profile` derives B_code_max and CODE_CAP from the owned,
schema/hash-bound LP3 identity. The code inventory is the actual complete
EmptyDB-backed account database, with current journal overlays. It resolves and
checks runtime bytes against code hashes, charges shared code once per ADDRESS,
uses checked sums, and rejects missing/mismatched code or an invalid snapshot.
Below-R9 B_code_max is rejected, not treated as a supported profile.

The Cancun environment keeps its normal code configuration throughout transaction
validation and interpreter execution. Only the existing create-return/deposit
handler temporarily sees CP.B_code_max, then restores the configuration, including
on errors. Thus top-level CREATE and CREATE/CREATE2 opcodes keep the original
independent 49152-byte initcode limit, word gas and evaluation order. Existing
EIP3541, code-deposit gas, nonce/checkpoint/warmth and outer rollback remain active.
CODE_CAP+1 returns a deposit failure through the existing checkpoint-revert path.

Code scheduled for SELFDESTRUCT remains observable before actual transaction-end
deletion, so receives no early credit. Subsequent complete-state inventory releases
charge only after actual deletion. Ordinary Cancun SELFDESTRUCT retains its code
and charge. No separate non-journaled counter or caller success flag is used.

Chain-only old APIs and `cancun_control` remain upstream controls, not PoCol apply.
PoCol candidates use the profile-bound API. No production path or boot is enabled.
ProfileIdentity alone still does NOT verify ParamGate, actual allocRoot/sysCodeHash,
system CP storage, full GenesisBuilder or block/persistence acceptance.

## Verified evidence

- 76 source-generated signed cases, plus two actual follow-up transactions, compare
  runtime equality/+1, minimum/default R9 code limits, all three signed types,
  compiled CREATE/CREATE2, initcode49151/49152/49153, gas equality/minus1,
  zero/EF code, logical cap±1, shared hashes, two deposits, outer revert, retained
  and actual deletion. Independent local py-evm computation subclasses implement
  the declared amendment; upstream library source is unchanged. Full roots,
  balances/nonces/code, gas and receipts match. This is NOT unmodified Cancun.
- Fixture generation and Solidity0.8.28 output reproduce byte-for-byte. Lossless
  plaintext `hexRuns` represents padded zero bytes; all signed bytes reconstruct
  exactly, with independent raw SHA256s. Full expanded fixture is retained locally.
  Small logical-cap profiles are schema-bound synthetic controls, not complete
  ParamGate-valid profiles or resource proofs.
- 90 native Rust tests pass, including six profile-bound compiled Registry/Escrow
  PAY boundary/revert calls compared with existing baseline results and full diffs.
- Twelve compiled mutation controls each build successfully and fail a meaningful
  assertion: runtime off/old, initcode coupling, logical off/equality, ignored
  journal overlay, physical deduplication, early deletion credit, enclosing
  rollback, unchecked code hash, disabled R9 and skipped state commit.
- All20661 original corpus identities are retained in the new code-delta scan:
  20647 unchanged; 14 individually enumerated. Eight actual amended executions
  match independent roots/logs/gas/success. Six contain pre-existing50000-byte
  runtime and independently fail actual VM snapshot inventory admission. These
  SIX ARE NOT GENESISBUILDER EXECUTION PASS. See individual-replacements.json.
  No dropped/blocked case or blanket exception. The original unmodified corpus
  controls remain separately mandatory, including the six original pre-states.

Earlier hosted R3 all-five-job evidence is separately published at
44b6134dad04c8a47684a2c2f22c5b4f84716d12. It predates this rule implementation.
New hosted execution is pending, not inherited from that run. The unmodified
signed py-evm audit's73 acceptances and legacy Windows8 blocked/2 unrun remain
recorded; neither is silently converted into a passing check.

## Mandatory open work

Actual GenesisBuilder allocations/system code/PAY and every proposed code-limit
case must still pass. Full supported-profile validation, overflow proof, import,
disk/crash/reorg/inverse diffs, full T2c, system phases, SV6 and finalization remain
mandatory. No primitive, snapshot check or comparison here passes those gates.

General R18/R21/caps64 derivations and measurements must bind the full CP/profile,
source and semantic/model version. This prototype hashes/scans complete code on
snapshot validation and deposits; its logical-byte counter is NOT a RAM, host-work,
gas, historical sharing or general resource bound. No resource fact is accepted.
Native PAY contributes zero actual code; five system addresses participate in the
inventory test, but that test is not their actual GenesisBuilder allocation.

Shared runtime support must also be introduced and checked on P2 independently;
PR25 is never merged into P2. P2 remains draft and unaccepted. PR25 retargets the
foundation only after accepted P2 merges; then all P3 and GitHub requirements apply.

## Reproduce

Use the pinned tool inventories and locked Python/Node dependencies from R3.
Generate CodeCapsWitness with `tests/compile_code_caps.cjs`, regenerate fixture
with `tests/code_caps_fixtures.py`, compare both committed JSONs byte-for-byte,
then run `cargo +1.99.1 test --offline --locked` for this crate and
`tests/code_caps_mutations.py OUTPUT`. Use CARGO_INCREMENTAL=0.

Build `code_caps_control` in release mode, run `tests/gst_code_replacements.py`
with the pinned2807-file corpus, explicit profile and original20661-case receipt
file, then `tests/gst_code_independent.py` over that complete delta inventory.
The workflow records exact arguments and uploads full receipts/logs. Local full
archives and their hashes are listed separately; they are not claimed published.
