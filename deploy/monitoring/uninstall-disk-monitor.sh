#!/usr/bin/env bash
# Removes the PoCol founder disk monitor's own files only: its three units and /opt/pocol-disk-monitor. With --purge it
# also removes /etc/pocol-disk-monitor (the owner's monitor.conf and secret.env) and /var/lib/pocol-disk-monitor (its
# measurements). It never touches PoCol's install, data, unit or the firewall.
#
# Usage:  sudo /opt/pocol-disk-monitor/uninstall-disk-monitor.sh [--purge]
# POCOL_MONITOR_ROOT is for the tests only.
set -euo pipefail

if [ "$(id -u)" != 0 ]; then
  echo 'Run with sudo.' >&2
  exit 1
fi
purge=no
case "${1:-}" in
  '') ;;
  --purge) purge=yes ;;
  *) echo 'Usage: uninstall-disk-monitor.sh [--purge]' >&2; exit 2 ;;
esac

root="${POCOL_MONITOR_ROOT:-}"
units="$root/etc/systemd/system"

systemctl disable --now pocol-disk-monitor.timer || true
systemctl stop pocol-disk-monitor.service pocol-disk-monitor-failure.service || true
for unit in pocol-disk-monitor.service pocol-disk-monitor.timer pocol-disk-monitor-failure.service; do
  rm -f -- "$units/$unit"
done
systemctl daemon-reload
systemctl reset-failed pocol-disk-monitor.service pocol-disk-monitor-failure.service 2>/dev/null || true
rm -rf -- "$root/opt/pocol-disk-monitor"

if [ "$purge" = yes ]; then
  rm -rf -- "$root/etc/pocol-disk-monitor" "$root/var/lib/pocol-disk-monitor"
  echo 'Removed the disk monitor, its configuration, its secrets and its measurements.'
else
  echo 'Removed the disk monitor. Kept /etc/pocol-disk-monitor and /var/lib/pocol-disk-monitor (use --purge to remove them).'
fi
