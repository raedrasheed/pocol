# P2 continuation checkpoint

PR24 remains draft against foundation c0ef36c68fb83f9474de7e70fe61973b466647a8.
Owner approved PAY option 1: development/LP4-PAY-APPROVAL.md. No prior owner
approval is missing for PAY; no repeat approval should be requested for that contract.

Native transfer boundary now executes actual revm-journal transfers with current
code identity, validation, 5000 gas, no callback/account creation and nested rollback.
Compiled Solidity harnesses are TEST harnesses, not completed PoCol system contracts.
Final control evidence includes 16 Rust tests (8 original +8 native integration),
28 independently executed py-evm comparisons and compiled PAY mutation controls.
All 23 LP4 gates, including PAY/T2b, remain NOT PASSED. Boot remains disabled.

Genuine remaining fork decision: CANCUN-COLLISION-DECISION.md proposes EIP-7610's
nonempty-storage creation collision as an additive LP4 rule. Original full corpus:
20661 variants, 13 failures; nonce/input fixes account for three, ten expose the
fork difference. Optional diagnostic probe matches 20661/20661 but is unapproved
and absent from native/PoCol execution. Do not mark T2 PASS or exclude those cases.

After owner resolution, implement the selected rule against a complete state
backend, not a partial loaded-slot list, and verify the listed boundaries/mutations.
Then complete real compiled Registry/Escrow reward/closing policy paths, full PAY
integration, PoCol field/type/fee/caps deviations, actual wire validation and complete
P2 independent comparisons/licenses/legacy regression. Only merge PR24 when all P2
criteria and GitHub requirements pass. Continue P3–P10 without routine confirmation.
R18/R21, caps64 minimum, GenesisBuilder, real finalization and SV6 remain mandatory.

Local Linux legacy tests cannot claim Windows journal/crash acceptance. New workflow
runs the existing Windows verifier and isolated experimental controls; inspect real
head-specific results and retain downloadable logs/artifacts before accepting them.
The diagnostic workflow does not label any LP4 gate passed. No production/main,
deployment, billing, Claude or coordinator action was taken.

Reproduction commands: README, commands.txt and pinned package/tool files. Compressed
JSONL receipts are ordinary gzip (mtime=0); SHA256SUMS hashes stored bytes, and
compression.json also hashes decompressed original bytes. R1's manifest remains
historical: resolve its source paths at PR24 original tree
9a4dbd41d30b5e506aad33df7e3d6e432ff600bb, not at changed R2 source paths.
