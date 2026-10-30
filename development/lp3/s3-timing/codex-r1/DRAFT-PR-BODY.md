## LP3 S3: bounded pure timing core

Base: merged S2 foundation 30daceb7e8c632041fd44a40afec57553f85fff2. This increment implements S3 from the accepted LP3 plan without starting S4.

Adds u128-checked L/U and wire-attempt bounds, fallback/proposer timestamps, strict attempt/future predicates, exact millisecond T_j (equality timely), and sigma/Xh/XW formulas. Network 14/22/36 and P11 4/6/10 are covered. No clocks, sleeps, scheduler, full FSM or boot authorization.

Tested source: c7008ae980d2c0a19a6d5c6ef26c61f7b73de346. Clean Rust 1.58.1 regression: 89 tests passed; six S3 tests include 10,000 seeded adversarial windows. Separate review against a Python unlimited-integer oracle: 10,524 cases, zero mismatches. All six mutations detected, each with an isolated target directory. Builds/rustfmt/fixture checks passed. M1 and all earlier acceptance evidence preserved.

Evidence/review: development/lp3/s3-timing/codex-r1/REVIEW-S3.md and RESULTS.json. Raw expected mutation failures are separated from successful detection. New Windows execution is BLOCKED BY PLATFORM, not PASS. Real-time jitter/scheduler, S4 and full ParamGate/builder work are NOT RUN and outside scope.

Separate Codex review pass completed; this is not an external-review identity or full LP3 acceptance. Draft for review; do not merge automatically or begin S4.
