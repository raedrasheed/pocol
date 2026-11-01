# LP3 acceptance matrix

The unchanged original requirements remain in LP3-MILESTONE.md. Approved amendment
01 is additive: no M1 source or prior evidence is rewritten. Final audit: [S9 acceptance report](lp3/s9-acceptance/codex-r1/ACCEPTANCE.md).
Acceptance is effective on the normal merge of the S9 documentation PR into foundation.
Status after the final S0–S8 evidence audit:

| Criterion | Scope/status | Evidence or remaining work |
|---|---|---|
| S0–S4 | PASS; previously merged and audited | Preserved reviews; current full supported Windows regression and Linux OS-limit checks; S4 merge c662bff |
| S5 R1–R26 evaluator | PASS under amendment 01; merged 18cf7ab | PR #15, codex-r2: 115 tests, 1450 oracle cases, 32 mutation controls |
| Original general R18 SYS derivation | DEFERRED LP4 — NOT PASSED | Amendment 01 tests 1, 4, 5 |
| Original general R21 trie bound derivation | DEFERRED LP4 — NOT PASSED | Amendment 01 tests 2, 4, 5 |
| Original caps/64 minimum SYS_SLOT_MAX | DEFERRED LP4 — NOT PASSED | Amendment 01 test 3 |
| Amended LP3 numeric R18/R21 fact evaluation | PASS evaluator on synthetic fixtures only | codex-r2 binding/version/coverage/provenance controls; boot disabled |
| S6 | PASS reference scope; merged 772cb1f | PR #16: strict type check, eight tests, independent Rust/TS/Python comparisons and mutation controls |
| S7 numerical gates | PASS scoped evidence; merged 5a04bdf | Actual AS1–AS19 traces, million full comparisons, ASQ/ASB and SV arithmetic; codex-r1 evidence |
| CF1–CF6 new core mapping | PASS newly approved definitions | LP3-CF-CORE-APPROVAL.md; codex-r2 Rust/TS/independent oracle 16000 cases and mutation controls; not recovered original definitions |
| T0 semantic core audit | PASS limited core scope | codex-r2 T0-MAPPING.md; strict M3 envelope/net/Rule3, actual PoW, maximum shape and CF6; real V20–V22 execution LP4, not full H acceptance |
| S8 | PASS; merged c054dad | PR #18: both final-head CI checks pass, automated review no findings; 145 Windows Rust tests, HTTP45/store24; 131 portable Rust/11 TS groups, independent oracles and 23 compiled mutation controls; platform blocks excluded |
| S9 | PASS final evidence/documentation audit; effective on S9 PR merge | 903 prior manifest entries; GitHub/source receipts, nine Linux CLI cases, negative acceptance controls and Arabic dashboard; explicit LP4 handoff |
| Production boot | DISABLED | LP3 evaluator success is not execution or boot acceptance |

LP3 may be accepted only against the explicitly amended scope once every remaining
required criterion is supported. Deferred original obligations stay visible at handoff.
