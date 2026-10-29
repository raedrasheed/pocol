# LP10 stage report — Integrated local PoCol network

Branch: `claude/affectionate-pascal-rjhh2k`. Runbook: [LP10-RUNBOOK.md](../lp10/LP10-RUNBOOK.md). Evidence: [evidence/](../lp10/evidence/) and [LP10-EVIDENCE.json](../lp10/LP10-EVIDENCE.json): the complete eight-suite run passed at commit `b381b14`, with a clean worktree.

## Completed
**Unified runner** `prototypes/lp10/run_lp10.py`:
- Builds everything and runs the suites in order.
- Writes evidence containing the commit, tools, duration and result of each suite.

**Integrated scenario** `tests/e2e_integrated.mjs` runs real Chrome and 3 pocold processes from a new directory:
1. Start from scratch; mining interfaces reject any request carrying Origin (BR15).
2. Publish site v1 through node a, read identical content in Chrome through each node, and accept it at RP trust level.
3. Partition the network: isolate c and publish v2 through it on the lesser-work branch. Chrome sees v2 through c while a sees v1.
4. Heal: c reorganized onto the heavier branch (D133), so Chrome through c displayed canonical v1, matching all three nodes.
5. Publish v3 through b and read it in Chrome through all three nodes.
6. Terminate b with SIGKILL, recover it from its journal, then fully restart the network. The served site remained unchanged.
7. Match hash, stateRoot and receiptsRoot at 142 confirmed heights across all three nodes.
8. Replay each node's journal in an independent process without peers; roots and site content matched byte for byte.
9. Resource measurements:
   - Node memory: approximately 21–35 MiB.
   - Journal: less than 1 MiB for approximately 150–300 blocks.
   - Extension-page memory: 7.7 MiB.

**A real defect discovered and repaired by this test**
- After partitioning, a node recorded a side block whose parent was older than the K_hist retention window. Live import reconstructed state (TrieRebuild).
- Journal replay rejected the same block (NeedsRebuild), preventing the node from starting.
- Replay now applies the same rule. A regression test reproduces the case: 70 blocks, then a branch from height 3, followed by reopening the node.

**Operational observation**
- T_blk must be at least 1+dFbWait.
- Otherwise the chain falls behind schedule and the RP client rejects the window at viewTargetCeil.
- The rule was recorded in the runbook.

## Second independent review (Website, publishing tools and replay)
- The reviewer found no authorization vulnerability in Website and confirmed the replay repair.
- All findings were repaired; details are in [REVIEW-RESOLUTION.md](../lp8/REVIEW-RESOLUTION.md):
  - **High:** block-body checks ran before binding the body to header roots.
    - Risk: a malicious peer could send a real header with a fake body, causing a valid block to be marked invalid.
    - Repair: bind the body first; reject a mismatch only as a copy, without marking the block.
    - Added a test proving that the real block is subsequently accepted.
  - **Medium, publisher:** validate the entire site before the first transaction. A 5 MiB site is rejected without any transaction.
  - **Medium, empty files:** accepted under pos-empty-file; all manifest vectors pass in Rust and JS.
  - **Medium, version-number race:** tested with two concurrent publishers; each published its own manifest.
  - **Low:** hostile-input-safe ABI decoding and preventing file overwrites during fetch.

## Tests
| Suite | Result |
|---|---|
| lp4-execution (Rust) | 129/129 |
| pocold (Rust) | 52/52: 32 unit, 10 consensus (including two regression tests), 4 storage and 6 site tests (including all manifest vectors) |
| Extension unit | 173/173 |
| e2e_devnet | 17/17 |
| e2e_site | 20/20 |
| e2e_browser (Chrome) | 66/66 |
| e2e_integrated (Chrome + 3 nodes) | 18/18 |

## Explicit limits
- Local network only, with deterministic development keys and no real funds.
- No deployment or merge into main.
- Browser items not claimed in [LP9-EXTENSION.md](../lp9/LP9-EXTENSION.md): BR24, ProfileMutex, remote endpoints and parts of T12/BR19b/BR20.
