# M1 Draft 0.32: Status

**The author ran nothing. M1 is not complete.**

| Item | Expected after root's execution |
|---|---|
| `history.globalClosure` | pass: 25 run failures and 9 root-evidence failures, each with proved direct or transitive witnesses |
| Stale-versus-fresh demonstration | stale (cut before HF-8) does not close; fresh closes |
| 11 mutations | every one prevents closure |
| Rows | 41 CompleteCandidate (candidate only) |
| Findings | F01–F25 closable at specification scope; F26 waits for root to record all 41 |
| Global M1 acceptance | **not claimed**; root's act |

## Exact remaining root action

1. **Run the supplement** in a preserved copy (Python standard library only; no Node needed) and require **zero FAIL**:
   `coordination\runtime\python311\python.exe m1-draft-0.32\tools\run_checks_032.py --root <tree> --out <dir>`
2. **Review** `history-closure-0.32.json` (every failure's proof path) and the eleven `mutation.*` results.
3. **Then, if accepted,** close C37 in the ledger, record the 41 rows as Complete and F26, and only then record M1 acceptance.

If anything is missing, the run names it in `coverage032.required` and in `status-0.32.json` under `missingRequired`. Any FAIL keeps every row's c5 blocked and the candidate open.

The coordinator continuation improvement is the next separate author job, after this review.
