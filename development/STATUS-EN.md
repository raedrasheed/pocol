# PoCol development status — 7 October 2026

Actual local work was imported into `work/m1-import`: the baseline, draft 0.1, revisions 0.2–0.9, vectors, tools and independent reviews. These are preserved file snapshots, not earlier Git history. [Draft pull request #2](https://github.com/raedrasheed/Proof-of-Collaboration/pull/2).

**M1 is incomplete and not approved for implementation.** The current scope is specifications, vectors and reference models only. No merge, deployment or real transactions.

## Results executed locally by Codex

| Revision | Passed | Recorded values | Failures |
|---|---:|---:|---:|
| 0.2 | 212 | 0 | 0 |
| 0.3 | 333 | 2 | 0 |
| 0.4 | 216 | 1 | 1 |
| 0.5 | 347 | 3 | 1 |
| 0.6 | 188 | 1 | 1 |
| 0.7 | 249 | 2 | 1 |
| 0.8 | 261 | 3 | 0 |
| 0.9 | 197 | 38 | 0 |

Recorded values are not passing tests. Earlier failures are preserved. Revision 0.8 corrected the wrong profile reference in 0.7; Codex ran alternative E's comparison, its five cases and related checks without failures. This does not turn the earlier run into a success or approve all of M1.

All 611 hash samples matched across Rust sha3, @noble/hashes and Python pycryptodome. Hashes of 486 positive files, the txA signature and its recovered signer address were verified using an independent Rust library. The transaction was not sent. Python and JavaScript BR16 generators also matched.

Baseline provenance was verified from a preserved local record: the approved design ID was recomputed and all twelve sections matched the reference files. Historical test definitions were recovered from local sources without publishing private conversations.

## Remaining

Latest draft: [0.15](m1/m1-draft-0.15/README.md). Issue ledger: [issue-ledger.json](m1/coordination/issue-ledger.json). Codex reports: [reviews](m1/coordination/reviews/).

Current coverage: 36 partial items, 5 not started and none complete under the full acceptance gate. Remaining work includes StoreQueue, recovery, storage and some network/log vectors. Model results do not establish browser, node or EVM operation or production security.

Claude authors and edits; Codex reviews and verifies independently in sequential turns. A sanitized summary is published after each completed cycle. A shared GitHub account does not constitute independent GitHub approval.

## Raed's decisions

Open questions are documented in [owner decisions](decisions/M1-OWNER-DECISIONS-EN.md). No answer has been received and none was treated as automatically approved.

## Public-copy privacy

Private sessions/conversations, explicit keys and recovery phrase, dependencies, binaries and caches were excluded. Test-key generation requires `POCOL_TEST_MNEMONIC` locally; its value is not stored in the repository. Public evidence is sanitized, and the publication register distinguishes local-source hashes from public-copy hashes. There is no artificially passing CI; these results come from documented local execution.

## Cycle 0.9 from the local interface

Raed submitted an M1-resumption instruction through the PoCol interface. Claude authored StoreQueue rules/vectors in a new draft; Codex independently reviewed and ran the batch: 197 passing checks, 38 recorded values, zero failures and 31 passing independent probes. The actual author result and coordinator review appeared in the same conversation. An independent Node UTF-8 source and hashes of 139 earlier files remained matching.

Q1–Q4 moved to partial; E1–E7 and V2–V4 have not started. Actual Chrome, TypeScript and MV3 measurements are not performed; the full gate and owner decisions remain. Next: E1–E3 without an owner decision, then remaining recovery/vectors. No merge, deployment or transactions. [Review](m1/coordination/reviews/REVIEW-0.9.md).

## Cycle 0.10 needs repair

Claude authored E1–E3 generation-fence/recovery models. Codex ran the runner in an independent review copy: 86 passed, 27 recorded values, zero failures. However, 24 further independent probes exposed 7 failures rejecting invalid names and booleans substituted for numbers. Codex requested C24 repair; the reasoned rejection appeared in the interface. The last accepted draft remains 0.9; no M1 acceptance or new owner decisions. [Review/evidence](m1/coordination/reviews/REVIEW-0.10.md).

## Cycle 0.11: C24 repaired, C25 discovered

Revision 0.11 passed earlier/new checks: 211 passed, 48 recorded, zero failures; original independent probes passed 24/24. Codex nevertheless found nonconservative namespace accounting: eight actual entries become nine without warning because invalid names were excluded from the count. C25 repair was requested in a new revision. Revision 0.11 and 0.10 failures remain preserved; the last accepted batch is 0.9. No new owner decision or production approval. [Review](m1/coordination/reviews/REVIEW-0.11.md).

## Cycle 0.12: fence repair verified

Revision 0.12 passed new/inherited checks: 237 passed, 64 recorded, zero failures; 17 independent accounting probes and 24 name/type probes. C24/C25 closed at specification-model level. Sources 0.10/0.11 and failing evidence are preserved. Coverage: 34 partial, 7 not started, zero complete. Next: E4–E7 and V2–V4, with no new owner decision. Actual measurements, owner decisions and full gate remain. [Review](m1/coordination/reviews/REVIEW-0.12.md).

## Cycle 0.13 needs a runner repair

309 passed, 99 recorded and one failure: a diagnostic-field name collision stopped remaining codec checks. Thirty independent probes passed. Codex requested a limited correction in a new revision before acceptance. Last accepted batch: 0.12; CR-E4-01/02 proposals for number/invalid-string representation remain unapproved. [Review](m1/coordination/reviews/REVIEW-0.13.md).

## Cycle 0.14: coverage guard exposed missing checks

347 passed, 113 recorded, 3 failures: a name-field collision stopped remaining E4 checks; the coverage guard identified nine unexecuted checks. Codex requested repairing the recording layer without dropping checks. Last accepted batch: 0.12; failures preserved. [Review](m1/coordination/reviews/REVIEW-0.14.md).

## Cycle 0.15: E4–E5 coverage verified within model assumptions

365 passed, 118 recorded, zero failures. C26 was repaired and all nine previously blocked checks ran; failing 0.13/0.14 evidence remains. CR-E4-01/02 representation remains proposed, not adopted. Coverage: 36 partial, 5 not started, zero complete. Next: E6–E7, then V2–V4. [Review](m1/coordination/reviews/REVIEW-0.15.md).

## M1 cycle — Draft 0.16

944 passes, 140 documented items and two failures in the defective deletion-control table. The second attempt at 11000 must be completed. Review requested changes; last accepted batch 0.15. No merge or deployment. [Review](m1/coordination/reviews/REVIEW-0.16.md).

## M1 cycle — Draft 0.17

1037 passes, 156 documented items and one failure. Corrected the deletion table and exposed a defect retaining past snapshots by a subsequently mutated reference. Twenty-one independent site/storage/reservation-limit cases passed. Last accepted batch 0.15. No merge or deployment. [Review](m1/coordination/reviews/REVIEW-0.17.md).

## M1 cycle — Draft 0.18

1060 passes, 168 documented items, zero failures. Four independent snapshot-preservation tests passed. E6–E7 accepted for specifications/vectors only: 38 partial, 3 not started (V2–V4), zero complete. The x12b conflict and owner decisions remain unapproved. Continue with LogClient. No merge or deployment. [Review](m1/coordination/reviews/REVIEW-0.18.md).

## M1 cycle — Draft 0.19

493 passes, 23 documented items and zero failures in original cases; 29 independent cases passed. An independent control exposed acceptance of an unknown event as an attempt terminator, contrary to S-a; C29 repair requested before LogClient acceptance. Last accepted batch 0.18. No owner-decision adoption, merge or deployment.

## M1 cycle — Draft 0.20

570 passes, 76 documented items, zero failures; 23 independent S-a checker and ambiguous-field cases passed. C29 and LogClient batch accepted for specifications/vectors only: 39 partial, 2 not started (V3–V4). Experiment-definition gaps/proposals are explicit; owner decisions unchanged. No merge or deployment.

## M1 cycle — Draft 0.21

110 passing checks, 29 documented, zero failures from the correct source path. The initial run's path-verification failure was preserved. An independent 4291-byte vector crashed the parser on deeply nested RLP; C30 repair requested before network acceptance. Three independent Keccak libraries agreed on K1–K3 and GSV1 hash. Last accepted batch 0.20; no owner-decision adoption, merge or deployment.

## M1 cycle — Draft 0.22

196 passes, 52 documented, zero failures; 23 independent cases passed for nesting to 5000 levels and truncated frames. C30 repaired; network batch accepted for specifications/vectors only: 40 partial, 1 not started (V4), zero complete. Next: X8 vectors, then specification-completeness audit. Owner decisions unchanged; no merge or deployment.

## M1 cycle — Draft 0.23

104 passes, 29 documented, zero failures in original cases; eight scripts passed syntax checks without execution. An independent probe found that a duplicate window erased recorded access and returned PASS; C31 repair requested before X8 acceptance. Control sensitivity must also be specified for each of five C7 paths. No actual Chrome/network results; owner decisions unchanged. Last accepted batch 0.22; no merge or deployment.

## M1 cycle — Draft 0.24

196 passes, 49 documented, zero failures; 45 independent cases passed for access-evidence preservation and ambiguity rejection. C31 repaired; five separate C7 pages and per-path sensitivity/cleanup plans added. No actual Chrome/network measurements. Coverage: 41 partial, zero not started, zero complete; this does not establish M1 completion. Next: item audit, original two reviews, hashes and owner decisions. No merge or deployment.

## M1 cycle — Draft 0.25

Full audit: 617 passed, 85 documented, zero failures. The generated-file path failure in the first review copy was preserved, then checks ran from source without dropping conditions. Three libraries agreed on 56 new hash inputs; 10 independent epoch-representation cases and 11 JavaScript cases passed; maximum record 1442009≤1442048. No string-acceptance change is needed: the baseline already rejects lone surrogates. Qualified technical judgments recorded without choosing owner policies. V1 authoring remained; manifest chunking into 1–3 chunks (U08) was referred as an owner-controlled change. No M1 completion, production implementation, merge or deployment.

## M1 review — Revision 0.26

Resumed the completed author task without a duplicate job. The prepared suite passed 302 checks with 75 recorded entries and zero failures. Independent verification matched 1027 ASERT cases, 347 hash preimages and 135 signatures. Review verdict: revise. Three malformed busy replies crash the reference checker (C32). A valid 256-share header requires 258 SHA-256 evaluations, conflicting with the source bound of 13; the proposed 27-call explanation also omits share hashes (V1-SHA-COST). The dashboard and evidence ledger record both blockers. Named owner decisions remain pending; M1 is not accepted. No production implementation, merge or deployment.

## M1 review — Revision 0.27

C32 crashes are repaired: 26 independent malformed cases pass. Prepared suite: 248 passed, 72 recorded, zero failures. Independent SHA/retry observations 18, hashes 3677, signatures 31 and ASERT 1027 all pass. Full-window SHA accounting is 3355 cached calls/unique inputs and 6721 uncached calls. The source bound remains 13 and requires owner authorization to amend. Review requests correction of the proposed rate-delay cap and the claim that 6000 ms retry sleep proves a 10 s overall load deadline. U14 and other owner decisions remain open. No merge or deployment.

## M1 review — Revision 0.28

Qualified specification corrections accepted. 33 prepared checks and 21 independent timing branches pass; 68 saved files from 0.26/0.27 are unchanged. Rate-cap policy is explicitly unapproved, compatibility formulas conditional, and 6000 ms means retry sleep only. Source SHA bound remains 13 despite verified 3355 distinct full-window evaluations. Recommend owner approval of explicit 3355 category bounds, rate cap 0..2000 ms, and a whole-load 10 s deadline through the verdict including computation. None is adopted; earlier owner decisions also remain pending. Current repair/review scope is complete, full M1 is owner-blocked. No merge or deployment.

## Owner decision guide after 0.28

Nine pending choices were originally explained in Arabic in the live dashboard and bilingual saved guides. No owner answer was recorded. The current Codex whole-load-through-verdict recommendation is distinguished from the older Claude per-request recommendation. U14 must explicitly define covered operations and reconcile proof-read caps. Independent audit confirms 139 original protected files unchanged, all nine decision IDs represented, and no further author repair identified in the reviewed scope. Named owner blockers remain.

## M1 cycle — Draft 0.29 and technical delegation

Nine technical choices were applied in a new draft under the user's standing delegation, not as personal approvals. There were 609 passing checks, 119 recorded items and 4 failures, all caused by a wrong sum condition in negative case N05; the intended rejection is correct. 1120 independent timeout checks passed; three libraries agreed on 668 newly computed hashes. Three independent cases exposed display allowed with a wrong/missing response ID or a duplicate result key. C33/C34 correction requested in a new revision, with 17 documented technical conventions, including one budget for header checking and content loading during RP navigation. The 41-row gate remains incomplete; evidence/failures preserved, next follow-up automatically authorized. No production deployment, transactions or merge.

## M1 cycle — Draft 0.30

Original C33/C34 repaired: 23 independent response-guard/bound checks passed; N05 and its mutations are correct. Prepared tests: 308 passes, 105 documented, 6 failures in textual review/provenance links and dependent checks; these will be corrected with actual machine-readable evidence. C35 appeared: error data containing lone surrogates was counted as 3347 bytes while actual JSON is 6647, so it was accepted rather than dropped, causing waiting and display. The defect remained after correcting UTF-8 loading in the first comparison and preserving it. Freeze and 668+4024 hash calculations passed; M1 gate incomplete. Next draft uses a matching native JSON oracle and repairs evidence links, then improves automatic follow-up. No personal approvals, production, deployment or merge.

## M1 cycle — Draft 0.31

C35/C36 repaired: a Node oracle measures actual JSON encoding and corrects review/provenance links. Twenty-four independent checks passed; 6647-byte data is dropped without waiting or display. Prepared results: 280 passes, 102 documented, 1 failure in historical link HF-12: the checker took a snapshot before recording passing HF-8. Next correction C37 links completed states and tests against closure on missing/failed evidence; no protocol change or hash reconstruction. Status: 38 candidates, 3 awaiting final review, M1 not yet complete. Earlier evidence preserved; continuation automatic under authorization.

## M1 specification-gate acceptance — Draft 0.32

Codex accepted the specification gate after auditing 41 rows and five criteria per row. 45 passing checks, 68 documented items, zero failures; 8 independent history checks and 11 passing sensitivity mutations. Twenty-five earlier run failures and 9 evidence files containing failures are linked to actual repairs; history was not deleted. 139 original files and 42 recent sources preserved. F01–F26 and C37 closed only within specification/evidence scope. The 9 choices and 17 conventions are delegated technical decisions, not personal approvals. No Chrome/node/EVM results, production, deployment or transactions. Next authorized work: improve coordinator follow-up after review and test it while preserving state/controls.

## Coordinator improvement complete — Draft 0.33

Verified local update: 82 passing tests and 18 independent HTTP checks. After reviewing the actual Claude receipt, exactly one follow-up request was created and delivered to the existing thread; Codex received and acknowledged it for publication/final verification, with no additional author or loop. Link, port, reviewer authority, thread, 38 requests, 27 reviews and owner answers preserved. The coordinator applied eight changed files from the matching Claude bundle after tests, with rollback copies; private settings/state were not published.

The current worklist is complete and empty; automatic follow-up was verified stopped due to worklistComplete, while authorization remains for future authorized tasks. M1 specifications accepted at 0.32 for all 41 rows; coordinator improvement 0.33 verified live. No personal approvals, production, transactions or merge. Code is in PR #4 and evidence here in PR #2.
