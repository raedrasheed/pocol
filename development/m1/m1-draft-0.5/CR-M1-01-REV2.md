# CR-M1-01 rev 2 — Proof-Bound Website-State Reads

Status: **proposal for owner approval; not approved.** Revision R5-04 (C06, C17, C18).

This revision supersedes §2–§6 of `../m1-draft-0.4/CR-M1-01-STATE-PROOF-READS.md`. Its §1 (the conflict, the site ban and the importer set), §7 (assumptions), §8 (the detector counterexample), §9 (MPT-1…12) and §11 (baseline text changes) stay in force, with the edits marked "rev 2" below. External semantic sources:
- EIP-1186 (`eth_getProof`): its parameters, its account and storage fields, and its account-absence rationale;
- the Ethereum Merkle-Patricia trie documentation (fixed-length secure-trie keys).

Neither source shows that pocold supports these semantics; that is E03.

## 2. Requests (rev 2)

| Name | `params` | Notes |
|---|---|---|
| Anchor (LN) | `"eth_getBlockByNumber", ["latest", false]` | In RP no request is sent: the anchor is the head header verified by the window |
| Proof 1 | `"eth_getProof", [W, [slot2], N]` | W is the 20-byte address as lowercase `0x`+40 hex. slot2 is `0x`+64 hex. N is a qty |
| Proof 2 | `"eth_getProof", [W, [base, base+1, base+2, e0, e1, e2], N]` | Always 6 storage keys, each `0x`+64 hex |

- Envelope: `{"jsonrpc":"2.0","id":<u32>,"method":…,"params":…}`.
- Each request has its own id, unique within the load (P: a per-load counter).

## 3. Response acceptance (rev 2, exact order)

**3.1 Transport** (FD:L4935): `recvLimit`, `recvParse`, `recvDepth`, `transport`, `busy`, or the 10 s per-request deadline → the attempt fails.

**3.2 Envelope** (strict; `proof_response_ref.envelope`):
- the response is a single JSON object (a batch array fails);
- no duplicate keys and depth ≤ 16 (FD:L1244);
- `"jsonrpc" == "2.0"`;
- `id` is an integer token equal to the request id;
- exactly one of `result` and `error`;
- no other top-level field;
- an `error` is `{code: integer, message: string, data?}`.

Any violation fails the attempt.

**3.3 Error** (`classify_error`):

| Code | Outcome |
|---|---|
| −32021 | `data.retryAfterMs` must be an integer in 0–10000 (P). If it is, wait that long and resend the **same** request with the **same** anchor (§6); this is not a restart. Otherwise the attempt fails |
| −32017 (outside K_eff) | Attempt fails (restart) |
| Any other code | Attempt fails (P) |

**3.4 Shape (EIP-1186):**
- Required fields: `balance` (hex quantity ≤ u256), `nonce` (≤ u64), `codeHash` (hash32), `storageHash` (hash32), `accountProof` (array of ≤ 65 node strings, each ≤ 564 bytes), `storageProof` (array whose length equals the number of requested keys).
- Each `storageProof` element is `{key, value, proof}`:
  - `key` may be zero-padded or compact; it must equal requested key i as a u256;
  - `value` is a hex quantity ≤ u256;
  - `proof` is ≤ 65 nodes, each ≤ 564 bytes.
- `address` is **optional**. If present, it must equal W, ignoring case.
- Unknown extra fields are ignored (P, U36).

**3.5 Binding and decision** (`decide`, in this order):
1. If `accountProof` is empty, it is accepted only when the anchored `stateRoot` is the empty-trie root `0x56e8…b421` = keccak256(0x80). In that case absence is authenticated. Otherwise the attempt fails.
2. Account proof from `stateRoot`, along the path **keccak256(W as 20 bytes)**:
   - **invalid**: the attempt fails;
   - **verified absence**: `noWebsite` with zero frames and no restart. The reported balance, nonce, codeHash and storageHash are **not used**.
   - **present**: the proven leaf RLP([nonce, balance, storageHash, codeHash]) must equal the reported fields, otherwise the attempt fails.
3. If the present account's `codeHash` = keccak256("") `0xc5d2…a470` (no code): `noWebsite`.
4. For each storage key i, the path is **keccak256(slot as 32 bytes)**, starting from `storageHash`:
   - an empty proof is allowed only if `storageHash` is the empty-trie root, and then means value 0;
   - **invalid**: the attempt fails;
   - **absent**: the reported value must be 0, otherwise the attempt fails;
   - **present**: the proven value must equal the reported value, otherwise the attempt fails.
5. The state checks of R4-01 (S1–S3 after proof 1, V1–V4 after proof 2). A violation gives `stateInvariant` with zero frames and no restart.

**PR-1 (rev 2):** the state trie and the storage tries are hexary Merkle-Patricia tries.
- The state trie is keyed by keccak256 of the **20-byte address**.
- A storage trie is keyed by keccak256 of the **32-byte big-endian slot**.

Both keys are 32 bytes, so every path is 64 nibbles. The bound derivation of 0.4 §4 (≤ 65 nodes per path, ≤ 564 bytes per node, 524288 bytes for 6 keys) depends only on the 64-nibble path length, so it is unchanged. PR-1 is still unstated in the baseline (BLK-03).

## 4–5. Bound and pool

Unchanged from 0.4: 524288 bytes [exact, conditional on PR-1 and PR-2]; `STATE_POOL` = 1 MiB; memory +1 MiB.

## 6. Request, retry and restart budget (rev 2)

| Quantity | Limit |
|---|---|
| Attempts per load | ≤ 4 (≤ 3 restarts) |
| Requests per attempt | anchor (LN only), proof 1, proof 2 |
| −32021 retries | ≤ 3 per request (FD:L1004 precedent); the 4th −32021 fails the attempt |
| Single wait | 0–10000 ms (P) |
| Cumulative wait per load | ≤ 20000 ms (P); beyond that, `unavailable` with zero frames |
| Sends per load | ≤ 4 × 3 × (1 + 3) = **48**, of which ≤ 32 are `eth_getProof` |
| Worst received bytes for state reads | 16 × 167936 + 32 × 524288 = 19464192 bytes, sequential, at most one proof request in flight per session. This replaces the 0.4 figure, which assumed no retries |

Content reads after the state reads: ≤ 2850 `eth_getCode` requests (R4-03 and R5-01).

The fixtures `BUD-1`…`BUD-7` simulate:
- one retry;
- retries that exhaust an attempt and force a restart;
- the cumulative wait cap;
- a failure followed by a successful attempt;
- the worst case of exactly 48 sends;
- early `noWebsite`;
- an early invariant violation.

## 7. Assumptions (rev 2 edit)

A3 now refers to PR-1 rev 2 (20-byte address keys versus 32-byte slot keys). A1, A2, A4 and A5 are unchanged.

## 9. Future tests (rev 2 additions)

These run in Phase A against candidate code:

| ID | Input | Expected |
|---|---|---|
| MPT-13 | Account-absence proof for a never-used address (anvil) | `noWebsite`, whatever the reported fields |
| MPT-14 | A response that includes `address` for another account | Binding failure |
| MPT-15 | −32021 with `retryAfterMs` from pocold (RG test harness) | Wait observed; same anchor; counted per §6 |
| MPT-16 | Storage keys returned in compact form by the client | Accepted when numerically equal |

## 10. What the fixtures do and do not show

`vectors/proof-response-cases.json` tests the envelope, error, shape and decision **order**, around a stated verifier outcome. **It does not verify any Merkle-Patricia proof**, and abstract mocks are not presented as such evidence.

The known-answer values check only the hashing conventions:
- keccak256 of slots 0, 1 and 2;
- the empty-trie root;
- keccak("");
- that the 20-byte account path differs from a 32-byte padded hash.
