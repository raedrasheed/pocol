# LOG-ACCT-01 — how logs are charged against LOG_BLOCK_MAX

- **Approval status:** PROPOSED. Not approved.
- **Implementation status:** alternative (c) is implemented provisionally (`lp4-execution
  user_phase::log_charge`, `caps::LOG_COUNT_MAX`), so that A03 could be repaired and tested.
- **Finding:** A03 (a 32,769-byte log was accepted with status 1).

## Exact normative text

1. `development/m1/reference/execution.md:82` (identical in FINAL_DESIGN.md:819):
   > SLOT_CAP، وACCT_CAP، وDIRTY_BLOCK_MAX=4096، وACCT_DIRTY_BLOCK_MAX=2048، وLOG_BLOCK_MAX=32 KiB، وLOG_CAP=1 GiB: توقف استثنائي.

   "SLOT_CAP, ACCT_CAP, DIRTY_BLOCK_MAX=4096, ACCT_DIRTY_BLOCK_MAX=2048, LOG_BLOCK_MAX=32 KiB,
   LOG_CAP=1 GiB: exceptional halt."
2. `execution.md:84`:
   > LOG_BLOCK_MAX مع حد عدد السجلات المشتق في LG2 (546) أساس لِمّتي P1 وP3 (D89).

   "LOG_BLOCK_MAX, with the log-count limit derived in LG2 (546), is the basis of lemmas P1 and P3
   (D89)."
3. `storage.md:53` (RetentionStore sizes):
   > … والسجل = 36+len.

   "… and a log = 36+len."
4. `storage.md:56–58`:
   > LG1 = d7 94 ‹20› c0 80، بمخزن 60 B. … LG2: 546 سجلًا تنجح، و547 تتوقف.

   "LG1 = d7 94 ‹20› c0 80, stored in 60 B. … LG2: 546 logs succeed, and 547 halt."
5. `governance.md:168`:
   > LogBomb: 546 سجل LOG0 لكل نداء، والبيانات 24 بايتًا آخر 8 منها = be64(arg).

   "LogBomb: 546 LOG0 logs per call; data of 24 bytes, the last 8 = be64(arg)."
6. `network.md:202` and `network.md:304`:
   > eth_getTransactionReceipt: ≤ 2·LOG_BLOCK_MAX+546·512 B+2 KiB ≈ 339 KiB.
   >
   > لِمّة P1: كتلة واحدة ≤ 32 KiB خامًا و≤ 546 سجلًا، وترميزها < 400 KiB.

   "eth_getTransactionReceipt: ≤ 2·LOG_BLOCK_MAX + 546·512 B + 2 KiB ≈ 339 KiB." / "Lemma P1: one
   block ≤ 32 KiB raw and ≤ 546 logs, and its encoding < 400 KiB."

## What is not specified
None of these texts says:
- what one log is charged against LOG_BLOCK_MAX: its data, its stored size, and whether topics
  count;
- whether "546" is a separate limit or only a consequence of the byte rule for LogBomb's logs.

LG2's boundary is exactly ⌊32768/60⌋ = 546, where 60 = 36 + 24 is LogBomb's 24-byte LOG0
charged 36 + |data|.

## Alternatives

| | per-log charge | count limit | LG2 (546 ok, 547 halt) | P1 / receipt bound (≤ 546 logs) |
|---|---|---|---|---|
| (a) | 36 + \|data\| | none | holds | **violated**: 910 empty LOG0 fit 32,760 B |
| (b) | \|data\| | none | **violated**: 547 × 24 = 13,128 B fits | violated |
| (c) recommended | 36 + 32·\|topics\| + \|data\| | ≤ 546 per block | holds | holds |
| (d) | 36 + \|RLP(log)\| (the stored size, storage.md:53) | none | **violated**: 546 × (36 + 48) = 45,864 B > 32,768 | holds for LogBomb |

(c) is the only reading consistent with every quoted text:
- For a LOG0 it is storage.md's "36+len" with len = |data|.
- It reproduces LG2.
- It charges topics, which the receipt bound encodes.
- It makes P1's "≤ 546 logs" a rule rather than a hope.

## Proposed amendment (to execution.md §السقوف, recorded here only)
English:
- One log is charged 36 + 32·|topics| + |data| bytes.
- The charges of a block's logs sum to at most LOG_BLOCK_MAX (equality allowed), and a block holds at
  most 546 logs.
- The first transaction that would exceed either limit halts exceptionally. A halted transaction adds
  nothing to either sum.

Arabic:
> يُحسب لكل سجل 36 + 32·|topics| + |data| بايتًا، ومجموع سجلات الكتلة ≤ LOG_BLOCK_MAX (المساواة مسموحة)، وعدد سجلات الكتلة ≤ 546. أول معاملة تتجاوز أحد الحدين تتوقف استثنائيًا، ولا يُضاف من سجلاتها شيء إلى أي من الحدين.

## Minimal examples (model vectors `vectors/log-acct-01.json`)

| scenario | (a) | (b) | (c) |
|---|---|---|---|
| L3: one LOG0, 32,733 B | halt | ok | halt (bytes) |
| L5: 547 LOG0 of 24 B (LG2) | halt | ok | halt (count) |
| L6: 547 empty LOG0 | ok (19,692 B) | ok | halt (count) |
| L9: one LOG2, 32,669 B | ok (32,705 B) | ok | halt (bytes: 36 + 64 + 32,669 = 32,769) |
| M1: three transactions of 20,000, 12,732 and 12,696 B | ok, halt, ok | ok, ok, halt | ok, halt, ok |

Where the alternatives disagree, the same transaction is ok under one and halts under the other.

## Effects
- **Receipts:** the halted transaction has status 0, no logs and an empty bloom contribution.
- **Gas:** a halt charges the whole gas limit at the effective price, so the cumulative gas and
  `gasUsed` of the block differ.
- **State root:** a halt reverts every change except the sender's nonce and fee, and the Escrow
  tip. The state root therefore differs for every block where the alternatives disagree.
- **Validity:** no block validity changes. Log limits halt the transaction, never the block.
- **Compatibility:**
  - Every accepted M1 vector is unchanged: LG1 is a storage size, and LG2 holds under (c).
  - Blocks valid under the audited snapshot 090eae4 that contain a transaction over these limits are
    still valid, but get different receipts and roots. No such chain exists outside local test runs.
  - The A13 history bound depends on LOG_CAP with (c)'s charge.

## Evidence
- Model vectors: `vectors/log-acct-01.json`, all four alternatives.
- Replay of the recommended alternative: `apply::tests::a03::proposals::log_acct_01_vectors`.
- Earlier unit tests: `apply::tests::a03::{log_block_max_below_at_and_above,
  lg2_546_logs_succeed_547_halt_and_topics_are_charged,
  cumulative_logs_across_transactions_and_a_halt_consumes_no_budget}`.
