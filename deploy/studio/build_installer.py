#!/usr/bin/env python3
"""Build an offline, checksum-verified installer; contains no credentials."""
import base64
import hashlib
from pathlib import Path

root = Path(__file__).resolve().parent
files = ['pocol_studio_gateway.py', 'install_studio_gateway.py']
payloads = {name: {'sha256': hashlib.sha256((root/name).read_bytes()).hexdigest(),
                   'base64': base64.b64encode((root/name).read_bytes()).decode()} for name in files}
script = '''#!/usr/bin/env bash
# Installs a separate loopback-only Studio relay and adds its existing-HTTPS route.
# Never modifies node configuration, blockchain data, miner/wallet keys, or Explorer code.
set -Eeuo pipefail
[[ "$(id -u)" == 0 ]] || { echo 'Run with sudo.' >&2; exit 1; }
[[ "$#" -le 1 && ( "$#" == 0 || "${1:-}" == --check ) ]] || { echo 'Usage: sudo bash Install-PoCol-Studio-Gateway.sh [--check]' >&2; exit 2; }
command -v python3 >/dev/null
command -v flock >/dev/null
exec 9>/run/lock/pocol-studio-gateway-install.lock
flock -n 9 || { echo 'Another Studio gateway installation is running.' >&2; exit 1; }
python3 - "$@" <<'POCOL_STUDIO_INSTALL'
import base64, hashlib, os, pathlib, subprocess, sys, tempfile
PAYLOADS = __PAYLOADS__
with tempfile.TemporaryDirectory(prefix='pocol-studio-install-') as directory:
    for name, payload in PAYLOADS.items():
        raw = base64.b64decode(payload['base64'], validate=True)
        if hashlib.sha256(raw).hexdigest() != payload['sha256']:
            raise SystemExit('Installer payload checksum mismatch')
        target = pathlib.Path(directory) / name
        target.write_bytes(raw)
        os.chmod(target, 0o600)
    subprocess.run([sys.executable, str(pathlib.Path(directory) / 'install_studio_gateway.py'), *sys.argv[1:]], check=True)
POCOL_STUDIO_INSTALL
'''.replace('__PAYLOADS__', repr(payloads))
(root/'Install-PoCol-Studio-Gateway.sh').write_text(script)
print('Built Install-PoCol-Studio-Gateway.sh')
