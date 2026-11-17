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
- the gateway's `/rpc` and `/health`, and the Studio relay `/studio/*` that `deploy/studio` adds to the same block.

Dotfiles, server code (`.php`), checksum lists and the extensions the `.htaccess` denied (`zip tar gz bak log sh md
json map`) are 404; there is no directory listing. `/api/*` and `/rpc` keep a 16 KiB request-body guard each, also
after the Studio installer raises the site's `max_size` to 128 KiB for its own route; the `/api/*` one answers as
rpc.php did, 413 `{"error":"Request size limit exceeded."}` as JSON.

## Files

- `explorer_site.py`: the Caddy site block (between `# BEGIN/END POCOL EXPLORER GATEWAY v1`), the checks of the
  static copy, and a loopback test copy of the block (`test-config`, used by `deploy/websites/tests/explorer_smoke.mjs`).
- `install_explorer_gateway.py`: the transactional installer of the gateway service, the static copy and the block.
- `test_explorer_gateway.py`: the block against the Studio installer in both orders, the update and refusal rules,
  the copy's checks on the committed snapshot, and the install, `--check` and rollback paths on temporary paths with
  every system command faked.

```sh
python3 -B -m unittest discover -s deploy/explorer -v
python3 -B -m unittest discover -s deploy/rewards -p test_explorer_api.py -v
```

## Install on the gateway server

Run from a checkout of the release commit: the installer installs `deploy/rewards/gateway.py`, `pocol_rewards.py` and
`explorer_api.py` from it, and reads the Studio installer's route rules from `deploy/studio`. It needs Python 3.10+,
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
bundle switched for a rehearsal network); it prints both ids. It refuses a copy with a hidden, server, checksum or
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
`/api/health.php` PASS, `/api/rpc.php` and `/rpc` naming the network, `/health`, hidden and server paths 404, and the
Studio route when it is installed. On any failure it restores the previous Caddyfile, files, site copy and service
state, and names what it could not restore together with the backup.

The HTTPS check needs the certificate for explorer.pocol.net, which Caddy obtains only once the DNS record points at
the gateway server. So run `--check` before the owner moves the record and the full install right after; TLS
verification is never relaxed.

The Caddyfile: on a fresh server the stock Debian/Ubuntu `:80` file is replaced by the block; other sites are kept;
an unmanaged `explorer.pocol.net` site, a second marked block, an edited marked block, or another route to port 8088
is refused for review (no changes made). An update replaces only the site root of the marked block and keeps the
Studio route and its body size. Install this before the Studio gateway (`deploy/studio`), which adds its route to this
block; either order of later updates gives the same file.

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
- wrong methods on the four paths get the PHP 405 and `Allow` header (not a 501), other `/api` paths are 404.

The budget is the gateway's: one global token bucket (80 tokens, 20 per second) shared by `/rpc`, the `/api` reads,
pocol.net's summary and the reward page, as it was behind the PHP relays, plus four relay slots shared by the `/api`
reads (PHP's four lock slots). A connection beyond the gateway's 16 request slots gets the same JSON busy answer (429,
`Retry-After: 3`), which Caddy passes on, not an empty 502.
