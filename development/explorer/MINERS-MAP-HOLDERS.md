# Explorer: Global Miners Map and PCOL Holders (design and API contract)

Branch `feat/explorer-miners-map-and-holders`, based on `main` at `04f77806` (after #118). This document is the
contract that the backend modules, the frontend module, the tests and the docs implement. Where it states a fact about
the chain it cites the code; where something cannot be known exactly, the feature says so instead of estimating.

Network: PoCol Mainnet 0.5.0, chain ID 2656089850, genesis
`0x664dc53a2fa5e9e7c94ae3d735b1eefabb8630fcbabf5e8b0d34f5db0b8cc64e`, symbol PCOL. Nothing here changes consensus,
genesis, rewards, balances, the node, the founders' servers or DNS. New code is stdlib Python (gateway) and
dependency-free JavaScript (explorer).

## 1. Facts this design rests on

| Fact | Source |
|---|---|
| No RPC enumerates accounts; no trace/debug methods; state is readable only for heights in [head-31, head] (K_hist 32); blocks and headers are served for every height | `prototypes/pocold/src/rpc.rs` `call` arms, `state_for` (-32017); `lp4-execution/src/admission.rs` k_hist |
| System contracts emit no LOG; rewards, bond accruals and refunds are internal PAY transfers with no receipt | `PoColSystemV3.sol:23`; `lp4-execution/src/pay.rs` |
| Bonds are held in RewardEscrow `0x…c0c002`'s balance, not in MinerRegistry; `heldTotal` (Registry slot 4) = Σ w2 over records with status≠0, tomb=0, bondState∈{1 HELD, 4 ACCRUING} | `PoColSystemV3.sol:505-507`; `v3_contracts.rs:599-611` |
| Protocol burns (`burnedTotal`, Escrow slot 0) stay inside Escrow's balance and are never paid out; only the base fee leaves the state (`baseFeePerGas × gasUsed` per block) | `PoColSystemV3.sol:150-309`; `user_phase.rs:251-263`; `v3_contracts.rs:1716-1717` |
| Gross minted G(h) = genesis supply + `issuance::cumulative(h)`; genesis supply on Mainnet = 4 founders × 10^24 = 4,000,000 PCOL (999,980 on each reward address + 20 PCOL bond each in Escrow) | `issuance.rs:147-178`; `pocold/src/testnet.rs:85-88,433-467`; FINDINGS F-34 |
| Σ of all account balances(h) = G(h) − F(h), F(h) = Σ_{i≤h} baseFee_i·gasUsed_i | `v3_contracts.rs:1716-1717` |
| `pocol_getCaps.accounts` = number of existing non-system accounts at the head, with `number`/`hash` | `rpc.rs:1185-1191`; `state_summary.rs:47-71` |
| `pocol_getMiners` = every registry record at the head (no height in the answer): `{id,status,tomb,bondState,lastShare,bond,registeredAt,reward,rewardBalance}` | `rpc.rs:1133-1134` |
| Inactive member (FN-2b C4): an active, live record with `h+1 ≥ max(lastShare, graceStart) + Θ_inact`, Θ_inact = CP25 = 360 blocks; graceStart = w3 >> 192 at `keccak(id‖7)+3` | `PoColSystemV3.sol:533-545`; FRESH-NETWORK-CP.md:25 |
| A block's `miner` is `winner = owner(nonce)` (record key) | `apply.rs:90-92`; `rpc.rs:1351` |
| Executed reward flows of block h (settle(h−D) beneficiaries, tip share) are reproduced exactly by `pocol_rewards.snapshot_v3(query, height=h)` while h is inside the state window | `deploy/rewards/pocol_rewards.py:1038-1199` |
| **No protocol message binds a peer IP to a miner.** Hello has no identity, shares `{tid,nonce}` and templates are relayed; miners mine in their own node | `net.rs:416-421`; `node.rs:3016-3029` |
| The observer node is outbound-only; founders' RPC is loopback-only; the gateway unit may reach only localhost | `deploy/explorer/README.md:108-122`; `rpc.rs:631-632`; `install_explorer_gateway.py:101-102` |
| The explorer bundle has no sources; it is edited only by anchored edits (`site_identity.py --edits`); React is not reachable from outside it; CSP `script-src 'self'`, `connect-src 'self'`, `img-src 'self' data:`; `.json` static files are 404 | `deploy/websites/README.md`; `deploy/rewards/explorer_api.py:48`; `deploy/explorer/explorer_site.py:50` |
| The bundle already contains 240 Natural Earth admin-0 country paths, pre-projected (equirectangular 1000×500), in the array `Ke` | `assets/index-*.js` |

## 2. Definitions shown to users (exact, at one block height h)

All amounts are integers in wei, carried as decimal strings, formatted with BigInt; 1 PCOL = 10^18 wei.

| Name | Definition |
|---|---|
| Genesis supply | 4,000,000 PCOL (profile rule, §1) |
| Issued | `cumulative(h)` |
| Minted (gross) G | genesis supply + issued |
| Base fees destroyed F | Σ baseFee·gasUsed over blocks 1..h |
| Protocol burned (locked) P | Escrow `burnedTotal` at h; unspendable, still inside Escrow's balance |
| Total burned | F + P (both parts shown) |
| **Current supply S** | G − F − P = Σ balances(h) − P. **The denominator of every percentage.** |
| Bonded | `heldTotal` at h |
| Pending rewards (system) | Escrow balance − heldTotal − burnedTotal (pots, carries and any surplus) |
| Available (an account) | `eth_getBalance(address, h)` |
| Bonded (an account) | the record's w2 when the account is a registry record ID whose record is held (status≠0, tomb=0, bondState∈{1,4}); else 0. Bonds are attributed to the miner ID (the key that controls the record), never also to the reward address |
| Total (an account) | Available + Bonded; for RewardEscrow: its balance − heldTotal − burnedTotal (so nothing is counted twice and Σ Total = S) |
| Circulating | Σ Available of non-system accounts (wallets, miners, reward addresses, contracts) |
| Supply % | Total / S × 100, computed with integers: `sharePercentE10 = floor(Total × 10^12 / S)` (percent × 10^10) |
| Holder | an account with Total > 0 |
| Account types | `system` (0x…c0c001-0x…c0c005, PAY 0x…c0c0ff, SYSTEM 0xff…fe), `contract` (code ≠ 0x), `miner` (a registry record ID), `reward` (a record's reward address that is not itself a record ID), `wallet` (other) |
| Registered miners | every registry record |
| Active miners | status 1 (ActList), tomb 0, bondState ∈ {1,4} |
| Inactive miners | active miners with h+1 ≥ max(lastShare, graceStart) + Θ_inact |
| Exited / punished | status 2 / tomb ≠ 0 |
| Transactions (an account) | top-level transactions sent or received since genesis (internal transfers are not transactions) |
| Blocks mined | blocks whose `miner` is the record ID, since genesis |
| Mining rewards | Σ of executed reward flows to the address, **since the index started** (`rewardsFrom`), null when the index has a gap; never claimed as a total since genesis |

Completeness check (shown, never hidden): the index can discover addresses only from transactions (from, to, created
contracts), the registry (IDs, reward addresses), the genesis founders and the system addresses. An address that only
ever received PCOL by an internal call of a non-system contract cannot be discovered over RPC. Each snapshot therefore
checks Σ discovered balances(h) against G − F and the discovered existing accounts against `pocol_getCaps.accounts`,
and publishes `verification.complete` with any difference (`unlocatedWei`, `accountsMissing`).

## 3. Holders index (`deploy/rewards/pocol_holders.py`)

A sibling of `pocol_transactions.py` in the gateway process: its own SQLite file (`POCOL_HOLDERS_DB`, default
`/var/lib/pocol-explorer-gateway/holders.sqlite3`), WAL, a daemon follower thread, read-only connections per page with
a 2 s progress limit, an identity meta row (refused when another network), the newest 4096 block hashes for
reorganizations, and Windows-safe connection handling.

- **History pass (from genesis, blocks only):** light blocks, full blocks when they carry transactions, receipts for
  contract creations. Per block: discovered addresses (first-seen height), top-level transaction counts per address,
  F contribution, the block's `miner`. Stored with per-block undo rows inside the reorg window.
- **Tip snapshots (inside the 32-block state window):** at a pinned height h (read the head, then every state read
  with the hex height h, then re-read the block hash at h; a change discards the snapshot): balances of the dirty set
  (transaction parties, created contracts, every contract with a balance, every registry ID and reward address, the
  system addresses), all registry records (`pocol_getMiners` bracketed by head-hash reads; graceStart from storage at h),
  `heldTotal`, `burnedTotal`, Escrow balance, `pocol_getCaps` when its `number` is h; then G, F, S and the
  verification. If Σ balances ≠ G − F, every known address is re-read at a new pinned height (full rescan); if it
  still differs, the snapshot is published with `complete:false` and the difference.
- **Rewards:** for each new tip block, `snapshot_v3(height=h)` flows are added per address (`amountWei`) and per miner
  ID (`bondAccruedWei`); `rewardsFrom` is the first height covered; a gap longer than the window sets rewards to null
  from then on until an explicit operator reset (documented).
- **Published snapshots** are materialized rankings (rank, address, totals, type, counters), so pages read by
  `(snapshot, rank)`, `(snapshot, type, rank)` and `(snapshot, address)` indexes with no sort. At least the last
  3 snapshots and at least 3 minutes of snapshots are kept so a paged reader stays consistent; a page pinned to a
  snapshot that is gone or no longer canonical is 409 `reset`.
- **Statistics** (counts, sums, top-N shares, balance buckets, the hourly history point) are computed by the writer at
  publication and stored with the snapshot, so the stats endpoint is a single-row read.
- **Bounds:** catch-up paced; at most one tip snapshot per new block, slower when a snapshot's materialization takes
  longer than a block; reads to the node through `Gateway.query` (loopback) only, batched where pocold allows.

## 4. Geography (`deploy/rewards/pocol_geoip.py`, `deploy/explorer/pocol_geo_reporter.py`)

GeoIP: DB-IP **IP to Country Lite** (CC BY 4.0, attribution "IP Geolocation by DB-IP" with a link to db-ip.com),
installed by the operator as a monthly CSV and converted by `pocol_geoip.py build` into a compact sorted binary file
(IPv4 and IPv6 ranges, two-letter country codes, release month and source SHA-256 in its header); lookups by binary
search on a memory map. No network access at run time; tests use a synthetic CSV. Only public unicast addresses are
looked up (private, loopback, link-local, CGNAT, documentation and reserved ranges are excluded and counted as not
locatable).

Sources, each labelled in the answer:

1. **Observer peers** (always available): the gateway host node's `pocol_status.peers` (in memory only), each IP
   looked up and replaced by `id = HMAC-SHA256(network salt, ip)[:24 hex]` before anything else. These are
   **network nodes, not miners**.
2. **Operator node reports** (only after the owner installs the reporter on a server): `pocol_geo_reporter.py` runs
   on a node host (the three founder servers are the intended hosts), reads its own node's `pocol_status` over
   loopback every 60 s, looks up its peers' countries locally with the same database, replaces every IP by the keyed
   id, and POSTs a signed report (`HMAC-SHA256(reporter key, body)`) to `https://explorer.pocol.net/api/geo-report.php`.
   The report names the node's own mining address (`pocol_status.miner`) when it mines. The gateway places that
   miner in the country of the report's source address (the server's public IP as seen by Caddy, looked up in memory
   and never stored). Raw IPs never leave a reporting server; nothing is written to disk on the gateway.
   Reports older than 180 s are dropped; a report naming a miner that is not a registry record is ignored.

What is known per miner: its registry state (exact, every miner). Its country only when an operator node report for
that miner is fresh; every other registered miner is **Unknown**. "Connected" = miners with a fresh operator report
whose node head is within 6 blocks of the index tip. Peers are never counted as miners, the same miner reported by
several servers counts once (by address), and the same peer seen by several servers counts once (by keyed id).

Per country (located miners only): miners located/active/inactive/connected, nodes, blocks mined by those miners in
the last 8,640 blocks (about 24 h) and their share of all blocks in that window, and their mining rewards since the
index start (null when not exact).

### 4.1 Report ingestion `POST /api/geo-report.php`

Headers `Content-Type: application/json`, `X-PoCol-Reporter: <name>`, `X-PoCol-Signature: <64 hex>`; no `Origin`;
body ≤ 16384 bytes. Body:

```json
{"schema":"pocol-geo-report-v1","reporter":"founder-1","chainId":2656089850,"genesisHash":"0x…","at":1795300000,
 "head":{"height":30000,"hash":"0x…"},"miner":"0x…"|null,
 "peers":[{"id":"<24 hex>","country":"US"|null}], "unlocated":0}
```

Answers: 204 accepted; 400 malformed; 401 unknown reporter or bad signature; 409 stale or replayed (`at` not newer
than the reporter's last, or outside ±120 s); 413; 415; 429. Reporter keys and the network salt are a systemd
credential of the gateway unit (`LoadCredential=geo-reporters:/etc/pocol-explorer-gateway/geo-reporters.json`), never
in the repository. Without the credential the route answers 503 and only observer data is shown.

## 5. Public API (all GET/HEAD, same-origin, through `Relay.forward`'s slots and budget)

Common: `Cache-Control: no-store`, the security headers, `{"data": …}` on 200, `{"error": "…"}` otherwise; 429 with
`Retry-After` when the budget or slots are spent; 503 while an index has no data. Query strings ≤ 4096 bytes, a
repeated key is 400.

### 5.1 `GET /api/holders.php`

Query: `page` (1..10^9, default 1), `size` ∈ {25, 50, 100} (default 25), `order` ∈ {`balance_desc`, `balance_asc`}
(default `balance_desc`), `type` ∈ {`all`, `wallet`, `miner`, `reward`, `contract`, `system`} (default `all`), `q`
(address prefix `0x[0-9a-fA-F]{1,40}`, lower-cased), `height` and `hash` (together: pin the snapshot).

```json
{"schema":"pocol-holders-v1","chainId":2656089850,"genesisHash":"0x…",
 "page":1,"size":25,"order":"balance_desc","type":"all","q":"",
 "total":123,
 "snapshot":{"height":30000,"hash":"0x…","timestamp":1795300000},
 "index":{"state":"ready|syncing|reconnecting","head":30001,"historyHeight":30001,"updatedAt":1795300005},
 "supply":{"currentSupplyWei":"…"},
 "verification":{"complete":true},
 "rows":[{"rank":1,"address":"0x…","type":"reward","label":null,"minerIds":["0x…"],"rewardAddress":null,
          "totalWei":"…","availableWei":"…","bondedWei":"0","sharePercentE10":"…",
          "transactions":12,"blocksMined":null,"rewardsWei":"…"|null}]}
```

`rank` is the account's position among all holders of the snapshot by Total descending (ties: address ascending),
whatever the filter and order. `label` names system accounts (`MinerRegistry`, `RewardEscrow`, `ShareLedger`,
`ChainCaps`, `ChunkFactory`, `PAY`, `SYSTEM`). `minerIds` lists the records a reward address serves;
`rewardAddress` is a miner ID's reward address when it differs. Errors: 400 bad query; 409
`{"error":"The holder snapshot changed. Refreshing.","reset":true}`.

### 5.2 `GET /api/holders-stats.php`

```json
{"schema":"pocol-holders-stats-v1","chainId":…,"genesisHash":"…",
 "snapshot":{…},"index":{…},
 "supply":{"genesisWei":"…","issuedWei":"…","mintedWei":"…","baseFeeBurnedWei":"…","protocolBurnedWei":"…",
           "totalBurnedWei":"…","currentSupplyWei":"…","circulatingWei":"…","availableWei":"…","bondedWei":"…",
           "systemWei":"…","pendingRewardsWei":"…"},
 "verification":{"complete":true,"balancesSumWei":"…","expectedSumWei":"…","unlocatedWei":"0",
                 "accountsIndexed":40,"accountsOnChain":40,"accountsMissing":0},
 "holders":{"count":38,"withAvailable":36,"withBonded":4,"new24h":3|null,"new24hFrom":29000|null},
 "concentration":{"top1":{"totalWei":"…","sharePercentE10":"…"},"top10":{…},"top50":{…},"top100":{…}},
 "distribution":[{"minWei":"0","maxWei":"1000000000000000000","accounts":3,"totalWei":"…"}, …],
 "history":{"from":29000,"points":[{"height":29000,"timestamp":…,"holders":35,"currentSupplyWei":"…",
                                    "top10SharePercentE10":"…"}]}}
```

Concentration and `holders` exclude system accounts (stated in the UI); the denominator is always
`currentSupplyWei`. Distribution buckets by Total in PCOL: [0,1), [1,10), [10,100), [100,1k), [1k,10k), [10k,100k),
[100k,1M), [1M,∞). History points are hourly from the index start (at most 2,000, older ones thinned); nothing
before the index start is shown or estimated.

### 5.3 `GET /api/miner-geography.php`

```json
{"schema":"pocol-miners-geography-v2","chainId":…,"genesisHash":"…","generatedAt":1795300000,
 "height":30000,"blockHash":"0x…",
 "database":{"provider":"DB-IP Lite","release":"2026-11","license":"CC BY 4.0"}|null,
 "miners":{"registered":4,"active":4,"inactive":0,"exited":0,"punished":0,"located":3,"unknown":1,"connected":3,
           "thetaInact":360},
 "nodes":{"observed":5,"located":5,"unlocated":0},
 "sources":[{"kind":"observer","name":"explorer","fresh":true,"reportedAt":…},
            {"kind":"operator","name":"founder-1","fresh":true,"reportedAt":…}],
 "blocksWindow":{"from":21361,"to":30000,"blocks":8640},
 "countries":[{"code":"US","miners":{"located":3,"active":3,"inactive":0,"connected":3},"nodes":5,
               "blocks":6480,"blocksSharePercentE10":"…","rewardsWei":"…"|null}],
 "unknown":{"miners":1,"blocks":2160}}
```

No IP, no keyed id, no miner address, no coordinate finer than a country centroid is ever in a public answer.
The old `/api/miner-locations.php` (schema v1) stays as it is (503), so older clients keep their behaviour.

## 6. Frontend (`assets/pocol-insights.js`, `assets/pocol-insights.css`)

- One hand-written dependency-free ES module and stylesheet, loaded by `index.html` next to the bundle
  (`?v=<12 hex>` content versions kept by `site_identity.py`). They use the explorer's own classes and tokens
  (`.panel`, `.panel-heading`, `.metrics`/`.metric`, `.table-wrap`, `.history-*`, `.badge`, `.secondary`,
  `.text-button`, `--blue #1aa9e4`, DM Sans/Manrope) so the new views look native. No new colours except the
  explorer's existing inactive grey `#8298aa` and neutral greys.
- A small anchored edit list `deploy/websites/tools/explorer-insights-edits.json` changes the bundle only to: add the
  `Holders` tab to the navigation array; give the Holders and Miner map pages their subtitles; render empty mount
  points (`div#pocol-holders`, `div#pocol-miners-map`, and the Overview's `div#pocol-miners-map-compact`) in place of
  the old map component; stop the old v1 map poll; expose the bundle's identity values and country geometry as
  `globalThis.PocolExplorerShared = {chainId, genesisHash, countries}` (variables, not new literals).
- Language: the new views carry an EN | العربية switch (stored in `localStorage` key `pocol-explorer-language`,
  default from `navigator.language`), set `lang`/`dir` on their own containers (RTL for Arabic), and format numbers
  with Western digits. The rest of the explorer stays as it is (English).
- Holders view: the statistics panel, the charts (top-10 vs rest, balance buckets, holders over time when history
  exists), and the table with server-side paging (First/‹/Page x of y/›/Last, rows 25/50/100), order toggle,
  address search, type filter, copy, and opening an address in the explorer's account dialog
  (`#page=Holders&q=<address>`). Auto-refresh every 15 s while visible: page 1 follows the newest snapshot; deeper
  pages keep their pinned snapshot and offer "Newer data available". Shows "Data at block #h · updated …".
- Miners map: the 240 country paths, a choropleth of located miners, glowing circular markers at country centroids,
  distance-based clustering at the current zoom with counts, active (blue) versus inactive (grey) segments, zoom
  (wheel, pinch, buttons, keyboard) and pan, a layer switch (Miners / Network nodes), auto-refresh every 30 s, a
  country panel on click, the statistics cards, a country table with counts and percentages (Unknown included as its
  own row), the data sources and their freshness, and the attribution "Map: Natural Earth · IP Geolocation by DB-IP".
- Charts follow the explorer's palette (blue series on the explorer's greys), thin marks, a legend for two series,
  hover tooltips, and a table view; no simulated data.
- Every answer is validated (schema, chain identity from `PocolExplorerShared`, shapes, integer strings) before it
  is shown.

## 7. Limits stated in the product

1. Miner countries are known only for miners whose node runs an operator reporter. Without reporters, every miner is
   Unknown and the map shows only the network nodes the explorer's own node is connected to.
2. A node's country is the GeoIP country of its public IP (VPN exit, NAT and hosting providers can differ from the
   operator's country).
3. Mining rewards are exact only from the index start; there is no exact total since genesis over RPC.
4. Holder completeness is verified, not guaranteed (§2); a difference is published.
5. No history of balances or holders exists before the index start.
