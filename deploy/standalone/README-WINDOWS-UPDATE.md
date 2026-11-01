# Windows icon and interface update — 2026-10-31

For an existing installation, this update replaces only `PoCol-Wallet.exe`.
The full Windows distribution retains the original engine binaries from
`5e4ac8ac83b4417eb33f33cd405db686f501d5c5`, identified by `source-commit.txt`
and `build-info.json`. The icon GUI was built and tested from
`103f5c43f574e469916e90f53a9cc1905bf8de4a`, identified by `UI-SOURCE-COMMIT.txt`.
Keep the existing engine, network file and Windows account data when applying
the small GUI update. No new mining registration is required.

1. Close the Windows wallet and let its engine stop.
2. Copy `PoCol-Wallet.exe` from this update into your existing PoCol folder,
   beside `pocol.exe`, replacing the old GUI executable.
3. Open `PoCol-Wallet.exe` from that folder. Your existing Windows user profile
   supplies the same account and locally saved transaction history.
4. Enable mining again if desired; it starts disabled after each restart.

Keep the existing `%LOCALAPPDATA%\PoCol` folder. This update does not migrate,
replace or reset wallet keys, balances, registration or network data.

## Changes

- The supplied PoCol icon appears in the executable, taskbar, window and dialogs, and PCOL asset cards.
- The Windows executable embeds 16, 24, 32, 48, 64, 128 and 256 pixel icon sizes.
- The application data directory, engine binaries and network configuration are unchanged.

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

`SHA256SUMS` supplies the hashes of this package, including the updated GUI.
Its GUI entry supersedes the old GUI hash in the original package manifests.
Keep this update's checksum file with the installation; original engine binaries
are unchanged.
The UI regression test runs the actual .NET Framework serializer and WPF window
on Windows. It first reproduces the history failure with the released source,
then checks the fix, exact amount text, receipt transitions and mining controls.
Any included activity preview is a labelled test fixture, not live account data.
