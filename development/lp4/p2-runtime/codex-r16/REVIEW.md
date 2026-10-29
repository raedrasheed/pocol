# Separate sequential configuration review

The R15 inventory checks both existing lockfiles. Local comparison venv had the
hashed blake2b-py extra; hosted main controls venv did not. Exact hosted traceback
confirms this missing prerequisite before actual Rust/compiled tests began.

Fix only installs the existing python-comparison-extra.lock with --only-binary,
--no-deps and --require-hashes, identical to the independent job. No new dependency
version, weakened predicate, skipped test, consensus rule or limit. Local inventory
with both locks already verifies24 distributions/1578 files; fresh-head CI must
verify its own environment and execute all required tests. Old failure preserved.

The separate1200-second selected retry actually returned BLOCKED_TIMEOUT2/total2.
This is NOT PASSED and not full corpus acceptance. Complete7200-second execution
continues; no pending/blocked check is promoted. See R15 for all evidence, review,
original39148982844 failure and remaining LP4 obligations.

No M1/LP3/owner decision/ledger/boot/foundation change. All26 obligations/P2/LP4
NOT PASSED, boot false, PR24 draft, no merge. Source whitespace check passed.
