# PoCol Wallet for Android

A native wallet: the PoCol node (Rust, `prototypes/pocold`, loaded as `libpocold.so`) runs in a foreground
service, and the Java interface talks to it through a small JNI bridge (`app/src/main/cpp/bridge.c`,
`pocold/src/mobile.rs`). The private key stays in Rust. Java only ever sees public data and, for a key
backup, the backup's ciphertext.

Build: `bash deploy/standalone/build-android.sh` (SDK 35, NDK 27.2.12479018, JDK 17, Gradle 8.11.1).
The release APK is unsigned; see `deploy/standalone/RELEASE-UPGRADE.md` for signing.

## Key backup and restore

**Settings → Key backup** has two actions:

- **Back up key to encrypted file.** Choose a passphrase of at least 12 characters and type it twice. The node
  encrypts the key, opens the result again to check it, and hands the encrypted bytes to Android's file picker.
  You choose where to save it (for example a USB drive or a cloud folder). The plaintext key is never written to
  disk, shown on screen or put on the clipboard.
- **Restore key from backup.** Pick a backup file and enter its passphrase. If restoring would replace the
  account this wallet uses now, the wallet shows both addresses and asks you to confirm. The replaced key is not
  deleted: it stays in the app's private folder as `identity.pocol-key.replaced-<address>`.

Both actions need the wallet disconnected. The passphrase dialogs set `FLAG_SECURE`, so Android blocks
screenshots, screen recording and the recents thumbnail while they are open. Passphrase fields turn off
autofill and suggestions. The passphrase is kept in `char[]`/`byte[]` buffers that are wiped after use, never in
a `String`.

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

## Application id and moving to a new signing key

The installed 0.4.0 app is `org.pocol.node`. Android installs an update only when it is signed by the installed
app's signer, or by a key that signer authorized through APK signature scheme v3 rotation. The installed app
has no key export, so a build signed only with a new key cannot replace it, and uninstalling it deletes its key.
`development/decisions/V3-DESIGN.md` (V3-33) sets the order:

1. **If the key that signed the installed APK can be recovered** (for a debug-signed build, that is the debug
   keystore of the machine that built it), sign the backup-capable release with it, or with a v3 signing
   lineage from it to the new key. Users then update in place, keep their account, and can make a backup from
   then on. Check the installed signer first: `apksigner verify --print-certs` on the installed APK, or
   `adb shell dumpsys package org.pocol.node`.
2. **Otherwise**, ship the new-key build as a **separate app**. It installs beside the old one:

   ```sh
   gradle -p prototypes/pocol-android assembleRelease -PpocolApplicationId=org.pocol.wallet
   ```

   The default build is still `org.pocol.node`. Another id gets the launcher label *PoCol Wallet (new)*;
   `-PpocolAppLabel=...` overrides it. `deploy/standalone/package.py --android-application-id
   org.pocol.wallet` packages it as `PoCol-Mainnet-0.4.0-arm64-org.pocol.wallet-unsigned.apk`. CI builds both
   ids.

   Users then move their funds before the re-genesis block S:

   1. Install the new app (*PoCol Wallet (new)*) and leave the old *PoCol Wallet* installed.
   2. In the new app, import the network file. **Back up the new key now** (Settings → Key backup) and keep the
      file and passphrase safe.
   3. A registered mining identity deregisters first (V3-33). Send moves only the spendable balance, not a
      locked bond, and the Android app has no deregister action. Follow the cutover announcement for member
      identities, and allow time for the bond to come back before S.
   4. In the old app, use **Send PCOL** to send the balance to the new app's address (Receive → Copy address in
      the new app). Leave enough for the network fee, and check the address on the review screen before you
      confirm.
   5. Wait until the new app shows the funds as included with confirmations. Only then uninstall the old app,
      if you want to.

   The cutover waits until this has been announced for long enough. Transfers after block S are voided by the
   re-genesis, so they must be included before it.

From the new app on, an encrypted backup is the recovery path. Restoring a backup on a new phone, or in a
reinstalled app, gives back the same account.
