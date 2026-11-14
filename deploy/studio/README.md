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

## Methods, chunk cache and budgets

Besides the publisher's reads and `eth_sendRawTransaction`, the relay serves the
read methods the extension's bridge exposes to site scripts:
`eth_maxPriorityFeePerGas`, `eth_feeHistory` (at most 128 blocks and 16
percentiles), `eth_getStorageAt`, `eth_getBlockByHash` (without transactions)
and `pocol_getLogs` (one `blockHash`, or a numeric range of at most 1024
blocks). `net_version` and `web3_clientVersion` are answered by the relay itself,
so the node's version is never exposed. The tags `safe` and `finalized` are
accepted wherever a block tag is.

`eth_getCode` at `latest` or `safe` is the site viewer's chunk read. The relay
asks the node for the code at `safe` first; code that is chunk-shaped there
(first byte `0x00`, at most 24,576 bytes) is kept in an in-memory LRU of raw
bytes (24 MiB, flushed whenever the node's identity check fails) and answered
from it afterwards. Code that is not at `safe` yet is forwarded and not cached,
and empty code is never cached. Chunk reads have their own per-client and global
budget and two reserved upstream slots, so a site open does not use the general
read budget and cache hits are served while the general slots are busy. The
systemd unit allows 256 MiB.

Every per-client budget keys on `X-Pocol-Client-IP`, which the installer's Caddy
route overwrites with the connecting address; the installer refuses a Caddy file
with another route to the relay port, and the unit starts the relay with
`--bind 127.0.0.1` (no other address is accepted).

For a test or rehearsal network, `--network FILE` takes the explorer gateway's
`--v3-network` format, `{"chainId": N, "genesisHash": "0x...", "specVersion": 2,
"v3": true}`, validated strictly; without it the pinned identity applies.

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
