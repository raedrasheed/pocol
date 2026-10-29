# D-A03-DIRTY — what the dirty limits count, and a halt whose own effects do not fit

- **Approval status:** PROPOSED. Not approved.
- **Implementation status:** alternative A (net changes; a halt residue that does not fit
  invalidates the block) is implemented provisionally:
  - `lp4-execution user_phase::BlockMeter::check` and `apply_halt`;
  - `UserError::DirtyAccounts(i)` and `UserError::AcctCap(i)`;
  - pocold's builder drops transaction `i` and retries (`Node::build_signed_template`).
- **Finding:** A03.

## Exact normative text
1. `development/m1/reference/execution.md:82`:
   > SLOT_CAP، وACCT_CAP، وDIRTY_BLOCK_MAX=4096، وACCT_DIRTY_BLOCK_MAX=2048، وLOG_BLOCK_MAX=32 KiB، وLOG_CAP=1 GiB: توقف استثنائي.

   "… DIRTY_BLOCK_MAX=4096, ACCT_DIRTY_BLOCK_MAX=2048 …: exceptional halt."
2. `execution.md:83`:
   > TXBYTES_CAP وTX_CAP: الكتلة باطلة، والبنّاء يتخطى المعاملة.

   "TXBYTES_CAP and TX_CAP: the block is invalid, and the builder skips the transaction."
3. `validation.md:2087` (T18b criterion):
   > الاستجابة المعلنة في execution وstorage بالضبط: فشل الإيداع، أو توقف استثنائي، أو بطلان الكتلة وتخطي المعاملة، أو تجميد.

   "Exactly the response declared in execution and storage: deposit failure, exceptional halt, block
   invalid and the transaction skipped, or freeze."

Background: an exceptional halt consumes the whole gas limit and reverts every effect of the
transaction except two:
- the sender's nonce increment and fee payment;
- the coinbase (Escrow) tip.

This is standard EVM semantics; the M1 text uses the term without restating it.

## What is not specified
1. What "dirty" counts: keys and accounts whose value changed (net, per transaction), or every key
   and account the transaction touched.
2. What happens when the halt's own residue does not fit. The residue is the sender's nonce and
   balance, plus the Escrow tip. Example: the block already has ACCT_DIRTY_BLOCK_MAX dirty accounts,
   and the halted transaction's sender is not among them. Applying the halt would make 2049 dirty
   accounts. Not applying it is impossible, because the body commits to the transaction. The text
   prescribes an exceptional halt, which assumes the halt itself always fits.

## Alternatives
- **A (recommended):**
  - Dirty means a net change in the transaction: a value that differs after it. An account
    receiving and paying out the same amount is unchanged.
  - A transaction whose changes would exceed either limit halts.
  - If the halt residue itself would exceed ACCT_DIRTY_BLOCK_MAX (or create an account beyond
    ACCT_CAP), the transaction cannot be applied, so the block is invalid. The builder skips the
    transaction, exactly as for TX_CAP (execution.md:83).
- **B:** as A, but the halt residue is exempt from the limits. The block is always valid. The
  effective bound becomes ACCT_DIRTY_BLOCK_MAX + (halted senders) + 1, and every derivation that
  uses the limit must use that larger figure.
- **A-touched:** as A, but every touched account counts, including accounts read or called without
  a change.
  - Reading or calling an account then costs dirty budget.
  - A transaction can then halt while changing no more accounts than the limit.
- A further option was rejected:
  - Dropping the halted transaction without effect contradicts the body commitment: the nonce
    would not advance, and the same transaction could be replayed.

Why A:
- It keeps the limit exact.
- It adds no new kind of response: "block invalid, builder skips" already exists for TX_CAP.
- A block it rejects can always be rebuilt without the offending transaction. Its sender is the
  only new account in the residue.

## The block-invalid case in detail (scenario D3)
Profile: ACCT_DIRTY_BLOCK_MAX = 2048 (real value), GAS_LIMIT 120M (needed to change 2046 accounts in
one transaction). Alice and Bob are funded accounts. SPRAY pays 1 wei to each of n new accounts and
receives n wei, so its own balance does not change. SPRAY_B is the same contract paying a different
range of new accounts. Every transaction pays a tip, so the Escrow coinbase changes.

1. **tx0** (Alice → SPRAY, n = 2046). It changes Alice, the Escrow and 2046 new accounts: 2048 in
   total, exactly at the limit. It succeeds with status 1. The block now has 2048 dirty accounts.
2. **tx1** (Bob → SPRAY_B, n = 1). Its changes would add Bob and one new account. The Escrow is
   already dirty. That makes 2050 > 2048, so it halts.
3. **tx1's halt residue** is Bob's nonce and balance, plus the Escrow tip, which is already dirty.
   That makes 2049 > 2048, so the residue does not fit.
4. **Under A:** tx1 cannot be applied. The block is invalid (`UserError::DirtyAccounts(1)`). A
   builder that hits this drops tx1 and builds [tx0], which is valid. Bob's transaction stays in the
   pool for a later block.
5. **Under B:** tx1 is applied as a halt. Bob's nonce is +1, Bob pays gas_limit × price, and the
   Escrow gains gas_limit × tip. The receipt has status 0. The block is valid with 2049 dirty
   accounts.

Scenario D4 is the same as D3, but tx1 is sent by Alice. Its residue (Alice, the Escrow) is already
dirty, so it fits. Under both A and B, tx1 halts with status 0 and the block is valid.

## Proposed amendment (to execution.md §السقوف, recorded here only)
English:
- DIRTY_BLOCK_MAX and ACCT_DIRTY_BLOCK_MAX bound the union, over a block's USER transactions, of
  the storage keys and accounts whose value a transaction changes (net, per transaction).
- A transaction whose changes would exceed either bound halts exceptionally.
- If the halt's own effects (nonce and fee of the sender, the coinbase tip) would exceed
  ACCT_DIRTY_BLOCK_MAX, or create an account beyond ACCT_CAP, the block is invalid, and the builder
  skips the transaction.

Arabic:
> يحد DIRTY_BLOCK_MAX وACCT_DIRTY_BLOCK_MAX اتحاد مفاتيح التخزين والحسابات التي تغيّر كل معاملة مستخدم قيمتها في الكتلة (التغير الصافي لكل معاملة). المعاملة التي يتجاوز تغييرها أحد الحدين تتوقف استثنائيًا. إن كانت آثار التوقف نفسه (nonce المرسل ورسومه وإكرامية COINBASE) تتجاوز ACCT_DIRTY_BLOCK_MAX، أو تنشئ حسابًا فوق ACCT_CAP، فالكتلة باطلة، والبنّاء يتخطى المعاملة.

## Minimal examples (`vectors/d-a03-dirty.json`)

| scenario | A | B | A-touched |
|---|---|---|---|
| D1: Alice sprays 2046 | valid, [1] | valid, [1] | valid, [0] (SPRAY counted: 2049) |
| D2: Alice sprays 2047 | valid, [0] | valid, [0] | valid, [0] |
| D3: then Bob sprays 1 fresh account | **invalid** at tx1 | valid, [1, 0] (2049 dirty accounts) | valid, [0, 1] |
| D4: then Alice sprays 1 fresh account | valid, [1, 0] | valid, [1, 0] | valid, [0, 1] |

## Effects
- **Validity:** A adds one block-invalid condition, a halt residue that does not fit. It is of the
  same kind as TX_CAP. B adds none. A-touched changes which transactions halt.
- **Receipts and gas:** where the alternatives disagree, a transaction has status 1 under one and
  status 0 under another, or is absent (A, invalid block). Gas used differs by the halted
  transaction's whole gas limit.
- **State root:** differs in every disagreeing case:
  - a halt keeps only nonce, fee and tip;
  - an invalid block has no root and is rebuilt without the transaction.
- **Compatibility:**
  - No accepted M1 vector is affected.
  - At the devnet GAS_LIMIT (8M), changing 2048 accounts or 4096 keys in one block is unaffordable
    (≈ 9,300 gas per account change, ≥ 2,900 per key). These limits, and therefore this decision, bind
    only at higher GAS_LIMIT profiles. The 120M test profile is one.
  - The A13 and RECV derivations that use ACCT_DIRTY_BLOCK_MAX stay exact under A and A-touched, but
    not under B.

## Evidence
- Model: `model/consensus_model.py::dirty_block`.
- Vectors: `vectors/d-a03-dirty.json`.
- Replay of A at the real limit: `apply::tests::a03::proposals::d_a03_dirty_vectors`. It includes
  D3's invalid block, `DirtyAccounts(1)`.
- Unit test: `apply::tests::a03::dirty_keys_and_accounts_per_block` (4096/4097 keys, 2048/2049
  accounts).
- Builder skip: `Node::build_signed_template` drops the reported index and retries.
