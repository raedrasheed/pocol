# Separate sequential review (same author)

Owner approved CREATE option 1. LP4-COLLISION-APPROVAL.md records its entire scope,
without changing M1 or historical proposal/evidence. No repeat approval is needed.

Review findings/fixes:
- A wrapper rejecting storage only after revm opened a creation frame could let
  destination-balance overflow win before collision. Replaced it with a Cancun-only
  create-frame adapter placing the complete storage check at the actual collision
  boundary after depth/funding/caller nonce and target warming, before creation.
  Added a max-destination-balance test checking the actual CreateCollision reason.
- Adapter follows pinned revm 19.7.0 create-frame logic; MIT copyright/permission
  notice included in licenses/REVM-MIT.txt. No geth implementation code is reused.
- Storage facts use InMemoryDB's complete EmptyDB-backed inventory plus current
  journal values, not just loaded slots or caller flags. Verify clearing, new keys,
  newly-created account storage and checkpoint restoration. No generic partial-DB
  implementation is offered; no resource/time bound or persisted-state claim made.
- Require Cancun explicitly when registering the adapter. A direct test initially
  used revm's default LATEST; fixed the test to select the supported fork. Compiler
  depth type mismatch is retained as a failed attempt, not a mutation success.
- Initial new-slot mutant survived because CacheDB's storage loader cached a zero
  base entry for the new key, making the fallback inventory also find it. Added a
  freshly created account whose cleared base cannot mask the journal-only branch.
  Final seven mutants compile and fail meaningful selected assertions.
- Created test-only compiled RewardDurabilityWitness and independent preserved
  GenesisSpec FORMAT witnesses. Current PAY-01 permits a zero-PAY probe then later
  erasure of the destination, including a prefunded balance-only address. No actual
  Registry/closing acceptance is claimed. A consolidated lifecycle/custody proposal
  identifies the remaining genuine owner rule; no proposed PAY-02 change is assumed.
- Historical alternatives script now refuses the current rule-tagged runner;
  approved execution no longer exposes an optional probe switch.

Verification: complete approved-source Cancun corpus has 20661 matching controls,
zero failures; expected exception presence is not signed-wire/error-equivalence
acceptance. 27 Rust tests (8 baseline,10 collision,8 PAY,1 lifecycle witness), 28
independent py-evm PAY cases, seven compiled CREATE assertion controls. Original
full-corpus failures and optional-probe evidence remain unchanged.

Windows old-head receipts: job succeeded, artifact digest verified, 76 source
identities matched (6 raw/70 CRLF) against tested head 25ec1f852a7bf9cd9c2960594f3962b18d9197c7.
Its aggregate has16 PASS,8 BLOCKED BY PLATFORM,2 NOT RUN; blocked/unrun rows are NOT
passes. Current changed workflow/runtime needs its own checks; no result is assumed.
All M1/LP3/previous evidence paths remain unchanged. All23 LP4 obligations stay
NOT PASSED; boot disabled. Full real system policies, PoCol differential, wire
validation, licenses, R18/R21/caps64/GenesisBuilder/finalization/SV6 remain mandatory.
