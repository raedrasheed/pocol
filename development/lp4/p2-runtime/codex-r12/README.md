# Independent engine and rejection-input evidence — P2 NOT PASSED

Primary runtime remains c70961187c23d3fd4bab6a99a43fd12001f7518e; run39064768819
verified at b76729bfd91b253f96346cb5f6fe56b537ef24e0 is preserved in R11. No native
runtime, dependency, resource ceiling, M1 or LP3 evidence is changed here.

The frozen unmodified py-evm0.12.1b1 runner visited all20661 Cancun cases:
19443 state/log MATCH,85 MISMATCH,1100 EXCEPTION_PRESENCE_ONLY,33 BLOCKED_TIMEOUT.
It used4 workers and10s/case. Every receipt is retained locally, including failures; none of
the latter three categories is a verified semantic PASS. Of85 differences,12 are
state-root disagreements and73 are expected rejections accepted by this engine/unsigned
adapter. The latter include max-fee affordability/value, blob-count/block admission,
initcode/intrinsic gas and sender-code checks. This is not actual signed-wire acceptance.

An explicit independent comparator profile adds approved CREATE-01 using complete
fixture storage keys plus all actual journal writes; values are read at collision
from current state, so rollback/clear cannot invent a collision. Separately, it corrects
py-evm's base-length-zero MODEXP shortcut using EIP-198 integer exponentiation:
0**0 mod m. Standard Cancun gas remains the independent engine's own. This does not
change PoCol rules or runtime. All12 selected root differences then MATCH. Actual
storage/clear/rollback/nonce assertions and6 precompile executions verify boundaries.
The WHOLE patched comparator suite is NOT RUN locally, not passed by aggregation.

Re-running all33 timeouts with this profile and60s/case yields27 MATCH and6 still
BLOCKED_TIMEOUT. All6 identifiers are retained in remaining-timeouts.jsonl. These
are3 loopMul variants and3 high-round Blake2f cases, not excluded or passed.

Independent fixture arithmetic now checks primary Rust rejection categories and
numeric predicates for1172 cases: intrinsic/access/initcode gas, full maximal payment
including value/blobs, nonce/EOA/code, blob counts/version/fee, block gas, priority fee
and initcode size. Unknown categories/errors fail closed. A disjunctive upstream
TR_NoFundsOrGas label accepts only its explicitly listed alternatives with the actual
predicate true. One frozen bigint input is ADAPTER_INPUT_ONLY, not a wire-RLP proof.
Two independently compiled Rust mutations remove the nonce guard or return a wrong
rejection reason; both are killed by meaningful independent assertions after successful
compilation. The reusable command was verified separately from the initial local harness.

Attempts preserved: initial unbounded single-worker run interrupted, NOT PASSED;
original source and partial receipts retained locally. The first rejection-auditor source
failed parsing GST's `0x:bigint` notation (ValueError); that source is preserved for
reproduction. The corrected auditor separates this adapter-only case explicitly.
The raw initial terminal traceback was not saved as a file and is not reconstructed.

CI now repeats the input/category audit and compiled controls. A separate independent
comparison job uses the explicit profile and fails for any disagreement, engine error
or timeout. A successful collection process alone cannot pass that job. Fresh-head CI
is pending and must be inspected; known unresolved validation/time-budget gaps remain.
P2/LP4 and all26 obligations NOT PASSED; boot false, PR24 draft, no foundation merge.

Publication restriction: the full unmodified and interrupted-case gzip archives are
NOT in this branch commit. Auto-review rejected their upload. Complete bytes remain
in a verified local backup and immutable local Git commit; see PUBLICATION-STATUS.json
for exact hashes, sizes, actual publication status and required specific review action.
This is a missing published-evidence obligation, not an acceptance waiver. No splitting
or alternate external route is used. New independent CI exports summaries only.

Publication resolution: specific owner authorization and decompressed inspection
accepted the exact two original blobs; both are restored in the current publication
commit. Historical rejection statements above describe the preceding checkpoint.
See ../codex-r13/OWNER-PUBLICATION-APPROVAL.md and current PUBLICATION-STATUS.json.
No acceptance requirement or execution result changes.
