# PoCol — Proof-of-Collaboration

PoCol is a coordinated proof-of-work consensus protocol. It keeps the parts of PoW that give it its security: the
hash-target puzzle and the chain with the most cumulative work. What it changes is how the mining work is organized.
Instead of every miner hashing its own competing candidate headers, all miners in a round work on **one canonical block
template**, and a deterministic seed gives each registered miner a **disjoint slice of the nonce space**. Honest miners
therefore never hash the same candidate header twice in a round, and the reward is shared among the winner and the
miners whose work is evidenced by *shares*.

This repository holds the specification and an experimental implementation:
- `pocold`, a Rust node with an EVM (revm, Cancun rules), P2P networking, mining, persistence and JSON-RPC;
- an execution layer with system contracts;
- on-chain website storage;
- a Chrome MV3 extension that verifies and displays on-chain sites.

> **Status: experimental.** The code runs local development networks (simnet) only. It is not production-ready, has not
> had an independent external security audit, and must not be used with real funds. See
> [Current status and limits](#current-status-and-limits).

---

## Contents

1. [Origin of the idea](#origin-of-the-idea)
2. [How PoCol works](#how-pocol-works)
3. [How the project was developed](#how-the-project-was-developed)
4. [Repository and code layout](#repository-and-code-layout)
5. [Requirements](#requirements)
6. [Quick start: a local devnet](#quick-start-a-local-devnet)
7. [Publishing a site and viewing it in Chrome](#publishing-a-site-and-viewing-it-in-chrome)
8. [JSON-RPC](#json-rpc)
9. [Tests](#tests)
10. [Contributing](#contributing)
11. [Current status and limits](#current-status-and-limits)
12. [License](#license)

---

## Origin of the idea

PoCol was conceived by **Raed S. Rasheed**.

The original design is described in [PhD-thesis-Raed-Rasheed-120211566-61.pdf](PhD-thesis-Raed-Rasheed-120211566-61.pdf).
The design was evaluated in simulation, using BlockSim extended with energy modelling, not on a distributed network:
- **Setup:** 20 logical miners (16 active, 4 reserve) and a nonce domain of 1,600 values, 100 per active miner.
- **Best result:** the best throughput-oriented policy kept **90.39%** of accepted-block output with a **55.05%**
  calculated energy reduction.
- **Against PoW:** in a matched comparison PoCol also used less energy, but produced fewer accepted blocks and had
  much longer rounds. The thesis reports this as an energy versus service-level trade-off under the configurations
  evaluated.
- **Stated limits:** the thesis lists open participation, fairness, incentive compatibility, Sybil resistance and
  PoW-equivalent security as **design objectives, not experimentally established guarantees**.

What the implementation takes from the thesis, and what it adds (the EVM layer, site storage, the browser extension,
network identity, resource policy), is listed in [development/m1/reference/scope.md](development/m1/reference/scope.md).

## How PoCol works

This is an outline. The normative rules are in [development/m1/reference/](development/m1/reference/): `consensus.md`,
`execution.md`, `network.md`, `storage.md`, `browser.md`, `governance.md`, `incentives.md` and `validation.md`.

### Network identity
- A network is defined by a `GenesisSpec`: a strict RLP encoding of the spec version, chainId, consensus parameters
  (`CP`), the initial allocation root, the initial miner list `M_0` and the system-contract code hash.
- `genesisHash = keccak256(GenesisSpec)`. Peers with a different identity or protocol version are disconnected.
- Before boot, **ParamGate** evaluates rules R1–R26 over the consensus and local parameters. A profile that violates a
  rule is refused, and the refusal lists the rules it violates.

### A mining round
1. **Template.** The proposer of attempt 0 signs the block template: transactions, timestamp and target. If the
   proposer is late, a deterministic, unsigned **fallback block** is built after `Δ_fb_wait`.
2. **Seed and assignment.** The seed is derived from the parent block's seal. It assigns each active miner a disjoint
   nonce range (`pocol_getAssignment`).
3. **Proof of work.** The PoW input is `TemplateID ‖ nonce`. A block is valid when its hash is below the target.
4. **Shares.** A nonce that meets the easier share target `T_share = target · m` is a share. The winner includes the
   share list in the block header.
5. **Difficulty.** ASERT, with an absolute anchor and a fixed width, steers the block time towards `T_blk`.

### Fork choice
- The chain with the **most cumulative work** wins. Ties are broken lexicographically along the branch path (D133).
- Reorganizations are bounded by `R_keep`; a deeper fork is refused.

### Rewards and settlement
- `R_tot = subsidy + fees`. The winner and the shares of a block are recorded in its child block (`ParentResult`).
- Settlement is deferred and runs through system contracts (Registry, Escrow, PAY), with exact rounding and
  classified burns.
- Miners register with a bond (`B_reg`). A miner inactive for long enough leaves the active set.

### Execution (EVM)
- User transactions run on revm 19.7 with Cancun rules. Consensus and execution are kept apart:
  `apply(S_parent, ctx, body, parentHeader) → (S, receipts, roots, …)`.
- The network has cumulative caps on accounts, storage slots, code and transaction bytes, and a final closing
  height `H_END`.

### Site storage and the browser
- **Storage.** A website is split into chunks deployed as contracts at CREATE2 addresses through `ChunkFactory`, with a
  manifest that carries every file's hash. Each site has versions and a `current()` pointer.
- **Verification.** The Chrome extension fetches a site and checks every chunk's address, the manifest hash and every
  file hash. It then renders the site in a sandboxed frame with no network access (CSP, per-tab DNR rules,
  RtcLockdown), and every request from the site goes through BridgeAuth.
- **Light client (RP).** Checks a window of 13 headers.
- **Wallet.** Devnet only. Every signature needs a confirmation that shows the exact bytes.

## How the project was developed

After the thesis, the implementation was built under the direction and approvals of the author, with AI coding agents
in separate roles:

- **Claude** (Anthropic) authors changes: specification, code, tests and evidence.
- **Codex** (OpenAI) reviews them independently, runs the checks, and has carried out some repair rounds.
- **The owner** takes every consensus and normative decision. Nothing counts as decided until the owner approves it
  explicitly. Open decisions are recorded in `development/audit-2026-10-22/proposals/`.

The working rules are in [development/README.md](development/README.md):
- every finding is tied to a requirement and to evidence;
- no item is closed by elapsed time or by agreement alone;
- a passing rerun does not explain an earlier failure.

Stages, in order:

| Stage | What it delivered | Where |
|---|---|---|
| **M1** | The specification: from the Arabic baseline to English drafts 0.1–0.33, with byte-level vectors and independent reviews | `development/m1/` |
| **LP1–LP2** | A first Rust node (RLP, GenesisSpec, ParamGate, headers, ASERT, assignment) and a candidate-header journal | `prototypes/lp1-node/`, `development/lp1`, `lp2` |
| **LP3** | A TypeScript reference model of the arithmetic, timing, membership and assignment, cross-checked against Rust | `prototypes/lp3-model/`, `development/lp3` |
| **LP4–LP8** | The execution layer: revm, system contracts, mempool, PAY, settlement, caps and resource bounds | `prototypes/lp4-execution/`, `development/lp4` |
| **LP5–LP7** | The `pocold` node: block production and mining, fork choice, multi-node sync, P2P and RPC | `prototypes/pocold/` |
| **LP9** | The Chrome MV3 extension | `prototypes/pocol-extension/`, `development/lp9` |
| **LP10** | An integrated devnet: site publishing, reorganization, restart and the browser | `prototypes/lp10/`, `development/lp10` |
| **Audit of 2026-10-22 and its rounds** | Repairs A01–A14, the A13 memory bound, the on-disk state store (redb), checkpoints, RpcGuard, I/O-fault handling, Windows and Chrome verification | `development/audit-2026-10-22/` |

Stage reports: [development/reports/](development/reports/) and [development/STATUS-AR.md](development/STATUS-AR.md),
written in Arabic. The status of every audit finding: `development/audit-2026-10-22/FINDING-STATUS.json`.

## Repository and code layout

```
.
├── PhD-thesis-Raed-Rasheed-120211566-61.pdf   the thesis (origin of the idea)
├── development/                               specification, tracking, evidence, decisions
│   ├── m1/reference/                          reference specification (Arabic): consensus, execution, network, storage, browser, …
│   ├── m1/M1-SPEC.md, m1-draft-*/            English specification drafts and their vectors
│   ├── VA-SPEC.md                             validation specification
│   ├── lp1 … lp10/                            evidence for each stage
│   ├── audit-2026-10-22/                      the audit, its repair rounds, evidence and the open-decision table
│   ├── reports/, STATUS-AR.md                 status summaries (Arabic)
│   └── *-APPROVAL.md, OWNER-STANDING-AUTHORIZATION.md   owner approvals
├── prototypes/
│   ├── pocold/                                the node (Rust 1.99.1)
│   ├── lp4-execution/                         execution layer and system contracts (Rust 1.99.1 + Solidity)
│   ├── pocol-extension/                       Chrome MV3 extension (JavaScript)
│   ├── lp1-node/                              the first node: encoding and verification (Rust 1.58)
│   ├── lp3-model/                             reference model (TypeScript)
│   └── lp10/run_lp10.py                       runs every suite
└── .github/workflows/                         Windows, Chrome, LP3 and LP4 checks
```

### `prototypes/pocold` — the node

| File | Responsibility |
|---|---|
| `src/main.rs` | Command line: `init-devnet`, `run`, `status` |
| `src/node.rs` | Event loop: import, validation, mempool, templates, recovery at open |
| `src/chain.rs` | Block tree, work-based fork choice, reorganization, retention, side blocks |
| `src/miner.rs` | Mining inside the assigned range, shares |
| `src/net.rs` | P2P over TCP: handshake, sync, gossip, connection limits |
| `src/rpc.rs` | JSON-RPC server with RpcGuard (queues, deadlines, rate limits, memory bounds) |
| `src/store.rs` | Block journal `blocks.journal` (append-only, torn-tail recovery) |
| `src/history.rs` | `history.redb`: receipts and the transaction index |
| `src/state_store.rs` | `state.redb`: state versions as on-disk tries (experimental `--state-store` mode) |
| `src/checkpoint.rs` | Sealed state checkpoints bound to the journal |
| `src/statediff.rs` | State diffs between blocks |
| `src/resources.rs` | The A13 model: the computed memory bound, checked at boot |
| `src/site.rs`, `src/bin/pocol-site.rs` | Publishing, fetching and verifying sites |
| `src/bin/pocol-sign.rs` | Signing a devnet transaction |
| `scripts/devnet.py` | Create, start, inspect and stop a local network |
| `tests/` | Consensus, network, storage, caps and budget tests; Python end-to-end tests |

### `prototypes/lp4-execution` — execution
- **Sources:**
  - `apply.rs` and `apply/`: block application;
  - `system.rs` and `system/`: system phases;
  - `pay.rs`, `native.rs`: the PAY boundary;
  - `pool.rs`: the mempool;
  - `state_trie.rs`, `node_trie.rs`: state roots;
  - `admission.rs` and `admission/`: ParamGate;
  - `resources.rs`: resource bounds;
  - `modexp.rs`: MODEXP fast paths, checked against revm;
  - `devnet.rs`: devnet genesis builder.
- **Contracts (`contracts/`):** `RegistryPayment.sol`, `PendingEscrow.sol`, `ChunkFactory.sol` and witness contracts. The
  storage layout is in `REGISTRY-LAYOUT.md`.

### `prototypes/pocol-extension` — the Chrome extension
- `src/sw.js`: the service worker (network profiles, DNR rules, routing).
- `src/lib/`:
  - `rp.js`: header window;
  - `site.js`: site fetch and verification;
  - `bridge.js`: BridgeAuth;
  - `sitestore.js`: per-site storage;
  - `wallet.js`: the wallet.
- `src/sandbox-loader.js`, `src/viewer.js`: the sandboxed frame.
- `build.mjs`: builds `dist/`, and deliberate fault builds used as negative test controls.

## Requirements

| Tool | Version | Used for |
|---|---|---|
| Rust | 1.99.1 (pinned in `rust-toolchain.toml`) | `pocold`, `lp4-execution` |
| Rust | 1.58.1 | `lp1-node` only |
| Python | 3.10 or newer | `devnet.py`, end-to-end tests |
| Node.js | 22 or newer | building and testing the extension |
| Chromium/Chrome + Playwright 1.62.1 | — | browser tests only |

The code builds on Linux, macOS and Windows. Windows and Chrome are checked automatically by the workflows in
`.github/workflows/`. Local networks bind to `127.0.0.1` only and use deterministic, worthless devnet keys.

## Quick start: a local devnet

```sh
git clone https://github.com/palcertcom/pocol.git && cd pocol

# 1) Build the node and its tools
cd prototypes/pocold && cargo build --release --locked && cd ../..
# -> prototypes/pocold/target/release/{pocold,pocol-site,pocol-sign}

# 2) A three-node network (a, b, c)
D=prototypes/pocold/scripts/devnet.py
python3 $D init  /tmp/devnet --miners a,b,c --t-blk 2   # genesis, devnet.json, profile.json
python3 $D start /tmp/devnet --log                      # P2P on 30300+i, RPC on 8540+i
python3 $D status /tmp/devnet                           # height, head, peers, production
python3 $D rpc /tmp/devnet a eth_blockNumber
python3 $D stop /tmp/devnet                             # or --only b to stop one node
```

Notes:
- **Timing.** Keep `T_blk ≥ 1 + dFbWait`. The defaults (`--t-blk 2 --fb-wait 1`) are sustainable.
- **Restart.** A node restarted with the same `--data` directory replays its journal and recovers torn tails.
  `pocold status --network DIR --data DIR` prints the head without starting the node.
- **On-disk state store (experimental).** Add `--state-store` to `pocold run`, or set `POCOLD_RUN_ARGS=--state-store`
  for `devnet.py`. Always open a data directory in the mode it was created in.
- **Running `pocold` directly:**
  ```sh
  pocold init-devnet --out NET --miners a,b,c [--chain-id 777900] [--t-blk S] ...
  pocold run --network NET --data DATA --name a --miner a --listen 127.0.0.1:30300 --rpc 127.0.0.1:8540 [--peer ADDR]...
  ```
  `pocold help` lists every option.
- **Allowed networks.** The default chainId is 777900 (simnet). The production network profile (chainId 777001)
  deliberately does not boot.

## Publishing a site and viewing it in Chrome

```sh
CID=$(python3 -c "import json;print(json.load(open('/tmp/devnet/net/devnet.json'))['chainId'])")
B=prototypes/pocold/target/release
$B/pocol-site publish --rpc 127.0.0.1:8540 --chain-id $CID --account alice --dir ./mysite
#   -> {"site":"0x…","version":0,...}   to publish an update, add --site 0x…
$B/pocol-site fetch --rpc 127.0.0.1:8541 --site 0x… --out ./fetched   # verifies every chunk and file
```

The Chrome extension:
1. Build it:
   ```sh
   cd prototypes/pocol-extension
   npm ci
   node build.mjs    # -> dist/
   ```
2. Open `chrome://extensions`, turn on Developer mode, choose **Load unpacked** and select
   `prototypes/pocol-extension/dist`.
3. On the extension's options page, paste `/tmp/devnet/net/profile.json` and click **Add**. The profile is accepted
   only if `genesisPre` hashes to `genesisHash` and the chainId matches.
4. To open a site, type `pocol devnet/0x<site>/` in the address bar.
5. The wallet is devnet only. Fund it with `pocol-sign`, then send the output with `eth_sendRawTransaction`.

Full details: [development/lp10/LP10-RUNBOOK.md](development/lp10/LP10-RUNBOOK.md) and
[development/lp9/LP9-EXTENSION.md](development/lp9/LP9-EXTENSION.md).

## JSON-RPC

The node serves a subset of the standard `eth_*` methods:
- `eth_blockNumber`, `eth_chainId`, `eth_getBalance`, `eth_getCode`, `eth_getStorageAt`;
- `eth_call`, `eth_estimateGas`, `eth_sendRawTransaction`, `eth_getTransactionReceipt`;
- `eth_getLogs`, `eth_getBlockByNumber`, `eth_feeHistory`, and others.

It also serves `net_*`, `web3_clientVersion`, and PoCol-specific methods:

| Method | Purpose |
|---|---|
| `pocol_status`, `pocol_syncStatus` | Node and sync status |
| `pocol_getTemplate`, `pocol_getAssignment`, `pocol_getTarget`, `pocol_submitShare` | Template, nonce range, target, share submission (local only) |
| `pocol_getMiners`, `pocol_getMinerSet` | Miners and the active set |
| `pocol_getSettlement`, `pocol_getEscrow`, `pocol_getPot`, `pocol_getBurnTotals` | Rewards, settlement, burns |
| `pocol_getHeaders`, `pocol_getBlockRlp`, `pocol_getLogs` | Headers for light clients, raw blocks, branch-bound logs |
| `pocol_getParams`, `pocol_getCaps`, `pocol_getTiming`, `pocol_getClosing`, `pocol_getRetention` | Parameters, caps, timing, closing, retention |
| `pocol_getRejects`, `pocol_getRpcStats` | Diagnostics |

RPC limits (connections, sizes, deadlines) are described in `development/m1/reference/network.md` and
`development/audit-2026-10-22/RPCGUARD.md`.

## Tests

```sh
# Every suite (evidence -> development/lp10/LP10-EVIDENCE.json)
python3 prototypes/lp10/run_lp10.py
python3 prototypes/lp10/run_lp10.py --only e2e-browser

# The node
cd prototypes/pocold
cargo test --release --locked
cargo test --release --locked --features io-fault \
  --test state_store --test consensus --test checkpoint_source --test eviction_memory   # deterministic I/O faults
cargo test --release --locked --features crash-test --test consensus                     # crashes at named steps
python3 tests/e2e_devnet.py && python3 tests/e2e_site.py

# The execution layer
cargo test --release --locked --manifest-path prototypes/lp4-execution/Cargo.toml

# The extension
cd prototypes/pocol-extension && npm ci && npm test
node tests/e2e_browser.mjs       # needs Chromium, Playwright and local nodes
node tests/e2e_integrated.mjs    # partition, reorganization, restart
```

- **Long measurements.** Journal growth to `H_END`, runs at the network's state caps and performance measurements are
  `#[ignore]` tests in `tests/journal_measure.rs`. Run them with `-- --ignored`.
- **Ports.** The suites use ports 30300–30302, 8540–8542 and 8600–8602. Run one suite at a time.
- **Failure evidence.** On a failure, the end-to-end harnesses keep the node logs, inputs and seeds in
  `POCOL_E2E_KEEP` or `tests/failures/`.

## Contributing

- **Consensus is owner-gated.** A change to a consensus rule, a `CP` parameter or a wire encoding needs an owner
  decision. Write a proposal in `development/audit-2026-10-22/proposals/` with the alternatives and their effects, and
  do not implement it before it is approved.
- **Local limits.** Resource limits are not raised without a decision. Every new data structure in the node must be
  counted in the A13 model (`src/resources.rs`). The `a13_bound_breakdown` test pins the total.
- **Test before you claim.** Add a test that fails without the fix, and record the command, the version and the
  result. Keep failures in the evidence. An ignored or cancelled test does not count as a pass.
- **Style.**
  - Code and technical specifications are in English; status summaries are in Arabic.
  - Follow the conventions of the file you edit.
  - Dependencies are pinned to exact versions in `Cargo.lock` and `package-lock.json`.
- **CI.**
  - `audit-r2-windows-verify.yml`: Windows Server 2022;
  - `pocol-browser-verify.yml`: Chrome on Linux, in both state modes;
  - `lp3-*.yml` and `lp4-execution-controls.yml`.
- **Security.**
  - Tests are limited to local nodes, local RPC and test browser profiles.
  - Never use real funds or real keys.
  - Do not publish exploit programs.

## Current status and limits

**Shown by tests, locally and in CI:**
- a three-node devnet produces blocks and stays in sync;
- reorganization at a network heal, in both directions;
- recovery after `SIGKILL`;
- site publishing and verified fetching;
- rendering in Chrome with isolation tested by negative controls;
- resuming state from checkpoints;
- the node stops on disk failures instead of judging valid blocks invalid;
- passing runs on Linux, Windows Server 2022 and Chrome.

**Not established, or still open:**
- **The production network profile does not boot on `pocold`.**
  - There is no approved network genesis.
  - Whole-state mode does not meet the A13 memory condition.
  - Rule R21 refuses the profile.
  - Only simnet admission exists (R22).
- **The memory bound.** In store mode the computed bound is about 5.2 GiB against a 6 GiB budget (the condition is
  model × 8/7 ≤ 6 GiB). Not every worst case has been measured.
- **Decisions pending with the owner.** Many consensus and normative decisions are still open: the six consensus
  proposals, the network caps, constant identity and timing calibration among them. They are listed in
  `development/audit-2026-10-22/proposals/DECISION-TABLE-2026-10-28.md`.
- **Design objectives.** PoW-equivalent security, fairness and Sybil resistance are not proven.
- **No external audit.** No independent external human security audit has been done.

**Do not use this code for a real network or with real funds.**

## License

The code in `prototypes/` is licensed under **Apache-2.0**, as declared in its `Cargo.toml` and `package.json` files.
Third-party licenses are in `prototypes/lp4-execution/licenses/` and ship with the libraries vendored into the
extension. The thesis remains the copyright of its author.

For questions about the idea or the project, open an issue in this repository.
