# LP3 acceptance under the explicitly approved scope

This is the final S0–S9 audit for `work/lp3-consensus-foundation`. Technical acceptance
is limited to LP3 core/reference computations with boot disabled. The resource
amendment and newly approved CF definitions are recorded separately; M1 and original
requirements/evidence remain unchanged. Acceptance becomes effective when this
S9 documentation PR merges normally into foundation after its GitHub requirements.
It never authorizes main, deployment, a live network, or full execution acceptance.

| Criterion | Verified acceptance evidence |
|---|---|
| S0 | Preserved LP1/LP2/M1 and decisions; immutable baseline comparison; prior accepted S0/S1 review plus current full supported Windows regression and local HTTP/regression. Exact toolchain, commands, exits, hashes and license provenance retained. |
| S1 | Full six-field/52CP GenesisSpec schema, exact literal 341-byte GSV1/Keccak K1–K3, L4n/order/deep and maximum framing tests. Original accepted S0/S1 review covers Linux OS limits plus Windows heap bounds; current Windows differential agrees on all 30075 cases, current full tests/allocator bounds pass. Optional full-size reference reruns not silently counted. |
| S2 | Full typed CP widths/minima/enums, validated v1 ForkSchedule, identity/hash/root/profile/node gates, private validated views, bootable=false. S2 review plus current profile/identity tests and fork/R24 negative controls. Actual builder outputs remain LP4. |
| S3 | u128 bounded timing/window/fallback/attempt arithmetic, equality/future/overflow, network Xh=22/XW=36 and P11 6/10; exact TJ core jitter/deadline cases and independent 10000-case replay. Physical timers/callbacks remain LP6. |
| S4 | Pure parent membership/nonce assignment/owner/Draw: sealed hash domains, range coverage, full 2^64 endpoints, weight window/CDF/unbiased rejection and retry limits. Independent 697 scenarios/7977 samplers replayed; boundary/mutation controls. Supplied parent facts are not state certificates. |
| S5 | R1–R26 ordered evaluator with missing-fact rejection under amendment 01; numeric resource facts bind complete CP/profile and model version, numeric gas/memory/timing checks, explicit provenance classes; no supplied-ok shortcut. 1450 independent cases, 32 controls, named PG-END/PG-L/PG-RPC and exact rational default metrics. Original derivation obligations below are NOT PASSED. |
| S6 | Proper pinned TypeScript strict typecheck, Node22 CI and executable reference model. Rust/TS/Python integer/hash comparisons: arithmetic, Genesis identity/negative errors, timing and assignment. Deep/shallow vectors match; no type-stripping-only typecheck claim. |
| S7 | Actual Rust AS1–AS19 traces and targets; 1000000 seeded complete comparisons with all 601 exponents and zero mismatches. ASQ all65536 fractions, max relative error .000117295624 <=.0002 and 1536 monotonic boundaries. ASB zero heap with positive counter control, <=512 bits, documented sequential batch benchmark <=1ms/<=10x AS1. Newly approved CF core: 16000 independent Rust/TS/oracle cases and boundary/mutation tests. T0 semantic core and SV supplied-fact arithmetic have actual receipts, with explicit execution exclusions. |
| S8 | 131 portable Rust tests, eleven TS groups/typecheck; new 12000+12000 seeded envelope/Genesis corpora; independent oracle replay; eleven compiled Rust and twelve compiled TS controls; 45 real HTTP process checks. Full supported Windows Rust, journal/crash/process and memory-allocation tests inspected. Failed attempts, platform blocks and optional not-run cases remain visible. Separate sequential review closes findings. |
| S9 | Arabic milestone dashboard and per-increment PR evidence preserved; this final matrix, independent evidence audit, negative acceptance controls, supported-platform receipts, final GitHub checks/requirements and foundation-only merge records. No unrun or blocked check counts passing. |

The numerical benchmark statistic is a median of eleven batch means after warmup,
not proof of worst-case single-call latency or a Windows/production timing bound.
Synthetic maximum envelopes prove shape/PoW math, not full real-block authentication.
T0 semantic generated inputs are not asserted to be absent literal V1–V26 bytes.
CF1–CF6 are newly owner-approved core definitions, not recovered original CF labels.

## Original obligations explicitly deferred — not passes

- General R18 resource derivations and general R21 trie bounds, and the caps/64
  minimum SYS_SLOT_MAX: mandatory LP4 tests in LP3-AMENDMENT-01.md.
- Real GenesisBuilder allocation MPT/syscode execution and authenticated measured or
  proven resource facts replacing synthetic fixtures before production acceptance.
- Actual finalization/cursor mutation, absence of `pending[F]`, zero final subsidy,
  real child-of-final rejection/state effects: mandatory LP4 tests in the separate
  LP3-CF-CORE-APPROVAL.md. SV6/real Escrow, account/balance/burn transitions remain
  execution work; pure arithmetic is not a pass for them.
- Full H acceptance/execution, physical scheduler, live consensus/network acceptance
  and boot authorization remain outside this LP3 acceptance.

Windows has no RLIMIT_AS: its blocked CLI limit rows are not passes. The accepted
S0/S1 review explicitly accepted Linux OS-limit plus Windows counting-allocator
coverage. Original full-size Windows evidence is preserved. The current Windows
run's optional full-size reference CLI/differential reruns are NOT RUN, not failures
and not fresh passes. All failures during this task are retained with corrections.

Reproduce the final read-only evidence gate from the repository root with:
`python3 development/lp3/s9-acceptance/codex-r1/audit.py development/lp3/s9-acceptance/codex-r1/github-receipts.json work/lp3-final-audit`
It verifies required live receipts, exact tested source, supported-platform statuses,
prior manifests and baseline preservation, scope-specific numerical controls, and
rejects missing, pending and failed GitHub checks. It does not execute deferred work.
