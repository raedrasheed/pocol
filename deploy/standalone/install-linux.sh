#!/usr/bin/env bash
# Fresh installation only; keys are generated as the service user on this host.
set -euo pipefail
if [ "$(id -u)" != 0 ]; then echo 'Run with sudo.' >&2; exit 1; fi
source_dir="$(cd "$(dirname "$0")" && pwd)"
if [ -e /opt/pocol-standalone ] || [ -e /etc/systemd/system/pocol-standalone.service ]; then
  echo 'Existing installation detected. Refusing to overwrite it.' >&2; exit 1
fi
cd "$source_dir"
sha256sum --check SHA256SUMS
if getent passwd pocol-standalone >/dev/null; then echo 'Service account already exists; inspect before installing.' >&2; exit 1; fi
useradd --system --home-dir /var/lib/pocol-standalone --shell /usr/sbin/nologin pocol-standalone
install -d -m 0755 /opt/pocol-standalone
for name in pocol pocold pocol-key pocol-sign pocol-site; do install -m 0755 "$name" "/opt/pocol-standalone/$name"; done
install -d -m 0700 -o pocol-standalone -g pocol-standalone /var/lib/pocol-standalone
runuser -u pocol-standalone -- /opt/pocol-standalone/pocol identity --home /var/lib/pocol-standalone
# Only the public address is readable by the setup operator; the key stays private.
install -m 0644 /var/lib/pocol-standalone/identity.address /opt/pocol-standalone/identity.address
cat > /etc/systemd/system/pocol-standalone.service <<'UNIT'
[Unit]
Description=PoCol experimental standalone node
After=network-online.target
Wants=network-online.target
ConditionPathExists=/opt/pocol-standalone/network.pocol.json
StartLimitIntervalSec=300
StartLimitBurst=5

[Service]
Type=exec
User=pocol-standalone
Group=pocol-standalone
UMask=0077
StateDirectory=pocol-standalone
StateDirectoryMode=0700
ExecStart=/opt/pocol-standalone/pocol start --home /var/lib/pocol-standalone --listen 0.0.0.0:30300 --rpc 127.0.0.1:8545
Restart=on-failure
RestartSec=10
TimeoutStopSec=30
KillMode=control-group
NoNewPrivileges=yes
PrivateTmp=yes
ProtectHome=yes
ProtectSystem=strict
ReadWritePaths=/var/lib/pocol-standalone
RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX
TasksMax=256
LimitNOFILE=4096
# A bounded pilot on the owner's 4 GiB VPS, not a proof of worst-case capacity.
MemoryHigh=2560M
MemoryMax=3G
MemorySwapMax=0

[Install]
WantedBy=multi-user.target
UNIT
systemctl daemon-reload
systemctl enable pocol-standalone.service
echo 'Installed. Save the public address above. After installing the shared network file in /opt/pocol-standalone/, start pocol-standalone.service.'
