# Windows verification of 22a82c2: FAIL, one test (a defect in 93a40d0, fixed after ad301e7)

- **Repository:** `palcertcom/pocol`.
- **Workflow:** audit-r2-windows-verify.yml, run 40589342452, https://github.com/palcertcom/pocol/actions/runs/40589342452
- **Commit under test:** `22a82c20bc285ab1d1d1e647fd629e1352b233e2`, not dirty.
- **Run:** 21:54:46 to 22:16:26 UTC on 2026-10-27.
- **Result:**
  - lp1 12/0, lp1-node 150/0, lp4 211/0, extension 234/0;
  - **pocold FAIL: 239 passed, 1 failed:** `e4_checkpoint_damage_falls_back_to_a_verified_state`.
- **Artifact** `audit-r2-windows-22a82c20bc285ab1d1d1e647fd629e1352b233e2`, id 12775566580, 28,414 bytes, sha256
  `d247507d2e59e4479bf0cf2e4195c74f6ec65688a425186f99dd1e5f3c80e9d3`. That is the digest the upload step logged,
  and the digest of the downloaded file. Its contents are this directory, unchanged.

## The failure (`logs/pocold.log`)

The case that failed is "a missing node only a middle version names". It removes from state.redb a node that
only one middle version of the newest checkpoint names, then reopens the node. The test expects the node to
replay from genesis. Instead, open panicked with `C-1 E4: state store read failed: state store damaged`
(`StateStore::read`, from `NodeStore::code`).

The cases run in the same order as on Linux, and every one before it printed the same lines. This case
printed no "refused" line before the panic, so the panic came before any checkpoint was read.

## Cause: a defect in `93a40d0`, reproduced on Linux

- The node is chosen by iterating a `HashSet`, so it differs from run to run. Twelve local runs passed.
- A sweep over every candidate node (an uncommitted scratch change to the test; its log is quoted in
  deferred-release/README.md) panicked for 14 of the first 363 candidates. The backtrace:
  `Node::open` (node.rs:361) → `Chain::enable_store` → `NodeTrie::build` → `NodeStore::code` → `StateStore::read`.
- **What happens:**
  - when the missing node is also owed by the dropped checkpoint (`release` table), the release at open
    (`release_all`, added in `93a40d0`) meets a row naming a node that is not stored, and marks the store
    damaged;
  - open then went on to pin genesis's version in the damaged store, and the store's guard refused the read;
  - before `93a40d0`, nothing at that point of open marked the store damaged.
- The Windows run on `93a40d0` itself passed: its run chose a node that is not owed.

## The fix

- A store found damaged before genesis is pinned is set aside and started empty, as `StateStore::open` sets aside
  one marked damaged; replay refills the new one (I1).
- `consensus::e4_checkpoint_open_rebuilds_a_store_its_release_finds_damaged` removes an owed node
  deterministically. It fails without the fix with the same panic, and passes with it.

The defect is local: it is in D-6's mechanism and makes open fail, not state wrong. Node code at `dd85cc0` and
`ad301e7` still has it.
