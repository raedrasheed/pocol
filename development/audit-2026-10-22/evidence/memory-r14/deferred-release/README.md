# D-6: a dropped checkpoint's nodes released in steps between events (`93a40d0`, `22a82c2`)

At the network's state caps, a checkpoint that dropped the oldest one held the consensus thread 10.3–13.7 s
(scale-d30dc71/ckpt-instrumented.log). It released every node only the dropped checkpoint held inside its own
transaction. This is D-6's local mechanism; no consensus, validity or specification rule changes.

## The mechanism

- **state.redb** keeps a `release` table: node id → the references it still owes. A node's stored count is the
  stored nodes that name it, plus its pins, plus what it owes.
- **`write_checkpoint`:**
  - first finishes, in its own transaction, what an earlier write left owed;
  - then takes the dropped checkpoint's pins off and owes each of its roots one reference.

  So the store owes at most one dropped checkpoint.
- **`release_step(budget)`:**
  - works off up to `budget` owed rows in one transaction, fsynced since `22a82c2`;
  - a node left with no reference is deleted, and each of its children is owed one;
  - a step that fails marks the store damaged, and the next open rebuilds it.
- **`Node::run`** takes one release step (RELEASE_SLICE, 256) whenever no event is waiting, or one between two
  events while work is owed. The pool's revalidation slices go first.
- **Open** finishes everything owed (`release_all`) before it reads a checkpoint.
- **A13:** the state.redb file term counts (KEPT + 2) sets of held versions, plus a release row for at most each
  node of the owed set. Network store mode: 5,581,321,485 B (5.198 GiB), 5.941 GiB required, under the 5.25 GiB
  model cap (A13-DERIVATION §3c).

## Files

| file | tree | what |
|---|---|---|
| `release-93a40d0.log` | `206eaa4` + the change committed as `93a40d0` | lp4 and pocold release suites: 451 passed, 0 failed |
| `crash-test-93a40d0.log` | the same | `--features crash-test`: crashes inside release steps 1, 20 and 60, and kills at increasing delays |
| `crash-test-22a82c2.log` | the tree committed as `22a82c2` (steps fsynced) | the same |
| `mutants-deferred-release-dd85cc0.log` | `22a82c2` + the change committed as `dd85cc0` (its test and comment) | seeded faults RL1–RL16 (below) |
| `release-review-fixes.log` | `dd85cc0` + the review fixes | lp4 and pocold release suites: 455 passed, 0 failed |
| `mutants-deferred-release-2.log` | the review fixes' commit | seeded faults RL1–RL16 again, adapted to the fixed code, and RV1–RV12 for the fixes (below) |
| scale-d30dc71/`ckpt-*.log` | `93a40d0`, `22a82c2` | the cost at the network's state caps (scale-d30dc71/README.md, § After the deferred release) |

## What the crash tests show

- After a crash inside a release step, the node opens as replay from genesis gives it, from the same journal.
  It resumes from a checkpoint, and the next run imports every block. This holds with the steps fsynced or not.
- `e4_checkpoint_survives_kills` takes a release step after each import, as the event loop does, and is killed
  at 11 increasing delays.

## The tests

- **`state_store::a_dropped_checkpoint_is_released_in_steps_and_ends_as_a_release_in_the_write`:**
  - five checkpoints, keeping two, and releases in steps of 3 (161 steps);
  - after every step the counts recount, and every pinned version reads as its state;
  - the dropped checkpoint's own nodes end neither stored nor cached;
  - the next write finishes what is left owed first;
  - a store that owes work reopens owing it, and `release_all` finishes it.
- **`consensus::e4_checkpoint_release_runs_at_open_and_between_events`** (added with this evidence):
  - a node that stopped owing a dropped checkpoint's nodes works them off at open;
  - a running node with no event but its ticks works off what a new checkpoint owes;
  - it then imports a block from its channel;
  - the counts recount at each stage.
- **`network_budget`:** the file term with the owed set and its rows.

## The independent review and its fixes

An independent review of `93a40d0` and `22a82c2` found nothing critical or high
(review-2026-10-27/D6-REVIEW.md). Its findings were fixed in the commit after `dd85cc0`:

1. **Medium.** A storage error in a release step (an I/O error, say) marked the store damaged, so the next
   open replayed from genesis. Now only a release row that contradicts the counts does. A storage error loses
   only the step: the steps stop, and the next checkpoint's write or open finishes what is owed.
   - Tests: `a_release_step_that_fails_on_io_leaves_the_store_whole` (Unix, RLIMIT_FSIZE in a child process)
     and `a_release_row_that_contradicts_the_counts_marks_the_store_damaged` (the step, the write and open).
2. **Low.** A pool backlog took every background step. With both pending, pool slices and release steps now
   alternate (`e4_release_steps_alternate_with_pool_slices`).
3. **Low.** No list of deleted ids is held; a deleted node leaves the cache as it is deleted.
4. **Informational:**
   - the flag is set after the write commits;
   - `stored_bytes` counts the release rows;
   - FORMAT 3, and a format-2 store opens as format 3
     (`a_format_2_store_opens_as_format_3_and_another_format_is_rebuilt`);
   - rows, not references, in the docs and stats (`released_rows`, `rowsReleased`; the earlier logs'
     `refsReleased` counts rows).

## A defect found on Windows (run 40589342452, `22a82c2`)

`e4_checkpoint_damage_falls_back_to_a_verified_state` failed on Windows in its case "a missing node only a middle
version names" (evidence/windows-22a82c2/RUN.md).
- **Cause:** a defect of `93a40d0`. When the node the case removes is also owed, the release at open marks the
  store damaged. Open then went on to pin genesis's version in that store and panicked, instead of rebuilding.
- **Why it was intermittent:** the case picks its node from a `HashSet`.
- **Reproduced on Linux** with a sweep over every candidate node (an uncommitted change to the test, run as a
  scratch binary):
  - 14 of the first 363 candidates panicked on `ad301e7` (`damage-sweep-before-fix.log`, the panic lines);
  - after the fix, the same sweep over all 587 candidates: **none panicked**. Each replayed from genesis and
    equals the reference node (`damage-sweep-after-fix.log`).
- **The fix:** `Node::open` sets aside a store found damaged before genesis is pinned, as `StateStore::open`
  sets aside one marked damaged, and replays from genesis.
- **The test:** `e4_checkpoint_open_rebuilds_a_store_its_release_finds_damaged` removes an owed node
  deterministically. It fails without the fix (the same panic; `open-damaged-without-fix.log`) and passes with
  it.

## Seeded faults

- **`mutants-deferred-release-dd85cc0.log`:** 16 faults on the code before the review fixes, **all killed**.
  RL13–RL15 (open does not release, the event loop takes no step or does not see owed work) are killed only
  by `e4_checkpoint_release_runs_at_open_and_between_events`.
- **`mutants-deferred-release-2.log`:** 28 faults on `ad301e7`, the review fixes in.
  - RL1–RL16 again, adapted to the fixed code: all killed. RL1 first failed to build (its pattern left a value
    untyped). Its pattern was corrected and the rerun, the last line, killed it.
  - RV1–RV9, faults in the review fixes: all killed, each by the test written for it.
  - **Three survive, with nothing that tests them:**
    - RV10 (`stored_bytes` omits the release rows): a measurement helper; no test reads it with rows owed.
    - RV11 (open marks the store damaged on any error from `release_all`): it needs a storage error inside
      `Node::open`, which these tests do not make. The store-level path is tested by
      `a_release_step_that_fails_on_io_leaves_the_store_whole`.
    - RV12 (the flag set before the write commits): with KEPT = 2, every write that finds rows owed also drops a
      checkpoint, so a failed commit leaves the flag right either way (review finding 4a).
- **The fix in `e453851`:** shown by its test against the code without it (`open-damaged-without-fix.log`), and
  by the sweep before and after.
