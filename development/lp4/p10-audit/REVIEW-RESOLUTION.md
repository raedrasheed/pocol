# LP4/LP5 P10 independent review — findings and resolution

Reviewer: separate Claude reviewer agent (same model family, independent context;
not a human or GitHub approval). Scope: PoColSystemV1.sol, apply.rs, pocold
chain.rs/node.rs/store.rs against consensus.md, execution.md, incentives.md.
Result: 0 blocking, 3 major, 5 minor. All resolved below; tests re-run.

| # | Finding | Resolution | Evidence |
|---|---|---|---|
| 1 major | Torn-journal recovery not crash-atomic (crash between renames lost the journal) | Copy original to torn-n (fsync), write prefix to tmp (fsync), atomic rename over the journal, fsync dir; refuse start when the journal is missing but torn-* exists | store.rs; tests/store.rs `missing_journal_with_recovery_artifacts_refuses_to_start` + every-offset torn test |
| 2 major | Honest nodes never produced evidence | Node records signed templates per (parent,h) and winner signatures per (TemplateID,nonce); a second distinct template ⇒ type-1 evidence + sticky FB; a second share root ⇒ type-2; evidence pool included (≤E_max, re-validated) in signed templates | node.rs; test `node_detects_template_equivocation_and_produces_valid_evidence` |
| 3 major | Evidence validity stricter/looser than literal text | **LP4-EVID-01 (approved, recommended):** type 1 = two different signed UTs at a=0, same (parentHash,h), cfg identity, UT.proposer = recovered signer for both, h ≤ h_incl and h ≥ h_incl−E_win, signer = DrawIndex(h) on the carrying branch (no requirement that parentHash is on the branch); type 2 = owner(nonce) under the assignment of the ancestor block with that TemplateID (any nonce < nMax, stored ranges), roots differ, both signatures by that owner | chain.rs evidence_type1/2 |
| 4a minor | a≠0 with proposer present classified envReject | proposer/a are committed ⇒ blockReject; only sig presence/recovery is envReject | test `template_form_classes_follow_d129` |
| 4b minor | Fallback item 13 signature checked before item 12 | Item 12 (body + Build_fb byte equality) evaluated before item 13 | chain.rs validate |
| 4c minor | E_max/EVID_BYTES_MAX and roots after evidence walk | Body bounds checked before evidence; item-15 roots computed directly before item 17 | chain.rs validate |
| 5 minor | T_j boundary (arrival exactly at T_j must lock) | tick uses `now > T_j`; template handler compares arrival ≤ T_j | node.rs |
| 6 minor | TrieRebuild did not re-derive ParentResult | Re-derived from header + rebuilt assignment; mismatch ⇒ parentResultMismatch | chain.rs ensure_state |
| 7 minor | Replay kept all states | retain() every 64 records and after replay | node.rs open |
| 8 minor | Production stuck at a=0 when Build_fb fails | Mode set to Idle so attempt advance proceeds | node.rs lock_fallback |

Items the reviewer verified as correct (settlement and burn classes, CP indices,
final-block order, phase order, Ledger prune boundary, recordShares encoding,
ParentResult D142, Build_fb, H-pre/H-full checks, D133, determinism, journal
fsync ordering) are listed in the review transcript summary above.
