# The reopen of the grow2 store (eb85bed), 2026-10-28

The grow2 store (h ≈ 114,901, 101 checkpoints written, two kept) was opened with the eb85bed binary and the node
log on at 16:25:16Z. The log went through a filter, and I stopped that run at 16:28:05Z to restart it
unbuffered. The stop killed the filter too, so the run's log lines are lost.

What remained shows the store was started over, as at the first H_END reopen:
- state.redb went from 76,165,120 B (16:24, the grow's last write) to 25,767,936 B at 16:28;
- history.redb was written at 16:27;
- no `.aside` file: neither store was set aside at open.

So again the checkpoints were not used. Why is not known: the reason is printed only in the lost lines
("checkpoint … refused: …" or "checkpoints unusable …", node.rs). The store was then deleted to free disk;
it cannot be examined again.

Runs on 051dfaa (the side set) do not show it at small heights:
- a clean stop at 20,000 resumed from record 19,456 ([repro-20000-clean-stop.log](repro-20000-clean-stop.log));
- a `kill -9` at 12,651 resumed from 12,288 ([repro-kill9-at-12651.log](repro-kill9-at-12651.log)).

Open: the refusal at H_END and at ~115,000 on the older code. A run to H_END on the current code, with the log
written straight to a file, is the next step (§ 4).

## Cause (found afterwards)

A run to H_END on 5cd7d43 logged `checkpoint 124: snapshot count beyond its bytes`. The snapshot decoder's count
guard assumed 134 bytes a record where 126 are written, so a snapshot of mostly minimal entries is refused. This
explains this reopen, the first H_END reopen and why small heights resume. See ../README.md § 4.
