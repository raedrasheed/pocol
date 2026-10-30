# LP3 S3 — implementation and separate review, 2026-10-17

## Scope and provenance

Foundation fetched at owner-specified merge `30daceb7e8c632041fd44a40afec57553f85fff2`, with parents d17273a and e79ada0; its tree equals the accepted S2 head. New branch `codex-lp3-s3-timing`, target `work/lp3-consensus-foundation`. Implementation/test commit **c7008ae980d2c0a19a6d5c6ef26c61f7b73de346**. No AGENTS.md exists under /workspace. Read development/README.md and LP3-MILESTONE.md; the owner's Codex-only instruction supersedes the historical Claude/coordinator workflow. Neither was invoked.

Selected S3 after merged S0/S1 and S2. M1 is unchanged. Normative formulas: consensus.md §3 and §4(a,c), threat.md timing section, governance.md named network/P11 values. RESULTS.json pins reference hashes. No new validity rule, timing assumption or accepted-spec amendment was introduced.

## Implementation

`timing.rs` adds pure arithmetic APIs:

- Validated inclusive L(a)=p.ts+a*D_att+1, U(a)=p.ts+(a+1)*D_att. All intermediate arithmetic is u128; zero duration and U>=2^63 reject before conversion. Attempts are protocol u32. max_attempt computes the largest representable valid attempt, or None if even zero cannot fit.
- Fallback timestamp L(0)+Delta_fb_wait at a=0, L(a) thereafter; invalid first fallback timestamps reject rather than clamp. First proposer timestamp max(L(0),C_P) rejects above U(0).
- Strict C>U(a)+Gamma transition-time predicate and a>=1 fallback readiness at C>=L(a). Caller must also enforce max_attempt before advancing; no successor FSM is inferred from the time predicate.
- T_j=max(b_j+Delta_rel,L(0)+Delta_fb_wait) computed exactly in milliseconds, preserving fractional receipt time. u128 receipt addition is checked. Arrival at exactly T_j is timely; +1 ms is not.
- Strict future predicate ts>C+Phi with wide addition. No future queue is implemented by this scalar predicate.
- sigma=max(Delta,Phi+1)+t_exec; Xh=max(Delta+t_exec,Phi+1)+Delta+2*t_exec; XW=max(Phi+1+Delta_fb_wait,Delta+t_exec+Delta_rel). Network sigma/Xh/XW=14/22/36; P11=4/6/10.

No wall-clock reads, sleeps, timers, unsafe code or scheduler dependency. This is an experimental library core, not new node startup, full FSM, signature/Draw validation, ParamGate or a boot authorization. Existing RP-window implementation and all previously accepted source remain unchanged except lib.rs exposing the new module. S4 was not started.

## Separate review pass

After committing the implementation, review used a fresh `/workspace/pocol-s3/review-target`, not author or mutation artifacts. Read each API against the accepted formulas and examined overflow/units/error boundaries. Addition/multiplication of u64/u32 timing inputs fits u128; only arbitrary u128 receipt-time addition requires checked_add. A validated window has L<=U<2^63, making narrowing safe. max_attempt subtracts only after checking parent<2^63 and handles the zero-slot case before subtracting one. Private window fields prevent invalid construction. Pure predicates do not assert unrelated signature, FSM or boot facts.

A new Python arbitrary-integer oracle (`oracle.py`) derives expected values separately from Rust. `oracle_probe.rs` only calls/prints actual APIs. All 10,524 cases matched, including 5,299 valid windows, invalid/limit cases, extreme metric inputs, millisecond boundaries and receipt overflow. Seed/input/output hashes are in review-oracle.json. No implementation finding remained in this pass. This is a separate Codex review pass, not a different external reviewer or formal GitHub approval; the increment remains a draft proposal for acceptance.

## Results, without conflating outcomes

| Check | Outcome |
|---|---|
| Offline Rust 1.58.1 build, formatting | PASS |
| Clean regression: lib 49, fixtures 12, GenesisSpec 16, S2 6, S3 6 | PASS: 89 tests |
| New S3 tests, including 10,000 seeded adversarial windows | PASS: 6 tests; cases are not additional test counts |
| Separate Python integer differential | PASS: 10524/10524, zero mismatches |
| Fixture CLI | PASS: window 22, ASERT 1027, hashes 3677, K1-K3 |
| Six isolated mutation controls | PASS: 6/6 detected through assertion failures |
| Unmodified implementation failures | None observed (FAIL count 0) |
| M1/prior evidence/.github/lockfile preservation | PASS |
| New Windows execution and Windows-only store/crash checks | BLOCKED BY PLATFORM; Linux host, never counted PASS |
| Full S1 heavy differential/allocation reruns | NOT RUN: unchanged code and preserved prior evidence |
| Real-time jitter, running scheduler/FSM/LP6 callbacks, S4, full ParamGate/builders | NOT RUN: outside this increment |

Mutation variants: accept U=2^63, reject arrival exactly at T_j, mark future at equality, advance at equality, delay later-attempt fallback, remove one t_exec from Xh. Each copy uses its own checkout and target directory. Raw cargo exits are 101 and logs contain failed assertions, not compile failures. This is successful negative-control detection, not passing faulty variants. Two mutations run concurrently with two Cargo build jobs each on a five-CPU machine; available RAM/disk were checked first. Dependencies come from unchanged checksum-verified vendored sources.

## Reproduction and preservation

Run `verify.py` from any directory with this workspace layout: repository /workspace/Proof-of-Collaboration, helpers oracle.py/oracle_probe.rs under /workspace/pocol-s3, output /workspace/pocol-s3/evidence. The saved scripts are the actual orchestration scripts; copy them back there to reproduce. Rust/Cargo homes remain /workspace/pocol-tools/cargo and /workspace/pocol-tools/rustup. Each mutation gets its own target; do not replace this with a shared target cache. commands.json records exact arrays, directories, target paths, raw exit codes and durations. The core regression/author/reviewer targets are distinct.

preservation.json pins every tracked prototype hash and verifies the protected M1, LP1/LP2, S1/S2 evidence, decisions, .github, manifests and lockfile unchanged from the merge base. Only new S3 source/tests and new evidence plus appended Arabic status are added. No prior acceptance evidence is edited. Source remains at c7008ae980d2c0a19a6d5c6ef26c61f7b73de346; later commits add documentation only.

Technical review result: S3 pure-core behavior verified within this scope. New Windows execution, external review/owner acceptance and any subsequent integration decision remain separate. No main merge, no S4 and no claim of full LP3 acceptance.
