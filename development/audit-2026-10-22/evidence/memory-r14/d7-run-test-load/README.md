# The D-7 run test under load (fixed by `e6d07b2`, a test change)

`pool_revalidation_at_the_gas_bound_does_not_hold_up_block_import` failed once in a pocold release suite run.
That suite ran beside an instrumented scale open (`suite-failure.log`). It failed with
``called `Option::unwrap()` on a `None` value`` at tests/consensus.rs:716, the test's status request.

## Cause, reproduced

- **Diagnostic build:** b47fb85 plus the review fixes and the ckpt phase, with one change to the test: a reply
  that is not a status now fails with the reply in the message. Runs from `../../../../../prototypes/pocold`.
- **With 3 CPU-bound loops beside the test** (`diag-load3.log`): 6 of 6 runs passed. The hold, from the second
  block to the answer, was 0.77–1.27 s.
- **With 8 loops** (`diag-load8.log`): 5 of 6 runs failed with
  `{"error":{"code":-32021,"data":{"reason":"busy","retryAfterMs":250},"message":"busy"}}`.
  - The status request was not admitted within QUEUE_WAIT (2 s). The node answered it busy, as network.md:232-234
    specifies.
  - Ahead of it in the queue were about four heavy transactions, of up to 0.5 s each under this load.
  - The run that passed held 1.96 s.
- **The test's own inconsistency:** its bound allows a loaded host 6·max_tx + 2 s. But the request it measures
  with was still subject to the 2 s admission limit.

## Fix (`e6d07b2`, test only)

- The test's node gives that request a QUEUE_WAIT of 120 s, the request's deadline. The test then measures the
  hold, which is its purpose. Admission has its own tests.
- No node code changed.

## After the fix

- **Same load, 8 loops** (`fixed-load8.log`): 6 of 6 runs passed. The hold was 1.97–2.68 s, each within that
  run's 6·max_tx + 2 s.
- **Seeded faults** (`mutants-d7-run-test.log`): the D-7 mutants this test killed before (D2, D3, D7, R1, from
  `../../d7-bound/mutants.log`) were run again against this test alone, on b47fb85 plus the test change. All
  4 are still killed, and BASE passes.
