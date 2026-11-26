# Deploying the Holders section and the miners map on the explorer gateway host

Arabic: [DEPLOY-INSIGHTS-AR.md](DEPLOY-INSIGHTS-AR.md). Design and API contract:
[MINERS-MAP-HOLDERS.md](../../development/explorer/MINERS-MAP-HOLDERS.md). Behaviour and limits: [README.md](README.md)
(Holders index, Miner map, Operator node reporter).

**Status: a plan; nothing here has been run on a host.** The deployment, the GeoIP database download and the operator
reporters each need the owner's approval first
([OWNER-DECISIONS-2026-11-25.md](../../development/decisions/OWNER-DECISIONS-2026-11-25.md), "Still the owner's to
decide"). Every command below runs on the existing gateway host (the one serving `explorer.pocol.net`, installed by
`install_explorer_gateway.py` from main at 04f77806), as a user with sudo, from a checkout of the merged release
commit. Nothing touches the founder servers, PoCol Mainnet's consensus, the node's data, keys or DNS; the gateway
only reads the host's own node on `127.0.0.1:8545`.

## Release plan for PR #120

The sections below (0 to 7) are the per-host steps; this plan orders them into one production release, with a go/no-go
gate and a rollback for every stage. Each stage runs only with the owner's explicit approval; nothing here has been run
on a production host. "Not measured" means exactly that: no duration below is a guess.

What production serves today (measured 2026-11-26):

- `explorer.pocol.net` is served directly by Caddy on the gateway host (no Cloudflare): `/`, the bundle
  `assets/index-e46a96e9d375.js` and `/api/health.php` answer with `cache-control: no-store`, so no cache purge is
  ever needed there. `https://explorer.pocol.net/SHA256SUMS.txt` is 404 (the served copy never includes it; the
  installer checks that 404).
- `pocol.net` is behind Cloudflare; `privacy.html` and `SHA256SUMS.txt` are served with `cf-cache-status: DYNAMIC`
  (not cached). Production's `pocol.net/SHA256SUMS.txt` is byte for byte the file of commit f8379198 (PR #119), SHA-256
  `20b3783e841dfc30f9a604718bd1fce6f9c280aed04c82392354778cc120f08b`: it still lists the first extension package. The
  four files now published under `pocol.net/downloads` are the re-signed package of PR #121 that main lists. Every other
  file main's `SHA256SUMS.txt` lists matches production byte for byte, except `api/network.php` and `api/reward.php`
  (run by PHP, not comparable) and `chrome-extension.html`, whose served bytes differ only because Cloudflare's Email
  Address Obfuscation rewrites `release@pocol.net` into `/cdn-cgi/l/email-protection` links and injects
  `/cdn-cgi/scripts/.../email-decode.min.js` (a Cloudflare setting, not the origin file; this release does not change
  it).
- The gateway was installed by `install_explorer_gateway.py` from main at 04f77806. PRs #119 and #121 changed nothing
  under `deploy/explorer`, `deploy/rewards` or the explorer's site files, so main at 4b17da2c has the same installer, the
  same unit and the same explorer copy, `site-1c9ec5374487` (12 files; computed from the commit, to confirm on the host
  with section 0's `grep -n 'root \*' /etc/caddy/Caddyfile`).

Order: stage 0 (independent), then, after PR #120 is merged, stage 1 (the privacy page), stage 2 (the explorer
gateway), and the optional stages 3 and 4. The privacy page goes first because the new gateway reads the public IP
addresses of its node's peers (below), which the live policy of 22 November does not describe.

### Stage 0: `pocol.net/SHA256SUMS.txt` to main's file (independent of PR #120)

This stage does not depend on PR #120 and can run before it is merged, once the owner approves this upload (no item of
OWNER-DECISIONS-2026-11-25.md covers it: record the approval with its date). Only the four download lines of PR #121
change:

| File | SHA-256 |
|---|---|
| production `pocol.net/SHA256SUMS.txt` (= f8379198) | `20b3783e841dfc30f9a604718bd1fce6f9c280aed04c82392354778cc120f08b` |
| main's `deploy/websites/pocol.net/SHA256SUMS.txt` (4b17da2c) | `14c386700ab2ac8d4edde416bbd04fa8717fcacbf9944d2782de9cc524842e72` |

| Line | Production | Main |
|---|---|---|
| `downloads/PoCol-Studio-IDE-Viewer-0.5.0.zip` | `f947c019...` | `6e38e72f331a52ca4de0cc701aad9e026ea0e08422468041fbc5bb5c24b71070` |
| `downloads/PoCol-Studio-IDE-Viewer-0.5.0.zip.sig` | `829e7bdc...` | `85f692b2316c09c6b11fe9591e65bebb2868aa821eefed3d9c2f1bf4a9042bdf` |
| `downloads/SHA256SUMS` | `470c39c9...` | `352e2231017abfdf30f0bf5558dcf71ed142f78c2f876aefb54bf5c59bc8f6eb` |
| `downloads/SHA256SUMS.sig` | `6cda09ce...` | `4cf40dca808b8efdc468dff3d2ed025115a8e7ea1e67c309d2efac2ed313d8fb` |

The four main values are the files now published under `pocol.net/downloads` (checked with `verify_release.py` against
`allowed_signers`, key `SHA256:r/IMxkMr+wv3OyAbiX3XYhSV4pqy3Lv5/z/VV06DJyc`).

1. On a computer with the repository: `git show 4b17da2c:deploy/websites/pocol.net/SHA256SUMS.txt > SHA256SUMS.txt`,
   then `sha256sum SHA256SUMS.txt` must print `14c38670...2e72` as above. Do not open or save the file in an editor (its
   line endings must stay LF).
2. cPanel > File Manager (Settings > "Show Hidden Files"): copy the Document Root's `SHA256SUMS.txt` into
   `/pocol-backups` and rename the copy `SHA256SUMS.txt.before` (create the folder `pocol-backups` in your home folder if it is not there),
   outside the Document Root as in step 2 of `DEPLOY-pocol.net-EN.md`, so it is never served and `host_files.py` does
   not list it as `DELETE`.
3. Upload the `SHA256SUMS.txt` of 1 into the Document Root, allowing it to overwrite. Nothing else changes.

**Gate:** from another machine `curl -s https://pocol.net/SHA256SUMS.txt | sha256sum` prints `14c38670...2e72` (the
file is DYNAMIC: no purge is needed). In cPanel > Terminal, step 7.1 of `DEPLOY-pocol.net-EN.md`
(`grep -v '  \.htaccess$' SHA256SUMS.txt | sha256sum -c --quiet - && echo "FILES OK"` in the Document Root) is expected to
print `FILES OK`; it has not been run on the host.

**Rollback:** delete the new `SHA256SUMS.txt` from the Document Root, copy `/pocol-backups/SHA256SUMS.txt.before` into
the Document Root and rename the copy `SHA256SUMS.txt`; the same curl then prints `20b3783e...f08b`. This puts back a
file that lists the withdrawn first extension package, which no longer matches the published downloads.

### Before stage 1: preconditions

1. The owner has merged PR #120 into main **with a merge commit** (called *the merge commit* below; its 12 hex name the
   package artifact of stage 1).
2. CI is green on the merge commit: every workflow its push to main runs, among them "Websites", "Consensus rules,
   profiles and extension release" (its job "Extension release (Studio, IDE, Viewer)") and "pocol.net cPanel package"
   (whose run produces stage 1's artifact).
3. The owner's approvals, each recorded with its date (the owner is asked to approve each stage together with its
   rollback, so that a no-go is rolled back at once and then reported):
   - the privacy policy text of stage 1, reviewed before it is published, and its publication on pocol.net
     (OWNER-DECISIONS-2026-11-25.md, "Still the owner's to decide", item 3, and this pocol.net deployment);
   - the gateway deployment of stage 2 (item 4);
   - every restart of `pocol-explorer-gateway.service` and every reload of Caddy: each installer run (stage 2, stage 3,
     stage 4's gateway step, the monthly GeoIP update, the recovery in "A rollback after a later stage"), the automatic
     restore of 6.1, the manual rollback of 6.2, and the stop and start of stage 3's rollback;
   - optional, for stage 3: the GeoIP database download and its CC BY 4.0 attribution (item 2);
   - optional, for stage 4: the reporter on each founder server, one approval per server (item 1).
4. On the gateway host, a checkout of the merge commit (`git rev-parse HEAD` in section 0 prints it), with no local
   change.

### Stage 1: the pocol.net privacy page

Must be live before stage 2: the new gateway reads its node's peer list and replaces each public peer IP address in
memory by a keyed code (`pocol_geoip.observe_peers`, a random salt, at most every 10 s while the map is read), even
without a GeoIP database; the revised policy is the one that describes this, and the country lookup of stage 3. Until
stage 3 the policy describes a lookup the gateway does not make yet (without a database nothing is located): it never
describes less than the gateway does. PR #120 changes on pocol.net only these three files; with stage 0 done, they are
the only files that differ from production:

| File | Production (= main) | New (PR #120) |
|---|---|---|
| `privacy.html` | `370e76bfc7c359c8bf0bf9926fa80106231a2785076a35621bbdf3bf70d2dbce` | `25c1a2731a1ebe1651126b3742d9fb905810773748a4f3fa42e0c6ce0d13c63e` |
| `privacy.js` | `e12dff2468bdbe8e9ff9695e244761e6cac29d8b29b3f5ce8a08a1ff330aaa94` | `c490c987472c971a8d124c396a037aeecb91806be4af3ad6d2705a5948ffd8ed` |
| `SHA256SUMS.txt` | `14c386700ab2ac8d4edde416bbd04fa8717fcacbf9944d2782de9cc524842e72` (after stage 0; `20b3783e...f08b` without it) | `b867e95a0ba5fe2c72a6e642f60d9377720dd0a0e8df0d44e9e5939f48f84a1c` (at decdb5d9; the merge commit's is the same while the site is unchanged) |

The new `SHA256SUMS.txt` differs from main's on the two privacy lines only (it already carries PR #121's four download
lines, so without stage 0 this stage makes stage 0's change too).

1. GitHub > Actions > "pocol.net cPanel package", the run on main for the merge commit (push event) > Summary >
   Artifacts: download `pocol.net-cpanel-<12 hex of the merge commit>` and extract it once on your computer. The
   artifact is kept 30 days (`retention-days: 30`): download it within 30 days of the merge; if it has expired, ask the
   owner before any `workflow_dispatch` (a new run is a new build). It holds
   `PoCol-pocol.net-site-<commit date>-<12 hex>.zip` and its `.sha256`, `pocol.net.htaccess`, `MANIFEST.json`,
   `PACKAGE-SHA256SUMS.txt` and the filled `DEPLOY-pocol.net-EN.md` and `-AR.md`. The same build at decdb5d9 gave
   `PoCol-pocol.net-site-2026-11-26-decdb5d96d60.zip`: its `SHA256SUMS.txt` lists 58 files, the 45 other files of the
   ZIP, `.htaccess` (shipped beside the ZIP as `pocol.net.htaccess`) and 12 downloads the ZIP does not contain; all 45
   and the `.htaccess` matched, and the 12 download lines matched the published downloads.
2. Follow the package's `DEPLOY-pocol.net-EN.md` (or `-AR.md`) from step 0 to step 9: check the package, back up
   (record the `STAMP`), upload, extract, merge `.htaccess`, permissions, step 7 until `host_files.py` prints
   `OK pocol.net`. `host_files.py` has not been run against the production host: its list, not this plan, says what
   to delete or upload again.

Besides the three files above, the guide's steps rewrite every file of the ZIP (with stage 0 done, the other 44 with
the bytes production already serves), may change `.htaccess` (step 5), set permissions (step 6) and delete the files
`host_files.py` marks `DELETE` (step 7). Write down what steps 5 to 7 changed: the rollback depends on it.

**Gate:** step 9 of that guide, and from another machine `curl -s https://pocol.net/privacy.html | sha256sum`,
`curl -s 'https://pocol.net/privacy.js?v=c490c987472c' | sha256sum` and `curl -s https://pocol.net/SHA256SUMS.txt |
sha256sum` print the three new values above; the page shows "Last updated: 25 November 2026" in English and «آخر
تحديث: 25 نوفمبر 2026» in Arabic, and the Miners map item in both. `chrome-extension.html` still differs over HTTP only
by Cloudflare's email rewriting (above): not a failure of this stage.

**Cloudflare:** `privacy.html` and `SHA256SUMS.txt` are DYNAMIC (not cached), and `privacy.html` links
`privacy.js?v=c490c987472c` (a content version), so no purge is needed. If a check still shows the old bytes, purge those
URLs in the Cloudflare dashboard (the pocol.net zone > Caching > Configuration > Purge Cache > Custom Purge):
`https://pocol.net/privacy.html`, `https://pocol.net/privacy.js?v=c490c987472c` and `https://pocol.net/SHA256SUMS.txt`.

**Rollback:** roll back stage 4, then stage 3, then stage 2 first if they are installed: the published policy must
describe what the gateway does. Then step 10 of `DEPLOY-pocol.net-EN.md` with the `STAMP` of its step 2. Only when step
7 deleted nothing (no `DELETE`) and step 5 left `.htaccess` byte for byte as it was, putting back the three previous
files is enough instead (`privacy.html` `370e76bf...`, `privacy.js` `e12dff24...`, `SHA256SUMS.txt` `14c38670...`, or
`20b3783e...f08b` if stage 0 was not done; from the backup of step 2, or from main 4b17da2c), checked with the same curl
lines.

### Stage 2: the explorer gateway, with neither a GeoIP database nor reporters

Only once stage 1 is live. Sections 0, 2, 3 and 4, **without `--geoip-db`** (section 1 is skipped):

```sh
D=$(mktemp -d) && python3 deploy/websites/tools/site_snapshot.py --out "$D" \
  && sudo python3 deploy/explorer/install_explorer_gateway.py --site "$D" --check
sudo python3 deploy/explorer/install_explorer_gateway.py --site "$D"
```

The new copy has 14 files: against main it replaces `assets/index-e46a96e9d375.js` with `assets/index-0ce9ca52c312.js`
(`0ce9ca52c312c1fa542d62dde06b6f0017022f90c1e1c60f681104467defff35`), adds `assets/pocol-insights.js`
(`e37a622126508836a62d4b4fe6373ac0716ef46064ae81a4b4c83622c04c7efa`) and `assets/pocol-insights.css`
(`dca8ac3be3dc45393b1c125ed487a44ec8be1c9738b1d08b594fb988ab3b2280`), and changes `index.html` (`89335937...` ->
`4b85f4b76b19d87dfd0d6e40a5ddf7bf4eb6a9b38c53f6874efaf571892dce86`). A snapshot built with `site_snapshot.py` at
decdb5d9 (PR #120's head) matched all 14 lines of `deploy/websites/explorer.pocol.net/SHA256SUMS.txt`; its site id is
`site-c29adcf97b30`, and the merge commit gives the same id as long as those 14 files are unchanged.

**Go/no-go gate** (no-go on any failure: roll back):

- Section 2 prints `CHECK PASSED: network <chain ID> <genesis hash> on the node, site-c29adcf97b30 (14 files, this
  checkout's snapshot), ...; the GeoIP database not installed (nothing is located), operator node reports disabled (no
  credential).`
- Section 3 prints `INSTALLED: ...`, `PASS: ...`, `Miner map: the GeoIP database not installed (nothing is located),
  operator node reports disabled (no credential).` and `Backup: ...` (write that path down: it is this stage's
  rollback).
- The checks of section 4, exactly: `/api/health.php` `"result":"PASS"`; `/api/holders.php` the 503 "PCOL holders are
  unavailable: ..." until the first snapshot, then `"state":"syncing"` and `"ready"`; `/api/holders-stats.php` and
  `/api/miner-geography.php` answering for the network (or, like `holders.php`, their JSON 503 until the first
  snapshot: not a no-go); `POST /api/geo-report.php` 503; once the first snapshot is there, in
  `/api/miner-geography.php` `miners.registered` equal to the registry's records, `located` 0, `unknown` equal to
  `registered` and `database` null; no IP address, keyed id or miner address in any answer; the browser checks in
  English and Arabic; the journal.
- The views that were there before (Overview, apart from its map card, and Transactions) look and work as before.

The holders index's first full pass from genesis has not been measured on PoCol Mainnet: stage 2 is complete once
`/api/holders.php` is `ready` and `holders-stats.php` shows `verification.complete` true (section 4), then watch it as in
section 5.

**Expected visible change:** a new Holders tab (one new item in the explorer's sidebar menu); the existing Miner map tab
and the Overview's map card now show the new map, which shows every registered miner as Unknown and says that no GeoIP
database is installed (no DB-IP attribution without a database).

**Rollback:** 6.1 (a failed install restores itself) or 6.2 with this stage's backup.

**Before stage 3 or 4:** keep a copy of the explorer copy that 6.2 with this stage's backup puts back, outside
`/srv/pocol-explorer` (the installer's pruning removes only `site-*` directories there):

```sh
sudo cp -a /srv/pocol-explorer/site-1c9ec5374487 /var/backups/pocol-explorer-gateway/site-1c9ec5374487.keep
```

### Stage 3 (optional): the GeoIP database

Only after the owner approves the download and the CC BY 4.0 attribution, and once stage 1 is live. Section 1 (download
and build), then sections 2 to 4 **with** `--geoip-db ~/geoip/dbip-country-YYYY-MM.bin`, from the same checkout of the
merge commit. The site copy and the unit are unchanged (same id; the installer reuses the copy); the installer adds
`/opt/pocol-explorer-gateway/geoip/dbip-country.bin`, restarts the gateway, which reads the holders index file it
already has, and reloads Caddy.

**Gate:** section 1's `BUILT ...` line with the SHA-256 recorded at download; section 2's `CHECK PASSED` naming the
database (release YYYY-MM, N IPv4 and M IPv6 ranges); section 3's `Miner map:` line naming it; section 4 with
`database.release` the month built, the map showing "IP Geolocation by DB-IP" linked to https://db-ip.com, and the
miners still Unknown (`located` 0 without a reporter).

**Rollback:** the installer has no option that removes a GeoIP database: an install without `--geoip-db` keeps the
installed one (`Miner map: the GeoIP database kept`). The gateway opens the file only when it starts
(`gateway.py`: `pocol_geoip.load_database` returns None when no file is there), so:

```sh
sudo systemctl stop pocol-explorer-gateway.service
sudo rm -f /opt/pocol-explorer-gateway/geoip/dbip-country.bin
sudo rmdir /opt/pocol-explorer-gateway/geoip 2>/dev/null || true               # only if it is now empty
sudo systemctl start pocol-explorer-gateway.service
```

After that `/api/miner-geography.php` answers `database` null, the map says no GeoIP database is installed and drops the
attribution, and a later install without `--geoip-db` prints `not installed (nothing is located)`. The holders index,
the unit, the Caddyfile and the site copy are not touched. (6.2 with this stage's backup would remove the database too,
since its backup lists the file with no earlier copy, but its last steps also delete the holders index file, which then
reads the chain from genesis again.) To go back from a monthly update to the previous month instead, run sections 2 to 4
with the previous month's `--geoip-db` file.

### Stage 4 (optional, per founder server): operator reporters

Only after stage 3 (the reporter is given that database: `install_geo_reporter.py` refuses to run without
`--database`) and with stage 1 live. Section 7, with the owner's approval for each founder server; the gateway part
(section 7, step 2) reruns sections 2 to 4 (without `--geoip-db`, which keeps the database). **Gate:** section 7, step
4, for that server. **Rollback:** section 7, step 5 (on the gateway, remove `geo-reporters.json` and rerun the
installer; never remove the file alone). Then, on the founder server and with the owner, remove
`/etc/pocol-geo-reporter/credential.json` (which `--uninstall` keeps; and the folder if it is now empty), the copied
`~/geoip/dbip-country-YYYY-MM.bin` and the installer's backups `/var/backups/pocol-geo-reporter`; on the gateway, check
that `/etc/systemd/system/pocol-explorer-gateway.service.d/geo-reporters.conf` is gone and that
`POST /api/geo-report.php` answers 503 again.

### A rollback after a later stage

Roll back in reverse order (stage 4, then 3, then 2, then 1). Every installer run after stage 2 (stage 3, stage 4's
gateway step, a monthly update) keeps only the new copy and the one the Caddyfile served before it, so it removes
`/srv/pocol-explorer/site-1c9ec5374487`, the copy that 6.2 with stage 2's backup puts back into the Caddyfile. Before
6.2's Caddy reload (its `sudo caddy validate ... && sudo systemctl reload caddy` line), put the copy kept before stage 3
or 4 back:

```sh
sudo cp -a /var/backups/pocol-explorer-gateway/site-1c9ec5374487.keep /srv/pocol-explorer/site-1c9ec5374487
```

Without that kept copy, after 6.2 the explorer's static files answer 404 until the copy is back: from a checkout of main
at 4b17da2c (whose installer accepts the 04f77806 unit that 6.2 restored), run the commands of sections 2 and 3 without
`--geoip-db`; they rebuild `site-1c9ec5374487` and check it over HTTPS. That path follows from the code and has not been
tested.

### Summary

| Stage | Changes | Who runs it | How long | Gate | Rollback |
|---|---|---|---|---|---|
| 0 | `pocol.net/SHA256SUMS.txt` (4 download lines) | the owner, cPanel File Manager | not measured (one file upload) | curl SHA-256 `14c38670...2e72` | `SHA256SUMS.txt.before` back (`20b3783e...f08b`) |
| 1 | on pocol.net `privacy.html`, `privacy.js`, `SHA256SUMS.txt`; the other ZIP files rewritten, `.htaccess` (step 5), permissions, the `DELETE` files of step 7 | the owner, cPanel | not measured | step 9 of the cPanel guide, the three SHA-256s | step 10 of the cPanel guide (the three previous files only if step 7 deleted nothing and `.htaccess` is unchanged) |
| 2 | gateway sources and unit, explorer copy `site-c29adcf97b30`, Caddy site root, holders index file; gateway restart, Caddy reload | an operator with sudo on the gateway host | not measured; the installer's loopback check retries for about 10 s (20 attempts 0.5 s apart) and its HTTPS check up to `--https-wait` (default 600 s); the holders index's first full pass not measured | section 2 and 3 output, section 4 | 6.1 / 6.2 |
| 3 (optional) | `/opt/pocol-explorer-gateway/geoip/dbip-country.bin`; gateway restart, Caddy reload | an operator with sudo on the gateway host | GeoIP build not measured; install as stage 2 | section 1 `BUILT`, sections 2 to 4 with the database | stop, remove the file, start (above) |
| 4 (optional) | per founder server: reporter service and timer, credential file; on the gateway `geo-reporters.json` and the unit drop-in; gateway restart, Caddy reload | the owner, per server; an operator on the gateway | not measured (the timer runs every 60 s) | section 7, step 4 | section 7, step 5, then the credential file and the copied database |

### Never in this release

- No change to consensus, the genesis or PoCol Mainnet's configuration (`network.json`, `forks.json`, chain ID, genesis
  hash).
- No node restart: the gateway only reads the host's node; `pocold` is never stopped or restarted.
- No change on a miner or a founder server outside stage 4.
- No change to the visual identity: the logo, `#1AA9E4`, the fonts and the existing menu items stay as they are; the
  only addition is the explorer's Holders item (owner instruction 2026-11-25 D3), in the existing style (D4).
- No rebuild or re-signing of a release package: the published downloads stay byte for byte.
- No DNS change.

## What the update changes on the host

| Where | Change |
|---|---|
| `/opt/pocol-explorer-gateway/` | `gateway.py`, `explorer_api.py` updated; `pocol_holders.py` and `pocol_geoip.py` added (the installer writes all six sources) |
| `/opt/pocol-explorer-gateway/geoip/dbip-country.bin` | the GeoIP database, only with `--geoip-db` |
| `/etc/systemd/system/pocol-explorer-gateway.service` | adds `Environment=POCOL_HOLDERS_DB=/var/lib/pocol-explorer-gateway/holders.sqlite3` and the new description; everything else, `MemoryMax=256M` included, is unchanged |
| `pocol-explorer-gateway.service.d/geo-reporters.conf` | only while `/etc/pocol-explorer-gateway/geo-reporters.json` exists (operator reports; not part of this deployment) |
| `/srv/pocol-explorer/site-<id>` | a new static copy, 14 files (new bundle, `assets/pocol-insights.js` and `.css`); the previous copy is kept for a rollback until the next installer run, which removes it (see "A rollback after a later stage") |
| `/etc/caddy/Caddyfile` | the marked block's site root only: `/api/*` already goes to the gateway, inside the 16 KiB body guard |
| `/var/lib/private/pocol-explorer-gateway/holders.sqlite3` (and `-wal`, `-shm`) | created by the service at its first start |

The new routes are `/api/holders.php`, `/api/holders-stats.php`, `/api/miner-geography.php` (GET, HEAD) and
`POST /api/geo-report.php` (503 until operator reports are enabled).

## 0. Record the current state (for the rollback)

```sh
git rev-parse HEAD                                   # the release commit of this checkout
sudo systemctl status pocol-explorer-gateway.service --no-pager
sudo cat /etc/systemd/system/pocol-explorer-gateway.service
grep -n 'root \*' /etc/caddy/Caddyfile               # the current site copy, /srv/pocol-explorer/site-<old id>
ls /srv/pocol-explorer/
curl -s --max-time 20 https://explorer.pocol.net/api/health.php   # "result":"PASS"
df -h / /var/lib /srv /opt; free -m
systemctl show pocol-explorer-gateway.service -p MemoryCurrent -p CPUUsageNSec
ps -o pid,rss,pcpu,etime,cmd -C pocold               # the observer node's memory and CPU before the update
```

Keep this output with the date. The gateway's memory and CPU and the node's before the update are the baseline for
step 5.

## 1. The GeoIP database (optional; only after the owner approves the download and its attribution)

Without it the update still works: nothing is located and the map says that no GeoIP database is installed.

1. On a machine of the owner's choice, download DB-IP's "IP to Country Lite" CSV file of the current month from
   DB-IP's download page (https://db-ip.com/db/download/ip-to-country-lite). Its licence is CC BY 4.0: the map shows
   "IP Geolocation by DB-IP" with a link to https://db-ip.com whenever a database is installed. Record the file's
   SHA-256 (`sha256sum`).
2. Copy it to the gateway host and build the binary database from the release checkout (no network access; the file
   is checked row by row, and `build` verifies the result):

   ```sh
   install -d -m 0700 ~/geoip
   python3 -I -B deploy/rewards/pocol_geoip.py build --input ~/geoip/dbip-country-lite-YYYY-MM.csv.gz \
       --release YYYY-MM --output ~/geoip/dbip-country-YYYY-MM.bin
   python3 -I -B deploy/rewards/pocol_geoip.py info ~/geoip/dbip-country-YYYY-MM.bin
   ```

   `build` prints `BUILT ...: release YYYY-MM, N IPv4 and M IPv6 ranges, source SHA-256 ...`; that SHA-256 must be the
   one recorded in 1. `info` prints the same header as JSON.

**Monthly update.** Repeat 1 and 2 with the new month's file, then steps 2 to 4 below with the new
`--geoip-db` (the installer checks the database record by record, replaces it and restarts the gateway). Without
`--geoip-db` an update keeps the installed database. The answer's `database.release` names the month in use.

## 2. Preflight (changes nothing)

```sh
D=$(mktemp -d) && python3 deploy/websites/tools/site_snapshot.py --out "$D" \
  && sudo python3 deploy/explorer/install_explorer_gateway.py --site "$D" --geoip-db ~/geoip/dbip-country-YYYY-MM.bin --check
```

(Leave out `--geoip-db` without a database.) It must print `CHECK PASSED: network <chain ID>
<genesis hash> on the node, site-<new id> (14 files, this
checkout's snapshot), the loopback port, and the candidate Caddy configuration; the GeoIP database ... (release
YYYY-MM, N IPv4 and M IPv6 ranges), operator node reports disabled (no credential).` followed by the copy's manifest. The chain ID and genesis hash are the ones `deploy/websites/site-identity.json` declares.
It refuses, with no change made: a unit with custom edits (it replaces only the units of this release, of 04f77806 and
of 1bb11f4c), an unrecognized `/opt/pocol-explorer-gateway`, an edited Caddy block, a database that fails its checks,
a node of another network, and a copy that is not this checkout's snapshot.

## 3. Install

```sh
sudo python3 deploy/explorer/install_explorer_gateway.py --site "$D" --geoip-db ~/geoip/dbip-country-YYYY-MM.bin
```

It backs up what it replaces under `/var/backups/pocol-explorer-gateway/<date>-<time>-<pid>/` (`Caddyfile.before`,
`file-<n>.before`, `state.json` listing the paths, the previous and the new site), writes the sources, the database and
the unit, restarts the gateway and checks it on loopback, writes and reloads the Caddyfile, then checks over HTTPS with
the public name and trusted certificates: every static file, the headers, `/api/health.php` PASS, `/api/rpc.php` and
`/rpc` naming the network, `/health`, `/api/holders.php` and `/api/miner-geography.php` (their data for the network, or
their JSON 503 while the holders index has no snapshot yet), hidden paths 404, and the Studio and faucet routes when
they are installed. It
prints `INSTALLED: ...`, `PASS: ...`, `Miner map: ...` and `Backup: /var/backups/pocol-explorer-gateway/...`. **Write
down that backup path.** On any failure it restores the previous Caddyfile, files, unit and service state by itself
(`INSTALL FAILED: ...`); if a restore step fails it prints `ROLLBACK INCOMPLETE` with the steps and the backup: then
follow 6.2.

## 4. Verify

From another machine:

```sh
curl -s --max-time 20 https://explorer.pocol.net/api/health.php                       # "result":"PASS"
curl -s --max-time 20 'https://explorer.pocol.net/api/holders.php?size=25' | head -c 600
curl -s --max-time 20 https://explorer.pocol.net/api/holders-stats.php | head -c 900
curl -s --max-time 20 https://explorer.pocol.net/api/miner-geography.php | head -c 900
curl -s -o /dev/null -w '%{http_code}\n' -X POST -H 'Content-Type: application/json' --data '{}' \
  https://explorer.pocol.net/api/geo-report.php                                       # 503: reports not enabled
```

- Right after the install the holders index reads the chain from genesis: `/api/holders.php` answers 503 "PCOL holders
  are unavailable: the holders index has no snapshot yet or is not installed." until its first snapshot, then
  `"state":"syncing"` and `"ready"`. The history pass is paced (5 ms per block plus the read); its duration on PoCol
  Mainnet has not been measured.
- Once ready: `"chainId"` the declared chain ID, the snapshot height close to the node's head, and in the statistics
  `verification.complete` true with `unlocatedWei` "0". PoCol Mainnet's only genesis accounts are the four founders,
  which the registry names, so a difference means an account the index cannot discover (F-104): it is published, not
  hidden; note it with its height.
- `/api/miner-geography.php`: `miners.registered` equal to the registry's records, `located` 0 and `unknown` equal to
  `registered` (no reporter), the source `explorer` fresh, and `database.release` the month built in step 1 (or
  `database` null without one). No IP address, keyed id or miner address appears in the answer.
- In a browser: https://explorer.pocol.net, the Holders tab (rows, paging, the statistics), the Miner map tab (the map,
  the Unknown row, the attribution) and the Overview's map card, in English and in Arabic (the EN | العربية switch).
- On the host: `sudo journalctl -u pocol-explorer-gateway.service -n 50 --no-pager` (the holders index logs
  `holders index: ...` warnings there, never a request or an address).

## 5. What to watch after the deployment (the first days)

| What | How | Expected; act when |
|---|---|---|
| Index state | `curl -s https://explorer.pocol.net/api/holders.php \| head -c 400` | `ready`, snapshot height a few blocks behind the head and moving; `reconnecting` for more than a few minutes means the node is not answering |
| Completeness | `holders-stats.php` `verification` | `complete` true; a difference is published (F-104), and while it is, every known balance is read again every 60 s, so a published balance lags by about one minute plus one snapshot's duration |
| Long scans | `sudo journalctl -u pocol-explorer-gateway.service --no-pager \| grep 'longer than the state window'` | none, or a few while the first (full) scan runs: each keeps its reads and goes on at a newer block, and the snapshot height still advances. If the height never advances, one attempt cannot read every contract, the registry's IDs and reward addresses, the system addresses and the addresses touched since the previous attempt within 28 blocks (about 4.7 minutes; README, Holders index): report it with the log lines |
| Rewards | `holders-stats.php` `rewards.from` | a height (the index start); null means a gap past the 32-block window (F-103): reset them with `pocol_holders.py reset-rewards` (README, Holders index), which keeps everything else |
| Gateway memory and CPU | `systemctl show pocol-explorer-gateway.service -p MemoryCurrent -p CPUUsageNSec`, `systemd-cgtop -b -n 1` | well below `MemoryMax=256M` (measured: a snapshot of 100,000 accounts under 50 MB); roll back if it approaches the limit or the service restarts |
| Node load | `ps -o rss,pcpu,etime -C pocold`; `pocol_status` head and peers | the observer keeps up with its peers; each new block adds one reward read (`snapshot_v3`) and a snapshot reads the balances that changed |
| Disk | `sudo du -sh /var/lib/private/pocol-explorer-gateway/` | about 23 MB per kept snapshot at 100,000 accounts; with about 19 snapshots kept at 10 s blocks (the newest 3 and every one younger than 180 s) measured 484 MB there; PoCol Mainnet has far fewer accounts today (a few MB) |
| Busy answers | the explorer's "busy" notices; `curl -w '%{http_code}'` | 429 with `Retry-After: 3` only under load (holders page 2 tokens, statistics 1, map 5) |

## 6. Rollback

### 6.1 Automatic

A failed install restores the previous state itself (step 3): it reloads the previous Caddy configuration and restarts
the previous service. Nothing else is needed, except removing a holders index file the new gateway may have created
before the failure, which the previous gateway ignores:
`sudo rm -f /var/lib/private/pocol-explorer-gateway/holders.sqlite3{,-wal,-shm}`.

### 6.2 After a successful install (manual), with the backup of step 3

Tested in a sandbox: the real installer on temporary host paths with every system command faked
(`test_explorer_gateway.py`'s harness), updating a host that ran the 04f77806 unit with `--geoip-db`, then these
steps; every path came back to its earlier bytes and the Caddyfile to the previous site copy. Not run on a host.

```sh
B=/var/backups/pocol-explorer-gateway/<the backup of step 3>
sudo cat "$B/state.json"                                   # paths, previousSite, site
sudo systemctl stop pocol-explorer-gateway.service
sudo python3 -I -B - "$B" <<'PY'
import json, os, pathlib, sys
backup, caddyfile = pathlib.Path(sys.argv[1]), pathlib.Path('/etc/caddy/Caddyfile')
state = json.loads((backup / 'state.json').read_text())
for i, name in enumerate(state['paths']):
    saved, target = backup / f'file-{i}.before', pathlib.Path(name)
    if saved.exists():
        target.write_bytes(saved.read_bytes()); os.chmod(target, 0o644); print('restored', target)
    elif target.exists():
        target.unlink(); print('removed new', target)
caddyfile.write_bytes((backup / 'Caddyfile.before').read_bytes()); os.chmod(caddyfile, 0o644)
print('restored', caddyfile, '- previous site:', state['previousSite'], '- new site:', state['site'])
PY
sudo rmdir /opt/pocol-explorer-gateway/geoip 2>/dev/null || true               # only if it is now empty
sudo rmdir /etc/systemd/system/pocol-explorer-gateway.service.d 2>/dev/null || true
sudo rm -f /var/lib/private/pocol-explorer-gateway/holders.sqlite3 \
  /var/lib/private/pocol-explorer-gateway/holders.sqlite3-wal /var/lib/private/pocol-explorer-gateway/holders.sqlite3-shm
sudo systemctl daemon-reload
sudo caddy validate --config /etc/caddy/Caddyfile --adapter caddyfile && sudo systemctl reload caddy
sudo systemctl start pocol-explorer-gateway.service
```

Then check: `grep -n 'root \*' /etc/caddy/Caddyfile` names the previous `site-<old id>` (the installer keeps that copy, but only until its next run: after a later
installer run, put it back first as in "A rollback after a later stage");
`sudo cat /etc/systemd/system/pocol-explorer-gateway.service` is the unit recorded in step 0;
`curl -s https://explorer.pocol.net/api/health.php` is PASS and the explorer shows its Overview and Transactions tab as
before. The new copy `/srv/pocol-explorer/site-<new id>` can then be removed (`sudo rm -rf` of that one directory). The
transaction index file is never touched. The previous release's installer cannot be used for this: it refuses the new
unit as a custom edit until the old unit is back.

## 7. Operator reporters on the founder servers (only after the owner's approval)

Not part of this deployment. Services on the founder servers change only with the owner's approval
([GENESIS-RECORD-2026-11-21.md](../../development/public-network/GENESIS-RECORD-2026-11-21.md)). If the owner approves:

1. The owner creates one network salt and one key per reporter (32 random bytes each, lower-case hex, for example
   `python3 -c "import secrets; print(secrets.token_hex(32))"`), on the hosts themselves, never in the repository, a
   chat or a ticket.
2. Gateway host: `/etc/pocol-explorer-gateway/geo-reporters.json` =
   `{"salt": "<salt>", "reporters": {"founder-1": "<key 1>", "founder-2": "<key 2>", "founder-3": "<key 3>"}}`,
   root-owned, mode 0600; then steps 2 to 4 again (the installer adds the credential drop-in; `Miner map:` says
   "operator node reports enabled"). `POST /api/geo-report.php` then answers 400/401 instead of 503 for an unsigned
   request.
3. Each founder server (with the owner's approval for that server): `/etc/pocol-geo-reporter/credential.json` =
   `{"reporter": "founder-N", "key": "<key N>", "salt": "<salt>"}`, root-owned, mode 0600; copy the database built in
   step 1; from a checkout of the release commit:
   `sudo python3 deploy/explorer/install_geo_reporter.py --database ~/geoip/dbip-country-YYYY-MM.bin --check`, then
   the same without `--check`. It installs the reporter's service and timer (every 60 s; DynamicUser, Nice=19,
   MemoryMax=64M, one loopback read of the node per run) and runs one dry run, before the timer starts, whose report
   it prints for review: the node's head, its own mining address and its peers' keyed ids and countries, never an
   IP. Nothing of the dry run is sent. The server needs outbound HTTPS to explorer.pocol.net; a firewall change is the owner's.
4. Check: `systemctl list-timers pocol-geo-reporter.timer`, `journalctl -u pocol-geo-reporter.service -n 5` (`SENT:
   ...`), and in `/api/miner-geography.php` the source `founder-N` fresh and `miners.located` counting that server's
   miner. Founder 4 (the owner's Windows PC) has no reporter: its miner stays Unknown.
5. Remove: `sudo python3 deploy/explorer/install_geo_reporter.py --uninstall` on the server (the credential file
   stays); on the gateway, remove `geo-reporters.json` and rerun steps 2 to 4 (the drop-in goes too; removing the file
   without rerunning the installer stops the gateway from starting, since its `LoadCredential=` needs the file).
