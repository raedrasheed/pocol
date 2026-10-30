# LP3 continuation at the S7 CF-definition decision

S5 merged through PR #15 at 18cf7ab3f0140715db4365c8f893d46d2084a3f4 under explicit
LP3 amendment 01. S6 merged through PR #16 at 772cb1f843c9214b9a8575856d4b34e38b7976ae.
The automatic GitHub review of S6 later completed with no findings. Local separate
review and required latest-head CI had already passed before its normal merge.

Current branch: codex-lp3-s7-evidence. Numerical S7 gates and pure SV arithmetic are
implemented/reviewed; the draft checkpoint PR must not merge as complete S7 yet.
Discover/update its existing branch PR; never create a duplicate.

Resolve CF-DECISION.md: provide canonical CF1-CF6 vectors/mapping, or explicitly
approve the proposed LP3 core mapping to the unchanged closing/final consensus rules.
Then implement Rust/TS predicates and the concrete cases, independent native oracle,
boundary/overflow/negative controls and a separate review. No silent CF pass or
implicit change to M1 is authorized by the current checkpoint.

Continue the explicit T0 core matrix audit: map literal L0/L1/L2/header/identity/PoW
cases to executable evidence. Inherited LP1 fixture/codec checks are evidence of
their recorded scope, not a blanket pass of V1-V26 or H-full. Do not substitute a
caller-supplied success flag for an unavailable state/execution check. Track full
SV6/actual settlement/finalization as LP4 execution, as allowed by the LP3 boundary.

After S7 is actually complete, verify live GitHub checks/reviews/protection and merge
normally into foundation. Then perform S8 integrated seeded adversarial/mutation
review across schema, forks, timing, membership, Draw and gate bypasses; fix findings,
preserve evidence, publish its own branch/PR, and merge only if criteria pass.
Finish S9 and audit every required S0-S8 row before declaring amended LP3 accepted.

Numerical evidence already passed: 120 Rust tests, 9 TS groups, strict TS checking,
actual AS1-AS19 traces, one million full Rust/TS/native-integer comparisons, Decimal-60
ASQ grid, allocation counter and documented final benchmark gates, 4000 independent
SV arithmetic cases, eight isolated Rust review mutations. See RESULTS.json, raw
logs and source digests. Do not rerun expensive checks without a new relevant change
or unresolved concern; reproducible scripts are preserved for independent reruns.

Use the GitHub connector for publication/PRs. The terminal proxy route is unavailable.
Pinned TypeScript tooling is at /workspace/work/lp3-tscompiler, obtained as a public
GitHub Actions artifact through the connector with SHA256 verification; it is not
in the repository. Do not print credentials or temporary signed URLs.

Keep M1 and all old evidence immutable. Original general R18/R21 resource derivations
and caps/64 minimum SYS_SLOT_MAX stay explicitly DEFERRED LP4, NOT PASSED, with the
amendment's acceptance tests. All resource fixtures so far are synthetic. Boot stays
disabled. No Claude/coordinator/main merge/deploy/billing change. Standing execution
authorization continues after the actual design question is resolved.
