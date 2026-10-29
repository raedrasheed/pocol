# LP4-LEDGER-01 — consolidated owner decision, NOT APPROVED

No earlier approval is reopened. This proposes a new explicit physical Ledger/
Registry interface and retention contract, without editing M1 or weakening any
LP4 obligation. Neither option is installed in the node or accepted as a gate.
All27 complete gates remain NOT PASSED; boot=false.

## What the accepted sources fix, and what they do not

M1 execution.md:38,63–66 specifies ShareLedger, pruneShares before recordShares,
count[j][owner] from stored ParentResult plus the implicit winner, and lastShare=j.
Consensus.md:121 specifies the assignment window[h−1−W_w,h−2]. Incentives.md:71–89
requires settlement atj+D and all remaining pots atF. They do not give Ledger
slot numbers, physical pruning height, recording ABI/call placement or treatment
of an owner whose Registry record is absent when its parent shares are recorded.
The four-word Registry layout and SYS-01 mandatory message semantics are already
approved and unchanged. Source hashes/search scope are in SOURCES.json.

This is consensus-observable. The compiled countermodel uses the SAME code and
parent state at h5,D2,W_w3. Counts1/2/3 are7/11/13. Assignment for h5 has already
read[1,3]; settlement needs3 and next assignment needs[2,4]. Deleting count1 now
or retaining it for one extra height both preserve those needed values but give
different actual EVM state roots and storage/gas usage. See diagnostic/results.json.
The mode flag exists ONLY in that synthetic diagnostic, never in the proposed
executor/contract API. The example is not a full Ledger or an acceptance PASS.

## Option1 — recommended complete additive contract

1. **Physical v1 layout.** At C0C003: slot0 is mapping(uint64=>uint256)total;
   slot1 is mapping(uint64=>mapping(address=>uint256))count; slot2 is
   mapping(uint64=>address[])owners. Standard32-byte Solidity mapping/array
   addressing applies, not packed8-byte height preimages. Let T_j=keccak(be256(j)
   ||be256(0)); N_j=keccak(be256(j)||be256(1)); count[j][a] is at
   keccak(pad32(a)||be256(N_j)). L_j=keccak(be256(j)||be256(2)) stores array length;
   its entries start atkeccak(be256(L_j))+i, low160-bit addresses with zero padding.
   total>0 means recorded; total0/empty list means absent. One count per distinct
   owner and a first-occurrence-ordered distinct list; no counters/log slots beyond
   these. These are NEW LP4 definitions, not recovered original M1 layout.
2. **Mandatory record entry point.** The existing recordShares phase calls
   Registry.recordShares(uint64,address[]) as ONE SYSTEM message, CALLER=SYSTEM,
   preserving the prescribed phase position and SYS-01 semantics. Registry makes
   an ordinary nested CALL to Ledger.record(uint64,address[]) which accepts
   CALLER=Registry only. Ledger.pruneShares() accepts SYSTEM only; views are public.
   No additional SYSTEM call, ORIGIN authorization, delegatecall or generic write
   endpoint is introduced. Canonical recording calldata is selector, be256(j),
   offset64, be256(N), then N zero-padded addresses: exactly100+32N bytes. Reject
   noncanonical offsets/padding/trailing data, zero addresses, NUMBER outsideu64,
   h<2, j!=h−1, N<1 or N>S_max+1. Internal Ledger.record enforces the same shape,
   j/length bounds and record-once condition, deriving counts from the addresses,
   not caller-supplied totals/success flags.
3. **Parent-only content and lastShare.** The executor derives the address
   sequence from stored validated ParentResult(j): committed shares in their
   canonical nonce order, followed by the implicit winner owner once. Repeated
   addresses count repeatedly; total=N=1+shareList.length; each distinct owner's
   count is its multiplicity. Current winning nonce/owners/shares/signatures never
   enter this API. Stored parent/grandparent rederivation and mismatch rejection
   remain mandatory. Registry updates lastShare=j once per existing distinct
   record, preserving every other packed field/bond/array and not consuming
   USER REG_OPS. Active/exited/tomb/PAID status never suppresses Ledger counts.
   For an actually absent all-zero Registry record, retain its count but do NOT
   create a record or write lastShare. A partially nonzero/malformed missing record
   or lastShare>j rejects the SYSTEM call; no silent overwrite/max/phantom record.
   This absent-record treatment is a NEW explicit LP4 definition. It must be tested
   independently of assumptions about ordinary unbond delays.
4. **Exact pruning boundary.** Let K=max(CP.D,CP.W_w), from the complete validated
   CP. pruneShares() at h removes ONLY j=h−K−1 when h>K+1; otherwise it is a no-op.
   No host/caller policy flag or extra-retention choice exists. For a positive j,
   require a recorded, internally consistent list/count/total; reject corrupt or
   unexpectedly missing facts. Delete all j counts, all owners array element
   slots, its length and total, with no orphan keys or retained marker. The list
   is distinct, nonzero and bounded byS_max+1; all positive counts sum to total.
   Current h assignment was derived from the parent before apply; current settle
   j=h−D and the next assignment's oldest count h−W_w remain available. Use checked
   arithmetic, including equality/underflow/u64 boundaries. At F the same prune
   rule applies; finalize does not introduce a separate Ledger cleanup policy.
5. **Genesis and replay.** Ledger initially has no recorded heights/counts/lists;
   reject nonzero alloc storage at this reserved Ledger address rather than
   admitting caller-prepopulated shares. Rebuild actual code, state MPT and complete
   sysCodeHash/profile identity; do not fabricate Registry records, reward-account
   nonces or reward accounts.
   GenesisBuilder, sync, reorg and TrieRebuild reproduce the same storage layout
   and deletion boundary. Existing Registry/ChainCaps/bond/PAY/CODE rules remain
   unchanged. Duplicate/out-of-order recording, wrong phase/context and imported
   corrupt facts reject the entire private candidate, not just one row.
6. **Settlement and assignment consumers.** Existing M1 formulas are unchanged:
   c_i, C_j and all winner/proposer/penalty/pay/round/base/invalid/final burn rules
   remain mandatory. Escrow reads the actual Ledger counts/list/total, requiring
   complete positive recorded data when settlement needs it. No absence-as-zero
   shortcut, synthetic success fixture or recomputed caller total can replace
   actual facts. Assignment uses the existing parent-state window; settle/finalize
   use their required real heights before any permitted deletion. This approval
   would not decide any OTHER presently missing settlement/evidence semantics.
7. **No resource waiver.** Every original cap and the approved10/2/10 Registry
   ceilings stay unchanged. A recorded height uses at most2k+2 nonzero Ledger
   slots for k distinct owners; after a complete normal phase at mostK heights
   are retained (early chain truncated). This is a layout count, not a full R18/
   R21 proof. Include actual prune/record/nested Registry calls, packed lastShare
   writes, EIP150/SSTORE sentry/raw-before-refund gas, all dirty/account/code/log/
   storage caps and physical host/trie/history/snapshot costs in independent
   general derivations bound to full CP/profile/code/model identity. Default,
   end/full and caps64 minimum remain mandatory. If a required cap contradicts
   actual execution, stop for an explicit owner decision; do not raise a cap,
   exclude the required profile/case or count a blocked check as passing.
8. **Acceptance remains separate.** Preserve M1/LP3/all prior evidence and every
   carried obligation. Initial five programs emit no LOGs. All Ledger/Registry/
   USER/pending/issuance/settlement/resource changes remain one block candidate;
   any error/cap/late commitment/durable write failure rolls everything back.
   Independent actual execution, general resources, real finalization/SV6,
   crash/reorg and GitHub requirements must pass before merging P4 or later
   dependent increments. Owner approval is not acceptance; boot remains disabled.

## Option2 — explicit alternative, not selected

Retain one extra count height: prune j=h−K−2 when h>K+2; at h<=K+2 do nothing. Keep the other contract clauses explicit. This produces different roots
and a larger physical retained-slot bound and would require its OWN fully checked
resource/genesis profile bindings and acceptance evidence. It cannot be treated
as equivalent to option1 or accepted under its expected roots. Neither option may
be silently selected merely because existing logical count vectors still pass.

## Mandatory acceptance after any approval (all currently NOT PASSED)

These are newly proposed LP4-LEDGER-01 cases, not recovered M1 test labels:

- Actual compiled full-five4/1024 GenesisBuilder and independent allocation/root/
  code/storage comparison; reject nonzero Ledger alloc and incomplete/mismatched
  identities. Keep GB1/GB-END and all original criteria intact.
- Real validated stored-parent/grandparent execution, h1 skip, single implicit
  winner, repeated owners, zero/maxS_max and distinct-owner boundaries; same
  ParentResult gives identical storage independent of valid signature envelopes.
- CALLER-only authority at both targets; actual signed USER calls and misleading
  ORIGIN cannot record/prune/update lastShare. Canonical ABI/height/phase/replay
  negatives and nested failure rollback; no added SYS messages/logs/USER receipts.
- Existing active/exited/tomb/PAID records preserve other fields; absent all-zero
  records do not become ghosts, malformed records/future lastShare reject; counts
  remain available for all existing SV values including missing-recipient SV5.
- D<W_w, D=W_w and D>W_w; h=K+1/K+2, equality/minus1/plus1, zero/max arithmetic,
  real settle and next-assignment window reads; repeated and final-phase pruning.
  Compare actual entire state roots/storage against an independent implementation.
- Full delete coverage, duplicate/missing-owner/total/count corruption, no orphan
  slots; independently measured key/account/gas/cap counters and general R18/R21,
  supported default/end/full and caps64 min/min−1 with complete identity bindings.
- Successfully compiled mutations for wrong prune equality/horizon, retained or
  deleted-needed counts, extra-retention substitution, wrong slot/padding/list,
  missing implicit winner, duplicated recording, USER/ORIGIN privilege, skipped
  lastShare, phantom records, silent corruption, omitted resource work, rollback
  and persistence. A build failure is not a killed mutation.
- Actual finalization/SV6, pending[F] absence/zero final subsidy, original SV/CL/CF
  and named T gates, durable fault/crash/reorg and supported-platform regressions.
  No synthetic/core component test substitutes for these execution obligations.
