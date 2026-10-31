# Linux icon update — 2026-10-31

The supplied PoCol icon is included at 16–512 pixels. The Linux distribution is the existing command-line node/service; this update adds a **PoCol Node** desktop-menu shortcut that displays read-only package information. It does not introduce a graphical wallet.

Extract the complete tarball to a permanent folder and run `./install-desktop-icon.sh` as your ordinary desktop user. It installs only that user's icon files and menu entry. You can skip it on headless servers.

Existing binaries are unchanged. Do not run the fresh-install `install-linux.sh` again to update an existing server, regenerate a network, or erase `/var/lib/pocol-standalone`. This icon release does not alter keys, service settings, network configuration or blockchain data.

`SHA256SUMS` covers every included file. The unchanged engine source remains the version in `source-commit.txt`; branding source provenance is in `BRANDING-SOURCE-COMMIT.txt`.
