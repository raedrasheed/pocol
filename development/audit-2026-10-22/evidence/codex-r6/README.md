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
