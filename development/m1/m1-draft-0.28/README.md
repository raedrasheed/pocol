# M1 Draft 0.28: retry-policy and status corrections (author turn 026)

- **For root review. Not approved.**
- Nothing was executed.
- Only new files under `m1-draft-0.28/` were written.
- The 0.27 tools and vectors are reused unchanged.

| File | Content |
|---|---|
| `M1-SPEC-0.28-AMENDMENT.md` | changes |
| `M1-STATUS-0.28.md` | rows and exact owner decisions |
| `M1-DECISION-PACKET-0.28-AR.md` | Arabic owner packet |
| `annex/C32-POLICY-CORRECTIONS.md` | corrected normative text, U14 branches, conditional timing |
| `owner/P-C32-2-CHANGE-REQUEST.md` | owner change request for `rate` delays |
| `vectors/u14-timing-cases.json` | conditional timelines |
| `audit/carry-forward-0.28.json` | bindings of the 0.27 evidence |
| `tools/run_checks_028.py` | small runner (root only); runs in seconds and rebuilds no fixtures |

## Command (root only)

```
coordination\runtime\python311\python.exe m1-draft-0.28\tools\run_checks_028.py --root <tree> --out <dir>
```

It writes only `run-results-0.28.json` in `--out` and never overwrites an earlier run.
