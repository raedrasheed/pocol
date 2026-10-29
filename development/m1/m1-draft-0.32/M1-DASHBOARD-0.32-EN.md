# M1 summary — Draft 0.32

**The author ran nothing. M1 is not complete.**

## Problem (C37)

In 0.31, check states were snapshotted before history.HF-8 was recorded; HF-12 saw an empty value even though HF-8 passed. This is the sole 0.31 failure. The root verified C35 and C36 repairs.

## What this supplement does

A small supplement without recomputation: no cryptography, Node or adapter. It reads the root's completed results and binds them to their hashes.

- Enumerates every historical failure and links each to direct or indirect repair evidence:
  - 25 failures in run files;
  - 9 root evidence files recording failures.
- Uses one closure procedure throughout, showing that the old snapshot does not close and the complete state does.
- Tests eleven deliberate mutations; each must prevent closure.
- Earlier failures remain in their files; nothing is deleted.

## Expected status after the root runs it

- **41 candidate-complete rows.** Candidates only.
- **Full M1 acceptance and recording F26** belong solely to the root after reviewing this run.
- No production code, deployment or transactions.
