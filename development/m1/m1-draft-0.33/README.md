# M1 Draft 0.33: coordinator continuation patch (author turn 031)

**Patch revision for root review. Not applied, not executed by the author.**

- M1 specification acceptance is unchanged and remains **0.32**.
- No specification change and no new M1 choice is made here.

| Path | Content |
|---|---|
| `coordinator/README.md` | technical description, test commands, apply and rollback procedure |
| `coordinator/apply-manifest.json` | changed files (relative path, change, reason), required unchanged files, test expectations |
| `coordinator/files/` | complete updated module files and new tests, in the target's directory layout |
| `coordinator/tools/bundle.mjs` | root-run helper: `assemble` (isolated complete tree plus computed old/new SHA-256 manifest), `verify`, `apply` (with backup), `rollback` |
| `M1-DASHBOARD-0.33-AR.md` | Arabic summary |

## Changed files

Target: `coordination/issue3-repo/tools/local-coordinator`.

**Modified:**
- `src/broker.mjs`
- `src/notifier.mjs`
- `src/server.mjs`
- `public/index.html`
- `public/app.js`
- `package.json`

**Added:**
- `test/continuation.test.mjs`
- `test/connection.test.mjs`

## About the hashes

The author cannot execute code, so no digest is asserted in the bundle. `bundle.mjs assemble` computes the exact `oldSha256` and `newSha256` of each changed file, and the `sha256` of each byte-for-byte copied unchanged file. It writes them to `<test dir>/APPLY-MANIFEST.json`. `apply` refuses if the live tree no longer matches.
