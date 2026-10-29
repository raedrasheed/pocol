# R3 exact continuation — owner code-cap decision pending, not accepted

PR25 branch codex-lp4-p3-wire; never merge into P2. PR24 remains draft C6
c6e56b069ad3d5a7c1323cae81ea53004f542512. Foundation remains
c0ef36c68fb83f9474de7e70fe61973b466647a8. M1/LP3/owner decisions unchanged.

R2 run39407652538 at6ce5b5acab41c57da0ca1eb4ba62682185c6e359 completed all5jobs.
See r2-hosted-verified.json, all ZIP/source hashes/20661 identities verified;
19488 exact roots/logs+1173 second-engine expected rejection presence;
zero exclusions/blocks/incomplete. Final ZIP12259641359 SHA256
d268f4a1c285ef7a65cee9a18669f949f19c2015e8a781073ad256cfbf2a465b.
Linux64/Windows64; legacy8BLOCKED+2NOT RUN remain nonpasses. Complete hosted
ZIPs/logs preserved under /workspace/work/lp4/p3-wire/hosted-39407652538 and
local-preservation.json checked archives. R18/earlier failed attempts remain intact.

R3 source adds LP3 identity-bound signed/native/USER execution, actual experimental
pool/RPC/loopback controls and real signed1173 negative proofs.86 Rust tests,
9 independent profile full-root/receipt cases,10pool+13wire+6USER compiled controls;
3 independent Geth compiled controls. All1173 primary signed negatives verified,
all1173 actual independent Geth rejects, including native1014 category/sender/hash
comparisons. Unmodified py-evm signed audit1100reject/73accepted remains NOT PASSED;
Geth ApplyMessage-only1171reject/2accepted is preserved, not a passing result.
Final Geth adds actual header-blob validator with a synthetic parent witness.

New unresolved consensus choice: CODE-CAP-DECISION.md options1/2. No answer
assumed, no code-size/CODE_CAP semantics installed. Recommended proposal replaces
runtime EIP170 with CP.B_code_max, preserves initcode49152 independently, and binds
logical per-account total code accounting/genesis/rollback. Alternative retains
Cancun runtime24576. Both need explicit owner interpretation; never infer it from
revm's parameter that also raises initcode to65536. M1 and all CP values stay fixed.
Before implementing selected semantics, record the actual owner answer separately.
Do not re-request PAY/CREATE/Registry or named-test approvals.

Unblocked work: PAY/Registry forced-transfer accounting and independent tests,
remaining exact source/license/distribution audit; signed/RPC/profile controls.
After decision implement/test/review actual code caps and affected original GST
case-by-case PoCol replacements, preserving all20661 upstream controls. Continue
full PoCol included-block CI3 rule17, header-derived fee/context, actual systems/
GenesisBuilder/GB1/1024/durable rewards, stored-parent phases, settlement/SV6/real
finalization and generalR18/R21/caps64/R16/LAG, persistence/crash/reorg. No pure core
or isolated USER candidate proves whole-block/disk acceptance. Raw-payload pool
quota is not full64MiB memory proof. All26 final gates NOT PASSED; boot false.

Inspect new R3 hosted run against actual pushed head; do not inherit R2 checks.
R2 artifact verification is complete and does not require another long comparison
on unchanged R2 source. Retarget PR25 to foundation only after accepted PR24 merge;
merge only when all P3 acceptance/protection requirements hold. No main/activation/
deploy/billing/Claude/coordinator/subagents/protection bypass. Connector PR/ref
writes with exact whole-tree comparison and expected-head lease.
