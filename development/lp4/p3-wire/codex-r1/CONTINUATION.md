# P3 R1 continuation — draft, NOT PASSED

Start from tested P2 remote c6e56b069ad3d5a7c1323cae81ea53004f542512,
whole tree d643038c4a6210587c14c364a041f161c14980b7.
Local branch codex-lp4-p3-wire. Accepted dependency graph allows P3 while P2 open.
Publish as stacked DRAFT targeting codex-lp4-p2-runtime for P3-only review;
NEVER merge into P2. Retarget foundation only after P2 accepted/merged, then require
all P3 acceptance and GitHub requirements. No duplicate PR24. CI now also triggers
on stacked base; prior P2 criteria/job strictness unchanged.

R18 final comparison and handoff are on evidence-only branch
codex-lp4-p2-evidence-r17 commit e7a4cbbbc677e55794e00623cedd6ba17c9fba0e:
all20661,19488 exact root/log matches+1173 expected rejection presence,
no excluded/blocked/incomplete; final artifact12255093116 digest
2d7316a037445fdb91e2430202db8ca595c0cae3966584f348558a5ecc2bf849.
PR24 remains draft/unmerged at C6; foundation c0ef36c68fb83f9474de7e70fe61973b466647a8.
R18 archives/logs/local all-case outputs are preserved with checked hashes there.

This branch implements signed legacy155/2930/1559 codec/sender validation,
London arithmetic, typed receipts/bloom/indexed roots with independent controls.
See REVIEW and results.json for verified scope. Next implement actual chain/type
pool/RPC and included-block rejection plus atomic user-body validation, wrong
roots/gas no commit, header-bound fees and remaining refund boundaries. Do not
substitute codec-to-error mapping for actual CI2/CI3 acceptance. Continue separate
review and evidence; inspect hosted checks rather than counting queued as PASS.
Then P4–P10 systems/GenesisBuilder/GB1/1024/durable rewards; real stored-parent
phases/settlement/SV6/finalization; all-profile R18/R21/caps64; persistence/crash
and final audit. All26 ledger obligations remain NOT PASSED, boot false.
Preserve M1/LP3/prior failed300/1200 comparisons and all approved owner decisions.
No repeat approvals, main/boot/deploy/billing/protection bypass/Claude/coordinator.
Use GitHub connector with whole-tree equality and expected-head lease.
