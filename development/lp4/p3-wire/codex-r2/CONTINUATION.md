# P3 R2 continuation

PR25 codex-lp4-p3-wire is stacked DRAFT against fixed P2 C6, never merge into P2.
R1 remote7bcc9854e7212cc78ebfc1e83fca162462b00d07 tree
eeae09233177d04a200f8179bfd5d1a8d47401f5; hosted run39406384484 was still in
progress at R2 recording. It is evidence for R1 only; do not count queued/running
or predecessor checks as passing on a new source. Inspect the R2 head run next.
P2 PR24 remains draft/unmerged C6; foundation remains c0ef36c68fb83f9474de7e70fe61973b466647a8.
R18 on evidence branch e7a4cbbbc677e55794e00623cedd6ba17c9fba0e verifies final
run39394783398, all20661, no drops/blocks;19488 exact roots/logs+1173 expected
rejection presence (not signed-wire/error-precedence proof). Keep all raw archives.

R2 implements only isolated transactional USER phase and checks tx/receipt/gas/bloom
commitments; computes intermediate state root and host tips from actual execution.
64 Rust tests plus regenerated unmodified VM sequential fixture and6 compiled
controls verified. See REVIEW/results and R1 for earlier79wire/192fee/18independent
single-execution/5root/13compiled controls. No full increment acceptance.

Continue actual pool/RPC CI2−32015 and full included-block CI3 rule17, validated
header basefee/context and atomic complete body/root/gas validation. Do not label
isolated USER phase as PoCol apply: system0/2 phases, parentResult, final-state
root, actual persistent import/reorg remain missing. Finish P2 exact source and
PAY/full integration acceptance, then P4 source-built five systems/GenesisBuilder
GB1/1024/durable reward; P5 actual stored-parent phases; P6 settlement/SV6/real
finalization; P7/P8 all-profile generalR18/R21/caps64min/min−1/R16/LAG; P9 disk,
crash/reorg; P10 full audit. Preserve all26 ledger NOT PASSED and boot false.
Retarget PR25 only after accepted P2 foundation merge and require all P3 criteria
plus GitHub protection before foundation-only merge. No routine owner approval
needed; do not ask again on recorded decisions. No main/deploy/billing/activation/
Claude/coordinator/protection bypass/subagents. Connector writes only, exact tree
comparison and expected-head lease. R2 raw failures/compiled logs kept in local
preservation receipt; metadata and reproducible public fixture published.
