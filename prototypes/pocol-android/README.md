# PoCol Wallet for Android

A native wallet: the PoCol node (Rust, `prototypes/pocold`, loaded as `libpocold.so`) runs in a foreground
service, and the Java interface talks to it through a small JNI bridge (`app/src/main/cpp/bridge.c`,
`pocold/src/mobile.rs`). The private key stays in Rust. Java only ever sees public data and, for a key
backup, the backup's ciphertext.

Build: `bash deploy/standalone/build-android.sh` (SDK 35, NDK 27.2.12479018, JDK 17, Gradle 8.11.1).
The release APK is unsigned. The release operator signs it locally with the release keystore (kept outside the
repository and CI artifacts), checks the signer with `apksigner verify --print-certs` and the application ID and
versionCode with `aapt dump badging`, and recomputes the checksums before publishing it as installable.

## Wallet only or miner

**Settings → Device role** chooses what this phone does (owner decision of 2026-11-16). Both roles run the local node,
which validates every block, so the balance is always verified on the device.

- **Wallet only** (the default, also for an app updated from a release without the choice): the phone never mines.
  There is no Mining tab and no **Manage mining** button, and the engine itself refuses mining, registration, top-up and
  rejoin in this role (`set_mode` and the wallet-only checks in `pocold/src/wallet.rs`).
- **Wallet and miner**: the Mining tab appears. Mining still starts off and runs only when you turn it on there, with an
  eligible registered account.

Switching back to wallet only stops mining at once, even if the choice cannot be saved (the app then shows the error
and may reopen as wallet and miner, with mining off). A registered account that is wallet only earns no mining rewards
and, once every seat is taken, can be replaced as an inactive member: the wallet card and the Device role card say so. A
switch while a confirmed rejoin is under way is refused until it
finishes or stops (it would lose the reactivation). The engine keeps the choice in its home (`app-mode.json`), so it
survives a restart and a key restore; the app keeps only a display copy. The buttons work while the wallet is connected.

## Mining actions

In the wallet-and-miner role, the Mining tab shows **Review mining registration**, and, when the engine offers them, **Rejoin mining** and **Top up
bond**. Each opens a review from the engine (`quote_*` commands of `pocold/src/wallet.rs`); nothing is signed before
**Confirm & sign**. After a confirmed rejoin the engine loop (`pocold/src/mobile.rs`, every 2 s as on the desktop) signs
`reactivate()` when the window opens; its steps appear as notices (the status's `event`, apart from the command answers
in `response`). On the v3 network the card shows the bond still to earn from mining and the reactivation clock; when
every seat is taken it says whether a registration replaces an inactive member (the review then names the member), or,
for a member out of the full set, that it waits for its bond refund and registers again
(`deploy/standalone/README.md`, "On the v3 network"). The texts are the Windows wallet's (`WalletText.java`, checked by
`deploy/standalone/test_android_jni.py`).

## Key backup and restore

**Settings → Key backup** has two actions:

- **Back up key to encrypted file.** Choose a passphrase of at least 12 characters and type it twice. Passphrases
  that guessing tools try first are refused: fewer than 5 different characters, a repeated part
  (`abcabcabcabc`), a sequence (`abcdefghijkl`, `345678901234`) and passwords built on a common word or keyboard
  run (`Password2024!`, `P@ssw0rd1234`, `qwertyuiop123`). The node encrypts the key and opens the result again to
  check it. Android's file picker then creates the file you choose (for example on a USB drive or in a cloud
  folder); the wallet writes the encrypted bytes, reads the file back and compares every byte before it reports
  the backup as saved. If anything fails, or the app was restarted while the picker was open, the wallet deletes
  the file the picker created (or asks you to, when the storage does not allow it), so an empty or partial file
  never looks like a backup. The plaintext key is never written to disk, shown on screen or put on the clipboard.
- **Restore key from backup.** Pick a backup file and enter its passphrase. If restoring would replace the
  account this wallet uses now, the wallet shows both addresses and asks you to confirm. The replaced key is not
  deleted: it stays in the app's private folder as `identity.pocol-key.replaced-<address>`.

Both actions need the wallet disconnected. The passphrase dialogs set `FLAG_SECURE`, so Android blocks
screenshots, screen recording and the recents thumbnail while they are open. Passphrase fields turn off
autofill, suggestions and keyboard learning. The wallet copies the passphrase out of the field into `char[]`/`byte[]`
buffers that are wiped after use (never a `String`), and overwrites the field's text before clearing it. This is
best effort: the keyboard app and the text field's earlier buffers can keep copies the wallet cannot reach, so
choose a passphrase you use nowhere else. A dialog still open when Android destroys the screen (for example on
rotation) is closed and its fields are wiped.

Android backup and device-to-device transfer copy nothing of this app: `allowBackup="false"` and
`fullBackupContent="false"` (Android 8 to 11), and `dataExtractionRules` (`res/xml/data_extraction_rules.xml`)
excludes every domain from both cloud backup and device transfer on Android 12 and later. Device transfer would
otherwise copy the raw key file, and the same key on two phones can lose a mining bond. Use the encrypted backup to
move a wallet.

Keep the backup file and its passphrase in different places. Anyone who has both controls the account. Nobody can
open the file without the passphrase, and there is no reset.

### File format

The format is defined once, in `prototypes/pocold/src/keyfile/backup.rs`. The Windows wallet and
`pocol export-key` / `pocol import-key` use the same format, so a backup made on one platform restores on the
other. A file is exactly 98 bytes:

| bytes | field |
|---|---|
| 16 | magic `POCOL-KEY-BACKUP` |
| 1 | format version (1) |
| 1 | KDF id (1 = PBKDF2-HMAC-SHA256) |
| 4 | iterations, big-endian (600,000; files outside 600,000..10,000,000 are refused before any work) |
| 16 | salt |
| 12 | AES-GCM nonce |
| 32 | AES-256-GCM ciphertext of the 32-byte secp256k1 key |
| 16 | GCM tag (the first 50 bytes are the additional data) |

The passphrase is the UTF-8 bytes of exactly what was typed, with no Unicode normalization. If you use
non-ASCII characters, restore with the same keyboard or input method you used to make the backup.

## Application id and signing

The app is `org.pocol.wallet`, the only application id that is built and packaged (`app/build.gradle`). The Java
package and the `namespace` stay `org.pocol.node`, because the JNI symbols that `bridge.c` exports
(`Java_org_pocol_node_*`) are named after it; only the installed id differs.

The release APK is unsigned until the owner signs it with the Android release key, which is kept outside the
repository and CI. Android updates an installed app only with the signer it was installed with, so every later release
of `org.pocol.wallet` must be signed with that same key; without it no installed copy can be updated again. An app
with another id (such as `org.pocol.node` from an earlier release) is a different app: this release neither updates
nor reads it.

An encrypted backup is the recovery path. Restoring a backup on a new phone, or in a reinstalled app, gives back the
same account.
