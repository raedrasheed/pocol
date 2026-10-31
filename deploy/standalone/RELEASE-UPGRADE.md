# PoCol Mainnet · 0.4.0 update

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
   completes; ordinary import intentionally refuses a changed release binding.
4. Review the network, genesis and activation block, then choose **Confirm network
   upgrade**. The native engine refuses a different chain, changed genesis,
   changed seeds, rollback, rescheduled fork or already-passed activation block.
5. Connect when the coordinated release is ready. Confirm the same public address
   and allow local synchronization to finish. Enable mining again if desired;
   mining starts disabled after restart. A new mining registration is not needed.

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
