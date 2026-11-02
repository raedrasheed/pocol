# Deployed website snapshots (2026-11-01)

These folders record the public document roots of `pocol.net`, `ide.pocol.net` and `explorer.pocol.net`. Each one is
deployable output: the H15000 website packages merged with the corrected English-extension 0.4.1 patch, taken from
`websites/` in the release asset `PoCol-Complete-Handoff-2026-11-01.zip`
([release `handoff-2026-11-01`](https://github.com/palcertcom/pocol/releases/tag/handoff-2026-11-01), SHA-256
`b9a3885a630bd7f32270488f741b68a5c923e3fc48c1478499ceb887c520d2f3`). They are not editable sources: the original IDE
TypeScript project and the main-site sources are not in this repository. `studio.pocol.net` is built from
`prototypes/pocol-studio/dist`, which was updated to the same deployed files.

Left out of Git and kept in the release asset:
- the download archives (`*.zip`): the native 0.4.0 Windows/Linux packages and the extension packages;
- a partial upload left by an earlier transfer (`downloads/.PoCol-Mainnet-0.4.0-Linux.zip.JZTfh5`). It is not served
  (HTTP 404).

The `.sha256` and `SHA256SUMS.txt` files that are kept list the archive hashes.

Not included, and must be preserved on the host:
- `explorer.pocol.net/api/config.php`;
- every other hosting configuration and credential.

The PHP files read `config.php` from the host, and this snapshot never overwrites it.

## Live comparison, 2026-11-01 (from outside, with strict TLS)

- **Identical:** 171 of 175 files byte-for-byte; no HTTP 403 or other error. PHP and dotfiles were not compared,
  because a server executes or hides them.
- **The four that differ:**
  - `pocol.net/style.css` and `pocol.net/assets/pocol-icon.png`, unversioned URL: a cached older copy (`age` about
    46 h). The pages request `style.css?v=11f712c55ca8`, and that URL returns the packaged file.
  - `studio.pocol.net/assets/pocol-icon.png`, unversioned URL: the same cached older copy. No page references it.
  - `ide.pocol.net/SHA256SUMS.txt`: the live copy lacks the line for the superseded bundle `assets/index-CJbrNXr_.js`.
- **Downloads:** the Windows and Linux 0.4.0 packages and the 0.4.1 extension match the release hashes.
- **Not checked:** file permissions on the host itself; there is no cPanel access from here.
- **Stale, unchanged by this snapshot:** `pocol.net/downloads/PoCol-Apps-SHA256SUMS.txt` is served live but is not in
  the package. It still lists old Android/testnet files.
