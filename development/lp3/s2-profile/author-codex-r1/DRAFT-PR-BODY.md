## LP3 S2: typed CP and non-bootable profile identity

After PR #10 merged at d17273af35880f0e1d8a438e529e69c22a18f731, this increment implements the next incomplete S2 criterion from the accepted LP3 plan.

Adds all 52 CP fields with checked Rust widths; a v1-only validated ForkSchedule; and bounded GenesisSpec hash/chain identity validation with explicit profile/node gates. Results always remain non-bootable; allocRoot/sysCodeHash builder verification and full ParamGate remain deferred.

Validation at a9f0460335fa9594b943e125dd77d780042eefec: offline locked Rust 1.58.1 build, six S2 integration tests and rustfmt passed. All five mutation controls were caught by test assertions. M1 and every prior acceptance-evidence directory are preserved. Completed S1 suites were not rerun because their implementation did not change.

Evidence and self-review: development/lp3/s2-profile/author-codex-r1/README.md. This is author verification/self-review, not independent S2 acceptance. Windows execution remains unrun. Timing/membership/nonce/Draw, S3–S8 and LP3 acceptance remain incomplete. No main workflow or accepted specification changes.

Draft for sequential review; do not merge automatically.
