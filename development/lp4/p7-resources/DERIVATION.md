# LP4 P7/P8 — resource derivation for the v1 system programs

Code: `prototypes/lp4-execution/src/resources.rs` (formulas), `src/apply.rs`
(CapsTracker meter and enforcement), tests in `src/resources/tests.rs`.
Engineering derivation bound to the v1 layout (PoColSystemV1.sol, LEDGER-01
option 1); it replaces the M1-era KN/KC/KF/AN/AC/AF figures, which were
computed for a different (unbuilt) layout and are therefore not reused.

## Enforcement (every block)
- Per SYSTEM call: gas <= min(30,000,000, G_SYS − used); revert ⇒ block invalid.
- Per block: Σ system gas <= G_SYS; distinct storage keys changed by SYSTEM
  calls <= SYS_KEYS_BLOCK_MAX; accounts touched by SYSTEM calls <=
  SYS_ACCT_BLOCK_MAX. Violation ⇒ block invalid (ApplyError::Budget).
- Programs emit no LOGs (a LOG ⇒ block invalid).

## Formulas (S=S_max, O=min(S+1,M_max), K=max(D,W_w), E=E_max, SW=min(SWEEP_MAX,16))
- SYS_SLOT_MAX ≥ slot_min = 5+5·RRM+M_max + K·(2·O+2) + 2+3·(D+1) + 52.
- Keys: common = 9E + (2O+2) + (3O+2) + 1 + 4; normal = common+1+(2+8·SW+1)+2;
  closing = common+(RET+2)+1+2; final = closing + 4·(D−1).
- Accounts: normal 8+O+1+SW; closing 8+O+1+RET; final 8+min(D·(O+1), M+R_max·D+1)+RET.

## Results (network-like CP: S=256, M=256, RRM=8192, D=16, W_w=64, E=4, RET=128)
- Derived: slot_min 74,222; keys 1459/1458/1518; accounts 281/393/425
  (all within SYS_KEYS_BLOCK_MAX=2048, SYS_ACCT_BLOCK_MAX=512).
- caps64 minimum: SYS_SLOT_MAX=slot_min passes R18, slot_min−1 fails R18;
  keys/accounts caps one below the derived maximum fail R18.
- Measured worst case (256 identities, 256 distinct owners at every height,
  4 evidence punishments, closing over 256 records with RET=128, final block):
  keys max 793/1292/1335, accounts max 260/256/256, max single call
  13,215,012 gas (recordShares), max block SYSTEM gas 84,223,420 ≤ G_SYS 96,000,000.

## Decisions taken under the standing authorization
- **LP4-FINAL-01 (approved, recommended):** finalize(tips) burns tips and marks
  F; each remaining pot is settled by its own SYSTEM call settleFinal(j). A
  single finalize call exceeded the 30M per-call cap in the worst case.
- **LP4-GAS-01 (approved, recommended):** settlement reads Ledger entries and
  payee reward addresses in one call each (entriesOf, payInfos). Formulas
  and burn classes are unchanged; this brought the worst final block from
  > 96M to 84.2M.

## R21 (historical state retention)
pocold keeps full state snapshots for heights > head − K_hist (default 32) plus
genesis; older states are dropped and rebuilt on demand (TrieRebuild with
stateRoot checks). Retained bytes ≤ (K_hist + side) · bytes(state), with
bytes(state) ≤ accounts·200 + slots·96 for the in-memory model. RPC state
queries outside the window return −32017. Structural trie sharing is not
implemented; the bound is therefore per full snapshot.
