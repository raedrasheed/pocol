# CR-M1-01 — Proof-Bound Website-State Reads (Specification Addendum Proposal)

Status: **proposal for owner approval; not approved.** It does not edit the baseline. If approved, the owner applies the baseline text changes in §11. Revision: R4-05 (C06, BLK-01).
Inherits: `../m1-draft-0.2/M1-SPEC-0.2.md` §4.2 (P20 algorithm), `../m1-draft-0.3/M1-SPEC-0.3-AMENDMENTS.md` R3-04 (reconciliation, P35, P36), and R4-01 of `M1-SPEC-0.4-AMENDMENTS.md` (invariant checks).

## 1. Conflict being resolved

FD:L1274 says the extension does not send a method that is missing from the RECV_LIMIT table. The table (FD:L1262–1273) does not list `eth_getProof`. This CR adds one table row and one receive pool, and it places the reader inside an existing module. Nothing else in the baseline changes:
- **Sites stay banned.** `eth_getProof` stays in the D95 deny list for every kind (FD:L882, FD:L1204). The bridge test is `annex/bridge-matrix.json` `denyList.everyKind`, which `run_checks_04.py` runs for all four kinds.
- **The importer set is unchanged.** The reader is a function of the ChunkFetcher module, `readWebsiteState(website, keys, N, session)`. No new module imports `HttpTransport` (FD:L4934). The fixtures `vectors/dependency-graphs.json` `G-new-importer` (must fail) and `G-reader-in-chunkfetcher` (must pass) enforce this.

## 2. Requests (exact)

All requests are compact JSON. `<id>` is a u32. `W` is the Website address in lowercase hex. `N` is the anchored block number, written as a qty (FD:L1160).

| Name | Body | When |
|---|---|---|
| Anchor (LN) | `{"jsonrpc":"2.0","id":<id>,"method":"eth_getBlockByNumber","params":["latest",false]}` | Start of each attempt. In RP no request is sent: the anchor is the head header verified by the RP window (FD:L985–1036) |
| Proof 1 | `{"jsonrpc":"2.0","id":<id>,"method":"eth_getProof","params":["W",["0x0000000000000000000000000000000000000000000000000000000000000002"],"N"]}` | After the anchor |
| Proof 2 | `{"jsonrpc":"2.0","id":<id>,"method":"eth_getProof","params":["W",[base,base+1,base+2,e0,e1,e2],"N"]}`; the keys are 32-byte `0x`-hex values for version n (0.2 §5.5); e0–e2 are the first three element slots | After proof 1, when R4-01 selects version n |

- The block parameter is always the number N. It is never a tag and never an EIP-1898 object.
- Proof 2 always carries exactly 6 keys. Element slots that are not used prove the value 0.

## 3. Response acceptance (exact order)

A failure in classes 1–4 ends the **attempt**. The attempt counts toward the limit of 4 (3 restarts) in §6.

| # | Class | Condition | Outcome |
|---|---|---|---|
| 1 | Transport | `recvLimit`, `recvParse`, `recvDepth`, `transport`, `busy` (FD:L4935), or the 10 s per-request deadline (U14 preferred reading) | Attempt fails |
| 2 | RPC error | A JSON-RPC `error` object. −32017 (outside `K_eff`) is the expected case; any other code is treated the same (P) | Attempt fails |
| 3 | Shape | `result` is not an object, or a required field is missing or mistyped. Required fields: `address` addr; `accountProof` array of 1..65 data strings, each decoding to ≤ 564 bytes; `balance` hex quantity ≤ u256; `codeHash` hash32; `nonce` qty; `storageHash` hash32; `storageProof` array of objects `{key, value, proof}`, where `value` is a hex quantity ≤ u256 and `proof` is an array of 0..65 data strings, each ≤ 564 bytes. Unknown extra fields are ignored (P, pending E03 on the pocold format) | Attempt fails |
| 4 | Binding | `address` ≠ W (case-insensitive); `storageProof` length ≠ number of requested keys; `storageProof[i].key` ≠ requested key i (compared as u256 values, because clients return either padded or compact keys); an account proof that does not verify from the anchored `stateRoot` to `keccak256(W)` with leaf value `RLP([nonce, balance, storageHash, codeHash])` equal to the reported fields; or a storage proof that does not verify from `storageHash` to `keccak256(key)` with `RLP(value)`, or to a valid absence with value 0 | Attempt fails |
| 5 | Account | The account is absent, or `codeHash = keccak256("")` | `noWebsite`: zero frames, no restart (P) |
| 6 | State | R4-01 checks S1–S3 (after proof 1) and V1–V4 (after proof 2) | `stateInvariant`: zero frames, no restart |

## 4. Byte bound (conditional) and why FD:L935 is not used

**Premises. Neither is stated in the baseline; E03 must confirm both:**
- **PR-1:** the state and storage tries are hexary Merkle-Patricia tries keyed by `keccak256` of the 32-byte key (64 nibbles), with Ethereum node encoding.
- **PR-2:** each proof element is a quoted `0x`-hex string of the node RLP, followed by a separator.

**FD:L935 is unsupported as a proof-size source.** FD:L935 gives `P(d) = 568d+186+32(d+1)` but defines neither P nor d, and the baseline gives no bound on trie depth. Note that 568 = 32+4+532 matches FD:L921, a storage record ("trie node = 32+4+RLP") of a full branch node. So P(d) reads as a per-path storage cost, not a proof size. No number is derived from it.

**Derivation under PR-1 and PR-2** (`annex/recv-limits.json` `proofBoundDerivation`, checked by `run_checks_04.py`):
- Path: every branch node consumes one nibble and every extension node at least one, and a leaf ends the path. So a path has at most 65 nodes.
- Largest node: a branch with 16 hash references and a value slot of at most 33 bytes is 564 bytes. The runner encodes such a node and checks the length.
- Per node in JSON: 2·564 + 5 = 1133 bytes.
- Per path: 65 · 1133 = 73645 bytes.
- Per request with k keys: (k+1) paths + 4096 bytes for fixed fields and the envelope. That is 151386 bytes for k = 1 and 519611 bytes for k = 6.
- The runner also builds the worst-case JSON response for k = 1 and k = 6 and checks it against these bounds.

**Proposed RECV_LIMIT row:** `524288 [exact, conditional on PR-1/PR-2]: eth_getProof (Website account, ≤ 6 keys)`. The client never sends more than 6 keys.

## 5. Pool and resource accounting

- **New pool `STATE_POOL = 1048576`**, that is 2 × 524288, so two proof reads can be in flight across all sessions. It sits outside RECV_INFLIGHT_MAX and is bounded by its bytes, in the same way as TRUST_POOL (FD:L1281–1283). Reservation is FIFO, and failure within 2 s is `busy`: the attempt fails (FD:L1237).
- **Rejected placement: CONTENT_POOL.** CONTENT_POOL is 8·68 KiB = 557056 bytes (FD:L1282). It could hold only one 524288-byte reservation, and that reservation would stall chunk fetches for every session.
- **Memory:** the reserved receive memory grows by 1 MiB. The baseline gives the current total as about 34.6 MiB (FD:L1286, FD:L2183); with this pool it becomes about 35.6 MiB. The CR updates both lines.
- **Node side, unchanged:** `eth_getProof` is heavy (FD:L1960); HEAVY_ACTIVE = 4 and HEAVY_QUEUE = 16 (FD:L1966); the per-connection bucket is 20/s with capacity 20 (FD:L1967).

## 6. Restart and request budget

- At most 4 attempts per load (one try plus 3 restarts, 0.2 §4.2 step 4).
- Per attempt: at most 1 anchor and 2 `eth_getProof` calls.
- Per load: at most 4 anchors (LN only) and 8 `eth_getProof` calls.
- Worst received bytes for state reads: 4 × 167936 + 8 × 524288 = 4866048 bytes, received sequentially, with at most one proof request per session in flight.
- After the state reads, chunk reads are bounded by R4-03: at most 2850 `eth_getCode` requests per load.

## 7. Authenticity assumptions

| ID | Assumption | Consequence if false |
|---|---|---|
| A1 | LN: the local node is trusted (D13) | Proofs then give single-root consistency only, not authenticity |
| A2 | RP: the anchor is a PoW-checked header inside the RP window | No execution verification. A consistent lie remains possible (D13, FD:L6288); the RP label rules apply unchanged |
| A3 | PR-1 and PR-2 (§4) | The byte bound and the verification algorithm do not apply. E03 fails, and the CR must be revised |
| A4 | Keccak-256 collision resistance | Proof binding fails |
| A5 | The code at W is a Website contract with the P13 layout | Not verified by the client. A later option, once E04 exists, is to compare `codeHash` with known runtime hashes (out of scope) |

## 8. The weaker detector is not equivalent

Alternative B (`eth_getStorageAt` at N with a block-hash check before and after) stays documented as a **counterexample**. Scenario `SNAP-03D` (0.3) shows that it renders content that was never current in any state. If the owner rejects this CR, P36 applies (0.3 R3-04): the viewer must state the conditional guarantee and must not claim single-state consistency.

## 9. Future MPT implementation tests

These run against candidate code in Phase A (R4-02), not now:

| ID | Input | Pass |
|---|---|---|
| MPT-1 | Proofs from anvil (Cancun) for a deployed Website: slot 2 and the 6 version keys | Verify against the block's `stateRoot` |
| MPT-2 | Proof for an unused element slot | Verifies as absence, value 0 |
| MPT-3 | One byte of one node flipped | Binding failure |
| MPT-4 | Last node removed | Binding failure |
| MPT-5 | An extra node appended | Binding failure |
| MPT-6 | Valid proof for another address | Binding failure (`address` mismatch) |
| MPT-7 | Valid proof for other keys | Binding failure (SNAP-07) |
| MPT-8 | Embedded nodes (< 32 bytes) on the path | Verify correctly |
| MPT-9 | Synthetic 65-node path with 564-byte branch nodes | Accepted, and the JSON fits the 524288 bound |
| MPT-10 | Response of exactly 524288 bytes / 524289 bytes | Accept / `recvLimit` |
| MPT-11 | −32017 from pocold outside `K_eff` | Restart; after 4 attempts, `inconsistent` with zero frames |
| MPT-12 | pocold `eth_getProof` on sv-fix (E03) | Node format, depth and size observed. PR-1/PR-2 hold, or the CR is revised |

## 10. Acceptance tests for this CR

Executable now: `run_checks_04.py` sections `recv.proof*`, `dependency.*`, `bridge.denyList` and `snapshot.*`. Owner-level acceptance is the explicit approval of §4 (premises), §5 (new pool and memory delta) and §11.

## 11. Proposed baseline text changes

The owner applies these if the CR is approved; they are shown in English:
1. After FD:L1272, add the row: "524288 [exact, conditional]: eth_getProof for the Website account with ≤ 6 storage keys. Worst (k+1)·65·1133+4096 = 519611 under hexary MPT with 32-byte hashed keys."
2. At FD:L1217, add `eth_getProof` for Website state to the internal-method examples, routed through ChunkFetcher.
3. At FD:L1278–1283, add the pool `STATE_POOL = 1 MiB` for Website-state proofs, outside RECV_INFLIGHT_MAX.
4. At FD:L1286 and FD:L2183, change the reserved receive total from about 34.6 MiB to about 35.6 MiB.
5. At FD:L4942, add to ChunkFetcher: `readWebsiteState(website, keys, N, session)`, with the algorithm of M1-spec §4.2 and R4-01.
