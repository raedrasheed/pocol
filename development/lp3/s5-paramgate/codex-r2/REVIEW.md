# Amended S5 evaluator — implementation and separate sequential review

Verdict: PASS for the LP3 scope explicitly amended by the owner on 2026-10-17.
This does not pass the original general resource derivations or complete LP3.

The additive amendment and matrix are development/LP3-AMENDMENT-01.md and
LP3-ACCEPTANCE-MATRIX.md. M1 and all old evidence, including the original S5 blocker,
remain unchanged. The original general R18/R21 derivations and caps/64 minimum SYS
slot requirement are DEFERRED TO LP4, NOT PASSED. The amendment lists their concrete
acceptance tests. Runtime boot permission remains unconditionally false.

## Implementation

`gate.rs` evaluates R1-R26 with unique, numerically ordered violation names and
explicit missing-fact diagnostics. Typed Rust inputs prevent undeclared fields;
the JSON schema-adapter guard returns explicit UnknownField/ExpectedObject errors.
No JSON/RPC wire protocol or production boot integration is claimed by this library.

The profile binding is SHA256 over domain PoColLP3Profile-v1, length-prefixed id
and GenesisSpec bytes, expected genesis hash, chainId, profile kind, all 52 CP
values as fixed be256, length-prefixed fork count and each be16/be64 activation,
all local timing/retention fields, six gossip byte limits, retentionStress, and all
29 RpcGuard fields in declared wire order with fixed integer widths. Binding tests
mutate every local/RPC/CP field and other identity surfaces. NodeKind is a trusted
evaluation context, not a resource-driving profile parameter.

Resource, identity and calibration facts require this binding, exact resource-model
version pocol-lp3-numeric-facts-v1, and a nonzero artifact digest. Resources require
exactly one ordinary/closing/final row, numeric key/account/live-slot usage, nonempty
per-call gas and block gas totals, durations and configured RET coverage for closing.
Historical retained horizon must cover min(K_hist,H_END); only simnet on simnet with
retentionStress exempts the byte budget, never missing facts/horizon. Identity facts
provide explicit alloc balances and roots; R23 sums them and issuance in 512 bits.
R24 decodes and hashes GenesisSpec, compares the full CP, chainId and supplied roots,
and checks schedule origin/order/supported versions. Only v1 is implemented; v2 is
rejected even on simnet. Calibration has numeric hashrate/duration and an artifact,
and target must match the exact genesis calibration formula.

Facts are classified synthetic fixture, measurement or proven bound. Classification
does not authenticate an external artifact or verify a proof. The current corpus is
explicitly synthetic. A pass proves evaluator behavior on these supplied facts,
not real system execution, measurement validity, worst-case resource safety or boot
readiness. Real artifacts and independent proof review are LP4 obligations.

## Review and actual evidence

After implementation commit 5317b01, a separate sequential Codex pass reviewed
arithmetic, strict bounds, binding coverage, phase completeness, budget exceptions,
identity/fork/profile bypass paths, provenance and boot-disabled behavior. Final source
is bound by final-source-binding.json at local commit 842a12d (complete SHA in file).
This is not a distinct external reviewer identity or formal GitHub approval.

- Regression: 115 tests passed (50 lib + 5 adversarial + 8 assignment + 12 fixtures +
  7 gate + 16 GenesisSpec + 5 prior RPC + 6 profile + 6 timing).
- Final seven gate tests rerun after metadata/UnknownField error refinements: PASS.
  Core arithmetic and earlier 108 regressions were unchanged by those refinements.
- Separate Python unlimited-integer oracle: 1450 full-rule scenarios, zero mismatches.
  It imports no Rust/project arithmetic; the adapter uses test fixtures only as inputs.
- Original independent Fraction RPC oracle: 5173 cases and 21 mutation detections
  remain preserved under codex-r1; R16/R20/R26 implementation unchanged.
- Final stable-source review: disable each R1-R26; ignore resource binding, model or
  artifact; bypass UnknownField, boot-disabled or accepted checks: 32/32 caught.
  Every faulty complete library and test executable compiles in its own directory;
  exit 101 must contain assertion failures, never a compile-error-only detection.
- Format and preservation checks passed. Commands/raw logs/source snapshots retained.

Initial failures are retained in INITIAL-FAILURES.txt and original sources/ledgers:
an R2 negative vector accidentally used the valid equality boundary (25 rather than
24); an early mutation pass overlapped a reviewer metadata edit and failed compiling
an old library against updated tests; a root-directory cargo fmt invocation lacked
the manifest. These were test/tool errors, corrected and rerun. They are not passes.
No unmodified final-source implementation test failure is observed.

Reproduce from repository root with cached locked dependencies and Rust 1.58.1:

    python3 development/lp3/s5-paramgate/codex-r2/verify.py /absolute/fresh/evidence

The reproduction runner builds in a fresh target, records versions/commands/exits,
runs 115 tests, compiles the oracle adapter and executes all 32 isolated mutations.
The original run used an existing dependency target; each mutant library/executable
was independently compiled and isolated. Review tools need not be production binaries.

No new Windows execution, actual EVM/SYS/GenesisBuilder/retention measurement, verified
external proof, caps/64 minimum, general resource derivation, S6-S9 or complete LP3
acceptance is claimed. There are no known unresolved implementation findings for
the amended S5 evaluator. Check live PR requirements before merge into foundation.
