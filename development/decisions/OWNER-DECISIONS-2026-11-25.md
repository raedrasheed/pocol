# Owner decisions (2026-11-25): the explorer's miners map and its Holders section

Status: **instructions given by the owner on 2026-11-25**, in the working session of that day, for the explorer
feature built on branch `feat/explorer-miners-map-and-holders`. This record states what the owner asked for and how the
branch implements it. It records no approval that the owner has not given: installing anything on a server, the
GeoIP database download, publishing the revised privacy policy and the deployment are listed under "Still the owner's
to decide" below and remain open.

The instructions are numbered D1 to D4 within this record; cite them with the date ("owner instruction 2026-11-25
D2"), since other records number theirs D1 to D7.

## The owner's words (verbatim)

> تحديد الموقع الجغرافي للمعدنين تلقائيًا باستخدام عناوين IP العامة المتاحة للشبكة، بالاستفادة من GeoIP.

> عدم إظهار عناوين IP الخاصة بالمعدنين للمستخدمين.

> الاكتفاء بالدولة أو موقع تقريبي يحافظ على الخصوصية.

> دعم اللغتين العربية والإنجليزية.

> لا أريد تغيير الهوية البصرية الحالية للمستكشف.

Each line is quoted again under its instruction with an English rendering. The owner's request also asked for a
Holders section of PCOL holders in the explorer (D3).

## Instructions

1. **D1: a global map of miners by country, detected automatically with GeoIP.**
   - *Words.* "تحديد الموقع الجغرافي للمعدنين تلقائيًا باستخدام عناوين IP العامة المتاحة للشبكة، بالاستفادة من GeoIP."
     ("Determine the miners' geographic location automatically from the public IP addresses available to the network,
     using GeoIP.")
   - *How the branch implements it.* The explorer's Miner map tab and its Overview card
     (`deploy/websites/explorer.pocol.net/assets/pocol-insights.js`) read `/api/miner-geography.php`
     (`deploy/rewards/pocol_geoip.py`). Countries come from DB-IP's "IP to Country Lite" database looked up locally on
     the gateway; nothing contacts a geolocation service. The design and API contract is
     [MINERS-MAP-HOLDERS.md](../explorer/MINERS-MAP-HOLDERS.md).
   - *What the code found, and what it means for "automatically".* No protocol message ties a peer's IP address to a
     miner: a node's handshake carries no identity, shares and templates are relayed, and miners mine in their own node
     (MINERS-MAP-HOLDERS.md §1). The IP addresses the network makes available therefore locate **network nodes**, not
     miners, and the explorer never presents a peer as a miner. A miner is located automatically only when its own
     node runs the operator reporter (`deploy/explorer/pocol_geo_reporter.py`), which names the node's miner
     (`pocol_status.miner`, the registry record ID) in a signed report; the gateway places that miner in the country of
     the report's source address. Every other registered miner is shown as **Unknown**, and the map says so. Without a
     reporter, the map shows every miner as Unknown and the network nodes that the explorer's own node is connected to.
2. **D2: no IP address is shown; country level only.**
   - *Words.* "عدم إظهار عناوين IP الخاصة بالمعدنين للمستخدمين." ("Do not show the miners' IP addresses to users.")
     and "الاكتفاء بالدولة أو موقع تقريبي يحافظ على الخصوصية." ("Be content with the country, or an approximate
     location that preserves privacy.")
   - *How the branch implements it.* The country is the only location: no city, region or coordinate finer than a
     country centroid. Each address is replaced in memory, at once, by a keyed id (`HMAC-SHA256(network salt, IP)`,
     first 24 hex digits) and its country; no IP address, keyed id or miner address is in any public answer
     (`deploy/rewards/explorer_api.py` checks the contract's exact keys), and none is written to disk. A reporter sends
     its node's head, its node's own mining address (the registry record ID) and its peers' keyed ids and countries,
     never an IP address; the gateway counts that mining address in the country of the report's source address, and
     only per-country counts are published. Visitors' addresses are never looked up. The privacy policy states this
     (`deploy/websites/pocol.net/privacy.html` and `privacy.js`, revised for publication by the owner, D4 below).
3. **D3: a Holders section.**
   - *How the branch implements it.* The Holders tab (`/api/holders.php`, `/api/holders-stats.php`;
     `deploy/rewards/pocol_holders.py`) ranks every discovered account by its total (available + bonded) at one block,
     with the supply, holder counts, concentration, balance ranges and an hourly history from the index start. Every
     percentage uses the exact current supply; completeness is verified against the chain's supply identity and any
     difference is published, never estimated; mining rewards are shown only since the index start
     (`deploy/explorer/README.md`, Holders index).
4. **D4: Arabic and English, and the explorer's current visual identity kept.**
   - *Words.* "دعم اللغتين العربية والإنجليزية." ("Support both Arabic and English.") and "لا أريد تغيير الهوية البصرية
     الحالية للمستكشف." ("I do not want to change the explorer's current visual identity.")
   - *How the branch implements it.* The Holders and Miners map views carry an English | العربية switch (right to left
     in Arabic, Western digits); the rest of the explorer bundle stays English, as decided for the launch on
     2026-11-16 (`deploy/websites/README.md`, Launch decisions), so this is an exception for these views only. The views
     use the explorer's own classes, fonts and colours (`--blue #1aa9e4`, the existing inactive grey `#8298aa`, neutral
     greys) and add no new colour; the bundle is changed only by five anchored edits
     (`deploy/websites/tools/explorer-insights-edits.json`).

## Still the owner's to decide or approve

None of these has been approved; each needs the owner's own decision and steps.

1. **The operator reporter on the three founder servers.** Services on the founder servers are changed only with the
   owner's approval ([GENESIS-RECORD-2026-11-21.md](../public-network/GENESIS-RECORD-2026-11-21.md)). Installing
   `deploy/explorer/install_geo_reporter.py` there (a timer every 60 s, one loopback read of the node and one HTTPS
   report per run), the reporters' names, and the keys and network salt the owner creates on the hosts (never in the
   repository) are the owner's decision. Until then every miner on the map is Unknown.
2. **The GeoIP database and its attribution.** Downloading DB-IP's "IP to Country Lite" monthly file on the gateway
   host, and its licence terms: CC BY 4.0 with the attribution "IP Geolocation by DB-IP" linked to https://db-ip.com,
   which the map shows whenever a database is installed. Without the file nothing is located.
3. **Publishing the revised privacy policy.** `pocol.net/privacy.html` gains one item, in English and Arabic, dated
   25 November 2026, describing the country lookup; the owner reviews the text before the page is published (the rule
   of the 2026-11-22 policy).
4. **The deployment.** Updating the explorer gateway host (the gateway with the holders index and the miner map, the
   new explorer files, and the GeoIP database), following
   [DEPLOY-INSIGHTS.md](../../deploy/explorer/DEPLOY-INSIGHTS.md), with its preflight, backup, checks and rollback.
5. **The Arabic in the explorer.** D4 asks for Arabic and English in these sections; the 2026-11-16 launch decision
   kept the explorer English. The branch applies Arabic to the two new views only; extending it to the whole explorer
   would be a separate decision.

## Where it stands (2026-11-25)

D1 to D4 are implemented on the branch and tested on a fake chain, on a local devnet
(`deploy/explorer/e2e_explorer_devnet.py`, `deploy/websites/tests/explorer_smoke.mjs`) and in a browser against fixtures;
nothing was installed, downloaded, published or deployed. The stated limits are in the findings ledger
([FINDINGS.md](../public-network/FINDINGS.md), F-101 to F-105).
