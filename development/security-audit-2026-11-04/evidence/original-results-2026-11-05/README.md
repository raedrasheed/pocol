# Original scan artifacts and triage

These are the original GitHub Actions artifacts, not a new scan of this branch. `manifest.json` records the two run SHAs, artifact IDs, byte counts and archive hashes. Its main SHA and CodeQL access observation describe the earlier capture time; see the [latest status](../../LATEST-STATUS-2026-11-05-AR.md) for subsequent changes.

| Artifact | Original archive SHA-256 | Extracted report |
| --- | --- | --- |
| OSV run41875907642 / artifact13345977967 | `3dc96f2ff2d8b883d01249f64a57466f82c0ad7e844fb2a738eae8c0b9fdb300` | `osv-results.sarif` (archive member `results.sarif`) |
| Semgrep run41889848835 / artifact13351764891 | `e165aa022f4540335499beead40fd02d6b0a0966fdc20073d6ef0a0924972fa0` | `semgrep-results.json` |

Archive hashes were verified before extraction. `original-osv-triage.json` preserves14 raw records, including duplicates; there are12 distinct advisory/package/version/file keys. `original-semgrep-triage.json` preserves all67 findings and their context-specific reasoning and limits. The Markdown tables present the same reviews with an added historical-status note. Open dependency and platform risks remain documented.

`SHA256SUMS` covers every other file in this directory. No authenticated download URLs, credentials, SDK caches or browser session data are included.
