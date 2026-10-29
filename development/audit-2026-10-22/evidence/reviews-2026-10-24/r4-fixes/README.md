# Fixes for the e2e review (R4-1, R4-2)

Both are test changes only; no source change.

- **R4-1 (MEDIUM): "heavier" was decided only by the nodes' own totalWork**, so a W(t) bug that every
  node shares passed.
  - Unit test (`af2f3a8`): `consensus::every_block_total_work_is_its_parents_plus_the_work_of_its_own_target`.
    It computes work(target) independently as (!t)/(t+1)+1.
  - e2e (`tests/e2e_devnet.py`): `pywork` computes W = Σ floor(2^256/(target+1)) in Python, from each
    canonical header's committed target (decoded from `pocol_getBlockRlp`). It requires every block's
    `difficulty` and `totalDifficulty` over RPC to equal it.
  - That check runs on a's and c's branches before the heal, where it is also the heavier/lighter
    precondition, and on every node's frozen tip.
- **R4-2 (MEDIUM): relay and Status refresh were never needed** in the full-mesh topology.
  - In step 5 a second test peer, claiming genesis, only reads. It must receive every block a imported
    from the relay peer: a block a already held is a duplicate and is not imported again, so it is
    left out.
  - In step 6 the session claim is a **second** Hello on step 5's connection, not a first Hello on a new
    one. a must ask with GetFrom within the 15 s bound.
  - The Status check of step 6 now reads the whole connection, the connect Status of step 5 included.

## Runs

| run | e2e_devnet.py sha256 | binary | result |
|---|---|---|---|
| base-1 | `343a6c15…` | `af2f3a8` | PASS 42/42, 285 s |
| base-2 | same | same | PASS 42/42, 287 s |
| Nworkpar (total work adds the parent's target) | same | mutant | FAIL at "work: … equal W computed here", first mismatch at block 2 on a and c |
| Nnorelay (no forwarding of peer blocks) | same | mutant | FAIL at "relay: a forwards every block …": 0/6 received |
| Nhello1 (only a peer's first Hello is taken) | same | mutant | FAIL at "relay: a Status refresh … starts a session": no GetFrom in 15 s |

Each mutant fails first at the check aimed at it. Each log starts with the time, the worktree HEAD and
the sha256 of the script and binaries. The mutant diffs are `mutant-*.diff`.

- Before these checks the reviewer ran the same three mutants: each passed 38/38.
- Two earlier attempts of base-1 failed in my own new checks, not in the node. They are kept as
  `attempt1-base-1.log` and `attempt2-base-1.log`:
  - attempt 1 read c's late blocks after the heal, when they are no longer canonical on c;
  - attempt 2 expected a connect Status on a connection that already had one.

## On the final code

After the R1-5 penalty and the C-1 S2 model landed (`d567ce1`), the same test ran once on that commit's
own build: PASS 42/42 in 284 s (`final-d567ce1.log`; the header has the HEAD and the hashes).
