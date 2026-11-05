Concurrent extension wallet creations could overwrite a newly created seed without replacement confirmation, and an in-flight creation could unlock after an explicit lock. Reserve creation before the first await and fence asynchronous stages with wallet epochs. Three deterministic regressions fail on the original main worker and pass with the fix.

The faucet accepted unbounded HTTP workers, queued competing claims behind a slow signer, and could fail concurrent status reads. Bound admission before spawning a worker, enforce an absolute header/body deadline, reject competing claims with 429, validate request framing, snapshot claims for status counts, and report uncertain transaction delivery accurately. The Android JNI bridge used modified UTF-8 where Rust expects standard UTF-8; convert the boundary explicitly and reject invalid or oversized arguments.

Additional changes:

- Patch follow-redirects from 1.16.1 to 1.16.2 in contract tooling; all 13 compiler fixtures remain byte-identical and system solc stays at 0.8.28.
- Update two obsolete vulnerability witnesses to assert safe behavior and add bounded parser mutation/fuzz verification.
- Include dated audit evidence, the retrieved original OSV/Semgrep artifacts with verified archive hashes and per-result triage, and owner-supplied CodeQL exports with context-specific reviews. Keep unresolved findings and extraction limits visible.

Validation recorded on the unchanged fix sources:

- Faucet: 15 local tests; JNI: 8 checked-JVM tests, plus the official unsigned Android arm64 build and zipalign.
- Node: 319 passed / 30 ignored. Execution: 221 passed / 0 failed / 1 ignored. Extension: 268 existing checks plus the 3 lifecycle regressions.
- PHP: 41 local checks. Websites: 20 Chromium checks with external requests blocked, including the existing IDE compiler and local EVM deployment/call.
- Decoder fuzzing: 25,050,824 bounded executions with no reported crash/OOM/timeout; the reusable locked harness also passed 5,000 ASan/LSan runs.
- Windows WPF build and before/after regression job [42025519955](https://github.com/palcertcom/pocol/actions/runs/42025519955) succeeded. Its 8 inputs match main byte-for-byte; the raw artifact download remains blocked, so its archive was not independently verified.

Known open work: crypto-bigint's reachable signing timing advisory, tmp and maintenance debt, full extension browser integration blocked by administrator policy, physical Android/ART validation, large-bundle analysis/timeouts, and remaining CodeQL triage. C# analysis quality remains low with build-mode none; Go dependency resolution is blocked by an artifact host policy, so Go was not built. The integration cannot export full CodeQL SARIF; owner screenshots establish alert observations, not complete scanner coverage.

This is a draft for review. Source fixes were already tested; documentation-only follow-up did not trigger redundant local test runs. PR checks on the delivery head must be assessed separately. Main's TLS 1.2 fix (PR24) is already merged and retained; no duplicate TLS fix, consensus changes, crypto dependency upgrades, workflow changes or schedules are introduced. No production deployment, live-chain stress test, real keys or funds were used.

Current Arabic status, evidence links, source references and remaining work: [LATEST-STATUS-2026-11-05-AR.md](https://github.com/palcertcom/pocol/blob/security/audit-delivery-2026-11-05/development/security-audit-2026-11-04/LATEST-STATUS-2026-11-05-AR.md). Earlier round reports retain their original observations and point to that update.
