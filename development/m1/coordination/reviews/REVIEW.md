# Codex independent review of draft 0.2

Revision reviewed: preserved m1-draft-0.2, copied under review-001. Decision: revision required, not accepted.

Executed with portable Python 3.11.1 obtained from python.org (archive SHA256 3a6269cf0c0f8440a333998059e5e7ed8eaa1de576695d535224d87123f2adb0). Full saved suite independently reproduced: 212/212 pass, 0 failures, 228.5 seconds. See m1-draft-0.2/results/run-results.json in this review directory and rerun-0.2-retry.txt. The original author results were not overwritten.

Independent adversarial probes (independent-probes.json):

- C03 confirmed: canonical nested RLP of 3391 bytes, well below 65536-byte limit, raises RecursionError instead of a manifest rejection. The structurally invalid manifest still needs deterministic handling. Do not reject arbitrary otherwise-valid RLP by silently adding a new protocol restriction.
- C04 confirmed: /a@v followed by 5000 nines raises ValueError (Python integer conversion limit), whereas spec 6.2 requires malformedSelector for u32 overflow.
- C05 confirmed modeling gap: two files sharing a single chunk are accepted; reported fetchCalls=1 but Provider.get invoked twice. This dictionary mock does not demonstrate actual request coalescing/cache behavior.
- C09 static finding: T1-12 needs n published and noncurrent to avoid AlreadyCurrent; second permission failure should use a valid published target. T1-10 must distinguish global slot 2 from v1 immutable content slots and define concrete valid actions on other versions.
- C06 snapshot support, authenticity assumptions, proof bounds, heavy request budget and error/restart mocks remain unverified.

Provenance probe: raw UTF-8, BOM removal, LF normalization and trailing-newline variants did not reproduce the recorded design ID. This does not prove corruption; baseline.json is a design-version approval record. Source version derivation remains undocumented in the exported package (baseline-provenance.json).

Full gate remains unmet: 37 required annex rows are unstarted. The approved baseline's explicit M1-spec list, lines 4955 onward, corroborates their inclusion. EVM/compiler/browser production evidence is outside the authorized work here; its future requirements must be specified without pretending they were run.

Owner question sent asynchronously: U01 full factory address, U02 explicit revoked-view warning, U10 retained publisher rights and U14 per-request timeout. No response inferred from elapsed time.
