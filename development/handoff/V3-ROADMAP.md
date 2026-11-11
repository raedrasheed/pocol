# PoCol v3 — implementation roadmap

Planning base: `main` at `bc7e683` (merge of GitHub #7). Read-only survey of the worktree; nothing was changed in it.
Inputs: `development/decisions/V3-DESIGN.md` (34 items, §3 deployment, §4 items), `V3-OWNER-DECISIONS-2026-11-08.md`
(all recommendations accepted: O-0=a O-6=b O-10=b O-11=b O-12=a O-13=b O-14=b O-15=b O-17=b O-18=b O-19=M3 O-20=b
O-21=b O-32=b O-33=b), `development/VA-SPEC.md` §2.3–2.4, `prototypes/pocold/consensus-boundary.txt`,
`consensus-sources.lock`, `consensus_boundary.rs`, `tests/consensus_guard.rs`.

GitHub numbering vs. the design's internal numbering: GitHub **#6** = internal "PR5" (consensus identity, boundary + lock,
generic fork schedule, launcher upgrade path); **#7** = internal "PR6" (gas-used pool/builder); open **#8** = internal
"PR7" (bounded memory; branch `claude/memory-bounds`, touches `pocold/src/chain.rs` and appends lock lines n=2..4);
open **#9** = internal "PR2b" (automatic outbound; node-local `net.rs`/`node.rs`/`node/discovery/*`). Below, v3 PRs
are named **V3-PR-x** to avoid clashing with either scheme.

Activation predicate used throughout (design §3.3):

```
v3_active(h) = active_version(h) >= 3  ||  genesis.spec_version >= 2
```

---

## 0. Ground truth at bc7e683 (what the survey found)

| Fact | Where | Consequence for v3 |
|---|---|---|
| `MAX_FORK_VERSION = 2` (generic `ForkSchedule::new_bounded`, runtime shape 1,2,… in order) | `lp1-node/src/profile.rs:218` (= `lp4-execution/src/p1_profile.rs`), `pocold/src/lib.rs:configured_forks` | fork term of `v3_active` is unreachable until a later PR raises it; `fork:3` gate is unspent |
| `SPEC_VERSION = 1`; decoder refuses any other gsVersion; `CP_LEN = 52` fixed array | `lp1-node/src/genesis_spec.rs:34,418` (= `lp4-execution/src/p1_genesis_spec.rs`) | a specVersion-2 genesis cannot be decoded by any current build → v3 rules gated on spec 2 are dormant |
| `GenesisSpec` has no version field; `ConsensusParams` is the 52 named CP fields | same files, `profile.rs` | V3-PR-B adds `spec_version` and CP schema v2 |
| `full_nonce_rule(v,m) = v>=2 \|\| m==1` (the `>=2` fix is done); `rp.js fullRange` and `pocol_rewards.full_nonce_rule` also `>=2` | `pocold/src/chain.rs:77`, `pocol-extension/src/lib/rp.js:27`, `deploy/rewards/pocol_rewards.py:67` | done (V3-34 part) |
| `v3_active` does not exist anywhere | — | V3-PR-B |
| `Spec::earnin()` = sysCodeHash equality with the earn-in set | `pocold/src/chain.rs:63-66` | becomes a program-set enum |
| `SystemPrograms::for_spec` knows continuous / v1 / earn-in | `lp4-execution/src/apply.rs:41-55` | add v3; delete `EARNIN_P_MAX`/`EARNIN_T_PROB` mirrors (`apply.rs:48-49`, used by `pocold/src/wallet.rs:71,210,268,285,500`) |
| `CONSENSUS_RULES_VERSION = 1`, part of the consensus identity together with chainId, genesisHash, flags and schedule | `pocold/src/identity.rs:14,73` | a `bump` would change the identity of main builds on the **live H15000 chain** (see §1) |
| Lock: one history line (`n=1 base rules=1 maxFork=2 spec=1`); #8 adds n=2..4 (neutral) | `pocold/consensus-sources.lock` | every v3 PR appends after #8's lines; rebase renumbers `n` |
| Gate rule: `gate=spec:S` refused iff some earlier line records `spec >= S`; `gate` only on `kind=neutral`; a vector change forces `neutral-vector`/`bump` | `pocold/consensus_boundary.rs:616-645` | see the dormant-gate strategy in §1 |
| `lp4-execution/tests/data/` is a pinned `vector` dir and holds `system-*.json` | `consensus-boundary.txt` | every program-bytecode change is `kind=neutral-vector` (no gate allowed) |
| Testnet/cutover genesis builder: `Config` is `deny_unknown_fields`, timing hard-coded (T_blk 10, τ 600, D_att 60, dFbWait 6), forks `[(1,0)]` | `pocold/src/testnet.rs:19-36,98-111,180-215` | needs a `v3` switch and explicit CP overrides |
| Cutover hard-codes the 20 PCOL bond and requires `earnin && continuous` | `pocold/src/cutover.rs:104,120` | read B_reg from the new CP; accept `v3` |
| Issuance is `cp[15]` every block; supply bound multiplies by `u64::MAX` when H_END=0; invariant has no carry term | `apply.rs:486-499,527-543`, `genesis_builder.rs:69-72` | V3-19/V3-15 |
| Log caps are constants 32 KiB / 546 | `lp4-execution/src/user_phase.rs:19-21,214-218` | V3-24 |
| Fork-choice key `(signed?0:1, import_seq, hash)` | `pocold/src/chain.rs:1173` | V3-04 |
| Membership fallback `ids = if active.is_empty() {live} else {active}` | `lp1-node/src/assignment.rs:84`; `pocol_rewards.py:287` (`members = active or live`) | V3-01 |
| Evidence kinds 1 and 2 only | `pocold/src/chain.rs:712-772` | V3-02 adds kind 3 |
| RPC `safe`/`finalized` = fixed offsets 6/60 | `pocold/src/rpc.rs:914-915` | V3-26 |
| No SimNet code exists (D44 is design text in `development/m1/reference/validation.md:1125`); in-process multi-chain tests exist (`pocold/tests/consensus.rs`, `earnin.rs`), plus Python independent models (`lp4-execution/tests/earnin_model.py`, `registry_transition_model.py`) | — | V3-PR-H must build the harness |
| CI on PRs to `main` runs `consensus_guard`, `check_lock_append_only.py`, pocold `consensus`/`caps`/`full_nonce`/`cutover`/…, lp4 `--lib`; it does **not** run `lp4-execution/tests/earnin_contracts.rs`, `pocold/tests/earnin.rs`, and does **not** recompile any `system-*.json` (`lp4-execution-controls.yml` triggers only on legacy branches) | `.github/workflows/*.yml` | V3-PR-E0 adds a main-branch reproduction job |
| `PR-0` decision record not written: no `V3-DECISIONS.md`, no `D-V3-*` in `decisions.json`; branch `claude/v3-decision-record` has no commits beyond main | `development/m1/reference/decisions.json` | V3-PR-A |

---

## 1. Consensus-lock strategy (the central constraint)

### 1.1 Why not `bump`
VA-SPEC §2.3 allows `bump` (CONSENSUS_RULES_VERSION += 1) "only together with a new genesis". v3 does ride a new
genesis, but `CONSENSUS_RULES_VERSION` is a single build-wide constant that enters the consensus identity on **every**
chain (`identity.rs:73`). A main build with rules=2 would refuse, and be refused by, every rules=1 node on the live
H15000 chain; `launcher.rs:76,244` would also refuse existing v2 invitations (bound to `consensusRulesVersion`). Before
S the live chain still needs releases from main (#8 memory bounds, #9 outbound, the V3-33 "no user transactions after
S" builder flag, the Android key-export build), and the operator's seeds must run the v3 release on H15000 until they
stop for the cutover. So **no `bump` line before H15000 is retired.** `bump` is the right tool only for a v3 rule that
changes a verdict on a spec-1 genesis — the plan has none; every v3 rule is behind `v3_active`.

### 1.2 The dormant-gate approach (recommended)
The guard computes "spent" from the `spec` / `maxFork` **recorded on earlier lines**, i.e. from the constants
`SPEC_VERSION` / `MAX_FORK_VERSION` at merge time (`consensus_boundary.rs:637-640`). Hence:

1. Every v3 rule PR keys its rules on `v3_active(h)` and **leaves `SPEC_VERSION = 1` and `MAX_FORK_VERSION = 2`.**
   No build on main can decode a specVersion-2 genesis, so for every genesis/schedule an accepting build allows the
   verdicts are identical: the change is truly neutral. Re-pin:
   `POCOL_REPIN_CONSENSUS=neutral POCOL_REPIN_GATE=spec:2 POCOL_REPIN_REF=<PR> POCOL_REPIN_NOTE="dormant v3: <items>; reachable only on genesis specVersion 2, which SPEC_VERSION=1 builds refuse" cargo test --release --locked --manifest-path prototypes/pocold/Cargo.toml --test consensus_guard`.
   Its line records `spec=1`, so `spec:2` stays fresh for the next v3 PR (verified against `history_problems`).
2. A PR that changes a file under `lp4-execution/tests/data/` or `lp1-node/tests/data/` (the compiled v3 programs,
   new vectors) must use `kind=neutral-vector` (gates are refused on it). Its note states "spec:2-dormant"; reviewers
   presume `bump` for vector changes, so the PR checklist must show the file is reachable only through the v3
   sysCodeHash / spec 2.
3. **Testing dormant rules needs a spec-2 genesis.** Add a Cargo feature `v3-preview` (lp1-node, lp4-execution and
   pocold, forwarding like `caps-test`): with it the decoder accepts gsVersion 1..=2. It is a consensus-affecting
   build, so it follows the `caps-test` precedent (`lp4-execution/src/lib.rs:6-8`, `admission.rs:219-221`):
   - identity flag bit2 `FLAG_V3_PREVIEW`, and when set the build's sources aggregate is appended to the identity (as
     for bit1 unpinned), so preview builds peer **only with byte-identical trees** — two preview commits with
     different v3 rules can never share an identity;
   - admission refuses a preview build for anything but simnet;
   - `deploy/standalone/package.py:31` additionally refuses build-info with `v3Preview:true`;
   - CI runs the v3 test suites with `--features v3-preview` (unit tests can also use a
     `#[doc(hidden)] decode_bounded(bytes, max_spec)` like `ForkSchedule::new_bounded`).
4. **Activation PR (V3-PR-I)** raises `SPEC_VERSION` to 2 (both copies), removes the preview-only restriction, and
   re-pins `neutral gate=spec:2`. Its line records `spec=2`: the gate is now **spent** (D2: merged = released).
5. After activation every v3 correction needs `gate=spec:3` (raise `SPEC_VERSION` to 3, regenerate the v3 genesis at
   specVersion 3; `v3_active` stays true since it tests `>= 2`) — or, once H15000 is retired and nothing on main
   needs to peer with rules-1 nodes, a `bump`. So V3-PR-I goes last before the rehearsal, after F/G/H are green.
6. **V3-23 machinery** (in-place code/CP swap) uses the same trick on the other axis: `gate=fork:3` with
   `MAX_FORK_VERSION` left at 2 (unspent until the first real in-place fork raises it).
7. Node-local changes that happen to live in boundary files (V3-07 draw-weight iteration and V3-26 retention floor in
   `chain.rs`) are plain `neutral` lines with a note (no gate), each with an old-vs-new differential test.

Alternative considered and rejected: a long-lived `v3` integration branch merged once with a single `spec:2` line —
it yields one unreviewable merge and still needs lock lines on the branch for its own CI.

Rebase hazard: the lock is append-only and `check_lock_append_only.py` requires main's history to be a prefix of the
branch's. Each v3 PR must be rebased and re-pinned (new `n`, new `sources`) if anything else touching the boundary
merges first. Land #8 (which already appends n=2..4 and changes `chain.rs`) before V3-PR-B.

---

## 2. System programs: how they are compiled and pinned today

**Sources:** `prototypes/lp4-execution/contracts/system/PoColSystemV1.sol` (live network),
`PoColSystemEarnIn.sol` (V1 + continuous `markClose` + earn-in, every change marked `EARN-IN`), and
`contracts/ChunkFactory.sol`. Each source compiles to five runtimes in a fixed order: MinerRegistry (0xC0C001),
RewardEscrow (C0C002), ShareLedger (C0C003), ChainCaps (C0C004), ChunkFactory (C0C005).

**Compiler and settings** (`prototypes/lp4-execution/tests/compile_system_v1.cjs`, `compile_system_earnin.cjs`):
npm `solc@0.8.28` (soljson, `0.8.28+commit.7893614a.Emscripten.clang`; the script throws on any other version),
locked in `prototypes/lp4-execution/tools/package{,-lock}.json` with `tmp` overridden to 0.2.7 (security follow-up,
`tools/README.md`). Settings: optimizer on, 200 runs, `evmVersion: cancun`, `metadata: {bytecodeHash: none,
appendCBOR: false}`, output `abi`, `evm.deployedBytecode.object`, `storageLayout`. `compile_system_v1.cjs --continuous`
produces `system-continuous.json`. Output JSON `{compiler, settings, programs[5]{name, runtime, abi}, warnings}`.
Command: `NODE_PATH=<tools>/node_modules node tests/compile_system_earnin.cjs tests/data/system-earnin.json`.

**Pinning chain:**
1. `tests/data/system-{v1,continuous,earnin}.json` are committed and are inside the consensus boundary's `vector`
   dir, so their bytes are in the lock's `file` lines and the `vectors` digest.
2. `apply.rs:41-50` `include_str!`s them into `SystemPrograms`; `sysCodeHash = keccak256(RLP([code×5, 0x80]))`
   (`apply.rs:58`) is a GenesisSpec field committed by the genesis hash; `genesis_builder.rs:22-27` re-checks code
   and the CP slots of ChainCaps on every parent and candidate.
3. `pocold/build.rs:86-88` adds the JSONs to the informational compatibility digest.

**Verification today:** none on main. `lp4-execution-controls.yml` (PRs to two legacy branches only) recompiles other
witnesses and never `system-*.json`. The only full reproduction is the one-off 13-output comparison in
`development/security-dependency-followup-2026-11-05/compare_compilers.py` (resolved soljson sha256
`9ef37cc1d2597ed6d2f706a960f449e33eb9eee920936cea77b11e6f43861970`, `evidence/resolved-tools.json`).

**Is solc available offline here?** Yes, from the local npm cache. `npm ci --offline --ignore-scripts` of the tools
lock into the scratchpad installed 8 packages; soljson sha256 matches the recorded `9ef37cc1…`. Re-running
`compile_system_earnin.cjs`, `compile_system_v1.cjs` and `compile_system_v1.cjs --continuous` reproduced
`system-earnin.json`, `system-v1.json` and `system-continuous.json` **byte for byte** (`cmp` equal). Node is v22.23.4
(README validates Node 24; ≥18 suffices). There is no native `solc`. **Do not use**
`deploy/websites/ide.pocol.net/compiler/soljson.js`: it is solc 0.8.38 (`THIRD-PARTY-NOTICES.txt:17-25`).

**Current sizes** (runtime bytes): earn-in MinerRegistry 9,249, RewardEscrow 4,867, ShareLedger 2,766, ChainCaps 114,
ChunkFactory 762. Devnet `B_code_max` = 32,768 (CP 34); headroom for the v3 registry ≈ 23 KB (keep under 24,576 to
stay EIP-170-sized for tooling).

**v3 additions (V3-PR-E0):** `contracts/system/PoColSystemV3.sol` (fork of the earn-in source, every change marked
`V3-xx`), `tests/compile_system_v3.cjs` (same compiler, same settings, sources `system/PoColSystemV3.sol` +
`ChunkFactory.sol`), `tests/data/system-v3.json`, `SystemPrograms::v3()`, a test pinning `v3().sys_code_hash()`, and a
**main-branch CI job** that runs `npm ci --prefix prototypes/lp4-execution/tools --ignore-scripts --no-audit --no-fund`,
recompiles all four system sets and `cmp`s them with the committed JSONs (fail on any byte). Every later program PR
regenerates `system-v3.json` with that command and re-pins `neutral-vector`.

---

## 3. Item-by-item status

Legend for "PR": the V3-PR that delivers the remaining work (§4). "Lock": the history-line kind that PR needs.

| Item | Done by merged PRs | Remains | Main code files | PR |
|---|---|---|---|---|
| V3-01 ranges for every live member (unbounded) | — | `ids = live` when `v3_active && Unbounded`; gateway mirror | `lp1-node/src/assignment.rs:60-87` (new `from_parent` flag), `pocold/src/chain.rs:438-445`, `deploy/rewards/pocol_rewards.py:270-287` | C1, G |
| V3-02 late share credit | — | evidence kind 3, branch dedup, no-dilution cap, `recordLate`, node share cache + proposer inclusion | `chain.rs:712-772` (+Entry field for LS set), `apply.rs` phase 0 + `EvidenceEffect`, `PoColSystemV3.sol` ShareLedger/Registry, `pocold/src/node.rs` evidence assembly (~`:2215`) and share cache, `pocol_rewards.py` | E3, G |
| V3-03 multi-hop relay | **done** (53a9e87) | — | — | — |
| V3-04 timely signed precedence | — | `timely` flag at first import; key `(timely, signed, importSeq, hash)`; persist first-import time across restarts | `chain.rs:1173` key + `Entry`, `pocold/src/node.rs` import path (`import_with`), `lp1-node/src/timing.rs:91-116` (T_j), journal metadata | C2 |
| V3-05 Δ_rel default + template latency telemetry | — | derive Δ_rel = σ+Δ+2t; per-proposer template-arrival counters in `pocol_status` | `pocold/src/testnet.rs:50-58` `local()`, launcher defaults, `node.rs`/`rpc.rs` | N1 |
| V3-06 timing recalibration (O-6=b: 30 s, D_att 150, dFbWait 10, Δ_rel 13 if WAN Δ≤2 s, else 60 s) | — | WAN soak measurement; CP values; ParamGate R2/R3/R6 at WAN locals | v3 profile (F), `lp1-node/src/gate.rs:258-262`, `timing.rs:128-130` | N1 (measure), F (values), J (soak) |
| V3-07 M_max 256 + draw weights from ledger owners | — | CP 18 = 256; iterate ledger owners in `draw` (result-identical); A13 at 256 | `chain.rs:446-452`, `pocold/src/resources.rs` | C1 (draw, neutral), F (value + A13) |
| V3-08 REG_OPS maintenance budget | — | separate per-block maintenance counter (REG_MAINT_MAX 8), one maintenance op per identity per block (`lastMaintHeight` bits) | `PoColSystemV3.sol` MinerRegistry, `contracts/REGISTRY-LAYOUT.md` | E1 |
| V3-09 R_max entry slot | — | `R_react` budget, `C_react` per-record clock, R_max 4; ParamGate `U ≥ W_act+2`, `C_react ≥ U` | `PoColSystemV3.sol`, CP schema v2, `gate.rs` | E1 (+B schema, F values) |
| V3-10 contestable seats (O-10=b) | — | epoch counters (epochIndex,countCur,countPrev) in recordShares/recordLate, epoch totals, `registerContest(reward, victim)` with θ=1/8, 3σ, tenure check, refund after U | `PoColSystemV3.sol`, `pocold/src/wallet.rs` (UI), `pocol_rewards.py` | E4 |
| V3-11 Θ_inact = 1 h | — | CP value (120 blocks at 30 s) | v3 profile | F |
| V3-12 B_reg = 20 PCOL | earn-in programs + devnet use 20 PCOL (`devnet.rs:53`) | cutover must read cp[16] instead of literal | `pocold/src/cutover.rs:120` | B |
| V3-13 probationers as proposers (O-13=b) | — | accruing filter `bond_state==4 && bond < B_reg/4` under `v3_active` | `chain.rs:442-444`, `assignment.rs` `with_accruing` | C1 |
| V3-14 probation pool (O-14=b) | — | P_MAX, T_PROB as CP fields via `cfg()`; delete host mirrors; contest inside the pool | `PoColSystemV3.sol`, `apply.rs:48-49`, `pocold/src/wallet.rs:71,210,268,285,500` | E0 (cfg + mirrors), E4 (contest) |
| V3-15 carry (O-15=b) | — | `carry` escrow slot; settleOne sends base burn to carry; `closeBlock` adds and zeroes it; final block burns it; invariant term | `PoColSystemV3.sol` RewardEscrow, `apply.rs:527-543`, `apply::chain_facts` | E2a |
| V3-16 type-2 offender-only burn | — | settle normally after `punish`; whole-pot burn kept for ACCRUING accused with bond < B_reg | `PoColSystemV3.sol` (`invalidate`/`settleOne`), `apply.rs:440-445` | E2b |
| V3-17 reporter reward ρ = 25 % | — | `punish(accused, reporter)` pays ρ·bond to the carrying block's proposer reward address, burns the rest | `PoColSystemV3.sol`, `apply.rs:444` | E2b |
| V3-18 E_win, U = 1 h | — | CP values; A13 (slim_depth = 2·R_keep + E_win + 257, `node.rs:593`) | v3 profile, `pocold/src/resources.rs` | F |
| V3-19 issuance M3 | — | `issuance.rs` (`S0>>k`, tail, `h/H_half`); `closeBlock(tips, issued)`; supply bound `2·S0·H_half + S_tail·2^64 < 2^127` | `lp4-execution/src/issuance.rs` (new), `apply.rs:486-499`, `genesis_builder.rs:69-72`, `lp1-node/src/gate.rs` (R supply ≈`:324`) | E2a |
| V3-20 fee market (O-20=b) | **partly** (#7): per-sender declared-gas quota, parking, parked relay, gas-used builder | consensus: γ_tip_bp 5000 direct proposer tip, baseFee floor 0.01 gwei; node-local: fee ordering, replace-by-fee, lowest-fee eviction | `lp4-execution/src/fees.rs:4-11`, `chain.rs:child_pre` base fee, `apply.rs` phase 2, `PoColSystemV3.sol` closeBlock; pool in `pocold/src/node.rs` | C3 (floor), E2a (tip split), N4 (pool) |
| V3-21 state caps + creation fee (O-21=b) | — | raise ACCT/SLOT/CODE caps (A13-verified); utilisation-priced burned fee per new account/slot/code byte | `lp4-execution/src/user_phase.rs:182-223` (+ PoolMeter), `state_summary.rs`, `code_caps.rs:27-32`, `pocold/src/resources.rs`, builder mirror in `node.rs` | C3 (rule), F (values + A13) |
| V3-22 skip-on-revert | DEFER | — | — | — |
| V3-23 code/CP replacement machinery | — | `cp_at(h)`, install compiled runtimes at H, height-aware `fixed_accounts`/`for_spec`, ASERT re-anchor, caps migration; ships dormant | `apply.rs:430-441`, `view.rs`, `genesis_builder.rs:22-27`, ~105 `.cp()` sites, `asert.rs` | K (parallel track) |
| V3-24 log caps | **builder part done** (#7: template selection models log count/bytes/LOG_CAP; pool refuses self-overflowing logs) | 256 KiB / 4,368 under `v3_active`; RESP_MAX ≥ 2·LOG_BLOCK_MAX (R26); `eth_estimateGas` modelling | `user_phase.rs:19-21,214-218`, `lp1-node/src/param_gate.rs` (R26), `pocold/src/rpc.rs` | C3, N4 |
| V3-25 execution bounds | — | GAS_ENVELOPE for v3 programs, adversarial block-validation sweep at v3 CP, t_exec | `lp4-execution/src/admission.rs:86-106`, `lp4-execution/src/resources.rs`, `pocold/src/resources.rs` | F |
| V3-26 finality semantics | — | persisted `finalized_h`, floor `max(head−r_keep, finalized_h)`, telemetry, docs | `chain.rs:794-830` (`retain`), `node.rs:585-750` (open/replay), `rpc.rs:914-915` | N2 |
| V3-27 grinding disclosure | — | docs (`threat.md`, `execution.md`) | docs | A |
| V3-28 launch readiness + admission | — | H1 computed `launch_ready`/`launchBlockers`; H2 later | `pocold/src/launcher.rs:83`, `build.rs:101`, `testnet.rs:235`, `lp4-execution/src/admission.rs:291-297` | N5 |
| V3-29 decision record + amendments | **design + owner answers merged** | `V3-DECISIONS.md`, `decisions.json` D-V3-*, consensus/threat/economics/incentives/governance amendments, D72/D133 amendments, lock strategy (§1) | `development/m1/reference/*.md`, `decisions.json` | A |
| V3-30 honest throughput | — | `--mine-threads N`, localhost work port, `networkHashrateEst` | `pocold/src/miner.rs`, `rpc.rs`, `resources.rs` (io_threads) | N3 |
| V3-31 security analysis | DEFER (external) | — | — | — |
| V3-32 genesis allocation (O-32=b) | — | publish holders/purpose; time-locked development-fund account + spending reports | docs + on-chain txs (operations) | A (docs), ops |
| V3-33 re-genesis conduct (O-33=b) | runbook exists (`deploy/mainnet/EARN-IN-CUTOVER-RUNBOOK.md`) | builder flag "no user txs after S"; publish old-chain data; Android key export (branch `claude/client-key-backup`, task 14); public last-owner-only commitment | `pocold/src/node.rs` builder, launcher flag, runbook, Android app | N6, J |
| V3-34 fork machinery + verifier hygiene | **mostly** (#6): generic schedule, runtime shape, `>=2` sites, launcher append-only/future-only `pocol upgrade`, `inspect-upgrade`, consensus identity/boundary/lock, `forks.js`/`preflight.py`/`nonce_upgrade.py`/`prepare_nonce_upgrade.py` | `v3_active`, spec-version plumbing, program-set registry, earn-in feature predicate, testnet CP overrides, cutover cp[16], off-node verifiers (spec v2 decode, membership, rewards) | `profile.rs`, `genesis_spec.rs`, `chain.rs:63`, `apply.rs:53`, `testnet.rs`, `cutover.rs`, `refdec.js`, `rp.js`, `pocol_rewards.py` | B, G |
| V3-R | REJECT | — | — | — |

---

## 4. PR sequence

Every consensus PR below keeps `SPEC_VERSION = 1`, `MAX_FORK_VERSION = 2`, `CONSENSUS_RULES_VERSION = 1` until
V3-PR-I, so the live chain (spec 1, `[(1,0),(2,15000)]`) and every existing spec-1 genesis keep byte-identical
verdicts. Standing acceptance gates for every PR: `consensus_guard`, `check_lock_append_only.py`, the existing
`full_nonce`/`fork_configuration`/`consensus`/`caps`/`cutover` suites, the pinned H15000 identity
`bcc4b71570fed4788d84baff137e26a88527c1fd17a11f595d92ce1e5e50b110` unchanged, and (from B on) a v3 job running the
v3 suites with `--features v3-preview`.

Effort: S ≤ 2 engineer-days, M 3–6, L 7–15.

### V3-PR-A — Decision record and normative amendments (docs) — V3-29, V3-27, V3-32 docs
- **Scope:** `development/decisions/V3-DECISIONS.md` (owner rule verbatim, the O-answers, supersession of
  `development/m1/reference/README.md:484-486` for compliant items), `decisions.json` D-V3-01…34; amendments to
  `consensus.md` (§5 nMax for v≥2, §8/10a H_END=0, §9 V3-04 key = D133 amendment, §12 V3-26), `threat.md`
  (A5/A6, grinding bounds, PREVRANDAO), `economics.md:67` (M3, carry), `incentives.md` (earn-in, V3-02/15/16/17),
  `governance.md` (P-live table, D72 amendment for V3-23, the v3 CP table of §5). Record the lock strategy of §1
  (dormant `spec:2`, `v3-preview`, no `bump` before H15000 retires, `spec:3` escape), and D3 (see risks): no skip
  needed — a later in-place fork on the v3 chain appends `(2,H)` (a no-op under nonceMode 1) then `(3,H')`.
  V3-32 disclosure text (holders, purpose, time-lock plan).
- **Files:** docs only. **Tests:** markdown/link checks. **Lock:** none (no boundary file).
- **Effort:** M. **Risk:** low; must merge before any consensus code (design §3.4 step 1).

### V3-PR-B — v3 scaffold: spec version, CP schema v2, `v3_active`, preview build, program-set registry, genesis tooling — V3-34, V3-12, PR-2 of the design
- **Scope:**
  - `genesis_spec.rs` (lp1 original + byte-identical `lp4-execution/src/p1_genesis_spec.rs`, CI `cmp`): `GenesisSpec.spec_version`; `CP_SCHEMA_V2` = the 52 v1 fields + appended: `H_half`, `S_tail`, `P_MAX`, `T_PROB`, `F0_acct`, `F0_slot`, `F0_code`, `gamma_tip_bp`, `baseFeeMin`, `R_react`, `C_react`, `E_epoch` (contest epoch in blocks), `theta_bp`, `k_sigma`, `rho_bp` (final list fixed here once, so later PRs never touch the schema); CP length per version; encode/decode by version; accepted gsVersion `1` in release, `1..=2` under `v3-preview`; `decode_bounded` for unit tests.
  - `profile.rs` (both copies): `ConsensusParams` new named fields (zero on v1); `ProfileIdentity::spec_version()`, `ProfileIdentity::v3_active(h)` defined once and re-exported to `pocold::chain::Spec::v3_active`.
  - `v3-preview` features in `lp1-node/Cargo.toml`, `lp4-execution/Cargo.toml`, `pocold/Cargo.toml`; `lp4-execution/src/lib.rs` `V3_PREVIEW`; `identity.rs` `FLAG_V3_PREVIEW` (bit2) + aggregate appended when set; `admission.rs` refuses preview off simnet; build-info `v3Preview`; `deploy/standalone/package.py` refuses it; VA-SPEC §2.1 text.
  - `chain.rs:63` `Spec::earnin()` → `Spec::programs() -> ProgramSet {V1, Continuous, EarnIn, V3}` and `earnin_rules()` ("earn-in or later"); wallet call sites follow.
  - `testnet.rs`: `Config.v3` (implies `earnin`, `continuous`, nonceMode 1, spec 2) and `cp: {name: value}` overrides validated against `CP_SCHEMA_V2` (unknown name / out of range → error; no override of fields the builder derives, e.g. target_g from `hashRate`); timing and `local()` taken from overrides; `DevnetConfig.spec_version` and new fields in `devnet.rs:48-62`.
  - `cutover.rs:104,120`: accept `v3`; bond = new spec's `cp[16]` (V3-12).
  - `ChainCaps` CP-slot count: `genesis_builder.rs:25,75` writes/checks all CP slots of the spec's length.
- **Files:** `lp1-node/src/{genesis_spec,profile}.rs`, `lp4-execution/src/{p1_genesis_spec,p1_profile,lib,admission,devnet,genesis_builder,apply}.rs`, `pocold/src/{chain,identity,lib,testnet,cutover,wallet}.rs`, three `Cargo.toml`, `deploy/standalone/package.py`, `development/VA-SPEC.md` §2.1/§2.3, `.github/workflows/full-nonce-upgrade.yml` (add a `--features v3-preview` job).
- **Tests:** `v3_active(h)` false for every height on the H15000 spec and on the spec-1 earn-in genesis; `[(1,0),(2,H2),(3,H3)]` via `new_bounded` keeps nMax = 2^64 above H3 and turns `v3_active` on at H3; release build refuses a gsVersion-2 spec (decoder corpus unchanged), preview build accepts it; spec-1 encode/decode byte-identical (genesis hashes of all committed networks unchanged); preview identity ≠ release identity and includes the aggregate; package.py refusal; testnet overrides (unknown field, out of range, derived field) refused; cutover bond read from CP (existing `tests/cutover.rs` + a 25-PCOL CP case); old-chain replay byte-identical (`rehearse_cutover.py --local` in CI already).
- **Lock:** `neutral`, `POCOL_REPIN_GATE=spec:2` (line records spec=1, gate stays fresh). Cargo manifests change but no new crates → no `neutral-dep`. No vector file changes in this PR (keep new vectors in `pocold/tests/`).
- **Effort:** L (8–10 d). **Risk:** medium — touches the genesis decoder shared by four implementations (Rust ×2, `refdec.js`, `pocol_rewards.py`); a spec-1 regression would change genesis hashes. Mitigation: byte-identity tests on every committed `genesis.spec.hex`.

### V3-PR-C1 — Membership and draw — V3-01, V3-13, V3-07 (draw iteration)
- **Scope:** `Membership::from_parent(.., all_live)` (new parameter or constructor; shared `assignment.rs`); `chain.rs::membership` passes `v3_active(h) && effective_nonce_mode(h)==Unbounded`; accruing filter `bond_state==4 && bond < B_reg/4` under `v3_active` (else today's E5 rule); `draw` iterates ledger owners of the window instead of M_max×W_w reads (same result, all chains).
- **Files:** `lp1-node/src/assignment.rs`, `pocold/src/chain.rs:438-452`, `lp1-node/tests/assignment.rs`.
- **Tests:** fails-on-original: 3 members, only X shares recently, X stops → today every candidate fails "13 owner", under v3 another member mines; bounded mode unchanged; `drawn` stored per block still verifies type-1 evidence; probationer at B_reg/4−1 excluded, at B_reg/4 drawn; draw-weight differential (old vs new iteration) over randomized ledgers on v1/earn-in/v3 states.
- **Lock:** `neutral gate=spec:2` (the draw refactor is neutral on all chains; note it).
- **Effort:** M. **Risk:** low.

### V3-PR-C2 — Fork choice: timely signed precedence — V3-04
- **Scope:** `Entry.timely` set once at first import from the node's local `T_j` (`timing.rs::deadline_ms`) and Φ, Δ: signed → timely iff import ≤ T_j+Φ; attempt-0 fallback → timely iff import ≥ T_j−(Δ+Φ); attempt ≥ 1 → timely. Key `(timely?0:1, signed?0:1, importSeq, hash)` only when `v3_active(h)`. Persist the first-import wall time per block in journal metadata so a restart re-derives the same flags (design says flags are local like importSeq, but a restart that re-imports everything "late" would flip ties; persisting avoids head churn).
- **Files:** `pocold/src/chain.rs` (`Entry`, `key`, `better`, import), `pocold/src/node.rs` (import_with, journal record), `pocold/src/store.rs`/journal format (node-local file), `lp1-node/src/timing.rs` (helper only).
- **Tests:** fails-on-original: timely fallback then late signed sibling → head stays under v3, switches on v1; early pre-mined fallback loses to the timely signed block; re-run the twenty-import-order D133 suite on v1 (unchanged) and v3; restart determinism (same head after replay); SimNet T10b(iv) lands in H.
- **Lock:** `neutral gate=spec:2`. Journal-format change is node-local but `store.rs` is a boundary file → covered by the same line; add a journal migration test.
- **Effort:** M. **Risk:** medium — fork choice; restart semantics; interacts with #8's journal/slim changes and #9's import paths (rebase after both).

### V3-PR-C3 — Execution caps and fees — V3-24, V3-20 (base-fee floor), V3-21 (creation fee rule)
- **Scope:** `user_phase.rs` log caps become `log_caps(v3)` = (256 KiB, 4,368) vs (32 KiB, 546); `fees::next_base_fee(.., floor)` with `floor = baseFeeMin` under `v3_active` (callers: `chain.rs::child_pre`, pool/RPC predictions); creation fee: per new account / new nonzero slot / new code byte `F0·CAP/(CAP−used)` from the running in-block count, charged to the sender and burned as a classified burn (receipt + `chain_facts` burn class), halting like a cap overflow if unpaid; `PoolMeter` and the builder (`node.rs` template selection) model both; `param_gate.rs` R26 RESP_MAX ≥ 2·LOG_BLOCK_MAX.
- **Files:** `lp4-execution/src/{user_phase,fees,state_summary,apply}.rs`, `pocold/src/{chain,node,rpc}.rs`, `lp1-node/src/param_gate.rs`.
- **Tests:** fails-on-original: front-load 32 KiB logs then an ERC-20 transfer halts on v1, succeeds on v3; dust-fill each cap then a legitimate fund/deploy halts on v1, pays the fee on v3; griefer fill-cost ≈ F0·CAP·(ln CAP+0.58) within 1 %; fee floor: 1000 empty blocks never go below baseFeeMin on v3 and decay as London on v1; conservation (`check_escrow_invariant`, supply) with fees burned; `user-phase.json` vectors untouched for v1.
- **Lock:** `neutral gate=spec:2` (new v3 vectors, if any, go in a separate `neutral-vector` commit or into `pocold/tests`).
- **Effort:** M. **Risk:** medium (creation-fee interaction with the C-1 summaries and the builder's PoolMeter).

### V3-PR-E0 — v3 program set, reproducible build, CI, CP-read constants — V3-14 (cfg part), V3-34 registry
- **Scope:** `PoColSystemV3.sol` = copy of `PoColSystemEarnIn.sol` with only: `ChainCaps.get` bound = v2 CP length; `P_MAX`/`T_PROB` read through `cfg()`; `compile_system_v3.cjs`; `system-v3.json`; `SystemPrograms::v3()` + `for_spec`; delete `EARNIN_P_MAX`/`EARNIN_T_PROB` and read CP in `wallet.rs`; `pocold/build.rs` adds `system-v3.json`; main-branch CI job reproducing all system JSONs (§2) and running `lp4-execution/tests/earnin_contracts.rs` + `pocold/tests/earnin.rs` (currently not on main CI) and their v3 twins.
- **Files:** `lp4-execution/contracts/system/PoColSystemV3.sol`, `tests/compile_system_v3.cjs`, `tests/data/system-v3.json`, `src/apply.rs`, `pocold/src/{wallet,chain}.rs`, `pocold/build.rs`, new workflow (or a job in `full-nonce-upgrade.yml`), `lp4-execution/tests/v3_contracts.rs` (earn-in scenarios replayed on v3 with P_MAX/T_PROB from CP).
- **Tests:** earn-in vectors pass on the v3 set at P_MAX=8/T_PROB=60480 (proves the fork is behaviour-equal); P_MAX=64/T_PROB from CP; the six earn-in mutants still fail; wallet bond features on the v3 set; sysCodeHash pinned.
- **Lock:** `neutral-vector` (system-v3.json is in the vector dir), note "v3 program set, reachable only via the v3 sysCodeHash on a specVersion-2 genesis".
- **Effort:** M. **Risk:** low.

### V3-PR-E1 — Registry budgets — V3-08, V3-09
- **Scope:** MinerRegistry: maintenance counter REG_MAINT_MAX = 8 (program constant) and `lastMaintHeight` (≤ 40 bits in w0 spare bits or one record word, re-derived against SYS_SLOT_MAX, `REGISTRY-LAYOUT.md`); register/deregister/claimBond/removeInactive use `REG_OPS − maintenance_used`; reactivate uses its own `R_react` budget and `C_react` per-record clock; reactivation window strictly before the refund sweep; ParamGate `U ≥ W_act+2`, `C_react ≥ U`.
- **Files:** `PoColSystemV3.sol`, `system-v3.json`, `REGISTRY-LAYOUT.md`, `lp1-node/src/gate.rs`, `lp4-execution/tests/v3_model.py` (independent Python model extending `earnin_model.py`) + `tests/data/v3-vectors.json`, `lp4-execution/tests/v3_contracts.rs`, write-ceiling measurement (as `earn_in_write_ceilings`).
- **Tests:** fails-on-original: 16 `setRewardAddress` from one identity, then another identity's register reverts on earn-in and succeeds on v3, spammer still evictable; 33 attacker records cycling deregister/reactivate, newcomer registers within k blocks; ACC_COUNT conservation through each path; model-vs-bytecode differential; planted mutants (counter shared, cooldown off-by-one, window = sweep height) fail.
- **Lock:** `neutral-vector`. **Effort:** M. **Risk:** low-medium (record bit layout; SYS caps R18).

### V3-PR-E2b — Evidence penalties — V3-16, V3-17
- **Scope:** Escrow `invalidate` no longer burns the whole pot on v3: settle normally after `punish`; tomb → offender's winnerPart/propPart/shares burn via `payTo`; whole-pot burn retained when the accused was ACCRUING with bond < B_reg at block j (recorded at `recordResult` time in pot meta, not read later); `punish(accused, reporter)` pays `rho_bp`·bond to the carrying block proposer's reward address (fallback block: no reporter → full burn), burns the rest; host passes the reporter in `apply.rs:440-445`.
- **Files:** `PoColSystemV3.sol`, `system-v3.json`, `apply.rs`, `pocold/src/chain.rs` (effect carries reporter), `v3_model.py`/vectors.
- **Tests:** fails-on-original: type-2 evidence before settle(j) pays contributors and burns only the offender's parts; ACCRUING-accused vector burns the whole pot; E_win boundary; self-report loses (1−ρ)·B; conservation every step.
- **Lock:** `neutral-vector`. **Effort:** M. **Risk:** low-medium.

### V3-PR-E3 — Late share credit — V3-02
- **Scope:** body evidence kind 3 `LS = RLP([3, be64(j), [n_1<…<n_k]])`, signed bodies only; validity (window `[max(1,h−D), h−1]` on the carrying branch, `n_i < nMax_j`, `≠ nonce_j`, share target, owner from stored `e.ranges` of block j, not in header shares / any LS(j) on the branch / repeated, no-dilution `C_header(j)+C_late(j) ≤ κm` read from parent state); `EvidenceEffect::Late{j, owners}`; phase-0 `recordLate(j, owners)` after `recordResult`, before `settle(h−D)`; ShareLedger/Registry `recordLate` (count, total, `lastShare = max(lastShare, j)` without revert); branch-derived dedup kept on `Entry` (restored by #8's `unslim` for slimmed entries); E_max/EVID_BYTES_MAX sizing (≤ 2.4 KB at S_max 256). Node-local: per-TemplateID share cache for the last D canonical templates (≤ 4·S_max each), proposer adds uncredited ones like evidence (`node.rs` ~`:2215`); A13 term ≈ D·4·S_max·8 B.
- **Files:** `pocold/src/chain.rs`, `pocold/src/node.rs`, `lp4-execution/src/apply.rs`, `PoColSystemV3.sol`, `system-v3.json`, `admission.rs` GAS_ENVELOPE entry for `recordLate`, `pocold/src/resources.rs`, `v3_model.py`, `tests/data/v3-vectors.json`.
- **Tests:** fails-on-original: winner omits B's share, LS(j) in j+1 pays B at settle(j); negative vectors (duplicates in list / across carriers / within a block, j outside window, non-ancestor tid, nonce ≥ nMax, other member's range, target miss, LS in a fallback body, past the κm cap); reorg across a carrier rolls the credit back (inverse diffs); replay determinism; T17 on every block; cutover-style rebuild through slimmed entries (`cutover::rebuild_at`) with LS evidence.
- **Lock:** `neutral-vector` (code + program + vectors in one line). **Effort:** L. **Risk:** medium-high (new validity rule, reorg/slim interaction, SYS key caps ≤ 64 owners per j).

### V3-PR-E2a — Issuance, carry, tip split, invariant — V3-19, V3-15, V3-20 (tip split)
- **Scope:** `lp4-execution/src/issuance.rs` `issuance(h) = h==0 ? 0 : max(k>=256 ? 0 : S0>>k, S_tail), k = h/H_half` (v3 only; v1 keeps `cp[15]`); `closeBlock(tips, issued)` with `pending[j].amount = issued + (1−γ_tip)·tips + carry`, zero carry; γ_tip·tips paid directly to the signed proposer's reward address (fallback: all to pot); settleOne base burn → carry (roundBurn still burned); final block burns carry; `check_escrow_invariant` `need = Σpending + held + burned + carry`; supply bound `2·S0·H_half + S_tail·2^64 < 2^127` in `genesis_builder.rs:69-72` and the ParamGate supply rule.
- **Files:** `issuance.rs` (new, inside the boundary by default), `apply.rs`, `genesis_builder.rs`, `lp1-node/src/gate.rs`, `PoColSystemV3.sol`, `system-v3.json`, `lp4-execution/tests/issuance_oracle.py` (extend), vectors.
- **Tests:** vectors at h = 1, H_half−1, H_half, 256·H_half; host credit = recorded pot every block over 3·H_half on a compressed fixture (small H_half); T17 with carry; fuzz C from 1 to S_max+1 plus late shares; "changing LS inclusion in j+1..j+D does not change pot j+1 once block j+1 is fixed"; tip split conservation; old-chain replay byte-identical.
- **Lock:** `neutral-vector`. **Effort:** L. **Risk:** medium (value conservation; the carry/late-share coupling is the principles-review finding 4).

### V3-PR-E4 — Contestable seats and probation pool — V3-10, V3-14 (contest)
- **Scope:** per-record epoch counters `(epochIndex, countCur, countPrev)` updated in `recordShares` and `recordLate`, epoch totals; `registerContest(reward, victim)` only when active == M_max; victim live, active, past grace, active for the whole previous epoch (registration/reactivation ≤ first block of previous epoch), `countPrev < E − k_sigma·√E` with `E = theta·Tprev/active` (integer sqrt, documented rounding); victim → exiting, refund after U; same statistic inside the probation pool restricted to ACCRUING victims (accrued bond refunded, record erased, ACC_COUNT updated). Wallet UI for contest.
- **Files:** `PoColSystemV3.sol`, `system-v3.json`, `REGISTRY-LAYOUT.md`, `pocold/src/wallet.rs`, `v3_model.py`, vectors.
- **Depends on:** E1 (V3-09 is a prerequisite of V3-14), E3 (V3-02 is a hard prerequisite of V3-10).
- **Tests:** full set + victim below threshold → success, refund after U; above threshold / in grace / set not full / joined after start of previous epoch → revert; pool fill, contest below/above, rotation race, ACC_COUNT conservation through promote/exit/displace/punish; SimNet eviction-rate study in H.
- **Lock:** `neutral-vector`. **Effort:** L. **Risk:** medium (new removal reason; code size — keep MinerRegistry < 24 KB).

### V3-PR-F — v3 profile values and resource proofs — V3-25, V3-06/07/11/12/14/18/21 values
- **Scope:** fix the v3 CP (draft in §5) in a committed profile (`deploy/mainnet/v3-profile.json` consumed by `new_network_config.py --v3` and `testnet.rs` overrides); GAS_ENVELOPE for every v3 SYSTEM call (opcode-level worst case, `admission.rs:86`); adversarial block-validation sweep at the v3 CP in store mode → t_exec; A13 re-run in store mode at M_max 256, raised caps, log caps, E_win/U, late-share cache (`pocold/src/resources.rs`, `lp4-execution/src/resources.rs`); ParamGate R1–R26 + the new rules at the v3 CP and the WAN locals measured by N1; Δ_rel default 13.
- **Files:** `lp4-execution/src/admission.rs`, `lp4-execution/src/resources.rs`, `pocold/src/resources.rs`, `lp1-node/src/gate.rs`, `deploy/mainnet/{new_network_config.py,v3-profile.json,test_new_network_config.py}`, `development/audit-2026-10-22/A13-DERIVATION.md` (update).
- **Tests:** admission of the v3 profile (store mode) passes; boot refuses a profile violating each new rule; A13 budget ≤ 6 GiB (`node.rs:85`) with margin; worst-case SYS gas ≤ G_SYS.
- **Lock:** GAS_ENVELOPE is in `admission.rs` (boundary): `neutral gate=spec:2` (boot admission only). Resource modules are excluded (`-lp4-execution/src/resources.rs`, `=pocold::resources`, `=lp4::gate`).
- **Depends on:** all E PRs (final programs), N1 measurement (Δ, φ). **Effort:** M–L. **Risk:** medium (if WAN Δ > 2 s, O-6 falls back to 60 s and every block-count value is recomputed — cheap since values are expressed in seconds).

### V3-PR-G — Off-node verifiers and clients — V3-34 verifier hygiene, PR-7 of the design
- **Scope:** `deploy/rewards/pocol_rewards.py` (spec v2 decode, `v3_active`, all-live membership, late shares, issuance M3, carry, tip split, offender-only burn, ρ); `pocol-extension/src/lib/refdec.js` (gsVersion 2, CP v2 schema) with new decoder-corpus items (release verdicts unchanged until I); `rp.js` (spec version, unchanged nMax rule); `forks.js` unchanged (MAX_FORK_VERSION stays 2); `deploy/mainnet/{preflight.py,network_identity.py}` and `deploy/public-testnet/pocol_identity.py` for a spec-2 genesis; Windows/Android/extension wallets: bond, top-up, probation, contest flows against the v3 programs; `lp3-model` CP schema (optional, its CI only runs on the legacy branch).
- **Files:** as listed; tests `deploy/rewards/test_pocol_rewards.py`, `pocol-extension/tests/*`, `deploy/mainnet/test_*.py`.
- **Tests:** gateway reproduces owners and payouts block by block on a v3 devnet chain produced by the preview build (differential against node receipts/escrow state); decoder corpus parity Rust vs JS; v1 gateway outputs unchanged on the H15000 fixture.
- **Lock:** none (outside the boundary), except new `pocold/tests/decoder_reference.rs` corpus items (not a vector dir).
- **Effort:** M–L. **Risk:** medium (four implementations of the reward rule must agree).

### V3-PR-H — Simulations and test vectors
- **Scope:** a deterministic in-process SimNet (`pocold/tests/simnet/`: N `Chain`+builder instances, virtual clock, latency/withholding injectors, re-using `devnet::build`) — none exists today; scenarios: **T10b(iv)** withholding proposer + pre-mined fallback coalition at b ∈ {0.2, 0.35, 0.5} (empty-block rate, extra reward; V3-04); **T7c(S)** share omission at b ∈ {0.3, 0.5} with late credit (V3-02); **T10 with ASERT** carry and burn (V3-15/19); contest eviction rate of honest 0.1–1 % members under a persistent prober and an omitting cartel (V3-10); T7/T7b re-run at M_max 256 (identity splitting, V3-07); Python Monte-Carlo cross-checks of the closed-form gains in V3-DESIGN §4 (V3-04 table). Consolidated `tests/data/v3-vectors.json` covering every new validity rule (positive and negative), consumed by Rust and by `pocol_rewards.py`.
- **Lock:** `neutral-vector` for the vector file; the harness lives in `pocold/tests` (test-only).
- **Depends on:** C2, E3, E2a, E4. **Effort:** L. **Risk:** medium (harness fidelity; results may re-open O-6/O-10 parameters).

### V3-PR-I — Activation (SPEC_VERSION 2)
- **Scope:** `SPEC_VERSION = 2` in both `genesis_spec.rs` copies (decoder accepts 1..=2); remove the preview-only acceptance (keep the feature as a no-op or delete it); `refdec.js` `SPEC_VERSION`/corpus verdicts (`decoder-verdicts.json`) regenerated; VA-SPEC/consensus.md state "spec 2 = v3"; freeze note.
- **Lock:** `neutral`, `POCOL_REPIN_GATE=spec:2` → **spends** spec:2. After this: corrections via `spec:3` (bump SPEC_VERSION, regenerate the genesis) or `bump` only after H15000 retirement.
- **Tests:** release build decodes and admits the v3 profile; every spec-1 genesis hash unchanged; H15000 identity unchanged; full v3 suite without `--features v3-preview`.
- **Depends on:** F, G (refdec/gateway), H green. **Effort:** S. **Risk:** low in code, high in process (gate spent; any later v3 change costs a spec bump).

### V3-PR-J — Cutover rehearsal and WAN soak
- **Scope:** `deploy/mainnet/rehearse_cutover.py --local --v3` (old continuous network → v3 genesis, specVersion 2, v3 programs, cp[16] bond, report checks), runbook update (`EARN-IN-CUTOVER-RUNBOOK.md` → v3 cutover, V3-33 conduct: no user txs after S, published old-chain data, Android path, last-owner-only commitment); the pending real-server rehearsal (`development/earnin-rollout-2026-11-07/local-rehearsal-reviewed.json:11,15`); multi-host WAN soak with injected latency on the v3 profile (fallback ratio, template latency, reorg depth).
- **Lock:** none (deploy scripts, docs). **Effort:** M + calendar time. **Risk:** medium (operational).

### Parallel tracks (no ordering constraint against the consensus chain except as noted)
- **N1 V3-05** Δ_rel derivation + per-proposer template latency counters (`testnet.rs:50-58`, launcher defaults, `node.rs`, `rpc.rs`). Node-local, no lock line (unless `chain.rs` is touched). **Land early**: it is the instrument for the V3-06 WAN measurement feeding F. S.
- **N2 V3-26** persisted `finalized_h`, floor `max(head−r_keep, finalized_h)` in `chain.rs::retain` and node open/replay, telemetry `pocol_finality`, docs. `chain.rs` is a boundary file: `neutral` line with note "local retention policy, like r_keep (excluded from identity)". Tests: depth-100 heavier fork keeps finalized; heavier-shorter branch then deep fork; restart. S–M. Rebase on #8.
- **N3 V3-30** `--mine-threads N` (disjoint slices of the own range), localhost `pocol_getWork/submitWork` (off by default, own-range check), `networkHashrateEst`; A13 io_threads. Node-local. S–M.
- **N4 V3-20 pool + V3-24 RPC**: fee-ordered pool, replace-by-fee, lowest-fee eviction on top of #7's quota/parking; `eth_estimateGas` models log caps. Node-local (`node.rs`, `rpc.rs`; `lp4-execution/src/pool.rs` is excluded). M.
- **N5 V3-28 H1** computed `launch_ready` + `launchBlockers` replacing the literal `publicLaunchReady:false` (`launcher.rs:83`, `build.rs:101`, `testnet.rs:235`). `build.rs` is a boundary file → `neutral` line. S.
- **N6 V3-33 node-local**: builder flag "include no user transactions after height S" (+ launcher/RPC switch), Android key-export / new-applicationId path (`claude/client-key-backup`, task 14), old-chain data export. Must ship in a release **running on H15000 before S** (uses #6's upgrade path). S–M.
- **K V3-23** in-place code/CP replacement machinery: `cp_at(h)` across ~105 `.cp()` sites, install runtimes at H via `WorkState::insert_info`, height-aware `fixed_accounts`/`for_spec`, ASERT re-anchor and D/W_w/subsidy transitions, CODE_CAP migration, SYS-cap exclusion of irregular writes. Reachable only through a schedule entry of version ≥ 3 → `neutral gate=fork:3` with MAX_FORK_VERSION left at 2. Tests with `new_bounded` schedules: devnet swap at H, replay and reorg across H, old-vs-new differential on live-shaped state. L, high risk. Off the critical path: land after V3-PR-I if it would delay the genesis (dormant code needs no genesis); O-0=a asks for it in release B, so target B and accept B+1 as fallback.

### Dependency graph and critical path

```
#8, #9 (open) ──► B
A ─────────────► B ─► C1, C2, C3, E0
                     E0 ─► E1 ─► E2b ─► E3 ─► E2a ─► E4
                     (E1 ─► E4 ; E3 ─► E4)
N1 (telemetry) ─► WAN measurement ─► F
C*, E* ─► F ─► I ◄─ G ◄─ (E2a, E3, E4)
C2, E3, E2a, E4 ─► H ─► I ─► J ─► genesis + cutover
N6 must be released on H15000 before S;  K anytime (before the first in-place fork)
```

Critical path ≈ A (M) → B (L) → E0 (M) → E1 (M) → E2b (M) → E3 (L) → E2a (L) → E4 (L) → H (L) → F (M–L) → I (S) → J (M):
roughly 75–105 engineer-days sequential; C1–C3, G, N1–N6 and K run in parallel.

---

## 5. Draft v3 CP (to be fixed in V3-PR-F; O-6=b assumed, T_blk = 30 s)

| Field | Value | Source |
|---|---|---|
| T_blk / D_att / dFbWait / Δ_rel (local) | 30 / 150 / 10 / 13 | O-6=b, valid iff measured WAN Δ ≤ 2 s and φ ≤ 2 s; else 60 / 300 / 25 / 26 |
| nonceMode / H_END | 1 / 0 | earn-in, continuous |
| M_max / R_max / REG_OPS | 256 / 4 / 16 | V3-07, V3-09 |
| R_react / C_react | 4 / ≥ U (e.g. 160) | V3-09 |
| B_reg | 20 PCOL | O-12=a |
| P_MAX / T_PROB | 64 / 86,400 blocks (30 days) | O-14=b |
| Θ_inact | 120 blocks (1 h) | O-11=b |
| E_win / U | 120 / 140 (= E_win + D + 16) | O-18=b; R7, U ≥ W_act+2 |
| W_act / D / W_w | 32 / 4 / 8 (unchanged unless SimNet says otherwise) | — |
| S0 (cp[15]) / H_half / S_tail | 1 PCOL / 4,207,680 blocks (4 y) / S0/32 = 0.03125 PCOL | O-19=M3 |
| γ_tip_bp / baseFeeMin | 5000 / 10^7 wei (0.01 gwei) | O-20=b |
| ρ_bp | 2500 | O-17=b |
| E_epoch / θ_bp / k_sigma | 2,880 blocks (1 day) / 1250 / 3 | O-10=b |
| ACCT_CAP / SLOT_CAP / CODE_CAP | 200,000 / 500,000 / 32 MiB (A13-verified in F) | O-21=b |
| F0_acct / F0_slot / F0_code | 10^15 wei (0.001 PCOL) / TBD / TBD (F) | O-21=b |
| LOG_BLOCK_MAX / LOG_COUNT_MAX (host constants) | 256 KiB / 4,368 | V3-24 |

---

## 6. Risks and open points

1. **Gate accounting.** The dormant-gate approach depends on nobody raising `SPEC_VERSION` before V3-PR-I. Add a guard
   test (or a CODEOWNERS rule) that `SPEC_VERSION` changes only in a PR whose line carries `gate=spec:<new>`.
2. **Preview builds** are consensus-affecting; the identity bit + appended aggregate + simnet-only admission +
   package refusal must all land together in B, or a preview node could peer with a pinned node on a spec-1 chain
   (it cannot disagree there, but the guarantee should not depend on that).
3. **Vector-dir changes are "presumed bump"** by policy; every E-PR review needs an explicit reachability statement.
4. **Rebase churn** with #8 (chain.rs slim/unslim/journal, lock n=2..4) and #9 (node.rs/net.rs): land both first.
5. **V3-04 restart semantics**: persisting first-import time is an addition to the design; without it a restart can
   change the head among equal-work siblings.
6. **V3-02 + #8 slimming**: evidence of kind 3 reads `e.ranges` of ancestors up to D back; slimmed entries must be
   restorable (cutover `rebuild_at` already handles E_win-deep restores for types 1/2; extend to kind 3).
7. **O-6 fallback** to 60 s if WAN Δ > 2 s: all block counts in §5 roughly halve; T_PROB and H_half are expressed in
   seconds in the profile so the change is mechanical.
8. **Four reward implementations** (Solidity, Rust host, `pocol_rewards.py`, wallet displays) must agree; the
   differential in G is mandatory before I.
9. **CI gaps**: no main-branch reproduction of system bytecode and no main CI for the earn-in contract/chain tests;
   fixed in E0. The reproduction depends on the npm registry or cache for `solc@0.8.28` (works offline here from cache).
10. **Code size**: MinerRegistry grows with E1 + E4; track runtime size in the E0 CI job (fail above 24,576 B).
11. **Future in-place forks on the v3 chain**: the v3 genesis schedule is `[(1,0)]`; the first in-place fork appends
    `(2,H)` (no-op under nonceMode 1) and `(3,H')`, raises MAX_FORK_VERSION to 3 and spends `fork:3`. Because
    `v3_active` already contains `active_version >= 3`, fork 3 on a spec-1 chain would also enable all v3 rules — moot
    once H15000 is retired, but document it (A).
12. **V3-33 builder flag and Android export** must be released on H15000 before S; they are on no consensus path but
    gate the cutover date.
