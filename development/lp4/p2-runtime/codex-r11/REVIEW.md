# Separate sequential review of hosted evidence

Same author, distinct pass after downloading and checking run39064768819 artifacts.
Verified run/head/tree, all three jobs and completed steps; verified ZIP digests before
safe extraction. Parsed both native logs:40 successes each, zero failed/ignored native
tests. Inspected all17+16+7 compiled controls: successful compilation and assertion
failure, not compilation failure. Kept the3 contradiction controls separately labelled.

Verified63 independent full-state/gas/output/domain comparisons and byte equality to
R8. Verified complete20661-case upstream receipts equal approved-rule R4 receipts.
Checked runtime notices and manifest byte equality to R9;266/107 counts unchanged.
Legacy report explicitly retains8 platform blocks and2 unrun checks. No additions
to PASS counts from skipped, blocked or exception-presence-only results.

Review finding: upstream reference-root comparison is not a complete second EVM run.
Continue with an independent unmodified py-evm runner, preserving every mismatch,
engine error and timed-out case. CREATE-01 differences must be individually explained;
engine/adaptor defects cannot be hidden by exclusions or owner-approval assumptions.
Independent wire validation, complete provenance, actual genesis/system/apply/finalization
and full resource/persistence gates remain open. No merge or acceptance is authorized
by these successful hosted jobs alone. M1 and previous evidence unchanged.
