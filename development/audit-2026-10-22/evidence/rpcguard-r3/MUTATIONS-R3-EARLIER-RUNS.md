# RpcGuard round 3: earlier mutation runs (incomplete, kept as found)

These three result files were written by the scratch runner `run_mutations_rpcguard_r3.py`
(sha256 e6f0d876…) before the final pass. They are copied here unchanged. None of them is the round's
mutation result: that is `MUTATIONS-RPCGUARD-R3.md` / `mutations-r3-final-d0e87f9.json`, a complete
re-run of every mutant on the final tree.

| file | tree | state | what it showed |
|---|---|---|---|
| `mutations-r3-65a828b-partial.json` | 65a828b | **partial**: stopped after 10 mutants, once RGM5 survived | RGM5-STOPHOOK survived: once every late answer became −32020, `heavy_compute_deadline_cancels_eth_call` no longer told the stop hook from the post-encoding check. Fixed in 1530d7f (the test requires the answer before 3/4 of the whole run). |
| `mutations-r3-1530d7f-truncated.json` | 1530d7f | **truncated**: 46 of the runner's 53 mutants; the run ended after R3-M4-32008, and R3-M4-NO-CLASS-LIMIT, R3-m5-DEADLINE-BY-LANE, R3-m6-READBACK-UNCHECKED, R3-m6-SHARED-COUNTER, R3-m1-RATE-NOTE, R3-m2-SMALL-TERM and R3-m2-NO-PARSE-FILTER never ran. The session was then interrupted (container restart). | 42 caught, 3 expected survivors (V8, I3b, R3-M2-NO-WALL-IN-WAIT), 1 INVALID: N2's control `queue_wait_bounds_the_wait_on_a_full_main_queue` failed on the clean tree (a burst of 150 connects overflowed the listen backlog; fixed in 8039a09). |
| `mutations-r3-n2-8039a09.json` | 8039a09 | complete for its one mutant | N2-MAINQ-WAIT-TO-WORK-DEADLINE caught, its control passing. |

Because the 1530d7f run is truncated, it is not counted; every mutant in it was run again on the
final tree (d0e87f9), with six more.
