# D-BODY-MAX — BODY_MAX bounds the encoded body on every path

- **Approval status:** PROPOSED. Not approved. This is a consistency alignment, but it can still
  change block validity.
- **Implementation status:** alternative (a) is implemented provisionally. `pocold Chain::validate`
  checks `body.encode().len() > BODY_MAX`, as `Body::decode` already did. The producer drops
  transactions until the encoded body fits.
- **Finding:** found by the first independent review of the A01 repair.

## Exact normative text
1. `development/m1/reference/consensus.md:56`:
   > Body = [txs, evidence] ≤ BODY_MAX.
2. `governance.md:12` and `governance.md:85`:
   > الكتلة [CP]: GAS_LIMIT=8000000، وbaseFee0=10^9، وTX_MAX=131072، وBODY_MAX=196608.
   >
   > R10: TX_MAX ≤ BODY_MAX−16 KiB.

   "Block [CP]: GAS_LIMIT=8000000, baseFee0=10^9, TX_MAX=131072, BODY_MAX=196608." / "R10: TX_MAX ≤
   BODY_MAX − 16 KiB."
3. `validation.md` T18b L2:
   > L2: معاملات بـcalldata 131000 بايت إلى Rv، تملأ BODY_MAX.

   "L2: transactions with 131,000-byte calldata to Rv, filling BODY_MAX."

## What is not specified
"Body = [txs, evidence] ≤ BODY_MAX" most naturally reads as the size of the encoded body. The audited
code checked two different quantities:
- decode (peers, journal) checked the encoded length;
- validation (every import, own blocks included) checked the sum of item bytes.

A node could therefore build and import a block whose encoding exceeds BODY_MAX. Its peers then
dropped that block at decode, and its own journal replay failed.

## Alternatives
- **(a) recommended:** |RLP([txs, evidence])| ≤ BODY_MAX, on every path. This is the literal reading,
  and decode already did it.
- **(b):** Σ |tx item| + Σ |evidence item| ≤ BODY_MAX. Then decode must change too: it would have to
  accept encodings up to BODY_MAX + the list headers.
- **(c):** the item sum plus a fixed 1024 B margin, roughly what the producer kept. This is
  stricter than (a) for many transactions and looser for none.

## Proposed clarification (to consensus.md §56, recorded here only)
English: "BODY_MAX bounds the length of the RLP encoding of Body = [txs, evidence], including every
list and string header; validation and decoding apply the same bound."

Arabic:
> يحد BODY_MAX طول ترميز RLP للجسم Body = [txs, evidence] بكل رؤوس القوائم والسلاسل، ويطبق التحقق وفك الترميز الحد نفسه.

## Minimal examples (`vectors/d-body-max.json`; the encoded lengths are computed by the model and checked equal to pocold's `Body::encode`)

| scenario | item sum | encoded | (a) | (b) | (c) |
|---|---|---|---|---|---|
| B1: 3 × 65,535 B | 196,605 | 196,623 | **invalid** | valid | invalid |
| B2: 2 × 98,000 B | 196,000 | 196,017 | valid | valid | **invalid** |
| B3: 131,072 B tx + 4 × 2,048 B evidence | 139,264 | 139,291 | valid | valid | valid |
| B4: 3 × 65,532 B | 196,596 | 196,614 | **invalid** | valid | invalid |

## Effects
- **Validity:** under (a), a block whose encoding exceeds BODY_MAX is invalid on import too. It was
  already undecodable on the network path.
- **Receipts, gas, state roots:** no change for any block valid under (a). (b) admits B1/B4-like
  blocks with their own roots.
- **Compatibility:**
  - No valid block of the specification becomes invalid, because the specification bounds the body
    itself.
  - The change removes the self-inconsistency: a block one path accepted and another rejected.
  - Under (b), decode would have to change, and RECV bounds that use BODY_MAX would need list-header
    slack.

## Evidence
- Vectors: `vectors/d-body-max.json`.
- Replay of (a): `pocold consensus::d_body_max_vectors` checks:
  - the encoded lengths equal the model's;
  - decode at BODY_MAX accepts exactly the bodies the model marks valid.
- `consensus::body_max_bounds_the_encoded_body_not_the_item_sum`: import and the producer.
