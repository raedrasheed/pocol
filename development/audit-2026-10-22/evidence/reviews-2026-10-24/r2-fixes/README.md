# Fixes for the RpcGuard review (R2)

All five are conformance fixes to the owner-approved RpcGuard implementation. Each brings the code to
what network.md already says, with no new normative text. Witness probes stayed in scratch.

- **R2-1 (MEDIUM): one batch held the consensus thread.** network.md (ط) separates RPC from consensus.
  - Change: between two batch members, `Node::drain_between_members` runs the light lane and then at
    most EVENT_TURN = 8 queued main events (P2P, miner, timer). Queued RPC requests are deferred to
    `run()`, which takes them before its next receive, so their order is kept.
  - A member is admitted in turn with its own snapshot (network.md (ج): one snapshot per permit), so a
    new head between two members is the same as between two requests.
  - Test: `net::a_batch_does_not_hold_p2p_events_for_its_whole_length`. A batch of 8 uninterruptible
    BLAKE2F eth_calls runs while a new P2P connection waits for the node's Hello.
  - Before the fix the Hello came 4.85 s into a 5.05 s batch (`r2-1-before.log`). After it, 442 ms
    into a 5,742 ms batch, then 390, 437 and 446 ms into batches of 4.95–5.19 s in three later runs
    (`r2-1-after.log`). A second heavy request during the batch is answered with a result or −32021
    busy (QUEUE_WAIT), never held for the whole batch.
- **R2-2 (LOW): eth_sendRawTransaction's execution had no COMPUTE_DEADLINE.** network.md (د) step 4
  and (و) require that cancellation be checked every 10⁶ gas.
  - Change: `Node::submit_tx_by` executes the pool admission with the stop hook
    (`native::admit_for_profile`; while the hook does not fire, the execution is the same).
  - A precompile run, which no poll can cut short, is checked again before the commit.
  - Past the deadline the transaction is refused with −32020 {deadline} before anything is pooled or
    relayed. So the late −32020 never hides an accepted transaction: P3 option A, the running reading,
    is unchanged. P2P and pool rebuilds pass no deadline.
  - Test: `net::a_transaction_admission_past_its_compute_deadline_is_refused_and_not_pooled`.
  - Three mutants are each killed (`r2-2-mutants.log`): no deadline (the code before the fix), a stop
    hook that never fires, and no check before the commit.
- **R2-3 (LOW): ERR_ITEM_MAX did not hold for ORIGIN_DENY or dispatch errors.**
  - Change: these items echo the id only up to ID_ECHO_MAX = 128 B, and −32601 echoes the method name
    only up to METHOD_ECHO_MAX = 42 B of JSON. That covers ORIGIN_DENY, an unknown method and a member
    without a method.
  - Method errors from an execution, such as revert data, are results and are unchanged.
  - Test: `net::guard_and_dispatch_error_items_stay_within_err_item_max`.
  - The mutants are killed (`r2-3-mutants.log`): with ids echoed unbounded, the ORIGIN_DENY item is
    100,091 B, the reviewer's own figure.
- **R2-4 (LOW): a reset after every answer that had bytes after it.**
  - Change: after an answer, a request connection shuts its write side and drains unread input for at
    most LINGER = 500 ms and REQ_BUF bytes (`rpc::linger`). A client that closes after reading ends it
    at once. The acceptor's 503 refusal (RPC_CONN_MAX) does not linger.
  - A first version also lingered there. `rpc_connection_cap_refuses_excess_unread_and_keeps_serving`
    caught it: refusals were serialized by up to 500 ms each.
  - A BATCH_WALL close stays immediate. The A13 RPC terms are unchanged: no buffer is added and the
    drain reuses the read chunk.
  - Test: `net::bytes_after_the_request_do_not_reset_the_answer`. A trailing CRLF (after 0 and
    300 ms) and a pipelined request, each after the ~14 MiB batch answer.
  - Before: `Connection reset by peer` after all 14,680,950 body bytes (`r2-4-before.log`). After: no
    error, the whole answer, 3 of 3 runs.
- **R2-5 (LOW): the whole-request REQ_READ_DEADLINE was pinned by no test.**
  - Test change in `net::rpc_connection_cap_refuses_excess_unread_and_keeps_serving`: a client sends one
    byte every 400 ms for 3 s, twice the 1.5 s deadline. It must be closed while it still trickles,
    1.4–2.3 s after its first byte.
  - The reviewer's mutant M13 (a per-read timeout) now fails it (`r2-5-m13.log`).

Suites: see `../r3-fixes/README.md`, which records the one run that covers R2 and R3-3 together.

Still open from R2 (NOTE): R2-6 to R2-11, text and single-test coverage, queued after the items above.
