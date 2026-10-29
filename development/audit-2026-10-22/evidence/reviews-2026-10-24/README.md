# Independent reviews of 2026-10-24

The owner asked that the missing independent reviews be completed. Each review was done by a separate
agent instance. Each reviewer was given the code and the spec, not the track's conclusions.

| review | subject | reviewed at | report |
|---|---|---|---|
| R1 | caps, round 3b final (`e985ce1`) | `ec148cf` | [R1-caps.md](R1-caps.md) |
| R2 | RpcGuard, round 3c (`dd7d1ec`), with the Windows socket code | `ec148cf` | [R2-rpcguard.md](R2-rpcguard.md) |
| R3 | the journal study, round 3c, and the effect of `462fffc` | `ec148cf` | [R3-journal.md](R3-journal.md) |
| R4 | e2e round 4: does the test detect non-convergence and the wrong branch? | `ec148cf` | [R4-e2e.md](R4-e2e.md) |
| R5 | `462fffc` (A11 reorg walk, R14/R19 calibration) | `ec148cf` | [R5-462fffc.md](R5-462fffc.md) |

Probes, witness tests, mutation runners and benchmark crates stayed in scratch, outside this public
repository. Only these reports are committed.

## Findings rated MEDIUM or above, and what happened to them

| finding | severity | disposition |
|---|---|---|
| R3-2: a reverting CREATE2-heavy transaction pays every `current_total` scan without changing the state, so the worst state-scan block is repeatable at every height | HIGH | C-2 item 2 (`2ce03c1`): a create reads only the journaled accounts. Re-measured: 247 CREATE2 creates at the near-cap state take 562 ms, against 4,862 ms before ([../c2-execution/README.md](../c2-execution/README.md)) |
| R3-3: open restores every adopted branch only at its end, re-executing from the lowest offloaded canonical block. Any reorg doubles later opens | HIGH | fixed: replay restores each adopted branch as live does; open does exactly live's rebuild and restore work ([r3-fixes/](r3-fixes/README.md)) |
| R3-1: the "7.3 h + 8.5 min" devnet bound double-counts TX_CAP and omits the per-transaction state term (≈ 7.45 h before R3-2 and R3-3) | MEDIUM | study correction queued; C-2 changes the per-transaction term |
| R3-6: every new head re-executes the whole pool with no gas bound (about 68–95 min at 4,096 MODEXP transactions) | MEDIUM | needs a local-policy decision (a pool gas limit changes which transactions are accepted): added to the decision package. C-2 already removed the per-submission scan and clone |
| R3-7: after `462fffc` a detached transaction already on the new branch pays a full scan before its nonce rejection | MEDIUM | fixed by C-2 (`2ce03c1`); also R5-1 |
| R5-1: a normal reorg's shared transactions each pay a full scan and a clone (74.6 s against 32.0 s in the reviewer's benchmark) | MEDIUM | fixed by C-2 (`2ce03c1`): neither cost remains. Re-measured: 380 transfers at the near-cap state take 655 ms, against 22,553 ms before |
| R2-1: one RPC batch holds the consensus thread (a P2P Hello waited 14.3 s; up to about 35 s by construction) | MEDIUM | fixed: queued P2P, miner and timer events run between batch members; a Hello came 442 ms into a 5.7 s batch, against 4.85 s into 5.05 s before ([r2-fixes/](r2-fixes/README.md)) |
| R1-1: near the TXBYTES room the builder offers transactions that depend on a skipped one and can starve an independent valid transaction | MEDIUM | fixed (`6e99b19`): the builder executes its selection on the parent state ([r1-fixes/](r1-fixes/README.md)) |
| R1-2: no committed test pins LOG_CAP accumulation within a block, its per-branch scope, or `cum_log_bytes` in `meta_clone` (L1, L2 and L4 survive) | MEDIUM | fixed (`6e99b19`): `tests/log_cap_scope.rs` (caps-test build) |
| R4-1: the e2e's only measure of "heavier" is the node's own totalWork, so a work-accounting bug that every node shares passes | MEDIUM | fixed: an independent W check in the unit test and the e2e; mutant Nworkpar is caught ([r4-fixes/](r4-fixes/README.md)) |
| R4-2: relay and Hello-refresh breakage pass (full mesh; every sync starts from a new connection) | MEDIUM | fixed: a listening peer and a second Hello in the e2e; mutants Nnorelay and Nhello1 are caught ([r4-fixes/](r4-fixes/README.md)) |

The LOW and NOTE findings are listed in each report. Those that need code or test changes are queued
after the items above, in the order of the decision package priorities.

LOW and NOTE findings fixed so far:
- R1-4 (`6e99b19`);
- R2-2 to R2-5 ([r2-fixes/](r2-fixes/README.md));
- R1-5: consensus.md:101 "the peer is punished" (a local disconnect; test
  `an_invalid_template_from_the_drawn_proposer_punishes_its_peer`, killed mutant `r1-fixes/r1-5-mutant.log`);
- the study corrections R3-1, R3-8 and R3-13, in
  [../../JOURNAL-GROWTH-CORRECTIONS-2026-10-24.md](../../JOURNAL-GROWTH-CORRECTIONS-2026-10-24.md).
