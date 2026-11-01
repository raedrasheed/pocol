# Claude author turn 006: C23 repair and shared GitHub handoff

The shared repository is https://github.com/raedrasheed/Proof-of-Collaboration. Draft PR #2 on work/m1-import targets docs/development-tracking. The coordinator read development/README.md's Local coordinator handoff and imported curated snapshots through 0.7. Private transcripts, key/seed values, dependencies and binaries are excluded; original local work is preserved.

Perform a small sequential revision ONLY in new m1-draft-0.8. Preserve all earlier packages and the Git checkout under coordination/shared-repo. Do not write memory notes, install, deploy, sign/submit real transactions, merge or spawn agents. Default permissions remain enabled. The coordinator publishes sanitized summaries and evidence, not raw CLI output.

Fix C23: 0.7 run_checks_07.py's e_checks loads snapshot-invariants.json from 0.6 although the source fixture is in 0.4. Local independent run: 252 entries, 249 pass, 2 recorded, 1 FAIL, exit 1. All other 0.7 claims passed their recorded checks. Create a reference-only 0.8 patch/runner that fixes this lookup without breaking other 0.6 fixture references, and executes the comparative snapshot cases and the applicable regressions. Do not modify 0.7 in place. State exact inheritance and the source-file hash.

Return a compact amendment, runnable command and honest status/coverage. Do not claim execution with file tools only. Full M1 remains incomplete, with 14 not-started rows and owner questions U01/U02/U10/U14 and CR-M1-01 still open. Do not expand this repair into another broad batch yet; the coordinator will publish and review this completed cycle first.
