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
- the explorer insights' reads `/api/holders.php`, `/api/holders-stats.php` and `/api/miner-geography.php` (GET, HEAD)
  and the operator node reports `POST /api/geo-report.php` (Holders index and Miner map below;
  [MINERS-MAP-HOLDERS.md](../../development/explorer/MINERS-MAP-HOLDERS.md) is their design and API contract);
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
- `pocol_geo_reporter.py`, `install_geo_reporter.py`: the operator node reporter and its installer (Operator node
  reporter below; not installed anywhere); `test_geo_reporter.py` tests both with a loopback fake node and every system
  command faked.
- `e2e_explorer_devnet.py`: the observer's command line, the gateway with its transaction index, holders index and
  miner map on a local v3 devnet, an operator report, a gateway restart and a reorganization (Transaction index,
  Holders index and Miner map below).
- [DEPLOY-INSIGHTS.md](DEPLOY-INSIGHTS.md) (Arabic: [DEPLOY-INSIGHTS-AR.md](DEPLOY-INSIGHTS-AR.md)): the deployment
  plan of the holders index and the miner map on the current gateway host, with its rollback.

```sh
python3 -B -m unittest discover -s deploy/explorer -v
python3 -B -m unittest discover -s deploy/rewards -p test_explorer_api.py -v
python3 -B -m unittest discover -s deploy/rewards -p test_pocol_transactions.py -v
python3 -B -m unittest discover -s deploy/rewards -p test_pocol_holders.py -v
python3 -B -m unittest discover -s deploy/rewards -p test_pocol_geoip.py -v
POCOLD=/path/to/pocold python3 -B deploy/explorer/e2e_explorer_devnet.py   # devnet ports 30300-30302, 8540-8542 free
```

The e2e takes free loopback ports for the observer's RPC and the gateway. It runs the gateway's own `main()` with
`HolderIndex`'s `genesis_wei` set to the devnet's genesis supply (a devnet funds its `--accounts` at genesis, which the
profile rule of the holders index does not count; without it every snapshot rightly publishes `complete: false` with
that difference), and sends the operator report from loopback with an `X-Forwarded-For` naming an address the
synthetic test CSV places in FR, as Caddy forwards a reporting server's address. No product code has a test-only path.

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

The first step must succeed: it builds the snapshot's fourteen public files (`deploy/websites/tools/explorer-s1-include.txt`),
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
`/api/health.php` PASS, `/api/rpc.php` and `/rpc` naming the network, `/health`, `/api/holders.php` and
`/api/miner-geography.php` (their data for the network, or their JSON 503 while the holders index has no snapshot yet),
hidden and server paths 404, the
Studio route when it is installed, and the faucet route when it is installed (`GET /faucet/status` answering with
the network's chain ID; nothing is posted to `/faucet/claim`). On any failure it restores the previous Caddyfile, files, site copy and service
state, and names what it could not restore together with the backup.

The miner map's options: `--geoip-db FILE` installs a GeoIP database built with `deploy/rewards/pocol_geoip.py build`
(checked record by record first) as `/opt/pocol-explorer-gateway/geoip/dbip-country.bin`; an update without it keeps
the installed one, and without any nothing is located. While `/etc/pocol-explorer-gateway/geo-reporters.json` exists
(root-owned, `0600`, valid) the installer adds the drop-in `pocol-explorer-gateway.service.d/geo-reporters.conf`
(`LoadCredential=geo-reporters:...`); without it there is no drop-in and the report route answers 503. Remove that
file only by rerunning the installer afterwards: `LoadCredential=` fails the service's start while its file is
missing. The installer replaces the unit of this release and the two before it (main at 04f77806 and at 1bb11f4c);
any other unit text is refused as a custom edit. `--check` names what the install would do with both
([DEPLOY-INSIGHTS.md](DEPLOY-INSIGHTS.md)).

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

## Holders index

The Holders tab (`/api/holders.php`, `/api/holders-stats.php`) comes from `deploy/rewards/pocol_holders.py`, installed
with the gateway and run in its process like the transaction index: the unit sets
`POCOL_HOLDERS_DB=/var/lib/pocol-explorer-gateway/holders.sqlite3` in the service's private state directory. No RPC
lists accounts and state is served only for the newest 32 blocks, so the index builds the list itself:

- A history pass from genesis reads every light block (the full block when it has transactions, the receipt of each
  contract creation): each address's first height, its top-level transactions sent or received, the blocks each miner
  ID won, and the base fee destroyed (`baseFeePerGas x gasUsed`). The newest 4096 blocks keep undo rows, so a
  reorganization is rolled back exactly; a deeper one indexes again from genesis.
- At the node's head h (inside its state window), a tip snapshot reads the registry (`pocol_getMiners`), `heldTotal`,
  `burnedTotal`, each live ActList record's graceStart and the balances of what changed, every read pinned to h, then
  reads the block at h again (a change discards the snapshot). It checks the supply identity: the sum of the found
  balances must equal the minted supply less the base fees destroyed. A difference first makes the next round read
  every known address again; one that survives that is published with `verification.complete: false` and the
  difference. When `pocol_getCaps` answers for h, its account count is compared too (else `accountsOnChain` is null).
- Mining rewards: each new block's executed reward flows (`pocol_rewards.snapshot_v3` of that block: the settlement's
  winner, proposer and share payments, the tip share and the evidence reporter's cut; bond refunds and transfers are
  not rewards) per reward address and miner ID, from the first block the index followed (`rewardsFrom`). A gap longer
  than the state window makes them null from then on. This release has no operator command for
  `HolderIndex.reset_rewards()`: the only reset is to stop the service, remove the index file (and its `-wal` and
  `-shm`) and start it, which indexes again from genesis and counts rewards from the new start.
- A snapshot is published as a materialized ranking (rank by Total descending, ties by address), read by index only;
  at least the newest 3 snapshots and every one younger than 180 s are kept, so a reader paging a pinned snapshot stays
  consistent. A page pinned to a snapshot that is gone or no longer canonical is 409
  `{"error":"The holder snapshot changed. Refreshing.","reset":true}`. Statistics (supply, holder counts, top-N
  concentration, balance ranges, an hourly history point from the index start, at most 2,000) are computed at
  publication, so the statistics read is one row.
- States: `syncing` (the history pass is behind the head), `ready`, `reconnecting` (the node cannot be read; the last
  snapshot is still served).

| Name | Definition at one block height h |
|---|---|
| Genesis supply | 4,000,000 PCOL (the Mainnet profile: 4 founders x 10^24 wei, each founder's allocation plus its bond) |
| Issued | `issuance::cumulative(h)` |
| Minted (gross) G | genesis supply + issued |
| Base fees destroyed F | sum of `baseFeePerGas x gasUsed` over blocks 1..h |
| Protocol burned (locked) P | RewardEscrow `burnedTotal` at h; unspendable, still inside RewardEscrow's balance |
| Total burned | F + P (both parts shown) |
| Current supply S | G - F - P = sum of balances(h) - P; the denominator of every percentage |
| Bonded | `heldTotal` at h |
| Pending rewards (system) | RewardEscrow balance - heldTotal - burnedTotal |
| Available (an account) | `eth_getBalance(address, h)` |
| Bonded (an account) | the record's bond when the account is a registry record ID whose bond is held (status != 0, not a tomb, bond state HELD or ACCRUING); else 0. A bond counts on the miner ID, never also on its reward address |
| Total (an account) | Available + Bonded; for RewardEscrow its balance - heldTotal - burnedTotal, so that the sum of Total is S |
| Circulating | sum of Available over non-system accounts |
| Supply % | `floor(Total x 10^12 / S)` as percent x 10^10 |
| Holder | an account with Total > 0 |
| Account types | `system` (0x...c0c001-0x...c0c005, PAY 0x...c0c0ff, SYSTEM 0xff...fe), `miner` (a registry record ID), `reward` (a record's reward address that is not a record ID), `contract` (code), `wallet` (other) |
| Transactions (an account) | top-level transactions sent or received since genesis (internal transfers are not transactions) |
| Blocks mined | blocks whose `miner` is the record ID, since genesis |
| Mining rewards | executed reward flows to the address since the index start (`rewardsFrom`); null after a gap; never a total since genesis |

Holder counts, concentration and balance ranges leave the system accounts out. Completeness is verified, not
guaranteed: an address that only received PCOL through an internal call of a non-system contract cannot be
discovered over RPC; such value is published as the difference `verification.unlocatedWei`, never hidden.

Bounds and measurements (`deploy/rewards/test_pocol_holders.py` ScaleTests, Python 3.11, a fake node with 100,000
accounts): publishing a snapshot of them took 1.4-1.6 s (tests require under 120 s); a page 0.4-0.6 ms at any depth,
order or type filter, 3 ms for a one-digit search prefix (tests require under 1 s; a page running 2 s is
interrupted); the file was 52.6 MB with those accounts and one published snapshot of them, and every further kept
snapshot adds its own rows. A snapshot's peak memory stayed under 50 MB (the service keeps `MemoryMax=256M`).
`EXPLAIN QUERY PLAN` of every page statement shows no sort and no table scan. Pages run on their own read-only
connections behind the gateway's two holder slots (`HOLDER_SLOTS`), never the node reads' four. Node load: the
history pass is paced (5 ms per block), at most one snapshot per new block and slower when one takes longer, and each
new block's rewards are one `snapshot_v3` read (at most 8,192 node calls and 60 s each). Not measured against PoCol
Mainnet; tested on a fake chain and on a local devnet (`e2e_explorer_devnet.py`, `explorer_smoke.mjs`).

## Miner map

The Miner map tab and the Overview's map card (`/api/miner-geography.php`, schema `pocol-miners-geography-v2`) show
registered miners and network nodes **by country only** (`deploy/rewards/pocol_geoip.py`, in the gateway's process).

What is known exactly, for every miner: its registry state (registered, active, inactive, exited, punished) from the
holders index's latest snapshot, its blocks in the last 8,640 blocks (about 24 h) and its mining rewards since the
index start. What is not known: where a miner is. No protocol message ties a peer's IP address to a miner, so the
gateway never infers one. A miner gets a country only from a fresh (at most 180 s old) signed report of an operator's
node that names that miner as its own (`pocol_status.miner`, the registry record ID); the miner is then placed in the
country of the report's source address. Every other registered miner is **Unknown**, and so is a miner whose fresh
reports name different countries. "Connected" counts reported miners whose node head is within 6 blocks of the index.

Sources, each labelled in the answer:

1. The observer node of this host: its `pocol_status.peers` (read at most every 10 s). These are network nodes, not
   miners, and they are never counted as miners.
2. Operator node reports (Operator node reporter, below): none until the owner installs a reporter.

Countries come from DB-IP's "IP to Country Lite" database (CC BY 4.0, attribution "IP Geolocation by DB-IP",
https://db-ip.com), installed by the operator as a file on the gateway (`--geoip-db`) and looked up locally; nothing
contacts a geolocation service. Without the file nothing is located and the answer's `database` is null. Only public
unicast addresses are looked up (private, loopback, link-local, CGNAT, documentation and reserved ranges count as not
locatable). Each address is replaced at once, in memory, by a keyed id (`HMAC-SHA256(network salt, IP)`, first 24 hex
digits: a node seen by several servers counts once) and its country; no IP address, keyed id, miner address or
coordinate finer than a country is ever in an answer (`explorer_api.miner_geography` requires the contract's exact
keys), and nothing of it is written to disk. A visitor's own IP address is never looked up: the report route looks up
its sender only after the report's signature has been verified.

The country of a node is the GeoIP country of its public address: a VPN exit, NAT or a hosting provider can place it
elsewhere than its operator. Without reporters (the state at deployment) every miner is Unknown and the map shows only
the network nodes the explorer's own node is connected to. The map is drawn from the bundle's own 240 Natural Earth
country shapes; the explorer loads nothing new from outside.

The old `/api/miner-locations.php` (schema v1) stays as it was: 503, "Miner locations are unavailable: the PoCol
network does not record where miners are.", so older clients keep their behaviour.

## Operator node reporter (not installed)

`pocol_geo_reporter.py` runs on a node host (the three founder servers are the intended hosts) from
`pocol-geo-reporter.timer` every 60 s. One run reads its own node once over loopback (`pocol_status` and
`pocol_getParams` in one batch on `127.0.0.1:8545`), checks the network, looks up its peers' countries locally with the
same database, replaces every IP by its keyed id, and POSTs one signed report (`HMAC-SHA256(reporter key, body)`, at
most 16,384 bytes) to `https://explorer.pocol.net/api/geo-report.php` with full certificate verification. Raw IP
addresses never leave the server and are never printed; it writes no file and never touches mining.
`install_geo_reporter.py --database FILE --check` runs every preflight; without `--check` it installs the reporter
(`/opt/pocol-geo-reporter`, `/etc/pocol-geo-reporter`, the service and the timer: DynamicUser,
ProtectSystem=strict, LoadCredential=geo-reporter, Nice=19, MemoryMax=64M), runs one `--dry-run` and enables the
timer, restoring the previous state on any failure; `--uninstall` removes it and keeps the credential.

It is not installed on any server. Services on the founder servers are changed only with the owner's approval
([GENESIS-RECORD-2026-11-21.md](../../development/public-network/GENESIS-RECORD-2026-11-21.md)), and installing it is
listed as an open owner decision in
[OWNER-DECISIONS-2026-11-25.md](../../development/decisions/OWNER-DECISIONS-2026-11-25.md). Each reporter needs its
key and the network salt, both in the gateway's credential and in that server's own credential file, made by the
owner on the hosts, never in the repository.

`POST /api/geo-report.php` answers 204 accepted, 400 malformed (an `Origin` header included), 401 an unknown reporter
or a bad signature, 409 a stale (`at` outside +-120 s) or replayed report, 413 over 16,384 bytes, 415 not JSON, 429
when the budget is spent, 503 without the reporters' credential.

## Routes and budgets of the insights reads

| Route | Answer | Token cost | Notes |
|---|---|---|---|
| `GET /api/holders.php` | `pocol-holders-v1` page | 2 | `page` 1..10^9, `size` 25/50/100, `order` balance_desc/balance_asc, `type`, `q` (address prefix), `height`+`hash` together; 400 bad or repeated key; 409 reset |
| `GET /api/holders-stats.php` | `pocol-holders-stats-v1` | 1 | other query keys ignored; at most 4,096 bytes |
| `GET /api/miner-geography.php` | `pocol-miners-geography-v2` | 5 | other query keys ignored; at most 4,096 bytes |
| `POST /api/geo-report.php` | 204 | 1 | no relay slot; no node read |

They share the gateway's token buckets (the node's 80 tokens at 20 a second; 40 at 10 a second per client) and the four
relay slots; holder and map reads also take one of the two holder slots. A spent budget or busy slots answer 429 with
`Retry-After: 3` (the transactions route answers 503 there), and 503 means no data yet (the index has no snapshot, or
is not installed). The explorer refreshes the Holders tab every 15 s while it is visible and the map every 30 s.

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
   without `--check`. It prints `INSTALLED` once the node answers for PoCol Mainnet with no miner. Run it from the
   repository checkout: it refuses an invitation whose chain ID and genesis hash are not the ones
   `deploy/websites/site-identity.json` pins (`--other-network` is only for a test network).
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

The holders index and the miner map on the gateway host that already runs the explorer (an update, with the GeoIP
database, its monthly update and the rollback) are in [DEPLOY-INSIGHTS.md](DEPLOY-INSIGHTS.md).

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
- wrong methods on the four paths get the PHP 405 and `Allow` header (not a 501), other `/api` paths are 404;
- the insights routes (`/api/holders.php`, `/api/holders-stats.php`, `/api/miner-geography.php`,
  `/api/geo-report.php`) never had a PHP relay: they follow MINERS-MAP-HOLDERS.md (Routes and budgets above).

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
