# e2e-devnet: classification of the integration failure on 82ddb03 (code 3342af3)

The original logs are kept unchanged in this directory; this note only classifies them.

- `integration-82ddb03.out`, `integration-82ddb03-e2e-devnet.log`: the integration run recorded
  **FAIL** at "heal: c reorganizes to the heavier branch". In that run the isolated node c built a
  branch heavier than the a/b side (a and b stalled on fallbacks while c was drawn), so a and b
  reorganized to c's branch and all three nodes converged (the convergence wait passed). The check
  assumed the a/b side is always heavier, which the protocol does not guarantee. D133 chose correctly.
- `e2e-rerun-3342af3.log`: three standalone reruns on the same code. Two PASS. One FAIL at "every
  miner produced blocks" with b=0 and c=0, because the `mined` counter restarts with its process, b
  and c are restarted by the test, and in that run the chain had already passed 100 blocks before the
  restarts.
- `e2e-baseline-687ac38.log`: four runs of the same test on the pre-D-REORG-REDB code. **Two FAIL at
  the same "every miner produced blocks" check** (c=0), so that failure is pre-existing.
- Test correction (in the commit that adds this note): the heal check now asserts the protocol
  property, that all nodes converge on a head at least as heavy as either pre-heal tip and that some
  node reorganized. The miner check sums each node's `mined` counter across its restarts (sampled
  before each stop or kill). A canonical-winner variant was tried and rejected as too strong: a
  partitioned miner can leave the W_act membership window of the heavier chain and then legitimately
  wins nothing there. Three runs with the corrected checks: PASS, PASS, PASS (17/17 each). The full
  integration is re-run on the new commit.
