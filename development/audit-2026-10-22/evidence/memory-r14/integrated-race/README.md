# e2e_integrated, whole-state run 1 on 8d80354: a race in the test's reorg-recovery check

**What failed.** One of 20 checks failed (e2e-integrated-whole-state-1.log): "reorg recovery: Chrome via c now
shows the canonical version, identical on all nodes". All three nodes showed `LP10 site v1`, identical, but
the canonical index read afterwards was 1 (v2). The other 19 checks passed, including the replays of all
three nodes and identical roots at heights 1..102. The next run (whole-state 2) passed 20 of 20.

**Why.** The check compared views taken at one moment with a version read at a later moment, while the
version legitimately changed in between.
- During the partition, c published v2 on its minority branch.
- At the heal, c adopted the heavier branch and returned that branch's 7 transactions to its pool. By the
  D-7 relay policy (8584283) it offers them to its peers: `reinsertedOffered: 7, reinsertedUnoffered: 0`.
- The healed network included them a block or two later. The node logs here (node-a/b/c.log; heights and
  times in ms since the epoch the node prints):

  | event | node a | node c |
  |---|---|---|
  | reopened after the heal | 10,063,978: head 73 | 10,063,410: head 40, then 73-75 imported by 10,064,889 |
  | h=76, 5 transactions | 10,065,278 | 10,065,298 |
  | h=77, 2 transactions | 10,066,161 | 10,066,178 |

- So v2 became canonical about 1.3-2.2 s after the nodes reopened: after the Chrome reads began and before
  the version was read.

**Not a node defect.**
- Returning and re-offering the transactions is the specified behaviour: consensus.md, excluded transactions
  return after they are checked again; the D-7 relay policy.
- Every node agreed at every height, and v3 was "version 2" in this run as in every earlier one, so v2 was
  included in the end every time.
- In the 10 earlier integrated runs of accept-24cc167 and accept-8584283 (evidence/d7-bound/), the
  canonical index after the heal was 0 in six and 1 in four. Which one a run sees depends only on whether
  the re-inclusion lands before the check.
- The overlap with a low-priority (`nice 19`, one job) compile on this host may have widened the window. It
  did not cause the race.

**The fix (test only, prototypes/pocol-extension/tests/e2e_integrated.mjs).**
- The views now count only when every node's version is the same before and after them. They are retaken
  if it moved in between, for up to 120 s.
- The assertion is unchanged: the views are identical on all nodes and equal to that canonical version.
- The pools still settle before v3, as in 919a50c.
- The original log stays here unchanged. The reruns of the fixed test are in ../accept-<commit>/.

## A second test defect: the taller branch is not always the heavier one (store-2-073923f/)

**What failed.** Of the six reruns of the fixed test on `073923f` (../accept-int-073923f/), five passed 20/20.
Store-mode run 2 stopped at "timeout: c reorganized" after 9 checks. That wait comes before the
reorg-recovery check changed above.

**Why.**
- Before the heal, the test waits until the majority branch is "heavier" by checking only heights:
  `ha > hc + 4`. That held at ab = 60 against c = 55.
- Fork choice (D133, `Chain::better`) compares total work, the sum of 2^256 / (target + 1) over a branch's
  blocks, not height. The blocks the nodes hold (blocks-known-to-a.txt and blocks-known-to-c.txt, read from
  copies of their data dirs) give these figures:

  | branch | tip | total work | work of its last blocks |
  |---|---|---:|---:|
  | ab (a and b meshed) | h=60, 0xe240f561e022 | 0x245033 (2,379,827) | 0xaedd–0xded8 |
  | c (isolated) | h=55, 0x07284e80a6cc | **0x27aa15 (2,599,445)** | 0x13b29–0x151cf |

- c's uninterrupted 1-second blocks (h=35–55) raised its difficulty by about 4% a block. The ab branch's
  slow early blocks (h=31–33, 4–5 s apart, two of them unsigned fallbacks) eased its own.
- So c's branch was the heavier, and a and b correctly adopted it. Their logs show "TrieRebuild for a fork
  below the K_hist window", and their canonical chain is c's from h=31 on.
- c had nothing to reorganize. All three nodes then agreed at every height, up to h=235 with the same hash.
- **Not a node defect**: the heaviest-work rule worked as specified.
- An observation not changed here: an isolated c stops producing after some blocks (here at h=55, about 24
  s before the heal; at 39 and 42 in other runs). The test does not depend on it.
  Reproduced and explained since: ../isolated-stall/README.md. It is the W_act nonce membership, specified
  behaviour.

**The fix (test only).** The wait now also requires a's total work (`pocol_status` head.totalWork) to
exceed c's before the heal. The heal then has a heavier majority branch for c to adopt, which is what the
check "c reorganized onto the heavier branch (D133)" asserts.

## The reruns of both fixes (../accept-int-2c40a9f/)

- `92f6a80` added the total-work wait, but its helper was named `work`, which shadowed the run directory in
  `main()`. Both reruns on it failed before the partition (../accept-int-92f6a80/, kept as they ran).
- `2c40a9f` renames the helper. Its reruns: **six of six PASS 20/20**, three per mode.
  - In every one, c reorganized onto the heavier branch.
  - The reorg-recovery views agreed with the canonical version.
