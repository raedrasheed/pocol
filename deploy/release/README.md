# Release signing

Owner decision 2026-11-16: every downloadable release file, and the release's `SHA256SUMS`, is signed with the
owner's ssh-ed25519 key through `ssh-keygen -Y sign`, in the fixed namespace `pocol-release`, under the principal
`release@pocol.net`. The owner holds the private key offline; it never enters this repository, a CI runner, a server
or a chat. A SHA-256 match alone is **integrity only**: it does not authenticate the publisher. A release is called
verified only when its signatures verify.

| File | Who runs it | What it does |
|---|---|---|
| `allowed_signers` | committed | The one release public key (today a marked **PLACEHOLDER**: nothing verifies until the owner commits the key) |
| `assemble_release.py` | owner | Copies the downloadable files into one directory and writes its `SHA256SUMS` (allowlisted names only) |
| `Sign-PoColRelease.ps1` / `sign-release.sh` | owner | Signs each listed file and `SHA256SUMS` (`FILE.sig`), then verifies the result |
| `verify_release.py` / `Verify-PoColRelease.ps1` | anyone | Verifies a whole release directory; prints `VERIFIED` only when every signature verifies |
| `test_release_signing.py`, `test_verify_release.ps1` | CI | Throwaway keys in temporary directories; never a committed private key |

The release set (`assemble_release.py`): `PoCol-Mainnet-V-linux-x86_64.tar.gz`, `PoCol-Mainnet-V-windows-x86_64.zip`,
`PoCol-Mainnet-V-arm64.apk` (already signed with the Android release key: an `-unsigned` APK or one without an APK
Signing Block is refused), `PoCol-Studio-IDE-Viewer-E.zip` (the release build only) and, when it is published,
`network.pocol.json`. V is `deploy/standalone/package.py`'s version, E the extension's `package.json` version. The
per-package `SHA256SUMS` inside the native packages and the extension's `.sha256` stay as integrity checks; the signed
`SHA256SUMS` is the one this directory writes.

## Owner procedure

1. **Make the key, once, offline** (a machine that is not a server and holds no checkout of this repository):

   ```sh
   ssh-keygen -t ed25519 -C release@pocol.net -f <offline path>/pocol-release
   ```

   Back up `<offline path>/pocol-release` and its passphrase offline. Losing it means a key rotation.
2. **Commit only the public line** into `allowed_signers` by a reviewed pull request, replacing the PLACEHOLDER line:
   `release@pocol.net namespaces="pocol-release" ssh-ed25519 AAAA...` (the first two fields of
   `pocol-release.pub`). Publish its fingerprint, `ssh-keygen -lf <offline path>/pocol-release.pub`, on pocol.net and
   here.
3. **Download the CI artifacts** of the release commit (five-device-build: Linux, Windows, Android; the extension
   package after the identity switch).
4. **Sign the APK** with the Android release key (`apksigner`, `prototypes/pocol-android/README.md`); rename it
   `PoCol-Mainnet-V-arm64.apk`.
5. **Assemble:** `python3 deploy/release/assemble_release.py --out RELEASE_DIR FILE...`
6. **Sign** (Windows ceremony PC, or `sign-release.sh` on Linux/macOS):

   ```powershell
   powershell -NoProfile -ExecutionPolicy Bypass -File deploy\release\Sign-PoColRelease.ps1 -Key <offline path>\pocol-release -Directory RELEASE_DIR
   ```

   It refuses a key inside the repository, a key that is not the committed one, an existing `.sig`, and a
   `SHA256SUMS` that does not match the files, before it writes anything.
7. **Verify** the result: `python3 deploy/release/verify_release.py RELEASE_DIR` (or `Verify-PoColRelease.ps1`).
   It must print `VERIFIED`.
8. **Upload** every file with its `.sig`, `SHA256SUMS` and `SHA256SUMS.sig`. Never upload a release that does not
   verify.

Rotation: replace the one key line by a reviewed pull request and publish the new fingerprint. An older release's
signatures stay checkable against the `allowed_signers` of that release's tagged commit.

## User procedure

Download `allowed_signers` from this repository (or pocol.net), check its fingerprint against the published one,
and verify one file with OpenSSH 8.2 or later:

```sh
ssh-keygen -Y verify -f allowed_signers -I release@pocol.net -n pocol-release -s FILE.sig < FILE
```

It prints `Good "pocol-release" signature for release@pocol.net`. For the whole release directory:
`python3 verify_release.py DIR` or, on Windows,
`powershell -NoProfile -ExecutionPolicy Bypass -File Verify-PoColRelease.ps1 -Directory DIR`. The Windows 10 built-in
OpenSSH (7.7) has no `-Y`; use Git for Windows' `usr\bin\ssh-keygen.exe` or a current Win32-OpenSSH (pass it with
`-SshKeygen`). A checksum that matches but has no valid signature is not a verified release.

## Wiring

- `.github/workflows/five-device-build.yml` runs `test_release_signing.py` on Linux, checks that the unsigned CI
  package does **not** verify (`NOT VERIFIED`), and runs `test_verify_release.ps1` on Windows.
- `.github/workflows/full-nonce-upgrade.yml`: after the identity switch, the freshly packaged extension must not verify
  until the owner signs it.
- `prototypes/pocol-extension/scripts/package-release.py` prints `UNSIGNED: ...`; it never signs.
- `deploy/standalone/package.py` writes the per-package `SHA256SUMS` (integrity); release-info carries no signing
  status, because it is stamped before signing.
