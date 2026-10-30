# LP9 — Chrome MV3 extension (local devnet)

Source: `prototypes/pocol-extension`. Specification: `development/m1/reference/browser.md`
(D14, D26, D34, D58, D77–D84, D95–D101), gates in `validation.md` (BR1–BR24, X8).
Scope: local `simnet` profiles only (OWNER-STANDING-AUTHORIZATION.md). Endpoints are
`http://127.0.0.1:<port>`; optional remote hosts are declared but not enabled.

## Architecture (as built)

| Component | File | Spec |
|---|---|---|
| Network profiles: genesisPre is decoded and hashed; chainId and genesisHash must match; netKey = keccak(RLP(['PoCol-net-v1', chainId, genesisHash])) | `src/sw.js` | D58, D77, D79 |
| HeaderNetCheck: the endpoint's chainId and genesisHash must equal the profile (4901) | `src/sw.js` | D79 |
| RP window: n ≤ 13 headers, items 1, 1b, viewFuture, 2–9, viewTargetCeil; trust text shown outside the frame | `src/lib/rp.js` | D77, D80–D84 |
| Site fetch: `current()`, manifest chunks, keccak of manifest and of every file, CREATE2 address of every chunk | `src/lib/site.js` | storage.md T16, D27 |
| DNR session rules per tab: block {2·tabId, prio 1}, allow {2·tabId+1, prio 2, `|chrome-extension://ID/`}; install verified, fail closed | `src/sw.js` | layer 2 |
| Sandbox page: CSP from browser.md; the loader is the only inline script and its sha256 is the only script hash | `build.mjs`, `src/sandbox-loader.js` | layer 1, D26 |
| Loader: RtcLockdown, nested-frame guards on every DOM insertion API and HTML sink, sanitization, blob: rewriting (src, href, srcset, url(), @import), ordered script execution, fetch/XHR served from the verified file map, link interception | `src/sandbox-loader.js` | D26 step 5, D96 |
| Viewer: one MessageChannel per frame, bridge text forwarded unparsed, replies of torn-down frames dropped, confirmations outside the frame, second `load` tears the frame down | `src/viewer.js` | D26, D99 |
| BridgeAuth: Bpre size, integer token bucket, B0 strict JSON (no duplicate keys, depth ≤ 8), B1 kind, B2 frozen Map allowlist, B3 exact schemas | `src/lib/bridge.js` | D95–D97 |
| B4 routing: pendingReads 4 / global 16, LogClient (pocol_getLogs, ≤ 1024 blocks, −32020), nav bucket 6000/1/2000, one pending confirmation, wallet one pending and ≤ 5/min | `src/sw.js` | D97, D99 |
| SiteStorage: literal quota, 4096 entries, one atomic set per write, per-key serialization; StoreQueue admission 8 / 256 KiB per session, 64 / 4 MiB global | `src/lib/sitestore.js` | D34, D96, D101 |
| Wallet: PBKDF2-SHA256 600000 + AES-GCM seed, HKDF key per netKey, 15-minute lock, EIP-1559 signing; the exact bytes and hash are shown before signing and submission | `src/lib/wallet.js`, `src/sw.js` | D14 |

Vendored crypto: `@noble/hashes` 1.8.0 and `@noble/secp256k1` 2.3.0 (MIT, licenses copied into `dist/vendor`).

## Fault builds (negative controls, never in the release)

`node build.mjs --fault=<name>` writes `dist-fault-<name>/`. Fault regions are marked in the sources as
`/*@fault NAME*/release/*@else*/fault/*@end*/`. The release build keeps only the release text, and the
build fails if any marker survives.

| Fault | Effect | Gate it must break |
|---|---|---|
| `noDnr` | DNR rules not installed | proves CSP alone holds (canary 0) |
| `cspOpen` | permissive sandbox CSP | proves DNR alone holds (canary 0) |
| `cspOpenNoDnr` | both | positive control: the probes reach the canary |
| `noRtc` | RtcLockdown removed | positive control: RTCPeerConnection is reachable |
| `allowAll` | B2 and the RpcReadClient re-check disabled | BR1 must fail (raw tx reaches RpcTap) |
| `noRefill` | bucket never refills | BR19a(a3) must fail |
| `quotaNoSubtract` | replacement does not subtract the old entry | BR17e(e7) must fail |

## Amendment LP9-DNR-01 (recommended option, applied under the standing authorization)

browser.md names the per-tab rule ids `{2·tabId, 2·tabId+1}`. In Chromium, tab ids reach 1.4·10^9,
so `2·tabId` exceeds the 32-bit rule-id range and `updateSessionRules` rejects it (observed:
"expected integer, found number"). Options: (1) per-tab slot `k` with ids `{2k, 2k+1}`, the
condition still `tabIds: [tabId]`, map rebuilt from `getSessionRules()`; (2) hash of tabId (collisions);
(3) one global rule set (loses per-tab scoping). Chosen: (1). Rules are installed and verified before
any fetch (fail closed). They are removed when the tab closes or its viewer session ends, unless a new
open for the same tab is in progress. `main_frame` is not blocked, because the frame has no
`allow-top-navigation`, so leaving the viewer restores normal browsing.

## Gate coverage

Evidence: `node tests/unit.mjs` (Node, release and fault builds) and `node tests/e2e_browser.mjs`
(Chromium with the extension, live 3-node pocold devnet, RpcTap, HTTP and UDP canaries).

| Gate | Status | Evidence |
|---|---|---|
| BR1–BR13 | claimed | unit (every listed vector); Chrome subset end to end with zero RpcTap requests |
| BR14 | claimed | Chrome: grant after a click, one eth_sendRawTransaction, raw = displayed bytes, keccak(raw) = displayed hash = result, included with identical receipts on 3 nodes; reject → 4001; wrong from → 4100 |
| BR15 | claimed (LN, pocold) | ORIGIN_DENY: −32601 with Origin, served without (e2e_integrated); extension requests carry the extension Origin (RpcTap) |
| BR16 | partial | 10^4-message invariant fuzz (no exception, accepted ⇒ allowlisted and schema-valid, fixed codes). No independent BridgeRef model |
| BR17a–g | claimed | unit (schema d, table e1–e9, legacy g); Chrome: a (persist + reload), e (24 messages, total 925694), f (isolation and own quota) |
| BR18a–h, BR18-L | claimed | Chrome: a, b, c (404), e (new tab via CDP history), f, h; unit: d, g, L |
| BR19a | claimed | unit, exact table a1–a7 |
| BR19b | partial | Chrome b1 (4 forwarded, 5th pending with eth_call held 2.5 s); b2–b5 not measured |
| BR19c | claimed (simplified) | Chrome: 60-message burst gives rate rejections, later read succeeds |
| BR20 | partial | second `load` teardown, nav bucket (4th rapid navigation → −32005 {nav}), replies of torn-down frames dropped; the BR20a table is not reproduced |
| BR21 (StoreQueue) | partial | admission 8 / 256 KiB / 64 / 4 MiB, per-key FIFO, quota decided before any write (zero `set` on 4300), STORE_ACTIVE_GLOBAL = 2, STORE_QUEUE_WAIT (−32005 {store}), STORE_WRITE_DEADLINE (−32603 {storeTimeout}, key blocked until Chrome settles); the explicit replyState/ownState trace (BridgeTrace) is not reproduced |
| BR22 (D103) | claimed (core) | `src/lib/records.js`: EpochFence (names gate 8 with sweep after T_LATE, generation window 4, confirmed epoch item, 3 retries), non-replaceable `(epoch, seq)` records `{fmt:2, epoch, seq, tomb, b64}`, RecoveryGate (newest valid, ≤ 4 gets, 5 s, checkpoint with one retry, prune below). Unit: I57 late write never wins, late write applied first becomes the base, corrupt newest skipped, all-corrupt → failed, gate/window/epoch-write failures. Chrome: service worker stopped over CDP → frames torn down with the loss notice → reload recovers the committed dictionary under epoch 2. RecoverySlots/ReadSlots FIFOs and gateId bookkeeping are simplified |
| BR23 (D104 disk) | partial | names admission `names + 1 + LATE_NAMES + EPOCH_NAMES_MAX ≤ STORE_RECORDS_MAX` and byte admission with LATE_RESERVE and META_RESERVE (4300 {sites|disk}, zero `set`); DiskLedger (D105) reserves synchronously with `unres/unresN`, released after the settle is re-measured |
| BR24 (D106 admin delete, tombs) | not claimed | no administrative delete UI; `tomb` is always false |
| D98 RecvGuard | claimed | unit: Content-Length, streamed over-limit abort, fatal UTF-8, depth 16/17, duplicate keys, error clipping, unlisted method never sent, deadline, pools |
| Network isolation | claimed | release: HTTP canary 0, UDP canary 0. noDnr: 0 (CSP alone). cspOpen: 0 (DNR alone). cspOpen+noDnr: hits > 0 (the probes work) |
| X8 (RtcLockdown) | measured | without RtcLockdown the sandbox sends STUN packets (UDP canary > 0); with it, RTCPeerConnection is undefined and the canary is 0. Residual: any nested realm would undo it, so every DOM insertion API and HTML sink refuses frames/objects, and contentWindow is null. A window[i] path needs a frame, which cannot be inserted |
| RP window D77–D84 | claimed | live: real Rust headers accepted; single mutations give netChain, netGenesis, viewFuture, 2, 3, 4, 5, 6, 7, 8, 9, 1, viewIncomplete; viewTargetCeil observed on a T_blk=1 devnet |
| netKey binding | claimed | tampered genesisHash rejected at import; another genesis pointed at this node → 4901 |
| T12 site matrix | partial | one site with relative CSS + @import, folders, classic and inline scripts, localStorage, fetch/XHR JSON, PNG in img and in CSS url(), internal links, contract read/write. No woff2, form or importmap-module site |
| ProfileMutex / eligibilityEpoch | not claimed | single-user local prototype; eligibility is checked as profile + unlocked wallet + approved grant |
| Remote endpoints | not enabled | profiles accept http://127.0.0.1:<port> only |

Independent review: [REVIEW-RESOLUTION.md](REVIEW-RESOLUTION.md) (4 findings, all fixed).
