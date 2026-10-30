# Codex S0/S1 acceptance review — 2026-10-17

## Scope and exact revisions

Reviewer: Codex, executing this review directly. No Claude invocation, coordinator restart, merge,
specification change, or later milestone. The old Claude/coordinator instructions in
`development/README.md` are historical and superseded by the owner's current instruction.
No AGENTS.md was found under /workspace.

- Freshly fetched PR #10 head: `bf2000fbca6ac090779de8955088e77504c7e298`.
- Freshly fetched `work/lp3-genesisspec-s1`: the same head.
- Freshly fetched main: `15a38774d9b47f4282366d7b294a1d8b8e919b0d`.
- LP3 foundation: `fd7a17fd51691b275343b0cbc96bfce1d2f90745`.
- Windows-tested implementation: `4e26e4d3049731645ab1d85ccfb9972d29d63390`.
- Local build/tests: unmodified source at `bf2000fbca6ac090779de8955088e77504c7e298`.
  `preservation.json` verifies that all `prototypes/` and `.github/` blobs equal `4e26e4d`.

Git access succeeded. GraphQL and REST requests were denied by the network proxy
(`CONNECT tunnel failed, response 403`). Consequently this review does **not** assert current
Draft/open/merged state, current comments/reviews/checks, or that no newer Actions run exists.
PR head and branch tips were inspected live through Git; PR metadata could not be inspected live.
The Windows evidence below was read from the fetched repository, not downloaded anew from Actions.
The separate ChatGPT review described by the owner has not been located in this fetched history;
none of its reported checks is counted as a check executed in this session.

## Evidence verified now

`commands.json` records fresh build/test commands, working directories, durations and exit codes.
Rust 1.58.1 / Cargo 1.58.0 compiled the crate with the unchanged lockfile, offline, from
checksum-verified vendored dependencies. Cargo 1.99.1 was used only to acquire/vendor dependencies
through the sparse registry, because the old Cargo Git-index fetch stalled. The old fetch was
terminated; an attempted old-Cargo `--config` override was unsupported, then replaced with the
supported CARGO_HOME/config.toml. No dependency or source edits were made.

- Targeted Rust run: 49 library tests, 16 GenesisSpec tests, 6 allocation cases passed.
- Fresh differential seed `20261017`: 30,075/30,075 agree, zero mismatches/refusals.
  This is a new corpus, not the preserved Windows seed. Every error detail is compared,
  including gsVersion; the round-one exception is absent.
- Reviewer-authored independent construction/Python-integer oracle: 216 rejection cases plus
  literal 341-byte GSV1 passed. Includes decimal conversion thresholds and leading zeros,
  huge declared lengths, crossing list boundaries, error precedence, depths 1500/1501/100000,
  and GSV1 exact re-encoding with bootable=false. See `independent_checks.py` and its JSON result.
- Preserved Windows evidence checker: all 102 checks passed (0 problems). SHA256SUMS also passed.
  The checker was read, not treated as a substitute for code review.

## S0 assessment

PASS for the reviewed source snapshot. `preservation.json` independently compares Git trees:
M1, LP1/LP2 evidence and decisions are unchanged from the LP3 base; dependencies, fixture inputs
and licenses are unchanged from accepted LP2; each previous author round and the failed Windows
run are unchanged from their recorded round commits. The current review adds new evidence only.

The stored successful Windows run identifies `4e26e4d`, main workflow SHA `15a3877`, toolchain,
commands, exit codes and source hashes. Every extracted artifact file equals its zip entry;
the zip matches the digest in the stored job log. All 52 source hashes map to tested Git blobs
(46 through LF-to-CRLF conversion, 6 unchanged). Pinned M1 inputs remain byte exact.

Reuse, rather than rerun, Windows-only regression evidence: 98 ordinary Rust tests passed;
store_journal 12/12, store_process 2/2 (the intentionally ignored helper is invoked by those tests),
HTTP 45/45, store end-to-end 24/24, fixtures 22/1027/3677 with K1–K3, builds and rustfmt passed.
These are preserved historical Windows executions, independently checked now, not fresh Windows
tests. No unrelated full M1 rerun or known-incompatible Linux storage rerun was needed.

## S1 assessment and closed findings

The CP table was checked against accepted consensus.md: six fields, all 52 CP widths/minima/enums,
gamma coupling, sorted/unique/nonreserved member IDs and reference error order. The parser checks
actual bytes before declared lengths drive traversal; it keeps iterative end offsets, not recursive
input trees. The decoded result is allocated only after schema validation. The batch reader reserves
two fixed buffers fallibly, streams/discards oversized lines and resumes on the next line.
The old scoped LP1 decoder is unchanged and is not used by this path.

The 16 existing GenesisSpec tests cover literal/hash identity and frozen K1–K3, all supplied L4n
negatives/supplements, deep/shallow error twins, bounds and error-order conflicts. Fresh execution,
fresh-seed differential and reviewer-written checks support the source inspection. Existing Windows
release-max evidence additionally covers 30,077 cases and full-size gsVersion; that costly corpus
was not rerun unnecessarily on Linux.

Round-one finding 1 (masked gsVersion details): CLOSED for this snapshot. Exact decimal rendering
is checked by fresh differential and independent Python integer comparisons around conversion
thresholds; no comparison exemption remains.

Round-one finding 2 (unbounded whole-file input): CLOSED for this snapshot. The fresh CLI
harness passed 9/9 checks (7 with RLIMIT_AS, 2 without a limit). The 192 MiB lines were refused
with exit 1 under 64/256 MiB; processing resumed after an oversize line; 100000 GSV1 inputs
completed under 64 MiB; the 8 MiB reservation case exited 2. No case was signalled or aborted.
See cli-memory.json. S1 passes within the explicit platform/resource scope below.

## Windows memory-limit decision

Reviewer decision: Linux OS-limit evidence plus Windows allocation evidence is sufficient for the
S1 criterion "bounded bytes before allocations" for this increment. S1 does not require a Windows
Job Object or universal OOM recovery. This does not count any blocked row as a pass.

The 16 blocked rows are 8 distinct cases: 7 repeated in debug/release, 1 release-only, plus a
summary row. Windows demonstrates bounded requested heap and real decoder execution; Linux
demonstrates process behavior under RLIMIT_AS. No Windows OS-limit test or reservation-failure
execution is claimed. A Job Object test is optional additional platform assurance, not an open
acceptance blocker under this reading of the existing criterion.

Ordinary decoder allocations can still abort below their working set. Only initial buffer
reservations are explicitly fallible; this is documented by the author. Neither this decision nor
the test results promise graceful handling of every possible memory limit or production readiness.

## Workflow drift and remaining operational limitation

The PR workflow allows an empty commit and falls back to github.sha; it lacks main's explicit
HEAD/script-existence/script-hash confirmation. This is a concrete provenance weakness for future
dispatches, not a failure of the recorded successful run: that run used main's stricter workflow
and an explicit SHA, and its stored log verifies the checkout and script hash.
Use main's strict workflow and a full SHA for subsequent acceptance runs; align the branch workflow
before relying on branch-selected dispatches. No workflow was modified in this review.

GitHub API access remains necessary to finish the live PR metadata/comment/check inspection.
An additive api.github.com network requirement was saved in the environment draft; saving does
not apply or publish it. After it is applied, retry native API reads before diagnosing credentials.
Git proxy authentication must not be confused with API authentication.

The local review is not a GitHub approval or merge. S0/S1 acceptance applies only to this increment;
contribution 1 and LP3 remain incomplete (S2–S8 and other contribution-1 APIs are outside scope).

## Verdict

Technical acceptance: S0 and S1 PASS for the exact reviewed source snapshot, with the Windows
platform decision above. No blocking implementation finding remains. Live PR metadata inspection
remains blocked externally; this report does not complete that operational part of the request.
No GitHub approval, PR update, commit, push or merge was performed.
