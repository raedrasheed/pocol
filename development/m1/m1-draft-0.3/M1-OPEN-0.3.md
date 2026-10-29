# M1 Draft 0.3 — Blockers, Decisions and Owner Questions

`../m1-draft-0.2/M1-UNRESOLVED-0.2.md` stays in force: U01–U22 and E01–E07 are unchanged unless they are amended here. Nothing here is resolved by this document.

## 1. Genuine blockers

A genuine blocker is a conflict or gap in the baseline that further drafting cannot remove. Missing work is not a blocker; it is scheduled in `M1-ANNEX-INVENTORY-0.3.md` §4.

### BLK-01 — Proof-bound reads conflict with the baseline's closed method lists (C06, U05)

**Exact source lines:**
- FD:L1274: "a method not listed is not sent". This refers to the RECV_LIMIT table, FD:L1262–1273.
- FD:L1262–1273: the table lists `eth_getStorageAt` (4096, exact) and `eth_getCode` (69632, exact). It does **not** list `eth_getProof`.
- FD:L4934: `HttpTransport` is imported **only** by RpcReadClient, WalletSubmit, LogClient, HeaderNetCheck, NetworkProfiles and ChunkFetcher.
- FD:L1217: internal methods go through HeaderNetCheck, LogClient and NetworkProfiles, "among them" the anchor `eth_getBlockByNumber` and the chunk `eth_getCode`. The list is non-exhaustive.
- FD:L882, FD:L1204: `eth_getProof` is banned for sites. This is not by itself a ban on the extension.
- FD:L1960, FD:L1966–1967: `eth_getProof` is heavy; HEAVY_ACTIVE = 4; the token bucket is 20/s with capacity 20.

**Consequence:** the preferred snapshot mechanism P20 (0.2 §4.2) cannot be implemented under the baseline as written. Separately, no listed `HttpTransport` importer is assigned to read Website state at all, under any mechanism.

**Preferred solution: change request CR-M1-01.** The baseline text is not edited by this draft.
1. Add one RECV_LIMIT row: `eth_getProof` [exact], for the Website account with at most 6 storage keys, and WorstLegit derived from the trie depth bound. The derivation is part of annex batch 5 (R1). It may use FD:L935 P(d) if the owner confirms that formula is the proof-size bound.
2. Place the Website-state reader in the ChunkFetcher module (P35), as `readWebsiteState(website, keys, N)`, so that no new `HttpTransport` importer is added. Pool: CONTENT_POOL (P; alternatively TRUST_POOL, owner's choice).
3. Budget: at most 4 anchors and 8 `eth_getProof` calls per load (P20 with 3 restarts).

**Falsifiable acceptance tests:**
- (a) A RecvFit/BG1-style check: RECV_LIMIT(`eth_getProof`) ≥ WorstLegit for each defined profile (network, sv-fix, end/full). It fails if any profile exceeds the limit.
- (b) SNAP-01…SNAP-13 (`vectors/snapshot-mock-scenarios.json`) pass when run against the implementation with RpcTap-scripted responses. SNAP-03D must still demonstrate the mixed render for alternative B.
- (c) dependency-cruiser: the set of `HttpTransport` importers is unchanged.
- (d) E03 on pocold: a slot-2 proof verifies against the anchored header `stateRoot`; a proof request outside `K_eff` returns −32017.
- (e) RpcTap: every load issues ≤ 4 anchors and ≤ 8 `eth_getProof` calls.
- (f) The BR deny-list test still rejects `eth_getProof` from a site frame (D95).

**Fallback if the owner rejects CR-M1-01:** P36, which is alternative B with `eth_getStorageAt`, within the baseline. Its guarantee is stated as conditional on the absence of an A → B → A reorganization of block N during the load (SNAP-03D). The reader-location question (P35) remains in that case too.

### BLK-02 — Extension-annex items without definitions (X3)

**Exact source lines:** FD:L4959 requires "Q8–Q12, X1–X7 and d1–d5" (written `د1–د5`). FD:L2644 (T12) references "X1–X7, d1–d5, Q1–Q12". `reference/validation.md:130` and `reference/implementation.md:300` repeat the same text.

**Search performed:** every `reference/*.md` file was searched for `Q8`, `Q12`, `X7`, `د5`, `\bQ[0-9]+\b` and `د[0-9]`. The only matches are the references above, plus unrelated namesakes:
- `Q1`–`Q5` are HeadCheck predicates (`reference/validation.md:1458` onward);
- `X1..X100` are block labels in the network tests;
- `X8` is defined separately (FD:L2645).

**Consequence:** row X3 cannot be specified from the baseline.

**Preferred solution:** the owner supplies the 22 definitions, or the source document they came from, as a baseline addendum through change request CR-M1-02.

**Falsifiable acceptance test:** each of the 22 IDs resolves to a defining text with a line reference, and each becomes a T12 test row with a literal pass criterion. The check fails if any ID is undefined or has no criterion.

## 2. New decisions (reviewers, unless the owner is named)

| ID | Question | Preferred proposal | Baseline compatibility | Falsifiable tests | Decider |
|---|---|---|---|---|---|
| U23 | RLP depth policy (C03) | P32: no depth rule; total iterative decoding; `struct.shape` rejects later | Compatible; it adds no rejection of valid RLP | `rlp-deep-21916-max` → `struct.shape` with recursion limit 120 | Reviewers |
| U24 | Cache insertion (C05) | P34: insert only chunks that passed every fetch rule; key (addr, len) | Key fixed by FD:L1309 | `fr-same-address-different-length` gives 2 requests; mutants in `fetch-requests.json` | Reviewers |
| U25 | Location of the Website-state reader | P35: a ChunkFetcher-module function | Within FD:L4934 if accepted as part of ChunkFetcher; otherwise needs a CR | dependency-cruiser importer set unchanged | Owner (with BLK-01) |
| U26 | Out-of-range ABI integer arguments | P37: revert with empty data before authorization; no truncation | Compatible | T1-16 (E01) | Reviewers |

## 3. Owner questions (no agent can decide these)

| ID | Question | Why it matters |
|---|---|---|
| Q-C06 | Accept CR-M1-01 (proof-bound reads; `eth_getProof` added to RECV_LIMIT; reader in ChunkFetcher), or choose P36 (a conditional guarantee with `eth_getStorageAt`)? | Decides U05 and BLK-01. The M1-spec cannot be accepted without this decision |
| Q-C08 | How was design ID `e8a19ecb…` derived? Can `state(3).json`, named in `reference/baseline.json`, be supplied? | The derivation is unestablished. `tools/design_id_probe.py` tests simple candidates only |
| Q-X3 | Supply the definitions of Q8–Q12, X1–X7 and d1–d5 | BLK-02 |
| Q-R1 | Move batch 5 (R1, RECV_LIMIT) ahead of the bridge batches? | BLK-01's acceptance test (a) needs R1 |
| Q-C02 | Is `coordination/runtime/python311` (embedded Python 3.11.1) the approved interpreter for independent reruns? Should the `py` launcher entry that points to the missing `<local-user-home>\MiniConda3\python.exe` be repaired? This draft does not touch either | Independent reruns of 0.2 and 0.3 |
| (from 0.2) | U01, U02, U05, U10, U14, U15 | Unchanged; still routed to the owner |

## 4. Experiments (amended)

- **E03, extended:** besides MPT format and verification, the run must replay SNAP-01…13 through RpcTap against the implementation, and measure the load-level request totals against the bucket (FD:L1967).
- **E01, extended:** add T1-16 (out-of-range `uint32` argument).
- The rest are unchanged.
