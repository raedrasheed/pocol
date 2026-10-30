# Preserve history checkpoints on I/O errors and make churn recovery verification reliable

Target: claude/memory-r14-followup (stacked on PR #2). Draft; no merge requested.

An I/O error while creating history tables was losing its error category and causing the node to
set the database aside and replay instead of preserving checkpoints. Preserve the category and
extend fault-injection coverage to history reads, writes and syncs. Add a real-journal regression
for F-B corruption after verification. Repair the reconnect-churn probe to require a Hello on its
own connection within the unchanged 500 ms deadline; live=0 does not drain TCP's accept backlog.

Validation and remaining limitations are in evidence/codex-r5/REVIEW.md and E2E-RESULTS.json.
The full local release suite and targeted I/O suites pass; negative controls detect the regressions.
New resource probe demonstrates an evicted-container undercount; the A13 proof stays open.
Windows, real-Chrome acceptance and private audit witnesses on this change have not been run.
No consensus rule, dependency, resource limit or M1 file changes. No full acceptance claimed.
