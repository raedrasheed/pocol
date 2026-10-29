# Load phase at the network's state caps (R4-4, 2026-10-28)

`jm_store_scale` with `JM_SCALE_PHASE=load` (tests/journal_measure.rs), on a fresh copy of the round-3 store at
the caps (949,767 accounts, 3.8 M slots, state.redb 2.16 GB), release, `POCOL_ROOT_CHECK=0`, Linux, with other
work on the host (the acceptance run and the H_END run). Each step prints VmHWM; the process holds one node.

| run | tree | result |
|---|---|---|
| [load-1.log](load-1.log) | 719da04 + the load phase | open, pool, full-write block measured; the reorg step failed in the test's builder (a signed block on a pruned parent: EmptyMembership) |
| [load-2.log](load-2.log) | the same, signed and fallback blocks alternating | the same three steps (VmHWM 454 → 471 MB at the pool step); the reorg step failed the same way |
| [load-3.log](load-3.log) | the same, `Chain::ensure_state` on the fork point | the reorg step failed: the fork point needs the journal's bytes (NeedsRebuild), which only the node can give |
| [load-4.log](load-4.log) | 719da04 + the side-set change + `Node::ensure_state` (the commit after 719da04) | all four steps; VmHWM 454,840,320 B at each |

The three failures were in the test's block builder, which reads the parent's state directly; the node's own
import turns a pruned or slimmed parent into NeedsRebuild and rebuilds it from the journal
(`Chain::validate`), which `Node::ensure_state` now exposes for tests.

Steps (load-4): open from the checkpoint at record 28,672 (226 s); the pool filled to its gas bound
(32,000,000: 4 creations of 7.95 M gas, the 5th refused "pool full (gas)"; its layer 1,424 fresh slots); a
full-gas block of fresh slots from another sender (496 ms) and the pool revalidated; a reorg onto a branch from
24 blocks below the head (25 blocks, 13 of them full-gas fresh-slot blocks; 20.7 s, import median 107 ms, max
2.07 s). The open's peak is the run's peak; nothing after it raised VmHWM.
