# SYS-01 R2 continuation — approved design, implementation incomplete

No owner decision is pending. All previously recorded approvals remain binding.
SYS-01 option1 is approved in development/LP4-SYS-APPROVAL.md against immutable
b1f25e0613fbeb3832dfb20252f76b92ef72b90d. No routine reconfirmation is needed.

PR26 branch codex-lp4-p4-system-contract remains draft, stacked on P2 for review.
P2 PR24 remote2c36d0fec08e69ee6005d0362216f4cd60b38c2c, P3 PR25 remotef3fbe13863030adfed6912450e255acd986238b5; foundationc0ef36c68fb83f9474de7e70fe61973b466647a8.
Never merge PR25 or26 into P2. Retarget foundation only after dependencies are
accepted/merged; require each complete increment and GitHub requirements.

Implemented internal message/candidate primitive, nine new tests and native USER
factory extraction; actual Registry/bridge/PAY calls. Source and portable evidence
are bound by source-bindings.json and SHA256SUMS. Current successful results:
80 Rust tests,13 independent full-root/raw-gas/refund/account comparisons,
15 built assertion mutations and99 tests on a separately recorded P3+SYS scratch
overlay. Oracle/vectors reproduce byte-identically. These do NOT pass full gates.
Full raw attempts/mutant sources/logs, earlier failures and the first surviving
mutant are in the local complete archive; LOCAL-PRESERVATION.json pins its SHA.
Public packet has compact results and logs; archive is local, not git-published.

Next authorized implementation:

1. Verify the new-source GitHub run after publication (all five jobs, artifacts,
   hashes, complete20661 identities, all exceptions/replacements, no omissions).
   The prior R4/R20 runs are preserved, not reused as new-source checks. No blocked
   or unrun check is passing; legacy blocked checks remain explicit.
2. Add actual compiled nested EIP150 boundaries and rollback controls and real
   complete Escrow closeBlock/pending byte writes/tips36 and exact padding/kind/
   offsets. The current helper/witness is not that contract. Preserve real PAY-02
   bond bridge and all CREATE/CODE/Registry behavior/caps.
3. Complete the five programs and actual GenesisBuilder, code/allocRoot/sysCodeHash,
   all resource caps and independent builder/state proofs. Current seeded state
   is synthetic even when all52 CP words match; do not call it GenesisBuilder.
4. Build a nonforgeable validated SYSTEM context from actual verified current UT
   (signed proposer/zero fallback), stored parent+grandparent/h1 empty. Enforce
   prescribed phases0-3. Keep authorization CALLER-based, USER ORIGIN unchanged,
   current-winner-independent metadata. The current primitive is crate-private
   because structural inputs are not validated block facts.
5. Integrate USER and all SYS calls into one private whole-block candidate and
   header/resource validation. Prove late failure restores all earlier effects;
   complete atomic disk/crash/reorg separately (P9).
6. Real settlement/finalization, pending[F] absence, final subsidy0, SV6; general
   R18 across supported CP domains with gross gas/remaining budget/EIP150, and
   R21 host snapshots/code scans/trie/history/sharing and caps64 minimum. Bind
   full numeric facts to CP/profile, approved semantics, code/artifact and model.

All27 full LP4 obligations, all full SYS groups and P2/P3 acceptance remain
NOT PASSED; boot=false. Preserve M1/LP3 and every prior failure/evidence. No main
merge, activation/deployment/billing/protection bypass or Claude/coordinator.
