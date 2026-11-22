# Explorer gateway host (Sites W8)

`explorer.pocol.net` is served by the explorer gateway server (owner decision 9, 2026-11-16): it replaces the old PHP
site there, and the owner moves its DNS record at launch. Nothing here touches DNS or servers. One Caddy site on that
server serves:

- the explorer snapshot (`deploy/websites/explorer.pocol.net`, a built bundle without sources) as static files with
  Caddy's `file_server`, with the headers its retired `.htaccess` set (Content-Security-Policy, X-Frame-Options DENY,
  Referrer-Policy, Permissions-Policy, nosniff) and `Cache-Control: no-store`, also on the answers Caddy writes itself
  (a missing file's 404, a 502 when the gateway cannot be reached; the block's `handle_errors`), as `Header always set`
  did;
- the snapshot's same-origin reads `/api/rpc.php` (POST), `/api/health.php`, `/api/miner-locations.php` and
  `/api/transactions.php` (GET), answered in process by the gateway (`deploy/rewards/explorer_api.py`, imported by
  `gateway.py`) with the checks of the PHP relays they replace. pocol.net's network summary
  (`pocol.net/api/network.php`) reads the same `/api/rpc.php`;
- the gateway's `/rpc` and `/health`, the Studio relay `/studio/*` that `deploy/studio` adds to the same block, and
  the fee faucet's `/faucet/claim` and `/faucet/status` that `deploy/faucet/faucet_route.py` adds to it (proxied to
  the faucet on `127.0.0.1:8547` with the prefix removed; `deploy/faucet/README.md`).

Dotfiles, server code (`.php`), checksum lists and the extensions the `.htaccess` denied (`zip tar gz bak log sh md
json map`) are 404; there is no directory listing. `/api/*` and `/rpc` keep a 16 KiB request-body guard each, also
after the Studio installer raises the site's `max_size` to 128 KiB for its own route; the `/api/*` one answers as
rpc.php did, 413 `{"error":"Request size limit exceeded."}` as JSON.

## Files

- `explorer_site.py`: the Caddy site block (between `# BEGIN/END POCOL EXPLORER GATEWAY v1`), the checks of the
  static copy, and a loopback test copy of the block (`test-config`, used by `deploy/websites/tests/explorer_smoke.mjs`).
- `install_explorer_gateway.py`: the transactional installer of the gateway service, the static copy and the block.
- `test_explorer_gateway.py`: the block against the Studio installer and the faucet route in every order, the update
  and refusal rules,
  the copy's checks on the committed snapshot, and the install, `--check` and rollback paths on temporary paths with
  every system command faked.
- `install_observer_node.py`: the installer of the host's own node, a non-mining `pocold run` (no key) on
  `127.0.0.1:8545` (Observer node below); `test_observer_node.py` tests it with every system command faked.
- `e2e_explorer_devnet.py`: the observer's command line, the gateway and its transaction index on a local v3 devnet,
  with a reorganization (Transaction index below).

```sh
python3 -B -m unittest discover -s deploy/explorer -v
python3 -B -m unittest discover -s deploy/rewards -p test_explorer_api.py -v
python3 -B -m unittest discover -s deploy/rewards -p test_pocol_transactions.py -v
POCOLD=/path/to/pocold python3 -B deploy/explorer/e2e_explorer_devnet.py   # devnet ports 30300-30302, 8540-8542, 8549, 8662
```

## Install on the gateway server

Run from a checkout of the release commit: the installer installs `deploy/rewards/gateway.py`, `pocol_rewards.py` and
`explorer_api.py` from it, and reads the Studio installer's and the faucet route's rules from `deploy/studio` and
`deploy/faucet`. It needs Python 3.10+,
systemd, Caddy (running), the node's RPC on `127.0.0.1:8545`, and the operator's network file
`/etc/pocol-explorer-gateway/network.json` (and optionally `forks.json`; `deploy/rewards/README.md`, Gateway startup
integration).

```sh
# The static copy, from the committed blobs, into a new directory; the check runs only if that build succeeded.
D=$(mktemp -d) && python3 deploy/websites/tools/site_snapshot.py --out "$D" \
  && sudo python3 deploy/explorer/install_explorer_gateway.py --site "$D" --check
sudo python3 deploy/explorer/install_explorer_gateway.py --site "$D"
curl --fail --max-time 20 https://explorer.pocol.net/api/health.php
```

The first step must succeed: it builds the snapshot's twelve public files (`deploy/websites/tools/explorer-s1-include.txt`),
checked against its `SHA256SUMS.txt`, into a new directory, so no copy of an earlier run is left to install. The
installer also compares the copy with this checkout's snapshot built the same way, and refuses one that differs (a
leftover or edited copy) unless `--accept-site site-<id>` names that copy's id (for a copy meant to differ, such as a
bundle switched for a devnet); it prints both ids. It refuses a copy with a hidden, server, checksum or
configuration file, a symbolic link, an unexpected file type, more than 64 files or 8 MiB, or an `index.html` script
or stylesheet that is missing. It is written root-owned and world-readable to `/srv/pocol-explorer/site-<first 12 hex of its manifest's
SHA-256>`, so a new copy is a new root and the Caddy reload switches it at once; the previous copy is kept for a
rollback and older ones are removed.

`--check` runs every preflight and changes nothing: the network file, the node's chain ID and genesis hash, the copy,
the free port 8088, and `caddy validate` of the candidate Caddyfile. It also refuses what the unprivileged readers
could not read or others could change: a `network.json` or `forks.json` not readable by others (`0644`) or in a
directory they may not enter, and a `/srv/pocol-explorer` or `/opt/pocol-explorer-gateway` that is not a root-owned
`0755` directory. The directories the installer creates are `0755` whatever root's umask. An install backs up what it replaces under
`/var/backups/pocol-explorer-gateway/`, writes the sources and the systemd unit `pocol-explorer-gateway.service`
(`--bind 127.0.0.1 --port 8088 --site-origin https://explorer.pocol.net`, no other address), restarts the service and
checks `/health`, `/api/rpc.php` and `/api/health.php` on loopback, then writes and reloads the Caddyfile and checks
over HTTPS, with the public name and trusted certificates: every static file byte for byte, the headers of `/`,
`/api/health.php` PASS, `/api/rpc.php` and `/rpc` naming the network, `/health`, hidden and server paths 404, the
Studio route when it is installed, and the faucet route when it is installed (`GET /faucet/status` answering with
the network's chain ID; nothing is posted to `/faucet/claim`). On any failure it restores the previous Caddyfile, files, site copy and service
state, and names what it could not restore together with the backup.

The HTTPS check needs the certificate for explorer.pocol.net, which Caddy obtains only once the DNS record points at
the gateway server. So run `--check` before the owner moves the record, and the full install once public resolvers
return this server for the name:

```sh
dig +short explorer.pocol.net @1.1.1.1; dig +short explorer.pocol.net @8.8.8.8   # both: this server's address
```

The install waits up to `--https-wait` seconds (default 600, from 60 to 3600) for the first certificate, past the
ACME client's first retries. If the HTTPS check still fails, it rolls back; wait for the record to propagate before
rerunning, because each rerun starts a new issuance and repeated failed validations can reach the CA's limit for the
name for an hour. TLS verification is never relaxed.

The Caddyfile: on a fresh server the stock Debian/Ubuntu `:80` file is replaced by the block; other sites are kept;
an unmanaged `explorer.pocol.net` site, a second marked block, an edited marked block, or another route to port 8088
is refused for review (no changes made). An update replaces only the site root of the marked block and keeps the
Studio route and its body size, and the faucet route; an edited, damaged or duplicated copy of either route is refused.
Install this before the Studio gateway (`deploy/studio`) and the faucet route (`deploy/faucet/faucet_route.py`), which
add their routes to this block (Studio after the host line, the faucet right before `@rpc path /rpc`); either order of
those installs and of later updates gives the same file.

## Observer node

The gateway reads only `127.0.0.1:8545`, and the founders' RPC is loopback-only, so the gateway host runs its own
node. `install_observer_node.py` installs one that never mines and holds no key: `pocold run` with no `--miner` or
`--miner-key-file`, RPC on `127.0.0.1:8545` only, the invitation's seeds (`--seed`), outbound P2P only unless
`--listen` is given, under `pocol-observer.service` (DynamicUser, StateDirectory, ProtectSystem=strict, the standalone
node's MemoryMax=7G, so a host with 8 GiB of RAM or more). The installer refuses to write a unit with a miner or another
RPC address. It checks the pocold binary against the SHA-256 the operator gives (from the verified signed release), the
invitation against its published SHA-256 (`--invitation-sha256` is required: the invitation decides the node's seeds
and network identity), and that `/etc/pocol-explorer-gateway/network.json` names the same network. Seeds must be
plain `IPv4:PORT` or `[IPv6]:PORT` (no IPv6 zone, no whitespace), at most 7 (pocold's limit), and every argument of
the unit's `ExecStart` must be plain path and address characters, so an invitation cannot add pocold flags or systemd
`%` specifiers. It writes the invitation's three network files exactly as `pocol start` would. After the start it waits for
the node's RPC to name the invitation's chain ID and genesis hash and report no miner; on any failure it rolls back.
Its unit is ordered before the gateway's (`After=pocol-observer.service`).

## Transaction index

The Transactions tab (`/api/transactions.php`) and `pocol_getTransactions` on `/rpc` come from
`deploy/rewards/pocol_transactions.py`, installed with the gateway and run in its process. The unit sets
`POCOL_TX_DB=/var/lib/pocol-explorer-gateway/transactions.sqlite3` in the service's private state directory.

- A daemon thread follows the node over loopback. It reads light blocks one height at a time, and reads a full block
  (by the light block's hash) only when the block has transactions. It stores every transaction with its block,
  position, block time, from, to, value and kind (Transfer, Contract call, Contract creation, Undecoded), newest first.
- Reorganizations: the newest 4096 heights keep their block and parent hashes. Each round compares the stored tip with
  the node's block at that height and, on a difference, removes everything above the fork point and indexes the new
  branch. A page pinned to a snapshot that is no longer canonical is 409 with `reset`, and the explorer starts again
  from page 1.
- Restarts resume from the file. A file of another network is refused at start.
- Bound: at most 500,000 transactions (`POCOL_TX_MAX_ROWS`; about 300 MB on disk with its indexes, measured 296 MB
  on a synthetic index of that size, plus room for the WAL). Past it the oldest whole blocks are removed, and the
  page's `index.from` names the first kept block, which the Transactions tab shows. A reorganization below that block
  lowers `index.from` to the fork point's next block.
- Orders: newest and oldest first walk the primary key; highest and lowest value walk the `(value, block, position)`
  index forwards or backwards (equal values: newest first for highest, oldest first for lowest; Undecoded
  transactions, which have no value, last and newest first in both), so no page sorts the table and a deep page only
  skips index entries. Measured on a synthetic 500,000-row index: page 5,000 of 100 rows (the `Last` button) under
  0.15 s in every order; the slowest page is a search with a one-digit prefix (about 3 in 16 rows match), about
  0.9 s. A page running 2 s is interrupted (503).
- The gateway never waits for it: pages are read on their own read-only WAL connections, behind their own two slots
  (`gateway.TX_SLOTS`), never the four slots of the node reads, so transaction pages can make only the Transactions
  tab busy, not the rest of the explorer. Node failures make the index `reconnecting` while the last data is still
  served.

It was tested against a fake chain (`test_pocol_transactions.py`) and on a local devnet (`e2e_explorer_devnet.py`,
`explorer_smoke.mjs`), not against PoCol Mainnet.

## Miner map

The network does not record where miners are, and nothing ties a peer's IP address to a miner address. The gateway has
no GeoIP module (`pocol_getNetworkGeography` and `/api/miner-locations.php` stay 503 with "Miner locations are
unavailable: the PoCol network does not record where miners are."), and the explorer's Miner map shows only that
notice: "Miner locations unavailable: the network has no location data for miners, and peers' IP addresses are not
used to place them (privacy)." Its map, counts, filters and GeoIP texts are shown only with geography data, which this
gateway never serves. Showing the countries of one node's peers would be an estimate about connections, not miners, and close to
personal data with few peers. That needs an explicit owner decision (a licensed database such as DB-IP Lite, its
attribution, and a privacy review); until then the view stays unavailable.

## Production deployment of the gateway host (exact steps)

None of this was run on a real host; the installers were tested with faked system commands and the services on a
local devnet. Prerequisites the owner provides:

- The website identity PR (step 7: `site_identity.py --to network.pocol.json --commit SHA --gateway-host
  explorer.pocol.net`) merged. A checkout whose explorer bundle names another network shows "The data source belongs
  to a different network." for every PoCol Mainnet answer.
- A host for the gateway, separate from the founders: 8 GiB of RAM or more, disk for the node's chain growth plus about
  300 MB for the index (plus room for its WAL), systemd, Python 3.10+, Caddy, inbound 80/443, and outbound P2P to the seeds on 30300.
- The signed release's `pocold` (signature verified per deploy/release) and its SHA-256, and the invitation
  `network.pocol.json` (PoCol Mainnet: SHA-256 `a74c1cf432e7fbca59aa6bd8250af660f1b1582d693b1b109316686f7241f233`).
- The DNS change, made by the owner, between steps 5 and 6.

Steps, from a checkout of the release commit:

1. Network file for the gateway, from the invitation's `devnet.json` `chainId` and `genesisHash`:
   `sudo install -d -m 0755 /etc/pocol-explorer-gateway`, then write `/etc/pocol-explorer-gateway/network.json`
   (mode 0644) as `{"chainId": CHAIN_ID, "genesisHash": "GENESIS_HASH", "specVersion": 2, "v3": true, "source":
   "https://explorer.pocol.net"}`, with the invitation's two values; they must equal the `chainId` and `genesisHash`
   that `deploy/websites/site-identity.json` pins (PoCol Mainnet's, set by the step-7 identity switch).
2. The observer node: `sudo python3 deploy/explorer/install_observer_node.py --invitation network.pocol.json
   --invitation-sha256 a74c1cf4... --pocold ./pocold --pocold-sha256 <release digest> --check`, then the same command
   without `--check`. It prints `INSTALLED` once the node answers for PoCol Mainnet with no miner.
3. Wait for the sync: `curl -s -X POST -H 'Content-Type: application/json' --data
   '{"jsonrpc":"2.0","id":1,"method":"pocol_status","params":[]}' http://127.0.0.1:8545`. The head height must grow
   and the node must have peers. The gateway can be installed while it syncs; its pages show the node's head.
4. Caddy: `sudo apt-get install caddy` (the stock `:80` Caddyfile is replaced by the explorer block).
5. Before the DNS move: `D=$(mktemp -d) && python3 deploy/websites/tools/site_snapshot.py --out "$D" && sudo python3
   deploy/explorer/install_explorer_gateway.py --site "$D" --check` must print `CHECK PASSED` with the chain ID
   and the genesis hash of step 1.
6. The owner points `explorer.pocol.net` at the host. Wait until `dig +short explorer.pocol.net @1.1.1.1` and
   `@8.8.8.8` both return it.
7. `sudo python3 deploy/explorer/install_explorer_gateway.py --site "$D"`: the gateway (with the transaction index)
   on 127.0.0.1:8088, the static copy and the Caddy block, checked over HTTPS with trusted certificates, rolled back on
   failure.
8. From another machine: `curl --fail --max-time 20 https://explorer.pocol.net/api/health.php` answers `"result":"PASS"`;
   `curl -s https://explorer.pocol.net/api/transactions.php` answers `"state":"syncing"` and then `"ready"`.
9. Then the Studio relay (`deploy/studio`) and the fee faucet route (`deploy/faucet/faucet_route.py`).

## Behaviour that differs from the PHP relays

The handlers answer as `api/rpc.php`, `health.php`, `miner-locations.php`, `transactions.php` and `lib.php` did, with
the same allow-list, bounds, status codes, messages, field filtering and headers
(`deploy/rewards/testdata/explorer-php-vectors.json` holds 131 requests answered by the PHP relays themselves next to
these handlers' answers). Where an in-process answer cannot be the PHP one, the safer one is kept:

- a spent gateway request budget gets the busy answer of each path: 429 with `Retry-After: 3` on `/api/rpc.php`, 429
  without it on `/api/health.php` and `/api/miner-locations.php`, 503 on `/api/transactions.php` (PHP answered 503
  naming its `config.php`);
- a request body needs exactly one `Content-Length` and no `Transfer-Encoding`;
- repeated transaction query keys and repeated `Origin` headers are refused;
- `/api/health.php` has no `phpReady` or `curlReady` keys; `gatewayReachable` is true when the read went through the
  relay (PHP: the gateway answered HTTP 200), even if the node answered it with an error;
- an empty filtered object is encoded `{}` (PHP encoded `[]`), and big integers in geography and transaction
  answers stay exact (PHP re-encoded them as floats);
- `/api/miner-locations.php` without geography says "Miner locations are unavailable: the PoCol network does not
  record where miners are." (PHP pointed to a GeoIP gateway update that does not exist);
- wrong methods on the four paths get the PHP 405 and `Allow` header (not a 501), other `/api` paths are 404.

The budget is the gateway's (deploy/rewards/gateway.py `charge`, tested by `test_gateway_limits.py`). The node's
bucket (80 tokens, 20 per second) is shared by `/rpc`, the `/api` reads, pocol.net's summary and the reward page, as
it was behind the PHP relays. Inside it, each client has its own bucket (40 tokens, 10 per second, at most 4096 clients
remembered). Behind Caddy, the client is the last `X-Forwarded-For` hop, which is the address Caddy appends; the
gateway trusts the header only from a loopback peer, and an IPv6 client counts by its /64. So one client cannot make
the explorer busy for everyone. Requests to `/rpc` cannot take the node's bucket below 30 tokens, so a flood sent to
`/rpc` leaves 30 tokens for the `/api` reads. That reserve does not protect against a flood of `/api/rpc.php`:
`/api/rpc.php` is just as public, it accepts requests without an `Origin` header (pocol.net's server-side summary sends
none), and an `Origin` header is easy to forge outside a browser, so many addresses reading there can spend the whole
node bucket, and every caller then gets 429 until it refills (20 tokens a second). A real share for the explorer
site would need an authenticated route for pocol.net's summary; there is none. Four relay slots are also shared by the
`/api` reads (PHP's four lock slots).

The per-client budget is per IPv4 address or IPv6 /64, so browsers behind one shared address (carrier-grade NAT, an
office or campus) share it. One open explorer tab costs about 2.5 tokens a second (the Overview refresh every 15 s:
status, parameters, a members page and 20 blocks, about 37 tokens; an open account view about 23 more), so about
four tabs behind one IPv4 address reach the 10 tokens a second and get 429 (the explorer retries a 429 after the
answer's `Retry-After`, then reports it). This is measured from the bundle's refresh code, not from traffic. A connection beyond the gateway's 16 request slots gets the same JSON busy answer (429,
`Retry-After: 3`), which Caddy passes on, not an empty 502.
