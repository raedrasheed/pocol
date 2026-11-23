# PoCol fee faucet

**PoCol Mainnet, and devnets of the rehearsal profile** (owner decision 2026-11-18, superseding the rehearsal-only
decision of 2026-11-16; [OWNER-DECISIONS-2026-11-18.md](../../development/decisions/OWNER-DECISIONS-2026-11-18.md);
the rehearsal profile is a test and devnet profile only,
[OWNER-DECISIONS-2026-11-20.md](../../development/decisions/OWNER-DECISIONS-2026-11-20.md) D1).
`config.mainnet.example.json` names mainnet (2656089850, `deploy/mainnet/v3-profile.json`) and `config.example.json`
a devnet of the rehearsal profile (1656550731, `deploy/mainnet/v3-rehearsal-profile.json`). On mainnet the owner's limits are
ceilings that the code enforces (`MAINNET_LIMITS`): a config above any of them is refused at start, before the key
file or the signer is touched. A newcomer can also receive its first fee by an ordinary transfer from any PCOL holder.

A new miner can register with a 0 PCOL bond, but its first transaction still pays a fee. The faucet sends a new account enough for that fee, and nothing more.

## Rules

| Rule | Default | Config key |
|---|---|---|
| Amount per address | 0.002 PCOL (`2000000000000000` wei) | `amountWei` |
| Once per address, ever | always | the ledger file |
| New accounts only: nonce 0, no code, balance below the amount | always | — |
| Transfers per UTC day | 200 (0.4 PCOL a day at most) | `dailyLimit` |
| Transfers per client per UTC day | 3 | `perClientDaily` |
| Node chain ID must equal the configured one | always | `chainId` |
| Node genesis hash (`pocol_getParams`) must equal the configured one | when set (recommended: the invitation's `devnet.json` `genesisHash`) | `genesisHash` |
| Operator balance at most the float, else the faucet stops | mainnet: 10 PCOL; elsewhere: none | `maxFloatWei` |

On mainnet the amount, the two daily limits and the float may be set lower than these values, never higher.

Why 0.002 PCOL is enough: the wallet's earn-in registration reserves a maximum fee for
400,000 gas, more than twice the 174,447 gas a zero-bond first registration uses on the V3 programs
(`prototypes/lp4-execution/tests/v3_contracts.rs`, `wallet_bond_features_keep_their_write_ceilings_and_gas_on_the_v3_set`;
194,143 on the earn-in programs, deleted in phase 6a). At the starting base
fee of 1 gwei the reservation is 0.0012 PCOL and the actual fee about 0.0004 PCOL. The amount still
covers the reservation while the base fee stays below 2 gwei.

On the v3 networks (mainnet and a devnet of the rehearsal profile) a zero-bond newcomer facing a full set registers into the seat of an
inactive member instead (FN-2b; `quote_register` sends registerReplacing): the wallet reserves REPLACE_GAS = 500,000
gas, about twice the largest replacement measured (217,381 gas, `prototypes/lp4-execution/tests/v3_contracts.rs`,
`replacing_write_ceilings_gas_and_user_sys_keys`). With the maximum fee at 2 x base fee + 1 gwei, 0.002 PCOL covers
that reservation only while the base fee stays below 1.5 gwei (at 1 gwei it is 0.0015 PCOL). Above that, such a
newcomer needs more than the faucet's amount; the faucet amount is sized for an ordinary registration.

The faucet creates no coins. It spends an operator account's own balance. Refill it with an ordinary
transfer.

## Key handling

The operator key never enters the faucet process. Each transfer is signed by
`pocol-sign --key-file KEY`, and the signed bytes are relayed to the local node with
`eth_sendRawTransaction`. The faucet refuses to start if the key file can be read by group or others.
It does not change any file's permissions. Use a dedicated operator key holding only the faucet's
float, never a miner key, a reward key or a key that holds other funds.

The operator key is a hot key: it signs on the server without a person, so it lives on the server, unlike the
owner's offline reward and release keys. Generate it on the server as the `pocol-faucet` user (below), so it never
travels, and fund its address with the float only. On mainnet a claim is refused while the operator account holds
more than `maxFloatWei` (10 PCOL), so a key that holds other funds cannot serve; refill it with a transfer that keeps
it at or below the float. The worst case of a stolen or drained operator key is the float it holds; with the daily
limit, a faucet that keeps running pays out at most 0.4 PCOL a day.

## Ledger

`ledger` is a JSON file of claims by address. It is written atomically (temp file, fsync, rename)
before a transfer is relayed, so a crash can never pay the same address twice.
- If the node refuses the transfer, the claim is removed and the address may try again.
- If the node does not answer, the claim stays as `"state": "unknown"`. Compare the operator
  account's nonce with the ledger before clearing such an entry by hand.

Keep the ledger with the node's backups. Losing it lets addresses claim a second time, but only
while they still have nonce 0 and a balance below the amount.

## Running

```
python3 deploy/faucet/pocol_faucet.py --config /etc/pocol-faucet/config.json
```

`config.example.json` lists every key. The service listens on `127.0.0.1:8547` by default. Put it
behind the HTTPS route below and set `trustProxy: true` only when the proxy in front of it overwrites
`X-Forwarded-For`, as that route does. `allowOrigin` names the one website origin allowed to call it from a browser;
it is sent as `Access-Control-Allow-Origin` on every JSON answer, errors and 404 included, and on the preflight.

- `POST /claim` with `{"address": "0x..."}` returns `{"type": "sent", "hash": ..., "amountWei": ..., "to": ...}`,
  or `{"type": "error", "code": ..., "message": ...}` (the codes are listed below). Send one `Content-Length` and an
  `application/json` body, up to 512 bytes; incomplete or ambiguous framing is rejected.
- `GET /status` returns `{"type": "status", "chainId", "amountWei", "faucet", "remainingToday", "dailyLimit",
  "perClientDaily"}`: the chain ID, the amount, the operator address, the transfers left today (all clients) and the
  two daily limits. It does not ask the node, so it answers even while a claim would be refused.
- `OPTIONS` on any path answers 204 with `Content-Length: 0` and, when `allowOrigin` is set, the origin,
  `Access-Control-Allow-Methods: GET, POST`, `Access-Control-Allow-Headers: Content-Type` and
  `Access-Control-Max-Age: 600`.

With `trustProxy` the per-client counter keys on the last `X-Forwarded-For` entry, if it is an IP address (otherwise
on the connecting address); without it, on the connecting address. An IPv4 address is its own key; an IPv6 address
counts by its /64, so one host cannot rotate addresses inside its /64 to pass the per-client limit. The mainnet example
config sets `trustProxy: true` and `allowOrigin: "https://pocol.net"`: the faucet runs behind the explorer.pocol.net
Caddy route, and without `trustProxy` every client would arrive as 127.0.0.1 and share one counter (3 claims a day
for everyone). Any local process on the server can still reach 127.0.0.1:8547 and set the header itself.

### Error codes

| Status | `code` | Meaning |
|---|---|---|
| 400 | `invalid_address` | Not a full 0x address of 42 characters |
| 400 | `reserved_address` | A system program, PAY, a precompile, the zero address or the operator itself |
| 400 | `contract_account` | The account holds code |
| 400 | `bad_request` | Framing (chunked, no or two `Content-Length`), an incomplete body, invalid JSON, or not exactly `{"address"}` |
| 404 | `not_found` | Any path other than `GET /status` and `POST /claim` (a query string included) |
| 408 | `deadline` | The body was not read within the 12-second request deadline; no claim was processed (a request whose line or headers miss the deadline is closed without an answer) |
| 409 | `already_claimed` | The ledger holds this address (sent, or an unknown outcome) |
| 409 | `not_new_account` | The account has sent a transaction |
| 409 | `already_funded` | The account already holds at least the amount |
| 413 | `too_large` | `Content-Length` is not a number, is 0, or is above 512 |
| 415 | `unsupported_media` | The body is not `application/json` |
| 429 | `busy` | Another claim is running; the only answer with `Retry-After: 2` |
| 429 | `daily_limit` | Today's transfers (UTC) are used up |
| 429 | `client_limit` | This client's transfers today (UTC) are used up |
| 500 | `signer_failed` | The signer failed, timed out or returned no transaction; nothing was sent |
| 502 | `node_refused` | The node answered with an error, to a check or to the send; nothing was sent and the claim is not kept |
| 502 | `node_unavailable` | The node could not be reached or gave no usable answer, or the ledger could not be written, before anything was sent |
| 502 | `unknown_outcome` | The send got no answer, or its answer could not be saved: a transfer may already have been submitted. The ledger keeps the claim, so the address is refused until the operator checks it; check the address in the explorer before retrying, never report it as failed |
| 503 | `wrong_network` | The node is on another chain; the faucet is stopped |
| 503 | `over_float` | The operator account holds more than `maxFloatWei`; the faucet is stopped |
| 503 | `empty` | The operator account cannot pay the amount and its fee |

`message` is English text for people; a page translates `code`. Answers that do not come from the faucet carry no
`code` and no CORS header, so a browser sees a network error: Caddy's own error when the faucet is down or slower than
the route's 45 s, Python's own errors (such as 431 for more than 100 headers), connections closed when 16 are already
open or when the request line or headers miss the 12-second deadline. The route passes only GET, POST and OPTIONS;
other methods on `/faucet/claim` and `/faucet/status` are answered by the explorer site, not the faucet.
After a Caddy timeout the faucet may still send, so a page treats a network error on a claim like `unknown_outcome`.

The server admits at most 16 simultaneous connections and closes excess connections before creating
a worker. Headers and body share an absolute 12-second read deadline, so trickling bytes cannot
retain a worker indefinitely. Only one claim runs at a time; another claim receives 429 immediately.
Keep the reverse proxy's connection/rate limits too: these bounds do not prevent network saturation.
If a send's response is lost, an error can mean the transfer was already submitted. Check the
address and the ledger's `unknown` entry before retrying; an error is not proof that nothing was sent.

`pocol-faucet.service` is a systemd unit for a dedicated `pocol-faucet` user. That user must own the
key file: `pocol-sign` refuses a key owned by another user or readable by others. On a server, create
the user and `/var/lib/pocol-faucet`, then the key as that user, and use the server's own signer:
`/opt/pocol-standalone/pocol-sign` on a mainnet server installed from the release
(`deploy/standalone/install-linux.sh`, as in `config.mainnet.example.json`), or
`/opt/pocol-testnet/bin/pocol-sign` on a devnet server of the rehearsal profile (as in `config.example.json`). On mainnet:

```
sudo useradd --system --home-dir /var/lib/pocol-faucet --shell /usr/sbin/nologin pocol-faucet
sudo install -d -m 0700 -o pocol-faucet -g pocol-faucet /var/lib/pocol-faucet
sudo -u pocol-faucet /opt/pocol-standalone/pocol-key generate --out /var/lib/pocol-faucet/operator.pocol-key
sudo install -D -m 0644 deploy/faucet/pocol_faucet.py /opt/pocol-faucet/pocol_faucet.py
sudo install -D -m 0644 deploy/faucet/config.mainnet.example.json /etc/pocol-faucet/config.json
sudo install -m 0644 deploy/faucet/pocol-faucet.service /etc/systemd/system/pocol-faucet.service
sudo systemctl daemon-reload
sudo systemctl enable --now pocol-faucet.service
```

`pocol-key generate` prints the operator address; send it the float (at most 10 PCOL on mainnet). On a devnet
server of the rehearsal profile use `/opt/pocol-testnet/bin/pocol-key` and `config.example.json` instead.

## Public route on explorer.pocol.net

A page on https://pocol.net calls the faucet from the browser at `https://explorer.pocol.net/faucet/status` (GET)
and `https://explorer.pocol.net/faucet/claim` (POST `{"address": "0x..."}`, `Content-Type: application/json`, which
sends a preflight). Call it directly from the browser, not through a relay on the pocol.net host: a relay would make
every claim come from that host's address and share one per-client counter.

`faucet_route.py` holds the Caddy route and installs it into the explorer's managed block
(`deploy/explorer/explorer_site.py`), the way `deploy/studio/install_studio_gateway.py` adds the Studio route:

```
    # BEGIN POCOL FAUCET v1
    @pocol_faucet {
        path /faucet/claim /faucet/status
        method GET POST OPTIONS
    }
    handle @pocol_faucet {
        uri strip_prefix /faucet
        reverse_proxy 127.0.0.1:8547 {
            header_up X-Forwarded-For {http.request.remote.host}
            transport http {
                dial_timeout 3s
                response_header_timeout 45s
            }
        }
    }
    # END POCOL FAUCET v1
```

- Only the two paths with GET, POST and OPTIONS reach the faucet, with the `/faucet` prefix removed; any other path
  is the explorer site's.
- `header_up` replaces whatever `X-Forwarded-For` a client sent with the connecting address, the key of the
  per-client limit.
- `response_header_timeout 45s` is longer than the signer's 30-second limit, leaving room for the claim's node calls.
- The route is inserted right before the block's `@rpc path /rpc` line and does not change the site's `max_size`, so
  installing it and the Studio route in either order gives the same file.
- An edited or duplicated route, or another route to port 8547, is refused for review.
- An explorer update (`deploy/explorer/install_explorer_gateway.py`) keeps the route exactly. Its HTTPS check then
  also requires `GET /faucet/status` to answer 200 with `type` `status` and the network's chain ID, so the faucet
  must be running while the explorer is updated.

Install it on the explorer gateway server, from a checkout, once the explorer block is installed and the faucet runs
there with `trustProxy: true` and `allowOrigin` set (as in `config.mainnet.example.json`):

```
sudo python3 deploy/faucet/faucet_route.py --check
sudo python3 deploy/faucet/faucet_route.py
curl --fail --max-time 20 https://explorer.pocol.net/faucet/status
```

The installer refuses a config that does not listen on `127.0.0.1:8547`, has `trustProxy` other than `true`, or has
no `https://` `allowOrigin`. It checks the faucet on loopback (`/status` names the config's chain ID; the preflight and
a probe claim carry the origin). It also checks that the candidate is exactly what an explorer update would keep, and
runs `caddy validate` on it. `--check` stops there and changes nothing.

An install backs up the Caddyfile under `/var/backups/pocol-faucet-route/`, writes the candidate and reloads Caddy.
It then checks over HTTPS with the public name and the default trust store: `/faucet/status`, the preflight, the probe
claim, and the explorer's `/health`. On any failure it restores the previous Caddyfile and reloads it. If the route is
already installed, it only runs the HTTPS check. The only claim it ever posts is `{"address": "0x"}`, which the faucet
refuses with 400 `invalid_address` before the ledger, the node or the signer is touched.

Each claim sends 0.002 PCOL and pays its own transfer fee (21,000 gas at the suggested gas price), so
1 PCOL pays for a little under 490 claims at the starting base fee, and a 10 PCOL float a little under 4,900.

## Tests

```
python3 -B -m unittest discover -s deploy/faucet -p 'test*.py' -v
python3 deploy/standalone/test_wallet_earnin.py   # run on its own (CI: public-mainnet-e2e.yml); real nodes, signer and wallet; time depends on the host's hash rate
```

The unit tests cover once per address, new accounts only, reserved addresses, the daily and
per-client limits, the chain ID pin, the mainnet ceilings (a config above the owner's limits is refused before the
key or the signer; the defaults equal them) and the mainnet float, both example configs, an empty faucet, a durable ledger before the send, refused
versus unanswered sends, connection admission, absolute read deadlines, request framing, busy-claim
rejection, a stable status count during a concurrent claim, and the HTTP surface.
`test_faucet_http.py` covers the status and code of every error and the code set itself, CORS on every answer, the
preflight, `Retry-After` on a busy claim only, the client key (last `X-Forwarded-For` entry, an invalid entry, IPv6 by
/64, `trustProxy` off), and `unknown_outcome` against `node_unavailable` and `node_refused`. `test_faucet_route.py` covers
the route and its guards, both install orders with the Studio route, refusals, the installer's check, install and
rollback paths with faked system commands, and, when Caddy is at `/usr/bin/caddy`, `caddy validate` of the patched block
and a loopback Caddy in front of a faucet with a fake node and signer. The end-to-end test starts a faucet from a genesis
miner's key file, funds a wallet that holds nothing, and that wallet then registers with a 0 PCOL
bond as its first transaction.

Deploying the faucet is the owner's action (A4), and needs the owner's approval for each server. Nothing here
deploys it.
