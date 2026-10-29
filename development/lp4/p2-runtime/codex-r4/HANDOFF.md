# P2 continuation: approved creation rule implemented; lifecycle policy pending

Foundation remains c0ef36c68fb83f9474de7e70fe61973b466647a8. PR24 remains draft,
not merged. M1, LP3 and earlier receipts preserved. Boot remains disabled.

Do not re-ask for PAY-01 or CREATE-01: both explicitly approved. Creation rule now
runs in the native backend and upstream control runner, with complete local storage
facts and normal Cancun ordering. Final full corpus:20661 matching controls,0 failures.
This is not complete PoCol T2/P2 acceptance. Real signed-wire/exception equivalence,
full PoCol fields/caps/system execution and license/notice audit are still incomplete.

Verified local work:27 Rust tests,28 independent py-evm PAY cases,7 compiled CREATE
controls. Prior hosted PAY controls remain separately preserved. Windows job at
25ec1f852a7bf9cd9c2960594f3962b18d9197c7 succeeded; retain exact16 PASS/8 platform
blocked/2 NOT RUN distinction. Newly changed sources/workflow need actual fresh
head-specific checks; pending is never passing.

Genuine owner decision: PAYMENT-LIFECYCLE-DECISION.md consolidates reward-address
lifetime, admission, custody and Registry→Escrow bond bridge. Current compiled
witness proves existing-at-call zero-PAY success cannot guarantee a future refund
recipient survives Cancun, even if it was funded before this transaction. Genesis
FORMAT permits zero/not-proven-present reward addresses. M1 separately requires
exact closing bond refunds and does not authorize burning missing-destination bonds.
Do not invent PAY-02, implicit genesis accounts, a fallback recipient or bond burn.

After resolution: implement complete actual Registry/Escrow routing, proposed-or-
selected eligibility/bridge/error precedence, closing/forced-transfer restrictions,
real storage/balances/journal and independent root/gas/mutation evidence. Keep SV5
reward payBurn and all original closing/refund amounts mandatory. Complete P2's
remaining actual semantics and fresh GitHub requirements before merge. Then continue
P3–P10 autonomously. No routine permission is needed within the accepted scope.
All23 live gates, general R18/R21, caps64 minimum, GenesisBuilder, real finalization
and SV6 remain NOT PASSED until individually verified. No main merge/production/
deploy/billing/Claude/coordinator action was taken.
