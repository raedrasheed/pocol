# Registry write ceilings versus approved REG_OPS — PROPOSED, NOT APPROVED

LP4-REGISTRY-01 option1 is owner-approved **in full**, separately recorded in
[LP4-REGISTRY-APPROVAL.md](../../../LP4-REGISTRY-APPROVAL.md). Its timing, budget,
traversal and status rules are not reopened. PAY-01/CREATE-01/PAY-02 remain binding.
No M1, LP3, historical proposal or evidence was changed.

## An actual acceptance contradiction, not a missing test label

M1 execution.md:55–60 fixes per-user-function maximum writes: register9,
deregister7, removeInactive7, reactivate5, setRewardAddress1, claimBond9.
The approved Registry contract also requires every successful user call to consume
REG_OPS in slot3 and, in part8, explicitly retains **all original per-function write
ceilings**. That part says a proof conflict returns to the owner rather than silently
changing a cap/formula or excluding the case. Approval did not waive the old ceiling.

A successful change of reward address must change Rec[id].w1, whose slot is
keccak256(pad32(id)||be256(7))+1, and consume the budget in slot3. The reproduced
address has two distinct slots. They cannot both be a single changed storage key.
Packing two counters into slot3 cannot pack it into w1; moving the reward/counter,
not storing it, delaying it, exempting the operation or ignoring it in the count would
change an approved rule or the original layout/meaning. This contradiction was missed
in the proposal review and is corrected transparently here, not silently interpreted away.

The same lower-bound analysis yields three conflicts, all reproduced by actual
compiled Solidity write blocks and independently traced py-evm SSTORE execution:

| Operation | Mandatory distinct changed keys | Lower bound | Original ceiling |
|---|---|---:|---:|
| register | four Rec words; appended RecArr and ActList elements; RecLen, ActLen, heldTotal; slot3 counters | 10 | 9 |
| setRewardAddress | Rec.w1; slot3 counters | 2 | 1 |
| claimBond from middle | four deleted Rec words; RecArr replacement and cleared tail; moved Rec.w0; RecLen; heldTotal; slot3 counters | 10 | 9 |

All listed keys actually change in the final witnesses. register appends alongside
an existing active member; claim preserves a separate active member (M_min1) while
compacting a three-record array. Destinations have original nonce1. B_reg=10^20 is
from governance.md:9. Fixtures are synthetic chain777902 and **not** full network
CP/profile acceptance, complete Registry implementation, resource bounds or boot.
Positive bond, nonzero h/runtime grace and middle deletion are valid required cases;
zero-bond or tail-only fixtures cannot silently replace them. PAY/account balance
changes are not counted as storage keys and cannot reduce these mandatory sets.

Independent comparisons match all changed key/value pairs, gas, return bytes and
caller/Registry balances. Three Solidity+Rust compiled assertion controls remove
one budget write at a time: the measured count falls, demonstrating why omitting
approved accounting is not a valid fix. No compile failure counts as a control.
The witness omits other full policy/routing checks, so these are *lower bounds*,
not proven final upper bounds. Original acceptance remains NOT PASSED.

## Option1 — explicit additive LP4 amendment (recommended)

Amend only these three LP4 per-function storage-write ceilings to:
register **10**, setRewardAddress **2**, claimBond **10**. The counters count toward
the same function's write budget; they are never hidden, exempted or attributed to
a different phase. Preserve deregister7/removeInactive7/reactivate5 and all layouts,
REG_OPS/R_max accounting, approved transition/PAY semantics and original observables.
M1 remains byte-identical. Record old 9/1/9 as **not satisfied under the original
criteria**, explicitly superseded for amended LP4 only after owner approval. Never
claim that the witnesses passed the original ceiling or that an owner decision is
itself proof of the new upper bounds.

Require full compiled Registry/Escrow paths, not only this lower-bound witness,
including max-record middle/tail/single deletion, zero/positive/max bond amounts,
all approved status/budget/time/closing cases, PAY admission/routing and rollback,
independent complete balances/storage/roots, compiled limit/accounting mutations,
repeated failed calls and exact R_max/REG_OPS budget boundaries before new-ceiling PASS.
Re-derive and measure all affected R18 SYS/user/block accesses/gas and R21 accounts/
trie facts with complete CP/profile/resource-model binding; caps64, R16/LAG and every
other mandatory gate remain unchanged and NOT PASSED until verified. No change to
SYS_KEYS_USER_MAX, block caps or a ParamGate formula is authorized by this option.
If implementation needs more than these proposed ceilings or any existing bound,
acceptance stays blocked; return that concrete contradiction without silent growth.
Boot disabled; no P2 merge before all required criteria and GitHub requirements pass.

Option2: retain the original write ceilings and supply a complete alternative to
successful-call slot3 REG_OPS accounting that fits them, as a new explicit consensus
amendment. No host success flags, unjournaled counters, dropped calls, skipped bonds,
hidden writes or assumed original-rule exception is accepted by default.

## Reproduce / status

See REPRO.md, write-vectors-final.json, independent-write-counts-final.json, compiler
artifact in prototypes/lp4-execution/tests/data/registry-write-budget.json and review.
All execution/resource/Registry/PAY integration gates remain NOT PASSED. Only the
contradiction and independent measurements are reproduced; no new rule is active.
