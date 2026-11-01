# Fix for the journal review (R3-3)

- **R3-3 (HIGH): a reorg anywhere in the journal made every later open re-execute the chain again.**
  This is local work at open. No validity, root or journal-format change.
  - Before: replay restored the adopted branches only once, at the end of open. That restore rebuilt
    the parent of the lowest offloaded canonical block (pruned by then, so from genesis). It then
    re-executed every canonical block up to the highest offloaded one.
  - Change: replay now does what live import does. It takes the fork height before `retain()`, then
    restores the adopted branch right after it (`Node::open`).
  - The final `restore_canonical(0)` stays as a safety net. It finds nothing unless replay skipped a
    record.
  - `Chain::restore_reexecs` counts the blocks the restores re-execute.
  - Test: `consensus::replay_restores_each_adopted_branch_with_the_work_live_did`. A 90-block chain has
    depth-1 adoptions of an offloaded signed branch at heights 5 and 80. It is reopened twice.
  - The test requires:
    - the same head as live;
    - every canonical block resident;
    - no skipped record;
    - the same RPC view as live, for every block and receipt;
    - the same rebuild and restore work as live, exactly;
    - less re-execution than the 75 blocks between the two reorgs.
  - Before: open did 4 rebuilds and 76 restore re-executions, against live's 0 and 2
    (`r3-3-before.log`). After: equal to live.

Remaining study corrections (R3-1, R3-2, R3-7, R3-8 and R3-13) go into a separate corrections note.
The original study stays unchanged.

## Suites for the R2 and R3-3 fixes (one run, release, on the tree of this commit)

- lp4-execution: 173 passed, 0 failed (`lp4.log`).
- pocold: 181 passed, 0 failed, 15 ignored (`pocold.log`). That is 176 before, plus the five new tests
  of R2-1 to R2-4 and R3-3.
- Load average at the end: 1.46 / 1.97 / 2.21 on 4 CPUs.
- The R4-1 unit test (`consensus::every_block_total_work_is_its_parents_plus_the_work_of_its_own_target`)
  was added after that run. It passes, and the reviewer's mutant Nworkpar (the parent's target instead
  of the block's own) fails it (`r4-1-nworkpar-unit.log`). The e2e side of R4-1 and R4-2 is recorded with
  its own runs.
