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

## Identity and edits (`tools/site_identity.py`)

The sources of the explorer and the IDE are not available (owner answer Q1, 2026-11-14), so every change to their
bundles is an anchored edit made by `tools/site_identity.py`. `site-identity.json` declares the identity the snapshots
pin (chain ID, genesis hash and the GenesisSpec that proves both, the display name, the IDE's RPC URL, the explorer's
seed list and gateway host, and the source links). Each anchor is a literal text that must occur exactly its count of
times, and no copy of an identity value may sit outside the anchors, so a moved anchor fails instead of leaving an old
copy behind. Every run checks the tree it would write the same way (after `--edits` too), so the tool never writes a
tree that its own `--check` refuses.

```sh
python3 -B deploy/websites/tools/site_identity.py --check            # what CI runs (.github/workflows/websites.yml)
python3 -B deploy/websites/tools/site_identity.py --to NEW.json --gateway-host HOST --commit SHA   # the switch (W15, W16)
python3 -B deploy/websites/tools/site_identity.py --edits EDITS.json  # anchored bundle edits (W7)
```

A changed bundle is renamed `<prefix>-<first 12 hex digits of its SHA-256>.js` and `index.html` follows; `?v=` content
versions and every `SHA256SUMS.txt` are regenerated. `--check` also refuses a retired identity
(`deploy/mainnet/retired-networks.json`, active once the snapshots pin a network that is not retired), any
`chatgpt.site` origin, a download link whose version is not the extension's `package.json` version, a missing
`index.html` script or stylesheet, and any `config.php` (never read). A `--to` run without `--gateway-host` is refused
unless the explorer's current gateway host is one of the new seeds, and the repository's own snapshots must name an
https RPC URL (a loopback `--rpc-url` is only for a test copy given with `--root`). The explorer bundle was renamed to that scheme
(`assets/index-473b5bd9e7ec.js`, the same bytes as the deployed `assets/index-Cj8htKRO.js`), and the superseded IDE bundle
`assets/index-CJbrNXr_.js` (it differed from `index-extension041-02af63944c2b.js` only in its download link) was
deleted with its `SHA256SUMS.txt` line.

Not included, and must be preserved on the host:
- `explorer.pocol.net/api/config.php` (`.gitignore` ignores every `api/config.php`; CI fails if one is tracked, and
  `explorer.pocol.net/api/.htaccess` must keep its `FilesMatch "^(config|lib)\.php$"` deny rule, which a test checks
  because `php -S` ignores `.htaccess`);
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
