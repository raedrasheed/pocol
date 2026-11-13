# Plan: remove the old network and launch a fresh v3-only PoCol network

I read `origin/main` at `d24843d` and changed nothing in `/home/user/pocol-import`.

## 1. Spot-checks of the riskiest claims

| Claim | Result |
|---|---|
| The lock can be reset with "delete the lock, then re-pin `base`". | **Half right.** `repin()` refuses `base` only while the lock has more than one history line (`consensus_boundary.rs:697-698`), so deleting the file and then running `POCOL_REPIN_CONSENSUS=base` works locally. |
| A "two-step merge" gets the reset past CI. | **Wrong.** `deploy/ci/check_lock_append_only.py::check()` returns *"the base has a consensus lock and this tree removed it"* when the lock is deleted. It also fails any history that is not a prefix of the base. So both steps fail. The checker needs an explicit one-time reset path (see §3). It runs in three workflows: `five-device-build.yml:42`, `full-nonce-upgrade.yml:36` and `public-testnet-operations.yml:32`. |
| Lock history length. | **23 lines** at `d24843d`, not 22. All have `rules=1 maxFork=2 spec=1`. |
| The guard's history rules. | `kind=bump` must set `rules = prev + 1`. No other kind may change `rules`. Every change to vectors needs `neutral-vector` or `bump`. Every change to deps needs `neutral-dep` or `bump`. The last line must equal the code's `CONSENSUS_RULES_VERSION`, `MAX_FORK_VERSION`, `SPEC_VERSION` and `PROTOCOL_VERSION(_MIN)`. The wire rules forbid changing a pinned wire key at or above `protocolMin` unless the lock is re-based. So the H15000 wire golden samples in `net.rs` can only change in the reset PR. |
| `SPEC_VERSION` | Confirmed `= 1` (`lp1-node/src/genesis_spec.rs:34`). `decode()` uses ceiling 2 only under `cfg!(feature="v3-preview")` (`:423`), and `MAX_SPEC_BYTES` depends on the same cfg (`:207`). |
| `MAX_FORK_VERSION` | Confirmed `= 2` (`profile.rs:278`). |
| R22 | Confirmed. The Network profile requires `chain_id == 777001` (`profile.rs:348`, `lp4 admission.rs:410`). |
| Preview admission | Confirmed. `admission.rs:431` refuses non-Simnet admission under `V3_PREVIEW`. `testnet.rs:252` refuses a v3 config without a preview build. `pocold/src/lib.rs:17` asserts the feature parity and `:25` declares `pub mod cutover`. |
| Probation pool size | Confirmed. `PoColSystemV3.sol:501` and `:521` revert `Capacity()` when `ACC_COUNT >= CP_P_MAX`. **So `P_MAX = 0` means no probation seats at all, which blocks every zero-bond newcomer. It does not mean "unlimited".** `GAS_DOMAIN_V3_P_MAX = 64` (`admission.rs:213`). |
| Member limit | `M_max` is a u16 CP field. `MAX_MEMBERS = 65535` (`genesis_spec.rs:89`, `:160`). |
| Files that read `deploy/mainnet/h15000/network.pocol.json` | Confirmed: `pocold/src/{launcher,mobile,testnet}.rs`, `pocold/tests/{admission,network_budget,v3_scaffold}.rs`, `lp4-execution/src/admission/tests.rs`, `pocol-extension/{src/lib/wallet.js,tests/unit.mjs}`, `deploy/standalone/{test_launcher,test_upgrade}.py` and `deploy/mainnet/test_{network_identity,preflight,standalone_preflight}.py`. **Delete `h15000/` last.** |
| `consensus_guard.rs` legacy test | Confirmed: `the_legacy_table_is_bound_to_the_h15000_chain` (`:62`). `consensus-boundary.txt:71` has `=pocold::cutover`. |
| `monitor.py` crashes on continuous chains | Confirmed. `monitor.py:49` does `closingHeight - 100`, but `node.rs:3201` reports `closingHeight: null` when `H_END == 0`. That is a TypeError on the new network. |
| Freeze feature | Confirmed: the `--freeze-user-txs-after` flag, the `POCOL_FREEZE_USER_TXS_AFTER` variable and the V3-33 text in `main.rs`. |
| `PLAN.md` coupling | Confirmed. `readiness.rs:30` `PLAN_GATES_OPEN` mirrors the `PLAN.md` ledger. |

### Conflicts between the surveyors, and how I resolve them

- **EarnIn program set.** One surveyor says keep `system-earnin`/`earnin.rs` because the owner wants the 20 PCOL earned bond. Another says delete them. **Delete them.** `ProgramSet::V3` (`PoColSystemV3.sol`) already carries the earn-in rules: zero bond, `B_reg` accrued from mining, probation. The spec-1 EarnIn set was the old cutover target. Two conditions before deleting:
  - Port the security regressions `earnin_refund_fix.rs` and `earnin_halt_proof.rs` (PAY-02 self-reward and mandatory refund) to `system-v3.json`.
  - Keep `earnin_model.py` until `v3_model_vectors.py` stops importing it.
- **`full-nonce-upgrade.yml`.** One surveyor says delete the job; another says simplify it. **Rename it and keep the generic suites:**
  - lp1 profile/gate/assignment
  - lp4 profile_execution
  - pocold `--lib`, `consensus_guard`, `decoder_reference`
  - the `cmp` checks of the shared decoder and profile copies
  - `test_launcher.py`
  - rewards, studio and faucet tests
  - the `browser` job

## 2. What must NOT be removed (the new network needs it)

- **Consensus discipline.** `consensus_boundary.rs`, `consensus-boundary.txt`, the boundary part of `build.rs`, `tests/consensus_guard.rs`, the `POCOL_RELEASE=1` refusal of unpinned trees, `deploy/ci/check_lock_append_only.py` (after the one-time reset), wire golden rules, and the `PROTOCOL_VERSION(_MIN) = 2` wire format.
- **Genesis and identity tooling.** `pocold init-testnet` (`testnet.rs::prepare`/`resolve`/`build` in their v3 path), `identity.rs` (`consensus_id`, `accepts`, `explain`, `FLAG_CAPS_TEST`, `FLAG_UNPINNED`), `deploy/public-testnet/pocol_identity.py` (minus `LEGACY_BUILDS`), `independent_keys.py`, `e2e_independent_keys.py`, `e2e_mixed_release.py`, `scripts/devnet.py`, `e2e_devnet.py`, `development/public-network/KEYS.md`.
- **Fork and upgrade machinery for future forks of the new chain.** `ForkSchedule`, `configured_forks`, launcher `schedule_transition`/`upgrade_transition`, `Home::upgrade`/`inspect_upgrade`, Invitation v2 (seeds, bootstrap, `seedsRetired`), `launch_peers`, the extension's `forks.js` and per-profile `forkSchedule`, and the generic upgrade/rebind dialogs in the wallets.
- **Shared consensus library.** `lp1-node` `NonceMode::Bounded` and the `gate.rs` paths, which LP1 fixtures use.
- **Finite-chain support in the v3 programs.** Keep the `H_END != 0` paths.
- **Deploy tooling.**
  - `deploy/public-testnet` seed tooling (`node_operator`, `prepare`, `install.sh`, `monitor` after the fix, `test_operations`/`test_service`/`test_gateway`).
  - `deploy/standalone` installers and builds (`install-linux.sh`, `Start-PoCol.cmd`, `build-android.sh`, `build-windows-wallet.ps1`, `linux-desktop/*`, `test_wallet*.py`, `test_android_jni.py`, `test_launcher.py`).
  - `package.py` (simplified).
  - `Initialize-PoColNetwork.ps1` (rewritten for v3).
  - The v3 path of the rewards gateway.
  - The faucet.
  - The Studio gateway.
  - `pocol.net`.
- **Clients.**
  - Extension: generic libraries (bridge, contracts, publisher, records, site, transport, crypto, studio-keyfile), Export-Studio-Wallet tools, browser e2e.
  - Android: `KeyBackup` and the backup exclusions. Keep `namespace` and the Java package `org.pocol.node`, because the 12 JNI symbols in `bridge.c` depend on them.
- **Development trees.** `development/audit-2026-10-22` (lp4 `apply/tests.rs`, `caps_identity.rs` and pocold `consensus.rs:1963` `include_str!` its `proposals/vectors`), `development/m1` (read by lp1-node, lp3-model, pocold `site.rs` and the Windows CI scripts), `development/lp4/p2-runtime` receipts (lp4-execution-controls.yml), and `development/security-*` (four workflows run scripts there). Also VA-SPEC §1 and §3-7, the m1 baseline specs, P2P-OUTBOUND and PARAMGATE decisions, and the thesis PDF.
- **Ordinary legacy names.** "legacy" in lp4-execution-controls (Ethereum legacytests) and lp3-s1 (Rust 1.58.1) is not the old chain. Leave it.

## 3. How the consensus lock is reset

**Recommended: one reset to a single `kind=base` line, in the last code PR before the release cut (Phase 8).** This follows the owner's "keep nothing". Steps:

1. In the same PR, give `check_lock_append_only.py` a one-shot exemption pinned to the old lock: `RETIRED_BASE_LOCK_SHA256 = sha256(origin/main:consensus-sources.lock)` at the moment of the reset.
   - `check()` passes only when all of these hold: `sha256(base_text)` equals that constant; the head lock has exactly one history line; that line is `kind=base`; it has `spec=2`; and its `rules`/`maxFork` equal the code's values.
   - The wire comparison is skipped in that case.
   - Add tests to `test_check_lock_append_only.py`.
   - Once merged, the base's lock no longer matches the constant, so the exemption can never fire again. Delete the constant in the identity-pin PR (Phase 10).
   - Do not use an admin merge or a bare `--skip` flag; neither is auditable.
2. `rm prototypes/pocold/consensus-sources.lock`, then `POCOL_REPIN_CONSENSUS=base POCOL_REPIN_REF=<PR> POCOL_REPIN_NOTE="fresh v3 network genesis" cargo test --test consensus_guard`, then re-run the guard without re-pin mode.
3. Regenerate `tests/data/wire-golden.json` with neutral samples (no `chain_id 1599846715`, no `start 15000`) in the same PR. This is the only point where pinned wire keys may change.
4. Update the guard's `INSTRUCTIONS` text: no "dormant gate" or "spec-1 verdicts unchanged"; new rules go behind `fork:V`; drop `full_nonce` as a neutrality check.

**Before the reset (Phases 2-7)**, every PR that touches boundary files appends normal lines (`neutral`, `neutral-vector`, `neutral-dep`) as the guard requires, so `POCOL_RELEASE=1` and CI stay green. Phase 4 appends one `kind=bump` line: rules 1→2 and spec 1→2, which is the guard's "bump only with a new genesis" case. All these interim lines are thrown away at the reset.

**Fallback, if the owner prefers no CI exemption:** keep the old history and append a final `bump` line instead of resetting. This works with no checker change, but 23 old-network lines stay in the repo, which conflicts with "keep nothing".

**After launch**, the append-only rule applies unchanged to the new history.

## 4. Phased plan (each phase is one reviewable PR unless marked)

Dependency order: **0 → 1 → 2 → 3 → 4 → 5 → 6 → 7 → 8 → (release, genesis ceremony) → 10**. Phase 9 (docs) runs in parallel after Phase 0.

### Phase 0: Record the decisions and move CI (docs and CI only, no boundary change)
- Add `development/decisions/V3-OWNER-DECISIONS-2026-11-13.md` with the 16 answers verbatim, and mark O-0, O-33, O-32, O-21, O-14, O-10/M_max and O-6 as superseded or affected.
- Rename `full-nonce-upgrade.yml` to e.g. `consensus.yml`. Keep the generic steps and the `browser` job. Delete `branding-icons.yml` (it triggers only on `work/branding-20261031`).
- Remove from `security-owner-evidence.yml` the steps pinned to the palcertcom 0.4.0 Android artifact (these never run in this repository).
- Check branch protection for required job names before renaming.
- **Lock:** no change.

### Phase 1: Remove transition tooling outside the consensus boundary
- **Delete:**
  - `deploy/mainnet/{EARN-IN-CUTOVER-RUNBOOK*.md, EARN-IN-PREPARATION-AR.md, rehearse_cutover.py, test_rehearse_cutover.py, new_network_config.py (+test), preflight.py (+test_preflight, test_standalone_preflight), collect_seeds.py, ssh_terminal.py (+tests)}`
  - `deploy/standalone/{prepare_nonce_upgrade.py (+test), FULL-NONCE-UPGRADE.md, RELEASE-UPGRADE.md, README-WINDOWS-UPDATE.md}`
  - `deploy/public-testnet/{nonce_upgrade.py, test_nonce_upgrade.py}`
  - `deploy/rewards/studio-fork-fields.patch`
  - `development/decisions/{LEGACY-CERTIFICATION-0.4.0.md, EARN-IN-MEMBERSHIP-PROPOSAL(-AR).md, evidence/cutover-rehearsal-local.log, evidence/earnin-wallet-e2e.log}`
  - `development/earnin-rollout-2026-11-07/`
- **Simplify:**
  - `package.py`: stop copying RELEASE-UPGRADE.md.
  - `test_release_package.py` (lines ~34 and ~91).
  - `README-LINUX-ICONS.md`: keep only the icon instructions.
  - `deploy/public-testnet/{prepare.py,node_operator.py}`: remove the freeze fields and the 0.4.0 marker branch.
  - `monitor.py`: **fix the `closingHeight` null bug**.
  - `faucet/README.md`: remove the cutover text.
- **CI:** `consensus.yml` (rehearse_cutover, test_nonce_upgrade, test_prepare_nonce_upgrade, `discover -s deploy/mainnet`, the cutover log upload); `security-owner-evidence.yml` (collect_seeds, ssh_terminal and preflight paths and steps); `windows-wallet-ui.yml` (README-WINDOWS-UPDATE copy).
- **Keep for now:** `deploy/mainnet/h15000/`, `network_identity.py` and `test_upgrade.py`. Code still reads them.
- **Lock:** no change (deploy and development are outside the boundary).

### Phase 2: Remove transition code in pocold (inside the boundary)
If Phase 2 is too large, split it into 2a (cutover, export and freeze) and 2b (0.4.0 compatibility).

- **2a. Delete:**
  - `src/cutover.rs`, `src/cutover/export.rs`, `lib.rs pub mod cutover`, the `=pocold::cutover` boundary entry.
  - `testnet.rs` `cutover_params`, `prepare_cutover`, the `cutover` flag and `cutover-report.json`.
  - The `main.rs` commands `cutover`, `export-chain`, `verify-export` and their USAGE lines.
  - The whole V3-33 freeze:
    - launcher: `FREEZE_*` and the `Home::*user_tx_freeze*` functions
    - node: `freeze_user_txs_after`, `USER_TX_FROZEN`, the stats, `status.userTxFreeze`
    - `rpc.rs` -32016
    - `main.rs` flag and env var
    - `bin/pocol.rs freeze-user-txs`
  - Tests: `tests/cutover.rs`, `consensus.rs::an_evicting_node_exports_…`, `v3_late_shares.rs::a_cutover_style_rebuild_…` (optionally keep the kind-3 rebuild coverage through `Node::rebuild_state`), freeze tests in `relay.rs`, and the `cli.rs` command list.
- **2b. Delete:**
  - launcher `LEGACY_BUILDS`, `Legacy`, `Switch`, `SWITCH`, `legacy_refusal`, `SCHEMA_V1`, `compatibility_sha256`, `LEGACY_UPGRADE_BACKUP`, the `legacy` parameters, `computed_network()`, and `describe()`'s `fullNonceActivationHeight`.
  - `net.rs` `legacy_hello`/`NetEvent::Legacy` and `node.rs` `legacy_hellos`. Also the `legacyHellos` fields in `node.rs`, `wallet.rs` and `discovery.rs`.
  - The `node.rs:~888` "old v3-preview journal" check.
  - `consensus_guard.rs::the_legacy_table_is_bound_to_the_h15000_chain`.
  - The `LEGACY_BUILDS` mirror and its test in `pocol_identity.py`/`test_pocol_identity.py`.
  - `test_launcher.py`'s h15000 refusal test, and `test_upgrade.py` (delete, or shrink to a generic rebind test).
  - Rebuild the launcher `upgrade_tests` that loaded h15000 on a neutral generated invitation.
- **Clients (same PR, because they call or read the removed surface):**
  - Android `WalletText` (`LEGACY_SEEDS`, `legacyHellos`, the "Full mining range" label)
  - Windows `Wallet.cs` legacy text and `WalletUiTests.cs:88,91`
  - `test_android_jni.py`
  - every wallet caller of `pocol freeze-user-txs`
- **Lock:** append `kind=neutral` (the guard prints the list of changed files).
- **Risk:** wide signature churn in launcher.rs. Spec-1 consensus verdicts do not change.

### Phase 3: Remove old-identity fixtures, then delete `deploy/mainnet/h15000/`
- **Rewrite these on generated fixtures** (spec-1 for now; Phase 6 moves them to v3):
  - `mobile.rs` T33
  - the `testnet.rs` `h15000()` test
  - `tests/admission.rs::the_h15000_admission_report_is_unchanged`. Delete it, and keep R18 coverage in lp4.
  - `tests/network_budget.rs:281-351` (the A13 continuous fixture). **Rebuild it on a generated continuous genesis; do not just drop it.**
  - the old-encodings GENESIS table in `v3_scaffold.rs` (h15000, the websites invitation, audit `evidence/r4`)
  - the H15000 R18 case in `lp4 admission/tests.rs:472-505`
  - `identity.rs` `the_h15000_identity_is_pinned` and the `(2,15000)` explain fixtures
  - extension `unit.mjs:~187-193` and `PUBLIC_WALLET_SALT`. Set the built-in salt to `p.netKey` and put the address change in the release notes.
  - the `studio_test.mjs` switched-identity tests
- **Replace `network_identity.py`** with a generic `set_network_identity.py --from OLD.json --to NEW.json`, with no `OLD_*` constants. Phase 10 passes the old invitation from a pre-deletion commit (`git show <sha>:deploy/mainnet/h15000/network.pocol.json`), so h15000 can go now.
- **Then delete:** `deploy/mainnet/h15000/`, `deploy/mainnet/README.md` (replace it with a new launch runbook stub), `deploy/websites/*/NETWORK-UPDATE-15000.txt`, `README-MAINNET-UPDATE-AR.txt`, `RETAINED-NATIVE-DOWNLOADS.json`, `pocol.net/downloads/network.pocol.json`, and the 0.3.x/0.4.0 `.sha256` files. Regenerate `SHA256SUMS`.
- **Lock:** append `neutral-vector` if a pinned vector directory changed, otherwise `neutral`.

### Phase 4: Make v3 the release rule set (decode spec 2 by default, remove the `v3-preview` feature)
- `SPEC_VERSION = 2` in `lp1-node/src/genesis_spec.rs` **and** the byte-identical `lp4-execution/src/p1_genesis_spec.rs` (the CI `cmp` enforces it). Drop the cfg branches in `decode`/`MAX_SPEC_BYTES`. Keep `CP_SCHEMA` v1 for now, because `CP_SCHEMA_V2` is its prefix plus 15 fields.
- Delete the `v3-preview` feature from all three `Cargo.toml` files, plus:
  - `lp4 V3_PREVIEW`, the `pocold lib.rs:17` assert, `admission.rs:431`, and `testnet.rs:252`
  - identity flag bit 2
  - `build.rs` `v3Preview`
  - every `#[cfg(feature="v3-preview")]`/`cfg_attr(... allow(dead_code))` in the v3 tests (`v3_earnin`, `v3_late_shares`, `v3_fork_choice`, `v3_membership`, `v3_scaffold`, `v3_execution`, lp4 `v3_contracts.rs`, `admission/tests.rs`). These tests become default tests.
- `CONSENSUS_RULES_VERSION = 2`. Old invitations and old releases are then refused at import, not only at peering.
- Merge the decoder verdicts into a single file. Regenerate it with `REFERENCE_WRITE=1` in `decoder_reference.rs`.
- Extension, same PR: `refdec.js`/`rp.js` `SPEC_VERSION = 2`, a single verdict file, and the `forks_test.mjs` assertions. **Never hand-edit the verdicts.**
- Python: `pocol_identity.py` `SPEC_VERSION = 2`. `package.py` and `node_operator.verify` no longer see a preview build.
- **CI:** delete the `v3-preview` job and the "Dependency-only preview must not compile" step, and move the `cmp` checks into `consensus.yml`. In `system-programs.yml`, drop the doubled preview runs.
- **Lock:** append **`kind=bump`** (rules 2, spec 2, maxFork 2). The guard requires `bump` because rules changes.
- **Risk:** medium. Spec-1 devnets still boot because they decode with `gsVersion ≤ 2`. Admission does not yet refuse them.

### Phase 5: Make devnets v3 by default and migrate the tests (launch evidence; may take 2-3 PRs)
- `lp4-execution/src/devnet.rs` `DevnetConfig::new`: spec 2, V3 programs, `nonceMode` 1, `H_END = 0` where the test expects continuous. Remove the `earnin` flag and the v1/continuous/earnin program choice.
- Fix the roughly 20 pocold test files that ran only on spec-1 rules: `consensus.rs`, `net`, `relay`, `rejoin`, `store`, `state_store`, `journal_measure`, `memory_bounds`, `network_budget`, `caps`, `fee_pool`, `discovery`, and the others. They hard-code the missing base-fee floor, legacy log caps, bounded nonces, the Continuous gas envelope and the 100 PCOL bond.
- Fix lp4 `apply/tests.rs` and `resources/tests.rs`.
- Re-point `launch_delay_asert.rs` at the chosen CP.
- Delete `earnin.rs` (covered by `v3_earnin.rs`).
- Python fixtures: `test_launcher.network()`, `test_wallet*` and `independent_keys.py` move to v3. Remove the `fullNonce`/`continuous`/`earnin` options. Port `test_wallet_earnin.py` to `v3: true`.
- This is the first time sync, reorg, A13 memory and checkpoints are checked on the rules mainnet will run, so treat the results as launch evidence.
- **Lock:** `neutral` or `neutral-vector` lines, depending on which pinned files change.
- **Split:** 5a covers devnet.rs plus lp4; 5b and 5c cover pocold test groups.

### Phase 6: Remove the spec-1 rule set (consensus-critical; one reviewed PR, or 6a + 6b)
- **6a: admission and programs.**
  - Admission requires program set V3, `nonceMode` 1 and `forks == [[1,0]]`.
  - `MAX_FORK_VERSION = 1` in both copies of `profile.rs`. Delete full-nonce and fork-v2 coverage: `tests/full_nonce.rs`, `fork_configuration.rs`, lp4 `profile_execution.rs` with `profile-execution.json`, and the v2 cases in lp1 `tests/profile.rs`. Keep the fail-closed `configured_forks` cases.
  - Delete `PoColSystemV1.sol`, `PoColSystemEarnIn.sol`, `compile_system_{v1,earnin}.cjs`, `system-{v1,continuous,earnin}.json`, the earn-in replay tests (`earnin_contracts.rs`, `earnin-vectors`) and the `v3_set_at_the_earnin_values…` equivalence test.
  - **First port `earnin_refund_fix.rs` and `earnin_halt_proof.rs` to `system-v3.json`.**
  - `apply.rs`: one bundled program set. Refuse unless `sysCodeHash == system-v3`. Rename `EARNIN_ACCRUING` to `ACCRUING`.
  - `admission.rs`: delete `GAS_ENVELOPE` and `GAS_ENVELOPE_EARNIN`. Make `GAS_ENVELOPE_V3` self-contained by **re-measuring `beginBlock` (944,000) and `pruneShares` (2,781,000) on v3**.
  - Rewrite lp4 `caps_identity.rs` on V3, or drop it.
  - `build.rs`: remove the digest entries for the deleted JSON files.
  - `testnet.rs`: always v3; drop `continuous`/`earnin`/`fullNonce`, `local_for`, the spec<2 branches and the spec-1 tests.
  - `system-programs.yml`: reproduce only `system-v3.json` and its mutants, and keep the EIP-170 size check.
- **6b: collapse the dead `v3_active`/`effective_nonce_mode` else-branches.**
  - `chain.rs` (12 sites, including `full_nonce_rule`), `node.rs` (12), `rpc.rs` (7), `wallet.rs` (5; remove the v1 `REGISTRY_GAS` and re-measure the registration gas on v3), lp4 `resources.rs` (6), `user_phase.rs` (2).
  - Keep the `rpc` methods. Keep `activeVersion` for future forks.
  - Do it mechanically, and require identical v3 verdicts on every Phase 5 test.
- **Lock:** `neutral-vector` lines.
- **Option:** 6b may follow launch. I advise against it: after launch, every consensus edit must be proven neutral on the live chain, so it costs more.

### Phase 7: Pre-genesis client and packaging cleanup (outside the boundary)
- **Android:** a single application id, `org.pocol.wallet`. Drop the `pocolApplicationId` switch, the "(new)" label and the README migration section. Reset `versionCode`/`versionName`.
- **Windows:** delete `released/Wallet-50c2f27.cs.txt` and the `-VerifyRegression` block in `build-windows-wallet.ps1`, and remove that switch from `windows-wallet-ui.yml`. Bump the `AssemblyVersion`.
- **Extension:** delete `PUBLIC_FORKS_KEY`, the `profiles.publicSchedule` handler and the options "PoCol Mainnet activation" control. Pin the built-in schedule `[[1,'0']]`. Keep `forks.js` with `MAX_FORK_VERSION = 1` to match `profile.rs`. Bump the version and rewrite the README text.
- **`package.py` and `test_release_package.py`:** a new version and names; no `LEGACY_BUILDS` check, no `requiresCoordinatedUpgrade`, no old-signer fields; take the identity from the invitation.
- **`five-device-build.yml`:** new name and artifact names; drop `--test cutover` and `test_upgrade.py`; a single-id Android badging check; remove the stale branch triggers.
- **`Initialize-PoColNetwork.ps1`:**
  - `v3 = $true`
  - 4 founders
  - server IPs as parameters
  - exclude 1599846715, 261597237, 777001 and 7779xx from the random chain id, and keep it below 2^32
  - drop `registrationPerBlock` if v3 ignores it
- **Rewards and gateway:** after V3-PR-G2 (#41) merges, the v3 network config becomes mandatory. Drop the v1 header and fork-schedule code, and the old-identity defaults.

### Phase 8: Lock reset and the last boundary cleanup (release cut)
- Apply the §3 procedure: checker exemption, delete the lock, re-pin `base`, neutral wire golden samples.
- Optional deletions inside the boundary, batched here:
  - old local data formats (`POCKPT01` `MAGIC_V1`, the first `POCOLJ2` layout, history.redb from before S4, `consensus.rs::pr7_encode_v1`)
  - the rest of the `build.rs` scope labels, if the owner decides on the labels (§5)
- Then build with `POCOL_RELEASE=1`, run the full CI, and tag the release.

### Genesis ceremony (operations, not a PR)
- Wipe the old installs so no old key is reused: `/opt/pocol-standalone`, `/opt/pocol-testnet` and service state on the 3 US servers; `%LOCALAPPDATA%\PoCol` on the PC; the `org.pocol.node` app on the phone.
- Generate fresh keys on each device with the independent-key flow.
- Run `pocold init-testnet` with the v3 config: 4 founding miners, `accounts` of 10^24 wei each, `F0_*` per decision, `P_MAX`/`M_max`/`T_blk` per decision, and the new chain id.
- Build the invitation with the 3 US seeds. Run `launch_delay_asert.rs` on the final CP.

### Phase 10: Pin the new identity (after the genesis hash exists)
- Run `set_network_identity.py --from <old invitation from git history> --to <new invitation>`, or edit by hand. Files:
  - `network_display.rs` `MAINNET_CHAIN_ID`/`MAINNET_GENESIS`
  - a new golden identity test in `identity.rs` and in `test_pocol_identity.py`
  - `studio-network.js` (`chainId`, `genesisHash`, the spec-2 `genesisPre`)
  - Studio `dist/app.js`; fix the Arabic "testnet" label
  - the studio gateway; regenerate the installer
  - the rewards gateway defaults
  - `pocol.net` JS/PHP; `reward.php` to `pocol-block-reward-v3`
  - faucet `config.example.json`
  - the root README identity block
  - tests that hard-code 1599846715 (`contract_ui_test`, `publisher_test`, `studio_test`); better, read `N.NETWORK.chainId` instead
- Remove `RETIRED_BASE_LOCK_SHA256` from the checker.
- **Lock:** `identity.rs` is in the boundary, so this appends the first `neutral` line of the new history.
- Explorer and IDE bundles have no sources in the repo; they need the sources or a careful constant patch.

### Phase 9: Rewrite the specs as v3-only (docs only, in parallel; finish before Phase 10)
- **VA-SPEC §2:** remove the H15000 golden identity, `legacyHellos` and 0.4.0 Hello arity, the `LEGACY_BUILDS` guard line and the dormant-gate policy. Restate §2.3 as the post-launch bump policy, and say the lock was reset once.
- **`m1/reference/governance.md`:** remove the P-live table, the dormant paragraph and the S/re-genesis paragraph; promote the candidate v3 profile to the base profile.
- **`consensus.md`, `incentives.md`, `economics.md`, `threat.md`, `execution.md`, `network.md`:** fold each v3 amendment into the base text and keep the section numbers. `threat.md` A6 must disclose the risks of an unlimited probation pool with no creation fee.
- **`V3-DECISIONS.md`:**
  - drop the "preserve spec-1 verdicts" footers, the Lock and activation strategy section and D-V3-33
  - drop the cutover clauses of D-V3-12 and D-V3-21
  - replace the H15000 holder table with the 4 new founders
  - fix the line-13 "handoff roadmap" reference
  - keep `decisions.json` in sync
- **`V3-DESIGN.md`:** replace §3 with a short "fresh v3 genesis launch" section; mark V3-33 moot; rewrite V3-32.
- **`V3-EXECUTION-FEES.md`:** remove the creation-fee sections if the fee is dropped.
- **`EARN-IN-SPEC.md`:** move §1-6 and the §9 mutant list into `incentives.md` and delete the file. Delete §7 C1-C6.
- **`OPERATIONS.md`:** remove the coordinated switch and the D1/D4 legacy text.
- **`PLAN.md`:** public mainnet directly. **Change `readiness.rs` `PLAN_GATES_OPEN` in the same commit.**
- **`development/README.md`, `STATUS-EN.md`:** rewrite.
- **Root README:** History, the `cutover` command, the deploy/mainnet links, the palcertcom links.
- **`deploy/standalone/README.md`:** a fresh-install guide; rebase on v3-pr-g3b.
- Keep V3-OWNER-DECISIONS-2026-11-08 and -12 as dated records with "superseded" notes, unless the owner asks for them to be deleted (§5).

## 5. Decisions needed

**From the owner:**
1. **Chain id.** Pick a fresh number. Avoid 1599846715, 261597237 (the planned earn-in id), 777001, 7779xx, and anything at or above 2^32: the Studio gateway checks 4 bytes, and `testnet.rs` refuses ids at or above 2^53. Keep Simnet/ExperimentalDevnet admission as the old network had. Moving to the R22 Network profile (777001 only) or Specification scope is a separate decision; until then the readiness `profile-kind` blocker stays open.
2. **The four founders.** Answer 15 suggests the 3 US servers and the PC as founding miners, with the phone as a wallet or light node. Please confirm. Also confirm that the 1,000,000 PCOL each is a genesis `accounts` allocation on top of mining rewards, and check it against the R23 supply bound with M3 issuance.
3. **"No maximum at all" (members).** `M_max` is a u16 with a hard limit of 65535. A13 memory, `REG_RECORDS_MAX` and the nonce partition all need some bound. Recommend: choose an explicit `M_max` and re-measure gas and A13 for it.
4. **"No limit on the probation pool."** `P_MAX` cannot be unlimited, and **`P_MAX = 0` blocks every zero-bond newcomer**. A large `P_MAX` (up to `M_max`) is outside the measured gas domain (64), so it needs new SYS-gas measurements, or admission records an R18 condition. A large pool also makes Sybil membership cheap, which must be disclosed in `threat.md`.
5. **"No account-creation fee."** Confirm whether this means `F0_acct`/`F0_slot`/`F0_code = 0`, the state-creation burn (the devnet default is 1e15), or the miner registration bond. With the burn at 0, an attacker can fill `ACCT_CAP`/`SLOT_CAP` and block new accounts for everyone; that is why O-21b was chosen. If confirmed, also remove `pocol_quoteCreationBurn`, `Halt::CreationFee` and the 2026-11-12 decision (2).
6. **Block time "set by difficulty, not time".** ASERT already sets difficulty from solve times, but it still needs a target spacing `T_blk`. Recommend keeping the O-6 value of 30 s as that target.
7. **Release version and labels.** The new version number, and whether `build-info` `scope` moves away from `experimental-testnet` and `publicLaunchReady` becomes true for a public mainnet.
8. **Android application id.** Recommend `org.pocol.wallet` only, with a new signer.
9. **Old owner-decision records.** Keep the 2026-11-08 and 2026-11-12 records as dated history with "superseded" notes (recommended), or delete them as part of "keep nothing". If deleted, the owner rule in §1 must move into the new record.
10. **Reusing the 3 US servers.** If they are reused, they must be wiped before `install-linux.sh` will run. They are otherwise only parameters.

**Technical decisions (recommendations; no owner input needed):**
- `CONSENSUS_RULES_VERSION = 2` at the new genesis.
- `MAX_FORK_VERSION = 1`.
- Reset the lock with the sha-pinned checker exemption.
- Finish Phases 5 and 6 before launch.
- Delete the EarnIn set and keep V3, after porting the F1-F3 regressions.
- Keep CP schema v1 decodable until Phase 6, then decide whether to refuse `gsVersion 1`. Refusing it means regenerating `tests/data/devnet-spec.hex` and `rp_fixture.mjs` in the extension as spec 2.

## 6. Cross-phase hazards
- **Lock checker:** fails on any reset without the exemption, and also fails on deleting the lock.
- **`h15000/` deletion:** about 15 Rust, JS and Python consumers must be rewritten first (Phase 3 before the delete).
- **`network_identity.py`:** reads `h15000/`. Replace it with the generic tool in Phase 3.
- **Decoder verdicts:** `refdec.js` and `genesis_spec.rs` must change in the same PR, with verdicts regenerated from Rust.
- **Built-in extension profile:** a spec-2 `genesisPre` needs Phase 4 before Phase 10.
- **Shared copies:** lp1 and lp4 `p1_*` must change together, and the `cmp` check must survive the CI move.
- **`PLAN.md`:** edits need a matching change to `readiness.rs` `PLAN_GATES_OPEN`.
- **Gas fixtures:** the R18/SYS gas fixtures need real re-measurement, not copied numbers (`beginBlock`, `pruneShares`, registration gas).
- **Branch protection:** may name the old workflow and job names.
- **In-flight branches:** G2, G3a and G3b, N5 and v3-pr-g3b-wallets edit `pocol_rewards`, `pocol_identity`, `package.py` and the standalone README. Merge or rebase them before Phases 7 and 9.
