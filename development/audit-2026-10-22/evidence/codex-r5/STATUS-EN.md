# Work progress — Codex R5

Reference: PR #2 in palcertcom/pocol at 47114d6. New work is on a separate local branch,
`codex/r5-storage-validation`; neither the Claude branch nor main was changed, and no consensus decisions were approved.

## Completed

- Added F-A coverage for history.redb reads and write/sync failures during table initialization.
- The tests exposed a new defect: some initialization errors were classified as corruption requiring a rebuild. The classification was fixed so opening stops while preserving the file and checkpoints; the test verifies resumption after the fault clears.
- Added an F-B test against the real JournalSource: six corruption/short-read cases after initial verification. The test detects a mutant with local-error classification removed. This tests the read/classification boundary; it does not inject a file modification in the middle of Node::open itself.
- Reproduced reconnect_churn: 6/12 failures. Measurements established that the TCP backlog was not empty when live reached zero. The recovery test now receives Hello on its own connection within the original 500 ms deadline. All 24/24 subsequent attempts passed, and it detected a control that rejects every connection. Peer networking implementation was unchanged.
- Passed 257 release cases, 114 cases in io-fault suites, and 54 cases in the final network and F-B check. Counts overlap and are not unique-test counts. The extension build and 234 unit checks passed.

## New finding preventing memory closure

The current budget assigns 98,304 bytes to the evicted-item structure. Measuring allocation requests with Rust 1.99.1 yielded 133,136 bytes after inserting 1,025 items and trimming the count to 1,024. Capacity does not shrink automatically. The code inserts a batch before trimming, so peak capacity and batch bounds must also be derived. The memory limit was not changed, and no claim was made that the whole node budget was exceeded; the demonstrated shortfall is limited to this item, by 34,832 bytes in the example.

## Remaining

- Correct the memory derivation and test large loads, R_keep and nonempty H_END; these were not completed in this round.
- Non-browser tests passed on the final code: network 44/44, site 21/21 and healing 12/12 in each of memory and store modes. Results are in E2E-RESULTS.json.
- Chrome was not run: the browser download was not a valid ZIP. Private witnesses were not run because their programs were unavailable.
- Windows was not run on the new changes. A previous commit's success does not cover new changes.
- R21 requires the unimplemented K_eff; R22 requires the network admission path and an approved genesis. Open decisions remain unapproved; general development authorization does not settle genesis values or amend consensus rules.
- No human or independent-agent review was performed in this round; self-review and negative tests were performed.

## Publication blocker

Automatic approval review rejected pushing the branch to the public repository because it did not consider the request explicit authorization to publish the changes. All commits and evidence are preserved locally; no alternative publication path was used and no new PR was opened. Once verification is complete, specific approval is needed to publish the branch and open a draft PR against the Claude branch, without merging.

Details, commands and limitations are in REVIEW.md and the adjacent logs. These are partial results, not PoCol acceptance.
