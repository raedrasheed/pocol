# U08 owner change request: canonical manifest-chunk split

**Status: OPEN, for the owner.** Root classified U08 as a change request in REVIEWER-DECISIONS-0.25 and in ledger entry U08-CR. It is not accepted through grouped P-0.2, and no reviewer default applies. This file routes the request. It decides nothing.

## The question

**What the baseline says.** It fixes the Website record as `Version{manifestHash, manifestLen|status|publishedBlock, manifest chunks}` (storage.md:32) and the limits `manifest ≤ 64 KiB, versions ≤ 1024` (storage.md:36, FD:L904). It does **not** restrict how a manifest is split into chunks: any list of chunks whose lengths sum to `manifestLen`, each 1–24575 bytes, is a valid input.

**What the draft adds.** Draft 0.2 adds P22/U08, which requires exactly ⌈len/24575⌉ chunks (1–3) with the sizes 24575, 24575, rest:
- **Contract side:** T1-14 `ManifestSplit(0)`.
- **Viewer side:** rule `manifest.split`, fixture `ver-manifest-noncanonical-split`.

This narrows the accepted input domain.

## Why it matters for bounded reads

CR-M1-01 (still an open owner item) reads a version with one `eth_getProof` request of **exactly 6 keys**:
- `base`, `base+1` and `base+2`;
- the first three element slots `e0..e2` (m1-draft-0.4/CR-M1-01-STATE-PROOF-READS.md, proof 2).

The `RECV_LIMIT` row it proposes (524288 bytes, ≤ 6 keys) and `STATE_POOL` = 1 MiB are derived from that count. A manifest may have more than three chunks only if the split is not canonical. Then:
- one proof request cannot cover the version;
- the per-load request count, the proof byte bound and the pool reservation all grow with the chunk count, up to 65536 one-byte chunks.

## Alternatives

| ID | Rule | Consequence |
|---|---|---|
| A | Canonical split enforced in the contract (`ManifestSplit`) and by the viewer | Narrows the contract input domain. Any manifest still has a valid canonical split, so no content becomes unpublishable; only the publisher's choice of split is removed. Writes are bounded (≤ 3 chunks). CR-M1-01's 6-key proof and its byte bound hold for every stored version |
| B | Contract rejects more than 3 chunks, but any sizes summing to `manifestLen` are allowed | Narrows less than A. The proof still needs ≤ 3 element keys, so the 6-key bound holds. Chunk addresses are no longer deterministic for a given manifest, and two publishers can store the same manifest twice under different splits |
| C | No contract rule (baseline as is); the viewer accepts any split | No narrowing. CR-M1-01 must be amended for multiple proof requests per version with a per-load key budget, and `RECV_LIMIT`, `STATE_POOL` and retry caps must be re-derived. A malicious version can force up to ⌈(3 + chunks)/6⌉ proof requests |
| D | No contract rule; the viewer refuses versions whose split is not canonical (`manifest.split`, as the 0.2 viewer fixture does) | No contract narrowing. A non-canonical version can be stored, published and selected, yet is never displayed. CR-M1-01's 6-key bound holds for every displayed version. Publisher tools must split canonically to be viewable |

## Dependencies

- **T1-14** encodes A. It changes under B, C or D.
- **The viewer fixture `ver-manifest-noncanonical-split`** encodes A or D.
- **CR-M1-01** assumes A, B or D.
- **F09 and F17** stay open until U08 is decided.

## Recommendation

**A.** It is the only option that keeps every on-chain version both bounded and deterministic without a viewer-only "stored but unviewable" class, and it costs publishers nothing that a valid manifest needs.

If the owner does not want to narrow the contract domain, **D** is the fallback that preserves the baseline contract and CR-M1-01's bound.
