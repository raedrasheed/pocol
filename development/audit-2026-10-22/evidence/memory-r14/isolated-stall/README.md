# Why an isolated node stops producing for a while (reproduced 2026-10-27)

Reproduced on this host on local loopback nodes only, with the node binary of the working tree at `1babbc7`
(node code as `2839f2a`). The reproduction runner stays outside the repository; these are its outputs.

## The runs

1. **Partition (`partition-a/b/c.log`, `poll.jsonl`).**
   - e2e_integrated's devnet and partition: miners a, b and c; T_blk 2 s, D_att 41 s, dFbWait 3 s, c = 8,
     work-bits 15, W_act 32 (the devnet default).
   - The devnet runs to height 16. Then a and b run meshed and c alone, for 150 s.
   - Every node's stdout is kept, and each node's `pocol_status` is polled every 0.5 s.
2. **Solo (`solo-c.log`).** c alone from its own data after the partition, for 600 s.

## What was observed

- **In the partition, c stalled twice.** It ended at height 37, against 151 for a and b.

  | c's state (poll.jsonl) | from t | until t |
  |---|---:|---:|
  | `FB h=17 a=0 shares=78 mining=false` | 6 s | 45.6 s |
  | `PROP h=31 a=0 shares=95 mining=false` | 62 s | ≈107 s |

  `mining=false` with the template locked: c's log says `range exhausted; idle until next template`, then
  `attempt 0 expired; advancing` about 40–46 s later, then the next attempt's block at once.
- **The solo run.** Two exhaustions at height 40 (attempts 0 and 1), then 411 blocks with one more exhaustion.

## Why (the code and the numbers)

- **The nonce space.** In bounded mode a block's attempt searches nonces below
  nonce_max = c·work(target) (lp1-node/src/assignment.rs `nonce_max`), about 8 times the expected hashes per
  solution. It is cut into equal ranges, one per member of the attempt's membership (`Assignment::new`).
  A miner searches only its own range (pocold/src/node.rs `lock`: `assignment_range`). When the range is
  exhausted, it idles until the next template (`MinerEvent::Exhausted`). That is the next attempt, after
  the attempt window D_att and Γ.
- **The membership.** It is the members whose last share is within W_act blocks of the height
  (`Membership::from_parent`: `last_share >= height − W_act`).
  - a's and b's last shares were at height 16 (`pocol_getMiners` on c: `lastShare: 16`).
  - So up to height 48 c's view still had 3 members, and c searched a third of the space:
    (8/3)·work hashes. It finds no solution in an attempt with probability e^(−8/3) ≈ 7%.
  - From height 49 c alone is the membership and searches all of it, with probability e^(−8) ≈ 0.03% of
    finding nothing.
- **The data match.**
  - The partition's stalls were at heights 17 and 31, and the solo run's at 40. All are at or below 48.
  - After height 48 the solo run produced 411 blocks with one exhaustion (at h=452). That is consistent
    with 0.03% per attempt. With the 7% of the first 32 blocks it would not be.
  - The late one came when c's difficulty had risen far enough that a whole range took 16 s to search.

## Verdict

This is the specified behaviour: consensus attempts, the W_act membership, and bounded nonces. It is not a
defect.
- **Effect:** for W_act blocks after it loses contact, a minority node keeps only its 1/M share of each
  attempt's nonces. Each attempt fails with probability e^(−c/M), and each failure costs an attempt window,
  D_att + Γ (about 46 s here).
- **Network values:** c = 2, W_act = 128, D_att = 300 s. An isolated member of M active members fails an
  attempt with probability e^(−2/M): 61% for M = 4, 94% for M = 32. Each failure waits about 300 s, for up to
  128 blocks.
- No change is proposed here. A change would alter consensus parameters or rules, which is the owner's
  decision.
