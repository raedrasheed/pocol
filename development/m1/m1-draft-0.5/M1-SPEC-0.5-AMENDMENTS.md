# M1 — Specification Draft 0.5: Amendments

Status: **review draft, NOT approved. Phase S (specifications) only.**
- No production code, no deployments, no transactions, no installs, no subagents.
- **Nothing in 0.5 was executed by the author.**

Inheritance: 0.5 = 0.2 + 0.3 amendments + 0.4 amendments + the amendments below. Unchanged material is inherited by reference. The 0.4 tools `bridge_ref.py` (schemas, bucket) and the 0.3 tools (`rlp_strict`, `m1model`, `m1paths`) are used unchanged by `tools/run_checks_05.py`.

| ID | Issue / row | Subject |
|---|---|---|
| R5-01 | C14 | Correct header accounting at the 65536 boundary (65561, not 65558) |
| R5-02 | C15 | httpsUrl decided by a native Node WHATWG oracle; the Python approximation is withdrawn |
| R5-03 | C16 | Internal pending-request handles, independent of frame ids |
| R5-04 | C17, C18 | CR-M1-01 rev 2: EIP-1186 semantics, address versus slot hashing, absence branch, envelope, −32021 retry budget |
| R5-05 | B4 | SiteStorage and Navigator semantics |
| R5-06 | B9 | The full BR17e table (24 messages, generated, retained in full) |
| R5-07 | B6 | Single write-path flow, with checkable properties and mutants |
| R5-08 | B8 | BR1–BR14 and BR17–BR18 messages and replies; signed txA; RpcTap zero-request criterion; test-key policy TK-1 (P) |
| R5-09 | U34 | Resolved: the BG1 lower bound 61824 is a conservative envelope reserve; guard precedence documented |
| R5-10 | B7 | Import-graph rule versus global-fetch rule: a concrete CI realization; no longer an owner question |

## R5-01 — Boundary accounting (C14)

0.4 expected `rb-2848-does-not-fit` to be 65558 bytes, computed with 3-byte list headers at every level. Codex measured 65561.

A long-form list header is 1 + L bytes:
- L = 2 for bodies of 256–65535 bytes;
- L = 3 for bodies of 65536 bytes or more.

| Level | 2847 refs: body / header / total | 2848 refs: body / header / total |
|---|---|---|
| Chunks list | 65481 / `f9ffc9` / 65484 | 65504 / `f9ffe0` / 65507 |
| File | 65524 / `f9fff4` / 65527 | **65547** / `fa01000b` / 65551 |
| Files list | 65527 / `f9fff7` / 65530 | **65551** / `fa01000f` / 65555 |
| Top level | 65532 / `f9fffc` / 65535 | **65557** / `fa010015` / **65561** |

At 2848 refs, three bodies cross 65535, so each of their headers gains one byte: 65558 + 3 = 65561.

The P39 bound (at most 2847 content references, at most 2850 requests per load) is unaffected. Its proof uses 54 + 23N only as a lower bound on the length.

The runner rebuilds both manifests and compares every level's body, header bytes and total (`vectors/request-bound-c14.json`). The other 0.4 request-bound fixtures passed in the Codex run and are inherited.

## R5-02 — httpsUrl oracle (C15)

The 0.4 Python approximation disagreed with WHATWG URL parsing in 6 of 9 Codex cases. It is **withdrawn as an oracle**.

- **Single source of truth: `tools/url_oracle.cjs`** (Node, native `URL`). It evaluates the whole FD:L1165 predicate:
  - printable ASCII 0x21–0x7e;
  - length ≤ 2048;
  - the literal prefix `https://`;
  - a successful parse with protocol `https:`, a non-empty hostname, and an empty username and password.

  It writes `results/url-oracle-0.5.json`.
- **Python harness:** `tools/bridge_ref_05.install_oracle` replaces the 0.4 `_https_url` with an exact-string lookup in that file.
  - **A value that is not in the file raises `OracleMissing`.** It is never accepted, and the old approximation is never consulted. The runner tests this with `url.unknownRaises`.
  - All 0.4 bridge cases are re-run under the oracle.
- **Cases** (`vectors/url-cases.json`):
  - Codex's 9 cases, with Codex's expected values;
  - 22 author cases, each tagged `source: author`:
    - every BR18g URL;
    - the case-sensitive prefix;
    - space and non-ASCII characters;
    - ports `:`, `:443` and `:65536`;
    - extra slashes and backslashes;
    - empty credentials;
    - hexadecimal IPv4 and a 5-part IPv4;
    - percent-encoded host bytes;
  - length 2048 versus 2049.

  A disagreement between an author expectation and the oracle is reported as a failure for review. It is not resolved silently.
- **Browser behaviour** (Chrome's URL parser inside the extension) remains a Phase A check (E06). Node and Chrome both implement the WHATWG URL Standard. This is stated as an assumption, not proven.

## R5-03 — Pending-request handles (C16)

**P40.** When RpcReadClient or LogClient accepts a request:
- it allocates an **internal opaque handle** from a worker-wide counter;
- the counters, orphaning, settlement and the final reply all key on that handle;
- the handle is never derived from the frame-supplied `id`.

The frame's `id` is kept only to build the reply envelope, unchanged (FD:L1113). So:
- **Repeated ids** within one frame, **equal ids** across frames, and equal ids across sessions are all accepted. They are processed independently and each gets its own reply with the id as sent. **No rejection policy is added**, because the baseline defines none.
- The session limit of 4 and the global limit of 16 count handles, not ids.

Fixtures PH1–PH6 (`vectors/pending-handles.json`):
- PH1 reproduces the Codex probe (`A` and `B` both use id 0; the reply for A decrements A).
- The others cover a repeated id in one frame, the same id in a torn-down frame and its replacement, completion order, the limit counted in handles, and orphan settlement across sessions.

The model is `bridge_ref_05.Pending`.

## R5-04 — CR-M1-01 rev 2 (C17, C18)

See `CR-M1-01-REV2.md`. It supersedes §2–§6 of the 0.4 CR; §1 and §7–§11 stay in force with the noted edits. Summary:
- **EIP-1186 semantics.** Required result fields: `balance`, `codeHash`, `nonce`, `storageHash`, `accountProof`, `storageProof`. `address` is optional; if present it must equal W. The proof is always checked against the requested W, whatever the response says.
- **Hashing.** The account path is keccak256 of the **20-byte address**. The storage path is keccak256 of the **32-byte slot**.
- **Decision order.** A verified absence (`noWebsite`) is branched **before** any comparison with a present account's leaf. An empty `accountProof` is allowed only when the anchored `stateRoot` is the empty-trie root. An empty storage proof is allowed only when `storageHash` is the empty-trie root.
- **Strict JSON-RPC envelope:**
  - a single object (no batch);
  - `"jsonrpc":"2.0"`;
  - `id` an integer equal to the request id;
  - exactly one of `result` and `error`;
  - no other fields;
  - depth ≤ 16 and no duplicate keys (FD:L1244).
- **−32021 handling.** Wait `retryAfterMs` and resend the same request with the same anchor. This is not a restart.
  - At most 3 retries per request (FD:L1004 precedent). The 4th −32021 fails the attempt.
  - `retryAfterMs` must be an integer in 0–10000 ms (P); otherwise the attempt fails.
  - At most 20000 ms of waiting per load (P); beyond that the load ends `unavailable` with zero frames.
  - At most 48 sends per load.
- **Fixtures** (`vectors/proof-response-cases.json`): 12 envelope cases, 7 error cases, 23 result cases, 7 budget simulations, and path known-answer values for slots 0–2, the empty-trie root and keccak(""). **These decision tables assume a verifier's outcome. They do not demonstrate MPT verification.**

## R5-05 — B4: SiteStorage and Navigator semantics

The model is `tools/sitestorage_ref.py`; the fixtures are in `vectors/sitestorage-nav-cases.json`.

**SiteStorage** (FD:L1356–1368):

| Operation | newTotal | Rejection |
|---|---|---|
| Add `set(k, v)`, k new | total + entry(k, v) | 4300 quota if > 1048576 (equality accepted); 4300 entries if the count would exceed 4096 |
| Replace `set(k, v)`, k present | total − entry(k, old) + entry(k, v) | 4300 quota |
| Delete `set(k, null)` | total − entry(k, old), or total if k is absent | Never by quota |
| Clear | 0 | Never by quota |

- `entry(k, v) = utf8len(k) + utf8len(v)`, measured after JSON decoding, without quotes or prefix.
- Every write, deletes and clears included, is subject to disk acceptance: 4300 `{reason:'disk'}` or `{reason:'sites', limit:64}`, with no effect.
- Atomicity: exactly one `chrome.storage.local.set` per accepted message. On 4300 there is no write. A failed set gives −32603 `{reason:'transport'}` and the snapshot is reloaded unchanged.
- Messages are serialized per (netKey, address) by StoreQueue.
- Storage key: `'site:'+netKey+':'+address`.

Choices made here, all P:
- **U38:** an accepted no-op delete still performs one write.
- **U39:** when both quota and entries would be exceeded, quota is reported first.
- Hex in the storage key is lowercase.

**Navigator** (FD:L1193–1198, FD:L4925–4929):
1. Deduct from the nav bucket: 6000 capacity, refill 1 per ms, cost 2000. P: the same integer formula as the message bucket. On a deficit, reply −32005 `{nav}` and do not tear down.
2. Normalize the path (0.2 §6.3).
3. Orphan the frame's requests, cancel the LoadJob, call `StoreQueue.cancelFrame`, and wait (`awaitActive`).
4. Tear down the frame and build a new frame in the same session, or show the viewer's 404 page.

The torn-down frame gets no reply. A path that fails normalization still consumes a nav token. A message that fails at B3 never reaches the Navigator, so it consumes a message token but no nav token.

**openExternal** (BR18e/f/h): one pending confirmation per frame. `open` gives `null` and one tab without an opener; `cancel` or `close` gives 4001; a second request while one is pending gives −32005 `{pending}`.

## R5-06 — B9: the BR17e table

`vectors/br17e-table.json` holds all 24 rows: message number, group e1–e8, key, value, total before, newTotal, reply and total after, transcribed from FD:L2711–2720, plus e9.

The runner:
- generates each message's literal text (compact JSON, ids 1–24);
- checks each text through `BridgeAuth.check` and SiteStorageRef;
- checks every byte length: envelope ≤ 128 bytes and message ≤ 61571 bytes, as FD:L2711 requires;
- checks that every 4300 leaves the dictionary, the total and the write count byte-identical;
- checks e9 (the final dictionary, total 925694);
- runs three mutants that must be detected: strict `<`, quota without subtracting the old entry, and non-atomic writes;
- writes all 24 texts with their sha256 to `results/br17e-expanded.json`.

The (e6) mutant "value difference without the key" is not modelled, because FD:L2722 does not state its formula.

## R5-07 — B6: the write path

`annex/write-path.json` holds the nodes, edges, method sources, exits and a Mermaid diagram. Path:

Frame → BridgeAuth → Wallet → signEligible (1st check) → Approval → Identity → signEligible (2nd check) → Sign(displayed bytes) → WalletSubmit (keccak(raw) == approved hash) → HttpTransport → `eth_sendRawTransaction`.

Properties:
- **W1:** every path to `eth_sendRawTransaction` passes the seven guarded nodes in order.
- **W2:** only WalletSubmit is a source of `eth_sendRawTransaction`.
- **W3:** BridgeAuth has no edge to WalletSubmit or HttpTransport.
- **W4:** only HttpTransport reaches the node.

The runner checks the base graph and five mutants. Exit codes 4001, 4100 and 4901 are the baseline's; their assignment to causes is P.

## R5-08 — B8: BR messages, txA and RpcTap

`annex/br-messages.json` holds the literal messages for:
- BR1–BR14;
- BR17a–g (BR17d by reference to the 0.4 cases, BR17e by reference to R5-06);
- BR18a–h and BR18-L.

It also gives the expected reply for each message (id, code and data, or result) and a criterion for each routed case.

The runner:
- executes every case whose outcome is decided at B0–B3;
- requires routed cases to pass B0–B3;
- writes every text with its UTF-8 length and sha256 to `results/br-messages-expanded.json`.

Items still open:
- BR10's LogClient error mapping, and the −32020 `{range}` data shape, are pending row V2.
- BR14's granted account comes from the wallet's derivation (FD:L1325). P: the test tool funds it.

**TK-1 (PROPOSAL, P).** The baseline names "fixture keys 1–10" (FD:L2467) without a derivation. Proposed: fixture key i is anvil's default account i−1, from the mnemonic `test test … junk`, path m/44'/60'/0'/0/(i−1). anvil funds these accounts by default. s1–s4 are keys 7–10.

**txA.** EIP-1559 type 2, from key 7 to key 8, value 1, nonce 0, chainId 777910. Gas 21000, tip 4 and maxFee 2·10⁹ are P, copied from tx1. Signed with RFC 6979 and low-s.

Key and txA checks:
- **Python:** derivation known-answer tests for anvil accounts 0 and 1, plus the private-key-1 address; recovery of the signer; low-s. The raw transaction and its hash are recorded.
- **Node, `tools/txa_check.cjs`:** an independent cross-check. It uses Node's PBKDF2/HMAC for BIP-39/32, OpenSSL secp256k1 for public keys, @noble/hashes for keccak, and a BigInt ECDSA verify and yParity recovery. The second runner pass requires it to pass.
- No transaction is sent.

**RpcTap** commands are `serve`, `hold` and `log` (B). The zero-request criterion is P:
- no log entry from readClient, logClient, walletSubmit or other with `tIn` in [send, reply + 1000 ms];
- for the whole run, no `eth_sendRawTransaction` from a source other than walletSubmit.

## R5-09 — U34 resolved

BG1's 61824 = 61440 + 256 + 128 is a **conservative lower bound with a 128-byte envelope reserve**, the same reserve FD:L2711 uses. 61823 fails, 61824 passes and 65536 passes. The tight compact message is 61790 bytes, inside the reserve; this is not a contradiction.

**Guard precedence:** the Bpre size check runs before parsing. A value that `str(61440)` would accept can still be rejected with `size` when JSON escaping pushes the message past 65536 bytes. Fixtures G1–G4 in `vectors/bridge-guard-cases.json` cover this, including escaped `"` and U+0001. The baseline limit is unchanged.

## R5-10 — Import graph versus global fetch

`annex/import-and-fetch-rules.json`:
- **Rule 1 (import graph):** dependency-cruiser, with the config file text given in full.
- **Rule 2 ("no fetch except HttpTransport"):** `fetch` is a global, which dependency-cruiser cannot see. Realized as (a) the closed set of HttpTransport importers in dependency-cruiser, plus (b) an ESLint configuration given in full. The ESLint part restricts the globals `fetch`, `XMLHttpRequest`, `WebSocket`, `EventSource` and `Request`, the property `globalThis.fetch` and its variants, computed member access on `globalThis`, `self` and `window`, `Reflect.get` on those objects, and `importScripts`.

Eight source fixtures give the expected verdicts. They are not executable in Phase S, because no install is permitted. Aliasing through a local variable is a recorded residual, with a proposed runtime stub for Phase D. This is a technical realization of the baseline rule with an unchanged consequence (FD:L2459), not an owner question.

## Evidence record

- **Executed by Codex, not by the author:**
  - 0.4: 216 passed, 1 recorded, 1 failed (C14);
  - hash triad: Rust sha3 0.9.1, @noble/hashes 1.7.1 and pycryptodome 3.24.0 agree on 611 samples, including all 486 declared content hashes of the positive files (`coordination/hash-triad/`).
- **0.5:** nothing executed by the author. The run sequence is in `README.md`.
