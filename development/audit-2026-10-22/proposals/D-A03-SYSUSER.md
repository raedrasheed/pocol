# D-A03-SYSUSER — what the user-context SYS allowance counts

- **Approval status:** PROPOSED. Not approved.
- **Implementation status:** alternative (a) is implemented provisionally (`user_phase::BlockMeter`:
  `Halt::SysKeys`, `Halt::SysAccounts`, `Halt::SysWriteClosing`). It is vacuous for the v1 programs,
  which expose no USER-callable storage write.
- **Finding:** A03 (and the R18 facts of A02).

## Exact normative text
1. `development/m1/reference/execution.md` (موارد النظام, D50/D54/D70):
   > سياق المستخدم: 256 مفتاحًا و16 حسابًا، وصفر في الإغلاق، وإلا SysWriteClosing.

   "User context: 256 keys and 16 accounts, and zero during closing, otherwise SysWriteClosing."
2. `decisions.json` D70:
   > النقل القسري وإضافة tip إلى COINBASE ليسا كتابة نظامية.

   "A forced transfer and adding a tip to COINBASE are not a system write." Rationale: "counting
   them as writes would invalidate every transaction with a tip during closing."
3. `governance.md:14`:
   > SYS_KEYS_USER_MAX=256

## What is not specified
- What a "key" and an "account" of the user-context allowance are.
- Whether the ChunkFactory nonce, which a USER CREATE2 through the factory increments, is a
  system write.

## Alternatives
- **(a) recommended:**
  - Storage keys of the five system programs changed by USER transactions, ≤ SYS_KEYS_USER_MAX per
    block.
  - The programs whose storage they change, ≤ 16.
  - Both are 0 while closing.
  - Balance credits (the Escrow tip, D70) and the ChunkFactory nonce are not storage writes and do
    not count.
  - Each limit is an exceptional halt.
- **(b):** as (a), but an Escrow tip credit also counts as a system write. This contradicts D70:
  every tipping transaction during closing would halt.

## Proposed amendment (recorded here only)
English: "The user-context SYS allowance counts storage keys of the five system programs changed by
USER transactions (≤ SYS_KEYS_USER_MAX per block) and the programs whose storage they change (≤ 16),
and is zero while closing (SysWriteClosing). Balance changes (D70) and the ChunkFactory nonce are not
system writes."

Arabic:
> تعد ميزانية النظام في سياق المستخدم مفاتيح تخزين برامج النظام الخمسة التي تغيرها معاملات المستخدم (≤ SYS_KEYS_USER_MAX لكل كتلة) والبرامج التي تتغير مخازنها (≤ 16)، وهي صفر في الإغلاق (SysWriteClosing). تغيرات الأرصدة (D70) وnonce الـChunkFactory ليست كتابة نظامية.

## Minimal examples (`vectors/d-a03-sysuser.json`)

| scenario | (a) | (b) |
|---|---|---|
| S1: ordinary block, a tipping transaction | ok | ok |
| S2: closing, a tipping transaction | ok | **halt SysWriteClosing** |
| S3: closing, a USER write to program storage | halt SysWriteClosing | halt SysWriteClosing |
| S4: 257 USER writes to program storage | halt SysKeys | halt SysKeys |

## Effects
- **Validity:** none. Every response is a halt.
- **Receipts, gas, roots:** differ only in S2-like cases, and only under (b).
- **Compatibility:**
  - With the v1 programs, (a) never triggers, because no USER path writes program storage.
  - R18's user allowance (SYS_KEYS_USER_MAX keys, 16 accounts, charged to the ordinary phase)
    relies on it.

## Evidence
- Vectors: `vectors/d-a03-sysuser.json`.
- Unit tests in `user_phase` (`meter(...).check`): 257 keys → SysKeys; closing → SysWriteClosing; a
  17th account → SysAccounts.
- S1/S2 under (a): a tip is a balance change, which `effects` does not count as a system key. This
  is covered by every apply test with a tip and by `a_halt_reverts_state_except_nonce_and_fee`.
