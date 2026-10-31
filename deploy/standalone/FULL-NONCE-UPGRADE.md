# Full 64-bit nonce upgrade

This change implements the owner's 31 October 2026 request to use the largest
supported mining domain: **0 through 18,446,744,073,709,551,615 inclusive**.
The domain size is `2^64`; range arithmetic uses `u128` for the exclusive endpoint.

## Activation and history

No activation height is selected by this source change. A network remains on its
existing rules unless its trusted local `devnet.json` contains a validated schedule
of the form `"forks": [[1, 0], [2, H]]`.

- Before `H`, version 1 uses the original genesis nonce mode.
- Starting at `H`, version 2 assigns the full 64-bit domain.
- Chain ID, genesis bytes/hash, allocation, system contracts, wallets and databases
  remain the same. Genesis CP4 stays unchanged; it describes the original rules.
- Both block construction and validation derive the assignment from the rewarded
  block's height. Historical blocks retain their original ownership and state roots.
- Peers must have matching source compatibility and activation schedules. Different
  heights are rejected during the handshake, before accepting transactions/blocks.
- Opening an existing journal checks every stored header against the configured
  version schedule before checkpoint restoration. A retroactive schedule or downgrade
  that conflicts with stored blocks fails closed.

The reward formula, share target, block target/difficulty and supply policy are
unchanged. A full range gives every active assignment member a nonempty range; it
does **not** guarantee that every registered miner contributes a share or receives a
payment in every block. At an easy block target a miner may find a solution before
other shares arrive.

## Prepare a coordinated release

All commands below are operator instructions. This document does not activate or
deploy a network. Do not modify `genesis.spec.hex` or initialize a replacement chain.

1. Build and test the same source revision for every participating seed and native
   client. Compare `pocol build-info` compatibility hashes across platforms. Android
   updates must use the existing installation's signing key; an unrelated APK signing
   key cannot update that installation. Do not uninstall a funded wallet to upgrade.
2. Agree on a future height `H` with enough time to stop and update all participating
   nodes. Read the current canonical head immediately before preparing the plan.
3. Save the new release's `pocol build-info` JSON, and prepare a new invitation:

   ```text
   python deploy/standalone/prepare_nonce_upgrade.py --network network.pocol.json --build-info new-build-info.json --activation-height H --observed-head CURRENT_HEAD --out-dir full-nonce-plan
   ```

   Replace `H` and `CURRENT_HEAD` with agreed numeric values. The tool creates a new
   directory with the invitation, browser schedule and checksum report. It never
   opens a wallet, edits a running service or selects a height automatically.
4. Stop the PoCol applications and services. Keep a recoverable backup of each
   existing home, including its identity and state, using the operator's normal
   private backup process. Distribute the reviewed public invitation and matching
   binaries through the existing trusted release channel.
5. On each existing native home, run the **new** binary explicitly:

   ```text
   pocol upgrade --home EXISTING_POCOL_HOME --network full-nonce-plan/network.pocol.json
   ```

   This checks the previous binding, exact genesis/allocation bytes, unchanged seeds,
   metadata and append-only activation. It refuses activation at or below any stored
   block height. It writes only public network configuration and its recovery records;
   the wallet key and state databases are retained. If interrupted, repeat the same
   explicit upgrade command; ordinary startup refuses an unfinished upgrade.
6. Seed bundles managed by `deploy/public-testnet/node_operator.py` have a separate
   `network-binding.json` that pins the release. Prepare a replacement public bundle
   from the existing verified one and the new native binaries:

   ```text
   python3 -B deploy/public-testnet/nonce_upgrade.py prepare --old-bundle OLD_BUNDLE --expected-old-manifest OLD_MANIFEST_SHA256 --bin-dir NEW_NATIVE_BIN --network full-nonce-plan/network.pocol.json --out NEW_BUNDLE
   ```

   Review the resulting manifest checksum, stop the service, and migrate only its
   release binding under both node/operator locks:

   ```text
   python3 -B deploy/public-testnet/nonce_upgrade.py migrate --old-bundle OLD_BUNDLE --new-bundle NEW_BUNDLE --expected-new-manifest NEW_MANIFEST_SHA256 --data EXISTING_STATE_DIR
   ```

   These offline commands preserve the node identity, peers, service definition,
   genesis and allocation. They verify journal framing/commit records without
   repairing or rewriting chain state, and keep a recoverable original binding.
   Install/switch to the reviewed new bundle only as part of the coordinated stopped
   release. The tool does not start services. Raw `pocold --network` services without
   this wrapper require the matching executable and reviewed `devnet.json`, with
   their original genesis/allocation and data directory retained.
7. Update the Chrome extension and configure the **locally trusted** profile schedule
   from `browser-fork-schedule.json`. RPC responses cannot authorize the fork height.
   RN/LN identities and wallet network keys retain the same chain/genesis identity.
8. Install the updated reward adapter and pass the same trusted schedule explicitly
   as documented in [`../rewards/README.md`](../rewards/README.md). Its corrected
   activity window is CP24 (`W_act`), not CP7 (fallback wait). Preserve the existing
   public RPC allowlist; these integrations need the public fork schedule fields.
9. Restart coordinated nodes and verify matching genesis, source compatibility,
   schedule, head synchronization and assignments. At `H`, headers must report
   consensus version 2 and the combined nonce ranges must cover `[0, 2^64)`.

Once version-2 blocks exist, use a corrected compatible release for recovery.
Do not roll back the schedule, restore an older node executable against that state,
or delete the chain/wallet to bypass a refusal.

Android has a separate explicit **Settings → Upgrade existing network** action.
Disconnect and reopen the application first, select the reviewed invitation, review
the chain ID and activation height, and confirm. Ordinary network import does not
silently upgrade an existing wallet. The native home lock is still enforced.

## Verification

Relevant checks include profile/Gate schedule validation, the seven-member two-value
regression, maximum nonce ownership, activation boundaries, old-block replay,
cross-activation reward settlement, reorgs, checkpoint restart, mismatched schedules,
wallet-preserving invitation upgrade, browser verification and reward projection.
See the pull request's validation results for the checks actually run on a revision.

The supported arithmetic is in `prototypes/lp1-node/src/assignment.rs`, effective
rules in `prototypes/pocold/src/chain.rs`, and unchanged distribution in
`prototypes/lp4-execution/contracts/system/PoColSystemV1.sol`.
