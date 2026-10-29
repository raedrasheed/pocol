# LOG-CAP-AGGREGATE — the scope of LOG_CAP

- **Approval status:** PROPOSED. Not approved.
- **Implementation status:** the chain alternative is implemented provisionally:
  - `BlockInput::cum_log_bytes`, the charged log bytes of the canonical ancestors;
  - `user_phase::BlockMeter` (`Halt::LogCap`).
- **Finding:** A03 (and the A13 history bound).

## Exact normative text
1. `development/m1/reference/execution.md:82`:
   > … وLOG_BLOCK_MAX=32 KiB، وLOG_CAP=1 GiB: توقف استثنائي.

   "… LOG_BLOCK_MAX=32 KiB, LOG_CAP=1 GiB: exceptional halt."
2. `validation.md:2065`:
   > ثوابت البناء caps-test (simnet فقط، غائبة عن الإصدار): LOG_CAP=2^24 …

   "caps-test build constants (simnet only, absent from the release): LOG_CAP=2^24 …"

## What is not specified
What LOG_CAP bounds is not stated.
- A per-block reading is vacuous, because LOG_BLOCK_MAX (32 KiB) is far smaller.
- The caps-test lowering it to 2^24 means it is reachable in a test, so it must be cumulative.

## Alternatives
- **chain (recommended):** the charged log bytes (LOG-ACCT-01) of the canonical chain up to the
  parent, plus this block's, ≤ LOG_CAP. The first transaction that would exceed it halts. This is
  the bound A13's history term relies on.
- **block:** per block. Vacuous.
- **retained:** only logs inside the RetentionStore window. That bounds the stored logs rather than
  the chain. Its value depends on local retention, which differs between nodes, and so it is not
  consensus-safe.

## Proposed amendment (recorded here only)
English: "LOG_CAP bounds the sum of charged log bytes over the canonical chain, this block included;
the first transaction that would exceed it halts exceptionally."

Arabic:
> يحد LOG_CAP مجموع بايتات السجلات المحسوبة على السلسلة القانونية شاملة الكتلة الحالية، وأول معاملة تتجاوزه تتوقف استثنائيًا.

## Minimal examples (`vectors/log-cap-aggregate.json`)

| scenario | chain | block | retained (window empty) |
|---|---|---|---|
| G1: 100 B left, LOG0 of 64 B (charge 100) | ok | ok | ok |
| G2: 100 B left, LOG0 of 65 B (charge 101) | **halt LogCap** | ok | ok |
| G3: chain total far below the cap | ok | ok | ok |

## Effects
- **Validity:** none. Every response is a halt.
- **Receipts, gas, roots:** differ once a chain approaches LOG_CAP.
- **Compatibility:**
  - Unreachable at the release LOG_CAP (1 GiB) for a short devnet.
  - Every node computes the same value under "chain", because it is a function of the canonical
    chain. That is not true of "retained".

## Evidence
- Vectors: `vectors/log-cap-aggregate.json`.
- Replay: `apply::tests::a03::proposals::log_cap_aggregate_vectors`.
- `apply::tests::a03::log_cap_is_aggregate_over_the_chain`.
