# Alternative E: a Single-Call `eth_call` Snapshot Getter (evaluation)

Revision R7-06 (C06 context). **Conclusion first:** E is baseline-compatible and gives single-state consistency **against an honest node** (LN or DEV). It does **not** bind the state to an authenticated root, so in RP it is strictly weaker than the proof-bound reads of CR-M1-01. It is **not presented as equivalent**. The requirement is not weakened. CR-M1-01 stays the pending owner decision for authenticated RP reads, and no new owner question is raised.

## Mechanism

- **Contract (P, Website ABI addition):** `websiteSnapshot(uint32 n) view returns (uint32 versionCount, uint32 currentVersion, uint32 selected, bytes32 manifestHash, uint32 manifestLen, uint8 status, uint64 publishedBlock, address[] chunks, uint32[] lengths)`. `n = 0` selects the current version. If n > versionCount, `selected = n` and the record fields are zero.
- **Client:** one request, `eth_call({to: website, data: selector ‖ abi(n)}, N)`, at the anchored number N (LN), or at the RP window head number. The decoded tuple goes through the same R4-01 checks: S1–S3, then V1–V4, then the outcome.
- **Transport, baseline-compatible:** `eth_call` is in the RECV_LIMIT table (1 MiB capped, FD:L1271), so FD:L1274 is satisfied without a change request. The reader stays in the ChunkFetcher module (no new importer). `eth_call` is heavy (FD:L838: a snapshot, 30M gas, 5 s), so it uses the same heavy budget as proofs, with one call instead of two.

## What E guarantees

| Property | P20 (CR-M1-01) | E |
|---|---|---|
| Single state for the whole tuple (no A→B→A mixing) | Yes; verified against the anchored stateRoot | Yes for an honest node (one eth_call runs on one snapshot); not verifiable by the client |
| Binding to an authenticated header (RP window, PoW-checked) | Yes: proofs verify against the header's stateRoot | **No.** The endpoint can return any tuple, and the client cannot detect it |
| Cost of a lie in RP | Forging needs a stateRoot inside a PoW-valid header (mining work) | None: return arbitrary ABI bytes |
| Needs a baseline change request | Yes (BLK-01: RECV_LIMIT row, pool) | No (method listed). Needs a Website ABI addition (P) |
| LN trust model (D13: the local node is trusted) | Consistent with LN | Consistent with LN |

The fixtures in `vectors/snapshot-e-cases.json` are executed by `tools/snapshot_e_ref.py` (an abstract model):
- **E-1:** an honest node at A, B or a sequence A→B→A gives one state per call, never mixed.
- **E-2:** a dishonest RP endpoint fabricates a published version with manifest `M_EVIL`. E renders it. Under P20 the same dishonest endpoint can only send forged proofs, and 0.3 SNAP-05 shows the result `inconsistent` with zero frames.
- **E-3:** an undefined status 3 in the tuple gives `stateInvariant`, exactly as under P20.

## Recommendation (P, within the pending CR decision)

- **LN and DEV:** E meets the M1 single-state requirement under the baseline's own LN trust (D13) without a change request. It could be adopted for LN/DEV if reviewers accept the ABI addition.
- **RP:** E would turn the authenticated single-root requirement into "trust the remote endpoint". That is a weakening, so it is **not** proposed for RP. RP reads need CR-M1-01 (owner), or the explicitly weaker P36 labelling. The CR stays the single pending decision, and E is listed only as an LN/DEV option inside it.
