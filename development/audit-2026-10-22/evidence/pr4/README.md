# PR4 — no state work before the pre-auth checks: evidence

Branch `claude/preauth-dos-checks`, base `53a9e87`. Linux x86_64, release builds.

- `a13-breakdown.log` — `a13_bound_breakdown` on the PR4 tree: total 3,242,261,392 B, the new term
  `queues/post-state verdicts per TemplateID and nonce (chain::COSTLY_MAX)` = 239,520 B
  (A13-DERIVATION §2b, PR4 row).
- `a13-breakdown-merge.log` — the same test after merging main (PR #3, pull-only discovery, `b83b584`):
  total 3,244,176,189 B = 3,243,936,669 B (main's total with discovery) + the same 239,520 B term.
- `mutants.json`, `mutants.log` — the mutation controls of the PR4 plan §6 (M1–M21, plus M20b and M22),
  and M23–M28 for the review fixes, produced by `mutants.py` (`python3 -I mutants.py <worktree> <outdir> [ID ...]`).
  Each mutant is a text edit of `prototypes/pocold/src/{chain,node}.rs`, listed with its exact replacement in
  each pass's log header; the sources are restored with `git checkout` after each run. Test set: the lib
  `f5f6_tests`, the PR4 and neighbouring tests of `tests/consensus.rs` (the plan's "unchanged, must pass" list,
  the D133 heal tests and `a13_bound_breakdown`) and `tests/relay.rs`. Pass 3 is the current evidence; passes 1
  and 2 are kept as the history of M6 and M9.
  - pass 1 (`e32a574`): every mutant KILLED except M6 (the mutant did not compile; corrected), M9
    (survived) and M17 (equivalent: item 14 is unreachable once items 6 and 9 hold, so no `14 ` verdict
    reaches the cache).
  - pass 2 (`35c0543`): M6 KILLED; M9 KILLED by the new test
    `pr4_an_imported_side_branch_equivocation_is_recorded`. M9 survived pass 1 because the new
    `observe_template` no longer materializes, so the plan's T1.3 cannot tell it apart; it still loses
    the evidence of a side branch whose drawn proposer differs from the canonical one.
  - M21 (solicited blocks on the global bucket) is killed by T6.2, not by the heal tests the plan names:
    a heal session needs one deep rebuild and passes either way.
  - pass 3 (`00fba86`, after the review fixes `2060983`, `34b533f`, `36ef14b`, `00fba86`): the whole list
    again. M6, M7 and M8 are rewritten for `34b533f`'s `observe_template` (the `drawn` expression on
    `canon`), and M21's search text for `00fba86` (`if on_branch`); each keeps its intent. New mutants:
    M23 the E_win age filter removed and M24 a whole-map clear of `seen_templates` (`2060983`), M25 the side
    parent's own Draw arm removed and M26 `seen_templates` keyed by (parent, h) again (`34b533f`), M27 the
    backward-clock clamp in `rebuild_token` removed (`36ef14b`), M28 every block from the session's peer
    charged to the session allowance (`00fba86`). Every mutant is KILLED except M17 (equivalent, as above)
    and M9, which is equivalent since `34b533f`: `observe_template` now also reads Draw(parent,0) of a
    resident side parent, and an imported block's parent is canonical or made resident by its validation,
    so every check `observe_template` makes holds for it (`m9-probe.log`: true for all 1,064 signed imports
    of the suite). The only difference is a pair older than E_win, which the mutant drops and the original
    records although `evidence_type1` refuses it as too old; no test asserts that.
- `m9-probe.log` — the probe behind M9's pass-3 verdict (a temporary print in `import_with`, not committed).
- `suite.log` — on `35c0543`: the full pocold suite (346 passed, 0 failed, 30 ignored), the consensus and
  caps tests on one thread, the `--ignored` spec-depth and deep-reorg tests (a11, adoption at 2048/2049, the
  store node at 2048/2049, the D133 tie-break at depth 400) and `tests/e2e_devnet.py` (PASS, 44 checks); on
  `00fba86`: the release build (0 warnings) and the consensus tests on one thread (117 passed, 0 failed,
  7 ignored).
