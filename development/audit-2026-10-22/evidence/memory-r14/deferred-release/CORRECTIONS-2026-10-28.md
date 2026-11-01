# Corrections to this directory's README (2026-10-28)

The README is kept as it was written. These statements in it are corrected here.

1. **"A storage error loses only the step: the steps stop, and the next checkpoint's write or open finishes
   what is owed"** (§ The independent review and its fixes, item 1), and the same words in the
   `release_step` comment ("the store stays whole … as after a failed block commit").
   - **What is true:** on disk the store stays whole. A failed write undoes the step, and a failed fsync may
     leave it durable (both shown with deterministic injected faults, `--features io-fault`).
   - **What was missed:** redb refuses every read and write of that database (`PreviousIo`) until it is
     opened again. So the running node does not go on with the store.
   - **What happened next, before the fix:** the next block's state commit failed. That failure was a
     `Reject::Block`, so the block was marked invalid, and it and every descendant were refused until the
     node restarted. A failed block commit did the same with no release step involved.
   - Fixed in the commit that carries this file: a local storage failure is `Reject::Local`, the node stops
     (exit 70) and marks nothing invalid, and the next open recovers.
   - Evidence: ../../r4/README.md § Storage faults, which also has the run without the fix.
2. **"RV11 … it needs a storage error inside `Node::open`, which these tests do not make"** (§ Seeded faults).
   - With an injected fault in the release at open, open went on reading the refused store and **panicked**.
   - Fixed: open returns the error and keeps the store and its checkpoints; the next open resumes from them.
   - RV10, RV11 and RV12 are killed by the new tests (../../r4/README.md § Seeded faults).
3. **"RV12 … a failed commit leaves the flag right either way"**. This is not so.
   - Case: nothing is owed, and the write would drop a checkpoint. With RV12 the flag is set while nothing is
     owed.
   - Case: rows are owed, one checkpoint is kept, and the write would drop none. With RV12 the flag is clear
     while rows are owed.
   - Both cases are tested with a write fault, which undoes the commit.
