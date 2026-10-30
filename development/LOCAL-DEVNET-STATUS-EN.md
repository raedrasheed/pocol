# Local PoCol network progress

LP2 accepted: durable storage/recovery of verified candidates, with second-writer exclusion and corruption-detection evidence. The LP3 plan and first Claude task are ready on a separate branch: complete genesis schema, timing, owner assignment and proposer selection, then parameter gates, TypeScript model and expanded ASERT tests. The first LP3 increment was started manually at Raed's request in a cloud session, without the local coordinator.

**LP3 S1, first increment (unreviewed draft):** complete v1 GenesisSpec parser: six fields, 52 bounded CP values, ordered initial membership and reserved addresses, following the adopted error order. It reencodes GSV1 (341 bytes) identically and matches its frozen hash. Fourteen new tests passed; 30059 cases matched the accepted reference; 8 deliberately seeded defects were detected. Exclusive-writer/crash checks were not run on Linux because they are Windows-only; they are not passes, and S0 is incomplete. Windows execution and independent review remain pending; LP3 is not accepted. Details: `lp3/s1-genesisspec/author-r1/README.md`.

**Round two (repairing two review findings):** gsVersion error detail now contains the full decimal value as in the reference; removed the exception hiding 57 differences, reproduced with our own inputs rather than the reviewer's. `genesis-decode` now reads lines into fixed buffers and explicitly rejects lines larger than the largest potentially valid GenesisSpec (2,818,474 bytes); this is a tool limit, not a validity judgment. All 30075 cases matched without exceptions; the program survived a 64 MiB memory limit; all twelve deliberately seeded defects were detected. Windows checks and independent round-two review remain unperformed. Details: `lp3/s1-genesisspec/author-r2/README.md`.

**Round three (preparing Windows checks):** the Python harness now writes explicit input bytes because Windows text mode changed them (`\r\n` became `\r\r\n`). The defect was reproduced by simulating newline conversion on Linux; intended input bytes are preserved. Added a manual-only Windows GitHub Actions workflow using Rust 1.58.1, read-only permissions and no secrets, uploading logs/results with the tested commit. Not yet run: the repository is private, included account minutes could not be confirmed, and the workflow must exist on the default branch before manual dispatch. No Windows verification yet. Details: `lp3/s1-genesisspec/author-r3-windows-ci/README.md`.

**First Windows run (37972422364, commit `9c02c7f`):** full Rust suite, storage journal, real-process crash/recovery tests, both memory tests, fixtures, `e2e_http.py` and `e2e_store.py` passed. Four Python checks failed before any test because Windows Python read UTF-8 vector files containing Arabic using default cp1252. Fixed by explicitly specifying UTF-8 in every text read/write without changing test conditions. Original evidence is preserved unchanged in `lp3/s1-genesisspec/windows-run-37972422364/`; the repair awaits another Windows run.

**Second Windows run (37976178670, tested commit `4e26e4d`):** successful: 20 passes, zero failures, 16 platform-blocked rows, nothing unrun. All LP1/LP2 checks passed with the accepted LP2 counts: journal 12/12, both real-process crash/recovery tests and `e2e_store.py` 24/24. LP3 also passed: 30075 cases matched the reference, or 30077 with maximum-width gsVersion added. Inputs were byte-identical to Linux; memory peaks matched Linux values. The sixteen blocked rows are not sixteen cases: eight CLI cases require an OS-enforced process-memory limit unavailable on Windows. Seven repeat in debug/release, plus one summary row. They are not passes; their evidence is Linux-only, while Windows heap-level measurement passed. No implementation defect was exposed; recording/runtime observations did not change any result and were left unchanged. The uploaded archive is preserved unchanged, with SHA-256 matching GitHub, in `lp3/s1-genesisspec/windows-run-37976178670/`. Commits after `4e26e4d` change documentation/evidence only, not code/scripts.

**S0/S1 assessment:** each item has author evidence on Linux/Windows, but none is accepted yet: rounds two through four lack independent review, and a passing run alone is not acceptance. Next: independent review of PR #10 at `4e26e4d`, then a reviewer/owner decision whether Linux memory-limit evidence suffices or Windows needs an equivalent check. Details: `lp3/s1-genesisspec/S0-S1-ASSESSMENT.md`.

Accepting an LP3 contribution does not complete the stage; every criterion needs its own evidence. Old fixture roots are non-bootable and cannot pass R24. Full execution and real genesis await LP4, followed by production, sync, contracts and browsing. No live network, deployment or merge yet.

Current blocker: automatic approval review rejected the next Claude-task command with `CreateProcess rejected: blocked by policy`, without another reason. No part executed and no second author task started. The rejected action was not retried through another route. Resume checkpoint/task preserved; sequential authorization stands, but automatic author launch awaits removal of the access restriction. The manual increment above does not remove it. Evidence summary: LOCAL-DEVNET-BLOCKER.md and draft #7.

### Codex update — 17 October 2026

Under the owner's current instruction, work continues through Codex only; earlier Claude/coordinator instructions are historical and are not used to resume work. Neither was launched.

Fetched branches and PR #10 head: `bf2000fbca6ac090779de8955088e77504c7e298`, with main at `15a38774d9b47f4282366d7b294a1d8b8e919b0d`. All code/tests/CI files match Windows-tested `4e26e4d3049731645ab1d85ccfb9972d29d63390`; M1 and earlier evidence preserved. Locally passed 49 library tests, 16 GenesisSpec tests, 6 allocation cases, a fresh-seed 30075-case comparison, 217 additional reviewer cases and 9 CLI checks (7 under an OS memory limit). Rechecked the saved Windows archive and digests without claiming a new Windows run or executing historical ChatGPT review results.

**Technical verdict:** accept S0/S1 for this scope; close both first-round findings. The reviewer judged Linux RLIMIT_AS evidence plus Windows heap-allocation evidence sufficient for the written criterion; no Windows OS-limit/allocation-failure testing or recovery from all out-of-memory conditions is claimed. Details, commands and limitations: [review](lp3/s1-genesisspec/codex-review-2026-10-17/REVIEW-S0-S1.md), [acceptance record](lp3/s1-genesisspec/codex-review-2026-10-17/ACCEPTANCE.json).

Live PR metadata/comments/reviews/latest runs remain unverified: the network proxy blocks api.github.com. The old Draft status is therefore not assumed current. A domain addition is saved in an environment-configuration draft and must be applied before retrying. The older PR-branch workflow allows a missing SHA; future acceptance runs must use main's stricter workflow with a full SHA. No blocking implementation defect, specification/code edit, commit, push, merge or new stage. LP3 overall is not accepted.

### Next increment S2 — Codex, 17 October 2026

Confirmed PR #10 merged into `work/lp3-consensus-foundation` at `d17273af35880f0e1d8a438e529e69c22a18f731` by fetching Git, checking both parents and matching the accepted-head tree. Selected S2 as the next incomplete LP3 criterion after S0/S1.

Implemented 52 typed CP values, ForkSchedule supporting only v1, hash/chainId binding and profile/node-kind gates. The result is always non-bootable; no substitute for actual builder verification or full ParamGate. Rust 1.58.1 build, six new tests and formatting passed; tests detected five seeded defects in temporary copies. Heavy S1 tests were not repeated; M1/earlier evidence unchanged. Implementation `a9f0460335fa9594b943e125dd77d780042eefec`; this is Codex self-review, not independent acceptance. Evidence: `lp3/s2-profile/author-codex-r1/README.md`.

New branch `codex-lp3-s2-profile` targets foundation through a separate draft PR; creation depends on GitHub API access, still blocked at last check. Claude/coordinator not launched; nothing merged into main. Independent S2 review, S3–S8 and full LP3 acceptance remain incomplete.

S2 branch/evidence pushed successfully. `gh pr create --draft` failed with `api.github.com/graphql: Forbidden`; no PR created or number invented. Prepared text/result saved in `DRAFT-PR-BODY.md` and `PR-CREATION-STATUS.json`. External remainder: create draft PR against foundation once API access is available, then independent review.

### Separate S2 review — Codex, 17 October 2026

Reviewed `f7b521591fa5b3d9a039c8157e0a50d1acee2a86` against S2/M1, not relying solely on the author report. Technical verdict: PASS within non-bootable library scope, no blocking implementation finding. Checked explicit mapping of 52 fields, all u16 version values, 1000 generated cases, hash/chainId binding, kind gates, private state and bootable=false. Passed 83 regressions, five reviewer suites, Linux writer rejection, fixtures and two compile-time private-state access-rejection tests.

Initial failure preserved: the shared author build cache retained a removed temporary-mutant path, causing 12 fixture failures. A clean separate target build passed all tests; this is a nonblocking reproducibility observation, with separate targets recommended per future mutant. Earlier evidence not rewritten. Windows platform-blocked; full ParamGate, builder, boot and S3–S8 unrun and outside acceptance. This is a separate Codex pass, not an external reviewer identity or GitHub approval. PASS/FAIL/BLOCKED BY PLATFORM/NOT RUN details: `lp3/s2-profile/review-codex-r1/REVIEW-S2.md`, `S2-ACCEPTANCE.json`. No merge or S3 start.

### S3: timing arithmetic and separate review — Codex, 17 October 2026

Fetched foundation and confirmed S2 merge at `30daceb7e8c632041fd44a40afec57553f85fff2`. Started `codex-lp3-s3-timing` for S3 only. Implementation `c7008ae980d2c0a19a6d5c6ef26c61f7b73de346` adds u128 L/U and attempt-bound arithmetic, fallback/proposer timestamps, exact millisecond T_j with equality included, transition/future rules with correct inequalities, and σ/Xh/XW derivation: network 14/22/36, P11 4/6/10. No sleep, system clock, scheduler or boot permission.

Passed 89 regressions, six S3 tests containing 10000 generated adversarial cases, 10524 comparisons against an independent Python reference without differences, fixtures and formatting. Six mutants detected with isolated copies/build directories. Separate Codex review completed without blocking implementation findings; not external approval or full LP3 acceptance. M1/all earlier evidence unchanged. Windows platform-blocked, not counted as passing; actual scheduling, jitter measurements, S4 and full ParamGate outside scope/unrun. Evidence: `lp3/s3-timing/codex-r1/REVIEW-S3.md`, `RESULTS.json`. Branch targets foundation through a draft PR; no merge or S4 start.

S3 branch/evidence published successfully. Draft-PR creation through gh failed with `api.github.com/graphql: Forbidden`; no PR or number created. Description saved in `lp3/s3-timing/codex-r1/DRAFT-PR-BODY.md`; comparison link/result in `PR-CREATION-STATUS.json`. Use the link to create a draft against foundation; no need to re-add the domain or rerun completed tests.

### S4: membership, nonce assignment and Draw — Codex, 17 October 2026

Fetched foundation and confirmed S3 merge at `866666e2f129c26575409d07d8b7b4fa11899f78`. Implemented S4 only on `codex-lp3-s4-assignment` at `71cbbcc5cfed12eda204e29579f31cf672e7d972`: A_h* from parent-state inputs, seed/tickets, bounded/unbounded nonceMax, complete half-open partition including empty ranges, nonce owner and Draw weights over `[h−1−Ww,h−2]`, ordered selection and 256-bit bias rejection. No signing, private keys, parent-state attestation or boot permission.

Passed 103 regressions and 697 full scenarios against independent Python/hashlib (including 7977 sampling cases and membership limit 65535/beyond); eight mutants detected with fresh separate build directories. Separate Codex review completed without blocking implementation defects; no unmet S4 arithmetic/API criterion under M1 A1. Initial reviewer-tool compilation failure/correction preserved; not an implementation failure. Windows platform-blocked, not PASS; live parent-state validation, ledger, EVM integration and S5 unimplemented/outside increment. Reviewer/owner approval remains separate; no full LP3 acceptance. Evidence: `lp3/s4-assignment/codex-r1/REVIEW-S4.md`, `RESULTS.json`. M1/earlier evidence unchanged; no merge, Claude/coordinator launch or S5 start.

S4 branch/evidence pushed successfully. Draft-PR creation failed with `api.github.com/graphql: Forbidden`; no PR created or number assumed. Comparison link/prepared description: `lp3/s4-assignment/codex-r1/PR-CREATION-STATUS.json`, `DRAFT-PR-BODY.md`, targeting foundation only. No need to re-add the domain or rerun completed tests.

### S4 acceptance/merge and published-environment check — Codex, 17 October 2026

Confirmed existing PR #14 and checked head `a59fbc569c6c63bb4c92bc5d9745f37547579f52`/evidence without duplicating it. Fresh offline locked Rust 1.58.1 run: 103 passes, zero failures; formatting passed. Reran independent Python reference with the saved reviewer tool: 697 scenarios, 7977 sampling cases, zero differences; the tool was not rebuilt in this follow-up. Digests matched 32 evidence files; M1, earlier increment evidence, dependencies and CI unchanged. Earlier eight-mutant evidence preserved, not repeated.

GitHub Codex review of the same head completed without findings; no formal GitHub approvals or recorded CI checks. Foundation metadata reported no protection and an empty ruleset list; protection-detail read returned 403 due to connector integration limits. PR-description update succeeded, establishing actual PR-write permission.

Under current authorization and after satisfying S4 arithmetic scope, accepted the increment and merged PR #14 into foundation only at `c662bff29fb748594224eb48d329d9b2aff1244e`, using normal GitHub merge guarded by the expected head. No protection bypass or main merge. No full LP3 acceptance: S5 onward remain; no new Windows run, live parent-state verification or boot permission.

Environment api.github.com probe returned curl 7/HTTP 000 because inherited proxy:8080 refused connection; failure preceded GitHub, not evidence of authentication or destination-policy rejection. Separate connector authenticated as raedrasheed and read the repository, updated/merged the PR; no credentials displayed. No Claude/coordinator, deployment or billing changes.

### Partial S5 and documented design blocker — Codex, 17 October 2026

Started a separate branch from foundation at f933ef8 after S4 merge c662bff. Added precisely specified R16/R20/R26 arithmetic, without a boot gate or S5-completion claim. 108 regressions passed, then five amended tests after closing a ceiling-rounding detection gap; 5173 cases matched an independent Python reference. All 21 mutants detected after repair; initial survivor and logs preserved.

A semantic blocker remains: general SYS derivations in R18 and historical-trie bound in R21 are unspecified in the reference, while tests require derivation for varying profiles. Constants or synthetic measurements cannot establish a general bound. Evidence/options: lp3/s5-paramgate/codex-r1/DESIGN-DECISION.md; review/commands: REVIEW.md and RESULTS.json. Partial draft must not be merged as S5 acceptance. S6–S9/full LP3 remain unrun; blocked/unrun checks not counted as passes. M1/S0–S4 evidence unchanged; no Claude/coordinator call.

### S5 after adopted amendment 01 — Codex, 17 October 2026

Owner explicitly adopted option one as an LP3 amendment without changing M1. Added R1–R26 evaluator binding resource/identity/calibration facts to the complete profile, CP, resource-model version and nonzero source digest. Missing facts, binding/version mismatch or missing stage coverage prevents admission. Synthetic facts, measurements and proven bounds are explicitly classified; this round's fixtures are synthetic only and boot always disabled.

Final version passed 115 regressions, 1450 independent comparisons and 32 mutation controls; earlier RPC evidence (5173 cases/21 controls) preserved. Initial tool/test errors and corrections preserved, not counted as passes. S5 evaluator technically accepted within amended scope, pending GitHub checks/merge. General R18/R21 derivations and caps/64 lower bound deferred to LP4, not passes under original LP3 criteria; acceptance tests explicit in LP3-AMENDMENT-01.md and acceptance matrix. Details: lp3/s5-paramgate/codex-r2. S6–S9/full LP3 remain.

### S5 merged and S6 review — Codex, 17 October 2026

PR #15 merged into foundation at 18cf7ab under amendment 01. Separate S6 branch adds a TypeScript model for integers, ASERT, timing/membership/partition/Draw, GenesisSpec and Keccak. Strict TypeScript 5.9.3 typechecking on Node 22 passed in GitHub and locally with a digest-pinned tool; Node 24 type stripping is an additional runtime check, not typechecking. Initial CI failed type narrowing; corrected without weakening strictness, result preserved.

Eight test suites passed, plus 12000 ASERT comparisons with Python/Rust, 10000 timing cases, 1200 Genesis cases with Python/Rust and 697 assignment scenarios with 7977 sampling cases, no differences. Twelve mutants detected; reviewer-tool failures/corrections preserved. M1, earlier evidence and Rust unchanged. S6 targets PR #16; final-head requirements must pass before merge. S7–S9/full LP3 remain; resource obligations deferred to LP4 remain explicit, not passing.

### S6 merged and partial S7 results — Codex, 17 October 2026

S6 merged through PR #16 into foundation at 772cb1f after final-head checks and separate review passed. GitHub automated review later completed without findings; it was not counted as successful while running.

Separate S7 branch passed 120 Rust tests, nine TypeScript suites and typechecking. One million complete path/target cases matched across Python/Rust/TS, covering all 601 s values from −300 to 300, with no differences or intermediate above 512 bits. ASQ passed 65536 fractions/1536 boundaries; maximum relative error approximately 0.0001173. Rust counter recorded zero allocations in 1069 calls with a positive allocation control; performance gates passed under documented methodology. 4000 SV arithmetic cases matched across Python/Rust/TS; eight mutants detected. No real Escrow/SV6 execution or boot permission.

S7 not accepted or merged: CF1–CF6 definitions occur only as names in the preserved M3 list; final T0 scope audit still required. Explicit proposed mapping to closing/finality rules recorded in lp3/s7-evidence/codex-r1/CF-DECISION.md without changing M1 or claiming those gates passed. Resume handoff preserved; S8–S9/full LP3 remain. General resource derivations and caps/64 lower bound remain deferred LP4 obligations, not passes.

## S7 update: CF approval and core tests

Owner adopted CF1–CF6 mappings as new definitions separate from M1. Rust/TypeScript passed 16000 cases against an independent reference, 127 Rust regressions, 10 TypeScript suites and typechecking. Tests detected 11 Rust mutants and 6 TypeScript CF mutants. Limited semantic T0 audit completed; actual finalization, cursor, absent pending[F] and zero final subsidy remain unimplemented LP4 work. codex-r1 evidence preserved, codex-r2 added. PR #17 awaits GitHub checks before merge; S8/S9 not yet accepted; boot disabled.

## S8 update: core audit and Windows regression

S7 merged at 5a04bdf; automated review later completed without findings. S8 locally passed 131 Rust tests, 11 TypeScript suites, typechecking, 24000 fresh adversarial cases and 45 real HTTP checks. Independent references matched and detected 23 compiled mutants. Attempt failures preserved, including initial loopback denial and unsupported Linux writer. PR #18 is a draft awaiting Windows/final-commit checks; S8/LP3 not accepted. M1, earlier evidence and disabled-boot decision unchanged.

## Final LP3 acceptance audit under amended scope

S8 merged at c054dad after both final-commit 56cda6f checks passed and automated review completed without findings. Windows evidence confirms 145 Rust tests, 45 HTTP checks, 24 storage checks and 30075 matching Genesis cases. Nine Linux memory-bound checks also passed. Eight Windows checks remain platform-blocked and two optional checks unrun; none counted as passes.

Final S0–S8 audit passed, verified 903 digest entries and rejected missing/running/failed checks. Durable S9 evidence, acceptance matrix and review report saved in development/lp3/s9-acceptance/codex-r1. Amended-scope LP3 acceptance takes effect when the S9 documentation PR merges into foundation. CF definitions newly adopted; M1 unchanged. General R18/R21 derivations, caps/64 bound, real finalization/cursor/pending[F]/final subsidy and SV6 remain unaccepted LP4 obligations. Boot disabled; no main merge, deployment or billing changes.

## LP4 start: plan and MPT roots

P0 plan merged through PR #20 at 9d3f99a; all 23 implementation obligations remain unpassed. Local P1 MPT-root implementation passed 134 Rust tests, 1218 comparisons against an independent insertion reference and 8 compiled mutants failing explicit assertions. Cases include 1024 accounts and 31/32-byte boundaries. Not GenesisBuilder, EVM execution or R18/R21 proof. Await final-commit GitHub/Windows checks before merge; boot disabled. Owner asked to map new T2/T2b/T2c/T24/T26 tests because accepted sources list names without defined cases; no answer assumed.

## LP4 P1 accepted only as a prerequisite

PR #21 merged at 93ca35b after Windows passed on final commit 3423bbf and automated review completed without findings. Windows passed 150 Rust tests; source/run/result evidence saved in lp4/p1-trie/codex-r2. Eight platform-blocked checks and two optional unrun checks not counted as passes. LP4 matrix expanded to link each obligation to source, responsible increment and required evidence; all 23 remain unpassed, boot disabled.

Required owner decision before dependent increments are accepted: map T2/T2b/T2c/T24/T26 as new LP4 definitions under the published proposal, or provide originals. No answer assumed. Earlier M1/LP3 acceptance/evidence preserved.

## Owner decision: LP4 test definitions

Owner adopted T2/T2b/T2c/T24/T26 mappings as new LP4 definitions, not original M1 definitions. Approval recorded separately without changing M1 or P0/P1 evidence. Test-definition blocker removed; P2 begins. All 23 implementation/resource obligations remain unpassed until actual evidence, independent comparisons, bounds and compiled controls. Boot disabled; other missing labels will be collected in one proposal.

### Claude update — 21 October 2026 (LP4–LP10)

Work on `claude/affectionate-pascal-rjhh2k` under [standing authorization](OWNER-STANDING-AUTHORIZATION.md). No main merge, deployment or real funds. Stage reports:
- [LP4–LP8](reports/STAGE-LP4-LP8-EN.md)
- [LP9](reports/STAGE-LP9-EN.md)
- [LP10](reports/STAGE-LP10-EN.md)

Currently working locally:
- **Nodes:** real pocold nodes (Rust/revm) produce PoCol blocks from fresh genesis. H-pre/H-full validation, D133 fork choice, durable journal, inter-node synchronization and RPC.
- **Execution:** system contracts, settlement and finalization match SV vectors.
- **On-chain sites (LP8):**
  - Website contract and chunk factory.
  - Publisher/fetcher verifies every chunk/file.
- **Chrome extension (LP9):**
  - RP or LN verification, three-layer network isolation.
  - BridgeAuth, generation-fenced local storage and local wallet.
- **Integrated scenario (LP10):**
  - Partition/heal with Chrome-visible reorganization.
  - SIGKILL and full restart.
  - Matching roots and journal replay.

Independent reviews:
- Two independent read-only agent reviews: extension, then Website/publishing/replay.
- All findings repaired with regressions, including one high finding: bind block body to header roots before any checks.
- Records: [LP9](lp9/REVIEW-RESOLUTION.md), [LP8](lp8/REVIEW-RESOLUTION.md).
- These are author/agent-review evidence, not formal Codex/owner acceptance; formal acceptance remains required by the plan.

Unclaimed items explicitly documented in [LP9-EXTENSION.md](lp9/LP9-EXTENSION.md) and [LP10-RUNBOOK.md](lp10/LP10-RUNBOOK.md).
