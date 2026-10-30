# LP4-CREATE-01 — approved additive storage-collision rule

Owner explicitly approved option 1 in the immutable
[CANCUN-COLLISION-DECISION.md](lp4/p2-runtime/codex-r2/CANCUN-COLLISION-DECISION.md).
This adds EIP-7610's condition to LP4 at all supported execution heights; it is a
new explicit LP4 rule, not a recovered original M1 rule. M1 remains unchanged.

Creation transactions, CREATE and CREATE2 must collide if the destination has
nonzero nonce, nonempty code or any nonzero storage slot. Preserve ordinary
collision gas, nonce, warmth, journaling and rollback behavior. Require actual
complete storage facts, including unloaded slots; never use a partial loaded-slot
scan, caller success flag or fixture address list. All 20661 Cancun controls and
the ten affected variants remain mandatory, plus the proposal's boundary,
negative, independent and compiled mutation tests. No acceptance is inferred
from approval or from the old diagnostic probe.

PAY authority remains LP4-PAY-APPROVAL.md. All other P2/LP4 criteria, general R18/R21,
caps64 minimum, GenesisBuilder, real finalization and SV6 are unchanged and mandatory.
Boot remains disabled. No main merge, production activation, deployment or billing.
