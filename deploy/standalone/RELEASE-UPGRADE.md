# PoCol Mainnet · release updates

Releases after 0.4.0 peer on a **consensus identity** (VA-SPEC §2): the chain, the
genesis, the consensus rules revision (`consensusRulesVersion`) and the fork schedule.
The build digest (`compatibilitySha256`) is informational. Launcher homes are bound
to `pocol-network-v2` invitations, which name the rules revision instead of a build
digest. Check what a file would change before applying it:

```text
pocol inspect-upgrade --home EXISTING_POCOL_HOME --network network.pocol.json
```

It prints `change` (`none`, `rebind` or `activation`), the `appended` entries and the
file's description (`consensusIdentity`, `activations`, …), and writes nothing. The
Windows and Android wallets run the same check before **Confirm network upgrade** and
show "Already applied", "Release update: consensus rules and schedule unchanged" or
one line per appended entry ("Full mining range at block H", "Consensus version v at
block H").

When a wallet finds no compatible peer it says why instead of only "Looking for
peers": "Peers use a different network configuration: apply the published network
upgrade file (Settings → Upgrade) or update PoCol." followed by the node's
`lastIncompatibleReason` (when consensus refusals were counted), or "Seeds still run
PoCol 0.4.0; wait for the announced switch." (when only 0.4.0 handshakes arrived).

## Same consensus rules: install and start

A release with the same `consensusRulesVersion` that adds no fork entry (it changes
only the build digest) needs no network file and no coordination. Stop the wallet or
service, install the complete new package over the application folder (keep the home),
and start. Seeds and wallets may update in any order: nodes with different digests
and the same identity peer. `pocol_status` shows the new `compatibilitySha256` and the
unchanged `consensusIdentity`.

## Fork activation: `pocol upgrade`

A new fork entry (for example `[2,H]`, or a later version once a release supports it)
is published as a v2 invitation that keeps the chain, genesis, allocation, seeds and
every existing entry and appends future entries only. Operators prepare it with
`prepare_nonce_upgrade.py --append V:H` (repeatable; `--activation-height H` is a
deprecated shorthand for `--append 2:H`), see `FULL-NONCE-UPGRADE.md`. Every node must
apply it before height `H`:

```text
pocol inspect-upgrade --home EXISTING_POCOL_HOME --network NEW_FILE
pocol upgrade --home EXISTING_POCOL_HOME --network NEW_FILE
```

The engine refuses a changed or removed entry, an entry at or below a stored block,
a version above this release's `maxForkVersion`, other seeds, allocation or genesis,
another `devnet.json` key, another rules revision, a v2 → v1 change and a running node.
It keeps the previous binding as `network.pocol.json.before-<16 hex of its fingerprint>`
(one per upgrade, so later upgrades never collide; the 0.4.0 `.before-full-nonce`
backup is left as it is) and a recoverable `network-upgrade.pending.json` intent. If
interrupted, rerun the **same** command (in a wallet, Upgrade existing network with the
same file): `inspect-upgrade` of that file reports the interrupted change with
`"resuming": true`, and any other file is refused until it is finished. A node that has not applied the entry is
refused by the others with "peer has not scheduled activation (v,H): it must apply the
published network upgrade"; the lagging node itself reports how to fix it in
`lastIncompatibleReason` and in the wallet.

## Optional rebind to a v2 file

`pocol upgrade` with a v2 file of the same content and schedule rebinds a home
(`change: rebind`): nothing about peering changes. This applies to a v2 file that
differs only in form, and to a v1 0.4.0 home only once its legacy entry is certified
(Option A below). `pocol import` (and `pocol start --network`) with an equivalent file
is accepted read-only and never rewrites a saved binding; a different invitation is
refused with "this home is bound to another invitation; if this is the same network in
a newer form run `pocol upgrade`; otherwise use a separate home".

## From 0.4.0: the one coordinated switch

Deployed 0.4.0 speaks a 9-item handshake and cannot peer with any later build. The
first release with the consensus identity (R1) is therefore the last coordinated
switch (`development/public-network/OPERATIONS.md`, owner decision D1). This code
implements the recommended **Option B**: the network moves to the held earn-in
re-genesis, published as a new v2 invitation. Seeds stop together before any of them
restarts (a launcher home started with `pocol start` mines at once, without waiting for
peers). Until the switch, R1 refuses an H15000 home with:

> This network was replaced by a new genesis. Follow the release notes to move the old
> network files aside; keep your wallet key.

Keep 0.4.0 installed until the switch is announced. Under Option A (R1 on the live
H15000 chain) the text would instead read "This release cannot yet run the PoCol
Mainnet 0.4.0 network. Keep 0.4.0 until the announced switch." and H15000 homes would
start unchanged once the legacy entry is certified
(`development/decisions/LEGACY-CERTIFICATION-0.4.0.md`). Nothing is certified in
this release, so every 0.4.0 binding is refused (fail closed).

## Rollback to 0.4.0

Before the switch nothing in a 0.4.0 home is changed by R1: it refuses the home and
writes nothing, so 0.4.0 keeps running it. After an Option A switch: stop the node; if
`network.pocol.json.before-<16 hex>` exists, restore it as `network.pocol.json` and
remove `network-upgrade.pending.json`; start 0.4.0. Do not publish a v2 twin of the
H15000 file until the rollback window has closed: 0.4.0 cannot parse v2. Once blocks of
a new fork version exist, do not roll back.

## Resync a launcher home

For a new genesis (Option B), or a home whose chain diverged beyond recovery: stop the
wallet or service; move `state/`, `wallet-activity.json`, `network.pocol.json`,
`network/` and any `network.pocol.json.before-*` and `network-upgrade.pending.json`
files into a folder outside the home; **keep `identity.pocol-key` and
`identity.address`** (the wallet key and address); then start with the new invitation
(`pocol start --home HOME --network NEW_FILE`, or import it in the wallet). Rewards
mined on a private branch are lost. Keep the moved files until the new chain has
synchronized.

## Pending upgrade left by 0.4.0

A home with an interrupted 0.4.0 upgrade (`network-upgrade.pending.json` written by
0.4.0) is refused with "an interrupted upgrade started by PoCol 0.4.0 is pending;
finish this upgrade with PoCol 0.4.0 first". Rerun the same upgrade with 0.4.0, then
install the new release. A pending intent for other rules names the release that
started it.

## Android 0.4.0 installs cannot be upgraded in place

0.4.0 Android packages are unsigned (`deploy/mainnet/h15000/README.md`), so no later
APK can update them in place (A3). The new bridge (`inspectUpgrade`) applies to future
signed installs only. Do not uninstall a funded wallet: Android backup is disabled and
uninstalling deletes its key.

## Release checklist

- The consensus guard is green and the build is pinned: `build-info.json` reports
  `consensusPinned: true` (packaging builds set `POCOL_RELEASE=1`, and `package.py`
  refuses anything else).
- For every release that re-pinned `consensus-sources.lock`, the H15000 replay is rerun
  before any legacy entry is (or stays) certified.
- A certified `LEGACY_BUILDS` entry's `sources` equals the build's
  `consensusSourcesSha256` (`package.py` refuses otherwise).
- The R1 hashes are pinned in `deploy/mainnet/preflight.py` (`R1_BINARIES`,
  `R1_SOURCE`) and the matching `collect_seeds.PREFLIGHT_SHA256`; until then the R1
  preflight profile fails closed.
- Package with `package.py --network NETWORK_FILE` when the release is for a published v2
  invitation: `release-info.json` then names its `consensusIdentity` (null otherwise),
  besides `consensusRulesVersion` and `protocolVersionMin`.

## The 0.4.0 update (reference)


This release adds the full unsigned 64-bit mining range at the network's shared
activation block and uses **PoCol Mainnet** for the existing network:

- Chain ID: `1599846715`
- Genesis: `0xbe137bfdb4fdf332f2e1500ee1a67e0b22ce6bdf57efa46fb4ea7b0debe509ca`
- Native currency: **PCOL**

The name identifies this chain; it does not change genesis, balances or mining
registration. Other networks keep a generic designation. Existing admission
conditions remain: `scope` and `publicLaunchReady: false` are unchanged. The name
is not a claim of completed audit, financial qualification or guaranteed value.

`release-info.json`, `build-info.json`, `source-commit.txt` and `SHA256SUMS` identify
the build. These build artifacts do **not** contain a network invitation, private
keys or a preselected activation block. Obtain the matching public upgrade file
through the network operator's authenticated release channel. Check its chain,
genesis, build compatibility and future activation block before applying it.

## Windows: keep the same account and wallet home

1. Close PoCol Wallet and allow its node process to stop. Keep the existing Windows
   user profile and `%LOCALAPPDATA%\PoCol` directory, including the encrypted key,
   identity, state and local history. The application folder is separate from this
   wallet directory.
2. Extract the **complete** 0.4.0 Windows package into an application folder. Keep
   `PoCol-Wallet.exe`, `pocol.exe`, `pocold.exe` and the remaining tools together;
   do not mix the old engine with the new interface. This is a portable package,
   not an MSI installer. Verify the published checksums before opening it.
3. Place the matching new `network.pocol.json` beside the application. Open
   **PoCol-Wallet.exe → Settings → Upgrade existing network** while disconnected,
   and select that same file. An existing wallet may stay offline until this step
   completes; ordinary import intentionally refuses a changed binding (later
   releases name `pocol upgrade` in that refusal and show the change first).
4. Review the network, genesis and activation block, then choose **Confirm network
   upgrade**. The native engine refuses a different chain, changed genesis,
   changed seeds, rollback, rescheduled fork or already-passed activation block.
5. Connect when the coordinated release is ready. Confirm the same public address
   and allow local synchronization to finish. Enable mining again if desired;
   mining in the wallet starts disabled after restart (a launcher home started with
   `pocol start` mines at once, without waiting for peers). A new mining registration
   is not needed.

The upgrade writes only public network configuration, retaining a backup and a
recoverable pending intent. It leaves the wallet key, address and stored chain
unchanged. If interrupted, keep those files and rerun the **same** upgrade. Do not
delete the wallet home or regenerate genesis to bypass a validation failure.

The equivalent explicit command is:

```powershell
.\pocol.exe upgrade --home "$env:LOCALAPPDATA\PoCol" --network .\network.pocol.json
```

Windows executables are not Authenticode-signed by this build workflow. Their
checksums establish artifact consistency, not publisher authentication.

## Linux: preserve the service user and state directory

`install-linux.sh` remains **fresh-install only**. Do not rerun it over an existing
service. Stop the existing service, stage the matching complete binaries and
public invitation, and apply `pocol upgrade --home EXISTING_HOME --network FILE`
as the existing wallet owner before restarting. Keep the same systemd service
account, ownership, key and state directory. For the standalone installer that
home is `/var/lib/pocol-standalone`; other deployments can use a different home.

Public seed deployments with a separate operational identity binding require the
reviewed `deploy/public-testnet/nonce_upgrade.py` stopped-state migration instead
of treating that binding as a standalone wallet invitation. Follow the release's
coordinated server rollout; do not substitute one layout for the other.

The Linux archive preserves executable permissions. Its optional desktop shortcut
shows package information; the Linux package remains command-line tools and a
node service, with no new graphical wallet.

## Android: release signing is required before distribution

The build produces **`PoCol-Mainnet-0.4.0-arm64-unsigned.apk`**, versionCode **5**,
application ID **`org.pocol.node`**. It is **not installable** until signed. The
release process intentionally does not generate a new debug key or ship a debug
APK as an update.

An in-place update must retain the installed application's signing identity (or
a supported, authorized signing-key lineage). The last distributed 0.3.1 APK,
versionCode 4, has certificate SHA-256:

```text
bbb399007be5217a5a939eaa9b0ed1335170f7397a7487ef6da9acddb21b8772
```

The matching private signing key is not included in these artifacts and was not
available to this release preparation. A public certificate fingerprint cannot
reconstruct it. A fresh release/debug key does not authorize an update to that
installed APK. Earlier distributed debug APKs used different certificates, so
verify the APK actually installed on each device rather than assuming a shared
signer across all earlier versions.

The release operator must sign locally with the retained matching keystore, verify
the resulting signer with Android SDK `apksigner verify --print-certs`, verify
application ID and versionCode with `aapt dump badging`, and test an **in-place**
update on a representative device. Keep signing keys and passwords outside the
repository, CI artifacts and public download folder. Recompute the signed APK's
checksums and release metadata before publishing it as installable.

Do **not** uninstall a wallet to work around a signature mismatch. Android backup
is disabled and uninstalling deletes its private account and local state. This
build has no portable recovery export. Until the matching signer is available,
the Android update is blocked; preserve the currently installed app.

After a successfully signed in-place application update, disconnect the wallet,
open **Settings → Upgrade existing network**, select the matching public network
file, and review the network, genesis and activation block before confirming.
Reconnect when the coordinated release is ready and verify the same address.

Android signing reference:
https://developer.android.com/studio/publish/app-signing
