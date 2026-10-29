# M1 Draft 0.21: V3 NetworkProfiles.validate (author turn 019)

- **For Codex review. Not approved.**
- Nothing was executed by the author.
- No memory notes, installs, deployments, transactions, merges, subagents or shell were used.
- Only new files under `m1-draft-0.21/` were written.

## Files

| File | Content |
|---|---|
| `M1-SPEC-0.21-AMENDMENT.md` | additions |
| `M1-STATUS-0.21.md` | coverage (proposed 40/1/0 after review), expectations |
| `annex/V3-NETWORKPROFILES.md` | normative annex, RecvFit derivations, gaps |
| `tools/netprofile_ref.py` | GenesisSpec decoder, validate, FakeTransport |
| `tools/run_checks_021.py` | runner |
| `vectors/v3-gsv1.json` | GSV1 literal bytes and tree, decoded values, K1–K3, 32 negatives |
| `vectors/v3-profiles.json` | profiles, RF1–RF4 and edges, transport scripts |

## Command (not run by the author)

From `D:\PoCol-Development`:

```
coordination\runtime\python311\python.exe m1-draft-0.21\tools\run_checks_021.py
```

It writes only `m1-draft-0.21/results/run-results-0.21.json` and exits 1 on any FAIL.
