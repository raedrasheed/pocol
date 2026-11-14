# PoCol fee faucet

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

Why 0.002 PCOL is enough: the wallet's earn-in registration reserves a maximum fee for
400,000 gas, about twice the 194,143 gas a zero-bond first registration uses
(`prototypes/lp4-execution/tests/earnin_contracts.rs`, `earn_in_write_ceilings`). At the starting base
fee of 1 gwei the reservation is 0.0012 PCOL and the actual fee about 0.0004 PCOL. The amount still
covers the reservation while the base fee stays below 2 gwei.

On the v3 network (the release rules; no v3 network identity is pinned yet) a zero-bond newcomer facing a full set registers into the seat of an
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
float, never a miner key or a key that holds other funds.

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
behind the existing HTTPS reverse proxy and set `trustProxy: true` only when that proxy sets
`X-Forwarded-For`. `allowOrigin` names the one website origin allowed to call it from a browser.

- `POST /claim` with `{"address": "0x..."}` returns `{"type": "sent", "hash": ..., "amountWei": ...}`,
  or `{"type": "error", "message": ...}`. Send one `Content-Length` and an
  `application/json` body, up to 512 bytes; incomplete or ambiguous framing is rejected.
  Request errors use 400, 408, 409, 413 or 415; limits or a busy signer use 429;
  node, signer or ledger errors use 500, 502 or 503.
- `GET /status` returns the chain ID, the amount, the operator address and the transfers left today.

The server admits at most 16 simultaneous connections and closes excess connections before creating
a worker. Headers and body share an absolute 12-second read deadline, so trickling bytes cannot
retain a worker indefinitely. Only one claim runs at a time; another claim receives 429 immediately.
Keep the reverse proxy's connection/rate limits too: these bounds do not prevent network saturation.
If a send's response is lost, an error can mean the transfer was already submitted. Check the
address and the ledger's `unknown` entry before retrying; an error is not proof that nothing was sent.

`pocol-faucet.service` is a systemd unit for a dedicated `pocol-faucet` user. That user must own the
key file: `pocol-sign` refuses a key owned by another user or readable by others. On a server, create
the user and `/var/lib/pocol-faucet`, then the key as that user, and use the server's own signer
(`/opt/pocol-testnet/bin/pocol-sign`, as in `config.example.json`):

```
sudo useradd --system --home-dir /var/lib/pocol-faucet --shell /usr/sbin/nologin pocol-faucet
sudo install -d -m 0700 -o pocol-faucet -g pocol-faucet /var/lib/pocol-faucet
sudo -u pocol-faucet /opt/pocol-testnet/bin/pocol-key generate --out /var/lib/pocol-faucet/operator.pocol-key
sudo install -D -m 0644 deploy/faucet/pocol_faucet.py /opt/pocol-faucet/pocol_faucet.py
sudo install -D -m 0644 deploy/faucet/config.example.json /etc/pocol-faucet/config.json
sudo install -m 0644 deploy/faucet/pocol-faucet.service /etc/systemd/system/pocol-faucet.service
sudo systemctl daemon-reload
sudo systemctl enable --now pocol-faucet.service
```

Each claim sends 0.002 PCOL and pays its own transfer fee (21,000 gas at the suggested gas price), so
1 PCOL pays for a little under 490 claims at the starting base fee.

## Tests

```
python3 -m unittest discover -s deploy/faucet -p 'test*.py' -v
python3 deploy/standalone/test_wallet_earnin.py   # opt-in, about 15 minutes: real nodes, signer and wallet
```

The unit tests cover once per address, new accounts only, reserved addresses, the daily and
per-client limits, the chain ID pin, an empty faucet, a durable ledger before the send, refused
versus unanswered sends, connection admission, absolute read deadlines, request framing, busy-claim
rejection, a stable status count during a concurrent claim, and the HTTP surface. The end-to-end test starts a faucet from a genesis
miner's key file, funds a wallet that holds nothing, and that wallet then registers with a 0 PCOL
bond as its first transaction.

Deploying the faucet is the owner's action (A4). Nothing here deploys it.
