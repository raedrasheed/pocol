# PoCol Studio gateway

A separate, bounded JSON-RPC relay for the Studio browser publisher and viewer.
It listens on `127.0.0.1:8089`, forwards allowed requests to the existing local
PoCol node, and uses the existing `gateway.pocol.net` Caddy site for HTTPS.
It never reads wallet keys or signs transactions.

## Check and build

```sh
python3 -m unittest -v test_studio_gateway.py
python3 build_installer.py
bash -n Install-PoCol-Studio-Gateway.sh
```

The installer embeds its Python source with SHA-256 checksums. It requires
Python 3, systemd, Caddy, `curl`, `flock`, the existing gateway configuration,
and the intended PoCol node on its configured loopback RPC endpoint. Read
`install_studio_gateway.py` before use on an existing server.

## Install on the first testnet node

Transfer `Install-PoCol-Studio-Gateway.sh` to the server, then run:

```sh
sudo bash ~/Install-PoCol-Studio-Gateway.sh --check
sudo bash ~/Install-PoCol-Studio-Gateway.sh
curl --fail --max-time 20 https://gateway.pocol.net/studio/health
```

Require a successful preflight before installation. The response must identify
chain `1599846715` and the expected genesis hash. Also test the health endpoint
from an external client after installation; a successful server-local HTTPS
probe does not prove external reachability.

The installer adds `/studio/rpc` and `/studio/health` without replacing the
Explorer's existing `/rpc` route. It validates the resulting Caddy configuration,
checks HTTPS and both services, and restores the previous configuration and
service state if validation fails. Backups are kept under
`/var/backups/pocol-studio-gateway/`. Unsupported Caddy layouts are refused
instead of being overwritten. The raw node RPC and relay ports remain local.

## Studio and extension

Host the editor on a separate `studio.pocol.net` cPanel document root with HTTPS.
Use the matching Studio extension build. The extension verifies the pinned
testnet identity and content hashes; RN mode trusts the remote node for chain
execution. Browser approval and local signing are still required for publication.
Chrome Web Store publication and review are separate from this package.

`README-AR.txt` contains the Arabic installation walkthrough and Windows transfer
command. No production deployment or live publication is established by the
unit tests. Verify the deployed gateway and a funded testnet publication on the
user's computer before treating the complete workflow as live.

Sources: [PoCol publisher](https://github.com/palcertcom/pocol/blob/50c2f27ee67961dec7aaf0eb735c0b641810e0ea/prototypes/pocold/src/site.rs),
[Caddy reverse proxy](https://caddyserver.com/docs/caddyfile/directives/reverse_proxy),
[cPanel Domains](https://docs.cpanel.net/cpanel/domains/domains/).
