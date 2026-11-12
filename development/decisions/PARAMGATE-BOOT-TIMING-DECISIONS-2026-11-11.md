# ParamGate boot timing: owner decisions, 2026-11-11

Scope: node-local boot admission (ParamGate R14/R19 calibration in `lp4-execution/src/admission.rs` and
`pocold::load_spec_for`). These decisions change no block validity rule, state root, fork choice, wire format or
consensus identity.

## Source

The two questions left open by PR #11 were put to the owner in Arabic on 2026-11-11 at 09:43 UTC, together with a
recommendation (09:45 UTC):

1. «هل تُطبع الأزمنة المقاسة في رسالة رفض بدء تشغيل العقدة؟»
   (Should the measured times be printed in the node's boot-refusal message?)
2. «هل يكفي بطء كتابة واحدة على القرص لرفض تشغيل عقدة؟»
   (Is one slow disk write enough to refuse a node's boot?)

The owner's answer, verbatim (2026-11-11, 09:46 UTC):

> اعتمد نعم للسؤال الأول و لا للسؤال الثاني

(Translation: "Adopt yes for the first question and no for the second.")

## Decisions

- **PG-1 (yes): report the measured times.** A R14/R19 refusal states the measured execution time, the store
  (fsync) latency, the sample count and the t_exec limit. Diagnostics only; no verdict changes.
- **PG-2 (no): one slow disk write must not refuse a boot.** R14/R19 and t_exec = 1 s stay unchanged; only the
  measurement becomes robust (several samples per block and a robust statistic, one re-measurement before a
  refusal). A host that is really too slow is still refused.

## Implementation

- PR #12 (merged 2026-11-11) implemented both, but by removing the durable commit from the R14/R19 figure
  altogether, so a host whose disk is slow on every write was admitted. The 2026-11-12 audit (findings pr12-1, T3,
  INV-3, pr12-3) found that this broke "a host that is really too slow is still refused".
- Branch `admission-commit-latency` (2026-11-12) keeps PR #12's per-block execution median and its one retry, and
  counts each block's durable commit in R14/R19 again. A block's cost is its median execution plus its commit. The
  calibration's single largest commit is counted at the median commit (the lower median for an even count). With at
  least two timed blocks one stalled write therefore cannot refuse a boot, but a disk whose typical commit exceeds
  t_exec is refused (generalized from one commit to max(1, n/4) commits by the 2026-11-12 note below). A profile
  with a single timed block (`H_END = 1`) counts that block's commit; there only the one retry tolerates a stall.
  The measured times (`phase_ms`, `execution_ms`, `store_fsync_ms`, `store_fsync_median_ms`, samples, `t_exec_ms`)
  are reported in the refusal and, on a successful boot, in one stderr line of `pocold run`. See A13-DERIVATION.md §3.
- The implementation text approved with PG-2 also allowed timing store latency apart from execution, with its own
  wider bound if needed. That option is not used: a store-mode node commits every imported block on its consensus
  thread, so the commit stays inside the same per-block t_exec budget, as it was before PR #12.

## Implementation note, 2026-11-12: isolated stalls, not a single one

- Evidence. On the Windows CI runner the pocold test
  `isolated_commit_stalls_are_admitted_but_sustained_execution_slowdown_is_refused` was refused:
  `-32018 ParamGate rejected [R14, R19]; timing: phase_ms=[2256, 2256, 2256], execution_ms=[14, 14, 14],
  store_fsync_ms=4169, store_fsync_median_ms=439, samples=66, t_exec_ms=1000`. The calibration timed 22 blocks and
  three of the 22 commits were slow: the one the test holds and the runner's natural multi-second fsync spikes. The
  rule above replaced only the largest commit, so the next one (2256 − 14 = 2,242 ms) reached R14/R19. A real
  Windows node with the same disk behaviour would be refused at boot, which PG-2 excludes ("one slow disk write must
  not refuse a boot").
- Change (branch `admission-commit-outliers`, node-local, `admission::block_costs`). "Single" becomes "isolated":
  with n timed blocks, the largest k = max(1, ⌊n/4⌋) commits are counted at the lower median commit. One to seven
  timed blocks keep exactly the rule above (k = 1); 22 blocks tolerate five stalled writes. A disk whose typical
  (lower-median) commit exceeds t_exec, or that is slow on more than k of its writes, is still refused, so "a host
  that is really too slow is still refused" holds. A single timed block (`H_END = 1`) still counts its commit.
- Unchanged: R14/R19, t_exec = 1 s, the per-block execution median, the one re-measurement before a refusal, the
  reported times, and every block validity rule, state root, fork choice, wire format and consensus identity. The
  owner's text above is unchanged. See A13-DERIVATION.md §3.
