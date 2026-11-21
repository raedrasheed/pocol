#!/usr/bin/env bash
# Installs the PoCol founder disk monitor (OWNER-DECISIONS-2026-11-21 D7) on one server. Idempotent: a second run
# upgrades the script and the units and keeps the owner's monitor.conf and secret.env. It installs only its own files
# (/opt/pocol-disk-monitor, /etc/pocol-disk-monitor, /var/lib/pocol-disk-monitor and three units) and never touches
# PoCol's install, data, unit or the firewall.
#
# Usage (as the admin user, from the folder that holds this script):  sudo ./install-disk-monitor.sh
# POCOL_MONITOR_ROOT is for the tests only: it installs under that directory instead of /.
set -euo pipefail

if [ "$(id -u)" != 0 ]; then
  echo 'Run with sudo: sudo ./install-disk-monitor.sh' >&2
  exit 1
fi

src="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
root="${POCOL_MONITOR_ROOT:-}"
opt="$root/opt/pocol-disk-monitor"
etc="$root/etc/pocol-disk-monitor"
state="$root/var/lib/pocol-disk-monitor"
units="$root/etc/systemd/system"

for file in pocol_disk_monitor.py monitor.conf.example secret.env.example README.md uninstall-disk-monitor.sh \
    systemd/pocol-disk-monitor.service systemd/pocol-disk-monitor.timer systemd/pocol-disk-monitor-failure.service; do
  if [ ! -f "$src/$file" ]; then
    echo "Missing $file next to this script; run it from deploy/monitoring." >&2
    exit 1
  fi
done

if ! command -v python3 >/dev/null 2>&1; then
  echo 'python3 is required (Ubuntu 24.04 ships it as /usr/bin/python3).' >&2
  exit 1
fi
if ! python3 -c 'import sys; sys.exit(0 if sys.version_info >= (3, 10) else 1)'; then
  echo 'python3 3.10 or later is required.' >&2
  exit 1
fi
python3 -I -B "$src/pocol_disk_monitor.py" --help >/dev/null

install -d -m 0755 "$opt" "$etc" "$units"
install -d -m 0700 "$state"
install -m 0755 "$src/pocol_disk_monitor.py" "$opt/pocol_disk_monitor.py"
install -m 0755 "$src/uninstall-disk-monitor.sh" "$opt/uninstall-disk-monitor.sh"
install -m 0644 "$src/README.md" "$opt/README.md"
install -m 0644 "$src/monitor.conf.example" "$etc/monitor.conf.example"
install -m 0644 "$src/secret.env.example" "$etc/secret.env.example"

# The owner's files are created once and never overwritten.
if [ -e "$etc/monitor.conf" ]; then
  echo "Kept the existing $etc/monitor.conf"
else
  install -m 0644 "$src/monitor.conf.example" "$etc/monitor.conf"
  echo "Created $etc/monitor.conf from the example: set SERVER_NAME and SERVER_IP."
fi
if [ -e "$etc/secret.env" ]; then
  echo "Kept the existing $etc/secret.env"
else
  install -m 0600 "$src/secret.env.example" "$etc/secret.env"
  echo "Created $etc/secret.env (root only, 0600) with placeholders: write the channel secrets into it."
fi

for unit in pocol-disk-monitor.service pocol-disk-monitor.timer pocol-disk-monitor-failure.service; do
  install -m 0644 "$src/systemd/$unit" "$units/$unit"
done

systemctl daemon-reload
systemctl enable --now pocol-disk-monitor.timer

cat <<'NEXT'
Installed the PoCol disk monitor (it only reads and notifies).
Next steps on this server:
  1. sudoedit /etc/pocol-disk-monitor/monitor.conf    (SERVER_NAME, SERVER_IP, channels, SMTP host)
  2. sudoedit /etc/pocol-disk-monitor/secret.env      (Telegram bot token and chat id, SMTP password, heartbeat URL)
  3. sudo python3 -I -B /opt/pocol-disk-monitor/pocol_disk_monitor.py --test-alert
     and confirm that the test notification arrived on every channel.
  4. sudo python3 -I -B /opt/pocol-disk-monitor/pocol_disk_monitor.py --simulate-usage 96
  5. systemctl list-timers pocol-disk-monitor.timer
See /opt/pocol-disk-monitor/README.md, "Acceptance test".
NEXT
