# R6: main integration and collection high-water accounting

Owner explicitly authorized merging all accumulated work into main and continuing memory, Windows and browser verification on 2026-10-29. PRs #3, #2 and #4 were merged with history preserved; main reached 1bf18a677b9498cb7d464b3b63695a30149de716. Integration is not production acceptance.

The R5 eviction-cache finding is addressed in the model, without changing retention, FIFO semantics, consensus or limits. Before trim, a batch may hold side_cap + 2*r_keep protected side entries, r_keep abandoned canonical entries and one imported block. The old EVICTED_MAX entries coexist with this batch. Removal does not shrink HashSet or VecDeque capacity. Count 280 bytes per high-water key plus 512 bytes: two hash allocations at 76 bytes/key and two deque allocations at 64 bytes/key, allowing old/new allocation overlap. The protect filter separately holds old/new sets and a temporary Vec: 216 bytes per side entry plus 512 bytes. This is conservative for the pinned Rust 1.99.1 collection representation; it is not a cross-version allocator or RSS theorem.

`eviction_memory` meters requested allocations for nine sizes from 1024 to 16385, including growth boundaries and trim to 1024. It reproduces the old 98,304-byte undercount as a positive control, checks peak and retained allocations against the new bound, and checks arithmetic overflow. Local targeted result: 1 memory test and 4 admission tests PASS. Full suites and remote platform results are pending at this commit.

The existing Windows script now includes checkpoint_source and eviction_memory in the io-fault suite. Its workflow covers main and PRs targeting main, binds checkout to the exact tested SHA and retains read-only permissions. The browser workflow runs Playwright 1.62.1's real Chromium with the MV3 extension and live network in memory/store modes, including partition/restart integration. Chromium is the tested engine; do not describe it as a separately verified Google Chrome branded release. Logs, checkout SHA and binary digest are retained. Failed runs remain failures.

Still open: full A13 proof and other untested maximum-resource workloads, Windows results on these changes, browser results, network-profile boot blockers and unapproved consensus decisions. No budget increase or implicit decision approval.

## Local follow-up
The initial full release run passed 257 tests but failed the Linux-only pinned A13 total, which still expected the old model. This failure is retained in release-initial.log. The corrected pin is derived independently: eviction delta 2,581,272 - 98,304 = 2,482,968; protect delta (216-48)*6144 + 512 = 1,032,704; total delta 3,515,672. Devnet total is 3,237,735,992 B. The itemized breakdown test passes and asserts the new eviction term. Full consensus rerun follows separately.

The network-profile candidate store model is 5,585,459,749 B (5.201864754 GiB), requiring 5.944988291 GiB after the existing 8/7 factor. It remains below 6 GiB; this does not lift the R21/R22/genesis boot blockers or close untested maximum workloads. The complete I/O-feature selection passes, including the new allocation and checkpoint-source tests. The old Genesis manual workflow is reconciled to require a full lowercase SHA, exact checkout match, script presence and recorded script hash.

The final release consensus rerun passes 93 tests (7 ignored), preserved in consensus-final.log. Together with the unchanged passing targets from the initial run, 258 release cases pass; 115 I/O-feature cases pass (overlapping coverage).

## Deletion-churn correction (supersedes the first R6 formulas above)
Further source review and 200-cycle insertion/removal probes confirmed tombstone-driven growth. Rust 1.99.1's vendored hashbrown 0.17.1 only rehashes in place at <= half full capacity; otherwise it can double even when live length is well below the previous capacity. The source identity and hash are in HASHBROWN-SOURCE.json. A retained table can therefore require up to 32/7 buckets per historical live key, each 33 bytes: rounded up to 152 B/key. Count two such allocations plus 512 bytes for growth overlap. The allocator tests now also exercise seven churn sizes over 200 cycles each.

Final formulas: hash32_churn_bytes(n)=304*n+512; eviction=(432*(EVICTED_MAX+side_cap+3*r_keep+1))+1024; protect metadata allowance=368*side_entries+1024; side membership=304*(side_entries+r_keep+1)+512. This also repairs the adjacent side-membership term, whose old 76 B/key convention missed deletion churn. No collection semantics or budgets change. Final devnet total: 3,241,940,480 B, a 7,720,160 B increase from R5. The earlier 3,515,672 B delta and 5.944988291 GiB figure above are historical, not the final calculation.

Browser run 40838466669 passed in both modes on 5d006d2: each mode 80 browser checks and 21 integrated checks. Its installer log identifies the actual browser as Chrome for Testing 151.0.7922.34 (Playwright chromium v1234). This does not cover the subsequent churn correction, which requires new exact-commit checks.

Churn-corrected local run: release consensus 93 PASS/7 ignored; allocator regression 1 PASS; network-budget tests 8 PASS. No failure.

## Final local and browser evidence
Final local release: 258 PASS, 0 FAIL, 30 ignored (release-final.log). Final I/O-feature selection: 115 PASS, 0 FAIL, 9 ignored (churn-io-final.log). Counts overlap. Chrome for Testing 151.0.7922.34 on production commit 7ff41d7 passed 80 browser + 21 integrated checks in each mode, plus 234 extension unit tests. Both ZIPs were downloaded, their SHA-256 values matched GitHub artifact metadata, and their recorded commit matched 7ff41d7. Expanded original files and per-file hashes are in chrome-memory/ and chrome-store/.

Windows run 40840510614 is still running on 1760ea8. That commit changes only the Windows workflow concurrency policy after 7ff41d7; the production tree is identical. Cancelled superseded runs are not counted as successes.

## Windows verification of the final production code
Run 40840240541 on a839c9e420aff9d2f32871a69023f713a1ea329f passed all eight recorded checks: LP1 journal 12; LP1 full 150; LP4 211; pocold 257; io-fault selection 114; extension install/build; extension units 234. Counts overlap. The downloaded ZIP SHA-256 matched GitHub's 9645c25bd2a5b8202d2ce3c89f5aab208723f0bc7401c9f2f7c76f4f48fa29d7. Its environment records the expected commit, dirty=false, Windows Server 2022, Rust 1.58.1/1.99.1 and Node 22.22.0. Original evidence is in windows-production/.

Git comparison confirms no change under prototypes/ or .github/scripts/ between this tested commit and main verification commit 1760ea8. The sole difference is the Windows workflow concurrency policy. This establishes Windows verification for the current production code; the additional run 40840510614 on main remains in progress and is not recorded as PASS. Subsequent evidence-only commits do not change production code.
