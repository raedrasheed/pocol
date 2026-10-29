# RpcGuard in pocold: four items that need a normative decision

**Status: PROPOSED — not approved.** None of the options below is adopted. P1 and P2 are open
limits; no option of either is implemented. P3 records how pocold reads a gap in the text: option A
is the running behaviour pending a decision (round 2 added code that encodes it, the `EFFECTFUL`
exemption in rpc.rs `member`; it keeps the behaviour pocold had before RpcGuard and under stage A).
P4 (round 3) records the methods pocold serves outside the classification table: pocold keeps
serving them pending a decision, and no option is implemented.

All four concern RpcGuard, a local service policy (D87, consensus.md §13). None touches validity,
fork choice, GenesisSpec, CP values or any protocol constant. Each option would still fix
RPC-visible behaviour beyond what network.md:182–330 says, so under the audit rules each is a
proposal only.

Context: development/audit-2026-10-22/RPCGUARD.md (per-item status), the round-2 review of
wf/rpcguard (findings M1, M2, B3) and the fix round on wf/rpcguard-r2.

## P1. RPC request rate across connections (network.md:215–218)

### What the approved text says, and what pocold does

> دلو رموز لكل اتصال بمعدل 20/s وسعة 20: المنفرد يستهلك رمزًا، والدفعة k رموزًا عند التحليل. النقص يعطي
> −32021 {rate, retryAfterMs} دون تنفيذ. … لا انتظار للرموز داخل طلب أو دفعة.

"A token bucket per connection, 20/s with capacity 20. A singleton takes one token, a batch of k
takes k at parse. A shortfall gives −32021 {rate, retryAfterMs} without running. … No waiting for
tokens inside a request or batch."

pocold closes every connection after one response. The fix round applies the bucket exactly as
written, per connection, to that one request (`rpc::rate_refused`): a full bucket of `bucket_cap`
tokens, k tokens taken. At the spec values (capacity 20 = BATCH_MAX) no request within BATCH_MAX is
ever refused. This is the same outcome the spec gives a client that opens a connection per request.

Neither the spec nor pocold bounds the aggregate request rate when clients reconnect: every new
connection starts with a full bucket. The only aggregate bounds are RPC_CONN_MAX (32 concurrent
connections), the admission queues (HEAVY_QUEUE 16, LIGHT_QUEUE 32) and QUEUE_WAIT, which answer
excess load with −32021 {busy}.

Stage A keyed a bucket by source IP instead. The review refuted it and the fix round removed it:

- Any 127/8 source address reaches the loopback listener, so one client could multiply its
  allowance, or reset every bucket by using more than 4096 addresses.
- Honest local tools sharing 127.0.0.1 were limited together.
- Refill was truncated below 1 ms, so sustained traffic above 1000 req/s admitted almost nothing.
- Per-IP keying is not in the approved text.

### Options

| option | rule | aggregate bound | per-client fairness | RPC-visible change |
|---|---|---|---|---|
| A (current) | per connection, on its one request | none beyond the conn cap and the queues | n/a | none (spec as written) |
| B | one bucket per server: rate RPC_CONN_MAX·20/s, capacity RPC_CONN_MAX·20 (640/s, 640) | the spec's persistent-connection aggregate (≤ RPC_CONN_MAX·rate) | none: one client can take it all | −32021 {rate} under aggregate load |
| C | HTTP keep-alive (IDLE 30 s, next request read after sendDone), bucket per connection | as the spec | per connection, as the spec | transport: connections persist; clients that read to EOF must honour Content-Length |
| D (stage A, rejected) | per source IP | none (evadable through 127/8) | wrong: groups honest tools | −32021 {rate} for co-located clients |

### Proposed text, if B is chosen

> pocold (one request per connection): an aggregate bucket per RPC server, refilled continuously at
> RPC_CONN_MAX·rate tokens/s up to RPC_CONN_MAX·bucket_cap, keeping the sub-token remainder between
> requests. A request takes its k tokens after the batch-size check and ORIGIN_DENY; a shortfall is
> −32021 {rate, retryAfterMs = ⌈(k − tokens)/(RPC_CONN_MAX·rate)⌉} without admission.

Tests it would need:

- sustained admission at sub-millisecond arrival intervals equals the refill rate (the refill
  regression the review found);
- a burst above capacity is refused with the right retryAfterMs;
- the local tooling (`pocol-site publish`/`fetch`, e2e_devnet.py, e2e_site.py) stays under the bound.

## P2. CANCEL_MAX for precompile runs (network.md:286)

### What the approved text says, and what pocold does

> رمز يُضبط عند الإغلاق أو أي مهلة، ويُفحص كل 16 كتلة ممسوحة وكل 1 MiB مرمّز وكل 10^6 gas. يحرر كل
> الموارد خلال ≤ CANCEL_MAX = 100 ms.

"A token is set at close or at any timeout, checked every 16 blocks scanned, every 1 MiB encoded and
every 10^6 gas. It releases every resource within ≤ CANCEL_MAX = 100 ms."

The fix round polls the stop hook at least every 10^6 gas of an interpreter frame
(`native::STOP_GAS`; lp4 test `the_stop_hook_is_polled_at_least_every_stop_gas`). A precompile
call is not interpreted, so it runs to its end. With eth_call's 30M gas, one BLAKE2F run of 29.9M
rounds took about 0.5 s on this host. A 20 ms deadline was answered −32020 between 480 and 569 ms
late (net test `a_precompile_call_past_its_deadline_is_32020_not_a_late_success`). The answer is
correct, because a late result is never returned, but cancellation is not bounded by 100 ms.
pocold therefore reports `cancel_max_ms` absent.

### Options

| option | effect on results | bound | cost |
|---|---|---|---|
| A (current) | none | precompile run ≤ its gas × the slowest precompile's time per gas (host-dependent; not proven) | none |
| B | eth_call/eth_estimateGas refuse (−32020 or halt) a single precompile call whose gas exceeds a per-precompile cap derived from a measured worst time per gas | ≤ 100 ms by construction on the measured host | RPC results differ from on-chain execution for large precompile calls |
| C | RPC-only re-implementations of BLAKE2F (poll between rounds), MODEXP and the pairing (poll between pairs) | close to STOP_GAS | duplicated cryptography; equivalence with revm must be proven by vectors |
| D | run each simulation on a worker thread and abandon it at the deadline | the answer at once; CPU and the state copy are released only at the end | breaks the one-thread A13 inventory; does not release resources within 100 ms |

If B or C is chosen, the deciding vectors are: BLAKE2F at 29.9M rounds, MODEXP at its worst
EIP-2565 pricing within 30M gas, and BN254 pairing with 880 pairs. Each is measured on the reference
host and on the Windows CI host.

## P3. An effectful method that completes after its COMPUTE_DEADLINE (network.md:235)

### What the approved text says, and what pocold does

> COMPUTE_DEADLINE من القبول شاملًا الترميز: 5s ثقيل و500 ms خفيف، وانتهاؤها −32020 {deadline}.

"COMPUTE_DEADLINE runs from admission, including encoding: 5 s heavy and 500 ms light, and its expiry is
−32020 {deadline}."

The text does not say what to answer when a method's effect has already happened before the deadline
expired. eth_sendRawTransaction (a light method) pools a transaction; pocol_submitShare (not served by
pocold) submits a share.

pocold checks the deadline before such a method runs; past the deadline, the method does not run and
the answer is −32020. For every other method, pocold also checks once the answer is encoded and
replaces a late answer with −32020 (round 2: a late result; round 3, review RG2-M1: a late result or
error). That check is not applied to the two effectful methods. A late answer to an effectful method
therefore keeps its result. This is the behaviour pocold had before RpcGuard (no deadline at all) and
under stage A (entry check only), so the fix rounds introduce no new semantics for these methods.

**Running behaviour (second review, m7).** Option A below is what pocold runs, pending a decision. It
is encoded in code since round 2 (`const EFFECTFUL` in rpc.rs, which `member` exempts from the
post-encoding check), so choosing B would change that code: remove the exemption and add a test that
a transaction pooled after its deadline is answered −32020. No committed test pins option A either:
the round-3 mutant that removes the exemption survives the deadline tests
(evidence/rpcguard-r3/MUTATIONS-RPCGUARD-R3.md, R3-P3-NO-EFFECTFUL-EXEMPTION), deliberately, since the
reading is not decided; a test for the chosen option belongs with the decision.

### Options

| option | answer when the effect has happened and the deadline expires before the answer is encoded | client sees |
|---|---|---|
| A (current) | the result (for example the transaction hash) | the truth; the deadline is enforced before the effect |
| B (literal reading) | −32020 {deadline} | an error for a transaction that was pooled; a retry is then rejected as a duplicate |

### Proposed clarifying text, if A is chosen

> For a method whose effect is applied when it runs (eth_sendRawTransaction, pocol_submitShare), the
> COMPUTE_DEADLINE is checked before the effect; once the effect is applied, its result is answered.

## P4. Methods outside the classification table (network.md:210)

Added in round 3, after the second review of round 2 (finding RG2-M4).

### What the approved text says, and what pocold does

> الطريقة غير المدرجة: −32601 قبل القبول، وإضافتها تتطلب سطرًا في الجدول واجتياز RG13.

"An unlisted method: −32601 before admission; adding one requires a line in the table and passing
RG13."

The table (network.md (ب)) lists the light and heavy methods. pocold differs in two ways.

1. **It serves nine methods that are not in the table**, on the heavy lane (anything not light is
   heavy), after admission, with a RESP_MAX reservation and the heavy COMPUTE_DEADLINE:

   | method | who relies on it |
   |---|---|
   | pocol_status | the extension's service worker (pocol-extension/src/sw.js, `rpc('pocol_status')`), scripts/devnet.py, tests/e2e_devnet.py, tests/e2e_site.py, the pocold test suites |
   | pocold_rpcCounters | the pocold test suites (pocold's own counters, a superset of pocol_getRpcStats) |
   | pocol_getBlockRlp | tests/e2e_devnet.py |
   | pocol_getMiners | no caller in the repository (a devnet inspection method) |
   | net_listening, net_peerCount, eth_syncing, eth_mining, eth_accounts | none in the repository; standard Ethereum JSON-RPC methods that generic tools probe (the extension answers eth_accounts itself and never forwards it) |

2. **A method it does not know is −32601 after admission, not before.** The same holds for the
   listed methods pocold does not implement: eth_getProof, and pocol_getTiming, pocol_getTarget,
   pocol_getCaps, pocol_syncStatus, pocol_getPot, pocol_getSettlement, pocol_getBurnTotals,
   pocol_getEscrow, pocol_getRetention, pocol_getClosing, pocol_getAssignment, pocol_getMinerSet
   (pocol_submitShare is not an RPC method in pocold; its miner submits in process). The answer is
   the right code; what differs is that the request waits for admission (and can be answered busy
   instead) and holds a reservation while it is answered.

Round 3 changes neither: refusing the methods in point 1 would break the extension, the devnet
scripts and the tests, and moving the −32601 before admission changes what a client sees under load.
RPCGUARD.md marks the two classification rows PARTIAL for this reason.

### Options

| option | rule | effect on pocold's tools | RPC-visible change |
|---|---|---|---|
| A (current) | serve the nine methods as heavy; −32601 for unknown methods after admission | none | none |
| B | add a table line for each served method (network.md (ب)), with its class and its worst encoded answer, and pass RG13 for the light ones; −32601 before admission for every other method | none, once the lines exist | an unknown method is −32601 at once, never busy |
| C | a pocold-local "inspection" class, outside RpcGuard's table, served only without an Origin header (as ORIGIN_DENY), so a site cannot reach it; −32601 before admission otherwise | the extension's pocol_status call needs a table line or must move to an approved method | sites lose pocol_status and the pocold_* methods |
| D | refuse every unlisted method (the literal reading) | breaks the extension, scripts/devnet.py, tests/e2e_devnet.py, tests/e2e_site.py and the test suites | the nine methods disappear |

### Proposed table lines, if B is chosen

The worst encoded answers below are pocold's, by inspection of rpc.rs; RG13 would measure them.

- light (≤ 4 KiB): net_listening, net_peerCount, eth_syncing, eth_mining, eth_accounts (pocold
  answers `[]`);
- heavy: pocol_status (its answer includes the admission report and peer list), pocold_rpcCounters,
  pocol_getBlockRlp (≤ 2 × the block's encoding + 1 KiB), pocol_getMiners (one entry per registry record).

Tests it would need: every listed method answers with its class's lane and limits; an unknown method
is −32601 without waiting for admission (while the consensus thread is held, it is answered at once,
never busy); the extension's e2e and tests/e2e_devnet.py stay green.
