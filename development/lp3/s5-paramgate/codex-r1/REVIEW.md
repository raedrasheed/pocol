# Partial S5 implementation and sequential Codex review

2026-10-17. Verdict: **partial arithmetic PASS; S5 BLOCKED BY DESIGN**.
No complete ParamGate, S5 acceptance, or LP3 acceptance is claimed. Do not merge this draft.

## Provenance and scope

Requested starting point is S4 merge c662bff29fb748594224eb48d329d9b2aff1244e.
Live foundation was f933ef8a1e43a22ee6c7ca08c0b14236e086a95f, its single documentation
successor. The local checkout was still the S4 source branch. Git fetch failed at the
inherited proxy before reaching GitHub; no credentials were printed. Via the working
connector, the exact merge and successor commit objects and updated dashboard were
retrieved. Git object hashes and complete tree hashes were checked before switching
to the separate codex-lp3-s5-paramgate branch. This preserves the actual foundation,
including its documentation successor; no synthetic history replaces it.

Implementation local commit: 2a04902869247eba1b7101c6b4b14e8b97a953e8.
Review test correction: d8053b4 (full commit recorded by the second sources.json).
Published connector commits may differ because the connector sets commit metadata;
the source SHA256 manifest binds the exact tested bytes independently of that metadata.

Added unambiguous R16/R20 registry metrics and R26 checks/derive only. Checked u128
arithmetic rejects overflow and invalid divisors. Integer milliseconds plus private,
reduced fractions preserve exact send/watchdog/wall values and comparisons without
float rounding. Single-response SEND_MAX includes slack; BATCH_SEND_MAX excludes
slack, and its watchdog/wall account for slack once, matching network.md and named
validation vectors. No public full-gate ok value or boot authorization exists.

## Separate review pass

After the implementation commit, reviewed formulas, units, narrowing, error paths,
and edge cases against governance.md, threat.md, network.md and validation.md.
The separate Python oracle imports no repository implementation and computes with
unlimited integers and Fraction. The Rust adapter merely exposes actual outputs.
This is a sequential Codex review pass, not another external reviewer identity or
formal GitHub approval. No Claude, coordinator or sub-agent was invoked.

Fresh clean build and regression passed 108 tests: 50 library, 5 adversarial,
8 assignment, 12 fixture, 16 GenesisSpec, 5 partial ParamGate, 6 profile and 6 timing.
Format passed. The oracle compared 5173 cases with zero mismatches, including
zero/error inputs, named defaults and boundary/seeded mutations of all 29 RPC fields.
Overflow/large-width checks are explicit Rust controls, not part of that oracle corpus.

The initial review found a real test-coverage gap: replacing R20 ceil with floor
survived, because network and end/full counts divide exactly. The production ceil
was correct. Added 129/128 and 127/128 cases, reran the changed five-test suite and
the oracle, and all 21 isolated mutants were caught. Sixteen removals of && terms,
the first R26 predicate, accepting derive errors, wrong LAG, wrong ceil and per-item
grace each compile successfully and then fail an unchanged assertion. Every mutant
has a separate temporary source, library and executable, removed after use.

The initial survivor (exit 0) and interrupted verification ledger are preserved in
review-r1 alongside the exact original tests and runner. The corrected results are
in review-r2. Mutation cargo/test exit 101 is the expected fault detection result;
it does not mean the unmodified implementation failed. The unchanged 103 earlier
regression tests were not redundantly rerun after the test-only correction. The five
partial-gate tests passed again. Source bytes did not change after the initial build.

## Reproduction

From the repository root, with Rust 1.58.1 and locked dependencies available:

    python3 development/lp3/s5-paramgate/codex-r1/verify.py /absolute/fresh/evidence

The default run builds clean, runs the complete 108-test selection and all controls.
The original continuation used a second fresh evidence directory and the first
clean build target as an optional second argument, avoiding a dependency rebuild
and limiting regression repetition to the changed tests. No mutated library enters
the real crate target. Exact commands, versions, raw exits and durations are recorded.
The manifest and oracle input/output hashes allow independent reproduction.

M1, LP1/LP2 and S1-S4 evidence, workflows and dependency manifests/lockfiles compare
unchanged against f933ef8. See preservation.json. No Windows execution, full gate,
EVM/SYS/retention measurement, full PG-END verdict, S6-S9 or LP3 completion is claimed.

## Blocking decision

See DESIGN-DECISION.md for exact sources, contradictory outcomes from plausible
historical accounting interpretations, and two concrete resolution choices. The
accepted permission for labeled measured test fixtures is preserved; the open issue
is whether observational maxima suffice for the general derive API and smallest
SYS_SLOT_MAX requirement, or a deterministic general resource model is required.
The partial arithmetic must not be merged as an accepted S5 increment.
