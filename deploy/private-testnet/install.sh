#!/usr/bin/env bash
# Run on a dedicated test host, after its private network/firewall has been checked.
set -euo pipefail
if [[ $# != 2 || $EUID != 0 ]]; then
  echo 'Usage: sudo bash install.sh /absolute/path/to/node-bundle EXPECTED_MANIFEST_SHA256' >&2
  exit 1
fi
source_bundle=$(realpath -- "$1")
expected=$2
if [[ ! $expected =~ ^[a-f0-9]{64}$ ]]; then
  echo 'Expected SHA-256 must come from the trusted release.json.' >&2
  exit 1
fi
if [[ -e /opt/pocol-private || -e /var/lib/pocol-private || -e /etc/systemd/system/pocol-private.service ]]; then
  echo 'Existing installation/data found. Stop and follow the documented upgrade/backup procedure.' >&2
  exit 1
fi
# Verify a root-owned snapshot before executing any uploaded Python or installing units.
staging=$(mktemp -d /var/tmp/pocol-install.XXXXXXXX)
trap 'rm -rf -- "$staging"' EXIT
cp -a -- "$source_bundle" "$staging/bundle"
bundle=$staging/bundle
actual=$(sha256sum -- "$bundle/manifest.json")
if [[ ${actual%% *} != "$expected" ]]; then
  echo 'Manifest identity mismatch.' >&2
  exit 1
fi
python3 - "$bundle/manifest.json" <<'PY'
import hashlib, json, sys
from pathlib import Path
manifest_path = Path(sys.argv[1])
root = manifest_path.parent
manifest = json.loads(manifest_path.read_text())
if manifest['schema'] != 'pocol-private-bundle-v1' or manifest['rehearsalOnly']:
    raise SystemExit('A loopback rehearsal bundle cannot be installed as a server release')
entries = list(root.rglob('*'))
if root.is_symlink() or any(p.is_symlink() for p in entries):
    raise SystemExit('Symlinks are not allowed in a deployment bundle')
if {p.relative_to(root).as_posix() for p in entries if p.is_file()} != set(manifest['sha256']) | {'manifest.json'}:
    raise SystemExit('Unexpected or missing files in deployment bundle')
for name, expected in manifest['sha256'].items():
    path = root / name
    if not path.resolve().is_relative_to(root.resolve()):
        raise SystemExit('Unsafe manifest path')
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    if digest.hexdigest() != expected:
        raise SystemExit('Bundle checksum mismatch: ' + name)
PY
python3 "$bundle/runtime.py" verify --bundle "$bundle"
systemd-analyze verify "$bundle/pocol-private.service"
if ! id -u pocol >/dev/null 2>&1; then
  useradd --system --home-dir /var/lib/pocol-private --shell /usr/sbin/nologin pocol
fi
install -d -m 0755 /opt/pocol-private
cp -a -- "$bundle/." /opt/pocol-private/
chown -R root:root /opt/pocol-private
chmod -R go-w /opt/pocol-private
install -m 0644 /opt/pocol-private/pocol-private.service /etc/systemd/system/pocol-private.service
systemctl daemon-reload
echo 'Installed; NOT started or enabled. Complete the host preflight in README.md before starting.'
