# Windows Activity update

This update replaces only `PoCol-Wallet.exe`. It is compatible with the existing
five-device release built from `50c2f27ee67961dec7aaf0eb735c0b641810e0ea`.
The `pocol.exe` engine, network file, Linux nodes and Android installation remain
the existing release. No new mining registration is required.

1. Close the Windows wallet and let its engine stop.
2. Copy `PoCol-Wallet.exe` from this update into your existing PoCol folder,
   beside `pocol.exe`, replacing the old GUI executable.
3. Open `PoCol-Wallet.exe` from that folder. Your existing Windows user profile
   supplies the same account and locally saved transaction history.
4. Enable mining again if desired; it starts disabled after each restart.

Keep the existing `%LOCALAPPDATA%\PoCol` folder. This update does not migrate,
replace or reset wallet keys, balances, registration or network data.

## Changes

- Activity now renders nested JSON lists returned by .NET Framework's
  `JavaScriptSerializer`, including `ArrayList`.
- The submitted-transaction message follows Pending, Included, Reverted and
  Unconfirmed receipt states and available confirmation counts.
- The connection label says **direct peers**: three connections to the seed
  servers do not mean there are only three miners or devices in the network.
- Periodic status refreshes no longer acknowledge an unrelated pending command.

Activity lists transactions submitted from this device. It is not a complete
history of incoming transfers or a list of mining rewards.

## Verification

`WALLET-UPDATE-SHA256SUMS` supplies the hash of the updated GUI. Its GUI entry
supersedes the old GUI hash in the original package manifests. Keep this update's
checksum file with the installation; other original binaries are unchanged.
The UI regression test runs the actual .NET Framework serializer and WPF window
on Windows. It first reproduces the history failure with the released source,
then checks the fix, exact amount text, receipt transitions and mining controls.
Any included activity preview is a labelled test fixture, not live account data.
