# D-A03-CAPS — what SLOT_CAP and ACCT_CAP count; the SYS_SLOT_MAX check after the block

- **Approval status:** PROPOSED. Not approved.
- **Implementation status:** alternative 2 is implemented provisionally:
  - `genesis_builder::check_caps`, which counts the user state;
  - `GenesisImage::verify_candidate`, which checks SYS_SLOT_MAX after the system phases;
  - `user_phase::BlockMeter` (`Halt::SlotCap` / `Halt::AcctCap`).
- **Finding:** A03 and A02 (the audited SYS_SLOT_MAX = 1 profile).

## Exact normative text
1. `development/m1/reference/execution.md:82`:
   > SLOT_CAP، وACCT_CAP، … : توقف استثنائي.

   "SLOT_CAP, ACCT_CAP, …: exceptional halt."
2. `governance.md:13–14`:
   > السقوف [CP]: … وSLOT_CAP=4·10^6، وACCT_CAP=10^6. / SYS [CP]: SYS_SLOT_MAX=131072، وSYS_KEYS_BLOCK_MAX=2048، وSYS_KEYS_USER_MAX=256، …

   "Caps [CP]: … SLOT_CAP=4·10^6, ACCT_CAP=10^6." / "SYS [CP]: SYS_SLOT_MAX=131072,
   SYS_KEYS_BLOCK_MAX=2048, SYS_KEYS_USER_MAX=256, …"
3. `governance.md:97`:
   > R18: SYS، ويشمل كلفة RET سجلًا ضمن G_SYS.

   "R18: SYS, including the cost of RET records within G_SYS."
4. `validation.md:2076` (T18b L7/L8 criterion):
   > RecLen ≤ 4450 واستخدام SYS ≤ SYS_SLOT_MAX في كل كتلة.

   "RecLen ≤ 4450, and SYS usage ≤ SYS_SLOT_MAX in every block."

## What is not specified
- Whether SLOT_CAP/ACCT_CAP count the five system programs' accounts and storage.
- When SYS_SLOT_MAX is checked.

SYSTEM calls cannot halt. If system storage counted against SLOT_CAP, user transactions could fill
the cap, and the same block's system phase would then pass it, with no halt available to stop it.

## Alternatives
- **1:** SLOT_CAP/ACCT_CAP count every account and slot, the programs included. A user transaction
  halts at the cap. A system write that passes it makes the block invalid, so the user phase must
  leave headroom for the later system phase. That couples execution to the programs' internals.
- **2 (recommended):**
  - SLOT_CAP/ACCT_CAP count the user state: everything outside the five programs, checked as an
    exceptional halt of USER transactions.
  - SYS_SLOT_MAX counts the five programs' nonzero slots. It is checked after the block's system
    phases, and a block that passes it is invalid.
  - SYS_SLOT_MAX ≥ the derived worst case (R18) makes that check unreachable for an admitted
    profile.
  - This is the split R21's bound already uses (SLOT_CAP + SYS_SLOT_MAX).

## Proposed amendment (to execution.md §السقوف, recorded here only)
English:
- SLOT_CAP and ACCT_CAP bound the nonzero storage slots and existing accounts outside the five
  system programs. A USER transaction that would exceed either halts exceptionally.
- The system programs' nonzero storage slots are bounded by SYS_SLOT_MAX after each block's system
  phases. A block that exceeds it is invalid.
- The genesis must satisfy both bounds.

Arabic:
> يحد SLOT_CAP وACCT_CAP خانات التخزين غير الصفرية والحسابات الموجودة خارج برامج النظام الخمسة، ومعاملة المستخدم التي تتجاوز أحدهما تتوقف استثنائيًا. وتحد SYS_SLOT_MAX خانات برامج النظام غير الصفرية بعد المراحل النظامية لكل كتلة، والكتلة التي تتجاوزها باطلة. ويجب أن يحقق genesis الحدين.

## Minimal examples (`vectors/d-a03-caps.json`; SLOT_CAP 100, 30 user slots, 60 system slots, one USER transaction, then the system phases add 3 slots)

| scenario | 1 | 2 |
|---|---|---|
| C1: user writes 10 | user ok (30 + 60 + 10 = 100), then the system phase reaches 103: **block invalid** | ok, valid |
| C2: user writes 70 | **halt SlotCap** (160 > 100), valid | ok (30 + 70 = 100), valid |
| C3: user writes 71 | halt SlotCap, valid | halt SlotCap (101 > 100), valid |
| C4: system storage would pass SYS_SLOT_MAX | block invalid | block invalid |

Under alternative 1 the user state can never grow past SLOT_CAP minus the programs' storage, and
that headroom moves as the programs' storage grows. Building a valid block also needs the builder
to predict the system phase's writes (C1).

## Effects
- **Validity:**
  - Under 2, a block is invalid only if system storage passes SYS_SLOT_MAX, which admission makes
    unreachable (R18).
  - Under 1, a block becomes invalid whenever user transactions leave less headroom than the system
    phase needs.
- **Receipts, gas, state root:** differ where a user transaction halts under one alternative and not
  the other (C2: user slots between SLOT_CAP − system slots and SLOT_CAP).
- **Compatibility:**
  - Genesis acceptance changes for a genesis whose total slots exceed SLOT_CAP while its user slots
    do not. That is unreachable at the devnet caps and at the network values.
  - The preserved LP4 post-phase-2 atomicity test now uses SYS_SLOT_MAX, the cap that governs phase 2
    writes.

## Evidence
- Vectors: `vectors/d-a03-caps.json`.
- Replay of 2:
  - `apply::tests::a03::proposals::d_a03_caps_vectors` (C1–C3);
  - `system::issuance_tests::actual_phase2_post_storage_cap_rejects_entire_candidate` (C4).
- `apply::tests::a03::slot_and_account_caps_halt_the_transaction`.
- Admission: `audit_reproductions_are_rejected_with_their_rules` (SYS_SLOT_MAX = 1 → [R18]).
