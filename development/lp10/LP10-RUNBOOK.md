# LP10 — Local PoCol devnet runbook

Local `simnet` network only (chainId 777900 by default; chainId 777001 is never
bootable). All keys are deterministic, disposable devnet keys derived from labels.
No real funds, no remote endpoints. Everything binds to 127.0.0.1.

## 1. Build

```sh
# Rust nodes and tools (toolchain pinned by rust-toolchain.toml)
cd prototypes/pocold && cargo build --release
# Chrome extension (MV3)
cd prototypes/pocol-extension && npm ci && node build.mjs      # -> dist/
```

Binaries: `pocold` (node), `pocol-site` (publish/fetch websites), `pocol-sign` (sign a devnet transaction).

## 2. Launch a fresh devnet (3 nodes)

```sh
D=prototypes/pocold/scripts/devnet.py
python3 $D init  /tmp/devnet --miners a,b,c --t-blk 2      # genesis, alloc, devnet.json, profile.json
python3 $D start /tmp/devnet --log                         # p2p 30300+i, rpc 8540+i
python3 $D status /tmp/devnet                              # height, head, peers, production
python3 $D rpc /tmp/devnet a eth_blockNumber
python3 $D stop /tmp/devnet                                # --only b to stop one node
```

Timing parameters: keep `T_blk >= 1 + dFbWait` (the default `--t-blk 2 --fb-wait 1` is sustainable).
A fallback block needs at least `1 + dFbWait` seconds. With a shorter T_blk the chain falls behind its
schedule, ASERT keeps easing the target, and RP clients correctly refuse the window at `viewTargetCeil`
(this was observed in LP9 with `--t-blk 1`).

Recovery: a node restarted with the same `--data` directory replays its journal (torn tails are
recovered; a corrupted complete record refuses to start). `pocold status --network DIR --data DIR`
replays offline and prints the head.

## 3. Publish and read a website

```sh
CID=$(python3 -c "import json;print(json.load(open('/tmp/devnet/net/devnet.json'))['chainId'])")
prototypes/pocold/target/release/pocol-site publish --rpc 127.0.0.1:8540 --chain-id $CID --account alice --dir ./mysite
#   -> {"site":"0x…","version":0,...}; update: add --site 0x… (new version, setCurrent)
prototypes/pocold/target/release/pocol-site fetch --rpc 127.0.0.1:8541 --site 0x… --out ./fetched
```

The fetch verifies every chunk's CREATE2 address, the manifest hash and every file hash.

## 4. Chrome

1. `chrome://extensions` → Developer mode → Load unpacked → `prototypes/pocol-extension/dist`.
2. Open the extension options (toolbar button) → paste `/tmp/devnet/net/profile.json` → Add.
   The profile is accepted only if `genesisPre` hashes to `genesisHash` and the chainId matches.
   The trust level is `RP` (13-header window). For a profile per node, set `"trustLevel":"LN"` and change `endpoint`.
3. Open a site: type `pocol devnet/0x<site>/` in the address bar (omnibox keyword `pocol`), or use the options page.
4. The trust banner above the page states what was verified. The site runs in a sandboxed frame with
   no network: CSP, per-tab DNR rules and RtcLockdown. Every site request goes through BridgeAuth.
5. Wallet (local devnet only): create it on the options page. Fund the shown address from a devnet account:
   `pocol-sign --chain-id $CID --account alice --to 0x<wallet> --value 1000000000000000000 --nonce <n>` and send the
   output with `eth_sendRawTransaction`. Every account grant and every transaction needs a click in the
   viewer, which shows the exact raw bytes and the hash.

## 5. Test suites

```sh
python3 prototypes/lp10/run_lp10.py                 # everything; evidence -> development/lp10/LP10-EVIDENCE.json
python3 prototypes/lp10/run_lp10.py --only e2e-browser
```

| Suite | What it proves |
|---|---|
| lp4-tests | execution, system contracts, settlement SV vectors, resource bounds |
| pocold-tests | consensus items, journal crash-atomicity, site encoding |
| ext-unit | BridgeAuth tables (BR1–BR19a), storage quota (BR17e), RecvGuard (D98), manifest T16, wallet, fault controls |
| e2e-devnet | 3 processes, ≥ 100 blocks, late join, partition/heal, SIGKILL, malicious peer, replay |
| e2e-site | publish v1/v2 through different nodes, verified fetch everywhere, rejected operations, restart |
| e2e-browser | Chrome: rendering, isolation canaries, BridgeAuth end to end, storage, navigation, wallet, negative fault builds |
| e2e-integrated | fresh launch, site update on a minority branch, reorg recovery in Chrome, SIGKILL + full restart, ≥ 100 identical blocks, replay of roots and content, resources |

Ports used by the suites: 30300–30302, 8540–8542 (nodes), 8600 (RpcTap), 8601/8602 (HTTP/UDP canaries).
Run one suite at a time.
