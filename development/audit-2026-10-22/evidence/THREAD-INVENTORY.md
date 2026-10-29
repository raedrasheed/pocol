# Thread inventory reconciliation (A13 point 3)

## Measurement (threadmem/t.rs, release, glibc, 256 KiB stacks)
- 200 threads that touch ~200 KiB of stack then exit, NOT joined, handles held:
  RSS +5,152 KiB = 25.8 KiB per finished-not-joined thread (NOT 256 KiB).
- After joining them: RSS ~unchanged (stacks already returned).
- 200 spawned and joined promptly one at a time: RSS delta 0 KiB (stacks reused from the cache).
Conclusion: a joinable thread, on exit, returns the bulk of its stack to glibc's stack cache
(the model's STACK_CACHE = 40 MiB term); it retains only ~26 KiB until join. So exiting-not-joined
threads do NOT each cost a full IO_STACK of fresh resident memory.

## Per-kind concurrency (running) and exiting-not-joined bound
| thread kind | spawn | max running | reap/join point | max exiting-not-joined |
|---|---|---|---|---|
| consensus | main.rs:15 | 1 | main joins at exit | 0 (not respawned) |
| miner | miner.rs:27 | 1 (PEAK proves) | MinerHandle::drop joins before the next start | ~0 (drop joins synchronously) |
| miner-fwd, listen-fwd, tick, p2p-listen, rpc acceptor | node/net/rpc | 1 each (5) | process lifetime | 0 |
| dial-{addr} | net.rs:238 | dials | process lifetime | 0 |
| dial-fwd-{a} | node.rs:170 | dials | process lifetime | 0 |
| peer reader (inbound) | net.rs:198 | peers_max | p2p-listen reaps at each accept | <= peers_max |
| peer writer | net.rs:181 | peers_max + dials | its reader joins it before sending Closed | ~0 (joined synchronously by the reader) |
| peer reader (outbound) | net.rs:198 | dials | the dialer joins the previous reader before the next | <= dials |
| rpc-{id} conn | rpc.rs:196 | conn_max | acceptor reaps at each accept | <= conn_max |

io_threads = 2*peers_max + 4*dials + conn_max + 6 counts max RUNNING (reader+writer per inbound,
dialer+dial-fwd+reader+writer per configured peer, conn_max rpc, 6 singletons).

Max exiting-not-joined E <= peers_max + dials + conn_max (readers + outbound readers + rpc conns that
finished between reap points; writers are joined synchronously; singletons never respawn).

## Model (resources.rs threads term)
- Running stacks: io_threads * IO_STACK (full resident, conservative).
- Exited stacks returned to glibc's cache: STACK_CACHE = 40 MiB (separate term).
- Exiting-not-joined remainder: E * IO_STACK counted CONSERVATIVELY (real ~26 KiB each, measured),
  so the term is an over-bound that also covers any thread momentarily holding its stack before the
  cache reclaims it. E = peers_max + dials + conn_max.
- consensus stack 16 MiB + main 1 MiB + 2*THREAD_HEAP.
The /proc thread COUNT invariant test may transiently show running + up to E threads; that is counted.
