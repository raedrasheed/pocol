# Deployed website snapshots (2026-11-01)

These folders record the public document roots of `pocol.net`, `ide.pocol.net` and `explorer.pocol.net` as they were
deployed for the retired network, taken from `websites/` in the release asset `PoCol-Complete-Handoff-2026-11-01.zip`
([release `handoff-2026-11-01`](https://github.com/palcertcom/pocol/releases/tag/handoff-2026-11-01), SHA-256
`b9a3885a630bd7f32270488f741b68a5c923e3fc48c1478499ceb887c520d2f3`). They are not editable sources: the original IDE
TypeScript project and the main-site sources are not in this repository. `studio.pocol.net` is built from
`prototypes/pocol-studio/dist`, which was updated to the same deployed files. The sites are rebuilt for the new
network before its launch (development/decisions/OWNER-DECISIONS-2026-11-13-FRESH-NETWORK.md).

Removed for the new network (2026-11-13): the old network's upgrade notes (`NETWORK-UPDATE-*.txt`,
`README-MAINNET-UPDATE-AR.txt`), its invitation (`pocol.net/downloads/network.pocol.json`), the retained-download
manifest and the checksums of the superseded 0.3.x and 0.4.0 packages. Each `SHA256SUMS.txt` was regenerated for the
files that remain. Until the rebuild, pages that link the removed invitation or the old packages are stale.

Left out of Git and kept in the release asset:
- the download archives (`*.zip`): the old native packages and the extension packages;
- a partial upload left by an earlier transfer (a hidden `downloads/.PoCol-Mainnet-*-Linux.zip.*` file). It is not
  served (HTTP 404).

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
- **Downloads:** the old Windows and Linux packages and the 0.4.1 extension matched the release hashes.
- **Not checked:** file permissions on the host itself; there is no cPanel access from here.
- **Stale, unchanged by this snapshot:** `pocol.net/downloads/PoCol-Apps-SHA256SUMS.txt` is served live but is not in
  the package. It still lists old Android/testnet files.
