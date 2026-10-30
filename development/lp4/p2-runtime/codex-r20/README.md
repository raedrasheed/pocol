# R20 — P2 CODE-01 runtime prerequisites, not acceptance

Authority: LP4-CODE-APPROVAL.md records the complete owner-approved option1.
This increment is implemented directly on P2's C6 source; PR25 is NOT merged into
P2. Its wire decoder, receipts/fees/USER phase, pool and RPC files are absent here.
The shared native/code_caps sources match published P3 R4 exactly, at
f3fbe13863030adfed6912450e255acd986238b5. Accepted LP1/LP3 schema/identity/trie/JSON
primitives are nine byte-identical copies, verified in local checks and CI.
No dependency, default CP, M1, LP3, PAY/CREATE/Registry rule or other limit changes.

The new profile-bound engine derives runtime and logical code limits from the
owned full schema/hash identity. Runtime deposit uses exact CP.B_code_max while
ordinary Cancun validation/opcodes retain independent49152 initcode. The actual
complete EmptyDB-backed inventory plus journal overlay charges code per address,
checks bytes/hash, uses checked arithmetic, preserves EIP3541/gas/nonce/checkpoint
and outer rollback, and grants deletion credit only after actual commit/removal.
Below-R9 profiles and invalid code snapshots reject. Old chain-only APIs remain
explicit baseline controls; schema identity is not full ParamGate or boot.

71 local Rust tests pass with exit0. The new native cases execute all76 independent
CODE-01 fixtures plus two follow-ups, comparing complete account-state MPT roots,
output, success, gas, logical bytes and immutable input snapshots. Native views
come from independently decoded and signature-checked public synthetic raw bytes;
P2 itself does NOT authenticate wire signatures. P3's separate signed tests remain
required. Compiler, complete fixtures and derived views reproduce byte-for-byte.
Small logical-cap controls are synthetic, not complete supported-profile proofs.

Eleven compiled mutations build successfully and trigger meaningful assertions:
runtime off/old, initcode coupling, logical off/equality, ignored overlay,
physical dedup, early deletion credit, enclosing rollback, unchecked hash and R9.
P3's additional production USER-commit mutation stays on P3; P2 has no USER phase.
This does not remove the mandatory persistence/import/crash/reorg/rollback gates.

The workflow retains every original Cancun and PAY/control job and adds compiler/
identity reproduction, native views, code mutations, the complete20661-case code
delta inventory, and independent comparisons of each delta. New P2 hosted checks
are pending publication, never inherited from C6 or P3. R4's separate delta proof
has8 actual execution matches plus6 oversized-input snapshot rejections, not14
GenesisBuilder passes. R4's additional PAY-pairing test uses PayBoundaryHarness
at the Registry/Escrow role addresses, not complete real system phases.

Mandatory open work includes actual GenesisBuilder/system allocation/PAY, all
carried R18/R21/caps64 and complete CP/profile/source/semantic/model provenance,
overflow proof, full T2c, real system phases/SV6/finalization and persistence/
crash/reorg. Complete-state code scans and DB clones are not RAM/host-work/gas or
historical bounds. All27 ledger gates remain NOT PASSED; boot remains false.
No P2/P3/LP4 acceptance, foundation/main merge, activation/deploy or billing action.

Reproduce the committed CODE compiler/fixture/view JSONs using the three scripts
named in the workflow, then cargo1.99.1 offline/locked tests and
tests/code_caps_mutations.py OUTPUT. The workflow pins exact arguments/dependencies.
Local archives preserve full receipts, mutant source/build/assertion logs and
earlier attempts; only files explicitly listed in publication evidence are public.
