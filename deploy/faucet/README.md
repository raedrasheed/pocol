# PoCol fee faucet (earn-in network)

Decision A2 in [the earn-in proposal](../../development/decisions/EARN-IN-MEMBERSHIP-PROPOSAL.md).
A new miner can register on the earn-in network with a 0 PCOL bond, but its first transaction still
pays a fee. The faucet sends a new account enough for that fee, and nothing more.

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

The faucet creates no coins. It spends an operator account's own balance, which moves to the new
network at the cutover (N1). Refill it with an ordinary transfer.

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
  or `{"type": "error", "message": ...}` with status 400, 409, 413, 429, 502 or 503.
- `GET /status` returns the chain ID, the amount, the operator address and the transfers left today.

`pocol-faucet.service` is a systemd unit for a dedicated `pocol-faucet` user.

## Tests

```
cd deploy/faucet && python3 -m unittest test_pocol_faucet -v
python3 deploy/standalone/test_wallet_earnin.py   # opt-in, about 15 minutes: real nodes, signer and wallet
```

The unit tests cover once per address, new accounts only, reserved addresses, the daily and
per-client limits, the chain ID pin, an empty faucet, a durable ledger before the send, refused
versus unanswered sends, and the HTTP surface. The end-to-end test starts a faucet from a genesis
miner's key file, funds a wallet that holds nothing, and that wallet then registers with a 0 PCOL
bond as its first transaction.

Deploying the faucet is the owner's action (A4). Nothing here deploys it.
