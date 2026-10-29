# Codex R5: storage failure tests and reconnect churn

Status: partial follow-up; no project acceptance, merge, consensus decision or limit change.
Source PR: https://github.com/palcertcom/pocol/pull/2
Base: `47114d6fe5e9c3901945a183ebd2fd0bd441f548`.
Local branch: `codex/r5-storage-validation`.
Implementation commits: `773b420a5b6e4d4c65e57a93897fd2b1a62e4312`, then `1f3475d` (test repair, formatting, comments).

## Confirmed new defect: history open table initialization

`History::open` already used `rerr` for database creation and integrity checks, but used the untyped
`err` conversion for table initialization and its commit. A transient write or sync failure there
lost its I/O category. The fallback treated it as damage, set history aside and discarded state
checkpoints through the existing history-rebuild path.

The existing per-path fault backend now also wraps history.redb when `io-fault` is enabled. Three
cases extend the existing checkpoint-preservation regression: read at open, write at table
initialization, and sync at initialization. On pre-repair production logic with the hooks/tests,
the write case failed: `history.redb write at open: the open succeeded (resumed from None, 0 refused)`.
After changing the initialization conversions to `rerr`, all seven cases pass. They verify the fault
actually fired, open returns an error, neither store is set aside, checkpoints survive, the next
open resumes, and the resulting node matches journal replay. Fault injection remains feature-gated.
This corrects local failure handling, not consensus validity.

## F-B: corruption after the journal first pass

`checkpoint_source.rs` opens a real journal, captures the verified offsets, then changes the file
before calling the real `JournalSource`. It covers length, length checksum, digest, payload, marker
and truncation. Every failure must carry `checkpoint::LOCAL`; unknown and out-of-prefix offsets still
return `None`. Bytes and commit metadata stay untouched by the failed read. Restoring the original
bytes allows the same source to read successfully again.

An isolated worktree mutation removing LOCAL from JournalSource's error fails the new test. This
covers F-B at the actual source/classification boundary; it does not inject a disk mutation into
the middle of Node::open itself. The existing Node::open I/O tests cover the LOCAL stop/preservation
path separately. No claim of additional end-to-end race coverage is made.

## reconnect_churn: reproduced accept-backlog race

The original test failed 6/12 single-core trials (0, 1 and 3 competing busy processes, four runs each).
All failures were the final `still accepts peers` assertion, not live-count or thread cleanup.
Eight diagnostic repeats failed seven times. One recorded example: before final connection,
live=0, handshaking=0, refused=114, threads=8; afterwards refused=193, live=0 and the final socket
was closed. Thus live=0 did not mean the TCP accept backlog was drained.

The test now retries probes within the original 500 ms and requires a correctly decoded Hello
on the probe's own socket, for the correct chain and genesis. It cannot pass merely by observing
another transient peer in the live counter. No production network code or admission limits change.
All 24 repeats passed (eight each with 0/1/3 busy processes on the same CPU). A separate-worktree
mutation refusing every inbound socket fails at the final probe. This establishes a concrete cause
for the reproduction here; it does not retroactively prove the cause of every historical failure.

## Validation

- Rust 1.99.1, Linux. Locked dependencies; Cargo manifests and lockfiles unchanged.
- Full pocold release suite on storage repair: 257 passed, 0 failed, 30 ignored.
- io-fault consensus/state_store/checkpoint_source: 114 passed, 0 failed, 9 ignored.
- Final network + checkpoint_source tests: 54 passed, 0 failed.
- Extension: build and 234 unit checks passed, no extension source changes.
- F-B and reject-all-peers mutation controls fail as required. History initialization regression
  fails on uncorrected production logic and passes after repair.
- These counts overlap across test configurations; they are NOT a count of unique test cases.
- One production-binary build failed with an empty-object archive mapping error. A targeted cleanup
  of generated LP4 release outputs and rebuild succeeded; both logs are retained. No source or
  dependency workaround was used.
- On final code `1f3475d`, all six non-browser end-to-end runs pass: heal 12/12, site 21/21, devnet 44/44 in each of memory and store modes. See E2E-RESULTS.json and the six logs. The final binary SHA-256 is recorded.
- Chrome suites: NOT RUN. Playwright browser installation failed with an invalid/truncated ZIP.
- Windows on these commits: NOT RUN. Previous PR #2 Windows results do not cover these changes.
- Private audit witnesses: NOT RUN; their programs are not present in the checked-out repository.

## New resource-accounting evidence (not resolved)

The existing evicted FIFO + hash-set term is `1024 * 2 * (32+16) = 98,304 B`.
`collection_probe.rs`, compiled on Rust 1.99.1, reproduces the actual container operations: insert
1,025 distinct hashes and then trim to 1,024. Allocator-requested live heap is **133,136 B**, with
HashSet capacity 1,792 and VecDeque capacity 2,048. Difference: **34,832 B**. This is requested heap,
not RSS or allocator overhead. Both containers retain capacity after elements are removed.

The production retention code can insert a batch before trimming, so larger high-water capacities
must also be bounded. Merely replacing 48 B with another constant without bounding the batch and
capacity history is insufficient. The protect-set term also remains underived. No A13 PASS is
claimed, and no existing limit or model was silently relaxed. This does not alone establish that
the full 6 GiB budget is exceeded.

## Remaining work

1. Derive and repair protect/FIFO accounting including transient allocations and high-water capacities;
   exercise the largest execution, code, batch, RPC and peer terms and R_keep-deep reorganization.
2. H_END with nonempty blocks and bounded side-block re-journalling; owner decision still open.
3. Full browser and witness acceptance plus Windows on the new exact code.
4. Historical unexplained failures remain open where original evidence is unavailable.
5. Network boot: R21 K_eff runtime mechanism is absent; R22 network admission path and approved network
   genesis are absent. See existing BOOT-BLOCKERS and DECISION-TABLE. No owner decisions approved here.

## Publication

Push of the first commit was rejected by automatic approval review: explicit public publication
permission was considered missing. No alternate upload route was attempted. All commits, reports
and evidence remain local until that specific authorization is provided. PR #2 and its branch
were not changed. This report is a self-review plus executable negative controls, not an external
or separate-agent security audit.

## Reproduction commands

Run from prototypes/pocold, using Rust 1.99.1:

```sh
cargo test --release --locked --no-fail-fast
cargo test --release --locked --features io-fault --test consensus --test state_store --test checkpoint_source -- --test-threads=1
cargo test --release --locked --test net --test checkpoint_source
cargo build --release --locked --bins -j 2
```

Run from the repository root with POCOLD_RUN_ARGS empty, then with it set to --state-store:

```sh
node prototypes/pocol-extension/tests/e2e_heal.mjs
python3 prototypes/pocold/tests/e2e_site.py
python3 prototypes/pocold/tests/e2e_devnet.py
```

The JS harness requires PLAYWRIGHT_MODULE to identify an installed Playwright index.mjs, even
though e2e_heal itself does not launch a browser. Extension checks: npm ci --ignore-scripts,
npm run build and npm test from prototypes/pocol-extension. The collection probe is compiled
with rustc +1.99.1 -O collection_probe.rs; it measures requested live heap, not OS RSS.

M1, prior evidence, dependency lockfiles, GitHub workflows and main remain unchanged.
