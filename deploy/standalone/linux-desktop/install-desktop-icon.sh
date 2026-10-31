#!/bin/sh
# Installs only the current user's menu entry and icons; never runs PoCol.
set -eu
package_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
case "$package_dir" in *'
'*) printf 'Package path must not contain newlines.\n' >&2; exit 1;; esac
case "${XDG_DATA_HOME:-}" in /*) data_dir=$XDG_DATA_HOME;; *) data_dir=$HOME/.local/share;; esac
[ -x "$package_dir/pocol" ] && [ -x "$package_dir/pocol-desktop" ] || { printf 'Keep this installer beside pocol and pocol-desktop.\n' >&2; exit 1; }
for size in 16 24 32 48 64 128 256 512; do
  install -d "$data_dir/icons/hicolor/${size}x${size}/apps"
  install -m 644 "$package_dir/pocol-$size.png" "$data_dir/icons/hicolor/${size}x${size}/apps/pocol.png"
done
install -d "$data_dir/applications"
escaped=$(printf '%s' "$package_dir/pocol-desktop" | sed 's/\\/\\\\/g; s/"/\\"/g; s/`/\\`/g; s/\$/\\$/g; s/%/%%/g')
{
  printf '[Desktop Entry]\nType=Application\nVersion=1.0\nName=PoCol Node\n'
  printf 'Comment=Local PoCol node and command-line tools\n'
  printf 'Exec="%s"\n' "$escaped"
  printf 'Icon=pocol\nTerminal=true\nCategories=Development;Network;\nStartupNotify=false\n'
} > "$data_dir/applications/pocol-node.desktop"
chmod 644 "$data_dir/applications/pocol-node.desktop"
if command -v gtk-update-icon-cache >/dev/null 2>&1; then gtk-update-icon-cache -f -t "$data_dir/icons/hicolor" >/dev/null 2>&1 || :; fi
if command -v update-desktop-database >/dev/null 2>&1; then update-desktop-database "$data_dir/applications" >/dev/null 2>&1 || :; fi
printf 'Installed PoCol Node menu icon for this user. Keep the package at: %s\n' "$package_dir"
