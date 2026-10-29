# Approved Registry contract — concrete acceptance blocker

The owner approved Registry option1 in full; LP4-REGISTRY-APPROVAL.md records it
without modifying M1 or historical evidence. Part8 requires returning a contradiction
with original write ceilings. No existing approval is being reopened.

Compiled Solidity write-set witnesses plus independent py-evm SSTORE traces reproduce
register10 versus9, setRewardAddress2 versus1, and middle claimBond10 versus9. These
are lower bounds, not final Registry implementation or accepted resource facts.
Three omission controls compile with Solidity and Rust and fail meaningful assertions;
removing approved accounting lowers the count but cannot resolve the contract safely.
29 Rust regressions and the44+2 existing independent comparisons still pass locally.

See REGISTRY-WRITE-BUDGET-DECISION.md for the single consolidated proposed resolution:
LP4-only ceilings10/2/10, subject to explicit owner amendment and actual full-contract
verification, independent resources, unchanged R16/LAG/caps64 and every old obligation.
It is NOT APPROVED; the old ceilings are NOT PASSED. Boot false; P2 draft/unmerged.

The previous head f0fea8e passed hosted run39055037623:20661 upstream GST matches,
44+2 comparisons and16 PAY mutation controls. Windows16PASS/8BLOCKED BY PLATFORM/
2NOT RUN; ZIP digests were checked before extraction. These are prior-head receipts,
not passing new-head CI. Source-only whitespace passes; preserved raw Cargo/Windows
log/CRLF whitespace is not reformatted and whole-evidence whitespace is not claimed.
Initial gas-oracle and local tool-invocation failures remain in the evidence/review.
See source-sha256.json and SHA256.json for reproducible byte identities.
