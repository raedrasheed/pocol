# LP4 implementation and acceptance plan

Base: accepted amended LP3 foundation `041061aa62d5bf772c4e45d790ce5e5a85aad151`.
Authority: current owner instruction authorizes implementation/tests/separate sequential
review/evidence and foundation-only PR management. It supersedes the historical
Claude/coordinator/manual-merge wording in LOCAL-DEVNET-PLAN.md; neither is invoked.
No M1 or LP3 rewrite, production activation, main merge, deployment or billing change.

Binding scope: LOCAL-DEVNET-PLAN.md LP4 row, M1 reference/implementation.md M4,
execution.md, consensus.md H-full10–17, incentives.md, governance/threat bounds;
LP3-AMENDMENT-01.md and LP3-CF-CORE-APPROVAL.md are additive carried obligations.
LP3 acceptance is not real execution/resource acceptance. Every execution obligation
starts NOT PASSED. Increment completion never implies full LP4 acceptance or boot.

LP4-CODE-01 is explicitly approved in [LP4-CODE-APPROVAL.md](LP4-CODE-APPROVAL.md).
Its complete runtime/initcode/logical-accounting contract and acceptance evidence
are mandatory across P2/P3/P4/P7/P8/P9/P10. Approval does not pass results or change
CP values, R9, M1 or any other limit. Original Cancun controls and individual
replacement proofs remain required, as do actual GenesisBuilder and persistence.

## Ordered increments (one branch/PR each)

P4/P5 SystemCall and phase-2 issuance are explicitly approved through
[LP4-SYS-APPROVAL.md](LP4-SYS-APPROVAL.md) and
[LP4-ISSUANCE-APPROVAL.md](LP4-ISSUANCE-APPROVAL.md).
The immutable proposals remain historical; approval does not accept implementation.
The accepted CODE/PAY/CREATE/Registry decisions stay closed. Every existing gate
and resource obligation remains mandatory and NOT PASSED; boot stays disabled.

| ID | Implementation | Acceptance before foundation merge |
|---|---|---|
| P0 | Source/decision inventory, traceable scope and acceptance ledger | Reproduce hashes and immutable baseline; separate review; identify missing named gate definitions rather than invent recovered originals. Documentation acceptance only. |
| P1 | Canonical Ethereum Merkle Patricia trie/root primitives: raw ordered transaction/receipt keys, secure address/storage keys, compact paths and inline-vs-hashed references | Independent Python Keccak/RLP/root oracle; empty/single/prefix/branch/long values, zero storage omission, permutations/duplicate rejection, 1024-account case; compile-and-assert mutations. This does not pass GenesisBuilder. |
| P2 | Isolated pinned Rust execution crate/toolchain with revm Cancun, required customizations and PAY; immutable account/state snapshots | Exact dependencies/licenses/compiler/source provenance; GeneralStateTests Cancun with every declared exception enumerated (not a blanket exclusion), independent EVM comparison, PAY no callback/no account creation behavior and rollback. Legacy LP1 crate remains reproducible. |
| P3 | Typed transactions (legacy EIP155/2930/1559), sender/chain validation, London base fee, tips, receipt/bloom/body roots | CI1–CI3, chainId mismatch, unprotected legacy/type3/type4 rejection, gas/refund/fee/burn/receipt tests; wrong body/roots/gas never commit; independent roots/state differential. |
| P4 | Source-built system contracts, PAY routing and deterministic GenesisBuilder | Versioned reproducible compiler settings/artifacts/storage layout/ABI; sysCodeHash = Keccak(RLP(C0C001..C0C005 code, empty PAY)); construct real allocation/storage/account MPT; GB1 and GB-END1024, heldTotal and sorted ActList; L4s Rust/anvil state-root agreement. Opaque GSV1/V1NET roots remain non-bootable fixtures. |
| P5 | Transactional apply phases, actual Registry/ShareLedger/Escrow/ChainCaps calls, validated parentResult and fallback construction | Exact phase order; <=30M/call and <=G_SYS/block; caller-only system paths; ParentResult from stored parent plus grandparent assignment only, h1 empty; API excludes current nonce/owners/shares/signatures; rederive mismatch halts import; fallback body c2c0c0/byte-exact UT. |
| P6 | Actual settlement/evidence/closing/finalization transitions | Full SV1–SV7s including real SV6; CF1–CF6 core mapping remains newly approved, now add real cursor/state effects and CL1–CL4; pending[F] absent, final subsidy zero, tips burned, all remaining pots settled, no child of final; balance/supply/held/burn accounting, all inverse diffs restored. |
| P7 | CapsTracker plus complete general R18 SYS derivations and caps64 minimum | All supported CP/profile domains: parameterized slot/key/account/gas bounds per phase/RET; independently counted real writes/accounts and reproducible source-bound measurements; reproduce KN/KC/KF=1752/1482/1395 and AN/AC/AF=404/388/257; changed RET/end/full; each bound mutation detected. caps64 minimum passes and minimum-1 fails R18, real execution reaches requirement. Estimated gas alone is not proof. |
| P8 | General R21 historical trie/sharing/retention resource derivation | Model complete profile/horizon/node encoding/sharing, independently count retained trie nodes/bytes; boundary budget and changed K_hist/H_END; only simnet+retentionStress exception; distinguish measured maxima from proven bounds. No synthetic or incomplete facts substituted. |
| P9 | Atomic persisted state/roots/receipts/body/diff/metadata commit and rollback/replay | All H-full10–17 positive/negative cases; crash at every write/flush/commit boundary yields exactly one complete committed state; invalid block leaves disk unchanged; profile binding, corruption rejection, reexecution/parentResult mismatch and reorg inverse-diff correctness. No fabricated portable-lock claim. |
| P10 | Integrated execution/resource audit and Arabic acceptance report | Independent EVM/state-root differential, supported OS full regression and crash tests, fault/mutation controls absent from release, exact code/model/profile/resource binding, all required named gates bound and verified, no unresolved findings. Every carried obligation individually PASSED only with receipts; full LP4 remains NOT PASSED until then. Boot remains disabled under this user's explicit instruction even if execution acceptance completes. |

Dependencies: P1→P2/P3→P4→P5→P6; P7 after real system paths P4–P6;
P8 requires actual trie representation and persistence design; P9 integrates actual
execution; P10 waits for all. Unblocked prerequisites proceed while a specific gate
is pending. Engineering order/tool versions/artifact layouts are documented reversible
choices, not purported owner answers or changes to consensus rules.

## Required checks and evidence contract

Each PR records base/head/source/input/model/profile identities, exact commands,
versions/licenses, exits, positive and negative results, reproducible seeds/oracles,
review findings/fixes, and SHA256 manifests. Author tests precede a distinct sequential
review (same author identified honestly). A separate oracle must not import the tested
implementation's root/arithmetic logic. A mutant must compile and fail a meaningful
assertion; compile/import failures are not successful controls.

Never merge before its scoped acceptance and actual GitHub requirements succeed.
Blocked, missing, pending and not-run checks are not passes. Preserve failed attempts
and previous evidence. Use working GitHub connector, expected-head updates and normal
merges only into foundation. Verify remote tree equals audited local bytes (especially
CRLF artifacts). No credential values, private-key material, caches or dependency
binaries in evidence. No real funded keys or production operations.

## Resolved named-test definitions; execution acceptance remains outstanding

The owner explicitly approved the proposed T2/T2b/T2c/T24/T26 mapping as new LP4
acceptance definitions. See [LP4-TEST-GATES-APPROVAL.md](LP4-TEST-GATES-APPROVAL.md).
The historical proposal remains unchanged; it is not recovered original M1 content.
Every mapped gate still requires real execution, independent comparisons, boundaries,
negative tests and compiled mutations before PASS. All carried obligations remain.
Additional missing test labels must be consolidated into one proposal. No consensus
rule may be invented or acceptance requirement weakened. Continue P2 autonomously.

Live ledger: [LP4-ACCEPTANCE.json](LP4-ACCEPTANCE.json). All final execution/resource
criteria remain NOT PASSED. No silent deferral beyond LP4. Boot disabled.

PAY native interface is now explicitly approved by [LP4-PAY-01](LP4-PAY-APPROVAL.md).
The newly approved conditional interpretation of “never fails” is additive; M1
and the immutable proposal remain unchanged. Approval does not pass PAY/T2b or P2.

Owner approved LP4-CREATE-01 in [LP4-COLLISION-APPROVAL.md](LP4-COLLISION-APPROVAL.md).
Nonempty-storage creation collision is an additive LP4 rule at every supported
height; no repeat fork approval is needed. All original execution/resource gates
remain mandatory and NOT PASSED until verified.

Owner approved the complete additive lifecycle contract in
[LP4-PAY-02](LP4-PAY-LIFECYCLE-APPROVAL.md). It explicitly narrows native recipient
eligibility to pre-transaction nonce/code and requires Escrow custody, the restricted
Registry release bridge, admission probes and semantic genesis rejection. All seven
parts of the historical proposal remain mandatory; partial native controls do not
pass integration, genesis, closing or resource gates. Existing R18/R21 measurements
must be re-derived for the complete implementation, not reused as accepted bounds.

Owner approved LP4-REGISTRY-01 option1 in full; see
[LP4-REGISTRY-APPROVAL.md](LP4-REGISTRY-APPROVAL.md). Slot3 successful-call accounting
and original write ceilings are both binding. Actual compiled lower-bound witnesses
and independent SSTORE traces expose a10/2/10 versus9/1/9 conflict for register,
setRewardAddress and middle claimBond. Part8 requires returning this contradiction;
no unapproved ceiling increase or budget exemption is implemented. See the explicitly
unapproved consolidated resolution in lp4/p2-runtime/codex-r7. All25 acceptance
obligations remain NOT PASSED; boot false, P2 draft, foundation unchanged.

The owner explicitly resolved the three write-ceiling conflicts by approving
[LP4-REGISTRY-02](LP4-REGISTRY-WRITE-APPROVAL.md) option1 in full: LP4-only10/2/10.
The preceding blocker statements describe the historical decision state; they do not
remain an approval stop. No other limit changes, and original9/1/9 are not passed.
Actual source-built Registry/Caps/bond-bridge implementation and independent tests
now proceed on PR24. Full GenesisBuilder/system settlement/finalization and complete
CP/profile-bound R18/R21/caps64 proofs remain mandatory. All26 ledger obligations
remain NOT PASSED until individually verified; approval and partial tests are not
whole LP4 acceptance. Boot false; preserve M1 and prior evidence.

LP4-SYS-01 option 1 is now owner-approved separately in [LP4-SYS-APPROVAL.md](LP4-SYS-APPROVAL.md). The immutable proposal remains historical. Approval passes no execution/resource gate; every unverified obligation and boot=false remains unchanged.

R3 verifies compiled nested SYS EIP150/rollback boundaries independently. A separate new funding-point/host-interface question is consolidated in `lp4/p4-genesis/codex-r3/ISSUANCE-DECISION.md` (NOT APPROVED); no issuance rule is installed and all prior approvals remain closed. All27 complete obligations remain NOT PASSED.

LP4-ISSUANCE-01 option1 is now owner-approved separately in LP4-ISSUANCE-APPROVAL.md. All eight clauses/acceptance cases bind implementation; no full gate passes and historical proposals remain unchanged.

R7 integrates actual source-built D27 factory into the sealed synthetic genesis image and authenticated USER/phase-2 comparisons. One Ledger witness and incomplete Escrow settlement remain explicit. All27 complete gates, general R18/R21/caps64, full validated phases/finalization/SV6 and durable atomicity remain NOT PASSED; boot=false. See lp4/p4-genesis/codex-r7.

LP4-LEDGER-01 is a NEW consolidated pending owner decision in lp4/p4-genesis/codex-r8/LEDGER-STATE-DECISION.md. The sources do not fix physical Ledger retention/layout and recording interface; compiled equal-code/equal-parent-state countermodels preserve needed logical counts but change state roots. Neither policy is installed or accepted. All prior decisions remain closed; independent checks/evidence work may continue.
