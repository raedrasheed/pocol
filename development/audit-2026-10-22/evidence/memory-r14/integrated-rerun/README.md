# e2e_integrated with the heal check fixed (a01148e, test only)

- **Why:** in acceptance on `e453851` (whole-state, run 1), the heal step failed because a and b, not c,
  reorganized (`../accept-e453851/README.md`). `a01148e` makes the step wait until the three nodes share one
  head and the side that lost has reorganized.
- **The tree:** `a01148e`. Node code is `e453851`'s. `HEAD` records the commit and its prototypes tree.
- **Scope:** local nodes, local RPC and a test Chrome profile only.
- **Load:** nothing else ran.

| run | mode | result | reorganized after the heal |
|---|---|---|---|
| 1 | whole-state | **PASS 20/20** | c (a=0, b=0, c=1) |
| 2 | whole-state | **PASS 20/20** | c |
| 3 | whole-state | **PASS 20/20** | c |
| 1 | store | **PASS 20/20** | c |
| 2 | store | **PASS 20/20** | c |

In all five runs, c was the side that reorganized. The other outcome, where a and b move onto c's branch, is
the one seen in `accept-e453851`. The fixed check accepts it, but no run here exercised it.
