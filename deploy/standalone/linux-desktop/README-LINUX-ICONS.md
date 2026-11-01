# PoCol Mainnet · Linux 0.4.0 desktop shortcut

The supplied PoCol icon is included at 16–512 pixels. The Linux distribution is a command-line node/service; the **PoCol Mainnet Node** desktop-menu shortcut displays read-only package information. It does not introduce a graphical wallet.

Extract the complete tarball to a permanent folder and run `./install-desktop-icon.sh` as your ordinary desktop user. It installs only that user's icon files and menu entry. You can skip it on headless servers.

This 0.4.0 release includes updated engine binaries. Follow **RELEASE-UPGRADE.md** before updating an existing node. Do not run the fresh-install `install-linux.sh` again to update an existing server, regenerate a network, or erase `/var/lib/pocol-standalone`. Installing the desktop icon alone does not alter keys, service settings, network configuration or blockchain data.

`SHA256SUMS` covers every included file. `source-commit.txt`, `build-info.json` and `release-info.json` identify the complete package source and compatibility.
