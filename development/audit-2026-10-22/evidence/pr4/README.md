# PR4 — no state work before the pre-auth checks: evidence

Branch `claude/preauth-dos-checks`, base `53a9e87`. Linux x86_64, release builds.

- `a13-breakdown.log` — `a13_bound_breakdown` on the PR4 tree: total 3,242,261,392 B, the new term
  `queues/post-state verdicts per TemplateID and nonce (chain::COSTLY_MAX)` = 239,520 B
  (A13-DERIVATION §2b, PR4 row).
- `mutants.json`, `mutants.log` — the mutation controls of the PR4 plan §6 (M1–M21, plus M20b and M22),
  produced by `mutants.py` (`python3 -I mutants.py <worktree> <outdir> [ID ...]`). Each mutant is a text edit
  of `prototypes/pocold/src/{chain,node}.rs`, listed with its exact replacement in the log header; the
  sources are restored with `git checkout` after each run. Test set: the lib `f5f6_tests`, the PR4 and
  neighbouring tests of `tests/consensus.rs` (the plan's "unchanged, must pass" list, the D133 heal tests
  and `a13_bound_breakdown`) and `tests/relay.rs`.
  - pass 1 (`e32a574`): every mutant KILLED except M6 (the mutant did not compile; corrected), M9
    (survived) and M17 (equivalent: item 14 is unreachable once items 6 and 9 hold, so no `14 ` verdict
    reaches the cache).
  - pass 2 (`35c0543`): M6 KILLED; M9 KILLED by the new test
    `pr4_an_imported_side_branch_equivocation_is_recorded`. M9 survived pass 1 because the new
    `observe_template` no longer materializes, so the plan's T1.3 cannot tell it apart; it still loses
    the evidence of a side branch whose drawn proposer differs from the canonical one.
  - M21 (solicited blocks on the global bucket) is killed by T6.2, not by the heal tests the plan names:
    a heal session needs one deep rebuild and passes either way.
- `suite.log` — on the final tree: the full pocold suite (346 passed, 0 failed, 30 ignored), the consensus and
  caps tests on one thread, the `--ignored` spec-depth and deep-reorg tests (a11, adoption at 2048/2049, the
  store node at 2048/2049, the D133 tie-break at depth 400) and `tests/e2e_devnet.py` (PASS, 44 checks).
