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

## What the update changes on the host

| Where | Change |
|---|---|
| `/opt/pocol-explorer-gateway/` | `gateway.py`, `explorer_api.py` updated; `pocol_holders.py` and `pocol_geoip.py` added (the installer writes all six sources) |
| `/opt/pocol-explorer-gateway/geoip/dbip-country.bin` | the GeoIP database, only with `--geoip-db` |
| `/etc/systemd/system/pocol-explorer-gateway.service` | adds `Environment=POCOL_HOLDERS_DB=/var/lib/pocol-explorer-gateway/holders.sqlite3` and the new description; everything else, `MemoryMax=256M` included, is unchanged |
| `pocol-explorer-gateway.service.d/geo-reporters.conf` | only while `/etc/pocol-explorer-gateway/geo-reporters.json` exists (operator reports; not part of this deployment) |
| `/srv/pocol-explorer/site-<id>` | a new static copy, 14 files (new bundle, `assets/pocol-insights.js` and `.css`); the previous copy is kept for a rollback |
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

(Leave out `--geoip-db` without a database.) It must print `CHECK PASSED: network 2656089850
0x664dc53a2fa5e9e7c94ae3d735b1eefabb8630fcbabf5e8b0d34f5db0b8cc64e on the node, site-<new id> (14 files, this
checkout's snapshot), the loopback port, and the candidate Caddy configuration; the GeoIP database ... (release
YYYY-MM, N IPv4 and M IPv6 ranges), operator node reports disabled (no credential).` followed by the copy's manifest.
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
- Once ready: `"chainId":2656089850`, the snapshot height close to the node's head, and in the statistics
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

A failed install restores the previous state itself (step 3). Nothing else is needed, except removing a holders index
file the new gateway may have created before the failure (6.2, step 5), which the previous gateway ignores.

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

Then check: `grep -n 'root \*' /etc/caddy/Caddyfile` names the previous `site-<old id>` (the installer keeps that copy);
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
