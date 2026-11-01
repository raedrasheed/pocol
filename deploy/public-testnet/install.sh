#!/usr/bin/env bash
# Fresh dedicated Ubuntu/systemd host. Never overwrite state, create keys or expose ports.
set -euo pipefail
if [[ $# != 2 || $EUID != 0 || ! $2 =~ ^[a-f0-9]{64}$ ]]; then
  echo 'Usage: sudo bash install.sh NODE_BUNDLE TRUSTED_MANIFEST_SHA256' >&2
  exit 1
fi
source_bundle=$(realpath -- "$1")
expected=$2
for path in /opt/pocol-testnet /var/lib/pocol-testnet /etc/systemd/system/pocol-testnet.service; do
  if [[ -e $path || -L $path ]]; then
    echo 'Existing installation or state: use the backup/upgrade runbook, never overwrite.' >&2
    exit 1
  fi
done
staging=$(mktemp -d /var/tmp/pocol-testnet-install.XXXXXXXX)
trap 'rm -rf -- "$staging"' EXIT
cp -a -- "$source_bundle" "$staging/bundle"
bundle=$staging/bundle
actual=$(sha256sum -- "$bundle/manifest.json")
if [[ ${actual%% *} != "$expected" ]]; then echo 'Manifest identity mismatch.' >&2; exit 1; fi
# This verifier is part of the reviewed installer; no uploaded code runs before all hashes match.
python3 -B - "$bundle" <<'PY'
import hashlib, json, stat, sys
from pathlib import Path
root = Path(sys.argv[1])
manifest = json.loads((root / 'manifest.json').read_text())
if manifest['schema'] != 'pocol-public-testnet-bundle-v1' or manifest['rehearsalOnly']:
    raise SystemExit('Not a server bundle')
entries = list(root.rglob('*'))
if root.is_symlink() or any(not (stat.S_ISDIR(p.lstat().st_mode) or stat.S_ISREG(p.lstat().st_mode)) for p in entries):
    raise SystemExit('Links and special files refused')
actual = {p.relative_to(root).as_posix(): p for p in entries if p.is_file()}
if set(actual) != set(manifest['sha256']) | {'manifest.json'}: raise SystemExit('File set mismatch')
for name, expected in manifest['sha256'].items():
    if name == 'manifest.json' or actual[name].stat().st_nlink != 1: raise SystemExit('Invalid entry')
    h = hashlib.sha256()
    with actual[name].open('rb') as stream:
        for data in iter(lambda: stream.read(1024 * 1024), b''): h.update(data)
    if h.hexdigest() != expected: raise SystemExit('Checksum mismatch: ' + name)
PY
chown -R root:root "$bundle"
# Everything in the verified bundle is PUBLIC. Normalize modes even when the
# preparer's umask was 077; secret credentials and state are outside this tree.
python3 -B - "$bundle" <<'PY'
from pathlib import Path
import sys
root = Path(sys.argv[1])
for path in [root, *root.rglob('*')]:
    path.chmod(0o755 if path.is_dir() or path.parent == root / 'bin' else 0o644)
PY
python3 -B "$bundle/node_operator.py" verify --bundle "$bundle" --expected-sha256 "$expected"
systemd-analyze verify "$bundle/pocol-testnet.service"
if ! id -u pocol-testnet >/dev/null 2>&1; then
  useradd --system --home-dir /var/lib/pocol-testnet --shell /usr/sbin/nologin pocol-testnet
fi
install -d -m 0755 /opt/pocol-testnet
cp -a -- "$bundle/." /opt/pocol-testnet/
install -d -m 0700 /etc/pocol-testnet
install -m 0644 "$bundle/pocol-testnet.service" /etc/systemd/system/pocol-testnet.service
systemctl daemon-reload
echo 'Installed only. Service, gateway and firewall remain inactive. Follow README.md before launch.'
