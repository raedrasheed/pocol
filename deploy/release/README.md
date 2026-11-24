# Release signing

Owner decision 2026-11-16: every downloadable release file, and the release's `SHA256SUMS`, is signed with the
owner's ssh-ed25519 key through `ssh-keygen -Y sign`, in the fixed namespace `pocol-release`, under the principal
`release@pocol.net`. The owner holds the private key offline; it never enters this repository, a CI runner, a server
or a chat. A SHA-256 match alone is **integrity only**: it does not authenticate the publisher. A release is called
verified only when its signatures verify.

| File | Who runs it | What it does |
|---|---|---|
| `allowed_signers` | committed | The one release public key, printable ASCII and LF only: the owner's key, committed 2026-11-18 (fingerprint below) |
| `assemble_release.py` | owner | Copies the downloadable files into one directory and writes its `SHA256SUMS` (allowlisted names only) |
| `Sign-PoColRelease.ps1` / `sign-release.sh` | owner | Signs each listed file and `SHA256SUMS` (`FILE.sig`), then verifies the result |
| `verify_release.py` / `Verify-PoColRelease.ps1` | anyone | Verifies a whole release directory; prints `VERIFIED` only when every signature verifies |
| `test_release_signing.py`, `test_verify_release.ps1` | CI | Throwaway keys in temporary directories; never a committed private key |
| `double_build.py`, `test_double_build.py` | CI, owner, anyone | Builds the Linux native binaries and package twice and fails unless every byte is the same (*Reproducible Linux build* below) |
| `compare_build_identity.py`, `test_compare_build_identity.py` | CI, anyone | Compares the consensus identity of the Linux, Windows and Android builds (*Cross-platform consensus identity* below) |

The release set (`assemble_release.py`): `PoCol-Mainnet-V-linux-x86_64.tar.gz`, `PoCol-Mainnet-V-windows-x86_64.zip`,
`PoCol-Mainnet-V-arm64.apk` (already signed with the Android release key: an `-unsigned` APK, or one without a v2 or
v3 APK Signing Block at its place just before the ZIP central directory, is refused), `PoCol-Studio-IDE-Viewer-V.zip`
(the release build only) and, when it is published, `network.pocol.json` (the mainnet invitation only: the checks of
`package.py --network`, so a rehearsal, placeholder, retired or spec-1 invitation is refused). V is
`deploy/standalone/package.py`'s version, and the extension's `package.json` version must be the same V (owner
decision 2026-11-16): until the identity switch sets it, the assembler refuses every release, so the genesis
ceremony runs on unsigned CI artifacts (`deploy/mainnet/README.md`, step 1) and the first signed release is built
from the switched tree (step 7). The
per-package `SHA256SUMS` inside the native packages and the extension's `.sha256` stay as integrity checks; the signed
`SHA256SUMS` is the one this directory writes.

## The release key

The owner's release key, committed in `allowed_signers` on 2026-11-18. Every signed release must verify with this
fingerprint, the one `ssh-keygen -Y verify` and the directory verifiers print:

```
256 SHA256:r/IMxkMr+wv3OyAbiX3XYhSV4pqy3Lv5/z/VV06DJyc release@pocol.net (ED25519)
```

## Owner procedure


1. **Make the key, once, offline** (a machine that is not a server and holds no checkout of this repository):

   ```sh
   ssh-keygen -t ed25519 -C release@pocol.net -f <offline path>/pocol-release
   ```

   Back up `<offline path>/pocol-release` and its passphrase offline. Losing it means a key rotation.
2. **Commit only the public line** into `allowed_signers` by a reviewed pull request, replacing the PLACEHOLDER line:
   `release@pocol.net namespaces="pocol-release" ssh-ed25519 AAAA...` (the first two fields of
   `pocol-release.pub`). Publish its fingerprint, `ssh-keygen -lf <offline path>/pocol-release.pub`, on pocol.net and
   here. Done for the current key on 2026-11-18 (*The release key* above); pocol.net publishes it with the first
   signed release.
3. **Download the CI artifacts** of the release commit, a commit after the identity switch (five-device-build:
   Linux, Windows, Android; the extension package). The Linux package is the artifact
   `PoCol-Mainnet-V-Linux-reproducible-COMMIT` of the *Native double build compare* job (the package two builds
   agreed on byte for byte), not the `native` job's Linux package: that one is built in the test tree without the
   double build's fixed build path, so its bytes differ. Take the artifacts only from a five-device-build run of
   that commit in which every job succeeded. (The compare job withholds the Linux package unless the `native` jobs,
   with their vendor, crypto-graph, consensus-lock and test checks, passed; the green run as a whole is still yours
   to check.)
   Packages built before the switch, such as the ceremony's, are never signed: the switch changes the native
   binaries too.
4. **Sign the APK** with the Android release key (`apksigner`, `prototypes/pocol-android/README.md`); rename it
   `PoCol-Mainnet-V-arm64.apk`.
5. **Assemble:** `python3 deploy/release/assemble_release.py --out RELEASE_DIR FILE...`
6. **Sign** (Windows ceremony PC, or `bash deploy/release/sign-release.sh --key <offline path>/pocol-release RELEASE_DIR`
   on Linux or macOS: bash 3.2 or later, OpenSSH 8.2 or later and python3, no GNU tools):

   ```powershell
   powershell -NoProfile -ExecutionPolicy Bypass -File deploy\release\Sign-PoColRelease.ps1 -Key <offline path>\pocol-release -Directory RELEASE_DIR
   ```

   It refuses a key inside the repository, a key that is not the committed one, an existing `.sig`, a
   `SHA256SUMS` that is not strict (names out of sorted order, listed twice, or not release file names) or does not
   match the files, and any file it does not list (hidden ones such as `.DS_Store` included), before it writes
   anything.
7. **Verify** the result: `python3 deploy/release/verify_release.py RELEASE_DIR` (or `Verify-PoColRelease.ps1`).
   It must print `VERIFIED` with the key fingerprint you published.
8. **Upload** every file with its `.sig`, `SHA256SUMS` and `SHA256SUMS.sig`. Never upload a release that does not
   verify.

Rotation: replace the one key line by a reviewed pull request and publish the new fingerprint. An older release's
signatures stay checkable against the `allowed_signers` of that release's tagged commit.

## User procedure

1. **The key.** Download `allowed_signers` from this repository (or pocol.net). It must hold exactly one key line,
   and that key's fingerprint must be the one published on pocol.net:

   ```sh
   LC_ALL=C grep -Ecv '^[[:blank:]]*(#|$)' allowed_signers
   grep -v '^ *#' allowed_signers | cut -d' ' -f3,4 | ssh-keygen -lf -
   ```

   The first must print `1` (`LC_ALL=C` keeps `[[:blank:]]` to the space and tab that ssh-keygen skips; in a UTF-8
   locale it also matches Unicode spaces, which would hide a key line): ssh-keygen accepts every key line in the file, and the second command silently skips a
   line it cannot cut (an added `release@pocol.net ssh-ed25519 KEY` line, or one spaced with tabs), so one printed
   fingerprint alone does not show the file holds one key. (`ssh-keygen -lf allowed_signers` does not work: it is not
   a public key file.) These commands only check the downloaded file. The check that decides is the fingerprint of
   the key that signed: the one on the `Good ... with ED25519 key SHA256:...` line of step 2, or in the directory
   verifiers' `VERIFIED` line (they refuse a file with a second key line), and it must be the published one.
2. **One download.** A `FILE.sig` signs only the file's bytes, not its name or version: an older signed file served
   under a newer name still has a good `FILE.sig`. So check the release's signed `SHA256SUMS` and the file's own line in
   it, with OpenSSH 8.2 or later:

   ```sh
   ssh-keygen -Y verify -f allowed_signers -I release@pocol.net -n pocol-release -s SHA256SUMS.sig < SHA256SUMS
   awk -v f=FILE '$2 == f' SHA256SUMS | sha256sum --check
   ```

   The first prints `Good "pocol-release" signature for release@pocol.net with ED25519 key SHA256:...`: that
   fingerprint must be the published one. The second must print `FILE: OK` (macOS: `shasum -a 256 --check`); it fails
   when `SHA256SUMS` does not list that name. The file names carry the version, so `SHA256SUMS` names the release.
   On Windows, compare `(Get-FileHash FILE -Algorithm SHA256).Hash` with the file's line, or verify the directory.
3. **The whole release directory:** `python3 verify_release.py DIR` or, on Windows,
   `powershell -NoProfile -ExecutionPolicy Bypass -File Verify-PoColRelease.ps1 -Directory DIR`. It checks
   `SHA256SUMS.sig`, every file's hash and name, and every `FILE.sig`. The Windows 10 built-in OpenSSH (7.7) has no
   `-Y`; use Git for Windows' `usr\bin\ssh-keygen.exe` or a current Win32-OpenSSH (pass it with `-SshKeygen`).

A checksum that matches but has no valid signature is not a verified release.

## Reproducible Linux build (double build)

`double_build.py` builds the Linux release binaries (`pocol`, `pocold`, `pocol-key`, `pocol-sign`, `pocol-site`) and
the Linux package (`package.py --platform Linux`) twice and compares the SHA-256 of every binary, of the
`.tar.gz`, of the uncompressed tar and of every archive member (with its mode, mtime and owner). Any difference
fails, with a summary: each differing member's sizes and first differing byte offset, and a diffoscope report when
diffoscope is installed.

```sh
python3 deploy/release/double_build.py            # two fresh copies of HEAD, one after the other (local)
python3 deploy/release/double_build.py --worktree # the same for the working tree, uncommitted files included
```

The two legs differ in checkout path (`a/src` and `b/other/deeper/checkout-b`), umask (0022 and 0077), `TZ` (UTC
and Pacific/Kiritimati), `LC_ALL`, source mtimes (leg b sets them all to 2001-02-03), cargo home path (leg b: another
directory; locally over the shared registry, in CI with the registry fetched again; `Cargo.lock` checksums pin the
bytes either way) and build time. The recipe fixes the rest:

- toolchain 1.99.1, `cargo build --locked --release --bins` with `POCOL_RELEASE=1` (a tree whose consensus sources are
  not pinned does not build), the release profile as committed;
- the build path cargo is given, `/tmp/pocol-native-build/src` (a symlink to the checkout; the checkout itself can be
  anywhere), held under an exclusive lock (`/tmp/pocol-native-build/lock`): a second build on the same machine is
  refused while one runs, and a build removes only its own link. The directory must belong to the builder and not be
  writable by others. Cargo derives each crate's `-C metadata`, and so every mangled symbol, from the absolute path of the
  path dependencies outside `prototypes/pocold` (`lp4-execution`, the `third-party/evm-crypto` patches): in a first
  local double build from two paths without it, `pocol-lp4-execution` got another metadata hash, and the binaries
  other symbol names. The manifests are consensus-pinned, so the recipe fixes the path instead;
- `--remap-path-prefix` for rustc and `-ffile-prefix-map` for the C and assembly sources the `cc` crate compiles
  (blst, c-kzg, secp256k1-sys): the build path, the real checkout path, the cargo home and the rustup home become
  `/pocol/src`, `/pocol/cargo-home` and `/pocol/rustup`, so no builder path is in the binaries;
- cargo runs in the checkout (as the `native` job does) and reads no configuration the commit does not carry: a
  `.cargo/config.toml` or `.cargo/config` in any directory above the checkout (such as `/tmp/.cargo`, which any local
  user can create, and which could set a rustc wrapper, a linker or `[env]`), or an untracked one in the checkout, is
  refused. The cargo home's and the user's own `~/.cargo` configuration are read as in any build of that user;
- build paths (checkout, cargo home, rustup home) must be absolute and contain no `=` or whitespace (they go into
  the compiler flags);
- `SOURCE_DATE_EPOCH` = the commit's committer time; `package.py` stamps every archive entry with it (without the
  variable it reads the same commit time), writes entries in name order as `0:0` without owner names, mode 0755 for
  executables and 0644 otherwise, ustar headers without pax records, and a gzip header without a name or time.

The recipe changes no source, manifest or consensus file: `build-info` (`compatibilitySha256`,
`consensusSourcesSha256`, `consensusPinned`) is computed from the source bytes as before and is the same in both
legs, as the compared archive shows. The compressed `.tar.gz` is byte-identical given the same zlib (`env.txt`
records the tool versions of each leg); the uncompressed tar's hash does not depend on it.

CI (`five-device-build.yml`) runs the two legs as parallel jobs on ubuntu-24.04 (*Native double build (a)* and
*(b)*, `double_build.py build --leg a|b`), then *Native double build compare* (`double_build.py compare`; on a
difference it installs diffoscope and prints its report). The comparison also re-hashes each leg's archive and
requires its `dist/` to hold exactly the archive both legs listed and that archive's `SHA256SUMS` line, so the
uploaded files are the ones compared. The
agreed package is uploaded as `PoCol-Mainnet-V-Linux-reproducible-COMMIT` only when the `native` jobs passed too.

Evidence so far: local double builds (2026-11-19, `--worktree`, both legs on one machine over a shared cargo
registry, the last one with the recipe as committed here) produced identical binaries and an identical package. The CI
jobs, in which leg b fetches the registry again under another cargo home, ran and passed on main on 2026-11-19,
compare job included: five-device-build runs 298 (741d5d9) and 300 (216ce2a)
([FINDINGS.md](../../development/public-network/FINDINGS.md) F-51).

Scope: the Linux x86_64 binaries and package only. The Windows package (and its WPF wallet) and the APK are not
covered by this check. Two builds on the same runner image agreeing is not an independent rebuild: gate I1 still
requires someone other than the release builder to rebuild the signed release and obtain the same bytes
(`development/public-network/PLAN.md`).

## Cross-platform consensus identity

`compare_build_identity.py` checks device compatibility (OWNER-DECISIONS-2026-11-20 D6 E). On Linux and Windows,
`genesis` fills `deploy/mainnet/v3-profile.json` with fixed throwaway values on its own chain (2656089850): genesis
time 1800000000, hash rate 1000000 H/s, and founder ids and reward addresses that are SHA-256 digests of public labels,
so no one holds their keys and the genesis can never be mined or launched. It runs `pocold init-testnet`, `pocol
network` and `pocol inspect` on it and keeps devnet.json, both summaries and `pocol build-info`. `compare` then requires:

- the same genesis hash on every platform, admitted in scope public-network as the network profile on a network node;
- the same value from every build-info (Linux, Windows and the Android engine's raw build.rs output) for each field the
  consensus identity or the handshake reads: `consensusRulesVersion`, `protocolVersion`, `protocolVersionMin`,
  `maxForkVersion`, `consensusPinned` (true), `consensusSourcesSha256` and `features` (empty);
- the same build digest (`compatibilitySha256`) and `version` everywhere;
- one consensus identity for every build on that genesis, computed with `deploy/public-testnet/pocol_identity.py`, and
  equal to the identity the Linux and Windows binaries computed themselves.

The Android engine cannot run on the CI host, so its identity is computed from its build identity, not observed. Its
mining path is exercised natively on the host instead (`pocold` lib test
`the_android_engine_mines_a_public_network_genesis_natively_on_the_host`). Neither is a device or emulator run.

## Wiring

- `.github/workflows/five-device-build.yml` runs `test_release_signing.py` and `test_double_build.py` on Linux, checks
  that the unsigned CI package does **not** verify (`NOT VERIFIED`), runs `test_verify_release.ps1` on Windows, and
  runs the native double build (`native-reproducible`, `native-reproducible-compare`). Each `native` job runs
  `compare_build_identity.py genesis` and the `android` job uploads its build-info; *Linux, Windows and Android consensus
  identity compare* (`consensus-identity-compare`) runs `compare_build_identity.py compare` on the three.
- `.github/workflows/full-nonce-upgrade.yml`: after the identity switch, the freshly packaged extension must not verify
  until the owner signs it.
- `prototypes/pocol-extension/scripts/package-release.py` prints `UNSIGNED: ...`; it never signs.
- `deploy/standalone/package.py` writes the per-package `SHA256SUMS` (integrity); release-info carries no signing
  status, because it is stamped before signing.
