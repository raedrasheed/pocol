# Plan (revision 2): on-chain HTML/CSS/JS sites, and explorer, Studio, IDE and the Chrome extension on the fresh v3 network

Owner requirement (verbatim): "يجب ان تكون الشبكة قابلة لتخزين html و css و JS لنشر صفحات الانترنت واعمل على المواقع الثلاثة المرفقة https://explorer.pocol.net/ https://studio.pocol.net/ https://ide.pocol.net/ وايضا الاضافة الخاصة بمتصفح كروم"
(the network must store HTML/CSS/JS to publish web pages; make the three sites and the Chrome extension work on the new network).

Base: main 984ea50 in `ro-main`, plus the Phase 4 branch `claude/fresh-net-phase4-v3-release` (a710afc). Work was read-only: no edits, builds or commits. `api/config.php` and credentials were never read. Paths are repo-relative. Extension paths are under `prototypes/pocol-extension/`; `pocold` and `lp4-execution` are under `prototypes/`.

Revision 2 folds in two critiques. Every point was checked against the code; the few rejected or modified points are in Appendix A. Changed or new items are marked **(r2)**.

## 0. Where things stand, and what this plan relies on

- **The storage layer already meets the requirement under v3.**
  - ChunkFactory at `0xC0C005` is byte-identical in the V3 program set. A chunk is at most 24,575 B; B_code_max is 32,768; FN-2a removed the creation burn.
  - `Website.sol` bytecode is pinned identically in `lp4-execution/tests/data/website.json` and `src/lib/website-bytecode.js`.
  - Manifest MIME ids cover html, css, js/mjs, json, svg, png, jpg, webp, woff2, txt.
  - The EVM spec id is CANCUN (`lp4-execution/src/code_caps.rs:98`, `collision.rs:32`), so EIP-6780 applies: once created, code at an address cannot change on that chain. Only a reorg can remove it. W1's cache relies on this.
- **What does not work yet for "HTML/CSS/JS sites" (r2).**
  - **ES-module imports between site files do not resolve.** `sandbox-loader.js:244-251` runs each `<script>` from a `blob:` URL, and the loader rewrites HTML attributes and CSS `url()`/`@import` (`:156-178`) but never JS `import`/`export … from`/`import()` specifiers. A relative specifier inside a blob module has no hierarchical base, and the sandbox CSP (`build.mjs:21-22`, `script-src 'sha256-…' blob:`) allows nothing else. Classic scripts and single-module bundles work; multi-module sites and code-split Vite builds fail. The explorer bundle has no `import(` (0 hits), so S1 would not reveal it. New item W2b.
  - **A long publish cannot finish.** `wallet.js:7` LOCK_MS is 15 min; `publisher-ui.js:133` sets `until` once per run, the sign callback (`:110`) checks but never extends it, and `:141` locks and aborts at expiry. The explorer (about 72 txs) takes about 12-24 min and a 4 MiB site about 30-57 min. New in W4.
  - **The fee rule stalls a large publish** (see W4 for the numbers).
  - **Old-network jobs block the Studio wallet for good.** `hasUnfinishedSigned` (`publisher-ui.js:85`) has no chain or genesis check, and cancel (`:66-73`) needs receipts on the current network. New in W4.
- **Measured cost.**
  - A full chunk uses about 5.395M gas, declared 6,589,500, so one full chunk per 8M block.
  - Base fee: London arithmetic (`lp4-execution/src/fees.rs:4-10`), target 4M. A full-chunk block raises it by about 4.36%. With baseFeeMin 10^7 wei (design row 55), a 4 MiB site (about 171 chunks, about 175 txs) moves it from about 0.01 to about 15-17 gwei; design §10 gives about 1.9 PCOL burned, plus about 0.9 PCOL in tips at the node's 1 gwei tip.
  - Explorer bundle **(r2)**: `icon-512.png` and `pocol-icon.png` are byte-identical (sha256 5abc663f…), and `SHA256SUMS.txt` is not published (W5). That leaves 10 files, about 67 unique chunks plus the manifest, about 72 txs. W5's test computes the exact count.
- **Nothing proves the round trip on v3.** `pocold/tests/e2e_site.py` and the browser e2e suites run only on `init-devnet` (spec 1). `publisher.html`/`publishSite` have never run against a real node. The Studio gateway has never served a site in a test.
- **deploy/websites has no CI (r2).** `full-nonce-upgrade.yml:5` paths do not include `deploy/websites/**`, and no workflow runs `deploy/websites/tests/api_security.php` or `browser_security.mjs`.
- **Branch state.**
  - Merged on main: Phase 3, FN-2a, FN-2b, G3a'.
  - Phase 4 is on `claude/fresh-net-phase4-v3-release` (a710afc, includes G3b'). Extension commit 1a6d140 sets `SPEC_VERSION = 2`, removes `--v3-preview`, and makes `package-release.py` require spec 2.
  - **(r2)** Phase 4 also touches files the Early PRs edit: `build.mjs`, `scripts/package-release.py`, `deploy/studio/test_studio_gateway.py`, `deploy/rewards/gateway.py`, `deploy/mainnet/set_network_identity.py` (and its test), `.github/workflows/full-nonce-upgrade.yml`.
  - FN-1 has no commits, so `pocol_getCaps` is listed at `rpc.rs:545` but has no handler.
- **Dependency chain (design §3):** Ph4 → FN-3 → FN-4 → FN-5 → Ph5(a,b,c) → Ph6a → Ph6b → Ph7+FN-6 → FN-7 → Ph8 → rehearsal → ceremony → Ph10. FN-1 any time.
- **Placement change, recorded (r2).** Design §3 puts FN-6 "with Phase 7". This plan splits it:
  - **FN-6a (Early, after Phase 4 merges):** W1-W6, W9, the built-in profile placeholder.
  - **FN-6b (with Phase 7):** W14.
  - A docs PR (W0) amends the design's §3 DAG and the FN-6 row to say so. This plan cannot edit the design itself.
- **Not site work.** The wallets' `registerReplacing` path belongs to G3b' (in Phase 4). The explorer only *displays* it (W7).

## 1. Ordered PRs

**Early (r2)** now means: merges right after Phase 4 (a710afc) is on main, or is stacked on the Phase 4 branch until then. It depends on none of Phases 5-6, is outside the consensus boundary, needs no identity and needs no lock line.

| # | PR | Blocked by | Fits |
|---|---|---|---|
| W0 | Docs: FN-6a/FN-6b split in design §3; W8 precondition in the removal plan's ceremony checklist | — | FN-6 |
| W9 | Website identity tool, `--check`, websites CI workflow, config.php guards; delete superseded IDE bundle | Ph4 merged (Early) | prepares Ph10/rehearsal |
| W1 | Studio gateway: canonical identity, safe-filled chunk cache with its own budget, dapp read methods, `--network` override | Ph4 merged (Early) | FN-6a |
| W2 | Extension: chunk-level fetch pool, retry of idempotent reads in every RPC wrapper | Early | FN-6a |
| W2b | **(r2)** Extension: ES-module imports inside sites | Early | owner requirement |
| W3 | Extension: remove chatgpt.site origins; manifest and release assertions | Ph4 merged (Early) | FN-6a |
| W4 | **(r2 rewritten)** Publisher: fee schedule from the node, publication key lease, checkpoint v2, foreign-network jobs | Early | FN-6a |
| W6 | Explorer gateway and PHP: `pocol_getMembers` (w0 only), `pocol_getMember`, `pocol_getRemovable`, `pocol_finality` | Ph4 merged (Early) | FN-6 "gateway activeSlice" |
| W5 | **(r2 rewritten)** Publish input: loud refusal kept, explicit exclusion, hard-deny list, folder-import order; MIME decision | Early (MIME part needs Q3) | FN-6a |
| W8 | Host-only explorer pieces as reviewed templates | Owner copy before the wipe (Q2), else rewrite | FN-6 / Ph7 |
| W7 | Explorer bundle: v3 membership, Registry decoding, light block refresh | Q1 (source); live test after Ph5 | FN-6 |
| — | Extension `SPEC_VERSION` 2 | **Ph4** (1a6d140) | Ph4 |
| W10 | v3 publish→serve e2e with the Rust CLI, CI | **Ph5a**; re-run after FN-3 | Ph5 |
| W11 | Browser e2e on v3: viewer, `publisher.html`, Studio gateway, explorer, IDE | **Ph5a** + W1, W2, W2b, W4 | Ph5/FN-6 |
| W12 | `pocol_getCaps` publisher pre-check | **FN-1** | FN-6 / C9 |
| W13 | Fork-cap and decoder follow-ups | **Ph6a** | Ph6a |
| W14 | Phase 7: forks UI removed, version bump, READMEs, download links, explorer gateway `--v3-network` mandatory, release signing | **Ph7** | Ph7+FN-6b |
| W15 | Rehearsal identity switch and rehearsal build | rehearsal genesis (after Ph8) | rehearsal S1 |
| W16 | Identity pin across all clients | **Ph10** | Ph10 |

Merge order of the Early PRs: W0 → W9 → W1 → W2 → W2b → W3 → W4 → W6 → W5 → W8 → W7. W9 is first so every later bundle edit goes through its anchored patcher and is checked by the new websites CI.

---

### W0. Docs (r2)
- `development/handoff/FRESH-NETWORK-DESIGN.md` §3 and the FN-6 row: record FN-6a (Early, after Ph4) and FN-6b (with Ph7), with the item lists above.
- `FRESH-NETWORK-REMOVAL-PLAN.md`, genesis ceremony section: add as a blocking precondition "W8 host-only pieces copied and redacted (or rewrite merged)", and name the host that runs `/opt/pocol-explorer-gateway` and the `gateway.pocol.net` Caddy (owner answer Q2c).
- `lp4-execution/contracts/REGISTRY-LAYOUT.md:52` lists bondState only as HELD1/BURNED2/PAID3, but the V3 program also uses ACCRUING = 4 (`PoColSystemV3.sol` :226, `chain.rs:567`). Fix the doc line in the same PR if the file is not boundary-locked; otherwise in Phase 9.

### W9. Website identity tool, constant checks and websites CI (Early)
`deploy/mainnet/set_network_identity.py` deliberately skips `deploy/websites/` (:39). Add a websites section to it or a separate `deploy/websites/tools/site_identity.py`, rebased on Phase 4's version of the tool:
- **Exact anchored counts per file**, failing on any mismatch:
  - explorer `index-*.js`: chain-id/genesis sites at 9:44653/44668, 9:45360-45602, 10:322/351, 10:434100/129, 10:453078/123, 10:467820/473591/473668;
  - explorer `index.html:7` meta;
  - IDE bundle `index-extension041-*.js` frozen object at 1027:39366-39640 (`chainId`, `chainIdHex` minimal lowercase hex, `genesisHash`, `name`), and **(r2)** the IDE `rpcUrl` (`w9.rpcUrl`, default the Studio gateway) as an optional anchor so tests can point it elsewhere;
  - explorer seed list (10:475668-475702, 10:475894) from the invitation's seeds;
  - source links (10:475517, 10:480248, 10:480497; Studio `app.js:42`) from `--commit`.
- **File handling:** rename each content-hashed file, update `index.html`, regenerate `SHA256SUMS.txt`.
- **`--check` for CI:** served identity matches the declared one; no retired id from `deploy/mainnet/retired-networks.json` (enabled at W15/W16); no `chatgpt.site`; every download link version equals extension `package.json`; every `index.html` references an existing hashed bundle; `SHA256SUMS.txt` matches.
- **Websites CI workflow (r2).** New `.github/workflows/websites.yml`:
  - paths: `deploy/websites/**`, `prototypes/pocol-studio/**`, `prototypes/pocol-extension/src/**`, `deploy/studio/**`, `deploy/rewards/gateway.py`, the workflow file;
  - runs `php deploy/websites/tests/api_security.php`, Playwright `browser_security.mjs`, and the tool's `--check`;
  - a guard that fails if `git ls-files` lists any `config.php`;
  - a test that `explorer.pocol.net/api/.htaccess` still contains the `FilesMatch "^(config|lib)\.php$"` deny rule (`php -S` ignores `.htaccess`, so nothing else checks it).
- **`.gitignore` (r2):** add `**/api/config.php`.
- **Same PR:** delete the superseded IDE bundle `assets/index-CJbrNXr_.js` and its `SHA256SUMS.txt` line. It differs only at 1027:101035, and the live server does not list it.
- **Test:** old → test identity → old is byte-identical; the tool fails on a moved anchor.

### W1. Studio gateway (Early) — `deploy/studio/`
`pocol_studio_gateway.py`:
- **Canonical raw-transaction chain id (:85).**
  - Replace `fields[0] != CHAIN_ID.to_bytes(4,'big')` with: `v = fields[0]`; `ok = 0 < len(v) <= 8 and v[0] != 0 and int.from_bytes(v,'big') == CHAIN_ID`.
  - Reasons: the extension signs with minimal RLP (`src/lib/wallet.js:32`), so any id below 2^24 fails today; and **(r2)** `CHAIN_ID.to_bytes(4)` raises OverflowError for ids ≥ 2^32, which `do_POST` (:290) does not catch.
- **Health and identity compare (:194-195):** `int(chain,16) == CHAIN_ID` after a strict hex-quantity check; `genesisHash` compared lowercase on both sides.
- **Test identity override (r2):** `--network FILE` in the explorer gateway's existing `--v3-network` format (`deploy/rewards/gateway.py:66`: `{"chainId","genesisHash","specVersion":2,"v3":true}`), validated strictly. The `CHAIN_ID`/`GENESIS` constants stay as the default, so the `set_network_identity.py` FILES counts do not change.
- **Chunk `eth_getCode` cache with its own budget (r2 revised).** The FN-6 "eth_getCode cache or separate budget" item.
  - **Fill only from `safe`.** On a `latest` request for code, the gateway first asks upstream `eth_getCode(addr,'safe')` (the node supports `safe`, `rpc.rs:986`; N2 sets it to 6 blocks). If that is non-empty and chunk-shaped (first byte 0x00, length ≤ 24,576), cache it and answer. Otherwise forward the original `latest` request and do not cache. Old chunks (the common case) cost one upstream call; fresh chunks cost two.
  - **Serve hits only for tags `latest` and `safe`**, never for `finalized`, `pending`, a number or a hash: code present at `safe` need not exist at an older height.
  - **No CREATE2 re-derivation.** Under CANCUN/EIP-6780 any code present at `safe` is final for that chain unless a reorg deeper than `safe` happens (an alarm condition, design N2). Every viewer verifies every chunk (factory address, length, keccak) anyway. Dropping the derivation avoids a vendored pure-Python Keccak, its CPU cost per miss, and a change to `build_installer.py:8` `files` and the FILES counts. See Appendix A1.
  - **Memory.** Store raw bytes, not hex. Bound the LRU at 24 MiB. Raise the unit's `MemoryMax=128M` (`install_studio_gateway.py:62`) to 256M in the same PR, because 16 connection threads can each build up to 2 MiB answers and `json.dumps` runs twice per answer (:296, :234).
  - **Keyed by identity.** The cache lives in memory only, is keyed by the verified genesisHash, and is flushed whenever `verify_network` fails or the identity changes.
  - **Budget.** `do_POST` charges the read bucket before `call()` (:297). Classify every item of a request first: cache hits are charged to a new per-client and global chunk bucket; misses for chunk-shaped requests are charged to the chunk bucket too and get 1-2 reserved upstream slots, separate from the 4 general `rpc_slots` (:151, :202). Defaults sized for one 4 MiB site open (about 175 reads) per client in a few seconds; final numbers follow Q8.
  - **Never cache `0x`.**
- **Allowlist additions (:25-27, :109-122)** for methods the bridge exposes to site JS (`src/lib/bridge.js:131-140`):
  - `eth_maxPriorityFeePerGas`;
  - `eth_feeHistory` (blockCount ≤ 128, ≤ 16 percentiles, mirroring the bridge schema);
  - `eth_getStorageAt`;
  - `eth_getBlockByHash` (full=false only);
  - **(r2)** `net_version` answered locally from `CHAIN_ID`, and `web3_clientVersion` answered with a constant, so the node version is not exposed;
  - `pocol_getLogs` (what `eth_getLogs` maps to, `sw.js:250-256`): both the `blockHash` form and a numeric range of at most 1024 blocks, matching `LOG_RANGE_MAX`, with the existing 2 MiB response cap.
- **Docstrings :4 and :330** say "test network". Text fix only.

Other files:
- `install_studio_gateway.py:113-114, :144-153`: the same canonical compare. Constants stay (in FILES).
- `Install-PoCol-Studio-Gateway.sh`: rebuild with `build_installer.py`.

Tests (`test_studio_gateway.py`, rebased on Phase 4's version):
- **Chain id:** ids of 1, 3 and 4 bytes accepted when configured; a leading zero refused; a 5-byte id is refused when `CHAIN_ID` is 4 bytes, and the gateway does not crash for a configured id ≥ 2^32; an uppercase-genesis node accepted.
- **Cache:** a hit makes no upstream call; `0x` not cached; non-chunk-shaped code not cached; a chunk present at `latest` but not at `safe` is answered but not cached; a `finalized` or numbered request bypasses the cache; the LRU bound holds; chunk-bucket exhaustion gives 429 while the read bucket is untouched; hits are served while all 4 general slots are busy; a reorged-out chunk (mock upstream drops it before `safe`) is reported missing and the publisher then stores it; identity change flushes the cache.
- **Allowlist:** each new method's parameter bounds; `net_version` makes no upstream call.

The test runs in CI at `full-nonce-upgrade.yml:58` and in `websites.yml`. If Phase 7 renames the workflow, keep it there.

### W2. Extension RPC resilience (Early, r2 revised)
- **`src/lib/site.js` `fetchSite` (:69-85).**
  - Today parallelism is per *file* (`limits.parallel` 8) while chunks inside a file are fetched one by one (:78). Replace it with a **chunk-level** pool whose width is at most what `CONTENT_POOL` (8 × 68 KiB, `transport.js:19`) admits without waiting past `RESERVE_WAIT_MS` 2000. Default 2 for the RN profile (the gateway's slots are global, not per client), 8 for LN/DEV; tuned by Q8.
  - Retry 429, -32021 ("busy"), transport timeouts and dropped connections (the gateway's 16-connection cap, `Server.slots` :308-313) with bounded exponential backoff and jitter: at most 6 attempts per chunk and a total deadline (for example 60 s). Safe because every chunk is verified.
- **Every other RPC wrapper (r2):** the publisher (`publisher-ui.js` rpc), `contracts.js` (IDE jobs) and the Studio content-script calls get the same retry, **for idempotent reads only**. `eth_sendRawTransaction` is never retried on timeout or busy; the publisher already re-checks the receipt and asks the user to resume (`publisher.js:188-192`).
- **`src/lib/transport.js:100`:** classify HTTP 429 as retryable, separate from other errors; honour `Retry-After` up to a cap.
- **Tests (`tests/site_fetch_test.mjs`)** with a mock RPC returning 429/-32021 or dropping the connection on the first k calls: the site opens; concurrency never exceeds the cap; a permanent 429 fails within the deadline with a clear error; a tampered chunk on retry is still rejected; a busy `eth_sendRawTransaction` is not retried.

### W2b. ES-module imports inside sites (Early, r2 new)
- **Loader (`src/sandbox-loader.js`).** Before creating a module's blob URL, rewrite its static `import … from '…'`, `export … from '…'` and `import('…')` with a **string-literal** specifier that resolves to a site path, replacing it with the blob URL of that module. Build blobs leaf-first (post-order) with a depth bound (8, like CSS) and a module-count bound. A cycle cannot be expressed with blob URLs: refuse it with a clear error shown in the viewer. Bare specifiers and absolute URLs stay blocked (CSP). Non-literal `import(expr)` cannot be rewritten: it fails, and the error says so.
- **Publish-time check** (`prepareSite` in `publisher.js`, Studio `model.js` `validateFiles`, `pocol-site publish`): parse JS/MJS files for relative module specifiers; refuse a cycle or a specifier that does not resolve to a site file before any signature, and warn on non-literal `import()`.
- **Keep the parser small and shared** between the loader and the publisher (a tokenizer that skips strings, comments, template literals and regex literals), with unit tests from adversarial inputs.
- **Tests:** a two-level module graph renders; a cycle is refused with the clear error both at publish and at view; a module importing CSS via `url()` still works; `fetch` remains blocked; `browser_security.mjs` still passes.
- If the owner prefers not to support multi-module sites, the fallback is the publish-time refusal only, with a documented "single bundle or classic scripts" profile. The recommendation is to support them, because Vite and similar tools produce them by default.

### W3. Remove the chatgpt.site origins (Early, FN-6a)
- **Remove `*.mystic-gecko-9511.chatgpt.site` entries in:**
  - `src/lib/studio-network.js:2` (STUDIO_ORIGINS; r2 line fix);
  - `src/studio-content.js:3`;
  - `src/lib/contracts.js:5` (IDE_ORIGINS);
  - `src/ide-content.js:3`;
  - `src/contract-ui.js:10`;
  - `build.mjs:64` (both content_scripts matches; rebased on Phase 4's `build.mjs`);
  - `README-STUDIO.md:48`;
  - `deploy/websites/ide.pocol.net/DEPLOY-README-AR.txt:67` (the origin line only; the surrounding "replace files in place" text is rewritten in W14).
- **Tests:**
  - `studio_test.mjs:54` and `contracts_test.mjs:106` use the chatgpt origin as the "other allowed origin": assert instead that a foreign origin is refused and `https://studio.pocol.net` / `https://ide.pocol.net` are accepted.
  - Manifest `content_scripts[*].matches` equals exactly `["https://studio.pocol.net/*"]` and `["https://ide.pocol.net/*"]`; `host_permissions` stays `["http://127.0.0.1/*", "https://gateway.pocol.net/*"]` (`studio_test.mjs:43`).
  - No `chatgpt.site` anywhere in `dist/`.
- **`scripts/package-release.py` refusals (r2).** Today these assertions live only in tests on `dist/`, and Phase 4 removes the `PREVIEW` name check. The packager itself refuses unless:
  - the manifest name equals the release name exactly (so `[TEST FAULT …]` and `[REHEARSAL]` builds are refused);
  - `host_permissions` and `content_scripts` match exactly;
  - the sandbox CSP equals `SANDBOX_CSP(hash,false)`;
  - `NETWORK.endpoint === "https://gateway.pocol.net/studio/rpc"` and `trustLevel === "RN"`;
  - no `chatgpt.site`;
  - the built-in profile is not the W4 placeholder (see D1) and its chain id is not in `retired-networks.json`.

### W4. Publisher: fees, key lease, checkpoint, foreign jobs (Early, r2 rewritten)
**Why the r1 rule fails.** The node's tip is constant: `eth_maxPriorityFeePerGas` is 1 gwei and `eth_gasPrice` is next base + 1 gwei (`rpc.rs:1026-1027`). The publisher's own load raises the base fee about 4.36% per full-chunk block. A single maxFee of `2·base + tip` ≈ 1.02 gwei at the floor is overtaken after about 108 chunks, then again around 134, 154 and 172. Also `publisher.js:181` requires every recorded tx to have `maxFee === f.maxFee`, and on resume every recorded chunk is revisited (`:226`), so any re-approval with a new maxFee breaks resume.

- **Fee schedule from the node (`src/lib/publisher.js:11-12, :69-74`).**
  - Read `base_next` (`eth_gasPrice − 1 gwei`, cross-checked with `eth_getBlockByNumber('latest').baseFeePerGas` and `eth_feeHistory`) and the tip (`eth_maxPriorityFeePerGas`, capped at 2 gwei; fallback 1 gwei).
  - **Simulate the exact London recurrence** of `fees.rs` over the planned tx sequence (deploy, each chunk at its estimated gasUsed `200000 + 260·size`, createVersion, setCurrent), assuming one publication tx per block and otherwise-empty blocks. That gives a projected base_i per tx. Set `maxFee_i = ceil(base_i × (1 + margin)) + tip` with margin 12.5% (one block of maximum rise). See Appendix A2 for why this replaces the critique's closed-form approximation.
  - **Approve one budget and one per-gas ceiling.** Budget = `Σ gas_i × maxFee_i`; ceiling = `max_i maxFee_i × 2`, never above an absolute 200 gwei. The approval screen shows the expected cost (`Σ est_gasUsed_i × (base_i + tip)`, about 2.8 PCOL for 4 MiB at the floor) and the approved budget separately.
  - **Sign just in time:** each tx is signed with `maxFee = max(maxFee_i, 2·current_base_next + tip)` provided it stays ≤ ceiling and the running spend stays ≤ budget (`:170`). If either would be exceeded, pause in a "fee rose" state that asks for a new budget/ceiling approval for the remaining txs only; confirmed chunks are kept (content addressing makes stores idempotent).
  - **Optional economy mode:** one chunk every second block. A full block (+4.36%) then an empty one (−12.5%) keeps the base fee at the floor: burn drops to about 0.01 PCOL for 4 MiB, but the publish takes about twice as long. Offered as a checkbox, default off.
  - **Lower tip option:** the tip is 100× the floor base fee. Expose a tip field (minimum the node's accepted minimum, if any; default the node value).
- **Checkpoint schema 2 (`publisher.js:126, :132, :181`).** Bind only the manifest, the budget, the ceiling, chainId and genesisHash. Each record keeps its own `maxFee`/`maxPriority`; on resume check `rec.tx.maxFee ≤ ceiling` and `rec.tx.maxPriority ≤ rec.tx.maxFee` instead of equality. Schema-1 checkpoints are migrated only if their stored identity is the current NETWORK; otherwise they are archived (below).
- **Final availability sweep (r2).** Before `createVersion`, re-read every chunk with tag `safe` and wait (bounded, for example 3 min) until all exist at `safe`. This closes the case where a chunk seen at `latest` (stored by another publisher, or served from any cache) is reorged out and `createVersion` would reference missing code. After `createVersion` is at `safe`, read the site back with `fetchSite` and verify it.
- **Publication key lease (r2).** After the user approves a job, each signature made *by that job* extends `until` by LOCK_MS, up to a job deadline shown in the approval (default 2 h, maximum 6 h). The lease ends on completion, error, pause or when the user presses Lock (`lock()` still aborts, `:32`). The same lease applies to IDE `contract.html` jobs. The IDE and other pages never get the extension.
- **Foreign-network jobs (r2).** Store chainId and genesisHash on every job (the binding at `:112` already carries them; the job record does not). `hasUnfinishedSigned` (`:85`) and the unresolved-job lookup (`:101`, both `studio:job:` and `ide:job:`) ignore jobs whose identity differs from NETWORK. On `onInstalled`/update, such jobs are moved to an archive key and listed read-only; "Cancel" on a foreign job archives it without needing receipts (`:66-73`).
- **Checkpoint size (r2).** Every `setJob` (`:15-17`) rewrites files as base64 (about 5.6 MB for 4 MiB) plus every signed raw tx (about 8.6 MB for 175 txs), about 3 times per chunk. Store files under a content key written once, raw txs one key per tx, and the job as a small index. Drop `raw` once its receipt is at `safe`. `unlimitedStorage` is present (`build.mjs:62`), so this is about speed and worker memory, not quota.
- **`src/publisher-ui.js:113, :148`.** Drop the literals `'200000000000'`/`'1000000000'`; quote only after connecting.
- **Rust CLI (`pocold/src/site.rs:159, :166`, `bin/pocol-site.rs`).** Signs just in time already: derive `maxFee = 2·base_next + tip` per tx, enforce a `--max-total-pcol` budget and `--max-fee-gwei` ceiling, `--priority-gwei` override, the same `safe` sweep before `createVersion`, and an `--economy` pacing flag.
- **Tests:**
  - `tests/publisher_test.mjs`: the simulated schedule matches a reference implementation of `fees.rs` on fixed inputs; a 171-chunk mock publish with a base fee that follows London completes with no "fee rose" pause; a forced external spike pauses and resumes keeping confirmed chunks; resume after re-approval passes (records with different maxFee); the ceiling and budget are enforced; schema-1 migration; a foreign-identity job does not block and can be archived; a publish longer than LOCK_MS completes and pressing Lock still aborts it; the `safe` sweep stores a chunk that disappeared.
  - `pocold/tests/site.rs`: fee derivation and budget refusal unit tests.

### W5. Publish input (Early; MIME addition waits for Q3) (r2 rewritten)
- **Keep the loud refusal as the default.** `prepareSite` (`publisher.js:47`) already refuses unknown extensions ("Unsupported file extension"), so `.php` fails loudly today. Publishing is permanent, so nothing is silently dropped by default in Studio, the extension or the CLI.
- **Explicit exclusion only:** `--exclude <glob>` and `--include-list <file>` in `pocol-site publish`; an "exclude" checklist in the Studio folder import. Every excluded path is listed in the approval.
- **Hard-deny list, whatever the flags:** dotfiles (`.htaccess`, `.git/**`, `.env*`), `config.*`, `*.key`, `*.pem`, `*.sql`, `*.bak*`, `api/**`, `SHA256SUMS*` (it names server-side files such as `api/*.php` and `.htaccess`). The approval says why each was refused.
- **Studio folder import order.** Studio has no ZIP import; it imports a folder (`dist/app.js:55` `#folder-input`, then `validateFiles` in `model.js:10`), which checks the 256-file and 4 MiB limits *before* any filtering, so a folder with `.git/` fails first. Apply the hard-deny filter, then the user's exclusions, then the limits.
- **Explorer for S1** is published only from an explicit file list built from the git-tracked snapshot (`git archive` of `deploy/websites/explorer.pocol.net`, then the include list), never from a host docroot or a test temp copy.
- **MIME (Q3).** `favicon.ico` and `site.webmanifest` have no MIME id. Default: they are not in the include list. If the owner wants them: add `ico` (`image/x-icon`) and a new `manifest+json` id (not `json`; check how `site.rs` normalization ties extension to mime id). Either change touches `src/lib/site.js`, `pocold/src/site.rs`, `publisher.js`, Studio `model.js` and the T16 vectors (`pocold/tests/site.rs`) together; client-only, earlier is better.
- **Tests:** CLI and JS deny lists and exclusion handling agree (shared vector file); every refusal is reported; the explorer include list gives 10 files and the expected unique-chunk count; a folder with `.git/` and 300 files under it imports once filtered.

### W6. Explorer gateway and PHP allowlist (Early; FN-6 "gateway activeSlice") (r2 revised)
`activeSlice` returns only `(ids, w0)` over the **active** list (`PoColSystemV3.sol:431-435`). w0 holds status, tomb, bondState, recPos, actPos, lastShare and registeredAt; `bond` is w2, the reward address w1, exit clocks w3 (`recordWords`, `:392`). Reading w1-w3 for 10,000 members would exceed `MAX_CALLS_V3 = 8192` and `MAX_SECONDS_V3 = 60` (`pocol_rewards.py:479-480`).
- **`deploy/rewards/gateway.py`:**
  - `pocol_getMembers(start, count)` over `activeSlice`, reusing the paging at `pocol_rewards.py:954-968`: count ≤ 256, bounded response, cached per head. Each record: `id`, `status`, `tomb`, `bondState` (1 HELD, 2 BURNED, 3 PAID, 4 ACCRUING), `lastShare`, `registeredAt`. No bond or reward fields.
  - `pocol_getMember(address)`: `recordWords` + `removalStatus` (`:437`) at one height: all four words decoded (bond, reward address, exitHeight, dueHeight, burnHeight, graceStart) and the removal verdict. Used for detail views, the ACCRUING proposer-eligibility flag (bond ≥ B_reg/4, `chain.rs:567-570`), and victims of `registerReplacing` (status 2), which are not in the active list. Token cost like `pocol_getBlockReward`.
  - Forward `pocol_getRemovable` (`rpc.rs:1119`) and `pocol_finality` (`rpc.rs:1085`) with token costs. `pocol_getBlockReward` is a gateway method (`gateway.py:44, :179`), not a node RPC; keep it.
  - Keep `pocol_getMiners` for compatibility; the bundle stops calling it in the refresh batch (W7), because at a full set it returns about 2.5-6.1 MB against the 4 MiB limits (`gateway.py:43, :156, :284`; `api/lib.php:5`).
  - A node RPC that reads membership directly (like `wallet::removable_json`) would be cheaper, but it is a pocold change and makes W6 wait for a node release. Deferred; see Appendix A6.
- **`deploy/websites/explorer.pocol.net/api/lib.php:30-43`:** add `pocol_getMembers`, `pocol_getMember`, `pocol_getRemovable`, `pocol_finality`. Handwritten PHP only; `config.php` is never touched or read.
- **Tests:** `api_security.php` for each new method and its bounds (now run by `websites.yml`); a gateway unit test for paging bounds, the response cap and `pocol_getMember` decoding against a mocked upstream, including bondState 4.

### W7. Explorer bundle v3 semantics (Q1; live check in W11 after Ph5) (r2 revised)
Preferred: the owner provides the explorer source (Q1) and it is rebuilt. Otherwise, anchored edits through the W9 patcher on `assets/index-Cj8htKRO.js`:
- **`Pe` (9:45112)** treats only `bondState===1` as eligible. Replace with labels for every bondState (1/2/3/4) and status (0/1/2):
  - "Held (20 PCOL)";
  - "Earning bond", with "x/20 PCOL" and the proposer-eligible flag (≥ 5 PCOL) shown in the detail view from `pocol_getMember`;
  - "Exited (refund at dueHeight)" from `pocol_getMember`;
  - "Burned", "Paid", "Tombed".
  - Fix the counts (10:470317-472517) to match.
- **Members table:** pages through `pocol_getMembers` outside the 3 s refresh batch (10:451347); columns lastShare ("inactive for N blocks", Θ 360) and registeredAt; bond only in the detail view.
- **Block refresh (r2):** the 3 s batch fetches 20 blocks with `full=true` (`eth_getBlockByNumber [ot(r-t),!0]`, line 10). A full v3 block is about 393 KB of JSON hex, so 20 can reach about 7.9 MB, over both 4 MiB limits. Change to `full=false` and load transactions lazily when a block is opened (the transaction view already uses `full=false`).
- **Transaction view:** decode Registry selectors at `0x…c0c001` (`PoColSystemV3.sol:441-448`): register, registerReplacing, reactivate, setRewardAddress, removeInactive, deregister, claimBond, topUp. Show a replacement with the victim's record from `pocol_getMember`. There are no events.
- **Optional:** per-block reward panel (`pocol-block-reward-v3`), "safe" badges (`pocol_finality`).
- **Afterwards:** rename the hashed file, update `index.html` and `SHA256SUMS.txt` (W9).
- **Tests:** a node test of the extracted label function on fixture records (every bondState and status); a size test that one refresh batch on full blocks stays under 4 MiB; the live smoke in W11.

### W8. Host-only explorer pieces into the repo (owner action first, Q2) (r2 revised)
- **Not in the repo**, lost on the wipe: `pocol_transactions` (`gateway.py:21`), `pocol_geoip` (`:17`), `/opt/pocol-explorer-gateway/geoip-config.json` (`:316`), the systemd unit (may carry `Environment=` values; `POCOL_TX_DB` is read at `:321`), `Install-PoCol-Explorer.sh`, the `gateway.pocol.net` Caddy block, and **(r2)** the host's IDE `api/` directory if any (the IDE snapshot has none).
- **Owner action (Q2).** The owner copies these off the servers before the wipe and **redacts them on the host** (licence keys, account ids, ACME e-mail, basic-auth hashes, database paths). Nothing is pasted into chat.
- **What is committed:** the two Python modules after review, `geoip-config.example.json`, a templated systemd unit and a Caddy template, all with placeholders. A secret scan (gitleaks or GitHub secret scanning) runs on the PR before review.
- **Payloads:** `chainId` as a number and `genesisHash` lowercase (the bundle compares strictly at 10:322 and 10:434100).
- **`deploy/rewards/install_explorer_gateway.py`** with the Caddy template, mirroring the Studio installer's check-then-apply with backup:
  - forces `--bind 127.0.0.1` (the gateway accepts any `--bind` today, `gateway.py:305`);
  - the template sets `header_up X-Pocol-Client-IP {http.request.remote.host}`, because every per-client bucket, including W1's chunk bucket, trusts that header (`pocol_studio_gateway.py:249-251`);
  - the template keeps the exact strings `install_studio_gateway.py:75-89` asserts: `max_size 16384`, "Request body exceeds 16 KiB", the Content-Length expression, `reverse_proxy 127.0.0.1:8088`, and the `/studio` route. Without them `install_studio_gateway.py:80-85` refuses on a fresh server.
- **If no copy is possible,** both modules are rewritten (geo map against a GeoIP source the owner names).
- **Tests:** payload-schema unit tests for both modules; the installer's rendered Caddy block passes `install_studio_gateway.py`'s check and contains the `header_up` line; bind is forced.

### Phase 4 (no extra PR): `SPEC_VERSION` 2
Merges with Phase 4 (1a6d140). Until then `validateProfile` (`sw.js:64-72`) rejects spec-2 genesis, so nothing can be viewed or published on spec 2. The comment at `rp.js:51` is fixed in W13.

### W10. v3 publish→serve e2e with the CLI (Phase 5a)
- **Port `pocold/tests/e2e_site.py`** to a spec-2, V3-program devnet (the `devnet.py init` default once 5a changes `DevnetConfig::new`).
- **Site:** `index.html` linking `style.css` and `app.js`, **(r2)** an ES module graph (`main.mjs` → `lib.mjs` → `util.mjs`, plus a literal `import()`), a JSON file, an SVG, a woff2 font and a PNG. Limits: a 1,048,576 B file accepted and 1,048,577 refused; about 4 MiB − 4 KiB accepted (about 171 chunks) and 4 MiB + 1 B refused before signing; 256 files accepted, 257 refused; a manifest spilling into 3 chunks.
- **Assertions:**
  - each chunk tx's gasUsed ≤ declared `200000 + 260·size`; at most one full chunk per block; repeat store idempotent;
  - **(r2)** the base-fee trajectory matches the publisher's simulation within the margin, there is no "fee rose" pause on an otherwise idle chain, and the total spent is ≤ the approved budget; economy mode keeps the base fee at the floor;
  - supply invariant: no burn beyond gas (FN-2a);
  - `pocol-site fetch` digests identical on all 3 nodes, before and after a node restart;
  - v2 `setCurrent` and `revoke`; concurrent publishers;
  - CODE_CAP-halt and AcctCap give a clear CLI error (small-cap devnet);
  - **(r2)** a chunk stored by another account, then reorged out before `safe`, is re-stored by the `safe` sweep (a devnet partition test if the harness supports it; otherwise a mocked upstream in the CLI unit tests).
- **CI:** new `sites-e2e` job; per PR the about 1 MiB case; the 4 MiB case nightly or on dispatch (about 175 blocks). Re-run after FN-3.

### W11. Browser e2e on v3 (Phase 5a + W1, W2, W2b, W4) (r2 revised)
- **`e2e_browser.mjs` and `e2e_integrated.mjs` on the v3 devnet.** The viewer opens the W10 site; assert CSS applied (`getComputedStyle`), JS ran, the module graph resolved (W2b), the font loaded, `fetch` to the network blocked, `window.pocol`/EIP-1193 `rpc_read` works (`eth_blockNumber`, `eth_call`).
- **Test build through the fault mechanism (r2).** `validateEndpoint` (`studio-network.js:11-20`) accepts loopback only as `http://127.0.0.1:<port>/` with no path, and refuses loopback for RN. So the test build uses a new `/*@fault testGateway*/` region in `studio-network.js`, handled by the existing `build.mjs` fault mechanism (`fault === …`, :70). It rewrites `NETWORK` to the devnet identity and `http://127.0.0.1:<port>/studio/rpc`, and allows that one loopback URL with path for RN, so the release RN path is exercised. Fault builds get the "[TEST FAULT …]" name and go to `dist-fault-*`; W3's packager refuses them.
- **`tests/e2e_publisher_gateway.mjs`:**
  - a local Studio gateway with `--network devnet.json` (W1);
  - drive Studio → handoff to `publisher.html` → publish an about 1.6 MB HTML/CSS/JS project through the gateway → view it through the same gateway;
  - assert: fees from the node (W4); no "fee rose" pause; the open succeeds within the budget;
  - **(r2) multi-viewer:** a small test proxy in front of the gateway sets distinct `X-Pocol-Client-IP` headers per viewer (otherwise every local viewer is one client, `:249-251`); with 3 concurrent viewers and one 4 MiB site (nightly), every viewer opens and cache hits are counted;
  - **(r2) long publish:** a publish longer than LOCK_MS (time-scaled with a test LOCK_MS) completes; pressing Lock aborts it;
  - **(r2) background tab:** run the publisher in a background tab (headed Chrome in the nightly job) and record per-tx latency; if Chrome's intensive timer throttling stretches receipt polling (`pollIntervalMs` 1000), move polling into the service worker or keep the page visible with a notice. This is a measured risk, not yet observed.
- **Explorer smoke.**
  - `php -S` on a temp copy (under `$RUNNER_TEMP`) of the explorer site, with a **test-generated** `config.php` written from a committed `api/config.example.php`. The example's keys are taken from what the tracked readers (`api/lib.php`, `api/rpc.php`) use; the real host file is never read.
  - It points at a local explorer `gateway.py --v3-network` on the devnet.
  - The bundle loads; an ACCRUING member shows "Earning bond"; a `registerReplacing` tx is decoded with its victim; one refresh batch stays under 4 MiB. After W6 and W7.
  - `.htaccess` deny rules are not exercised by `php -S`; they are covered by W9's file test.
- **IDE smoke (r2).** The IDE snapshot has no `api/` directory and its default `rpcUrl` is the Studio gateway (`w9.rpcUrl`). The test patches a temp copy with the W9 tool (identity and `rpcUrl` pointing at the local Studio gateway), serves it, and compiles and deploys a small contract on the devnet through the extension. `L9`/`R9` compare the frozen constants, hence the patch.
- **Re-run `browser_security.mjs`** after every bundle patch (automatic in `websites.yml`).

### W12. `pocol_getCaps` pre-check (after FN-1)
- `src/lib/transport.js:12-18` RECV_LIMIT entry; Studio gateway allowlist; `publisher.js` refuses before signing if remaining CODE_CAP or ACCT_CAP is below the site's need; the same in `pocol-site`.
- **Test:** a cap-test devnet near full gives a refusal before any signature (rehearsal S1's last clause).

### W13. Phase 6a follow-ups (Ph6a)
- `src/lib/forks.js:6` `MAX_FORK_VERSION` (2 today) set with `profile.rs` (1 after Phase 6a); update the text at `:7` and `forks_test.mjs:11-26`; the comment at `rp.js:51`.
- If Phase 6 refuses spec 1: regenerate `tests/data/devnet-spec.hex` and `rp_fixture.mjs` as spec 2.

### W14. Phase 7 + FN-6b (Ph7)
- **Extension:**
  - Delete `PUBLIC_FORKS_KEY`/`storedPublicProfile` (`sw.js:46-48`), the `profiles.publicSchedule` handler (`sw.js:539-546`), and the options "activation" section and handlers (`options.html:33-40`, `options.js:9-11, :28-36`). Pin the schedule to `[[1,'0']]`.
  - Texts: `options.js:15`, `options.html:24, :44`. Rewrite or delete `studio_test.mjs:83-155`.
  - Bump `package.json:3` from 0.4.1 to the Q7 version. READMEs: `README-IDE.md:57-63`, `README-AR.md:55, :61`, `README-STUDIO.md:23, :48`.
  - **(r2) Upgrade notes:** export the wallet, remove 0.4.1, then load the new version (two installed versions both inject into studio/ide.pocol.net, and wallet data stays with the old extension id). Q7 decides whether to pin a manifest `key` so the id is stable.
  - Rename the release job (`full-nonce-upgrade.yml:76-95`).
- **(r2) Release integrity:** sign the release zip with an owner key (minisign or `ssh-keygen -Y sign`), publish the public key in the repo and on pocol.net, and verify in `package-release.py --verify`. Reproducible zip (fixed timestamps and order) with a CI job that builds twice and compares hashes. The `.sha256` next to the zip stays, but is no longer the only check.
- **Download links** through W9: Studio `dist/app.js:43`; IDE bundle 1027:101031 (href and "0.4.0" label) and the "test PCOL" text (~1027:47137); pocol.net `index.html`, `chrome-extension.html`, `guide-content.js`, `SHA256SUMS.txt`.
- **IDE deploy doc:** rewrite `DEPLOY-README-AR.txt`'s "replace files in place" text for the fresh-network address change.
- **Studio:** fix the Arabic "شبكة اختبارية" label at `app.js:5` without changing the id counts (3/1/0); `checkNetwork` (`app.js:53`) canonical: `Number(cid)` and lowercase both genesis values.
- **Explorer gateway:** `--v3-network` mandatory; delete the `:40-41` defaults and the v1/fork-schedule path; update FILES.
- **`Initialize-PoColNetwork.ps1`:** server IPs as parameters; explorer seed list from the invitation through W9.

### W15. Rehearsal identity switch (after Ph8) (r2 revised)
- `set_network_identity.py --from <retired> --to <rehearsal invitation>` plus the W9 websites section, on an unmerged rehearsal branch: `studio-network.js` (chainId, genesisHash, spec-2 genesisPre), Studio, both gateways, explorer and IDE bundles, pocol.net.
- **The rehearsal chain id differs from mainnet** (design FN-0 already fixes both ids). Check it here and add it to `retired-networks.json` in W16, because EIP-1559 signatures bind only the chain id: rehearsal txs signed with reused keys could otherwise replay on mainnet.
- **Rehearsal build:** manifest name "[REHEARSAL] …", output file `PoCol-Rehearsal-<ver>.zip`, refused by the W3 packager as a release; given to the owner only.
- **Origins (Q5b).** The content scripts match only `https://studio.pocol.net/*` and `https://ide.pocol.net/*` (`build.mjs:64`). S1 therefore needs either rehearsal builds of Studio and IDE deployed over the production domains (a server action, after the old network is stopped), or rehearsal origins and host permissions in the rehearsal build only (the W3 exact-match test then runs with a rehearsal expectation). Recommendation: production domains, after the old network is stopped, so the release origins are what is tested.
- **S1:** the explorer bundle (W5 include list) and a 4 MiB synthetic site with a module graph published; digests identical on all 4 nodes; viewer, Studio and IDE open both through the gateway within the budget; the `pocol_getCaps` check runs.
- Each S1 deploy and DNS change is listed in §3 and needs its own owner approval.

### W16. Phase 10 identity pin
- The same command `--from <rehearsal or retired> --to <mainnet invitation>`; turn on the retired-id check in W9 `--check`; add the rehearsal id to `retired-networks.json`.
- Tests read `N.NETWORK.chainId` instead of the literal (`publisher_test.mjs:13`, `contracts_test.mjs:13`, `contract_ui_test.mjs:27, :83`, `studio_test.mjs:16, :119`).
- Studio and explorer source links point to the release commit.
- pocol.net `reward.js`/`api/reward.php` to `pocol-block-reward-v3`.
- The operator sets the new values in the host `config.php` (`chain_id` as an int, lowercase `genesis_hash`). Owner action on the host; never read or committed by us.

## 2. Needs the genesis identity (Phase 10, or the rehearsal invitation for W15)
- Extension `studio-network.js` (chainId, genesisHash, spec-2 genesisPre), `publisher.html:9`, `contract.html:7`.
- Studio `app.js:5, :6, :53`, `README.md`.
- Studio gateway `:19-20` and installer `:113-114, :144-153`.
- Explorer gateway defaults (if still present) and the explorer `config.php` (operator).
- Explorer bundle constants and seeds, `index.html` meta.
- IDE frozen network object, `rpcUrl` if changed, and `DEPLOY-README-AR.txt` network block.
- pocol.net `network.js`, `reward.js`, `api/network.php`, `api/reward.php`.
- Source and commit links (release tag from Ph8).
- The decoder, the module loader, the fee schedule and all site-storage logic need **no** identity.

## 3. Needs the servers (only with explicit owner approval for each step) (r2 revised)
- **Before the wipe (ceremony precondition, W0):** the owner copies and redacts the W8 pieces and the Studio/IDE/pocol.net header configuration (Q2), on the host, without pasting them anywhere.
- **Rehearsal S1 (each step approved separately):** stop the old network; DNS for any new host (Q5); install the explorer gateway, then the Studio gateway; deploy rehearsal Studio and IDE (Q5b); after S1, restore.
- **After the ceremony, in this order, with a rollback snapshot before each step:**
  1. Install both gateways in check-only mode, then apply (W8 installer and `install_studio_gateway.py`, both check-then-apply with backup). Caddy needs DNS pointing at the host for ACME.
  2. Health check both gateways (identity, `--v3-network`, header forwarding).
  3. Owner edits `config.php` on the host.
  4. Upload explorer, Studio, IDE and pocol.net bundles. Uploads contain only the files listed in each `SHA256SUMS.txt`; nothing under `api/` is deleted; `config.php` is never transferred.
  5. Download links and the extension zip in `downloads/` (no Chrome Web Store, `README-STUDIO.md:19`); remove the 0.4.1 zip and its `.sha256`.
  6. Owner-run post-deploy smoke: security headers on Studio, IDE, pocol.net and explorer match the committed templates; the explorer `.htaccess` hash matches the repo; cache headers do not serve old hashed bundles. `/api/config.php` is never probed.
- Nothing in W0-W14 deploys anything.

## 4. Technical decisions (recommendation; no owner input needed unless they object)
- **D1 (r2 changed). Built-in profile placeholder.** After Phase 4 merges, replace the retired spec-1 pin (chain 1599846715) in `studio-network.js` with a clearly marked spec-2 placeholder from the devnet spec fixture, via `set_network_identity`, so main no longer ships old-network identity and Phase 10 stays a single `--from/--to`. The W3 packager refuses a build with the placeholder, so no release can ship it; the published 0.4.1 zip is unaffected until W14/W16. This matches FN-6 ("built-in profile placeholder") and also removes the Phase 6 dependency on spec-1 decodability.
- **D2. Gateway identity.** Constants (tool-switched) plus `--network` in the explorer's `--v3-network` format for tests and the rehearsal. No mandatory config file before Phase 10.
- **D3 (r2 changed). Chunk cache.** Fill only from `safe`, serve only for `latest`/`safe`, shape check only, raw bytes, 24 MiB, keyed by genesis. No CREATE2 re-derivation (Appendix A1).
- **D4. E2E cost.** Per-PR CI runs about 1 MiB; 4 MiB, multi-viewer, background-tab and long-publish cases run nightly or on dispatch.
- **D5 (r2 new). Module support.** Support relative static and literal dynamic imports in the loader (W2b); refuse cycles.
- **D6 (r2 new). Fees.** Simulated London schedule, one budget and one ceiling, just-in-time signing, optional economy pacing.

## 5. Open questions for the owner
1. **Q1. Sources.** Do you have the source code of explorer.pocol.net and ide.pocol.net? Without it, the explorer's v3 logic and identity changes are anchored edits to minified bundles: doable with checks (W9), but fragile.
2. **Q2. Before the US servers are wiped,** can you copy, and redact on the server, these files? Please do not paste them into chat.
   - `pocol_transactions.py`, `pocol_geoip.py`, `geoip-config.json`, the systemd units, the Caddyfile, `Install-PoCol-Explorer.sh`;
   - **(r2)** the IDE host `api/` directory if it exists, and the web-server security headers (CSP, frame-ancestors) used for studio.pocol.net, ide.pocol.net and pocol.net.
   - **Q2b.** Does the geo map use a licensed GeoIP database?
   - **Q2c (r2).** Which server runs `/opt/pocol-explorer-gateway` and the `gateway.pocol.net` Caddy?
3. **Q3. MIME types.** Should published sites support `favicon.ico` and `site.webmanifest`? That adds two MIME types before genesis; otherwise they are left out when publishing.
4. **Q4. The on-chain copy of the explorer.** Storage reference only (recommended), or a working explorer inside the viewer? A working one needs the source rewritten to read through the extension and `pocol_*` methods added to the bridge's frozen `ALLOW` (`bridge.js:131`), which widens what every on-chain site can call; that would need matching limits in the bridge and the gateway and bridge tests.
5. **Q5. The rehearsal gateway.** gateway.pocol.net after the old network is stopped, or a separate host such as rehearsal-gateway.pocol.net (needs DNS and a permissions change in the rehearsal build)? Should the rehearsal extension build go to you only?
   - **Q5b (r2).** Where do rehearsal Studio and IDE run: over studio.pocol.net and ide.pocol.net after the old network is stopped (recommended), or on separate rehearsal origins?
6. **Q6. Network name.** Keep the display name "PoCol Mainnet" in the explorer, IDE, Studio and extension?
7. **Q7. Extension version and distribution.** New version (for example 0.5.0)? Chrome Web Store still out of scope (zip only)? **(r2)** Should the extension carry a fixed manifest `key` so its id stays the same across installs, and will you hold the release signing key (minisign or SSH)?
8. **Q8. Viewing target.** Acceptable time to open a 4 MiB site, and how many simultaneous viewers should the gateway support? This sizes W1's chunk budget, its reserved slots and W2's per-viewer width.
9. **Q9 (r2).** For large publications, is the slower economy mode (about half speed, near-zero burn) wanted as an option, or as the default?

## 6. Blocking summary
- **Now:** W0.
- **After Phase 4 merges (or stacked on it):** W9, W1, W2, W2b, W3, W4, W6, W5 (refusal/exclusion part), W8 (as a rewrite), D1.
- **Phase 4:** SPEC_VERSION 2; nothing can be viewed or published on spec 2 before it.
- **Phase 5a:** W10, W11, live checks of W7. FN-3 forces a re-run of W10.
- **FN-1:** W12.
- **Phase 6a:** W13.
- **Phase 7:** W14.
- **After Phase 8 / rehearsal genesis:** W15.
- **Phase 10:** W16.
- **Owner answers:** Q1 gates W7's method; Q2 gates W8 and must happen before the wipe; Q3 gates W5's MIME part; Q5/Q5b gate W15; Q7 gates W14's signing and id; Q8 sizes W1/W2.
- **Servers:** each step in §3 only with explicit owner approval.

## Appendix A. Critique points rejected or modified (with reasons)
All other points of both critiques were verified against the code and adopted as written (marked r2 above).

- **A1. Critique 1 D5 / Critique 2 D9: vendored Keccak and CREATE2 re-derivation in the gateway: rejected; Critique 2's alternative adopted.** The EVM spec is CANCUN (`code_caps.rs:98`), so under EIP-6780 code cannot change once created; only a reorg removes it, and filling from `safe` covers that. Viewers verify every chunk. The derivation would only protect cache memory from non-chunk code, which the shape check, size cap and LRU bound already do. Dropping it avoids pure-Python Keccak CPU per miss and a change to `build_installer.py` `files` and the FILES counts. Critique 1's correction of the derivation formula (initcode = 11-byte prefix ‖ data, `site.js:21-22`) is right but moot.
- **A2. Critique 1 D1's closed-form trajectory `base_next·(1 + (gasUsed − 4M)/(8·4M))^i`: modified.** It assumes constant gas per block and ignores the deploy, createVersion and setCurrent txs, the integer rounding and the floor in `fees.rs`. The plan simulates the exact recurrence of `fees.rs` per planned tx, adds a one-block margin, and signs just in time against the live base fee. The substance (per-tx fees, one budget, one ceiling, no equality check at `:181`, checkpoint schema 2) is adopted.
- **A3. Critique 2 D2: serve cache hits for `finalized`: rejected.** `finalized` is older than `safe`; code present at `safe` may not exist at the finalized height. Hits are served only for `latest` and `safe`.
- **A4. Critique 1 "the ≥ 2^32 case is moot": adopted with a change.** The positive ≥ 2^32 test is removed. A no-crash test for an oversized configured id is kept, because the current `to_bytes(4)` raises OverflowError, which `do_POST` does not catch.
- **A5. Critique 2 D7 "rehearsal chain id ≠ mainnet": already decided.** Design FN-0 fixes both chain ids. The plan adds a check in W15 and the retired-list entry in W16 only.
- **A6. Critique 1 D6 "better: a node RPC in rpc.rs": deferred.** A pocold RPC needs a node release and would take W6 out of the Early set. The gateway reads w0 pages through `activeSlice` and single records through `recordWords` + `removalStatus`, within the existing call budgets. A node RPC can follow if the gateway's cost is too high.
- **A7. Critique 2 D8 "record the FN-6 split in the design": adopted as a separate docs PR (W0).** This plan may write only its own file, so the design and removal plan are amended by W0, not here.
- **A8. Critique 1 G5 Chrome background-tab throttling: adopted as a measurement, not a design change.** The critique marks it unverified; W11 measures it and names the fallback (polling in the service worker, or a visible-page notice).
- **A9. Critique 2 D10 "CI builds twice and compares hashes": adopted with a precondition.** Zip output is not reproducible today (timestamps, order). W14 makes it reproducible first, then adds the double build. Manifest `key` versus Web Store is an owner decision (Q7), not a default.
- **A10. Critique 2 D3 hard-deny of `*.sql`, `*.pem`, `*.key`:** these extensions already have no MIME id and are refused today. They stay in the hard-deny list so that a future MIME addition or an `--include-list` cannot publish them.
- **A11. Critique 2 D9 "RN per-viewer limit 2": adopted as the default, pending Q8.**
- **A12. Critique 1 G1 "W2's 4 matches rpc_slots" and the "fetchSite :189-205" reference:** both are correct criticisms; fixed (chunk-level pool, default 2 for RN; `site.js` has 86 lines, `fetchSite` is `:69-85`). The `setInterval` lock check is at `publisher-ui.js:141`, not `:157`.
