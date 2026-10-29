# Continuation handoff — no repeat owner approval

All prior PAY/CREATE/Registry decisions and explicit LP4-only10/2/10 amendment are
approved. Do not ask those questions again. Other limits remain unchanged; M1 and
historical evidence unchanged. Boot false, P2 draft, foundation c0ef36c68fb83f9474de7e70fe61973b466647a8.

Runtime source c70961187c23d3fd4bab6a99a43fd12001f7518e has verified hosted run39063488297:
40 Rust tests on each Linux/Windows,63 bytecode/full-root/domain-model comparisons,
17 Registry compiled controls,44+2 prior native comparisons,20661 upstream controls.
Legacy Windows16PASS/8BLOCKED/2NOT RUN. Source tree2b3ed68adc3b37899236d60b650eff5d6f0b9d56.
R8 includes actual source-built user/sweep/closing bond implementation, full ABI/layout,
static SSTORE bounds, synthetic numeric measurements and initial failures/fixes.

Current audit additions do not alter runtime code/dependencies/limits. They prepare
107 default all-target package notices and verify266 locked archives; pinned missing
license grants and nested notices are retained. New source-notice CI uses isolated
CARGO_HOME and requires fresh head-specific inspection. Do not attribute source-head
success to a later workflow head. GitHub connector lease/tree checks remain required.

Next work under the accepted P0-P10 plan: verify current head CI; complete remaining
P2 provenance/test-tool/GST/PoCol deviation acceptance; implement actual GenesisBuilder
with semantic GenesisRewardNotDurable, explicit durable allocations, derived roots/
identities and independent GB1/1024 comparison, preserving GenesisSpec format/error
order and old non-bootable fixtures. Complete remaining system contracts and actual
apply/parentResult/phases, Escrow pending/settlement/SV5/SV6 and real finalization.
Re-derive/measure full admitted CP/profile/model-bound R18/SYS and general R21/caps64/
R16/LAG, then persisted atomic state/crash/reorg and final audit. All26 ledger entries
NOT PASSED. Synthetic resources cannot satisfy those facts or authorize merge/boot.
No main/production/deployment/billing/Claude/coordinator/protection bypass. Continue
existing branch/PR24; no duplicate. No new design approval is needed for completed
Registry scope; approvals are not acceptance and original9/1/9 remain unpassed.

Environment: isolated CARGO_HOME=/workspace/work/lp4/p2/cargo-home,
RUSTUP_HOME=/workspace/pocol-tools/rustup; rustup proxy /workspace/pocol-tools/cargo/bin/cargo
+1.99.1; direct toolchain PATH must not pass + syntax to the direct cargo executable.
Python=/workspace/work/lp4/p2-pay/py-evm/bin/python; solc NODE_PATH=/workspace/work/lp4/p2-pay/tools/node_modules.
Work=/workspace/work/lp4/p2-registry-full; local Git commit IDs differ from connector
commit IDs but tree equality is verified. Use latest remote head and expected SHA.
