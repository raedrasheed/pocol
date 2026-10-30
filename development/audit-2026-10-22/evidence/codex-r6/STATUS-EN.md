# PoCol merge and verification status — R6

The accumulated branches and pull requests #3, #2, #4, #5 and #6 were merged into main with their history preserved. The latest implementation code is at 7ff41d7; 1760ea8 changes only the Windows workflow scheduling policy. Merging does not constitute production-network acceptance.

**Final result: Linux, Windows Server 2022 and Chrome for Testing checks passed on the final repair code, and their evidence was preserved. The broader acceptance limits below remain unchanged.**

## Memory accounting repair

The accounting now covers retained fingerprint-container capacity after deletion, the batch accumulated before the eviction ledger is trimmed, and coexisting containers during reallocation. It then adds the effect of repeated insertion and deletion (tombstones), as implemented by hashbrown 0.17.1 bundled with Rust 1.99.1. The correction covers the eviction ledger, side-branch membership and the protection set. Consensus rules, eviction order and the 6 GiB budget are unchanged.

The candidate network model in store mode on Linux x86_64 is 5,589,664,237 bytes, or 5.205780488 GiB; the requirement including reserve is 5.949463416 GiB. This is a model result under its stated assumptions, not an RSS measurement across all worst cases or full closure of A13.

## Local evidence

After the broader correction, 93 release consensus tests, one allocation test covering growth boundaries and deletion/insertion cycles, and eight budget tests passed. There were 115 passing cases with I/O fault injection, including history.redb and JournalSource verification. Counts overlap. The final comprehensive local run completed with 258 passed, zero failed and 30 ignored tests; ignored tests were not counted as passes.

## External checks

- Windows Server 2022: https://github.com/palcertcom/pocol/actions/runs/40840240541 — all eight checks passed on a839c9e: full LP1 150, LP4 211, node 257, I/O checks 114 and extension 234; counts overlap. The separate LP1 journal check passed 12/12. Evidence was downloaded and its digest verified; the checkout was clean and matched the requested commit.
- Every file under prototypes and the Windows script in the tested commit was verified identical to main at 1760ea8; only workflow scheduling differs. The Windows results therefore cover the current implementation code.
- Direct main verification: https://github.com/palcertcom/pocol/actions/runs/40840510614 — also passed on 1760ea8, with all eight checks and the same counts above. Its evidence archive was downloaded, its digest matched, and the commit identity and clean tested checkout were confirmed.
- Chrome: https://github.com/palcertcom/pocol/actions/runs/40840275422 — passed: 80 browser checks and 21 integration checks in each of memory and store modes, plus 234 extension unit tests. Both evidence archives were downloaded, their digests matched GitHub, and commit and result verification completed.
- Earlier run logs confirm Chrome for Testing 151.0.7922.34 through Playwright 1.62.1. The test runs on Linux, loads the actual extension and connects to PoCol nodes; it is not merely a DOM simulation.

Cancelled, ignored or unexecuted runs are not counted as successes. Worst-case resource obligations and open network decisions remain outstanding. This is not an independent external human audit.
