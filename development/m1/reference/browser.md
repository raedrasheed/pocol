# Browser and isolation

**Address**
- omnibox: 'pocol <profileName>/<0xaddr>[/path][@v<n>]'.
- web3:// is only a convenient input form. ERC-4804 and ERC-6860 are not supported in v1, and their texts have not been read.
- We do not claim native Chrome support for any scheme.

**Network profile (D58, D77, D79)**
{profileId, name, chainId, genesisPre, genesisHash, forkSchedule, endpoint, trustLevel ∈ {DEV, LN, RP}}.

Admission:
1. Decode genesisPre without any gs* error.
2. keccak256(genesisPre) = genesisHash.
3. chainId matches.
4. Signed networks.json, or a confirmation displaying the complete hash.
5. RecvFit (D100): calculate WorstLegit for each CP-dependent exact method from the profile's own values: B_code_max, BODY_MAX, TX_MAX and genesisPre length. Any RECV_LIMIT overrun rejects the profile with −32019 {rule: recvFit}, rather than admitting a network whose honest responses would later be rejected.

netKey = keccak256(RLP(['PoCol-net-v1', chainId, genesisHash])).
- Check on opening, every 30s and on signing. Any mismatch tears down the frame and cancels requests with 4901.
- Duplicate chainId with different genesis: read-only with a “conflicting” label, and 4100 for signing.

**RP header window (D77, D80–D84)**

Counts (D82):
- W = [b, h], where b = max(1, h−12), and n = h−b+1 ≤ 13.
- Reference b−1 when b > 1; it is not counted.
- Retrieved: from = max(1, b−1), count = h−from+1 ≤ 14; this is light (count ≤ 64).

Client constants (D84):
- VIEW_SLACK_S = 4.
- ceilTarget = min(2^256−1, target_g·2^4).
- minWorkRP = floor(2^256/(ceilTarget+1)).
- C is the browser clock, and Φ_view = 10s.

Steps:
0. h = eth_blockNumber.
   - At h=0: no pocol_getHeaders or frame; viewNoBlocks with message “The network has not produced block 1 yet; no header is available for RP verification”.
   - eth_requestAccounts and eth_sendTransaction return 4901; recheck every 30s.
1. Calculate b, from and count without violating D81.
   - −32018 for a valid request gives viewIncomplete.
   - −32021 retries after retryAfterMs up to 3 times, then gives viewIncomplete.
2. The response contains count headers at consecutive heights from..h; too few or too many gives viewIncomplete.
3. Reference (b > 1): item 1, then 1b, then viewFuture (ref.ts ≤ C+Φ_view). No work or window check.
4. For each x ∈ W in ascending order:
   - Item 1, then 1b, then viewFuture.
   - Item 2: the parent is the reference, or the virtual genesis header when b=1; parentHash(1) must equal cfg.genesisHash, otherwise viewGenesis.
   - Items 3 and 4.
   - Item 5 (fixed-width ASERT).
   - viewTargetCeil: target_x ≤ ceilTarget, before PoW.
   - Items 6–9.
5. Message: “RP: work and identity bound for n headers [back to genesis], with computed work of at least <minWorkRP in decimal> per header (work(ceilTarget)), without execution or membership verification”.
   - At minWorkRP = 1 append “The work bound is ineffective for this network”.
   - “back to genesis” only when b=1.
   - No percentage or time estimate in the message.
6. Any violation: label, reject display, return 4901 and rely on no root.
7. Items 10–17 are not evaluable; consistent lying is possible.

Client cost: ≤ 14 header decodes, ≤ 13 ASERT operations with operands ≤ 512 bits, and ≤ 13 SHA-256 operations.

Effect of D84: an honest network more than 4τ behind shows viewTargetCeil: 4 hours in network and 40 minutes in P11. The remedy is LN.

RW examples:
- h=3 → b=1, from=1, count=3, n=3, back to genesis.
- h=13 → b=1, from=1, count=13, n=13.
- h=14 → b=2, from=1, count=14, n=13.
- h=20 → b=8, from=7, count=14, n=13.

HC examples:
- target_g=2^240 → ceilTarget=2^244 → minWorkRP=4095.
- target_g=2^256−1 → minWorkRP=1.

LN at h=0: display and signing are allowed because the node verified GenesisSpec and allocRoot.

**LogClient (D89, D91)**

The page's only eth_getLogs path:
- BridgeAuth checks the filter schema (D95).
- The bridge resolves tags to numbers.
- Reject a range > 1024 before sending with −32020 {reason: range}.
- Pass to LogClient.fetchAll; never send raw eth_getLogs.

Limit reason classes:
- P1 = {resultBytes, resultCount, scanBytes}, provided a ≤ lc ≤ b−1.
- RES = {deadline, scratch}, provided a−1 ≤ lc ≤ b−1.
- Any violation or other reason: serverViolation.

withinLimit marker:
- Mark [a, lc] produced by P1; every subrange inherits the marker.
- P1 on a marked range gives serverViolation (lemma P3); RES on it is allowed.

LogClient.step(range{a, b, withinLimit, retries}, reply):
- Complete(logs): every blockNumber ∈ [a, b], blockHash is constant per height, and (blockNumber, logIndex) is strictly ascending after the last append. Then Append; a violation gives serverViolation.
- Limit{r ∈ P1, lc}: Replace([a, lc]+withinLimit then [lc+1, b] with the original marker). Do not skip ahead before completing [a, lc].
- Limit{r ∈ RES, lc}:
  - If lc ≥ a: Replace([a, lc] then [lc+1, b]) with the original marker.
  - Otherwise, if b > a: Replace([a, mid] then [mid+1, b]), where mid = a+floor((b−a)/2).
  - Otherwise (a=b): Retry(500 ms) with retries+1; at retries = 3 return Error{r, block: a}.
- −32021: Retry(retryAfterMs), not counted in retries but counted in totalRequests.
- −32022 (anchorNotCanonical or beyondAnchor): Restart.
- −32601: Error{unsupported}.
- Any other error: explicit Error.

Request accounting:
- logRequests: pocol_getLogs requests in the attempt, including retries.
- headRequests: eth_getBlockByNumber requests in the attempt (anchor and pre-commit check).
- totalRequests: every RPC request across all attempts in the run so far.
- maxRequests bounds only totalRequests and is checked before sending. A request that would exceed maxRequests is not sent; emit abort(id, tooManyRequests), then Error{tooManyRequests}.

Termination:
- Replace replaces one range with two strictly shorter ranges.
- Retry ≤ 3 per block; Restart ≤ maxRestarts.
- maxRequests is an explicit ceiling.
- Attempt steps ≤ 5·(b−a+1) plus −32021 retries; all are bounded by maxRequests.

Correctness lemma: the ranges of one completed attempt form an ordered disjoint partition of [from, to], and every piece is complete on the anchor branch (P2). Concatenation equals the range result for that branch.

sink contract = {begin(attemptId, anchor), piece(attemptId, seq, range, logs), abort(attemptId, reason), commit(attemptId, summary)}:
- summary = {pieces, logs, logRequests, headRequests, totalRequests, anchor}; totalRequests includes aborted attempts.
- attemptId starts at 1 and increases by one.
- anchor = {hash, number} from eth_getBlockByNumber('latest') at begin. If to > anchor.number: abort(id, beyondHead), then Error.
- seq starts at 0 per attempt; pieces follow range order and may be empty.
- Before commit: eth_getBlockByNumber(anchor.number).hash = anchor.hash; a mismatch is treated as Restart.
- Restart: abort(id, reorg), then begin(id+1, new anchor). maxRestarts = 3, hence ≤ 4 attempts, then abort(id, reorgUnstable) and Error.

Invariants (checked by SinkChecker):
- (S-a) Per attempt: one begin, then ≥ 0 piece events, then exactly one terminal event (abort or commit).
- (S-b) No call after the terminal event; no attempt before the previous one ends.
- (S-c) At most one commit, only for the final attempt.
- (S-d) The committed attempt's ranges cover [from, to] exactly.
- (S-e) pieces and logs match what was delivered; logRequests and headRequests match the attempt's MockLogServer trace; totalRequests matches all run requests.

Consumer semantics: piece data is provisional until commit. On abort, discard all data for attemptId. The result is correct for the declared anchor branch; a reorg after the pre-commit check is not detected.

The two collectors:
- fetchAll (bridge): internal sink cleared on abort.
  - maxBytes = 4 MiB per attempt, timeout 30s, maxRequests = 4096.
  - Exceeding limits gives tooLarge, timeout or tooManyRequests without a partial result.
  - The page receives data only after commit.
- fetchEach (test tool, Node.js): external sink, timeout 600s, maxRequests = 65536; unavailable to pages.

In RP: −32601 gives unsupported; consistent lying is possible (D13).

**Bridge and its authorization (D95, D96, D97)**

Unified envelope (D96):
- A message is JSON text = {id, kind, payload}, with exactly these three fields.
- id is an integer in [0, 2^32−1] generated by the loader; it is only the response key.
- kind ∈ {rpc_read, wallet_req, storage_set, nav}.
- payload = {method, params}, with exactly these two fields; params is always an array for every kind, even when empty. No alternative form for any kind.
- Response: {id, result} or {id, error: {code, message, data?}}. id = null when no valid id can be extracted.
- No partial result with success status.

General rule:
- The site is an adversary (A7b). Every message passes through BridgeAuth.check in the extension worker before any network request or local effect; deny by default anything absent from the matrix.
- The allowlist is a frozen Map built at module load, not a plain JS object; __proto__, constructor and hasOwnProperty do not match.
- Method names match exact bytes: case-sensitive, no whitespace or control characters, ≤ 64 ASCII bytes.
- RpcGuard's light/heavy table is not a permission table.

Arrival stamp (D97):
- The worker stamps every message on port receipt with arrivalMs = floor(performance.now()) in the worker, before any check. The site cannot control it.
- Process messages one by one in arrival order.

Check order (stop at the first violation; identical for every kind):
- Bpre (without parsing):
  - Size > BRIDGE_MSG_MAX = 64 KiB: −32600 {reason: size}, id = null, without consuming a token.
  - Rate bucket per site session (D97, D99), with integer state {tokens_mt, last_ms}:
    - On session creation (not frame creation): tokens_mt = BUCKET_CAP_MT = 50000, last_ms = creation time. Navigation does not refill the bucket.
    - For each message passing the size check: tokens_mt = min(50000, tokens_mt + 50·(arrivalMs − last_ms)), then last_ms = arrivalMs.
    - If tokens_mt ≥ 1000: subtract 1000 and continue. Otherwise: −32005 {reason: rate}, id = null, without deduction.
    - A message passing the rate check consumes its token even if later rejected in B0–B3.
- B0 structure: parseStrict rejects invalid JSON, duplicate keys at any depth and depth > 8. Then: outer fields exactly {id, kind, payload}, valid in-range integer id, string kind, payload object with exactly {method, params}, string method and array params. Any violation: −32600; return id if valid, otherwise null.
- B1: kind ∈ the set, otherwise −32600 {reason: kind}.
- B2: (kind, method) appears in the matrix, otherwise 4200 {method}, with method truncated to its first 64 UTF-16 units.
- B3: params exactly matches the method schema in element count, types, permitted fields and limits, otherwise −32602 {path}. path is 'params', 'params[i]' or 'params[i].field'.
- B4 routing, then the state constraint enforced by the destination component (D97), not BridgeAuth:
  - rpc_read routes to RpcReadClient, except eth_getLogs to LogClient.fetchAll. Both pass through the session's pending counter:
    - If pendingReads = 4 or the global count reaches 16: −32005 {reason: pending}, without a request.
    - Then RecvGuard.reserve (D98). If reservation is unavailable within 2s: −32005 {reason: busy}.
  - wallet_req routes to Wallet with its own constraints (one pending request and ≤ 5 per minute).
  - storage_set routes to StoreQueue (D101). A full queue gives −32005 {reason: store} without appending. Once admitted, SiteStorage executes messages in order; exceeding quota gives 4300 {reason: quota, limit: 1048576} under the accounting below.
  - nav routes to Navigator; a second pending confirmation gives −32005 {reason: pending}.

pendingReads counter (D97):
- Per site session (D99), initially 0 when the session is created.
- Increment by one when RpcReadClient or LogClient admits the message, before the first request.
- Decrement by one when the final response is sent to the frame: success, remote error, or transport error/timeout (10s for RpcReadClient, 30s for fetchAll), returned as −32603 {reason: transport}.
- Tearing down the frame does not reset it. Requests from a torn-down frame become orphans:
  - Cancel through AbortController without a response.
  - Decrement for each request only when it settles: the transport promise rejects and the receive buffer and reservation are released.
  - Maximum settlement time is the transport timeout itself.
- Reset only when the session ends, after every request has settled.
- Internal LogClient retries do not increment it: one eth_getLogs message = one pending request.

Primitive types:
- addr = '0x'+40 hex.
- hash32 and slot32 = '0x'+64 hex.
- qty = hex without leading zeros within u64.
- tag ∈ {'latest', 'safe', 'finalized'} or qty. Reject pending, earliest and EIP-1898 objects.
- data = '0x'+even-length hex ≤ 64 KiB.
- str(n) = a string without lone surrogates, with UTF-8 encoded length between 1 and n bytes.
- pathStr = printable ASCII (0x21–0x7e), length 1..2048, starting with '/'.
- httpsUrl = printable ASCII of length ≤ 2048, beginning literally with 'https://', parsed by URL with protocol = 'https:', nonempty hostname, and empty username and password.

rpc_read matrix (method: schema):
- eth_chainId, net_version, web3_clientVersion, eth_blockNumber, eth_gasPrice, eth_maxPriorityFeePerGas: [].
- eth_getBalance, eth_getCode, eth_getTransactionCount: [addr, tag].
- eth_getStorageAt: [addr, slot32, tag].
- eth_getBlockByNumber: [tag, false]. eth_getBlockByHash: [hash32, false]. Full blocks (true) are rejected.
- eth_getTransactionByHash, eth_getTransactionReceipt: [hash32].
- eth_call: [callObj, tag]. eth_estimateGas: [callObj] or [callObj, tag]. A third parameter (state override) is rejected.
  - callObj fields may only be from {from, to, gas, gasPrice, maxFeePerGas, maxPriorityFeePerGas, value, data, input}.
  - to is mandatory: no deployment simulation.
  - gas ≤ 30000000; data or input, not both.
  - Both simulate on a snapshot without committing state or broadcasting.
- eth_feeHistory: [qty ≤ 128, tag, percentiles], where percentiles contains ≤ 16 ascending numbers in [0, 100]. This product limit is narrower than the node's (≤ 1024) to keep responses within the derived RECV_LIMIT (D100).
- eth_getLogs: [filter], in either form:
  - {fromBlock, toBlock, address?, topics?}, where address is addr or ≤ 16 addresses, and topics has ≤ 4 positions, each null, hash32 or ≤ 16 hashes.
  - {blockHash, address?, topics?}.

wallet_req matrix:
- eth_accounts, eth_requestAccounts, eth_chainId: [].
- eth_sendTransaction: [txObj], where txObj fields are from {from, to, gas, value, data, maxFeePerGas, maxPriorityFeePerGas, nonce}. from is the account granted to (netKey, siteAddress), otherwise 4100.

storage_set matrix (D96, local, no network request):
- site_storageSet: [str(256), str(61440) or null]. null means deletion.
- site_storageClear: [].
- Successful result is null.

nav matrix (D96, local, no network request):
- site_navigate: [pathStr]. The viewer applies storage normalization, tears down the frame and builds a new one at the path under D26. A missing or normalization-rejected path shows the viewer's 404 page. No response to the torn-down frame. The new frame joins the same site session with its bucket and two counters (D99).

Session navigation bucket, charged before teardown:
- NAV_CAP_MT = 6000, NAV_REFILL_MT_PER_MS = 1, NAV_COST_MT = 2000. This allows three consecutive navigations, then one every 2s.
- Insufficient balance gives −32005 {reason: 'nav'} without teardown.
- site_openExternal: [httpsUrl]. Show confirmation with the full URL. Clicking “Open” opens a new tab without opener and returns null; rejecting or closing gives 4001. A second pending confirmation from the same frame: −32005 {reason: pending}.

The site_ prefix is reserved for local methods and absent from rpc_read; no site_ method reaches RpcReadClient or a node.

Prohibitions:
- Deny by default; the following examples are tested explicitly.
- Prohibited in every kind: eth_sendRawTransaction, eth_sign, personal_*, eth_signTypedData*, wallet_*, eth_subscribe, eth_newFilter and related methods, debug_*, txpool_*, admin_*, anvil_*, evm_*, eth_getProof, and all pocol_* (including pocol_submitShare and pocol_getTemplate).
- eth_sendTransaction outside wallet_req is prohibited; read methods inside wallet_req, site_* methods outside their class, and RPC methods inside storage_set or nav are prohibited.

The only network-write path:
1. wallet_req/eth_sendTransaction.
2. Approval bound to the network and bytes (D14, D26).
3. Wallet signs the displayed bytes.
4. WalletSubmit sends eth_sendRawTransaction with only those bytes, after verifying keccak256(raw) = approved txHash.

Only WalletSubmit owns the raw-send transport.

Code isolation:
- RpcReadClient is the only network transport available to Bridge. It rechecks the name against the same read matrix as a second defense and rejects anything else with 4200 without a request.
- Internal methods pass through HeaderNetCheck, LogClient and NetworkProfiles, not Bridge, and consume neither the frame bucket nor its pending count. These include pocol_getHeaders, pocol_getParams, pocol_getLogs, eth_getBlockByNumber for the anchor, and eth_getCode for chunk fetching.
- CI dependency-cruiser rule: Bridge must not import WalletSubmit or raw HttpTransport.

BridgeTrace (D97, test build and explicit activation only):
- One row per message: {frame, seq, id|null, arrivalMs, tokensBefore_mt, bpre ∈ {pass, size, rate}, stage ∈ {B0, B1, B2, B3, B4, ok}, code|null, route|null, pendingBefore|null, storeTotalBefore|null, storeTotalAfter|null, forwardedSeq|null, replyMs|null}.
- storeTotalBefore and storeTotalAfter are only for storage_set messages, using the accounting below.
- Does not change decisions; fixed 1 MiB buffer; overflow makes the run inconclusive.

Errors:
- Return fixed forms to the frame: 4200 {method}, −32600 {reason?}, −32602 {path}, −32005 {reason}, 4300 {reason, limit}, −32603 {reason ∈ {transport, recvLimit, recvDepth, recvParse, storeTimeout}}, 4001, 4100, 4901. −32005 reasons are {rate, pending, nav, busy, store}.
- 4300 and −32005 are local bridge codes. −32005 comes from EIP-1474; 4300 is explicitly nonstandard.
- Remote error text is forwarded only for an allowed method that reached the server.

In RP: the same matrix, with local checks before sending remotely.

**Remote response reception: RecvGuard (D98)**

Every extension network request goes through HttpTransport.request(method, body, {recvLimit, deadline, pool}), including RpcReadClient, LogClient, HeaderNetCheck, NetworkProfiles, ChunkFetcher and WalletSubmit. Never call response.text() or response.json().

Read steps:
1. Reservation: reserve recvLimit from the pool before sending, using a FIFO queue. Failure within 2s gives busy.
2. If Content-Length > recvLimit, cancel before reading any byte.
3. Read response.body with a stream reader:
   - Count bytes after Chrome decompression, so the same count stops a decompression bomb.
   - Check each chunk before appending. Discard a chunk that would exceed recvLimit, and cancel through AbortController: recvLimit.
4. At end:
   - TextDecoder in fatal mode, otherwise recvParse.
   - Then parseStrict with depth ≤ 16 and duplicate-key rejection, otherwise recvDepth or recvParse.
   - The parser is iterative, not recursive, with linear cost.
5. An error response (non-200 HTTP status or error object) has the same limits:
   - Truncate message to 256 UTF-16 units.
   - Forward data only if its encoding is ≤ 4 KiB; otherwise remove it.
6. Any cancellation, timeout or error discards the entire buffer:
   - No partial content reaches a frame, sink or viewer.
   - Release the reservation on settlement.

RECV_LIMIT per method (D100). Each method has one of two classes:
- exact: RECV_LIMIT ≥ WorstLegit(method), the worst legitimate HTTP body from pocold, computed using CP and the parameters the client actually sends (bridge schema or internal call). This promises full LN compatibility, checked by BG1 for defined profiles and by RecvFit for each profile.
- capped: deliberately narrower product limit than the node, in this literal closed list:
  - eth_call capped at 1 MiB; the node permits RESP_MAX = 8 MiB.
  - eth_estimateGas capped at 64 KiB. Success ≤ 4 KiB; a revert error with larger data gives recvLimit.
  - A legitimate response above the limit gives −32603 {recvLimit} and is recorded in T12.
  - Adding a capped method requires a row here and review.

WorstLegit in bytes. Assumptions: compact JSON without spaces, 69 B per textual u256 quantity including its comma, 32 B per decimal percentage.
- 4096 [exact]: eth_chainId, net_version, web3_clientVersion, eth_blockNumber, eth_gasPrice, eth_maxPriorityFeePerGas, eth_getBalance, eth_getStorageAt, eth_getTransactionCount, eth_sendRawTransaction. Result ≤ 70 B; envelope ≤ 1024.
- 69632 (68 KiB) [exact]: eth_getCode. Worst case 2·B_code_max+1024 = 66560.
- 98304 (96 KiB) [exact]:
  - pocol_getHeaders with count ≤ 14: worst case 14·(2·3067+16)+1024 = 87124.
  - pocol_getParams: worst case 2·|genesisPre|+4096, requiring |genesisPre| ≤ 47104, checked by RecvFit.
- 167936 (164 KiB) [exact]: eth_getBlockBy* with hashes. Worst case 2048+floor(BODY_MAX/85)·70 = 163958 in network. The previous 160 KiB (163840) limit was 118 B too small.
- 180224 (176 KiB) [exact]: eth_feeHistory under bridge schema n ≤ 128, p ≤ 16. Worst case 325+2·(n+1)·69+2·n·32+n·(69p+3) = 168015, conservatively including both blob fields.
- 266240 (260 KiB) [exact]: eth_getTransactionByHash. Worst case 2·TX_MAX+2048 = 264192.
- 348160 (340 KiB) [exact]: eth_getTransactionReceipt. Worst case 2·32768+546·512+2048 = 347136.
- 1048576 [capped]: eth_call.
- 65536 [capped]: eth_estimateGas.
- 8 MiB [exact in LN]: pocol_getLogs; worst case RESP_MAX. Above it is fetchAll's aggregate 4 MiB limit. In RP, a remote server not implementing D88 may exceed it, producing explicit Error{recvLimit}.
- Unlisted methods are not sent.

RP applies the same bounds; the exact promise applies only to pocold in LN. A noncompact server (using spaces) may produce recvLimit; this is disclosed.

Pools (worker-global bounds):
- SITE_POOL = 32 MiB for all site requests.
- SESSION_RECV_MAX = 10 MiB per session.
- TRUST_POOL = 2 MiB, separate for HeaderNetCheck and NetworkProfiles, so a site cannot starve it.
- CONTENT_POOL = 8·68 KiB for chunk fetching.
- RECV_INFLIGHT_MAX = 48 requests across site and content pools. The trust pool is outside this count and bounded by bytes, so request count cannot starve it. Bridge traffic does not exceed 16+8; this bound is a second defense tested directly by MR10c.

Effect:
- Buffered reception ≤ sum of pools (approximately 34.6 MiB), plus one stream chunk per request.
- Temporary parsing memory is measured in MR10.
- A slow request holds its reservation until timeout (10s or 30s).

**Site session: SiteSession (D99)**

Creation and termination:
- Key (tabId, netKey, siteAddress).
- Created only by user action through the omnibox or extension UI.
- Ends on tab close or user navigation to another address or network.

Session-owned resources:
- Message bucket, pendingReads and navigation bucket.
- SESSION_RECV_MAX reservation.
- ChunkCache and its StoreQueue quota (message and byte counters, D101).

Frames and loading:
- Every frame created by site_navigate or reload joins the same session; no quota is renewed.
- One LoadJob per session. Navigation cancels the previous LoadJob; orphaned chunk requests stay in CONTENT_POOL until settlement.
- Revoke the previous load's blob URLs on teardown.
- Ignore torn-down-frame port messages after closing; they do not consume tokens.

Chunk memory:
- ChunkCache keyed by chunk address and length. The address derives from keccak256(data), making it a content key.
- Limit 4 MiB+64 KiB per session and 16 MiB globally, with LRU across inactive sessions.
- Navigation within a cached version does not refetch chunks.

Cumulative per-session limits regardless of repeated navigation:
- ≤ 4 live or orphaned reads.
- Messages by second t ≤ 50+50·t.
- Navigations by second t ≤ 3+t/2.
- Fetch each chunk once while it remains cached.

Message handling does not wait for the network; large parses are bounded by RECV_LIMIT, keeping the worker responsive.

**Signing eligibility**
- Every profile edit is under ProfileMutex and increments eligibilityEpoch.
- signEligible: the profile matches its frozen copy, no conflict, netKey not revoked, connection active, wallet unlocked, epoch matches, and (LN or DEV or RP with h ≥ 1 and a successful window).
- Inside the lock: eligibility, identity, second check, sign the displayed bytes, send through WalletSubmit, then release.
- Per-netKey key = HKDF-SHA256(seed, salt=netKey, info='PoCol-wallet-v1').

**manifest.json (MV3)**
- sandbox.pages = [sandbox.html].
- CSP: 'sandbox allow-scripts allow-forms; default-src 'none'; script-src 'sha256-<H_loader>' blob:; style-src blob: 'unsafe-inline'; img-src blob: data:; font-src blob:; media-src blob:; connect-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none'; form-action 'none''.
- Permissions: storage, unlimitedStorage (D103), omnibox, declarativeNetRequest, host 127.0.0.1 and optional_host for remote endpoints.

**Loading (D26)**
1. viewer freezes context and checks identity and the DNR guard.
2. The worker fetches and verifies.
3. iframe with sandbox='allow-scripts allow-forms'.
4. Deliver MessageChannel once; bind the frame to SiteSession (D99) without creating a new bucket or counter.
5. The loader applies RtcLockdown, freezes, sanitizes and executes blob scripts in order.
6. A second load event tears down the frame.

**Sanitization**
- Remove: on*, javascript:, iframe/frame/object/embed/base/portal, meta http-equiv, scripts inside svg, prefetch/preconnect/dns-prefetch/prerender.
- Rewrite: src, href, srcset, url(), @import.
- Intercept: relative fetch and XHR.

Unsupported: eval, relative import, Workers, cookies, IndexedDB and WebRTC.

**Network isolation**
- Layer 1: CSP.
- Layer 2: per-tab session DNR:
  - Block {id 2·tabId, priority 1}.
  - Allow {id 2·tabId+1, priority 2, urlFilter '|chrome-extension://<ID>/'}.
  - Installation fails closed.
- Layer 3: RtcLockdown; its argument is unproven, and X8 is a gate.
- Links (D96): the loader intercepts link clicks. A relative or absolute in-site link sends nav/site_navigate with the path; an external https link sends nav/site_openExternal. Links with other schemes are not followed.

**Site-local storage (D34, D96)**
- Logical storage key: 'site:'+netKey+':'+address; its state is a dictionary {key → value} stored in immutable versioned records (D103).
- Literal quota accounting (SITE_STORE_MAX = 1048576 bytes):
  - entry(k, v) = utf8len(k) + utf8len(v), UTF-8 encoding of the strings as present in params after JSON decoding, excluding quotes, JSON structure and the 'site:' prefix.
  - total = Σ entry over all pairs stored for the site.
  - site_storageSet(k, v) for string v: newTotal = total − (entry(k, old) if k exists) + entry(k, v). Accept if newTotal ≤ 1048576; equality is allowed. Otherwise 4300 {reason: 'quota', limit: 1048576}.
  - site_storageSet(k, null): delete; newTotal = total − entry(k, old), or total if k is absent. Never rejected for quota.
  - site_storageClear: newTotal = 0; never rejected for quota.
  - (D104) Two additional constraints:
    - An insertion making pair count > SITE_ENTRIES_MAX = 4096 is rejected with 4300 {reason: 'entries', limit: 4096}.
    - Every write, including deletion and clear, requires disk admission and may be rejected with 4300 {reason: 'disk'|'sites'} without any effect.
- Atomicity: SiteStorage computes newTotal, then writes the complete state using one chrome.storage.local.set operation for a new record (D103). At 4300 no write is called, so the snapshot remains byte-identical. A write failure returns −32603 {reason: transport} and reloads the snapshot from storage.
- SiteStorage serializes messages per storage key (netKey, address) through StoreQueue, preventing overlapping total calculations.

**StoreQueue (D101)**

Queued message size:
- qbytes(msg) = 64 + utf8len(k) + utf8len(v), counting null v as 0.
- site_storageClear counts as 64.
- Maximum message = 64+256+61440 = 61760.

Admission at B4, before appending and without any write. Admit a message only if all four conditions hold:
- sessQ.n+1 ≤ STORE_SESSION_MSGS_MAX = 8.
- sessQ.bytes+qbytes ≤ STORE_SESSION_BYTES_MAX = 262144.
- globQ.n+1 ≤ STORE_GLOBAL_MSGS_MAX = 64.
- globQ.bytes+qbytes ≤ STORE_GLOBAL_BYTES_MAX = 4194304.

Equality is allowed. Any violation immediately gives −32005 {reason: store}. Counters include every admitted message not yet in ownState = released (see ownership lifecycle below), including active writes answered with storeTimeout but not yet settled.

Arithmetic fact (I55): with defaults, the global byte limit is unreachable because 64 maximum messages = 64·61760 = 3952640 < 4194304. The global count limit always triggers first; the global byte limit is an additional defense against later constant changes. BR21e tests its condition separately using test parameters.

Ordering:
- FIFO per storage key in admission order (arrivalMs then seq), even across multiple tabs for the same site.
- set and clear execute in the same order.
- At most one active write per key.
- Extension-wide ≤ STORE_ACTIVE_GLOBAL = 2 active writes. Other keys wait in a global FIFO.

Execution:
- Head evaluation (D102, I56): when a message becomes its key's first unreleased message and the key has no active write, compute newTotal immediately from confirmed total. Do not wait for a global slot or call Chrome.
  - Overrun: reply 4300 {reason: quota, limit: 1048576}, then release in the same step (queued → released). Zero set calls; no slot or lock held. Immediately evaluate the key's next head.
  - Admission (always including deletion and clear): ownState = ready. Wait for a global slot in global FIFO admission order (arrivalMs then seq).
- Take the global slot only when starting the write (ready → active); a quota-rejected message never holds a slot.
- The evaluation remains valid until start because only that key's writes change its total, and they are behind the message in FIFO.
- STORE_QUEUE_WAIT applies to both queued and ready.
- One chrome.storage.local.set writes the complete dictionary. Single-item atomicity is assumed and measured by BR21c.
- Success updates dictionary and total, then replies null.

Timeouts:
- STORE_QUEUE_WAIT = 10s from admission until write start. Expiry gives −32005 {store}; remove the message without writing.
- STORE_WRITE_DEADLINE = 5s for an active write. Expiry gives −32603 {reason: storeTimeout}, meaning the outcome is unknown.
  - The key stays blocked and the active-write slot held until the Chrome promise settles.
  - On settlement, reload the dictionary from storage and recompute total.
  - Do not start the key's next write before this, preventing command reordering.

Ownership lifecycle (D102, I54):
- Every admitted message has two separate states:
  - Reply state: replyState ∈ {none, sent, suppressed}.
  - Ownership state: ownState ∈ {queued, ready, active, released}.
    - queued: not yet evaluated.
    - ready: passed quota, awaiting a slot.
- Send at most one final response; it does not change ownership.
- Owned resources:
  - Per message: qbytes and one message in session/global counters.
  - Active messages also own the global active-write slot and storage-key lock.
- release(msg) is the sole place that subtracts counters and frees slot/lock. Execute exactly once per admitted message. A second call for the same message is an implementation error detected by BR21.
- Transitions:
  - queued → ready: successful head evaluation.
  - ready → active: take a global slot and start set, without release.
  - queued → released in three cases:
    - Quota-rejected head evaluation: reply 4300, no set or slot.
    - STORE_QUEUE_WAIT expiry at acceptMs+10000: reply −32005 {store}.
    - Cancellation on teardown or session end: suppressed, no response.
  - ready → released: STORE_QUEUE_WAIT expiry or cancellation, with the same responses.
  - active → released: only when the chrome.storage.local.set promise settles, successfully or unsuccessfully; nothing else.
- Settlement before deadline: update dictionary and total (or reload on failure), reply (null or −32603 {transport}), then release in the same step.
- STORE_WRITE_DEADLINE expiry: reply −32603 {storeTimeout}; replyState = sent. Ownership stays active, with counters, slot and lock.
- Settlement after storeTimeout or frame teardown: reload dictionary, recompute total, then release without any response.
- Frame teardown after storeTimeout does not change counters: the message has been answered but remains active.
- Invariant: at every moment session/global counters equal the count and qbytes sum of unreleased messages.
  - New messages can therefore still be admitted while an active write is pending, but only within remaining limits.
  - Messages for the blocked key wait until settlement or STORE_QUEUE_WAIT.

Teardown and navigation (D99):
- Cancel unstarted messages from the torn-down frame without writing or replying; free their counters immediately.
- An active write completes because Chrome writes cannot be cancelled; count it until settlement and discard its response.
- Navigator waits for the key's active write to settle until STORE_WRITE_DEADLINE before taking the new frame's snapshot. After timeout, load the frame with confirmed state and an “Uncertain save state” badge.
- Session termination applies the same rule.

Memory:
- Queues ≤ 4 MiB.
- Active writes ≤ 2·(1048576+61760) in values plus serialized copy, estimated ≤ 5 MiB, measured in BR21c.
- Loaded key dictionaries: one per key with a live session; evict others by LRU above STORE_DICT_CACHE_MAX = 8 MiB.
- The loader builds localStorage from a local snapshot on load; reads are synchronous from the snapshot.
- The loader enforces quota locally with the same accounting: setItem causing total > 1048576 throws QuotaExceededError, keeping its snapshot ≤ 1 MiB.
- Coalesce writes every 250ms into a dirty-key set retaining the last value per key; clear() empties the set and marks clear. Local queue memory is therefore ≤ the local snapshot.
- The loader sends storage_set only if three conditions hold:
  - Sent but unanswered messages < 8.
  - Sent but unanswered bytes + qbytes ≤ 256 KiB.
  - Local estimate indicates a bucket token.
- A rate or store response makes the item dirty again unless a newer value was written.
- On intercepted internal links, the loader sends dirty keys, waits up to 2s for responses, then sends site_navigate. A site directly calling site_navigate accepts losing pending writes (D101).
- A loader write receiving 4300: restore the last confirmed value in the local snapshot and show a “Not saved” badge.
- No sharing between addresses or netKeys; a site cannot access another's key because SiteStorage derives the key from frame context, not the message.

**Worker-generation recovery (D103, I57)**

D101 and D102 apply within one worker generation. The following governs transitions between generations.

Alternatives:
- (A) Persistent owner: an offscreen document or port keeping the worker alive. MV3 does not guarantee their survival, leaving the same question on their disappearance.
- (B) Only reread the dictionary at boot: a late write from a dead generation overwrites a newer write (I57 scenario).
- (C) IndexedDB transactions: transaction outcome on context loss is unverified and introduces a second store.
- (D) Selected: immutable records versioned by (epoch, seq), with a generation fence and per-key checkpoint.

**1. Fence**
- Name (D134): fence item for E is only 'pocol:epoch:'+hex16(E), without nonce; value {bootMs: Date.now()} ≤ EPOCH_BYTES_MAX. Every generation calculating the same E writes the same name.
- At boot, list 'pocol:epoch:*' with getKeys. EN is the count, M the maximum, E = M+1. In order: name gate, generation window (section 6), then write. No set before both gates pass.
- Name gate (D134): EN + 1 ≤ EPOCH_NAMES_MAX = 8. On violation:
  - Issue nothing; increment epochGateBlocks.
  - Wait T_LATE from this generation's boot using its monotonic clock, sweep as below, then repeat enumeration and gate once.
  - If still violated: storage_set returns −32603 {storeRecovery, reason: 'epochNames'} throughout the generation; no snapshot; escalate action 25.
- Write 'pocol:epoch:'+hex16(E) with {bootMs} and wait for success. STORE_EPOCH_RETRY retries use the same name, adding no names.
- No data write or session before confirmation.
- Retry failure up to STORE_EPOCH_RETRY = 3 times. Then storage_set returns −32603 {reason: storeRecovery}; no snapshot.
- Sweep (D134): after T_LATE from the live generation's boot using its monotonic clock, delete every epoch item whose name is smaller than the largest visible at sweep time, regardless of stored bootMs.
  - Never delete the largest; the upper bound is nondecreasing.
  - Sweep in both confirmed and gate-blocked generations.
  - A failed remove leaves the item counted in EN.
- Writers lemma: let M_L be the largest visible item when live generation L boots. Every generation writing a name X ≤ M_L booted before L, because any later generation sees M_L and chooses a larger number. Thus each such writer booted ≤ bootMs(L), and all its operations settle before bootMs(L)+T_LATE (A15c). A swept name cannot return.
- Name-bound lemma:
  - Every name ≤ M has appeared before because each writer of X saw X−1.
  - By the writers lemma, a deleted name cannot return.
  - Thus names present or able to appear at any instant are limited to visible names plus {M+1}.
  - The gate at last issuance preserves EN+1 ≤ 8; names increase only through a boot passing the gate.
  - Therefore epoch items ≤ 8 at every instant, regardless of generations disappearing before confirmation.
- Fence lemma:
  - Every generation writing data first confirmed its fence; any later generation sees it and chooses a strictly larger E.
  - A generation that did not confirm its fence wrote no data.
  - Among generations sharing E (D134), only one confirms: the second booted after the first disappeared; if the first had confirmed, the second would see E and choose E+1.

**2. Record**
- Every set writes a new immutable item: 'site:'+netKey+':'+addr+':r:'+hex16(E)+':'+hex8(seq).
- Value {fmt: 2, epoch, seq, tomb, b64}. b64 is base64 of pairs serialized as [be32(len k)‖k‖be32(len v)‖v] in bytewise key order (D104).
- (D104) One SeqAlloc allocates seq per (key, generation):
  - Starts at 0; alloc() returns the current value and increments before issuing any set, whether a checkpoint, checkpoint retry or ordinary write.
  - No other allocator; seq is not derived from the checkpoint number.
- Range: seq ∈ [0, 2^32−1], epoch ∈ [1, 2^64−1]. Exhausted seq makes the key's writes in this generation return −32603 {storeRecovery} without set. Exhausted epoch disables local storage.
- Never reuse seq even if its promise fails or times out.
- Confirmed state = dict in the greatest present valid (epoch, seq) record.
- After a record succeeds, delete lower records best-effort. Failed deletions remain charged to disk (D104).

**3. Per-key recovery: RecoveryGate (D104)**
Each key has one state per generation ∈ {unrecovered, recovering, ready, failed} and one shared recovery operation. Any snapshot or storage_set from any tab waits for that same operation, never starting another.

On first access (unrecovered → recovering), start the read phase (D110, I63):
- t_s is transition time. Create a new gateId for the key and start total RECOVERY_READ_DEADLINE = 5s, including read-slot wait, getKeys, every get, record decoding and validation.
- Every getKeys or get takes a ReadSlots slot (RECOVERY_READ_SLOTS = 2 extension-wide) through global ReadFIFO. Release only when that read promise itself settles. No read without a slot.
- List key record names by prefix filtering getKeys; get only the greatest. If corrupt, descend to the next, up to RECOVERY_GETS_MAX = 4 get reads.
- Select the greatest valid record. Skip corrupt records and count them in recoveryCorrupt.
- No record for the key means dictionary {}.
- Corrupt records with no valid record within four reads make the key failed; messages return −32603 {storeRecovery, reason: corrupt}. No issuance or seq allocation. Records remain and can be administratively deleted.
- Deadline expiry before reads complete returns the key to unrecovered with readTimeout:
  - Pending messages return −32603 {storeRecovery, reason: readTimeout} and are released.
  - No issuance, seq allocation or disk reservation.
  - Next access starts a new gate with a new gateId.
- Accept a read result only if gateId matches the current gate and it is in the read phase. Otherwise ignore the result, release only its slot and increment lateReadsDropped. Thus no checkpoint is issued after resolution.
- Reads do not change disk; pending reads do not enter LATE_RESERVE or LATE_NAMES. Memory is bounded: ≤ 2 pending reads, each ≤ RECORD_BYTES_MAX or a name list ≤ STORE_RECORDS_MAX+LATE_NAMES under A15c.
- Request a RecoverySlots slot (D109), not STORE_ACTIVE_GLOBAL, with wait ≤ RECOVERY_SLOT_WAIT = 5s from the request.
- On obtaining the slot, in the same synchronous step: DiskLedger.reserve, then s_c = SeqAlloc.alloc(), then issue checkpoint (E, s_c) with the same dictionary. STORE_RECOVERY_DEADLINE = 5s starts at issuance.
- Disk rejection: no issuance; release the slot in the same step and count the attempt failed without a pending operation.
- First-attempt slot unavailable within 5s: no issuance or seq allocation. Return key to unrecovered; pending messages return −32603 {storeRecovery, reason: busy} and are released. Next access starts a new gate.
- On success: ready, confirmed = (E, s_c). Subsequent writes use the same SeqAlloc.
- Then delete lower records.

Before checkpoint success:
- Key messages remain queued, counted and subject to STORE_QUEUE_WAIT.
- No snapshot.

On checkpoint failure or timeout:
- One second attempt with fresh alloc() and the same dictionary. Request it when the first settles unsuccessfully or times out; request a fresh RecoverySlots slot, wait ≤ 5s. The first attempt remains pending, owning its slot and counted on disk until settlement.
- Second-attempt slot unavailable within 5s is treated as its timeout: key failed under D107, no issuance or seq allocation.
- If successful first-attempt settlement makes the key ready while waiting for the second slot, withdraw its request from RecoveryFIFO without issuance.
- Resolution bound (D110): read phase completes or expires by ≤ t_s+5s, so first slot request t0 ≤ t_s+5s. Slot by ≤ t0+5s, timeout by ≤ t0+10s, second slot by ≤ t0+15s, second timeout by ≤ t0+20s.
- The gate therefore resolves within ≤ 25s of t_s into one of four states: ready, failed, unrecovered due to busy, or unrecovered due to readTimeout.
- Messages awaiting recovery remain subject to STORE_QUEUE_WAIT = 10s and may receive store before resolution.

RecoverySlots (D109, I62):
- RECOVERY_SLOTS = 2 extension-wide, separate from STORE_ACTIVE_GLOBAL and AdminSlots. Slot state ∈ {free, held(op)}.
- Acquire through global RecoveryFIFO in request order, in the same synchronous step as DiskLedger.reserve and issuance.
- Release only on checkpoint-promise settlement, success or failure, or disk rejection before issuance. Neither timeout nor resolution releases it. Generation loss drops all state.
- RecoverySlots lemma: every issued checkpoint owns a slot from issuance until settlement and cannot issue without one. Thus unsettled checkpoints per generation ≤ 2 at every instant, including generation loss, regardless of keys or attempts.
- Resolution lemma: read phase ≤ 5s under a total deadline; each checkpoint-slot wait ≤ 5s; each checkpoint timeout ≤ 5s; attempts ≤ 2. Resolution is therefore ≤ 25s from t_s without assuming any read/write promise settles. Post-resolution read results are ignored through gateId, changing neither resolution nor issuing an operation.
- The two events (D107):
  - apply: the record appears in storage and becomes visible to listKeys.
  - settle: the set promise settles in the live generation.
  - Resolution depends only on settle; apply without settle does not change state.
- The first successful settle of any attempt before resolution immediately makes the key ready.
- confirmed = greatest successfully settled version. Both checkpoints carry the same dictionary, so later success of the other attempt does not change it.
- ready never returns to an earlier state.
- Resolve failed once every attempt has settled unsuccessfully or timed out. It remains terminal for that generation; a late successful settle cannot change it and increments lateRecordsSeen.
- Failure or timeout of both attempts makes the key failed: its messages return −32603 {storeRecovery} and are released; no snapshot in this generation.
- A late attempt, if applied, has a lower version than every post-ready write and carries the same recovered dictionary.

Resolution lemma:
- Every previous-generation write has version < (E, 0).
- If applied before recovery read, it becomes the checkpoint basis. Otherwise it remains below every subsequent record and never becomes visible.
- Thus its outcome resolves once without reversing or superseding a subsequent write.
- A write answered null committed before the response; no record is deleted before a greater record commits, so it is not lost.

Allocation lemma (D104):
- Within a generation, every set receives a fresh number before issuance; record names cannot repeat.
- Every late operation has a lower version than any operation allocated after it.
- The single gate prevents two parallel checkpoints.

Survival lemma:
- Deletion, whether sweeping or recovery, removes record r only after observing a present h > r for the same key.
- The greatest record is never removed.
- Inductively, the greatest applied record remains present even if deletion applies late.

**4. Memory**
- Worker loss drops queues, counters, locks, sessions and reservations.
- The new generation starts with zero counters and inherits no message IDs.
- No response reaches a message from a previous generation.

**5. Frames**
- viewer holds a runtime port to the worker; the worker announces genId in hello.
- On onDisconnect or a different genId:
  - viewer immediately tears down every site frame.
  - Show “The extension worker disconnected; recent writes may not have been saved” with a “Reload” button.
- No automatic reload. The new session requires user action (D99), preventing a site from renewing its bucket by crashing the worker.
- Dirty keys in the torn-down frame's snapshot are lost with notice and are not sent from an old snapshot.
- Reload takes a snapshot after key recovery.

**6. Disk**
Binding limits (D104):
- Record size: with SITE_ENTRIES_MAX = 4096, enc ≤ RECORD_BYTES_MAX = 4·ceil((1048576+8·4096)/3)+256 = 1442048.
- Measurement:
  - inUse = getBytesInUse(null), measured after boot sweep and after every set/remove settlement.
  - names = count returned by getKeys.
- Admission rule for every set (data, checkpoint, deletion or clear):
  - inUse + unres + enc(w) + LATE_RESERVE + META_RESERVE ≤ STORE_DISK_HARD = 256 MiB.
  - names + unresN + 1 + LATE_NAMES + EPOCH_NAMES_MAX ≤ STORE_RECORDS_MAX = 1024; reserve 24+8 = 32 (D134). Visible items count both in names and in reserve, conservatively.
  - unres and unresN are sums of live DiskLedger reservations (D105), in bytes and names, not separate estimates.
  - LATE_RESERVE = GEN_WINDOW_MAX·(STORE_ACTIVE_GLOBAL+RECOVERY_SLOTS)·RECORD_BYTES_MAX = 4·4·1442048 = 23072768; LATE_NAMES = GEN_WINDOW_MAX·(STORE_ACTIVE_GLOBAL+RECOVERY_SLOTS+ADMIN_SLOTS) = 24.
    - Six per generation: two global slots, two checkpoint slots (D109), two tombstone slots (D108).
    - Epoch items are separate, with total EPOCH_NAMES_MAX = 8 reserved (D134).
  - META_RESERVE = 1 MiB for epoch items and tombstones. Only tombstones are exempt from the byte condition and charged to META_RESERVE. Constraint (D108): META_RESERVE ≥ STORE_RECORDS_MAX·TOMB_BYTES_MAX + (GEN_WINDOW_MAX+1)·ADMIN_SLOTS·TOMB_BYTES_MAX + EPOCH_NAMES_MAX·EPOCH_BYTES_MAX = 524288+5120+2048 = 531456 ≤ 1048576 (D134). Components:
    - Retained tombstones ≤ STORE_RECORDS_MAX because tombstones are not exempt from the name condition.
    - Unsettled tombstones ≤ ADMIN_SLOTS for the live generation and each dead generation.
    - EPOCH_BYTES_MAX = 256; epoch items ≤ EPOCH_NAMES_MAX = 8 at every instant (D134).
- Violation: 4300 {reason: 'disk', limit} before any set; never delete a record to make room.
- DiskLedger (D105): admission and reservation are one synchronous worker step, with no await between check and addition. JavaScript is single-threaded, so admissions cannot overlap.
  - reserve(op): check conditions against current R_bytes, R_names and R_sites; in the same step add enc(op), one name and a site slot if new; return an operation-owned DiskTicket.
  - Includes deletion, clear, checkpoint and tombstone (its bytes are in META_RESERVE). remove has a zero ticket.
  - Ticket lifetime:
    - On settle, become settled with settleSeq; remain counted.
    - Drop only on completion of a refresh (getBytesInUse and getKeys) requested after settleSeq, adopting its measurement in that same step. There is no gap between reservation removal and measured effect.
  - Serialize refresh: one at a time, scheduled after every settle.
  - Site slot: after successful writing, it appears in measured sitesLive. On failure, release with that same measurement.
  - A ticket whose promise never settles remains until generation loss, then LATE_RESERVE covers it under A15c.
  - Subtract deletions only through measurement; delay overestimates usage rather than understating it.
  - Reservation lemma: every live-generation operation absent from the last measurement owns a live ticket. Chrome-measured disk ≤ inUse + R_bytes + dead-generation effects; each admission preserves the bound. Likewise for names and sites.
- Site slots (D105, D112, I66):
  - sitesLive = distinct (netKey, address) keys with at least one record name in the last adopted getKeys, regardless of content: data, {} or tombstone.
    - Never count a key twice.
    - Remove a key only after an adopted measurement sees no record for it and it has no live-generation pin (D113).
    - pin (D113): every non-remove DiskLedger ticket for an operation on a key counted at admission pins it in sitesLive until ticket removal (D105). The measurement dropping the ticket was requested after settle and sees its effect if applied. Thus no operation-created key remains uncounted.
    - A site-slot ticket drops in the same step as adopting the measurement that sees it (D105), with no gap or double count.
  - Two operations can create a new key: data set and checkpoint for a recordless key (dictionary {}).
    - A tombstone cannot create a key because it issues only after a record exists for the key in the live generation.
    - remove cannot create a key.
  - LATE_SITES_PER_GEN = STORE_ACTIVE_GLOBAL+RECOVERY_SLOTS = 4. Every unsettled creating operation owns a slot until settle (D102, D109). A successfully settled operation is visible to the next generation; a rejected operation does not apply (A15d). Thus dead-generation operations able to create keys later ≤ 4.
  - lateGens(now) = number of visible epoch items X ≤ E, including the live generation, with bootMs(X) ≥ now−T_LATE (D134).
    - Each X represents its preceding generation, whose operations apply or are dropped before bootMs(X)+T_LATE (A15c).
    - Count ≤ GEN_WINDOW_MAX under the generation window.
    - These items are not swept before leaving the window.
    - (D134) An unconfirmed generation writes only a shared-name epoch item, not data. Older generations may write that item's value late, making stored bootMs older.
    - Coverage lemma: for every dead data-writing generation D, every writer of E_D+1 booted after D. Otherwise, seeing E_D before D booted would make D choose E_D+1, not E_D.
    - Thus stored bootMs ≥ boot time of D's successor, keeping the item in the window while D's operations may apply.
    - Visible items from unconfirmed generations also count, conservatively.
  - LATE_SITES(now) = LATE_SITES_PER_GEN·lateGens(now) ≤ 16.
    - X's contribution does not disappear on elapsed time alone, but on completing a refresh requested after bootMs(X)+T_LATE and adopting its measurement in the same step.
    - DiskLedger schedules refresh at every expiry.
  - Admission: every new-key-creating operation (not in sitesLive, no reserved slot) requires sitesLive + R_sites + LATE_SITES(now) + 1 ≤ STORE_SITES_MAX = 64.
    - Reserve the slot in the same admission step.
    - Violation gives 4300 {reason: 'sites', limit: 64, retryAfterMs?} without set.
    - retryAfterMs is time until the earliest expiry sufficient for admission; omit when sitesLive + R_sites alone prevent admission.
    - Operations on counted or reserved keys need no slot and are unaffected by LATE_SITES.
  - Removing a key's last record (sole tombstone) requires:
    - No session, running gate or live ticket for the key.
    - Every epoch item older than the live generation has left the window.
    - Release the slot only through subsequent measurement.
  - Counted key with retained tombstone (D113, I67):
    - A tombstone is a record, so the key is in sitesLive with its slot retained.
    - Every operation on it reserves no slot, does not increase R_sites, and pins it. Includes a checkpoint with dictionary {} and the first set after deletion resolves.
    - A new slot is needed only when all three hold: last record removed, an adopted measurement sees no key, and no pin. Then the first operation is creating and subject to admission.
  - TombReaper (D113):
    - Check removal conditions above at every adopted refresh and every epoch window exit.
    - If satisfied, issue remove for the sole tombstone with a zero ticket.
    - Opening a key session before issuance prevents it.
    - Issued remove is not cancelled. A later operation before the measurement that no longer sees the key is treated as acting on a counted key and pins it. Thus it is neither double-counted nor dropped before that operation's effect appears.
    - Late remove from a dead generation follows the same rule because pin follows live-generation operations.
  - Sites lemma (conditional on A15c and A15d): at every instant each on-disk key is in one of three cases:
    - (1) In the last adopted measurement.
    - (2) Has a live ticket in the live generation.
    - (3) Created by an unsettled operation from dead generation D: ≤ 4 per D, ending before bootMs(succ(D))+T_LATE; succ(D) stays in lateGens until a measurement requested after that time.
    - Admission preserves the sum of the three bounds ≤ 64. Therefore on-disk key count ≤ 64 at every instant.
    - No record is automatically deleted to repair an overrun.
  - Effect: after each boot, new-site admission capacity decreases by 4 per generation in the window, for up to 60s. Existing sites are unaffected; rejection occurs only if sitesLive + R_sites > 63 − LATE_SITES(now).
- Bound lemma (conditional on A15c): Chrome-measured disk ≤ inUse + unres + operations of ≤ GEN_WINDOW_MAX dead generations ≤ STORE_DISK_HARD. Each dead generation has:
  - ≤ 2 data sets, because there are 2 global slots.
  - ≤ RECOVERY_SLOTS = 2 checkpoints, by the RecoverySlots lemma (D109).
  - ≤ ADMIN_SLOTS = 2 tombstones, by the AdminSlots lemma (D108), within META_RESERVE.
  - Epoch items are not counted per generation; total EPOCH_NAMES_MAX = 8 follows from the name-bound lemma (D134), within META_RESERVE.
- Generation window: EpochFence does not issue its item if GEN_WINDOW_MAX = 4 visible epoch items have bootMs ≥ now−T_LATE (D134).
  - Lemma: a data-writing generation sees ≤ 3 items in the window before issuance.
  - After issuance, no new name other than its own appears (name-bound lemma); a late write does not make bootMs newer.
  - Thus lateGens ≤ 4 throughout its lifetime.
  - Wait until the oldest leaves the window; no write or snapshot before then.
  - A waiting generation has written no item and does not count in the window.
- Sweep: at boot and after every successful set, delete only records below the greatest present record per key. A failed remove leaves the record counted in inUse and names, so admission stops at the bound instead of growing.
- Entire-site deletion requires user action from “Storage management”, with confirmation showing site and size:
  - Execute AdminDelete (D106) below, then sweep lower records.
  - One AdminDelete(key) per key; any second request (from another page) shares its promise.
  - 1: key becomes deleting; viewer prevents new sessions and shows “Deleting data”.
  - 2: close every key session in every tab:
    - Tear down frames with notice “Site data deleted”, without automatic reload.
    - Their queued and ready messages become suppressed, then release.
    - Active writes retain ownership until settle (D102).
  - 3: wait for RecoveryGate resolution (≤ 25s, D109, D110). ready, failed and unrecovered (busy or readTimeout) all continue. The latter issued no write; late reads are ignored through gateId. If unrecovered with no running gate, continue immediately.
  - 4: allocate the tombstone version with SeqAlloc.alloc() after those two steps, with a DiskLedger ticket. Take an AdminSlots slot (D108), not STORE_ACTIVE_GLOBAL. No slot gives adminBusy without a tombstone.
    - Do not wait for active writes; its version is greater than every previously allocated version.
    - Allocate no other version for the key while deleting.
  - Successful tombstone settle: confirmed = tombstone, dictionary {}, total = 0. Late settlement of an older write does not change the dictionary because its version < confirmed.
  - Tombstone failure or timeout (5s):
    - Second attempt with a fresh version on an AdminSlots slot (D108).
    - First attempt retains its slot until settle.
  - The following outcome applies in three cases: both attempts fail, both time out, or no second-attempt slot:
    - Key becomes unrecovered; the first later session performs fresh recovery with a checkpoint.
    - Page shows “Deletion failed”, or “Deletion outcome uncertain” for timeout.
    - A tombstone applied before recovery read becomes its basis: successful deletion. One applied afterward is below the checkpoint and never becomes visible.
  - Recreation (D113): a user-created session after resolution sees {}; its first set version is greater than the tombstone.
    - While the tombstone remains counted, the session reserves no site slot and is unaffected by LATE_SITES.
    - If TombReaper removed it and an adopted measurement sees no key, the first checkpoint creates the key and reserves one slot under D105/D112.
  - Deletion lemma: each set in the generation was allocated before the tombstone or after resolution; earlier-generation versions < (E, 0). Therefore no record from an old snapshot supersedes the tombstone.
  - No-late-tombstone lemma (D111): each tombstone issues in a step passing the guard, op.state = running. Resolution changes state to settled before any later step; nothing returns it to running. No tombstone can issue for a resolved operation, so no such tombstone supersedes a set from a post-resolution session. A pre-resolution unsettled tombstone has a lower version than every post-resolution allocation and cannot supersede a new write.
  - Do not delete the tombstone before every older generation leaves the T_LATE window.
- AdminSlots (D108, I61): tombstone-slot ownership.
  - ADMIN_SLOTS = 2 extension-wide slots outside STORE_ACTIVE_GLOBAL. Slot state ∈ {free, held(op)}.
  - Acquire only at tombstone set issuance, through global AdminFIFO in request order, in the same synchronous step as DiskLedger.reserve.
  - Release only when the tombstone promise settles, success or failure. Neither timeout response, AdminDelete resolution nor notice releases it. Generation loss drops all state; a pending tombstone remains a late dead-generation operation.
  - Wait ≤ ADMIN_SLOT_WAIT = 5s per attempt; no tombstone without a slot.
  - Attempt 1 without a slot: adminBusy. No tombstone; key unrecovered; data retained. Page shows “Could not start deletion; data retained”.
  - Attempt 2 starts after first-attempt failure settlement or timeout. If no slot:
    - uncertain if the first is pending.
    - failed if the first has failed.
    - In either case, key becomes unrecovered.
  - Late successful tombstone settlement after resolution does not change resolution; subsequent recovery governs it as above.
  - Operation identity (D111, I65):
    - Each AdminDelete has a generation-unique opId and state ∈ {running, settled}.
    - deleting[key] holds the running operation's opId.
    - Every AdminFIFO request carries its operation's opId and requestId.
  - Resolution: the first successful tombstone-attempt settlement before resolution immediately yields deleted. Resolution with any outcome (deleted, failed, uncertain or adminBusy) performs the following in one synchronous step:
    - op.state = settled.
    - Withdraw every pending AdminFIFO request for the operation without seq allocation or reservation.
    - Remove the key from deleting state.
    - Release new sessions to proceed.
  - Issuance guard: in the synchronous slot-grant step, before DiskLedger.reserve and SeqAlloc.alloc, check:
    - Request not withdrawn.
    - op.state = running.
    - Key is deleting.
    - deleting[key].opId = op.opId.
  - Any failed check: immediately return the slot to the next AdminFIFO request in the same step, with no reserve, alloc or set; increment adminStaleDropped. Grant, guard, reserve, allocation and issuance form one step without await.
  - RecoverySlots (D109) uses the same guard: issue only if request gateId is current and gate unresolved. A mismatch returns the slot and increments recoveryStaleDropped.
  - AdminDelete time bound ≤ 25s (gate resolution including reads, D110) + 2·(5s wait + 5s deadline) = 45s.
  - AdminSlots lemma:
    - Every issued tombstone owns a slot from issuance until settle; none issues without a slot.
    - Therefore issued unsettled tombstones per generation ≤ ADMIN_SLOTS at every instant, including generation loss.
    - Each dead generation leaves ≤ 2 late tombstones, regardless of keys or attempts.
  - Different-key requests compete in AdminFIFO; same-key requests share one promise.
  - TOMB_BYTES_MAX = 512 for tombstone name plus encoded value. RecordCodec rejects larger values; nothing is issued.
- Enumeration: getKeys is mandatory; availability is checked in M0 and BR22c.
  - Absence disables local storage with storeRecovery; never use get(null).
  - Enumeration memory is proportional to names ≤ STORE_RECORDS_MAX+LATE_NAMES under A15c.

**7. Monitoring**
stats returns {epoch, recoveries, recoveryCorrupt, lateRecordsSeen, storeRecoveryFail, inUse, names, sitesLive, diskRejects, removeFailures, genWaitMs, seqMax, resvBytes, resvNames, resvSites, ticketsLive, adminDeletes, adminDeleteFailed, adminDeleteUncertain, adminBusy, adminSlotsHeld, adminTombsUnsettledMax, recoverySlotsHeld, recoveryUnsettledMax, recoveryBusy, readSlotsHeld, readTimeouts, lateReadsDropped, recoveryCorruptFail, adminStaleDropped, recoveryStaleDropped, lateGens, lateSites, sitesRejectsLate, pinnedKeys, tombReaps, epochNames, epochGateBlocks, epochResurrected}. lateRecordsSeen counts records appearing after the checkpoint with lower versions.

**Wallet (D14)**
- eth_requestAccounts requires approval per (netKey, siteAddress).
- eth_sendTransaction after connection, with approval bound to txHash.
- Cancel with 4001 on port disconnection, worker restart, more than one pending request or more than 5 per minute.
- Reject: switch/addEthereumChain, eth_sign, personal_sign, signTypedData, eth_sendRawTransaction.
- AES-GCM with PBKDF2 at 600k iterations; lock after 15 minutes.

Opening a site alone signs nothing, grants no account, broadcasts no transaction and opens no external tab. Store policies are unverified; the experiment uses an unpacked extension.
